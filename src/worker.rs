//! Фоновый поток: опрашивает датчики и sysfs, общается с контроллером
//! подсветки и выполняет команды UI. Результаты уходят в сигналы
//! (`set()` потокобезопасен); читать сигналы отсюда нельзя.

use crate::autoapply::{self, Saved, ServiceState};
use crate::hw::battery::{self, Battery, ChargeType};
use crate::hw::device::{self, SystemInfo, Toggle, Toggles};
use crate::hw::fans::{self, Fan};
use crate::hw::power::{self, PowerMode, PowerState, Tunable};
use crate::hw::sensors::{Gpu, Sampler, Sensors};
use crate::hw::sysfs;
use crate::spectrum::device::{Error as KbError, KeyMatrix, Keyboard};
use crate::spectrum::protocol::{Effect, Rgb};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use syngui::prelude::*;

/// Разделы, от которых зависит частота опроса.
pub mod page {
    pub const HOME: u8 = 0;
    pub const LIGHTING: u8 = 2;
}

const SYSFS_POLL: Duration = Duration::from_millis(1000);
/// Видеокарту опрашиваем реже, на батарее — ещё реже.
const GPU_POLL_AC: Duration = Duration::from_secs(2);
const GPU_POLL_BATTERY: Duration = Duration::from_secs(10);
const RGB_FRAME: Duration = Duration::from_millis(100);
const RGB_STATUS: Duration = Duration::from_millis(1500);
const RGB_RETRY: Duration = Duration::from_secs(3);

pub enum Job {
    SetPowerMode(PowerMode),
    SetTunable(String, i64),
    SetFanTarget(u8, u32),
    SetChargeType(ChargeType),
    SetToggle(Toggle, bool),
    RgbProfile(u8),
    RgbBrightness(u8),
    RgbLogo(bool),
    /// Перечитать слои профиля в `rgb_effects`.
    RgbLoad(u8),
    RgbWrite(u8, Vec<Effect>),
    RgbReset(u8),
    /// Включить/выключить службу автоприменения.
    SetAutoApply(bool),
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum RgbLink {
    #[default]
    Searching,
    NotFound,
    NoAccess(String),
    Error(String),
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RgbState {
    pub link: RgbLink,
    pub name: String,
    pub profile: u8,
    pub brightness: u8,
    pub logo: bool,
    pub matrix: KeyMatrix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    Success,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub kind: NoticeKind,
    pub text: String,
    /// Растёт с каждым уведомлением — одинаковый текст всё равно покажется.
    pub seq: u64,
}

/// Сигналы, в которые пишет поток.
#[derive(Clone, Copy)]
pub struct Sink {
    pub sensors: RwSignal<Sensors>,
    pub power: RwSignal<PowerState>,
    pub tunables: RwSignal<Vec<Tunable>>,
    pub fans: RwSignal<Vec<Fan>>,
    pub battery: RwSignal<Option<Battery>>,
    pub toggles: RwSignal<Toggles>,
    pub info: RwSignal<SystemInfo>,
    pub rgb: RwSignal<RgbState>,
    pub rgb_frame: RwSignal<Vec<(u16, Rgb)>>,
    /// Слои профиля, как они записаны в контроллере: (профиль, слои).
    pub rgb_effects: RwSignal<Option<(u8, Vec<Effect>)>>,
    pub busy: RwSignal<bool>,
    pub notice: RwSignal<Option<Notice>>,
    /// Служба автоприменения и сохранённые значения режима «Свой».
    pub autoapply: RwSignal<(ServiceState, Saved)>,
}

pub fn spawn(sink: Sink, simulate: bool, page: Arc<AtomicU8>, rx: Receiver<Job>) {
    std::thread::Builder::new()
        .name("legion-worker".into())
        .spawn(move || {
            let mut w = Worker {
                sink,
                simulate,
                page,
                sampler: Sampler::default(),
                kb: None,
                matrix: KeyMatrix::default(),
                last_gpu: Gpu::Absent,
                seq: 0,
                last_rgb_try: None,
            };
            w.run(rx)
        })
        .expect("не удалось запустить фоновый поток");
}

struct Worker {
    sink: Sink,
    simulate: bool,
    page: Arc<AtomicU8>,
    sampler: Sampler,
    kb: Option<Keyboard>,
    /// Матрица зон не меняется, пока контроллер подключён.
    matrix: KeyMatrix,
    last_gpu: Gpu,
    seq: u64,
    last_rgb_try: Option<Instant>,
}

impl Worker {
    fn run(&mut self, rx: Receiver<Job>) {
        self.sink.info.set(device::read_info());
        self.refresh_sysfs();
        self.refresh_autoapply();
        let mut next_sysfs = Instant::now();
        let mut next_gpu = Instant::now();
        let mut next_status = Instant::now();
        let mut next_frame = Instant::now();
        loop {
            let now = Instant::now();
            let page = self.page.load(Ordering::Relaxed);

            if now >= next_sysfs {
                let gpu = page == page::HOME && now >= next_gpu;
                let on_ac = battery::read_battery().is_none_or(|b| b.ac_online);
                if gpu {
                    next_gpu = now + if on_ac { GPU_POLL_AC } else { GPU_POLL_BATTERY };
                }
                let mut s = self.sampler.read(gpu);
                if !gpu {
                    // Сохраняем прошлое состояние видеокарты до следующего опроса.
                    s.gpu = self.last_gpu.clone();
                }
                self.last_gpu = s.gpu.clone();
                self.sink.sensors.set(s);
                self.refresh_sysfs();
                next_sysfs = now + SYSFS_POLL;
            }

            if self.kb.is_none() && self.last_rgb_try.is_none_or(|t| t.elapsed() >= RGB_RETRY) {
                self.connect_rgb();
            }
            if self.kb.is_some() && now >= next_status {
                self.rgb_status();
                next_status = now + RGB_STATUS;
            }
            if self.kb.is_some() && page == page::LIGHTING && now >= next_frame {
                self.rgb_frame();
                next_frame = now + RGB_FRAME;
            }

            let wait = if page == page::LIGHTING && self.kb.is_some() { RGB_FRAME } else { Duration::from_millis(250) };
            match rx.recv_timeout(wait) {
                Ok(job) => self.handle(job),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn notify(&mut self, kind: NoticeKind, text: impl Into<String>) {
        self.seq += 1;
        self.sink.notice.set(Some(Notice { kind, text: text.into(), seq: self.seq }));
    }

    fn refresh_sysfs(&mut self) {
        self.sink.power.set(power::read_state());
        self.sink.tunables.set(power::read_tunables());
        self.sink.fans.set(fans::read_fans());
        self.sink.battery.set(battery::read_battery());
        self.sink.toggles.set(device::read_toggles());
    }

    fn refresh_autoapply(&mut self) {
        let service = if self.simulate { ServiceState::default() } else { autoapply::service_state() };
        self.sink.autoapply.set((service, autoapply::load()));
    }

    fn sysfs_result(&mut self, r: sysfs::Result<()>, ok: String) {
        match r {
            Ok(()) => self.notify(NoticeKind::Success, ok),
            Err(sysfs::Error::Rejected(..)) if power::read_state().mode != Some(PowerMode::Custom) => {
                self.notify(NoticeKind::Error, "Драйвер принимает это значение только в режиме «Свой»")
            }
            Err(e) => self.notify(NoticeKind::Error, e.to_string()),
        }
        self.refresh_sysfs();
    }

    fn handle(&mut self, job: Job) {
        match job {
            Job::SetPowerMode(m) => {
                let r = power::set_mode(m);
                self.sysfs_result(r, format!("Режим питания: {}", m.label()));
            }
            Job::SetTunable(name, v) => {
                let label = self.label_of(&name);
                let r = power::set_tunable(&name, v);
                if r.is_ok() {
                    autoapply::remember_tunable(&name, v);
                    self.refresh_autoapply();
                }
                self.sysfs_result(r, format!("{label}: {v}"));
            }
            Job::SetFanTarget(i, rpm) => {
                let r = fans::set_target(i, rpm);
                if r.is_ok() {
                    autoapply::remember_fan(i, rpm);
                    self.refresh_autoapply();
                }
                let text = if rpm == 0 { "авто".to_string() } else { format!("{rpm} об/мин") };
                self.sysfs_result(r, format!("Вентилятор {i}: {text}"));
            }
            Job::SetChargeType(t) => {
                let r = battery::set_charge_type(t);
                self.sysfs_result(r, format!("Режим зарядки: {}", t.label()));
            }
            Job::SetToggle(t, on) => {
                let r = device::set_toggle(t, on);
                let state = if on { "включено" } else { "выключено" };
                self.sysfs_result(r, format!("{}: {state}", t.label()));
            }
            Job::RgbProfile(n) => self.rgb_cmd(|kb| kb.set_profile(n), format!("Профиль подсветки {n}"), Some(n)),
            Job::RgbBrightness(v) => self.rgb_cmd(|kb| kb.set_brightness(v), String::new(), None),
            Job::RgbLogo(on) => self.rgb_cmd(
                |kb| kb.set_logo(on),
                format!("Логотип {}", if on { "включён" } else { "выключен" }),
                None,
            ),
            Job::RgbLoad(n) => self.load_effects(n),
            Job::RgbWrite(n, effects) => {
                self.sink.busy.set(true);
                self.rgb_cmd(|kb| kb.set_effects(n, &effects), format!("Эффекты записаны в профиль {n}"), Some(n));
                self.sink.busy.set(false);
            }
            Job::SetAutoApply(on) => {
                match autoapply::set_service(on) {
                    Ok(()) if on => self.notify(NoticeKind::Success, "Автоприменение включено"),
                    Ok(()) => self.notify(NoticeKind::Success, "Автоприменение выключено"),
                    Err(e) => self.notify(NoticeKind::Error, format!("Служба автоприменения: {e}")),
                }
                self.refresh_autoapply();
            }
            Job::RgbReset(n) => {
                self.rgb_cmd(|kb| kb.reset_profile(n), format!("Профиль {n} сброшен к заводскому"), Some(n))
            }
        }
    }

    fn label_of(&self, name: &str) -> String {
        power::read_tunables().into_iter().find(|t| t.name == name).map(|t| t.label).unwrap_or_else(|| name.into())
    }

    // --- подсветка ---------------------------------------------------------

    fn connect_rgb(&mut self) {
        self.last_rgb_try = Some(Instant::now());
        let res = Keyboard::open(self.simulate).and_then(|mut kb| {
            let m = kb.key_matrix()?;
            Ok((kb, m))
        });
        match res {
            Ok((kb, matrix)) => {
                let name = kb.name();
                self.kb = Some(kb);
                self.matrix = matrix.clone();
                self.sink.rgb.set(RgbState { link: RgbLink::Ready, name, matrix, ..Default::default() });
                self.rgb_status();
            }
            Err(e) => self.rgb_fail(e),
        }
    }

    fn rgb_fail(&mut self, e: KbError) {
        self.kb = None;
        let link = match e {
            KbError::NotFound => RgbLink::NotFound,
            KbError::Permission(p) => RgbLink::NoAccess(p),
            e => RgbLink::Error(e.to_string()),
        };
        self.sink.rgb.set(RgbState { link, ..Default::default() });
    }

    /// Профиль, яркость и логотип могут поменяться с клавиатуры (Fn+Пробел).
    fn rgb_status(&mut self) {
        let Some(kb) = self.kb.as_mut() else { return };
        let res = (|| Ok::<_, KbError>((kb.profile()?, kb.brightness()?, kb.logo()?)))();
        match res {
            Ok((profile, brightness, logo)) => {
                let name = kb.name();
                let matrix = self.matrix.clone();
                let st = RgbState { link: RgbLink::Ready, name, profile, brightness, logo, matrix };
                self.sink.rgb.set(st);
            }
            Err(e) => self.rgb_fail(e),
        }
    }

    fn rgb_frame(&mut self) {
        let Some(kb) = self.kb.as_mut() else { return };
        match kb.state() {
            Ok(Some(frame)) => self.sink.rgb_frame.set(frame),
            Ok(None) => {}
            Err(e) => self.rgb_fail(e),
        }
    }

    fn load_effects(&mut self, n: u8) {
        let Some(kb) = self.kb.as_mut() else { return };
        match kb.effects(n) {
            Ok(e) => self.sink.rgb_effects.set_always(Some((n, e))),
            Err(e) => {
                self.notify(NoticeKind::Error, format!("Не удалось прочитать профиль {n}: {e}"));
            }
        }
    }

    fn rgb_cmd(&mut self, f: impl FnOnce(&mut Keyboard) -> crate::spectrum::device::Result<()>, ok: String, reload: Option<u8>) {
        let Some(kb) = self.kb.as_mut() else {
            return self.notify(NoticeKind::Error, "Контроллер подсветки не подключён");
        };
        match f(kb) {
            Ok(()) => {
                if !ok.is_empty() {
                    self.notify(NoticeKind::Success, ok);
                }
                std::thread::sleep(Duration::from_millis(60));
                self.rgb_status();
                if let Some(n) = reload {
                    self.load_effects(n);
                }
            }
            Err(e) => {
                self.notify(NoticeKind::Error, format!("Подсветка: {e}"));
                if matches!(e, KbError::NotFound | KbError::Io(_)) {
                    self.rgb_fail(e);
                }
            }
        }
    }
}


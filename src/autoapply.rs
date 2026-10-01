//! Служба `linux-legion --daemon` и её настройки (`~/.config/linux-legion/custom.conf`).
//!
//! Что делает служба:
//! * **режим «Свой»** — восстанавливает сохранённые лимиты и обороты при
//!   входе в систему, при переключении в «Свой» (Fn+Q) и после сна: прошивка
//!   держит их только до перезагрузки, а обороты — и до сна;
//! * **лимиты CPU по режиму (RAPL)** — BIOS Legion Pro 7 Gen 10 при загрузке
//!   ставит в RAPL MMIO 30/30 Вт и дальше их не меняет; в Windows значения
//!   режимов пишет Legion Space, в Linux — эта служба: Тихий 55/65,
//!   Баланс 90/125, Производительность 145/190, Экстрим 160/205, «Свой» —
//!   из лимитов прошивки;
//! * **EPP по режиму** — `performance` в Производительность/Экстрим/Свой,
//!   `balance_performance` в Балансе, `balance_power` в Тихом;
//! * **программное авто вентиляторов** — EC этой модели после ручного
//!   задания оборотов больше не возвращается к своей кривой (ни по «0», ни
//!   по смене режима, ни после сна или тёплой перезагрузки — только сброс
//!   EC). Служба ведёт вентиляторы по температуре сама.
//!
//! ```text
//! tunable.ppt_pl1_spl=95
//! fan.1=3000          # 0 — авто
//! fan.curve=1         # программное авто
//! cpu.rapl=1          # лимиты CPU по режиму
//! cpu.epp=0           # EPP по режиму
//! rapl.performance=150/190   # свои PL1/PL2 для режима
//! ```

use crate::hw::cpufreq;
use crate::hw::fans::{self, Fan};
use crate::hw::power::{self, PowerMode};
use crate::hw::rapl::{self, Limits};
use crate::hw::sensors::{Gpu, Sampler};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub const UNIT: &str = "linux-legion-autoapply.service";

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Переадресовать каталог настроек (демо-режим не трогает настоящие).
pub fn set_dir(dir: PathBuf) {
    let _ = DIR.set(dir);
}

fn dir() -> PathBuf {
    DIR.get().cloned().unwrap_or_else(|| {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("linux-legion")
    })
}

fn path() -> PathBuf {
    dir().join("custom.conf")
}

/// Переключатели службы.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    /// Вентиляторы по температуре (программное авто).
    FanCurve,
    /// Лимиты CPU (RAPL) по режиму.
    Rapl,
    /// EPP по режиму.
    Epp,
}

impl Flag {
    fn key(self) -> &'static str {
        match self {
            Flag::FanCurve => "fan.curve",
            Flag::Rapl => "cpu.rapl",
            Flag::Epp => "cpu.epp",
        }
    }
}

/// Сохранённые настройки.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub tunables: BTreeMap<String, i64>,
    /// Номер вентилятора → об/мин (0 — авто).
    pub fans: BTreeMap<u8, u32>,
    pub fan_curve: bool,
    pub rapl: bool,
    pub epp: bool,
    /// Свои PL1/PL2 для режимов (поверх `PowerMode::cpu_limits`).
    pub rapl_modes: BTreeMap<String, Limits>,
}

impl Default for Saved {
    fn default() -> Self {
        Saved {
            tunables: BTreeMap::new(),
            fans: BTreeMap::new(),
            fan_curve: false,
            rapl: true,
            epp: false,
            rapl_modes: BTreeMap::new(),
        }
    }
}

impl Saved {
    fn parse(text: &str) -> Self {
        let mut s = Saved::default();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.split('#').next().unwrap_or("").trim());
            let flag = |s: &mut bool| {
                if let Ok(n) = v.parse::<u8>() {
                    *s = n != 0;
                }
            };
            if let Some(name) = k.strip_prefix("tunable.") {
                if let Ok(v) = v.parse() {
                    s.tunables.insert(name.to_string(), v);
                }
            } else if k == Flag::FanCurve.key() {
                flag(&mut s.fan_curve);
            } else if k == Flag::Rapl.key() {
                flag(&mut s.rapl);
            } else if k == Flag::Epp.key() {
                flag(&mut s.epp);
            } else if let Some(i) = k.strip_prefix("fan.") {
                if let (Ok(i), Ok(v)) = (i.parse(), v.parse()) {
                    s.fans.insert(i, v);
                }
            } else if let Some(mode) = k.strip_prefix("rapl.") {
                if let Some((a, b)) = v.split_once('/') {
                    if let (Ok(pl1), Ok(pl2)) = (a.trim().parse(), b.trim().parse()) {
                        if PowerMode::from_key(mode).is_some() {
                            s.rapl_modes.insert(mode.to_string(), Limits { pl1, pl2 });
                        }
                    }
                }
            }
        }
        s
    }

    fn render(&self) -> String {
        let mut out = String::from("# linux-legion: настройки службы и режима «Свой»\n");
        for (k, v) in &self.tunables {
            out.push_str(&format!("tunable.{k}={v}\n"));
        }
        for (k, v) in &self.fans {
            out.push_str(&format!("fan.{k}={v}\n"));
        }
        out.push_str(&format!("{}={}\n", Flag::FanCurve.key(), self.fan_curve as u8));
        out.push_str(&format!("{}={}\n", Flag::Rapl.key(), self.rapl as u8));
        out.push_str(&format!("{}={}\n", Flag::Epp.key(), self.epp as u8));
        for (m, l) in &self.rapl_modes {
            out.push_str(&format!("rapl.{m}={}/{}\n", l.pl1, l.pl2));
        }
        out
    }

    /// Лимиты CPU для режима: свои из конфига, иначе заводские Lenovo;
    /// для «Свой» — текущие атрибуты прошивки.
    pub fn limits_for(&self, mode: PowerMode) -> Option<Limits> {
        if let Some(l) = self.rapl_modes.get(mode.key()) {
            return Some(*l);
        }
        match mode.cpu_limits() {
            Some((pl1, pl2)) => Some(Limits { pl1, pl2 }),
            None => {
                let t = power::read_tunables();
                let get = |n: &str| t.iter().find(|t| t.name == n).map(|t| t.value as u32);
                let pl1 = get("ppt_pl1_spl")?;
                Some(Limits { pl1, pl2: get("ppt_pl2_sppt").unwrap_or(pl1).max(pl1) })
            }
        }
    }
}

pub fn load() -> Saved {
    std::fs::read_to_string(path()).map(|t| Saved::parse(&t)).unwrap_or_default()
}

fn store(s: &Saved) -> std::io::Result<()> {
    std::fs::create_dir_all(dir())?;
    let tmp = path().with_extension("tmp");
    std::fs::write(&tmp, s.render())?;
    std::fs::rename(tmp, path())
}

/// Запомнить значение, которое пользователь применил в режиме «Свой».
pub fn remember_tunable(name: &str, value: i64) {
    let mut s = load();
    s.tunables.insert(name.to_string(), value);
    let _ = store(&s);
}

pub fn remember_fan(index: u8, rpm: u32) {
    let mut s = load();
    s.fans.insert(index, rpm);
    let _ = store(&s);
}

pub fn remember_flag(flag: Flag, on: bool) {
    let mut s = load();
    match flag {
        Flag::FanCurve => s.fan_curve = on,
        Flag::Rapl => s.rapl = on,
        Flag::Epp => s.epp = on,
    }
    let _ = store(&s);
}

/// Применить сохранённое для режима «Свой» (лимиты прошивки и обороты).
/// Обороты пишутся всегда: `fanN_target` драйвера — кэш, после перезагрузки
/// он показывает 0, хотя EC может помнить старое значение.
pub fn apply_custom() -> Result<Vec<String>, String> {
    if power::read_state().mode != Some(PowerMode::Custom) {
        return Ok(Vec::new());
    }
    let saved = load();
    let mut done = Vec::new();
    let mut errors = Vec::new();
    let current: BTreeMap<String, i64> = power::read_tunables().into_iter().map(|t| (t.name, t.value)).collect();
    for (name, v) in &saved.tunables {
        if current.get(name) == Some(v) || !current.contains_key(name) {
            continue;
        }
        match power::set_tunable(name, *v) {
            Ok(()) => done.push(format!("{name}={v}")),
            Err(e) => errors.push(e.to_string()),
        }
    }
    if !saved.fan_curve {
        let present: Vec<u8> = fans::read_fans().into_iter().map(|f| f.index).collect();
        for (i, rpm) in saved.fans.iter().filter(|(i, _)| present.contains(i)) {
            match fans::set_target(*i, *rpm) {
                Ok(()) => done.push(format!("fan{i}={rpm}")),
                Err(e) => errors.push(e.to_string()),
            }
        }
    }
    if errors.is_empty() {
        Ok(done)
    } else {
        Err(errors.join("; "))
    }
}

/// Лимиты CPU и EPP под режим (если включены). Пишет только отличия.
pub fn apply_cpu(mode: PowerMode) -> Result<Vec<String>, String> {
    let saved = load();
    let mut done = Vec::new();
    let mut errors = Vec::new();
    if saved.rapl && rapl::read().present() {
        match saved.limits_for(mode) {
            Some(l) => match rapl::set(l) {
                Ok(d) => done.extend(d),
                Err(e) => errors.push(e.to_string()),
            },
            None => errors.push("лимиты «Свой»: атрибуты прошивки не найдены".into()),
        }
    }
    if saved.epp && cpufreq::read().current.is_some() {
        match cpufreq::set(mode.epp()) {
            Ok(true) => done.push(format!("epp={}", mode.epp())),
            Ok(false) => {}
            Err(e) => errors.push(e.to_string()),
        }
    }
    if errors.is_empty() {
        Ok(done)
    } else {
        Err(errors.join("; "))
    }
}

/// `--apply`: всё разом.
pub fn apply() -> Result<Vec<String>, String> {
    let mut done = apply_custom()?;
    if let Some(mode) = power::read_state().mode {
        done.extend(apply_cpu(mode)?);
    }
    Ok(done)
}

/// Вернуть вентиляторы EC («0 = авто» по документации драйвера). Нужно при
/// выходе из «Свой» без программного авто: EC Legion хранит ручные обороты
/// и в других режимах.
pub fn release_fans() -> Vec<String> {
    fans::read_fans()
        .iter()
        .filter(|f| f.writable)
        .filter_map(|f| fans::set_target(f.index, 0).ok().map(|_| format!("fan{}=авто", f.index)))
        .collect()
}

// ---------------------------------------------------------------------------
// Программное авто вентиляторов

/// Кривая: температура → доля диапазона [min, max].
const CURVE: [(f32, f32); 7] = [(45.0, 0.0), (55.0, 0.08), (65.0, 0.2), (75.0, 0.4), (85.0, 0.65), (92.0, 0.85), (97.0, 1.0)];

/// Выше этой температуры фиксированные обороты «Своего» не ниже кривой.
const SAFETY_TEMP: f32 = 95.0;

fn curve(temp: f32) -> f32 {
    let (mut lo, mut hi) = (CURVE[0], CURVE[CURVE.len() - 1]);
    if temp <= lo.0 {
        return lo.1;
    }
    if temp >= hi.0 {
        return hi.1;
    }
    for w in CURVE.windows(2) {
        if temp >= w[0].0 && temp <= w[1].0 {
            (lo, hi) = (w[0], w[1]);
            break;
        }
    }
    lo.1 + (hi.1 - lo.1) * (temp - lo.0) / (hi.0 - lo.0)
}

fn mode_factor(mode: Option<PowerMode>) -> f32 {
    match mode {
        Some(PowerMode::Quiet) => 0.7,
        Some(PowerMode::Performance) => 1.2,
        Some(PowerMode::Extreme) => 1.35,
        _ => 1.0,
    }
}

/// Обороты для вентилятора по температуре и режиму.
pub fn curve_rpm(fan: &Fan, temp: f32, mode: Option<PowerMode>) -> u32 {
    let frac = (curve(temp) * mode_factor(mode)).clamp(0.0, 1.0);
    let span = fan.max.saturating_sub(fan.min) as f32;
    let rpm = fan.min as f32 + span * frac;
    let step = fan.step.max(1);
    ((rpm / step as f32).round() as u32 * step).clamp(fan.min, fan.max)
}

/// Состояние программного авто.
#[derive(Default)]
struct FanCurve {
    sampler: Sampler,
    /// Последнее записанное: вентилятор → (об/мин, сколько тиков подряд хотелось ниже).
    last: BTreeMap<u8, (u32, u8)>,
    gpu_temp: Option<f32>,
}

impl FanCurve {
    /// Один шаг. Вверх — сразу, вниз — после трёх шагов подряд, чтобы
    /// обороты не дёргались.
    fn tick(&mut self, mode: Option<PowerMode>, saved: &Saved) -> Vec<String> {
        let s = self.sampler.read(true);
        let cpu = s.cpu_temp;
        match s.gpu {
            Gpu::Active(g) => self.gpu_temp = g.temp,
            Gpu::Sleeping | Gpu::Absent => self.gpu_temp = None,
        }
        let gpu = self.gpu_temp;
        let Some(cpu_t) = cpu.or(gpu) else { return Vec::new() };
        let gpu_t = gpu.unwrap_or(cpu_t);
        let mut done = Vec::new();
        for f in fans::read_fans().iter().filter(|f| f.writable && f.max > 0) {
            let temp = match f.index {
                1 => cpu_t,
                2 => gpu_t,
                _ => cpu_t.max(gpu_t),
            };
            let auto = curve_rpm(f, temp, mode);
            let fixed = (mode == Some(PowerMode::Custom)).then(|| saved.fans.get(&f.index).copied()).flatten();
            let want = match fixed {
                Some(rpm) if rpm > 0 => {
                    if temp >= SAFETY_TEMP {
                        rpm.max(auto)
                    } else {
                        rpm
                    }
                }
                _ => auto,
            };
            let (last, falling) = self.last.get(&f.index).copied().unwrap_or((u32::MAX, 0));
            let write = if last == u32::MAX || want > last {
                true
            } else if want < last {
                if falling + 1 >= 3 {
                    true
                } else {
                    self.last.insert(f.index, (last, falling + 1));
                    false
                }
            } else {
                self.last.insert(f.index, (last, 0));
                false
            };
            if write && fans::set_target(f.index, want).is_ok() {
                self.last.insert(f.index, (want, 0));
                done.push(format!("fan{}={want} ({temp:.0}°)", f.index));
            }
        }
        done
    }
}

/// Сдвиг CLOCK_BOOTTIME относительно CLOCK_MONOTONIC растёт только во сне.
fn sleep_offset() -> Duration {
    let read = |id| {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        unsafe { libc::clock_gettime(id, &mut ts) };
        Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
    };
    read(libc::CLOCK_BOOTTIME).saturating_sub(read(libc::CLOCK_MONOTONIC))
}

const CPU_PERIOD: Duration = Duration::from_secs(5);
const FAN_PERIOD: Duration = Duration::from_secs(2);

/// `linux-legion --daemon`: следит за режимом и сном, применяет сохранённое,
/// держит лимиты CPU и EPP по режиму, ведёт вентиляторы по температуре.
pub fn daemon() -> ! {
    let log = |m: String| eprintln!("linux-legion: {m}");
    let report = |why: &str, r: Result<Vec<String>, String>| match r {
        Ok(done) if !done.is_empty() => log(format!("{why}: {}", done.join(", "))),
        Ok(_) => {}
        Err(e) => log(format!("{why}: {e}")),
    };
    log("служба запущена".into());
    // Драйверам после загрузки нужна пара секунд.
    std::thread::sleep(Duration::from_secs(2));
    let mut saved = load();
    let mut conf_mtime = std::fs::metadata(path()).and_then(|m| m.modified()).ok();
    let mut mode = power::read_state().mode;
    report("старт", apply_custom());
    if let Some(m) = mode {
        report("старт", apply_cpu(m));
    }
    let mut curve = FanCurve::default();
    let mut last_sleep = sleep_offset();
    let mut next_cpu = Instant::now() + CPU_PERIOD;
    let mut next_fan = Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(1));
        // Настройки меняет UI — перечитываем по mtime.
        let mtime = std::fs::metadata(path()).and_then(|m| m.modified()).ok();
        if mtime != conf_mtime {
            conf_mtime = mtime;
            let fresh = load();
            if fresh.fan_curve && !saved.fan_curve {
                curve = FanCurve::default();
            }
            saved = fresh;
            if let Some(m) = mode {
                report("настройки", apply_cpu(m));
            }
        }
        let now_mode = power::read_state().mode;
        let slept = sleep_offset();
        if slept > last_sleep + Duration::from_secs(1) {
            // После сна EC сбрасывает обороты не сразу — чуть подождать.
            std::thread::sleep(Duration::from_secs(2));
            report("после сна", apply_custom());
            if let Some(m) = now_mode {
                report("после сна", apply_cpu(m));
            }
            curve.last.clear();
        } else if now_mode != mode {
            if now_mode == Some(PowerMode::Custom) {
                std::thread::sleep(Duration::from_millis(500));
                report("включён «Свой»", apply_custom());
            } else if mode == Some(PowerMode::Custom) && !saved.fan_curve {
                let done = release_fans();
                if !done.is_empty() {
                    log(format!("выключен «Свой»: {}", done.join(", ")));
                }
            }
            if let Some(m) = now_mode {
                report("режим", apply_cpu(m));
            }
            curve.last.clear();
            next_cpu = Instant::now() + CPU_PERIOD;
        }
        mode = now_mode;
        last_sleep = slept;

        let now = Instant::now();
        if now >= next_cpu {
            next_cpu = now + CPU_PERIOD;
            if let Some(m) = mode {
                report("лимиты", apply_cpu(m));
            }
        }
        if saved.fan_curve && now >= next_fan {
            next_fan = now + FAN_PERIOD;
            let done = curve.tick(mode, &saved);
            if !done.is_empty() {
                log(format!("вентиляторы: {}", done.join(", ")));
            }
        }
    }
}

/// Состояние службы автоприменения.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceState {
    /// Юнит установлен (пакетом).
    pub installed: bool,
    pub enabled: bool,
    pub active: bool,
}

fn systemctl(args: &[&str]) -> Option<String> {
    let out = Command::new("systemctl").arg("--user").args(args).output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn service_state() -> ServiceState {
    let enabled = systemctl(&["is-enabled", UNIT]).unwrap_or_default();
    ServiceState {
        installed: !enabled.is_empty() && enabled != "not-found",
        enabled: enabled == "enabled",
        active: systemctl(&["is-active", UNIT]).as_deref() == Some("active"),
    }
}

pub fn set_service(on: bool) -> Result<(), String> {
    let out = Command::new("systemctl")
        .args(["--user", if on { "enable" } else { "disable" }, "--now", UNIT])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip() {
        let mut s = Saved::default();
        s.tunables.insert("ppt_pl1_spl".into(), 95);
        s.fans.insert(1, 3000);
        s.fans.insert(4, 0);
        s.fan_curve = true;
        s.epp = true;
        s.rapl_modes.insert("performance".into(), Limits { pl1: 150, pl2: 190 });
        let text = s.render();
        assert!(text.contains("tunable.ppt_pl1_spl=95"));
        assert!(text.contains("rapl.performance=150/190"));
        assert_eq!(Saved::parse(&text), s);
        // мусор и комментарии игнорируются, значения по умолчанию
        let p = Saved::parse("# x\nfoo\nfan.x=1\ntunable.a=oops\nfan.2 = 2500 # comment\nrapl.nope=1/2\n");
        assert_eq!(p.fans.get(&2), Some(&2500));
        assert!(p.tunables.is_empty());
        assert!(p.rapl_modes.is_empty());
        assert!(p.rapl && !p.epp && !p.fan_curve);
        let p = Saved::parse("cpu.rapl=0\n");
        assert!(!p.rapl);
    }

    #[test]
    fn limits_for_modes() {
        let mut s = Saved::default();
        assert_eq!(s.limits_for(PowerMode::Performance), Some(Limits { pl1: 145, pl2: 190 }));
        s.rapl_modes.insert("performance".into(), Limits { pl1: 100, pl2: 120 });
        assert_eq!(s.limits_for(PowerMode::Performance), Some(Limits { pl1: 100, pl2: 120 }));
        assert_eq!(s.limits_for(PowerMode::Quiet), Some(Limits { pl1: 55, pl2: 65 }));
    }

    #[test]
    fn fan_curve_shape() {
        let fan = Fan { index: 1, label: "", rpm: 0, target: 0, min: 1600, max: 5200, step: 100, writable: true };
        assert_eq!(curve_rpm(&fan, 30.0, Some(PowerMode::Balanced)), 1600);
        assert_eq!(curve_rpm(&fan, 100.0, Some(PowerMode::Balanced)), 5200);
        let mid = curve_rpm(&fan, 75.0, Some(PowerMode::Balanced));
        assert_eq!(mid, 3000);
        assert!(curve_rpm(&fan, 75.0, Some(PowerMode::Quiet)) < mid);
        assert!(curve_rpm(&fan, 75.0, Some(PowerMode::Performance)) > mid);
        assert_eq!(curve_rpm(&fan, 75.0, Some(PowerMode::Balanced)) % 100, 0);
    }
}

//! Контекст приложения и оболочка окна: шапка, навигация, страница, строка состояния.

use super::{icons, pages, widgets};
use crate::hw::battery::Battery;
use crate::hw::device::{SystemInfo, Toggles};
use crate::hw::power::{PowerMode, PowerState};
use crate::hw::sensors::Sensors;
use crate::spectrum::protocol::Effect;
use crate::worker::{self, Job, NoticeKind, RgbLink, RgbState, Sink};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use syngui::prelude::*;
use syngui::widgets::GestureDetector;

const BRAND_PNG: &[u8] = include_bytes!("../../assets/icon/linux-legion-128.png");

/// Разделы навигации.
pub const PAGES: [(&str, &str); 5] = [
    ("Главная", icons::DASHBOARD),
    ("Производительность", icons::SPEED),
    ("Подсветка", icons::PALETTE),
    ("Батарея", icons::BATTERY_FULL),
    ("Устройство", icons::LAPTOP),
];

#[derive(Clone)]
pub struct AppCtx {
    pub sink: Sink,
    pub page: RwSignal<usize>,
    /// Черновик лимитов мощности: имя атрибута → значение.
    pub tun_edit: RwSignal<BTreeMap<String, i64>>,
    /// Черновик оборотов: номер вентилятора → об/мин (0 — авто).
    pub fan_edit: RwSignal<BTreeMap<u8, u32>>,
    /// Редактируемые слои активного профиля подсветки.
    pub rgb_edit: RwSignal<Vec<Effect>>,
    pub rgb_layer: RwSignal<Option<usize>>,
    /// Выделенные зоны (коды), по возрастанию.
    pub rgb_sel: RwSignal<Vec<u16>>,
    jobs: Sender<Job>,
    pending: Arc<Mutex<Option<Receiver<Job>>>>,
    page_flag: Arc<AtomicU8>,
    simulate: bool,
}

impl AppCtx {
    /// Создаёт сигналы. Вызывать один раз, до `App::run`.
    pub fn new(simulate: bool) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            sink: Sink {
                sensors: use_signal(Sensors::default()),
                power: use_signal(PowerState::default()),
                tunables: use_signal(Vec::new()),
                fans: use_signal(Vec::new()),
                battery: use_signal(None::<Battery>),
                toggles: use_signal(Toggles::default()),
                info: use_signal(SystemInfo::default()),
                rgb: use_signal(RgbState::default()),
                rgb_frame: use_signal(Vec::new()),
                rgb_effects: use_signal(None),
                busy: use_signal(false),
                notice: use_signal(None),
                autoapply: use_signal(Default::default()),
            },
            page: use_signal(0usize),
            tun_edit: use_signal(BTreeMap::new()),
            fan_edit: use_signal(BTreeMap::new()),
            rgb_edit: use_signal(Vec::new()),
            rgb_layer: use_signal(None),
            rgb_sel: use_signal(Vec::new()),
            jobs: tx,
            pending: Arc::new(Mutex::new(Some(rx))),
            page_flag: Arc::new(AtomicU8::new(0)),
            simulate,
        }
    }

    /// Запускает фоновый поток и эффекты уровня приложения (после
    /// инициализации рантайма окна).
    pub fn start(&self) {
        if let Some(rx) = self.pending.lock().unwrap().take() {
            worker::spawn(self.sink, self.simulate, self.page_flag.clone(), rx);
        }
        self.install_effects();
    }

    pub fn send(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    pub fn go(&self, page: usize) {
        self.page.set(page);
        self.page_flag.store(page as u8, Ordering::Relaxed);
    }

    /// Эффекты живут всё время работы, поэтому ставятся вне страниц.
    fn install_effects(&self) {
        // Сменился профиль подсветки (с клавиатуры или из UI) — прочитать его слои.
        let c = self.clone();
        create_effect(move || {
            let st = c.sink.rgb.get();
            if st.link != RgbLink::Ready || st.profile == 0 {
                return;
            }
            if c.sink.rgb_effects.get_untracked().map(|e| e.0) != Some(st.profile) {
                c.send(Job::RgbLoad(st.profile));
            }
        });
        // Прочитанные слои становятся черновиком.
        let c = self.clone();
        create_effect(move || {
            let Some((_, effects)) = c.sink.rgb_effects.get() else { return };
            let first = (!effects.is_empty()).then_some(0);
            c.rgb_edit.set(effects);
            c.rgb_layer.set(first);
        });
    }

    /// Черновик слоёв отличается от записанного в контроллер.
    pub fn rgb_dirty(&self) -> bool {
        let edit = self.rgb_edit.get();
        self.sink.rgb_effects.get().is_some_and(|(_, e)| e != edit)
    }

    pub fn edit_rgb(&self, f: impl FnOnce(&mut Vec<Effect>)) {
        let mut v = self.rgb_edit.get_untracked();
        f(&mut v);
        self.rgb_edit.set(v);
    }

    pub fn power_mode(&self) -> Option<PowerMode> {
        self.sink.power.get().mode
    }
}

pub fn build_root(ctx: AppCtx) -> impl Widget {
    Column::new()
        .gap(0.0)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .child(header(ctx.clone()))
        .child(
            Row::new()
                .gap(0.0)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .child(nav(ctx.clone()))
                .child(content(ctx.clone()))
                .class("grow"),
        )
        .child(footer(ctx))
        .class("root")
}

fn header(ctx: AppCtx) -> impl Widget {
    let (c1, c2, c3) = (ctx.clone(), ctx.clone(), ctx.clone());
    Row::new()
        .gap(14.0)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .child(Image::from_bytes("brand-icon", BRAND_PNG.to_vec()).fit(ImageFit::Contain).class("brand-icon"))
        .child(
            Column::new()
                .gap(1.0)
                .child(Text::new("LEGION").class("brand-kicker"))
                .child(move || {
                    let model = c1.sink.info.get().model;
                    Text::new(if model.is_empty() { "Lenovo Legion".to_string() } else { model }).class("brand-title")
                }),
        )
        .child(DecoratedBox::new().class("grow"))
        .child(move || mode_pill(&c2))
        .child(widgets::reactive_box(move || temp_pill(c3.sink.sensors.get())))
        .child(widgets::reactive_box(move || battery_pill(ctx.sink.battery.get())))
        .class("header")
}

fn pill(content: impl Widget + 'static) -> impl Widget {
    DecoratedBox::new().child(content).class("pill")
}

fn mode_pill(ctx: &AppCtx) -> impl Widget {
    let (tone, text) = match ctx.power_mode() {
        Some(m) => (m.tone(), m.label()),
        None => ("idle", "Режим неизвестен"),
    };
    pill(
        Row::new()
            .gap(8.0)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .child(DecoratedBox::new().class(&format!("dot dot-{tone}")))
            .child(Text::new(text).class("pill-text")),
    )
}

fn temp_pill(s: Sensors) -> Box<dyn Widget> {
    let Some(t) = s.cpu_temp else { return widgets::nothing() };
    Box::new(pill(
        Row::new()
            .gap(6.0)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .child(Icon::new(icons::THERMOSTAT).class("pill-icon"))
            .child(Text::new(format!("CPU {t:.0}°")).class("pill-text")),
    ))
}

fn battery_pill(b: Option<Battery>) -> Box<dyn Widget> {
    let Some(b) = b else { return widgets::nothing() };
    let icon = if b.ac_online { icons::BATTERY_CHARGING } else { icons::BATTERY_FULL };
    let tone = if !b.ac_online && b.percent <= 15 { "bad" } else { "ok" };
    Box::new(pill(
        Row::new()
            .gap(6.0)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .child(Icon::new(icon).class(&format!("pill-icon tone-{tone}")))
            .child(Text::new(format!("{}%", b.percent)).class("pill-text")),
    ))
}

fn nav(ctx: AppCtx) -> impl Widget {
    let mut col = Column::new().gap(6.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
    col = col.child(Text::new("ЦЕНТР УПРАВЛЕНИЯ").class("nav-caption"));
    for (i, (label, icon)) in PAGES.into_iter().enumerate() {
        let (c, sel) = (ctx.clone(), ctx.page);
        col = col.child(
            GestureDetector::new()
                .on_click(move || c.go(i))
                .cursor(syngui::CursorIcon::Pointer)
                .child(move || {
                    let on = sel.get() == i;
                    DecoratedBox::new()
                        .child(
                            Row::new()
                                .gap(12.0)
                                .cross_axis_alignment(CrossAxisAlignment::Center)
                                .child(Icon::new(icon).class(if on { "nav-icon nav-icon-on" } else { "nav-icon" }))
                                .child(Text::new(label).class(if on { "nav-text nav-text-on" } else { "nav-text" })),
                        )
                        .class(if on { "nav-item nav-item-on" } else { "nav-item" })
                }),
        );
    }
    let info = ctx.sink.info;
    col.child(DecoratedBox::new().class("grow"))
        .child(move || {
            let i = info.get();
            Column::new()
                .gap(3.0)
                .child(Text::new(i.machine_type.clone()).class("nav-foot-strong"))
                .child(Text::new(format!("BIOS {}", i.bios)).class("nav-foot"))
                .class("nav-footer")
        })
        .class("nav")
}

fn content(ctx: AppCtx) -> impl Widget {
    ScrollView::new()
        .vertical()
        .child(Padding::all(24.0).child(widgets::reactive_box(move || match ctx.page.get() {
            0 => Box::new(pages::home::view(ctx.clone())),
            1 => Box::new(pages::power::view(ctx.clone())),
            2 => Box::new(pages::lighting::view(ctx.clone())),
            3 => Box::new(pages::battery::view(ctx.clone())),
            _ => Box::new(pages::device::view(ctx.clone())),
        })))
        .class("content")
        .class("grow")
}

fn footer(ctx: AppCtx) -> impl Widget {
    let c = ctx.clone();
    Row::new()
        .gap(10.0)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .child(widgets::reactive_box(move || status_line(&c)))
        .child(DecoratedBox::new().class("grow"))
        .child(move || {
            let text = if ctx.simulate { "демо-режим · данные имитированы" } else { concat!("linux-legion ", env!("CARGO_PKG_VERSION")) };
            Text::new(text).class("footer-note")
        })
        .class("footer")
}

fn status_line(ctx: &AppCtx) -> Box<dyn Widget> {
    if ctx.sink.busy.get() {
        return Box::new(
            Row::new()
                .gap(10.0)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .child(CircularProgress::new().indeterminate().size(16.0).stroke_width(2.0))
                .child(Text::new("Запись в контроллер…").class("status-text")),
        );
    }
    match ctx.sink.notice.get() {
        Some(n) => {
            let (icon, tone) = match n.kind {
                NoticeKind::Success => (icons::CHECK, "ok"),
                NoticeKind::Error => (icons::ERROR, "bad"),
            };
            Box::new(
                Row::new()
                    .gap(8.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .child(Icon::new(icon).class(&format!("status-icon tone-{tone}")))
                    .child(Text::new(n.text).class("status-text")),
            )
        }
        None => Box::new(Text::new("Готово").class("status-text")),
    }
}

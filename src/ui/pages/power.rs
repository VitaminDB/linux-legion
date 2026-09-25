//! Производительность: режимы питания, лимиты мощности и вентиляторы режима «Свой».

use super::home::mode_icon;
use crate::hw::power::PowerMode;
use crate::ui::app::AppCtx;
use crate::ui::icons;
use crate::ui::widgets::{access_banner, bar, card, card_with, labeled, page_header, reactive, reactive_box};
use crate::worker::Job;
use syngui::prelude::*;
use syngui::widgets::*;
use syngui::CursorIcon;

pub fn view(ctx: AppCtx) -> impl Widget {
    let c = ctx.clone();
    Column::new()
        .gap(18.0)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .child(page_header(
            "Производительность",
            "Режим питания меняет лимиты мощности и работу вентиляторов. На клавиатуре — Fn+Q.",
        ))
        .child(reactive_box(move || {
            let st = c.sink.power.get();
            if st.mode.is_some() && !st.writable {
                Box::new(access_banner())
            } else {
                crate::ui::widgets::nothing()
            }
        }))
        .child(card("Режим питания", "", modes(ctx.clone())))
        .child(custom_notice(ctx.clone()))
        .child(tunables(ctx.clone()))
        .child(fans(ctx))
        .class("page")
}

fn modes(ctx: AppCtx) -> impl Widget {
    reactive(move || {
        let st = ctx.sink.power.get();
        let modes = if st.choices.is_empty() { PowerMode::ALL.to_vec() } else { st.choices.clone() };
        let mut row = Row::new().gap(12.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
        for m in modes {
            let c = ctx.clone();
            let on = st.mode == Some(m);
            let cls = if on { format!("mode-tile mode-tile-on tile-{}", m.tone()) } else { "mode-tile".to_string() };
            row = row.child(
                GestureDetector::new()
                    .on_click(move || c.send(Job::SetPowerMode(m)))
                    .cursor(CursorIcon::Pointer)
                    .child(
                        DecoratedBox::new()
                            .child(
                                Column::new()
                                    .gap(10.0)
                                    .cross_axis_alignment(CrossAxisAlignment::Start)
                                    .child(
                                        DecoratedBox::new()
                                            .child(Center::new().child(Icon::new(mode_icon(m)).class("mode-icon")))
                                            .class(&format!("mode-icon-badge badge-{}", m.tone())),
                                    )
                                    .child(Text::new(m.label()).class("mode-label"))
                                    .child(Text::new(m.hint()).class("mode-hint")),
                            )
                            .class(&cls),
                    )
                    .class("grow"),
            );
        }
        row
    })
}

/// Подсказка: лимиты и вентиляторы меняются только в режиме «Свой».
fn custom_notice(ctx: AppCtx) -> impl Widget {
    reactive_box(move || {
        let st = ctx.sink.power.get();
        if st.mode == Some(PowerMode::Custom) || !st.choices.contains(&PowerMode::Custom) {
            return crate::ui::widgets::nothing();
        }
        let c = ctx.clone();
        Box::new(
            DecoratedBox::new()
                .child(
                    Row::new()
                        .gap(14.0)
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .child(Icon::new(icons::INFO).class("banner-icon-info"))
                        .child(
                            Text::new(
                                "Лимиты мощности и обороты вентиляторов прошивка разрешает менять только в режиме «Свой». \
                                 В остальных режимах они показаны для справки.",
                            )
                            .class("row-desc grow"),
                        )
                        .child(
                            Button::new("Включить «Свой»")
                                .icon(icons::MODE_CUSTOM)
                                .on_click(move || c.send(Job::SetPowerMode(PowerMode::Custom)))
                                .class("btn-primary"),
                        ),
                )
                .class("banner banner-info"),
        )
    })
}

fn tunables(ctx: AppCtx) -> impl Widget {
    let (c_apply, c_reset) = (ctx.clone(), ctx.clone());
    let actions = move || {
        let custom = c_apply.power_mode() == Some(PowerMode::Custom);
        let dirty = !c_apply.tun_edit.get().is_empty();
        let (ca, cr) = (c_apply.clone(), c_reset.clone());
        Row::new()
            .gap(8.0)
            .child(
                Button::new("По умолчанию")
                    .icon(icons::RESTART)
                    .disabled(!custom)
                    .on_click(move || {
                        let map = cr
                            .sink
                            .tunables
                            .get_untracked()
                            .into_iter()
                            .filter_map(|t| t.default.map(|d| (t.name, d)))
                            .collect();
                        cr.tun_edit.set(map);
                    })
                    .class("btn-ghost"),
            )
            .child(
                Button::new("Применить")
                    .icon(icons::CHECK)
                    .disabled(!custom || !dirty)
                    .on_click(move || {
                        for (name, v) in ca.tun_edit.get_untracked() {
                            ca.send(Job::SetTunable(name, v));
                        }
                        ca.tun_edit.set(Default::default());
                    })
                    .class("btn-primary"),
            )
    };
    card_with(
        icons::BOLT,
        "Лимиты мощности",
        actions,
        reactive_box(move || {
            let list = ctx.sink.tunables.get();
            if list.is_empty() {
                return Box::new(
                    Text::new("Драйвер lenovo-wmi-other не отдаёт настраиваемых лимитов на этой модели.").class("muted"),
                );
            }
            let custom = ctx.power_mode() == Some(PowerMode::Custom);
            let edit = ctx.tun_edit.get();
            let mut col = Column::new().gap(22.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            for t in list {
                let v = edit.get(&t.name).copied().unwrap_or(t.value);
                let changed = v != t.value;
                let c = ctx.clone();
                let name = t.name.clone();
                let value = if changed { format!("{} → {v} {}", t.value, t.unit) } else { format!("{v} {}", t.unit) };
                col = col.child(
                    Column::new()
                        .gap(8.0)
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .child(labeled(&t.label, value))
                        .child(
                            Slider::new()
                                .value(v as f32)
                                .range(t.min as f32, t.max as f32)
                                .step(t.step as f32)
                                .disabled(!custom || !t.writable)
                                .on_change(move |x| {
                                    let x = x.round() as i64;
                                    let mut m = c.tun_edit.get_untracked();
                                    m.insert(name.clone(), x);
                                    c.tun_edit.set(m);
                                })
                                .class("wide-slider"),
                        )
                        .child(
                            Row::new()
                                .gap(8.0)
                                .child(Text::new(t.hint.clone()).class("row-desc grow"))
                                .child(Text::new(format!("{}–{} {}", t.min, t.max, t.unit)).class("scale-label")),
                        ),
                );
            }
            Box::new(col)
        }),
    )
}

fn fans(ctx: AppCtx) -> impl Widget {
    let c_apply = ctx.clone();
    let actions = move || {
        let custom = c_apply.power_mode() == Some(PowerMode::Custom);
        let dirty = !c_apply.fan_edit.get().is_empty();
        let (ca, cr) = (c_apply.clone(), c_apply.clone());
        Row::new()
            .gap(8.0)
            .child(
                Button::new("Все — авто")
                    .icon(icons::AUTO)
                    .disabled(!custom)
                    .on_click(move || {
                        let map = cr.sink.fans.get_untracked().iter().map(|f| (f.index, 0)).collect();
                        cr.fan_edit.set(map);
                    })
                    .class("btn-ghost"),
            )
            .child(
                Button::new("Применить")
                    .icon(icons::CHECK)
                    .disabled(!custom || !dirty)
                    .on_click(move || {
                        for (i, rpm) in ca.fan_edit.get_untracked() {
                            ca.send(Job::SetFanTarget(i, rpm));
                        }
                        ca.fan_edit.set(Default::default());
                    })
                    .class("btn-primary"),
            )
    };
    card_with(
        icons::FAN,
        "Вентиляторы",
        actions,
        reactive_box(move || {
            let list = ctx.sink.fans.get();
            if list.is_empty() {
                return Box::new(Text::new("Вентиляторы не найдены (нужен драйвер lenovo-wmi-other).").class("muted"));
            }
            let custom = ctx.power_mode() == Some(PowerMode::Custom);
            let edit = ctx.fan_edit.get();
            let mut col = Column::new().gap(22.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            for f in list {
                let target = edit.get(&f.index).copied().unwrap_or(f.target);
                let auto = target == 0;
                let changed = edit.contains_key(&f.index) && target != f.target;
                let (c_t, c_s) = (ctx.clone(), ctx.clone());
                let (idx, min, step) = (f.index, f.min, f.step);
                let frac = if f.max > 0 { f.rpm as f32 / f.max as f32 } else { 0.0 };
                let set = move |c: &AppCtx, v: u32| {
                    let mut m = c.fan_edit.get_untracked();
                    m.insert(idx, v);
                    c.fan_edit.set(m);
                };
                let target_text = if auto { "авто".to_string() } else { format!("{target} об/мин") };
                col = col.child(
                    Column::new()
                        .gap(10.0)
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .child(
                            Row::new()
                                .gap(12.0)
                                .cross_axis_alignment(CrossAxisAlignment::Center)
                                .child(Icon::new(icons::FAN).class("fan-icon"))
                                .child(
                                    Column::new()
                                        .gap(2.0)
                                        .child(Text::new(format!("{} · №{}", f.label, f.index)).class("row-title"))
                                        .child(
                                            Text::new(format!(
                                                "Сейчас {} об/мин · цель: {}{}",
                                                f.rpm,
                                                target_text,
                                                if changed { " (не применено)" } else { "" }
                                            ))
                                            .class(if changed { "row-desc changed" } else { "row-desc" }),
                                        )
                                        .class("grow"),
                                )
                                .child(Text::new("Авто").class("field-label"))
                                .child(
                                    Toggle::new()
                                        .on(auto)
                                        .on_change(move |on| {
                                            let v = if on { 0 } else { min };
                                            set(&c_t, v)
                                        })
                                        .class(if custom && f.writable { "" } else { "toggle-off" }),
                                ),
                        )
                        .child(bar(frac, "fan"))
                        .child(
                            Slider::new()
                                .value(if auto { f.min as f32 } else { target as f32 })
                                .range(f.min as f32, f.max as f32)
                                .step(f.step as f32)
                                .disabled(!custom || auto || !f.writable)
                                .on_change(move |x| {
                                    let v = ((x / step as f32).round() as u32) * step;
                                    set(&c_s, v)
                                })
                                .class("wide-slider"),
                        )
                        .child(
                            Row::new()
                                .child(Text::new(format!("{} об/мин", f.min)).class("scale-label"))
                                .child(DecoratedBox::new().class("grow"))
                                .child(Text::new(format!("{} об/мин", f.max)).class("scale-label")),
                        ),
                );
            }
            Box::new(col)
        }),
    )
}

//! Производительность: режимы питания, лимиты мощности и вентиляторы режима «Свой».

use super::home::mode_icon;
use crate::autoapply::Flag;
use crate::hw::power::PowerMode;
use crate::ui::app::AppCtx;
use crate::ui::icons;
use crate::ui::widgets::{
    access_banner, bar, card, card_with, kv, labeled, page_header, reactive, reactive_box, setting_row,
};
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
        .child(cpu(ctx.clone()))
        .child(fans(ctx.clone()))
        .child(autoapply(ctx))
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
        let (ca, cr, cm) = (c_apply.clone(), c_reset.clone(), c_reset.clone());
        Row::new()
            .gap(8.0)
            .child(
                Button::new("Максимум")
                    .icon(icons::BOLT)
                    .disabled(!custom)
                    .on_click(move || {
                        let map = cm.sink.tunables.get_untracked().into_iter().map(|t| (t.name, t.max)).collect();
                        cm.tun_edit.set(map);
                    })
                    .class("btn-ghost"),
            )
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
            let (svc, saved) = ctx.sink.autoapply.get();
            let mut col = Column::new().gap(22.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            let c = ctx.clone();
            let desc = if saved.fan_curve && !(svc.enabled && svc.active) {
                "Включено, но служба не запущена — включите её в разделе «Служба» ниже."
            } else if saved.fan_curve {
                "Служба ведёт вентиляторы по температуре CPU и видеокарты во всех режимах; \
                 в «Свой» заданные здесь обороты держатся как есть (выше 95 °C — не ниже кривой)."
            } else {
                "EC Legion Pro 7 Gen 10 после ручного задания оборотов не возвращается к своей кривой: \
                 ни по «авто», ни по смене режима, ни после сна и перезагрузки — только после сброса EC \
                 (выключить, отсоединить питание, подержать кнопку питания 30 с). Если вы задавали \
                 обороты вручную — включите, и вентиляторы снова будут следовать температуре."
            };
            col = col.child(setting_row(
                "Программное авто (по температуре)",
                desc,
                Box::new(Toggle::new().on(saved.fan_curve).on_change(move |v| c.send(Job::SetFlag(Flag::FanCurve, v)))),
            ));
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

/// Служба: автоприменение «Своего» после перезагрузки и сна, лимиты CPU, вентиляторы.
fn autoapply(ctx: AppCtx) -> impl Widget {
    card_with(
        icons::RESTART,
        "Служба",
        || Text::new("").class("card-hint"),
        reactive_box(move || {
            let (svc, saved) = ctx.sink.autoapply.get();
            let tunables = ctx.sink.tunables.get();
            let fans = ctx.sink.fans.get();
            let mut col = Column::new().gap(16.0).cross_axis_alignment(CrossAxisAlignment::Stretch);

            let c = ctx.clone();
            let desc = if !svc.installed {
                "Служба ставится вместе с пакетом. При сборке из исходников скопируйте \
                 packaging/linux-legion-autoapply.service в ~/.config/systemd/user/."
            } else if svc.enabled && svc.active {
                "Служба работает: восстанавливает значения «Своего» при входе, при переключении в «Свой» \
                 и после сна, держит лимиты CPU и EPP по режиму, ведёт вентиляторы по температуре."
            } else if svc.enabled {
                "Служба включена, но сейчас не запущена — проверьте journalctl --user -u linux-legion-autoapply."
            } else {
                "Прошивка забывает лимиты и обороты после перезагрузки, а лимиты CPU в RAPL вообще никто, \
                 кроме службы, не выставляет. Служба делает это сама (linux-legion --daemon)."
            };
            col = col.child(setting_row(
                "Служба linux-legion",
                desc,
                Box::new(
                    Toggle::new()
                        .on(svc.enabled)
                        .on_change(move |v| c.send(Job::SetAutoApply(v)))
                        .class(if svc.installed { "" } else { "toggle-off" }),
                ),
            ));

            let mut items: Vec<String> = saved
                .tunables
                .iter()
                .map(|(name, v)| {
                    let t = tunables.iter().find(|t| &t.name == name);
                    let label = t.map_or(name.clone(), |t| t.label.clone());
                    format!("{label}: {v} {}", t.map_or("", |t| t.unit))
                })
                .collect();
            items.extend(saved.fans.iter().map(|(i, rpm)| {
                let label = fans.iter().find(|f| f.index == *i).map_or("Вентилятор", |f| f.label);
                let v = if *rpm == 0 { "авто".to_string() } else { format!("{rpm} об/мин") };
                format!("Вентилятор «{label}»: {v}")
            }));
            if items.is_empty() {
                col = col.child(
                    Text::new(
                        "Пока ничего не сохранено: значения запоминаются кнопками «Применить» в режиме «Свой».",
                    )
                    .class("muted"),
                );
            } else {
                let mut list = Column::new().gap(2.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
                for it in items {
                    let (k, v) = it.rsplit_once(": ").unwrap_or((it.as_str(), ""));
                    list = list.child(crate::ui::widgets::kv(k, v.to_string()));
                }
                col = col.child(Text::new("Сохранённые значения").class("field-label")).child(list);
            }
            Box::new(col)
        }),
    )
}

/// Лимиты CPU (RAPL) и EPP по режиму — то, что в Windows делает Legion Space.
fn cpu(ctx: AppCtx) -> impl Widget {
    card_with(
        icons::MEMORY,
        "Процессор по режимам",
        || Text::new("").class("card-hint"),
        reactive_box(move || {
            let st = ctx.sink.cpu.get();
            let (_, saved) = ctx.sink.autoapply.get();
            let power = ctx.sink.power.get();
            if !st.rapl.present() && st.epp.current.is_none() {
                return Box::new(Text::new("RAPL (powercap) и EPP (cpufreq) недоступны.").class("muted"));
            }
            let mut col = Column::new().gap(16.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            let fmt = |l: Option<crate::hw::rapl::Limits>| match l {
                Some(l) => format!("{} / {} Вт", l.pl1, l.pl2),
                None => "—".to_string(),
            };
            if st.rapl.present() {
                let c = ctx.clone();
                let desc = if !st.rapl.writable {
                    "Нет прав на запись в powercap — обновите udev-правило пакета (группа wheel)."
                } else {
                    "BIOS при загрузке ставит в RAPL MMIO 30/30 Вт и больше их не меняет: без этого \
                     процессор во всех режимах упирается в 30 Вт. Служба держит в RAPL (MSR и MMIO) \
                     заводские лимиты режима, а в «Свой» — лимиты, заданные выше."
                };
                col = col.child(setting_row(
                    "Лимиты CPU по режиму (RAPL)",
                    desc,
                    Box::new(
                        Toggle::new()
                            .on(saved.rapl)
                            .on_change(move |v| c.send(Job::SetFlag(Flag::Rapl, v)))
                            .class(if st.rapl.writable { "" } else { "toggle-off" }),
                    ),
                ));
                let mut list = Column::new().gap(2.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
                let modes = if power.choices.is_empty() { PowerMode::ALL.to_vec() } else { power.choices.clone() };
                for m in modes {
                    let v = saved.limits_for(m);
                    let mark = if power.mode == Some(m) { " ●" } else { "" };
                    list = list.child(kv(&format!("{}{mark}", m.label()), fmt(v)));
                }
                list = list.child(kv("Сейчас в RAPL: MSR", fmt(st.rapl.msr)));
                list = list.child(kv("Сейчас в RAPL: MMIO", fmt(st.rapl.mmio)));
                col = col.child(Text::new("PL1 / PL2 по режимам (свои — rapl.<режим>=PL1/PL2 в custom.conf)").class("field-label")).child(list);
            }
            if let Some(cur) = st.epp.current.clone() {
                let c = ctx.clone();
                let desc = if !st.epp.writable {
                    "Нет прав на запись EPP — обновите udev-правило пакета (группа wheel)."
                } else {
                    "performance в Производительность/Экстрим/Свой, balance_performance в Балансе, \
                     balance_power в Тихом. Перекрывает tuned и power-profiles-daemon. Без performance \
                     процессор не добирает верхние ступени turbo."
                };
                let want = power.mode.map(|m| m.epp()).unwrap_or("—");
                col = col.child(setting_row(
                    "EPP по режиму",
                    desc,
                    Box::new(
                        Toggle::new()
                            .on(saved.epp)
                            .on_change(move |v| c.send(Job::SetFlag(Flag::Epp, v)))
                            .class(if st.epp.writable { "" } else { "toggle-off" }),
                    ),
                ));
                let now = if st.epp.pinned() {
                    format!("{cur} — governor performance, EPP прибит к performance")
                } else {
                    format!("{cur} (для режима: {want})")
                };
                col = col.child(kv("EPP сейчас", now));
            }
            Box::new(col)
        }),
    )
}

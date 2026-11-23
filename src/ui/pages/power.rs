//! Производительность: режимы питания, лимиты мощности и вентиляторы режима «Свой».

use super::home::mode_icon;
use crate::autoapply::{Flag, Profile};
use crate::hw::power::{PowerMode, Tunable};
use crate::hw::rapl::Limits;
use crate::hw::sensors::Gpu;
use crate::ui::app::AppCtx;
use crate::ui::icons;
use crate::ui::widgets::{
    access_banner, bar, card_with, kv, labeled, page_header, reactive, reactive_box, setting_row,
};
use crate::worker::{CloneFrom, Job};
use syngui::prelude::*;
use syngui::widgets::*;
use syngui::CursorIcon;

pub fn view(ctx: AppCtx) -> impl Widget {
    let c = ctx.clone();
    Column::new()
        .gap(18.0)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .child(page_header(
            t!("Производительность"),
            t!("Режим питания меняет лимиты мощности и работу вентиляторов. На клавиатуре — Fn+Q."),
        ))
        .child(reactive_box(move || {
            let st = c.sink.power.get();
            if st.mode.is_some() && !st.writable {
                Box::new(access_banner())
            } else {
                crate::ui::widgets::nothing()
            }
        }))
        .child(modes_card(ctx.clone()))
        .child(profile_editor(ctx.clone()))
        .child(custom_notice(ctx.clone()))
        // Имя — для `--scroll-to power-limits` (снимки для README).
        .child(syngui::widgets::Named::new(
            "power-limits",
            Row::new()
                .gap(18.0)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .child(tunables(ctx.clone(), Group::Cpu).class("grow").class("half"))
                .child(tunables(ctx.clone(), Group::Gpu).class("grow").class("half")),
        ))
        .child(cpu(ctx.clone()))
        .child(fans(ctx.clone()))
        .child(autoapply(ctx))
        .class("page")
}

/// Плитка режима или профиля.
fn tile(icon: &'static str, tone: &str, on: bool, label: String, hint: String, click: impl Fn() + Send + Sync + 'static) -> impl Widget {
    let cls = if on { format!("mode-tile mode-tile-on tile-{tone}") } else { "mode-tile".to_string() };
    GestureDetector::new()
        .on_click(click)
        .cursor(CursorIcon::Pointer)
        .child(
            DecoratedBox::new()
                .child(
                    Column::new()
                        .gap(10.0)
                        .cross_axis_alignment(CrossAxisAlignment::Start)
                        .child(
                            DecoratedBox::new()
                                .child(Center::new().child(Icon::new(icon).class("mode-icon")))
                                .class(&format!("mode-icon-badge badge-{tone}")),
                        )
                        .child(Text::new(label).class("mode-label"))
                        .child(Text::new(hint).class("mode-hint")),
                )
                .class(&cls),
        )
        .class("grow")
}

/// Кратко, что в профиле: «Экстрим · CPU ≤ 90 °C · 160/205 Вт».
fn profile_summary(p: &Profile) -> String {
    let mut parts = vec![p.base.label()];
    if let Some(t) = p.cpu_temp {
        parts.push(t!("CPU ≤ {t} °C", t = t));
    }
    if let Some(l) = p.rapl {
        parts.push(t!("{pl1} / {pl2} Вт", pl1 = l.pl1, pl2 = l.pl2));
    }
    if p.base == PowerMode::Custom && !p.tunables.is_empty() {
        parts.push(t!("свои лимиты прошивки"));
    }
    parts.join(" · ")
}

/// Встроенные режимы прошивки и свои профили; «Клонировать» — копия того, что включено.
fn modes_card(ctx: AppCtx) -> impl Widget {
    let c = ctx.clone();
    let actions = move || {
        let (_, saved) = c.sink.autoapply.get();
        let mode = c.power_mode();
        let from = match mode {
            Some(m) => match saved.active_for(m) {
                Some(p) => Some(CloneFrom::Profile(p.id)),
                None => Some(CloneFrom::Mode(m)),
            },
            None => None,
        };
        let cc = c.clone();
        Button::new(t!("Клонировать"))
            .icon(icons::COPY)
            .disabled(from.is_none())
            .on_click(move || {
                if let Some(f) = from {
                    cc.send(Job::CloneProfile(f));
                }
            })
            .class("btn-ghost")
    };
    card_with(
        icons::SPEED,
        t!("Режим питания"),
        actions,
        reactive(move || {
            let st = ctx.sink.power.get();
            let (_, saved) = ctx.sink.autoapply.get();
            let profile_on = st.mode.and_then(|m| saved.active_for(m)).map(|p| p.id);
            let modes = if st.choices.is_empty() { PowerMode::ALL.to_vec() } else { st.choices.clone() };
            let mut row = Row::new().gap(12.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            for m in modes {
                let c = ctx.clone();
                let on = st.mode == Some(m) && profile_on.is_none();
                row = row.child(tile(mode_icon(m), m.tone(), on, m.label(), m.hint(), move || c.send(Job::SetPowerMode(m))));
            }
            let mut col = Column::new().gap(14.0).cross_axis_alignment(CrossAxisAlignment::Stretch).child(row);
            col = col.child(Text::new(t!("Свои профили")).class("field-label"));
            if saved.profiles.is_empty() {
                col = col.child(
                    Text::new(t!(
                        "Пока нет. «Клонировать» создаёт профиль из включённого режима: например, «Экстрим» \
                         (полная мощность видеокарты) с пределом температуры процессора."
                    ))
                    .class("muted"),
                );
            } else {
                // По 5 в ряд, как встроенные режимы.
                for chunk in saved.profiles.chunks(5) {
                    let mut r = Row::new().gap(12.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
                    for p in chunk {
                        let c = ctx.clone();
                        let id = p.id;
                        let on = profile_on == Some(id);
                        r = r.child(tile(mode_icon(p.base), p.base.tone(), on, p.name.clone(), profile_summary(p), move || {
                            c.send(Job::ActivateProfile(id))
                        }));
                    }
                    for _ in chunk.len()..5 {
                        r = r.child(DecoratedBox::new().class("grow"));
                    }
                    col = col.child(r);
                }
            }
            col
        }),
    )
}

/// Редактор действующего своего профиля.
fn profile_editor(ctx: AppCtx) -> impl Widget {
    reactive_box(move || {
        // Пересобирается, только когда сменился сохранённый профиль, — поле
        // имени не теряет фокус при вводе.
        let Some(orig) = ctx.prof_orig.get() else { return crate::ui::widgets::nothing() };
        let id = orig.id;
        let (c_save, c_del, c_name) = (ctx.clone(), ctx.clone(), ctx.clone());
        let actions = move || {
            let dirty = c_save.prof_edit.get().as_ref() != c_save.prof_orig.get().as_ref();
            let (cs, cd) = (c_save.clone(), c_del.clone());
            Row::new()
                .gap(8.0)
                .child(
                    Button::new(t!("Удалить"))
                        .icon(icons::DELETE)
                        .on_click(move || cd.send(Job::DeleteProfile(id)))
                        .class("btn-ghost"),
                )
                .child(
                    Button::new(t!("Сохранить"))
                        .icon(icons::CHECK)
                        .disabled(!dirty)
                        .on_click(move || {
                            if let Some(mut p) = cs.prof_edit.get_untracked() {
                                if p.name.trim().is_empty() {
                                    p.name = p.base.label();
                                }
                                p.name = p.name.trim().to_string();
                                cs.send(Job::SaveProfile(p));
                            }
                        })
                        .class("btn-primary"),
                )
        };
        let base_dd = {
            let c = ctx.clone();
            let st = ctx.sink.power.get_untracked();
            let modes = if st.choices.is_empty() { PowerMode::ALL.to_vec() } else { st.choices.clone() };
            let mut dd = Dropdown::new().width(240.0).leading_icon(mode_icon(orig.base));
            for m in modes {
                dd = dd.item(DropdownItem::new(m.key(), m.label()));
            }
            dd.selected(orig.base.key()).on_change(move |k: &str| {
                if let Some(m) = PowerMode::from_key(k) {
                    c.edit_profile(|p| p.base = m);
                }
            })
        };
        let body = Column::new()
            .gap(18.0)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .child(
                Row::new()
                    .gap(16.0)
                    .cross_axis_alignment(CrossAxisAlignment::End)
                    .child(
                        Column::new()
                            .gap(6.0)
                            .cross_axis_alignment(CrossAxisAlignment::Stretch)
                            .child(Text::new(t!("Название")).class("field-label"))
                            .child(
                                TextField::new()
                                    .text(orig.name.clone())
                                    .placeholder(t!("Название профиля"))
                                    .on_change(move |t| c_name.edit_profile(|p| p.name = t.to_string()))
                                    .class("field"),
                            )
                            .class("grow"),
                    )
                    .child(
                        Column::new()
                            .gap(6.0)
                            .child(Text::new(t!("Режим прошивки")).class("field-label"))
                            .child(base_dd),
                    ),
            )
            .child(profile_limits(ctx.clone()));
        Box::new(card_with(icons::EDIT, t!("Профиль «{name}»", name = orig.name), actions, body))
    })
}

/// PL1/PL2 и предел температуры CPU профиля.
fn profile_limits(ctx: AppCtx) -> impl Widget {
    reactive_box(move || {
        let Some(p) = ctx.prof_edit.get() else { return crate::ui::widgets::nothing() };
        let cpu = ctx.sink.cpu.get();
        let (_, saved) = ctx.sink.autoapply.get();
        let tunables = ctx.sink.tunables.get_untracked();
        let range = |name: &str, lo: u32, hi: u32| {
            tunables.iter().find(|t| t.name == name && t.adjustable()).map_or((lo, hi), |t| (t.min as u32, t.max as u32))
        };
        let factory = saved.limits_for(p.base);
        let mut col = Column::new().gap(18.0).cross_axis_alignment(CrossAxisAlignment::Stretch);

        // PL1/PL2
        let c = ctx.clone();
        let fdesc = match factory {
            Some(l) => t!("Выключено — заводские для режима: {pl1} / {pl2} Вт.", pl1 = l.pl1, pl2 = l.pl2),
            None => t!("Выключено — лимиты прошивки режима «Свой»."),
        };
        col = col.child(setting_row(
            t!("Свои PL1 / PL2"),
            if saved.rapl { fdesc } else { t!("Служба не ведёт лимиты CPU (выключено в «Процессор по режимам»).") },
            Box::new(Toggle::new().on(p.rapl.is_some()).on_change(move |on| {
                c.edit_profile(|p| p.rapl = if on { factory.or(Some(Limits { pl1: 90, pl2: 125 })) } else { None })
            })),
        ));
        if let Some(l) = p.rapl {
            let (lo1, hi1) = range("ppt_pl1_spl", 30, 200);
            let (lo2, hi2) = range("ppt_pl2_sppt", 30, 250);
            let (c1, c2) = (ctx.clone(), ctx.clone());
            col = col
                .child(slider_row(t!("CPU: длительная мощность (PL1)"), l.pl1, t!("Вт"), lo1, hi1, move |v| {
                    c1.edit_profile(|p| {
                        if let Some(r) = p.rapl.as_mut() {
                            r.pl1 = v;
                            r.pl2 = r.pl2.max(v);
                        }
                    })
                }))
                .child(slider_row(t!("CPU: кратковременная мощность (PL2)"), l.pl2, t!("Вт"), lo2, hi2, move |v| {
                    c2.edit_profile(|p| {
                        if let Some(r) = p.rapl.as_mut() {
                            r.pl2 = v;
                            r.pl1 = r.pl1.min(v);
                        }
                    })
                }));
        }

        // Предел температуры (TCC)
        match cpu.tcc {
            Some(tcc) => {
                let (lo, hi) = tcc.range();
                let c = ctx.clone();
                let desc = if !tcc.writable {
                    t!("Нет прав на запись TCC Offset — обновите udev-правило пакета (группа wheel).")
                } else {
                    t!("Процессор сбрасывает частоту при этой температуре в любом режиме (Intel TCC). \
                     Служба держит предел, пока профиль действует. Сейчас: {now} °C.", now = tcc.limit())
                };
                let start = tcc.limit().clamp(lo, hi).min(95);
                col = col.child(setting_row(
                    t!("Предел температуры CPU"),
                    desc,
                    Box::new(
                        Toggle::new()
                            .on(p.cpu_temp.is_some())
                            .on_change(move |on| c.edit_profile(|p| p.cpu_temp = on.then_some(start))),
                    ),
                ));
                if let Some(t) = p.cpu_temp {
                    let c = ctx.clone();
                    col = col.child(slider_row(t!("CPU: температурный предел"), t, "°C".into(), lo, hi, move |v| {
                        c.edit_profile(|p| p.cpu_temp = Some(v))
                    }));
                }
            }
            None => {
                col = col.child(
                    Text::new(t!("Предел температуры CPU недоступен: нет intel_tcc_cooling (TCC Offset).")).class("muted"),
                );
            }
        }
        if p.base == PowerMode::Custom {
            col = col.child(
                Text::new(t!(
                    "Основа — «Свой»: лимиты прошивки и обороты вентиляторов ниже («Применить») сохраняются в этот профиль."
                ))
                .class("row-desc"),
            );
        }
        Box::new(col)
    })
}

/// Подпись, значение и ползунок с краями шкалы.
fn slider_row(label: String, v: u32, unit: String, lo: u32, hi: u32, set: impl Fn(u32) + Send + Sync + 'static) -> impl Widget {
    Column::new()
        .gap(8.0)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .child(labeled(label, format!("{v} {unit}")))
        .child(
            Slider::new()
                .value(v as f32)
                .range(lo as f32, hi as f32)
                .step(1.0)
                .on_change(move |x| set(x.round() as u32))
                .class("wide-slider"),
        )
        .child(
            Row::new()
                .child(DecoratedBox::new().class("grow"))
                .child(Text::new(format!("{lo}–{hi} {unit}")).class("scale-label")),
        )
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
                                t!("Лимиты мощности и обороты вентиляторов прошивка разрешает менять только в режиме «Свой». \
                                 В остальных режимах они показаны для справки."),
                            )
                            .class("row-desc grow"),
                        )
                        .child(
                            Button::new(t!("Включить «Свой»"))
                                .icon(icons::MODE_CUSTOM)
                                .on_click(move || c.send(Job::SetPowerMode(PowerMode::Custom)))
                                .class("btn-primary"),
                        ),
                )
                .class("banner banner-info"),
        )
    })
}

/// Карточка лимитов: процессор или видеокарта.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Cpu,
    Gpu,
}

impl Group {
    fn of(name: &str) -> Group {
        if name.starts_with("gpu_") {
            Group::Gpu
        } else {
            Group::Cpu
        }
    }

    /// Атрибуты группы, которые прошивка разрешает менять (диапазон не 0–0).
    fn list(self, all: Vec<Tunable>) -> Vec<Tunable> {
        all.into_iter().filter(|t| t.adjustable() && Group::of(&t.name) == self).collect()
    }
}

fn tunables(ctx: AppCtx, group: Group) -> syngui::StyledWidget<DecoratedBox> {
    let (c_apply, c_reset) = (ctx.clone(), ctx.clone());
    let actions = move || {
        let custom = c_apply.power_mode() == Some(PowerMode::Custom);
        let names: Vec<String> = group.list(c_apply.sink.tunables.get()).into_iter().map(|t| t.name).collect();
        let dirty = c_apply.tun_edit.get().keys().any(|k| names.contains(k));
        let (ca, cr, cm) = (c_apply.clone(), c_reset.clone(), c_reset.clone());
        let pick = move |c: &AppCtx, f: fn(&Tunable) -> Option<i64>| {
            let mut m = c.tun_edit.get_untracked();
            for t in group.list(c.sink.tunables.get_untracked()) {
                if let Some(v) = f(&t) {
                    m.insert(t.name, v);
                }
            }
            c.tun_edit.set(m);
        };
        Row::new()
            .gap(8.0)
            .child(DecoratedBox::new().class("grow"))
            .child(
                Button::new(t!("Максимум"))
                    .icon(icons::BOLT)
                    .disabled(!custom || names.is_empty())
                    .on_click(move || pick(&cm, |t| Some(t.max)))
                    .class("btn-ghost"),
            )
            .child(
                Button::new(t!("По умолчанию"))
                    .icon(icons::RESTART)
                    .disabled(!custom || names.is_empty())
                    .on_click(move || pick(&cr, |t| t.default))
                    .class("btn-ghost"),
            )
            .child(
                Button::new(t!("Применить"))
                    .icon(icons::CHECK)
                    .disabled(!custom || !dirty)
                    .on_click(move || {
                        let mut m = ca.tun_edit.get_untracked();
                        let mine: Vec<(String, i64)> = m.iter().filter(|(k, _)| Group::of(k) == group).map(|(k, v)| (k.clone(), *v)).collect();
                        for (name, v) in mine {
                            m.remove(&name);
                            ca.send(Job::SetTunable(name, v));
                        }
                        ca.tun_edit.set(m);
                    })
                    .class("btn-primary"),
            )
    };
    let (icon, title) = match group {
        Group::Cpu => (icons::MEMORY, t!("Процессор: лимиты прошивки")),
        Group::Gpu => (icons::GPU, t!("Видеокарта: лимиты прошивки")),
    };
    // Кнопки — внизу: в половинной карточке заголовок узкий.
    let body = reactive_box(move || {
            let all = ctx.sink.tunables.get();
            let list = group.list(all.clone());
            let custom = ctx.power_mode() == Some(PowerMode::Custom);
            let edit = ctx.tun_edit.get();
            let mut col = Column::new().gap(22.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            if group == Group::Gpu {
                col = col.child(gpu_info(&ctx, &all));
            }
            if list.is_empty() {
                let text = if all.is_empty() {
                    t!("Драйвер lenovo-wmi-other не отдаёт настраиваемых лимитов на этой модели.")
                } else {
                    t!("Прошивка не разрешает менять эти лимиты.")
                };
                return Box::new(col.child(Text::new(text).class("muted")));
            }
            for t in list {
                let v = edit.get(&t.name).copied().unwrap_or(t.value);
                let changed = v != t.value;
                let c = ctx.clone();
                let name = t.name.clone();
                let value = if changed { format!("{} → {v} {}", t.value, t!(t.unit)) } else { format!("{v} {}", t!(t.unit)) };
                col = col.child(
                    Column::new()
                        .gap(8.0)
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .child(labeled(t!(&t.label), value))
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
                                .child(Text::new(t!(&t.hint)).class("row-desc grow"))
                                .child(Text::new(format!("{}–{} {}", t.min, t.max, t!(t.unit))).class("scale-label")),
                        ),
                );
            }
            Box::new(col)
        });
    card_with(
        icon,
        title,
        || Text::new("").class("card-hint"),
        Column::new()
            .gap(22.0)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .child(body)
            .child(DecoratedBox::new().class("grow"))
            .child(actions),
    )
}

/// Мощность видеокарты сейчас и почему в «Свой» её не поднять.
fn gpu_info(ctx: &AppCtx, all: &[Tunable]) -> impl Widget {
    let mut col = Column::new().gap(2.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
    match ctx.sink.sensors.get().gpu {
        Gpu::Active(g) => {
            let w = |v: Option<f32>| v.map_or("—".to_string(), |v| t!("{v} Вт", v = format!("{v:.0}")));
            col = col.child(kv(t!("Предел мощности сейчас"), format!("{} / {}", w(g.power_limit_w), w(g.power_max_w))));
            col = col.child(kv(t!("Потребление"), w(g.power_w)));
        }
        Gpu::Sleeping => col = col.child(kv(t!("Предел мощности сейчас"), t!("видеокарта спит"))),
        Gpu::Absent => {}
    }
    let fixed = |n: &str| all.iter().find(|t| t.name == n && !t.adjustable()).map(|t| t.value);
    if let (Some(ctgp), Some(ppab)) = (fixed("gpu_nv_ctgp"), fixed("gpu_nv_ppab")) {
        col = col.child(kv(t!("TGP в «Свой» (прошивка)"), t!("{ctgp} + {ppab} Вт Dynamic Boost", ctgp = ctgp, ppab = ppab)));
        col = col.child(
            Text::new(t!(
                "Прошивка не даёт менять TGP в режиме «Свой» (диапазон 0–0), поэтому видеокарта там ограничена \
                 {sum} Вт. Полная мощность — в «Производительности» и «Экстриме»: для «Экстрима» с пределом \
                 температуры процессора клонируйте «Экстрим» и включите предел в профиле.",
                sum = ctgp + ppab
            ))
            .class("row-desc"),
        );
    }
    col
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
                Button::new(t!("Все — авто"))
                    .icon(icons::AUTO)
                    .disabled(!custom)
                    .on_click(move || {
                        let map = cr.sink.fans.get_untracked().iter().map(|f| (f.index, 0)).collect();
                        cr.fan_edit.set(map);
                    })
                    .class("btn-ghost"),
            )
            .child(
                Button::new(t!("Применить"))
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
        t!("Вентиляторы"),
        actions,
        reactive_box(move || {
            let list = ctx.sink.fans.get();
            if list.is_empty() {
                return Box::new(Text::new(t!("Вентиляторы не найдены (нужен драйвер lenovo-wmi-other).")).class("muted"));
            }
            let custom = ctx.power_mode() == Some(PowerMode::Custom);
            let edit = ctx.fan_edit.get();
            let (svc, saved) = ctx.sink.autoapply.get();
            let mut col = Column::new().gap(22.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            let c = ctx.clone();
            let desc = if saved.fan_curve && !(svc.enabled && svc.active) {
                t!("Включено, но служба не запущена — включите её в разделе «Служба» ниже.")
            } else if saved.fan_curve {
                t!("Служба ведёт вентиляторы по температуре CPU и видеокарты во всех режимах; \
                 в «Свой» заданные здесь обороты держатся как есть (выше 95 °C — не ниже кривой).")
            } else {
                t!("EC Legion Pro 7 Gen 10 после ручного задания оборотов не возвращается к своей кривой: \
                 ни по «авто», ни по смене режима, ни после сна и перезагрузки — только после сброса EC \
                 (выключить, отсоединить питание, подержать кнопку питания 30 с). Если вы задавали \
                 обороты вручную — включите, и вентиляторы снова будут следовать температуре.")
            };
            col = col.child(setting_row(
                t!("Программное авто (по температуре)"),
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
                let target_text = if auto { t!("авто").to_string() } else { t!("{target} об/мин", target = target) };
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
                                        .child(Text::new(format!("{} · №{}", t!(f.label), f.index)).class("row-title"))
                                        .child(
                                            Text::new(if changed {
                                                t!("Сейчас {rpm} об/мин · цель: {target} (не применено)", rpm = f.rpm, target = target_text)
                                            } else {
                                                t!("Сейчас {rpm} об/мин · цель: {target}", rpm = f.rpm, target = target_text)
                                            })
                                            .class(if changed { "row-desc changed" } else { "row-desc" }),
                                        )
                                        .class("grow"),
                                )
                                .child(Text::new(t!("Авто")).class("field-label"))
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
                                .child(Text::new(t!("{min} об/мин", min = f.min)).class("scale-label"))
                                .child(DecoratedBox::new().class("grow"))
                                .child(Text::new(t!("{max} об/мин", max = f.max)).class("scale-label")),
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
        t!("Служба"),
        || Text::new("").class("card-hint"),
        reactive_box(move || {
            let (svc, saved) = ctx.sink.autoapply.get();
            let tunables = ctx.sink.tunables.get();
            let fans = ctx.sink.fans.get();
            let mut col = Column::new().gap(16.0).cross_axis_alignment(CrossAxisAlignment::Stretch);

            let c = ctx.clone();
            let desc = if !svc.installed {
                t!("Служба ставится вместе с пакетом. При сборке из исходников скопируйте \
                 packaging/linux-legion-autoapply.service в ~/.config/systemd/user/.")
            } else if svc.enabled && svc.active {
                t!("Служба работает: восстанавливает значения «Своего» при входе, при переключении в «Свой» \
                 и после сна, держит лимиты CPU и EPP по режиму, ведёт вентиляторы по температуре.")
            } else if svc.enabled {
                t!("Служба включена, но сейчас не запущена — проверьте journalctl --user -u linux-legion-autoapply.")
            } else {
                t!("Прошивка забывает лимиты и обороты после перезагрузки, а лимиты CPU в RAPL вообще никто, \
                 кроме службы, не выставляет. Служба делает это сама (linux-legion --daemon).")
            };
            col = col.child(setting_row(
                t!("Служба linux-legion"),
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
                    let label = t.map_or(name.clone(), |t| t!(&t.label));
                    format!("{label}: {v} {}", t.map_or(String::new(), |t| t!(t.unit)))
                })
                .collect();
            items.extend(saved.fans.iter().map(|(i, rpm)| {
                let label = fans.iter().find(|f| f.index == *i).map_or(t!("Вентилятор"), |f| t!(f.label));
                let v = if *rpm == 0 { t!("авто").to_string() } else { t!("{rpm} об/мин", rpm = rpm) };
                t!("Вентилятор «{label}»: {value}", label = label, value = v)
            }));
            if items.is_empty() {
                col = col.child(
                    Text::new(
                        t!("Пока ничего не сохранено: значения запоминаются кнопками «Применить» в режиме «Свой»."),
                    )
                    .class("muted"),
                );
            } else {
                let mut list = Column::new().gap(2.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
                for it in items {
                    let (k, v) = it.rsplit_once(": ").unwrap_or((it.as_str(), ""));
                    list = list.child(crate::ui::widgets::kv(k, v.to_string()));
                }
                col = col.child(Text::new(t!("Сохранённые значения")).class("field-label")).child(list);
            }
            Box::new(col)
        }),
    )
}

/// Лимиты CPU (RAPL) и EPP по режиму — то, что в Windows делает Legion Space.
fn cpu(ctx: AppCtx) -> impl Widget {
    card_with(
        icons::MEMORY,
        t!("Процессор по режимам"),
        || Text::new("").class("card-hint"),
        reactive_box(move || {
            let st = ctx.sink.cpu.get();
            let (_, saved) = ctx.sink.autoapply.get();
            let power = ctx.sink.power.get();
            if !st.rapl.present() && st.epp.current.is_none() {
                return Box::new(Text::new(t!("RAPL (powercap) и EPP (cpufreq) недоступны.")).class("muted"));
            }
            let mut col = Column::new().gap(16.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            let fmt = |l: Option<crate::hw::rapl::Limits>| match l {
                Some(l) => t!("{pl1} / {pl2} Вт", pl1 = l.pl1, pl2 = l.pl2),
                None => "—".to_string(),
            };
            if st.rapl.present() {
                let c = ctx.clone();
                let desc = if !st.rapl.writable {
                    t!("Нет прав на запись в powercap — обновите udev-правило пакета (группа wheel).")
                } else {
                    t!("BIOS при загрузке ставит в RAPL MMIO 30/30 Вт и больше их не меняет: без этого \
                     процессор во всех режимах упирается в 30 Вт. Служба держит в RAPL (MSR и MMIO) \
                     заводские лимиты режима, а в «Свой» — лимиты, заданные выше.")
                };
                col = col.child(setting_row(
                    t!("Лимиты CPU по режиму (RAPL)"),
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
                list = list.child(kv(t!("Сейчас в RAPL: MSR"), fmt(st.rapl.msr)));
                list = list.child(kv(t!("Сейчас в RAPL: MMIO"), fmt(st.rapl.mmio)));
                col = col.child(Text::new(t!("PL1 / PL2 по режимам (свои — rapl.<режим>=PL1/PL2 в custom.conf)")).class("field-label")).child(list);
            }
            if let Some(cur) = st.epp.current.clone() {
                let c = ctx.clone();
                let desc = if !st.epp.writable {
                    t!("Нет прав на запись EPP — обновите udev-правило пакета (группа wheel).")
                } else {
                    t!("performance в Производительность/Экстрим/Свой, balance_performance в Балансе, \
                     balance_power в Тихом. Перекрывает tuned и power-profiles-daemon. Без performance \
                     процессор не добирает верхние ступени turbo.")
                };
                let want = power.mode.map(|m| m.epp()).unwrap_or("—");
                col = col.child(setting_row(
                    t!("EPP по режиму"),
                    desc,
                    Box::new(
                        Toggle::new()
                            .on(saved.epp)
                            .on_change(move |v| c.send(Job::SetFlag(Flag::Epp, v)))
                            .class(if st.epp.writable { "" } else { "toggle-off" }),
                    ),
                ));
                let now = if st.epp.pinned() {
                    t!("{cur} — governor performance, EPP прибит к performance", cur = cur)
                } else {
                    t!("{cur} (для режима: {want})", cur = cur, want = want)
                };
                col = col.child(kv(t!("EPP сейчас"), now));
            }
            Box::new(col)
        }),
    )
}

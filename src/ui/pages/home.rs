//! Главная: режим питания в один клик и мониторинг.

use crate::hw::battery::Status;
use crate::hw::power::PowerMode;
use crate::hw::sensors::Gpu;
use crate::ui::app::AppCtx;
use crate::ui::icons;
use crate::ui::widgets::{bar, card_with, load_color, reactive, reactive_box, ring, temp_color};
use crate::worker::Job;
use syngui::prelude::*;
use syngui::widgets::*;
use syngui::CursorIcon;

pub fn mode_icon(m: PowerMode) -> &'static str {
    match m {
        PowerMode::Quiet => icons::MODE_QUIET,
        PowerMode::Balanced => icons::MODE_BALANCED,
        PowerMode::Performance => icons::MODE_PERF,
        PowerMode::Extreme => icons::MODE_EXTREME,
        PowerMode::Custom => icons::MODE_CUSTOM,
    }
}

pub fn view(ctx: AppCtx) -> impl Widget {
    Column::new()
        .gap(18.0)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .child(hero(ctx.clone()))
        .child(
            Row::new()
                .gap(18.0)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .child(DecoratedBox::new().child(cpu_card(ctx.clone())).class("grow"))
                .child(DecoratedBox::new().child(gpu_card(ctx.clone())).class("grow"))
                .child(DecoratedBox::new().child(mem_card(ctx.clone())).class("grow"))
                .child(DecoratedBox::new().child(battery_card(ctx.clone())).class("grow")),
        )
        .child(fans_card(ctx))
        .class("page")
}

fn hero(ctx: AppCtx) -> impl Widget {
    let info = ctx.sink.info;
    DecoratedBox::new()
        .child(
            Column::new()
                .gap(20.0)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .child(move || {
                    let i = info.get();
                    Column::new()
                        .gap(6.0)
                        .child(Text::new("LENOVO LEGION").class("hero-kicker"))
                        .child(Text::new(if i.model.is_empty() { "Legion".into() } else { i.model.clone() }).class("hero-title"))
                        .child(
                            Text::new(format!(
                                "{} · {} · {:.0} ГБ ОЗУ",
                                if i.cpu.is_empty() { "CPU" } else { &i.cpu },
                                if i.gpu.is_empty() { "GPU" } else { &i.gpu },
                                i.memory_gb.round()
                            ))
                            .class("hero-sub"),
                        )
                })
                .child(mode_strip(ctx)),
        )
        .class("hero")
}

/// Быстрый выбор режима питания (как Fn+Q).
pub fn mode_strip(ctx: AppCtx) -> impl Widget {
    reactive(move || {
        let st = ctx.sink.power.get();
        let mut row = Row::new().gap(10.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
        let modes = if st.choices.is_empty() { PowerMode::ALL.to_vec() } else { st.choices.clone() };
        for m in modes {
            let c = ctx.clone();
            let on = st.mode == Some(m);
            let cls = if on { format!("mode-chip mode-chip-on chip-{}", m.tone()) } else { "mode-chip".to_string() };
            row = row.child(
                GestureDetector::new()
                    .on_click(move || c.send(Job::SetPowerMode(m)))
                    .cursor(CursorIcon::Pointer)
                    .child(
                        DecoratedBox::new()
                            .child(
                                Row::new()
                                    .gap(8.0)
                                    .cross_axis_alignment(CrossAxisAlignment::Center)
                                    .child(Icon::new(mode_icon(m)).class(&format!("mode-chip-icon tone-{}", m.tone())))
                                    .child(Text::new(m.label()).class("mode-chip-text")),
                            )
                            .class(&cls),
                    )
                    .class("grow"),
            );
        }
        row
    })
}

fn stat_card(icon: &str, title: &str, body: Box<dyn Widget>) -> impl Widget {
    DecoratedBox::new()
        .child(
            Column::new()
                .gap(14.0)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .child(
                    Row::new()
                        .gap(8.0)
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .child(Icon::new(icon).class("card-icon"))
                        .child(Text::new(title).class("card-title")),
                )
                .children(vec![body]),
        )
        .class("card stat-card")
}

fn line(label: &str, value: String) -> impl Widget {
    Row::new()
        .gap(8.0)
        .child(Text::new(label).class("stat-label"))
        .child(DecoratedBox::new().class("grow"))
        .child(Text::new(value).class("stat-value"))
        .class("stat-line")
}

fn cpu_card(ctx: AppCtx) -> impl Widget {
    reactive(move || {
        let s = ctx.sink.sensors.get();
        let temp = s.cpu_temp.unwrap_or(0.0);
        let body = Column::new()
            .gap(10.0)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .child(Center::new().child(ring(
                temp / 100.0,
                temp_color(temp),
                132.0,
                s.cpu_temp.map_or("—".into(), |t| format!("{t:.0}°")),
                "температура",
            )))
            .child(line("Загрузка", format!("{:.0} %", s.cpu_load)))
            .child(bar(s.cpu_load / 100.0, "accent"))
            .child(line("Частота", s.cpu_mhz.map_or("—".into(), |m| format!("{:.2} ГГц", m as f32 / 1000.0))));
        stat_card(icons::MEMORY, "Процессор", Box::new(body))
    })
}

fn gpu_card(ctx: AppCtx) -> impl Widget {
    reactive(move || {
        let s = ctx.sink.sensors.get();
        let body: Box<dyn Widget> = match s.gpu {
            Gpu::Active(g) => {
                let temp = g.temp.unwrap_or(0.0);
                let load = g.load.unwrap_or(0.0);
                Box::new(
                    Column::new()
                        .gap(10.0)
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .child(Center::new().child(ring(
                            temp / 100.0,
                            temp_color(temp),
                            132.0,
                            g.temp.map_or("—".into(), |t| format!("{t:.0}°")),
                            "температура",
                        )))
                        .child(line("Загрузка", format!("{load:.0} %")))
                        .child(bar(load / 100.0, "accent"))
                        .child(line("Мощность", g.power_w.map_or("—".into(), |p| format!("{p:.0} Вт")))),
                )
            }
            Gpu::Sleeping => Box::new(
                Column::new()
                    .gap(10.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .child(ring(0.0, Color::TRANSPARENT, 132.0, "zZ".into(), "спит"))
                    .child(Text::new("Дискретная видеокарта отключена для экономии энергии").class("muted-center")),
            ),
            Gpu::Absent => Box::new(
                Column::new()
                    .gap(10.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .child(ring(0.0, Color::TRANSPARENT, 132.0, "—".into(), "нет данных"))
                    .child(Text::new("Данные появятся через пару секунд").class("muted-center")),
            ),
        };
        stat_card(icons::GPU, "Видеокарта", body)
    })
}

fn mem_card(ctx: AppCtx) -> impl Widget {
    reactive(move || {
        let s = ctx.sink.sensors.get();
        let frac = if s.mem_total_gb > 0.0 { s.mem_used_gb / s.mem_total_gb } else { 0.0 };
        let body = Column::new()
            .gap(10.0)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .child(Center::new().child(ring(frac, load_color(frac), 132.0, format!("{:.0}%", frac * 100.0), "занято")))
            .child(line("Используется", format!("{:.1} ГБ", s.mem_used_gb)))
            .child(line("Всего", format!("{:.1} ГБ", s.mem_total_gb)));
        stat_card(icons::LAYERS, "Память", Box::new(body))
    })
}

fn battery_card(ctx: AppCtx) -> impl Widget {
    reactive_box(move || {
        let Some(b) = ctx.sink.battery.get() else {
            return Box::new(stat_card(icons::BATTERY_FULL, "Батарея", Box::new(Text::new("Нет батареи").class("muted"))));
        };
        let color = match b.percent {
            p if p <= 15 => Color::from_hex("#f43f5e"),
            p if p <= 35 => Color::from_hex("#fbbf24"),
            _ => Color::from_hex("#34d399"),
        };
        let time = match (b.status, b.minutes_left()) {
            (Status::Discharging, Some(m)) => format!("{} ч {:02} мин", m / 60, m % 60),
            (Status::Charging, Some(m)) => format!("до полной {} ч {:02} мин", m / 60, m % 60),
            _ => "—".into(),
        };
        let body = Column::new()
            .gap(10.0)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .child(Center::new().child(ring(b.percent as f32 / 100.0, color, 132.0, format!("{}%", b.percent), b.status.label())))
            .child(line("Питание", if b.ac_online { "от сети".into() } else { "от батареи".into() }))
            .child(line("Осталось", time));
        Box::new(stat_card(if b.ac_online { icons::BATTERY_CHARGING } else { icons::BATTERY_FULL }, "Батарея", Box::new(body)))
    })
}

fn fans_card(ctx: AppCtx) -> impl Widget {
    let c = ctx.clone();
    card_with(
        icons::FAN,
        "Вентиляторы",
        move || {
            let custom = c.power_mode() == Some(PowerMode::Custom);
            Text::new(if custom { "ручное управление" } else { "автоматически" }).class("card-hint")
        },
        reactive(move || {
            let fans = ctx.sink.fans.get();
            let mut row = Row::new().gap(24.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            if fans.is_empty() {
                return row.child(Text::new("Драйвер lenovo-wmi-other не отдаёт данные о вентиляторах").class("muted"));
            }
            for f in fans {
                let frac = if f.max > 0 { f.rpm as f32 / f.max as f32 } else { 0.0 };
                row = row.child(
                    Column::new()
                        .gap(8.0)
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .child(
                            Row::new()
                                .gap(8.0)
                                .child(Text::new(f.label).class("stat-label"))
                                .child(DecoratedBox::new().class("grow"))
                                .child(Text::new(format!("{} об/мин", f.rpm)).class("stat-value")),
                        )
                        .child(bar(frac, "fan"))
                        .class("grow"),
                );
            }
            row
        }),
    )
}

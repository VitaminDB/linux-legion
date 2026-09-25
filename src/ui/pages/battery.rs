//! Батарея: режим зарядки, состояние и износ аккумулятора.

use crate::hw::battery::{ChargeType, Status};
use crate::ui::app::AppCtx;
use crate::ui::icons;
use crate::ui::widgets::{access_banner, bar, card, kv, nothing, page_header, reactive_box, ring};
use crate::worker::Job;
use syngui::prelude::*;
use syngui::widgets::*;
use syngui::CursorIcon;

fn charge_icon(t: ChargeType) -> &'static str {
    match t {
        ChargeType::Standard => icons::BATTERY_FULL,
        ChargeType::Fast => icons::BOLT,
        ChargeType::LongLife => icons::BATTERY_SAVER,
    }
}

pub fn view(ctx: AppCtx) -> impl Widget {
    Column::new()
        .gap(18.0)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .child(page_header("Батарея", "Режим зарядки и здоровье аккумулятора."))
        .child(reactive_box(move || {
            let Some(b) = ctx.sink.battery.get() else {
                return Box::new(Text::new("Батарея не найдена.").class("muted"));
            };
            let health = b.health();
            let time = match (b.status, b.minutes_left()) {
                (Status::Discharging, Some(m)) => format!("{} ч {:02} мин до разряда", m / 60, m % 60),
                (Status::Charging, Some(m)) => format!("{} ч {:02} мин до полного заряда", m / 60, m % 60),
                _ => String::new(),
            };
            let pct_color = match b.percent {
                p if p <= 15 => Color::from_hex("#f43f5e"),
                p if p <= 35 => Color::from_hex("#fbbf24"),
                _ => Color::from_hex("#34d399"),
            };

            let status = Row::new()
                .gap(28.0)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .child(ring(b.percent as f32 / 100.0, pct_color, 170.0, format!("{}%", b.percent), b.status.label()))
                .child(
                    Column::new()
                        .gap(10.0)
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .child(
                            Row::new()
                                .gap(10.0)
                                .cross_axis_alignment(CrossAxisAlignment::Center)
                                .child(Icon::new(if b.ac_online { icons::PLUG } else { icons::BATTERY_FULL }).class("card-icon"))
                                .child(
                                    Text::new(if b.ac_online { "Подключено к сети" } else { "Работа от батареи" })
                                        .class("big-line"),
                                ),
                        )
                        .child(if time.is_empty() { nothing() } else { Box::new(Text::new(time).class("row-desc")) })
                        .child(kv(
                            if b.status == Status::Charging { "Мощность заряда" } else { "Потребление" },
                            if b.power_w > 0.1 { format!("{:.1} Вт", b.power_w) } else { "—".into() },
                        ))
                        .child(kv("Запас энергии", format!("{:.1} из {:.1} Вт·ч", b.energy_now_wh, b.energy_full_wh)))
                        .child(kv("Напряжение", format!("{:.2} В", b.voltage_v)))
                        .class("grow"),
                );

            let mut modes = Row::new().gap(12.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            for t in ChargeType::ALL {
                if !b.charge_types.contains(&t) {
                    continue;
                }
                let on = b.charge_type == Some(t);
                let c = ctx.clone();
                modes = modes.child(
                    GestureDetector::new()
                        .on_click(move || c.send(Job::SetChargeType(t)))
                        .cursor(CursorIcon::Pointer)
                        .child(
                            DecoratedBox::new()
                                .child(
                                    Column::new()
                                        .gap(10.0)
                                        .child(
                                            DecoratedBox::new()
                                                .child(Center::new().child(Icon::new(charge_icon(t)).class("mode-icon")))
                                                .class(if on { "mode-icon-badge badge-on" } else { "mode-icon-badge" }),
                                        )
                                        .child(Text::new(t.label()).class("mode-label"))
                                        .child(Text::new(t.hint()).class("mode-hint")),
                                )
                                .class(if on { "mode-tile mode-tile-on tile-accent" } else { "mode-tile" }),
                        )
                        .class("grow"),
                );
            }

            let mut col = Column::new().gap(18.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            if !b.charge_types.is_empty() && !b.charge_type_writable {
                col = col.child(access_banner());
            }
            col = col.child(card("Состояние", "", status));
            if !b.charge_types.is_empty() {
                col = col.child(card("Режим зарядки", "Как в Vantage: быстрая зарядка или сбережение аккумулятора.", modes));
            }

            let mut health_col = Column::new().gap(10.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            if let Some(h) = health {
                health_col = health_col
                    .child(
                        Row::new()
                            .gap(8.0)
                            .child(Text::new("Остаток ёмкости").class("stat-label"))
                            .child(DecoratedBox::new().class("grow"))
                            .child(Text::new(format!("{h:.0} %")).class("stat-value")),
                    )
                    .child(bar(h / 100.0, if h > 80.0 { "ok" } else if h > 60.0 { "warn" } else { "bad" }));
            }
            health_col = health_col
                .child(kv("Ёмкость сейчас", format!("{:.1} Вт·ч", b.energy_full_wh)))
                .child(kv("Ёмкость новой батареи", format!("{:.1} Вт·ч", b.energy_design_wh)))
                .child(kv("Циклов заряда", b.cycles.map_or("—".into(), |c| c.to_string())))
                .child(kv("Модель", format!("{} {}", b.manufacturer, b.model).trim().to_string()));
            col = col.child(card("Здоровье аккумулятора", "", health_col));
            Box::new(col)
        }))
        .class("page")
}

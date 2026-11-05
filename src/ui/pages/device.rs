//! Устройство: аппаратные переключатели и сведения о системе.

use crate::hw::device::Toggle as Switch;
use crate::ui::app::AppCtx;
use crate::ui::icons;
use crate::ui::widgets::{access_banner, card, kv, page_header, reactive, reactive_box, setting_row};
use crate::worker::Job;
use syngui::prelude::*;

fn toggle_icon(t: Switch) -> &'static str {
    match t {
        Switch::FnLock => icons::LOCK,
        Switch::Camera => icons::VIDEOCAM,
        Switch::UsbCharging => icons::USB,
    }
}

pub fn view(ctx: AppCtx) -> impl Widget {
    let (c_t, c_i) = (ctx.clone(), ctx);
    Column::new()
        .gap(18.0)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .child(page_header(t!("Устройство"), t!("Аппаратные переключатели и сведения о ноутбуке.")))
        .child(reactive_box(move || {
            let t = c_t.sink.toggles.get();
            if t.items.is_empty() {
                return Box::new(card(t!("Переключатели"), "", Text::new(t!("Драйвер ideapad_acpi не найден.")).class("muted")));
            }
            let mut col = Column::new().gap(18.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            if t.items.iter().any(|i| !i.2) {
                col = col.child(access_banner());
            }
            let mut rows = Column::new().gap(20.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            for (kind, on, writable) in t.items {
                let c = c_t.clone();
                rows = rows.child(
                    Row::new()
                        .gap(14.0)
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .child(
                            DecoratedBox::new()
                                .child(Center::new().child(Icon::new(toggle_icon(kind)).class("row-icon")))
                                .class("row-icon-badge"),
                        )
                        .child(
                            DecoratedBox::new()
                                .child(setting_row(
                                    kind.label(),
                                    kind.hint(),
                                    Box::new(
                                        Toggle::new()
                                            .on(on)
                                            .on_change(move |v| c.send(Job::SetToggle(kind, v)))
                                            .class(if writable { "" } else { "toggle-off" }),
                                    ),
                                ))
                                .class("grow"),
                        ),
                );
            }
            Box::new(col.child(card(t!("Переключатели"), "", rows)))
        }))
        .child(card(
            t!("О системе"),
            "",
            reactive(move || {
                let i = c_i.sink.info.get();
                let rgb = c_i.sink.rgb.get();
                Column::new()
                    .gap(2.0)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .child(kv(t!("Модель"), format!("{} {}", i.vendor, i.model)))
                    .child(kv(t!("Машинный тип"), i.machine_type.clone()))
                    .child(kv("BIOS", format!("{} ({})", i.bios, i.bios_date)))
                    .child(kv(t!("Процессор"), t!("{cpu} · {cpu_threads} потоков", cpu = i.cpu, cpu_threads = i.cpu_threads)))
                    .child(kv(t!("Видеокарта"), if i.gpu.is_empty() { "—".into() } else { i.gpu.clone() }))
                    .child(kv(t!("Память"), t!("{memory_gb} ГБ", memory_gb = format!("{:.1}", i.memory_gb))))
                    .child(kv(t!("Контроллер подсветки"), if rgb.name.is_empty() { "—".into() } else { rgb.name.clone() }))
                    .child(kv(t!("Система"), t!("{os} · ядро {kernel}", os = i.os, kernel = i.kernel)))
            }),
        ))
        .class("page")
}

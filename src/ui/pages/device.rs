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
        .child(page_header(t!("Устройство", "Device"), t!("Аппаратные переключатели и сведения о ноутбуке.", "Hardware switches and laptop details.")))
        .child(reactive_box(move || {
            let t = c_t.sink.toggles.get();
            if t.items.is_empty() {
                return Box::new(card(t!("Переключатели", "Switches"), "", Text::new(t!("Драйвер ideapad_acpi не найден.", "ideapad_acpi driver not found.")).class("muted")));
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
            Box::new(col.child(card(t!("Переключатели", "Switches"), "", rows)))
        }))
        .child(card(
            t!("О системе", "About"),
            "",
            reactive(move || {
                let i = c_i.sink.info.get();
                let rgb = c_i.sink.rgb.get();
                Column::new()
                    .gap(2.0)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .child(kv(t!("Модель", "Model"), format!("{} {}", i.vendor, i.model)))
                    .child(kv(t!("Машинный тип", "Machine type"), i.machine_type.clone()))
                    .child(kv("BIOS", format!("{} ({})", i.bios, i.bios_date)))
                    .child(kv(t!("Процессор", "CPU"), tf!("{} · {} потоков", "{} · {} threads", i.cpu, i.cpu_threads)))
                    .child(kv(t!("Видеокарта", "GPU"), if i.gpu.is_empty() { "—".into() } else { i.gpu.clone() }))
                    .child(kv(t!("Память", "Memory"), tf!("{:.1} ГБ", "{:.1} GB", i.memory_gb)))
                    .child(kv(t!("Контроллер подсветки", "Lighting controller"), if rgb.name.is_empty() { "—".into() } else { rgb.name.clone() }))
                    .child(kv(t!("Система", "System"), tf!("{} · ядро {}", "{} · kernel {}", i.os, i.kernel)))
            }),
        ))
        .class("page")
}

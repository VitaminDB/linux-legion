//! linux-legion — центр управления ноутбуками Lenovo Legion в Linux:
//! режимы питания, лимиты мощности, вентиляторы, подсветка Spectrum, батарея.
//!
//! `linux_legion`              — работа с ноутбуком;
//! `linux_legion --simulate`   — демо-режим без железа;
//! `linux_legion --page N`     — открыть раздел N (0 главная … 4 устройство);
//! `linux_legion --apply`      — применить сохранённые значения режима «Свой»;
//! `linux_legion --daemon`     — служба автоприменения (см. `autoapply`);
//! `LEGION_TRACE=1`            — печатать пакеты подсветки в stderr.

mod autoapply;
mod hw;
mod spectrum;
mod ui;
mod worker;

use syngui::prelude::*;
use syngui::text::icon_fonts::material;
use ui::app::{build_root, AppCtx};

const STYLES: &str = include_str!("../styles/app.mss");
const ICON: &[u8] = include_bytes!("../assets/icon/linux-legion-256.png");

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let simulate = args.iter().any(|a| a == "--simulate");
    if simulate {
        let root = hw::sim::install();
        autoapply::set_dir(root.join("config"));
    }
    if args.iter().any(|a| a == "--daemon") {
        autoapply::daemon();
    }
    if args.iter().any(|a| a == "--apply") {
        match autoapply::apply() {
            Ok(done) if done.is_empty() => println!("нечего применять (режим не «Свой» или значения уже стоят)"),
            Ok(done) => println!("применено: {}", done.join(", ")),
            Err(e) => {
                eprintln!("ошибка: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    // Сигналы создаются один раз, до run().
    let ctx = AppCtx::new(simulate);
    if let Some(n) = args.iter().position(|a| a == "--page").and_then(|i| args.get(i + 1)?.parse().ok()) {
        ctx.go(n);
    }

    App::new()
        .title("Legion Control")
        .app_id("linux-legion")
        .with_window_icon_png(ICON)
        .size(1360, 900)
        .min_size(1180, 720)
        .with_icon_font(material::FONT_DATA)
        .with_styles_str(STYLES)
        .run(move |_| {
            ctx.start();
            Box::new(build_root(ctx.clone()))
        });
}

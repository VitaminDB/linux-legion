//! linux-legion — центр управления ноутбуками Lenovo Legion в Linux:
//! режимы питания, лимиты мощности, вентиляторы, подсветка Spectrum, батарея.
//!
//! `linux_legion`              — работа с ноутбуком;
//! `linux_legion --simulate`   — демо-режим без железа;
//! `linux_legion --page N`     — открыть раздел N (0 главная … 4 устройство);
//! `LEGION_TRACE=1`            — печатать пакеты подсветки в stderr.

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
        hw::sim::install();
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

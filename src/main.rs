//! linux-legion — центр управления ноутбуками Lenovo Legion в Linux:
//! режимы питания, лимиты мощности, вентиляторы, подсветка Spectrum, батарея.
//!
//! `linux_legion`              — работа с ноутбуком;
//! `linux_legion --simulate`   — демо-режим без железа;
//! `linux_legion --page N`     — открыть раздел N (0 главная … 4 устройство);
//! `linux_legion --screenshot F` — снимок окна в PNG без показа окна;
//! `LEGION_LANG=en`             — язык интерфейса (главнее выбора в шапке);
//! `linux_legion --apply`      — применить сохранённые значения режима «Свой»;
//! `linux_legion --daemon`     — служба автоприменения (см. `autoapply`);
//! `LEGION_TRACE=1`            — печатать пакеты подсветки в stderr.

#[macro_use]
extern crate syngui;

mod autoapply;
mod hw;
mod i18n;
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
    i18n::init();
    if args.iter().any(|a| a == "--daemon") {
        autoapply::daemon();
    }
    if args.iter().any(|a| a == "--apply") {
        match autoapply::apply() {
            Ok(done) if done.is_empty() => println!("{}", t!("нечего применять (режим не «Свой» или значения уже стоят)")),
            Ok(done) => println!("{}", t!("применено: {list}", list = done.join(", "))),
            Err(e) => {
                eprintln!("{}", t!("ошибка: {e}", e = e));
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
    if let Some(out) = args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1)) {
        let opts = syngui::shot::ShotOptions { wait: std::time::Duration::from_millis(2500), ..Default::default() };
        let build = move || -> Box<dyn Widget> {
            ctx.start();
            Box::new(build_root(ctx.clone()))
        };
        if let Err(e) = syngui::shot::render_png_with(build, STYLES, (1360, 900), 1.0, out, opts) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
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
            // Смена языка (выбор в шапке) — перестроить всё окно.
            let ctx = ctx.clone();
            Box::new(ui::widgets::reactive(move || {
                syngui::i18n::subscribe();
                build_root(ctx.clone())
            }))
        });
}

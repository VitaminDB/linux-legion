//! Язык интерфейса. Код пишется по-русски, строки — `t!("…")`/`n_!` (syngui
//! i18n, ключ — сама строка); переводы — `i18n/<язык>.lang`, собираются
//! `syngui/scripts/i18n-extract.py i18n/en.lang src` (и для остальных языков).
//!
//! Язык: `LEGION_LANG`, иначе выбранный в шапке (`~/.config/linux-legion/ui.conf`,
//! `language=en`), иначе язык системы (`LANG`).

use std::path::PathBuf;

/// Язык исходных строк.
pub const SOURCE: &str = "ru";

const CATALOGS: &[&str] = &[
    include_str!("../i18n/ru.lang"),
    include_str!("../i18n/en.lang"),
    include_str!("../i18n/de.lang"),
    include_str!("../i18n/fr.lang"),
    include_str!("../i18n/es.lang"),
    include_str!("../i18n/it.lang"),
    include_str!("../i18n/pt-BR.lang"),
    include_str!("../i18n/pl.lang"),
    include_str!("../i18n/uk.lang"),
    include_str!("../i18n/kk.lang"),
    include_str!("../i18n/tr.lang"),
    include_str!("../i18n/zh-CN.lang"),
    include_str!("../i18n/ja.lang"),
    include_str!("../i18n/ko.lang"),
];

const ENV: &str = "LEGION_LANG";

fn path() -> PathBuf {
    crate::autoapply::dir().join("ui.conf")
}

/// Язык, выбранный в программе (пусто — как в системе).
pub fn saved() -> String {
    std::fs::read_to_string(path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == "language")
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_default()
}

/// Язык задан переменной окружения — она главнее выбора в программе.
pub fn forced() -> Option<String> {
    std::env::var(ENV).ok().map(|l| l.trim().to_string()).filter(|l| !l.is_empty())
}

fn requested() -> String {
    forced()
        .or_else(|| Some(saved()).filter(|l| !l.is_empty()))
        .unwrap_or_else(|| syngui::i18n::system_language().to_string())
}

/// Подключить каталоги и выставить язык. Вызывать до вывода любых строк.
pub fn init() {
    syngui::i18n::set_source_language(SOURCE);
    syngui::i18n::register_catalogs(CATALOGS);
    syngui::i18n::set_language(requested().as_str());
}

/// Выбрать язык (пусто — как в системе): запомнить и сразу применить —
/// подписанный на язык корень окна перестраивается.
pub fn choose(tag: &str) {
    let p = path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::write(&p, format!("language={tag}\n")) {
        eprintln!("linux-legion: {}: {e}", p.display());
    }
    syngui::i18n::set_language(requested().as_str());
}

/// Языки для выбора: (тег, название на самом языке), по названию.
pub fn choices() -> Vec<(String, String)> {
    let mut list: Vec<(String, String)> =
        syngui::i18n::languages().into_iter().map(|l| (l.tag.tag().to_string(), l.name)).collect();
    list.sort_by(|a, b| a.1.cmp(&b.1));
    list
}

/// Название языка системы (для пункта «Как в системе»).
pub fn system_name() -> String {
    let sys = syngui::i18n::system_language();
    choices()
        .into_iter()
        .find(|(tag, _)| syngui::i18n::Lang::new(tag).base() == sys.base())
        .map(|(_, name)| name)
        .unwrap_or_else(|| sys.to_string())
}

#[cfg(test)]
mod tests {
    use crate::ui::app::{build_root, AppCtx};
    use crate::ui::widgets::reactive;
    use syngui::prelude::*;
    use syngui::render::DrawCommand;
    use syngui::testing::*;

    const STYLES: &str = include_str!("../styles/app.mss");

    fn texts(h: &mut TestHarness) -> Vec<String> {
        h.paint()
            .iter_all_commands()
            .filter_map(|c| match c {
                DrawCommand::Text { text, .. } => Some(text.to_string()),
                _ => None,
            })
            .collect()
    }

    fn settle(h: &mut TestHarness) {
        for _ in 0..3 {
            h.rebuild();
            h.apply_mss_dirty(STYLES);
            h.layout(1360.0, 900.0);
        }
    }

    /// Все каталоги разбираются, и в каждом переведено всё.
    #[test]
    fn catalogs_parse_and_complete() {
        for text in super::CATALOGS {
            let c = syngui::i18n::Catalog::parse(text).expect("каталог разбирается");
            let empty = text.lines().filter(|l| l.ends_with("= \"\"")).count();
            assert_eq!(empty, 0, "{}: непереведённых строк {empty}", c.tag);
        }
    }

    /// Выбор языка в шапке перестраивает всё окно на лету и запоминается.
    #[test]
    fn language_switch_rebuilds_window() {
        let dir = std::env::temp_dir().join(format!("linux-legion-i18n-{}", std::process::id()));
        crate::autoapply::set_dir(dir.clone());
        let _ = TestHarness::new(Box::new(DecoratedBox::new()));
        super::init();
        super::choose("ru");
        let ctx = AppCtx::new(true);
        let mut h = TestHarness::new(Box::new(reactive(move || {
            syngui::i18n::subscribe();
            build_root(ctx.clone())
        })));
        h.apply_mss(STYLES);
        settle(&mut h);
        let ru = texts(&mut h);
        assert!(ru.iter().any(|t| t == "Главная"), "{ru:?}");

        super::choose("en");
        settle(&mut h);
        let en = texts(&mut h);
        assert!(en.iter().any(|t| t == "Home"), "{en:?}");
        assert!(en.iter().any(|t| t == "CONTROL CENTER"), "{en:?}");
        assert!(!en.iter().any(|t| t.chars().any(|c| ('а'..='я').contains(&c))), "русское в en: {en:?}");
        assert_eq!(super::saved(), "en");

        // «Как в системе» — пустое значение
        super::choose("ru");
        settle(&mut h);
        assert!(texts(&mut h).iter().any(|t| t == "Главная"));
        let _ = std::fs::remove_dir_all(dir);
    }
}

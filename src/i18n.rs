//! Язык интерфейса: русский или английский.
//!
//! Перевод стоит в коде рядом с оригиналом: `t!("Батарея", "Battery")` для
//! готовой строки и `tf!("{} Вт", "{} W", p)` вместо `format!`. Язык выбирается
//! один раз при запуске: `LINUX_LEGION_LANG=ru|en`, иначе по первой непустой из
//! `LC_ALL`, `LC_MESSAGES`, `LANG` — английский, если она начинается с `en`.
//! Во всех остальных случаях — русский.

use std::sync::OnceLock;

static ENGLISH: OnceLock<bool> = OnceLock::new();

/// Интерфейс на английском.
pub fn english() -> bool {
    *ENGLISH.get_or_init(|| {
        let lang = ["LINUX_LEGION_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|v| std::env::var(v).ok())
            .find(|v| !v.is_empty());
        lang.is_some_and(|v| v.starts_with("en"))
    })
}

/// Строка на текущем языке.
pub fn pick(ru: &'static str, en: &'static str) -> &'static str {
    if english() {
        en
    } else {
        ru
    }
}

/// Строка на текущем языке: `t!("русский", "English")`.
macro_rules! t {
    ($ru:literal, $en:literal $(,)?) => {
        $crate::i18n::pick($ru, $en)
    };
}

/// `format!` на текущем языке: `tf!("{} Вт", "{} W", p)`.
macro_rules! tf {
    ($ru:literal, $en:literal $(,)?) => {
        if $crate::i18n::english() { format!($en) } else { format!($ru) }
    };
    ($ru:literal, $en:literal, $($arg:tt)*) => {
        if $crate::i18n::english() { format!($en, $($arg)*) } else { format!($ru, $($arg)*) }
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn picks_one_language_for_both_macros() {
        let n = 3;
        let english = super::english();
        assert_eq!(t!("Батарея", "Battery"), if english { "Battery" } else { "Батарея" });
        assert_eq!(tf!("{n} зоны", "{n} zones"), if english { "3 zones" } else { "3 зоны" });
        assert_eq!(tf!("{} Вт", "{} W", n), if english { "3 W" } else { "3 Вт" });
    }
}

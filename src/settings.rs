//! Настройки интерфейса: `~/.config/linux-legion/ui.conf`, строки `ключ=значение`
//! (`language=en`, `tray=1`).

use std::path::PathBuf;

fn path() -> PathBuf {
    crate::autoapply::dir().join("ui.conf")
}

fn read_all() -> Vec<(String, String)> {
    std::fs::read_to_string(path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

pub fn get(key: &str) -> Option<String> {
    read_all().into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Записать значение, остальные строки сохранить.
pub fn set(key: &str, value: &str) {
    let mut all = read_all();
    match all.iter_mut().find(|(k, _)| k == key) {
        Some(e) => e.1 = value.to_string(),
        None => all.push((key.to_string(), value.to_string())),
    }
    let text: String = all.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
    let p = path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::write(&p, text) {
        eprintln!("linux-legion: {}: {e}", p.display());
    }
}

/// Закрытие окна прячет его в трей (по умолчанию — да).
pub fn tray() -> bool {
    get("tray").is_none_or(|v| v != "0")
}

pub fn set_tray(on: bool) {
    set("tray", if on { "1" } else { "0" });
    syngui::app::set_hide_on_close(on);
}

#[cfg(test)]
mod tests {
    #[test]
    fn keeps_other_keys() {
        // Каталог общий с другими тестами (OnceLock) — свои ключи.
        crate::autoapply::set_dir(std::env::temp_dir().join(format!("linux-legion-settings-{}", std::process::id())));
        super::set("test_key", "a");
        super::set("tray", "0");
        super::set("test_key", "b");
        assert_eq!(super::get("test_key").as_deref(), Some("b"));
        assert!(!super::tray());
    }
}

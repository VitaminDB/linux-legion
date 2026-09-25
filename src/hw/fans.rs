//! Вентиляторы: hwmon драйвера `lenovo-wmi-other`.
//!
//! `fanN_input` — текущие обороты, `fanN_target` — заданные (0 — авто),
//! `fanN_min`/`fanN_max` — допустимый диапазон, `fanN_div` — шаг.

use super::sysfs::{self, Result};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fan {
    pub index: u8,
    pub label: &'static str,
    pub rpm: u32,
    /// 0 — автоматическое управление.
    pub target: u32,
    pub min: u32,
    pub max: u32,
    pub step: u32,
    pub writable: bool,
}

fn hwmon() -> Option<PathBuf> {
    sysfs::find_dir("/sys/class/hwmon", |p| sysfs::read(&p.join("name")).as_deref() == Some("lenovo_wmi_other"))
}

fn label(index: u8) -> &'static str {
    match index {
        1 => "Процессор",
        2 => "Видеокарта",
        3 => "Дополнительный",
        4 => "Системный",
        _ => "Вентилятор",
    }
}

pub fn read_fans() -> Vec<Fan> {
    let Some(dir) = hwmon() else { return Vec::new() };
    (1..=8u8)
        .filter_map(|i| {
            let num = |f: &str| sysfs::read_num::<u32>(&dir.join(format!("fan{i}_{f}")));
            Some(Fan {
                index: i,
                label: label(i),
                rpm: num("input")?,
                target: num("target").unwrap_or(0),
                min: num("min").unwrap_or(0),
                max: num("max").unwrap_or(0),
                step: num("div").unwrap_or(100).max(1),
                writable: sysfs::writable(&dir.join(format!("fan{i}_target"))),
            })
        })
        .collect()
}

pub fn set_target(index: u8, rpm: u32) -> Result<()> {
    let dir = hwmon().ok_or_else(|| sysfs::Error::NotFound("hwmon lenovo_wmi_other".into()))?;
    sysfs::write(&dir.join(format!("fan{index}_target")), &rpm.to_string())
}

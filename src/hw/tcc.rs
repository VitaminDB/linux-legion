//! Предел температуры CPU через Intel TCC offset (`intel_tcc_cooling`).
//!
//! Процессор сбрасывает частоту при `TjMax − offset`. В отличие от атрибута
//! прошивки `cpu_temp` (меняется только в режиме «Свой»), TCC работает в любом
//! режиме — так получается «Экстрим» (175 Вт у видеокарты) с пределом
//! температуры процессора. BIOS сбрасывает offset при смене режима, поэтому
//! служба держит его сама.

use super::sysfs::{self, Result};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Tcc {
    /// TjMax, °C.
    pub tjmax: u32,
    /// Текущий offset, °C.
    pub offset: u32,
    /// Наибольший offset, который принимает драйвер.
    pub max_offset: u32,
    pub writable: bool,
}

impl Tcc {
    /// Температура срабатывания сейчас.
    pub fn limit(&self) -> u32 {
        self.tjmax.saturating_sub(self.offset)
    }

    /// Допустимый диапазон предела, °C: снизу — не ниже 70 °C.
    pub fn range(&self) -> (u32, u32) {
        (self.tjmax.saturating_sub(self.max_offset).max(70), self.tjmax)
    }
}

fn device() -> Option<PathBuf> {
    sysfs::find_dir("/sys/class/thermal", |p| {
        p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("cooling_device"))
            && sysfs::read(&p.join("type")).as_deref() == Some("TCC Offset")
    })
}

/// TjMax: `temp1_crit` пакета у coretemp (на Arrow Lake HX — 105 °C).
fn tjmax() -> Option<u32> {
    let dir = sysfs::find_dir("/sys/class/hwmon", |p| sysfs::read(&p.join("name")).as_deref() == Some("coretemp"))?;
    sysfs::read_num::<u32>(&dir.join("temp1_crit")).map(|v| v / 1000).filter(|&t| t > 50)
}

pub fn read() -> Option<Tcc> {
    let dev = device()?;
    let cur = dev.join("cur_state");
    Some(Tcc {
        tjmax: tjmax().unwrap_or(100),
        offset: sysfs::read_num(&cur)?,
        max_offset: sysfs::read_num(&dev.join("max_state")).unwrap_or(0),
        writable: sysfs::writable(&cur),
    })
}

/// Выставить предел (°C). Пишет только при отличии; `Ok(true)` — записано.
pub fn set_limit(limit: u32) -> Result<bool> {
    let dev = device().ok_or_else(|| sysfs::Error::NotFound("TCC Offset".into()))?;
    let Some(t) = read() else { return Err(sysfs::Error::NotFound(dev)) };
    let offset = t.tjmax.saturating_sub(limit).min(t.max_offset);
    if offset == t.offset {
        return Ok(false);
    }
    sysfs::write(&dev.join("cur_state"), &offset.to_string())?;
    Ok(true)
}

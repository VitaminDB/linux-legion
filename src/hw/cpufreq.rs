//! EPP — energy_performance_preference драйвера intel_pstate/amd-pstate:
//! насколько охотно процессор поднимает частоту. Значения:
//! `performance`, `balance_performance`, `balance_power`, `power`.
//! Governor при этом остаётся `powersave` (у intel_pstate это штатный
//! governor с HWP; `performance` просто прибивает EPP к performance).

use super::sysfs::{self, Result};
use std::path::PathBuf;

fn policies() -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(sysfs::sys("/sys/devices/system/cpu/cpufreq")) else { return Vec::new() };
    let mut v: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("policy")))
        .collect();
    v.sort();
    v
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Epp {
    pub current: Option<String>,
    pub choices: Vec<String>,
    /// scaling_governor; при `performance` EPP прибит к performance и не
    /// пишется (EBUSY) — так делает, например, tuned-ppd в режиме Производительность.
    pub governor: Option<String>,
    pub writable: bool,
}

impl Epp {
    pub fn pinned(&self) -> bool {
        self.governor.as_deref() == Some("performance")
    }
}

pub fn read() -> Epp {
    let Some(p0) = policies().into_iter().next() else { return Epp::default() };
    let f = p0.join("energy_performance_preference");
    Epp {
        current: sysfs::read(&f),
        choices: sysfs::read(&p0.join("energy_performance_available_preferences"))
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect(),
        governor: sysfs::read(&p0.join("scaling_governor")),
        writable: sysfs::writable(&f),
    }
}

/// Записать EPP во все policy. Возвращает true, если что-то поменялось.
/// Policy с governor `performance` пропускаются — там EPP и так performance.
pub fn set(pref: &str) -> Result<bool> {
    let mut changed = false;
    for p in policies() {
        if sysfs::read(&p.join("scaling_governor")).as_deref() == Some("performance") {
            continue;
        }
        let f = p.join("energy_performance_preference");
        if sysfs::read(&f).as_deref() == Some(pref) {
            continue;
        }
        sysfs::write(&f, pref)?;
        changed = true;
    }
    Ok(changed)
}

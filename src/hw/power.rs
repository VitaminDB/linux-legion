//! Режимы питания (Fn+Q) и лимиты мощности режима «Пользовательский».
//!
//! * режим — `platform-profile` драйвера `lenovo-wmi-gamezone`;
//! * лимиты — firmware attributes драйвера `lenovo-wmi-other`
//!   (`/sys/class/firmware-attributes/lenovo-wmi-other-0/attributes/*`).

use super::sysfs::{self, Result};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PowerMode {
    Quiet,
    Balanced,
    Performance,
    Extreme,
    Custom,
}

impl PowerMode {
    pub const ALL: [PowerMode; 5] =
        [PowerMode::Quiet, PowerMode::Balanced, PowerMode::Performance, PowerMode::Extreme, PowerMode::Custom];

    pub fn sysfs(self) -> &'static str {
        match self {
            PowerMode::Quiet => "low-power",
            PowerMode::Balanced => "balanced",
            PowerMode::Performance => "performance",
            PowerMode::Extreme => "max-power",
            PowerMode::Custom => "custom",
        }
    }

    pub fn from_sysfs(s: &str) -> Option<Self> {
        Some(match s {
            "low-power" | "quiet" | "cool" => PowerMode::Quiet,
            "balanced" => PowerMode::Balanced,
            "performance" | "balanced-performance" => PowerMode::Performance,
            "max-power" => PowerMode::Extreme,
            "custom" => PowerMode::Custom,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            PowerMode::Quiet => "Тихий",
            PowerMode::Balanced => "Баланс",
            PowerMode::Performance => "Производительность",
            PowerMode::Extreme => "Экстрим",
            PowerMode::Custom => "Свой",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            PowerMode::Quiet => "Тишина и холодный корпус, ниже производительность",
            PowerMode::Balanced => "Автоматический баланс шума и мощности",
            PowerMode::Performance => "Максимальная мощность для игр и работы",
            PowerMode::Extreme => "Мощность сверх штатной, вентиляторы на пределе",
            PowerMode::Custom => "Ваши лимиты мощности и обороты вентиляторов",
        }
    }

    /// Цвет индикатора на кнопке питания, как в Legion.
    pub fn tone(self) -> &'static str {
        match self {
            PowerMode::Quiet => "quiet",
            PowerMode::Balanced => "balanced",
            PowerMode::Performance => "perf",
            PowerMode::Extreme => "extreme",
            PowerMode::Custom => "custom",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PowerState {
    pub mode: Option<PowerMode>,
    pub choices: Vec<PowerMode>,
    pub writable: bool,
}

fn profile_dir() -> Option<PathBuf> {
    let by_name = |want: Option<&str>| {
        sysfs::find_dir("/sys/class/platform-profile", |p| {
            let name = sysfs::read(&p.join("name")).unwrap_or_default();
            want.is_none_or(|w| name == w)
        })
    };
    by_name(Some("lenovo-wmi-gamezone")).or_else(|| by_name(None))
}

/// Файлы профиля: (текущий, варианты).
fn profile_files() -> Option<(PathBuf, PathBuf)> {
    if let Some(d) = profile_dir() {
        return Some((d.join("profile"), d.join("choices")));
    }
    let legacy = sysfs::sys("/sys/firmware/acpi/platform_profile");
    legacy.exists().then(|| (legacy.clone(), legacy.with_file_name("platform_profile_choices")))
}

pub fn read_state() -> PowerState {
    let Some((cur, choices)) = profile_files() else { return PowerState::default() };
    let mode = sysfs::read(&cur).and_then(|s| PowerMode::from_sysfs(&s));
    let mut list: Vec<PowerMode> = sysfs::read(&choices)
        .unwrap_or_default()
        .split_whitespace()
        .filter_map(PowerMode::from_sysfs)
        .collect();
    list.sort_by_key(|m| PowerMode::ALL.iter().position(|x| x == m));
    list.dedup();
    PowerState { mode, choices: list, writable: sysfs::writable(&cur) }
}

pub fn set_mode(mode: PowerMode) -> Result<()> {
    let (cur, _) = profile_files().ok_or_else(|| sysfs::Error::NotFound("platform_profile".into()))?;
    sysfs::write(&cur, mode.sysfs())
}

// ---------------------------------------------------------------------------
// Лимиты мощности

#[derive(Clone, Debug, PartialEq)]
pub struct Tunable {
    pub name: String,
    pub label: String,
    pub hint: String,
    pub unit: &'static str,
    pub value: i64,
    pub min: i64,
    pub max: i64,
    pub step: i64,
    pub default: Option<i64>,
    pub writable: bool,
}

fn attrs_dir() -> Option<PathBuf> {
    sysfs::find_dir("/sys/class/firmware-attributes", |p| {
        p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("lenovo-wmi-other"))
    })
    .map(|d| d.join("attributes"))
}

/// Подписи известных атрибутов: (название, пояснение, единица).
fn describe(name: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match name {
        "ppt_pl1_spl" => ("CPU: длительная мощность (PL1)", "Мощность, которую процессор держит неограниченно долго.", "Вт"),
        "ppt_pl2_sppt" => ("CPU: кратковременная мощность (PL2)", "Всплеск мощности на десятки секунд под нагрузкой.", "Вт"),
        "ppt_pl3_fppt" => ("CPU: пиковая мощность (PL3)", "Короткие пики в миллисекунды.", "Вт"),
        "ppt_pl1_apu_spl" => ("APU: длительная мощность", "Лимит мощности встроенной графики.", "Вт"),
        "cpu_temp" => ("CPU: температурный предел", "Температура, при которой процессор сбрасывает частоту.", "°C"),
        "gpu_temp" => ("GPU: температурный предел", "Температура, при которой видеокарта сбрасывает частоту.", "°C"),
        "gpu_nv_ctgp" => ("GPU: настраиваемый TGP", "Базовая мощность видеокарты NVIDIA.", "Вт"),
        "gpu_nv_ppab" => ("GPU: Dynamic Boost", "Мощность, которую видеокарта забирает у процессора.", "Вт"),
        "gpu_nv_tpp" => ("GPU: полная мощность", "Суммарный лимит видеокарты.", "Вт"),
        "gpu_nv_cpu_boost" => ("GPU: буст при нагрузке CPU", "", "Вт"),
        _ => return None,
    })
}

pub fn read_tunables() -> Vec<Tunable> {
    let Some(dir) = attrs_dir() else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<Tunable> = rd
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if sysfs::read(&p.join("type")).as_deref() != Some("integer") {
                return None;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            let num = |f: &str| sysfs::read_num::<i64>(&p.join(f));
            let (label, hint, unit) = match describe(&name) {
                Some((l, h, u)) => (l.to_string(), h.to_string(), u),
                None => (sysfs::read(&p.join("display_name")).unwrap_or_else(|| name.clone()), String::new(), ""),
            };
            Some(Tunable {
                value: num("current_value")?,
                min: num("min_value")?,
                max: num("max_value")?,
                step: num("scalar_increment").unwrap_or(1).max(1),
                default: num("default_value"),
                writable: sysfs::writable(&p.join("current_value")),
                name,
                label,
                hint,
                unit,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn set_tunable(name: &str, value: i64) -> Result<()> {
    let dir = attrs_dir().ok_or_else(|| sysfs::Error::NotFound("firmware-attributes".into()))?;
    sysfs::write(&dir.join(name).join("current_value"), &value.to_string())
}

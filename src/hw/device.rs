//! Переключатели `ideapad_acpi` и сведения о системе.

use super::sysfs::{self, Result};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Toggle {
    FnLock,
    Camera,
    UsbCharging,
}

impl Toggle {
    pub const ALL: [Toggle; 3] = [Toggle::FnLock, Toggle::Camera, Toggle::UsbCharging];

    fn file(self) -> &'static str {
        match self {
            Toggle::FnLock => "fn_lock",
            Toggle::Camera => "camera_power",
            Toggle::UsbCharging => "usb_charging",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Toggle::FnLock => "Fn Lock",
            Toggle::Camera => "Питание веб-камеры",
            Toggle::UsbCharging => "Зарядка по USB в выключенном состоянии",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Toggle::FnLock => "F1–F12 работают как функциональные клавиши без удержания Fn.",
            Toggle::Camera => "Выключает камеру аппаратно — ни одна программа не сможет её включить.",
            Toggle::UsbCharging => "Порт USB с молнией заряжает телефон, даже когда ноутбук выключен или спит.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Toggles {
    /// (переключатель, состояние, можно ли менять)
    pub items: Vec<(Toggle, bool, bool)>,
}

fn ideapad_dir() -> Option<PathBuf> {
    sysfs::find_dir("/sys/bus/platform/drivers/ideapad_acpi", |p| p.join("fn_lock").exists() || p.join("camera_power").exists())
}

pub fn read_toggles() -> Toggles {
    let Some(d) = ideapad_dir() else { return Toggles::default() };
    Toggles {
        items: Toggle::ALL
            .iter()
            .filter_map(|&t| {
                let p = d.join(t.file());
                sysfs::read_bool(&p).map(|v| (t, v, sysfs::writable(&p)))
            })
            .collect(),
    }
}

pub fn set_toggle(t: Toggle, on: bool) -> Result<()> {
    let d = ideapad_dir().ok_or_else(|| sysfs::Error::NotFound("ideapad_acpi".into()))?;
    sysfs::write(&d.join(t.file()), if on { "1" } else { "0" })
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct SystemInfo {
    /// «Legion Pro 7 16IAX10H»
    pub model: String,
    /// Машинный тип, «83F5».
    pub machine_type: String,
    pub vendor: String,
    pub bios: String,
    pub bios_date: String,
    pub cpu: String,
    pub cpu_threads: usize,
    pub gpu: String,
    pub memory_gb: f32,
    pub kernel: String,
    pub os: String,
}

pub fn read_info() -> SystemInfo {
    let dmi = |f: &str| sysfs::read(&sysfs::sys(&format!("/sys/class/dmi/id/{f}"))).unwrap_or_default();
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let cpu = cpuinfo
        .lines()
        .find_map(|l| l.strip_prefix("model name").and_then(|r| r.split_once(':')).map(|(_, v)| v.trim().to_string()))
        .unwrap_or_default();
    let threads = cpuinfo.lines().filter(|l| l.starts_with("processor")).count();
    let mem_kb: f64 = std::fs::read_to_string("/proc/meminfo")
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .and_then(|v| v.split_whitespace().next()?.parse().ok())
        .unwrap_or(0.0);
    let os = std::fs::read_to_string("/etc/os-release")
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_default();
    let mut model = dmi("product_version");
    if model.is_empty() || model.chars().all(|c| c.is_ascii_digit() || c.is_ascii_uppercase()) {
        model = dmi("product_family");
    }
    SystemInfo {
        model,
        machine_type: dmi("product_name"),
        vendor: dmi("sys_vendor"),
        bios: dmi("bios_version"),
        bios_date: dmi("bios_date"),
        cpu: cpu.replace("(R)", "").replace("(TM)", ""),
        cpu_threads: threads,
        gpu: super::sensors::gpu_name().unwrap_or_default(),
        memory_gb: (mem_kb / 1024.0 / 1024.0) as f32,
        kernel: sysfs::read(std::path::Path::new("/proc/sys/kernel/osrelease")).unwrap_or_default(),
        os,
    }
}

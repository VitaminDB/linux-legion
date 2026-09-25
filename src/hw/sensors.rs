//! Мониторинг: температура, загрузка и частота CPU, память, видеокарта NVIDIA.
//!
//! Видеокарту опрашивает `nvidia-smi`, но только когда она уже проснулась:
//! опрос спящей дискретной карты будит её и сажает батарею.

use super::sysfs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Sensors {
    pub cpu_temp: Option<f32>,
    /// Загрузка 0…100.
    pub cpu_load: f32,
    pub cpu_mhz: Option<u32>,
    pub mem_used_gb: f32,
    pub mem_total_gb: f32,
    pub gpu: Gpu,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Gpu {
    #[default]
    Absent,
    /// Карта в D3cold — не опрашиваем.
    Sleeping,
    Active(GpuStats),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct GpuStats {
    pub temp: Option<f32>,
    pub load: Option<f32>,
    pub power_w: Option<f32>,
    pub clock_mhz: Option<u32>,
    pub mem_used_mb: Option<u32>,
    pub mem_total_mb: Option<u32>,
}

/// Считает загрузку CPU между вызовами по /proc/stat.
#[derive(Default)]
pub struct Sampler {
    prev: Option<(u64, u64)>,
}

impl Sampler {
    pub fn read(&mut self, with_gpu: bool) -> Sensors {
        let (used, total) = memory();
        Sensors {
            cpu_temp: cpu_temp(),
            cpu_load: self.cpu_load(),
            cpu_mhz: cpu_mhz(),
            mem_used_gb: used,
            mem_total_gb: total,
            gpu: if with_gpu { gpu() } else { Gpu::Absent },
        }
    }

    fn cpu_load(&mut self) -> f32 {
        let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
        let Some(line) = stat.lines().next() else { return 0.0 };
        let v: Vec<u64> = line.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
        if v.len() < 4 {
            return 0.0;
        }
        let idle = v[3] + v.get(4).copied().unwrap_or(0);
        let total: u64 = v.iter().take(8).sum();
        let load = match self.prev {
            Some((pi, pt)) if total > pt => {
                let dt = (total - pt) as f32;
                (1.0 - (idle.saturating_sub(pi)) as f32 / dt) * 100.0
            }
            _ => 0.0,
        };
        self.prev = Some((idle, total));
        load.clamp(0.0, 100.0)
    }
}

/// Температура пакета CPU (coretemp «Package id 0», k10temp «Tctl»).
fn cpu_temp() -> Option<f32> {
    let dir = sysfs::find_dir("/sys/class/hwmon", |p| {
        matches!(sysfs::read(&p.join("name")).as_deref(), Some("coretemp" | "k10temp" | "zenpower"))
    })?;
    let mut best: Option<f32> = None;
    for i in 1..=64 {
        let input = dir.join(format!("temp{i}_input"));
        let Some(v) = sysfs::read_num::<f32>(&input) else { continue };
        let label = sysfs::read(&dir.join(format!("temp{i}_label"))).unwrap_or_default();
        if label.starts_with("Package") || label == "Tctl" || label == "Tdie" {
            return Some(v / 1000.0);
        }
        best = Some(best.map_or(v / 1000.0, |b: f32| b.max(v / 1000.0)));
    }
    best
}

/// Средняя текущая частота ядер.
fn cpu_mhz() -> Option<u32> {
    let rd = std::fs::read_dir(sysfs::sys("/sys/devices/system/cpu")).ok()?;
    let freqs: Vec<u64> = rd
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with("cpu") && n[3..].chars().all(|c| c.is_ascii_digit()) && n.len() > 3
        })
        .filter_map(|e| sysfs::read_num::<u64>(&e.path().join("cpufreq/scaling_cur_freq")))
        .collect();
    (!freqs.is_empty()).then(|| (freqs.iter().sum::<u64>() / freqs.len() as u64 / 1000) as u32)
}

fn memory() -> (f32, f32) {
    let info = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let get = |k: &str| -> f32 {
        info.lines()
            .find_map(|l| l.strip_prefix(k))
            .and_then(|v| v.split_whitespace().next()?.parse::<f32>().ok())
            .unwrap_or(0.0)
    };
    let total = get("MemTotal:");
    let avail = get("MemAvailable:");
    ((total - avail) / 1048576.0, total / 1048576.0)
}

/// PCI-устройство дискретной видеокарты NVIDIA.
fn nvidia_pci() -> Option<PathBuf> {
    sysfs::find_dir("/sys/bus/pci/devices", |p| {
        sysfs::read(&p.join("vendor")).as_deref() == Some("0x10de")
            && sysfs::read(&p.join("class")).is_some_and(|c| c.starts_with("0x03"))
    })
}

/// Название видеокарты — из /proc драйвера, не трогая саму карту.
pub fn gpu_name() -> Option<String> {
    let dir = std::fs::read_dir("/proc/driver/nvidia/gpus").ok()?.flatten().next()?.path();
    std::fs::read_to_string(dir.join("information"))
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("Model:"))
        .map(|s| s.trim().to_string())
}

fn gpu() -> Gpu {
    let Some(pci) = nvidia_pci() else { return Gpu::Absent };
    if sysfs::read(&pci.join("power/runtime_status")).as_deref() == Some("suspended") {
        return Gpu::Sleeping;
    }
    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=temperature.gpu,utilization.gpu,power.draw,clocks.gr,memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output();
    let Ok(out) = out else { return Gpu::Active(GpuStats::default()) };
    Gpu::Active(parse_smi(&String::from_utf8_lossy(&out.stdout)))
}

fn parse_smi(s: &str) -> GpuStats {
    let f: Vec<&str> = s.lines().next().unwrap_or("").split(',').map(str::trim).collect();
    let num = |i: usize| f.get(i).and_then(|v| v.parse::<f32>().ok());
    GpuStats {
        temp: num(0),
        load: num(1),
        power_w: num(2),
        clock_mhz: num(3).map(|v| v as u32),
        mem_used_mb: num(4).map(|v| v as u32),
        mem_total_mb: num(5).map(|v| v as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvidia_smi() {
        let s = parse_smi("48, 0, 5.63, 180, 12, 24463\n");
        assert_eq!(s.temp, Some(48.0));
        assert_eq!(s.power_w, Some(5.63));
        assert_eq!(s.clock_mhz, Some(180));
        assert_eq!(s.mem_total_mb, Some(24463));
        // [N/A] и пустые поля
        let s = parse_smi("[N/A], 3, [N/A]");
        assert_eq!(s.temp, None);
        assert_eq!(s.load, Some(3.0));
    }
}

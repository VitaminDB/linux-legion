//! Мониторинг: температура, загрузка и частота CPU, память, видеокарта.
//!
//! Видеокарту NVIDIA опрашивает `nvidia-smi`, AMD — файлы драйвера amdgpu в
//! sysfs; обе только когда карта уже проснулась: опрос спящей дискретной
//! карты будит её и сажает батарею. Если NVIDIA нет (модели только со
//! встроенной Radeon, например Legion 7 15ASH11 с Radeon 8060S), берётся
//! видеокарта AMD.

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
    /// Действующий предел мощности (Вт): прошивка меняет его по режиму.
    pub power_limit_w: Option<f32>,
    /// Наибольший предел карты (Вт).
    pub power_max_w: Option<f32>,
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

/// PCI-устройство видеокарты AMD на драйвере amdgpu (встроенной или
/// дискретной). Класс 0x03xx: VGA (0x0300) или Display controller (0x0380),
/// как у встроенной графики Strix Halo.
fn amd_pci() -> Option<PathBuf> {
    sysfs::find_dir("/sys/bus/pci/devices", |p| {
        sysfs::read(&p.join("vendor")).as_deref() == Some("0x1002")
            && sysfs::read(&p.join("class")).is_some_and(|c| c.starts_with("0x03"))
            && std::fs::read_link(p.join("driver"))
                .is_ok_and(|d| d.file_name().is_some_and(|n| n == "amdgpu"))
    })
}

/// Название видеокарты — из /proc драйвера, не трогая саму карту.
pub fn gpu_name() -> Option<String> {
    nvidia_name().or_else(amd_name)
}

fn nvidia_name() -> Option<String> {
    let dir = std::fs::read_dir("/proc/driver/nvidia/gpus").ok()?.flatten().next()?.path();
    std::fs::read_to_string(dir.join("information"))
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("Model:"))
        .map(|s| s.trim().to_string())
}

/// Название видеокарты AMD. У встроенной точная модель есть только в имени
/// процессора («AMD RYZEN AI MAX+ 392 w/ Radeon 8060S»): в pci.ids одна запись
/// на всё семейство («Strix Halo [Radeon Graphics / Radeon 8050S Graphics /
/// Radeon 8060S Graphics]»). Иначе — запись pci.ids.
fn amd_name() -> Option<String> {
    let pci = amd_pci()?;
    if let Some(name) = std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|c| radeon_from_cpu(&c)) {
        return Some(name);
    }
    let device = sysfs::read(&pci.join("device"))?;
    let ids = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids"]
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())?;
    pci_ids_name(&ids, "1002", device.trim_start_matches("0x"))
}

/// «… w/ Radeon 8060S» из строки model name → «Radeon 8060S».
fn radeon_from_cpu(cpuinfo: &str) -> Option<String> {
    let model = cpuinfo.lines().find_map(|l| l.strip_prefix("model name"))?;
    let (_, gpu) = model.split_once(" w/ ")?;
    let gpu = gpu.trim();
    (!gpu.is_empty()).then(|| gpu.to_string())
}

/// Имя устройства из pci.ids; из «Семейство [Модель]» берётся модель.
fn pci_ids_name(ids: &str, vendor: &str, device: &str) -> Option<String> {
    let mut in_vendor = false;
    for line in ids.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            in_vendor = line.starts_with(vendor);
            continue;
        }
        if in_vendor && !line.starts_with("\t\t") {
            let rest = &line[1..];
            if rest.len() > 4 && rest[..4].eq_ignore_ascii_case(device) {
                let name = rest[4..].trim();
                let name = match (name.find('['), name.rfind(']')) {
                    (Some(a), Some(b)) if b > a + 1 => &name[a + 1..b],
                    _ => name,
                };
                return Some(name.to_string());
            }
        }
    }
    None
}

fn gpu() -> Gpu {
    if let Some(pci) = nvidia_pci() {
        if sysfs::read(&pci.join("power/runtime_status")).as_deref() == Some("suspended") {
            return Gpu::Sleeping;
        }
        let out = Command::new("nvidia-smi")
            .args([
                "--query-gpu=temperature.gpu,utilization.gpu,power.draw,clocks.gr,memory.used,memory.total,enforced.power.limit,power.max_limit",
                "--format=csv,noheader,nounits",
            ])
            .output();
        let Ok(out) = out else { return Gpu::Active(GpuStats::default()) };
        return Gpu::Active(parse_smi(&String::from_utf8_lossy(&out.stdout)));
    }
    let Some(pci) = amd_pci() else { return Gpu::Absent };
    if sysfs::read(&pci.join("power/runtime_status")).as_deref() == Some("suspended") {
        return Gpu::Sleeping;
    }
    Gpu::Active(amd_stats(&pci))
}

/// Показатели amdgpu из sysfs: hwmon (температура edge, мощность, частота
/// sclk, предел мощности), загрузка и видеопамять — в каталоге устройства.
/// У встроенной графики VRAM — выделенная из ОЗУ область, предела мощности нет.
fn amd_stats(pci: &std::path::Path) -> GpuStats {
    let hwmon = std::fs::read_dir(pci.join("hwmon"))
        .ok()
        .and_then(|rd| rd.flatten().map(|e| e.path()).find(|p| sysfs::read(&p.join("name")).as_deref() == Some("amdgpu")));
    let h = |f: &str| hwmon.as_ref().and_then(|d| sysfs::read_num::<f64>(&d.join(f)));
    let mb = |f: &str| sysfs::read_num::<u64>(&pci.join(f)).map(|b| (b / 1_048_576) as u32);
    GpuStats {
        temp: h("temp1_input").map(|v| (v / 1000.0) as f32),
        load: sysfs::read_num::<f32>(&pci.join("gpu_busy_percent")),
        // power1_average есть не на всех поколениях, power1_input — на новых.
        power_w: h("power1_average").or_else(|| h("power1_input")).map(|v| (v / 1_000_000.0) as f32),
        clock_mhz: h("freq1_input").map(|v| (v / 1_000_000.0) as u32),
        mem_used_mb: mb("mem_info_vram_used"),
        mem_total_mb: mb("mem_info_vram_total"),
        power_limit_w: h("power1_cap").map(|v| (v / 1_000_000.0) as f32),
        power_max_w: h("power1_cap_max").map(|v| (v / 1_000_000.0) as f32),
    }
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
        power_limit_w: num(6),
        power_max_w: num(7),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_amdgpu_sysfs() {
        let dir = std::env::temp_dir().join(format!("linux-legion-amdgpu-{}", std::process::id()));
        let hw = dir.join("hwmon/hwmon5");
        std::fs::create_dir_all(&hw).unwrap();
        for (f, v) in [("name", "amdgpu"), ("temp1_input", "45000"), ("power1_input", "16931000"), ("freq1_input", "898000000")] {
            std::fs::write(hw.join(f), v).unwrap();
        }
        for (f, v) in [("gpu_busy_percent", "7"), ("mem_info_vram_used", "3359010816"), ("mem_info_vram_total", "8589934592")] {
            std::fs::write(dir.join(f), v).unwrap();
        }
        let s = amd_stats(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(s.temp, Some(45.0));
        assert_eq!(s.load, Some(7.0));
        assert_eq!(s.power_w, Some(16.931));
        assert_eq!(s.clock_mhz, Some(898));
        assert_eq!(s.mem_used_mb, Some(3203));
        assert_eq!(s.mem_total_mb, Some(8192));
        assert_eq!(s.power_limit_w, None);
    }

    #[test]
    fn names_amd_gpu() {
        let cpu = "processor\t: 0\nmodel name\t: AMD RYZEN AI MAX+ 392 w/ Radeon 8060S\n";
        assert_eq!(radeon_from_cpu(cpu).as_deref(), Some("Radeon 8060S"));
        assert_eq!(radeon_from_cpu("model name\t: AMD Ryzen 7 5800X 8-Core Processor\n"), None);
        let ids = "1002  Advanced Micro Devices, Inc. [AMD/ATI]\n\t1586  Strix Halo [Radeon Graphics / Radeon 8050S Graphics / Radeon 8060S Graphics]\n\t\t17aa 3d3c  Something\n\t744c  Navi 31 [Radeon RX 7900 XT/7900 XTX]\n10de  NVIDIA Corporation\n\t1586  Not this one\n";
        assert_eq!(pci_ids_name(ids, "1002", "744c").as_deref(), Some("Radeon RX 7900 XT/7900 XTX"));
        assert_eq!(pci_ids_name(ids, "1002", "1586").as_deref(), Some("Radeon Graphics / Radeon 8050S Graphics / Radeon 8060S Graphics"));
        assert_eq!(pci_ids_name(ids, "1002", "ffff"), None);
    }

    #[test]
    fn parses_nvidia_smi() {
        let s = parse_smi("48, 0, 5.63, 180, 12, 24463, 95.00, 175.00\n");
        assert_eq!(s.power_limit_w, Some(95.0));
        assert_eq!(s.power_max_w, Some(175.0));
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

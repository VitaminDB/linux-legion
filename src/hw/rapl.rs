//! RAPL — лимиты мощности пакета CPU (PL1/PL2) через powercap.
//!
//! У Intel два интерфейса: `intel-rapl` (MSR) и `intel-rapl-mmio`;
//! процессор держит меньший из двух. На Legion Pro 7 Gen 10 BIOS при
//! загрузке ставит в MMIO 30/30 Вт и больше их не трогает — значения
//! режимов Fn+Q в Windows выставляет Legion Space/DPTF, в Linux этого никто
//! не делает, и процессор во всех режимах упирается в 30 Вт. Служба
//! `linux-legion --daemon` берёт это на себя (см. `autoapply`).

use super::sysfs::{self, Result};
use std::path::PathBuf;

/// Лимиты в ваттах.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Limits {
    pub pl1: u32,
    pub pl2: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Rapl {
    pub msr: Option<Limits>,
    pub mmio: Option<Limits>,
    /// Хотя бы один интерфейс доступен на запись.
    pub writable: bool,
}

impl Rapl {
    pub fn present(&self) -> bool {
        self.msr.is_some() || self.mmio.is_some()
    }
}

/// Зоны пакета: (путь, это MMIO).
fn zones() -> Vec<(PathBuf, bool)> {
    let Ok(rd) = std::fs::read_dir(sysfs::sys("/sys/class/powercap")) else { return Vec::new() };
    let mut out: Vec<(PathBuf, bool)> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            (n == "intel-rapl:0" || n == "intel-rapl-mmio:0")
                && sysfs::read(&p.join("name")).is_some_and(|s| s.starts_with("package"))
        })
        .map(|p| {
            let mmio = p.file_name().is_some_and(|n| n.to_string_lossy().contains("mmio"));
            (p, mmio)
        })
        .collect();
    out.sort();
    out
}

fn pl_path(zone: &PathBuf, idx: u8) -> PathBuf {
    zone.join(format!("constraint_{idx}_power_limit_uw"))
}

fn read_zone(zone: &PathBuf) -> Option<Limits> {
    let pl1 = sysfs::read_num::<u64>(&pl_path(zone, 0))?;
    let pl2 = sysfs::read_num::<u64>(&pl_path(zone, 1)).unwrap_or(pl1);
    Some(Limits { pl1: (pl1 / 1_000_000) as u32, pl2: (pl2 / 1_000_000) as u32 })
}

pub fn read() -> Rapl {
    let mut r = Rapl::default();
    for (zone, mmio) in zones() {
        let lim = read_zone(&zone);
        if mmio {
            r.mmio = lim;
        } else {
            r.msr = lim;
        }
        r.writable |= sysfs::writable(&pl_path(&zone, 0));
    }
    r
}

/// Записать PL1/PL2 в зону. Поднимаем — сначала PL2, опускаем — сначала
/// PL1: PL1 не должен оказаться выше PL2 даже на миг.
fn write_zone(zone: &PathBuf, cur: Limits, want: Limits) -> Result<()> {
    let w = |idx: u8, v: u32| sysfs::write(&pl_path(zone, idx), &(v as u64 * 1_000_000).to_string());
    if want.pl2 >= cur.pl2 {
        if want.pl2 != cur.pl2 {
            w(1, want.pl2)?;
        }
        if want.pl1 != cur.pl1 {
            w(0, want.pl1)?;
        }
    } else {
        if want.pl1 != cur.pl1 {
            w(0, want.pl1)?;
        }
        w(1, want.pl2)?;
    }
    Ok(())
}

/// Выставить PL1/PL2 во все зоны пакета. Пишет только то, что отличается;
/// возвращает описание сделанного.
///
/// MSR-зона на Arrow Lake HX только читается (BIOS сам держит там 165/210,
/// запись даёт ENODATA даже root) — её ошибки не считаются, если есть MMIO.
pub fn set(want: Limits) -> Result<Vec<String>> {
    let zones = zones();
    let has_mmio = zones.iter().any(|(_, mmio)| *mmio);
    let mut done = Vec::new();
    for (zone, mmio) in &zones {
        let cur = read_zone(zone).unwrap_or_default();
        if want == cur {
            continue;
        }
        match write_zone(zone, cur, want) {
            Ok(()) => done.push(format!("{} {}/{} Вт", if *mmio { "mmio" } else { "msr" }, want.pl1, want.pl2)),
            Err(_) if !*mmio && has_mmio => {}
            Err(e) => return Err(e),
        }
    }
    Ok(done)
}

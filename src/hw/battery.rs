//! Батарея и адаптер питания (`/sys/class/power_supply`).

use super::sysfs::{self, Result};
use std::path::PathBuf;

/// Режим заряда (`charge_types`). В Vantage: «Быстрая зарядка» и
/// «Режим сбережения» (заряд до ~80 %).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChargeType {
    Fast,
    Standard,
    LongLife,
}

impl ChargeType {
    pub const ALL: [ChargeType; 3] = [ChargeType::Standard, ChargeType::Fast, ChargeType::LongLife];

    fn sysfs(self) -> &'static str {
        match self {
            ChargeType::Fast => "Fast",
            ChargeType::Standard => "Standard",
            ChargeType::LongLife => "Long_Life",
        }
    }

    fn from_sysfs(s: &str) -> Option<Self> {
        Some(match s {
            "Fast" => ChargeType::Fast,
            "Standard" => ChargeType::Standard,
            "Long_Life" => ChargeType::LongLife,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            ChargeType::Fast => "Быстрая зарядка",
            ChargeType::Standard => "Обычная",
            ChargeType::LongLife => "Сбережение",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            ChargeType::Fast => "Заряжает быстрее, но сильнее нагревает аккумулятор.",
            ChargeType::Standard => "Штатная зарядка до 100 %.",
            ChargeType::LongLife => "Заряд держится около 80 % — продлевает жизнь аккумулятора, если ноутбук всегда в сети.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Charging => "Заряжается",
            Status::Discharging => "Разряжается",
            Status::Full => "Заряжена",
            Status::NotCharging => "Не заряжается",
            Status::Unknown => "—",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Battery {
    pub percent: u8,
    pub status: Status,
    pub ac_online: bool,
    /// Мощность заряда/разряда, Вт.
    pub power_w: f32,
    pub energy_now_wh: f32,
    pub energy_full_wh: f32,
    pub energy_design_wh: f32,
    pub voltage_v: f32,
    pub cycles: Option<u32>,
    pub model: String,
    pub manufacturer: String,
    pub charge_type: Option<ChargeType>,
    pub charge_types: Vec<ChargeType>,
    pub charge_type_writable: bool,
}

impl Battery {
    /// Износ: сколько процентов исходной ёмкости осталось.
    pub fn health(&self) -> Option<f32> {
        (self.energy_design_wh > 0.0).then(|| (self.energy_full_wh / self.energy_design_wh * 100.0).min(100.0))
    }

    /// Оценка времени до разряда/заряда в минутах.
    pub fn minutes_left(&self) -> Option<u32> {
        if self.power_w < 0.5 {
            return None;
        }
        let wh = match self.status {
            Status::Discharging => self.energy_now_wh,
            Status::Charging => (self.energy_full_wh - self.energy_now_wh).max(0.0),
            _ => return None,
        };
        Some((wh / self.power_w * 60.0) as u32)
    }
}

fn battery_dir() -> Option<PathBuf> {
    sysfs::find_dir("/sys/class/power_supply", |p| sysfs::read(&p.join("type")).as_deref() == Some("Battery"))
}

fn ac_online() -> bool {
    sysfs::find_dir("/sys/class/power_supply", |p| sysfs::read(&p.join("type")).as_deref() == Some("Mains"))
        .and_then(|p| sysfs::read_bool(&p.join("online")))
        .unwrap_or(false)
}

/// Разбор `[Fast] Standard Long_Life`: (текущий, доступные).
fn parse_charge_types(s: &str) -> (Option<ChargeType>, Vec<ChargeType>) {
    let mut cur = None;
    let mut all = Vec::new();
    for w in s.split_whitespace() {
        let (sel, name) = match w.strip_prefix('[').and_then(|w| w.strip_suffix(']')) {
            Some(n) => (true, n),
            None => (false, w),
        };
        if let Some(t) = ChargeType::from_sysfs(name) {
            if sel {
                cur = Some(t);
            }
            all.push(t);
        }
    }
    (cur, all)
}

pub fn read_battery() -> Option<Battery> {
    let d = battery_dir()?;
    let num = |f: &str| sysfs::read_num::<f64>(&d.join(f));
    // µWh/µW; у части батарей вместо energy — charge (µAh) и current (µA).
    let volt = num("voltage_now").unwrap_or(0.0) / 1e6;
    let wh = |e: &str, c: &str| num(e).map(|v| v / 1e6).or_else(|| num(c).map(|v| v / 1e6 * volt)).unwrap_or(0.0);
    let power = num("power_now").map(|v| v / 1e6).or_else(|| num("current_now").map(|v| v / 1e6 * volt));
    let status = match sysfs::read(&d.join("status")).as_deref() {
        Some("Charging") => Status::Charging,
        Some("Discharging") => Status::Discharging,
        Some("Full") => Status::Full,
        Some("Not charging") => Status::NotCharging,
        _ => Status::Unknown,
    };
    let (charge_type, charge_types) = parse_charge_types(&sysfs::read(&d.join("charge_types")).unwrap_or_default());
    Some(Battery {
        percent: num("capacity").unwrap_or(0.0).clamp(0.0, 100.0) as u8,
        status,
        ac_online: ac_online(),
        power_w: power.unwrap_or(0.0).abs() as f32,
        energy_now_wh: wh("energy_now", "charge_now") as f32,
        energy_full_wh: wh("energy_full", "charge_full") as f32,
        energy_design_wh: wh("energy_full_design", "charge_full_design") as f32,
        voltage_v: volt as f32,
        cycles: sysfs::read_num(&d.join("cycle_count")),
        model: sysfs::read(&d.join("model_name")).unwrap_or_default(),
        manufacturer: sysfs::read(&d.join("manufacturer")).unwrap_or_default(),
        charge_type,
        charge_types,
        charge_type_writable: sysfs::writable(&d.join("charge_types")),
    })
}

pub fn set_charge_type(t: ChargeType) -> Result<()> {
    let d = battery_dir().ok_or_else(|| sysfs::Error::NotFound("battery".into()))?;
    sysfs::write(&d.join("charge_types"), t.sysfs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_charge_types() {
        let (cur, all) = parse_charge_types("[Fast] Standard Long_Life");
        assert_eq!(cur, Some(ChargeType::Fast));
        assert_eq!(all, vec![ChargeType::Fast, ChargeType::Standard, ChargeType::LongLife]);
        assert_eq!(parse_charge_types("Standard [Long_Life] Custom").0, Some(ChargeType::LongLife));
    }
}

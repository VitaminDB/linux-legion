//! Служба `linux-legion --daemon` и её настройки (`~/.config/linux-legion/custom.conf`).
//!
//! Что делает служба:
//! * **режим «Свой»** — восстанавливает сохранённые лимиты и обороты при
//!   входе в систему, при переключении в «Свой» (Fn+Q) и после сна: прошивка
//!   держит их только до перезагрузки, а обороты — и до сна;
//! * **лимиты CPU по режиму (RAPL)** — BIOS Legion Pro 7 Gen 10 при загрузке
//!   ставит в RAPL MMIO 30/30 Вт и дальше их не меняет; в Windows значения
//!   режимов пишет Legion Space, в Linux — эта служба: Тихий 55/65,
//!   Баланс 90/125, Производительность 145/190, Экстрим 160/205, «Свой» —
//!   из лимитов прошивки;
//! * **EPP по режиму** — `performance` в Производительность/Экстрим/Свой,
//!   `balance_performance` в Балансе, `balance_power` в Тихом;
//! * **программное авто вентиляторов** — EC этой модели после ручного
//!   задания оборотов больше не возвращается к своей кривой (ни по «0», ни
//!   по смене режима, ни после сна или тёплой перезагрузки — только сброс
//!   EC). Служба ведёт вентиляторы по температуре сама.
//!
//! ```text
//! tunable.ppt_pl1_spl=95
//! fan.1=3000          # 0 — авто
//! fan.curve=1         # программное авто
//! cpu.rapl=1          # лимиты CPU по режиму
//! cpu.epp=0           # EPP по режиму
//! rapl.performance=150/190   # свои PL1/PL2 для режима
//! profile.1.name=Экстрим 90°  # свой профиль на основе режима прошивки
//! profile.1.base=extreme
//! profile.1.rapl=160/205
//! profile.1.cpu_temp=90       # предел температуры CPU (TCC), в любом режиме
//! profile.1.tunable.ppt_pl1_spl=150   # лимиты прошивки и обороты — при base=custom
//! profile.1.fan.1=3000
//! profile.active=1            # действует, пока включён его режим
//! ```
//!
//! **Свои профили** — именованные наборы поверх режима прошивки: режим
//! (Тихий…Экстрим, «Свой»), PL1/PL2, предел температуры CPU, а при основе
//! «Свой» — лимиты прошивки и обороты. Активный профиль действует, пока
//! включён его режим (в том числе по Fn+Q); выбор встроенного режима в
//! интерфейсе снимает профиль.

use crate::hw::cpufreq;
use crate::hw::fans::{self, Fan};
use crate::hw::power::{self, PowerMode};
use crate::hw::rapl::{self, Limits};
use crate::hw::sensors::{Gpu, Sampler};
use crate::hw::tcc;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub const UNIT: &str = "linux-legion-autoapply.service";

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Переадресовать каталог настроек (демо-режим не трогает настоящие).
pub fn set_dir(dir: PathBuf) {
    let _ = DIR.set(dir);
}

pub fn dir() -> PathBuf {
    DIR.get().cloned().unwrap_or_else(|| {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("linux-legion")
    })
}

fn path() -> PathBuf {
    dir().join("custom.conf")
}

/// Переключатели службы.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    /// Вентиляторы по температуре (программное авто).
    FanCurve,
    /// Лимиты CPU (RAPL) по режиму.
    Rapl,
    /// EPP по режиму.
    Epp,
}

impl Flag {
    fn key(self) -> &'static str {
        match self {
            Flag::FanCurve => "fan.curve",
            Flag::Rapl => "cpu.rapl",
            Flag::Epp => "cpu.epp",
        }
    }
}

/// Свой профиль: режим прошивки и что поверх него.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub id: u32,
    pub name: String,
    /// Режим прошивки, на котором основан профиль.
    pub base: PowerMode,
    /// PL1/PL2 (RAPL); `None` — заводские для режима.
    pub rapl: Option<Limits>,
    /// Предел температуры CPU (TCC), °C; `None` — как задаёт прошивка.
    pub cpu_temp: Option<u32>,
    /// Лимиты прошивки (действуют только при основе «Свой»).
    pub tunables: BTreeMap<String, i64>,
    /// Обороты (только «Свой»): номер → об/мин, 0 — авто.
    pub fans: BTreeMap<u8, u32>,
}

impl Profile {
    pub fn new(id: u32, name: impl Into<String>, base: PowerMode) -> Self {
        Profile {
            id,
            name: name.into(),
            base,
            rapl: None,
            cpu_temp: None,
            tunables: BTreeMap::new(),
            fans: BTreeMap::new(),
        }
    }
}

/// Что действует в режиме: из активного профиля или встроенное.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Effective {
    pub rapl: Option<Limits>,
    pub cpu_temp: Option<u32>,
    pub tunables: BTreeMap<String, i64>,
    pub fans: BTreeMap<u8, u32>,
}

/// Сохранённые настройки.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub tunables: BTreeMap<String, i64>,
    /// Номер вентилятора → об/мин (0 — авто).
    pub fans: BTreeMap<u8, u32>,
    pub fan_curve: bool,
    pub rapl: bool,
    pub epp: bool,
    /// Свои PL1/PL2 для режимов (поверх `PowerMode::cpu_limits`).
    pub rapl_modes: BTreeMap<String, Limits>,
    /// Свои профили по порядку создания.
    pub profiles: Vec<Profile>,
    /// Активный свой профиль.
    pub active: Option<u32>,
}

impl Default for Saved {
    fn default() -> Self {
        Saved {
            tunables: BTreeMap::new(),
            fans: BTreeMap::new(),
            fan_curve: false,
            rapl: true,
            epp: false,
            rapl_modes: BTreeMap::new(),
            profiles: Vec::new(),
            active: None,
        }
    }
}

fn parse_limits(v: &str) -> Option<Limits> {
    let (a, b) = v.split_once('/')?;
    Some(Limits { pl1: a.trim().parse().ok()?, pl2: b.trim().parse().ok()? })
}

impl Saved {
    fn parse(text: &str) -> Self {
        let mut s = Saved::default();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let Some((k, v)) = line.split_once('=') else { continue };
            if let Some(rest) = k.trim().strip_prefix("profile.") {
                s.parse_profile(rest, v);
                continue;
            }
            let (k, v) = (k.trim(), v.split('#').next().unwrap_or("").trim());
            let flag = |s: &mut bool| {
                if let Ok(n) = v.parse::<u8>() {
                    *s = n != 0;
                }
            };
            if let Some(name) = k.strip_prefix("tunable.") {
                if let Ok(v) = v.parse() {
                    s.tunables.insert(name.to_string(), v);
                }
            } else if k == Flag::FanCurve.key() {
                flag(&mut s.fan_curve);
            } else if k == Flag::Rapl.key() {
                flag(&mut s.rapl);
            } else if k == Flag::Epp.key() {
                flag(&mut s.epp);
            } else if let Some(i) = k.strip_prefix("fan.") {
                if let (Ok(i), Ok(v)) = (i.parse(), v.parse()) {
                    s.fans.insert(i, v);
                }
            } else if let Some(mode) = k.strip_prefix("rapl.") {
                if let (Some(l), Some(_)) = (parse_limits(v), PowerMode::from_key(mode)) {
                    s.rapl_modes.insert(mode.to_string(), l);
                }
            }
        }
        // Профиль без режима не годится; активный — только существующий.
        s.profiles.retain(|p| p.id != 0);
        if s.active.is_some_and(|id| s.profile(id).is_none()) {
            s.active = None;
        }
        s
    }

    /// Строка `profile.<rest>=<v>`. Имя берётся как есть (в нём может быть «#»).
    fn parse_profile(&mut self, rest: &str, raw: &str) {
        let clean = raw.split('#').next().unwrap_or("").trim();
        if rest == "active" {
            self.active = clean.parse().ok().filter(|&id| id != 0);
            return;
        }
        let Some((id, field)) = rest.split_once('.') else { return };
        let Ok(id) = id.parse::<u32>() else { return };
        if id == 0 {
            return;
        }
        let i = match self.profiles.iter().position(|p| p.id == id) {
            Some(i) => i,
            None => {
                self.profiles.push(Profile::new(id, format!("#{id}"), PowerMode::Custom));
                self.profiles.len() - 1
            }
        };
        let p = &mut self.profiles[i];
        match field {
            "name" => p.name = raw.trim().to_string(),
            "base" => {
                if let Some(m) = PowerMode::from_key(clean) {
                    p.base = m;
                }
            }
            "rapl" => p.rapl = parse_limits(clean),
            "cpu_temp" => p.cpu_temp = clean.parse().ok(),
            _ => {
                if let Some(name) = field.strip_prefix("tunable.") {
                    if let Ok(v) = clean.parse() {
                        p.tunables.insert(name.to_string(), v);
                    }
                } else if let Some(n) = field.strip_prefix("fan.") {
                    if let (Ok(n), Ok(v)) = (n.parse(), clean.parse()) {
                        p.fans.insert(n, v);
                    }
                }
            }
        }
    }

    pub fn profile(&self, id: u32) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// Активный профиль, если сейчас включён его режим.
    pub fn active_for(&self, mode: PowerMode) -> Option<&Profile> {
        self.active.and_then(|id| self.profile(id)).filter(|p| p.base == mode)
    }

    /// Что действует в режиме.
    pub fn effective(&self, mode: PowerMode) -> Effective {
        match self.active_for(mode) {
            Some(p) => Effective {
                rapl: p.rapl.or_else(|| self.builtin_limits(mode)),
                cpu_temp: p.cpu_temp,
                tunables: p.tunables.clone(),
                fans: p.fans.clone(),
            },
            None => Effective {
                rapl: self.builtin_limits(mode),
                cpu_temp: None,
                tunables: self.tunables.clone(),
                fans: self.fans.clone(),
            },
        }
    }

    pub fn next_id(&self) -> u32 {
        self.profiles.iter().map(|p| p.id).max().unwrap_or(0) + 1
    }

    fn render(&self) -> String {
        let mut out = String::from("# linux-legion: настройки службы и режима «Свой»\n");
        for (k, v) in &self.tunables {
            out.push_str(&format!("tunable.{k}={v}\n"));
        }
        for (k, v) in &self.fans {
            out.push_str(&format!("fan.{k}={v}\n"));
        }
        out.push_str(&format!("{}={}\n", Flag::FanCurve.key(), self.fan_curve as u8));
        out.push_str(&format!("{}={}\n", Flag::Rapl.key(), self.rapl as u8));
        out.push_str(&format!("{}={}\n", Flag::Epp.key(), self.epp as u8));
        for (m, l) in &self.rapl_modes {
            out.push_str(&format!("rapl.{m}={}/{}\n", l.pl1, l.pl2));
        }
        for p in &self.profiles {
            let id = p.id;
            out.push_str(&format!("profile.{id}.name={}\n", p.name.replace('\n', " ")));
            out.push_str(&format!("profile.{id}.base={}\n", p.base.key()));
            if let Some(l) = p.rapl {
                out.push_str(&format!("profile.{id}.rapl={}/{}\n", l.pl1, l.pl2));
            }
            if let Some(t) = p.cpu_temp {
                out.push_str(&format!("profile.{id}.cpu_temp={t}\n"));
            }
            for (k, v) in &p.tunables {
                out.push_str(&format!("profile.{id}.tunable.{k}={v}\n"));
            }
            for (k, v) in &p.fans {
                out.push_str(&format!("profile.{id}.fan.{k}={v}\n"));
            }
        }
        if let Some(id) = self.active {
            out.push_str(&format!("profile.active={id}\n"));
        }
        out
    }

    /// Лимиты CPU для режима: свои из конфига, иначе заводские Lenovo;
    /// для «Свой» — текущие атрибуты прошивки.
    pub fn limits_for(&self, mode: PowerMode) -> Option<Limits> {
        self.builtin_limits(mode)
    }

    fn builtin_limits(&self, mode: PowerMode) -> Option<Limits> {
        if let Some(l) = self.rapl_modes.get(mode.key()) {
            return Some(*l);
        }
        match mode.cpu_limits() {
            Some((pl1, pl2)) => Some(Limits { pl1, pl2 }),
            None => {
                let t = power::read_tunables();
                let get = |n: &str| t.iter().find(|t| t.name == n).map(|t| t.value as u32);
                let pl1 = get("ppt_pl1_spl")?;
                Some(Limits { pl1, pl2: get("ppt_pl2_sppt").unwrap_or(pl1).max(pl1) })
            }
        }
    }
}

pub fn load() -> Saved {
    std::fs::read_to_string(path()).map(|t| Saved::parse(&t)).unwrap_or_default()
}

fn store(s: &Saved) -> std::io::Result<()> {
    std::fs::create_dir_all(dir())?;
    let tmp = path().with_extension("tmp");
    std::fs::write(&tmp, s.render())?;
    std::fs::rename(tmp, path())
}

/// Активный профиль на основе «Свой», если он сейчас действует.
fn active_custom(s: &mut Saved) -> Option<&mut Profile> {
    let id = s.active_for(PowerMode::Custom)?.id;
    s.profiles.iter_mut().find(|p| p.id == id)
}

/// Запомнить значение, которое пользователь применил в режиме «Свой»
/// (в активный профиль, если он действует, иначе — во встроенный «Свой»).
pub fn remember_tunable(name: &str, value: i64) {
    let mut s = load();
    match active_custom(&mut s) {
        Some(p) => p.tunables.insert(name.to_string(), value),
        None => s.tunables.insert(name.to_string(), value),
    };
    let _ = store(&s);
}

pub fn remember_fan(index: u8, rpm: u32) {
    let mut s = load();
    match active_custom(&mut s) {
        Some(p) => p.fans.insert(index, rpm),
        None => s.fans.insert(index, rpm),
    };
    let _ = store(&s);
}

/// Сохранить профиль (новый или изменённый).
pub fn save_profile(p: Profile) -> std::io::Result<()> {
    let mut s = load();
    match s.profiles.iter_mut().find(|x| x.id == p.id) {
        Some(x) => *x = p,
        None => s.profiles.push(p),
    }
    store(&s)
}

pub fn delete_profile(id: u32) -> std::io::Result<()> {
    let mut s = load();
    s.profiles.retain(|p| p.id != id);
    if s.active == Some(id) {
        s.active = None;
    }
    store(&s)
}

/// Сделать профиль активным (`None` — встроенные режимы).
pub fn set_active(id: Option<u32>) -> std::io::Result<()> {
    let mut s = load();
    s.active = id.filter(|id| s.profile(*id).is_some());
    store(&s)
}

pub fn remember_flag(flag: Flag, on: bool) {
    let mut s = load();
    match flag {
        Flag::FanCurve => s.fan_curve = on,
        Flag::Rapl => s.rapl = on,
        Flag::Epp => s.epp = on,
    }
    let _ = store(&s);
}

/// Применить сохранённое для режима «Свой» (лимиты прошивки и обороты).
/// Обороты пишутся всегда: `fanN_target` драйвера — кэш, после перезагрузки
/// он показывает 0, хотя EC может помнить старое значение.
pub fn apply_custom() -> Result<Vec<String>, String> {
    if power::read_state().mode != Some(PowerMode::Custom) {
        return Ok(Vec::new());
    }
    let saved = load();
    let eff = saved.effective(PowerMode::Custom);
    let mut done = Vec::new();
    let mut errors = Vec::new();
    let current: BTreeMap<String, i64> = power::read_tunables().into_iter().map(|t| (t.name, t.value)).collect();
    for (name, v) in &eff.tunables {
        if current.get(name) == Some(v) || !current.contains_key(name) {
            continue;
        }
        match power::set_tunable(name, *v) {
            Ok(()) => done.push(format!("{name}={v}")),
            Err(e) => errors.push(e.to_string()),
        }
    }
    if !saved.fan_curve {
        let present: Vec<u8> = fans::read_fans().into_iter().map(|f| f.index).collect();
        for (i, rpm) in eff.fans.iter().filter(|(i, _)| present.contains(i)) {
            match fans::set_target(*i, *rpm) {
                Ok(()) => done.push(format!("fan{i}={rpm}")),
                Err(e) => errors.push(e.to_string()),
            }
        }
    }
    if errors.is_empty() {
        Ok(done)
    } else {
        Err(errors.join("; "))
    }
}

/// Лимиты CPU и EPP под режим (если включены). Пишет только отличия.
pub fn apply_cpu(mode: PowerMode) -> Result<Vec<String>, String> {
    let saved = load();
    let mut done = Vec::new();
    let mut errors = Vec::new();
    let eff = saved.effective(mode);
    if saved.rapl && rapl::read().present() {
        match eff.rapl {
            Some(l) => match rapl::set(l) {
                Ok(d) => done.extend(d),
                Err(e) => errors.push(e.to_string()),
            },
            None => errors.push("лимиты «Свой»: атрибуты прошивки не найдены".into()),
        }
    }
    match apply_tcc(eff.cpu_temp) {
        Ok(Some(d)) => done.push(d),
        Ok(None) => {}
        Err(e) => errors.push(e),
    }
    if saved.epp && cpufreq::read().current.is_some() {
        match cpufreq::set(mode.epp()) {
            Ok(true) => done.push(format!("epp={}", mode.epp())),
            Ok(false) => {}
            Err(e) => errors.push(e.to_string()),
        }
    }
    if errors.is_empty() {
        Ok(done)
    } else {
        Err(errors.join("; "))
    }
}

/// Процесс выставлял свой предел TCC — без профиля вернуть заводской.
static TCC_TOUCHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Заводской предел температуры CPU: `default_value` атрибута прошивки
/// `cpu_temp` (на Legion Pro 7 Gen 10 — 103 °C, TCC offset 2), иначе TjMax.
fn tcc_default() -> Option<u32> {
    power::read_tunables().into_iter().find(|t| t.name == "cpu_temp").and_then(|t| t.default).map(|v| v as u32)
}

/// Предел температуры CPU профиля; без него — вернуть заводской, если трогали.
fn apply_tcc(want: Option<u32>) -> Result<Option<String>, String> {
    use std::sync::atomic::Ordering;
    let Some(cur) = tcc::read() else {
        return match want {
            Some(_) => Err("предел температуры CPU: нет intel_tcc_cooling (TCC Offset)".into()),
            None => Ok(None),
        };
    };
    let target = match want {
        Some(t) => t,
        None if TCC_TOUCHED.load(Ordering::Relaxed) => tcc_default().unwrap_or(cur.tjmax),
        None => return Ok(None),
    };
    match tcc::set_limit(target) {
        Ok(changed) => {
            TCC_TOUCHED.store(want.is_some(), Ordering::Relaxed);
            Ok(changed.then(|| format!("tcc {target}°C")))
        }
        Err(e) => Err(e.to_string()),
    }
}

/// `--apply`: всё разом.
pub fn apply() -> Result<Vec<String>, String> {
    let mut done = apply_custom()?;
    if let Some(mode) = power::read_state().mode {
        done.extend(apply_cpu(mode)?);
    }
    Ok(done)
}

/// Вернуть вентиляторы EC («0 = авто» по документации драйвера). Нужно при
/// выходе из «Свой» без программного авто: EC Legion хранит ручные обороты
/// и в других режимах.
pub fn release_fans() -> Vec<String> {
    fans::read_fans()
        .iter()
        .filter(|f| f.writable)
        .filter_map(|f| fans::set_target(f.index, 0).ok().map(|_| format!("fan{}=авто", f.index)))
        .collect()
}

// ---------------------------------------------------------------------------
// Программное авто вентиляторов

/// Кривая: температура → доля диапазона [min, max].
const CURVE: [(f32, f32); 7] = [(45.0, 0.0), (55.0, 0.08), (65.0, 0.2), (75.0, 0.4), (85.0, 0.65), (92.0, 0.85), (97.0, 1.0)];

/// Выше этой температуры фиксированные обороты «Своего» не ниже кривой.
const SAFETY_TEMP: f32 = 95.0;

fn curve(temp: f32) -> f32 {
    let (mut lo, mut hi) = (CURVE[0], CURVE[CURVE.len() - 1]);
    if temp <= lo.0 {
        return lo.1;
    }
    if temp >= hi.0 {
        return hi.1;
    }
    for w in CURVE.windows(2) {
        if temp >= w[0].0 && temp <= w[1].0 {
            (lo, hi) = (w[0], w[1]);
            break;
        }
    }
    lo.1 + (hi.1 - lo.1) * (temp - lo.0) / (hi.0 - lo.0)
}

fn mode_factor(mode: Option<PowerMode>) -> f32 {
    match mode {
        Some(PowerMode::Quiet) => 0.7,
        Some(PowerMode::Performance) => 1.2,
        Some(PowerMode::Extreme) => 1.35,
        _ => 1.0,
    }
}

/// Обороты для вентилятора по температуре и режиму.
pub fn curve_rpm(fan: &Fan, temp: f32, mode: Option<PowerMode>) -> u32 {
    let frac = (curve(temp) * mode_factor(mode)).clamp(0.0, 1.0);
    let span = fan.max.saturating_sub(fan.min) as f32;
    let rpm = fan.min as f32 + span * frac;
    let step = fan.step.max(1);
    ((rpm / step as f32).round() as u32 * step).clamp(fan.min, fan.max)
}

/// Состояние программного авто.
#[derive(Default)]
struct FanCurve {
    sampler: Sampler,
    /// Последнее записанное: вентилятор → (об/мин, сколько тиков подряд хотелось ниже).
    last: BTreeMap<u8, (u32, u8)>,
    gpu_temp: Option<f32>,
}

impl FanCurve {
    /// Один шаг. Вверх — сразу, вниз — после трёх шагов подряд, чтобы
    /// обороты не дёргались.
    fn tick(&mut self, mode: Option<PowerMode>, saved: &Saved) -> Vec<String> {
        let s = self.sampler.read(true);
        let cpu = s.cpu_temp;
        match s.gpu {
            Gpu::Active(g) => self.gpu_temp = g.temp,
            Gpu::Sleeping | Gpu::Absent => self.gpu_temp = None,
        }
        let gpu = self.gpu_temp;
        let Some(cpu_t) = cpu.or(gpu) else { return Vec::new() };
        let gpu_t = gpu.unwrap_or(cpu_t);
        let mut done = Vec::new();
        let fixed_rpm = (mode == Some(PowerMode::Custom)).then(|| saved.effective(PowerMode::Custom).fans);
        for f in fans::read_fans().iter().filter(|f| f.writable && f.max > 0) {
            let temp = match f.index {
                1 => cpu_t,
                2 => gpu_t,
                _ => cpu_t.max(gpu_t),
            };
            let auto = curve_rpm(f, temp, mode);
            let fixed = fixed_rpm.as_ref().and_then(|m| m.get(&f.index).copied());
            let want = match fixed {
                Some(rpm) if rpm > 0 => {
                    if temp >= SAFETY_TEMP {
                        rpm.max(auto)
                    } else {
                        rpm
                    }
                }
                _ => auto,
            };
            let (last, falling) = self.last.get(&f.index).copied().unwrap_or((u32::MAX, 0));
            let write = if last == u32::MAX || want > last {
                true
            } else if want < last {
                if falling + 1 >= 3 {
                    true
                } else {
                    self.last.insert(f.index, (last, falling + 1));
                    false
                }
            } else {
                self.last.insert(f.index, (last, 0));
                false
            };
            if write && fans::set_target(f.index, want).is_ok() {
                self.last.insert(f.index, (want, 0));
                done.push(format!("fan{}={want} ({temp:.0}°)", f.index));
            }
        }
        done
    }
}

/// Сдвиг CLOCK_BOOTTIME относительно CLOCK_MONOTONIC растёт только во сне.
fn sleep_offset() -> Duration {
    let read = |id| {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        unsafe { libc::clock_gettime(id, &mut ts) };
        Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
    };
    read(libc::CLOCK_BOOTTIME).saturating_sub(read(libc::CLOCK_MONOTONIC))
}

const CPU_PERIOD: Duration = Duration::from_secs(5);
const FAN_PERIOD: Duration = Duration::from_secs(2);

/// `linux-legion --daemon`: следит за режимом и сном, применяет сохранённое,
/// держит лимиты CPU и EPP по режиму, ведёт вентиляторы по температуре.
pub fn daemon() -> ! {
    let log = |m: String| eprintln!("linux-legion: {m}");
    let report = |why: &str, r: Result<Vec<String>, String>| match r {
        Ok(done) if !done.is_empty() => log(format!("{why}: {}", done.join(", "))),
        Ok(_) => {}
        Err(e) => log(format!("{why}: {e}")),
    };
    log("служба запущена".into());
    // Драйверам после загрузки нужна пара секунд.
    std::thread::sleep(Duration::from_secs(2));
    let mut saved = load();
    let mut conf_mtime = std::fs::metadata(path()).and_then(|m| m.modified()).ok();
    let mut mode = power::read_state().mode;
    report("старт", apply_custom());
    if let Some(m) = mode {
        report("старт", apply_cpu(m));
    }
    let mut curve = FanCurve::default();
    let mut last_sleep = sleep_offset();
    let mut next_cpu = Instant::now() + CPU_PERIOD;
    let mut next_fan = Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(1));
        // Настройки меняет UI — перечитываем по mtime.
        let mtime = std::fs::metadata(path()).and_then(|m| m.modified()).ok();
        if mtime != conf_mtime {
            conf_mtime = mtime;
            let fresh = load();
            if fresh.fan_curve && !saved.fan_curve {
                curve = FanCurve::default();
            }
            saved = fresh;
            if let Some(m) = mode {
                report("настройки", apply_cpu(m));
            }
        }
        let now_mode = power::read_state().mode;
        let slept = sleep_offset();
        if slept > last_sleep + Duration::from_secs(1) {
            // После сна EC сбрасывает обороты не сразу — чуть подождать.
            std::thread::sleep(Duration::from_secs(2));
            report("после сна", apply_custom());
            if let Some(m) = now_mode {
                report("после сна", apply_cpu(m));
            }
            curve.last.clear();
        } else if now_mode != mode {
            if now_mode == Some(PowerMode::Custom) {
                std::thread::sleep(Duration::from_millis(500));
                report("включён «Свой»", apply_custom());
            } else if mode == Some(PowerMode::Custom) && !saved.fan_curve {
                let done = release_fans();
                if !done.is_empty() {
                    log(format!("выключен «Свой»: {}", done.join(", ")));
                }
            }
            if let Some(m) = now_mode {
                report("режим", apply_cpu(m));
            }
            curve.last.clear();
            next_cpu = Instant::now() + CPU_PERIOD;
        }
        mode = now_mode;
        last_sleep = slept;

        let now = Instant::now();
        if now >= next_cpu {
            next_cpu = now + CPU_PERIOD;
            if let Some(m) = mode {
                report("лимиты", apply_cpu(m));
            }
        }
        if saved.fan_curve && now >= next_fan {
            next_fan = now + FAN_PERIOD;
            let done = curve.tick(mode, &saved);
            if !done.is_empty() {
                log(format!("вентиляторы: {}", done.join(", ")));
            }
        }
    }
}

/// Состояние службы автоприменения.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceState {
    /// Юнит установлен (пакетом).
    pub installed: bool,
    pub enabled: bool,
    pub active: bool,
}

fn systemctl(args: &[&str]) -> Option<String> {
    let out = Command::new("systemctl").arg("--user").args(args).output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn service_state() -> ServiceState {
    let enabled = systemctl(&["is-enabled", UNIT]).unwrap_or_default();
    ServiceState {
        installed: !enabled.is_empty() && enabled != "not-found",
        enabled: enabled == "enabled",
        active: systemctl(&["is-active", UNIT]).as_deref() == Some("active"),
    }
}

pub fn set_service(on: bool) -> Result<(), String> {
    let out = Command::new("systemctl")
        .args(["--user", if on { "enable" } else { "disable" }, "--now", UNIT])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip() {
        let mut s = Saved::default();
        s.tunables.insert("ppt_pl1_spl".into(), 95);
        s.fans.insert(1, 3000);
        s.fans.insert(4, 0);
        s.fan_curve = true;
        s.epp = true;
        s.rapl_modes.insert("performance".into(), Limits { pl1: 150, pl2: 190 });
        let text = s.render();
        assert!(text.contains("tunable.ppt_pl1_spl=95"));
        assert!(text.contains("rapl.performance=150/190"));
        assert_eq!(Saved::parse(&text), s);
        // мусор и комментарии игнорируются, значения по умолчанию
        let p = Saved::parse("# x\nfoo\nfan.x=1\ntunable.a=oops\nfan.2 = 2500 # comment\nrapl.nope=1/2\n");
        assert_eq!(p.fans.get(&2), Some(&2500));
        assert!(p.tunables.is_empty());
        assert!(p.rapl_modes.is_empty());
        assert!(p.rapl && !p.epp && !p.fan_curve);
        let p = Saved::parse("cpu.rapl=0\n");
        assert!(!p.rapl);
    }

    #[test]
    fn profiles_roundtrip() {
        let mut s = Saved::default();
        let mut p = Profile::new(3, "Экстрим #90° = тихо", PowerMode::Extreme);
        p.rapl = Some(Limits { pl1: 160, pl2: 205 });
        p.cpu_temp = Some(90);
        s.profiles.push(p);
        let mut c = Profile::new(5, "Свой копия", PowerMode::Custom);
        c.tunables.insert("ppt_pl1_spl".into(), 120);
        c.fans.insert(1, 3000);
        s.profiles.push(c);
        s.active = Some(3);
        let text = s.render();
        assert!(text.contains("profile.3.cpu_temp=90"));
        assert_eq!(Saved::parse(&text), s);
        assert_eq!(s.next_id(), 6);

        // профиль действует только в своём режиме
        let e = s.effective(PowerMode::Extreme);
        assert_eq!(e.cpu_temp, Some(90));
        assert_eq!(e.rapl, Some(Limits { pl1: 160, pl2: 205 }));
        let e = s.effective(PowerMode::Performance);
        assert_eq!(e.cpu_temp, None);
        assert_eq!(e.rapl, Some(Limits { pl1: 145, pl2: 190 }));
        // без своих PL — заводские режима
        s.profiles[0].rapl = None;
        assert_eq!(s.effective(PowerMode::Extreme).rapl, Some(Limits { pl1: 160, pl2: 205 }));
        // профиль «Свой» подменяет встроенные лимиты и обороты
        s.active = Some(5);
        s.tunables.insert("ppt_pl1_spl".into(), 90);
        assert_eq!(s.effective(PowerMode::Custom).tunables.get("ppt_pl1_spl"), Some(&120));
        assert_eq!(s.effective(PowerMode::Custom).fans.get(&1), Some(&3000));
        s.active = None;
        assert_eq!(s.effective(PowerMode::Custom).tunables.get("ppt_pl1_spl"), Some(&90));

        // активный — только существующий; мусор не ломает разбор
        let p = Saved::parse("profile.active=9\nprofile.x.name=a\nprofile.0.name=b\nprofile.2.base=nope\n");
        assert_eq!(p.active, None);
        assert_eq!(p.profiles.len(), 1);
        assert_eq!(p.profiles[0].base, PowerMode::Custom);
    }

    #[test]
    fn limits_for_modes() {
        let mut s = Saved::default();
        assert_eq!(s.limits_for(PowerMode::Performance), Some(Limits { pl1: 145, pl2: 190 }));
        s.rapl_modes.insert("performance".into(), Limits { pl1: 100, pl2: 120 });
        assert_eq!(s.limits_for(PowerMode::Performance), Some(Limits { pl1: 100, pl2: 120 }));
        assert_eq!(s.limits_for(PowerMode::Quiet), Some(Limits { pl1: 55, pl2: 65 }));
    }

    #[test]
    fn fan_curve_shape() {
        let fan = Fan { index: 1, label: "", rpm: 0, target: 0, min: 1600, max: 5200, step: 100, writable: true };
        assert_eq!(curve_rpm(&fan, 30.0, Some(PowerMode::Balanced)), 1600);
        assert_eq!(curve_rpm(&fan, 100.0, Some(PowerMode::Balanced)), 5200);
        let mid = curve_rpm(&fan, 75.0, Some(PowerMode::Balanced));
        assert_eq!(mid, 3000);
        assert!(curve_rpm(&fan, 75.0, Some(PowerMode::Quiet)) < mid);
        assert!(curve_rpm(&fan, 75.0, Some(PowerMode::Performance)) > mid);
        assert_eq!(curve_rpm(&fan, 75.0, Some(PowerMode::Balanced)) % 100, 0);
    }
}

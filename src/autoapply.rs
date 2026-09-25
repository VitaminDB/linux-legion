//! Автоприменение значений режима «Свой» (лимиты мощности, обороты
//! вентиляторов). Прошивка держит их только до перезагрузки, а обороты — и
//! до сна, поэтому пользовательская служба `linux-legion --daemon` применяет
//! сохранённые значения при входе в систему, при переключении в «Свой» и
//! после выхода из сна.
//!
//! Значения лежат в `~/.config/linux-legion/custom.conf`:
//!
//! ```text
//! tunable.ppt_pl1_spl=95
//! fan.1=3000
//! ```

use crate::hw::fans;
use crate::hw::power::{self, PowerMode};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

pub const UNIT: &str = "linux-legion-autoapply.service";

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Переадресовать каталог настроек (демо-режим не трогает настоящие).
pub fn set_dir(dir: PathBuf) {
    let _ = DIR.set(dir);
}

fn dir() -> PathBuf {
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

/// Сохранённые значения режима «Свой».
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Saved {
    pub tunables: BTreeMap<String, i64>,
    /// Номер вентилятора → об/мин (0 — авто).
    pub fans: BTreeMap<u8, u32>,
}

impl Saved {
    fn parse(text: &str) -> Self {
        let mut s = Saved::default();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            if let Some(name) = k.strip_prefix("tunable.") {
                if let Ok(v) = v.parse() {
                    s.tunables.insert(name.to_string(), v);
                }
            } else if let Some(i) = k.strip_prefix("fan.") {
                if let (Ok(i), Ok(v)) = (i.parse(), v.parse()) {
                    s.fans.insert(i, v);
                }
            }
        }
        s
    }

    fn render(&self) -> String {
        let mut out = String::from("# linux-legion: значения режима «Свой», применяются автоматически\n");
        for (k, v) in &self.tunables {
            out.push_str(&format!("tunable.{k}={v}\n"));
        }
        for (k, v) in &self.fans {
            out.push_str(&format!("fan.{k}={v}\n"));
        }
        out
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

/// Запомнить значение, которое пользователь применил в режиме «Свой».
pub fn remember_tunable(name: &str, value: i64) {
    let mut s = load();
    s.tunables.insert(name.to_string(), value);
    let _ = store(&s);
}

pub fn remember_fan(index: u8, rpm: u32) {
    let mut s = load();
    s.fans.insert(index, rpm);
    let _ = store(&s);
}

/// Применить сохранённое, если включён режим «Свой». Пишет только то, что
/// отличается от текущего. Возвращает описание сделанного.
pub fn apply() -> Result<Vec<String>, String> {
    if power::read_state().mode != Some(PowerMode::Custom) {
        return Ok(Vec::new());
    }
    let saved = load();
    let mut done = Vec::new();
    let mut errors = Vec::new();
    let current: BTreeMap<String, i64> = power::read_tunables().into_iter().map(|t| (t.name, t.value)).collect();
    for (name, v) in &saved.tunables {
        if current.get(name) == Some(v) || !current.contains_key(name) {
            continue;
        }
        match power::set_tunable(name, *v) {
            Ok(()) => done.push(format!("{name}={v}")),
            Err(e) => errors.push(e.to_string()),
        }
    }
    let fans_now: BTreeMap<u8, u32> = fans::read_fans().into_iter().map(|f| (f.index, f.target)).collect();
    for (i, rpm) in &saved.fans {
        if fans_now.get(i) == Some(rpm) || !fans_now.contains_key(i) {
            continue;
        }
        match fans::set_target(*i, *rpm) {
            Ok(()) => done.push(format!("fan{i}={rpm}")),
            Err(e) => errors.push(e.to_string()),
        }
    }
    if errors.is_empty() {
        Ok(done)
    } else {
        Err(errors.join("; "))
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

/// `linux-legion --daemon`: следит за режимом и сном, применяет сохранённое.
pub fn daemon() -> ! {
    let log = |m: String| eprintln!("linux-legion: {m}");
    let run = |why: &str| match apply() {
        Ok(done) if !done.is_empty() => log(format!("{why}: применено {}", done.join(", "))),
        Ok(_) => {}
        Err(e) => log(format!("{why}: {e}")),
    };
    log("автоприменение режима «Свой» запущено".into());
    // Драйверам после загрузки нужна пара секунд.
    std::thread::sleep(Duration::from_secs(2));
    run("старт");
    let mut last_mode = power::read_state().mode;
    let mut last_sleep = sleep_offset();
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let mode = power::read_state().mode;
        let slept = sleep_offset();
        if slept > last_sleep + Duration::from_secs(1) {
            // После сна EC сбрасывает обороты не сразу — чуть подождать.
            std::thread::sleep(Duration::from_secs(2));
            run("после сна");
        } else if mode != last_mode && mode == Some(PowerMode::Custom) {
            std::thread::sleep(Duration::from_millis(500));
            run("включён «Свой»");
        }
        last_mode = mode;
        last_sleep = slept;
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
        let text = s.render();
        assert!(text.contains("tunable.ppt_pl1_spl=95"));
        assert_eq!(Saved::parse(&text), s);
        // мусор и комментарии игнорируются
        let p = Saved::parse("# x\nfoo\nfan.x=1\ntunable.a=oops\nfan.2 = 2500\n");
        assert_eq!(p.fans.get(&2), Some(&2500));
        assert!(p.tunables.is_empty());
    }
}

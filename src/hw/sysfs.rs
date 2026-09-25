//! Чтение и запись атрибутов sysfs. В демо-режиме все пути `/sys/...`
//! переадресуются во временный каталог с имитацией ноутбука.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Переадресовать `/sys` (вызывать до первого обращения).
pub fn set_root(root: PathBuf) {
    let _ = ROOT.set(root);
}

/// Реальный путь для `/sys/...`.
pub fn sys(path: &str) -> PathBuf {
    match ROOT.get() {
        Some(root) => root.join(path.trim_start_matches('/')),
        None => PathBuf::from(path),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Нет прав на запись (не установлено udev-правило).
    Permission(PathBuf),
    NotFound(PathBuf),
    /// Драйвер отверг значение (EINVAL/EBUSY/EOPNOTSUPP…).
    Rejected(PathBuf, String),
    Io(PathBuf, String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Permission(p) => write!(f, "нет прав на запись {} — установите udev-правило", p.display()),
            Error::NotFound(p) => write!(f, "{} не найден", p.display()),
            Error::Rejected(p, e) => write!(f, "драйвер отклонил значение ({}): {e}", p.display()),
            Error::Io(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

pub fn read_num<T: FromStr>(path: &Path) -> Option<T> {
    read(path)?.parse().ok()
}

pub fn read_bool(path: &Path) -> Option<bool> {
    read_num::<u8>(path).map(|v| v != 0)
}

pub fn write(path: &Path, value: &str) -> Result<()> {
    std::fs::write(path, value).map_err(|e| {
        let p = path.to_path_buf();
        match e.kind() {
            io::ErrorKind::PermissionDenied => Error::Permission(p),
            io::ErrorKind::NotFound => Error::NotFound(p),
            _ => match e.raw_os_error() {
                Some(libc::EINVAL) | Some(libc::EBUSY) | Some(libc::EOPNOTSUPP) | Some(libc::ENXIO) => {
                    Error::Rejected(p, e.to_string())
                }
                _ => Error::Io(p, e.to_string()),
            },
        }
    })
}

/// Первый каталог в `dir`, для которого `pred` истинно (по алфавиту).
pub fn find_dir(dir: &str, pred: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(sys(dir)).ok()?.flatten().map(|e| e.path()).collect();
    entries.sort();
    entries.into_iter().find(|p| pred(p))
}

/// Можно ли писать в файл текущему пользователю.
pub fn writable(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return false };
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

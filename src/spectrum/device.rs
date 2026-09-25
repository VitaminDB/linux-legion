//! Клавиатура Spectrum: команды высокого уровня поверх транспорта.

use super::protocol::{self as p, Effect, Packet, Rgb, REPORT_ID, REPORT_LEN};
use super::sim::SimTransport;
use std::ffi::CString;
use std::fmt;
use std::io;

pub const VID: u16 = 0x048D;

/// Паузы перед чтением ответа при повторах запроса, мс.
const QUERY_WAITS: [u64; 5] = [20, 60, 100, 150, 250];

#[derive(Debug)]
pub enum Error {
    NotFound,
    /// Нет прав на `/dev/hidrawN` (нужно udev-правило).
    Permission(String),
    Io(io::Error),
    /// Контроллер ответил не на тот запрос или не поддерживает протокол.
    Protocol(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound => write!(f, "контроллер подсветки Spectrum не найден"),
            Error::Permission(p) => write!(f, "нет доступа к {p} — установите udev-правило"),
            Error::Io(e) => write!(f, "ошибка ввода-вывода: {e}"),
            Error::Protocol(s) => write!(f, "{s}"),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Канал feature report 0x07.
pub trait Transport: Send {
    fn set_feature(&mut self, pkt: &Packet) -> Result<()>;
    fn get_feature(&mut self) -> Result<Packet>;
    fn name(&self) -> String;
}

/// Матрица зон контроллера.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyMatrix {
    pub width: usize,
    pub height: usize,
    /// `rows[y][x]`, 0 — пусто.
    pub rows: Vec<Vec<u16>>,
    /// Зоны вне матрицы (логотип на крышке).
    pub extra: Vec<u16>,
}

impl KeyMatrix {
    /// Все коды зон без повторов, в порядке обхода.
    pub fn codes(&self) -> Vec<u16> {
        let mut out = Vec::new();
        for &k in self.rows.iter().flatten().chain(self.extra.iter()) {
            if k != 0 && !out.contains(&k) {
                out.push(k);
            }
        }
        out
    }

    /// Позиция первой клетки зоны.
    pub fn position(&self, code: u16) -> Option<(usize, usize)> {
        self.rows
            .iter()
            .enumerate()
            .find_map(|(y, r)| r.iter().position(|&k| k == code).map(|x| (x, y)))
    }
}

pub struct Keyboard {
    t: Box<dyn Transport>,
    trace: bool,
}

impl Keyboard {
    pub fn open(simulate: bool) -> Result<Self> {
        let t: Box<dyn Transport> =
            if simulate { Box::new(SimTransport::new()) } else { Box::new(HidrawTransport::open()?) };
        let mut kb = Self { t, trace: std::env::var_os("LEGION_TRACE").is_some() };
        let r = kb.query(p::op::COMPATIBILITY, &[])?;
        if r[4] != 0 {
            return Err(Error::Protocol(format!("контроллер несовместим (код {})", r[4])));
        }
        Ok(kb)
    }

    pub fn name(&self) -> String {
        self.t.name()
    }

    fn query(&mut self, op: u8, params: &[u8]) -> Result<Packet> {
        self.query_checked(op, params, |_| true)
    }

    /// Запрос с ожиданием ответа. Контроллер готовит ответ не сразу (чтение
    /// профиля — до ~50 мс). Слишком раннее чтение возвращает прошлый ответ
    /// и сбрасывает текущий — дальше идут только кадры подсветки, поэтому
    /// при неудаче запрос повторяется с паузой длиннее.
    fn query_checked(&mut self, op: u8, params: &[u8], ok: impl Fn(&Packet) -> bool) -> Result<Packet> {
        let req = p::request(op, params);
        for wait in QUERY_WAITS {
            // В буфере может лежать непрочитанный ответ на прошлую команду
            // (в том числе чужого процесса) — забираем его, чтобы не принять
            // за ответ на эту.
            self.t.get_feature()?;
            self.send(&req)?;
            std::thread::sleep(std::time::Duration::from_millis(wait));
            let r = self.t.get_feature()?;
            if self.trace {
                eprintln!("<< {}", hex(&r[..24]));
            }
            if r[1] == op && ok(&r) {
                return Ok(r);
            }
        }
        Err(Error::Protocol(format!("контроллер не ответил на запрос 0x{op:02X}")))
    }

    fn send(&mut self, pkt: &Packet) -> Result<()> {
        if self.trace {
            eprintln!(">> {}", hex(&pkt[..24]));
        }
        self.t.set_feature(pkt)
    }

    pub fn key_matrix(&mut self) -> Result<KeyMatrix> {
        let r = self.query(p::op::KEY_COUNT, &[7])?;
        let (height, width) =
            p::parse_key_count(&r).ok_or_else(|| Error::Protocol("нет размера матрицы".into()))?;
        let mut rows = Vec::with_capacity(height);
        for y in 0..height {
            let r = self.query_checked(p::op::KEY_PAGE, &[7, y as u8], |r| r[4] == 7 && r[5] == y as u8)?;
            rows.push(p::parse_key_page(&r, width));
        }
        let r = self.query_checked(p::op::KEY_PAGE, &[8, 0], |r| r[4] == 8)?;
        let extra = p::parse_key_page(&r, width).into_iter().filter(|&k| k != 0).collect();
        Ok(KeyMatrix { width, height, rows, extra })
    }

    pub fn brightness(&mut self) -> Result<u8> {
        Ok(self.query(p::op::GET_BRIGHTNESS, &[])?[4])
    }

    pub fn set_brightness(&mut self, v: u8) -> Result<()> {
        self.send(&p::request(p::op::SET_BRIGHTNESS, &[v.min(p::MAX_BRIGHTNESS)]))
    }

    pub fn profile(&mut self) -> Result<u8> {
        Ok(self.query(p::op::GET_PROFILE, &[])?[4])
    }

    pub fn set_profile(&mut self, n: u8) -> Result<()> {
        self.send(&p::request(p::op::SET_PROFILE, &[n]))
    }

    /// Вернуть профилю заводские эффекты.
    pub fn reset_profile(&mut self, n: u8) -> Result<()> {
        self.send(&p::request(p::op::PROFILE_DEFAULT, &[n]))
    }

    pub fn logo(&mut self) -> Result<bool> {
        Ok(self.query(p::op::GET_LOGO, &[])?[4] == 1)
    }

    pub fn set_logo(&mut self, on: bool) -> Result<()> {
        self.send(&p::request(p::op::SET_LOGO, &[on as u8]))
    }

    pub fn effects(&mut self, profile: u8) -> Result<Vec<Effect>> {
        let r = self.query_checked(p::op::GET_EFFECTS, &[profile], |r| r[4] == profile)?;
        p::decode_effects(&r)
            .map(|(_, e)| e)
            .ok_or_else(|| Error::Protocol("не удалось разобрать описание профиля".into()))
    }

    pub fn set_effects(&mut self, profile: u8, effects: &[Effect]) -> Result<()> {
        let pkt = p::encode_effects(profile, &p::compress(effects)).map_err(|e| Error::Protocol(e.to_string()))?;
        self.send(&pkt)
    }

    /// Текущий кадр подсветки: цвет каждой зоны. `None` — вместо кадра
    /// пришёл запоздалый ответ на команду.
    pub fn state(&mut self) -> Result<Option<Vec<(u16, Rgb)>>> {
        let r = self.t.get_feature()?;
        Ok((r[1] == p::STATE_REPORT).then(|| p::parse_state(&r)))
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// hidraw

/// `_IOC(_IOC_READ|_IOC_WRITE, 'H', nr, len)` из `<linux/hidraw.h>`.
const fn hidioc(nr: u32, len: usize) -> libc::c_ulong {
    ((3 << 30) | ((len as u32) << 16) | ((b'H' as u32) << 8) | nr) as libc::c_ulong
}
const HIDIOCSFEATURE: libc::c_ulong = hidioc(0x06, REPORT_LEN);
const HIDIOCGFEATURE: libc::c_ulong = hidioc(0x07, REPORT_LEN);

pub struct HidrawTransport {
    fd: libc::c_int,
    path: String,
    pid: u16,
}

/// Интерфейс Spectrum: VID 048D, PID `c1xx`/`c9xx`, в дескрипторе есть
/// feature report 0x07 длиной 959 байт (+ Report ID).
pub fn find() -> Option<(String, u16)> {
    let mut entries: Vec<_> = std::fs::read_dir("/sys/class/hidraw").ok()?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let dev = e.path().join("device");
        let Ok(uevent) = std::fs::read_to_string(dev.join("uevent")) else { continue };
        let Some((vid, pid)) = parse_hid_id(&uevent) else { continue };
        if vid != VID || !matches!(pid & 0xFF00, 0xC100 | 0xC900) {
            continue;
        }
        let Ok(desc) = std::fs::read(dev.join("report_descriptor")) else { continue };
        if feature_report_len(&desc, REPORT_ID) == Some(REPORT_LEN - 1) {
            // Кандидат найден, но у c193 тот же дескриптор при другом протоколе —
            // окончательно решает проверка совместимости в `Keyboard::open`.
            let name = e.file_name().to_string_lossy().into_owned();
            if probe(&format!("/dev/{name}")) {
                return Some((format!("/dev/{name}"), pid));
            }
        }
    }
    None
}

/// Отвечает ли устройство на запрос совместимости. Ошибку доступа не прячем:
/// пусть `open` сообщит о ней.
fn probe(path: &str) -> bool {
    match HidrawTransport::open_path(path, 0) {
        Ok(mut t) => {
            t.set_feature(&p::request(p::op::COMPATIBILITY, &[])).is_ok()
                && t.get_feature().is_ok_and(|r| r[1] == p::op::COMPATIBILITY)
        }
        Err(Error::Permission(_)) => true,
        Err(_) => false,
    }
}

fn parse_hid_id(uevent: &str) -> Option<(u16, u16)> {
    let line = uevent.lines().find_map(|l| l.strip_prefix("HID_ID="))?;
    let mut it = line.split(':');
    let _bus = it.next()?;
    let vid = u32::from_str_radix(it.next()?, 16).ok()?;
    let pid = u32::from_str_radix(it.next()?, 16).ok()?;
    Some((vid as u16, pid as u16))
}

/// Длина feature report `id` в байтах по report descriptor.
fn feature_report_len(desc: &[u8], id: u8) -> Option<usize> {
    let (mut i, mut cur_id, mut size, mut count) = (0, 0u8, 0u32, 0u32);
    while i < desc.len() {
        let prefix = desc[i];
        if prefix == 0xFE {
            i += 3 + *desc.get(i + 1).unwrap_or(&0) as usize;
            continue;
        }
        let n = match prefix & 0x03 {
            3 => 4,
            n => n as usize,
        };
        let mut val = 0u32;
        for k in 0..n {
            val |= (*desc.get(i + 1 + k).unwrap_or(&0) as u32) << (8 * k);
        }
        match prefix & 0xFC {
            0x84 => cur_id = val as u8,
            0x74 => size = val,
            0x94 => count = val,
            0xB0 if cur_id == id => return Some((size * count / 8) as usize),
            _ => {}
        }
        i += 1 + n;
    }
    None
}

impl HidrawTransport {
    pub fn open() -> Result<Self> {
        let (path, pid) = find().ok_or(Error::NotFound)?;
        Self::open_path(&path, pid)
    }

    fn open_path(path: &str, pid: u16) -> Result<Self> {
        let cpath = CString::new(path).map_err(|_| Error::NotFound)?;
        let fd = unsafe { libc::open(cpath.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
        if fd < 0 {
            let e = io::Error::last_os_error();
            return Err(match e.kind() {
                io::ErrorKind::PermissionDenied => Error::Permission(path.to_string()),
                io::ErrorKind::NotFound => Error::NotFound,
                _ => Error::Io(e),
            });
        }
        Ok(Self { fd, path: path.to_string(), pid })
    }

    fn ioctl(&mut self, req: libc::c_ulong, buf: &mut Packet) -> Result<()> {
        let rc = unsafe { libc::ioctl(self.fd, req, buf.as_mut_ptr()) };
        if rc < 0 {
            let e = io::Error::last_os_error();
            return Err(match e.raw_os_error() {
                Some(libc::ENODEV) | Some(libc::ENXIO) => Error::NotFound,
                _ => Error::Io(e),
            });
        }
        Ok(())
    }
}

impl Transport for HidrawTransport {
    fn set_feature(&mut self, pkt: &Packet) -> Result<()> {
        let mut buf = *pkt;
        self.ioctl(HIDIOCSFEATURE, &mut buf)
    }

    fn get_feature(&mut self) -> Result<Packet> {
        let mut buf = [0u8; REPORT_LEN];
        buf[0] = REPORT_ID;
        self.ioctl(HIDIOCGFEATURE, &mut buf)?;
        Ok(buf)
    }

    fn name(&self) -> String {
        format!("ITE {:04x} · {}", self.pid, self.path.trim_start_matches("/dev/"))
    }
}

impl Drop for HidrawTransport {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Report descriptor интерфейса Spectrum Legion Pro 7 16IAX10H (начало).
    const DESC: &[u8] = &[
        0x06, 0x89, 0xff, 0x09, 0x10, 0xa1, 0x01, 0x85, 0x5a, 0x09, 0x01, 0x15, 0x00, 0x26, 0xff, 0x00,
        0x75, 0x08, 0x95, 0x10, 0xb1, 0x00, 0xc0, 0x06, 0x89, 0xff, 0x09, 0x07, 0xa1, 0x01, 0x85, 0x07,
        0x09, 0x01, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x96, 0xbf, 0x03, 0xb1, 0x00, 0xc0,
    ];

    #[test]
    fn ioctl_numbers() {
        assert_eq!(HIDIOCSFEATURE, 0xC3C0_4806);
        assert_eq!(HIDIOCGFEATURE, 0xC3C0_4807);
    }

    #[test]
    fn finds_report_length() {
        assert_eq!(feature_report_len(DESC, 0x07), Some(959));
        assert_eq!(feature_report_len(DESC, 0x5A), Some(16));
        assert_eq!(feature_report_len(DESC, 0x01), None);
    }

    /// Запись на живой клавиатуре: `cargo test -- --ignored live_`.
    /// Пишет тестовые слои в неактивный профиль, сверяет, возвращает исходные
    /// и снова включает профиль, который был активен (чтение переключает).
    #[test]
    #[ignore]
    fn live_write_and_restore_profile() {
        use crate::spectrum::protocol::{EffectType, Rgb};
        let mut kb = Keyboard::open(false).expect("клавиатура не найдена");
        let active = kb.profile().unwrap();
        let target = if active == 3 { 4 } else { 3 };
        let original = kb.effects(target).unwrap();
        let test = vec![
            Effect::new(EffectType::RainbowWave, (1..=0x20).collect()),
            Effect { colors: vec![Rgb::new(255, 0, 0)], ..Effect::new(EffectType::Always, vec![0x43, 0x6d, 0x6e, 0x58]) },
        ];
        kb.set_effects(target, &test).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let back = kb.effects(target).unwrap();
        kb.set_effects(target, &original).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let restored = kb.effects(target).unwrap();
        kb.set_profile(active).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(kb.profile().unwrap(), active, "активный профиль должен вернуться");
        assert_eq!(back, crate::spectrum::protocol::compress(&test));
        assert_eq!(restored, original);
    }

    #[test]
    fn parses_uevent() {
        assert_eq!(parse_hid_id("HID_ID=0003:0000048D:0000C197\n"), Some((0x048D, 0xC197)));
    }
}

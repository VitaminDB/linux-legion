//! Физическое расположение зон подсветки для визуального редактора.
//!
//! Координаты — в «клавишах» (1 = ширина обычной клавиши). Основной блок
//! шириной 15, через полклавиши — цифровой блок шириной 4. Зоны корпуса
//! (задние вентиляционные отверстия, боковины, передняя полоса) раскладываются
//! вокруг клавиатуры в порядке, в котором их отдаёт матрица контроллера.

use super::device::KeyMatrix;

#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub code: u16,
    pub label: &'static str,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub group: Group,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    Keys,
    Numpad,
    Rear,
    Side,
    Front,
    Logo,
}

/// Строка основного блока: (код, подпись, ширина); код 0 — пропуск.
type Row = &'static [(u16, &'static str, f32)];

const F: f32 = 15.0 / 16.0;

const MAIN: [(f32, f32, Row); 7] = [
    (0.0, 0.8, &[
        (0x01, "Esc", F), (0x02, "F1", F), (0x03, "F2", F), (0x04, "F3", F), (0x05, "F4", F),
        (0x06, "F5", F), (0x07, "F6", F), (0x08, "F7", F), (0x09, "F8", F), (0x0A, "F9", F),
        (0x0B, "F10", F), (0x0C, "F11", F), (0x0D, "F12", F), (0x0E, "Ins", F), (0x0F, "PrtSc", F),
        (0x10, "Del", F),
    ]),
    (1.0, 1.0, &[
        (0x16, "`", 1.0), (0x17, "1", 1.0), (0x18, "2", 1.0), (0x19, "3", 1.0), (0x1A, "4", 1.0),
        (0x1B, "5", 1.0), (0x1C, "6", 1.0), (0x1D, "7", 1.0), (0x1E, "8", 1.0), (0x1F, "9", 1.0),
        (0x20, "0", 1.0), (0x21, "-", 1.0), (0x22, "=", 1.0), (0x38, "Backspace", 2.0),
    ]),
    (2.0, 1.0, &[
        (0x40, "Tab", 1.5), (0x42, "Q", 1.0), (0x43, "W", 1.0), (0x44, "E", 1.0), (0x45, "R", 1.0),
        (0x46, "T", 1.0), (0x47, "Y", 1.0), (0x48, "U", 1.0), (0x49, "I", 1.0), (0x4A, "O", 1.0),
        (0x4B, "P", 1.0), (0x4C, "[", 1.0), (0x4D, "]", 1.0), (0x4E, "\\", 1.5),
    ]),
    (3.0, 1.0, &[
        (0x55, "Caps", 1.75), (0x6D, "A", 1.0), (0x6E, "S", 1.0), (0x58, "D", 1.0), (0x59, "F", 1.0),
        (0x5A, "G", 1.0), (0x71, "H", 1.0), (0x72, "J", 1.0), (0x5B, "K", 1.0), (0x5C, "L", 1.0),
        (0x5D, ";", 1.0), (0x5F, "'", 1.0), (0x77, "Enter", 2.25),
    ]),
    (4.0, 1.0, &[
        (0x6A, "Shift", 2.25), (0x82, "Z", 1.0), (0x83, "X", 1.0), (0x6F, "C", 1.0), (0x70, "V", 1.0),
        (0x87, "B", 1.0), (0x88, "N", 1.0), (0x73, "M", 1.0), (0x74, ",", 1.0), (0x75, ".", 1.0),
        (0x76, "/", 1.0), (0x8D, "Shift", 2.75),
    ]),
    (5.0, 1.0, &[
        (0x7F, "Ctrl", 1.25), (0x80, "Fn", 1.0), (0x96, "Win", 1.0), (0x97, "Alt", 1.25),
        (0x98, "", 5.5), (0x9A, "Alt", 1.0), (0x9B, "Ctrl", 1.0), (0, "", 1.0), (0x9D, "↑", 1.0),
    ]),
    (6.0, 1.0, &[(0, "", 12.0), (0x9C, "←", 1.0), (0x9F, "↓", 1.0), (0xA1, "→", 1.0)]),
];

/// Цифровой блок: (код, подпись, x, y, w, h), x от левого края блока.
const NUMPAD: &[(u16, &str, f32, f32, f32, f32)] = &[
    (0x11, "Home", 0.0, 0.0, 1.0, 0.8),
    (0x12, "End", 1.0, 0.0, 1.0, 0.8),
    (0x13, "PgUp", 2.0, 0.0, 1.0, 0.8),
    (0x14, "PgDn", 3.0, 0.0, 1.0, 0.8),
    (0x26, "Num", 0.0, 1.0, 1.0, 1.0),
    (0x27, "/", 1.0, 1.0, 1.0, 1.0),
    (0x28, "*", 2.0, 1.0, 1.0, 1.0),
    (0x29, "-", 3.0, 1.0, 1.0, 1.0),
    (0x4F, "7", 0.0, 2.0, 1.0, 1.0),
    (0x50, "8", 1.0, 2.0, 1.0, 1.0),
    (0x51, "9", 2.0, 2.0, 1.0, 1.0),
    (0x68, "+", 3.0, 2.0, 1.0, 2.0),
    (0x79, "4", 0.0, 3.0, 1.0, 1.0),
    (0x7B, "5", 1.0, 3.0, 1.0, 1.0),
    (0x7C, "6", 2.0, 3.0, 1.0, 1.0),
    (0x8E, "1", 0.0, 4.0, 1.0, 1.0),
    (0x90, "2", 1.0, 4.0, 1.0, 1.0),
    (0x92, "3", 2.0, 4.0, 1.0, 1.0),
    (0xA7, "Enter", 3.0, 4.0, 1.0, 2.0),
    (0xA3, "0", 0.0, 5.0, 2.0, 1.0),
    (0xA5, ".", 2.0, 5.0, 1.0, 1.0),
];

pub const NUMPAD_X: f32 = 15.5;
pub const KEYBOARD_W: f32 = NUMPAD_X + 4.0;
pub const KEYBOARD_H: f32 = 7.0;

/// Полосы корпуса: отступ от клавиатуры и толщина.
pub const STRIP: f32 = 0.35;
pub const STRIP_GAP: f32 = 0.35;

pub const LOGO: u16 = 0x5DD;

/// Классификация зон корпуса по коду.
fn case_group(code: u16) -> Option<Group> {
    match code {
        0x3E9..=0x3FF => Some(Group::Rear),
        0x1F5 | 0x1F6 | 0x1FD | 0x1FE => Some(Group::Side),
        0x1F7..=0x1FC => Some(Group::Front),
        LOGO => Some(Group::Logo),
        _ => None,
    }
}

/// Раскладка для конкретного контроллера: только зоны, которые он знает.
/// Незнакомые коды уходят в строку «прочее» под клавиатурой.
pub fn build(matrix: &KeyMatrix) -> Vec<Zone> {
    let present = matrix.codes();
    let has = |c: u16| present.contains(&c);
    let mut zones = Vec::new();

    let y0 = STRIP + STRIP_GAP;
    let x0 = STRIP + STRIP_GAP;
    for (y, h, row) in MAIN {
        let mut x = 0.0;
        for &(code, label, w) in row {
            if code != 0 && has(code) {
                zones.push(Zone { code, label, x: x0 + x, y: y0 + y, w, h, group: Group::Keys });
            }
            x += w;
        }
    }
    for &(code, label, x, y, w, h) in NUMPAD {
        if has(code) {
            zones.push(Zone { code, label, x: x0 + NUMPAD_X + x, y: y0 + y, w, h, group: Group::Numpad });
        }
    }

    // Зоны корпуса по порядку в матрице.
    let order = |g: Group, by_row: bool| -> Vec<u16> {
        let mut v: Vec<(usize, usize, u16)> = present
            .iter()
            .filter(|&&c| case_group(c) == Some(g))
            .map(|&c| {
                let (x, y) = matrix.position(c).unwrap_or((c as usize, 0));
                (x, y, c)
            })
            .collect();
        v.sort_by_key(|&(x, y, c)| if by_row { (y, x, c) } else { (x, y, c) });
        v.into_iter().map(|t| t.2).collect()
    };
    let spread = |zones: &mut Vec<Zone>, codes: Vec<u16>, g: Group, x: f32, y: f32, len: f32, horizontal: bool| {
        let n = codes.len().max(1) as f32;
        for (i, code) in codes.into_iter().enumerate() {
            let step = len / n;
            let (zx, zy, w, h) = if horizontal {
                (x + i as f32 * step, y, step, STRIP)
            } else {
                (x, y + i as f32 * step, STRIP, step)
            };
            zones.push(Zone { code, label: "", x: zx, y: zy, w, h, group: g });
        }
    };
    spread(&mut zones, order(Group::Rear, false), Group::Rear, x0, 0.0, KEYBOARD_W, true);
    let sides = order(Group::Side, true);
    let (left, right): (Vec<u16>, Vec<u16>) = sides.into_iter().partition(|&c| {
        matrix.position(c).map_or(c < 0x1F9, |(x, _)| x < matrix.width / 2)
    });
    spread(&mut zones, left, Group::Side, 0.0, y0, KEYBOARD_H, false);
    spread(&mut zones, right, Group::Side, x0 + KEYBOARD_W + STRIP_GAP, y0, KEYBOARD_H, false);
    spread(
        &mut zones,
        order(Group::Front, false),
        Group::Front,
        x0,
        y0 + KEYBOARD_H + STRIP_GAP,
        KEYBOARD_W,
        true,
    );

    // Логотип на крышке рисуется отдельно, вне схемы корпуса.
    if has(LOGO) {
        zones.push(Zone { code: LOGO, label: "LEGION", x: 0.0, y: 0.0, w: 0.0, h: 0.0, group: Group::Logo });
    }

    let known: Vec<u16> = zones.iter().map(|z| z.code).collect();
    let mut x = x0;
    let extra_y = y0 + KEYBOARD_H + STRIP_GAP + STRIP + 0.4;
    for &code in &present {
        if known.contains(&code) || code == LOGO {
            continue;
        }
        zones.push(Zone { code, label: "?", x, y: extra_y, w: 0.9, h: 0.9, group: Group::Keys });
        x += 1.0;
    }
    zones
}

/// Полные размеры раскладки (без логотипа).
pub fn extent(zones: &[Zone]) -> (f32, f32) {
    zones
        .iter()
        .filter(|z| z.group != Group::Logo)
        .fold((0.0f32, 0.0f32), |(w, h), z| (w.max(z.x + z.w), h.max(z.y + z.h)))
}

/// Типовые наборы для быстрого выделения.
pub fn preset(zones: &[Zone], name: Preset) -> Vec<u16> {
    let pick = |f: &dyn Fn(&Zone) -> bool| zones.iter().filter(|z| f(z)).map(|z| z.code).collect();
    match name {
        Preset::All => pick(&|_| true),
        Preset::Keyboard => pick(&|z| matches!(z.group, Group::Keys | Group::Numpad)),
        Preset::Case => pick(&|z| matches!(z.group, Group::Rear | Group::Side | Group::Front | Group::Logo)),
        Preset::Wasd => pick(&|z| matches!(z.code, 0x43 | 0x6D | 0x6E | 0x58)),
        Preset::Arrows => pick(&|z| matches!(z.code, 0x9C | 0x9D | 0x9F | 0xA1)),
        Preset::FRow => pick(&|z| (0x01..=0x14).contains(&z.code)),
        Preset::Digits => pick(&|z| (0x16..=0x22).contains(&z.code)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    All,
    Keyboard,
    Case,
    Wasd,
    Arrows,
    FRow,
    Digits,
}

impl Preset {
    pub const ALL: [Preset; 7] =
        [Preset::All, Preset::Keyboard, Preset::Case, Preset::Wasd, Preset::Arrows, Preset::FRow, Preset::Digits];

    pub fn label(self) -> &'static str {
        match self {
            Preset::All => "Все зоны",
            Preset::Keyboard => "Клавиатура",
            Preset::Case => "Корпус",
            Preset::Wasd => "WASD",
            Preset::Arrows => "Стрелки",
            Preset::FRow => "F-ряд",
            Preset::Digits => "Цифры",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::sim::{LOGO as SIM_LOGO, MATRIX};
    use super::*;

    fn matrix() -> KeyMatrix {
        KeyMatrix {
            width: 22,
            height: 9,
            rows: MATRIX.iter().map(|r| r.to_vec()).collect(),
            extra: vec![SIM_LOGO],
        }
    }

    #[test]
    fn every_zone_of_live_keyboard_is_placed() {
        let m = matrix();
        let zones = build(&m);
        let codes: Vec<u16> = zones.iter().map(|z| z.code).collect();
        for c in m.codes() {
            if c != LOGO {
                assert!(codes.contains(&c), "зона 0x{c:X} не размещена");
            }
        }
        // ни одной «неизвестной» зоны
        assert!(zones.iter().all(|z| z.label != "?"));
        assert_eq!(zones.iter().filter(|z| z.group == Group::Rear).count(), 18);
        assert_eq!(zones.iter().filter(|z| z.group == Group::Front).count(), 6);
        assert_eq!(zones.iter().filter(|z| z.group == Group::Side).count(), 4);
    }

    #[test]
    fn main_rows_have_equal_width() {
        for (_, _, row) in MAIN.iter().take(6) {
            let w: f32 = row.iter().map(|r| r.2).sum();
            assert!((w - 15.0).abs() < 0.01 || w < 15.0, "строка {w}");
        }
    }

    #[test]
    fn sides_split_left_right() {
        let zones = build(&matrix());
        let left: Vec<u16> = zones.iter().filter(|z| z.group == Group::Side && z.x < 1.0).map(|z| z.code).collect();
        assert_eq!(left, vec![0x1F5, 0x1F6]);
    }
}

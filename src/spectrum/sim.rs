//! Симулятор контроллера Spectrum для `--simulate`: матрица Legion Pro 7
//! 16IAX10H, шесть профилей и приблизительная анимация эффектов.

use super::device::{Result, Transport};
use super::protocol::{self as p, op, Effect, EffectType, Packet, Rgb, REPORT_LEN};
use std::time::Instant;

/// Матрица зон живой клавиатуры (22 × 9) и зона вне матрицы.
pub const MATRIX: [[u16; 22]; 9] = [
    [0, 0, 0x3e9, 0x3f3, 0x3ea, 0x3f4, 0x3eb, 0x3f5, 0x3ec, 0x3f6, 0x3ed, 0x3ee, 0x3ef, 0x3f7, 0x3f0, 0x3f8, 0x3f1, 0x3f9, 0x3f2, 0x3fa, 0, 0],
    [0, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0],
    [0, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0, 0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x38, 0x38, 0x26, 0x27, 0x28, 0x29, 0],
    [0, 0x40, 0x42, 0, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0, 0x4c, 0x4d, 0x4e, 0x4f, 0x50, 0x51, 0x68, 0],
    [0, 0x55, 0x55, 0x6d, 0x6e, 0x58, 0x59, 0x5a, 0x71, 0, 0x72, 0x5b, 0x5c, 0x5d, 0x5f, 0x77, 0x77, 0x79, 0x7b, 0x7c, 0x68, 0],
    [0x1f5, 0x6a, 0x6a, 0x82, 0x83, 0, 0x6f, 0x70, 0x87, 0x88, 0x73, 0x74, 0x75, 0, 0x76, 0x8d, 0x8d, 0x8e, 0x90, 0x92, 0xa7, 0x1fe],
    [0x1f5, 0x7f, 0x80, 0x96, 0x97, 0x98, 0x98, 0x98, 0x98, 0x98, 0x98, 0x9a, 0x9b, 0, 0, 0x9d, 0, 0xa3, 0xa3, 0xa5, 0xa7, 0x1fe],
    [0x1f6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x9c, 0x9f, 0, 0xa1, 0, 0, 0, 0x1fd],
    [0x1f6, 0x1f7, 0x1f7, 0x1f7, 0x1f7, 0x1f8, 0x1f8, 0x1f8, 0x1f9, 0x1f9, 0x1f9, 0x1fa, 0x1fa, 0x1fa, 0x1fb, 0x1fb, 0x1fb, 0x1fc, 0x1fc, 0x1fc, 0x1fc, 0x1fd],
];
pub const LOGO: u16 = 0x5dd;

pub struct SimTransport {
    started: Instant,
    brightness: u8,
    profile: u8,
    logo: bool,
    profiles: [Vec<Effect>; 6],
    /// Ответ на последний запрос; `None` — вернуть кадр подсветки.
    pending: Option<Packet>,
}

impl SimTransport {
    pub fn new() -> Self {
        let all = all_codes();
        let keys: Vec<u16> = all.iter().copied().filter(|&k| k < 0x100).collect();
        let case: Vec<u16> = all.iter().copied().filter(|&k| k >= 0x100).collect();
        let solid = |c: Rgb| Effect { colors: vec![c], ..Effect::new(EffectType::Always, all.clone()) };
        let wasd = vec![0x43, 0x6d, 0x6e, 0x58];
        let profiles = [
            vec![solid(Rgb::new(0xff, 0xff, 0xff))],
            vec![solid(Rgb::new(0x15, 0x8d, 0xdd))],
            vec![Effect::new(EffectType::RainbowWave, all.clone())],
            vec![
                Effect::new(EffectType::RainbowScrew, case),
                solid(Rgb::new(0x40, 0x00, 0x80)).with_keys(keys),
                solid(Rgb::new(0xff, 0x30, 0x10)).with_keys(wasd),
            ],
            vec![Effect { colors: vec![], ..Effect::new(EffectType::ColorPulse, all.clone()) }],
            vec![Effect::new(EffectType::Smooth, all)],
        ];
        Self { started: Instant::now(), brightness: 5, profile: 3, logo: true, profiles, pending: None }
    }

    fn respond(&mut self, req: &Packet) -> Packet {
        let mut r = [0u8; REPORT_LEN];
        r[0] = p::REPORT_ID;
        r[1] = req[1];
        r[2] = 1;
        match req[1] {
            op::COMPATIBILITY => r[4] = 0,
            op::KEY_COUNT => {
                r[2] = 3;
                r[4] = 7;
                r[5] = MATRIX.len() as u8;
                r[6] = MATRIX[0].len() as u8;
            }
            op::KEY_PAGE => {
                r[4] = req[4];
                r[5] = req[5];
                let row: Vec<u16> = if req[4] == 8 {
                    let mut v = vec![0; 22];
                    v[0] = LOGO;
                    v
                } else {
                    MATRIX.get(req[5] as usize).map(|r| r.to_vec()).unwrap_or_default()
                };
                for (x, k) in row.iter().enumerate() {
                    r[6 + 3 * x] = x as u8;
                    r[7 + 3 * x..9 + 3 * x].copy_from_slice(&k.to_le_bytes());
                }
            }
            op::GET_BRIGHTNESS => r[4] = self.brightness,
            op::GET_PROFILE => r[4] = self.profile,
            op::GET_LOGO => r[4] = self.logo as u8,
            op::GET_EFFECTS => {
                let n = req[4].clamp(1, 6);
                let mut pkt = p::encode_effects(n, &self.profiles[n as usize - 1]).unwrap_or(r);
                pkt[1] = op::GET_EFFECTS;
                pkt[2] = 0xC0;
                return pkt;
            }
            _ => {}
        }
        r
    }

    /// Кадр анимации: цвета по эффектам текущего профиля.
    fn frame(&self) -> Packet {
        let t = self.started.elapsed().as_secs_f32();
        let dim = self.brightness as f32 / p::MAX_BRIGHTNESS as f32;
        let mut colors: std::collections::HashMap<u16, Rgb> = all_codes().into_iter().map(|k| (k, Rgb::default())).collect();
        for e in &self.profiles[self.profile as usize - 1] {
            let keys: Vec<u16> = if e.keys.is_empty() { all_codes() } else { e.keys.clone() };
            for k in keys {
                let (x, y) = position(k);
                let speed = e.speed.max(1) as f32;
                let c = match e.kind {
                    EffectType::Always => e.colors.first().copied().unwrap_or_default(),
                    EffectType::ColorPulse | EffectType::ColorChange => {
                        let base = pick(&e.colors, (t * 0.3 * speed) as usize);
                        scale(base, 0.5 + 0.5 * (t * speed * 2.0).sin())
                    }
                    EffectType::RainbowScrew => {
                        let a = ((y - 4.0).atan2(x - 11.0) / std::f32::consts::TAU) + t * 0.2 * speed;
                        hsv(a)
                    }
                    _ => hsv(x / 22.0 - t * 0.15 * speed + y / 30.0),
                };
                colors.insert(k, c);
            }
        }
        let mut r = [0u8; REPORT_LEN];
        r[0] = p::REPORT_ID;
        r[1] = p::STATE_REPORT;
        let mut pos = 4;
        let mut list: Vec<_> = colors.into_iter().collect();
        list.sort_by_key(|(k, _)| *k);
        for (k, c) in list {
            if !self.logo && k == LOGO {
                continue;
            }
            let c = scale(c, dim);
            r[pos..pos + 2].copy_from_slice(&k.to_le_bytes());
            r[pos + 2] = c.r;
            r[pos + 3] = c.g;
            r[pos + 4] = c.b;
            pos += 5;
        }
        r
    }
}

impl Effect {
    fn with_keys(mut self, keys: Vec<u16>) -> Self {
        self.keys = keys;
        self
    }
}

impl Transport for SimTransport {
    fn set_feature(&mut self, req: &Packet) -> Result<()> {
        match req[1] {
            op::SET_BRIGHTNESS => self.brightness = req[4].min(p::MAX_BRIGHTNESS),
            op::SET_PROFILE => self.profile = req[4].clamp(1, 6),
            op::SET_LOGO => self.logo = req[4] == 1,
            op::PROFILE_DEFAULT => {
                let n = req[4].clamp(1, 6) as usize;
                self.profiles[n - 1] = SimTransport::new().profiles[n - 1].clone();
            }
            op::SET_EFFECTS => {
                let mut resp = *req;
                resp[1] = op::GET_EFFECTS;
                if let Some((n, e)) = p::decode_effects(&resp) {
                    self.profiles[n.clamp(1, 6) as usize - 1] = e;
                }
            }
            _ => {
                self.pending = Some(self.respond(req));
                return Ok(());
            }
        }
        self.pending = None;
        Ok(())
    }

    fn get_feature(&mut self) -> Result<Packet> {
        std::thread::sleep(std::time::Duration::from_millis(2));
        Ok(self.pending.take().unwrap_or_else(|| self.frame()))
    }

    fn name(&self) -> String {
        "симулятор Spectrum".into()
    }
}

fn all_codes() -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    for &k in MATRIX.iter().flatten() {
        if k != 0 && !out.contains(&k) {
            out.push(k);
        }
    }
    out.push(LOGO);
    out
}

fn position(code: u16) -> (f32, f32) {
    for (y, row) in MATRIX.iter().enumerate() {
        if let Some(x) = row.iter().position(|&k| k == code) {
            return (x as f32, y as f32);
        }
    }
    (11.0, 0.0)
}

fn pick(colors: &[Rgb], i: usize) -> Rgb {
    if colors.is_empty() {
        hsv(i as f32 * 0.17)
    } else {
        colors[i % colors.len()]
    }
}

fn scale(c: Rgb, k: f32) -> Rgb {
    let f = |v: u8| (v as f32 * k.clamp(0.0, 1.0)) as u8;
    Rgb::new(f(c.r), f(c.g), f(c.b))
}

/// Цвет радуги по фазе 0…1.
fn hsv(h: f32) -> Rgb {
    let h = h.rem_euclid(1.0) * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    Rgb::new((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::super::device::Keyboard;
    use super::*;

    #[test]
    fn simulated_keyboard_roundtrip() {
        let mut kb = Keyboard::open(true).unwrap();
        let m = kb.key_matrix().unwrap();
        assert_eq!((m.width, m.height), (22, 9));
        assert_eq!(m.extra, vec![LOGO]);
        assert_eq!(kb.profile().unwrap(), 3);
        kb.set_profile(2).unwrap();
        assert_eq!(kb.profile().unwrap(), 2);
        let e = vec![Effect::new(EffectType::Rain, vec![1, 2, 3])];
        kb.set_effects(5, &e).unwrap();
        assert_eq!(kb.effects(5).unwrap(), e);
        assert!(!kb.state().unwrap().unwrap().is_empty());
    }
}

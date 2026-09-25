//! Протокол RGB-контроллера Lenovo Legion Spectrum (ITE 8258, `048d:c197` и
//! семейство `048d:c9xx`).
//!
//! Обмен идёт одним feature report `0x07` длиной 960 байт. Запрос —
//! `HIDIOCSFEATURE`, ответ на него забирается следующим `HIDIOCGFEATURE`:
//!
//! ```text
//! запрос: [07] [op] [C0] [03] [параметры…]
//! ответ:  [07] [op] [len] [00] [данные…]
//! ```
//!
//! `HIDIOCGFEATURE` без предшествующего запроса возвращает текущие цвета всех
//! зон — «живой» кадр анимации (см. [`parse_state`]).
//!
//! Особенности прошивки Legion Pro 7 16IAX10H (`c197`), найденные на железе:
//! * ответ готовится до ~50 мс; раннее чтение отдаёт прошлый ответ и
//!   сбрасывает текущий — запрос надо повторять;
//! * непрочитанный ответ лежит в буфере до следующего чтения, даже от
//!   другого процесса;
//! * чтение слоёв профиля (`0xCC`) делает этот профиль активным.
//!
//! Формат описания профиля (команды `0xCB`/`0xCC`) сверен с Lenovo Legion
//! Toolkit и с дампами живой клавиатуры Legion Pro 7 16IAX10H.

use std::fmt;

pub const REPORT_ID: u8 = 0x07;
pub const REPORT_LEN: usize = 960;
/// Второй байт кадра подсветки (ответ `HIDIOCGFEATURE` без запроса).
pub const STATE_REPORT: u8 = 0x03;

/// Коды операций (второй байт пакета).
pub mod op {
    pub const KEY_COUNT: u8 = 0xC4;
    pub const KEY_PAGE: u8 = 0xC5;
    pub const SET_PROFILE: u8 = 0xC8;
    pub const PROFILE_DEFAULT: u8 = 0xC9;
    pub const GET_PROFILE: u8 = 0xCA;
    pub const SET_EFFECTS: u8 = 0xCB;
    pub const GET_EFFECTS: u8 = 0xCC;
    pub const GET_BRIGHTNESS: u8 = 0xCD;
    pub const SET_BRIGHTNESS: u8 = 0xCE;
    pub const COMPATIBILITY: u8 = 0xD1;
    pub const GET_LOGO: u8 = 0xA5;
    pub const SET_LOGO: u8 = 0xA6;
}

/// Максимальная яркость (0 — выключено).
pub const MAX_BRIGHTNESS: u8 = 9;
/// Аппаратные профили переключаются Fn+Пробел.
pub const PROFILES: std::ops::RangeInclusive<u8> = 1..=6;
/// Условный код «все зоны» у эффектов, которые всегда охватывают всё устройство.
const ALL_ZONES: u16 = 0x65;

pub type Packet = [u8; REPORT_LEN];

/// Запрос `op` с параметрами.
pub fn request(op: u8, params: &[u8]) -> Packet {
    let mut b = [0u8; REPORT_LEN];
    b[0] = REPORT_ID;
    b[1] = op;
    b[2] = 0xC0;
    b[3] = 0x03;
    b[4..4 + params.len()].copy_from_slice(params);
    b
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EffectType {
    RainbowScrew = 1,
    RainbowWave = 2,
    ColorChange = 3,
    ColorPulse = 4,
    ColorWave = 5,
    Smooth = 6,
    Rain = 7,
    Ripple = 8,
    AudioBounce = 9,
    AudioRipple = 10,
    Always = 11,
    Type = 12,
    AuroraSync = 13,
}

/// Какие цвета задаёт эффект.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorKind {
    None,
    /// Ровно один цвет.
    Single,
    /// Список цветов; пустой — случайные цвета.
    Multi,
}

impl EffectType {
    /// Эффекты, которые можно выбрать в редакторе (синхронизация с экраном
    /// требует захвата экрана и пока не поддерживается).
    pub const EDITABLE: [EffectType; 12] = [
        EffectType::Always,
        EffectType::RainbowWave,
        EffectType::RainbowScrew,
        EffectType::ColorWave,
        EffectType::ColorChange,
        EffectType::ColorPulse,
        EffectType::Smooth,
        EffectType::Rain,
        EffectType::Ripple,
        EffectType::Type,
        EffectType::AudioBounce,
        EffectType::AudioRipple,
    ];

    pub fn from_u8(v: u8) -> Option<Self> {
        use EffectType::*;
        Some(match v {
            1 => RainbowScrew,
            2 => RainbowWave,
            3 => ColorChange,
            4 => ColorPulse,
            5 => ColorWave,
            6 => Smooth,
            7 => Rain,
            8 => Ripple,
            9 => AudioBounce,
            10 => AudioRipple,
            11 => Always,
            12 => Type,
            13 => AuroraSync,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        use EffectType::*;
        match self {
            Always => "Статичный цвет",
            RainbowWave => "Радужная волна",
            RainbowScrew => "Радужная спираль",
            ColorWave => "Цветная волна",
            ColorChange => "Смена цветов",
            ColorPulse => "Пульсация",
            Smooth => "Плавный переход",
            Rain => "Дождь",
            Ripple => "Рябь от нажатий",
            Type => "Подсветка нажатий",
            AudioBounce => "Эквалайзер",
            AudioRipple => "Звуковая рябь",
            AuroraSync => "Синхронизация с экраном",
        }
    }

    pub fn has_speed(self) -> bool {
        use EffectType::*;
        matches!(self, ColorChange | ColorPulse | ColorWave | Rain | RainbowScrew | RainbowWave | Ripple | Smooth | Type)
    }

    /// Направление волны (вверх/вниз/влево/вправо).
    pub fn has_direction(self) -> bool {
        matches!(self, EffectType::ColorWave | EffectType::RainbowWave)
    }

    /// Направление вращения.
    pub fn has_rotation(self) -> bool {
        self == EffectType::RainbowScrew
    }

    pub fn color_kind(self) -> ColorKind {
        use EffectType::*;
        match self {
            Always => ColorKind::Single,
            ColorChange | ColorPulse | ColorWave | Rain | Ripple | Smooth | Type => ColorKind::Multi,
            _ => ColorKind::None,
        }
    }

    /// Эффект всегда занимает все зоны и отменяет остальные слои.
    pub fn is_all_zones(self) -> bool {
        matches!(self, EffectType::AudioBounce | EffectType::AudioRipple | EffectType::AuroraSync)
    }

    /// Эффект реагирует на всю клавиатуру — его нельзя частично перекрыть.
    pub fn is_whole_keyboard(self) -> bool {
        matches!(self, EffectType::Type | EffectType::Ripple)
    }
}

/// Направление волны.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum Direction {
    #[default]
    None = 0,
    BottomToTop = 1,
    TopToBottom = 2,
    RightToLeft = 3,
    LeftToRight = 4,
}

impl Direction {
    pub const ALL: [Direction; 4] =
        [Direction::LeftToRight, Direction::RightToLeft, Direction::BottomToTop, Direction::TopToBottom];

    fn from_u8(v: u8) -> Self {
        match v {
            1 => Direction::BottomToTop,
            2 => Direction::TopToBottom,
            3 => Direction::RightToLeft,
            4 => Direction::LeftToRight,
            _ => Direction::None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Direction::LeftToRight => "Вправо",
            Direction::RightToLeft => "Влево",
            Direction::BottomToTop => "Вверх",
            Direction::TopToBottom => "Вниз",
            Direction::None => "—",
        }
    }
}

/// Направление вращения спирали.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum Rotation {
    #[default]
    None = 0,
    Clockwise = 1,
    CounterClockwise = 2,
}

impl Rotation {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Rotation::Clockwise,
            2 => Rotation::CounterClockwise,
            _ => Rotation::None,
        }
    }
}

/// Один слой эффекта в профиле.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Effect {
    pub kind: EffectType,
    /// 1…3, 0 — не задано.
    pub speed: u8,
    pub direction: Direction,
    pub rotation: Rotation,
    pub colors: Vec<Rgb>,
    /// Зоны (коды клавиш). У эффектов [`EffectType::is_all_zones`] пусто.
    pub keys: Vec<u16>,
}

impl Effect {
    /// Новый слой с разумными параметрами по умолчанию.
    pub fn new(kind: EffectType, keys: Vec<u16>) -> Self {
        let mut e = Self {
            kind,
            speed: 0,
            direction: Direction::None,
            rotation: Rotation::None,
            colors: Vec::new(),
            keys: if kind.is_all_zones() { Vec::new() } else { keys },
        };
        e.normalize();
        e
    }

    /// Приводит параметры в соответствие типу эффекта (после смены типа).
    pub fn normalize(&mut self) {
        let k = self.kind;
        if k.has_speed() {
            self.speed = self.speed.clamp(1, 3);
        } else {
            self.speed = 0;
        }
        if k.has_direction() {
            if self.direction == Direction::None {
                self.direction = Direction::LeftToRight;
            }
        } else {
            self.direction = Direction::None;
        }
        if k.has_rotation() {
            if self.rotation == Rotation::None {
                self.rotation = Rotation::Clockwise;
            }
        } else {
            self.rotation = Rotation::None;
        }
        match k.color_kind() {
            ColorKind::None => self.colors.clear(),
            ColorKind::Single => {
                self.colors.truncate(1);
                if self.colors.is_empty() {
                    self.colors.push(Rgb::new(0x15, 0x8D, 0xDD));
                }
            }
            ColorKind::Multi => {}
        }
        if k.is_all_zones() {
            self.keys.clear();
        }
    }

    /// Режим цвета, как его ждёт прошивка: 0 — нет, 1 — случайные, 2 — список.
    fn color_mode(&self) -> u8 {
        match self.kind.color_kind() {
            ColorKind::None => 0,
            ColorKind::Single => 2,
            ColorKind::Multi if self.colors.is_empty() => 1,
            ColorKind::Multi => 2,
        }
    }

    fn encoded_len(&self) -> usize {
        let keys = if self.kind.is_all_zones() { 1 } else { self.keys.len() };
        1 + 13 + 1 + 3 * self.colors.len() + 1 + 2 * keys
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// Описание не помещается в пакет.
    TooLarge(usize),
    /// В слое больше 255 зон или цветов.
    TooManyItems,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::TooLarge(n) => {
                write!(f, "описание профиля занимает {n} байт из {REPORT_LEN} — уменьшите число слоёв")
            }
            EncodeError::TooManyItems => write!(f, "в слое больше 255 зон или цветов"),
        }
    }
}

/// Что останется от каждого слоя после записи: зоны, которые он реально
/// красит (`None` — слой отброшен целиком). Каждая зона достаётся верхнему
/// (последнему) слою; слой «на все зоны» отменяет остальные; слой, реагирующий
/// на нажатия, нельзя перекрыть частично — он отбрасывается.
pub fn survivors(effects: &[Effect]) -> Vec<Option<Vec<u16>>> {
    let mut out = vec![None; effects.len()];
    if let Some(top) = effects.iter().rposition(|e| e.kind.is_all_zones()) {
        out[top] = Some(Vec::new());
        return out;
    }
    let mut used = std::collections::HashSet::new();
    for (i, e) in effects.iter().enumerate().rev() {
        if e.kind.is_whole_keyboard() && e.keys.iter().any(|k| used.contains(k)) {
            continue;
        }
        let keys: Vec<u16> = e.keys.iter().copied().filter(|k| used.insert(*k)).collect();
        if !keys.is_empty() {
            out[i] = Some(keys);
        }
    }
    out
}

/// Убирает перекрытия слоёв так же, как Vantage (см. [`survivors`]).
pub fn compress(effects: &[Effect]) -> Vec<Effect> {
    effects
        .iter()
        .zip(survivors(effects))
        .filter_map(|(e, keys)| keys.map(|keys| Effect { keys, ..e.clone() }))
        .collect()
}

/// Пакет `0xCB`: записать слои в профиль `profile` (1…6).
pub fn encode_effects(profile: u8, effects: &[Effect]) -> Result<Packet, EncodeError> {
    let total = 7 + effects.iter().map(Effect::encoded_len).sum::<usize>();
    if total > REPORT_LEN {
        return Err(EncodeError::TooLarge(total));
    }
    let mut b = [0u8; REPORT_LEN];
    let mut p = 0;
    let mut put = |b: &mut Packet, v: u8| {
        b[p] = v;
        p += 1;
    };
    for v in [REPORT_ID, op::SET_EFFECTS, 0xC0, 0x03, profile, 0x01, 0x01] {
        put(&mut b, v);
    }
    for (i, e) in effects.iter().enumerate() {
        let keys: Vec<u16> = if e.kind.is_all_zones() { vec![ALL_ZONES] } else { e.keys.clone() };
        if keys.len() > 255 || e.colors.len() > 255 {
            return Err(EncodeError::TooManyItems);
        }
        put(&mut b, i as u8 + 1);
        let header = [
            0x06,
            0x01,
            e.kind as u8,
            0x02,
            e.speed,
            0x03,
            e.rotation as u8,
            0x04,
            e.direction as u8,
            0x05,
            e.color_mode(),
            0x06,
            0x00,
        ];
        for v in header {
            put(&mut b, v);
        }
        put(&mut b, e.colors.len() as u8);
        for c in &e.colors {
            put(&mut b, c.r);
            put(&mut b, c.g);
            put(&mut b, c.b);
        }
        put(&mut b, keys.len() as u8);
        for k in keys {
            let [lo, hi] = k.to_le_bytes();
            put(&mut b, lo);
            put(&mut b, hi);
        }
    }
    // Как у остальных запросов и у заводских профилей — константа 0xC0
    // (Legion Toolkit пишет сюда длину по модулю 255, контроллеру всё равно,
    // но потом он возвращает её при чтении).
    b[2] = 0xC0;
    Ok(b)
}

/// Ответ на `0xCC`: номер профиля и его слои.
pub fn decode_effects(buf: &[u8]) -> Option<(u8, Vec<Effect>)> {
    let mut r = Reader { buf, pos: 0 };
    if r.u8()? != REPORT_ID || r.u8()? != op::GET_EFFECTS {
        return None;
    }
    r.skip(2)?;
    let profile = r.u8()?;
    r.skip(2)?;
    let mut effects = Vec::new();
    let mut last_no = 1;
    loop {
        let Some(no) = r.u8() else { break };
        if no < last_no {
            break;
        }
        last_no = no;
        let h = r.take(13)?;
        let Some(kind) = EffectType::from_u8(h[2]) else { break };
        let n_colors = r.u8()? as usize;
        let mut colors = Vec::with_capacity(n_colors);
        for _ in 0..n_colors {
            let c = r.take(3)?;
            colors.push(Rgb::new(c[0], c[1], c[2]));
        }
        let n_keys = r.u8()? as usize;
        let mut keys = Vec::with_capacity(n_keys);
        for _ in 0..n_keys {
            let k = r.take(2)?;
            keys.push(u16::from_le_bytes([k[0], k[1]]));
        }
        if kind.is_all_zones() || keys == [ALL_ZONES] {
            keys.clear();
        }
        effects.push(Effect {
            kind,
            speed: h[4],
            rotation: Rotation::from_u8(h[6]),
            direction: Direction::from_u8(h[8]),
            colors,
            keys,
        });
    }
    Some((profile, effects))
}

/// Текущие цвета всех зон из ответа `HIDIOCGFEATURE` без запроса.
pub fn parse_state(buf: &[u8]) -> Vec<(u16, Rgb)> {
    buf.get(4..)
        .unwrap_or_default()
        .chunks_exact(5)
        .filter_map(|c| {
            let code = u16::from_le_bytes([c[0], c[1]]);
            (code != 0).then(|| (code, Rgb::new(c[2], c[3], c[4])))
        })
        .collect()
}

/// Ответ на `0xC4`: (строк, зон в строке).
pub fn parse_key_count(buf: &[u8]) -> Option<(usize, usize)> {
    (buf.get(1) == Some(&op::KEY_COUNT)).then(|| (buf[5] as usize, buf[6] as usize))
}

/// Ответ на `0xC5`: коды зон строки матрицы.
pub fn parse_key_page(buf: &[u8], width: usize) -> Vec<u16> {
    (0..width)
        .map(|x| {
            let i = 6 + 3 * x + 1;
            buf.get(i..i + 2).map_or(0, |k| u16::from_le_bytes([k[0], k[1]]))
        })
        .collect()
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.buf.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.buf.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(s)
    }

    fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Начало ответа `0xCC` живой клавиатуры (профиль 2: «Always», #158DDD, 130 клавиш).
    fn live_profile() -> Vec<u8> {
        let mut b = vec![
            0x07, 0xcc, 0xc0, 0x03, 0x02, 0x01, 0x01, 0x01, 0x06, 0x01, 0x0b, 0x02, 0x00, 0x03, 0x00, 0x04,
            0x00, 0x05, 0x02, 0x06, 0x00, 0x01, 0x15, 0x8d, 0xdd, 0x82,
        ];
        for k in 1..=130u16 {
            b.extend_from_slice(&k.to_le_bytes());
        }
        b.resize(REPORT_LEN, 0);
        b
    }

    #[test]
    fn decodes_live_profile() {
        let (profile, effects) = decode_effects(&live_profile()).unwrap();
        assert_eq!(profile, 2);
        assert_eq!(effects.len(), 1);
        let e = &effects[0];
        assert_eq!(e.kind, EffectType::Always);
        assert_eq!(e.colors, vec![Rgb::new(0x15, 0x8d, 0xdd)]);
        assert_eq!(e.keys.len(), 130);
        assert_eq!(e.speed, 0);
    }

    #[test]
    fn encode_roundtrip() {
        let effects = vec![
            Effect::new(EffectType::RainbowWave, vec![1, 2, 3]),
            Effect { colors: vec![Rgb::new(255, 0, 0)], ..Effect::new(EffectType::Always, vec![0x42, 0x6d]) },
        ];
        let pkt = encode_effects(3, &effects).unwrap();
        assert_eq!(&pkt[..7], &[0x07, 0xCB, 0xC0, 0x03, 3, 1, 1]);
        // Заголовок слоя как у Vantage.
        assert_eq!(&pkt[7..21], &[1, 6, 1, 2, 2, 1, 3, 0, 4, 4, 5, 0, 6, 0]);
        let mut resp = pkt;
        resp[1] = op::GET_EFFECTS;
        let (profile, back) = decode_effects(&resp).unwrap();
        assert_eq!(profile, 3);
        assert_eq!(back, effects);
    }

    #[test]
    fn live_profile_reencodes_identically() {
        let src = live_profile();
        let (p, eff) = decode_effects(&src).unwrap();
        let pkt = encode_effects(p, &eff).unwrap();
        // Всё, кроме кода операции, совпадает байт в байт.
        assert_eq!(&pkt[2..], &src[2..]);
    }

    #[test]
    fn all_zone_effect_uses_marker() {
        let pkt = encode_effects(1, &[Effect::new(EffectType::AudioBounce, vec![1, 2])]).unwrap();
        // нет цветов, одна «зона» 0x0065
        assert_eq!(&pkt[21..25], &[0, 1, 0x65, 0x00]);
        let mut resp = pkt;
        resp[1] = op::GET_EFFECTS;
        assert!(decode_effects(&resp).unwrap().1[0].keys.is_empty());
    }

    #[test]
    fn compress_later_layer_wins() {
        let base = Effect::new(EffectType::RainbowWave, vec![1, 2, 3, 4]);
        let top = Effect::new(EffectType::Always, vec![3, 4, 5]);
        let out = compress(&[base, top.clone()]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].keys, vec![1, 2]);
        assert_eq!(out[1], top);
        // слой, полностью перекрытый сверху, исчезает
        let hidden = Effect::new(EffectType::Rain, vec![5]);
        assert_eq!(compress(&[hidden, top.clone()]), vec![top]);
    }

    #[test]
    fn survivors_report_hidden_layers() {
        let all: Vec<u16> = (1..=10).collect();
        let a = Effect::new(EffectType::RainbowWave, all.clone());
        let b = Effect::new(EffectType::Always, all.clone());
        let c = Effect::new(EffectType::Always, vec![3]);
        // второй слой на те же зоны целиком прячет первый
        assert_eq!(survivors(&[a.clone(), b.clone()]), vec![None, Some(all.clone())]);
        // третий откусывает зону у второго
        let s = survivors(&[a, b, c]);
        assert_eq!(s[0], None);
        assert_eq!(s[1].as_ref().unwrap().len(), 9);
        assert_eq!(s[2], Some(vec![3]));
        // «рябь» нельзя перекрыть частично
        let r = Effect::new(EffectType::Ripple, all);
        assert_eq!(survivors(&[r, Effect::new(EffectType::Always, vec![1])])[0], None);
    }

    #[test]
    fn compress_all_zone_effect_wins() {
        let a = Effect::new(EffectType::Always, vec![1]);
        let b = Effect::new(EffectType::AudioRipple, vec![]);
        assert_eq!(compress(&[b.clone(), a]), vec![b]);
    }

    #[test]
    fn too_large_is_rejected() {
        let keys: Vec<u16> = (1..=200).collect();
        let layers: Vec<Effect> = (0..4).map(|_| Effect::new(EffectType::Rain, keys.clone())).collect();
        assert!(matches!(encode_effects(1, &layers), Err(EncodeError::TooLarge(_))));
    }

    #[test]
    fn parses_state_and_key_pages() {
        let mut st = vec![0x07, 0x03, 0x8e, 0x02, 0xe9, 0x03, 0x07, 0x2f, 0x49, 0xf3, 0x03, 0x07, 0x2f, 0x49];
        st.resize(REPORT_LEN, 0);
        assert_eq!(parse_state(&st), vec![(0x3e9, Rgb::new(7, 0x2f, 0x49)), (0x3f3, Rgb::new(7, 0x2f, 0x49))]);

        let kc = [0x07, 0xc4, 0x03, 0x00, 0x07, 0x09, 0x16];
        assert_eq!(parse_key_count(&kc), Some((9, 22)));

        let mut page = vec![0x07, 0xc5, 0, 0, 7, 1, 0, 0, 0, 1, 1, 0, 2, 2, 0];
        page.resize(REPORT_LEN, 0);
        assert_eq!(&parse_key_page(&page, 4), &[0, 1, 2, 0]);
    }

    #[test]
    fn normalize_fills_defaults() {
        let mut e = Effect::new(EffectType::Always, vec![1]);
        assert_eq!(e.colors.len(), 1);
        e.kind = EffectType::RainbowScrew;
        e.normalize();
        assert!(e.colors.is_empty());
        assert_eq!(e.rotation, Rotation::Clockwise);
        assert_eq!(e.speed, 1);
    }
}

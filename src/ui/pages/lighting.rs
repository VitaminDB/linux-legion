//! Подсветка Spectrum: профили, яркость, живая схема клавиатуры и корпуса,
//! редактор слоёв эффектов.

use crate::spectrum::layout::{self, Group, Preset, Zone};
use crate::spectrum::protocol::{self as proto, ColorKind, Direction, Effect, EffectType, Rgb, Rotation};
use crate::ui::app::AppCtx;
use crate::ui::icons;
use crate::ui::widgets::{access_banner, card, card_with, nothing, page_header, reactive, reactive_box};
use crate::worker::{Job, RgbLink};
use std::collections::HashMap;
use syngui::prelude::*;
use syngui::widgets::buttons::Segment;
use syngui::widgets::*;
use syngui::containers::Positioned;
use syngui::{CursorIcon, StyleValue};

/// Пикселей на «клавишу».
const U: f32 = 40.0;
/// Зазор между клавишами.
const GAP: f32 = 4.0;

const PALETTE: [Rgb; 10] = [
    Rgb::new(255, 255, 255),
    Rgb::new(255, 0, 0),
    Rgb::new(255, 96, 0),
    Rgb::new(255, 210, 0),
    Rgb::new(0, 255, 64),
    Rgb::new(0, 220, 255),
    Rgb::new(21, 141, 221),
    Rgb::new(0, 48, 255),
    Rgb::new(150, 0, 255),
    Rgb::new(255, 0, 170),
];

fn fx_icon(t: EffectType) -> &'static str {
    use EffectType::*;
    match t {
        Always => icons::FX_STATIC,
        RainbowWave => icons::FX_WAVE,
        RainbowScrew => icons::FX_SCREW,
        ColorWave => icons::FX_GRADIENT,
        ColorChange => icons::FX_CHANGE,
        ColorPulse => icons::FX_PULSE,
        Smooth => icons::FX_SMOOTH,
        Rain => icons::FX_RAIN,
        Ripple => icons::FX_RIPPLE,
        Type => icons::FX_TYPE,
        AudioBounce => icons::FX_AUDIO,
        AudioRipple => icons::FX_AUDIO_RIPPLE,
        AuroraSync => icons::AUTO,
    }
}

fn color(c: Rgb) -> Color {
    Color::from_srgb(c.r, c.g, c.b, 1.0)
}

pub fn view(ctx: AppCtx) -> impl Widget {
    let c = ctx.clone();
    Column::new()
        .gap(18.0)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .child(page_header(
            "Подсветка",
            "Legion Spectrum: клавиши, задние вентиляционные отверстия, боковины, передняя полоса и логотип.",
        ))
        .child(reactive_box(move || {
            let st = c.sink.rgb.get();
            match st.link {
                RgbLink::Ready => Box::new(editor(c.clone())),
                RgbLink::Searching => Box::new(message(icons::SEARCH, "Поиск контроллера подсветки…", "")),
                RgbLink::NotFound => Box::new(message(
                    icons::KEYBOARD,
                    "Контроллер Spectrum не найден",
                    "На этой модели нет RGB-подсветки Spectrum или её контроллер пока не поддерживается.",
                )),
                RgbLink::NoAccess(_) => Box::new(access_banner()),
                RgbLink::Error(e) => Box::new(message(icons::ERROR, "Ошибка связи с контроллером", &e)),
            }
        }))
        .class("page")
}

fn message(icon: &str, title: &str, hint: &str) -> impl Widget {
    DecoratedBox::new()
        .child(
            Column::new()
                .gap(12.0)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .child(DecoratedBox::new().child(Center::new().child(Icon::new(icon).class("ph-icon"))).class("ph-badge"))
                .child(Text::new(title).class("ph-title"))
                .child(Text::new(hint).class("ph-hint")),
        )
        .class("ph-card")
}

fn editor(ctx: AppCtx) -> impl Widget {
    Column::new()
        .gap(18.0)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .child(
            Row::new()
                .gap(18.0)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .child(DecoratedBox::new().child(card("Профиль", "Fn + Пробел переключает профили с клавиатуры", profiles(ctx.clone()))).class("grow"))
                .child(DecoratedBox::new().child(card("Яркость и логотип", "Fn + Пробел долгим нажатием — яркость", brightness(ctx.clone()))).class("grow")),
        )
        .child(scheme_card(ctx.clone()))
        .child(
            Row::new()
                .gap(18.0)
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .child(DecoratedBox::new().child(layers_card(ctx.clone())).class("layers-col"))
                .child(DecoratedBox::new().child(layer_editor(ctx.clone())).class("grow")),
        )
        .child(save_bar(ctx))
}

// ---------------------------------------------------------------------------
// профиль, яркость

fn profiles(ctx: AppCtx) -> impl Widget {
    reactive(move || {
        let cur = ctx.sink.rgb.get().profile;
        let mut row = Row::new().gap(8.0);
        for n in proto::PROFILES {
            let c = ctx.clone();
            row = row.child(
                Button::new(n.to_string())
                    .on_click(move || c.send(Job::RgbProfile(n)))
                    .class(if n == cur { "profile-btn profile-btn-on" } else { "profile-btn" }),
            );
        }
        row
    })
}

fn brightness(ctx: AppCtx) -> impl Widget {
    reactive(move || {
        let st = ctx.sink.rgb.get();
        let (c_b, c_l) = (ctx.clone(), ctx.clone());
        let cur = st.brightness;
        let has_logo = st.matrix.extra.contains(&layout::LOGO);
        let mut col = Column::new().gap(14.0).cross_axis_alignment(CrossAxisAlignment::Stretch).child(
            Row::new()
                .gap(12.0)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .child(Icon::new(icons::BRIGHTNESS).class("fan-icon"))
                .child(
                    Slider::new()
                        .value(cur as f32)
                        .range(0.0, proto::MAX_BRIGHTNESS as f32)
                        .step(1.0)
                        .on_change(move |v| {
                            let v = v.round() as u8;
                            if v != c_b.sink.rgb.get_untracked().brightness {
                                c_b.send(Job::RgbBrightness(v));
                            }
                        })
                        .class("wide-slider grow"),
                )
                .child(Text::new(if cur == 0 { "выкл.".into() } else { format!("{cur} / {}", proto::MAX_BRIGHTNESS) }).class("field-value")),
        );
        if has_logo {
            col = col.child(
                Row::new()
                    .gap(12.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .child(Text::new("Логотип LEGION на крышке").class("row-title grow"))
                    .child(Toggle::new().on(st.logo).on_change(move |v| c_l.send(Job::RgbLogo(v)))),
            );
        }
        col
    })
}

// ---------------------------------------------------------------------------
// схема

fn scheme_card(ctx: AppCtx) -> impl Widget {
    let c_head = ctx.clone();
    card_with(
        icons::KEYBOARD,
        "Схема подсветки",
        move || {
            let n = c_head.rgb_sel.get().len();
            Text::new(if n == 0 { "щёлкните по зонам, чтобы выделить".to_string() } else { format!("выделено зон: {n}") })
                .class("card-hint")
        },
        reactive(move || {
            let st = ctx.sink.rgb.get();
            let zones = layout::build(&st.matrix);
            Column::new()
                .gap(14.0)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .child(presets(ctx.clone(), zones.clone()))
                .child(Center::new().child(keyboard(ctx.clone(), zones)))
        }),
    )
}

fn presets(ctx: AppCtx, zones: Vec<Zone>) -> impl Widget {
    let mut row = Row::new().gap(8.0).cross_axis_alignment(CrossAxisAlignment::Center);
    for p in Preset::ALL {
        let (c, z) = (ctx.clone(), zones.clone());
        row = row.child(
            Button::new(p.label())
                .on_click(move || {
                    let mut keys = layout::preset(&z, p);
                    keys.sort_unstable();
                    // Повторный щелчок по тому же набору снимает выделение.
                    let same = c.rgb_sel.get_untracked() == keys;
                    c.rgb_sel.set(if same { Vec::new() } else { keys });
                })
                .class("chip"),
        );
    }
    let c = ctx.clone();
    row.child(DecoratedBox::new().class("grow")).child(
        Button::new("Снять выделение").icon(icons::DESELECT).on_click(move || c.rgb_sel.set(Vec::new())).class("chip"),
    )
}

fn keyboard(ctx: AppCtx, zones: Vec<Zone>) -> impl Widget {
    let (w, h) = layout::extent(&zones);
    let (wp, hp) = (w * U, h * U);
    let logo = zones.iter().find(|z| z.group == Group::Logo).cloned();
    let plain: Vec<Zone> = zones.into_iter().filter(|z| z.group != Group::Logo).collect();

    let frame_ctx = ctx.clone();
    let frame_zones = plain.clone();
    let colors = reactive(move || {
        let frame: HashMap<u16, Rgb> = frame_ctx.sink.rgb_frame.get().into_iter().collect();
        let bright = frame_ctx.sink.rgb.get().brightness;
        let rects: Vec<(f32, f32, f32, f32, Color)> = frame_zones
            .iter()
            .map(|z| {
                let c = frame.get(&z.code).map(|&c| boost(c, bright));
                let col = match c {
                    Some(c) if c != Rgb::default() => color(c),
                    _ => Color::from_srgb(0x1c, 0x20, 0x29, 1.0),
                };
                (z.x * U + GAP / 2.0, z.y * U + GAP / 2.0, z.w * U - GAP, z.h * U - GAP, col)
            })
            .collect();
        Canvas::new(move |c: &mut CanvasContext, _| {
            for &(x, y, w, h, col) in &rects {
                c.set_color(col);
                c.fill_rounded_rect(x, y, w, h, 5.0);
            }
        })
        .size(wp, hp)
    });

    let sel_ctx = ctx.clone();
    let overlay = reactive(move || {
        let sel = sel_ctx.rgb_sel.get();
        let mut stack = Stack::new().child(sized(wp, hp));
        for z in &plain {
            let on = sel.binary_search(&z.code).is_ok();
            let c = sel_ctx.clone();
            let code = z.code;
            let cls = match (on, z.group) {
                (true, _) => "zone zone-on",
                (false, Group::Keys | Group::Numpad) => "zone",
                (false, _) => "zone zone-case",
            };
            let cell = GestureDetector::new()
                .on_click(move || toggle_key(&c, code))
                .cursor(CursorIcon::Pointer)
                .child(
                    DecoratedBox::new()
                        .child(Center::new().child(Text::new(z.label).class(if z.w < 1.2 && z.label.len() > 3 { "zone-label zone-label-sm" } else { "zone-label" })))
                        .class(cls)
                        .style("width", StyleValue::px(z.w * U - GAP))
                        .style("height", StyleValue::px(z.h * U - GAP)),
                );
            stack = stack.child(
                Positioned::new(cell)
                    .at(z.x * U + GAP / 2.0, z.y * U + GAP / 2.0)
                    .dimensions(z.w * U - GAP, z.h * U - GAP),
            );
        }
        stack
    });

    let body = DecoratedBox::new()
        .child(Stack::new().child(sized(wp, hp)).child(colors).child(overlay))
        .class("kb-body");

    let mut col = Column::new().gap(12.0).cross_axis_alignment(CrossAxisAlignment::Center);
    if let Some(l) = logo {
        col = col.child(logo_chip(ctx, l.code));
    }
    col.child(body).child(
        Row::new()
            .gap(18.0)
            .child(legend("zone-case-sample", "зоны корпуса"))
            .child(legend("zone-on-sample", "выделено"))
            .child(Text::new("Цвета — живой кадр с контроллера").class("scale-label")),
    )
}

fn legend(cls: &str, text: &str) -> impl Widget {
    Row::new()
        .gap(6.0)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .child(DecoratedBox::new().class(cls))
        .child(Text::new(text).class("scale-label"))
}

/// Логотип на крышке — отдельная зона над схемой.
fn logo_chip(ctx: AppCtx, code: u16) -> impl Widget {
    let (c_col, c_sel) = (ctx.clone(), ctx);
    GestureDetector::new()
        .on_click(move || toggle_key(&c_sel, code))
        .cursor(CursorIcon::Pointer)
        .child(reactive(move || {
            let on = c_col.rgb_sel.get().contains(&code);
            let bright = c_col.sink.rgb.get().brightness;
            let col = c_col
                .sink
                .rgb_frame
                .get()
                .into_iter()
                .find(|(k, _)| *k == code)
                .map(|(_, c)| boost(c, bright))
                .unwrap_or_default();
            DecoratedBox::new()
                .child(
                    Row::new()
                        .gap(10.0)
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .child(DecoratedBox::new().class("logo-dot").style("background-color", color(col)))
                        .child(Text::new("LEGION · логотип на крышке").class("logo-text")),
                )
                .class(if on { "logo-chip logo-chip-on" } else { "logo-chip" })
        }))
}

fn sized(w: f32, h: f32) -> impl Widget {
    DecoratedBox::new().style("width", StyleValue::px(w)).style("height", StyleValue::px(h))
}

fn toggle_key(ctx: &AppCtx, code: u16) {
    let mut sel = ctx.rgb_sel.get_untracked();
    match sel.binary_search(&code) {
        Ok(i) => {
            sel.remove(i);
        }
        Err(i) => sel.insert(i, code),
    }
    ctx.rgb_sel.set(sel);
}

/// Контроллер отдаёт цвета, уже умноженные на яркость, — возвращаем
/// исходную насыщенность для наглядности.
fn boost(c: Rgb, brightness: u8) -> Rgb {
    if brightness == 0 {
        return c;
    }
    let k = proto::MAX_BRIGHTNESS as f32 / brightness as f32;
    let f = |v: u8| (v as f32 * k).min(255.0) as u8;
    Rgb::new(f(c.r), f(c.g), f(c.b))
}

// ---------------------------------------------------------------------------
// слои

fn layer_subtitle(e: &Effect) -> String {
    let zones = if e.kind.is_all_zones() { "все зоны".to_string() } else { format!("зон: {}", e.keys.len()) };
    if e.kind.has_speed() {
        format!("{zones} · скорость {}", e.speed)
    } else {
        zones
    }
}

fn layers_card(ctx: AppCtx) -> impl Widget {
    let c_head = ctx.clone();
    card_with(
        icons::LAYERS,
        "Слои эффектов",
        move || {
            let p = c_head.sink.rgb.get().profile;
            Text::new(format!("профиль {p}")).class("card-hint")
        },
        reactive(move || {
            let effects = ctx.rgb_edit.get();
            let cur = ctx.rgb_layer.get();
            let sel_empty = ctx.rgb_sel.get().is_empty();
            let n = effects.len();
            let mut col = Column::new().gap(8.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
            if effects.is_empty() {
                col = col.child(Text::new("В профиле нет эффектов — подсветка погашена.").class("muted"));
            }
            // Верхний (последний) слой — первым в списке.
            for i in (0..n).rev() {
                let e = &effects[i];
                let on = cur == Some(i);
                let (c_sel, c_up, c_dn, c_del) = (ctx.clone(), ctx.clone(), ctx.clone(), ctx.clone());
                let mut dots = Row::new().gap(4.0);
                for c in e.colors.iter().take(6) {
                    dots = dots.child(DecoratedBox::new().class("mini-dot").style("background-color", color(*c)));
                }
                if e.colors.is_empty() && e.kind.color_kind() != ColorKind::Single {
                    dots = dots.child(DecoratedBox::new().class("mini-dot mini-rainbow"));
                }
                col = col.child(
                    GestureDetector::new()
                        .on_click(move || c_sel.rgb_layer.set(Some(i)))
                        .cursor(CursorIcon::Pointer)
                        .child(
                            DecoratedBox::new()
                                .child(
                                    Row::new()
                                        .gap(10.0)
                                        .cross_axis_alignment(CrossAxisAlignment::Center)
                                        .child(Icon::new(fx_icon(e.kind)).class("layer-icon"))
                                        .child(
                                            Column::new()
                                                .gap(3.0)
                                                .child(Text::new(format!("{}. {}", i + 1, e.kind.label())).class("row-title"))
                                                .child(Text::new(layer_subtitle(e)).class("row-desc"))
                                                .class("grow"),
                                        )
                                        .child(dots)
                                        .child(
                                            ToolButton::new(icons::UP)
                                                .tooltip("Выше")
                                                .disabled(i + 1 >= n)
                                                .on_click(move || {
                                                    c_up.edit_rgb(|v| v.swap(i, i + 1));
                                                    c_up.rgb_layer.set(Some(i + 1));
                                                }),
                                        )
                                        .child(
                                            ToolButton::new(icons::DOWN)
                                                .tooltip("Ниже")
                                                .disabled(i == 0)
                                                .on_click(move || {
                                                    c_dn.edit_rgb(|v| v.swap(i, i - 1));
                                                    c_dn.rgb_layer.set(Some(i - 1));
                                                }),
                                        )
                                        .child(ToolButton::new(icons::DELETE).tooltip("Удалить слой").on_click(move || {
                                            c_del.edit_rgb(|v| {
                                                v.remove(i);
                                            });
                                            let left = c_del.rgb_edit.get_untracked().len();
                                            c_del.rgb_layer.set(left.checked_sub(1).map(|m| i.min(m)));
                                        })),
                                )
                                .class(if on { "layer-row layer-row-on" } else { "layer-row" }),
                        ),
                );
            }
            let (c_add, c_all) = (ctx.clone(), ctx.clone());
            col.child(
                Row::new()
                    .gap(8.0)
                    .child(
                        Button::new("Новый слой")
                            .icon(icons::ADD)
                            .on_click(move || {
                                let mut keys = c_add.rgb_sel.get_untracked();
                                if keys.is_empty() {
                                    let zones = layout::build(&c_add.sink.rgb.get_untracked().matrix);
                                    keys = layout::preset(&zones, Preset::All);
                                }
                                c_add.edit_rgb(|v| v.push(Effect::new(EffectType::Always, keys)));
                                let n = c_add.rgb_edit.get_untracked().len();
                                c_add.rgb_layer.set(Some(n - 1));
                            })
                            .class("btn-primary grow"),
                    )
                    .child(
                        Button::new("Эффект на всё")
                            .icon(icons::SELECT_ALL)
                            .on_click(move || {
                                let zones = layout::build(&c_all.sink.rgb.get_untracked().matrix);
                                let keys = layout::preset(&zones, Preset::All);
                                c_all.rgb_edit.set(vec![Effect::new(EffectType::RainbowWave, keys)]);
                                c_all.rgb_layer.set(Some(0));
                            })
                            .class("btn-ghost"),
                    ),
            )
            .child(
                Text::new(if sel_empty {
                    "Новый слой ляжет на все зоны. Чтобы покрасить часть клавиш, сначала выделите их на схеме."
                } else {
                    "Новый слой ляжет на выделенные зоны."
                })
                .class("row-desc"),
            )
        }),
    )
}

fn layer_editor(ctx: AppCtx) -> impl Widget {
    reactive_box(move || {
        let effects = ctx.rgb_edit.get();
        let Some(i) = ctx.rgb_layer.get().filter(|&i| i < effects.len()) else {
            return Box::new(card(
                "Параметры слоя",
                "",
                Text::new("Выберите слой слева или нажмите «Новый слой» — он ляжет на выделенные зоны или, если ничего не выделено, на все.")
                    .class("muted"),
            ));
        };
        let e = effects[i].clone();
        let mut col = Column::new().gap(20.0).cross_axis_alignment(CrossAxisAlignment::Stretch);
        col = col.child(effect_grid(ctx.clone(), i, e.kind));
        if e.kind.has_speed() {
            let c = ctx.clone();
            col = col.child(field(
                "Скорость",
                SegmentedButton::new(vec![Segment::new("Медленно"), Segment::new("Средне"), Segment::new("Быстро")])
                    .selected(e.speed.clamp(1, 3) as usize - 1)
                    .on_change(move |s| set_layer(&c, i, |e| e.speed = s as u8 + 1))
                    .class("seg"),
            ));
        }
        if e.kind.has_direction() {
            let c = ctx.clone();
            col = col.child(field(
                "Направление",
                SegmentedButton::new(Direction::ALL.iter().map(|d| Segment::new(d.label())).collect())
                    .selected(Direction::ALL.iter().position(|d| *d == e.direction).unwrap_or(0))
                    .on_change(move |s| set_layer(&c, i, |e| e.direction = Direction::ALL[s]))
                    .class("seg"),
            ));
        }
        if e.kind.has_rotation() {
            let c = ctx.clone();
            col = col.child(field(
                "Вращение",
                SegmentedButton::new(vec![Segment::new("По часовой"), Segment::new("Против часовой")])
                    .selected(if e.rotation == Rotation::CounterClockwise { 1 } else { 0 })
                    .on_change(move |s| {
                        set_layer(&c, i, |e| e.rotation = if s == 1 { Rotation::CounterClockwise } else { Rotation::Clockwise })
                    })
                    .class("seg"),
            ));
        }
        match e.kind.color_kind() {
            ColorKind::Single => col = col.child(single_color(ctx.clone(), i, e.colors.first().copied().unwrap_or_default())),
            ColorKind::Multi => col = col.child(multi_color(ctx.clone(), i, e.colors.clone())),
            ColorKind::None => {}
        }
        col = col.child(zones_field(ctx.clone(), i, &e));
        Box::new(card("Параметры слоя", "", col))
    })
}

fn field(label: &str, control: impl Widget + 'static) -> impl Widget {
    Column::new()
        .gap(8.0)
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .child(Text::new(label).class("field-label"))
        .child(control)
}

fn set_layer(ctx: &AppCtx, i: usize, f: impl FnOnce(&mut Effect)) {
    ctx.edit_rgb(|v| {
        if let Some(e) = v.get_mut(i) {
            f(e);
            e.normalize();
        }
    });
}

fn effect_grid(ctx: AppCtx, i: usize, cur: EffectType) -> impl Widget {
    let mut grid = Grid::new(4).gap(8.0);
    for t in EffectType::EDITABLE {
        let c = ctx.clone();
        grid = grid.child(
            GestureDetector::new()
                .on_click(move || {
                    let all = layout::preset(&layout::build(&c.sink.rgb.get_untracked().matrix), Preset::All);
                    set_layer(&c, i, |e| {
                        // Эффект «на все зоны» теряет список — при возврате восстановим все.
                        if e.keys.is_empty() && !t.is_all_zones() {
                            e.keys = all;
                        }
                        e.kind = t;
                    })
                })
                .cursor(CursorIcon::Pointer)
                .child(
                    DecoratedBox::new()
                        .child(
                            Column::new()
                                .gap(6.0)
                                .cross_axis_alignment(CrossAxisAlignment::Center)
                                .child(Icon::new(fx_icon(t)).class("fx-icon"))
                                .child(Text::new(t.label()).class("fx-label")),
                        )
                        .class(if t == cur { "fx-tile fx-tile-on" } else { "fx-tile" }),
                ),
        );
    }
    field("Эффект", grid)
}

fn swatch(c: Rgb, on: bool) -> impl Widget {
    DecoratedBox::new()
        .class(if on { "color-swatch color-swatch-on" } else { "color-swatch" })
        .style("background-color", color(c))
}

fn single_color(ctx: AppCtx, i: usize, cur: Rgb) -> impl Widget {
    let mut row = Row::new().gap(8.0).cross_axis_alignment(CrossAxisAlignment::Center);
    for p in PALETTE {
        let c = ctx.clone();
        row = row.child(
            GestureDetector::new()
                .on_click(move || set_layer(&c, i, |e| e.colors = vec![p]))
                .cursor(CursorIcon::Pointer)
                .child(swatch(p, p == cur)),
        );
    }
    let c = ctx.clone();
    row = row.child(
        ColorPicker::new()
            .color(ColorValue::new(cur.r, cur.g, cur.b))
            .width(150.0)
            .on_change(move |v| set_layer(&c, i, |e| e.colors = vec![Rgb::new(v.r, v.g, v.b)])),
    );
    field(&format!("Цвет · {}", cur.to_hex()), row)
}

fn multi_color(ctx: AppCtx, i: usize, colors: Vec<Rgb>) -> impl Widget {
    let mut chosen = Row::new().gap(8.0).cross_axis_alignment(CrossAxisAlignment::Center);
    if colors.is_empty() {
        chosen = chosen.child(Text::new("Случайные цвета").class("muted"));
    }
    for (k, c) in colors.iter().enumerate() {
        let cx = ctx.clone();
        chosen = chosen.child(
            Tooltip::new(
                GestureDetector::new()
                    .on_click(move || set_layer(&cx, i, |e| {
                        e.colors.remove(k);
                    }))
                    .cursor(CursorIcon::Pointer)
                    .child(swatch(*c, true)),
                "Убрать цвет",
            ),
        );
    }
    let mut palette = Row::new().gap(8.0).cross_axis_alignment(CrossAxisAlignment::Center);
    for p in PALETTE {
        let c = ctx.clone();
        palette = palette.child(
            GestureDetector::new()
                .on_click(move || set_layer(&c, i, |e| {
                    if e.colors.len() < 8 {
                        e.colors.push(p)
                    }
                }))
                .cursor(CursorIcon::Pointer)
                .child(swatch(p, false)),
        );
    }
    let c = ctx.clone();
    let last = colors.last().copied().unwrap_or(Rgb::new(255, 0, 0));
    palette = palette.child(
        ColorPicker::new().color(ColorValue::new(last.r, last.g, last.b)).width(150.0).on_change(move |v| {
            let rgb = Rgb::new(v.r, v.g, v.b);
            // Пипетка правит последний цвет, а если цветов нет — добавляет.
            set_layer(&c, i, |e| match e.colors.last_mut() {
                Some(l) => *l = rgb,
                None => e.colors.push(rgb),
            })
        }),
    );
    let c = ctx.clone();
    field(
        "Цвета — щелчок по цвету в палитре добавляет его, по выбранному — убирает",
        Column::new()
            .gap(10.0)
            .child(
                Row::new()
                    .gap(10.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .child(chosen)
                    .child(Button::new("Случайные").on_click(move || set_layer(&c, i, |e| e.colors.clear())).class("chip")),
            )
            .child(palette),
    )
}

fn zones_field(ctx: AppCtx, i: usize, e: &Effect) -> Box<dyn Widget> {
    if e.kind.is_all_zones() {
        return Box::new(field("Зоны", Text::new("Этот эффект всегда охватывает все зоны и отменяет остальные слои.").class("muted")));
    }
    let n = e.keys.len();
    let sel_n = ctx.rgb_sel.get().len();
    let keys = e.keys.clone();
    let (c_set, c_show) = (ctx.clone(), ctx);
    let whole = if e.kind.is_whole_keyboard() {
        "Реагирует на нажатия: если зоны слоя перекрыты сверху, слой будет пропущен."
    } else {
        ""
    };
    Box::new(field(
        &format!("Зоны · {n}"),
        Column::new()
            .gap(8.0)
            .child(
                Row::new()
                    .gap(8.0)
                    .child(
                        Button::new(format!("Назначить выделенные ({sel_n})"))
                            .icon(icons::EDIT)
                            .disabled(sel_n == 0)
                            .on_click(move || {
                                let sel = c_set.rgb_sel.get_untracked();
                                set_layer(&c_set, i, |e| e.keys = sel);
                            })
                            .class("btn-ghost"),
                    )
                    .child(
                        Button::new("Выделить зоны слоя")
                            .icon(icons::SELECT_ALL)
                            .on_click(move || {
                                let mut k = keys.clone();
                                k.sort_unstable();
                                k.dedup();
                                c_show.rgb_sel.set(k);
                            })
                            .class("btn-ghost"),
                    ),
            )
            .child(if whole.is_empty() { nothing() } else { Box::new(Text::new(whole).class("row-desc")) }),
    ))
}

// ---------------------------------------------------------------------------
// запись

fn save_bar(ctx: AppCtx) -> impl Widget {
    reactive(move || {
        let dirty = ctx.rgb_dirty();
        let busy = ctx.sink.busy.get();
        let profile = ctx.sink.rgb.get().profile;
        let effects = ctx.rgb_edit.get();
        let size_err = proto::encode_effects(profile, &proto::compress(&effects)).err();
        let (c_undo, c_reset, c_save) = (ctx.clone(), ctx.clone(), ctx.clone());
        let status: Box<dyn Widget> = match (&size_err, dirty) {
            (Some(e), _) => Box::new(
                Row::new()
                    .gap(8.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .child(Icon::new(icons::WARNING).class("status-icon tone-bad"))
                    .child(Text::new(e.to_string()).class("status-text")),
            ),
            (None, true) => Box::new(
                Row::new()
                    .gap(8.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .child(DecoratedBox::new().class("dot dot-warn"))
                    .child(Text::new(format!("Изменения ещё не записаны в профиль {profile}")).class("status-text")),
            ),
            (None, false) => Box::new(
                Row::new()
                    .gap(8.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .child(Icon::new(icons::CHECK).class("status-icon tone-ok"))
                    .child(Text::new(format!("Профиль {profile} хранится в памяти клавиатуры")).class("status-text")),
            ),
        };
        DecoratedBox::new()
            .child(
                Row::new()
                    .gap(10.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .children(vec![status])
                    .child(DecoratedBox::new().class("grow"))
                    .child(
                        Button::new("Заводской профиль")
                            .icon(icons::RESTART)
                            .disabled(busy)
                            .on_click(move || c_reset.send(Job::RgbReset(c_reset.sink.rgb.get_untracked().profile)))
                            .class("btn-ghost"),
                    )
                    .child(
                        Button::new("Отменить")
                            .icon(icons::UNDO)
                            .disabled(!dirty || busy)
                            .on_click(move || {
                                if let Some((_, e)) = c_undo.sink.rgb_effects.get_untracked() {
                                    c_undo.rgb_edit.set(e);
                                }
                            })
                            .class("btn-ghost"),
                    )
                    .child(
                        Button::new(format!("Записать в профиль {profile}"))
                            .icon(icons::SAVE)
                            .disabled(!dirty || busy || size_err.is_some())
                            .on_click(move || {
                                let p = c_save.sink.rgb.get_untracked().profile;
                                c_save.send(Job::RgbWrite(p, c_save.rgb_edit.get_untracked()));
                            })
                            .class("btn-primary"),
                    ),
            )
            .class("save-bar")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spectrum::device::KeyMatrix;
    use crate::spectrum::sim::{LOGO, MATRIX};
    use crate::worker::RgbState;
    use syngui::testing::*;

    const STYLES: &str = include_str!("../../../styles/app.mss");

    fn ctx() -> AppCtx {
        // Разрешает чтение сигналов в потоке теста.
        let _ = TestHarness::new(Box::new(DecoratedBox::new()));
        let ctx = AppCtx::new(true);
        let matrix = KeyMatrix {
            width: 22,
            height: 9,
            rows: MATRIX.iter().map(|r| r.to_vec()).collect(),
            extra: vec![LOGO],
        };
        ctx.sink.rgb.set(RgbState { link: RgbLink::Ready, profile: 3, brightness: 5, logo: true, matrix, ..Default::default() });
        ctx.sink.rgb_effects.set(Some((3, vec![])));
        ctx
    }

    fn harness(w: Box<dyn Widget>) -> TestHarness {
        let mut h = TestHarness::new(w);
        h.apply_mss(STYLES);
        for _ in 0..3 {
            h.rebuild();
            h.apply_mss_dirty(STYLES);
            h.layout(1300.0, 1400.0);
        }
        h
    }

    #[test]
    fn click_selects_zone() {
        let ctx = ctx();
        let zones: Vec<Zone> = layout::build(&ctx.sink.rgb.get_untracked().matrix)
            .into_iter()
            .filter(|z| z.group != Group::Logo)
            .collect();
        let w_idx = zones.iter().position(|z| z.code == 0x43).unwrap();
        let mut h = harness(Box::new(scheme_card(ctx.clone())));
        let cells = h.find_by_type_name("Positioned");
        assert_eq!(cells.len(), zones.len(), "каждая зона — Positioned");
        let b = h.element_bounds(cells[w_idx]);
        eprintln!("W bounds {b:?}");
        h.send_events(&click_at(Point::new(b.x() + b.width() / 2.0, b.y() + b.height() / 2.0)));
        assert_eq!(ctx.rgb_sel.get_untracked(), vec![0x43]);
    }

    #[test]
    fn new_layer_button() {
        let ctx = ctx();
        let mut h = harness(Box::new(layers_card(ctx.clone())));
        let btn = h.find_by_class("btn-primary")[0];
        let b = h.element_bounds(btn);
        let click = |h: &mut TestHarness| {
            h.send_events(&click_at(Point::new(b.x() + b.width() / 2.0, b.y() + b.height() / 2.0)))
        };
        // Без выделения — слой на все зоны.
        click(&mut h);
        let all = ctx.rgb_edit.get_untracked();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].keys.len(), 130, "все зоны вместе с логотипом");
        // С выделением — только на выделенные.
        ctx.rgb_sel.set(vec![0x43, 0x58]);
        h.rebuild();
        h.layout(1300.0, 1400.0);
        click(&mut h);
        let v = ctx.rgb_edit.get_untracked();
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].keys, vec![0x43, 0x58]);
        assert_eq!(ctx.rgb_layer.get_untracked(), Some(1));
    }
}

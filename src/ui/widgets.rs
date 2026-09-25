//! Общие элементы страниц.

use super::icons;
use syngui::prelude::*;
use syngui::widgets::*;
use syngui::StyleValue;

/// Заголовок страницы.
pub fn page_header(title: &str, subtitle: &str) -> impl Widget {
    Column::new()
        .gap(4.0)
        .child(Text::new(title).class("page-title"))
        .child(Text::new(subtitle).class("page-sub"))
}

/// Карточка-секция с заголовком.
pub fn card<M>(title: &str, hint: &str, body: impl IntoWidget<M>) -> impl Widget {
    let mut head = Column::new().gap(3.0).child(Text::new(title).class("card-title"));
    if !hint.is_empty() {
        head = head.child(Text::new(hint).class("card-hint"));
    }
    DecoratedBox::new()
        .child(
            Column::new()
                .gap(16.0)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .child(head)
                .child(body),
        )
        .class("card")
}

/// Карточка с иконкой в заголовке и произвольным правым краем.
pub fn card_with<M, N>(icon: &str, title: &str, trailing: impl IntoWidget<N>, body: impl IntoWidget<M>) -> impl Widget {
    DecoratedBox::new()
        .child(
            Column::new()
                .gap(16.0)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .child(
                    Row::new()
                        .gap(10.0)
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .child(Icon::new(icon).class("card-icon"))
                        .child(Text::new(title).class("card-title"))
                        .child(DecoratedBox::new().class("grow"))
                        .child(trailing),
                )
                .child(body),
        )
        .class("card")
}

/// Строка «название + пояснение … контрол».
pub fn setting_row(title: &str, desc: &str, control: Box<dyn Widget>) -> impl Widget {
    Row::new()
        .gap(16.0)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .child(
            Column::new()
                .gap(3.0)
                .child(Text::new(title).class("row-title"))
                .child(Text::new(desc).class("row-desc"))
                .class("grow"),
        )
        .children(vec![control])
}

/// Подпись над контролом со значением справа.
pub fn labeled(label: &str, value: String) -> impl Widget {
    Row::new()
        .gap(8.0)
        .child(Text::new(label).class("field-label"))
        .child(DecoratedBox::new().class("grow"))
        .child(Text::new(value).class("field-value"))
}

/// Пара «ключ — значение» для таблиц сведений.
pub fn kv(key: &str, value: impl Into<String>) -> impl Widget {
    Row::new()
        .gap(12.0)
        .child(Text::new(key).class("kv-key"))
        .child(DecoratedBox::new().class("grow"))
        .child(Text::new(value.into()).selectable(true).class("kv-value"))
        .class("kv-row")
}

/// Горизонтальная полоса заполнения 0…1.
pub fn bar(frac: f32, tone: &str) -> impl Widget {
    let frac = frac.clamp(0.0, 1.0);
    Row::new()
        .gap(0.0)
        .child(
            DecoratedBox::new()
                .class(&format!("bar-fill bar-{tone}"))
                .style("width", StyleValue::percent(frac * 100.0)),
        )
        .class("bar-track")
}

/// Плашка «нужны права» с командой установки udev-правила.
pub fn access_banner() -> impl Widget {
    DecoratedBox::new()
        .child(
            Row::new()
                .gap(14.0)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .child(Icon::new(icons::LOCK).class("banner-icon"))
                .child(
                    Column::new()
                        .gap(6.0)
                        .child(Text::new("Нет прав на изменение настроек").class("row-title"))
                        .child(
                            Text::new(
                                "Установите udev-правило из пакета — оно даёт группе wheel доступ к режимам \
                                 питания, вентиляторам и подсветке:",
                            )
                            .class("row-desc"),
                        )
                        .child(
                            DecoratedBox::new()
                                .child(
                                    Text::new(
                                        "sudo install -Dm644 packaging/70-linux-legion.rules /etc/udev/rules.d/\n\
                                         sudo udevadm control --reload-rules && sudo udevadm trigger",
                                    )
                                    .selectable(true)
                                    .class("code"),
                                )
                                .class("code-box"),
                        )
                        .class("grow"),
                ),
        )
        .class("banner")
}

/// Кольцевой индикатор: дуга 270° с подписью в центре.
pub fn ring(frac: f32, color: Color, size: f32, value: String, caption: &str) -> impl Widget {
    let frac = frac.clamp(0.0, 1.0);
    let canvas = Canvas::new(move |c: &mut CanvasContext, _| {
        let s = c.width().min(c.height());
        let stroke = (s * 0.075).max(6.0);
        let r = s / 2.0 - stroke;
        let (cx, cy) = (c.width() / 2.0, c.height() / 2.0);
        let a0 = std::f32::consts::PI * 0.75;
        let sweep = std::f32::consts::PI * 1.5;
        c.set_stroke_width(stroke);
        c.set_color(Color::from_srgb(0x26, 0x2b, 0x36, 1.0));
        c.draw_arc(cx, cy, r, a0, a0 + sweep);
        let cap = |c: &mut CanvasContext, a: f32| c.fill_circle(cx + r * a.cos(), cy + r * a.sin(), stroke / 2.0);
        cap(c, a0);
        cap(c, a0 + sweep);
        if frac > 0.0 {
            c.set_color(color);
            let a1 = a0 + sweep * frac;
            c.draw_arc(cx, cy, r, a0, a1);
            cap(c, a0);
            cap(c, a1);
        }
    })
    .size(size, size);
    DecoratedBox::new()
        .child(
            Stack::new()
                .fit(StackFit::Expand)
                .child(canvas)
                .child(Center::new().child(
                    Column::new()
                        .gap(0.0)
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .child(Text::new(value).class("ring-value"))
                        .child(Text::new(caption).class("ring-caption")),
                )),
        )
        .style("width", StyleValue::px(size))
        .style("height", StyleValue::px(size))
}

/// Цвет по температуре: холодно — голубой, горячо — красный.
pub fn temp_color(t: f32) -> Color {
    match t {
        t if t < 55.0 => Color::from_hex("#38bdf8"),
        t if t < 70.0 => Color::from_hex("#34d399"),
        t if t < 85.0 => Color::from_hex("#fbbf24"),
        _ => Color::from_hex("#f43f5e"),
    }
}

/// Цвет по загрузке.
pub fn load_color(frac: f32) -> Color {
    match frac {
        f if f < 0.5 => Color::from_hex("#60a5fa"),
        f if f < 0.8 => Color::from_hex("#a78bfa"),
        _ => Color::from_hex("#f472b6"),
    }
}

/// Реактивный участок: пересобирается при изменении прочитанных сигналов.
pub fn reactive<W: Widget + 'static>(f: impl Fn() -> W + Send + Sync + 'static) -> Reactive {
    Reactive::new(move || vec![Box::new(f()) as Box<dyn Widget>])
}

/// То же для ветвлений с разными типами виджетов.
pub fn reactive_box(f: impl Fn() -> Box<dyn Widget> + Send + Sync + 'static) -> Reactive {
    Reactive::new(move || vec![f()])
}

/// Пустышка.
pub fn nothing() -> Box<dyn Widget> {
    Box::new(DecoratedBox::new())
}

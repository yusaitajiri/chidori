//! The kit's building blocks in Iced: buttons (the primary with the kamon's inner line), switches,
//! checkboxes, fields, the dialog sheet in the frame, the rule. Every value comes from the tokens;
//! colours are read from the palette when drawn.

use std::borrow::Borrow;
use std::time::Instant;

use iced::border::Radius;
use iced::widget::{
    Button, Column, Container, Space, Svg, button, column, container, pick_list, row, svg, text_input, tooltip,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Theme, Vector};

use crate::icons::{Icon, icon};
use crate::theme::{self, alpha, line, p, plain, t, tokens};

pub type El<'a, M> = Element<'a, M>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// The one accent-filled action of a view, with the kamon's inner line.
    Primary,
    /// A destructive confirmation: the primary's shape in the brand's brick.
    Danger,
    Secondary,
    Ghost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Sm,
    Md,
    Lg,
}

impl Size {
    fn height(self) -> f32 {
        match self {
            Size::Sm => 28.0,
            Size::Md => 32.0,
            Size::Lg => 40.0,
        }
    }

    fn pad_x(self) -> f32 {
        match self {
            Size::Sm => 10.0,
            Size::Md => tokens::SPACE_3,
            Size::Lg => tokens::SPACE_4,
        }
    }
}

fn hovered(status: button::Status) -> bool {
    matches!(status, button::Status::Hovered | button::Status::Pressed)
}

/// A button's look by variant, for any button content.
pub fn button_style(variant: Variant) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let p = p();
        let disabled = status == button::Status::Disabled;
        let (bg, fg, border) = match variant {
            Variant::Primary => (if hovered(status) { p.accent_hover } else { p.accent }, p.on_accent, None),
            Variant::Danger => (p.danger, p.surface, None),
            Variant::Secondary => {
                (if hovered(status) { p.surface_sunken } else { p.surface_raised }, p.ink, Some(p.border_strong))
            }
            Variant::Ghost => (if hovered(status) { p.surface_sunken } else { Color::TRANSPARENT }, p.ink, None),
        };
        let fade = |c: Color| if disabled { alpha(c, c.a * 0.45) } else { c };
        button::Style {
            background: Some(Background::Color(fade(bg))),
            text_color: fade(fg),
            border: Border {
                color: fade(border.unwrap_or(Color::TRANSPARENT)),
                width: if border.is_some() { 1.0 } else { 0.0 },
                radius: tokens::RADIUS_MD.into(),
            },
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

/// The kamon's inner line: 1 px in the button's text colour, 3 px inside its edge.
fn inner_line<'a, M: 'a>(content: impl Into<El<'a, M>>, line_color: Color) -> Container<'a, M> {
    container(content).center_y(Length::Fill).style(move |_| container::Style {
        border: Border { color: line_color, width: 1.0, radius: (tokens::RADIUS_MD - 3.0).into() },
        ..container::Style::default()
    })
}

/// A labelled button. `on` is `None` for a disabled one.
pub fn btn<'a, M: Clone + 'a>(label: impl Into<String>, variant: Variant, size: Size, on: Option<M>) -> Button<'a, M> {
    btn_with(row![plain(label.into(), tokens::LABEL)], variant, size, on)
}

/// A button with an icon before its label.
pub fn btn_icon<'a, M: Clone + 'a>(
    which: Icon,
    label: impl Into<String>,
    variant: Variant,
    size: Size,
    on: Option<M>,
) -> Button<'a, M> {
    let fg = match variant {
        Variant::Primary => p().on_accent,
        Variant::Danger => p().surface,
        _ => p().ink,
    };
    btn_with(
        row![icon(which, 14.0, fg), plain(label.into(), tokens::LABEL)].spacing(6).align_y(Alignment::Center),
        variant,
        size,
        on,
    )
}

/// A button around any content; primary and danger get the inner line.
pub fn btn_with<'a, M: Clone + 'a>(
    content: iced::widget::Row<'a, M>,
    variant: Variant,
    size: Size,
    on: Option<M>,
) -> Button<'a, M> {
    let content = content.align_y(Alignment::Center);
    let lined = matches!(variant, Variant::Primary | Variant::Danger);
    let body: El<'a, M> = if lined {
        let line_color = if variant == Variant::Primary { p().on_accent } else { p().surface };
        inner_line(container(content).padding([0.0, size.pad_x() - 3.0]), line_color).into()
    } else {
        container(content).center_y(Length::Fill).padding([0.0, size.pad_x()]).into()
    };
    let b = button(body).height(size.height()).padding(if lined { 2 } else { 0 }).style(button_style(variant));
    match on {
        Some(m) => b.on_press(m),
        None => b,
    }
}

/// An icon-only button with its name as a tooltip (the React UI's `IconButton`).
pub fn icon_button<'a, M: Clone + 'a>(which: Icon, label: impl Into<String>, size: Size, on: Option<M>) -> El<'a, M> {
    icon_button_tinted(which, label, size, on, None)
}

/// An icon button whose icon has a colour of its own while `tint` is set (a pressed pin, ...).
pub fn icon_button_tinted<'a, M: Clone + 'a>(
    which: Icon,
    label: impl Into<String>,
    size: Size,
    on: Option<M>,
    tint: Option<Color>,
) -> El<'a, M> {
    let side = if size == Size::Sm { 28.0 } else { 32.0 };
    let glyph = if size == Size::Sm { 14.0 } else { 16.0 };
    let enabled = on.is_some();
    let b = button(container(icon_svg(which, glyph, tint)).center(side)).width(side).height(side).padding(0).style(
        move |_, status| {
            let p = p();
            button::Style {
                background: hovered(status).then_some(Background::Color(p.surface_sunken)),
                text_color: p.ink_muted,
                border: Border { radius: tokens::RADIUS_MD.into(), ..Border::default() },
                ..button::Style::default()
            }
            .with_opacity(if enabled { 1.0 } else { 0.4 })
        },
    );
    let b = match on {
        Some(m) => b.on_press(m),
        None => b,
    };
    with_tip(b, label.into())
}

/// An icon that follows its button: muted, ink while hovered (or the given tint).
fn icon_svg<'a>(which: Icon, size: f32, tint: Option<Color>) -> Svg<'a> {
    icon(which, size, Color::BLACK).style(move |_, status| svg::Style {
        color: Some(tint.unwrap_or(if status == svg::Status::Hovered { p().ink } else { p().ink_muted })),
    })
}

trait WithOpacity {
    fn with_opacity(self, o: f32) -> Self;
}

impl WithOpacity for button::Style {
    fn with_opacity(mut self, o: f32) -> Self {
        self.text_color = alpha(self.text_color, self.text_color.a * o);
        if let Some(Background::Color(c)) = self.background {
            self.background = Some(Background::Color(alpha(c, c.a * o)));
        }
        self
    }
}

/// A tooltip in the brand's floating sheet.
pub fn with_tip<'a, M: 'a>(content: impl Into<El<'a, M>>, tip: String) -> El<'a, M> {
    if tip.is_empty() {
        return content.into();
    }
    tooltip(
        content,
        container(line(tip, tokens::CAPTION, p().ink)).padding([2.0, tokens::SPACE_2]).style(|_| floating()),
        tooltip::Position::Bottom,
    )
    .gap(4)
    .into()
}

/// A sheet that floats over content: raised, hairline, the brand's one shadow.
pub fn floating() -> container::Style {
    let p = p();
    container::Style {
        background: Some(p.surface_raised.into()),
        text_color: Some(p.ink),
        border: Border { color: p.border, width: 1.0, radius: tokens::RADIUS_LG.into() },
        shadow: Shadow {
            color: if theme::is_dark() { alpha(Color::BLACK, 0.4) } else { alpha(p.ink, 0.08) },
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        snap: true,
    }
}

/// The switch: a squared track, never a pill (`radius-full` is for people).
pub fn switch<'a, M: Clone + 'a>(on: bool, msg: Option<M>) -> El<'a, M> {
    let enabled = msg.is_some();
    // A switch that can't be changed fades as a whole, track and knob.
    let a = if enabled { 1.0 } else { 0.5 };
    let knob = container(Space::new()).width(16).height(16).style(move |_| container::Style {
        background: Some(alpha(p().surface_raised, a).into()),
        border: Border { radius: 4.0.into(), ..Border::default() },
        ..container::Style::default()
    });
    let track = container(knob)
        .width(36)
        .height(20)
        .padding(2)
        .align_x(if on { Alignment::End } else { Alignment::Start })
        .style(move |_| container::Style {
            background: Some(alpha(if on { p().accent } else { p().border_strong }, a).into()),
            border: Border { radius: tokens::RADIUS_SM.into(), ..Border::default() },
            ..container::Style::default()
        });
    let b = button(track).padding(0).style(|_, _| button::Style::default());
    match msg {
        Some(m) => b.on_press(m).into(),
        None => b.into(),
    }
}

/// A row that toggles: its label, then the switch.
pub fn toggle_row<'a, M: Clone + 'a>(label: impl Into<String>, on: bool, msg: Option<M>) -> El<'a, M> {
    let enabled = msg.is_some();
    let color = if enabled { p().ink } else { alpha(p().ink, 0.5) };
    let r = row![container(t(label.into(), tokens::BODY_SMALL, color)).width(Length::Fill), switch(on, msg.clone())]
        .spacing(tokens::SPACE_3)
        .align_y(Alignment::Center)
        .padding([tokens::SPACE_2, 0.0]);
    let b = button(r).padding(0).style(|_, _| button::Style::default());
    match msg {
        Some(m) => b.on_press(m).into(),
        None => b.into(),
    }
}

/// A labelled setting.
pub fn field<'a, M: 'a>(label: impl Into<String>, content: impl Into<El<'a, M>>) -> El<'a, M> {
    column![line(label.into(), tokens::CAPTION, p().ink_muted).font(theme::SANS_MEDIUM), content.into()]
        .spacing(6)
        .padding([tokens::SPACE_2, 0.0])
        .into()
}

pub fn input_style(_: &Theme, status: text_input::Status) -> text_input::Style {
    let p = p();
    let border = match status {
        text_input::Status::Focused { .. } => p.accent,
        text_input::Status::Hovered => p.border_strong,
        _ => p.border,
    };
    text_input::Style {
        background: Background::Color(p.surface_sunken),
        border: Border { color: border, width: 1.0, radius: tokens::RADIUS_MD.into() },
        icon: p.ink_muted,
        placeholder: p.ink_subtle,
        value: p.ink,
        selection: p.accent_tint,
    }
}

/// The kit's text box.
pub fn input<'a, M: Clone + 'a>(
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> M + 'a,
) -> text_input::TextInput<'a, M> {
    text_input(placeholder, value)
        .on_input(on_input)
        .font(theme::SANS)
        .size(tokens::BODY_SMALL.size)
        .padding([6.0, 10.0])
        .style(input_style)
}

/// A dropdown of choices (the kit's select).
pub fn select<'a, T, L, M>(options: L, selected: Option<T>, on_select: impl Fn(T) -> M + 'a) -> El<'a, M>
where
    T: ToString + PartialEq + Clone + 'a,
    L: Borrow<[T]> + 'a,
    M: Clone + 'a,
{
    pick_list(options, selected, on_select)
        .font(theme::SANS)
        .text_size(tokens::BODY_SMALL.size)
        .padding([6.0, 10.0])
        .width(Length::Fill)
        .style(|_, status| {
            let p = p();
            pick_list::Style {
                text_color: p.ink,
                placeholder_color: p.ink_subtle,
                handle_color: p.ink_muted,
                background: Background::Color(p.surface_sunken),
                border: Border {
                    color: if status == pick_list::Status::Hovered { p.border_strong } else { p.border },
                    width: 1.0,
                    radius: tokens::RADIUS_MD.into(),
                },
            }
        })
        .menu_style(|_| {
            let p = p();
            iced::overlay::menu::Style {
                background: p.surface_raised.into(),
                border: Border { color: p.border, width: 1.0, radius: tokens::RADIUS_LG.into() },
                text_color: p.ink,
                selected_text_color: p.accent_deep,
                selected_background: p.accent_tint.into(),
                shadow: floating().shadow,
            }
        })
        .into()
}

/// A thin progress bar in the accent.
pub fn progress<'a, M: 'a>(value: f32, max: f32) -> El<'a, M> {
    let frac = if max > 0.0 { (value / max).clamp(0.0, 1.0) } else { 0.0 };
    let bar = container(Space::new())
        .width(Length::FillPortion((frac * 1000.0).round().max(0.0) as u16))
        .height(6)
        .style(|_| container::Style {
            background: Some(p().accent.into()),
            border: Border { radius: 3.0.into(), ..Border::default() },
            ..container::Style::default()
        });
    let rest = Space::new().width(Length::FillPortion(((1.0 - frac) * 1000.0).round().max(0.0) as u16));
    container(row![bar, rest])
        .width(Length::Fill)
        .height(6)
        .style(|_| container::Style {
            background: Some(p().border.into()),
            border: Border { radius: 3.0.into(), ..Border::default() },
            ..container::Style::default()
        })
        .into()
}

/// A 1 px line across: hairlines part a screen.
pub fn hairline<'a, M: 'a>() -> El<'a, M> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style { background: Some(p().border.into()), ..container::Style::default() })
        .into()
}

/// A short vertical divider between title bar groups.
pub fn divider<'a, M: 'a>() -> El<'a, M> {
    container(Space::new())
        .width(1)
        .height(16)
        .style(|_| container::Style { background: Some(p().border.into()), ..container::Style::default() })
        .into()
}

fn strong_line<'a, M: 'a>(height: f32) -> El<'a, M> {
    container(Space::new())
        .width(Length::Fill)
        .height(height)
        .style(|_| container::Style { background: Some(p().border_strong.into()), ..container::Style::default() })
        .into()
}

/// 子持ち罫: a 2 px line over a 1 px line with 3 px between, under an app bar only.
pub fn rule<'a, M: 'a>() -> El<'a, M> {
    column![strong_line(2.0), Space::new().height(3), strong_line(1.0)].into()
}

/// The kamon's frame around the one container being worked in: a 2 px band, a 3 px gap and a 1 px
/// line. `bg` fills inside the band.
pub fn frame<'a, M: 'a>(content: impl Into<El<'a, M>>, bg: Color, radius: f32) -> Container<'a, M> {
    let inner = container(content).style(move |_| container::Style {
        border: Border { color: p().border_strong, width: 1.0, radius: (radius - 5.0).max(0.0).into() },
        ..container::Style::default()
    });
    container(inner).padding(3).style(move |_| container::Style {
        background: Some(bg.into()),
        border: Border { color: p().border_strong, width: 2.0, radius: radius.into() },
        ..container::Style::default()
    })
}

/// A dialog: a sheet in the kamon's frame over the dimmed window, with its title centred, a hairline
/// before the text and the actions as full-width buttons, the main one on top.
pub fn dialog<'a, M: Clone + 'a>(
    title: impl Into<String>,
    body: Option<El<'a, M>>,
    actions: Vec<El<'a, M>>,
) -> El<'a, M> {
    let mut sheet = Column::new()
        .push(
            container(t(title.into(), TypeHeading::DIALOG, p().ink).font(theme::SANS_MEDIUM).center())
                .width(Length::Fill)
                .center_x(Length::Fill),
        )
        .spacing(0);
    match body {
        Some(b) => {
            sheet = sheet
                .push(Space::new().height(tokens::SPACE_3))
                .push(hairline())
                .push(Space::new().height(tokens::SPACE_3));
            sheet = sheet.push(b).push(Space::new().height(tokens::SPACE_4));
        }
        None => sheet = sheet.push(Space::new().height(tokens::SPACE_4)),
    }
    let buttons = Column::with_children(actions).spacing(6).width(Length::Fill);
    sheet = sheet.push(buttons);
    let framed = frame(
        container(sheet).padding(Padding::new(tokens::SPACE_4).bottom(tokens::SPACE_3)),
        p().surface,
        tokens::RADIUS_LG,
    )
    .max_width(384)
    .style(|_| {
        let mut s = floating();
        s.background = Some(p().surface.into());
        s.border = Border { color: p().border_strong, width: 2.0, radius: tokens::RADIUS_LG.into() };
        s
    });
    let backdrop = container(framed)
        .center(Length::Fill)
        .padding(20)
        .style(|_| container::Style { background: Some(alpha(p().ink, 0.3).into()), ..container::Style::default() });
    iced::widget::opaque(backdrop)
}

/// A dialog's full-width action button, its label centred and the primary's inner line around
/// the whole button.
pub fn dialog_button<'a, M: Clone + 'a>(label: impl Into<String>, variant: Variant, on: Option<M>) -> El<'a, M> {
    let lined = matches!(variant, Variant::Primary | Variant::Danger);
    let label = container(plain(label.into(), tokens::LABEL)).center_x(Length::Fill).center_y(Length::Fill);
    let body: El<'a, M> = if lined {
        let line_color = if variant == Variant::Primary { p().on_accent } else { p().surface };
        inner_line(label, line_color).width(Length::Fill).into()
    } else {
        label.into()
    };
    let b = button(body)
        .width(Length::Fill)
        .height(Size::Lg.height())
        .padding(if lined { 2 } else { 0 })
        .style(button_style(variant));
    match on {
        Some(m) => b.on_press(m).into(),
        None => b.into(),
    }
}

/// The dialog title's size: between body-small and heading, as the React UI had it.
pub struct TypeHeading;

impl TypeHeading {
    pub const DIALOG: crate::theme::TypeStyle = tokens::BODY;
}

/// A titled group of settings on a card, its rows parted by hairlines.
pub fn section<'a, M: 'a>(title: Option<String>, rows: Vec<El<'a, M>>) -> El<'a, M> {
    let mut col = Column::new();
    if let Some(title) = title {
        col = col.push(
            container(line(title, tokens::CAPTION, p().ink_muted).font(theme::SANS_MEDIUM))
                .padding(Padding::ZERO.top(8).bottom(2)),
        );
    }
    let n = rows.len();
    for (i, r) in rows.into_iter().enumerate() {
        col = col.push(r);
        if i + 1 < n {
            col = col.push(hairline());
        }
    }
    container(col)
        .width(Length::Fill)
        .padding([6.0, 14.0])
        .style(|_| container::Style {
            background: Some(p().surface_raised.into()),
            border: Border { color: p().border, width: 1.0, radius: tokens::RADIUS_LG.into() },
            ..container::Style::default()
        })
        .into()
}

/// The loader icon, turning with the clock (the app ticks while one is shown).
pub fn spinner<'a, M: 'a>(size: f32, now: Instant) -> El<'a, M> {
    let turn = (now.duration_since(*START).as_millis() % 900) as f32 / 900.0;
    icon(Icon::LoaderCircle, size, p().ink_muted).rotation(iced::Radians(turn * std::f32::consts::TAU)).into()
}

static START: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);

/// A rounded square of a project's colour (FR-64): 10 px, never a fill or a chip.
pub fn swatch<'a, M: 'a>(color: Color, size: f32) -> El<'a, M> {
    container(Space::new())
        .width(size)
        .height(size)
        .style(move |_| container::Style {
            background: Some(color.into()),
            border: Border { radius: Radius::from(size * 0.3), ..Border::default() },
            ..container::Style::default()
        })
        .into()
}

/// A plain clickable area with no look of its own (rows, the title bar's tags).
pub fn bare<'a, M: Clone + 'a>(content: impl Into<El<'a, M>>, on: Option<M>, hover_bg: bool) -> Button<'a, M> {
    let b = button(content).padding(0).style(move |_, status| button::Style {
        background: (hover_bg && hovered(status)).then_some(Background::Color(p().surface_sunken)),
        border: Border { radius: tokens::RADIUS_MD.into(), ..Border::default() },
        ..button::Style::default()
    });
    match on {
        Some(m) => b.on_press(m),
        None => b,
    }
}

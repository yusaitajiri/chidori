//! The kamon brand in Iced: tokens from `brand/tokens.json`, the two fonts, and the palette in use.
//! Styles read the palette when they draw, so a switch between light and dark restyles everything.

use std::sync::atomic::{AtomicBool, Ordering};

use iced::widget::text::{LineHeight, Wrapping};
use iced::widget::{Text, text};
use iced::{Color, Font, font};

// The brand's whole token set is generated, used or not, so the app reads like the kit.
#[allow(dead_code)]
pub mod tokens {
    use iced::Color;
    include!(concat!(env!("OUT_DIR"), "/tokens.rs"));
}

pub use tokens::{Palette, TypeStyle};

pub const SANS: Font = Font::with_name("Zen Kaku Gothic New");
pub const SANS_MEDIUM: Font = Font { weight: font::Weight::Medium, ..SANS };
pub const MONO: Font = Font::with_name("IBM Plex Mono");

pub const FONTS: [&[u8]; 3] = [
    include_bytes!("../brand/fonts/ZenKakuGothicNew-Regular.ttf"),
    include_bytes!("../brand/fonts/ZenKakuGothicNew-Medium.ttf"),
    include_bytes!("../brand/fonts/IBMPlexMono-Regular.ttf"),
];

static DARK: AtomicBool = AtomicBool::new(false);

pub fn set_dark(dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
}

pub fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

/// The palette in use: light or dark, following Windows.
pub fn p() -> &'static Palette {
    if is_dark() { &tokens::DARK } else { &tokens::LIGHT }
}

/// `a` over `b`, `amount` of the way (CSS `color-mix`).
pub fn mix(a: Color, b: Color, amount: f32) -> Color {
    Color {
        r: a.r * amount + b.r * (1.0 - amount),
        g: a.g * amount + b.g * (1.0 - amount),
        b: a.b * amount + b.b * (1.0 - amount),
        a: 1.0,
    }
}

pub fn alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

/// The hint under a finished session's line: the accent's tint, faded into the ground.
pub fn hint() -> Color {
    mix(p().accent_tint, p().surface, 0.45)
}

/// Text in one of the brand's type styles.
pub fn t<'a>(s: impl text::IntoFragment<'a>, style: TypeStyle, color: Color) -> Text<'a> {
    let font = if style.mono {
        MONO
    } else if style.medium {
        SANS_MEDIUM
    } else {
        SANS
    };
    text(s).size(style.size).line_height(LineHeight::Absolute(style.line_height.into())).font(font).color(color)
}

/// One line in the colour of what holds it (a button's label follows the button's state).
pub fn plain<'a>(s: impl text::IntoFragment<'a>, style: TypeStyle) -> Text<'a> {
    let font = if style.mono {
        MONO
    } else if style.medium {
        SANS_MEDIUM
    } else {
        SANS
    };
    text(s)
        .size(style.size)
        .line_height(LineHeight::Absolute(style.line_height.into()))
        .font(font)
        .wrapping(Wrapping::None)
}

/// One line that never wraps (labels, buttons, the title bar).
pub fn line<'a>(s: impl text::IntoFragment<'a>, style: TypeStyle, color: Color) -> Text<'a> {
    t(s, style, color).wrapping(Wrapping::None)
}

/// Project colours (FR-64): small squares only, from the brand's palette, never the accent.
pub fn project_color(name: &str) -> Color {
    let p = p();
    match name {
        "blue" => p.ai,
        "teal" => p.tetsu,
        "green" => p.moss,
        "amber" => p.warning,
        "violet" => p.budo,
        "brown" => p.blush,
        _ => p.ink_subtle,
    }
}

pub const PROJECT_COLORS: [&str; 7] = ["slate", "blue", "teal", "green", "amber", "violet", "brown"];

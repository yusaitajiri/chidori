//! The UI's icons: Lucide's, as the React UI drew them (`brand/icons/`, ISC licence), tinted at draw time.

use std::sync::LazyLock;

use iced::widget::svg::{self, Handle, Svg};
use iced::{Color, Length};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    AppWindow,
    ArrowDown,
    ArrowLeft,
    ArrowRightLeft,
    Camera,
    Check,
    ChevronDown,
    ChevronRight,
    CircleAlert,
    CircleCheckBig,
    Copy,
    Download,
    Ellipsis,
    FileAudio,
    FileCode,
    FileOutput,
    FileText,
    FileType,
    Gauge,
    History,
    ImagePlus,
    Info,
    Keyboard,
    Link,
    LoaderCircle,
    Maximize,
    Mic,
    Minimize,
    Minus,
    MonitorSpeaker,
    Pause,
    Pencil,
    Pin,
    Play,
    RefreshCw,
    Scissors,
    Search,
    Settings,
    ShieldCheck,
    Star,
    StepForward,
    Trash,
    TriangleAlert,
    X,
}

macro_rules! handles {
    ($($name:literal),*) => {
        [$(LazyLock::new(|| Handle::from_memory(include_bytes!(concat!("../brand/icons/", $name, ".svg")).as_slice()))),*]
    };
}

static HANDLES: [LazyLock<Handle>; 44] = handles!(
    "app-window",
    "arrow-down",
    "arrow-left",
    "arrow-right-left",
    "camera",
    "check",
    "chevron-down",
    "chevron-right",
    "circle-alert",
    "circle-check-big",
    "copy",
    "download",
    "ellipsis",
    "file-audio",
    "file-code",
    "file-output",
    "file-text",
    "file-type",
    "gauge",
    "history",
    "image-plus",
    "info",
    "keyboard",
    "link-2",
    "loader-circle",
    "maximize-2",
    "mic",
    "minimize-2",
    "minus",
    "monitor-speaker",
    "pause",
    "pencil",
    "pin",
    "play",
    "refresh-cw",
    "scissors",
    "search",
    "settings",
    "shield-check",
    "star",
    "step-forward",
    "trash-2",
    "triangle-alert",
    "x"
);

/// An icon at a size, in one colour.
pub fn icon<'a>(which: Icon, size: f32, color: Color) -> Svg<'a> {
    svg(which)
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_, _| svg::Style { color: Some(color) })
}

fn svg<'a>(which: Icon) -> Svg<'a> {
    Svg::new((*HANDLES[which as usize]).clone())
}

static LOGO_SMALL: LazyLock<Handle> =
    LazyLock::new(|| Handle::from_memory(include_bytes!("../brand/logo/chidori-small.svg").as_slice()));
static LOGO_SMALL_DARK: LazyLock<Handle> =
    LazyLock::new(|| Handle::from_memory(include_bytes!("../brand/logo/chidori-small-on-dark.svg").as_slice()));
static LOGO: LazyLock<Handle> =
    LazyLock::new(|| Handle::from_memory(include_bytes!("../brand/logo/chidori.svg").as_slice()));
static LOGO_DARK: LazyLock<Handle> =
    LazyLock::new(|| Handle::from_memory(include_bytes!("../brand/logo/chidori-on-dark.svg").as_slice()));

/// Chidori's kamon, in its own colours: the small drawing below 32 px, the dark one in dark mode.
pub fn logo<'a>(size: f32) -> Svg<'a> {
    let dark = crate::theme::is_dark();
    let handle = match (size < 32.0, dark) {
        (true, false) => &*LOGO_SMALL,
        (true, true) => &*LOGO_SMALL_DARK,
        (false, false) => &*LOGO,
        (false, true) => &*LOGO_DARK,
    };
    Svg::new(handle.clone()).width(Length::Fixed(size)).height(Length::Fixed(size))
}

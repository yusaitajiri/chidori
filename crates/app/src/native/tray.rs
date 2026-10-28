//! Tray icon (FR-90): the app icon in ink while idle, paused or finishing, and in its 朱 while
//! recording. Left-click shows or hides the window; its menu starts, stops, shoots and quits.

use std::cell::RefCell;

use chidori_core::events::UiState;
use chidori_core::settings::Locale;
use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::bridge::{self, UiEvent};

pub const TOGGLE: &str = "tray:toggle";
pub const SHOT: &str = "tray:shot";
pub const SHOW: &str = "tray:show";
pub const QUIT: &str = "tray:quit";

struct Labels {
    start: &'static str,
    stop: &'static str,
    shot: &'static str,
    show: &'static str,
    quit: &'static str,
    idle_tip: &'static str,
    rec_tip: &'static str,
    finishing_tip: &'static str,
}

fn labels(locale: Locale) -> Labels {
    match locale {
        Locale::Ja => Labels {
            start: "開始",
            stop: "停止",
            shot: "スクショ",
            show: "ウィンドウを表示",
            quit: "終了",
            idle_tip: "Chidori",
            rec_tip: "Chidori — 録音中",
            finishing_tip: "Chidori — 仕上げ中…",
        },
        Locale::En => Labels {
            start: "Start",
            stop: "Stop",
            shot: "Screenshot",
            show: "Show window",
            quit: "Quit",
            idle_tip: "Chidori",
            rec_tip: "Chidori — recording",
            finishing_tip: "Chidori — finishing…",
        },
    }
}

// 32 px renders of icons/tray/*.svg: the kit's small app icon, its frame filled with paper so it
// reads on light and dark taskbars. recording.svg is the kit's icons/chidori/icon-small.svg as it
// is; idle.svg is the same drawing in ink, the brand's one-colour form.
fn picture(state: UiState) -> Option<Icon> {
    let png: &[u8] = match state {
        UiState::Recording => include_bytes!("../../icons/tray/recording.png"),
        _ => include_bytes!("../../icons/tray/idle.png"),
    };
    let img = image::load_from_memory(png).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).ok()
}

fn menu(locale: Locale, recording: bool) -> Menu {
    let l = labels(locale);
    let menu = Menu::new();
    let items: [&dyn tray_icon::menu::IsMenuItem; 5] = [
        &MenuItem::with_id(TOGGLE, if recording { l.stop } else { l.start }, true, None),
        &MenuItem::with_id(SHOT, l.shot, recording, None),
        &MenuItem::with_id(SHOW, l.show, true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(QUIT, l.quit, true, None),
    ];
    if let Err(e) = menu.append_items(&items) {
        tracing::warn!("tray menu: {e}");
    }
    menu
}

thread_local! {
    static TRAY: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
}

/// Creates the tray icon; on the main thread, whose message loop serves it.
pub fn create(locale: Locale) {
    TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
            bridge::send(UiEvent::TrayClick);
        }
    }));
    let mut builder = TrayIconBuilder::new()
        .with_tooltip("Chidori")
        .with_menu(Box::new(menu(locale, false)))
        .with_menu_on_left_click(false);
    if let Some(icon) = picture(UiState::Ready) {
        builder = builder.with_icon(icon);
    }
    match builder.build() {
        Ok(tray) => TRAY.with(|t| *t.borrow_mut() = Some(tray)),
        Err(e) => tracing::error!("tray icon: {e}"),
    }
}

/// Reflects the app state in the icon, tooltip and menu.
pub fn update(state: UiState, locale: Locale) {
    TRAY.with(|t| {
        let Some(tray) = &*t.borrow() else { return };
        let l = labels(locale);
        let tip = match state {
            UiState::Recording | UiState::Paused => l.rec_tip,
            UiState::Finishing => l.finishing_tip,
            _ => l.idle_tip,
        };
        let _ = tray.set_icon(picture(state));
        let _ = tray.set_tooltip(Some(tip));
        tray.set_menu(Some(Box::new(menu(locale, matches!(state, UiState::Recording | UiState::Paused)))));
    });
}

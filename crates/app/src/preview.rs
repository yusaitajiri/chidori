//! Renders screens to PNG without opening a window, for checking how they look:
//! `cargo test -p chidori preview -- --ignored --test-threads 1` writes
//! `<target>/previews/<name>-light.png` and `-dark.png` at 2× scale.
//!
//! Each preview builds the real `App` on a core whose folders are in a temporary directory, sets
//! up the state it wants to show, and draws `App::view`.

use std::path::PathBuf;
use std::sync::Arc;

use chidori_core::asr::worker::AsrWorker;
use chidori_core::host::{AppHandle, Paths};
use chidori_core::state::{AppHost, AppState};
use iced::{Settings, Size};

use crate::app::App;
use crate::theme;

/// A core on a temporary folder: no models, no sessions, default settings.
pub fn core() -> (AppHandle, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let base = dir.path().to_path_buf();
    let paths = Paths {
        config: base.join("config"),
        local_data: base.join("local"),
        logs: base.join("logs"),
        resources: base.join("resources"),
        documents: base.join("documents"),
    };
    crate::bridge::init();
    let core = AppHandle::new(paths, "0.0.0", Arc::new(crate::bridge::Events), Arc::new(crate::bridge::AppShell));
    let worker = AsrWorker::spawn(Arc::new(AppHost { app: core.clone() }));
    core.manage(AppState::new(&core, worker));
    (core, dir)
}

/// The app as it starts, with setup done so the main view shows.
pub fn app() -> (App, tempfile::TempDir) {
    let (core, dir) = core();
    let (mut app, _) = App::new(core);
    // A fresh settings file takes the language of Windows, so pin it or an English runner sees English.
    app.settings.locale = chidori_core::settings::Locale::Ja;
    app.info.setup_done = true;
    app.rec.state = chidori_core::events::UiState::Ready;
    (app, dir)
}

fn out_dir() -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    target.join("previews")
}

/// Draws the app at a size, light and dark, into `previews/<name>-{light,dark}.png`.
pub fn shot(app: &App, name: &str, size: Size) {
    let dir = out_dir();
    std::fs::create_dir_all(&dir).expect("previews folder");
    for dark in [false, true] {
        theme::set_dark(dark);
        let settings = Settings {
            fonts: theme::FONTS.iter().map(|f| std::borrow::Cow::Borrowed(*f)).collect(),
            default_font: theme::SANS,
            ..Settings::default()
        };
        let mut sim = iced_test::simulator::Simulator::with_size(settings, size, app.view());
        let snap = sim.snapshot(&iced::Theme::Light).expect("snapshot");
        let theme_name = if dark { "dark" } else { "light" };
        let path = dir.join(format!("{name}-{theme_name}"));
        // The simulator names the file after its renderer and never replaces one that exists.
        for backend in ["wgpu", "tiny-skia"] {
            let _ = std::fs::remove_file(dir.join(format!("{name}-{theme_name}-{backend}.png")));
        }
        snap.matches_image(&path).expect("writing the preview");
    }
    theme::set_dark(false);
}

#[test]
#[ignore = "writes PNGs for review"]
fn preview_start() {
    let (app, _dir) = app();
    shot(&app, "start", crate::app::EXPANDED);
    shot(&app, "start-compact", crate::app::COMPACT);
}

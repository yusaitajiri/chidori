//! The app: its state, what the core and the shell send it, and the window it draws. Each screen
//! is a module under `screens` with its own state, messages, update and view; this file routes
//! between them and holds what they share.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use chidori_core::audio::source::{AudioApp, Device};
use chidori_core::error::AppError;
use chidori_core::events::{Event, Notice, NoticeLevel, UiState};
use chidori_core::host::AppHandle;
use chidori_core::models::manager::ModelEntry;
use chidori_core::ops::{self, AppInfo, SessionView};
use chidori_core::session::projects::Project;
use chidori_core::settings::{Layout, Settings};
use iced::widget::{Space, column, container, stack};
use iced::{Border, Element, Length, Size, Subscription, Task, Theme, window};

use crate::bridge::{self, UiEvent};
use crate::model::line::LineHistory;
use crate::model::recording::Recording;
use crate::model::source::{AppLevels, SourceChoice};
use crate::model::transcript::Transcript;
use crate::native::{self, menu};
use crate::screens;
use crate::theme::{self, p};
use crate::{actions, i18n};

pub const EXPANDED: Size = Size::new(420.0, 640.0);
const EXPANDED_MIN: Size = Size::new(360.0, 420.0);
pub const COMPACT: Size = Size::new(380.0, 170.0);
const COMPACT_MIN: Size = Size::new(340.0, 150.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Main,
    Settings,
    History,
    /// A past session, opened from 履歴.
    Session,
}

/// A section of 設定, or the list of sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Index,
    General,
    Audio,
    Transcription,
    Models,
    Screenshots,
    Hotkeys,
    Output,
}

/// The three places named in the title bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Main,
    History,
    Settings,
}

impl View {
    /// A past session sits under 履歴, the model list under 設定.
    pub fn place(self) -> Place {
        match self {
            View::History | View::Session => Place::History,
            View::Settings => Place::Settings,
            View::Main => Place::Main,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl From<NoticeLevel> for Level {
    fn from(l: NoticeLevel) -> Self {
        match l {
            NoticeLevel::Info => Level::Info,
            NoticeLevel::Warn => Level::Warn,
            NoticeLevel::Error => Level::Error,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub level: Level,
    pub at: Instant,
}

#[derive(Debug, Clone)]
pub struct Banner {
    pub id: u64,
    pub notice: Notice,
}

const TOAST_FOR: Duration = Duration::from_secs(2);

pub struct App {
    pub core: AppHandle,
    pub window: Option<window::Id>,
    pub hwnd: Option<u64>,
    /// The window is minimized or in the tray: nothing animates.
    pub hidden: bool,
    pub now: Instant,

    pub settings: Settings,
    pub info: AppInfo,
    /// The model catalog, for showing model names.
    pub models: Vec<ModelEntry>,
    pub rec: Recording,
    /// The live (or last recorded) session; the core's live events feed it.
    pub live: Transcript,
    pub line: LineHistory,
    pub source: SourceChoice,
    pub app_levels: AppLevels,
    pub projects: Vec<Project>,
    /// Counts changes to which session is in which project, so lists of sessions reload.
    pub projects_version: u64,

    pub view: View,
    pub settings_tab: SettingsTab,
    pub viewing_session: Option<String>,
    pub banners: Vec<Banner>,
    pub toasts: Vec<Toast>,
    /// The border flashes when a screenshot is taken.
    pub flash: Option<Instant>,
    /// A file is held over the window (FR-18).
    pub dropping: bool,
    next_id: u64,
    /// The layout the window was last sized for.
    shown_layout: Option<Layout>,

    pub main: screens::main_view::State,
    pub history: screens::history::State,
    pub viewer: screens::viewer::State,
    pub settings_ui: screens::settings::State,
    pub models_ui: screens::models::State,
    pub wizard: screens::wizard::State,
    pub dialogs: screens::dialogs::State,
}

#[derive(Debug, Clone)]
pub enum Message {
    Bridge(UiEvent),
    WindowOpened(window::Id),
    Hwnd(u64),
    WindowEvent(window::Event),
    ThemeMode(iced::theme::Mode),
    Tick(Instant),

    Go(View),
    GoSettings(SettingsTab),
    OpenSession(String),
    Win(WinAction),
    DismissBanner(u64),
    /// Runs a notice's action; the banner that offered it, if any, goes.
    NoticeAction(String, Option<u64>),
    Toast(String, Level),
    /// A command failed: a banner or a toast, by its error code (`fallback` is the toast's key).
    Failed(AppError, &'static str),
    /// Shows a native popup menu, at a point in the window or at the cursor.
    Popup(Vec<menu::Item>, Option<iced::Point>),

    SettingsChanged(Box<(Settings, Settings)>),
    Info(Box<AppInfo>),
    Models(Vec<ModelEntry>),
    Projects(Vec<Project>),
    /// Which session is in which project changed.
    ProjectsChanged,
    Apps(Vec<AudioApp>),
    Devices(Vec<Device>),
    LiveLoaded(Box<SessionView>),
    /// Applies a partial settings object (validated in the core).
    UpdateSettings(serde_json::Value),

    Action(actions::Action),

    Titlebar(screens::titlebar::Msg),
    Main(screens::main_view::Msg),
    History(screens::history::Msg),
    Viewer(screens::viewer::Msg),
    SettingsUi(screens::settings::Msg),
    ModelsUi(screens::models::Msg),
    Wizard(screens::wizard::Msg),
    Dialog(screens::dialogs::Msg),
    Noop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinAction {
    Drag,
    Minimize,
    Close,
    ToggleOnTop,
    ToggleLayout,
    Show,
    Hide,
    ToggleVisible,
}

pub fn run() -> iced::Result {
    let mut app = iced::application(App::boot, App::update, App::view)
        .title(|_: &App| "Chidori".to_string())
        .subscription(App::subscription)
        .theme(App::theme)
        .default_font(theme::SANS)
        .exit_on_close_request(false)
        .window(window::Settings {
            size: EXPANDED,
            min_size: Some(EXPANDED_MIN),
            decorations: false,
            position: saved_position().map_or(window::Position::Centered, window::Position::Specific),
            icon: window_icon(),
            ..window::Settings::default()
        });
    for font in theme::FONTS {
        app = app.font(font);
    }
    app.run()
}

fn window_icon() -> Option<window::Icon> {
    let img = image::load_from_memory(include_bytes!("../icons/32x32.png")).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    window::icon::from_rgba(img.into_raw(), w, h).ok()
}

/// Where the window was last time (`window.json`), so it opens there again.
fn saved_position() -> Option<iced::Point> {
    let app = bridge::core()?;
    let bytes = std::fs::read(app.paths().config.join("window.json")).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    Some(iced::Point::new(v.get("x")?.as_f64()? as f32, v.get("y")?.as_f64()? as f32))
}

fn save_position(app: &AppHandle, at: iced::Point) {
    let body = serde_json::json!({ "x": at.x, "y": at.y });
    let _ = std::fs::create_dir_all(&app.paths().config);
    if let Err(e) = std::fs::write(app.paths().config.join("window.json"), body.to_string()) {
        tracing::debug!("saving the window position: {e}");
    }
}

/// Runs a blocking core call off the UI thread and turns its result into a message.
pub fn call<T: Send + 'static>(
    app: &App,
    f: impl FnOnce(&AppHandle) -> T + Send + 'static,
    map: impl FnOnce(T) -> Message + Send + 'static,
) -> Task<Message> {
    let core = app.core.clone();
    Task::future(async move {
        match chidori_core::host::runtime().spawn_blocking(move || f(&core)).await {
            Ok(v) => map(v),
            Err(e) => {
                tracing::error!("core call panicked: {e}");
                Message::Noop
            }
        }
    })
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        Self::new(bridge::core().expect("core set before the window opens").clone())
    }

    pub fn new(core: AppHandle) -> (Self, Task<Message>) {
        let settings = ops::settings(&core);
        let info = ops::app_info(&core);
        let mut source = SourceChoice::default();
        source.init_from(&settings);
        let mut rec = Recording::default();
        rec.apply(&info.state);
        rec.engine = Some(info.engine.clone());
        let app = App {
            core,
            window: None,
            hwnd: None,
            hidden: false,
            now: Instant::now(),
            settings,
            info,
            models: Vec::new(),
            rec,
            live: Transcript::default(),
            line: LineHistory::default(),
            source,
            app_levels: AppLevels::default(),
            projects: Vec::new(),
            projects_version: 0,
            view: View::Main,
            settings_tab: SettingsTab::Index,
            viewing_session: None,
            banners: Vec::new(),
            toasts: Vec::new(),
            flash: None,
            dropping: false,
            next_id: 1,
            shown_layout: None,
            main: Default::default(),
            history: Default::default(),
            viewer: Default::default(),
            settings_ui: Default::default(),
            models_ui: Default::default(),
            wizard: Default::default(),
            dialogs: Default::default(),
        };
        let follow = app.follow_session(app.rec.session_id.clone());
        let tasks = Task::batch([
            iced::system::theme().map(Message::ThemeMode),
            app.refresh_models(),
            app.refresh_projects(),
            app.refresh_devices(),
            follow,
            screens::dialogs::on_launch(&app),
        ]);
        (app, tasks)
    }

    pub fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn layout(&self) -> Layout {
        self.settings.window.layout
    }

    /// The setup wizard shows until setup is done and a model exists.
    pub fn needs_wizard(&self) -> bool {
        !self.info.setup_done || self.rec.state == UiState::NeedsModel
    }

    /// Only 録音 has a compact form; everywhere else the window is expanded.
    pub fn effective_layout(&self) -> Layout {
        if self.needs_wizard() || self.view != View::Main { Layout::Expanded } else { self.layout() }
    }

    pub fn compact(&self) -> bool {
        self.effective_layout() == Layout::Compact
    }

    pub fn refresh_models(&self) -> Task<Message> {
        call(self, ops::models_list, Message::Models)
    }

    pub fn refresh_info(&self) -> Task<Message> {
        call(self, |c| Box::new(ops::app_info(c)), Message::Info)
    }

    pub fn refresh_projects(&self) -> Task<Message> {
        call(self, ops::list_projects, Message::Projects)
    }

    pub fn refresh_apps(&mut self) -> Task<Message> {
        self.source.loading_apps = true;
        call(self, |_| ops::list_audio_apps().unwrap_or_default(), Message::Apps)
    }

    pub fn refresh_devices(&self) -> Task<Message> {
        call(self, |_| ops::list_mic_devices().unwrap_or_default(), Message::Devices)
    }

    /// Loads the transcript of a session the core reports as active.
    fn follow_session(&self, session_id: Option<String>) -> Task<Message> {
        let Some(id) = session_id else { return Task::none() };
        if self.live.session_id.as_deref() == Some(id.as_str()) {
            return Task::none();
        }
        call(
            self,
            move |c| ops::get_session(c, &id).ok(),
            |v| v.map_or(Message::Noop, |v| Message::LiveLoaded(Box::new(v))),
        )
    }

    /// Whether the live transcript shows the session being recorded, so live events belong to it.
    fn is_live(&self) -> bool {
        self.rec.session_id.is_some() && self.live.session_id == self.rec.session_id
    }

    pub fn go(&mut self, view: View) {
        if view == View::Settings {
            // 設定 opens at its list unless a section is asked for.
            self.settings_tab = SettingsTab::Index;
        }
        self.view = view;
    }

    pub fn toast(&mut self, text: impl Into<String>, level: Level) {
        self.toasts.push(Toast { text: text.into(), level, at: self.now });
        if self.toasts.len() > 3 {
            self.toasts.remove(0);
        }
    }

    pub fn push_banner(&mut self, notice: Notice) {
        // One banner per code; the newest replaces an older one.
        self.banners.retain(|b| b.notice.code != notice.code);
        let id = self.next_id();
        self.banners.push(Banner { id, notice });
        if self.banners.len() > 3 {
            self.banners.remove(0);
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Bridge(event) => self.on_bridge(event),
            Message::WindowOpened(id) => {
                self.window = Some(id);
                let mut tasks = vec![window::raw_id::<Message>(id).map(Message::Hwnd)];
                if self.settings.window.always_on_top {
                    tasks.push(window::set_level(id, window::Level::AlwaysOnTop));
                }
                tasks.push(self.sync_layout());
                Task::batch(tasks)
            }
            Message::Hwnd(hwnd) => {
                self.hwnd = Some(hwnd);
                native::window::apply_chrome(hwnd);
                native::window::set_capture_excluded(hwnd, self.settings.screenshot.exclude_self);
                Task::none()
            }
            Message::WindowEvent(event) => self.on_window_event(event),
            Message::ThemeMode(mode) => {
                theme::set_dark(mode == iced::theme::Mode::Dark);
                Task::none()
            }
            Message::Tick(now) => {
                self.now = now;
                self.toasts.retain(|t| now.duration_since(t.at) < TOAST_FOR);
                if self.flash.is_some_and(|f| now.duration_since(f) > Duration::from_millis(400)) {
                    self.flash = None;
                }
                self.live.age(now);
                self.app_levels.advance(now);
                Task::batch([screens::main_view::tick(self), screens::viewer::tick(self)])
            }
            Message::Go(view) => {
                self.go(view);
                let refresh = match view {
                    View::History => screens::history::on_open(self),
                    View::Settings => Task::batch([self.refresh_info(), self.refresh_models()]),
                    _ => Task::none(),
                };
                Task::batch([refresh, self.sync_layout()])
            }
            Message::GoSettings(tab) => {
                self.view = View::Settings;
                self.settings_tab = tab;
                Task::batch([self.refresh_info(), self.refresh_models(), self.sync_layout()])
            }
            Message::OpenSession(id) => {
                self.viewing_session = Some(id.clone());
                self.view = View::Session;
                Task::batch([screens::viewer::open(self, id), self.sync_layout()])
            }
            Message::Win(action) => self.on_window_action(action),
            Message::DismissBanner(id) => {
                self.banners.retain(|b| b.id != id);
                Task::none()
            }
            Message::NoticeAction(command, banner) => {
                if let Some(id) = banner {
                    self.banners.retain(|b| b.id != id);
                }
                actions::run_notice_action(self, &command)
            }
            Message::Toast(text, level) => {
                self.toast(text, level);
                Task::none()
            }
            Message::Failed(err, fallback) => {
                actions::report_error(self, &err, fallback);
                Task::none()
            }
            Message::Popup(items, at) => {
                if let Some(hwnd) = self.hwnd {
                    menu::popup(hwnd, &items, at);
                }
                Task::none()
            }
            Message::SettingsChanged(both) => {
                let (old, new) = *both;
                self.apply_settings(&old, &new);
                self.settings = new;
                Task::batch([self.sync_layout(), self.refresh_info()])
            }
            Message::UpdateSettings(partial) => {
                if let Some(hk) = partial.get("hotkeys") {
                    for k in ["toggle", "screenshot", "mark", "cut"] {
                        if let Some(acc) = hk.get(k).and_then(|v| v.as_str()).filter(|a| !a.trim().is_empty())
                            && let Err(e) = native::hotkeys::validate(acc)
                        {
                            self.toast(i18n::tf("failed", &[("detail", &e)]), Level::Error);
                            return Task::none();
                        }
                    }
                }
                call(self, move |c| Box::new(ops::set_settings(c, &partial)), Message::SettingsChanged)
            }
            Message::Info(info) => {
                self.info = *info;
                Task::none()
            }
            Message::Models(models) => {
                self.models = models;
                Task::none()
            }
            Message::Projects(projects) => {
                self.projects = projects;
                Task::none()
            }
            Message::ProjectsChanged => {
                self.projects_version += 1;
                let mut tasks = vec![self.refresh_projects(), screens::history::on_open(self)];
                // The session on screen shows its project in its header: reloaded with it.
                if let Some(id) = self.live.session_id.clone() {
                    tasks.push(call(
                        self,
                        move |c| ops::get_session(c, &id).ok(),
                        |v| v.map_or(Message::Noop, |v| Message::LiveLoaded(Box::new(v))),
                    ));
                }
                if self.view == View::Session
                    && let Some(id) = self.viewing_session.clone()
                {
                    tasks.push(Task::done(Message::OpenSession(id)));
                }
                Task::batch(tasks)
            }
            Message::Apps(apps) => {
                self.source.set_apps(apps);
                Task::none()
            }
            Message::Devices(devices) => {
                self.source.devices = devices;
                Task::none()
            }
            Message::LiveLoaded(view) => {
                self.live.load(*view);
                Task::none()
            }
            Message::Action(a) => actions::run(self, a),
            Message::Titlebar(m) => screens::titlebar::update(self, m),
            Message::Main(m) => screens::main_view::update(self, m),
            Message::History(m) => screens::history::update(self, m),
            Message::Viewer(m) => screens::viewer::update(self, m),
            Message::SettingsUi(m) => screens::settings::update(self, m),
            Message::ModelsUi(m) => screens::models::update(self, m),
            Message::Wizard(m) => screens::wizard::update(self, m),
            Message::Dialog(m) => screens::dialogs::update(self, m),
            Message::Noop => Task::none(),
        }
    }

    /// Settings that act on the shell: the locale, hotkeys, always on top, capture exclusion.
    fn apply_settings(&mut self, old: &Settings, new: &Settings) {
        if new.locale != old.locale {
            i18n::set_locale(new.locale);
            native::tray::update(self.rec.state, new.locale);
        }
        if new.hotkeys != old.hotkeys {
            native::hotkeys::register_all(&self.core);
        }
        if new.screenshot.exclude_self != old.screenshot.exclude_self
            && let Some(hwnd) = self.hwnd
        {
            native::window::set_capture_excluded(hwnd, new.screenshot.exclude_self);
        }
    }

    /// Sizes the window for the layout it shows, when that changed.
    pub fn sync_layout(&mut self) -> Task<Message> {
        let Some(id) = self.window else { return Task::none() };
        let layout = self.effective_layout();
        if self.shown_layout == Some(layout) {
            return Task::none();
        }
        self.shown_layout = Some(layout);
        let (size, min, resizable) = match layout {
            Layout::Compact => (COMPACT, COMPACT_MIN, false),
            Layout::Expanded => (EXPANDED, EXPANDED_MIN, true),
        };
        Task::batch([
            window::set_resizable(id, true),
            window::set_min_size(id, Some(min)),
            window::resize(id, size),
            window::set_resizable(id, resizable),
        ])
    }

    fn on_bridge(&mut self, event: UiEvent) -> Task<Message> {
        match event {
            UiEvent::Core(e) => self.on_core(e),
            UiEvent::TrayClick => self.on_window_action(WinAction::ToggleVisible),
            UiEvent::SecondInstance => self.on_window_action(WinAction::Show),
            UiEvent::Exit => iced::exit(),
            UiEvent::Menu(id) => {
                tracing::debug!("menu pick {id}");
                self.on_menu(id)
            }
        }
    }

    /// A pick in a native menu, routed by its id's prefix to the screen that opened it.
    fn on_menu(&mut self, id: String) -> Task<Message> {
        let (owner, rest) = id.split_once(':').unwrap_or((id.as_str(), ""));
        let rest = rest.to_string();
        match owner {
            "tray" => match id.as_str() {
                native::tray::TOGGLE => actions::run(self, actions::Action::ToggleRecording),
                native::tray::SHOT => actions::run(self, actions::Action::Screenshot),
                native::tray::SHOW => self.on_window_action(WinAction::Show),
                native::tray::QUIT => actions::request_quit(self),
                _ => Task::none(),
            },
            "action" => actions::on_menu(self, &rest),
            "titlebar" => screens::titlebar::on_menu(self, &rest),
            "main" => screens::main_view::on_menu(self, &rest),
            "history" => screens::history::on_menu(self, &rest),
            "viewer" => screens::viewer::on_menu(self, &rest),
            "settings" => screens::settings::on_menu(self, &rest),
            "dialog" => screens::dialogs::on_menu(self, &rest),
            _ => Task::none(),
        }
    }

    fn on_core(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::State(p) => {
                let before = self.rec.state;
                // The line keeps its own history: a new recording starts an empty one, a resume
                // (or a continued session, FR-08) leaves a gap.
                if p.state == UiState::Recording
                    && let Some(id) = &p.session_id
                {
                    let again = self.line.session_id.as_deref() == Some(id.as_str());
                    self.line.start(id);
                    if again && before != UiState::Recording {
                        self.line.gap();
                    }
                }
                self.rec.apply(&p);
                native::tray::update(p.state, self.settings.locale);
                let needs_info = (before == UiState::NeedsModel) != (p.state == UiState::NeedsModel);
                let mut tasks = vec![self.follow_session(p.session_id.clone()), self.sync_layout()];
                if needs_info {
                    tasks.push(self.refresh_info());
                }
                Task::batch(tasks)
            }
            Event::Levels(levels) => {
                if self.rec.state == UiState::Recording {
                    self.line.push(&levels);
                }
                screens::settings::on_levels(self, &levels);
                screens::wizard::on_levels(self, &levels);
                Task::none()
            }
            Event::AppLevels(levels) => {
                self.app_levels.hear(&levels, self.now);
                Task::none()
            }
            Event::Segment(seg) => {
                if self.is_live() {
                    self.live.add_segment(seg);
                }
                Task::none()
            }
            Event::Partial(p) => {
                if self.is_live() {
                    self.live.set_partial(p);
                }
                Task::none()
            }
            Event::SegmentRemoved(p) => {
                if self.is_live() {
                    self.live.remove_item(&p.id);
                }
                Task::none()
            }
            Event::SegmentUpdated(seg) => {
                if self.is_live() {
                    self.live.update_segment(seg);
                }
                Task::none()
            }
            Event::Screenshot(p) => {
                if self.is_live() {
                    self.live.add_screenshot(p);
                    self.line.shot();
                }
                Task::none()
            }
            Event::Marker(p) => {
                if self.is_live() {
                    self.live.add_marker(p);
                }
                Task::none()
            }
            Event::Lag(l) => {
                self.rec.lag = Some(l);
                Task::none()
            }
            Event::Finishing(p) => {
                self.rec.finishing = Some(p);
                Task::none()
            }
            Event::ModelStatus(status) => {
                self.rec.engine = Some(status);
                Task::batch([self.refresh_info(), self.refresh_models()])
            }
            Event::ModelDownload(p) => {
                Task::batch([screens::models::on_download(self, &p), screens::wizard::on_download(self, &p)])
            }
            Event::SessionSaved(p) => {
                self.rec.last_saved_folder = Some(p.folder.clone());
                self.toast(i18n::t("saved"), Level::Info);
                let id = p.session_id.clone();
                if self.live.session_id.as_deref() == Some(id.as_str()) {
                    call(
                        self,
                        move |c| ops::get_session(c, &id).ok(),
                        |v| v.map_or(Message::Noop, |v| Message::LiveLoaded(Box::new(v))),
                    )
                } else {
                    Task::none()
                }
            }
            Event::Notice(n) => {
                actions::show_notice(self, n);
                Task::none()
            }
        }
    }

    fn on_window_event(&mut self, event: window::Event) -> Task<Message> {
        match event {
            window::Event::CloseRequested => self.on_window_action(WinAction::Close),
            window::Event::Resized(size) => {
                self.set_hidden(size.width < 1.0 || size.height < 1.0);
                Task::none()
            }
            window::Event::Moved(at) => {
                save_position(&self.core, at);
                Task::none()
            }
            window::Event::Focused => {
                self.set_hidden(false);
                Task::none()
            }
            window::Event::FileHovered(_) => {
                self.dropping = self.can_drop();
                Task::none()
            }
            window::Event::FilesHoveredLeft => {
                self.dropping = false;
                Task::none()
            }
            window::Event::FileDropped(path) => {
                let ok = self.can_drop();
                self.dropping = false;
                if ok { actions::transcribe_file(self, Some(path)) } else { Task::none() }
            }
            _ => Task::none(),
        }
    }

    /// A dropped file is transcribed only when nothing records and setup is done (FR-18).
    fn can_drop(&self) -> bool {
        self.rec.state == UiState::Ready && !self.needs_wizard()
    }

    fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
        chidori_core::host::set_ui_hidden(hidden);
    }

    pub fn on_window_action(&mut self, action: WinAction) -> Task<Message> {
        let Some(id) = self.window else { return Task::none() };
        match action {
            WinAction::Drag => window::drag(id),
            WinAction::Minimize => {
                self.set_hidden(true);
                window::minimize(id, true)
            }
            WinAction::Close => {
                // Closing while recording or finishing hides to the tray; otherwise the app quits.
                if self.rec.busy() {
                    native::notify::show(&chidori_core::host::os_text(self.settings.locale, "hiddenToTray", ""));
                    self.on_window_action(WinAction::Hide)
                } else {
                    iced::exit()
                }
            }
            WinAction::Hide => {
                self.set_hidden(true);
                window::set_mode(id, window::Mode::Hidden)
            }
            WinAction::Show => {
                self.set_hidden(false);
                Task::batch([
                    window::set_mode(id, window::Mode::Windowed),
                    window::minimize(id, false),
                    window::gain_focus(id),
                ])
            }
            WinAction::ToggleVisible => {
                if self.hidden {
                    self.on_window_action(WinAction::Show)
                } else {
                    self.on_window_action(WinAction::Hide)
                }
            }
            WinAction::ToggleOnTop => {
                let on = !self.settings.window.always_on_top;
                let level = if on { window::Level::AlwaysOnTop } else { window::Level::Normal };
                Task::batch([
                    window::set_level(id, level),
                    Task::done(Message::UpdateSettings(serde_json::json!({ "window": { "alwaysOnTop": on } }))),
                ])
            }
            WinAction::ToggleLayout => {
                // Only 録音 has a compact form: shrinking from 履歴 or 設定 goes to 録音.
                let compact = self.view == View::Main && self.layout() == Layout::Compact;
                let next = if compact { Layout::Expanded } else { Layout::Compact };
                self.view = View::Main;
                self.settings.window.layout = next;
                let core = self.core.clone();
                chidori_core::host::runtime().spawn_blocking(move || ops::set_layout(&core, next));
                self.sync_layout()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            bridge::subscription().map(Message::Bridge),
            window::open_events().map(Message::WindowOpened),
            window::events().map(|(_, e)| Message::WindowEvent(e)),
            iced::system::theme_changes().map(Message::ThemeMode),
            screens::main_view::subscription(self),
            screens::viewer::subscription(self),
            screens::history::subscription(self),
            screens::settings::subscription(self),
            screens::wizard::subscription(self),
            screens::dialogs::subscription(self),
        ];
        if let Some(every) = self.tick_every() {
            subs.push(iced::time::every(every).map(Message::Tick));
        }
        Subscription::batch(subs)
    }

    /// How often to redraw for what moves, or never: nothing moves while the window is hidden.
    fn tick_every(&self) -> Option<Duration> {
        if self.hidden {
            return None;
        }
        let frame = if self.compact() { Duration::from_millis(33) } else { Duration::from_millis(16) };
        if screens::main_view::animating(self) || screens::viewer::animating(self) {
            return Some(frame);
        }
        let waves = self.app_levels.moving(self.now);
        let spinning = screens::spinning(self);
        if waves || spinning || !self.live.fresh.is_empty() {
            return Some(Duration::from_millis(50));
        }
        if !self.toasts.is_empty() || self.flash.is_some() {
            return Some(Duration::from_millis(100));
        }
        None
    }

    fn theme(&self) -> Theme {
        let p = p();
        Theme::custom(
            "kamon",
            iced::theme::Palette {
                background: p.surface,
                text: p.ink,
                primary: p.accent,
                success: p.success,
                warning: p.warning,
                danger: p.danger,
            },
        )
    }

    pub fn view(&self) -> Element<'_, Message> {
        let body: Element<'_, Message> = if self.needs_wizard() {
            screens::wizard::view(self)
        } else {
            match self.view {
                View::Main => screens::main_view::view(self),
                View::Settings => screens::settings::view(self),
                View::History => screens::history::view(self),
                View::Session => screens::viewer::view(self),
            }
        };
        let page = column![screens::titlebar::view(self), container(body).height(Length::Fill)];
        let mut layers: Vec<Element<'_, Message>> = vec![page.into()];
        if self.dropping {
            layers.push(screens::notices::drop_overlay());
        }
        layers.push(screens::notices::toasts(self));
        if let Some(d) = screens::dialogs::view(self) {
            layers.push(d);
        }
        let flash = self.flash.is_some();
        container(stack(layers).width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| {
                let p = p();
                container::Style {
                    background: Some(p.surface.into()),
                    text_color: Some(p.ink),
                    border: Border {
                        color: if flash { p.accent } else { p.border },
                        width: if flash { 2.0 } else { 1.0 },
                        radius: 0.0.into(),
                    },
                    ..container::Style::default()
                }
            })
            .into()
    }
}

/// An empty element, for a part with nothing to show.
pub fn nothing<'a>() -> Element<'a, Message> {
    Space::new().into()
}

/// A path for a screenshot of a session (`images/…` under its folder).
pub fn session_file(folder: &std::path::Path, rel: &str) -> PathBuf {
    folder.join(rel)
}

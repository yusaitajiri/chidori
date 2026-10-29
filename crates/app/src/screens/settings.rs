//! 設定 (section 15): a list of its sections, each showing what it is set to now; a section opens
//! with a back arrow in the title bar (DECISIONS → UI, "Settings is a list"). A change saves at
//! once, except the word list and the title format, which save when you leave them, and the
//! sliders, which save when let go. Also the shortcut recorder the wizard uses too.

use chidori_core::export::LabelMode;
use chidori_core::ops;
use chidori_core::session::model::SourceId;
use chidori_core::settings::{Hotkeys, Language, Layout, Locale, ScreenshotTarget, Settings};
use iced::keyboard::{self, key::Physical};
use iced::widget::{Column, Space, button, column, container, row, slider, text_editor};
use iced::{Alignment, Background, Border, Element, Length, Padding, Subscription, Task, mouse};
use serde_json::{Value, json};

use super::form::{self, Opt, choose, opt};
use crate::actions::with_parent;
use crate::app::{App, Message, SettingsTab, call};
use crate::i18n::{t, tf};
use crate::icons::{Icon, icon};
use crate::theme::{self, line, p, tokens};
use crate::widgets::{Size, Variant, btn, field, hairline, input_style, section, toggle_row};

/// The sections in the order the list shows them.
pub const TABS: [(SettingsTab, &str); 7] = [
    (SettingsTab::General, "tabGeneral"),
    (SettingsTab::Audio, "tabAudio"),
    (SettingsTab::Transcription, "tabTranscription"),
    (SettingsTab::Models, "tabModels"),
    (SettingsTab::Screenshots, "tabScreenshots"),
    (SettingsTab::Hotkeys, "tabHotkeys"),
    (SettingsTab::Output, "tabOutput"),
];

fn tab_key(tab: SettingsTab) -> &'static str {
    TABS.iter().find(|(t, _)| *t == tab).map_or("settingsTitle", |(_, k)| k)
}

/// The global shortcuts (FR-01, FR-06, FR-07, FR-30).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hotkey {
    Toggle,
    Screenshot,
    Mark,
    Cut,
}

impl Hotkey {
    pub const ALL: [Hotkey; 4] = [Hotkey::Toggle, Hotkey::Screenshot, Hotkey::Mark, Hotkey::Cut];

    /// Its key in the settings and in `hotkey_errors`.
    pub fn key(self) -> &'static str {
        match self {
            Hotkey::Toggle => "toggle",
            Hotkey::Screenshot => "screenshot",
            Hotkey::Mark => "mark",
            Hotkey::Cut => "cut",
        }
    }

    fn label_key(self) -> &'static str {
        match self {
            Hotkey::Toggle => "hotkeyToggle",
            Hotkey::Screenshot => "hotkeyScreenshot",
            Hotkey::Mark => "hotkeyMark",
            Hotkey::Cut => "hotkeyCut",
        }
    }

    fn value(self, h: &Hotkeys) -> &str {
        match self {
            Hotkey::Toggle => &h.toggle,
            Hotkey::Screenshot => &h.screenshot,
            Hotkey::Mark => &h.mark,
            Hotkey::Cut => &h.cut,
        }
    }
}

/// A key pressed while a shortcut is being recorded: its physical key by name (`KeyR`, `F9`,
/// `Escape`) and the modifiers held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPress {
    pub code: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
}

/// Turns a key press into an accelerator such as `Ctrl+Alt+R`, or nothing for a press that can't
/// be one (a modifier alone, a key with no name).
pub fn accelerator_from(k: &KeyPress) -> Option<String> {
    let code = k.code.as_str();
    let fkey = code.len() >= 2 && code.starts_with('F') && code[1..].chars().all(|c| c.is_ascii_digit());
    let key = if let Some(letter) =
        code.strip_prefix("Key").filter(|l| l.len() == 1 && l.chars().all(|c| c.is_ascii_uppercase()))
    {
        letter.to_string()
    } else if let Some(digit) =
        code.strip_prefix("Digit").filter(|d| d.len() == 1 && d.chars().all(|c| c.is_ascii_digit()))
    {
        digit.to_string()
    } else if fkey || ["Space", "Insert", "Home", "End", "PageUp", "PageDown", "Pause"].contains(&code) {
        code.to_string()
    } else {
        return None;
    };
    // A bare letter would fire while typing anywhere: Ctrl or Alt is needed (F-keys may stand alone).
    if !fkey && !k.ctrl && !k.alt {
        return None;
    }
    let mods = [(k.ctrl, "Ctrl"), (k.alt, "Alt"), (k.shift, "Shift"), (k.logo, "Super")];
    let mut parts: Vec<&str> = mods.iter().filter(|(on, _)| *on).map(|(_, m)| *m).collect();
    parts.push(&key);
    Some(parts.join("+"))
}

/// The two sliders under 音声 › 詳細設定.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slider {
    Threshold,
    Hangover,
}

#[derive(Debug, Default)]
pub struct State {
    /// The shortcut waiting for keys, if any.
    pub recording: Option<Hotkey>,
    advanced: bool,
    vocab: text_editor::Content,
    vocab_dirty: bool,
    template: Option<String>,
    template_dirty: bool,
    /// A slider's value while it is moved; kept after it is let go so it doesn't jump back while
    /// the save goes through.
    threshold: Option<f32>,
    hangover: Option<f32>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Open(SettingsTab),
    Save(Value),
    MicDevice(Option<String>),
    ToggleAdvanced,
    Slide(Slider, f32),
    SlideDone(Slider),
    Vocab(text_editor::Action),
    Template(String),
    /// Enter in the title format, or a click anywhere: what was being typed is saved.
    Leave,
    PickFolder,
    OpenLogs,
    Record(Hotkey),
    ClearHotkey(Hotkey),
    Key(KeyPress),
}

fn save(partial: Value) -> Task<Message> {
    Task::done(Message::UpdateSettings(partial))
}

fn msg(m: Msg) -> Message {
    Message::SettingsUi(m)
}

/// One word a line, blanks dropped.
pub fn vocabulary_of(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
}

/// Saves what is being typed, once it changed.
fn leave(app: &mut App) -> Task<Message> {
    let s = &mut app.settings_ui;
    let mut tasks = Vec::new();
    if s.vocab_dirty {
        s.vocab_dirty = false;
        tasks.push(save(json!({ "vocabulary": vocabulary_of(&s.vocab.text()) })));
    }
    if s.template_dirty {
        s.template_dirty = false;
        if let Some(template) = &s.template {
            tasks.push(save(json!({ "output": { "titleTemplate": template } })));
        }
    }
    Task::batch(tasks)
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    match m {
        Msg::Open(tab) => {
            let flush = leave(app);
            let s = &mut app.settings_ui;
            s.recording = None;
            s.advanced = false;
            s.vocab = text_editor::Content::with_text(&app.settings.vocabulary.join("\n"));
            s.template = None;
            s.threshold = None;
            s.hangover = None;
            let devices = if tab == SettingsTab::Audio { app.refresh_devices() } else { Task::none() };
            Task::batch([flush, devices, Task::done(Message::GoSettings(tab))])
        }
        Msg::Save(partial) => save(partial),
        Msg::MicDevice(id) => {
            app.source.mic_device_id = id.clone();
            save(json!({ "source": { "micDeviceId": id } }))
        }
        Msg::ToggleAdvanced => {
            app.settings_ui.advanced = !app.settings_ui.advanced;
            Task::none()
        }
        Msg::Slide(which, v) => {
            match which {
                Slider::Threshold => app.settings_ui.threshold = Some(v),
                Slider::Hangover => app.settings_ui.hangover = Some(v),
            }
            Task::none()
        }
        Msg::SlideDone(which) => match which {
            Slider::Threshold => app.settings_ui.threshold.map_or(Task::none(), |v| {
                save(json!({ "vad": { "startThreshold": (v as f64 * 100.0).round() / 100.0 } }))
            }),
            Slider::Hangover => app
                .settings_ui
                .hangover
                .map_or(Task::none(), |v| save(json!({ "vad": { "hangoverMs": v.round() as u32 } }))),
        },
        Msg::Vocab(action) => {
            let s = &mut app.settings_ui;
            s.vocab_dirty |= action.is_edit();
            s.vocab.perform(action);
            Task::none()
        }
        Msg::Template(v) => {
            app.settings_ui.template = Some(v);
            app.settings_ui.template_dirty = true;
            Task::none()
        }
        Msg::Leave => {
            // A click anywhere also ends recording a shortcut, as leaving its button does.
            app.settings_ui.recording = None;
            leave(app)
        }
        Msg::PickFolder => {
            let parent = app.hwnd;
            call(
                app,
                move |_| with_parent(rfd::FileDialog::new(), parent).pick_folder(),
                |folder| {
                    folder.map_or(Message::Noop, |f| {
                        Message::UpdateSettings(json!({ "output": { "root": f.to_string_lossy() } }))
                    })
                },
            )
        }
        Msg::OpenLogs => {
            let dir = app.info.log_dir.clone();
            call(
                app,
                move |c| {
                    if ops::allowed_path(c, &dir)
                        && let Err(e) = open::that_detached(&dir)
                    {
                        tracing::warn!("opening the logs: {e}");
                    }
                },
                |_| Message::Noop,
            )
        }
        Msg::Record(which) => {
            app.settings_ui.recording = Some(which);
            Task::none()
        }
        Msg::ClearHotkey(which) => save(json!({ "hotkeys": { which.key(): "" } })),
        Msg::Key(k) => {
            let Some(which) = app.settings_ui.recording else { return Task::none() };
            if k.code == "Escape" {
                app.settings_ui.recording = None;
                return Task::none();
            }
            match accelerator_from(&k) {
                Some(acc) => {
                    app.settings_ui.recording = None;
                    save(json!({ "hotkeys": { which.key(): acc } }))
                }
                None => Task::none(),
            }
        }
    }
}

pub fn on_menu(_app: &mut App, _rest: &str) -> Task<Message> {
    Task::none()
}

pub fn on_levels(_app: &mut App, _levels: &[(SourceId, f32)]) {}

pub fn spinning(_app: &App) -> bool {
    false
}

/// Keys while a shortcut is being recorded, and clicks while one is recorded or text waits to be
/// saved (a click elsewhere is leaving it).
pub fn subscription(app: &App) -> Subscription<Message> {
    let s = &app.settings_ui;
    if s.recording.is_none() && !s.vocab_dirty && !s.template_dirty {
        return Subscription::none();
    }
    iced::event::listen_with(|event, _, _| match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            physical_key: Physical::Code(code),
            modifiers,
            repeat: false,
            ..
        }) => Some(msg(Msg::Key(KeyPress {
            code: format!("{code:?}"),
            ctrl: modifiers.control(),
            alt: modifiers.alt(),
            shift: modifiers.shift(),
            logo: modifiers.logo(),
        }))),
        // Comes before the release that presses a button, so recording another shortcut still works.
        iced::Event::Mouse(mouse::Event::ButtonPressed(_)) => Some(msg(Msg::Leave)),
        _ => None,
    })
}

pub fn view(app: &App) -> Element<'_, Message> {
    let tab = app.settings_tab;
    if tab == SettingsTab::Index {
        return index(app);
    }
    let body: Element<'_, Message> = match tab {
        SettingsTab::Models => super::models::list(app),
        _ => {
            let groups = match tab {
                SettingsTab::General => general(app),
                SettingsTab::Audio => audio(app),
                SettingsTab::Transcription => transcription(app),
                SettingsTab::Screenshots => screenshots(app),
                SettingsTab::Hotkeys => hotkeys(app),
                _ => output(app),
            };
            container(Column::with_children(groups).spacing(tokens::SPACE_3))
                .padding(Padding::new(14.0).top(tokens::SPACE_3).bottom(tokens::SPACE_4))
                .into()
        }
    };
    column![form::head(t(tab_key(tab)), Some(t("settingsTitle"))), hairline(), form::scroll(body)].into()
}

fn language_key(l: Language) -> &'static str {
    match l {
        Language::Ja => "langJa",
        Language::En => "langEn",
        Language::Auto => "langAuto",
    }
}

fn target_key(target: ScreenshotTarget) -> &'static str {
    match target {
        ScreenshotTarget::CursorMonitor => "targetCursor",
        ScreenshotTarget::AllMonitors => "targetAll",
        ScreenshotTarget::AppWindow => "targetApp",
    }
}

/// The last folders of a path: `Documents › Chidori`.
pub fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.split(['\\', '/']).filter(|p| !p.is_empty()).collect();
    parts[parts.len().saturating_sub(2)..].join(" › ")
}

/// Whether any shortcut failed to register.
fn hotkeys_taken(app: &App) -> bool {
    !app.info.hotkey_errors.is_empty()
}

/// What a section is set to now, for its row in the list.
pub fn summary(app: &App, tab: SettingsTab) -> String {
    let s: &Settings = &app.settings;
    match tab {
        SettingsTab::General => if s.locale == Locale::Ja { "日本語" } else { "English" }.to_string(),
        SettingsTab::Audio => {
            let mic = s
                .source
                .mic_device_id
                .as_ref()
                .and_then(|id| app.source.devices.iter().find(|d| &d.id == id))
                .map_or_else(|| t("defaultDevice"), |d| d.name.clone());
            let mut parts = vec![mic];
            if s.echo_guard {
                parts.push(t("echoGuard"));
            }
            parts.join(" · ")
        }
        SettingsTab::Transcription => {
            let device = if s.use_gpu && app.info.gpu_compiled { t("gpu") } else { t("cpu") };
            format!("{} · {device}", t(language_key(s.language)))
        }
        SettingsTab::Models => {
            super::models::model_name(&app.models, &s.model_id).map_or_else(|| s.model_id.clone(), |n| n.full)
        }
        SettingsTab::Screenshots => t(target_key(s.screenshot.target)),
        SettingsTab::Hotkeys => {
            if hotkeys_taken(app) {
                return t("hotkeyTaken");
            }
            let set: Vec<&str> = Hotkey::ALL.iter().map(|h| h.value(&s.hotkeys)).filter(|v| !v.is_empty()).collect();
            if set.is_empty() { t("hotkeyOff") } else { set.join(" · ") }
        }
        SettingsTab::Output => short_path(&s.output.root),
        SettingsTab::Index => String::new(),
    }
}

/// The list: each section, what it is set to, and a chevron; the version and the logs below.
fn index(app: &App) -> Element<'_, Message> {
    let mut rows = Column::new().push(hairline());
    for (tab, key) in TABS {
        let attention = tab == SettingsTab::Hotkeys && hotkeys_taken(app);
        let value = form::fit(&summary(app, tab), 210.0, tokens::CAPTION.size);
        let value = line(value, tokens::CAPTION, if attention { p().ink } else { p().ink_muted }).font(if attention {
            theme::SANS_MEDIUM
        } else {
            theme::SANS
        });
        let r = row![
            container(line(t(key), tokens::LABEL, p().ink)).width(Length::Fill),
            value,
            icon(Icon::ChevronRight, 14.0, p().ink_muted),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .padding([tokens::SPACE_3, 6.0]);
        let b =
            button(r).width(Length::Fill).padding(0).on_press(msg(Msg::Open(tab))).style(|_, status| button::Style {
                background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                    .then_some(Background::Color(p().surface_sunken)),
                ..button::Style::default()
            });
        rows = rows.push(b).push(hairline());
    }
    let foot = row![
        line(tf("version", &[("v", &app.info.version)]), tokens::CAPTION, p().ink_muted),
        Space::new().width(Length::Fill),
        form::link(theme::plain(t("openLogs"), tokens::CAPTION).font(theme::SANS_MEDIUM), msg(Msg::OpenLogs)),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::new(6.0).top(tokens::SPACE_3).bottom(0));
    column![
        form::head(t("settingsTitle"), None),
        form::scroll(container(column![rows, foot]).padding(Padding::new(14.0).top(0).bottom(tokens::SPACE_4)))
    ]
    .into()
}

/// A labelled dropdown.
fn pick<'a, T: Clone + PartialEq + 'a>(
    label: &str,
    options: Vec<Opt<T>>,
    value: &T,
    on: impl Fn(T) -> Message + 'a,
) -> Element<'a, Message> {
    field(t(label), choose(options, value, on))
}

/// A switch that saves at once.
fn switch<'a>(label: &str, on: bool, partial: impl Fn(bool) -> Value) -> Element<'a, Message> {
    toggle_row(t(label), on, Some(msg(Msg::Save(partial(!on)))))
}

fn general(app: &App) -> Vec<Element<'_, Message>> {
    let s = &app.settings;
    vec![section(
        None,
        vec![
            pick("language", vec![opt(Locale::Ja, "日本語"), opt(Locale::En, "English")], &s.locale, |v| {
                msg(Msg::Save(json!({ "locale": v })))
            }),
            pick(
                "layout",
                vec![opt(Layout::Compact, t("layoutCompact")), opt(Layout::Expanded, t("layoutExpanded"))],
                &s.window.layout,
                |v| msg(Msg::Save(json!({ "window": { "layout": v } }))),
            ),
            switch("checkUpdates", s.updates.check, |v| json!({ "updates": { "check": v } })),
        ],
    )]
}

/// 詳細設定: closed at first, under a section's card.
fn advanced<'a>(open: bool, rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let summary = button(
        row![
            container(theme::plain(t("advanced"), tokens::CAPTION).font(theme::SANS_MEDIUM)).width(Length::Fill),
            icon(if open { Icon::ChevronDown } else { Icon::ChevronRight }, 15.0, p().ink_muted),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([10.0, 0.0])
    .on_press(msg(Msg::ToggleAdvanced))
    .style(|_, status| button::Style {
        text_color: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            p().ink
        } else {
            p().ink_muted
        },
        ..button::Style::default()
    });
    let mut col = Column::new().push(summary);
    if open {
        col = col.push(hairline());
        let n = rows.len();
        for (i, r) in rows.into_iter().enumerate() {
            col = col.push(r);
            if i + 1 < n {
                col = col.push(hairline());
            }
        }
    }
    container(col)
        .width(Length::Fill)
        .padding([0.0, 14.0])
        .style(|_| container::Style {
            background: Some(p().surface_raised.into()),
            border: Border { color: p().border, width: 1.0, radius: tokens::RADIUS_LG.into() },
            ..container::Style::default()
        })
        .into()
}

fn slider_row<'a>(
    label: String,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    value: f32,
    which: Slider,
) -> Element<'a, Message> {
    let bar = slider(range, value, move |v| msg(Msg::Slide(which, v)))
        .step(step)
        .on_release(msg(Msg::SlideDone(which)))
        .style(|_, status| {
            let p = p();
            let active = matches!(status, slider::Status::Hovered | slider::Status::Dragged);
            slider::Style {
                rail: slider::Rail {
                    backgrounds: (p.accent.into(), p.border.into()),
                    width: 4.0,
                    border: Border { radius: 2.0.into(), ..Border::default() },
                },
                handle: slider::Handle {
                    shape: slider::HandleShape::Rectangle { width: 12, border_radius: 4.0.into() },
                    background: (if active { p.accent_hover } else { p.accent }).into(),
                    border_width: 2.0,
                    border_color: p.surface_raised,
                },
            }
        });
    field(label, bar)
}

fn audio(app: &App) -> Vec<Element<'_, Message>> {
    let s = &app.settings;
    let ui = &app.settings_ui;
    let mut mics = vec![opt(None, t("defaultDevice"))];
    mics.extend(app.source.devices.iter().map(|d| opt(Some(d.id.clone()), d.name.clone())));
    let threshold = ui.threshold.unwrap_or(s.vad.start_threshold);
    let hangover = ui.hangover.unwrap_or(s.vad.hangover_ms as f32);
    vec![
        section(
            None,
            vec![
                pick("micDevice", mics, &s.source.mic_device_id, |v| msg(Msg::MicDevice(v))),
                switch("echoGuard", s.echo_guard, |v| json!({ "echoGuard": v })),
                switch("keepAudio", s.audio.record, |v| json!({ "audio": { "record": v } })),
            ],
        ),
        advanced(
            ui.advanced,
            vec![
                slider_row(
                    format!("{}: {threshold:.2}", t("vadThreshold")),
                    0.3..=0.8,
                    0.05,
                    threshold,
                    Slider::Threshold,
                ),
                slider_row(
                    format!("{}: {}", t("hangover"), hangover.round()),
                    400.0..=1200.0,
                    40.0,
                    hangover,
                    Slider::Hangover,
                ),
            ],
        ),
    ]
}

fn transcription(app: &App) -> Vec<Element<'_, Message>> {
    let s = &app.settings;
    let gpu = app.info.gpu_compiled;
    let vocab = text_editor(&app.settings_ui.vocab)
        .on_action(|a| msg(Msg::Vocab(a)))
        .font(theme::SANS)
        .size(tokens::BODY_SMALL.size)
        .height(96)
        .padding([tokens::SPACE_2, 10.0])
        .style(|theme, status| {
            let input = input_style(
                theme,
                match status {
                    text_editor::Status::Focused { is_hovered } => {
                        iced::widget::text_input::Status::Focused { is_hovered }
                    }
                    text_editor::Status::Hovered => iced::widget::text_input::Status::Hovered,
                    text_editor::Status::Disabled => iced::widget::text_input::Status::Disabled,
                    text_editor::Status::Active => iced::widget::text_input::Status::Active,
                },
            );
            text_editor::Style {
                background: input.background,
                border: input.border,
                placeholder: input.placeholder,
                value: input.value,
                selection: input.selection,
            }
        });
    vec![
        section(
            None,
            vec![
                pick(
                    "transcriptionLanguage",
                    [Language::Ja, Language::En, Language::Auto]
                        .into_iter()
                        .map(|l| opt(l, t(language_key(l))))
                        .collect(),
                    &s.language,
                    |v| msg(Msg::Save(json!({ "language": v }))),
                ),
                // Only a GPU build can use the GPU.
                toggle_row(t("useGpu"), s.use_gpu, gpu.then(|| msg(Msg::Save(json!({ "useGpu": !s.use_gpu }))))),
                switch("accuracyFirst", s.accuracy_first, |v| json!({ "accuracyFirst": v })),
                switch("partials", s.partials, |v| json!({ "partials": v })),
                field(t("vocabulary"), vocab),
            ],
        ),
        advanced(
            app.settings_ui.advanced,
            vec![
                switch("hallucinationFilter", s.hallucination_filter, |v| json!({ "hallucinationFilter": v })),
                switch("audioCtx", s.audio_ctx_experimental, |v| json!({ "audioCtxExperimental": v })),
            ],
        ),
    ]
}

fn screenshots(app: &App) -> Vec<Element<'_, Message>> {
    let s = &app.settings;
    let targets = [ScreenshotTarget::AppWindow, ScreenshotTarget::CursorMonitor, ScreenshotTarget::AllMonitors];
    vec![section(
        None,
        vec![
            pick(
                "screenshotTarget",
                targets.into_iter().map(|v| opt(v, t(target_key(v)))).collect(),
                &s.screenshot.target,
                |v| msg(Msg::Save(json!({ "screenshot": { "target": v } }))),
            ),
            switch("excludeSelf", s.screenshot.exclude_self, |v| json!({ "screenshot": { "excludeSelf": v } })),
            switch("shutterSound", s.screenshot.sound, |v| json!({ "screenshot": { "sound": v } })),
        ],
    )]
}

fn hotkeys(app: &App) -> Vec<Element<'_, Message>> {
    vec![section(None, Hotkey::ALL.iter().map(|h| hotkey_input(app, *h)).collect())]
}

/// A shortcut: オフ until its button is pressed and then the keys; オフにする turns it off again.
/// Why Windows refused one shows under it.
pub fn hotkey_input(app: &App, which: Hotkey) -> Element<'_, Message> {
    let value = which.value(&app.settings.hotkeys);
    let recording = app.settings_ui.recording == Some(which);
    let set = !value.is_empty() && !recording;
    let shown = if recording {
        t("hotkeyRecord")
    } else if value.is_empty() {
        t("hotkeyOff")
    } else {
        value.to_string()
    };
    let text = line(shown, tokens::BODY_SMALL, if set || recording { p().ink } else { p().ink_muted }).font(if set {
        theme::MONO
    } else {
        theme::SANS
    });
    let keys = button(container(text).center_y(Length::Fill))
        .width(Length::Fill)
        .height(32)
        .padding([0.0, 10.0])
        .on_press(msg(Msg::Record(which)))
        .style(move |_, status| {
            let p = p();
            let border = if recording {
                p.accent
            } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                p.border_strong
            } else {
                p.border
            };
            button::Style {
                background: Some(p.surface_sunken.into()),
                text_color: p.ink,
                border: Border { color: border, width: 1.0, radius: tokens::RADIUS_MD.into() },
                ..button::Style::default()
            }
        });
    let mut r = row![keys].spacing(6).align_y(Alignment::Center);
    if set {
        r = r.push(btn(t("hotkeyClear"), Variant::Secondary, Size::Md, Some(msg(Msg::ClearHotkey(which)))));
    }
    let mut col = Column::new().push(r).spacing(tokens::SPACE_1);
    if let Some(error) = app.info.hotkey_errors.get(which.key()) {
        col = col.push(
            theme::t(tf("hotkeyFailed", &[("error", error)]), tokens::CAPTION, p().danger).font(theme::SANS_MEDIUM),
        );
    }
    field(t(which.label_key()), col)
}

fn output(app: &App) -> Vec<Element<'_, Message>> {
    let s = &app.settings;
    let root = iced::widget::text_input("", &s.output.root)
        .font(theme::SANS)
        .size(tokens::CAPTION.size)
        .padding([7.0, 10.0])
        .style(input_style);
    let folder = row![root, btn(t("change"), Variant::Secondary, Size::Md, Some(msg(Msg::PickFolder)))]
        .spacing(6)
        .align_y(Alignment::Center);
    let template = app.settings_ui.template.as_deref().unwrap_or(&s.output.title_template);
    let template = column![
        crate::widgets::input("", template, |v| msg(Msg::Template(v))).on_submit(msg(Msg::Leave)),
        line(tf("titleTemplateHelp", &[("app", &"{app}"), ("date", &"{date}")]), tokens::CAPTION, p().ink_muted),
    ]
    .spacing(6);
    let labels = [(LabelMode::Auto, "labelsAuto"), (LabelMode::On, "labelsOn"), (LabelMode::Off, "labelsOff")];
    vec![
        section(None, vec![field(t("outputRoot"), folder), field(t("titleTemplate"), template)]),
        section(
            None,
            vec![
                switch("timestamps", s.export.timestamps, |v| json!({ "export": { "timestamps": v } })),
                pick("labels", labels.into_iter().map(|(v, k)| opt(v, t(k))).collect(), &s.export.labels, |v| {
                    msg(Msg::Save(json!({ "export": { "labels": v } })))
                }),
                switch("mergeParagraphs", s.export.merge_paragraphs, |v| json!({ "export": { "mergeParagraphs": v } })),
                switch(
                    "screenshotMarkers",
                    s.copy.screenshot_markers,
                    |v| json!({ "copy": { "screenshotMarkers": v } }),
                ),
                switch("autoCopyOnStop", s.auto_copy_on_stop, |v| json!({ "autoCopyOnStop": v })),
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: &str, ctrl: bool, alt: bool, shift: bool) -> KeyPress {
        KeyPress { code: code.into(), ctrl, alt, shift, logo: false }
    }

    #[test]
    fn builds_accelerators() {
        assert_eq!(accelerator_from(&key("KeyR", true, true, false)).as_deref(), Some("Ctrl+Alt+R"));
        assert_eq!(accelerator_from(&key("Digit5", true, false, true)).as_deref(), Some("Ctrl+Shift+5"));
        assert_eq!(accelerator_from(&key("F9", false, false, false)).as_deref(), Some("F9"));
        assert_eq!(accelerator_from(&key("PageUp", false, true, false)).as_deref(), Some("Alt+PageUp"));
    }

    #[test]
    fn rejects_bare_letters_shift_only_and_modifiers_alone() {
        assert_eq!(accelerator_from(&key("KeyR", false, false, false)), None);
        assert_eq!(accelerator_from(&key("KeyR", false, false, true)), None);
        assert_eq!(accelerator_from(&key("ControlLeft", true, false, false)), None);
        assert_eq!(accelerator_from(&key("Escape", true, false, false)), None);
    }

    #[test]
    fn accelerators_register() {
        for k in [key("KeyK", true, true, false), key("F9", false, false, false), key("Space", true, false, true)] {
            let acc = accelerator_from(&k).unwrap();
            assert!(crate::native::hotkeys::validate(&acc).is_ok(), "{acc}");
        }
    }

    #[test]
    fn vocabulary_is_one_word_a_line() {
        assert_eq!(vocabulary_of(" Chidori \n\nwhisper.cpp\n  \n"), ["Chidori", "whisper.cpp"]);
    }

    #[test]
    fn paths_show_their_last_folders() {
        assert_eq!(short_path(r"C:\Users\me\Documents\Chidori"), "Documents › Chidori");
        assert_eq!(short_path("D:/Notes/"), "D: › Notes");
        assert_eq!(short_path(""), "");
    }

    #[test]
    fn the_list_says_what_each_section_is_set_to() {
        crate::i18n::set_locale(Locale::Ja);
        let (mut app, _dir) = crate::preview::app();
        app.source.devices =
            vec![chidori_core::audio::source::Device { id: "usb".into(), name: "USB Mic".into(), is_default: false }];
        app.settings.source.mic_device_id = Some("usb".into());
        app.settings.hotkeys.toggle = "Ctrl+Shift+R".into();
        app.settings.hotkeys.screenshot = "Ctrl+Shift+S".into();
        app.settings.output.root = r"C:\Users\me\Documents\Chidori".into();
        app.info.gpu_compiled = true;
        assert_eq!(summary(&app, SettingsTab::General), "日本語");
        assert_eq!(summary(&app, SettingsTab::Audio), "USB Mic · エコー対策");
        assert_eq!(summary(&app, SettingsTab::Transcription), "日本語 · GPU");
        assert_eq!(summary(&app, SettingsTab::Hotkeys), "Ctrl+Shift+R · Ctrl+Shift+S");
        assert_eq!(summary(&app, SettingsTab::Output), "Documents › Chidori");
        app.info.gpu_compiled = false;
        assert_eq!(summary(&app, SettingsTab::Transcription), "日本語 · CPU");
        app.info.hotkey_errors.insert("toggle".into(), "taken".into());
        assert_eq!(summary(&app, SettingsTab::Hotkeys), "ショートカットを登録できませんでした");
        app.settings.hotkeys = Hotkeys::default();
        app.info.hotkey_errors.clear();
        assert_eq!(summary(&app, SettingsTab::Hotkeys), "オフ");
    }

    #[test]
    fn recording_a_shortcut_saves_it_and_escape_leaves_it_alone() {
        let (mut app, _dir) = crate::preview::app();
        let _ = update(&mut app, Msg::Record(Hotkey::Toggle));
        assert_eq!(app.settings_ui.recording, Some(Hotkey::Toggle));
        // A bare letter is not taken; recording goes on.
        let _ = update(&mut app, Msg::Key(key("KeyK", false, false, false)));
        assert_eq!(app.settings_ui.recording, Some(Hotkey::Toggle));
        let _ = update(&mut app, Msg::Key(key("Escape", false, false, false)));
        assert_eq!(app.settings_ui.recording, None);
        let _ = update(&mut app, Msg::Record(Hotkey::Mark));
        let _ = update(&mut app, Msg::Key(key("KeyK", true, true, false)));
        assert_eq!(app.settings_ui.recording, None);
    }
}

#[cfg(test)]
mod previews {
    use chidori_core::events::DownloadPayload;
    use chidori_core::models::catalog;
    use chidori_core::models::manager::ModelEntry;
    use chidori_core::settings::BenchmarkRecord;

    use super::*;
    use crate::app::View;
    use crate::preview::{app, shot};

    pub fn catalog_entries() -> Vec<ModelEntry> {
        catalog::all()
            .iter()
            .map(|e| ModelEntry {
                entry: e.clone(),
                installed: false,
                selected: false,
                recommended: e.id == "turbo-q5",
                partial_bytes: 0,
                downloading: false,
                benchmark: None,
            })
            .collect()
    }

    fn at(tab: SettingsTab) -> (App, tempfile::TempDir) {
        let (mut app, dir) = app();
        app.view = View::Settings;
        app.settings_tab = tab;
        app.settings_ui.vocab = text_editor::Content::with_text(&app.settings.vocabulary.join("\n"));
        (app, dir)
    }

    #[test]
    #[ignore = "writes PNGs for review"]
    fn preview_settings() {
        let (mut app, _dir) = at(SettingsTab::Index);
        app.settings.hotkeys.toggle = "Ctrl+Shift+R".into();
        app.settings.output.root = r"C:\Users\me\Documents\Chidori".into();
        app.models = catalog_entries();
        shot(&app, "settings-index", crate::app::EXPANDED);

        let (app, _dir) = at(SettingsTab::General);
        shot(&app, "settings-general", crate::app::EXPANDED);

        let (mut app, _dir) = at(SettingsTab::Audio);
        app.settings_ui.advanced = true;
        shot(&app, "settings-audio", crate::app::EXPANDED);

        let (mut app, _dir) = at(SettingsTab::Transcription);
        app.settings_ui.vocab = text_editor::Content::with_text("Chidori\nwhisper.cpp");
        shot(&app, "settings-transcription", crate::app::EXPANDED);

        let (app, _dir) = at(SettingsTab::Screenshots);
        shot(&app, "settings-screenshots", crate::app::EXPANDED);

        let (mut app, _dir) = at(SettingsTab::Hotkeys);
        app.settings.hotkeys.toggle = "Ctrl+Shift+R".into();
        app.settings.hotkeys.screenshot = "Ctrl+Shift+S".into();
        app.settings_ui.recording = Some(Hotkey::Mark);
        app.info.hotkey_errors.insert("screenshot".into(), "Ctrl+Shift+S is taken".into());
        shot(&app, "settings-hotkeys", crate::app::EXPANDED);

        let (mut app, _dir) = at(SettingsTab::Output);
        app.settings.output.root = r"C:\Users\me\Documents\Chidori".into();
        shot(&app, "settings-output", crate::app::EXPANDED);

        let (mut app, _dir) = at(SettingsTab::Models);
        let mut models = catalog_entries();
        models[0].installed = true;
        models[0].selected = true;
        models[0].benchmark = Some(BenchmarkRecord { seconds: 2.5, tier: "comfortable".into(), gpu: true });
        let small = models.iter().position(|m| m.entry.id == "small-q5").unwrap();
        models[small].downloading = true;
        app.models_ui.progress.insert(
            "small-q5".into(),
            DownloadPayload {
                id: "small-q5".into(),
                received: models[small].entry.size_bytes / 3,
                total: models[small].entry.size_bytes,
                bytes_per_sec: 4.2e6,
                phase: "downloading",
                error: None,
            },
        );
        app.models = models.clone();
        shot(&app, "settings-models", crate::app::EXPANDED);
    }
}

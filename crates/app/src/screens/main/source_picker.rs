//! What to listen to (FR-10 to FR-13): an app, everything the PC plays, or the mic alone, and
//! whether it is a conversation, when the mic records your voice too and the transcript names the
//! lines 相手 and 自分. Before a recording it is the start screen's two fields (the compact window
//! shows the choice as one line opening the picker over the window); while recording it is the
//! header's dropdown, where a pick switches the source at once (FR-17).

use chidori_core::audio::source::AudioApp;
use chidori_core::settings::SourceMode;
use iced::widget::{Column, Space, button, column, container, image, row, scrollable};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Task};

use super::popover::popover;
use super::{Msg, app_wave};
use crate::app::{App, Message, call};
use crate::i18n::t;
use crate::icons::{Icon, icon};
use crate::theme::{self, alpha, line, p, tokens};
use crate::widgets::{Size, floating, icon_button, spinner, with_tip};

/// Which list is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Open {
    #[default]
    None,
    Source,
    Mic,
    /// The recording header's dropdown.
    Switch,
}

#[derive(Debug, Default)]
pub struct PickerState {
    pub open: Open,
    /// The compact window's picker over the whole window.
    pub panel: bool,
}

#[derive(Debug, Clone)]
pub enum PickMsg {
    Toggle(Open),
    Close,
    Pick(SourceMode, Option<u32>),
    Refresh,
    Refreshed,
    Conversation,
    Mic(Option<String>),
    Panel(bool),
}

fn m(msg: PickMsg) -> Message {
    Message::Main(Msg::Pick(msg))
}

/// The apps whose sound is on screen: the chosen app's, and while a list is open, every app with
/// an audio session in it.
pub fn shown_pids(app: &App) -> Vec<u32> {
    let s = &app.source;
    let mut pids = Vec::new();
    if s.mode == SourceMode::App
        && let Some(a) = &s.app
        && a.root_pid != 0
    {
        pids.push(a.root_pid);
    }
    if matches!(app.main.picker.open, Open::Source | Open::Switch) {
        pids.extend(s.apps.iter().filter(|a| a.has_session).map(|a| a.root_pid));
    }
    pids
}

pub fn update(app: &mut App, msg: PickMsg) -> Task<Message> {
    match msg {
        PickMsg::Toggle(which) => {
            let st = &mut app.main.picker;
            if st.open == which {
                st.open = Open::None;
                return Task::none();
            }
            st.open = which;
            match which {
                Open::Mic => app.refresh_devices(),
                Open::None => Task::none(),
                _ => refresh(app),
            }
        }
        PickMsg::Close => {
            app.main.picker.open = Open::None;
            Task::none()
        }
        PickMsg::Pick(mode, pid) => {
            if let Some(pid) = pid
                && let Some(a) = app.source.apps.iter().find(|a| a.root_pid == pid).cloned()
            {
                app.source.set_app(&a);
            }
            app.source.mode = mode;
            app.main.picker.open = Open::None;
            super::source_changed(app)
        }
        PickMsg::Refresh => refresh(app),
        PickMsg::Refreshed => Task::none(),
        PickMsg::Conversation => {
            app.source.include_mic = !app.source.include_mic;
            super::source_changed(app)
        }
        PickMsg::Mic(id) => {
            app.source.mic_device_id = id.clone();
            app.main.picker.open = Open::None;
            Task::batch([
                Task::done(Message::UpdateSettings(serde_json::json!({ "source": { "micDeviceId": id } }))),
                super::source_changed(app),
            ])
        }
        PickMsg::Panel(open) => {
            app.main.picker.panel = open;
            app.main.picker.open = Open::None;
            if open { refresh(app) } else { Task::none() }
        }
    }
}

/// A fresh app list; the waves follow it.
pub fn refresh(app: &mut App) -> Task<Message> {
    app.refresh_apps().chain(Task::done(m(PickMsg::Refreshed)))
}

/// Tells the core which apps' sound to read when that changed.
pub fn sync_watch(app: &mut App, pids: Vec<u32>) -> Task<Message> {
    match app.app_levels.watch(pids, app.now) {
        Some(pids) => call(app, move |c| chidori_core::ops::watch_app_levels(c, pids), |_| Message::Noop),
        None => Task::none(),
    }
}

/// An app's icon, or the window glyph when it has none.
fn app_icon<'a>(app: &App, exe: Option<&str>, size: f32) -> Element<'a, Message> {
    match exe.and_then(|e| app.source.icon_of(e)) {
        Some(h) => image(h.clone()).width(size).height(size).into(),
        None => icon(Icon::AppWindow, size - 2.0, p().ink_muted).into(),
    }
}

fn hovered(s: button::Status) -> bool {
    matches!(s, button::Status::Hovered | button::Status::Pressed)
}

/// A field: a box on the raised ground, ink-edged while its list is open.
fn field_style(open: bool) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let p = p();
        button::Style {
            background: Some(p.surface_raised.into()),
            text_color: p.ink,
            border: Border {
                color: if open {
                    p.ink
                } else if hovered(status) {
                    p.border_strong
                } else {
                    p.border
                },
                width: 1.0,
                radius: tokens::RADIUS_MD.into(),
            },
            ..button::Style::default()
        }
    }
}

/// How a switch looks, inside a control that is a switch as a whole: a squared track.
fn switch_mark<'a>(on: bool) -> Element<'a, Message> {
    let knob = container(Space::new()).width(16).height(16).style(|_| container::Style {
        background: Some(p().surface_raised.into()),
        border: Border { radius: 4.0.into(), ..Border::default() },
        ..container::Style::default()
    });
    container(knob)
        .width(36)
        .height(20)
        .padding(2)
        .align_x(if on { Alignment::End } else { Alignment::Start })
        .style(move |_| container::Style {
            background: Some(if on { p().accent } else { p().border_strong }.into()),
            border: Border { radius: tokens::RADIUS_SM.into(), ..Border::default() },
            ..container::Style::default()
        })
        .into()
}

/// One choice in a list: its icon, name, anything extra (the app's wave) and a check, kept in
/// every row so the waves line up.
fn option<'a>(
    selected: bool,
    enabled: bool,
    glyph: Element<'a, Message>,
    label: String,
    extra: Option<Element<'a, Message>>,
    on: Message,
) -> Element<'a, Message> {
    let font = if selected { theme::SANS_MEDIUM } else { theme::SANS };
    let mut r = row![
        container(glyph).center(20),
        container(line(label, tokens::BODY_SMALL, p().ink).font(font)).width(Length::Fill).clip(true),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if let Some(e) = extra {
        r = r.push(e);
    }
    let check: Element<'a, Message> =
        if selected { icon(Icon::Check, 14.0, p().ink).into() } else { Space::new().width(14).height(14).into() };
    r = r.push(check);
    let b = button(r).width(Length::Fill).padding([8, 10]).style(move |_, status| {
        let p = p();
        button::Style {
            background: hovered(status).then_some(Background::Color(p.surface_sunken)),
            text_color: p.ink,
            border: Border { radius: tokens::RADIUS_MD.into(), ..Border::default() },
            ..button::Style::default()
        }
        .with_alpha(if enabled { 1.0 } else { 0.45 })
    });
    if enabled { b.on_press(on).into() } else { b.into() }
}

trait Faded {
    fn with_alpha(self, a: f32) -> Self;
}

impl Faded for button::Style {
    fn with_alpha(mut self, a: f32) -> Self {
        self.text_color = alpha(self.text_color, self.text_color.a * a);
        self
    }
}

fn group<'a>(label: String, action: Option<Element<'a, Message>>) -> Element<'a, Message> {
    let mut r =
        row![container(line(label, tokens::CAPTION, p().ink_muted).font(theme::SANS_MEDIUM)).width(Length::Fill)]
            .align_y(Alignment::Center);
    if let Some(a) = action {
        r = r.push(a);
    }
    container(r).padding(Padding::new(10.0).top(8).bottom(2)).into()
}

/// The sources to pick from: システム全体, マイクだけ, then the apps, those playing first (FR-11).
fn options<'a>(app: &'a App) -> Vec<Element<'a, Message>> {
    let s = &app.source;
    let supported = app.info.app_loopback_supported;
    let mut list = vec![
        option(
            s.mode == SourceMode::System,
            true,
            icon(Icon::MonitorSpeaker, 18.0, p().ink).into(),
            t("srcSystem"),
            None,
            m(PickMsg::Pick(SourceMode::System, None)),
        ),
        option(
            s.mode == SourceMode::Mic,
            true,
            icon(Icon::Mic, 18.0, p().ink).into(),
            t("srcMicOnly"),
            None,
            m(PickMsg::Pick(SourceMode::Mic, None)),
        ),
    ];
    let refresh: Element<'a, Message> = if s.loading_apps {
        container(spinner(13.0, app.now)).center(24).into()
    } else {
        icon_button(Icon::RefreshCw, t("refresh"), Size::Sm, Some(m(PickMsg::Refresh)))
    };
    list.push(group(t("srcApps"), Some(refresh)));
    let app_option = |a: &'a AudioApp| {
        let o = option(
            s.is_chosen(a),
            supported,
            app_icon(app, Some(&a.exe), 20.0),
            a.name.clone(),
            a.has_session.then(|| app_wave::wave(app, Some(a.root_pid), 30.0, 14.0)),
            m(PickMsg::Pick(SourceMode::App, Some(a.root_pid))),
        );
        if supported { o } else { with_tip(o, t("win11Required")) }
    };
    list.extend(s.apps.iter().filter(|a| a.has_session).map(app_option));
    if s.apps.iter().any(|a| !a.has_session) {
        list.push(group(t("otherApps"), None));
    }
    list.extend(s.apps.iter().filter(|a| !a.has_session).map(app_option));
    if s.apps.is_empty() {
        let note = if s.loading_apps { t("loading") } else { t("noApps") };
        list.push(
            container(line(note, tokens::CAPTION, p().ink_muted)).center_x(Length::Fill).padding([12, 10]).into(),
        );
    }
    list
}

/// The floating sheet a list sits on.
fn sheet<'a>(items: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    container(scrollable(Column::with_children(items).padding(4)).height(Length::Shrink))
        .style(|_| floating())
        .width(Length::Fill)
        .into()
}

/// The chosen source's icon and name.
fn chosen<'a>(app: &App, dense: bool) -> (Element<'a, Message>, String, bool) {
    let s = &app.source;
    let size = if dense { 17.0 } else { 20.0 };
    match s.mode {
        SourceMode::App => (
            app_icon(app, s.app.as_ref().map(|a| a.exe.as_str()), if dense { 18.0 } else { 22.0 }),
            s.app.as_ref().map_or_else(|| t("chooseApp"), |a| a.name.clone()),
            s.app.is_none(),
        ),
        SourceMode::System => (icon(Icon::MonitorSpeaker, size, p().ink).into(), t("srcSystem"), false),
        SourceMode::Mic => (icon(Icon::Mic, size, p().ink).into(), t("srcMicOnly"), false),
    }
}

fn chevron<'a>(open: bool) -> Element<'a, Message> {
    icon(Icon::ChevronDown, 15.0, p().ink_muted)
        .rotation(iced::Radians(if open { std::f32::consts::PI } else { 0.0 }))
        .into()
}

/// What 相手 is: an app, everything the PC plays, or nothing but the mic (FR-10, FR-11).
fn source_field<'a>(app: &'a App, dense: bool) -> Element<'a, Message> {
    let open = app.main.picker.open == Open::Source;
    let (glyph, name, missing) = chosen(app, dense);
    let mut r = row![
        container(glyph).center(22),
        container(
            line(name, tokens::BODY_SMALL, if missing { p().ink_muted } else { p().ink }).font(theme::SANS_MEDIUM)
        )
        .width(Length::Fill)
        .clip(true),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if app.source.mode == SourceMode::App {
        let pid = app.source.app.as_ref().map(|a| a.root_pid);
        r = r.push(if dense { app_wave::wave(app, pid, 30.0, 14.0) } else { app_wave::wave(app, pid, 36.0, 16.0) });
    }
    r = r.push(chevron(open));
    let anchor = button(container(r).center_y(Length::Fill))
        .width(Length::Fill)
        .height(if dense { 40 } else { 48 })
        .padding([0, 12])
        .style(field_style(open))
        .on_press(m(PickMsg::Toggle(Open::Source)));
    popover(anchor, open.then(|| sheet(options(app))), m(PickMsg::Close)).into()
}

/// Whether to record a conversation (「会話として録音」, FR-12), or with the mic as the only
/// source, just which mic it is (FR-13).
fn mic_field<'a>(app: &'a App, dense: bool, only: bool) -> Element<'a, Message> {
    let s = &app.source;
    let open = app.main.picker.open == Open::Mic;
    let on = only || s.include_mic;
    let tone = if on { p().ink } else { p().ink_muted };
    let device = s.mic_name().filter(|_| s.mic_device_id.is_some()).unwrap_or_else(|| t("defaultDevice"));
    let mut labels = Column::new().push(
        line(if only { t("srcMic") } else { t("conversation") }, tokens::BODY_SMALL, tone).font(theme::SANS_MEDIUM),
    );
    if !dense {
        labels = labels.push(line(device, tokens::CAPTION, p().ink_muted));
    }
    let mut body =
        row![icon(Icon::Mic, if dense { 17.0 } else { 19.0 }, tone), container(labels).width(Length::Fill).clip(true)]
            .spacing(10)
            .align_y(Alignment::Center);
    let main: Element<'a, Message> = if only {
        container(body).width(Length::Fill).into()
    } else {
        body = body.push(switch_mark(s.include_mic));
        button(body)
            .width(Length::Fill)
            .padding(0)
            .style(|_, _| button::Style::default())
            .on_press(m(PickMsg::Conversation))
            .into()
    };
    let r = row![main, icon_button(Icon::ChevronDown, t("chooseMic"), Size::Sm, Some(m(PickMsg::Toggle(Open::Mic))))]
        .spacing(4)
        .align_y(Alignment::Center);
    let anchor = container(r)
        .width(Length::Fill)
        .height(if dense { 40 } else { 48 })
        .padding(Padding::new(0.0).left(12).right(6))
        .align_y(Alignment::Center)
        .style(move |_| container::Style {
            background: Some(p().surface_raised.into()),
            border: Border {
                color: if open { p().ink } else { p().border },
                width: 1.0,
                radius: tokens::RADIUS_MD.into(),
            },
            ..container::Style::default()
        });
    let list = open.then(|| {
        let mut items = vec![option(
            s.mic_device_id.is_none(),
            true,
            icon(Icon::Mic, 16.0, p().ink).into(),
            t("defaultDevice"),
            None,
            m(PickMsg::Mic(None)),
        )];
        items.extend(s.devices.iter().map(|d| {
            option(
                s.mic_device_id.as_deref() == Some(d.id.as_str()),
                true,
                icon(Icon::Mic, 16.0, p().ink).into(),
                d.name.clone(),
                None,
                m(PickMsg::Mic(Some(d.id.clone()))),
            )
        }));
        sheet(items)
    });
    popover(anchor, list, m(PickMsg::Close)).into()
}

/// The source and the mic, as two fields.
pub fn picker<'a>(app: &'a App, dense: bool) -> Element<'a, Message> {
    column![source_field(app, dense), mic_field(app, dense, app.source.mode == SourceMode::Mic)]
        .spacing(if dense { 6 } else { 8 })
        .into()
}

/// The choice in one line, `Zoom · 会話`, opening the picker.
pub fn summary<'a>(app: &'a App, open: bool, height: f32, on: Message) -> Element<'a, Message> {
    let s = &app.source;
    let (glyph, name, _) = chosen(app, true);
    let mut r = row![
        container(glyph).center(18),
        container(line(name, tokens::BODY_SMALL, p().ink).font(theme::SANS_MEDIUM)).clip(true)
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if s.mode != SourceMode::Mic && s.include_mic {
        r = r.push(line(format!("· {}", t("conversationShort")), tokens::CAPTION, p().ink_muted));
    }
    r = r.push(Space::new().width(Length::Fill));
    if s.mode == SourceMode::App {
        r = r.push(app_wave::wave(app, s.app.as_ref().map(|a| a.root_pid), 30.0, 14.0));
    }
    r = r.push(chevron(open));
    let b = button(container(r).center_y(Length::Fill))
        .width(Length::Fill)
        .height(height)
        .padding([0, 12])
        .style(field_style(open))
        .on_press(on);
    with_tip(b, t("chooseSource"))
}

/// The recording's source as a dropdown under its one line (FR-17): the sources, and
/// 会話として録音 under them. A pick switches the source at once.
pub fn dropdown<'a>(app: &'a App) -> Element<'a, Message> {
    let open = app.main.picker.open == Open::Switch;
    let anchor = summary(app, open, 36.0, m(PickMsg::Toggle(Open::Switch)));
    let list = open.then(|| {
        let mut items = options(app);
        if app.source.mode != SourceMode::Mic {
            let on = app.source.include_mic;
            let r = row![
                icon(Icon::Mic, 18.0, if on { p().ink } else { p().ink_muted }),
                container(line(t("conversation"), tokens::BODY_SMALL, p().ink)).width(Length::Fill).clip(true),
                switch_mark(on),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            items.push(crate::widgets::hairline());
            items.push(
                button(r)
                    .width(Length::Fill)
                    .padding([8, 10])
                    .style(|_, status| button::Style {
                        background: hovered(status).then_some(Background::Color(p().surface_sunken)),
                        text_color: p().ink,
                        border: Border { radius: tokens::RADIUS_MD.into(), ..Border::default() },
                        ..button::Style::default()
                    })
                    .on_press(m(PickMsg::Conversation))
                    .into(),
            );
        }
        sheet(items)
    });
    popover(anchor, list, m(PickMsg::Close)).min_width(280.0).align_end().into()
}

/// The full picker over the window under the title bar: the compact window's way to choose before
/// a recording, and to transcribe a file instead (FR-18).
pub fn panel<'a>(app: &'a App) -> Element<'a, Message> {
    let head = row![
        container(line(t("listenTo"), tokens::CAPTION, p().ink_muted).font(theme::SANS_MEDIUM)).width(Length::Fill),
        crate::widgets::icon_button_tinted(
            Icon::Check,
            t("done"),
            Size::Sm,
            Some(m(PickMsg::Panel(false))),
            Some(p().ink)
        ),
    ]
    .align_y(Alignment::Center);
    let body = column![head, picker(app, true), file_link(app, true)].spacing(4);
    container(scrollable(container(body).padding(Padding::new(12.0).top(2).bottom(8))))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style { background: Some(p().surface.into()), ..container::Style::default() })
        .into()
}

/// 「ファイルから文字起こし」: a recording made elsewhere, transcribed as fast as the PC can.
pub fn file_link<'a>(app: &App, close_panel: bool) -> Element<'a, Message> {
    let enabled = app.rec.state == chidori_core::events::UiState::Ready;
    let r = row![
        icon(Icon::FileAudio, 13.0, Color::BLACK).style(|_, status| iced::widget::svg::Style {
            color: Some(if status == iced::widget::svg::Status::Hovered { p().ink } else { p().ink_muted })
        }),
        iced::widget::text(t("transcribeFile")).size(tokens::CAPTION.size).font(theme::SANS_MEDIUM)
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let b = button(r).padding([2, 4]).style(move |_, status| button::Style {
        text_color: if !enabled {
            alpha(p().ink_muted, 0.45)
        } else if hovered(status) {
            p().ink
        } else {
            p().ink_muted
        },
        ..button::Style::default()
    });
    if !enabled {
        return b.into();
    }
    let action = Message::Action(crate::actions::Action::TranscribeFile);
    if close_panel {
        b.on_press(Message::Main(Msg::Batch(vec![m(PickMsg::Panel(false)), action]))).into()
    } else {
        b.on_press(action).into()
    }
}

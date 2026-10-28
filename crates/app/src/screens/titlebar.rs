//! The title bar over every screen, kept while the views under it change (DECISIONS → UI, "One
//! title bar"): back (a past session, a section of 設定), the name or what is recording, the three
//! places as words (expanded) or icons (compact), and the window buttons. Dragging it moves the
//! window. No rule under it: only the setup wizard draws one, under its title.

use chidori_core::events::UiState;
use chidori_core::settings::{Layout, SourceMode};
use iced::widget::{Space, container, mouse_area, row};
use iced::{Alignment, Border, Element, Length, Padding, Task};

use crate::app::{App, Message, Place, SettingsTab, View, WinAction};
use crate::i18n::t;
use crate::icons::{Icon, icon};
use crate::model::format::sources_text;
use crate::native::menu::Item;
use crate::theme::{self, line, p, tokens};
use crate::widgets::{Size, bare, divider, icon_button, icon_button_tinted, with_tip};

#[derive(Debug, Clone)]
pub enum Msg {
    Back,
    /// The compact window's 録音中 tag: a native menu to switch the source (FR-17).
    SourceMenu,
    /// The app list is fresh: the menu opens.
    SourceMenuReady,
}

/// Where the back arrow leads: a past session back to 履歴, a section of 設定 back to its list.
fn back_of(app: &App) -> Option<Msg> {
    if app.needs_wizard() {
        return None;
    }
    match app.view {
        View::Session => Some(Msg::Back),
        View::Settings if app.settings_tab != SettingsTab::Index => Some(Msg::Back),
        _ => None,
    }
}

pub fn update(app: &mut App, msg: Msg) -> Task<Message> {
    match msg {
        Msg::Back => match app.view {
            View::Session => Task::done(Message::Go(View::History)),
            View::Settings => {
                app.settings_tab = SettingsTab::Index;
                Task::none()
            }
            _ => Task::none(),
        },
        Msg::SourceMenu => {
            let refresh = app.refresh_apps();
            refresh.chain(Task::done(Message::Titlebar(Msg::SourceMenuReady)))
        }
        Msg::SourceMenuReady => Task::done(Message::Popup(source_menu(app), None)),
    }
}

/// The source menu, once the app list is fresh.
fn source_menu(app: &App) -> Vec<Item> {
    let s = &app.source;
    let supported = app.info.app_loopback_supported;
    let mut items = vec![
        Item::check("titlebar:src-system", t("srcSystem"), s.mode == SourceMode::System),
        Item::check("titlebar:src-mic", t("srcMicOnly"), s.mode == SourceMode::Mic),
        Item::Separator,
    ];
    let playing = s.apps.iter().filter(|a| a.has_session);
    let others = s.apps.iter().filter(|a| !a.has_session);
    for a in playing.chain(others) {
        items.push(
            Item::check(format!("titlebar:src-app:{}", a.root_pid), a.name.clone(), s.is_chosen(a)).enabled(supported),
        );
    }
    if !s.apps.is_empty() {
        items.push(Item::Separator);
    }
    items.push(
        Item::check("titlebar:src-conversation", t("conversation"), s.mode != SourceMode::Mic && s.include_mic)
            .enabled(s.mode != SourceMode::Mic),
    );
    items
}

/// A pick in the source menu changes the choice; the main view turns it into a switch.
pub fn on_menu(app: &mut App, rest: &str) -> Task<Message> {
    let s = &mut app.source;
    match rest.split_once(':') {
        Some(("src-app", pid)) => {
            if let Some(a) = s.apps.iter().find(|a| a.root_pid.to_string() == pid).cloned() {
                s.set_app(&a);
                s.mode = SourceMode::App;
            }
        }
        _ => match rest {
            "src-system" => s.mode = SourceMode::System,
            "src-mic" => s.mode = SourceMode::Mic,
            "src-conversation" => s.include_mic = !s.include_mic,
            _ => return Task::none(),
        },
    }
    super::main_view::source_changed(app)
}

/// What is recording, in place of the name while a recording runs: a mark in the accent (grey
/// while paused or finishing), the state and the source.
fn live_tag(app: &App) -> Element<'_, Message> {
    let state = app.rec.state;
    let quiet = matches!(state, UiState::Paused | UiState::Finishing);
    let from_file =
        app.rec.sources.as_ref().is_some_and(|s| s.ids.contains(&chidori_core::session::model::SourceId::File));
    let label = match state {
        UiState::Paused => t("paused"),
        UiState::Finishing => t(if from_file { "transcribingFile" } else { "finishing" }),
        _ => t("recording"),
    };
    let sources = sources_text(app.rec.sources.as_ref());
    let mark = container(Space::new()).width(8).height(8).style(move |_| container::Style {
        background: Some(if quiet { p().border_strong } else { p().accent }.into()),
        border: Border { radius: 2.0.into(), ..Border::default() },
        ..container::Style::default()
    });
    let mut body = row![mark, line(label, tokens::CAPTION, p().ink).font(theme::SANS_MEDIUM)]
        .spacing(tokens::SPACE_2)
        .align_y(Alignment::Center);
    if !sources.is_empty() {
        body = body.push(container(line(sources.clone(), tokens::CAPTION, p().ink_muted)).clip(true));
    }
    let body = container(body).padding([2.0, 4.0]).clip(true);
    if app.view == View::Main && app.compact() && state != UiState::Finishing {
        return with_tip(bare(body, Some(Message::Titlebar(Msg::SourceMenu)), true), t("switchSource"));
    }
    if app.view == View::Main {
        // The title bar may be too narrow for the source: it shows on hover.
        return with_tip(body, sources);
    }
    with_tip(bare(body, Some(Message::Go(View::Main)), true), t("backToLive"))
}

/// The name, with the app's kamon as a window's icon sits beside its title.
fn brand<'a>() -> Element<'a, Message> {
    row![crate::icons::logo(18.0), line("Chidori", tokens::BODY_SMALL, p().ink).font(theme::SANS_MEDIUM)]
        .spacing(tokens::SPACE_2)
        .align_y(Alignment::Center)
        .into()
}

/// 録音 · 履歴 · 設定 as words; the current place is in the accent's tint, and 録音 carries a mark
/// while recording.
fn places(app: &App) -> Element<'_, Message> {
    let busy = app.rec.busy();
    let current = app.view.place();
    let entries = [
        (Place::Main, t("tabRecord"), View::Main),
        (Place::History, t("history"), View::History),
        (Place::Settings, t("settings"), View::Settings),
    ];
    let mut r = row![].align_y(Alignment::Center);
    for (place, label, view) in entries {
        let here = place == current;
        let mut content = row![].spacing(6).align_y(Alignment::Center);
        if place == Place::Main && busy {
            content = content.push(container(Space::new()).width(6).height(6).style(|_| container::Style {
                background: Some(p().accent.into()),
                border: Border { radius: 2.0.into(), ..Border::default() },
                ..container::Style::default()
            }));
        }
        let color = if here { p().accent_deep } else { p().ink_muted };
        content = content.push(line(label, tokens::BODY_SMALL, color).font(theme::SANS_MEDIUM));
        let b = iced::widget::button(container(content).padding([3.0, 7.0]))
            .padding(0)
            .style(move |_, status| iced::widget::button::Style {
                background: if here {
                    Some(p().accent_tint.into())
                } else if matches!(status, iced::widget::button::Status::Hovered) {
                    Some(p().surface_sunken.into())
                } else {
                    None
                },
                border: Border { radius: tokens::RADIUS_SM.into(), ..Border::default() },
                ..iced::widget::button::Style::default()
            })
            .on_press(Message::Go(view));
        let tip = if place == Place::Main && busy && !here { t("backToLive") } else { String::new() };
        r = r.push(with_tip(b, tip));
    }
    r.into()
}

fn window_buttons(app: &App, layout_toggle: bool) -> Element<'_, Message> {
    let on_top = app.settings.window.always_on_top;
    let compact = app.view == View::Main && app.layout() == Layout::Compact;
    let mut r = row![icon_button_tinted(
        Icon::Pin,
        t("alwaysOnTop"),
        Size::Sm,
        Some(Message::Win(WinAction::ToggleOnTop)),
        on_top.then(|| p().ink),
    )]
    .align_y(Alignment::Center);
    if layout_toggle {
        let (glyph, label) = if compact { (Icon::Maximize, t("expand")) } else { (Icon::Minimize, t("shrink")) };
        r = r.push(icon_button(glyph, label, Size::Sm, Some(Message::Win(WinAction::ToggleLayout))));
    }
    r.push(icon_button(Icon::Minus, t("minimize"), Size::Sm, Some(Message::Win(WinAction::Minimize))))
        .push(close_button())
        .into()
}

/// Close turns brick on hover, like the system's own.
fn close_button<'a>() -> Element<'a, Message> {
    let b = iced::widget::button(
        container(icon(Icon::X, 15.0, iced::Color::BLACK).style(|_, status| iced::widget::svg::Style {
            color: Some(if status == iced::widget::svg::Status::Hovered { p().surface } else { p().ink_muted }),
        }))
        .center(28),
    )
    .width(28)
    .height(28)
    .padding(0)
    .style(|_, status| iced::widget::button::Style {
        background: matches!(status, iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed)
            .then(|| p().danger.into()),
        border: Border { radius: tokens::RADIUS_MD.into(), ..Border::default() },
        ..iced::widget::button::Style::default()
    })
    .on_press(Message::Win(WinAction::Close));
    with_tip(b, t("close"))
}

pub fn view(app: &App) -> Element<'_, Message> {
    let setup = app.needs_wizard();
    let compact = app.compact();
    let mut bar = row![].spacing(4).align_y(Alignment::Center).height(40).padding(Padding::ZERO.left(12).right(6));
    if back_of(app).is_some() {
        bar = bar.push(icon_button(Icon::ArrowLeft, t("back"), Size::Sm, Some(Message::Titlebar(Msg::Back))));
    }
    let lead: Element<'_, Message> = if app.rec.busy() { live_tag(app) } else { brand() };
    // The empty part of the bar drags the window.
    bar = bar.push(mouse_area(container(lead).width(Length::Fill).clip(true)).on_press(Message::Win(WinAction::Drag)));
    if !setup {
        if compact {
            bar = bar
                .push(icon_button(Icon::History, t("history"), Size::Sm, Some(Message::Go(View::History))))
                .push(icon_button(Icon::Settings, t("settings"), Size::Sm, Some(Message::Go(View::Settings))));
        } else {
            bar = bar.push(places(app));
        }
        bar = bar.push(container(divider()).padding([0, 4]));
    }
    bar = bar.push(window_buttons(app, !setup));
    mouse_area(bar).on_press(Message::Win(WinAction::Drag)).into()
}

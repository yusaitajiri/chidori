//! Banners at the top with one action (section 5), toasts under the title bar, and what shows
//! while a file is held over the window.

use iced::widget::{Column, Space, button, column, container, row};
use iced::{Alignment, Border, Element, Length, Padding};

use crate::app::{App, Level, Message};
use crate::i18n::{t, t_json};
use crate::icons::{Icon, icon};
use crate::theme::{self, line, p, t as text, tokens};
use crate::widgets::floating;

/// Errors on the brand's brick tint and warnings on its ochre, each with its icon and words, so
/// none relies on colour alone: the accent is near both.
pub fn banners(app: &App) -> Element<'_, Message> {
    if app.banners.is_empty() {
        return Space::new().into();
    }
    let rows = app.banners.iter().map(|b| {
        let n = &b.notice;
        let level: Level = n.level.into();
        let (glyph, tone, icon_tone) = match level {
            Level::Error => (Icon::CircleAlert, p().danger_tint, p().danger),
            Level::Warn => (Icon::TriangleAlert, p().warning_tint, p().warning),
            Level::Info => (Icon::Info, p().surface_sunken, p().ink_muted),
        };
        let mut r = row![
            icon(glyph, 15.0, icon_tone),
            container(text(t_json(&n.message, &n.params), tokens::CAPTION, p().ink)).width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if let Some(action) = &n.action {
            let command = action.command.clone();
            let id = b.id;
            r = r.push(
                button(line(t(&action.label), tokens::CAPTION, p().ink).font(theme::SANS_MEDIUM))
                    .padding([4.0, 10.0])
                    .style(crate::widgets::button_style(crate::widgets::Variant::Secondary))
                    .on_press(Message::NoticeAction(command, Some(id))),
            );
        }
        r = r.push(
            button(icon(Icon::X, 14.0, p().ink_muted))
                .padding(2)
                .style(|_, _| button::Style::default())
                .on_press(Message::DismissBanner(b.id)),
        );
        container(r)
            .padding([8, 12])
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(tone.into()),
                border: Border { radius: tokens::RADIUS_LG.into(), ..Border::default() },
                ..container::Style::default()
            })
            .into()
    });
    container(Column::with_children(rows).spacing(6)).padding(Padding::ZERO.left(12).right(12).bottom(6)).into()
}

/// In-app toasts, shown for 2 s under the title bar: a small floating sheet whose icon tells the level.
pub fn toasts(app: &App) -> Element<'_, Message> {
    if app.toasts.is_empty() {
        return Space::new().into();
    }
    let items = app.toasts.iter().map(|toast| {
        let glyph = match toast.level {
            Level::Error => Icon::CircleAlert,
            Level::Warn => Icon::TriangleAlert,
            Level::Info => Icon::CircleCheckBig,
        };
        container(
            row![
                icon(glyph, 14.0, p().ink),
                line(toast.text.clone(), tokens::CAPTION, p().ink).font(theme::SANS_MEDIUM)
            ]
            .spacing(tokens::SPACE_2)
            .align_y(Alignment::Center),
        )
        .padding([8, 14])
        .style(|_| floating())
        .into()
    });
    container(Column::with_children(items).spacing(6).align_x(Alignment::Center))
        .width(Length::Fill)
        .padding(Padding::ZERO.top(44).left(16).right(16))
        .center_x(Length::Fill)
        .into()
}

/// While a file is held over the window (FR-18).
pub fn drop_overlay<'a>() -> Element<'a, Message> {
    let body = column![
        icon(Icon::FileAudio, 28.0, p().ink_muted),
        line(t("dropToTranscribe"), tokens::BODY_SMALL, p().ink_muted).font(theme::SANS_MEDIUM)
    ]
    .spacing(tokens::SPACE_2)
    .align_x(Alignment::Center);
    container(container(body).center(Length::Fill))
        .padding(Padding::ZERO.top(40))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style { background: Some(p().surface.into()), ..container::Style::default() })
        .into()
}

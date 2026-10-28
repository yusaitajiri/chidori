//! The tops of the recording and of a finished session. The recording keeps its project's colour
//! (FR-64), its title (click to rename) and what is recorded, a dropdown to switch it (FR-17), in
//! place above the transcript. A finished session's header scrolls with its transcript: the
//! title, when it started, its length, lines and screenshots in big numerals, and 詳細, closed at
//! first, for the model and the folder it was saved to.

use std::rc::Rc;

use chidori_core::session::model::TimelineItem;
use iced::widget::{Column, Space, button, column, container, hover, row, text_input};
use iced::{Alignment, Border, Element, Length, Padding};

use super::source_picker;
use super::transcript::{ListMsg, ListState, Wrap};
use crate::app::{App, Message};
use crate::i18n::{is_ja, t, tf};
use crate::icons::{Icon, icon};
use crate::model::format::{Unit, day_of, day_text, duration_parts, time_of};
use crate::model::transcript::Transcript;
use crate::screens::session_menu;
use crate::theme::{self, line, p, project_color, tokens};
use crate::widgets::{hairline, swatch, with_tip};

/// A project's colour as a small rounded square; hollow for none.
fn mark<'a>(color: Option<iced::Color>, size: f32) -> Element<'a, Message> {
    match color {
        Some(c) => swatch(c, size),
        None => container(Space::new())
            .width(size)
            .height(size)
            .style(move |_| container::Style {
                border: Border { color: p().border_strong, width: 1.5, radius: (size * 0.3).into() },
                ..container::Style::default()
            })
            .into(),
    }
}

/// The session's project, when it is one of the projects.
fn project_of(app: &App, tr: &Transcript) -> Option<(iced::Color, String)> {
    let id = tr.project.as_deref()?;
    app.projects.iter().find(|p| p.id == id).map(|p| (project_color(&p.color), p.name.clone()))
}

fn project_menu(app: &App, tr: &Transcript) -> Option<Message> {
    let id = tr.session_id.clone()?;
    Some(Message::Popup(session_menu::project_items(app, &[id], tr.project.as_deref()), None))
}

fn ghost(status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then(|| p().surface_sunken.into()),
        text_color: p().ink,
        border: Border { radius: tokens::RADIUS_MD.into(), ..Border::default() },
        ..button::Style::default()
    }
}

/// A title as a button that renames it, or the box to rename it in.
fn title<'a>(
    name: String,
    draft: Option<String>,
    input_id: iced::widget::Id,
    style: tokens::TypeStyle,
    wrap: Wrap,
) -> Element<'a, Message> {
    if let Some(draft) = draft {
        let input = text_input(&t("newTitle"), &draft)
            .id(input_id)
            .on_input(move |v| wrap(ListMsg::TitleInput(v)))
            .on_submit(wrap(ListMsg::TitleSave))
            .font(theme::SANS_MEDIUM)
            .size(style.size)
            .padding([4, 10])
            .style(crate::widgets::input_style);
        let save = button(container(icon(Icon::Check, 15.0, p().on_accent)).center(36))
            .width(36)
            .height(36)
            .padding(0)
            .style(crate::widgets::button_style(crate::widgets::Variant::Primary))
            .on_press(wrap(ListMsg::TitleSave));
        return row![input, with_tip(save, t("save"))].spacing(6).align_y(Alignment::Center).into();
    }
    let label = theme::t(name, style, p().ink).font(theme::SANS_MEDIUM);
    let base =
        button(container(label).padding([2, 6])).padding(0).style(|_, s| ghost(s)).on_press(wrap(ListMsg::RenameStart));
    let pencil = container(icon(Icon::Pencil, 13.0, p().ink_muted))
        .align_right(Length::Fill)
        .center_y(Length::Fill)
        .padding([0, 6]);
    with_tip(hover(base, pencil), t("rename"))
}

/// The top of the recording view.
pub fn live_header<'a>(app: &'a App, tr: &'a Transcript, list: &'a ListState, wrap: Wrap) -> Element<'a, Message> {
    let project = project_of(app, tr);
    let label = project.as_ref().map_or_else(|| t("noProject"), |(_, n)| format!("{}: {n}", t("project")));
    let mut left = row![].spacing(4).align_y(Alignment::Center);
    if tr.session_id.is_some() {
        let b = button(container(mark(project.map(|p| p.0), 11.0)).center(28))
            .width(28)
            .height(28)
            .padding(0)
            .style(|_, s| ghost(s));
        let b = match project_menu(app, tr) {
            Some(m) => b.on_press(m),
            None => b,
        };
        left = left.push(with_tip(b, label));
    }
    left = left.push(
        container(title(
            tr.title.clone().unwrap_or_default(),
            list.title.clone(),
            list.title_input(),
            tokens::LABEL,
            wrap,
        ))
        .width(Length::Fill)
        .clip(true),
    );
    let r = row![
        container(left).width(Length::FillPortion(45)),
        container(source_picker::dropdown(app)).width(Length::FillPortion(55)),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    container(column![container(r).padding([8, 0]), hairline()]).padding(Padding::ZERO.left(18).right(18)).into()
}

/// The last folders of a path, as Explorer's address bar reads: `Chidori › 2026-10-03_1513_Zoom`.
fn short_path(folder: &std::path::Path) -> String {
    let parts: Vec<String> = folder
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    parts[parts.len().saturating_sub(2)..].join(" › ")
}

/// Everyday and full name of a catalog model, and on what it ran.
fn model_text(app: &App, tr: &Transcript) -> String {
    let mut parts = Vec::new();
    if let Some(id) = &tr.model_id {
        let full = app
            .models
            .iter()
            .find(|m| &m.entry.id == id)
            .map_or_else(|| id.clone(), |m| if is_ja() { m.entry.name_ja.clone() } else { m.entry.name_en.clone() });
        parts.push(full);
    }
    if let Some(gpu) = tr.gpu {
        parts.push(t(if gpu { "gpu" } else { "cpu" }));
    }
    parts.join(" · ")
}

/// A finished session's header, built whenever its row is (it scrolls with the transcript).
pub fn session_header<'a>(
    app: &'a App,
    tr: &'a Transcript,
    list: &'a ListState,
    wrap: Wrap,
) -> (Rc<dyn Fn() -> Element<'static, Message> + 'a>, u64) {
    let name = tr.title.clone().unwrap_or_default();
    let draft = list.title.clone();
    let details = list.details;
    let id = list.title_input();
    let project = project_of(app, tr);
    let menu = project_menu(app, tr);
    let when = tr.started_at.as_ref().map(|s| {
        let day = day_text(day_of(s), chrono::Datelike::year(&chrono::Local::now()));
        let when = if is_ja() { format!("{day}{}", time_of(s)) } else { format!("{day}, {}", time_of(s)) };
        tf("startedWhen", &[("when", &when)])
    });
    let duration = tr.duration_ms;
    let lines = tr.items.iter().filter(|i| matches!(i, TimelineItem::Segment(_))).count();
    let shots = tr.items.iter().filter(|i| matches!(i, TimelineItem::Screenshot(_))).count();
    let model = model_text(app, tr);
    let folder = tr.folder.clone();
    let session = tr.session_id.clone();
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (&name, &draft, details, &when, duration, lines, shots, &model, &folder).hash(&mut h);
        project.as_ref().map(|p| &p.1).hash(&mut h);
        app.projects_version.hash(&mut h);
        h.finish()
    };
    let build = move || -> Element<'static, Message> {
        let mut col = Column::new().push(title(name.clone(), draft.clone(), id.clone(), tokens::HEADING, wrap));
        let mut meta = row![].spacing(10).align_y(Alignment::Center);
        if session.is_some() {
            let label = project.as_ref().map_or_else(|| t("noProject"), |(_, n)| n.clone());
            let tone = if project.is_some() { p().ink } else { p().ink_muted };
            let font = if project.is_some() { theme::SANS_MEDIUM } else { theme::SANS };
            let r = row![
                mark(project.as_ref().map(|p| p.0), 10.0),
                line(label, tokens::CAPTION, tone).font(font),
                icon(Icon::ChevronDown, 12.0, p().ink_muted)
            ]
            .spacing(6)
            .align_y(Alignment::Center);
            let b = button(r).padding([2, 4]).style(|_, s| ghost(s));
            meta = meta.push(match &menu {
                Some(m) => b.on_press(m.clone()),
                None => b,
            });
        }
        if let Some(w) = &when {
            meta = meta.push(line(w.clone(), tokens::CAPTION, p().ink_muted));
        }
        col = col.push(container(meta).padding(Padding::ZERO.top(2)));

        let big = |n: u64| line(n.to_string(), tokens::TITLE, p().ink).font(theme::MONO);
        let unit = |s: String| line(s, tokens::CAPTION, p().ink_muted);
        let mut numbers = row![].spacing(18).align_y(Alignment::End);
        if let Some(ms) = duration {
            let mut d = row![].spacing(2).align_y(Alignment::End);
            for (n, u) in duration_parts(ms) {
                let u = t(match u {
                    Unit::H => "unitH",
                    Unit::M => "unitM",
                    Unit::S => "unitS",
                });
                d = d.push(big(n)).push(container(unit(u)).padding(Padding::ZERO.right(4).bottom(4)));
            }
            numbers = numbers.push(d);
        }
        numbers = numbers.push(
            row![big(lines as u64), container(unit(t("unitLines"))).padding(Padding::ZERO.bottom(4))]
                .spacing(2)
                .align_y(Alignment::End),
        );
        if shots > 0 {
            numbers = numbers.push(
                row![big(shots as u64), container(unit(t("unitShots"))).padding(Padding::ZERO.bottom(4))]
                    .spacing(2)
                    .align_y(Alignment::End),
            );
        }
        numbers = numbers.push(Space::new().width(Length::Fill));
        if !model.is_empty() || folder.is_some() {
            let chevron = icon(Icon::ChevronDown, 13.0, p().ink_muted).rotation(iced::Radians(if details {
                std::f32::consts::PI
            } else {
                0.0
            }));
            let toggle = button(
                row![line(t("details"), tokens::CAPTION, p().ink_muted).font(theme::SANS_MEDIUM), chevron]
                    .spacing(2)
                    .align_y(Alignment::Center),
            )
            .padding([0, 4])
            .style(|_, s| ghost(s))
            .on_press(wrap(ListMsg::Details));
            numbers = numbers.push(container(toggle).padding(Padding::ZERO.bottom(4)));
        }
        col = col.push(container(numbers).padding(Padding::ZERO.top(10)));

        if details {
            let mut dl = Column::new().spacing(4);
            let entry = |k: String, v: Element<'static, Message>| -> Element<'static, Message> {
                row![
                    container(line(k, tokens::CAPTION, p().ink_muted)).width(64),
                    container(v).width(Length::Fill).clip(true)
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .into()
            };
            if !model.is_empty() {
                dl = dl.push(entry(t("model"), line(model.clone(), tokens::CAPTION, p().ink).into()));
            }
            if let Some(f) = &folder {
                let link = button(line(short_path(f), tokens::CAPTION, p().ink))
                    .padding(0)
                    .style(|_, s| button::Style {
                        border: Border {
                            color: if matches!(s, button::Status::Hovered) { p().ink } else { p().border_strong },
                            width: 0.0,
                            radius: 0.0.into(),
                        },
                        text_color: p().ink,
                        ..button::Style::default()
                    })
                    .on_press(Message::Action(crate::actions::Action::OpenFolder(session.clone())));
                dl = dl.push(entry(t("savedTo"), with_tip(link, f.to_string_lossy().into_owned())));
            }
            col = col.push(container(dl).padding(Padding::ZERO.top(10)));
        }
        container(column![container(col).padding(Padding::ZERO.top(12).bottom(14)), hairline()])
            .padding(Padding::ZERO.left(18).right(18))
            .width(Length::Fill)
            .into()
    };
    (Rc::new(build), key)
}

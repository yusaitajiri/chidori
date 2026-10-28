//! Dialogs over everything: recovery at launch (flow D), quitting while recording (FR-93),
//! 書き出し, renaming a row (FR-63), moving sessions to the Recycle Bin (FR-62), and a project's
//! name and colour (FR-64). Each is a sheet in the kamon's frame with its main action on top; while
//! one is open it is the view's one frame and one primary.

use chidori_core::error::AppError;
use chidori_core::events::UiState;
use chidori_core::ops;
use chidori_core::session::projects::Project;
use chidori_core::session::recovery::SessionSummary;
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{Column, Row, Space, button, column, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Subscription, Task};

use crate::actions::{self, Action, ExportKind};
use crate::app::{App, Level, Message, call};
use crate::i18n::{t, tf};
use crate::icons::{Icon, icon};
use crate::model::format::{date_time_of, hms};
use crate::theme::{self, PROJECT_COLORS, line, p, project_color, tokens};
use crate::widgets::{self, Variant, dialog, dialog_button, with_tip};

/// The one text box a dialog can hold, focused when it opens.
const INPUT: &str = "dialog-input";

/// A project's name is kept short enough for a menu and a title.
const NAME_MAX: usize = 40;

#[derive(Debug, Default)]
pub struct State {
    open: Option<Dialog>,
    /// Sessions that ended in a crash, offered one at a time at launch (flow D).
    recoverable: Vec<SessionSummary>,
    recovering: bool,
    /// Markdown comes as one ZIP: remembered while the app runs.
    zip: bool,
    /// Counts dialogs opened, so each one with a text box focuses it once.
    opened: u64,
}

#[derive(Debug, Clone)]
enum Dialog {
    ConfirmQuit,
    Rename {
        id: String,
        title: String,
    },
    Delete {
        ids: Vec<String>,
        title: Option<String>,
        /// From 履歴's 選択: selection ends once they are gone.
        from_selection: bool,
    },
    Export {
        id: String,
        busy: Option<ExportKind>,
    },
    Project(ProjectForm),
    DeleteProject(Project),
}

/// A project's name and colour: a new one (`editing` None) for these sessions, or one being changed.
#[derive(Debug, Clone)]
struct ProjectForm {
    editing: Option<Project>,
    session_ids: Vec<String>,
    /// Asked from 履歴's filter menu: the new project's page opens.
    open_page: bool,
    name: String,
    color: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Close,
    Escape,
    Focus(u64),
    Quit,
    QuitAnswer(Result<bool, AppError>),
    Title(String),
    SaveTitle,
    ConfirmDelete,
    ExportAs(ExportKind),
    Zip(bool),
    Exported,
    ProjectName(String),
    ProjectColor(&'static str),
    SaveProject,
    /// A project was made (and its sessions put in it); `bool`: its page opens.
    ProjectMade(Result<Project, AppError>, Vec<String>, bool),
    DeleteProject,
    /// Sessions were put in a project, or in none.
    Assigned(Vec<String>, Option<String>, Result<(), AppError>),
    Recoverable(Vec<SessionSummary>),
    Recover,
    Recovered(Result<(), AppError>),
    RecoverLater,
}

fn msg(m: Msg) -> Message {
    Message::Dialog(m)
}

/// Whether a dialog is over the window, so the view under it leaves keys alone.
pub fn is_open(app: &App) -> bool {
    app.dialogs.open.is_some() || !app.dialogs.recoverable.is_empty()
}

/// An export is running, with its spinner (the app ticks to turn it).
pub fn spinning(app: &App) -> bool {
    matches!(app.dialogs.open, Some(Dialog::Export { busy: Some(_), .. }))
}

fn open(app: &mut App, d: Dialog) {
    if matches!(d, Dialog::Rename { .. } | Dialog::Project(_)) {
        app.dialogs.opened += 1;
    }
    app.dialogs.open = Some(d);
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    let st = &mut app.dialogs;
    match m {
        Msg::Close => {
            // An export on its way finishes first.
            if !matches!(st.open, Some(Dialog::Export { busy: Some(_), .. })) {
                st.open = None;
            }
            Task::none()
        }
        Msg::Escape => {
            if st.open.is_some() {
                update(app, Msg::Close)
            } else {
                update(app, Msg::RecoverLater)
            }
        }
        Msg::Focus(n) => {
            if n == st.opened && st.open.is_some() {
                iced::widget::operation::focus(INPUT).chain(iced::widget::operation::move_cursor_to_end(INPUT))
            } else {
                Task::none()
            }
        }
        Msg::Quit => {
            st.open = None;
            call(app, ops::request_quit, |r| msg(Msg::QuitAnswer(r)))
        }
        // Quitting while recording stops first; the core ends the app once the session is saved.
        Msg::QuitAnswer(Ok(true)) => iced::exit(),
        Msg::QuitAnswer(Ok(false)) => Task::none(),
        Msg::QuitAnswer(Err(e)) => {
            actions::report_error(app, &e, "failed");
            Task::none()
        }
        Msg::Title(s) => {
            if let Some(Dialog::Rename { title, .. }) = &mut st.open {
                *title = s;
            }
            Task::none()
        }
        Msg::SaveTitle => {
            let Some(Dialog::Rename { id, title }) = &st.open else { return Task::none() };
            let next = title.trim().to_string();
            if next.is_empty() {
                return Task::none();
            }
            let id = id.clone();
            st.open = None;
            let renamed = actions::run(app, Action::Rename(id, next));
            renamed.chain(super::history::reload(app))
        }
        Msg::ConfirmDelete => {
            let Some(Dialog::Delete { ids, from_selection, .. }) = st.open.take() else { return Task::none() };
            if from_selection {
                super::history::stop_selecting(app);
            }
            actions::run(app, Action::Delete(ids))
        }
        Msg::ExportAs(kind) => {
            let zip = st.zip;
            let Some(Dialog::Export { id, busy }) = &mut st.open else { return Task::none() };
            if busy.is_some() {
                return Task::none();
            }
            *busy = Some(kind);
            let id = id.clone();
            actions::run(app, Action::Export(kind, Some(id), zip)).chain(Task::done(msg(Msg::Exported)))
        }
        Msg::Exported => {
            if matches!(st.open, Some(Dialog::Export { .. })) {
                st.open = None;
            }
            Task::none()
        }
        Msg::Zip(on) => {
            st.zip = on;
            Task::none()
        }
        Msg::ProjectName(s) => {
            if let Some(Dialog::Project(f)) = &mut st.open {
                f.name = s.chars().take(NAME_MAX).collect();
            }
            Task::none()
        }
        Msg::ProjectColor(c) => {
            if let Some(Dialog::Project(f)) = &mut st.open {
                f.color = c.to_string();
            }
            Task::none()
        }
        Msg::SaveProject => {
            let Some(Dialog::Project(f)) = &st.open else { return Task::none() };
            let name = f.name.trim().to_string();
            if name.is_empty() {
                return Task::none();
            }
            let Some(Dialog::Project(f)) = st.open.take() else { return Task::none() };
            match f.editing {
                Some(project) => {
                    let changed = Project { name, color: f.color, ..project };
                    call(
                        app,
                        move |c| ops::update_project(c, &changed),
                        |r| match r {
                            Ok(list) => Message::Projects(list),
                            Err(e) => Message::Failed(e, "saveFailed"),
                        },
                    )
                }
                None => {
                    let (ids, color, open_page) = (f.session_ids, f.color, f.open_page);
                    let ids2 = ids.clone();
                    call(
                        app,
                        move |c| {
                            let made = ops::create_project(c, &name, &color)?;
                            if !ids2.is_empty() {
                                ops::set_project(c, &ids2, Some(made.id.clone()))?;
                            }
                            Ok(made)
                        },
                        move |r| msg(Msg::ProjectMade(r, ids, open_page)),
                    )
                }
            }
        }
        Msg::ProjectMade(r, ids, open_page) => {
            match r {
                Ok(made) => {
                    follow_project(app, &ids, Some(made.id.clone()));
                    if open_page {
                        super::history::show_project(app, made.id);
                    }
                }
                Err(e) => actions::report_error(app, &e, "saveFailed"),
            }
            // Even a half-done save may have made the project.
            Task::done(Message::ProjectsChanged)
        }
        Msg::DeleteProject => {
            let Some(Dialog::DeleteProject(project)) = st.open.take() else { return Task::none() };
            call(
                app,
                move |c| ops::delete_project(c, &project.id),
                |r| match r {
                    // Its sessions now show as in no project.
                    Ok(_) => Message::ProjectsChanged,
                    Err(e) => Message::Failed(e, "deleteFailed"),
                },
            )
        }
        Msg::Assigned(ids, project, r) => match r {
            Ok(()) => {
                follow_project(app, &ids, project);
                Task::done(Message::ProjectsChanged)
            }
            Err(e) => {
                actions::report_error(app, &e, "saveFailed");
                Task::none()
            }
        },
        Msg::Recoverable(list) => {
            st.recoverable = list;
            Task::none()
        }
        Msg::Recover => {
            if st.recovering {
                return Task::none();
            }
            let Some(id) = st.recoverable.first().map(|s| s.id.clone()) else { return Task::none() };
            st.recovering = true;
            call(app, move |c| ops::recover_session(c, &id).map(|_| ()), |r| msg(Msg::Recovered(r)))
        }
        Msg::Recovered(r) => {
            st.recovering = false;
            if !st.recoverable.is_empty() {
                st.recoverable.remove(0);
            }
            match r {
                Ok(()) => {
                    app.toast(t("recovered"), Level::Info);
                    super::history::reload(app)
                }
                Err(e) => {
                    actions::report_error(app, &e, "saveFailed");
                    Task::none()
                }
            }
        }
        Msg::RecoverLater => {
            if !st.recovering && !st.recoverable.is_empty() {
                st.recoverable.remove(0);
            }
            Task::none()
        }
    }
}

/// The transcript on screen follows a change to its project.
fn follow_project(app: &mut App, ids: &[String], project: Option<String>) {
    if app.live.session_id.as_ref().is_some_and(|s| ids.contains(s)) {
        app.live.project = project;
    }
}

/// A pick in a menu this module's items came from (`dialog:` ids): 書き出し…, a project, or a new one.
pub fn on_menu(app: &mut App, rest: &str) -> Task<Message> {
    let (verb, arg) = rest.split_once(':').unwrap_or((rest, ""));
    let ids = |s: &str| s.split(',').filter(|i| !i.is_empty()).map(str::to_string).collect::<Vec<_>>();
    match verb {
        "export" if !arg.is_empty() => {
            ask_export(app, arg.to_string());
            Task::none()
        }
        "assign" => {
            let Some((project, sessions)) = arg.split_once(':') else { return Task::none() };
            let sessions = ids(sessions);
            if sessions.is_empty() {
                return Task::none();
            }
            let project = (project != "-").then(|| project.to_string());
            let (ids2, project2) = (sessions.clone(), project.clone());
            call(app, move |c| ops::set_project(c, &ids2, project2), move |r| msg(Msg::Assigned(sessions, project, r)))
        }
        "new-project" => {
            ask_new_project(app, ids(arg));
            Task::none()
        }
        _ => Task::none(),
    }
}

/// Offers to recover sessions that ended in a crash, once per launch, when nothing records.
pub fn on_launch(app: &App) -> Task<Message> {
    if app.rec.state != UiState::Ready {
        return Task::none();
    }
    call(app, ops::list_recoverable, |list| msg(Msg::Recoverable(list)))
}

pub fn ask_confirm_quit(app: &mut App) {
    open(app, Dialog::ConfirmQuit);
}

/// Asks before moving sessions to the Recycle Bin; one is named by its title.
pub fn ask_delete(app: &mut App, ids: Vec<String>, title: Option<String>) {
    let title = title.or_else(|| if ids.len() == 1 { title_of(app, &ids[0]) } else { None });
    open(app, Dialog::Delete { ids, title, from_selection: false });
}

/// 削除 under 履歴's 選択: once they are gone, selection ends.
pub fn ask_delete_selected(app: &mut App, ids: Vec<String>) {
    open(app, Dialog::Delete { ids, title: None, from_selection: true });
}

/// Renames a session shown as a row; an open session renames in its header instead.
pub fn ask_rename(app: &mut App, id: String) {
    let title = title_of(app, &id).unwrap_or_default();
    open(app, Dialog::Rename { id, title });
}

/// A session's title, from 履歴's list or the transcript on screen.
fn title_of(app: &App, id: &str) -> Option<String> {
    super::history::title_of(app, id)
        .or_else(|| (app.live.session_id.as_deref() == Some(id)).then(|| app.live.title.clone()).flatten())
}

/// Events this screen listens to beyond its widgets: Escape closes the dialog on top, and a dialog
/// with a text box focuses it once it shows.
pub fn subscription(app: &App) -> Subscription<Message> {
    if !is_open(app) {
        return Subscription::none();
    }
    let mut subs = vec![iced::event::listen_with(on_key)];
    if matches!(app.dialogs.open, Some(Dialog::Rename { .. } | Dialog::Project(_))) {
        subs.push(Subscription::run_with(("dialog-focus", app.dialogs.opened), focus_once));
    }
    Subscription::batch(subs)
}

fn on_key(e: iced::Event, _: iced::event::Status, _: iced::window::Id) -> Option<Message> {
    match e {
        iced::Event::Keyboard(keyboard::Event::KeyPressed { key: Key::Named(Named::Escape), .. }) => {
            Some(msg(Msg::Escape))
        }
        _ => None,
    }
}

fn focus_once(n: &(&'static str, u64)) -> impl iced::futures::Stream<Item = Message> + use<> {
    iced::futures::stream::once(std::future::ready(msg(Msg::Focus(n.1))))
}

/// 書き出し: the dialog to pick Markdown, PDF or Typst for a session.
pub fn ask_export(app: &mut App, session_id: String) {
    open(app, Dialog::Export { id: session_id, busy: None });
}

/// 新しいプロジェクト…: makes a project and puts these sessions in it (FR-64).
pub fn ask_new_project(app: &mut App, session_ids: Vec<String>) {
    new_project(app, session_ids, false);
}

/// 新しいプロジェクト… in 履歴's filter menu: the new, empty project's page opens.
pub fn ask_new_project_page(app: &mut App) {
    new_project(app, Vec::new(), true);
}

fn new_project(app: &mut App, session_ids: Vec<String>, open_page: bool) {
    let color = fresh_color(&app.projects).to_string();
    open(app, Dialog::Project(ProjectForm { editing: None, session_ids, open_page, name: String::new(), color }));
}

/// Changes a project's name and colour, from its page in 履歴.
pub fn ask_edit_project(app: &mut App, project: Project) {
    let (name, color) = (project.name.clone(), project.color.clone());
    open(
        app,
        Dialog::Project(ProjectForm { editing: Some(project), session_ids: Vec::new(), open_page: false, name, color }),
    );
}

/// Asks before deleting a project; its sessions stay, in no project.
pub fn ask_delete_project(app: &mut App, project: Project) {
    open(app, Dialog::DeleteProject(project));
}

/// A colour the other projects don't use yet, so a new one stands apart; grey last.
pub fn fresh_color(projects: &[Project]) -> &'static str {
    PROJECT_COLORS.iter().copied().find(|c| *c != "slate" && !projects.iter().any(|p| p.color == *c)).unwrap_or("slate")
}

/// The i18n key naming a colour: `colorBlue`.
fn color_key(c: &str) -> String {
    let mut chars = c.chars();
    match chars.next() {
        Some(first) => format!("color{}{}", first.to_uppercase(), chars.as_str()),
        None => "color".into(),
    }
}

/// A project's colour as a small rounded square (the dot is the app's only circle); hollow for none.
pub fn project_mark<'a, M: 'a>(color: Option<&str>, size: f32) -> Element<'a, M> {
    let fill = color.map(project_color);
    let radius = if size >= 16.0 { 4.0 } else { 3.0 };
    container(Space::new())
        .width(size)
        .height(size)
        .style(move |_| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                color: if fill.is_some() { Color::TRANSPARENT } else { p().border_strong },
                width: if fill.is_some() { 0.0 } else { 1.5 },
                radius: radius.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn body_text<'a>(s: String) -> Element<'a, Message> {
    theme::t(s, tokens::BODY_SMALL, p().ink).into()
}

/// The open dialog over everything, or none.
pub fn view(app: &App) -> Option<Element<'_, Message>> {
    let st = &app.dialogs;
    let cancel = || dialog_button(t("cancel"), Variant::Secondary, Some(msg(Msg::Close)));
    let Some(d) = &st.open else { return recovery(app) };
    Some(match d {
        Dialog::ConfirmQuit => dialog(
            t("confirmQuitTitle"),
            Some(body_text(t("confirmQuitBody"))),
            vec![dialog_button(t("quitAndSave"), Variant::Danger, Some(msg(Msg::Quit))), cancel()],
        ),
        Dialog::Rename { title, .. } => {
            let ok = !title.trim().is_empty();
            let field = widgets::input("", title, |s| msg(Msg::Title(s))).id(INPUT).on_submit(msg(Msg::SaveTitle));
            dialog(
                t("rename"),
                Some(field.into()),
                vec![dialog_button(t("save"), Variant::Primary, ok.then(|| msg(Msg::SaveTitle))), cancel()],
            )
        }
        Dialog::Delete { ids, title, .. } => {
            let heading = match (ids.len(), title) {
                (1, Some(title)) => tf("confirmDelete", &[("title", title)]),
                (n, _) => tf("confirmDeleteMany", &[("n", &n)]),
            };
            dialog(
                heading,
                None,
                vec![dialog_button(t("delete"), Variant::Danger, Some(msg(Msg::ConfirmDelete))), cancel()],
            )
        }
        Dialog::Export { busy, .. } => {
            let rows = [
                (ExportKind::Markdown, Icon::FileText, "formatMarkdown"),
                (ExportKind::Pdf, Icon::FileType, "formatPdf"),
                (ExportKind::Typst, Icon::FileCode, "formatTypst"),
            ]
            .into_iter()
            .map(|(kind, glyph, key)| format_row(app, kind, glyph, key, *busy));
            dialog(
                t("export"),
                Some(Column::with_children(rows).spacing(6).into()),
                vec![dialog_button(t("cancel"), Variant::Secondary, busy.is_none().then(|| msg(Msg::Close)))],
            )
        }
        Dialog::Project(f) => {
            let ok = !f.name.trim().is_empty();
            let field = widgets::input(&t("projectName"), &f.name, |s| msg(Msg::ProjectName(s)))
                .id(INPUT)
                .on_submit(msg(Msg::SaveProject));
            let body = column![field, color_choice(&f.color)].spacing(tokens::SPACE_3);
            let (title, save) =
                if f.editing.is_some() { ("editProject", "save") } else { ("newProject", "createProject") };
            dialog(
                t(title),
                Some(body.into()),
                vec![dialog_button(t(save), Variant::Primary, ok.then(|| msg(Msg::SaveProject))), cancel()],
            )
        }
        Dialog::DeleteProject(project) => dialog(
            tf("confirmDeleteProject", &[("name", &project.name)]),
            Some(body_text(t("deleteProjectNote"))),
            vec![dialog_button(t("delete"), Variant::Danger, Some(msg(Msg::DeleteProject))), cancel()],
        ),
    })
}

/// Flow D: offers to recover a session that has no end marker, one at a time.
fn recovery(app: &App) -> Option<Element<'_, Message>> {
    let st = &app.dialogs;
    let s = st.recoverable.first()?;
    let meta = format!(
        "{} · {} · {}",
        date_time_of(&s.started_at),
        hms(s.duration_ms),
        tf("segmentsCount", &[("n", &s.segments)])
    );
    let card = container(
        column![line(s.title.clone(), tokens::LABEL, p().ink), line(meta, tokens::CAPTION, p().ink_muted),].spacing(2),
    )
    .width(Length::Fill)
    .padding([10.0, tokens::SPACE_3])
    .style(|_| container::Style {
        background: Some(p().surface_sunken.into()),
        border: Border { radius: tokens::RADIUS_LG.into(), ..Border::default() },
        ..container::Style::default()
    });
    Some(dialog(
        t("recoverTitle"),
        Some(card.into()),
        vec![
            dialog_button(t("recover"), Variant::Primary, (!st.recovering).then(|| msg(Msg::Recover))),
            dialog_button(t("recoverLater"), Variant::Secondary, Some(msg(Msg::RecoverLater))),
        ],
    ))
}

/// One of 書き出し's formats; Markdown carries its ZIP switch.
fn format_row<'a>(
    app: &App,
    kind: ExportKind,
    glyph: Icon,
    key: &str,
    busy: Option<ExportKind>,
) -> Element<'a, Message> {
    let idle = busy.is_none();
    let mut content = Row::new()
        .push(icon(glyph, 18.0, p().ink_muted))
        .push(container(line(t(key), tokens::LABEL, p().ink)).width(Length::Fill))
        .spacing(tokens::SPACE_3)
        .align_y(Alignment::Center);
    if busy == Some(kind) {
        content = content.push(widgets::spinner(16.0, app.now));
    }
    if kind == ExportKind::Markdown {
        let zip = app.dialogs.zip;
        let toggle = idle.then(|| msg(Msg::Zip(!zip)));
        let label = row![
            line("ZIP", tokens::CAPTION, p().ink_muted).font(theme::SANS_MEDIUM),
            widgets::switch(zip, toggle.clone())
        ]
        .spacing(tokens::SPACE_2)
        .align_y(Alignment::Center);
        content = content.push(with_tip(widgets::bare(label, toggle, false), t("zipToggle")));
    }
    let b = button(container(content).padding([10.0, tokens::SPACE_3]).center_y(46))
        .width(Length::Fill)
        .padding(0)
        .style(move |_, status| {
            let p = p();
            let hot = idle && matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(if hot { p.surface_sunken } else { p.surface_raised }.into()),
                text_color: p.ink,
                border: Border {
                    color: if hot { p.border_strong } else { p.border },
                    width: 1.0,
                    radius: tokens::RADIUS_LG.into(),
                },
                ..button::Style::default()
            }
        });
    if idle { b.on_press(msg(Msg::ExportAs(kind))).into() } else { b.into() }
}

/// The colour choice: one square per colour, the chosen one ringed.
fn color_choice<'a>(chosen: &str) -> Element<'a, Message> {
    let squares = PROJECT_COLORS.iter().map(|&c| {
        let on = c == chosen;
        let b = button(container(project_mark(Some(c), 16.0)).center(26))
            .width(28)
            .height(28)
            .padding(0)
            .style(move |_, status| button::Style {
                background: None,
                border: Border {
                    color: if on {
                        p().ink
                    } else if matches!(status, button::Status::Hovered) {
                        p().border_strong
                    } else {
                        Color::TRANSPARENT
                    },
                    width: 1.0,
                    radius: tokens::RADIUS_MD.into(),
                },
                ..button::Style::default()
            })
            .on_press(msg(Msg::ProjectColor(c)));
        with_tip(b, t(&color_key(c)))
    });
    Row::with_children(squares).spacing(6).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: &str, color: &str) -> Project {
        Project { id: id.into(), name: id.into(), color: color.into() }
    }

    #[test]
    fn a_new_project_gets_a_colour_no_other_uses_grey_last() {
        assert_eq!(fresh_color(&[]), "blue");
        assert_eq!(fresh_color(&[p("a", "blue"), p("b", "teal")]), "green");
        let all: Vec<_> = PROJECT_COLORS.iter().filter(|c| **c != "slate").map(|c| p(c, c)).collect();
        assert_eq!(fresh_color(&all), "slate");
    }

    #[test]
    fn colours_have_names() {
        crate::i18n::set_locale(chidori_core::settings::Locale::Ja);
        assert_eq!(color_key("blue"), "colorBlue");
        for c in PROJECT_COLORS {
            assert_ne!(t(&color_key(c)), color_key(c), "{c} has no name");
        }
    }
}

#[cfg(test)]
mod previews {
    use super::*;
    use crate::preview::{app, shot};
    use crate::screens::history::previews::{fixture, session};

    #[test]
    #[ignore = "writes PNGs for review"]
    fn preview_history_dialogs() {
        let (mut app, _dir) = app();
        fixture(&mut app);
        let size = crate::app::EXPANDED;

        ask_rename(&mut app, "a1".into());
        shot(&app, "history-dialog-rename", size);
        ask_delete(&mut app, vec!["a1".into()], None);
        shot(&app, "history-dialog-delete", size);
        ask_delete_selected(&mut app, vec!["a1".into(), "b22".into()]);
        shot(&app, "history-dialog-delete-many", size);
        ask_export(&mut app, "a1".into());
        shot(&app, "history-dialog-export", size);
        app.dialogs.zip = true;
        app.dialogs.open = Some(Dialog::Export { id: "a1".into(), busy: Some(ExportKind::Pdf) });
        shot(&app, "history-dialog-export-busy", size);
        ask_new_project(&mut app, vec!["a1".into()]);
        if let Some(Dialog::Project(f)) = &mut app.dialogs.open {
            f.name = "顧客インタビュー".into();
        }
        shot(&app, "history-dialog-new-project", size);
        let first = app.projects[0].clone();
        ask_edit_project(&mut app, first.clone());
        shot(&app, "history-dialog-edit-project", size);
        ask_delete_project(&mut app, first);
        shot(&app, "history-dialog-delete-project", size);
        ask_confirm_quit(&mut app);
        shot(&app, "history-dialog-quit", size);
        app.dialogs.open = None;
        app.dialogs.recoverable = vec![session("r1", "2026-10-27T14:05:00+09:00", "Zoom 定例", "途中まで", 1_234_000)];
        shot(&app, "history-dialog-recover", size);
    }
}

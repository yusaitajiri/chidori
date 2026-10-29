//! 履歴: past sessions by day, each with its own line; searchable by title and opening words
//! (FR-62, FR-63), and narrowed to one project (FR-64), which makes the page that project's: its
//! name on top and its own menu to change or delete it. 選択 picks several to move to the Recycle
//! Bin at once, such as all the short and empty ones, or to put in a project.

use std::collections::HashSet;

use chidori_core::ops;
use chidori_core::session::model::SourceId;
use chidori_core::session::projects::Project;
use chidori_core::session::recovery::SessionSummary;
use chrono::{Datelike, Local, NaiveDate};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{Column, Row, Space, button, column, container, mouse_area, row, scrollable, stack};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Subscription, Task};

use super::dialogs::{self, project_mark};
use super::session_line;
use super::session_menu::{self, SessionRef, menu_text};
use crate::app::{App, Message, View, call};
use crate::i18n::{t, tf};
use crate::icons::{Icon, icon};
use crate::model::format::{DayName, day_name, day_of, day_text, duration_text, time_of};
use crate::model::line::{Shape, shape_from_activity};
use crate::native::menu::Item;
use crate::theme::{self, line, p, plain, tokens};
use crate::widgets::{self, Size, Variant, btn, hairline, icon_button};

const SEARCH: &str = "history-search";

/// Shorter than this, a recording is most likely a test or started by accident.
const SHORT_MS: u64 = 10_000;

#[derive(Debug, Default)]
pub struct State {
    /// None until the list first loads.
    sessions: Option<Vec<Entry>>,
    query: String,
    selecting: bool,
    selected: HashSet<String>,
    filter: Filter,
    /// The row under the pointer: its time gives way to its ⋯.
    hovered: Option<String>,
    /// Today, as of when the list loaded, for 今日 and 昨日.
    today: Option<NaiveDate>,
}

/// A row: the session and its line, built once when the list loads.
#[derive(Debug, Clone)]
struct Entry {
    s: SessionSummary,
    shape: Shape,
    /// It has both sides, so its line has 自分 below the baseline.
    two: bool,
}

impl Entry {
    fn new(s: SessionSummary) -> Self {
        let two = s.sources.contains(&SourceId::Mic) && s.sources.iter().any(|id| *id != SourceId::Mic);
        Entry { shape: shape_from_activity(&s.activity), two, s }
    }
}

/// Which sessions the list shows: all, those in no project, or one project's.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    None,
    Project(String),
}

#[derive(Debug, Clone)]
pub enum Msg {
    Loaded(Vec<SessionSummary>),
    Query(String),
    ClearQuery,
    /// Escape, `true` when the search field had it.
    Escape(bool),
    ToggleSelecting,
    Open(String),
    RowMenu(String),
    FilterMenu,
    /// The project's own menu, on its page.
    PageMenu,
    /// Selects every row shown, or none once all are.
    ChooseAll,
    ChooseEmpty,
    ChosenProjectMenu,
    DeleteChosen,
    Hover(String),
    Unhover(String),
}

fn msg(m: Msg) -> Message {
    Message::History(m)
}

/// A recording under 10 s or with no transcript: greyed out (still readable, still openable),
/// with no line.
fn is_empty_session(s: &SessionSummary) -> bool {
    s.segments == 0 || s.duration_ms < SHORT_MS
}

/// Text as search compares it: NFKC (full-width letters and digits as ASCII, half-width kana as
/// full-width), any case.
fn norm(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization as _;
    s.nfkc().flat_map(char::to_lowercase).collect()
}

fn known<'a>(projects: &'a [Project], id: Option<&str>) -> Option<&'a Project> {
    id.and_then(|id| projects.iter().find(|p| p.id == id))
}

/// A deleted project can't stay the filter.
fn effective(filter: &Filter, projects: &[Project]) -> Filter {
    match filter {
        Filter::Project(id) if known(projects, Some(id)).is_none() => Filter::All,
        f => f.clone(),
    }
}

/// The sessions the search and the filter let through, by day, in the list's order.
fn days<'a>(
    entries: &'a [Entry],
    query: &str,
    filter: &Filter,
    projects: &[Project],
) -> Vec<(&'a str, Vec<&'a Entry>)> {
    let q = norm(query.trim());
    let mut out: Vec<(&str, Vec<&Entry>)> = Vec::new();
    for e in entries {
        let project = known(projects, e.s.project.as_deref());
        let in_filter = match filter {
            Filter::All => true,
            Filter::None => project.is_none(),
            Filter::Project(id) => project.is_some_and(|p| &p.id == id),
        };
        if !in_filter || !(q.is_empty() || norm(&e.s.title).contains(&q) || norm(&e.s.preview).contains(&q)) {
            continue;
        }
        let day = day_of(&e.s.started_at);
        match out.last_mut() {
            Some((d, items)) if *d == day => items.push(e),
            _ => out.push((day, vec![e])),
        }
    }
    out
}

/// Only what the search shows can be selected, so nothing hidden is deleted.
fn visible(app: &App) -> Vec<&Entry> {
    let st = &app.history;
    let filter = effective(&st.filter, &app.projects);
    days(st.sessions.as_deref().unwrap_or(&[]), &st.query, &filter, &app.projects)
        .into_iter()
        .flat_map(|(_, items)| items)
        .collect()
}

fn chosen(app: &App) -> Vec<String> {
    visible(app).into_iter().filter(|e| app.history.selected.contains(&e.s.id)).map(|e| e.s.id.clone()).collect()
}

/// Reloads the list.
pub fn reload(app: &App) -> Task<Message> {
    call(app, ops::list_sessions, |list| msg(Msg::Loaded(list)))
}

/// History is shown: its list reloads, and the search takes the keys.
pub fn on_open(app: &mut App) -> Task<Message> {
    app.history.today = Some(Local::now().date_naive());
    let focus = if app.view == View::History { iced::widget::operation::focus(SEARCH) } else { Task::none() };
    Task::batch([reload(app), focus])
}

pub fn stop_selecting(app: &mut App) {
    app.history.selecting = false;
    app.history.selected.clear();
}

/// Shows one project's page.
pub fn show_project(app: &mut App, id: String) {
    app.history.filter = Filter::Project(id);
}

/// A session's title, if the list holds it.
pub fn title_of(app: &App, id: &str) -> Option<String> {
    app.history.sessions.as_ref()?.iter().find(|e| e.s.id == id).map(|e| e.s.title.clone())
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    let st = &mut app.history;
    match m {
        Msg::Loaded(list) => {
            let ids: HashSet<&str> = list.iter().map(|s| s.id.as_str()).collect();
            st.selected.retain(|id| ids.contains(id.as_str()));
            st.sessions = Some(list.into_iter().map(Entry::new).collect());
            Task::none()
        }
        Msg::Query(q) => {
            st.query = q;
            Task::none()
        }
        Msg::ClearQuery => {
            st.query.clear();
            Task::none()
        }
        // In the search field Escape clears the search; elsewhere it leaves selection mode.
        Msg::Escape(in_search) => {
            if in_search {
                st.query.clear();
            } else if st.selecting {
                stop_selecting(app);
            }
            Task::none()
        }
        Msg::ToggleSelecting => {
            if st.selecting {
                stop_selecting(app);
            } else {
                st.selecting = true;
            }
            Task::none()
        }
        Msg::Open(id) => {
            if st.selecting {
                if !st.selected.remove(&id) {
                    st.selected.insert(id);
                }
                Task::none()
            } else {
                Task::done(Message::OpenSession(id))
            }
        }
        Msg::RowMenu(id) => {
            let list = app.history.sessions.as_deref().unwrap_or(&[]);
            let Some(e) = list.iter().find(|e| e.s.id == id) else { return Task::none() };
            let s = SessionRef { id: &e.s.id, recoverable: e.s.recoverable, project: e.s.project.as_deref() };
            Task::done(Message::Popup(session_menu::items(app, s, true), None))
        }
        Msg::FilterMenu => Task::done(Message::Popup(filter_menu(app), None)),
        Msg::PageMenu => Task::done(Message::Popup(
            vec![
                Item::action("history:edit-project", t("editProject")),
                Item::Separator,
                Item::action("history:delete-project", t("deleteProject")),
            ],
            None,
        )),
        Msg::ChooseAll => {
            let shown: Vec<String> = visible(app).iter().map(|e| e.s.id.clone()).collect();
            let all = !shown.is_empty() && shown.iter().all(|id| app.history.selected.contains(id));
            app.history.selected = if all { HashSet::new() } else { shown.into_iter().collect() };
            Task::none()
        }
        Msg::ChooseEmpty => {
            let empties = visible(app).into_iter().filter(|e| is_empty_session(&e.s)).map(|e| e.s.id.clone()).collect();
            app.history.selected = empties;
            Task::none()
        }
        Msg::ChosenProjectMenu => {
            let ids = chosen(app);
            if ids.is_empty() {
                return Task::none();
            }
            Task::done(Message::Popup(session_menu::project_items(app, &ids, None), None))
        }
        Msg::DeleteChosen => {
            let ids = chosen(app);
            if !ids.is_empty() {
                dialogs::ask_delete_selected(app, ids);
            }
            Task::none()
        }
        Msg::Hover(id) => {
            st.hovered = Some(id);
            Task::none()
        }
        Msg::Unhover(id) => {
            if st.hovered.as_ref() == Some(&id) {
                st.hovered = None;
            }
            Task::none()
        }
    }
}

/// The project filter: all, each project, none, then a new project.
fn filter_menu(app: &App) -> Vec<Item> {
    let filter = effective(&app.history.filter, &app.projects);
    let mut items = vec![Item::check("history:filter-all", t("allProjects"), filter == Filter::All)];
    for p in &app.projects {
        let on = filter == Filter::Project(p.id.clone());
        items.push(Item::check(format!("history:filter:{}", p.id), menu_text(&p.name), on));
    }
    items.push(Item::check("history:filter-none", t("noProject"), filter == Filter::None));
    items.push(Item::Separator);
    items.push(Item::action("history:filter-new", t("newProjectMenu")));
    items
}

pub fn on_menu(app: &mut App, rest: &str) -> Task<Message> {
    let page = match effective(&app.history.filter, &app.projects) {
        Filter::Project(id) => known(&app.projects, Some(&id)).cloned(),
        _ => None,
    };
    match rest.split_once(':') {
        Some(("filter", id)) => app.history.filter = Filter::Project(id.to_string()),
        _ => match rest {
            "filter-all" => app.history.filter = Filter::All,
            "filter-none" => app.history.filter = Filter::None,
            // A new project opens as its own page, empty until sessions join it.
            "filter-new" => dialogs::ask_new_project_page(app),
            "edit-project" => {
                if let Some(p) = page {
                    dialogs::ask_edit_project(app, p);
                }
            }
            "delete-project" => {
                if let Some(p) = page {
                    dialogs::ask_delete_project(app, p);
                }
            }
            _ => {}
        },
    }
    Task::none()
}

/// Escape leaves selection mode, or clears the search from its field; a dialog takes it first.
pub fn subscription(app: &App) -> Subscription<Message> {
    if app.view != View::History || dialogs::is_open(app) {
        return Subscription::none();
    }
    iced::event::listen_with(on_key)
}

fn on_key(e: iced::Event, status: iced::event::Status, _: iced::window::Id) -> Option<Message> {
    match e {
        iced::Event::Keyboard(keyboard::Event::KeyPressed { key: Key::Named(Named::Escape), .. }) => {
            Some(msg(Msg::Escape(status == iced::event::Status::Captured)))
        }
        _ => None,
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    let st = &app.history;
    let filter = effective(&st.filter, &app.projects);
    let page = match &filter {
        Filter::Project(id) => known(&app.projects, Some(id)),
        _ => None,
    };
    let all: &[Entry] = st.sessions.as_deref().unwrap_or(&[]);
    let days = days(all, &st.query, &filter, &app.projects);
    let shown: Vec<&Entry> = days.iter().flat_map(|(_, items)| items.iter().copied()).collect();

    let list: Element<'_, Message> = match &st.sessions {
        None => loading(),
        Some(list) if list.is_empty() => empty_history(),
        Some(_) if days.is_empty() => {
            let text =
                if page.is_some() && st.query.trim().is_empty() { t("projectEmpty") } else { t("historyNoMatch") };
            container(theme::t(text, tokens::BODY_SMALL, p().ink_muted).center())
                .width(Length::Fill)
                .padding(tokens::SPACE_8)
                .center_x(Length::Fill)
                .into()
        }
        Some(_) => {
            let today = st.today.unwrap_or_else(|| Local::now().date_naive());
            let sections = days.iter().map(|(day, items)| {
                let rows = items.iter().map(|e| row_view(app, e, page.is_some()));
                column![day_header(day, today), Column::with_children(rows).spacing(2)].into()
            });
            scrollable(
                Column::with_children(sections).padding(Padding::ZERO.left(10).right(10).bottom(tokens::SPACE_3)),
            )
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new().width(6).scroller_width(6).margin(2),
            ))
            .style(scroll_style)
            .height(Length::Fill)
            .into()
        }
    };

    let mut page_col = Column::new()
        .push(header(app, page, &filter, &shown))
        .push(hairline())
        .push(container(list).width(Length::Fill).height(Length::Fill));
    if st.selecting {
        page_col = page_col.push(hairline()).push(selection_bar(app, &shown));
    }
    page_col.width(Length::Fill).height(Length::Fill).into()
}

/// The title (or the project's square and name), its count, ⋯ and 選択; the search and the
/// project filter under it, and while selecting, the quick picks.
fn header<'a>(app: &'a App, page: Option<&'a Project>, filter: &Filter, shown: &[&Entry]) -> Element<'a, Message> {
    let st = &app.history;
    let sessions = st.sessions.as_deref().unwrap_or(&[]);
    let title: Element<'a, Message> = match page {
        Some(pr) => {
            // A long name wraps rather than pushing the count and the menu out of the window.
            let width = if rough_width(&pr.name, tokens::TITLE.size) > 240.0 { Length::Fill } else { Length::Shrink };
            row![project_mark(Some(&pr.color), 14.0), theme::t(pr.name.clone(), tokens::TITLE, p().ink).width(width)]
                .spacing(10)
                .align_y(Alignment::Center)
                .width(width)
                .into()
        }
        None => line(t("historyTitle"), tokens::TITLE, p().ink).into(),
    };
    let count = match page {
        Some(pr) => Some(sessions.iter().filter(|e| e.s.project.as_deref() == Some(pr.id.as_str())).count()),
        None => (!sessions.is_empty()).then_some(sessions.len()),
    };
    let mut top = Row::new().push(title).spacing(10).align_y(Alignment::End);
    if let Some(n) = count {
        top = top.push(
            container(line(tf("historyCount", &[("n", &n)]), tokens::CAPTION, p().ink_muted))
                .padding(Padding::ZERO.bottom(5)),
        );
    }
    let mut actions = Row::new().spacing(4).align_y(Alignment::Center);
    if page.is_some() {
        actions = actions.push(icon_button(Icon::Ellipsis, t("projectMenu"), Size::Sm, Some(msg(Msg::PageMenu))));
    }
    if !sessions.is_empty() {
        let label = if st.selecting { t("done") } else { t("select") };
        actions = actions.push(btn(label, Variant::Ghost, Size::Sm, Some(msg(Msg::ToggleSelecting))));
    }
    let top = row![container(top).width(Length::Fill), actions].align_y(Alignment::Center);

    let mut search_row = Row::new().push(search_box(&st.query)).spacing(6);
    if !app.projects.is_empty() {
        search_row = search_row.push(filter_button(page, filter));
    }

    let mut col = Column::new().push(top).push(Space::new().height(10)).push(search_row);
    if st.selecting {
        let all = !shown.is_empty() && shown.iter().all(|e| st.selected.contains(&e.s.id));
        let empties = shown.iter().filter(|e| is_empty_session(&e.s)).count();
        let mut picks = Row::new()
            .push(link(if all { t("selectNone") } else { t("selectAll") }, msg(Msg::ChooseAll)))
            .spacing(tokens::SPACE_4);
        if empties > 0 {
            picks = picks.push(link(tf("selectEmpty", &[("n", &empties)]), msg(Msg::ChooseEmpty)));
        }
        col = col.push(Space::new().height(tokens::SPACE_2)).push(container(picks).padding([0, 2]));
    }
    container(col)
        .width(Length::Fill)
        .padding(Padding::new(tokens::SPACE_4).top(tokens::SPACE_3).bottom(tokens::SPACE_3))
        .into()
}

/// The search field: the magnifier inside its left end, a clear button at its right once there is
/// something to clear.
fn search_box(query: &str) -> Element<'_, Message> {
    let field = widgets::input(&t("searchHistory"), query, |q| msg(Msg::Query(q)))
        .id(SEARCH)
        .line_height(iced::widget::text::LineHeight::Absolute(18.0.into()))
        .padding(Padding { top: 10.0, bottom: 10.0, left: 36.0, right: 32.0 })
        .width(Length::Fill);
    let glass = container(icon(Icon::Search, 15.0, p().ink_muted))
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding(Padding::ZERO.left(tokens::SPACE_3));
    let mut layers = stack![field, glass].width(Length::Fill);
    if !query.is_empty() {
        let clear = button(container(icon(Icon::X, 12.0, p().ink_muted)).center(20))
            .width(20)
            .height(20)
            .padding(0)
            .style(|_, status| button::Style {
                background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                    .then_some(Background::Color(p().surface_raised)),
                border: Border { radius: 10.0.into(), ..Border::default() },
                ..button::Style::default()
            })
            .on_press(msg(Msg::ClearQuery));
        let clear = widgets::with_tip(clear, t("close"));
        layers = layers.push(
            container(clear)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::End)
                .align_y(Alignment::Center)
                .padding(Padding::ZERO.right(tokens::SPACE_2)),
        );
    }
    layers.into()
}

/// The project filter beside the search, shown once a project exists: what it narrows to, in ink
/// with its square when it narrows.
fn filter_button<'a>(page: Option<&'a Project>, filter: &Filter) -> Element<'a, Message> {
    let narrowed = *filter != Filter::All;
    let name = match (page, filter) {
        (Some(p), _) => p.name.clone(),
        (None, Filter::None) => t("noProject"),
        _ => t("project"),
    };
    let mut content = Row::new().spacing(6).align_y(Alignment::Center);
    if narrowed {
        content = content.push(project_mark(page.map(|p| p.color.as_str()), 10.0));
    }
    let font = if narrowed { theme::SANS_MEDIUM } else { theme::SANS };
    // Past its widest the button keeps its chevron and shortens the name.
    let room = 140.0 - 20.0 - 19.0 - if narrowed { 16.0 } else { 0.0 };
    let name = super::form::fit(&name, room, tokens::CAPTION.size);
    content = content.push(container(plain(name, tokens::CAPTION).font(font)).max_width(room).clip(true)).push(icon(
        Icon::ChevronDown,
        13.0,
        p().ink_muted,
    ));
    let b = button(container(content).height(Length::Fill).center_y(Length::Fill))
        .height(38)
        .padding([0.0, 10.0])
        .style(move |_, status| {
            let p = p();
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: None,
                text_color: if narrowed { p.ink } else { p.ink_muted },
                border: Border {
                    color: if narrowed {
                        p.ink
                    } else if hot {
                        p.border_strong
                    } else {
                        p.border
                    },
                    width: 1.0,
                    radius: tokens::RADIUS_MD.into(),
                },
                ..button::Style::default()
            }
        })
        .on_press(msg(Msg::FilterMenu));
    container(b).max_width(140).into()
}

/// About how wide a line of text is: Japanese characters are square, Latin ones narrower.
fn rough_width(s: &str, size: f32) -> f32 {
    s.chars().map(|c| if c.is_ascii() { 0.6 } else { 1.0 }).sum::<f32>() * size
}

/// A quiet text button: muted, ink when pointed at.
fn link<'a>(label: String, on: Message) -> Element<'a, Message> {
    button(plain(label, tokens::CAPTION).font(theme::SANS_MEDIUM))
        .padding(0)
        .style(|_, status| button::Style {
            text_color: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                p().ink
            } else {
                p().ink_muted
            },
            ..button::Style::default()
        })
        .on_press(on)
        .into()
}

/// 今日 or 昨日 with the date, or the date alone.
fn day_header<'a>(day: &str, today: NaiveDate) -> Element<'a, Message> {
    let name = day_name(day, today).map(|d| match d {
        DayName::Today => t("today"),
        DayName::Yesterday => t("yesterday"),
    });
    let date = day_text(day, today.year());
    let mut r = Row::new().spacing(tokens::SPACE_2).align_y(Alignment::Center);
    let date = match name {
        Some(name) => {
            r = r.push(line(name, tokens::CAPTION, p().ink).font(theme::SANS_MEDIUM));
            line(date, tokens::CAPTION, p().ink_muted)
        }
        None => line(date, tokens::CAPTION, p().ink).font(theme::SANS_MEDIUM),
    };
    container(r.push(date))
        .padding(Padding { top: 14.0, bottom: tokens::SPACE_1, left: tokens::SPACE_2, right: tokens::SPACE_2 })
        .into()
}

/// One past session: its title (after its project's square, unless the page is that project's),
/// time, line, opening words and length. While selecting, a tick leads and a click ticks it.
fn row_view<'a>(app: &'a App, e: &'a Entry, in_project: bool) -> Element<'a, Message> {
    let st = &app.history;
    let s = &e.s;
    let empty = is_empty_session(s);
    let checked = st.selecting && st.selected.contains(&s.id);
    let hovered = st.hovered.as_deref() == Some(s.id.as_str());
    let ink = if empty { p().ink_muted } else { p().ink };

    let mut title = Row::new().spacing(tokens::SPACE_2).align_y(Alignment::Center);
    if !in_project && let Some(project) = known(&app.projects, s.project.as_deref()) {
        title = title.push(widgets::with_tip(project_mark(Some(&project.color), 10.0), project.name.clone()));
    }
    let style = if empty { tokens::BODY_SMALL } else { tokens::LABEL };
    title = title.push(container(line(s.title.clone(), style, ink)).clip(true));
    if s.recoverable {
        title = title.push(
            container(line(t("needsRecovery"), tokens::CAPTION, p().warning).font(theme::SANS_MEDIUM))
                .padding([0.0, 6.0])
                .style(|_| container::Style {
                    background: Some(p().warning_tint.into()),
                    border: Border { radius: tokens::RADIUS_SM.into(), ..Border::default() },
                    ..container::Style::default()
                }),
        );
    }
    // The time gives way to the row's ⋯ while the row is pointed at.
    let time_color = if hovered && !st.selecting { Color::TRANSPARENT } else { p().ink_muted };
    let top = row![
        container(title).width(Length::Fill).clip(true),
        line(time_of(&s.started_at), tokens::CAPTION, time_color).font(theme::MONO)
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let preview = if s.preview.is_empty() { t("emptyTranscript") } else { s.preview.clone() };
    let bottom = row![
        // Ends in an ellipsis at the width it gets, instead of a glyph cut in half beside the length.
        iced::widget::responsive(move |size| {
            container(line(
                super::form::fit(&preview, size.width, tokens::CAPTION.size),
                tokens::CAPTION,
                p().ink_muted,
            ))
            .width(Length::Fill)
            .clip(true)
            .into()
        })
        .height(Length::Shrink),
        line(duration_text(s.duration_ms), tokens::CAPTION, p().ink_muted)
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let mut body = Column::new().push(top);
    if !empty {
        body = body.push(container(session_line::small(&e.shape, e.two)).padding(Padding::ZERO.top(6).bottom(4)));
    }
    body = body.push(bottom).width(Length::Fill);

    let mut content = Row::new().spacing(tokens::SPACE_3).align_y(Alignment::Center);
    if st.selecting {
        content = content.push(tick(checked));
    }
    content = content.push(body);

    let lit = checked || (hovered && !st.selecting);
    let row_button = button(content)
        .width(Length::Fill)
        .padding(Padding { top: 10.0, bottom: tokens::SPACE_2, left: tokens::SPACE_2, right: tokens::SPACE_2 })
        .style(move |_, status| button::Style {
            background: (lit || matches!(status, button::Status::Hovered | button::Status::Pressed))
                .then_some(Background::Color(p().surface_sunken)),
            text_color: p().ink,
            border: Border { radius: tokens::RADIUS_LG.into(), ..Border::default() },
            ..button::Style::default()
        })
        .on_press(msg(Msg::Open(s.id.clone())));

    let mut layers = stack![row_button].width(Length::Fill);
    if hovered && !st.selecting {
        let more = icon_button(Icon::Ellipsis, t("moreActions"), Size::Sm, Some(msg(Msg::RowMenu(s.id.clone()))));
        layers = layers.push(container(more).width(Length::Fill).align_x(Alignment::End).padding(6));
    }
    mouse_area(layers).on_enter(msg(Msg::Hover(s.id.clone()))).on_exit(msg(Msg::Unhover(s.id.clone()))).into()
}

/// A row's checkbox in selection mode: a rounded square, ticked in ink.
fn tick<'a>(on: bool) -> Element<'a, Message> {
    let mark: Element<'a, Message> = if on { icon(Icon::Check, 12.0, p().surface).into() } else { Space::new().into() };
    container(mark)
        .center(18)
        .style(move |_| {
            let p = p();
            container::Style {
                background: Some(if on { p.ink } else { p.surface_raised }.into()),
                border: Border { color: if on { p.ink } else { p.border_strong }, width: 1.5, radius: 4.0.into() },
                ..container::Style::default()
            }
        })
        .into()
}

/// While the list loads: three quiet blocks where rows will be.
fn loading<'a>() -> Element<'a, Message> {
    let block = || {
        container(Space::new()).width(Length::Fill).height(86).style(|_| container::Style {
            background: Some(p().surface_sunken.into()),
            border: Border { radius: tokens::RADIUS_LG.into(), ..Border::default() },
            ..container::Style::default()
        })
    };
    column![block(), block(), block()].spacing(tokens::SPACE_2).padding(Padding::new(14.0).top(tokens::SPACE_3)).into()
}

fn empty_history<'a>() -> Element<'a, Message> {
    let body = column![
        icon(Icon::History, 26.0, p().ink_muted),
        theme::t(t("historyEmpty"), tokens::BODY_SMALL, p().ink_muted)
    ]
    .spacing(tokens::SPACE_3)
    .align_x(Alignment::Center);
    container(body).center(Length::Fill).padding(Padding::ZERO.bottom(40)).into()
}

/// Under the list while selecting: how many are chosen, then プロジェクト and 削除 for them.
fn selection_bar<'a>(app: &'a App, shown: &[&Entry]) -> Element<'a, Message> {
    let n = shown.iter().filter(|e| app.history.selected.contains(&e.s.id)).count();
    let some = n > 0;
    // Under a dialog the dialog holds the view's primary.
    let delete = if dialogs::is_open(app) { Variant::Secondary } else { Variant::Primary };
    let r = row![
        container(line(tf("selectedCount", &[("n", &n)]), tokens::CAPTION, p().ink_muted)).width(Length::Fill),
        btn(t("project"), Variant::Secondary, Size::Lg, some.then(|| msg(Msg::ChosenProjectMenu))),
        btn(t("delete"), delete, Size::Lg, some.then(|| msg(Msg::DeleteChosen))),
    ]
    .spacing(tokens::SPACE_2)
    .align_y(Alignment::Center);
    container(r).padding([10.0, 18.0]).into()
}

fn scroll_style(theme: &iced::Theme, status: scrollable::Status) -> scrollable::Style {
    let mut s = scrollable::default(theme, status);
    let p = p();
    let hot = matches!(
        status,
        scrollable::Status::Hovered { is_vertical_scrollbar_hovered: true, .. } | scrollable::Status::Dragged { .. }
    );
    s.vertical_rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(if hot { p.ink_subtle } else { p.border_strong }),
            border: Border { radius: 3.0.into(), ..Border::default() },
        },
    };
    s
}

#[cfg(test)]
mod tests {
    use chidori_core::session::recovery::Activity;

    use super::*;

    fn s(id: &str, started_at: &str, title: &str, preview: &str) -> SessionSummary {
        SessionSummary {
            id: id.into(),
            title: title.into(),
            started_at: started_at.into(),
            duration_ms: 60_000,
            folder: String::new(),
            segments: 3,
            screenshots: 0,
            recoverable: false,
            sources: vec![SourceId::App, SourceId::Mic],
            preview: preview.into(),
            activity: Activity { others: vec![], me: vec![], shots: vec![] },
            project: None,
        }
    }

    fn ids<'a>(days: &[(&'a str, Vec<&'a Entry>)]) -> Vec<(&'a str, Vec<&'a str>)> {
        days.iter().map(|(d, items)| (*d, items.iter().map(|e| e.s.id.as_str()).collect())).collect()
    }

    #[test]
    fn groups_by_day_in_order() {
        let list: Vec<Entry> = [
            s("a", "2026-10-03T15:00:00+09:00", "A", ""),
            s("b", "2026-10-03T09:00:00+09:00", "B", ""),
            s("c", "2026-10-02T20:00:00+09:00", "C", ""),
        ]
        .into_iter()
        .map(Entry::new)
        .collect();
        assert_eq!(
            ids(&days(&list, "", &Filter::All, &[])),
            [("2026-10-03", vec!["a", "b"]), ("2026-10-02", vec!["c"])]
        );
    }

    #[test]
    fn searches_titles_and_opening_words_in_any_case_and_width() {
        let list: Vec<Entry> = [
            s("a", "2026-10-03T15:00:00+09:00", "Zoom 定例", "来週の予定"),
            s("b", "2026-10-03T09:00:00+09:00", "講義", "ＧＰＵの話"),
        ]
        .into_iter()
        .map(Entry::new)
        .collect();
        let found =
            |q: &str| ids(&days(&list, q, &Filter::All, &[])).into_iter().flat_map(|(_, i)| i).collect::<Vec<_>>();
        assert_eq!(found("zoom"), ["a"]);
        assert_eq!(found(" 予定 "), ["a"]);
        assert_eq!(found("gpu"), ["b"]);
        assert_eq!(found("nothing"), Vec::<&str>::new());
    }

    #[test]
    fn filters_by_project_and_forgets_a_deleted_one() {
        let projects = [Project { id: "p1".into(), name: "P".into(), color: "blue".into() }];
        let mut a = s("a", "2026-10-03T15:00:00+09:00", "A", "");
        a.project = Some("p1".into());
        let mut b = s("b", "2026-10-03T09:00:00+09:00", "B", "");
        b.project = Some("gone".into());
        let list: Vec<Entry> =
            [a, b, s("c", "2026-10-03T08:00:00+09:00", "C", "")].into_iter().map(Entry::new).collect();
        let found = |f: &Filter| {
            ids(&days(&list, "", &effective(f, &projects), &projects))
                .into_iter()
                .flat_map(|(_, i)| i)
                .collect::<Vec<_>>()
        };
        assert_eq!(found(&Filter::Project("p1".into())), ["a"]);
        // A session whose project was deleted is in none.
        assert_eq!(found(&Filter::None), ["b", "c"]);
        assert_eq!(found(&Filter::Project("gone".into())), ["a", "b", "c"]);
    }

    #[test]
    fn short_and_empty_sessions_are_greyed() {
        let mut short = s("a", "2026-10-03T15:00:00+09:00", "A", "");
        short.duration_ms = 9_000;
        let mut silent = s("b", "2026-10-03T15:00:00+09:00", "B", "");
        silent.segments = 0;
        assert!(is_empty_session(&short));
        assert!(is_empty_session(&silent));
        assert!(!is_empty_session(&s("c", "2026-10-03T15:00:00+09:00", "C", "")));
    }

    #[test]
    fn a_session_with_both_sides_draws_two() {
        assert!(Entry::new(s("a", "2026-10-03T15:00:00+09:00", "A", "")).two);
        let mut mic = s("b", "2026-10-03T15:00:00+09:00", "B", "");
        mic.sources = vec![SourceId::Mic];
        assert!(!Entry::new(mic).two);
    }
}

#[cfg(test)]
pub(crate) mod previews {
    use chidori_core::session::recovery::Activity;

    use super::*;
    use crate::preview::{app, shot};

    /// A session's activity like a conversation: 相手 and 自分 taking turns.
    fn activity(seed: u32) -> Activity {
        let wave = |i: usize, phase: f32| {
            let x = i as f32 / 9.0 + phase;
            ((x.sin() * 0.5 + 0.5) * (x * 0.37).cos().abs() * 100.0) as u8
        };
        Activity {
            others: (0..128).map(|i| wave(i, seed as f32)).collect(),
            me: (0..128).map(|i| wave(i, seed as f32 + 2.0) / 2).collect(),
            shots: vec![20, 64, 101],
        }
    }

    pub(crate) fn session(id: &str, started_at: &str, title: &str, preview: &str, ms: u64) -> SessionSummary {
        SessionSummary {
            id: id.into(),
            title: title.into(),
            started_at: started_at.into(),
            duration_ms: ms,
            folder: String::new(),
            segments: if preview.is_empty() { 0 } else { 12 },
            screenshots: 3,
            recoverable: false,
            sources: vec![SourceId::App, SourceId::Mic],
            preview: preview.into(),
            activity: activity(id.len() as u32),
            project: None,
        }
    }

    pub(crate) fn fixture(app: &mut App) {
        let today = Local::now().date_naive();
        let at = |days: i64, hm: &str| format!("{}T{hm}:00+09:00", today - chrono::Duration::days(days));
        app.projects = vec![
            Project { id: "p1".into(), name: "定例ミーティング".into(), color: "blue".into() },
            Project { id: "p2".into(), name: "講義".into(), color: "green".into() },
        ];
        let mut a = session(
            "a1",
            &at(0, "15:13"),
            "Zoom 定例",
            "それでは今週の進捗から確認していきます。まず開発チームから",
            3_120_000,
        );
        a.project = Some("p1".into());
        let mut b = session("b22", &at(0, "10:02"), "Chrome", "", 6_000);
        b.sources = vec![SourceId::App];
        let mut c = session(
            "c333",
            &at(1, "19:40"),
            "機械学習 第5回",
            "前回は勾配降下法の話をしました。今日は正則化について",
            5_400_000,
        );
        c.project = Some("p2".into());
        c.recoverable = true;
        let d =
            session("d4444", &at(9, "13:30"), "Teams 面談", "本日はお時間をいただきありがとうございます", 1_860_000);
        app.history.sessions = Some([a, b, c, d].into_iter().map(Entry::new).collect());
        app.history.today = Some(today);
        app.view = View::History;
    }

    #[test]
    #[ignore = "writes PNGs for review"]
    fn preview_history() {
        let (mut app, _dir) = app();
        fixture(&mut app);
        shot(&app, "history", crate::app::EXPANDED);

        app.history.hovered = Some("a1".into());
        shot(&app, "history-hover", crate::app::EXPANDED);
        app.history.hovered = None;

        app.history.selecting = true;
        app.history.selected = ["a1".to_string(), "b22".to_string()].into_iter().collect();
        shot(&app, "history-select", crate::app::EXPANDED);
        stop_selecting(&mut app);

        app.history.filter = Filter::Project("p1".into());
        app.history.query = "zoom".into();
        shot(&app, "history-project", crate::app::EXPANDED);
        app.history.query = "nothing".into();
        shot(&app, "history-nomatch", crate::app::EXPANDED);

        app.history.filter = Filter::All;
        app.history.query.clear();
        app.history.sessions = None;
        shot(&app, "history-loading", crate::app::EXPANDED);
        app.history.sessions = Some(Vec::new());
        app.projects.clear();
        shot(&app, "history-empty", crate::app::EXPANDED);
    }
}

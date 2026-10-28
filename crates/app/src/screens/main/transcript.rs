//! The transcript: a list that builds only the rows in view (a long session stays as cheap as a
//! short one), following the newest line until the reader scrolls up (FR-40). Turns name the
//! speaker only when both sides were recorded; the words still coming are pencil grey and darken
//! into place; screenshots sit where they were taken; markers are rules with what happened. A
//! finished session's lines can be played from, starred, edited and deleted; a live one's starred.
//! The compact window shows the last two lines instead.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use chidori_core::error::AppError;
use chidori_core::events::UiState;
use chidori_core::host::AppHandle;
use chidori_core::ops;
use chidori_core::session::model::{Marker, MarkerKind, Screenshot, Segment, SourceId, TimelineItem};
use iced::widget::operation::{self, AbsoluteOffset};
use iced::widget::scrollable::{self, Viewport};
use iced::widget::text::Wrapping;
use iced::widget::{
    Column, Id, Space, button, column, container, hover, image, lazy, row, sensor, stack, text_editor, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Task};

use super::rows::{self, Row, RowKind, Rows};
use crate::app::{App, Message};
use crate::i18n::{t, tf};
use crate::icons::{Icon, icon};
use crate::model::format::{day_of, day_text};
use crate::model::transcript::Transcript;
use crate::theme::{self, line, mix, p, tokens};
use crate::widgets::{Size, Variant, btn_with, floating, frame, spinner, with_tip};

/// How far below the expanded deck's top its line lies; the transcript runs on under the deck down
/// to it, so only the tint under the line covers the text.
pub const LINE_Y: f32 = 30.0;
/// Speech this far behind is shown as being written. A fast GPU writes a line well within it, so
/// the note only appears where text is slow to come, such as on a CPU, which has no grey partials.
const WRITING_MS: u64 = 1_500;
/// The room the note takes under the last line.
const WRITING_H: f32 = 28.0;
/// The widest the lines run, however wide the window.
const MAX_W: f32 = 760.0;
/// Rows built beyond the view, above and below.
const OVERSCAN: f32 = 360.0;
/// The side padding of a row.
const PAD_X: f32 = 18.0;

pub type Wrap = fn(ListMsg) -> Message;

/// A line being edited.
pub struct Edit {
    pub id: String,
    pub content: text_editor::Content,
}

/// The list's state: where it is scrolled, what each row measured, what is being edited.
pub struct ListState {
    pub id: Id,
    editor_id: Id,
    title_id: Id,
    offset: f32,
    viewport_h: f32,
    width: f32,
    /// The view keeps up with the newest line (live only)...
    following: Cell<bool>,
    /// ...held at the bottom by the list's own anchor, with only the last rows in it, as it is
    /// whenever the list first shows; once the reader has scrolled, it is held by snapping.
    anchored: Cell<bool>,
    /// Where the anchored rows start in the whole list.
    anchored_top: Cell<f32>,
    last_top: f32,
    heights: HashMap<String, f32>,
    heights_version: u64,
    cache: RefCell<Option<Cache>>,
    pub editing: Option<Edit>,
    /// A screenshot's caption being written: its ID and the draft.
    pub caption: Option<(String, String)>,
    /// A row pointed at on the session's line, marked for a moment.
    pub marked: Option<(String, Instant)>,
    /// The line being played.
    pub playing: Option<String>,
    thumbs: HashMap<String, image::Handle>,
    decoding: HashSet<String>,
    /// The title being renamed (the header's or the recording's).
    pub title: Option<String>,
    /// 詳細 is open under the finished session's header.
    pub details: bool,
}

struct Cache {
    sig: u64,
    rows: Rc<Rows>,
    width: u32,
    heights_version: u64,
    /// Where each row starts, and after the last, where the rows end.
    tops: Rc<Vec<f32>>,
}

impl ListState {
    pub fn new(id: &'static str, editor: &'static str, title: &'static str) -> Self {
        Self {
            id: Id::new(id),
            editor_id: Id::new(editor),
            title_id: Id::new(title),
            offset: 0.0,
            viewport_h: 0.0,
            width: 0.0,
            following: Cell::new(true),
            anchored: Cell::new(true),
            anchored_top: Cell::new(0.0),
            last_top: 0.0,
            heights: HashMap::new(),
            heights_version: 0,
            cache: RefCell::new(None),
            editing: None,
            caption: None,
            marked: None,
            playing: None,
            thumbs: HashMap::new(),
            decoding: HashSet::new(),
            title: None,
            details: false,
        }
    }

    /// A different session: its own rows, measures and pictures.
    pub fn reset(&mut self) {
        self.heights.clear();
        self.heights_version += 1;
        self.thumbs.clear();
        self.decoding.clear();
        self.editing = None;
        self.caption = None;
        self.marked = None;
        self.playing = None;
        self.title = None;
        self.details = false;
        self.forget();
        self.offset = 0.0;
        self.last_top = 0.0;
    }

    /// The rows of a transcript, built again only when it changed.
    pub fn rows(&self, tr: &Transcript, header: bool) -> Rc<Rows> {
        self.layout(tr, header).0
    }

    fn layout(&self, tr: &Transcript, header: bool) -> (Rc<Rows>, Rc<Vec<f32>>) {
        let sig = rows::signature(tr, header);
        let width = self.list_width().round() as u32;
        let mut cache = self.cache.borrow_mut();
        let fresh = match &*cache {
            Some(c) if c.sig == sig => {
                if c.width == width && c.heights_version == self.heights_version {
                    return (c.rows.clone(), c.tops.clone());
                }
                c.rows.clone()
            }
            _ => Rc::new(rows::build(tr, header)),
        };
        let w = width as f32;
        let mut tops = Vec::with_capacity(fresh.rows.len() + 1);
        let mut y = 0.0;
        for r in &fresh.rows {
            tops.push(y);
            y += self.heights.get(&r.key).copied().unwrap_or_else(|| estimate(tr, r, w));
        }
        tops.push(y);
        let tops = Rc::new(tops);
        *cache =
            Some(Cache { sig, rows: fresh.clone(), width, heights_version: self.heights_version, tops: tops.clone() });
        (fresh, tops)
    }

    /// The list is off screen: when it shows again it starts at the newest line.
    pub fn forget(&self) {
        self.following.set(true);
        self.anchored.set(true);
    }

    /// Stops following (a finished session opens at its header).
    pub fn stay(&self) {
        self.following.set(false);
        self.anchored.set(false);
    }

    /// The box a title is renamed in.
    pub fn title_input(&self) -> Id {
        self.title_id.clone()
    }

    fn list_width(&self) -> f32 {
        if self.width > 0.0 { self.width.min(MAX_W) } else { crate::app::EXPANDED.width }
    }

    /// Brings a row to the middle of the view.
    pub fn scroll_to_row(&self, tr: &Transcript, header: bool, index: usize) -> Task<Message> {
        let (_, tops) = self.layout(tr, header);
        let (Some(top), Some(bottom)) = (tops.get(index), tops.get(index + 1)) else { return Task::none() };
        let view = if self.viewport_h > 0.0 { self.viewport_h } else { 400.0 };
        let y = (top - (view - (bottom - top)) / 2.0).max(0.0);
        operation::scroll_to(self.id.clone(), AbsoluteOffset { x: None, y: Some(y) })
    }

    /// Whether a row is in view, with `clear` px kept free at the bottom (the deck's line).
    fn shows(&self, tops: &[f32], index: usize, clear: f32) -> bool {
        match (tops.get(index), tops.get(index + 1)) {
            (Some(a), Some(b)) => *b > self.offset && *a < self.offset + self.viewport_h - clear,
            _ => false,
        }
    }

    /// While playing, the transcript keeps up with the sound, unless the reader has scrolled away
    /// from the line that was playing.
    pub fn follow_playing(&mut self, tr: &Transcript, header: bool, key: Option<String>) -> Task<Message> {
        let prev = std::mem::replace(&mut self.playing, key.clone());
        let Some(key) = key else { return Task::none() };
        if prev.as_deref() == Some(key.as_str()) {
            return Task::none();
        }
        let (rows, tops) = self.layout(tr, header);
        let index_of = |k: &str| rows.rows.iter().position(|r| r.key == k);
        let Some(index) = index_of(&key) else { return Task::none() };
        let was_shown = prev.as_deref().and_then(index_of).is_none_or(|i| self.shows(&tops, i, LINE_Y));
        if was_shown && !self.shows(&tops, index, LINE_Y) {
            self.scroll_to_row(tr, header, index)
        } else {
            Task::none()
        }
    }
}

/// How tall a row is likely to be before it is drawn and measured: its text wrapped by a rough
/// width per character (a CJK character is a full em, ASCII about half).
fn estimate(tr: &Transcript, r: &Row, width: f32) -> f32 {
    let text_w = (width - 2.0 * PAD_X).max(40.0);
    let lines = |s: &str| -> f32 {
        let em = tokens::BODY.size;
        let mut n = 1.0;
        let mut x = 0.0;
        for c in s.chars() {
            let w = if c.is_ascii() { em * 0.55 } else { em };
            if x + w > text_w {
                n += 1.0;
                x = 0.0;
            }
            x += w;
        }
        n
    };
    let body = |text: &str| (if r.head { 34.0 } else { 2.0 }) + lines(text) * tokens::BODY.line_height + 2.0;
    match r.kind {
        RowKind::Header => 150.0,
        RowKind::Partial(s) => body(tr.partials.get(&s).map_or("", |p| p.text.as_str())),
        RowKind::Item(i) => match &tr.items[i] {
            TimelineItem::Segment(s) => body(&s.text),
            TimelineItem::Screenshot(s) => {
                14.0 + 22.0 + image_size(s, text_w).1 + if s.caption.is_some() { 24.0 } else { 0.0 } + 4.0
            }
            TimelineItem::Marker(m) => {
                if m.kind == MarkerKind::Cut {
                    50.0
                } else {
                    42.0
                }
            }
        },
    }
}

/// A screenshot's size in the list: at most 220 px high and the width of the lines.
fn image_size(s: &Screenshot, max_w: f32) -> (f32, f32) {
    let (w, h) = (s.width.max(1) as f32, s.height.max(1) as f32);
    let height = 220.0_f32.min(max_w * h / w);
    (height * w / h, height)
}

#[derive(Debug, Clone)]
pub enum ListMsg {
    Scrolled(Viewport),
    Measured(String, f32),
    JumpLatest,
    PlayFrom(u64),
    Seek(u64),
    Unmark,
    Star(String, bool),
    Edit(String),
    EditAction(text_editor::Action),
    SaveEdit,
    CancelEdit,
    DeleteLine(String),
    Caption(String),
    CaptionInput(String),
    SaveCaption,
    DeleteShot(String),
    OpenImage(PathBuf),
    Thumb(String, Option<image::Handle>),
    /// A line came back changed (edited or starred).
    Changed(Result<Segment, AppError>),
    /// An item was deleted, or its caption saved.
    Removed(String, Result<(), AppError>),
    CaptionSaved(String, String, Result<(), AppError>),
    RenameStart,
    TitleInput(String),
    TitleSave,
    TitleSaved(String, Result<(), AppError>),
    Details,
    Escape,
}

/// Runs a blocking core call off the UI thread.
pub fn run<T: Send + 'static>(
    core: &AppHandle,
    f: impl FnOnce(&AppHandle) -> T + Send + 'static,
    map: impl FnOnce(T) -> Message + Send + 'static,
) -> Task<Message> {
    let core = core.clone();
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

fn failed(e: AppError) -> Message {
    Message::Failed(e, "saveFailed")
}

/// What a list's messages change: its state and its transcript. `live` follows the newest line;
/// `header` says whether the rows start with the session's header.
pub fn update(
    core: &AppHandle,
    list: &mut ListState,
    tr: &mut Transcript,
    msg: ListMsg,
    wrap: Wrap,
    live: bool,
    header: bool,
) -> Task<Message> {
    let Some(sid) = tr.session_id.clone() else { return Task::none() };
    match msg {
        ListMsg::Scrolled(vp) => {
            let h = vp.bounds().height;
            let content = vp.content_bounds().height;
            list.viewport_h = h;
            if (vp.bounds().width - list.width).abs() > 0.5 {
                list.width = vp.bounds().width;
                list.heights.clear();
                list.heights_version += 1;
            }
            if live && list.anchored.get() {
                // Anchored, the offset counts from the bottom.
                let gap = vp.absolute_offset().y;
                if gap <= 32.0 {
                    return Task::none();
                }
                // Scrolled up: the whole list takes over, where the reader is.
                let top = list.anchored_top.get() + (content - h - gap).max(0.0);
                list.following.set(false);
                list.anchored.set(false);
                list.offset = top;
                list.last_top = top;
                return operation::scroll_to(list.id.clone(), AbsoluteOffset { x: None, y: Some(top) });
            }
            let top = vp.absolute_offset().y;
            list.offset = top;
            let gap = content - top - h;
            if gap <= 32.0 {
                list.following.set(true);
            } else if top < list.last_top - 2.0 {
                list.following.set(false);
            }
            list.last_top = top;
            if live && list.following.get() && gap > 1.0 {
                operation::snap_to_end(list.id.clone())
            } else {
                Task::none()
            }
        }
        ListMsg::Measured(key, h) => {
            let old = list.heights.insert(key.clone(), h);
            if old.is_none_or(|o| (o - h).abs() > 0.5) {
                list.heights_version += 1;
            }
            // A saved screenshot is read from its file once it comes into view.
            let shot = tr.items.iter().find_map(|i| match i {
                TimelineItem::Screenshot(s) if s.id == key => Some(s.clone()),
                _ => None,
            });
            match (shot, &tr.folder) {
                (Some(s), Some(folder))
                    if !tr.thumbs.contains_key(&s.id)
                        && !list.thumbs.contains_key(&s.id)
                        && list.decoding.insert(s.id.clone()) =>
                {
                    let path = crate::app::session_file(folder, &s.file);
                    let id = s.id.clone();
                    Task::future(async move {
                        let handle = chidori_core::host::runtime()
                            .spawn_blocking(move || decode_thumb(&path))
                            .await
                            .ok()
                            .flatten();
                        wrap(ListMsg::Thumb(id, handle))
                    })
                }
                _ => Task::none(),
            }
        }
        ListMsg::Thumb(id, handle) => {
            list.decoding.remove(&id);
            if let Some(h) = handle {
                list.thumbs.insert(id, h);
            }
            Task::none()
        }
        ListMsg::JumpLatest => {
            list.following.set(true);
            operation::snap_to_end(list.id.clone())
        }
        // Played by the screen, which owns the player.
        ListMsg::PlayFrom(_) => Task::none(),
        ListMsg::Seek(ms) => {
            let rows = list.rows(tr, header);
            let Some(index) = rows::seek_index(tr, &rows, ms) else { return Task::none() };
            list.marked = Some((rows.rows[index].key.clone(), Instant::now()));
            list.stay();
            list.scroll_to_row(tr, header, index)
        }
        ListMsg::Unmark => {
            if list.marked.as_ref().is_some_and(|(_, at)| at.elapsed() >= MARK_FOR) {
                list.marked = None;
            }
            Task::none()
        }
        ListMsg::Star(id, important) => {
            run(core, move |c| ops::mark_segment(c, &sid, &id, important), move |r| wrap(ListMsg::Changed(r)))
        }
        ListMsg::Changed(Ok(seg)) => {
            if list.editing.as_ref().is_some_and(|e| e.id == seg.id) {
                list.editing = None;
            }
            tr.update_segment(seg);
            Task::none()
        }
        ListMsg::Changed(Err(e))
        | ListMsg::Removed(_, Err(e))
        | ListMsg::CaptionSaved(_, _, Err(e))
        | ListMsg::TitleSaved(_, Err(e)) => Task::done(failed(e)),
        ListMsg::Edit(id) => {
            let text = tr.items.iter().find_map(|i| match i {
                TimelineItem::Segment(s) if s.id == id => Some(s.text.clone()),
                _ => None,
            });
            let Some(text) = text else { return Task::none() };
            list.editing = Some(Edit { id, content: text_editor::Content::with_text(&text) });
            operation::focus(list.editor_id.clone())
        }
        ListMsg::EditAction(a) => {
            if let Some(e) = &mut list.editing {
                e.content.perform(a);
            }
            Task::none()
        }
        ListMsg::SaveEdit => {
            let Some(e) = &list.editing else { return Task::none() };
            let (id, text) = (e.id.clone(), e.content.text());
            run(core, move |c| ops::update_segment(c, &sid, &id, &text), move |r| wrap(ListMsg::Changed(r)))
        }
        ListMsg::CancelEdit => {
            list.editing = None;
            Task::none()
        }
        ListMsg::DeleteLine(id) => run(
            core,
            move |c| ops::delete_segment(c, &sid, &id).map(|_| id),
            move |r| match r {
                Ok(id) => wrap(ListMsg::Removed(id, Ok(()))),
                Err(e) => wrap(ListMsg::Removed(String::new(), Err(e))),
            },
        ),
        ListMsg::DeleteShot(id) => run(
            core,
            move |c| ops::delete_screenshot(c, &sid, &id).map(|_| id),
            move |r| match r {
                Ok(id) => wrap(ListMsg::Removed(id, Ok(()))),
                Err(e) => wrap(ListMsg::Removed(String::new(), Err(e))),
            },
        ),
        ListMsg::Removed(id, Ok(())) => {
            tr.remove_item(&id);
            Task::none()
        }
        ListMsg::Caption(id) => {
            if list.caption.as_ref().is_some_and(|(c, _)| *c == id) {
                list.caption = None;
                return Task::none();
            }
            let current = tr.items.iter().find_map(|i| match i {
                TimelineItem::Screenshot(s) if s.id == id => s.caption.clone(),
                _ => None,
            });
            list.caption = Some((id, current.unwrap_or_default()));
            operation::focus(Id::new("caption"))
        }
        ListMsg::CaptionInput(v) => {
            if let Some((_, draft)) = &mut list.caption {
                *draft = v;
            }
            Task::none()
        }
        ListMsg::SaveCaption => {
            let Some((id, draft)) = list.caption.clone() else { return Task::none() };
            run(
                core,
                move |c| ops::set_caption(c, &sid, &id, &draft).map(|_| (id, draft)),
                move |r| match r {
                    Ok((id, draft)) => wrap(ListMsg::CaptionSaved(id, draft, Ok(()))),
                    Err(e) => wrap(ListMsg::CaptionSaved(String::new(), String::new(), Err(e))),
                },
            )
        }
        ListMsg::CaptionSaved(id, caption, Ok(())) => {
            tr.set_caption(&id, caption.trim());
            list.caption = None;
            Task::none()
        }
        ListMsg::OpenImage(path) => {
            if ops::allowed_path(core, &path) {
                let _ = open::that_detached(path);
            }
            Task::none()
        }
        ListMsg::RenameStart => {
            list.title = Some(tr.title.clone().unwrap_or_default());
            operation::focus(list.title_id.clone())
        }
        ListMsg::TitleInput(v) => {
            list.title = Some(v);
            Task::none()
        }
        ListMsg::TitleSave => {
            let Some(draft) = list.title.take() else { return Task::none() };
            let next = draft.trim().to_string();
            if next.is_empty() || Some(&next) == tr.title.as_ref() {
                return Task::none();
            }
            run(
                core,
                move |c| ops::rename_session(c, &sid, &next).map(|_| next),
                move |r| match r {
                    Ok(title) => wrap(ListMsg::TitleSaved(title, Ok(()))),
                    Err(e) => wrap(ListMsg::TitleSaved(String::new(), Err(e))),
                },
            )
        }
        ListMsg::TitleSaved(title, Ok(())) => {
            tr.title = Some(title);
            Task::none()
        }
        ListMsg::Details => {
            list.details = !list.details;
            Task::none()
        }
        ListMsg::Escape => {
            if list.editing.is_some() {
                list.editing = None;
            } else if list.caption.is_some() {
                list.caption = None;
            } else {
                list.title = None;
            }
            Task::none()
        }
    }
}

/// A row stays marked this long after the session's line pointed at it.
pub const MARK_FOR: Duration = Duration::from_millis(1600);

/// A saved screenshot, read and shrunk to what the list shows (2× for sharp screens).
fn decode_thumb(path: &std::path::Path) -> Option<image::Handle> {
    let img = ::image::open(path).ok()?.thumbnail(1520, 440).into_rgba8();
    let (w, h) = img.dimensions();
    Some(image::Handle::from_rgba(w, h, img.into_raw()))
}

/// Speech heard but not written yet, from the core's lag (FR-25), while recording.
pub fn writing(app: &App) -> bool {
    matches!(app.rec.state, UiState::Recording | UiState::Paused)
        && !app.rec.deferred
        && app.rec.lag.as_ref().is_some_and(|l| l.lag_ms >= WRITING_MS)
}

/// 「文字起こし中…」 with a spinner, so a slow PC shows that the words are on their way.
pub fn writing_note<'a>(now: Instant) -> Element<'a, Message> {
    row![spinner(12.0, now), line(t("transcribing"), tokens::CAPTION, p().ink_muted)]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
}

fn centered_note<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    container(content).center(Length::Fill).padding([0, 24]).into()
}

/// What a list shows and how.
pub struct Ctx<'a> {
    pub app: &'a App,
    pub tr: &'a Transcript,
    pub list: &'a ListState,
    pub live: bool,
    pub editable: bool,
    /// The finished session's header, as the first row.
    pub header: Option<Rc<dyn Fn() -> Element<'static, Message> + 'a>>,
    /// A key for the header's look, so it is built again when it changes.
    pub header_key: u64,
    /// Lines can be played from.
    pub playable: bool,
    pub wrap: Wrap,
    /// How far the deck reaches over the bottom of the list.
    pub under: f32,
}

/// The transcript as a list of the rows in view.
pub fn view<'a>(c: Ctx<'a>) -> Element<'a, Message> {
    let (rows, tops) = c.list.layout(c.tr, c.header.is_some());
    let has_lines = rows.has_lines();
    let note = c.live && writing(c.app) && !rows.has_partial;
    if c.live && !has_lines {
        let body: Element<'a, Message> = if writing(c.app) {
            writing_note(c.app.now)
        } else {
            line(t("waitingForSpeech"), tokens::CAPTION, p().ink_muted).into()
        };
        return container(centered_note(body)).padding(Padding::ZERO.bottom(c.under)).into();
    }
    if !has_lines && c.header.is_none() {
        return centered_note(line(t("emptyTranscript"), tokens::CAPTION, p().ink_muted).into());
    }
    let n = rows.rows.len();
    let total = tops[n];
    let view_h = if c.list.viewport_h > 0.0 { c.list.viewport_h } else { crate::app::EXPANDED.height };
    let anchored = c.live && c.list.anchored.get() && c.list.following.get();
    let (first, last) = if c.live && c.list.following.get() {
        let mut first = n;
        while first > 0 && total - tops[first - 1] < view_h + OVERSCAN {
            first -= 1;
        }
        (first.saturating_sub(1), n)
    } else {
        let from = c.list.offset - OVERSCAN;
        let to = c.list.offset + view_h + OVERSCAN;
        let first = tops[1..].partition_point(|b| *b <= from).min(n.saturating_sub(1));
        let last = tops[..n].partition_point(|a| *a < to).max(first + 1).min(n);
        (first, last)
    };

    let list = c.list;
    let tr = c.tr;
    let app = c.app;
    let wrap = c.wrap;
    let editable = c.editable;
    let playable = c.playable;
    let live = c.live;
    let two = rows.two;
    let width = list.list_width();
    let header = c.header.clone();
    let build = {
        let rows = rows.clone();
        move || -> Vec<Element<'static, Message>> {
            (first..last)
                .map(|i| {
                    let r = &rows.rows[i];
                    let body = match r.kind {
                        RowKind::Header => {
                            let mut col =
                                Column::new().push(header.as_ref().map_or_else(|| Space::new().into(), |h| h()));
                            if !has_lines {
                                col = col.push(
                                    container(line(t("emptyTranscript"), tokens::CAPTION, p().ink_muted))
                                        .center_x(Length::Fill)
                                        .padding([40, 0]),
                                );
                            }
                            col.into()
                        }
                        _ => row_view(app, tr, list, r, two, editable && !live, playable, width, wrap),
                    };
                    let highlighted = list.marked.as_ref().is_some_and(|(k, _)| *k == r.key)
                        || list.playing.as_deref() == Some(r.key.as_str());
                    let body = container(body).width(Length::Fill).style(move |_| container::Style {
                        background: highlighted.then(|| p().surface_sunken.into()),
                        ..container::Style::default()
                    });
                    let key = r.key.clone();
                    let k2 = key.clone();
                    sensor(body)
                        .key(key.clone())
                        .anticipate(OVERSCAN)
                        .on_show(move |s| wrap(ListMsg::Measured(key.clone(), s.height)))
                        .on_resize(move |s| wrap(ListMsg::Measured(k2.clone(), s.height)))
                        .into()
                })
                .collect()
        }
    };

    let rows_el: Element<'a, Message> = match &list.editing {
        // The editor borrows its text, so the rows are built as they are while one is open.
        Some(edit) => {
            let mut col = Column::new();
            for (i, el) in build().into_iter().enumerate() {
                let r = &rows.rows[first + i];
                if r.key == edit.id
                    && let RowKind::Item(k) = r.kind
                    && let TimelineItem::Segment(s) = &tr.items[k]
                {
                    col = col.push(edit_row(app, tr, s, r.head, two, edit, list.editor_id.clone(), wrap));
                } else {
                    col = col.push(el);
                }
            }
            col.into()
        }
        None => {
            let key = look_key(&c, &rows, first, last);
            container(lazy(key, move |_| Column::with_children(build()).width(Length::Fill))).width(Length::Fill).into()
        }
    };

    let bottom = 20.0 + c.under + if note { WRITING_H } else { 0.0 };
    // Anchored at the bottom, the list holds only its last rows.
    let (above, below) = if anchored { (0.0, 0.0) } else { (tops[first], total - tops[last]) };
    list.anchored_top.set(tops[first]);
    let mut content = column![Space::new().height(above), rows_el, Space::new().height(below)];
    if note {
        content = content.push(container(writing_note(app.now)).padding(Padding::ZERO.left(PAD_X).top(8)));
    }
    content = content.push(Space::new().height(bottom));
    let scroll = iced::widget::scrollable(container(content).max_width(MAX_W).center_x(Length::Fill))
        .id(list.id.clone())
        .on_scroll(move |vp| wrap(ListMsg::Scrolled(vp)))
        .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::new().width(4).scroller_width(4).margin(3)))
        .style(|_, _| scroll_style())
        .anchor_y(if anchored { scrollable::Anchor::End } else { scrollable::Anchor::Start })
        .width(Length::Fill)
        .height(Length::Fill);
    if !(c.live && !c.list.following.get()) {
        return scroll.into();
    }
    let jump = button(
        row![
            icon(Icon::ArrowDown, 13.0, p().ink),
            line(t("jumpLatest"), tokens::CAPTION, p().ink).font(theme::SANS_MEDIUM)
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([6, 14])
    .style(|_, status| {
        let mut s = floating();
        if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            s.background = Some(p().surface_sunken.into());
        }
        s.border.radius = tokens::RADIUS_MD.into();
        button::Style { background: s.background, text_color: p().ink, border: s.border, shadow: s.shadow, snap: true }
    })
    .on_press(wrap(ListMsg::JumpLatest));
    stack![
        scroll,
        container(jump).align_bottom(Length::Fill).center_x(Length::Fill).padding(Padding::ZERO.bottom(c.under + 16.0))
    ]
    .into()
}

/// A hash of everything the rows in view look like, so they are built again only when it changes.
fn look_key(c: &Ctx<'_>, rows: &Rows, first: usize, last: usize) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    rows::signature(c.tr, c.header.is_some()).hash(&mut h);
    (first, last, c.live, c.editable, c.playable, c.header_key).hash(&mut h);
    theme::is_dark().hash(&mut h);
    crate::i18n::is_ja().hash(&mut h);
    (c.list.list_width() as u32, c.list.thumbs.len()).hash(&mut h);
    c.list.marked.as_ref().map(|m| &m.0).hash(&mut h);
    c.list.playing.hash(&mut h);
    c.list.caption.hash(&mut h);
    c.tr.thumbs.len().hash(&mut h);
    // Lines that arrived a moment ago darken into place.
    for r in &rows.rows[first..last] {
        if let Some(age) = fresh_age(c.app, c.tr, &r.key) {
            (r.key.as_str(), (age * 5.0) as u8).hash(&mut h);
        }
    }
    h.finish()
}

/// How far a fresh line has darkened into place, 0..1; `None` once it has settled.
fn fresh_age(app: &App, tr: &Transcript, key: &str) -> Option<f32> {
    let at = tr.fresh.get(key)?;
    let t = app.now.saturating_duration_since(*at).as_secs_f32() / 0.2;
    (t < 1.0).then_some(t.max(0.0))
}

fn scroll_style() -> scrollable::Style {
    let p = p();
    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: p.border_strong.into(),
            border: Border { radius: 2.0.into(), ..Border::default() },
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: p.surface_raised.into(),
            border: Border::default(),
            shadow: iced::Shadow::default(),
            icon: p.ink,
        },
    }
}

/// A speaker's name in the colour of their side of the line: 相手 in the accent, 自分 ink.
pub fn speaker_color(source: SourceId) -> Color {
    if source == SourceId::Mic { p().ink } else { p().accent }
}

pub fn speaker_name(source: SourceId) -> String {
    t(if source == SourceId::Mic { "me" } else { "others" })
}

/// A turn's first line: the speaker's name (with both sides recorded), then the time.
fn turn_head(source: SourceId, time: String, two: bool) -> Element<'static, Message> {
    let mut r = row![].spacing(8).align_y(Alignment::Center);
    if two {
        r = r.push(line(speaker_name(source), tokens::CAPTION, speaker_color(source)).font(theme::SANS_MEDIUM));
    }
    r.push(line(time, tokens::CAPTION, p().ink_muted).font(theme::MONO)).into()
}

/// A small action in a row's hover box.
fn act(which: Icon, label: String, msg: Message, pressed: bool) -> Element<'static, Message> {
    let b = button(
        container(icon(which, 13.0, Color::BLACK).style(move |_, status| iced::widget::svg::Style {
            color: Some(if pressed || status == iced::widget::svg::Status::Hovered { p().ink } else { p().ink_muted }),
        }))
        .center(24),
    )
    .width(24)
    .height(24)
    .padding(0)
    .style(|_, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then(|| Background::Color(p().surface_sunken)),
        border: Border { radius: tokens::RADIUS_SM.into(), ..Border::default() },
        ..button::Style::default()
    })
    .on_press(msg);
    with_tip(b, label)
}

/// The actions shown over a row while the pointer is on it.
fn actions(items: Vec<Element<'static, Message>>) -> Element<'static, Message> {
    let bar = container(iced::widget::Row::with_children(items).spacing(2)).padding(2).style(|_| container::Style {
        background: Some(p().surface.into()),
        border: Border { color: p().border, width: 1.0, radius: tokens::RADIUS_MD.into() },
        ..container::Style::default()
    });
    container(bar).width(Length::Fill).align_right(Length::Fill).padding(Padding::ZERO.top(4).right(8)).into()
}

#[allow(clippy::too_many_arguments)]
fn row_view(
    app: &App,
    tr: &Transcript,
    list: &ListState,
    r: &Row,
    two: bool,
    editable: bool,
    playable: bool,
    width: f32,
    wrap: Wrap,
) -> Element<'static, Message> {
    match r.kind {
        RowKind::Header => Space::new().into(),
        RowKind::Partial(source) => {
            let p_ = tr.partials.get(&source);
            let text = p_.map_or_else(String::new, |p| p.text.clone());
            let time = p_.and_then(|p| p.t_start_ms).map(|t| tr.clock(t)).unwrap_or_default();
            let mut col = Column::new();
            if r.head {
                col = col.push(turn_head(source, time, two)).push(Space::new().height(2));
            }
            col = col.push(theme::t(text, tokens::BODY, p().ink_subtle));
            container(col)
                .width(Length::Fill)
                .padding(Padding::new(PAD_X).top(if r.head { 14 } else { 2 }).bottom(2))
                .into()
        }
        RowKind::Item(i) => match &tr.items[i] {
            TimelineItem::Segment(s) => segment_row(app, tr, s, r.head, two, editable, playable, wrap),
            TimelineItem::Screenshot(s) => screenshot_row(tr, list, s, editable, width, wrap),
            TimelineItem::Marker(m) => marker_row(tr, m),
        },
    }
}

/// A line's text: bolder when important, with 編集済み after it when edited.
fn line_text(s: &Segment, color: Color) -> Element<'static, Message> {
    let font = if s.important { theme::SANS_MEDIUM } else { theme::SANS };
    let body = iced::widget::text::Span::new(s.text.clone()).color(color).font(font);
    let mut spans: Vec<iced::widget::text::Span<'static, (), iced::Font>> = vec![body];
    if s.edited {
        spans.push(iced::widget::text::Span::new(" "));
        spans.push(
            iced::widget::text::Span::new(t("edited"))
                .size(tokens::CAPTION.size - 2.0)
                .color(p().ink_muted)
                .font(theme::SANS)
                .border(Border { color: p().border, width: 1.0, radius: 3.0.into() })
                .padding([0, 4]),
        );
    }
    iced::widget::rich_text(spans)
        .size(tokens::BODY.size)
        .line_height(iced::widget::text::LineHeight::Absolute(tokens::BODY.line_height.into()))
        .width(Length::Fill)
        .into()
}

#[allow(clippy::too_many_arguments)]
fn segment_row(
    app: &App,
    tr: &Transcript,
    s: &Segment,
    head: bool,
    two: bool,
    editable: bool,
    playable: bool,
    wrap: Wrap,
) -> Element<'static, Message> {
    let ink = match fresh_age(app, tr, &s.id) {
        Some(age) => mix(p().ink, p().ink_subtle, age),
        None => p().ink,
    };
    let mut col = Column::new();
    if head {
        col = col.push(turn_head(s.source, tr.clock(s.t_start_ms), two)).push(Space::new().height(2));
    }
    col = col.push(line_text(s, ink));
    let body = container(col).width(Length::Fill).padding(Padding::new(PAD_X).top(if head { 14 } else { 2 }).bottom(2));
    // An important line: an ink stroke in the margin, 11 px left of the text, and bolder text.
    let body: Element<'static, Message> = if s.important {
        let stroke = container(Space::new().width(3).height(Length::Fill)).style(|_| container::Style {
            background: Some(p().ink.into()),
            border: Border { radius: 1.5.into(), ..Border::default() },
            ..container::Style::default()
        });
        let top = if head { 14.0 + tokens::CAPTION.line_height + 2.0 } else { 2.0 };
        stack![
            body,
            container(stroke).height(Length::Fill).padding(Padding::ZERO.left(PAD_X - 11.0).top(top + 6.0).bottom(8))
        ]
        .into()
    } else {
        body.into()
    };
    if tr.session_id.is_none() {
        return body;
    }
    let mut items = Vec::new();
    if playable {
        items.push(act(Icon::Play, t("playFromHere"), wrap(ListMsg::PlayFrom(s.t_start_ms)), false));
    }
    let mark = if s.important { t("unmarkImportant") } else { t("markImportant") };
    items.push(act(Icon::Star, mark, wrap(ListMsg::Star(s.id.clone(), !s.important)), s.important));
    if editable {
        items.push(act(Icon::Pencil, t("editLine"), wrap(ListMsg::Edit(s.id.clone())), false));
        items.push(act(Icon::Trash, t("deleteLine"), wrap(ListMsg::DeleteLine(s.id.clone())), false));
    }
    hover(body, actions(items))
}

/// A line being edited, in the kamon's frame: the one container being worked in.
#[allow(clippy::too_many_arguments)]
fn edit_row<'a>(
    _app: &App,
    tr: &Transcript,
    s: &Segment,
    head: bool,
    two: bool,
    edit: &'a Edit,
    editor_id: Id,
    wrap: Wrap,
) -> Element<'a, Message> {
    let mut col = Column::new();
    if head {
        col = col.push(turn_head(s.source, tr.clock(s.t_start_ms), two)).push(Space::new().height(2));
    }
    let editor = text_editor(&edit.content)
        .id(editor_id)
        .on_action(move |a| wrap(ListMsg::EditAction(a)))
        .font(theme::SANS)
        .size(tokens::BODY.size)
        .line_height(iced::widget::text::LineHeight::Absolute(tokens::BODY.line_height.into()))
        .min_height(64)
        .padding(tokens::SPACE_3 - 4.0)
        .key_binding(move |k| {
            let enter = matches!(k.key, iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter));
            if enter && k.modifiers.command() {
                Some(text_editor::Binding::Custom(wrap(ListMsg::SaveEdit)))
            } else if matches!(k.key, iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)) {
                Some(text_editor::Binding::Custom(wrap(ListMsg::CancelEdit)))
            } else {
                text_editor::Binding::from_key_press(k)
            }
        })
        .style(|_, _| text_editor::Style {
            background: Color::TRANSPARENT.into(),
            border: Border::default(),
            placeholder: p().ink_subtle,
            value: p().ink,
            selection: p().accent_tint,
        });
    let buttons = row![
        Space::new().width(Length::Fill),
        btn_with(
            row![icon(Icon::X, 13.0, p().ink), crate::theme::plain(t("cancel"), tokens::LABEL)].spacing(6),
            Variant::Ghost,
            Size::Sm,
            Some(wrap(ListMsg::CancelEdit))
        ),
        btn_with(
            row![icon(Icon::Check, 13.0, p().on_accent), crate::theme::plain(t("save"), tokens::LABEL)].spacing(6),
            Variant::Primary,
            Size::Sm,
            Some(wrap(ListMsg::SaveEdit))
        ),
    ]
    .spacing(6);
    col = col.push(column![frame(editor, p().surface_raised, tokens::RADIUS_LG), buttons].spacing(6).padding([4, 0]));
    container(col).width(Length::Fill).padding(Padding::new(PAD_X).top(if head { 14 } else { 2 }).bottom(2)).into()
}

fn screenshot_row(
    tr: &Transcript,
    list: &ListState,
    s: &Screenshot,
    editable: bool,
    width: f32,
    wrap: Wrap,
) -> Element<'static, Message> {
    let time = tr.clock(s.t_ms);
    let head = row![
        icon(Icon::Camera, 12.0, p().ink_muted),
        line(time.clone(), tokens::CAPTION, p().ink_muted).font(theme::MONO)
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let path = tr.folder.as_ref().map(|f| crate::app::session_file(f, &s.file));
    let handle = tr.thumbs.get(&s.id).or_else(|| list.thumbs.get(&s.id)).cloned();
    let (w, h) = image_size(s, (width - 2.0 * PAD_X).max(40.0));
    let picture: Element<'static, Message> = match handle {
        Some(handle) => {
            let img = image(handle).width(w).height(h);
            let b = button(img).padding(1).style(|_, status| button::Style {
                background: Some(p().surface_sunken.into()),
                border: Border {
                    color: if matches!(status, button::Status::Hovered) { p().border_strong } else { p().border },
                    width: 1.0,
                    radius: tokens::RADIUS_SM.into(),
                },
                ..button::Style::default()
            });
            let b = match path {
                Some(path) => b.on_press(wrap(ListMsg::OpenImage(path))),
                None => b,
            };
            with_tip(b, t("openImage"))
        }
        // Not read yet, or no file: a dashed-looking empty frame of a fixed size.
        None => container(icon(Icon::Camera, 18.0, p().ink_muted))
            .center_x(192)
            .center_y(80)
            .style(|_| container::Style {
                border: Border { color: p().border, width: 1.0, radius: tokens::RADIUS_SM.into() },
                ..container::Style::default()
            })
            .into(),
    };
    let mut col = column![head, Space::new().height(4), picture];
    let open = list.caption.as_ref().filter(|(id, _)| *id == s.id);
    if let (Some(caption), None) = (&s.caption, open) {
        col = col
            .push(container(theme::t(caption.clone(), tokens::CAPTION, p().ink_muted)).padding(Padding::ZERO.top(6)));
    }
    if let Some((_, draft)) = open {
        let input = text_input(&t("caption"), draft)
            .id(Id::new("caption"))
            .on_input(move |v| wrap(ListMsg::CaptionInput(v)))
            .on_submit(wrap(ListMsg::SaveCaption))
            .font(theme::SANS)
            .size(tokens::CAPTION.size)
            .padding([6, 10])
            .style(|th, st| {
                let mut s = crate::widgets::input_style(th, st);
                s.border.color = p().accent;
                s
            });
        let save = btn_with(
            row![icon(Icon::Check, 13.0, p().on_accent)],
            Variant::Primary,
            Size::Sm,
            Some(wrap(ListMsg::SaveCaption)),
        );
        col =
            col.push(container(row![input, save].spacing(6).align_y(Alignment::Center)).padding(Padding::ZERO.top(6)));
    }
    let body: Element<'static, Message> =
        container(col).width(Length::Fill).padding(Padding::new(PAD_X).top(14).bottom(4)).into();
    if !editable {
        return body;
    }
    hover(
        body,
        actions(vec![
            act(Icon::ImagePlus, t("addCaption"), wrap(ListMsg::Caption(s.id.clone())), false),
            act(Icon::Trash, t("deleteScreenshot"), wrap(ListMsg::DeleteShot(s.id.clone())), false),
        ]),
    )
}

/// A marker as a rule with what happened. A pause or a gap had no sound, so its line stays flat; a
/// cut starts a part, so its line is darker and it stands further from the part before.
fn marker_row(tr: &Transcript, m: &Marker) -> Element<'static, Message> {
    let detail = m.detail.clone().unwrap_or_else(|| "?".into());
    let (glyph, label) = match m.kind {
        MarkerKind::SourceReattached => (Icon::Link, t("markerReattached")),
        MarkerKind::Paused => (Icon::Pause, t("markerPaused")),
        MarkerKind::Resumed => (Icon::Play, t("markerResumed")),
        MarkerKind::Cut => (Icon::Scissors, t("markerCut")),
        MarkerKind::SourceChanged => (Icon::ArrowRightLeft, tf("markerSourceChanged", &[("source", &detail)])),
        MarkerKind::Continued => (Icon::StepForward, t("markerContinued")),
        MarkerKind::Unprocessed => (Icon::TriangleAlert, tf("markerUnprocessed", &[("n", &detail)])),
    };
    // A continuation on another day says which (FR-08).
    let day = match (&m.detail, &tr.started_at) {
        (Some(d), Some(start)) if m.kind == MarkerKind::Continued && d.as_str() != day_of(start) => {
            format!("{} ", day_text(d, chrono::Datelike::year(&chrono::Local::now())))
        }
        _ => String::new(),
    };
    let cut = m.kind == MarkerKind::Cut;
    let tone = if cut { p().ink } else { p().ink_muted };
    let mut label_row = row![icon(glyph, 11.0, tone), line(label, tokens::CAPTION, tone).font(theme::SANS_MEDIUM)]
        .spacing(6)
        .align_y(Alignment::Center);
    if m.kind != MarkerKind::Unprocessed {
        label_row = label_row.push(line(format!("{day}{}", tr.clock(m.t_ms)), tokens::CAPTION, tone).font(theme::MONO));
    }
    let rule = move || -> Element<'static, Message> {
        container(Space::new().height(1))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(if cut { p().border_strong } else { p().border }.into()),
                ..container::Style::default()
            })
            .into()
    };
    container(row![rule(), label_row, rule()].spacing(12).align_y(Alignment::Center))
        .width(Length::Fill)
        .padding(if cut { Padding::new(PAD_X).top(24).bottom(8) } else { Padding::new(PAD_X).top(12).bottom(12) })
        .into()
}

/// The compact window: the last two lines, the newest words in view.
pub fn tail<'a>(app: &'a App, tr: &'a Transcript, list: &'a ListState) -> Element<'a, Message> {
    let rows = list.rows(tr, false);
    let shown = rows::tail(tr, &rows);
    if shown.is_empty() {
        return if writing(app) {
            container(writing_note(app.now)).padding([0, 4]).into()
        } else {
            container(line(t("waitingForSpeech"), tokens::CAPTION, p().ink_muted)).padding([0, 4]).into()
        };
    }
    // The compact window's width is fixed; what fits beside the name is about this many ems.
    let avail = crate::app::COMPACT.width - 2.0 * 12.0 - 8.0;
    let label = |source: SourceId| -> Option<Element<'a, Message>> {
        rows.two
            .then(|| line(speaker_name(source), tokens::CAPTION, speaker_color(source)).font(theme::SANS_MEDIUM).into())
    };
    let lines = shown.iter().filter_map(|r| -> Option<Element<'a, Message>> {
        match r.kind {
            RowKind::Partial(s) => {
                let text = tr.partials.get(&s).map_or("", |p| p.text.as_str());
                Some(tail_line(label(s), None, text, p().ink_subtle, false, avail))
            }
            RowKind::Item(i) => match &tr.items[i] {
                TimelineItem::Screenshot(_) => Some(
                    row![
                        icon(Icon::Camera, 12.0, p().ink_muted),
                        line(t("screenshot"), tokens::CAPTION, p().ink_muted)
                    ]
                    .spacing(6)
                    .height(24)
                    .align_y(Alignment::Center)
                    .into(),
                ),
                TimelineItem::Segment(s) => {
                    let star: Option<Element<'a, Message>> =
                        s.important.then(|| icon(Icon::Star, 11.0, p().ink).into());
                    Some(tail_line(label(s.source), star, &s.text, p().ink, s.important, avail))
                }
                TimelineItem::Marker(_) => None,
            },
            RowKind::Header => None,
        }
    });
    container(Column::with_children(lines)).padding([0, 4]).into()
}

/// One line of the tail, cut at its start so the newest words stay visible.
fn tail_line<'a>(
    label: Option<Element<'a, Message>>,
    star: Option<Element<'a, Message>>,
    text: &str,
    color: Color,
    important: bool,
    avail: f32,
) -> Element<'a, Message> {
    let mut r = row![].spacing(8).align_y(Alignment::Center).height(24);
    let mut room = avail;
    if let Some(l) = label {
        r = r.push(l);
        room -= tokens::CAPTION.size * 2.0 + 8.0;
    }
    if let Some(s) = star {
        r = r.push(s);
        room -= 19.0;
    }
    let size = tokens::BODY_SMALL.size;
    let fits = |s: &str| s.chars().map(|c| if c.is_ascii() { size * 0.55 } else { size }).sum::<f32>() <= room;
    let shown = if fits(text) {
        text.to_string()
    } else {
        let chars: Vec<char> = text.chars().collect();
        let mut from = 0;
        while from < chars.len() && !fits(&format!("…{}", chars[from..].iter().collect::<String>())) {
            from += 1;
        }
        format!("…{}", chars[from..].iter().collect::<String>())
    };
    let font = if important { theme::SANS_MEDIUM } else { theme::SANS };
    r = r.push(
        container(
            theme::t(shown, tokens::BODY_SMALL, color)
                .font(font)
                .wrapping(Wrapping::None)
                .align_x(iced::alignment::Horizontal::Right),
        )
        .width(Length::Fill)
        .clip(true),
    );
    r.into()
}

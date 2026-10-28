//! The transcript's rows: its items and the words still coming, strictly by time (section 9, the
//! same rule as `order_timeline`, with provisional text placed at its utterance's start), grouped
//! into turns that name the speaker and the time.

use std::hash::{Hash, Hasher};

use chidori_core::session::model::{MarkerKind, SourceId, TimelineItem};

use crate::model::transcript::Transcript;

/// A new turn (speaker and time shown) starts at a speaker change, after a picture or marker,
/// and every minute.
pub const TURN_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// The finished session's header, scrolling with its lines.
    Header,
    /// An item of the transcript, by its index in `Transcript::items`.
    Item(usize),
    /// Provisional text of an utterance still being transcribed.
    Partial(SourceId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub key: String,
    pub kind: RowKind,
    /// The first line of a turn, which names the speaker and the time.
    pub head: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Rows {
    pub rows: Vec<Row>,
    /// Both sides were recorded: turns carry speaker names.
    pub two: bool,
    /// Grey words are on screen.
    pub has_partial: bool,
}

impl Rows {
    pub fn has_lines(&self) -> bool {
        self.rows.iter().any(|r| r.kind != RowKind::Header)
    }
}

/// 相手 sorts before 自分 on ties.
fn tie_rank(s: SourceId) -> u8 {
    u8::from(s == SourceId::Mic)
}

/// On a tie a resume, reattach, cut, source or continuation marker comes before the lines it
/// precedes; pause and unprocessed markers come after the lines they follow.
fn marker_rank(k: MarkerKind) -> u8 {
    if matches!(k, MarkerKind::Paused | MarkerKind::Unprocessed) { 3 } else { 0 }
}

/// What a row sorts by: time, kind, side, then its ID (capture order).
type Key<'a> = (u64, u8, u8, &'a str);

fn item_key(i: &TimelineItem) -> Key<'_> {
    match i {
        TimelineItem::Segment(s) => (s.t_start_ms, 1, tie_rank(s.source), &s.id),
        TimelineItem::Screenshot(s) => (s.t_ms, 2, 0, &s.id),
        TimelineItem::Marker(m) => (m.t_ms, marker_rank(m.kind), 0, &m.id),
    }
}

/// The partials' sources, in a fixed order.
const PARTIAL_SOURCES: [SourceId; 4] = [SourceId::App, SourceId::System, SourceId::File, SourceId::Mic];

pub fn partial_key(s: SourceId) -> String {
    format!("~partial-{}", s.as_str())
}

/// The turn being read: whose it is and when it started.
#[derive(Default)]
struct Turn {
    source: Option<SourceId>,
    start: Option<u64>,
}

impl Turn {
    /// Whether a line starts a new turn; it then becomes the turn.
    fn next(&mut self, source: SourceId, t: Option<u64>) -> bool {
        let t = t.or(self.start);
        let head = self.source != Some(source)
            || match (t, self.start) {
                (Some(t), Some(s)) => t.saturating_sub(s) >= TURN_MS,
                _ => true,
            };
        if head {
            self.source = Some(source);
            self.start = t;
        }
        head
    }
}

/// Builds the rows of a transcript; `header` puts the session's header first.
pub fn build(tr: &Transcript, header: bool) -> Rows {
    let pending: Vec<(SourceId, String, Option<u64>)> = PARTIAL_SOURCES
        .iter()
        .filter_map(|s| tr.partials.get(s).filter(|p| !p.text.is_empty()).map(|p| (*s, partial_key(*s), p.t_start_ms)))
        .collect();
    enum Entry<'a> {
        Item(usize, &'a TimelineItem),
        Partial(SourceId, &'a str, Option<u64>),
    }
    let mut entries: Vec<(Key<'_>, Entry<'_>)> =
        tr.items.iter().enumerate().map(|(i, it)| (item_key(it), Entry::Item(i, it))).collect();
    for (s, key, start) in &pending {
        // Without a start time, provisional text stays at the end.
        entries.push(((start.unwrap_or(u64::MAX), 1, tie_rank(*s), key.as_str()), Entry::Partial(*s, key, *start)));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut rows = Vec::with_capacity(entries.len() + 1);
    if header {
        rows.push(Row { key: "header".into(), kind: RowKind::Header, head: false });
    }
    let mut turn = Turn::default();
    for (_, e) in entries {
        match e {
            Entry::Item(i, TimelineItem::Segment(s)) => {
                let head = turn.next(s.source, Some(s.t_start_ms));
                rows.push(Row { key: s.id.clone(), kind: RowKind::Item(i), head });
            }
            Entry::Partial(s, key, start) => {
                let head = turn.next(s, start);
                rows.push(Row { key: key.to_string(), kind: RowKind::Partial(s), head });
            }
            Entry::Item(i, it) => {
                turn.source = None;
                rows.push(Row { key: it.id().to_string(), kind: RowKind::Item(i), head: true });
            }
        }
    }
    Rows { rows, two: tr.two_sides(), has_partial: !pending.is_empty() }
}

/// What the rows of a transcript depend on, so they are built again only when it changes.
pub fn signature(tr: &Transcript, header: bool) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    tr.session_id.hash(&mut h);
    tr.version.hash(&mut h);
    header.hash(&mut h);
    tr.sources.len().hash(&mut h);
    for s in PARTIAL_SOURCES {
        if let Some(p) = tr.partials.get(&s) {
            (s.as_str(), &p.utterance_id, &p.text, p.t_start_ms).hash(&mut h);
        }
    }
    h.finish()
}

/// The time of a row, for jumping to a moment of the session.
pub fn row_time(tr: &Transcript, r: &Row) -> Option<u64> {
    match r.kind {
        RowKind::Header => None,
        RowKind::Partial(s) => Some(tr.partials.get(&s).and_then(|p| p.t_start_ms).unwrap_or(u64::MAX)),
        RowKind::Item(i) => Some(match &tr.items[i] {
            TimelineItem::Segment(s) => s.t_start_ms,
            TimelineItem::Screenshot(s) => s.t_ms,
            TimelineItem::Marker(m) => m.t_ms,
        }),
    }
}

/// The row to bring into view for a moment pointed at on the session's line: the first at or
/// after it, else the last.
pub fn seek_index(tr: &Transcript, rows: &Rows, ms: u64) -> Option<usize> {
    if rows.rows.is_empty() {
        return None;
    }
    Some(rows.rows.iter().position(|r| row_time(tr, r).is_some_and(|t| t >= ms)).unwrap_or(rows.rows.len() - 1))
}

/// The line being played (FR-09): the last one that started before where the sound is.
pub fn playing_key(tr: &Transcript, rows: &Rows, ms: u64) -> Option<String> {
    let mut key = None;
    for r in &rows.rows {
        if let RowKind::Item(i) = r.kind
            && let TimelineItem::Segment(s) = &tr.items[i]
        {
            if s.t_start_ms > ms {
                break;
            }
            key = Some(r.key.clone());
        }
    }
    key
}

/// The compact window's last two lines: the words still coming included, markers left out.
pub fn tail<'a>(tr: &Transcript, rows: &'a Rows) -> Vec<&'a Row> {
    let shown: Vec<&Row> = rows
        .rows
        .iter()
        .filter(|r| match r.kind {
            RowKind::Partial(_) => true,
            RowKind::Item(i) => !matches!(tr.items[i], TimelineItem::Marker(_)),
            RowKind::Header => false,
        })
        .collect();
    shown[shown.len().saturating_sub(2)..].to_vec()
}

#[cfg(test)]
mod tests {
    use chidori_core::events::PartialPayload;
    use chidori_core::session::model::{Marker, Screenshot, Segment, SourceInfo};

    use super::*;

    fn seg(id: &str, source: SourceId, s: u64, e: u64) -> TimelineItem {
        TimelineItem::Segment(Segment {
            id: id.into(),
            source,
            t_start_ms: s,
            t_end_ms: e,
            text: id.into(),
            text_original: None,
            edited: false,
            utterance_id: Some(format!("u-{id}")),
            important: false,
        })
    }

    fn shot(id: &str, t: u64) -> TimelineItem {
        TimelineItem::Screenshot(Screenshot {
            id: id.into(),
            t_ms: t,
            file: format!("{id}.png"),
            width: 1,
            height: 1,
            caption: None,
        })
    }

    fn marker(id: &str, t: u64, kind: MarkerKind) -> TimelineItem {
        TimelineItem::Marker(Marker { id: id.into(), t_ms: t, kind, detail: None })
    }

    fn transcript(items: Vec<TimelineItem>) -> Transcript {
        Transcript {
            session_id: Some("s1".into()),
            sources: vec![
                SourceInfo { id: SourceId::App, label: "Zoom".into(), exe: None, device: None, name: None },
                SourceInfo { id: SourceId::Mic, label: "自分".into(), exe: None, device: None, name: None },
            ],
            items,
            ..Default::default()
        }
    }

    fn partial(tr: &mut Transcript, source: SourceId, start: Option<u64>) {
        tr.set_partial(PartialPayload {
            source,
            utterance_id: format!("u-p-{}", source.as_str()),
            text: "…".into(),
            t_start_ms: start,
        });
    }

    fn ids(tr: &Transcript) -> Vec<String> {
        build(tr, false).rows.into_iter().map(|r| r.key).collect()
    }

    // The same cases as `session::model::tests` in the core.
    #[test]
    fn a_screenshot_outside_sentences_keeps_its_time() {
        let tr = transcript(vec![
            seg("seg_000001", SourceId::App, 1000, 2000),
            shot("img_0001", 2500),
            seg("seg_000002", SourceId::App, 3000, 4000),
        ]);
        assert_eq!(ids(&tr), ["seg_000001", "img_0001", "seg_000002"]);
    }

    #[test]
    fn a_screenshot_inside_a_sentence_sorts_at_its_own_time() {
        let tr = transcript(vec![
            shot("img_0001", 3000),
            seg("seg_000002", SourceId::Mic, 5000, 7000),
            seg("seg_000001", SourceId::App, 1000, 6000),
        ]);
        assert_eq!(ids(&tr), ["seg_000001", "img_0001", "seg_000002"]);
    }

    #[test]
    fn several_screenshots_keep_capture_order() {
        let tr = transcript(vec![
            shot("img_0002", 4000),
            seg("seg_000001", SourceId::App, 1000, 6000),
            shot("img_0001", 2000),
            shot("img_0003", 5000),
            shot("img_0004", 5000),
        ]);
        assert_eq!(ids(&tr), ["seg_000001", "img_0001", "img_0002", "img_0003", "img_0004"]);
    }

    #[test]
    fn ties_put_segments_first_others_before_me_then_id() {
        let tr = transcript(vec![
            shot("img_0001", 1000),
            seg("seg_000003", SourceId::Mic, 1000, 1000),
            seg("seg_000002", SourceId::App, 1000, 1000),
            seg("seg_000001", SourceId::App, 1000, 1000),
        ]);
        assert_eq!(ids(&tr), ["seg_000001", "seg_000002", "seg_000003", "img_0001"]);
    }

    #[test]
    fn markers_on_a_tie_go_around_the_lines_they_describe() {
        let tr = transcript(vec![
            seg("seg_000002", SourceId::App, 5000, 6000),
            marker("mk_0002", 5000, MarkerKind::Resumed),
            marker("mk_0001", 2000, MarkerKind::Paused),
            seg("seg_000001", SourceId::App, 2000, 2000),
            seg("seg_000003", SourceId::App, 8000, 9000),
            marker("mk_0003", 8000, MarkerKind::Cut),
        ]);
        assert_eq!(ids(&tr), ["seg_000001", "mk_0001", "mk_0002", "seg_000002", "mk_0003", "seg_000003"]);
    }

    #[test]
    fn provisional_text_sits_at_its_utterance_start_or_at_the_end() {
        let mut tr = transcript(vec![seg("seg_000001", SourceId::App, 1000, 2000), shot("img_0001", 4000)]);
        partial(&mut tr, SourceId::App, Some(3000));
        assert_eq!(ids(&tr), ["seg_000001", "~partial-app", "img_0001"]);
        let mut tr = transcript(vec![seg("seg_000001", SourceId::App, 1000, 2000), shot("img_0001", 4000)]);
        partial(&mut tr, SourceId::Mic, None);
        assert_eq!(ids(&tr), ["seg_000001", "img_0001", "~partial-mic"]);
        assert!(build(&tr, false).has_partial);
    }

    #[test]
    fn the_final_line_takes_the_place_of_its_words() {
        let mut tr = transcript(Vec::new());
        tr.set_partial(PartialPayload {
            source: SourceId::App,
            utterance_id: "u-a".into(),
            text: "こんにち".into(),
            t_start_ms: Some(0),
        });
        assert_eq!(ids(&tr), ["~partial-app"]);
        let TimelineItem::Segment(s) = seg("a", SourceId::App, 0, 2000) else { unreachable!() };
        tr.add_segment(Segment { utterance_id: Some("u-a".into()), ..s });
        assert_eq!(ids(&tr), ["a"]);
    }

    #[test]
    fn turns_name_the_speaker_at_a_change_and_every_minute() {
        let tr = transcript(vec![
            seg("a", SourceId::App, 0, 2000),
            seg("b", SourceId::App, 3000, 5000),
            seg("c", SourceId::Mic, 6000, 8000),
            // A minute on, the same speaker starts a new turn.
            seg("d", SourceId::Mic, 70_000, 72_000),
        ]);
        let rows = build(&tr, false);
        let heads: Vec<&str> = rows.rows.iter().filter(|r| r.head).map(|r| r.key.as_str()).collect();
        assert_eq!(heads, ["a", "c", "d"]);
        assert!(rows.two);
        let one = Transcript { sources: tr.sources[..1].to_vec(), ..tr.clone() };
        assert!(!build(&one, false).two);
    }

    #[test]
    fn a_picture_or_marker_ends_a_turn() {
        let tr =
            transcript(vec![seg("a", SourceId::App, 0, 1000), shot("i", 1500), seg("b", SourceId::App, 2000, 3000)]);
        let heads: Vec<bool> = build(&tr, false).rows.iter().map(|r| r.head).collect();
        assert_eq!(heads, [true, true, true]);
    }

    #[test]
    fn the_moment_pointed_at_finds_the_first_line_at_or_after_it() {
        let tr = transcript(
            [0, 10_000, 20_000, 30_000]
                .iter()
                .enumerate()
                .map(|(i, t)| seg(&format!("s{i}"), SourceId::App, *t, t + 2000))
                .collect(),
        );
        let rows = build(&tr, true);
        // Row 0 is the header; 20 s is the first line at or after 15 s.
        assert_eq!(seek_index(&tr, &rows, 15_000), Some(3));
        assert_eq!(seek_index(&tr, &rows, 99_000), Some(4));
    }

    #[test]
    fn the_line_played_is_the_last_that_started_before_the_sound() {
        let tr = transcript(vec![seg("a", SourceId::App, 0, 2000), seg("b", SourceId::App, 4000, 6000)]);
        let rows = build(&tr, true);
        assert_eq!(playing_key(&tr, &rows, 3000).as_deref(), Some("a"));
        assert_eq!(playing_key(&tr, &rows, 4000).as_deref(), Some("b"));
    }

    #[test]
    fn the_tail_is_the_last_two_lines_with_words_coming_and_without_markers() {
        let mut tr = transcript(vec![
            seg("a", SourceId::App, 0, 1000),
            seg("b", SourceId::Mic, 2000, 3000),
            marker("m", 3000, MarkerKind::Cut),
        ]);
        partial(&mut tr, SourceId::App, Some(4000));
        let rows = build(&tr, false);
        let keys: Vec<&str> = tail(&tr, &rows).iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, ["b", "~partial-app"]);
    }

    #[test]
    fn the_signature_follows_partials_and_changes() {
        let mut tr = transcript(vec![seg("a", SourceId::App, 0, 1000)]);
        let before = signature(&tr, false);
        partial(&mut tr, SourceId::App, Some(2000));
        assert_ne!(signature(&tr, false), before);
        assert_ne!(signature(&tr, true), signature(&tr, false));
    }
}

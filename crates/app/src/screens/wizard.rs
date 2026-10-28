//! The setup wizard (section 5, FR-70): welcome → model → download and speed test → a 10-second
//! mic test → shortcuts (optional) → done. Its title and step bar are its app bar, so the
//! kamon's rule runs under them (DECISIONS → UI, "The kamon's double line").

use std::time::Instant;

use chidori_core::error::{AppError, ErrorCode};
use chidori_core::events::DownloadPayload;
use chidori_core::mic_test::MicTestResult;
use chidori_core::models::benchmark::BenchmarkResult;
use chidori_core::models::catalog::{DEFAULT_MODEL_ID, LIGHT_MODEL_ID};
use chidori_core::models::manager::ModelEntry;
use chidori_core::ops;
use chidori_core::session::model::SourceId;
use iced::widget::canvas::{self, Frame, LineCap, LineJoin, Path, Stroke};
use iced::widget::{Column, Row, Space, button, column, container, row};
use iced::{Alignment, Border, Element, Length, Padding, Point, Rectangle, Renderer, Task, Theme, mouse};

use super::form;
use super::models::{Bench, description_of, download_progress, name_of};
use super::settings::{self, Hotkey};
use crate::actions::report_error;
use crate::app::{App, Message, call};
use crate::i18n::t;
use crate::icons::{Icon, icon};
use crate::model::format::bytes;
use crate::model::line::{height_of, settle, smooth};
use crate::theme::{self, line, p, tokens};
use crate::widgets::{Size, Variant, btn, btn_icon, hairline, rule, spinner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Welcome,
    Model,
    Download,
    MicTest,
    Hotkeys,
    Done,
}

pub const STEPS: [Step; 6] = [Step::Welcome, Step::Model, Step::Download, Step::MicTest, Step::Hotkeys, Step::Done];

impl Step {
    fn title_key(self) -> &'static str {
        match self {
            Step::Welcome => "wizardWelcome",
            Step::Model => "wizardModel",
            Step::Download => "wizardDownload",
            Step::MicTest => "wizardMicTest",
            Step::Hotkeys => "wizardHotkeys",
            Step::Done => "wizardDone",
        }
    }
}

/// The mic test drawn as the recording line: while it listens, the pen draws your voice.
#[derive(Debug, Default)]
struct Wave {
    samples: Vec<f32>,
    height: f32,
    at: Option<Instant>,
}

/// Steps the wave keeps; older ones have scrolled out of its 200 px.
const WAVE_MAX: usize = 400;

#[derive(Debug, Default)]
pub struct State {
    step: usize,
    /// The model picked; the recommended one until then.
    choice: Option<String>,
    bench: Option<BenchmarkResult>,
    benching: bool,
    mic: Option<MicTestResult>,
    testing: bool,
    wave: Wave,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Next,
    Back,
    Choose(String),
    Retry,
    /// The model chosen is heavy for this PC: take the light one.
    UseLight,
    Models(Vec<ModelEntry>),
    Selected(String, Result<(), AppError>, Vec<ModelEntry>),
    Benchmarked(Result<BenchmarkResult, AppError>),
    MicTest,
    MicTested(Result<MicTestResult, AppError>),
    Finish,
}

fn msg(m: Msg) -> Message {
    Message::Wizard(m)
}

impl State {
    pub fn step(&self) -> Step {
        STEPS[self.step.min(STEPS.len() - 1)]
    }
}

/// The model the wizard sets up: the one picked, else the one recommended for this PC.
pub fn chosen_id(app: &App) -> String {
    app.wizard.choice.clone().unwrap_or_else(|| {
        let r = &app.info.recommended_model;
        if r.is_empty() { DEFAULT_MODEL_ID.to_string() } else { r.clone() }
    })
}

fn chosen(app: &App) -> Option<&ModelEntry> {
    let id = chosen_id(app);
    app.models.iter().find(|m| m.entry.id == id)
}

/// Whether 次へ may be pressed: after the download once the model is in use and measured, and
/// not while the mic test listens.
pub fn can_next(app: &App) -> bool {
    let w = &app.wizard;
    match w.step() {
        Step::Download => chosen(app).is_some_and(|m| m.installed && m.selected) && !w.benching,
        Step::MicTest => !w.testing,
        _ => true,
    }
}

/// On the download step: starts the download, then uses the model once it is here, then measures
/// it. Runs on entering the step and whenever the model list changes.
fn drive(app: &mut App) -> Task<Message> {
    if app.wizard.step() != Step::Download {
        return Task::none();
    }
    let Some(m) = chosen(app) else { return Task::none() };
    let id = m.entry.id.clone();
    let (installed, selected, downloading) = (m.installed, m.selected, m.downloading);
    let phase = app.models_ui.progress.get(&id).map(|p| p.phase);
    if !installed && !downloading && phase != Some("downloading") {
        return start_download(app, id);
    }
    if installed && !selected {
        let picked = id.clone();
        return call(
            app,
            move |c| (ops::model_select(c, &id), ops::models_list(c)),
            move |(r, list)| msg(Msg::Selected(picked, r, list)),
        );
    }
    if installed && selected && app.wizard.bench.is_none() && !app.wizard.benching {
        app.wizard.benching = true;
        return call(app, move |c| ops::run_benchmark(c, &id), |r| msg(Msg::Benchmarked(r)));
    }
    Task::none()
}

fn start_download(app: &App, id: String) -> Task<Message> {
    call(app, move |c| ops::model_download(c, &id), |r| r.err().map_or(Message::Noop, |e| Message::Failed(e, "failed")))
}

fn go_to(app: &mut App, step: usize) -> Task<Message> {
    app.wizard.step = step.min(STEPS.len() - 1);
    drive(app)
}

pub fn update(app: &mut App, m: Msg) -> Task<Message> {
    match m {
        Msg::Next => go_to(app, app.wizard.step + 1),
        Msg::Back => go_to(app, app.wizard.step.saturating_sub(1)),
        Msg::Choose(id) => {
            app.wizard.choice = Some(id);
            Task::none()
        }
        Msg::Retry => match chosen(app) {
            Some(m) => start_download(app, m.entry.id.clone()),
            None => Task::none(),
        },
        Msg::UseLight => {
            app.wizard.choice = Some(LIGHT_MODEL_ID.to_string());
            app.wizard.bench = None;
            drive(app)
        }
        Msg::Models(list) => {
            app.models = list;
            drive(app)
        }
        Msg::Selected(id, r, list) => {
            match r {
                Ok(()) => app.settings.model_id = id,
                Err(e) => report_error(app, &e, "failed"),
            }
            app.models = list;
            drive(app)
        }
        Msg::Benchmarked(r) => {
            app.wizard.benching = false;
            // A failed speed test doesn't hold up setup.
            app.wizard.bench = r.ok();
            Task::none()
        }
        Msg::MicTest => {
            let w = &mut app.wizard;
            w.testing = true;
            w.mic = None;
            w.wave = Wave { at: Some(Instant::now()), ..Wave::default() };
            call(app, |c| ops::mic_test(c, 10), |r| msg(Msg::MicTested(r)))
        }
        Msg::MicTested(r) => {
            app.wizard.testing = false;
            match r {
                Ok(result) => app.wizard.mic = Some(result),
                Err(e) => report_error(app, &e, "failed"),
            }
            Task::none()
        }
        Msg::Finish => call(
            app,
            |c| {
                ops::complete_setup(c);
                Box::new(ops::app_info(c))
            },
            Message::Info,
        ),
    }
}

/// The mic test's levels draw its wave.
pub fn on_levels(app: &mut App, levels: &[(SourceId, f32)]) {
    let w = &mut app.wizard;
    if !w.testing {
        return;
    }
    let level = levels.iter().find(|(s, _)| *s == SourceId::Mic).map(|(_, d)| *d);
    let wave = &mut w.wave;
    wave.height = settle(wave.height, height_of(level));
    wave.samples.push(wave.height);
    if wave.samples.len() > WAVE_MAX {
        wave.samples.remove(0);
    }
    wave.at = Some(Instant::now());
}

/// The chosen model arrived: the list rereads, so the wizard can use and measure it.
pub fn on_download(app: &mut App, p: &DownloadPayload) -> Task<Message> {
    if !app.needs_wizard() || app.wizard.step() != Step::Download || p.phase != "done" || p.id != chosen_id(app) {
        return Task::none();
    }
    call(app, ops::models_list, |list| msg(Msg::Models(list)))
}

pub fn spinning(app: &App) -> bool {
    app.needs_wizard() && (app.wizard.benching || app.wizard.testing)
}

pub fn subscription(_app: &App) -> iced::Subscription<Message> {
    iced::Subscription::none()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let w = &app.wizard;
    let step = w.step();
    let i = w.step.min(STEPS.len() - 1);
    let bar = Row::with_children((0..STEPS.len()).map(|n| {
        let done = n <= i;
        container(Space::new())
            .width(Length::Fill)
            .height(4)
            .style(move |_| container::Style {
                background: Some(if done { p().accent } else { p().border }.into()),
                border: Border { radius: 2.0.into(), ..Border::default() },
                ..container::Style::default()
            })
            .into()
    }))
    .spacing(6);
    let head = column![
        form::head(t(step.title_key()), Some(format!("{} / {}", i + 1, STEPS.len()))),
        container(bar).padding(Padding::new(tokens::SPACE_4).top(0).bottom(tokens::SPACE_3)),
        container(rule()).padding([0.0, tokens::SPACE_3]),
    ];
    let body: Element<'_, Message> = match step {
        Step::Welcome => intro(Icon::ShieldCheck, t("wizardWelcomeBody")),
        Step::Model => model_step(app),
        Step::Download => download_step(app),
        Step::MicTest => mic_step(app),
        Step::Hotkeys => hotkey_step(app),
        Step::Done => form::para(t("wizardDoneBody")),
    };
    column![head, form::scroll(container(body).width(Length::Fill).padding([14.0, 18.0])), hairline(), footer(app),]
        .into()
}

/// A step's icon over its one line.
fn intro<'a>(which: Icon, text: String) -> Element<'a, Message> {
    column![icon(which, 28.0, p().ink), form::para(text)].spacing(tokens::SPACE_3).into()
}

fn footer(app: &App) -> Element<'_, Message> {
    let w = &app.wizard;
    let back: Element<'_, Message> = if w.step > 0 {
        btn(t("back"), Variant::Secondary, Size::Md, Some(msg(Msg::Back))).into()
    } else {
        Space::new().into()
    };
    let mut end = row![].spacing(tokens::SPACE_2).align_y(Alignment::Center);
    if w.step() == Step::MicTest {
        end = end.push(btn(t("skip"), Variant::Ghost, Size::Md, Some(msg(Msg::Next))));
    }
    end = end.push(if w.step() == Step::Done {
        btn(t("wizardFinish"), Variant::Primary, Size::Md, Some(msg(Msg::Finish)))
    } else {
        btn(t("next"), Variant::Primary, Size::Md, can_next(app).then(|| msg(Msg::Next)))
    });
    container(row![back, Space::new().width(Length::Fill), end].align_y(Alignment::Center))
        .padding(Padding::new(tokens::SPACE_4).top(10).bottom(tokens::SPACE_3))
        .into()
}

/// The visible models as choices, the recommended one picked until another is.
fn model_step(app: &App) -> Element<'_, Message> {
    let id = chosen_id(app);
    let cards = app.models.iter().filter(|m| !m.entry.hidden || m.installed).map(|m| {
        let on = m.entry.id == id;
        let mut name = row![theme::t(name_of(m), tokens::LABEL, p().ink)].spacing(6).align_y(Alignment::Center);
        if m.recommended {
            name = name.push(form::tag(t("recommended"), true));
        }
        let mut about = format!("{} · {}", description_of(m), bytes(m.entry.size_bytes));
        if m.installed {
            about = format!("{about} · {}", t("installed"));
        }
        let content = row![
            container(form::radio_mark(on)).padding(Padding::ZERO.top(3)),
            column![name.wrap(), theme::t(about, tokens::CAPTION, p().ink_muted)].width(Length::Fill),
        ]
        .spacing(10);
        button(container(content).width(Length::Fill).padding(tokens::SPACE_3))
            .width(Length::Fill)
            .padding(0)
            .on_press(msg(Msg::Choose(m.entry.id.clone())))
            .style(move |_, status| {
                let s = form::card_style(on, matches!(status, button::Status::Hovered | button::Status::Pressed));
                button::Style { background: s.background, border: s.border, ..button::Style::default() }
            })
            .into()
    });
    Column::with_children(cards).spacing(tokens::SPACE_2).into()
}

fn download_step(app: &App) -> Element<'_, Message> {
    let w = &app.wizard;
    let Some(m) = chosen(app) else { return crate::app::nothing() };
    let p_ = app.models_ui.progress.get(&m.entry.id);
    let mut col = Column::new().spacing(tokens::SPACE_3).push(theme::t(
        format!("{} · {}", name_of(m), bytes(m.entry.size_bytes)),
        tokens::LABEL,
        p().ink,
    ));
    if !m.installed {
        let received = p_.map_or(m.partial_bytes, |p| p.received);
        col = col.push(download_progress(received, m.entry.size_bytes, p_.map_or(0.0, |p| p.bytes_per_sec)));
        if let Some(failed) = p_.filter(|p| p.phase == "error") {
            let checksum = failed.error.as_ref().is_some_and(|e| e.code == ErrorCode::ModelChecksum);
            col = col.push(
                row![
                    theme::t(t(if checksum { "checksumError" } else { "networkError" }), tokens::CAPTION, p().danger),
                    btn(t("retry"), Variant::Secondary, Size::Sm, Some(msg(Msg::Retry))),
                ]
                .spacing(tokens::SPACE_2)
                .align_y(Alignment::Center),
            );
        }
        return col.into();
    }
    let mut done = Column::new().spacing(tokens::SPACE_1).push(line(
        format!("✓ {}", t("installed")),
        tokens::CAPTION,
        p().success,
    ));
    if w.benching {
        done = done.push(
            row![spinner(13.0, app.now), line(format!("{}…", t("benchmark")), tokens::CAPTION, p().ink_muted)]
                .spacing(4)
                .align_y(Alignment::Center),
        );
    }
    if let Some(b) = &w.bench {
        let b = Bench::from(b);
        let tone = if b.heavy() { p().warning } else { p().success };
        done = done.push(theme::t(b.text(), tokens::CAPTION, tone));
        if b.heavy() && m.entry.id != LIGHT_MODEL_ID {
            done = done.push(
                row![
                    theme::t(t("tierHeavyHint"), tokens::CAPTION, tone),
                    btn(t("download"), Variant::Secondary, Size::Sm, Some(msg(Msg::UseLight))),
                ]
                .spacing(tokens::SPACE_2)
                .align_y(Alignment::Center),
            );
        }
    }
    col.push(done).into()
}

fn mic_step(app: &App) -> Element<'_, Message> {
    let w = &app.wizard;
    // Secondary: 次へ is the step's one primary.
    let test = if w.testing {
        crate::widgets::btn_with(
            row![spinner(14.0, app.now), theme::plain(t("wizardMicListening"), tokens::LABEL)].spacing(6),
            Variant::Secondary,
            Size::Md,
            None,
        )
    } else {
        btn_icon(Icon::Mic, t("wizardMicStart"), Variant::Secondary, Size::Md, Some(msg(Msg::MicTest)))
    };
    let mut col = column![
        icon(Icon::Mic, 28.0, p().ink),
        form::para(t("wizardMicTestBody")),
        row![test, wave(app)].spacing(tokens::SPACE_3).align_y(Alignment::Center),
    ]
    .spacing(tokens::SPACE_3);
    if let Some(mic) = &w.mic {
        let mut result = Column::new();
        if let Some(device) = &mic.device {
            result = result
                .push(line(device.clone(), tokens::CAPTION, p().ink_muted))
                .push(Space::new().height(tokens::SPACE_1));
        }
        if mic.text.is_empty() {
            result = result.push(theme::t(t("wizardMicNothing"), tokens::CAPTION, p().ink_muted));
        } else {
            result = result.push(line(t("wizardMicResult"), tokens::CAPTION, p().ink_muted)).push(theme::t(
                mic.text.clone(),
                tokens::BODY_SMALL,
                p().ink,
            ));
        }
        col = col.push(form::card(result, false));
    }
    col.into()
}

fn hotkey_step(app: &App) -> Element<'_, Message> {
    column![
        icon(Icon::Keyboard, 28.0, p().ink),
        form::para(t("wizardHotkeysBody")),
        column![settings::hotkey_input(app, Hotkey::Toggle), settings::hotkey_input(app, Hotkey::Screenshot)],
    ]
    .spacing(tokens::SPACE_3)
    .into()
}

const WAVE_W: f32 = 200.0;
const WAVE_H: f32 = 36.0;
const BASE: f32 = 26.0;
const UP: f32 = 20.0;
const STEP_PX: f32 = 1.6;
const START_X: f32 = 6.0;
const END_PAD: f32 = 8.0;
/// How far the line not written yet takes to come back down to the baseline.
const SLOPE: f32 = 12.0;

struct WaveProgram<'a> {
    samples: &'a [f32],
    /// How far the pen has moved past the newest step, 0..1.
    frac: f32,
    active: bool,
}

/// Where the pen is and the written line's points, for samples in a box `WAVE_W` wide. The pen
/// moves right until it is `END_PAD` from the end; after that the line scrolls under it.
fn wave_points(samples: &[f32], frac: f32) -> (Vec<Point>, Point) {
    let n = samples.len();
    if n == 0 {
        return (Vec::new(), Point::new(START_X, BASE));
    }
    let t = (n as f32 - 1.0 + frac).max(0.0);
    let px = (START_X + t * STEP_PX).min(WAVE_W - END_PAD);
    let x = |i: usize| px - (t - i as f32) * STEP_PX;
    // A light 1-2-1 blur, so a step that stands alone does not look like a spike.
    let soft = |i: usize| (samples[i.saturating_sub(1)] + 2.0 * samples[i] + samples[(i + 1).min(n - 1)]) / 4.0;
    let first = (0..n).find(|i| x(*i) >= -STEP_PX).unwrap_or(0);
    let mut pts = Vec::with_capacity(n - first + 2);
    if first == 0 {
        // The line rises from the baseline where it starts.
        pts.push(Point::new(x(0) - STEP_PX, BASE));
    }
    pts.extend((first..n).map(|i| Point::new(x(i), BASE - soft(i) * UP)));
    let pen = Point::new(px, BASE - soft(n - 1) * UP);
    pts.push(pen);
    (pts, pen)
}

impl canvas::Program<Message> for WaveProgram<'_> {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let stroke = |c| {
            Stroke::default()
                .with_color(c)
                .with_width(2.6)
                .with_line_cap(LineCap::Round)
                .with_line_join(LineJoin::Round)
        };
        let (pts, pen) = wave_points(self.samples, self.frac);
        let ahead = Path::new(|b| {
            b.move_to(pen);
            if BASE - pen.y >= 0.5 {
                let h = SLOPE / 2.0;
                b.bezier_curve_to(
                    Point::new(pen.x + h, pen.y),
                    Point::new(pen.x + h, BASE),
                    Point::new(pen.x + SLOPE, BASE),
                );
            }
            b.line_to(Point::new(WAVE_W, BASE));
        });
        frame.stroke(&ahead, stroke(p().border_strong));
        if pts.len() > 1 {
            frame.stroke(&Path::new(|b| smooth(b, &pts, false)), stroke(p().accent));
        }
        frame.fill(&Path::circle(pen, if self.active { 3.5 } else { 5.0 }), p().accent);
        vec![frame.into_geometry()]
    }
}

fn wave(app: &App) -> Element<'_, Message> {
    let w = &app.wizard;
    let frac = w.wave.at.map_or(0.0, |at| (app.now.saturating_duration_since(at).as_secs_f32() / 0.1).clamp(0.0, 1.0));
    let samples: &[f32] = if w.testing { &w.wave.samples } else { &[] };
    let program = WaveProgram { samples, frac: if w.testing { frac } else { 0.0 }, active: w.testing };
    canvas::Canvas::new(program).width(WAVE_W).height(WAVE_H).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, installed: bool, selected: bool) -> ModelEntry {
        ModelEntry {
            entry: chidori_core::models::catalog::find(id).unwrap().clone(),
            installed,
            selected,
            recommended: id == DEFAULT_MODEL_ID,
            partial_bytes: 0,
            downloading: false,
            benchmark: None,
        }
    }

    #[test]
    fn the_recommended_model_is_chosen_until_another_is() {
        let (mut app, _dir) = crate::preview::app();
        app.info.recommended_model = "turbo-q5".into();
        assert_eq!(chosen_id(&app), "turbo-q5");
        let _ = update(&mut app, Msg::Choose("small-q5".into()));
        assert_eq!(chosen_id(&app), "small-q5");
    }

    #[test]
    fn next_waits_for_the_model_in_use_and_measured_and_for_the_mic_test() {
        let (mut app, _dir) = crate::preview::app();
        app.info.setup_done = false;
        app.info.recommended_model = "turbo-q5".into();
        app.models = vec![entry("turbo-q5", false, false)];
        app.wizard.step = 2;
        assert!(!can_next(&app));
        app.models = vec![entry("turbo-q5", true, false)];
        assert!(!can_next(&app));
        app.models = vec![entry("turbo-q5", true, true)];
        app.wizard.benching = true;
        assert!(!can_next(&app));
        app.wizard.benching = false;
        assert!(can_next(&app));
        app.wizard.step = 3;
        app.wizard.testing = true;
        assert!(!can_next(&app));
        app.wizard.step = 4;
        assert!(can_next(&app));
    }

    #[test]
    fn a_heavy_model_gives_way_to_the_light_one() {
        let (mut app, _dir) = crate::preview::app();
        app.wizard.step = 0;
        app.wizard.bench = Some(BenchmarkResult {
            seconds: 14.0,
            tier: chidori_core::models::benchmark::Tier::Heavy,
            gpu: false,
            text: String::new(),
        });
        let _ = update(&mut app, Msg::UseLight);
        assert_eq!(chosen_id(&app), LIGHT_MODEL_ID);
        assert!(app.wizard.bench.is_none());
    }

    #[test]
    fn the_wave_rises_with_the_voice_and_the_pen_stops_short_of_the_end() {
        let (pts, pen) = wave_points(&[0.0, 1.0, 1.0], 0.0);
        assert!(pen.y < BASE);
        assert_eq!(pts.first().map(|p| p.y), Some(BASE));
        let long = vec![0.5; 400];
        let (_, pen) = wave_points(&long, 1.0);
        assert!((pen.x - (WAVE_W - END_PAD)).abs() < 1e-3);
    }
}

#[cfg(test)]
mod previews {
    use chidori_core::models::benchmark::Tier;

    use super::*;
    use crate::preview::{app, shot};

    fn at(step: usize) -> (App, tempfile::TempDir) {
        let (mut app, dir) = app();
        app.info.setup_done = false;
        app.info.recommended_model = "turbo-q5".into();
        app.models = chidori_core::models::catalog::all()
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
            .collect();
        app.wizard.step = step;
        (app, dir)
    }

    #[test]
    #[ignore = "writes PNGs for review"]
    fn preview_wizard() {
        let (app, _dir) = at(0);
        shot(&app, "wizard-welcome", crate::app::EXPANDED);

        let (app, _dir) = at(1);
        shot(&app, "wizard-model", crate::app::EXPANDED);

        let (mut app, _dir) = at(2);
        let size = app.models[0].entry.size_bytes;
        app.models_ui.progress.insert(
            "turbo-q5".into(),
            DownloadPayload {
                id: "turbo-q5".into(),
                received: size / 2,
                total: size,
                bytes_per_sec: 6.1e6,
                phase: "downloading",
                error: None,
            },
        );
        shot(&app, "wizard-download", crate::app::EXPANDED);

        let (mut app, _dir) = at(2);
        app.models[0].installed = true;
        app.models[0].selected = true;
        app.wizard.bench = Some(BenchmarkResult { seconds: 14.2, tier: Tier::Heavy, gpu: false, text: String::new() });
        shot(&app, "wizard-download-heavy", crate::app::EXPANDED);

        let (mut app, _dir) = at(3);
        app.wizard.testing = true;
        for i in 0..60 {
            let level = if (i / 8) % 2 == 0 { -14.0 } else { -50.0 };
            on_levels(&mut app, &[(SourceId::Mic, level)]);
        }
        app.now = app.wizard.wave.at.unwrap();
        shot(&app, "wizard-mic-listening", crate::app::EXPANDED);
        app.wizard.testing = false;
        app.wizard.mic =
            Some(MicTestResult { peak_dbfs: -12.0, text: "テスト中です".into(), device: Some("USB Mic".into()) });
        shot(&app, "wizard-mic-result", crate::app::EXPANDED);

        let (mut app, _dir) = at(4);
        app.settings.hotkeys.toggle = "Ctrl+Shift+R".into();
        shot(&app, "wizard-hotkeys", crate::app::EXPANDED);

        let (app, _dir) = at(5);
        shot(&app, "wizard-done", crate::app::EXPANDED);
    }
}

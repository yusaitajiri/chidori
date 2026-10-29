//! What the core says about the recording: its state, source, time, lag and finishing progress.

use std::time::Instant;

use chidori_core::asr::worker::EngineStatus;
use chidori_core::events::{LagPayload, ProgressPayload, RecordedSources, StatePayload, UiState};
use chidori_core::screenshot::WindowInfo;

#[derive(Debug, Clone)]
pub struct Recording {
    pub state: UiState,
    pub session_id: Option<String>,
    pub sources: Option<RecordedSources>,
    pub folder: Option<String>,
    /// The window screenshots take, when one was picked for this recording (FR-34).
    pub shot_window: Option<WindowInfo>,
    /// Transcription waits for Stop (FR-25).
    pub deferred: bool,
    /// Recorded time at `elapsed_at`; the UI counts on between the core's updates.
    pub elapsed_ms: u64,
    pub elapsed_at: Instant,
    pub lag: Option<LagPayload>,
    pub finishing: Option<ProgressPayload>,
    pub engine: Option<EngineStatus>,
    pub last_saved_folder: Option<String>,
}

impl Default for Recording {
    fn default() -> Self {
        Self {
            state: UiState::Ready,
            session_id: None,
            sources: None,
            folder: None,
            shot_window: None,
            deferred: false,
            elapsed_ms: 0,
            elapsed_at: Instant::now(),
            lag: None,
            finishing: None,
            engine: None,
            last_saved_folder: None,
        }
    }
}

pub fn is_busy(s: UiState) -> bool {
    matches!(s, UiState::Recording | UiState::Paused | UiState::Finishing)
}

impl Recording {
    pub fn apply(&mut self, p: &StatePayload) {
        let finishing = p.state == UiState::Finishing;
        let live = matches!(p.state, UiState::Recording | UiState::Paused);
        self.session_id = p.session_id.clone().or_else(|| if finishing { self.session_id.clone() } else { None });
        self.sources = p.sources.clone().or_else(|| if finishing { self.sources.clone() } else { None });
        if let Some(f) = &p.folder {
            self.folder = Some(f.clone());
        }
        self.shot_window = p.shot_window.clone();
        self.deferred = p.deferred;
        self.elapsed_ms = p.elapsed_ms.unwrap_or(if live || finishing { self.elapsed_ms } else { 0 });
        self.elapsed_at = Instant::now();
        if !live {
            self.lag = None;
        }
        if !finishing {
            self.finishing = None;
        }
        self.state = p.state;
    }

    pub fn busy(&self) -> bool {
        is_busy(self.state)
    }

    /// The timer's time: counting on while recording, standing still while paused.
    pub fn elapsed(&self, now: Instant) -> u64 {
        if self.state == UiState::Recording {
            self.elapsed_ms + now.saturating_duration_since(self.elapsed_at).as_millis() as u64
        } else {
            self.elapsed_ms
        }
    }
}

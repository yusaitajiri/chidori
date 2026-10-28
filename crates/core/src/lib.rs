//! Chidori's core: capture, transcription, sessions, models and export. The UI lives in the app
//! crate and talks to this through `host::AppHandle` and `events::Event`.

pub mod asr;
pub mod audio;
pub mod error;
pub mod events;
pub mod export;
pub mod host;
pub mod mic_test;
pub mod migrate;
pub mod models;
pub mod net;
pub mod ops;
pub mod platform;
pub mod recorder;
pub mod screenshot;
pub mod segmenter;
pub mod session;
pub mod settings;
pub mod state;
pub mod update;
pub mod vad;

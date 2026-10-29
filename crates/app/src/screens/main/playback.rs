//! Plays a finished session's sound (FR-09): from a line or from the deck's button. A session
//! continued later has one file per recording; playing runs on from one into the next. Its own
//! thread decodes (the decoder stays on the thread that opened it) and feeds the sound card; the
//! UI reads where it is four times a second.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chidori_core::audio::decode::{self, AudioDecoder, Block};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Where the player is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Idle,
    Playing,
    Paused,
    /// The last file played to its end.
    Ended,
    /// A file couldn't be opened or played.
    Failed,
}

impl Status {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Status::Playing,
            2 => Status::Paused,
            3 => Status::Ended,
            4 => Status::Failed,
            _ => Status::Idle,
        }
    }
}

enum Cmd {
    Play { files: Vec<(PathBuf, u64)>, from_ms: u64 },
    Pause,
    Resume,
    Stop,
}

#[derive(Default)]
struct Shared {
    status: AtomicU8,
    pos_ms: AtomicU64,
}

impl Shared {
    fn set(&self, s: Status) {
        self.status.store(s as u8, Ordering::Relaxed);
    }
}

/// A session's player. The thread starts with the first play and ends with the player.
#[derive(Default)]
pub struct Player {
    tx: Option<Sender<Cmd>>,
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player").field("status", &self.status()).finish()
    }
}

impl Player {
    pub fn status(&self) -> Status {
        Status::from_u8(self.shared.status.load(Ordering::Relaxed))
    }

    /// Where the sound is, in ms into the session, while it plays or is paused.
    pub fn position(&self) -> Option<u64> {
        matches!(self.status(), Status::Playing | Status::Paused).then(|| self.shared.pos_ms.load(Ordering::Relaxed))
    }

    pub fn playing(&self) -> bool {
        self.status() == Status::Playing
    }

    fn send(&mut self, cmd: Cmd) {
        if self.tx.is_none() {
            let (tx, rx) = mpsc::channel();
            let shared = self.shared.clone();
            let spawned = std::thread::Builder::new().name("playback".into()).spawn(move || run(rx, shared));
            if let Err(e) = spawned {
                tracing::warn!("playback thread: {e}");
                self.shared.set(Status::Failed);
                return;
            }
            self.tx = Some(tx);
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send(cmd);
        }
    }

    /// Plays from a moment of the session; `files` are its sound files with their start times.
    pub fn play_from(&mut self, files: Vec<(PathBuf, u64)>, ms: u64) {
        self.shared.pos_ms.store(ms, Ordering::Relaxed);
        self.shared.set(Status::Playing);
        self.send(Cmd::Play { files, from_ms: ms });
    }

    pub fn pause(&mut self) {
        if self.status() == Status::Playing {
            self.shared.set(Status::Paused);
            self.send(Cmd::Pause);
        }
    }

    pub fn resume(&mut self) {
        if self.status() == Status::Paused {
            self.shared.set(Status::Playing);
            self.send(Cmd::Resume);
        }
    }

    /// Stops and lets go of the file.
    pub fn stop(&mut self) {
        if self.tx.is_some() {
            self.send(Cmd::Stop);
        }
        self.shared.set(Status::Idle);
    }

    /// Forgets an end or a failure once the UI has seen it.
    pub fn reset(&mut self) {
        if matches!(self.status(), Status::Ended | Status::Failed) {
            self.shared.set(Status::Idle);
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Cmd::Stop);
        }
    }
}

/// About this much sound is decoded ahead of the sound card.
const AHEAD_MS: u64 = 400;

/// What the sound card plays from: samples at its rate and channel count, and how many frames it
/// has taken since the last seek.
#[derive(Default)]
struct Queue {
    samples: VecDeque<f32>,
    taken: u64,
}

/// One file being played.
struct Part {
    decoder: Box<dyn AudioDecoder>,
    start_ms: u64,
    /// Where in the file the queue's first sample is, in ms.
    base_ms: u64,
    /// Frames still to skip before the moment asked for (in the file's rate).
    skip_ms: u64,
    resampler: Resampler,
}

struct Output {
    stream: cpal::Stream,
    rate: u32,
    channels: u16,
    queue: Arc<Mutex<Queue>>,
}

fn run(rx: Receiver<Cmd>, shared: Arc<Shared>) {
    let mut out: Option<Output> = None;
    let mut files: Vec<(PathBuf, u64)> = Vec::new();
    let mut index = 0usize;
    let mut part: Option<Part> = None;
    let mut paused = false;
    loop {
        let busy = part.is_some() && !paused;
        let cmd = if busy {
            match rx.try_recv() {
                Ok(c) => Some(c),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            }
        } else {
            match rx.recv() {
                Ok(c) => Some(c),
                Err(_) => return,
            }
        };
        match cmd {
            Some(Cmd::Play { files: f, from_ms }) => {
                files = f;
                paused = false;
                index = files.iter().rposition(|(_, s)| *s <= from_ms).unwrap_or(0);
                if out.is_none() {
                    out = open_output().map_err(|e| tracing::warn!("playback output: {e:#}")).ok();
                }
                let Some(o) = &out else {
                    shared.set(Status::Failed);
                    continue;
                };
                if let Ok(mut q) = o.queue.lock() {
                    *q = Queue::default();
                }
                part = match open_part(&files, index, from_ms) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        tracing::warn!("playback: {e:#}");
                        shared.set(Status::Failed);
                        None
                    }
                };
                let _ = o.stream.play();
            }
            Some(Cmd::Pause) => {
                paused = true;
                if let Some(o) = &out {
                    let _ = o.stream.pause();
                }
            }
            Some(Cmd::Resume) => {
                paused = false;
                if let Some(o) = &out {
                    let _ = o.stream.play();
                }
            }
            Some(Cmd::Stop) => {
                part = None;
                out = None;
                paused = false;
            }
            None => {}
        }
        let (Some(o), Some(p)) = (&out, &mut part) else { continue };
        if paused {
            continue;
        }
        let (queued, taken) = o.queue.lock().map(|q| (q.samples.len(), q.taken)).unwrap_or((0, 0));
        let frames_ahead = queued / usize::from(o.channels.max(1));
        shared.pos_ms.store(p.start_ms + p.base_ms + taken * 1000 / u64::from(o.rate), Ordering::Relaxed);
        if (frames_ahead as u64) * 1000 / u64::from(o.rate) >= AHEAD_MS {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        match p.decoder.read() {
            Ok(Some(block)) => {
                let samples = p.feed(&block, o.rate, o.channels);
                if !samples.is_empty()
                    && let Ok(mut q) = o.queue.lock()
                {
                    q.samples.extend(samples);
                }
            }
            Ok(None) => {
                // The file is done once the sound card has played what is queued.
                if queued > 0 {
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }
                index += 1;
                if index >= files.len() {
                    part = None;
                    out = None;
                    shared.set(Status::Ended);
                    continue;
                }
                let start = files[index].1;
                if let Ok(mut q) = o.queue.lock() {
                    *q = Queue::default();
                }
                part = match open_part(&files, index, start) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        tracing::warn!("playback: {e:#}");
                        shared.set(Status::Failed);
                        None
                    }
                };
            }
            Err(e) => {
                tracing::warn!("playback decode: {e:#}");
                part = None;
                out = None;
                shared.set(Status::Failed);
            }
        }
    }
}

fn open_part(files: &[(PathBuf, u64)], index: usize, from_ms: u64) -> anyhow::Result<Part> {
    let (path, start_ms) = files.get(index).ok_or_else(|| anyhow::anyhow!("no sound file"))?;
    let decoder = decode::open(path)?;
    let into = from_ms.saturating_sub(*start_ms);
    Ok(Part { decoder, start_ms: *start_ms, base_ms: into, skip_ms: into, resampler: Resampler::new() })
}

impl Part {
    /// A decoded block as the sound card's samples, after skipping to the moment asked for.
    fn feed(&mut self, block: &Block, out_rate: u32, out_channels: u16) -> Vec<f32> {
        let ch = usize::from(block.channels.max(1));
        let mut frames = &block.samples[..];
        if self.skip_ms > 0 {
            let in_block = (frames.len() / ch) as u64 * 1000 / u64::from(block.rate.max(1));
            if in_block <= self.skip_ms {
                self.skip_ms -= in_block;
                return Vec::new();
            }
            let skip = (self.skip_ms * u64::from(block.rate) / 1000) as usize * ch;
            frames = &frames[skip.min(frames.len())..];
            self.skip_ms = 0;
        }
        self.resampler.process(frames, block.rate, block.channels, out_rate, out_channels)
    }
}

/// Linear resampling and channel mapping, carried across blocks.
struct Resampler {
    /// The previous block's last frame, which the next block's first output may fall after.
    prev: Vec<f32>,
    /// Where the next output frame falls, in input frames from `prev` (or the block's start).
    pos: f64,
}

impl Resampler {
    fn new() -> Self {
        Self { prev: Vec::new(), pos: 0.0 }
    }

    fn process(&mut self, input: &[f32], in_rate: u32, in_ch: u16, out_rate: u32, out_ch: u16) -> Vec<f32> {
        let ic = usize::from(in_ch.max(1));
        let oc = usize::from(out_ch.max(1));
        if self.prev.len() != ic {
            self.prev.clear();
        }
        let mut buf = Vec::with_capacity(self.prev.len() + input.len());
        buf.extend_from_slice(&self.prev);
        buf.extend_from_slice(input);
        let n = buf.len() / ic;
        if n < 2 {
            self.prev = buf[..n * ic].to_vec();
            return Vec::new();
        }
        let channel = |fr: &[f32], c: usize| {
            if ic == 1 {
                fr[0]
            } else if oc == 1 {
                fr.iter().sum::<f32>() / ic as f32
            } else {
                fr[c.min(ic - 1)]
            }
        };
        let step = f64::from(in_rate) / f64::from(out_rate.max(1));
        let mut out = Vec::with_capacity(((n as f64 / step) as usize + 2) * oc);
        while self.pos < (n - 1) as f64 {
            let i = self.pos.floor() as usize;
            let f = (self.pos - i as f64) as f32;
            let (a, b) = (&buf[i * ic..(i + 1) * ic], &buf[(i + 1) * ic..(i + 2) * ic]);
            for c in 0..oc {
                let (x, y) = (channel(a, c), channel(b, c));
                out.push(x + (y - x) * f);
            }
            self.pos += step;
        }
        self.prev = buf[(n - 1) * ic..n * ic].to_vec();
        self.pos -= (n - 1) as f64;
        out
    }
}

fn open_output() -> anyhow::Result<Output> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or_else(|| anyhow::anyhow!("no output device"))?;
    let supported = device.default_output_config()?;
    let config = supported.config();
    let queue = Arc::new(Mutex::new(Queue::default()));
    let channels = config.channels;
    let rate = config.sample_rate;
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build::<f32>(&device, config, queue.clone())?,
        cpal::SampleFormat::I16 => build::<i16>(&device, config, queue.clone())?,
        cpal::SampleFormat::I32 => build::<i32>(&device, config, queue.clone())?,
        cpal::SampleFormat::U16 => build::<u16>(&device, config, queue.clone())?,
        other => anyhow::bail!("unsupported sample format {other:?}"),
    };
    Ok(Output { stream, rate, channels, queue })
}

fn build<T>(device: &cpal::Device, config: cpal::StreamConfig, queue: Arc<Mutex<Queue>>) -> anyhow::Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let ch = usize::from(config.channels.max(1));
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            let Ok(mut q) = queue.lock() else { return };
            let mut taken = 0u64;
            for frame in data.chunks_mut(ch) {
                if q.samples.len() >= ch {
                    for s in frame.iter_mut() {
                        *s = T::from_sample(q.samples.pop_front().unwrap_or(0.0));
                    }
                    taken += 1;
                } else {
                    for s in frame.iter_mut() {
                        *s = T::from_sample(0.0);
                    }
                }
            }
            q.taken += taken;
        },
        |e| tracing::warn!("playback stream: {e}"),
        None,
    )?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_keeps_the_length_and_maps_channels() {
        let mut r = Resampler::new();
        let mono: Vec<f32> = (0..4800).map(|i| i as f32 / 4800.0).collect();
        let mut out = Vec::new();
        for chunk in mono.chunks(480) {
            out.extend(r.process(chunk, 48_000, 1, 44_100, 2));
        }
        let frames = out.len() / 2;
        assert!((frames as i64 - 4410).abs() <= 2, "{frames}");
        assert!(out.chunks(2).all(|f| f[0] == f[1]));
        // Rising input stays rising across blocks.
        assert!(out.chunks(2).map(|f| f[0]).collect::<Vec<_>>().windows(2).all(|w| w[1] >= w[0]));
    }
}

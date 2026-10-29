//! The sound of an audio or video file, decoded piece by piece (FR-18). Windows decodes with Media
//! Foundation (`audio::win::media`), which reads what the PC can play; Ogg Opus, which it can't,
//! is read here with libopus. Elsewhere only WAV and Ogg Opus are read.

use std::path::Path;

/// Interleaved samples in [-1, 1], at the rate and channel count they came in. A file may change
/// format partway (a new stream), so each block says its own.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

/// Not `Send`: Media Foundation's reader stays on the thread that opened it.
pub trait AudioDecoder {
    /// The file's length, when the container states it.
    fn duration_ms(&self) -> Option<u64>;
    /// The next block; `None` at the end of the file.
    fn read(&mut self) -> anyhow::Result<Option<Block>>;
}

/// Why a file gives no sound to transcribe, for the UI to say in words.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("the file has no sound track")]
    NoAudio,
    #[error("this PC can't read the file's format")]
    Unsupported,
}

/// Opens a file for decoding, on the thread that will read it.
pub fn open(path: &Path) -> anyhow::Result<Box<dyn AudioDecoder>> {
    if OggOpusDecoder::is_ogg_opus(path) {
        return Ok(Box::new(OggOpusDecoder::open(path)?));
    }
    #[cfg(windows)]
    {
        Ok(Box::new(super::win::media::MediaDecoder::open(path)?))
    }
    #[cfg(not(windows))]
    {
        Ok(Box::new(WavDecoder::open(path)?))
    }
}

/// About this much is decoded at a time.
const BLOCK_MS: u64 = 100;

pub struct WavDecoder {
    reader: hound::WavReader<std::io::BufReader<std::fs::File>>,
    spec: hound::WavSpec,
}

impl WavDecoder {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let reader = hound::WavReader::open(path).map_err(|e| match e {
            hound::Error::IoError(e) => anyhow::Error::from(e),
            _ => DecodeError::Unsupported.into(),
        })?;
        let spec = reader.spec();
        Ok(Self { reader, spec })
    }
}

impl AudioDecoder for WavDecoder {
    fn duration_ms(&self) -> Option<u64> {
        Some(u64::from(self.reader.duration()) * 1000 / u64::from(self.spec.sample_rate))
    }

    fn read(&mut self) -> anyhow::Result<Option<Block>> {
        let n = (u64::from(self.spec.sample_rate) * BLOCK_MS / 1000) as usize * usize::from(self.spec.channels);
        let samples: Vec<f32> = match self.spec.sample_format {
            hound::SampleFormat::Float => self.reader.samples::<f32>().take(n).collect::<Result<_, _>>()?,
            hound::SampleFormat::Int => {
                let scale = (1u64 << (self.spec.bits_per_sample - 1)) as f32;
                self.reader.samples::<i32>().take(n).map(|s| s.map(|v| v as f32 / scale)).collect::<Result<_, _>>()?
            }
        };
        if samples.is_empty() {
            return Ok(None);
        }
        Ok(Some(Block { rate: self.spec.sample_rate, channels: self.spec.channels, samples }))
    }
}

/// Opus decodes at 48 kHz; its longest packet is 120 ms.
const OPUS_RATE: u32 = 48_000;
const OPUS_MAX_FRAMES: usize = 5_760;

/// Ogg Opus (`.opus`, most `.ogg` voice messages, and the sound Chidori keeps, FR-09), which Windows
/// has no reader for: decoded with the libopus the app already carries to write that sound.
pub struct OggOpusDecoder {
    reader: ogg::reading::PacketReader<std::io::BufReader<std::fs::File>>,
    decoder: opusic_c::Decoder,
    channels: u16,
    /// Frames still to drop at the start: the encoder's lookahead (`pre-skip`).
    skip: usize,
    duration_ms: Option<u64>,
    out: Vec<f32>,
}

impl OggOpusDecoder {
    /// An Ogg stream whose first packet is an Opus header (RFC 7845).
    fn is_ogg_opus(path: &Path) -> bool {
        use std::io::Read;
        let mut head = [0u8; 36];
        std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)).is_ok()
            && head.starts_with(b"OggS")
            && head[28..].starts_with(b"OpusHead")
    }

    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let mut reader = ogg::reading::PacketReader::new(std::io::BufReader::new(std::fs::File::open(path)?));
        let head = reader.read_packet()?.ok_or(DecodeError::Unsupported)?.data;
        // Mapping family 0 is mono or stereo; more channels need libopus's multistream decoder.
        if head.len() < 19 || !head.starts_with(b"OpusHead") || !matches!(head[9], 1 | 2) || head[18] != 0 {
            return Err(DecodeError::Unsupported.into());
        }
        let channels = u16::from(head[9]);
        let pre_skip = u16::from_le_bytes([head[10], head[11]]);
        // The comment header.
        reader.read_packet()?.ok_or(DecodeError::NoAudio)?;
        let layout = if channels == 1 { opusic_c::Channels::Mono } else { opusic_c::Channels::Stereo };
        let decoder = opusic_c::Decoder::new(layout, opusic_c::SampleRate::Hz48000)
            .map_err(|e| anyhow::anyhow!("opus decoder: {e:?}"))?;
        let duration_ms =
            last_granule(path).map(|g| g.saturating_sub(u64::from(pre_skip)) * 1000 / u64::from(OPUS_RATE));
        Ok(Self {
            reader,
            decoder,
            channels,
            skip: usize::from(pre_skip),
            duration_ms,
            out: vec![0.0; OPUS_MAX_FRAMES * usize::from(channels)],
        })
    }
}

/// The granule position of a stream's last page: where its sound ends, in 48 kHz samples.
fn last_granule(path: &Path) -> Option<u64> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    // A page is at most about 64 KiB.
    f.seek(SeekFrom::Start(len.saturating_sub(70_000))).ok()?;
    let mut tail = Vec::new();
    f.read_to_end(&mut tail).ok()?;
    let at = tail.windows(4).rposition(|w| w == b"OggS")?;
    let granule = tail.get(at + 6..at + 14)?;
    Some(u64::from_le_bytes(granule.try_into().ok()?))
}

impl AudioDecoder for OggOpusDecoder {
    fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    fn read(&mut self) -> anyhow::Result<Option<Block>> {
        let ch = usize::from(self.channels);
        loop {
            let Some(packet) = self.reader.read_packet()? else { return Ok(None) };
            let frames = self
                .decoder
                .decode_float_to_slice(&packet.data, &mut self.out, false)
                .map_err(|e| anyhow::anyhow!("opus decode: {e:?}"))?;
            let dropped = frames.min(self.skip);
            self.skip -= dropped;
            if frames > dropped {
                let samples = self.out[dropped * ch..frames * ch].to_vec();
                return Ok(Some(Block { rate: OPUS_RATE, channels: self.channels, samples }));
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A WAV of a 440 Hz tone, `secs` long.
    pub fn tone_wav(path: &Path, rate: u32, channels: u16, secs: f32) {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..(secs * rate as f32) as usize {
            let v = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / rate as f32).sin() * 0.5;
            for _ in 0..channels {
                w.write_sample((v * i16::MAX as f32) as i16).unwrap();
            }
        }
        w.finalize().unwrap();
    }

    fn read_all(d: &mut dyn AudioDecoder) -> (u32, u16, usize) {
        let (mut rate, mut channels, mut n) = (0, 0, 0);
        while let Some(b) = d.read().unwrap() {
            (rate, channels) = (b.rate, b.channels);
            n += b.samples.len();
        }
        (rate, channels, n)
    }

    #[test]
    fn a_wav_is_read_whole_in_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        tone_wav(&path, 22_050, 2, 1.5);
        let mut d = WavDecoder::open(&path).unwrap();
        assert_eq!(d.duration_ms(), Some(1_500));
        assert_eq!(read_all(&mut d), (22_050, 2, 33_075 * 2));
    }

    #[test]
    fn the_sound_chidori_keeps_reads_back() {
        use crate::session::audio::{AudioChunk, AudioRecorder};
        use crate::session::model::SourceId;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("0001.ogg");
        let sound = AudioRecorder::start(path.clone(), 0).unwrap();
        let tone: Vec<f32> = (0..32_000).map(|i| (i as f32 * 0.17).sin() * 0.3).collect();
        sound.sender().send(AudioChunk { source: SourceId::File, pos: 0, samples: tone }).unwrap();
        sound.finish();
        let mut d = open(&path).unwrap();
        assert!(d.duration_ms().is_some_and(|ms| ms.abs_diff(2_000) <= 40), "{:?}", d.duration_ms());
        let (rate, channels, n) = read_all(d.as_mut());
        assert_eq!((rate, channels), (48_000, 1));
        assert!(n.abs_diff(96_000) <= 960, "{n} samples");
    }

    #[test]
    fn what_is_not_a_sound_file_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.wav");
        std::fs::write(&path, "not a sound file").unwrap();
        let e = WavDecoder::open(&path).err().unwrap();
        assert!(matches!(e.downcast_ref::<DecodeError>(), Some(DecodeError::Unsupported)));
    }

    /// Media Foundation reads what Windows plays, WAV included, and gives float samples.
    #[cfg(windows)]
    #[test]
    fn media_foundation_reads_a_wav() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        tone_wav(&path, 44_100, 1, 2.0);
        let mut d = open(&path).unwrap();
        assert!(d.duration_ms().is_some_and(|ms| ms.abs_diff(2_000) <= 50), "{:?}", d.duration_ms());
        let (rate, channels, n) = read_all(d.as_mut());
        assert_eq!((rate, channels), (44_100, 1));
        assert!(n.abs_diff(88_200) <= 2_048, "{n} samples");
    }

    #[cfg(windows)]
    #[test]
    fn media_foundation_refuses_what_it_cannot_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.mp4");
        std::fs::write(&path, "not a video").unwrap();
        let e = open(&path).err().unwrap();
        assert!(matches!(e.downcast_ref::<DecodeError>(), Some(DecodeError::Unsupported)), "{e:#}");
    }
}

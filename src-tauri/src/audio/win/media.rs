//! Decodes a file's sound with Media Foundation's source reader (FR-18): every container and codec
//! Windows plays (MP4, M4A, MOV, MKV, WebM, MP3, WAV, FLAC, WMA, WMV and more) and the audio track
//! of a video. The reader decodes to float samples at the file's own rate; the DSP resamples.

use std::path::Path;

use windows::Win32::Media::MediaFoundation::{
    IMFSourceReader, MF_E_INVALIDSTREAMNUMBER, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND,
    MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_PD_DURATION, MF_SOURCE_READER_ALL_STREAMS, MF_SOURCE_READER_FIRST_AUDIO_STREAM,
    MF_SOURCE_READER_MEDIASOURCE, MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED, MF_SOURCE_READERF_ENDOFSTREAM,
    MF_SOURCE_READERF_ERROR, MF_VERSION, MFAudioFormat_Float, MFCreateMediaType, MFCreateSourceReaderFromURL,
    MFMediaType_Audio, MFSTARTUP_NOSOCKET, MFShutdown, MFStartup,
};
use windows::core::HSTRING;

use crate::audio::decode::{AudioDecoder, Block, DecodeError};

const AUDIO: u32 = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;

/// Media Foundation for as long as the decoder lives.
struct Started;

impl Drop for Started {
    fn drop(&mut self) {
        // SAFETY: balances the MFStartup that made this value.
        let _ = unsafe { MFShutdown() };
    }
}

pub struct MediaDecoder {
    // Dropped before `_mf`, which shuts Media Foundation down.
    reader: IMFSourceReader,
    rate: u32,
    channels: u16,
    duration_ms: Option<u64>,
    _mf: Started,
}

/// Whether Media Foundation itself turned the file down (its own error codes, `0xC00D….`: an
/// unknown container, a missing codec, a file that isn't what its name says), rather than the
/// file system (missing, locked).
fn unsupported(e: &windows::core::Error) -> bool {
    (e.code().0 as u32) >> 16 == 0xC00D
}

impl MediaDecoder {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let _ = wasapi::initialize_mta();
        // SAFETY: plain Media Foundation calls on this thread's COM apartment; every pointer passed
        // lives across the call.
        unsafe {
            MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET)?;
            let mf = Started;
            let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), None)
                .map_err(|e| if unsupported(&e) { DecodeError::Unsupported.into() } else { anyhow::Error::from(e) })?;
            // Only the sound: a video's pictures are never decoded.
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(AUDIO, true).map_err(|e| {
                if e.code() == MF_E_INVALIDSTREAMNUMBER { DecodeError::NoAudio.into() } else { anyhow::Error::from(e) }
            })?;
            let want = MFCreateMediaType()?;
            want.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            want.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)?;
            reader
                .SetCurrentMediaType(AUDIO, None, &want)
                .map_err(|e| if unsupported(&e) { DecodeError::Unsupported.into() } else { anyhow::Error::from(e) })?;
            let duration_ms = reader
                .GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
                .ok()
                .and_then(|v| u64::try_from(&v).ok())
                .map(|hns| hns / 10_000);
            let mut decoder = Self { reader, rate: 0, channels: 0, duration_ms, _mf: mf };
            decoder.read_format()?;
            Ok(decoder)
        }
    }

    fn read_format(&mut self) -> anyhow::Result<()> {
        // SAFETY: as in `open`.
        unsafe {
            let t = self.reader.GetCurrentMediaType(AUDIO)?;
            self.rate = t.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)?;
            self.channels = t.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)? as u16;
        }
        anyhow::ensure!(self.rate > 0 && self.channels > 0, "no audio format");
        Ok(())
    }
}

impl AudioDecoder for MediaDecoder {
    fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    fn read(&mut self) -> anyhow::Result<Option<Block>> {
        loop {
            let mut flags = 0u32;
            let mut sample = None;
            // SAFETY: the out pointers live across the call.
            unsafe { self.reader.ReadSample(AUDIO, 0, None, Some(&mut flags), None, Some(&mut sample))? };
            let flag = |f: windows::Win32::Media::MediaFoundation::MF_SOURCE_READER_FLAG| flags & f.0 as u32 != 0;
            anyhow::ensure!(!flag(MF_SOURCE_READERF_ERROR), "the file could not be read to the end");
            if flag(MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED) {
                self.read_format()?;
            }
            if let Some(sample) = sample {
                // SAFETY: the buffer is locked while its `len` bytes are copied.
                let samples = unsafe {
                    let buffer = sample.ConvertToContiguousBuffer()?;
                    let mut data = std::ptr::null_mut();
                    let mut len = 0u32;
                    buffer.Lock(&mut data, None, Some(&mut len))?;
                    let bytes = std::slice::from_raw_parts(data, len as usize);
                    let samples: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_ne_bytes(*b)).collect();
                    buffer.Unlock()?;
                    samples
                };
                if !samples.is_empty() {
                    return Ok(Some(Block { rate: self.rate, channels: self.channels, samples }));
                }
            }
            if flag(MF_SOURCE_READERF_ENDOFSTREAM) {
                return Ok(None);
            }
        }
    }
}

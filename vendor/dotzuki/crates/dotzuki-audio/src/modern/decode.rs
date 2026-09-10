//! Streaming file-audio decoding (WAV / OGG-Vorbis / FLAC / MP3) via
//! symphonia — the `modern-audio` feature's decoder.
//!
//! Keeps the crate's zero-I/O rule: the caller supplies a
//! [`MediaSource`](symphonia::core::io::MediaSource) (a `Cursor<Vec<u8>>`
//! over VFS bytes, a `File`, …), the decoder pulls packets on demand and
//! converts every source format to interleaved stereo `f32` at the source's
//! native rate. Nothing is ever loaded "fully decoded" into memory — a long
//! BGM streams packet by packet.

use std::io;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder as SDecoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Why decoding failed.
#[derive(Debug)]
pub enum DecodeError {
    /// The stream could not be opened as a supported format/codec.
    Unsupported(String),
    /// The container held no decodable audio track.
    NoAudioTrack,
    /// Decoding broke mid-stream.
    Decode(String),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Unsupported(m) => write!(f, "unsupported audio format: {m}"),
            DecodeError::NoAudioTrack => write!(f, "no decodable audio track in stream"),
            DecodeError::Decode(m) => write!(f, "audio decode error: {m}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Static facts about an opened stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    /// Sample rate of the decoded audio (Hz).
    pub sample_rate: u32,
    /// Channel count of the *source* (decoded output is always stereo).
    pub source_channels: u16,
    /// Total frame count (one frame = one sample per channel), when known.
    pub total_frames: Option<u64>,
}

impl StreamInfo {
    /// Approximate duration in seconds, when the total frame count is known.
    pub fn duration_secs(&self) -> Option<f64> {
        self.total_frames
            .map(|f| f as f64 / self.sample_rate as f64)
    }
}

/// A streaming decoder: fills callers with interleaved stereo `f32` frames.
pub trait SourceDecoder: Send {
    /// Write as many stereo frames as possible into `out` (interleaved
    /// L/R `f32`, `2 * frames` samples per `frames`). Returns the number of
    /// frames written; `0` means the stream is exhausted.
    fn next_frames(&mut self, out: &mut [f32]) -> usize;
    /// Static stream facts (rate, channel count, length).
    fn info(&self) -> StreamInfo;
    /// Rewind to the very beginning (used for looped playback).
    fn seek_to_start(&mut self) -> Result<(), DecodeError>;
}

/// Open a decoder over `source`. `ext_hint` (e.g. `"ogg"`, `"wav"`) helps the
/// container probe but is optional — content sniffing works on its own.
pub fn open(
    source: Box<dyn MediaSource>,
    ext_hint: Option<&str>,
) -> Result<Box<dyn SourceDecoder>, DecodeError> {
    let mss = MediaSourceStream::new(source, Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = ext_hint {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| DecodeError::Unsupported(format!("{e}")))?;

    let format = probed.format;
    // Pick the first non-null audio track (preferring the container's
    // default), decode from it, ignore any other tracks (e.g. video).
    let track_id = format
        .default_track()
        .map(|t| t.id)
        .or_else(|| {
            format
                .tracks()
                .iter()
                .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
                .map(|t| t.id)
        })
        .ok_or(DecodeError::NoAudioTrack)?;
    let params = format
        .tracks()
        .iter()
        .find(|t| t.id == track_id)
        .ok_or(DecodeError::NoAudioTrack)?
        .codec_params
        .clone();
    let sample_rate = params.sample_rate.unwrap_or(0);
    let src_channels = params.channels.map(|c| c.iter().count()).unwrap_or(0) as u16;
    let total_frames = params.n_frames;

    let decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions::default())
        .map_err(|e| DecodeError::Unsupported(format!("{e}")))?;

    Ok(Box::new(SymphoniaDecoder {
        format,
        decoder,
        track_id,
        sample_rate,
        src_channels,
        total_frames,
        pending: Vec::new(),
        pending_pos: 0,
        eof: false,
    }))
}

/// The symphonia-backed decoder implementation.
struct SymphoniaDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SDecoder>,
    track_id: u32,
    sample_rate: u32,
    src_channels: u16,
    total_frames: Option<u64>,
    /// Decoded-but-unconsumed interleaved stereo samples.
    pending: Vec<f32>,
    pending_pos: usize,
    eof: bool,
}

impl SourceDecoder for SymphoniaDecoder {
    fn next_frames(&mut self, out: &mut [f32]) -> usize {
        let mut written = 0usize;
        while written < out.len() / 2 {
            // 1. Drain whatever was decoded earlier.
            if self.pending_pos < self.pending.len() {
                let room = (out.len() - written * 2).min(self.pending.len() - self.pending_pos);
                out[written * 2..written * 2 + room]
                    .copy_from_slice(&self.pending[self.pending_pos..self.pending_pos + room]);
                self.pending_pos += room;
                written += room / 2;
                continue;
            }
            // 2. Decode the next packet into `pending`.
            if self.eof || !self.decode_next_packet() {
                self.eof = true;
                break;
            }
        }
        written
    }

    fn info(&self) -> StreamInfo {
        StreamInfo {
            sample_rate: self.sample_rate,
            source_channels: self.src_channels,
            total_frames: self.total_frames,
        }
    }

    fn seek_to_start(&mut self) -> Result<(), DecodeError> {
        use symphonia::core::formats::SeekMode;
        self.format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: 0u64.into(),
                    track_id: None,
                },
            )
            .map_err(|e| DecodeError::Decode(format!("seek: {e}")))?;
        self.pending.clear();
        self.pending_pos = 0;
        self.eof = false;
        Ok(())
    }
}

impl SymphoniaDecoder {
    /// Decode one packet into `pending` (stereo interleaved). Returns
    /// `false` on clean end-of-stream; malformed packets are skipped.
    fn decode_next_packet(&mut self) -> bool {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    return false
                }
                Err(SError::DecodeError(_)) => continue, // skip corrupt packet
                Err(_) => return false,
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let frames = decoded.frames() as usize;
                    if frames == 0 {
                        continue;
                    }
                    let spec = decoded.spec();
                    self.sample_rate = spec.rate;
                    self.src_channels = spec.channels.iter().count() as u16;

                    let mut buf = SampleBuffer::<f32>::new(frames as u64, *spec);
                    buf.copy_interleaved_ref(decoded);
                    let samples = buf.samples();

                    // Normalise to stereo: mono duplicates the channel,
                    // >2 drops down to the front pair.
                    self.pending.clear();
                    match self.src_channels {
                        1 => {
                            self.pending.reserve(samples.len() * 2);
                            for &s in samples {
                                self.pending.push(s);
                                self.pending.push(s);
                            }
                        }
                        2 => self.pending.extend_from_slice(samples),
                        n => {
                            self.pending.reserve(samples.len() / n as usize * 2);
                            for ch in samples.chunks(n as usize) {
                                self.pending.push(ch[0]);
                                self.pending.push(ch[1]);
                            }
                        }
                    }
                    self.pending_pos = 0;
                    return true;
                }
                Err(SError::DecodeError(_)) => continue, // skip corrupt packet
                Err(SError::ResetRequired) => {
                    // Format was seeked; reset the decoder to re-sync.
                    self.decoder.reset();
                    continue;
                }
                Err(_) => return false,
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    /// Build a minimal RIFF/WAVE file: 16-bit PCM mono 440 Hz sine,
    /// `seconds` long, at `rate` Hz.
    pub(crate) fn make_wav(rate: u32, seconds: u32) -> Vec<u8> {
        let samples = rate * seconds;
        let data_len = samples * 2;
        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 2).to_le_bytes()); // byte rate
        wav.extend_from_slice(&2u16.to_le_bytes()); // block align
        wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..samples {
            let v = (i as f32 * 2.0 * std::f32::consts::PI * 440.0 / rate as f32).sin();
            wav.write_all(&((v * 32000.0) as i16).to_le_bytes())
                .unwrap();
        }
        wav
    }

    #[test]
    fn decodes_wav_pcm16_mono_to_stereo() {
        let bytes = make_wav(8000, 1);
        let mut dec = open(Box::new(Cursor::new(bytes)), Some("wav")).unwrap();
        let info = dec.info();
        assert_eq!(info.sample_rate, 8000);
        assert_eq!(info.source_channels, 1);

        let mut out = vec![0.0f32; 8000 * 2]; // 1 s of stereo frames
        let frames = dec.next_frames(&mut out);
        assert_eq!(frames, 8000, "full second must decode");
        // A sine must be non-silent and within range.
        assert!(out.iter().any(|&s| s.abs() > 0.1));
        assert!(out.iter().all(|&s| s.abs() <= 1.0));
        // Mono duplicated to both channels.
        for pair in out.chunks_exact(2).take(100) {
            assert_eq!(pair[0], pair[1]);
        }
        // Exhausted afterwards.
        assert_eq!(dec.next_frames(&mut out), 0);
    }

    #[test]
    fn wav_seek_to_start_restarts() {
        let bytes = make_wav(8000, 1);
        let mut dec = open(Box::new(Cursor::new(bytes)), Some("wav")).unwrap();
        let mut out = vec![0.0f32; 8000 * 2];
        let n1 = dec.next_frames(&mut out);
        let first = out[..n1 * 2].to_vec();
        assert_eq!(dec.next_frames(&mut out), 0, "stream exhausted");
        dec.seek_to_start().unwrap();
        let mut again = vec![0.0f32; 8000 * 2];
        let n = dec.next_frames(&mut again);
        assert_eq!(n, 8000);
        assert_eq!(again[..n * 2], first, "looped audio must be identical");
    }

    #[test]
    fn rejects_garbage() {
        let err = open(Box::new(Cursor::new(b"this is not audio".to_vec())), None);
        assert!(err.is_err(), "garbage must be rejected, not panic");
    }

    #[test]
    fn ogg_vorbis_decodes() {
        // Pre-encoded fixture (1 s, 8 kHz 440 Hz sine, generated with
        // ffmpeg at fixture creation time) — proves the OGG container +
        // Vorbis codec path end to end.
        let bytes: &[u8] = include_bytes!("../../tests/fixtures/tone.ogg");
        let mut dec = open(Box::new(Cursor::new(bytes.to_vec())), Some("ogg")).unwrap();
        let info = dec.info();
        assert_eq!(info.sample_rate, 8000);
        let mut out = vec![0.0f32; 8000 * 2];
        let frames = dec.next_frames(&mut out);
        assert!(
            frames >= 7500,
            "~1 s of audio expected, got {frames} frames"
        );
        assert!(out.iter().any(|&s| s.abs() > 0.1), "tone must be audible");
        // Loop works on compressed sources too. Note: Vorbis is lossy — a
        // rewind lands on the same *signal* but not on sample-identical
        // frames (granule/pre-roll alignment), so assert statistically.
        let n2 = dec.next_frames(&mut out);
        dec.seek_to_start().unwrap();
        let mut again = vec![0.0f32; 8000 * 2];
        let n3 = dec.next_frames(&mut again);
        assert!(n3 >= n2, "seek restarts the stream ({n3} >= {n2} frames)");
        let peak = again[..n3 * 2].iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        assert!(
            peak > 0.3,
            "seeked stream plays the same tone (peak {peak})"
        );
    }
}

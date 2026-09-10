//! Device output glue: pull stereo samples from a [`SampleSource`] into a
//! real audio backend.
//!
//! Everything here is game-agnostic GB-APU plumbing shared by every
//! frontend: the emulated APU is advanced `CPU_CLOCK_HZ / sample_rate`
//! cycles per output sample, [`Apu::mix_sample`](crate::apu::Apu::mix_sample)
//! yields one stereo `i16` pair, and the pair is normalised to `[-1.0, 1.0]`
//! floats ([`render_apu_stereo`]). The game thread keeps advancing the
//! *sequencer* once per video frame; the backend's callback thread only
//! ticks the APU and mixes.
//!
//! Backends (both optional, so the default build stays dependency-light):
//!
//! - [`CpalOutput`] (feature `cpal`, native): opens the default output
//!   device at [`OUTPUT_SAMPLE_RATE`] and pulls samples on cpal's callback
//!   thread;
//! - [`WebAudioOutput`] (feature `web-audio`, `wasm32` only): wires a Web
//!   Audio `ScriptProcessorNode` to the destination and pulls samples from
//!   `onaudioprocess`.
//!
//! A source is anything implementing [`SampleSource`] — including any
//! `FnMut(&mut [f32], u32) + Send + 'static` closure, so a frontend typically
//! captures its shared `Arc<Mutex<AudioManager>>` and renders from its APU.

use crate::apu::Apu;
use crate::CPU_CLOCK_HZ;

#[cfg(feature = "cpal")]
mod cpal_output;
#[cfg(feature = "cpal")]
pub use cpal_output::CpalOutput;

#[cfg(all(target_arch = "wasm32", feature = "web-audio"))]
mod web_output;
#[cfg(all(target_arch = "wasm32", feature = "web-audio"))]
pub use web_output::WebAudioOutput;

/// Standard device output sample rate (the GB APU is resampled to it by
/// ticking `CPU_CLOCK_HZ / OUTPUT_SAMPLE_RATE` cycles per sample).
pub const OUTPUT_SAMPLE_RATE: u32 = crate::SAMPLE_RATE;

/// APU peak amplitude used to normalise `mix_sample`'s `i16` output to
/// `[-1.0, 1.0]`.
pub const MAX_AMPLITUDE: f32 = 480.0;

/// A pull-model stereo sample source.
///
/// `render` fills `out` with interleaved L/R `f32` samples at `sample_rate`
/// Hz, normalised to `[-1.0, 1.0]`. Called on the audio backend's callback
/// thread, so implementations must be `Send`.
pub trait SampleSource: Send + 'static {
    fn render(&mut self, out: &mut [f32], sample_rate: u32);
}

/// Any suitable closure is a sample source — the common case is a closure
/// capturing the shared `Arc<Mutex<…>>` that owns the APU.
impl<F> SampleSource for F
where
    F: FnMut(&mut [f32], u32) + Send + 'static,
{
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        self(out, sample_rate)
    }
}

/// Fill `out` (interleaved L/R) with stereo samples from `apu`, ticking
/// `CPU_CLOCK_HZ / sample_rate` cycles per sample and normalising by
/// [`MAX_AMPLITUDE`]. This is the exact sample path every backend uses, so
/// all frontends render byte-identical audio.
pub fn render_apu_stereo(apu: &mut Apu, out: &mut [f32], sample_rate: u32) {
    let cycles_per_sample = CPU_CLOCK_HZ / sample_rate;
    for frame in out.chunks_mut(2) {
        apu.tick_n(cycles_per_sample);
        let (left, right) = apu.mix_sample();
        frame[0] = left as f32 / MAX_AMPLITUDE;
        frame[1] = right as f32 / MAX_AMPLITUDE;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closure_is_a_sample_source() {
        let mut source = |out: &mut [f32], rate: u32| {
            assert_eq!(rate, OUTPUT_SAMPLE_RATE);
            out.fill(0.5);
        };
        let mut buf = vec![0.0; 8];
        SampleSource::render(&mut source, &mut buf, OUTPUT_SAMPLE_RATE);
        assert!(buf.iter().all(|&s| s == 0.5));
    }

    #[test]
    fn render_apu_stereo_fills_interleaved_frames() {
        let mut apu = Apu::new();
        let mut buf = vec![1.0; 16];
        render_apu_stereo(&mut apu, &mut buf, OUTPUT_SAMPLE_RATE);
        // A powered-off APU mixes silence, but every slot is written.
        assert!(buf.iter().all(|&s| s == 0.0));
    }
}

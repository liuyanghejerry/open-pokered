//! Native output via cpal (feature `cpal`): the default output device at
//! 44.1 kHz stereo, pulling samples from a [`SampleSource`] on cpal's
//! callback thread.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::{SampleSource, OUTPUT_SAMPLE_RATE};

/// A live cpal output stream pulling samples from a [`SampleSource`].
///
/// Dropping it stops the stream. Construction is infallible-by-`Option`:
/// `None` means no output device (CI/headless) or the stream could not be
/// built/started — the caller continues silent.
pub struct CpalOutput {
    /// The live stream, kept alive for the lifetime of playback; dropping it
    /// stops the stream.
    _stream: cpal::Stream,
}

impl CpalOutput {
    /// Open the default output device at [`OUTPUT_SAMPLE_RATE`] Hz stereo
    /// and start pulling samples from `source` on the callback thread.
    pub fn new<S: SampleSource>(mut source: S) -> Option<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(OUTPUT_SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    source.render(data, OUTPUT_SAMPLE_RATE);
                },
                |err| log::error!("audio stream error: {err}"),
                None,
            )
            .ok()?;
        stream.play().ok()?;

        Some(Self { _stream: stream })
    }
}

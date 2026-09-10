//! Browser output via the Web Audio API (feature `web-audio`, `wasm32`
//! only): a `ScriptProcessorNode` wired to the destination, pulling samples
//! from a [`SampleSource`] in `onaudioprocess`.

use wasm_bindgen::prelude::*;

use super::SampleSource;

/// A live Web Audio pipeline (`AudioContext` → `ScriptProcessorNode` →
/// destination) pulling samples from a [`SampleSource`].
///
/// The context starts at 44.1 kHz where the browser allows it; the actual
/// negotiated rate is reported via [`sample_rate`](Self::sample_rate) and
/// passed to the source on every render. Browsers may suspend the context
/// until a user gesture — call [`try_resume`](Self::try_resume) before play
/// commands.
pub struct WebAudioOutput {
    ctx: web_sys::AudioContext,
    _processor: web_sys::ScriptProcessorNode,
    /// Keeps the onaudioprocess closure alive.
    _closure: Closure<dyn FnMut(web_sys::AudioProcessingEvent)>,
    sample_rate: u32,
}

impl WebAudioOutput {
    /// Create the context and processor (buffer 2048 ≈ 46 ms at 44.1 kHz,
    /// 0 inputs, 2 outputs) and start pulling from `source`. `None` when the
    /// context or node cannot be created.
    pub fn new<S: SampleSource>(mut source: S) -> Option<Self> {
        let opts = web_sys::AudioContextOptions::new();
        opts.set_sample_rate(super::OUTPUT_SAMPLE_RATE as f32);
        let ctx = web_sys::AudioContext::new_with_context_options(&opts)
            .or_else(|_| web_sys::AudioContext::new())
            .ok()?;

        let sample_rate = ctx.sample_rate() as u32;

        let processor = ctx
            .create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(
                2048, 0, 2,
            )
            .ok()?;

        let closure = Closure::wrap(Box::new(
            move |event: web_sys::AudioProcessingEvent| {
                let output = match event.output_buffer() {
                    Ok(buf) => buf,
                    Err(_) => return,
                };
                let length = output.length() as usize;

                let mut interleaved = vec![0.0_f32; length * 2];
                source.render(&mut interleaved, sample_rate);

                let mut left_buf = vec![0.0_f32; length];
                let mut right_buf = vec![0.0_f32; length];
                for (i, frame) in interleaved.chunks_exact(2).enumerate() {
                    left_buf[i] = frame[0];
                    right_buf[i] = frame[1];
                }

                let _ = output.copy_to_channel(&left_buf, 0);
                let _ = output.copy_to_channel(&right_buf, 1);
            },
        )
            as Box<dyn FnMut(web_sys::AudioProcessingEvent)>);

        processor.set_onaudioprocess(Some(closure.as_ref().unchecked_ref()));

        // With 0 input channels the processor only needs a connection to the
        // destination for onaudioprocess to fire.
        processor.connect_with_audio_node(&ctx.destination()).ok()?;

        log::info!(
            "Web Audio initialized (sample_rate={}, buffer=2048)",
            sample_rate
        );

        Some(Self {
            ctx,
            _processor: processor,
            _closure: closure,
            sample_rate,
        })
    }

    /// The negotiated context sample rate (44.1 kHz where the browser
    /// honours the request).
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Resume the context if suspended (browsers gate audio behind a user
    /// gesture). Cheap to call before every play command.
    pub fn try_resume(&self) {
        if self.ctx.state() == web_sys::AudioContextState::Suspended {
            let _ = self.ctx.resume();
        }
    }
}

//! Persistent output-sample phase for the existing hosted APU clock.
//! This converts device sample demand, not source CPU/IRQ elapsed time.

use dotzuki_audio::{apu::Apu, output::MAX_AMPLITUDE, CPU_CLOCK_HZ};

/// One output stream's fixed sample rate and fractional CPU-cycle remainder.
/// No audio channel or sequencer state is owned here.
#[derive(Default)]
pub(super) struct PcmRenderer {
    sample_rate: Option<u32>,
    remainder: u32,
}

impl PcmRenderer {
    pub(super) fn render(&mut self, apu: &mut Apu, out: &mut [f32], rate: u32) {
        // Backends negotiate one positive rate for the lifetime of a stream.
        // Check the complete request before advancing any APU/sample state.
        assert!(rate > 0, "PCM sample rate must be positive");
        assert_eq!(out.len() % 2, 0, "PCM output must contain stereo pairs");
        if let Some(previous) = self.sample_rate {
            assert_eq!(rate, previous, "PCM stream sample rate changed");
        }
        if out.is_empty() {
            return;
        }
        self.sample_rate = Some(rate);
        for frame in out.chunks_exact_mut(2) {
            let phase = u64::from(self.remainder) + u64::from(CPU_CLOCK_HZ);
            apu.tick_n((phase / u64::from(rate)) as u32);
            self.remainder = (phase % u64::from(rate)) as u32;
            let (left, right) = apu.mix_sample();
            frame[0] = left as f32 / MAX_AMPLITUDE;
            frame[1] = right as f32 / MAX_AMPLITUDE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::Vec;

    fn sounding_apu() -> Apu {
        let mut apu = Apu::new();
        for (port, byte) in [
            (0xff26, 0x80),
            (0xff24, 0x77),
            (0xff25, 0x11),
            (0xff11, 0x80),
            (0xff12, 0xf3),
            (0xff13, 0x57),
            (0xff14, 0x87),
        ] {
            apu.write_register(port, byte);
        }
        apu
    }

    #[test]
    fn one_output_second_advances_exactly_one_cpu_second_at_both_device_rates() {
        for rate in [44_100, 48_000] {
            let mut actual = sounding_apu();
            let mut expected = actual.clone();
            expected.tick_n(CPU_CLOCK_HZ);
            let mut renderer = PcmRenderer::default();
            renderer.render(&mut actual, &mut vec![0.0; rate as usize * 2], rate);
            assert_eq!(actual.playback_state(), expected.playback_state());
            assert_eq!(renderer.remainder, 0);
            // Demonstrate that the former per-sample floor fails this clock
            // requirement; this is not only a comparison between new paths.
            let mut truncated = sounding_apu();
            truncated.tick_n((CPU_CLOCK_HZ / rate) * rate);
            assert_ne!(actual.playback_state(), truncated.playback_state());
        }
    }

    #[test]
    fn irregular_callback_partitions_preserve_every_sample_and_live_apu_state() {
        for rate in [44_100, 48_000] {
            let mut whole_apu = sounding_apu();
            let mut split_apu = whole_apu.clone();
            let mut whole = PcmRenderer::default();
            let mut split = PcmRenderer::default();
            let mut expected = vec![0.0; 4097 * 2];
            whole.render(&mut whole_apu, &mut expected, rate);
            let mut actual = Vec::new();
            for frames in [1, 7, 32, 0, 511, 735, 2, 2048, 761] {
                let mut output = vec![0.0; frames * 2];
                split.render(&mut split_apu, &mut output, rate);
                actual.extend(output);
            }
            assert_eq!(actual, expected);
            assert_eq!(split_apu.playback_state(), whole_apu.playback_state());
            assert_eq!(split.remainder, whole.remainder);
        }
    }

    #[test]
    fn samples_observe_register_changes_between_callbacks_without_resetting_phase() {
        let mut actual = sounding_apu();
        let mut expected = actual.clone();
        let mut renderer = PcmRenderer::default();
        let mut output = [0.0; 2];
        renderer.render(&mut actual, &mut output, 44_100);
        expected.tick_n(CPU_CLOCK_HZ / 44_100);
        actual.write_register(0xff13, 0x83);
        expected.write_register(0xff13, 0x83);
        for n in 2..=17u64 {
            let before = ((n - 1) * u64::from(CPU_CLOCK_HZ)) / 44_100;
            let after = (n * u64::from(CPU_CLOCK_HZ)) / 44_100;
            expected.tick_n((after - before) as u32);
            let (left, right) = expected.mix_sample();
            renderer.render(&mut actual, &mut output, 44_100);
            assert_eq!(
                output,
                [left as f32 / MAX_AMPLITUDE, right as f32 / MAX_AMPLITUDE]
            );
            assert_eq!(actual.playback_state(), expected.playback_state());
        }
    }

    #[test]
    fn embedding_output_keeps_fractional_time_between_public_render_calls() {
        let audio = super::super::AudioOutput::new_pcm();
        let mut expected = sounding_apu();
        audio.manager.lock().unwrap().apu = expected.clone();
        let mut frames = 0u64;
        let mut elapsed = 0;
        for count in [1, 735, 2, 17, 511, 1024] {
            frames += count;
            let next = (frames * u64::from(CPU_CLOCK_HZ)) / 44_100;
            expected.tick_n((next - elapsed) as u32);
            elapsed = next;
            audio.render_pcm(&mut vec![0.0; count as usize * 2]);
            assert_eq!(
                audio.manager.lock().unwrap().apu.playback_state(),
                expected.playback_state()
            );
        }
    }
}

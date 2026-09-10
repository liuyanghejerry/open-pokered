//! Mixing bus for the `modern-audio` subsystem.
//!
//! A [`Mixer`] renders any number of streaming voices into interleaved
//! stereo `f32`. Voices are routed to one of two buses — [`Bus::Bgm`] or
//! [`Bus::Sfx`] — each with its own volume, fade and DSP chain; a master
//! volume sits on the final output. Sources run at their native sample
//! rate and are resampled with linear interpolation to the mixer rate.
//!
//! Everything is sample-driven and allocation-light: `render_into` works on
//! a reusable block buffer, voices drop their finished state automatically,
//! and a voice cap keeps runaway one-shots bounded.

use crate::modern::decode::SourceDecoder;
use crate::modern::dsp::DspChain;

/// Output block size (frames) — bounds per-block buffer reuse.
const CHUNK_FRAMES: usize = 256;
/// How many source frames a voice decodes per fill call.
const DECODE_CHUNK_FRAMES: usize = 2048;
/// Cap on frames of decoded source kept buffered per voice before the
/// consumed prefix is dropped (~256 KiB per voice max at f32 stereo).
const CACHE_CAP_FRAMES: usize = 65_536;

/// A mixing bus (routing + volume group).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bus {
    /// Background-music group (looping tracks).
    Bgm,
    /// Sound-effect group (one-shots).
    Sfx,
}

impl Bus {
    fn index(self) -> usize {
        match self {
            Bus::Bgm => 0,
            Bus::Sfx => 1,
        }
    }

    const ALL: [Bus; 2] = [Bus::Bgm, Bus::Sfx];
}

/// Options for starting a voice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayOptions {
    /// Voice gain, `0.0..=1.0`. Default `1.0`.
    pub volume: f32,
    /// Stereo pan, `-1.0` (hard left) .. `1.0` (hard right). Default `0.0`.
    pub pan: f32,
    /// Fade-in duration in seconds (`None` = instant). Default `None`.
    pub fade_in: Option<f32>,
    /// Fade-out duration in seconds, used when the voice is stopped or a
    /// looping BGM is replaced (`None` = instant cut). Default `None`.
    pub fade_out: Option<f32>,
    /// Loop the source forever (BGM). Default `false`.
    pub loop_audio: bool,
}

impl Default for PlayOptions {
    fn default() -> Self {
        Self {
            volume: 1.0,
            pan: 0.0,
            fade_in: None,
            fade_out: None,
            loop_audio: false,
        }
    }
}

/// A linear gain ramp, advanced per output frame.
#[derive(Debug, Clone, Copy)]
struct Fade {
    from: f32,
    target: f32,
    total_frames: u32,
    remaining: u32,
}

impl Fade {
    fn new(from: f32, target: f32, seconds: f32, sample_rate: u32) -> Self {
        let total = (seconds.max(0.0) * sample_rate as f32).round() as u32;
        Self {
            from,
            target,
            total_frames: total.max(1),
            remaining: total.max(1),
        }
    }

    /// Current interpolated gain.
    fn gain(&self) -> f32 {
        let t = 1.0 - self.remaining as f32 / self.total_frames as f32;
        self.from + (self.target - self.from) * t
    }

    /// Advance one frame; returns `true` when the ramp is complete.
    fn step(&mut self) -> bool {
        if self.remaining > 0 {
            self.remaining -= 1;
        }
        self.remaining == 0
    }
}

/// A per-bus state: volume, optional fade, DSP chain.
#[derive(Debug, Clone)]
pub struct BusState {
    pub volume: f32,
    fade: Option<Fade>,
    pub dsp: DspChain,
}

impl Default for BusState {
    fn default() -> Self {
        Self {
            volume: 1.0,
            fade: None,
            dsp: DspChain::default(),
        }
    }
}

/// One playing voice: a decoder source plus playback state.
struct Voice {
    id: u64,
    bus: Bus,
    source: Box<dyn SourceDecoder>,
    volume: f32,
    pan: f32,
    loop_audio: bool,
    /// Source frames consumed per output frame (`src_rate / out_rate`).
    step: f64,
    /// Fractional source position (in source frames).
    frac: f64,
    /// Decoded source frames; `cache[0]` corresponds to source frame `base`.
    cache: Vec<f32>,
    base: usize,
    eof: bool,
    finished: bool,
    fade_in: Option<Fade>,
    fade_out: Option<Fade>,
    /// Length of one loop in source frames (recorded at the first EOF;
    /// 0 = unknown/not looping).
    loop_len: f64,
}

impl Voice {
    fn new(
        id: u64,
        bus: Bus,
        source: Box<dyn SourceDecoder>,
        opts: &PlayOptions,
        sample_rate: u32,
    ) -> Self {
        let src_rate = source.info().sample_rate.max(1) as f64;
        Self {
            id,
            bus,
            source,
            volume: opts.volume.clamp(0.0, 2.0),
            pan: opts.pan.clamp(-1.0, 1.0),
            loop_audio: opts.loop_audio,
            step: src_rate / sample_rate as f64,
            frac: 0.0,
            cache: Vec::new(),
            base: 0,
            eof: false,
            finished: false,
            fade_in: opts.fade_in.map(|s| Fade::new(0.0, 1.0, s, sample_rate)),
            fade_out: None,
            loop_len: 0.0,
        }
    }

    /// Make sure source frame `f` (and its neighbour) are decoded.
    fn ensure_frames(&mut self, f: usize) {
        // Drop a long-consumed prefix to bound memory.
        if f > self.base + CACHE_CAP_FRAMES {
            let drop_frames = f - CACHE_CAP_FRAMES;
            let drop = drop_frames * 2;
            self.cache.drain(..drop.min(self.cache.len()));
            self.base = drop_frames;
        }
        let mut buf = vec![0.0f32; DECODE_CHUNK_FRAMES * 2];
        while self.base + self.cache.len() / 2 < f + 2 {
            if self.eof {
                break;
            }
            let n = self.source.next_frames(&mut buf);
            if n == 0 {
                if self.loop_audio && self.loop_len == 0.0 {
                    // First pass completed: record the loop length (the
                    // exact source-frame position, independent of cache
                    // eviction). The rewind itself happens in
                    // `render_frame`, which folds `frac` back by this.
                    self.loop_len = self.frac;
                }
                self.eof = true;
                break;
            }
            self.cache.extend_from_slice(&buf[..n * 2]);
        }
    }

    /// Linear-interpolated source frame at `frac`.
    fn frame_at(&mut self, f: usize, t: f32) -> (f32, f32) {
        let have = self.cache.len() / 2;
        if have == 0 {
            return (0.0, 0.0);
        }
        let i0 = f.min(have - 1);
        let i1 = (f + 1).min(have - 1);
        let l = self.cache[i0 * 2] * (1.0 - t) + self.cache[i1 * 2] * t;
        let r = self.cache[i0 * 2 + 1] * (1.0 - t) + self.cache[i1 * 2 + 1] * t;
        (l, r)
    }

    /// Render one output frame; sets `finished` at end of stream.
    fn render_frame(&mut self) -> (f32, f32) {
        if self.finished {
            return (0.0, 0.0);
        }
        // Looping: fold the source position back into the current pass.
        if self.eof && self.loop_audio && self.loop_len > 0.0 && self.frac >= self.loop_len {
            self.frac -= self.loop_len;
            if self.source.seek_to_start().is_ok() {
                self.eof = false;
                self.cache.clear();
                self.base = 0;
            } else {
                self.finished = true;
            }
        }
        let f = self.frac.floor() as usize;
        self.ensure_frames(f);
        let (l, r) = self.frame_at(f, (self.frac - f as f64) as f32);
        self.frac += self.step;
        // EOF tolerance: float accumulation may leave `frac` a hair short
        // of the true end, so finish once we're within one frame of it.
        if self.eof
            && !self.loop_audio
            && self.frac.floor() as usize + 1 >= self.cache.len() / 2 + self.base
        {
            self.finished = true;
        }

        let mut gain = self.volume;
        if let Some(fi) = &mut self.fade_in {
            gain *= fi.gain();
            if fi.step() {
                self.fade_in = None;
            }
        }
        if let Some(fo) = &mut self.fade_out {
            gain *= fo.gain();
            if fo.step() {
                self.finished = true;
            }
        }

        // Equal-power pan.
        let (l, r) = if self.pan.abs() > f32::EPSILON {
            let theta = (self.pan + 1.0) * std::f32::consts::FRAC_PI_4;
            (l * theta.cos(), r * theta.sin())
        } else {
            (l, r)
        };
        (l * gain, r * gain)
    }

    /// Begin (or replace) a fade-out towards zero; completes → remove.
    fn start_fade_out(&mut self, seconds: f32, sample_rate: u32) {
        let from = self.fade_out.as_ref().map(|f| f.gain()).unwrap_or(1.0);
        self.fade_out = Some(Fade::new(
            from * self.volume.max(0.0),
            0.0,
            seconds,
            sample_rate,
        ));
    }
}

/// The mixer: voices → buses → master → output.
pub struct Mixer {
    sample_rate: u32,
    master_volume: f32,
    buses: [BusState; 2],
    voices: Vec<Voice>,
    next_id: u64,
    /// Reusable per-bus accumulation buffers (CHUNK_FRAMES * 2 samples).
    bus_buf: [Vec<f32>; 2],
    /// Cap on simultaneous SFX voices (oldest is dropped beyond it).
    max_sfx_voices: usize,
}

impl Mixer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            master_volume: 1.0,
            buses: [BusState::default(), BusState::default()],
            voices: Vec::new(),
            next_id: 1,
            bus_buf: [vec![0.0; CHUNK_FRAMES * 2], vec![0.0; CHUNK_FRAMES * 2]],
            max_sfx_voices: 16,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    // ── Levels ──

    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 2.0);
    }

    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    pub fn set_bus_volume(&mut self, bus: Bus, volume: f32) {
        self.buses[bus.index()].volume = volume.clamp(0.0, 2.0);
    }

    pub fn bus_volume(&self, bus: Bus) -> f32 {
        self.buses[bus.index()].volume
    }

    /// Ramp a bus's volume towards `target` over `seconds`.
    pub fn fade_bus(&mut self, bus: Bus, target: f32, seconds: f32) {
        let bs = &mut self.buses[bus.index()];
        let from = bs.fade.as_ref().map(|f| f.gain()).unwrap_or(bs.volume);
        bs.fade = Some(Fade::new(
            from,
            target.clamp(0.0, 2.0),
            seconds,
            self.sample_rate,
        ));
    }

    /// The DSP chain attached to a bus (lowpass/reverb).
    pub fn bus_dsp_mut(&mut self, bus: Bus) -> &mut DspChain {
        &mut self.buses[bus.index()].dsp
    }

    // ── Voices ──

    /// Start a voice on `bus` and return its id.
    pub fn play(&mut self, source: Box<dyn SourceDecoder>, bus: Bus, opts: &PlayOptions) -> u64 {
        if bus == Bus::Sfx {
            // Bound one-shots: drop the oldest SFX voice beyond the cap.
            let sfx_ids: Vec<u64> = self
                .voices
                .iter()
                .filter(|v| v.bus == Bus::Sfx)
                .map(|v| v.id)
                .collect();
            if sfx_ids.len() >= self.max_sfx_voices {
                if let Some(oldest) = sfx_ids.first() {
                    self.voices.retain(|v| v.id != *oldest);
                }
            }
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.voices
            .push(Voice::new(id, bus, source, opts, self.sample_rate));
        id
    }

    /// Stop a voice: immediately, or with a fade-out when `fade_seconds`
    /// is given. Unknown ids are a no-op.
    pub fn stop_voice(&mut self, id: u64, fade_seconds: Option<f32>) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
            match fade_seconds {
                Some(s) if s > 0.0 => v.start_fade_out(s, self.sample_rate),
                _ => v.finished = true,
            }
        }
    }

    /// Whether a voice with this id is still playing.
    pub fn voice_playing(&self, id: u64) -> bool {
        self.voices.iter().any(|v| v.id == id && !v.finished)
    }

    /// Number of currently active voices.
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| !v.finished).count()
    }

    /// Stop every voice (all buses), optionally fading over `seconds`.
    pub fn stop_all(&mut self, fade_seconds: Option<f32>) {
        for v in &mut self.voices {
            match fade_seconds {
                Some(s) if s > 0.0 => v.start_fade_out(s, self.sample_rate),
                _ => v.finished = true,
            }
        }
    }

    // ── Rendering ──

    /// Render `out.len() / 2` stereo frames into `out` (interleaved L/R,
    /// `f32`, `[-1.0, 1.0]`). Finished voices are dropped.
    pub fn render_into(&mut self, out: &mut [f32]) {
        let mut frame = 0;
        while frame < out.len() / 2 {
            let n = (out.len() / 2 - frame).min(CHUNK_FRAMES);
            self.render_block(&mut out[frame * 2..frame * 2 + n * 2], n);
            frame += n;
        }
        self.voices.retain(|v| !v.finished);
    }

    /// Render `n` frames of this block.
    fn render_block(&mut self, out: &mut [f32], n: usize) {
        for buf in self.bus_buf.iter_mut() {
            buf[..n * 2].fill(0.0);
        }
        // 1. Voices → bus buffers.
        for v in &mut self.voices {
            let idx = v.bus.index();
            for i in 0..n {
                let (l, r) = v.render_frame();
                let buf = &mut self.bus_buf[idx];
                buf[i * 2] += l;
                buf[i * 2 + 1] += r;
            }
        }
        // 2. Buses: volume/fade + DSP → out; master applied at the end.
        for bus in Bus::ALL {
            let idx = bus.index();
            let (vol, mut fade, dsp) = {
                let bs = &mut self.buses[idx];
                (bs.volume, bs.fade.as_mut(), &mut bs.dsp)
            };
            for i in 0..n {
                let mut l = self.bus_buf[idx][i * 2];
                let mut r = self.bus_buf[idx][i * 2 + 1];
                l *= vol;
                r *= vol;
                if let Some(fd) = fade.as_deref_mut() {
                    let g = fd.gain();
                    l *= g;
                    r *= g;
                    fd.step(); // per-frame ramp
                }
                let (l, r) = dsp.process_frame(l, r);
                out[i * 2] += l;
                out[i * 2 + 1] += r;
            }
            // Ramp done: pin the bus to the target level.
            if let Some(fd) = fade {
                if fd.remaining == 0 {
                    let target = fd.target;
                    self.buses[idx].fade = None;
                    self.buses[idx].volume = target;
                }
            }
        }
        // 3. Master.
        let m = self.master_volume;
        if m != 1.0 {
            for s in out.iter_mut() {
                *s *= m;
            }
        }
        // Soft-clip to keep pathological sums in range.
        for s in out.iter_mut() {
            if *s > 1.0 {
                *s = 1.0;
            } else if *s < -1.0 {
                *s = -1.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modern::decode::open;
    use std::io::Cursor;

    /// A synthetic decoder source: N frames of a constant stereo value,
    /// then EOF. Lets tests drive the mixer deterministically.
    struct TestSource {
        remaining: usize,
        value: f32,
    }

    impl SourceDecoder for TestSource {
        fn next_frames(&mut self, out: &mut [f32]) -> usize {
            let n = (out.len() / 2).min(self.remaining);
            for i in 0..n {
                out[i * 2] = self.value;
                out[i * 2 + 1] = self.value;
            }
            self.remaining -= n;
            n
        }
        fn info(&self) -> crate::modern::decode::StreamInfo {
            crate::modern::decode::StreamInfo {
                sample_rate: 44_100,
                source_channels: 2,
                total_frames: Some(0),
            }
        }
        fn seek_to_start(&mut self) -> Result<(), crate::modern::decode::DecodeError> {
            Ok(())
        }
    }

    #[test]
    fn renders_voice_at_volume() {
        let mut mx = Mixer::new(44_100);
        let id = mx.play(
            Box::new(TestSource {
                remaining: 100,
                value: 0.5,
            }),
            Bus::Bgm,
            &PlayOptions::default(),
        );
        assert!(mx.voice_playing(id));
        let mut out = vec![0.0f32; 100 * 2];
        mx.render_into(&mut out);
        assert!(out.iter().all(|&s| (s - 0.5).abs() < 1e-4));
        assert!(!mx.voice_playing(id), "voice finished at EOF");
        assert_eq!(mx.active_voices(), 0);
    }

    #[test]
    fn pan_hard_left_right() {
        let mut mx = Mixer::new(44_100);
        mx.play(
            Box::new(TestSource {
                remaining: 10,
                value: 1.0,
            }),
            Bus::Sfx,
            &PlayOptions {
                pan: -1.0,
                ..PlayOptions::default()
            },
        );
        let mut out = vec![0.0f32; 10 * 2];
        mx.render_into(&mut out);
        assert!(out[0] > 0.9 && out[1] < 0.1, "hard left: {:?}", &out[..2]);

        let mut mx = Mixer::new(44_100);
        mx.play(
            Box::new(TestSource {
                remaining: 10,
                value: 1.0,
            }),
            Bus::Sfx,
            &PlayOptions {
                pan: 1.0,
                ..PlayOptions::default()
            },
        );
        let mut out = vec![0.0f32; 10 * 2];
        mx.render_into(&mut out);
        assert!(out[1] > 0.9 && out[0] < 0.1, "hard right: {:?}", &out[..2]);
    }

    #[test]
    fn bus_volume_and_fade_apply() {
        let mut mx = Mixer::new(44_100);
        mx.set_bus_volume(Bus::Bgm, 0.5);
        mx.play(
            Box::new(TestSource {
                remaining: 100,
                value: 0.4,
            }),
            Bus::Bgm,
            &PlayOptions::default(),
        );
        let mut out = vec![0.0f32; 100 * 2];
        mx.render_into(&mut out);
        assert!(out.iter().all(|&s| (s - 0.2).abs() < 1e-4), "0.4 * 0.5");

        // A fade from the bus's current volume down to 0 over 1 s (44100
        // frames) must complete and pin the bus at 0.
        let mut mx = Mixer::new(44_100);
        mx.set_bus_volume(Bus::Bgm, 1.0);
        mx.fade_bus(Bus::Bgm, 0.0, 1.0);
        mx.play(
            Box::new(TestSource {
                remaining: 44_100 * 2,
                value: 0.5,
            }),
            Bus::Bgm,
            &PlayOptions::default(),
        );
        let mut out = vec![0.0f32; 44_100 * 2];
        mx.render_into(&mut out);
        assert!(out[0] > 0.4, "fade starts near full volume");
        assert!(
            out[out.len() - 2].abs() < 1e-4,
            "fade ends silent: {}",
            out[out.len() - 2]
        );
        assert_eq!(mx.bus_volume(Bus::Bgm), 0.0, "bus pinned at target");
    }

    #[test]
    fn voice_fade_in_ramps_up() {
        let mut mx = Mixer::new(44_100);
        mx.play(
            Box::new(TestSource {
                remaining: 44_100,
                value: 1.0,
            }),
            Bus::Bgm,
            &PlayOptions {
                fade_in: Some(0.5),
                ..PlayOptions::default()
            },
        );
        let mut out = vec![0.0f32; 44_100 * 2];
        mx.render_into(&mut out);
        assert!(out[0].abs() < 0.02, "fade-in starts near 0: {}", out[0]);
        assert!(
            out[out.len() - 2] > 0.98,
            "fade-in ends near 1: {}",
            out[out.len() - 2]
        );
    }

    #[test]
    fn stop_with_fade_out_then_removal() {
        let mut mx = Mixer::new(44_100);
        let id = mx.play(
            Box::new(TestSource {
                remaining: 44_100 * 2,
                value: 1.0,
            }),
            Bus::Bgm,
            &PlayOptions {
                fade_out: Some(0.25),
                ..PlayOptions::default()
            },
        );
        mx.stop_voice(id, Some(0.25));
        let mut out = vec![0.0f32; 44_100];
        mx.render_into(&mut out);
        assert!(out[0] > 0.5, "fade-out starts loud: {}", out[0]);
        assert!(out[out.len() - 2].abs() < 1e-3, "ends silent");
        assert_eq!(mx.active_voices(), 0, "voice removed after fade-out");
    }

    #[test]
    fn sfx_cap_drops_oldest() {
        let mut mx = Mixer::new(44_100);
        mx.max_sfx_voices = 2;
        let a = mx.play(
            Box::new(TestSource {
                remaining: 100,
                value: 1.0,
            }),
            Bus::Sfx,
            &PlayOptions::default(),
        );
        let b = mx.play(
            Box::new(TestSource {
                remaining: 100,
                value: 1.0,
            }),
            Bus::Sfx,
            &PlayOptions::default(),
        );
        let c = mx.play(
            Box::new(TestSource {
                remaining: 100,
                value: 1.0,
            }),
            Bus::Sfx,
            &PlayOptions::default(),
        );
        assert!(!mx.voice_playing(a), "oldest SFX dropped");
        assert!(mx.voice_playing(b) && mx.voice_playing(c));
    }

    #[test]
    fn loops_at_eof_when_requested() {
        // Real WAV decode + loop: 1 s source, render 2 s → the second
        // second must repeat the first.
        let wav = crate::modern::decode::tests::make_wav(8000, 1);
        let mut mx = Mixer::new(8000);
        mx.play(
            open(Box::new(Cursor::new(wav)), Some("wav")).unwrap(),
            Bus::Bgm,
            &PlayOptions {
                loop_audio: true,
                ..PlayOptions::default()
            },
        );
        let mut out = vec![0.0f32; 8000 * 2];
        mx.render_into(&mut out);
        assert!(mx.voice_playing(1), "looped voice still active");
        let first = &out[..4000];
        let second = &out[8000..12000];
        // Loop rewind keeps the phase to within a hair of a sample, so the
        // two passes differ by < 0.001 per sample (exact equality would be
        // float-luck, not correctness).
        for (a, b) in first.iter().zip(second) {
            assert!(
                (a - b).abs() < 0.001,
                "loop repeats: {a} vs {b} (first pass vs second)"
            );
        }
    }

    #[test]
    fn resamples_8k_source_to_44k() {
        let wav = crate::modern::decode::tests::make_wav(8000, 1);
        let mut mx = Mixer::new(44_100);
        mx.play(
            open(Box::new(Cursor::new(wav)), Some("wav")).unwrap(),
            Bus::Bgm,
            &PlayOptions::default(),
        );
        let mut out = vec![0.0f32; 44_100 * 2];
        mx.render_into(&mut out);
        // One second at 44.1k from a 1 s 8k source; EOF at the end.
        assert!(out[0..44_000].iter().any(|&s| s.abs() > 0.1));
        assert!(!mx.voice_playing(1), "source exhausted after resample");
    }

    #[test]
    fn two_voices_sum_on_same_bus() {
        let mut mx = Mixer::new(44_100);
        mx.play(
            Box::new(TestSource {
                remaining: 100,
                value: 0.25,
            }),
            Bus::Bgm,
            &PlayOptions::default(),
        );
        mx.play(
            Box::new(TestSource {
                remaining: 100,
                value: 0.25,
            }),
            Bus::Bgm,
            &PlayOptions::default(),
        );
        let mut out = vec![0.0f32; 100 * 2];
        mx.render_into(&mut out);
        assert!(out.iter().all(|&s| (s - 0.5).abs() < 1e-4));
    }
}

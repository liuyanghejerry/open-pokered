//! DSP effects for the modern mixer — `modern-audio` feature.
//!
//! Two small, allocation-free-after-setup processors, applied per bus:
//!
//! - [`Lowpass`]: one-pole IIR, good enough for muffled/underwater-ish
//!   filtering and bus "cutoff" tricks.
//! - [`Reverb`]: a compact Schroeder reverb (3 combs + 2 allpasses on a
//!   summed mono signal, fed back into stereo), the classic cheap room.
//!
//! [`DspChain`] holds an optional instance of each and processes a stereo
//! frame in one call.

/// One-pole lowpass filter. `y[n] = y[n-1] + a·(x[n] − y[n-1])` with
/// `a = 2π·fc/fs / (1 + 2π·fc/fs)` (RC approximation of the −3 dB point).

#[derive(Debug, Clone)]
pub struct Lowpass {
    cutoff_hz: f32,
    coeff: f32,
    /// Per-channel running state.
    state: [f32; 2],
}

impl Lowpass {
    pub fn new(cutoff_hz: f32, sample_rate: u32) -> Self {
        let mut lp = Self {
            cutoff_hz: 0.0,
            coeff: 0.0,
            state: [0.0; 2],
        };
        lp.set_cutoff(cutoff_hz, sample_rate);
        lp
    }

    pub fn set_cutoff(&mut self, cutoff_hz: f32, sample_rate: u32) {
        let cutoff = cutoff_hz.clamp(20.0, sample_rate as f32 * 0.45);
        let w = 2.0 * std::f32::consts::PI * cutoff / sample_rate as f32;
        self.coeff = w / (1.0 + w);
        self.cutoff_hz = cutoff;
    }

    pub fn cutoff_hz(&self) -> f32 {
        self.cutoff_hz
    }

    pub fn reset(&mut self) {
        self.state = [0.0; 2];
    }

    /// Process one stereo frame.
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let a = self.coeff;
        self.state[0] += a * (l - self.state[0]);
        self.state[1] += a * (r - self.state[1]);
        (self.state[0], self.state[1])
    }
}

/// Comb filter with lowpass damping in the feedback loop.
#[derive(Debug, Clone)]
struct Comb {
    buf: Vec<f32>,
    idx: usize,
    feedback: f32,
    damping: f32,
    filter_state: f32,
}

impl Comb {
    fn new(delay_frames: usize, feedback: f32, damping: f32) -> Self {
        Self {
            buf: vec![0.0; delay_frames.max(1)],
            idx: 0,
            feedback,
            damping,
            filter_state: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buf[self.idx];
        let filtered = delayed * (1.0 - self.damping) + self.filter_state * self.damping;
        self.filter_state = filtered;
        self.buf[self.idx] = input + filtered * self.feedback;
        self.idx = (self.idx + 1) % self.buf.len();
        delayed
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
        self.idx = 0;
        self.filter_state = 0.0;
    }
}

/// Allpass filter (diffuses the comb output).
#[derive(Debug, Clone)]
struct Allpass {
    buf: Vec<f32>,
    idx: usize,
    feedback: f32,
}

impl Allpass {
    fn new(delay_frames: usize, feedback: f32) -> Self {
        Self {
            buf: vec![0.0; delay_frames.max(1)],
            idx: 0,
            feedback,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buf[self.idx];
        let out = -input + delayed;
        self.buf[self.idx] = input + delayed * self.feedback;
        self.idx = (self.idx + 1) % self.buf.len();
        out
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
        self.idx = 0;
    }
}

/// Room-size parameter for [`Reverb`] (0.0 = small, 1.0 = large).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomSize(pub f32);

impl Default for RoomSize {
    fn default() -> Self {
        RoomSize(0.4)
    }
}

/// Compact Schroeder reverb: 3 combs + 2 allpasses over a summed mono
/// signal, mixed back into stereo (dry + wet). Delays are scaled from the
/// classic Freeverb values to the actual sample rate.
#[derive(Debug, Clone)]
pub struct Reverb {
    combs: [Comb; 3],
    allpasses: [Allpass; 2],
    wet: f32,
    dry: f32,
}

impl Reverb {
    /// Delay lengths (frames) at 44.1 kHz — classic Freeverb comb values.
    const COMB_DELAYS_44K: [usize; 3] = [1116, 1188, 1277];
    /// Allpass delays (frames) at 44.1 kHz.
    const ALLPASS_DELAYS_44K: [usize; 2] = [556, 441];

    pub fn new(sample_rate: u32, room: RoomSize) -> Self {
        let scale = sample_rate as f32 / 44_100.0;
        let room = room.0.clamp(0.0, 1.0);
        // Freeverb: feedback = room_size * 0.28 + 0.7.
        let feedback = room * 0.28 + 0.7;
        let damping = 0.4; // fixed, gentle damping
        let combs = [
            Comb::new(
                (Self::COMB_DELAYS_44K[0] as f32 * scale) as usize,
                feedback,
                damping,
            ),
            Comb::new(
                (Self::COMB_DELAYS_44K[1] as f32 * scale) as usize,
                feedback,
                damping,
            ),
            Comb::new(
                (Self::COMB_DELAYS_44K[2] as f32 * scale) as usize,
                feedback,
                damping,
            ),
        ];
        let allpasses = [
            Allpass::new((Self::ALLPASS_DELAYS_44K[0] as f32 * scale) as usize, 0.5),
            Allpass::new((Self::ALLPASS_DELAYS_44K[1] as f32 * scale) as usize, 0.5),
        ];
        Self {
            combs,
            allpasses,
            wet: 0.32,
            dry: 0.68,
        }
    }

    /// Wet/dry mix in `0.0..=1.0` (0.32 = noticeable but subtle room).
    pub fn set_mix(&mut self, wet: f32) {
        let wet = wet.clamp(0.0, 1.0);
        self.wet = wet;
        self.dry = 1.0 - wet;
    }

    pub fn reset(&mut self) {
        for c in &mut self.combs {
            c.clear();
        }
        for a in &mut self.allpasses {
            a.clear();
        }
    }

    /// Process one stereo frame.
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        // Mono-fold the input for the effect tail.
        let mono = (l + r) * 0.5;
        let mut tail = mono;
        for c in &mut self.combs {
            tail = c.process(tail);
        }
        for a in &mut self.allpasses {
            tail = a.process(tail);
        }
        let wet_l = tail + l * self.wet * 0.5;
        let wet_r = tail + r * self.wet * 0.5;
        (l * self.dry + wet_l, r * self.dry + wet_r)
    }
}

/// A bus's effect chain: optional lowpass + optional reverb.
#[derive(Debug, Clone, Default)]
pub struct DspChain {
    pub lowpass: Option<Lowpass>,
    pub reverb: Option<Reverb>,
}

impl DspChain {
    pub fn process_frame(&mut self, l: f32, r: f32) -> (f32, f32) {
        let (l, r) = match &mut self.lowpass {
            Some(lp) => lp.process(l, r),
            None => (l, r),
        };
        match &mut self.reverb {
            Some(rv) => rv.process(l, r),
            None => (l, r),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowpass_attenuates_high_frequencies() {
        let rate = 8000;
        let mut lp = Lowpass::new(200.0, rate);
        // High-freq square-ish signal: amplitude must drop sharply.
        let mut out = Vec::new();
        for i in 0..4000 {
            let x = if (i / 4) % 2 == 0 { 1.0 } else { -1.0 };
            out.push(lp.process(x, x).0);
        }
        // Steady-state peak after settling must be well below the input.
        let peak = out[2000..].iter().fold(0.0f32, |a, &v| a.max(v.abs()));
        assert!(peak < 0.35, "200 Hz lowpass on ~1 kHz square: peak {peak}");
    }

    #[test]
    fn lowpass_passes_low_frequencies() {
        let rate = 8000;
        let mut lp = Lowpass::new(500.0, rate);
        // 100 Hz sine at 8 kHz → 80 samples/period.
        let mut out = Vec::new();
        for i in 0..1600 {
            let x = (i as f32 * 2.0 * std::f32::consts::PI * 100.0 / rate as f32).sin();
            out.push(lp.process(x, x).0);
        }
        let peak = out[800..].iter().fold(0.0f32, |a, &v| a.max(v.abs()));
        assert!(peak > 0.85, "100 Hz through 500 Hz lowpass: peak {peak}");
    }

    #[test]
    fn reverb_adds_tail_to_impulse() {
        let mut rv = Reverb::new(8000, RoomSize(0.5));
        // Feed a single impulse, then silence; energy must linger.
        let mut energy_after = 0.0f32;
        let mut first = (0.0, 0.0);
        for i in 0..8000 {
            let (l, r) = rv.process(if i == 0 { 1.0 } else { 0.0 }, 0.0);
            if i == 0 {
                first = (l, r);
            }
            if i > 1000 {
                energy_after += l * l + r * r;
            }
        }
        assert!(first.0.abs() > 0.5, "dry path must pass the impulse");
        assert!(
            energy_after > 0.001,
            "reverb must leave a decaying tail, energy {energy_after}"
        );
    }

    #[test]
    fn reverb_tail_decays_to_silence() {
        let mut rv = Reverb::new(8000, RoomSize(0.5));
        rv.process(1.0, 1.0);
        let mut last = 0.0f32;
        for _ in 0..60_000 {
            let (l, r) = rv.process(0.0, 0.0);
            last = l.abs().max(r.abs());
        }
        assert!(last < 0.001, "tail must decay, last={last}");
    }
}

//! Restoration data read from the sole live native APU. Timing counters and
//! oscillator positions are not serialized; controls and current amplitude are.
use crate::{
    apu::Apu,
    channel::{LengthCounter, PulseChannel, VolumeEnvelope},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LengthControls {
    pub enabled: bool,
    pub remaining: u16,
}
impl From<&LengthCounter> for LengthControls {
    fn from(value: &LengthCounter) -> Self {
        Self {
            enabled: value.enabled,
            remaining: value.counter,
        }
    }
}
impl LengthControls {
    fn install(&self, value: &mut LengthCounter) {
        value.enabled = self.enabled;
        value.counter = self.remaining;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeControls {
    pub register: u8,
    pub current_volume: u8,
}
impl From<&VolumeEnvelope> for EnvelopeControls {
    fn from(value: &VolumeEnvelope) -> Self {
        Self {
            register: value.read_nrx2(),
            current_volume: value.current_volume,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SweepControls {
    pub register: u8,
    pub shadow_frequency: u16,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PulseControls {
    pub enabled: bool,
    pub dac_enabled: bool,
    pub duty: u8,
    pub frequency: u16,
    pub length: LengthControls,
    pub envelope: EnvelopeControls,
    pub sweep: Option<SweepControls>,
}
impl From<&PulseChannel> for PulseControls {
    fn from(value: &PulseChannel) -> Self {
        Self {
            enabled: value.enabled,
            dac_enabled: value.dac_enabled,
            duty: value.duty as u8,
            frequency: value.freq_reg,
            length: (&value.length).into(),
            envelope: (&value.envelope).into(),
            sweep: value.sweep.as_ref().map(|sweep| SweepControls {
                register: sweep.read_nr10(),
                shadow_frequency: sweep.shadow_freq,
                enabled: sweep.enabled,
            }),
        }
    }
}
impl PulseControls {
    fn install(&self, apu: &mut Apu, first: bool) {
        let base = if first { 0xff11 } else { 0xff16 };
        if first {
            if let Some(sweep) = &self.sweep {
                apu.write_register(0xff10, sweep.register);
            }
        }
        apu.write_register(base, self.duty << 6);
        apu.write_register(base + 1, self.envelope.register);
        apu.write_register(base + 2, self.frequency as u8);
        apu.write_register(
            base + 3,
            ((self.frequency >> 8) as u8 & 7)
                | (u8::from(self.length.enabled) << 6)
                | (u8::from(self.enabled) << 7),
        );
        let channel = if first { &mut apu.ch1 } else { &mut apu.ch2 };
        channel.enabled = self.enabled;
        channel.dac_enabled = self.dac_enabled;
        channel.envelope.current_volume = self.envelope.current_volume;
        self.length.install(&mut channel.length);
        if let (Some(target), Some(saved)) = (&mut channel.sweep, &self.sweep) {
            target.shadow_freq = saved.shadow_frequency;
            target.enabled = saved.enabled;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveControls {
    pub enabled: bool,
    pub dac_enabled: bool,
    pub frequency: u16,
    pub volume_code: u8,
    pub wave_ram: [u8; 16],
    pub length: LengthControls,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoiseControls {
    pub enabled: bool,
    pub dac_enabled: bool,
    pub polynomial: u8,
    pub envelope: EnvelopeControls,
    pub length: LengthControls,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApuControls {
    pub power: bool,
    pub volume: u8,
    pub panning: u8,
    pub pulse1: PulseControls,
    pub pulse2: PulseControls,
    pub wave: WaveControls,
    pub noise: NoiseControls,
}
impl From<&Apu> for ApuControls {
    fn from(value: &Apu) -> Self {
        Self {
            power: value.power,
            volume: value.nr50,
            panning: value.nr51,
            pulse1: (&value.ch1).into(),
            pulse2: (&value.ch2).into(),
            wave: WaveControls {
                enabled: value.ch3.enabled,
                dac_enabled: value.ch3.dac_enabled,
                frequency: value.ch3.freq_reg,
                volume_code: value.ch3.volume_code,
                wave_ram: value.ch3.wave_ram,
                length: (&value.ch3.length).into(),
            },
            noise: NoiseControls {
                enabled: value.ch4.enabled,
                dac_enabled: value.ch4.dac_enabled,
                polynomial: (value.ch4.clock_shift << 4)
                    | (u8::from(value.ch4.width_mode) << 3)
                    | value.ch4.divisor_code,
                envelope: (&value.ch4.envelope).into(),
                length: (&value.ch4.length).into(),
            },
        }
    }
}
impl ApuControls {
    /// Install once in the existing fresh APU. No sequencer work or inferred
    /// song controls are involved. Native trigger calls initialize private
    /// timing; their oscillator/envelope/frame-sequencer phase is excluded.
    pub(crate) fn install(&self, apu: &mut Apu) {
        apu.write_register(0xff26, 0x80);
        self.pulse1.install(apu, true);
        self.pulse2.install(apu, false);
        apu.write_register(0xff1a, 0);
        for (index, &value) in self.wave.wave_ram.iter().enumerate() {
            apu.write_register(0xff30 + index as u16, value);
        }
        apu.write_register(0xff1a, u8::from(self.wave.dac_enabled) << 7);
        apu.write_register(0xff1c, self.wave.volume_code << 5);
        apu.write_register(0xff1d, self.wave.frequency as u8);
        apu.write_register(
            0xff1e,
            ((self.wave.frequency >> 8) as u8 & 7)
                | (u8::from(self.wave.length.enabled) << 6)
                | (u8::from(self.wave.enabled) << 7),
        );
        apu.ch3.enabled = self.wave.enabled;
        self.wave.length.install(&mut apu.ch3.length);
        apu.write_register(0xff21, self.noise.envelope.register);
        apu.write_register(0xff22, self.noise.polynomial);
        apu.write_register(
            0xff23,
            (u8::from(self.noise.length.enabled) << 6) | (u8::from(self.noise.enabled) << 7),
        );
        apu.ch4.enabled = self.noise.enabled;
        apu.ch4.dac_enabled = self.noise.dac_enabled;
        apu.ch4.envelope.current_volume = self.noise.envelope.current_volume;
        self.noise.length.install(&mut apu.ch4.length);
        apu.write_register(0xff24, self.volume);
        apu.write_register(0xff25, self.panning);
        // The manager is hosted and its APU is the sole software output owner;
        // an originally unpowered APU still has meaningful stored controls.
        apu.power = self.power;
    }
}

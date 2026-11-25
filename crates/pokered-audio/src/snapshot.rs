//! Logical playback and complete software APU state for hosted snapshots.
//! A DTO restores the same live manager; it never advances a second sequencer.
use crate::alloc_prelude::*;
use crate::music_data::MusicId;
pub use crate::native_controls::{
    ApuControls, EnvelopeControls, LengthControls, NoiseControls, PulseControls, SweepControls,
    WaveControls,
};
use crate::{sequencer::Sequencer, ChannelFlags1, ChannelFlags2, ChannelState};
use serde::{Deserialize, Serialize};

macro_rules! channel_snapshot {
    ($($name:ident: $ty:ty),* $(,)?) => {
        #[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
        pub struct MusicChannelSnapshot {
            $(pub $name: $ty,)*
            pub flags1: u8, pub flags2: u8,
            pub vibrato: [u8;4],
            pub pitch_slide: (u16,u16,u16,u8,u8,u8),
        }
        impl From<&ChannelState> for MusicChannelSnapshot {
            fn from(ch: &ChannelState) -> Self {
                Self { $($name:ch.$name.clone(),)* flags1:ch.flags1.bits(),flags2:ch.flags2.bits(),
                    vibrato:[ch.vibrato.delay_reload,ch.vibrato.delay_counter,ch.vibrato.extent,ch.vibrato.rate],
                    pitch_slide:(ch.pitch_slide.target_freq,ch.pitch_slide.current_freq,ch.pitch_slide.freq_step,
                        ch.pitch_slide.freq_frac,ch.pitch_slide.step_frac,ch.pitch_slide.length_modifier) }
            }
        }
        impl MusicChannelSnapshot {
            fn restore(&self) -> ChannelState {
                let mut ch=ChannelState::new();
                $(ch.$name=self.$name.clone();)*
                ch.flags1=ChannelFlags1::from_bits_retain(self.flags1);
                ch.flags2=ChannelFlags2::from_bits_retain(self.flags2);
                [ch.vibrato.delay_reload,ch.vibrato.delay_counter,ch.vibrato.extent,ch.vibrato.rate]=self.vibrato;
                (ch.pitch_slide.target_freq,ch.pitch_slide.current_freq,ch.pitch_slide.freq_step,
                    ch.pitch_slide.freq_frac,ch.pitch_slide.step_frac,ch.pitch_slide.length_modifier)=self.pitch_slide;
                ch
            }
        }
    }
}
channel_snapshot! {
    data:Vec<u8>, ptr:usize, return_ptr:usize,
    delay_counter:u8, delay_frac:u8, note_speed:u8, note_length:u8,
    octave:u8, frequency:u16, freq_lo_saved:u8, volume_envelope:u8,
    duty_cycle:u8, duty_cycle_pattern:u8, loop_counter:u8, sound_id:u8,
    active:bool, wave_instrument:u8, stereo_panning:u8, trigger:bool,
    vibrato_freq_lo:Option<u8>, pitch_sweep_value:u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencerSnapshot {
    pub channels: [MusicChannelSnapshot; 8],
    pub music_tempo: u16,
    pub sfx_tempo: u16,
    pub stereo_panning: u8,
    pub music_playing: bool,
    pub sfx_playing: bool,
    pub current_music_id: u8,
    pub current_sfx_id: u8,
    pub frequency_modifier: i16,
    pub tempo_modifier: i16,
    pub fade_counter: u8,
    pub pending_nr50: Option<u8>,
    pub noise_instruments: Vec<Vec<u8>>,
}
impl From<&Sequencer> for SequencerSnapshot {
    fn from(seq: &Sequencer) -> Self {
        Self {
            channels: core::array::from_fn(|i| (&seq.channels[i]).into()),
            music_tempo: seq.music_tempo,
            sfx_tempo: seq.sfx_tempo,
            stereo_panning: seq.stereo_panning,
            music_playing: seq.music_playing,
            sfx_playing: seq.sfx_playing,
            current_music_id: seq.current_music_id,
            current_sfx_id: seq.current_sfx_id,
            frequency_modifier: seq.frequency_modifier,
            tempo_modifier: seq.tempo_modifier,
            fade_counter: seq.fade_counter,
            pending_nr50: seq.pending_nr50,
            noise_instruments: seq.noise_instruments.clone(),
        }
    }
}
impl SequencerSnapshot {
    pub(crate) fn restore(&self) -> Sequencer {
        Sequencer {
            channels: core::array::from_fn(|i| self.channels[i].restore()),
            music_tempo: self.music_tempo,
            sfx_tempo: self.sfx_tempo,
            stereo_panning: self.stereo_panning,
            music_playing: self.music_playing,
            sfx_playing: self.sfx_playing,
            current_music_id: self.current_music_id,
            current_sfx_id: self.current_sfx_id,
            frequency_modifier: self.frequency_modifier,
            tempo_modifier: self.tempo_modifier,
            fade_counter: self.fade_counter,
            pending_nr50: self.pending_nr50,
            noise_instruments: self.noise_instruments.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioSnapshot {
    pub current_music: Option<MusicId>,
    pub sequencer: SequencerSnapshot,
    pub saved_music: Vec<(MusicId, SequencerSnapshot)>,
    pub master_volume: [u8; 2],
    pub no_audio_fade_out: bool,
    pub low_health_alarm_enabled: bool,
    pub low_health_timer: u8,
    pub music_after_sfx: Option<MusicId>,
    /// Retained for snapshots produced before full software phase was captured.
    pub apu: ApuControls,
    /// The complete state is authoritative when present. Missing phase keeps
    /// the older control-only restore guarantee rather than inventing counters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apu_playback: Option<crate::apu::ApuPlaybackState>,
    /// Fractional CPU-cycle remainder and negotiated sample rate of hosted
    /// `PcmRenderer` (`output::pcm_renderer`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pcm_phase: Option<(Option<u32>, u32)>,
}

impl Serialize for MusicId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(*self as u8)
    }
}
impl<'de> Deserialize<'de> for MusicId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = u8::deserialize(deserializer)?;
        Self::from_u8(value).ok_or_else(|| serde::de::Error::custom("unknown native music id"))
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "full_apu_snapshot_tests.rs"]
mod full_apu_tests;

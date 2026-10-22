//! Retained Audio1/3_UpdateMusic CPU body over the one existing native sequencer.
//! Physical APU writes and IRQ epilogue remain outside this component.
use super::music_note_instructions::{MusicNoteInput, MusicNoteInstructions, MusicNoteOutcome};
use super::note_preparation::PreparationInput;
use super::note_stream_instructions::{
    MusicCommandState, StreamInput, StreamInputError, StreamStepError,
};
use super::press_ab_instructions::{
    PressAbInput, PressAbInstructions, PressAbOutcome, PressAbStepError,
};
use super::timer_work::{ChannelEffectWorkInput, EffectInstructionWork};
use super::{AudioClockError, AudioFrameWork};
use crate::{music_data::MusicId, sequencer::Sequencer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum SourceSong {
    Pokecenter,
    Dungeon3,
    SilphCo,
}
impl SourceSong {
    pub fn bank(self) -> u8 {
        match self {
            Self::Pokecenter => 2,
            Self::Dungeon3 | Self::SilphCo => 31,
        }
    }
    /// Pinned readonly original header offsets / 3, not native enum identities.
    pub fn raw_id(self) -> u8 {
        match self {
            Self::Pokecenter => 189,
            Self::Dungeon3 => 232,
            Self::SilphCo => 243,
        }
    }
    pub(crate) fn music(self) -> MusicId {
        match self {
            Self::Pokecenter => MusicId::POKECENTER,
            Self::Dungeon3 => MusicId::DUNGEON3,
            Self::SilphCo => MusicId::SILPH_CO,
        }
    }
    pub(crate) fn from_music(id: MusicId) -> Option<Self> {
        Some(match id {
            MusicId::POKECENTER => Self::Pokecenter,
            MusicId::DUNGEON3 => Self::Dungeon3,
            MusicId::SILPH_CO => Self::SilphCo,
            _ => return None,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
struct Voice {
    active: bool,
    id: u8,
    delay: u8,
    fraction: u8,
    ptr: usize,
    return_ptr: usize,
    loop_counter: u8,
    speed: u8,
    volume: u8,
    duty: u8,
    octave: u8,
    flags1: u8,
    flags2: u8,
    vib_delay: u8,
    vib_reload: u8,
    vib_extent: u8,
    vib_rate: u8,
    frequency: u16,
    saved_low: u8,
    data_fingerprint: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
struct NativeStart {
    voices: [Voice; 8],
    music_tempo: u16,
    sfx_tempo: u16,
    music_id: u8,
    sfx_id: u8,
    music_playing: bool,
    sfx_playing: bool,
    frequency_modifier: i16,
    tempo_modifier: i16,
    panning: u8,
}
impl NativeStart {
    fn read(seq: &Sequencer) -> Self {
        Self {
            voices: core::array::from_fn(|i| {
                let c = &seq.channels[i];
                let hash = c.data.iter().fold(0xcbf29ce484222325u64, |h, b| {
                    (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
                });
                Voice {
                    active: c.active,
                    id: c.sound_id,
                    delay: c.delay_counter,
                    fraction: c.delay_frac,
                    ptr: c.ptr,
                    return_ptr: c.return_ptr,
                    loop_counter: c.loop_counter,
                    speed: c.note_speed,
                    volume: c.volume_envelope,
                    duty: c.duty_cycle << 6,
                    octave: c.octave,
                    flags1: c.flags1.bits(),
                    flags2: c.flags2.bits(),
                    vib_delay: c.vibrato.delay_counter,
                    vib_reload: c.vibrato.delay_reload,
                    vib_extent: c.vibrato.extent,
                    vib_rate: c.vibrato.rate,
                    frequency: c.frequency,
                    saved_low: c.freq_lo_saved,
                    data_fingerprint: hash,
                }
            }),
            music_tempo: seq.music_tempo,
            sfx_tempo: seq.sfx_tempo,
            music_id: seq.current_music_id,
            sfx_id: seq.current_sfx_id,
            music_playing: seq.music_playing,
            sfx_playing: seq.sfx_playing,
            frequency_modifier: seq.frequency_modifier,
            tempo_modifier: seq.tempo_modifier,
            panning: seq.stereo_panning,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameStepError<E> {
    Clock(E),
    Stream(StreamInputError),
    ChangedProgram,
    Unsupported(AudioClockError),
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum Stage {
    Init,
    Channel(u8),
    Kind(u8),
    Mute(u8),
    Call,
    ReadyEntry(u8),
    Effect(EffectInstructionWork),
    Music(MusicNoteInstructions),
    Press(PressAbInstructions),
    Tail(u8),
    Return,
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct SourceAudioFrame {
    song: SourceSong,
    bank: u8,
    mute: u8,
    start: NativeStart,
    ids: [u8; 8],
    fractions: [u8; 8],
    tempo: u16,
    channel: u8,
    stage: Stage,
    cycles: u32,
    pub(crate) committed: Option<Result<AudioFrameWork, AudioClockError>>,
}
impl SourceAudioFrame {
    pub fn new(
        seq: &Sequencer,
        song: SourceSong,
        bank: u8,
        mute: u8,
    ) -> Result<Self, AudioClockError> {
        if bank != song.bank() || mute != 0 {
            return Err(AudioClockError::UnsupportedSoundBank);
        }
        let start = NativeStart::read(seq);
        for i in 5..8 {
            if start.voices[i].active {
                return Err(AudioClockError::ActiveSfx(i as u8));
            }
        }
        let mut ids = core::array::from_fn(|i| {
            if !start.voices[i].active {
                0
            } else if i < 4 {
                song.raw_id()
            } else {
                super::press_ab_work::SOURCE_PRESS_AB_ID
            }
        });
        for (i, v) in start.voices.iter().enumerate() {
            if !v.active {
                continue;
            }
            if i < 4 {
                if v.id != song.music() as u8 || v.delay == 0 {
                    return Err(AudioClockError::UnsupportedEffect(i as u8));
                }
                ids[i] = song.raw_id();
            } else {
                if i != 4
                    || v.id != crate::sfx_data::SfxId::PressAB as u8
                    || seq.channels[i].data != crate::sfx_data::SFX_PRESS_AB_CH5
                    || v.flags1 != 4
                    || v.flags2 != 0
                    || v.speed != 1
                    || seq.sfx_tempo != 0x100
                {
                    return Err(AudioClockError::ActiveSfx(i as u8));
                }
                if v.delay == 0
                    && !(v.ptr == 0 && v.duty == 0 && v.volume == 0xf0 && v.fraction == 0)
                {
                    return Err(AudioClockError::UnsupportedEffect(4));
                }
                ids[i] = super::press_ab_work::SOURCE_PRESS_AB_ID;
            }
            if v.delay > 1 {
                EffectInstructionWork::new(Self::effect_input(&start, &ids, i))
                    .ok_or(AudioClockError::UnsupportedEffect(i as u8))?;
            }
        }
        let fractions = core::array::from_fn(|i| start.voices[i].fraction);
        let tempo = start.music_tempo;
        Ok(Self {
            song,
            bank,
            mute,
            start,
            ids,
            fractions,
            tempo,
            channel: 0,
            stage: Stage::Init,
            cycles: 0,
            committed: None,
        })
    }
    pub fn matches_native(&self, seq: &Sequencer) -> bool {
        self.start == NativeStart::read(seq)
    }
    pub fn bank(&self) -> u8 {
        self.bank
    }
    pub fn mute(&self) -> u8 {
        self.mute
    }
    pub fn song(&self) -> SourceSong {
        self.song
    }
    pub fn cycles(&self) -> u32 {
        self.cycles
    }
    pub fn finished(&self) -> bool {
        matches!(self.stage, Stage::Done)
    }
    fn effect_input(start: &NativeStart, ids: &[u8; 8], i: usize) -> ChannelEffectWorkInput {
        let v = start.voices[i];
        ChannelEffectWorkInput {
            channel: i as u8,
            paired_sfx_active: i < 4 && ids[i + 4] != 0,
            note_delay: v.delay,
            flags1: v.flags1,
            flags2: v.flags2,
            vibrato_delay: v.vib_delay,
            vibrato_extent: v.vib_extent,
            vibrato_rate: v.vib_rate,
            frequency_low: v.saved_low,
        }
    }
    fn note(&self, seq: &Sequencer) -> Result<Stage, AudioClockError> {
        let i = self.channel as usize;
        let v = self.start.voices[i];
        if i == 4 {
            return Ok(Stage::Press(
                PressAbInstructions::new(
                    PressAbInput {
                        bank: self.bank,
                        source_sfx_id: self.ids[4],
                        ptr: v.ptr,
                        fraction: self.fractions[4],
                        duty: v.duty,
                        flags1: v.flags1,
                        flags2: v.flags2,
                        note_speed: v.speed,
                        sfx_tempo: self.start.sfx_tempo,
                        vibrato_reload: v.vib_reload,
                    },
                    &seq.channels[4].data,
                )
                .ok_or(AudioClockError::UnsupportedEffect(4))?,
            ));
        }
        let state = MusicCommandState {
            speed: v.speed,
            volume: if i == 2 {
                (v.volume & 0x30) << 1
            } else {
                v.volume
            },
            duty: v.duty,
            octave: v.octave,
            flags1: v.flags1,
            vibrato_delay: v.vib_delay,
        };
        Ok(Stage::Music(
            MusicNoteInstructions::new(MusicNoteInput {
                preparation: PreparationInput {
                    stream: StreamInput {
                        channel: self.channel,
                        ptr: v.ptr,
                        return_ptr: v.return_ptr,
                        loop_counter: v.loop_counter,
                        state,
                        vibrato_reload: v.vib_reload,
                    },
                    flags2: v.flags2,
                    fraction: self.fractions[i],
                    tempo: self.tempo,
                },
                bank: self.bank,
                paired_sfx_active: self.ids[i + 4] != 0,
                source_sfx_id: self.ids[4],
                frequency_modifier: 0,
                previous_frequency_low: v.saved_low,
            })
            .ok_or(AudioClockError::UnsupportedEffect(self.channel))?,
        ))
    }
    /// Caller must keep its starting native state/data fixed and IRQ ownership
    /// exclusive until completion. No native update or physical bus write occurs.
    pub fn try_step<E>(
        &mut self,
        seq: &Sequencer,
        execute: impl FnOnce(u8) -> Result<(), E>,
    ) -> Result<bool, FrameStepError<E>> {
        let i = self.channel as usize;
        match &mut self.stage {
            Stage::Effect(work) => {
                let t = work.peek().expect("pending effect");
                execute(t).map_err(FrameStepError::Clock)?;
                work.next();
                self.cycles += u32::from(t);
                if work.finished() {
                    self.stage = Stage::Tail(0);
                }
                return Ok(false);
            }
            Stage::Music(work) => {
                let mut accepted = 0;
                let done = work
                    .try_step(&seq.channels[i].data, |t| {
                        execute(t)?;
                        accepted = t;
                        Ok(())
                    })
                    .map_err(|e| match e {
                        StreamStepError::Clock(e) => FrameStepError::Clock(e),
                        StreamStepError::Input(e) => FrameStepError::Stream(e),
                    })?;
                self.cycles += u32::from(accepted);
                if done {
                    let (tempo, fraction, ended) =
                        match work.result().expect("completed music body") {
                            MusicNoteOutcome::Note(n) => (
                                n.preparation.stream.tempo,
                                Some(n.preparation.fraction),
                                false,
                            ),
                            MusicNoteOutcome::End(n) => (n.stream.tempo, None, n.ending.ended),
                        };
                    if let Some(tempo) = tempo {
                        self.tempo = tempo;
                        self.fractions[..4].fill(0);
                    }
                    if let Some(fraction) = fraction {
                        self.fractions[i] = fraction;
                    }
                    if ended {
                        self.ids[i] = 0;
                    }
                    self.stage = Stage::Tail(0);
                }
                return Ok(false);
            }
            Stage::Press(work) => {
                let mut accepted = 0;
                let done = work
                    .try_step(&seq.channels[4].data, |t| {
                        execute(t)?;
                        accepted = t;
                        Ok(())
                    })
                    .map_err(|e| match e {
                        PressAbStepError::Clock(e) => FrameStepError::Clock(e),
                        _ => FrameStepError::ChangedProgram,
                    })?;
                self.cycles += u32::from(accepted);
                if done {
                    match work.result().expect("completed PressAB") {
                        PressAbOutcome::Note { fraction, .. } => self.fractions[4] = fraction,
                        PressAbOutcome::End { .. } => self.ids[4] = 0,
                    };
                    self.stage = Stage::Tail(0);
                }
                return Ok(false);
            }
            _ => (),
        }
        let Some(t) = self.next_cycles() else {
            return Ok(true);
        };
        // Construct the next child before accepting its transfer instruction.
        let note = if matches!(self.stage, Stage::ReadyEntry(5)) {
            Some(self.note(seq).map_err(FrameStepError::Unsupported)?)
        } else {
            None
        };
        execute(t).map_err(FrameStepError::Clock)?;
        self.advance(note);
        self.cycles += u32::from(t);
        Ok(self.finished())
    }
    fn next_cycles(&self) -> Option<u8> {
        use Stage::*;
        Some(match self.stage {
            Init => 8,
            Channel(n) => {
                if n < 5 {
                    [8, 12, 8, 8, 4][n as usize]
                } else if self.ids[self.channel as usize] == 0 {
                    12
                } else {
                    8
                }
            }
            Kind(n) => {
                if n < 2 {
                    [4, 8][n as usize]
                } else if self.channel >= 4 {
                    12
                } else {
                    8
                }
            }
            Mute(n) => [16, 4, 12][n as usize],
            Call => 24,
            ReadyEntry(n) => [8, 12, 8, 8, 8, 16][n as usize],
            Tail(n) => {
                if n < 3 {
                    [4, 4, 8][n as usize]
                } else if self.channel == 7 {
                    8
                } else {
                    12
                }
            }
            Return => 16,
            Done => return None,
            Effect(_) | Music(_) | Press(_) => unreachable!("child owns instruction"),
        })
    }
    fn advance(&mut self, note: Option<Stage>) {
        use Stage::*;
        self.stage = match self.stage {
            Init => Channel(0),
            Channel(n) => {
                if n == 5 {
                    if self.ids[self.channel as usize] == 0 {
                        Tail(0)
                    } else {
                        Kind(0)
                    }
                } else {
                    Channel(n + 1)
                }
            }
            Kind(n) => {
                if n == 2 {
                    if self.channel >= 4 {
                        Call
                    } else {
                        Mute(0)
                    }
                } else {
                    Kind(n + 1)
                }
            }
            Mute(n) => {
                if n == 2 {
                    Call
                } else {
                    Mute(n + 1)
                }
            }
            Call => {
                let i = self.channel as usize;
                if self.start.voices[i].delay > 1 {
                    Effect(
                        EffectInstructionWork::new(Self::effect_input(&self.start, &self.ids, i))
                            .expect("validated effect"),
                    )
                } else {
                    ReadyEntry(0)
                }
            }
            ReadyEntry(n) => {
                if n == 5 {
                    note.expect("preflighted note transfer")
                } else {
                    ReadyEntry(n + 1)
                }
            }
            Tail(n) => {
                if n == 3 {
                    if self.channel == 7 {
                        Return
                    } else {
                        self.channel += 1;
                        Channel(0)
                    }
                } else {
                    Tail(n + 1)
                }
            }
            Return => Done,
            Done => Done,
            Effect(_) | Music(_) | Press(_) => unreachable!("child progress"),
        };
    }
}

//! Retained Audio1/3_UpdateMusic CPU body over the one existing native sequencer.
//! Physical APU writes and IRQ epilogue remain outside this component.
use super::music_note_instructions::{MusicNoteInput, MusicNoteInstructions, MusicNoteOutcome};
use super::note_preparation::PreparationInput;
use super::note_stream_instructions::{
    MusicCommandState, StreamInput, StreamInputError, StreamStepError,
};
use super::pitch_slide_instructions::PitchSlideState;
use super::press_ab_instructions::{
    PressAbInput, PressAbInstructions, PressAbOutcome, PressAbStepError,
};
use super::timer_work::{ChannelEffectWorkInput, EffectInstructionWork};
use super::{AudioClockError, AudioFrameWork};
use crate::{music_data::MusicId, sequencer::Sequencer};

// Keep the persisted FNV-1a fingerprint exactly, including inactive programs.
// On Thumb-1 the generic u64 multiply calls __aeabi_lmul for every byte.
fn program_fingerprint(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        let value = hash ^ u64::from(byte);
        #[cfg(all(target_arch = "arm", target_pointer_width = "32", target_os = "none"))]
        {
            fnv_multiply_arm32(value)
        }
        #[cfg(not(all(target_arch = "arm", target_pointer_width = "32", target_os = "none")))]
        {
            value.wrapping_mul(0x0000_0100_0000_01b3)
        }
    })
}

// The prime is 2^40 + 435. Both 16-bit partial products and their carry fit
// in u32; the upper word intentionally wraps modulo 2^32, as in the u64 hash.
#[cfg(any(
    test,
    all(target_arch = "arm", target_pointer_width = "32", target_os = "none")
))]
#[inline]
fn fnv_multiply_arm32(value: u64) -> u64 {
    let lo = value as u32;
    let hi = (value >> 32) as u32;
    let low_product = (lo & 0xffff) * 435;
    let high_product = (lo >> 16) * 435 + (low_product >> 16);
    let result_lo = (low_product & 0xffff) | (high_product << 16);
    let result_hi = hi
        .wrapping_mul(435)
        .wrapping_add(high_product >> 16)
        .wrapping_add(lo << 8);
    (u64::from(result_hi) << 32) | u64::from(result_lo)
}

#[cfg(test)]
#[path = "frame_fingerprint_tests.rs"]
mod fingerprint_tests;

// Original MapSongBanks plus BikeRiding/Surfing, from the pinned source ROM.
// Identifying a song does not waive the instruction interpreter's state checks.
macro_rules! field_songs {
    ($($song:ident => ($music:ident, $bank:literal, $id:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[cfg_attr(any(test, feature = "source-clock-serde"), derive(serde::Serialize, serde::Deserialize))]
        pub enum SourceSong { $($song),+ }
        impl SourceSong {
            pub const ALL: &'static [Self] = &[$(Self::$song),+];
            pub fn bank(self) -> u8 { match self { $(Self::$song => $bank),+ } }
            /// Original header offsets / 3; native enum zero is not silence.
            pub fn raw_id(self) -> u8 { match self { $(Self::$song => $id),+ } }
            pub(crate) fn music(self) -> MusicId { match self { $(Self::$song => MusicId::$music),+ } }
            pub(crate) fn from_music(id: MusicId) -> Option<Self> {
                Some(match id { $(MusicId::$music => Self::$song),+, _ => return None })
            }
        }
    };
}
field_songs! {
    PalletTown => (PALLET_TOWN, 2, 186),
    Pokecenter => (POKECENTER, 2, 189),
    Gym => (GYM, 2, 192),
    Cities1 => (CITIES1, 2, 195),
    Cities2 => (CITIES2, 2, 199),
    Celadon => (CELADON, 2, 202),
    Cinnabar => (CINNABAR, 2, 205),
    Vermilion => (VERMILION, 2, 208),
    Lavender => (LAVENDER, 2, 212),
    SSAnne => (SSANNE, 2, 216),
    SafariZone => (SAFARI_ZONE, 2, 229),
    Routes1 => (ROUTES1, 2, 235),
    Routes2 => (ROUTES2, 2, 239),
    Routes3 => (ROUTES3, 2, 243),
    Routes4 => (ROUTES4, 2, 247),
    IndigoPlateau => (INDIGO_PLATEAU, 2, 251),
    OaksLab => (OAKS_LAB, 31, 205),
    BikeRiding => (BIKE_RIDING, 31, 210),
    Surfing => (SURFING, 31, 214),
    GameCorner => (GAME_CORNER, 31, 217),
    Dungeon1 => (DUNGEON1, 31, 224),
    Dungeon2 => (DUNGEON2, 31, 228),
    Dungeon3 => (DUNGEON3, 31, 232),
    CinnabarMansion => (CINNABAR_MANSION, 31, 236),
    PokemonTower => (POKEMON_TOWER, 31, 240),
    SilphCo => (SILPH_CO, 31, 243),
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
    wave_instrument: u8,
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
    #[cfg_attr(any(test, feature = "source-clock-serde"), serde(default))]
    pitch_slide: PitchSlideState,
    data_fingerprint: u64,
}
impl Voice {
    fn started_noise(instrument: u8) -> Self {
        Self {
            active: true,
            id: instrument,
            delay: 1,
            fraction: 0,
            ptr: 0,
            return_ptr: 0,
            loop_counter: 0,
            speed: 1,
            volume: 0xf0,
            wave_instrument: 0,
            duty: 0,
            octave: 4,
            flags1: 4,
            flags2: 0,
            vib_delay: 0,
            vib_reload: 0,
            vib_extent: 0,
            vib_rate: 0,
            frequency: 0,
            saved_low: 0,
            pitch_slide: PitchSlideState::default(),
            data_fingerprint: program_fingerprint(
                PressAbInstructions::noise_data(instrument).expect("canonical drum constructor"),
            ),
        }
    }
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
                    wave_instrument: c.wave_instrument,
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
                    pitch_slide: PitchSlideState::from_native(&c.pitch_slide),
                    data_fingerprint: program_fingerprint(&c.data),
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
    panning: Option<u8>,
    start: NativeStart,
    physical_output: bool,
    ids: [u8; 8],
    fractions: [u8; 8],
    started_drum: Option<u8>,
    tempo: u16,
    channel: u8,
    stage: Stage,
    cycles: u32,
    pub(crate) committed: Option<Result<AudioFrameWork, AudioClockError>>,
    pub(crate) source_failed: Option<AudioClockError>,
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
        if seq.channels[3].active && !super::canonical_noise_table(seq) {
            return Err(AudioClockError::UnsupportedEffect(3));
        }
        let start = NativeStart::read(seq);
        if start.voices[6].active {
            return Err(AudioClockError::ActiveSfx(6));
        }
        let mut ids = core::array::from_fn(|i| {
            if !start.voices[i].active {
                0
            } else if i < 4 {
                song.raw_id()
            } else if i == 4 || i == 5 {
                super::press_ab_work::source_pulse_id(i, start.voices[i].id, &seq.channels[i].data)
                    .unwrap_or(0)
            } else if i == 7 {
                super::press_ab_work::source_noise_id(start.voices[i].id, &seq.channels[i].data)
                    .unwrap_or(0)
            } else {
                start.voices[i].id
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
                let valid_program = match i {
                    4 | 5 => super::press_ab_work::source_pulse_id(i, v.id, &seq.channels[i].data).is_some()
                        && seq.sfx_tempo == 0x100,
                    7 => {
                        super::press_ab_work::source_noise_id(v.id, &seq.channels[i].data).is_some()
                            && v.duty == 0
                    }
                    _ => false,
                };
                if !valid_program || v.flags1 != 4 || v.flags2 != 0 || v.speed != 1 {
                    return Err(AudioClockError::ActiveSfx(i as u8));
                }
                if v.delay == 0
                    && !(v.ptr == 0 && v.duty == 0 && v.volume == 0xf0 && v.fraction == 0)
                {
                    return Err(AudioClockError::UnsupportedEffect(i as u8));
                }
            }
            if v.delay > 1 {
                EffectInstructionWork::with_pitch_slide(
                    Self::effect_input(&start, &ids, i),
                    Some(v.pitch_slide),
                )
                .ok_or(AudioClockError::UnsupportedEffect(i as u8))?;
            }
        }
        let fractions = core::array::from_fn(|i| start.voices[i].fraction);
        let tempo = start.music_tempo;
        Ok(Self {
            song,
            bank,
            mute,
            panning: None,
            start,
            physical_output: false,
            ids,
            fractions,
            started_drum: None,
            tempo,
            channel: 0,
            stage: Stage::Init,
            cycles: 0,
            committed: None,
            source_failed: None,
        })
    }
    /// Retain the same instruction cursor with actual APU accesses enabled.
    /// The source path admits unmodified PressAB, SWAP and canonical drum programs.
    pub fn new_with_bus(
        seq: &Sequencer,
        song: SourceSong,
        bank: u8,
        mute: u8,
    ) -> Result<Self, AudioClockError> {
        let mut work = Self::new(seq, song, bank, mute)?;
        if seq.frequency_modifier != 0 || seq.tempo_modifier != 0 {
            for i in 4..=5 {
                if seq.channels[i].active {
                    return Err(AudioClockError::UnsupportedEffect(i as u8));
                }
            }
        }
        work.physical_output = true;
        Ok(work)
    }
    pub fn has_bus_output(&self) -> bool {
        self.physical_output
    }
    pub fn matches_native(&self, seq: &Sequencer) -> bool {
        self.start == NativeStart::read(seq)
            && (!self.start.voices[3].active || super::canonical_noise_table(seq))
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
        let v = self.voice(i);
        if matches!(i, 4 | 5 | 7) {
            let input = PressAbInput {
                bank: self.bank,
                source_sfx_id: self.ids[4],
                ptr: v.ptr,
                fraction: self.fractions[i],
                duty: v.duty,
                flags1: v.flags1,
                flags2: v.flags2,
                note_speed: v.speed,
                sfx_tempo: self.start.sfx_tempo,
                vibrato_reload: v.vib_reload,
            };
            let work = if i <= 5 && v.id == crate::sfx_data::SfxId::Swap as u8 {
                PressAbInstructions::new_swap(input, i as u8, self.data(seq, i))
            } else if i == 4 {
                PressAbInstructions::new(input, self.data(seq, i))
            } else {
                PressAbInstructions::new_noise(
                    input,
                    self.ids[i],
                    self.panning.unwrap_or(self.start.panning),
                    self.data(seq, i),
                )
            };
            return Ok(Stage::Press(
                work.ok_or(AudioClockError::UnsupportedEffect(i as u8))?,
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
            MusicNoteInstructions::with_drum_context(
                MusicNoteInput {
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
                },
                Some(self.ids[7] != 0),
            )
            .ok_or(AudioClockError::UnsupportedEffect(self.channel))?,
        ))
    }
    fn voice(&self, i: usize) -> Voice {
        if i == 7 {
            if let Some(instrument) = self.started_drum {
                return Voice::started_noise(instrument);
            }
        }
        self.start.voices[i]
    }
    fn data<'a>(&self, seq: &'a Sequencer, i: usize) -> &'a [u8] {
        if i == 7 {
            if let Some(instrument) = self.started_drum {
                return PressAbInstructions::noise_data(instrument).expect("canonical drum data");
            }
        }
        &seq.channels[i].data
    }
    /// Caller must keep its starting native state/data fixed and IRQ ownership
    /// exclusive until completion. No native update or physical bus write occurs.
    pub fn try_step<E>(
        &mut self,
        seq: &Sequencer,
        execute: impl FnOnce(u8) -> Result<(), E>,
    ) -> Result<bool, FrameStepError<E>> {
        if self.physical_output {
            return Err(FrameStepError::ChangedProgram);
        }
        self.step_inner(seq, |op| execute(op.tcycles).map(|()| None))
    }
    /// Execute one actual instruction/access. Accepted reads must return Some;
    /// refusal precedes all clock/APU effects. Keep the APU and native sequencer
    /// exclusively owned until the complete frame and its logical commit finish.
    pub fn try_step_with_bus<E>(
        &mut self,
        seq: &Sequencer,
        execute: impl FnOnce(super::apu_bus::ApuInstruction) -> Result<Option<u8>, E>,
    ) -> Result<bool, FrameStepError<E>> {
        if !self.physical_output {
            return Err(FrameStepError::ChangedProgram);
        }
        self.step_inner(seq, execute)
    }
    /// Keep an effect child borrowed until it returns or the clock refuses an
    /// instruction. The bus may accept a fixed prefix without APU accesses;
    /// otherwise each instruction retains its original individual callback.
    pub(crate) fn run_with_bus<B: super::apu_bus::SourceAudioFrameBus>(
        &mut self,
        seq: &Sequencer,
        apu: &mut crate::apu::Apu,
        bus: &mut B,
    ) -> Result<(), FrameStepError<B::Error>> {
        use super::apu_bus::ApuInstruction;
        if !self.physical_output {
            return Err(FrameStepError::ChangedProgram);
        }
        while !self.finished() {
            if let Stage::Effect(work) = &mut self.stage {
                assert!(!work.finished(), "pending effect");
                loop {
                    let prefix = work.plain_prefix();
                    if prefix.len() > 1 {
                        let count = bus
                            .execute_plain_run(apu, prefix)
                            .map_err(FrameStepError::Clock)?;
                        if count != 0 {
                            self.cycles += work.accept_plain_prefix(count);
                            if work.finished() {
                                break;
                            }
                            continue;
                        }
                    }
                    let mut accepted = 0;
                    let done = work
                        .try_step_with_apu(|t, write| {
                            let op = write.map_or_else(
                                || ApuInstruction::plain(t),
                                |w| ApuInstruction::write(t, w.address, w.value),
                            );
                            bus.execute(apu, op)?;
                            accepted = t;
                            Ok(())
                        })
                        .map_err(FrameStepError::Clock)?;
                    self.cycles += u32::from(accepted);
                    if done {
                        break;
                    }
                }
                self.stage = Stage::Tail(0);
            } else {
                let prefix = self.dispatch_plain_prefix();
                if prefix.len() > 1 {
                    let count = bus
                        .execute_plain_run(apu, prefix)
                        .map_err(FrameStepError::Clock)?;
                    if count != 0 {
                        self.accept_dispatch_plain_prefix(count);
                        continue;
                    }
                }
                self.step_inner(seq, |op| bus.execute(apu, op))?;
            }
        }
        Ok(())
    }

    // These source blocks only inspect the frame's immutable channel/mute
    // input. Child construction and transfer stay on the checked step path.
    fn dispatch_plain_prefix(&self) -> &'static [u8] {
        use Stage::*;
        let (code, index): (&[u8], u8) = match self.stage {
            Channel(n) if self.ids[self.channel as usize] == 0 => (&[8, 12, 8, 8, 4, 12], n),
            Channel(n) => (&[8, 12, 8, 8, 4, 8], n),
            Kind(n) if self.channel >= 4 => (&[4, 8, 12], n),
            Kind(n) => (&[4, 8, 8], n),
            Mute(n) => (&[16, 4, 12], n),
            ReadyEntry(n) if n < 5 => (&[8, 12, 8, 8, 8], n),
            Tail(n) if self.channel == 7 => (&[4, 4, 8, 8], n),
            Tail(n) => (&[4, 4, 8, 12], n),
            _ => return &[],
        };
        &code[usize::from(index)..]
    }

    fn accept_dispatch_plain_prefix(&mut self, count: usize) {
        use Stage::*;
        let prefix = self.dispatch_plain_prefix();
        assert!(count > 0 && count <= prefix.len());
        self.cycles += prefix[..count].iter().map(|&t| u32::from(t)).sum::<u32>();
        match &mut self.stage {
            Channel(n) | Kind(n) | Mute(n) | ReadyEntry(n) | Tail(n) => *n += (count - 1) as u8,
            _ => unreachable!("nonempty dispatch prefix"),
        }
        self.advance(None);
    }
    fn step_inner<E>(
        &mut self,
        seq: &Sequencer,
        execute: impl FnOnce(super::apu_bus::ApuInstruction) -> Result<Option<u8>, E>,
    ) -> Result<bool, FrameStepError<E>> {
        use super::apu_bus::ApuInstruction;
        let i = self.channel as usize;
        match &mut self.stage {
            Stage::Effect(work) => {
                let t = work.peek().expect("pending effect");
                if self.physical_output {
                    work.try_step_with_apu(|t, write| {
                        let op = write.map_or_else(
                            || ApuInstruction::plain(t),
                            |w| ApuInstruction::write(t, w.address, w.value),
                        );
                        execute(op).map(|_| ())
                    })
                    .map_err(FrameStepError::Clock)?;
                } else {
                    execute(ApuInstruction::plain(t)).map_err(FrameStepError::Clock)?;
                    work.next();
                }
                self.cycles += u32::from(t);
                if work.finished() {
                    self.stage = Stage::Tail(0);
                }
                return Ok(false);
            }
            Stage::Music(work) => {
                let mut accepted = 0;
                let clock = |op: ApuInstruction| {
                    let value = execute(op)?;
                    accepted = op.tcycles;
                    Ok(value)
                };
                let done = if self.physical_output {
                    work.try_step_with_bus(
                        &seq.channels[i].data,
                        self.panning.unwrap_or(self.start.panning),
                        self.start.voices[i].wave_instrument,
                        super::wave_patterns::for_bank(self.bank)
                            .expect("source frame validates its audio bank"),
                        clock,
                    )
                } else {
                    work.try_step(&seq.channels[i].data, |t| {
                        clock(ApuInstruction::plain(t)).map(|_| ())
                    })
                }
                .map_err(|e| match e {
                    StreamStepError::Clock(e) => FrameStepError::Clock(e),
                    StreamStepError::Input(e) => FrameStepError::Stream(e),
                })?;
                self.cycles += u32::from(accepted);
                if done {
                    let (tempo, fraction, panning, drum, ended) =
                        match work.result().expect("completed music body") {
                            MusicNoteOutcome::Note(n) => (
                                n.preparation.stream.tempo,
                                Some(n.preparation.fraction),
                                n.preparation.stream.panning,
                                n.preparation.stream.drum,
                                false,
                            ),
                            MusicNoteOutcome::End(n) => (
                                n.stream.tempo,
                                None,
                                n.stream.panning,
                                n.stream.drum,
                                n.ending.ended,
                            ),
                        };
                    if let Some(tempo) = tempo {
                        self.tempo = tempo;
                        self.fractions[..4].fill(0);
                    }
                    if let Some(fraction) = fraction {
                        self.fractions[i] = fraction;
                    }
                    if let Some(panning) = panning {
                        self.panning = Some(panning);
                    }
                    if ended {
                        self.ids[i] = 0;
                    }
                    if let Some(drum) = drum.filter(|drum| drum.started) {
                        self.started_drum = Some(drum.instrument);
                        self.ids[7] = drum.instrument;
                        self.fractions[7] = 0;
                    }
                    self.stage = Stage::Tail(0);
                }
                return Ok(false);
            }
            Stage::Press(work) => {
                let data = if i == 7 {
                    self.started_drum
                        .and_then(PressAbInstructions::noise_data)
                        .unwrap_or(&seq.channels[i].data)
                } else {
                    &seq.channels[i].data
                };
                let mut accepted = 0;
                let clock = |op: ApuInstruction| {
                    let value = execute(op)?;
                    accepted = op.tcycles;
                    Ok(value)
                };
                let done = if self.physical_output {
                    work.try_step_with_bus(data, clock)
                } else {
                    work.try_step(data, |t| clock(ApuInstruction::plain(t)).map(|_| ()))
                }
                .map_err(|e| match e {
                    PressAbStepError::Clock(e) => FrameStepError::Clock(e),
                    _ => FrameStepError::ChangedProgram,
                })?;
                self.cycles += u32::from(accepted);
                if done {
                    match work.result().expect("completed SFX body") {
                        PressAbOutcome::Note { fraction, .. } => self.fractions[i] = fraction,
                        PressAbOutcome::End { .. } => self.ids[i] = 0,
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
        execute(ApuInstruction::plain(t)).map_err(FrameStepError::Clock)?;
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
        // Assign each small dispatch variant in place. A shared match result
        // would carry the much larger note child through every plain opcode.
        match self.stage {
            Init => self.stage = Channel(0),
            Channel(n) if n < 5 => self.stage = Channel(n + 1),
            Channel(_) if self.ids[self.channel as usize] == 0 => self.stage = Tail(0),
            Channel(_) => self.stage = Kind(0),
            Kind(n) if n < 2 => self.stage = Kind(n + 1),
            Kind(_) if self.channel >= 4 => self.stage = Call,
            Kind(_) => self.stage = Mute(0),
            Mute(n) if n < 2 => self.stage = Mute(n + 1),
            Mute(_) => self.stage = Call,
            Call => {
                let i = self.channel as usize;
                if self.voice(i).delay > 1 {
                    self.stage = Effect(
                        EffectInstructionWork::with_pitch_slide(
                            Self::effect_input(&self.start, &self.ids, i),
                            Some(self.voice(i).pitch_slide),
                        )
                        .expect("validated effect"),
                    );
                } else {
                    self.stage = ReadyEntry(0);
                }
            }
            ReadyEntry(5) => self.stage = note.expect("preflighted note transfer"),
            ReadyEntry(n) => self.stage = ReadyEntry(n + 1),
            Tail(n) if n < 3 => self.stage = Tail(n + 1),
            Tail(_) if self.channel == 7 => self.stage = Return,
            Tail(_) => {
                self.channel += 1;
                self.stage = Channel(0);
            }
            Return | Done => self.stage = Done,
            Effect(_) | Music(_) | Press(_) => unreachable!("child progress"),
        }
    }
}

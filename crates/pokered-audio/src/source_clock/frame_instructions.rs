//! Retained Audio1/3_UpdateMusic CPU body over the one existing native sequencer.
//! Physical APU writes and IRQ epilogue remain outside this component.
use super::music_note_instructions::{MusicNoteInput, MusicNoteInstructions, MusicNoteOutcome};
use super::note_preparation::{PreparationInput, PreparationResult};
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
// Production guards read every byte in 8-byte words to avoid per-byte u64 multiplies on Thumb-1.
#[cfg(any(test, feature = "source-clock-serde"))]
fn program_fingerprint(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        fingerprint_multiply(hash ^ u64::from(byte))
    })
}

#[inline(always)]
fn fingerprint_multiply(value: u64) -> u64 {
    #[cfg(all(target_arch = "arm", target_pointer_width = "32", target_os = "none"))]
    {
        fnv_multiply_arm32(value)
    }
    #[cfg(not(all(target_arch = "arm", target_pointer_width = "32", target_os = "none")))]
    {
        value.wrapping_mul(0x0000_0100_0000_01b3)
    }
}

fn persisted_program_fingerprint(data: &[u8]) -> u64 {
    #[cfg(any(test, feature = "source-clock-serde"))]
    {
        program_fingerprint(data)
    }
    #[cfg(not(any(test, feature = "source-clock-serde")))]
    {
        let _ = data;
        0
    }
}

fn fast_program_fingerprint(data: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut chunks = data.chunks_exact(8);
    for chunk in &mut chunks {
        let word = u64::from_le_bytes(chunk.try_into().unwrap());
        hash = fingerprint_multiply(hash ^ word);
    }
    let mut tail = 0u64;
    for (index, &byte) in chunks.remainder().iter().enumerate() {
        tail |= u64::from(byte) << (index * 8);
    }
    hash = fingerprint_multiply(hash ^ tail);
    fingerprint_multiply(hash ^ data.len() as u64)
}

fn compact_program_fingerprint(data: &[u8]) -> u64 {
    const PRIME: u32 = 0x0100_0193;
    let mut low = 0xcbf2_9ce4u32;
    let mut high = 0x8422_2325u32;
    let mut chunks = data.chunks_exact(8);
    for chunk in &mut chunks {
        let a = u32::from_le_bytes(chunk[..4].try_into().unwrap());
        let b = u32::from_le_bytes(chunk[4..].try_into().unwrap());
        low = (low ^ a).wrapping_mul(PRIME);
        high = (high ^ b).rotate_left(11).wrapping_add(low);
    }
    let mut a = 0u32;
    let mut b = 0u32;
    for (index, &byte) in chunks.remainder().iter().enumerate() {
        if index < 4 {
            a |= u32::from(byte) << (index * 8);
        } else {
            b |= u32::from(byte) << ((index - 4) * 8);
        }
    }
    let len = data.len() as u32;
    low = (low ^ a ^ len).wrapping_mul(PRIME);
    high = (high ^ b ^ len.rotate_left(1))
        .rotate_left(11)
        .wrapping_add(low);
    (u64::from(high) << 32) | u64::from(low)
}

fn retained_fast_program_fingerprint(data: &[u8]) -> u64 {
    #[cfg(any(test, feature = "source-clock-serde"))]
    {
        fast_program_fingerprint(data)
    }
    #[cfg(not(any(test, feature = "source-clock-serde")))]
    {
        compact_program_fingerprint(data)
    }
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
    data_fingerprint: core::cell::Cell<u64>,
    #[cfg_attr(any(test, feature = "source-clock-serde"), serde(default))]
    fast_data_fingerprint: core::cell::Cell<u64>,
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
            data_fingerprint: core::cell::Cell::new(persisted_program_fingerprint(
                PressAbInstructions::noise_data(instrument).expect("canonical drum constructor"),
            )),
            fast_data_fingerprint: core::cell::Cell::new(retained_fast_program_fingerprint(
                PressAbInstructions::noise_data(instrument).expect("canonical drum constructor"),
            )),
        }
    }
}
#[derive(Clone, Debug)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
struct NativeStart {
    #[cfg_attr(any(test, feature = "source-clock-serde"), serde(default))]
    fingerprints_valid: core::cell::Cell<bool>,
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
impl PartialEq for NativeStart {
    fn eq(&self, other: &Self) -> bool {
        let voices_match = if self.fingerprints_valid.get() && other.fingerprints_valid.get() {
            self.voices == other.voices
        } else {
            self.voices.iter().zip(&other.voices).all(|(a, b)| {
                let a = a.clone();
                let b = b.clone();
                a.data_fingerprint.set(0);
                a.fast_data_fingerprint.set(0);
                b.data_fingerprint.set(0);
                b.fast_data_fingerprint.set(0);
                a == b
            })
        };
        voices_match
            && self.music_tempo == other.music_tempo
            && self.sfx_tempo == other.sfx_tempo
            && self.music_id == other.music_id
            && self.sfx_id == other.sfx_id
            && self.music_playing == other.music_playing
            && self.sfx_playing == other.sfx_playing
            && self.frequency_modifier == other.frequency_modifier
            && self.tempo_modifier == other.tempo_modifier
            && self.panning == other.panning
    }
}
impl Eq for NativeStart {}
impl NativeStart {
    fn read(seq: &Sequencer) -> Self {
        Self::read_inner(seq, false)
    }
    fn read_with_fingerprints(seq: &Sequencer) -> Self {
        Self::read_inner(seq, true)
    }
    fn read_inner(seq: &Sequencer, fingerprints: bool) -> Self {
        let mut voices: [Voice; 8] = core::array::from_fn(|i| {
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
                data_fingerprint: core::cell::Cell::new(0),
                fast_data_fingerprint: core::cell::Cell::new(0),
            }
        });
        if fingerprints {
            for (voice, channel) in voices.iter_mut().zip(&seq.channels) {
                voice
                    .data_fingerprint
                    .set(persisted_program_fingerprint(&channel.data));
                voice
                    .fast_data_fingerprint
                    .set(retained_fast_program_fingerprint(&channel.data));
            }
        }
        Self {
            fingerprints_valid: core::cell::Cell::new(fingerprints),
            voices,
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
#[derive(Clone, Debug)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct SourceAudioFrame {
    song: SourceSong,
    bank: u8,
    mute: u8,
    panning: Option<u8>,
    start: alloc::boxed::Box<NativeStart>,
    physical_output: bool,
    ids: [u8; 8],
    fractions: [u8; 8],
    #[cfg_attr(any(test, feature = "source-clock-serde"), serde(skip))]
    prepared_notes: [Option<PreparationResult>; 4],
    #[cfg_attr(any(test, feature = "source-clock-serde"), serde(skip, default))]
    prepared_no_note_cycles: Option<u32>,
    started_drum: Option<u8>,
    #[cfg_attr(any(test, feature = "source-clock-serde"), serde(skip))]
    started_voice: core::cell::OnceCell<Voice>,
    tempo: u16,
    channel: u8,
    stage: Stage,
    cycles: u32,
    pub(crate) committed: Option<Result<AudioFrameWork, AudioClockError>>,
    pub(crate) source_failed: Option<AudioClockError>,
}
impl PartialEq for SourceAudioFrame {
    fn eq(&self, other: &Self) -> bool {
        self.song == other.song
            && self.bank == other.bank
            && self.mute == other.mute
            && self.panning == other.panning
            && self.start == other.start
            && self.physical_output == other.physical_output
            && self.ids == other.ids
            && self.fractions == other.fractions
            && self.started_drum == other.started_drum
            && self.started_voice == other.started_voice
            && self.tempo == other.tempo
            && self.channel == other.channel
            && self.stage == other.stage
            && self.cycles == other.cycles
            && self.committed == other.committed
            && self.source_failed == other.source_failed
    }
}
impl Eq for SourceAudioFrame {}

impl SourceAudioFrame {
    pub fn new(
        seq: &Sequencer,
        song: SourceSong,
        bank: u8,
        mute: u8,
    ) -> Result<Self, AudioClockError> {
        Self::new_inner(seq, song, bank, mute, false, None)
    }
    fn new_inner(
        seq: &Sequencer,
        song: SourceSong,
        bank: u8,
        mute: u8,
        effects_validated: bool,
        prepared_ids: Option<[u8; 8]>,
    ) -> Result<Self, AudioClockError> {
        if bank != song.bank() || mute != 0 {
            return Err(AudioClockError::UnsupportedSoundBank);
        }
        if seq.channels[3].active && !effects_validated && !super::canonical_noise_table(seq) {
            return Err(AudioClockError::UnsupportedEffect(3));
        }
        let start = NativeStart::read(seq);
        let mut ids = prepared_ids.unwrap_or_else(|| {
            core::array::from_fn(|i| {
                if !start.voices[i].active {
                    0
                } else if i < 4 {
                    song.raw_id()
                } else if i == 4 || i == 5 {
                    super::press_ab_work::source_pulse_id(
                        i,
                        start.voices[i].id,
                        &seq.channels[i].data,
                    )
                    .unwrap_or(0)
                } else if i == 7 {
                    super::press_ab_work::source_noise_id(start.voices[i].id, &seq.channels[i].data)
                        .unwrap_or(0)
                } else {
                    start.voices[i].id
                }
            })
        });
        if !effects_validated {
            if start.voices[6].active {
                return Err(AudioClockError::ActiveSfx(6));
            }
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
                        4 | 5 => ids[i] != 0 && seq.sfx_tempo == 0x100,
                        7 => ids[i] != 0 && v.duty == 0,
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
        }
        let fractions = core::array::from_fn(|i| start.voices[i].fraction);
        let tempo = start.music_tempo;
        Ok(Self {
            song,
            bank,
            mute,
            panning: None,
            start: alloc::boxed::Box::new(start),
            physical_output: false,
            ids,
            fractions,
            prepared_notes: [None; 4],
            prepared_no_note_cycles: None,
            started_drum: None,
            started_voice: core::cell::OnceCell::new(),
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
        Self::with_bus(Self::new(seq, song, bank, mute)?, seq)
    }
    pub(crate) fn new_with_bus_prepared(
        seq: &Sequencer,
        song: SourceSong,
        bank: u8,
        mute: u8,
        prepared: &super::PreparedAudioFrame,
    ) -> Result<alloc::boxed::Box<Self>, AudioClockError> {
        let mut work = alloc::boxed::Box::new(Self::new_inner(
            seq,
            song,
            bank,
            mute,
            true,
            Some(prepared.ids()),
        )?);
        work.prepared_notes = core::array::from_fn(|i| prepared.note_preparation(i));
        work.prepared_no_note_cycles = prepared.no_note_cycles().filter(|_| {
            (0..8).all(|i| {
                if work.ids[i] == 0 {
                    return true;
                }
                let voice = work.voice(i);
                let cycles = EffectInstructionWork::with_pitch_slide(
                    Self::effect_input(&work.start, &work.ids, i),
                    Some(voice.pitch_slide),
                )
                .and_then(|effect| effect.no_apu_write_program());
                voice.delay > 1 && cycles.is_some()
            })
        });
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
    fn with_bus(mut work: Self, seq: &Sequencer) -> Result<Self, AudioClockError> {
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
    pub(crate) fn ensure_fingerprints(&self, seq: &Sequencer) {
        if !self.start.fingerprints_valid.get() {
            for (voice, channel) in self.start.voices.iter().zip(&seq.channels) {
                voice
                    .data_fingerprint
                    .set(persisted_program_fingerprint(&channel.data));
                voice
                    .fast_data_fingerprint
                    .set(retained_fast_program_fingerprint(&channel.data));
            }
            self.start.fingerprints_valid.set(true);
        }
    }
    pub fn matches_native(&self, seq: &Sequencer) -> bool {
        self.ensure_fingerprints(seq);
        *self.start == NativeStart::read_with_fingerprints(seq)
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
        let v = &start.voices[i];
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
        let input = MusicNoteInput {
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
        };
        let work = if let Some(preparation) = self.prepared_notes[i] {
            MusicNoteInstructions::with_preparation(input, preparation)
        } else {
            MusicNoteInstructions::with_drum_context(input, Some(self.ids[7] != 0))
        };
        Ok(Stage::Music(
            work.ok_or(AudioClockError::UnsupportedEffect(self.channel))?,
        ))
    }
    fn voice(&self, i: usize) -> &Voice {
        if i == 7 {
            if let Some(instrument) = self.started_drum {
                return self
                    .started_voice
                    .get_or_init(|| Voice::started_noise(instrument));
            }
        }
        &self.start.voices[i]
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
        self.step_inner::<false, false, _>(seq, |op| execute(op.tcycles).map(|()| None))
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
        self.step_inner::<true, false, _>(seq, execute)
    }
    fn no_note_program_cycles(&self) -> Option<u32> {
        if self.stage != Stage::Init {
            return None;
        }
        if let Some(cycles) = self.prepared_no_note_cycles {
            return Some(cycles);
        }
        let mut total = 8 + 16;
        for i in 0..8 {
            if self.ids[i] == 0 {
                total += if i == 0 { 80 } else { 76 };
                continue;
            }
            let voice = self.voice(i);
            if voice.delay <= 1 {
                return None;
            }
            let effect = EffectInstructionWork::with_pitch_slide(
                Self::effect_input(&self.start, &self.ids, i),
                Some(voice.pitch_slide),
            )?;
            let effect_cycles = effect.no_apu_write_program()?;
            total += if i == 0 {
                if i < 4 {
                    100
                } else {
                    72
                }
            } else if i < 4 {
                68
            } else {
                72
            };
            total += 24 + effect_cycles + if i == 7 { 24 } else { 28 };
        }
        Some(total)
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
        if let Some(cycles) = self.no_note_program_cycles() {
            if bus
                .execute_plain_interval(apu, cycles)
                .map_err(FrameStepError::Clock)?
            {
                self.cycles += cycles;
                self.channel = 8;
                self.stage = Stage::Done;
                return Ok(());
            }
        }
        while !self.finished() {
            if let Stage::Effect(work) = &mut self.stage {
                debug_assert!(!work.finished(), "pending effect");
                loop {
                    if let Some(cycles) = work.no_apu_write_program() {
                        if bus
                            .execute_plain_interval(apu, cycles)
                            .map_err(FrameStepError::Clock)?
                        {
                            self.cycles += cycles;
                            work.complete_no_apu_write();
                            break;
                        }
                    }
                    let prefix = work.plain_prefix();
                    if !prefix.is_empty() {
                        let count = bus
                            .execute_plain_run(apu, prefix)
                            .map_err(FrameStepError::Clock)?;
                        if count != 0 {
                            self.cycles += work.accept_plain_prefix(count, prefix);
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
                if let Stage::Music(work) = &mut self.stage {
                    let prefix = work.plain_prefix();
                    if !prefix.is_empty() {
                        let count = bus
                            .execute_plain_run(apu, prefix)
                            .map_err(FrameStepError::Clock)?;
                        if count != 0 {
                            self.cycles += work.accept_plain_prefix(count, prefix);
                            continue;
                        }
                    }
                } else if let Stage::Press(work) = &mut self.stage {
                    let prefix = work.plain_prefix();
                    if !prefix.is_empty() {
                        let count = bus
                            .execute_plain_run(apu, prefix)
                            .map_err(FrameStepError::Clock)?;
                        if count != 0 {
                            self.cycles += work.accept_plain_prefix(count, prefix);
                            continue;
                        }
                    }
                }
                let (prefix, total) = if matches!(self.stage, Stage::Music(_) | Stage::Press(_)) {
                    (&[] as &[u8], 0)
                } else {
                    self.dispatch_plain_prefix()
                };
                debug_assert_eq!(total, prefix.iter().map(|&t| u32::from(t)).sum::<u32>());
                if prefix.len() > 1 {
                    let count = bus
                        .execute_plain_run_total(apu, prefix, total)
                        .map_err(FrameStepError::Clock)?;
                    if count != 0 {
                        self.accept_dispatch_plain_prefix(count, prefix, total);
                        continue;
                    }
                }
                self.step_inner::<true, true, _>(seq, |op| bus.execute(apu, op))?;
            }
        }
        Ok(())
    }

    // These source blocks only inspect the frame's immutable channel/mute
    // input. Child construction and transfer stay on the checked step path.
    fn dispatch_plain_prefix(&self) -> (&'static [u8], u32) {
        use Stage::*;
        let (code, index, remaining): (&[u8], u8, u32) = match self.stage {
            Channel(0) if self.ids[self.channel as usize] == 0 && self.channel == 7 => {
                (&[8, 12, 8, 8, 4, 12, 4, 4, 8, 8], 0, 76)
            }
            Channel(0) if self.ids[self.channel as usize] == 0 => {
                (&[8, 12, 8, 8, 4, 12, 4, 4, 8, 12], 0, 80)
            }
            Channel(0) if self.channel < 4 => (&[8, 12, 8, 8, 4, 8, 4, 8, 8, 16, 4, 12], 0, 100),
            Channel(0) => (&[8, 12, 8, 8, 4, 8, 4, 8, 12], 0, 72),
            Channel(n) if self.ids[self.channel as usize] == 0 => (
                &[8, 12, 8, 8, 4, 12],
                n,
                [52, 44, 32, 24, 16, 12, 0][usize::from(n)],
            ),
            Channel(n) => (
                &[8, 12, 8, 8, 4, 8],
                n,
                [48, 40, 28, 20, 12, 8, 0][usize::from(n)],
            ),
            Kind(n) if self.channel >= 4 => (&[4, 8, 12], n, [24, 20, 12, 0][usize::from(n)]),
            Kind(n) => (&[4, 8, 8], n, [20, 16, 8, 0][usize::from(n)]),
            Mute(n) => (&[16, 4, 12], n, [32, 16, 12, 0][usize::from(n)]),
            ReadyEntry(n) if n < 5 => {
                (&[8, 12, 8, 8, 8], n, [44, 36, 24, 16, 8, 0][usize::from(n)])
            }
            Tail(n) if self.channel == 7 => (&[4, 4, 8, 8], n, [24, 20, 16, 8, 0][usize::from(n)]),
            Tail(n) => (&[4, 4, 8, 12], n, [28, 24, 20, 12, 0][usize::from(n)]),
            _ => return (&[], 0),
        };
        (&code[usize::from(index)..], remaining)
    }

    fn accept_dispatch_plain_prefix(&mut self, count: usize, prefix: &[u8], total: u32) {
        use Stage::*;
        debug_assert!(count > 0 && count <= prefix.len());
        self.cycles += if count == prefix.len() {
            total
        } else {
            let mut accepted = 0;
            for &cycles in &prefix[..count] {
                accepted += u32::from(cycles);
            }
            accepted
        };
        if matches!(self.stage, Channel(0)) && prefix.len() > 6 {
            if self.ids[self.channel as usize] == 0 {
                self.stage = if count <= 6 {
                    Channel((count - 1) as u8)
                } else {
                    Tail((count - 7) as u8)
                };
            } else if self.channel < 4 {
                self.stage = if count <= 6 {
                    Channel((count - 1) as u8)
                } else if count <= 9 {
                    Kind((count - 7) as u8)
                } else {
                    Mute((count - 10) as u8)
                };
            } else {
                self.stage = if count <= 6 {
                    Channel((count - 1) as u8)
                } else {
                    Kind((count - 7) as u8)
                };
            }
            self.advance(None);
            return;
        }
        if count == prefix.len() {
            match self.stage {
                Channel(_) => {
                    self.stage = if self.ids[self.channel as usize] == 0 {
                        Tail(0)
                    } else {
                        Kind(0)
                    };
                }
                Kind(_) => {
                    self.stage = if self.channel >= 4 { Call } else { Mute(0) };
                }
                Mute(_) => self.stage = Call,
                ReadyEntry(n) => {
                    self.stage = ReadyEntry(n + (count - 1) as u8);
                    self.advance(None);
                }
                Tail(_) => {
                    if self.channel == 7 {
                        self.stage = Return;
                    } else {
                        self.channel += 1;
                        self.stage = Channel(0);
                    }
                }
                _ => unreachable!("nonempty dispatch prefix"),
            }
            return;
        }
        match &mut self.stage {
            Channel(n) | Kind(n) | Mute(n) | ReadyEntry(n) | Tail(n) => *n += (count - 1) as u8,
            _ => unreachable!("nonempty dispatch prefix"),
        }
        self.advance(None);
    }
    fn step_inner<const PHYSICAL: bool, const FIXED: bool, E>(
        &mut self,
        seq: &Sequencer,
        execute: impl FnOnce(super::apu_bus::ApuInstruction) -> Result<Option<u8>, E>,
    ) -> Result<bool, FrameStepError<E>> {
        use super::apu_bus::ApuInstruction;
        let i = self.channel as usize;
        match &mut self.stage {
            Stage::Effect(work) => {
                let t = work.peek().expect("pending effect");
                if PHYSICAL {
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
                let done = if PHYSICAL {
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
                let done = if PHYSICAL {
                    if FIXED {
                        work.try_step_with_bus_fixed(data, clock)
                    } else {
                        work.try_step_with_bus(data, clock)
                    }
                } else if FIXED {
                    work.try_step_fixed(data, |t| clock(ApuInstruction::plain(t)).map(|_| ()))
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
        let preparation_cycles = match &note {
            Some(Stage::Music(work)) => work.cycles(),
            _ => 0,
        };
        execute(ApuInstruction::plain(t)).map_err(FrameStepError::Clock)?;
        self.advance(note);
        self.cycles += preparation_cycles + u32::from(t);
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
                let voice = self.voice(i);
                let delay = voice.delay;
                let pitch_slide = voice.pitch_slide;
                if delay > 1 {
                    self.stage = Effect(
                        EffectInstructionWork::with_pitch_slide(
                            Self::effect_input(&self.start, &self.ids, i),
                            Some(pitch_slide),
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

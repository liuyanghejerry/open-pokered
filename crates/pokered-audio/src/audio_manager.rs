//! pokered's audio manager: a thin game-specific layer over the generic
//! [`dotzuki_audio::manager::AudioManager`] (fade state machine, NR50 master
//! volume, cross-track resume states, VBlank orchestration — all sunk into
//! the engine).
//!
//! What stays here is the Pokémon wiring: the music/SFX track tables, the
//! 19 noise-instrument registrations, the cry pitch/tempo modifiers
//! (`wFrequencyModifier`/`wTempoModifier`), the low-health alarm with its
//! direct NR11–NR14 tone writes, and the alternate tempo/start routines
//! (audio/alternate_tempo.asm, audio/poke_flute.asm).

use core::ops::{Deref, DerefMut};

use alloc::boxed::Box;

use crate::music_data::{self, MusicId};
use crate::sfx_data::{self, SfxId};
use dotzuki_audio::manager::TrackData;
use dotzuki_audio::sequencer::{CHAN1, CHAN2, CHAN3, CHAN5, CHAN6, CHAN7};

pub use dotzuki_audio::manager::FadeState;

/// The generic engine manager (fade/master volume/resume states) this
/// wrapper delegates to — also reachable directly via `Deref`/`DerefMut`.
pub type EngineManager = dotzuki_audio::manager::AudioManager<MusicId, SfxId>;

fn music_track(id: MusicId) -> TrackData {
    let track = music_data::get_music_track(id);
    TrackData {
        sound_id: id as u8,
        channels: track.channels,
        tempo: track.tempo,
    }
}

fn sfx_track(id: SfxId) -> TrackData {
    let track = sfx_data::get_sfx_track(id);
    TrackData {
        sound_id: id as u8,
        channels: track.channels,
        tempo: 0, // SFX tempos are set per play call, not from the track.
    }
}

// ── Low-health alarm (audio/low_health_alarm.asm) ───────────────────────
// Tone data written straight to the pulse-1 hardware registers NR11–NR14
// (NR10 is always zeroed first), from the `alarm_tone` macro data at
// low_health_alarm.asm:64-80: length/duty, envelope, 16-bit frequency
// (lo byte = NR13, hi byte = NR14 with the trigger bit set).

/// `.toneDataHi` (low_health_alarm.asm:72-73): `alarm_tone $A0, $E2, $8750`.
const ALARM_TONE_HI: [u8; 4] = [0xA0, 0xE2, 0x50, 0x87];
/// `.toneDataLo` (low_health_alarm.asm:75-76): `alarm_tone $B0, $E2, $86EE`.
const ALARM_TONE_LO: [u8; 4] = [0xB0, 0xE2, 0xEE, 0x86];
/// `.toneDataSilence` (low_health_alarm.asm:79-80): `alarm_tone $00, $00, $8000`.
const ALARM_TONE_SILENCE: [u8; 4] = [0x00, 0x00, 0x00, 0x80];

pub struct AudioManager {
    /// The generic engine manager: fade state machine, NR50 master volume,
    /// saved/resume music states, SFX playback with cry modifiers. All the
    /// engine methods and the `sequencer`/`apu` fields are reachable
    /// directly through `Deref`.
    pub engine: EngineManager,

    /// `wLowHealthAlarm` bit 7 (`BIT_LOW_HEALTH_ALARM`): alarm enabled.
    low_health_alarm_enabled: bool,
    /// `wLowHealthAlarm` bits 0-6 (`LOW_HEALTH_TIMER_MASK`): tone timer.
    low_health_timer: u8,
    /// Overworld ItemUsePokeFlute stops map music, waits for the channel-3
    /// jingle to end, then starts the map music again.
    music_after_sfx: Option<MusicId>,
}

impl Deref for AudioManager {
    type Target = EngineManager;

    fn deref(&self) -> &EngineManager {
        &self.engine
    }
}

impl DerefMut for AudioManager {
    fn deref_mut(&mut self) -> &mut EngineManager {
        &mut self.engine
    }
}

impl AudioManager {
    pub fn new() -> Self {
        let mut engine = EngineManager::new(music_track, sfx_track);
        // Register the 19 noise-instrument (drum) streams with the sequencer
        // so music `drum_note` commands can trigger them on CHAN8
        // (audio/engine_1.asm:673-688). Instrument N (1-19) → SfxId N-1.
        engine.sequencer.register_noise_instruments(
            (0..19)
                .filter_map(sfx_data::SfxId::from_u8)
                .filter_map(|id| sfx_data::get_sfx_track(id).channels[3])
                .map(|stream| stream.to_vec())
                .collect(),
        );
        Self {
            engine,
            low_health_alarm_enabled: false,
            low_health_timer: 0,
            music_after_sfx: None,
        }
    }

    pub fn play_sfx(&mut self, id: SfxId) {
        if self.alarm_suppresses_sfx(id) {
            return;
        }
        self.engine.play_sfx(id);
    }

    /// GetMoveSound's command-table modifiers in the battle audio bank.
    /// Audio2 accepts them for battle SFX as well as cries. CHAN8 noise
    /// notes retain fixed tempo and noise parameters in the sequencer.
    pub fn play_move_sfx(&mut self, id: SfxId, pitch_mod: u8, tempo_mod: u8) {
        if self.alarm_suppresses_sfx(id) {
            return;
        }
        if (SfxId::Peck as u8..=SfxId::SilphScope as u8).contains(&(id as u8)) {
            self.engine
                .play_sfx_with_modifiers(id, pitch_mod as i16, 0x80 + tempo_mod as u16);
        } else {
            self.engine.play_sfx(id);
        }
    }

    /// Cerulean/Saffron/Cinnabar badge text selects GET_KEY_ITEM in bank 2,
    /// which resolves to only BALL_POOF's second (noise) channel in Red.
    pub fn play_badge_bank_quirk(&mut self) {
        let noise = sfx_data::get_sfx_track(SfxId::BallPoof).channels[3].unwrap();
        self.sequencer.frequency_modifier = 0;
        self.sequencer
            .play_sfx(SfxId::BallPoof as u8, &[noise.to_vec()], 3, 0x0100);
    }

    /// Audio2's PlayApplyingAttackSound: select from the original presentation
    /// byte, then use $80 + wTempoModifier. These are battle SFX, which Audio2
    /// explicitly permits to use modifiers (unlike the Audio1/3 plain-SFX path).
    pub fn play_applying_attack_sound(&mut self, effectiveness: u8) {
        let factor = effectiveness & 0x7f;
        if factor == 0 {
            return;
        }
        let (id, pitch, tempo) = if factor == 10 {
            (SfxId::Damage, 0x20, 0x30)
        } else if factor > 10 {
            (SfxId::SuperEffective, 0xe0, 0xff)
        } else {
            (SfxId::NotVeryEffective, 0x50, 0x01)
        };
        if !self.alarm_suppresses_sfx(id) {
            self.engine.play_sfx_with_modifiers(id, pitch, 0x80 + tempo);
        }
    }

    /// Play a cry with the original engine's cry modifiers
    /// (`wFrequencyModifier`/`wTempoModifier`, home/pokemon.asm `GetCryData`).
    ///
    /// Audio1/3 gate these on IsCry; Audio2 additionally supports battle
    /// SFX (see play_applying_attack_sound). Cry tempo is $80 + tempo_mod;
    /// the plain play_sfx API uses $100 with no requested pitch modifier.
    pub fn play_cry(&mut self, id: SfxId, pitch_mod: u8, tempo_mod: u8) {
        if self.alarm_suppresses_sfx(id) {
            return;
        }
        self.engine.play_sfx_with_modifiers(
            id,
            pitch_mod as i16,
            0x0080u16.wrapping_add(tempo_mod as u16),
        );
    }

    /// While the low-health alarm is active, SFX channel 5 (pulse 1)
    /// processing is suppressed so it cannot fight the alarm's direct
    /// register writes (audio/engine_2.asm:162-170).
    fn alarm_suppresses_sfx(&self, id: SfxId) -> bool {
        self.low_health_alarm_enabled && self.engine.sfx_start_channel(id) == Some(0)
    }

    /// `Music_PokeFluteInBattle` (audio/poke_flute.asm:1-12): begin playing
    /// SFX_CAUGHT_MON, then immediately overwrite the command pointers of SFX
    /// channels 5-7 with the in-battle flute streams
    /// (`Audio2_OverwriteChannelPointer`). The overwrite lands before the next
    /// sequencer tick, so the caught-mon data never executes — the PlaySound
    /// call exists to initialize the three channels; only the flute melody
    /// sounds. When the streams end, the battle music resumes on those
    /// channels (the music channels kept running underneath).
    ///
    /// Skipped entirely while the low-health alarm is active, mirroring the
    /// guard at the original call site (engine/items/item_effects.asm:1735-1737).
    pub fn play_flute_in_battle(&mut self) {
        if self.low_health_alarm_enabled {
            return;
        }
        self.play_sfx(SfxId::CaughtMon);
        self.sequencer
            .override_channel_stream(CHAN5, sfx_data::POKEFLUTE_IN_BATTLE_CH5);
        self.sequencer
            .override_channel_stream(CHAN6, sfx_data::POKEFLUTE_IN_BATTLE_CH6);
        self.sequencer
            .override_channel_stream(CHAN7, sfx_data::POKEFLUTE_IN_BATTLE_CH7);
    }

    pub fn play_flute_overworld(&mut self, resume_music: MusicId) {
        self.engine.stop_music();
        self.play_sfx(SfxId::Pokeflute);
        self.music_after_sfx = Some(resume_music);
    }

    /// `Music_RivalAlternateStart` (audio/alternate_tempo.asm:2-12): play
    /// MUSIC_MEET_RIVAL, then overwrite the command pointers of music channels
    /// 1-3 with the alternate-start streams (`Audio1_OverwriteChannelPointer`,
    /// alternate_tempo.asm:13-18) — the rival theme with a different first
    /// measure. Used at rival encounters (OaksLab, Route22, CeruleanCity,
    /// PokemonTower2F, SilphCo7F, SSAnne2F).
    ///
    /// The saved-resume-state for MEET_RIVAL is discarded first: the original
    /// `PlayMusic` always restarts the song, and the whole point of the
    /// alternate start is its opening measure.
    pub fn play_meet_rival_alternate_start(&mut self) {
        self.discard_saved_music_state(MusicId::MEET_RIVAL);
        self.play_music(MusicId::MEET_RIVAL);
        self.sequencer
            .override_channel_stream(CHAN1, music_data::MEETRIVAL_CH1_ALTERNATE_START);
        self.sequencer
            .override_channel_stream(CHAN2, music_data::MEETRIVAL_CH2_ALTERNATE_START);
        self.sequencer
            .override_channel_stream(CHAN3, music_data::MEETRIVAL_CH3_ALTERNATE_START);
    }

    /// `Music_RivalAlternateTempo` (audio/alternate_tempo.asm:21-27): play
    /// MUSIC_MEET_RIVAL, then overwrite channel 1's pointer with the
    /// alternate-tempo stream (tempo 100 instead of 112). Used for the second
    /// Route22 rival battle approach (scripts/Route22.asm:252).
    pub fn play_meet_rival_alternate_tempo(&mut self) {
        self.discard_saved_music_state(MusicId::MEET_RIVAL);
        self.play_music(MusicId::MEET_RIVAL);
        self.sequencer
            .override_channel_stream(CHAN1, music_data::MEETRIVAL_CH1_ALTERNATE_TEMPO);
    }

    /// `Music_RivalAlternateStartAndTempo` (audio/alternate_tempo.asm:30-34):
    /// the alternate start on channels 1-3, then channel 1's pointer is
    /// overwritten *again* with the start+tempo stream. Used after the second
    /// Route22 rival battle (scripts/Route22.asm:333).
    pub fn play_meet_rival_alternate_start_and_tempo(&mut self) {
        self.play_meet_rival_alternate_start();
        self.sequencer
            .override_channel_stream(CHAN1, music_data::MEETRIVAL_CH1_ALTERNATE_START_AND_TEMPO);
    }

    /// `Music_Cities1AlternateTempo` (audio/alternate_tempo.asm:37-50): fade
    /// out the current music (reload value 10, stop after the fade), wait for
    /// the fade to finish, then restart MUSIC_CITIES1 with channel 1's pointer
    /// overwritten by the slower alternate-tempo stream (tempo 232 instead of
    /// 144). Used when Prof. Oak enters the Hall of Fame room
    /// (scripts/ChampionsRoom.asm:112).
    ///
    /// The original blocks for 100 frames with `DelayFrames`; here the
    /// engine's fade machinery queues the restart, and the fade-complete hook
    /// pokes the channel-1 override right after the new song starts.
    pub fn play_cities1_alternate_tempo(&mut self) {
        self.discard_saved_music_state(MusicId::CITIES1);
        self.fade_out_then_play(
            MusicId::CITIES1,
            10,
            Some(Box::new(|seq| {
                seq.override_channel_stream(CHAN1, music_data::CITIES1_CH1_ALTERNATE_TEMPO);
            })),
        );
    }

    /// Handle a script `playMusic(...)` string for the alternate tempo/start
    /// variants (audio/alternate_tempo.asm). Returns `true` if `name` was one
    /// of the special variant IDs and the corresponding routine was run;
    /// `false` for ordinary music IDs, which the caller maps to `MusicId`.
    ///
    /// Scenes use these at the original `farcall Music_*Alternate*` sites
    /// (e.g. scripts/Route22.asm:174, scripts/ChampionsRoom.asm:112) — the
    /// pointer-overwrite happens in the same call, exactly like the original.
    pub fn play_script_music(&mut self, name: &str) -> bool {
        match name {
            "MUSIC_MEET_RIVAL_ALTERNATE_START" => self.play_meet_rival_alternate_start(),
            "MUSIC_MEET_RIVAL_ALTERNATE_TEMPO" => self.play_meet_rival_alternate_tempo(),
            "MUSIC_MEET_RIVAL_ALTERNATE_START_AND_TEMPO" => {
                self.play_meet_rival_alternate_start_and_tempo()
            }
            "MUSIC_CITIES1_ALTERNATE_TEMPO" => self.play_cities1_alternate_tempo(),
            _ => return false,
        }
        true
    }

    fn prepare_source_frame(
        &self,
    ) -> Result<crate::source_clock::PreparedAudioFrame, crate::source_clock::AudioClockError> {
        use crate::source_clock::{AudioClockError, PreparedAudioFrame};
        if matches!(
            self.engine.last_music_id(),
            Some(MusicId::SILPH_CO | MusicId::DUNGEON3 | MusicId::POKECENTER)
        ) && !self.engine.is_fading()
            && !self.low_health_alarm_enabled
        {
            PreparedAudioFrame::from_shared_music(&self.engine.sequencer)
        } else {
            Err(AudioClockError::UnsupportedSoundBank)
        }
    }

    /// Advance the existing sequencer exactly once and report supported
    /// source work. Unsupported music/effects still receive that same tick;
    /// they never acquire an invented IRQ duration or a second update.
    pub fn update_frame_with_source_work(
        &mut self,
    ) -> Result<crate::source_clock::AudioFrameWork, crate::source_clock::AudioClockError> {
        let prepared = self.prepare_source_frame();
        self.update_frame();
        prepared?.finish(&self.engine.sequencer)
    }

    /// Inspect supported work without mutation, obtain the owner's admission,
    /// then advance the one existing manager exactly once. The exclusive borrow
    /// keeps music/SFX state fixed between preparation and commit.
    ///
    /// Preflight/Admission errors advance nothing. Advanced means a native tick
    /// already occurred, including a failed postcondition; the owner must retain
    /// that outcome rather than retry. This API does not clock source instructions,
    /// reserve a frame, model an IRQ or make aggregate cycles resumable. The
    /// caller still owns instruction progress and IRQ/foreground exclusion.
    pub fn try_update_frame_with_source_work<E>(
        &mut self,
        admit: impl FnOnce(crate::source_clock::AudioFrameWork) -> Result<(), E>,
    ) -> Result<crate::source_clock::AudioFrameWork, crate::source_clock::AudioAdmissionError<E>>
    {
        use crate::source_clock::AudioAdmissionError;
        let prepared = self
            .prepare_source_frame()
            .map_err(AudioAdmissionError::Preflight)?;
        admit(prepared.work()).map_err(AudioAdmissionError::Admission)?;
        self.update_frame();
        prepared
            .finish(&self.engine.sequencer)
            .map_err(AudioAdmissionError::Advanced)
    }

    fn source_timer_context(
        &self,
        bank: u8,
        mute: u8,
    ) -> Result<
        crate::source_clock::timer_work::TimerFrameContext,
        crate::source_clock::AudioClockError,
    > {
        use crate::source_clock::{
            timer_work::{ChannelEffectWorkInput, TimerFrameContext},
            AudioClockError,
        };
        self.prepare_source_frame()?;
        let expected_bank = match self.engine.last_music_id() {
            Some(MusicId::POKECENTER) => 2,
            Some(MusicId::SILPH_CO | MusicId::DUNGEON3) => 31,
            _ => return Err(AudioClockError::UnsupportedSoundBank),
        };
        if bank != expected_bank || mute != 0 || self.music_after_sfx.is_some() {
            return Err(AudioClockError::UnsupportedSoundBank);
        }
        let seq = &self.engine.sequencer;
        let context = TimerFrameContext {
            bank,
            mute,
            sound_ids: core::array::from_fn(|i| {
                if seq.channels[i].active {
                    seq.channels[i].sound_id
                } else {
                    0
                }
            }),
            effects: core::array::from_fn(|i| {
                let ch = &seq.channels[i];
                ch.active.then(|| ChannelEffectWorkInput {
                    channel: i as u8,
                    paired_sfx_active: i < 4 && seq.channels[i + 4].active,
                    note_delay: ch.delay_counter,
                    flags1: ch.flags1.bits(),
                    flags2: ch.flags2.bits(),
                    vibrato_delay: ch.vibrato.delay_counter,
                    vibrato_extent: ch.vibrato.extent,
                    vibrato_rate: ch.vibrato.rate,
                    frequency_low: ch.freq_lo_saved,
                })
            }),
        };
        Ok(context)
    }

    /// Prepare retained Audio1/3 timer instructions from the one native
    /// sequencer. The interrupt owner supplies its actual bank/mute bytes.
    /// Next-note frames remain unsupported by this cursor, rather than
    /// silently receiving aggregate work or an inactive replacement channel.
    pub fn begin_source_timer_frame(
        &self,
        bank: u8,
        mute: u8,
    ) -> Result<crate::source_clock::timer_work::TimerFrameWork, crate::source_clock::AudioClockError>
    {
        crate::source_clock::timer_work::TimerFrameWork::new(self.source_timer_context(bank, mute)?)
    }

    /// Accept at most one complete CPU instruction, then commit the existing
    /// native manager exactly once after RET. A refusal must occur before the
    /// callback advances its clock. Accepted instructions are never rolled back.
    /// Keep this cursor through IRQ suspension and do not run an ordinary audio
    /// update in parallel. APU write timestamps are not supplied by this API.
    pub fn step_source_timer_frame<E>(
        &mut self,
        work: &mut crate::source_clock::timer_work::TimerFrameWork,
        execute: impl FnOnce(u8) -> Result<(), E>,
    ) -> Result<
        Option<crate::source_clock::AudioFrameWork>,
        crate::source_clock::AudioAdmissionError<E>,
    > {
        use crate::source_clock::{AudioAdmissionError, AudioClockError};
        if let Some(result) = work.committed {
            return result.map(Some).map_err(AudioAdmissionError::Advanced);
        }
        let context = self
            .source_timer_context(work.context().bank, work.context().mute)
            .map_err(AudioAdmissionError::Preflight)?;
        if &context != work.context() {
            return Err(AudioAdmissionError::Preflight(
                AudioClockError::NativeStateMismatch(0),
            ));
        }
        let prepared = self
            .prepare_source_frame()
            .map_err(AudioAdmissionError::Preflight)?;
        if !work
            .try_step(execute)
            .map_err(AudioAdmissionError::Admission)?
        {
            return Ok(None);
        }
        if work.cycles() != prepared.work().cycles {
            return Err(AudioAdmissionError::Preflight(
                AudioClockError::NativeStateMismatch(0),
            ));
        }
        self.update_frame();
        let result = prepared.finish(&self.engine.sequencer);
        work.committed = Some(result);
        result.map(Some).map_err(AudioAdmissionError::Advanced)
    }

    fn source_frame_song(
        &self,
        bank: u8,
        mute: u8,
    ) -> Result<
        crate::source_clock::frame_instructions::SourceSong,
        crate::source_clock::AudioClockError,
    > {
        use crate::source_clock::{frame_instructions::SourceSong, AudioClockError};
        let song = self
            .engine
            .last_music_id()
            .and_then(SourceSong::from_music)
            .ok_or(AudioClockError::UnsupportedSoundBank)?;
        if bank != song.bank()
            || mute != 0
            || self.engine.is_fading()
            || self.low_health_alarm_enabled
            || self.music_after_sfx.is_some()
        {
            return Err(AudioClockError::UnsupportedSoundBank);
        }
        Ok(song)
    }

    /// Inspect one existing native sequencer. Actual source bank/mute must be
    /// supplied by the interrupt owner; no inferred default source byte is used.
    pub fn begin_source_audio_frame(
        &self,
        bank: u8,
        mute: u8,
    ) -> Result<
        crate::source_clock::frame_instructions::SourceAudioFrame,
        crate::source_clock::AudioClockError,
    > {
        let song = self.source_frame_song(bank, mute)?;
        self.prepare_source_frame()?;
        crate::source_clock::frame_instructions::SourceAudioFrame::new(
            &self.engine.sequencer,
            song,
            bank,
            mute,
        )
    }

    /// Resume real source instructions under this exclusive borrow until the
    /// owner refuses one, or the full source RET completes. Accepted CPU time
    /// is not rolled back. Native audio commits once after that RET, and its
    /// outcome is retained. The callback must refuse before clocking and must
    /// not reenter the same manager/output. Physical APU writes and IRQ return
    /// are not scheduled by this API; the caller owns IRQ/foreground exclusion.
    pub fn resume_source_audio_frame<E>(
        &mut self,
        work: &mut crate::source_clock::frame_instructions::SourceAudioFrame,
        mut execute: impl FnMut(u8) -> Result<(), E>,
    ) -> Result<
        crate::source_clock::AudioFrameWork,
        crate::source_clock::AudioAdmissionError<
            crate::source_clock::frame_instructions::FrameStepError<E>,
        >,
    > {
        use crate::source_clock::{AudioAdmissionError, AudioClockError, AudioFrameWork};
        if let Some(result) = work.committed {
            return result.map_err(AudioAdmissionError::Advanced);
        }
        let song = self
            .source_frame_song(work.bank(), work.mute())
            .map_err(AudioAdmissionError::Preflight)?;
        if song != work.song() || !work.matches_native(&self.engine.sequencer) {
            return Err(AudioAdmissionError::Preflight(
                AudioClockError::NativeStateMismatch(0),
            ));
        }
        let prepared = self
            .prepare_source_frame()
            .map_err(AudioAdmissionError::Preflight)?;
        while !work.finished() {
            work.try_step(&self.engine.sequencer, &mut execute)
                .map_err(AudioAdmissionError::Admission)?;
        }
        if prepared.work()
            != (AudioFrameWork {
                cycles: work.cycles(),
            })
        {
            return Err(AudioAdmissionError::Preflight(
                AudioClockError::NativeStateMismatch(0),
            ));
        }
        self.update_frame();
        let result = prepared.finish(&self.engine.sequencer);
        work.committed = Some(result);
        result.map_err(AudioAdmissionError::Advanced)
    }

    /// Call once per VBlank (~60 Hz). The engine advances fade → master
    /// volume → sequencer; the alarm then writes pulse-1 registers *after*
    /// the sequencer so it overrides channel 1, exactly as the original's
    /// direct hardware writes override whatever the sound engine put there.
    pub fn update_frame(&mut self) {
        self.engine.update_frame();
        if self.music_after_sfx.is_some() && !self.engine.is_sfx_playing() {
            let music = self.music_after_sfx.take().unwrap();
            self.engine.play_music(music);
        }
        if self.low_health_alarm_enabled {
            self.tick_low_health_alarm();
        }
    }

    /// Enable/disable the low-health alarm (`wLowHealthAlarm`,
    /// audio/low_health_alarm.asm). Driven by the battle UI from
    /// `BattleScreen::low_health_alarm` — the original sets bit 7 when the
    /// player mon's HP bar is red and clears it on faint/heal/battle end.
    pub fn set_low_health_alarm(&mut self, enable: bool) {
        if enable == self.low_health_alarm_enabled {
            return;
        }
        self.low_health_alarm_enabled = enable;
        if enable {
            // A freshly written wLowHealthAlarm has timer 0, so the first
            // enabled VBlank plays the hi tone (low_health_alarm.asm:9-14).
            self.low_health_timer = 0;
        } else {
            // `.disableAlarm` + `.toneDataSilence` (low_health_alarm.asm:34-39).
            self.write_alarm_tone(&ALARM_TONE_SILENCE);
        }
    }

    /// Whether the low-health alarm bit is set — the original's
    /// `WaitForSoundToFinish` returns immediately while it is
    /// (home/delay.asm:15-18), so frontends mirror that with this.
    pub fn low_health_alarm_active(&self) -> bool {
        self.low_health_alarm_enabled
    }

    /// `Music_DoLowHealthAlarm` (low_health_alarm.asm:9-32): a 30-frame
    /// cycle — timer 0 plays the hi tone and reloads 30, timer 20 plays
    /// the lo tone, otherwise just count down.
    fn tick_low_health_alarm(&mut self) {
        if self.low_health_timer == 0 {
            self.write_alarm_tone(&ALARM_TONE_HI);
            self.low_health_timer = 30;
        } else {
            if self.low_health_timer == 20 {
                self.write_alarm_tone(&ALARM_TONE_LO);
            }
            self.low_health_timer -= 1;
        }
    }

    /// `.playTone` (low_health_alarm.asm:50-62): NR10 is zeroed, then the
    /// 4 tone bytes go to NR11–NR14, overriding all other channel-1 sound.
    fn write_alarm_tone(&mut self, tone: &[u8; 4]) {
        self.apu.write_register(0xFF10, 0);
        for (i, &b) in tone.iter().enumerate() {
            self.apu.write_register(0xFF11 + i as u16, b);
        }
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, not(target_os = "none")))]
mod source_admission_tests;

#[cfg(all(test, not(target_os = "none")))]
mod source_timer_tests;

#[cfg(all(test, not(target_os = "none")))]
mod source_frame_tests;

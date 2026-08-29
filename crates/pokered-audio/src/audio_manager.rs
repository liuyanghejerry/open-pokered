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

use std::ops::{Deref, DerefMut};

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
        }
    }

    pub fn play_sfx(&mut self, id: SfxId) {
        if self.alarm_suppresses_sfx(id) {
            return;
        }
        self.engine.play_sfx(id);
    }

    /// Play a cry with the original engine's cry modifiers
    /// (`wFrequencyModifier`/`wTempoModifier`, home/pokemon.asm `GetCryData`).
    ///
    /// In the disassembly these only take effect for cries
    /// (`Audio1_ApplyFrequencyModifier` / `Audio1_SetSfxTempo` gate on
    /// `Audio1_IsCry`): the cry's tempo is `0x0080 + tempo_mod` (plain SFX
    /// always run at `0x0100`) and `pitch_mod` is added to every note
    /// frequency.
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
        self.sequencer.override_channel_stream(
            CHAN1,
            music_data::MEETRIVAL_CH1_ALTERNATE_START_AND_TEMPO,
        );
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

    /// Call once per VBlank (~60 Hz). The engine advances fade → master
    /// volume → sequencer; the alarm then writes pulse-1 registers *after*
    /// the sequencer so it overrides channel 1, exactly as the original's
    /// direct hardware writes override whatever the sound engine put there.
    pub fn update_frame(&mut self) {
        self.engine.update_frame();
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

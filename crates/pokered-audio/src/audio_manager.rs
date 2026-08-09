use std::collections::HashMap;

use crate::music_data::{self, MusicId};
use crate::sfx_data::{self, SfxId};
use dotzuki_audio::apu::Apu;
use dotzuki_audio::sequencer::Sequencer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeState {
    None,
    FadingOut,
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
    pub sequencer: Sequencer,
    pub apu: Apu,

    master_volume_left: u8,
    master_volume_right: u8,

    pub(crate) fade_state: FadeState,
    pub(crate) fade_counter: u8,
    pub(crate) fade_counter_reload: u8,
    pub(crate) fade_queued_music: Option<MusicId>,
    /// CHAN1 stream to poke in after a fade-queued music restart
    /// (`Music_Cities1AlternateTempo`, audio/alternate_tempo.asm:48-50).
    pub(crate) fade_queued_ch1_override: Option<&'static [u8]>,

    no_audio_fade_out: bool,

    last_music_id: Option<MusicId>,

    saved_music_states: HashMap<MusicId, Sequencer>,

    /// `wLowHealthAlarm` bit 7 (`BIT_LOW_HEALTH_ALARM`): alarm enabled.
    low_health_alarm_enabled: bool,
    /// `wLowHealthAlarm` bits 0-6 (`LOW_HEALTH_TIMER_MASK`): tone timer.
    low_health_timer: u8,
}

impl AudioManager {
    pub fn new() -> Self {
        let mut sequencer = Sequencer::new();
        // Register the 19 noise-instrument (drum) streams with the sequencer
        // so music `drum_note` commands can trigger them on CHAN8
        // (audio/engine_1.asm:673-688). Instrument N (1-19) → SfxId N-1.
        sequencer.register_noise_instruments(
            (0..19)
                .filter_map(sfx_data::SfxId::from_u8)
                .filter_map(|id| sfx_data::get_sfx_track(id).channels[3])
                .map(|stream| stream.to_vec())
                .collect(),
        );
        Self {
            sequencer,
            apu: Apu::new(),
            master_volume_left: 7,
            master_volume_right: 7,
            fade_state: FadeState::None,
            fade_counter: 0,
            fade_counter_reload: 0,
            fade_queued_music: None,
            fade_queued_ch1_override: None,
            no_audio_fade_out: false,
            last_music_id: None,
            saved_music_states: HashMap::new(),
            low_health_alarm_enabled: false,
            low_health_timer: 0,
        }
    }

    pub fn master_volume_left(&self) -> u8 {
        self.master_volume_left
    }

    pub fn master_volume_right(&self) -> u8 {
        self.master_volume_right
    }

    pub fn set_master_volume(&mut self, left: u8, right: u8) {
        self.master_volume_left = left.min(7);
        self.master_volume_right = right.min(7);
        self.apply_master_volume();
    }

    pub fn fade_state(&self) -> FadeState {
        self.fade_state
    }

    pub fn last_music_id(&self) -> Option<MusicId> {
        self.last_music_id
    }

    pub fn set_no_audio_fade_out(&mut self, val: bool) {
        self.no_audio_fade_out = val;
    }

    pub fn no_audio_fade_out(&self) -> bool {
        self.no_audio_fade_out
    }

    pub fn play_music(&mut self, id: MusicId) {
        // If switching away from a different playing track, save its state
        if self.sequencer.music_playing {
            if let Some(last_id) = self.last_music_id {
                if last_id != id {
                    self.saved_music_states
                        .insert(last_id, self.sequencer.clone());
                }
            }
        }

        // If we have a saved resume state for this track, restore it
        if let Some(saved) = self.saved_music_states.remove(&id) {
            self.sequencer.restore_music_from(&saved);
            self.last_music_id = Some(id);
            self.fade_state = FadeState::None;
            self.fade_queued_music = None;
            self.master_volume_left = 7;
            self.master_volume_right = 7;
            self.apply_master_volume();
            return;
        }

        self.fade_state = FadeState::None;
        self.fade_queued_music = None;
        self.last_music_id = Some(id);

        let track = music_data::get_music_track(id);
        let mut channel_data = Vec::new();
        for ch_opt in &track.channels {
            if let Some(data) = ch_opt {
                channel_data.push(data.to_vec());
            }
        }
        self.sequencer
            .play_music(id as u8, &channel_data, track.tempo);
        self.master_volume_left = 7;
        self.master_volume_right = 7;
        self.apply_master_volume();
    }

    pub fn play_music_with_fade(&mut self, id: MusicId, fade_speed: u8) {
        if self.last_music_id == Some(id) {
            return;
        }

        if !self.sequencer.music_playing {
            self.play_music(id);
            return;
        }

        self.fade_state = FadeState::FadingOut;
        self.fade_counter = fade_speed;
        self.fade_counter_reload = fade_speed;
        self.fade_queued_music = Some(id);
    }

    pub fn fade_out(&mut self, fade_speed: u8) {
        if !self.sequencer.music_playing {
            return;
        }

        self.fade_state = FadeState::FadingOut;
        self.fade_counter = fade_speed;
        self.fade_counter_reload = fade_speed;
        self.fade_queued_music = None;
    }

    pub fn play_sfx(&mut self, id: SfxId) {
        self.play_sfx_internal(id, 0, 0x0100);
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
        self.play_sfx_internal(id, pitch_mod, 0x0080u16.wrapping_add(tempo_mod as u16));
    }

    fn play_sfx_internal(&mut self, id: SfxId, pitch_mod: u8, tempo: u16) {
        let track = sfx_data::get_sfx_track(id);
        let mut channel_data = Vec::new();
        let mut start_channel = 0usize;
        let mut found_first = false;

        for (hw_idx, ch_opt) in track.channels.iter().enumerate() {
            if let Some(data) = ch_opt {
                if !found_first {
                    start_channel = hw_idx;
                    found_first = true;
                }
                channel_data.push(data.to_vec());
            }
        }

        // While the low-health alarm is active, SFX channel 5 (pulse 1)
        // processing is suppressed so it cannot fight the alarm's direct
        // register writes (audio/engine_2.asm:162-170).
        if self.low_health_alarm_enabled && start_channel == 0 {
            return;
        }

        if !channel_data.is_empty() {
            self.sequencer.frequency_modifier = pitch_mod as i16;
            self.sequencer
                .play_sfx(id as u8, &channel_data, start_channel, tempo);
        }
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
        self.sequencer.override_channel_stream(
            dotzuki_audio::sequencer::CHAN5,
            sfx_data::POKEFLUTE_IN_BATTLE_CH5,
        );
        self.sequencer.override_channel_stream(
            dotzuki_audio::sequencer::CHAN6,
            sfx_data::POKEFLUTE_IN_BATTLE_CH6,
        );
        self.sequencer.override_channel_stream(
            dotzuki_audio::sequencer::CHAN7,
            sfx_data::POKEFLUTE_IN_BATTLE_CH7,
        );
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
        self.saved_music_states.remove(&MusicId::MEET_RIVAL);
        self.play_music(MusicId::MEET_RIVAL);
        self.sequencer.override_channel_stream(
            dotzuki_audio::sequencer::CHAN1,
            music_data::MEETRIVAL_CH1_ALTERNATE_START,
        );
        self.sequencer.override_channel_stream(
            dotzuki_audio::sequencer::CHAN2,
            music_data::MEETRIVAL_CH2_ALTERNATE_START,
        );
        self.sequencer.override_channel_stream(
            dotzuki_audio::sequencer::CHAN3,
            music_data::MEETRIVAL_CH3_ALTERNATE_START,
        );
    }

    /// `Music_RivalAlternateTempo` (audio/alternate_tempo.asm:21-27): play
    /// MUSIC_MEET_RIVAL, then overwrite channel 1's pointer with the
    /// alternate-tempo stream (tempo 100 instead of 112). Used for the second
    /// Route22 rival battle approach (scripts/Route22.asm:252).
    pub fn play_meet_rival_alternate_tempo(&mut self) {
        self.saved_music_states.remove(&MusicId::MEET_RIVAL);
        self.play_music(MusicId::MEET_RIVAL);
        self.sequencer.override_channel_stream(
            dotzuki_audio::sequencer::CHAN1,
            music_data::MEETRIVAL_CH1_ALTERNATE_TEMPO,
        );
    }

    /// `Music_RivalAlternateStartAndTempo` (audio/alternate_tempo.asm:30-34):
    /// the alternate start on channels 1-3, then channel 1's pointer is
    /// overwritten *again* with the start+tempo stream. Used after the second
    /// Route22 rival battle (scripts/Route22.asm:333).
    pub fn play_meet_rival_alternate_start_and_tempo(&mut self) {
        self.play_meet_rival_alternate_start();
        self.sequencer.override_channel_stream(
            dotzuki_audio::sequencer::CHAN1,
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
    /// The original blocks for 100 frames with `DelayFrames`; here the fade
    /// machinery queues the restart, and `fade_complete` pokes the channel-1
    /// override right after the new song starts.
    pub fn play_cities1_alternate_tempo(&mut self) {
        if !self.sequencer.music_playing {
            // Nothing to fade out: play the alternate-tempo song directly.
            self.saved_music_states.remove(&MusicId::CITIES1);
            self.play_music(MusicId::CITIES1);
            self.sequencer.override_channel_stream(
                dotzuki_audio::sequencer::CHAN1,
                music_data::CITIES1_CH1_ALTERNATE_TEMPO,
            );
            return;
        }
        self.saved_music_states.remove(&MusicId::CITIES1);
        self.fade_state = FadeState::FadingOut;
        self.fade_counter = 10;
        self.fade_counter_reload = 10;
        self.fade_queued_music = Some(MusicId::CITIES1);
        self.fade_queued_ch1_override = Some(music_data::CITIES1_CH1_ALTERNATE_TEMPO);
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

    pub fn stop_music(&mut self) {
        self.sequencer.stop_music();
        self.last_music_id = None;
        self.fade_state = FadeState::None;
        self.fade_queued_music = None;
        self.saved_music_states.clear();
    }

    /// Clear saved music resume states. Called on map transitions,
    /// so the new map's BGM starts fresh rather than resuming a
    /// previously-saved position.
    pub fn clear_saved_music_states(&mut self) {
        self.saved_music_states.clear();
    }

    pub fn stop_sfx(&mut self) {
        self.sequencer.stop_sfx();
    }

    pub fn stop_all(&mut self) {
        self.sequencer.stop_all();
        self.last_music_id = None;
        self.fade_state = FadeState::None;
        self.fade_queued_music = None;
        self.saved_music_states.clear();
    }

    /// Call once per VBlank (~60 Hz). Ticks sequencer, processes fade, applies to APU.
    pub fn update_frame(&mut self) {
        self.process_fade();
        // Apply the fade/master volume *before* the sequencer tick, matching
        // the original VBlank order (home/vblank.asm:53 `call FadeOutAudio`
        // runs before home/vblank.asm:62 `call Audio1_UpdateMusic`). This
        // lets an in-song `volume` ($F0) command override NR50 for the rest
        // of the frame; the fade machinery re-owns NR50 on the next frame.
        self.apply_master_volume();
        self.sequencer.update_frame(&mut self.apu);
        // The alarm writes pulse-1 registers after the sequencer so it
        // overrides channel 1, exactly as the original's direct hardware
        // writes override whatever the sound engine put there.
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

    fn process_fade(&mut self) {
        if self.fade_state != FadeState::FadingOut {
            if !self.no_audio_fade_out {
                self.apply_master_volume();
            }
            return;
        }

        if self.fade_counter > 0 {
            self.fade_counter -= 1;
            return;
        }

        self.fade_counter = self.fade_counter_reload;

        if self.master_volume_left == 0 && self.master_volume_right == 0 {
            self.fade_complete();
            return;
        }

        self.master_volume_left = self.master_volume_left.saturating_sub(1);
        self.master_volume_right = self.master_volume_right.saturating_sub(1);
        self.apply_master_volume();
    }

    fn fade_complete(&mut self) {
        self.fade_state = FadeState::None;

        self.sequencer.stop_all();

        if let Some(next_id) = self.fade_queued_music.take() {
            self.play_music(next_id);
            // Music_Cities1AlternateTempo (audio/alternate_tempo.asm:48-50):
            // after the fade-out + restart, overwrite channel 1's pointer
            // with the queued alternate-tempo stream.
            if let Some(stream) = self.fade_queued_ch1_override.take() {
                self.sequencer
                    .override_channel_stream(dotzuki_audio::sequencer::CHAN1, stream);
            }
        } else {
            self.fade_queued_ch1_override = None;
        }
    }

    fn apply_master_volume(&mut self) {
        let nr50 = (self.master_volume_left << 4) | self.master_volume_right;
        self.apu.nr50 = nr50;
    }

    pub fn is_fading(&self) -> bool {
        self.fade_state == FadeState::FadingOut
    }

    pub fn is_music_playing(&self) -> bool {
        self.sequencer.music_playing
    }

    pub fn is_sfx_playing(&self) -> bool {
        self.sequencer.sfx_playing
    }

    pub fn nr50(&self) -> u8 {
        self.apu.nr50
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

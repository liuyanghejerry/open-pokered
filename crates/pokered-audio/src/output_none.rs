//! Bare-metal (GBA) no-op `AudioOutput` — the twin of `output.rs`.
//!
//! The GBA build has no cpal/Web Audio device layer (dotzuki-audio is
//! host-only), so every method is an empty body with the exact public
//! surface the frontends call (`game.rs`, `pokered-tui`): the calls keep
//! compiling and the game runs silent.

use crate::music_data::MusicId;
use crate::sfx_data::SfxId;

pub struct AudioOutput;

impl AudioOutput {
    /// Always `Some` — there is no device to fail on; constructing is free.
    pub fn new() -> Option<Self> {
        Some(Self)
    }

    pub fn play_music(&self, _id: MusicId) {}

    pub fn play_music_with_fade(&self, _id: MusicId, _fade_speed: u8) {}

    pub fn clear_saved_music_states(&self) {}

    pub fn fade_out(&self, _fade_speed: u8) {}

    pub fn play_sfx(&self, _id: SfxId) {}

    /// Play a species cry with pitch/length modifiers (`PlayCry`).
    pub fn play_cry(&self, _id: SfxId, _pitch_mod: u8, _tempo_mod: u8) {}

    /// In-battle POKé FLUTE jingle (`Music_PokeFluteInBattle`).
    pub fn play_flute_in_battle(&self) {}

    /// Alternate tempo/start music variants; no music means no variants.
    pub fn play_script_music(&self, _name: &str) -> bool {
        false
    }

    pub fn is_sfx_playing(&self) -> bool {
        false
    }

    /// Drive the low-health alarm (`wLowHealthAlarm`) from the battle UI.
    pub fn set_low_health_alarm(&self, _enable: bool) {}

    pub fn low_health_alarm_active(&self) -> bool {
        false
    }

    pub fn stop_music(&self) {}

    pub fn stop_all(&self) {}

    pub fn last_music_id(&self) -> Option<MusicId> {
        None
    }

    pub fn update_frame(&self) {}

    /// Resume the audio context if the browser suspended it — no-op here.
    pub fn try_resume(&self) {}
}

impl Default for AudioOutput {
    fn default() -> Self {
        Self
    }
}

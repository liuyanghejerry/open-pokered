//! Frame-level music/SFX orchestration: [`AudioManager`].
//!
//! Sits on top of the [`Sequencer`](crate::sequencer::Sequencer) +
//! [`Apu`](crate::apu::Apu) pair and owns everything that happens once per
//! VBlank (~60 Hz) rather than per audio sample:
//!
//! - **master volume** — a logical left/right level (0-7 each) stamped onto
//!   NR50 every frame *before* the sequencer tick, so an in-stream `volume`
//!   command can still override NR50 for the rest of that frame and the fade
//!   machinery re-owns it on the next one;
//! - **fade-out** — [`FadeState`] walks the master volume down one step per
//!   `fade_counter_reload` frames (the countdown lives in
//!   [`Sequencer::fade_counter`]); on completion the music is stopped and an
//!   optionally queued track starts (see [`play_music_with_fade`],
//!   [`fade_out`], [`fade_out_then_play`]);
//! - **resume states** — switching music mid-play snapshots the live
//!   sequencer under the old id; requesting that id again resumes from the
//!   snapshot via [`Sequencer::restore_music_from`] instead of restarting;
//! - **SFX/cry playback** — track lookup, first-used-channel detection, and
//!   pitch/tempo modifiers ([`play_sfx_with_modifiers`]) for cry-style
//!   playback on top of [`Sequencer::frequency_modifier`].
//!
//! The manager is game-agnostic: the game supplies its track tables through
//! two `fn` pointers (`MusicId -> TrackData`, `SfxId -> TrackData`) at
//! construction, and the id types are generic parameters.
//!
//! ## Hooks
//!
//! Two generic extension points cover engine-level behaviours that write
//! hardware state outside the sequencer:
//!
//! - [`set_post_frame_hook`](Self::set_post_frame_hook): runs at the very end
//!   of [`update_frame`](Self::update_frame), *after* the sequencer tick — a
//!   direct register write here overrides whatever the sound engine produced
//!   this frame (e.g. an alarm tone poked straight into the pulse-1
//!   registers NR11–NR14);
//! - the `on_restart` callback of [`fade_out_then_play`](Self::fade_out_then_play):
//!   runs right after the queued track starts, so a game can poke a channel
//!   stream override before the next tick (the classic
//!   fade-out-then-overwrite-channel-pointer routine).

use std::collections::HashMap;
use std::hash::Hash;

use crate::apu::Apu;
use crate::sequencer::Sequencer;

/// Full master volume (NR50 per-side range is 0-7).
const FULL_VOLUME: u8 = 7;

/// Music fade-out state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeState {
    None,
    FadingOut,
}

/// A game track as seen by the [`AudioManager`]: the channel byte streams in
/// hardware-channel order plus the initial tempo and the engine sound id.
///
/// Produced by the game's track-lookup callback. `tempo` is only meaningful
/// for music; SFX tempos are set per play call (plain SFX run at `0x0100`,
/// cries at `0x0080 + modifier`).
#[derive(Debug, Clone, Copy)]
pub struct TrackData {
    /// The sound id handed to the sequencer (channel `sound_id` fields).
    pub sound_id: u8,
    /// Channel streams in hardware-channel order (pulse1, pulse2, wave,
    /// noise); `None` slots are unused by this track.
    pub channels: [Option<&'static [u8]>; 4],
    /// Initial tempo (16-bit fixed point: high byte integer, low byte
    /// fraction). Ignored for SFX.
    pub tempo: u16,
}

/// The per-frame music/SFX orchestrator. Generic over the game's music id
/// (`M`) and SFX id (`S`) types.
///
/// Construct with the game's two track-lookup callbacks, then drive
/// [`update_frame`](Self::update_frame) once per VBlank. The APU half
/// ([`apu`](Self::apu)) is advanced per audio sample by the output layer
/// (see [`crate::output`]).
pub struct AudioManager<M, S> {
    pub sequencer: Sequencer,
    pub apu: Apu,

    /// Game callback: music id → track data.
    music_track: fn(M) -> TrackData,
    /// Game callback: SFX id → track data.
    sfx_track: fn(S) -> TrackData,

    master_volume_left: u8,
    master_volume_right: u8,

    /// Current fade state (`FadeState::None` when no fade is under way).
    pub fade_state: FadeState,
    /// Frames between master-volume steps during a fade-out (the countdown
    /// itself lives in [`Sequencer::fade_counter`]).
    pub fade_counter_reload: u8,
    /// Track to start when the current fade-out completes.
    pub fade_queued_music: Option<M>,
    /// Runs right after a fade-queued track starts (e.g. to overwrite a
    /// channel stream before the next tick).
    fade_complete_hook: Option<Box<dyn FnOnce(&mut Sequencer) + Send>>,
    /// Runs at the end of every [`update_frame`](Self::update_frame), after
    /// the sequencer tick — direct register writes here override the sound
    /// engine for this frame.
    post_frame_hook: Option<Box<dyn FnMut(&mut Apu, &mut Sequencer) + Send>>,

    no_audio_fade_out: bool,

    last_music_id: Option<M>,

    /// Resume snapshots of previously playing tracks, keyed by music id.
    saved_music_states: HashMap<M, Sequencer>,
}

impl<M: Copy + Eq + Hash + 'static, S: Copy + 'static> AudioManager<M, S> {
    /// Create a manager powered by the game's track tables. The APU starts
    /// unpowered — the output layer writes NR52 (or the game does) before any
    /// sound can come out.
    pub fn new(music_track: fn(M) -> TrackData, sfx_track: fn(S) -> TrackData) -> Self {
        Self {
            sequencer: Sequencer::new(),
            apu: Apu::new(),
            music_track,
            sfx_track,
            master_volume_left: FULL_VOLUME,
            master_volume_right: FULL_VOLUME,
            fade_state: FadeState::None,
            fade_counter_reload: 0,
            fade_queued_music: None,
            fade_complete_hook: None,
            post_frame_hook: None,
            no_audio_fade_out: false,
            last_music_id: None,
            saved_music_states: HashMap::new(),
        }
    }

    pub fn master_volume_left(&self) -> u8 {
        self.master_volume_left
    }

    pub fn master_volume_right(&self) -> u8 {
        self.master_volume_right
    }

    pub fn set_master_volume(&mut self, left: u8, right: u8) {
        self.master_volume_left = left.min(FULL_VOLUME);
        self.master_volume_right = right.min(FULL_VOLUME);
        self.apply_master_volume();
    }

    pub fn fade_state(&self) -> FadeState {
        self.fade_state
    }

    pub fn last_music_id(&self) -> Option<M> {
        self.last_music_id
    }

    pub fn set_no_audio_fade_out(&mut self, val: bool) {
        self.no_audio_fade_out = val;
    }

    pub fn no_audio_fade_out(&self) -> bool {
        self.no_audio_fade_out
    }

    /// Install the post-frame hook (see the module docs). `None` clears it.
    pub fn set_post_frame_hook(
        &mut self,
        hook: Option<Box<dyn FnMut(&mut Apu, &mut Sequencer) + Send>>,
    ) {
        self.post_frame_hook = hook;
    }

    /// Play a music track. If a *different* track is currently playing, its
    /// live sequencer state is snapshotted under its id first; if the
    /// requested track has a snapshot, playback resumes from it
    /// ([`Sequencer::restore_music_from`]) instead of restarting. Cancels any
    /// fade and restores full master volume.
    pub fn play_music(&mut self, id: M) {
        // If switching away from a different playing track, save its state.
        if self.sequencer.music_playing {
            if let Some(last_id) = self.last_music_id {
                if last_id != id {
                    self.saved_music_states
                        .insert(last_id, self.sequencer.clone());
                }
            }
        }

        // If we have a saved resume state for this track, restore it.
        if let Some(saved) = self.saved_music_states.remove(&id) {
            self.sequencer.restore_music_from(&saved);
            self.last_music_id = Some(id);
            self.clear_fade();
            self.reset_master_volume();
            return;
        }

        self.clear_fade();
        self.last_music_id = Some(id);

        let track = (self.music_track)(id);
        let channel_data: Vec<Vec<u8>> = track
            .channels
            .iter()
            .flatten()
            .map(|data| data.to_vec())
            .collect();
        self.sequencer
            .play_music(track.sound_id, &channel_data, track.tempo);
        self.reset_master_volume();
    }

    /// Fade the current music out, then start `id`. No-op if `id` is already
    /// the current track; plays immediately (no fade) when nothing is
    /// playing.
    pub fn play_music_with_fade(&mut self, id: M, fade_speed: u8) {
        if self.last_music_id == Some(id) {
            return;
        }

        if !self.sequencer.music_playing {
            self.play_music(id);
            return;
        }

        self.begin_fade(fade_speed, Some(id), None);
    }

    /// Fade the current music out to silence, then stop. No-op when no music
    /// is playing.
    pub fn fade_out(&mut self, fade_speed: u8) {
        if !self.sequencer.music_playing {
            return;
        }

        self.begin_fade(fade_speed, None, None);
    }

    /// Fade the current music out, then start `id` and run `on_restart`
    /// right after it starts (before the next sequencer tick) — the generic
    /// form of the classic fade-out-then-overwrite-channel-pointer routine.
    /// When no music is playing there is nothing to fade: the track starts
    /// and the hook runs immediately.
    pub fn fade_out_then_play(
        &mut self,
        id: M,
        fade_speed: u8,
        on_restart: Option<Box<dyn FnOnce(&mut Sequencer) + Send>>,
    ) {
        if !self.sequencer.music_playing {
            self.play_music(id);
            if let Some(hook) = on_restart {
                hook(&mut self.sequencer);
            }
            return;
        }

        self.begin_fade(fade_speed, Some(id), on_restart);
    }

    /// Play a one-shot sound effect (tempo `0x0100`, no pitch modifier).
    pub fn play_sfx(&mut self, id: S) {
        self.play_sfx_with_modifiers(id, 0, 0x0100);
    }

    /// Play a sound effect with cry-style modifiers: `frequency_mod` is
    /// installed as [`Sequencer::frequency_modifier`] (added to every note
    /// frequency) and `tempo` becomes the SFX tempo.
    pub fn play_sfx_with_modifiers(&mut self, id: S, frequency_mod: i16, tempo: u16) {
        let track = (self.sfx_track)(id);
        let Some(start_channel) = self.sfx_start_channel(id) else {
            return;
        };
        let channel_data: Vec<Vec<u8>> = track
            .channels
            .iter()
            .flatten()
            .map(|data| data.to_vec())
            .collect();
        self.sequencer.frequency_modifier = frequency_mod;
        self.sequencer
            .play_sfx(track.sound_id, &channel_data, start_channel, tempo);
    }

    /// The first hardware channel (0-3) this SFX uses — the index of the
    /// first `Some` slot in its track's channel table. `None` for an empty
    /// track. Lets the game gate SFX by channel (e.g. suppressing pulse-1
    /// SFX while an alarm owns those registers).
    pub fn sfx_start_channel(&self, id: S) -> Option<usize> {
        let track = (self.sfx_track)(id);
        track.channels.iter().position(|ch| ch.is_some())
    }

    pub fn stop_music(&mut self) {
        self.sequencer.stop_music();
        self.last_music_id = None;
        self.clear_fade();
        self.saved_music_states.clear();
    }

    /// Clear saved music resume states. Called on map transitions, so the
    /// new map's BGM starts fresh rather than resuming a previously-saved
    /// position.
    pub fn clear_saved_music_states(&mut self) {
        self.saved_music_states.clear();
    }

    /// Drop the resume snapshot of a single track, so the next
    /// [`play_music`](Self::play_music) for it starts fresh.
    pub fn discard_saved_music_state(&mut self, id: M) {
        self.saved_music_states.remove(&id);
    }

    pub fn stop_sfx(&mut self) {
        self.sequencer.stop_sfx();
    }

    pub fn stop_all(&mut self) {
        self.sequencer.stop_all();
        self.last_music_id = None;
        self.clear_fade();
        self.saved_music_states.clear();
    }

    /// Call once per VBlank (~60 Hz): advance the fade, stamp the master
    /// volume onto NR50, tick the sequencer, then run the post-frame hook.
    pub fn update_frame(&mut self) {
        self.process_fade();
        // Apply the fade/master volume *before* the sequencer tick, matching
        // the original VBlank order (fade-out routine runs before the music
        // update). This lets an in-song `volume` command override NR50 for
        // the rest of the frame; the fade machinery re-owns NR50 next frame.
        self.apply_master_volume();
        self.sequencer.update_frame(&mut self.apu);
        // The hook writes after the sequencer so it can override the sound
        // engine's registers for this frame (e.g. an alarm tone poked
        // straight into NR11–NR14).
        if let Some(hook) = &mut self.post_frame_hook {
            hook(&mut self.apu, &mut self.sequencer);
        }
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

    fn begin_fade(
        &mut self,
        fade_speed: u8,
        queued: Option<M>,
        hook: Option<Box<dyn FnOnce(&mut Sequencer) + Send>>,
    ) {
        self.fade_state = FadeState::FadingOut;
        self.sequencer.fade_counter = fade_speed;
        self.fade_counter_reload = fade_speed;
        self.fade_queued_music = queued;
        self.fade_complete_hook = hook;
    }

    fn clear_fade(&mut self) {
        self.fade_state = FadeState::None;
        self.fade_queued_music = None;
        self.fade_complete_hook = None;
    }

    fn reset_master_volume(&mut self) {
        self.master_volume_left = FULL_VOLUME;
        self.master_volume_right = FULL_VOLUME;
        self.apply_master_volume();
    }

    fn process_fade(&mut self) {
        if self.fade_state != FadeState::FadingOut {
            if !self.no_audio_fade_out {
                self.apply_master_volume();
            }
            return;
        }

        if self.sequencer.fade_counter > 0 {
            self.sequencer.fade_counter -= 1;
            return;
        }

        self.sequencer.fade_counter = self.fade_counter_reload;

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
            // Take the hook first: `play_music` clears pending fade state.
            let hook = self.fade_complete_hook.take();
            self.play_music(next_id);
            // Fade-then-restart routines (e.g. an alternate-tempo restart)
            // poke their channel overrides right after the new track starts.
            if let Some(hook) = hook {
                hook(&mut self.sequencer);
            }
        } else {
            self.fade_complete_hook = None;
        }
    }

    fn apply_master_volume(&mut self) {
        let nr50 = (self.master_volume_left << 4) | self.master_volume_right;
        self.apu.nr50 = nr50;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequencer::CHAN1;

    // ── A tiny fake game library ─────────────────────────────────────────

    /// note_type 12/$C7, octave 5, note C len 5, sound_ret — after one tick
    /// the channel is mid-note with ptr = 4 (used to test resume states).
    static THEME_A_CH1: &[u8] = &[0xDC, 0xC7, 0xE5, 0x04, 0xFF];
    static THEME_B_CH1: &[u8] = &[0xDC, 0xC7, 0xE5, 0x14, 0xFF];
    static SFX_BEEP_CH5: &[u8] = &[0x20, 0xE2, 0x50, 0x87, 0xFF];

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Music {
        ThemeA,
        ThemeB,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Sfx {
        Beep,
    }

    fn music_track(id: Music) -> TrackData {
        match id {
            Music::ThemeA => TrackData {
                sound_id: 1,
                channels: [Some(THEME_A_CH1), None, None, None],
                tempo: 0x0070,
            },
            Music::ThemeB => TrackData {
                sound_id: 2,
                channels: [Some(THEME_B_CH1), None, None, None],
                tempo: 0x0090,
            },
        }
    }

    fn sfx_track(id: Sfx) -> TrackData {
        match id {
            Sfx::Beep => TrackData {
                sound_id: 0x40,
                channels: [Some(SFX_BEEP_CH5), None, None, None],
                tempo: 0,
            },
        }
    }

    fn manager() -> AudioManager<Music, Sfx> {
        AudioManager::new(music_track, sfx_track)
    }

    #[test]
    fn new_starts_at_full_volume_and_no_fade() {
        let mgr = manager();
        assert_eq!(mgr.master_volume_left(), FULL_VOLUME);
        assert_eq!(mgr.master_volume_right(), FULL_VOLUME);
        assert_eq!(mgr.fade_state(), FadeState::None);
        assert!(!mgr.is_fading());
        assert!(!mgr.is_music_playing());
        assert_eq!(mgr.last_music_id(), None);
        assert_eq!(mgr.nr50(), 0x77);
    }

    #[test]
    fn set_master_volume_clamps_and_writes_nr50() {
        let mut mgr = manager();
        mgr.set_master_volume(15, 3);
        assert_eq!(mgr.master_volume_left(), FULL_VOLUME);
        assert_eq!(mgr.master_volume_right(), 3);
        assert_eq!(mgr.nr50(), 0x73);
    }

    #[test]
    fn play_music_starts_track_and_resets_volume() {
        let mut mgr = manager();
        mgr.set_master_volume(2, 2);
        mgr.play_music(Music::ThemeA);
        assert!(mgr.is_music_playing());
        assert_eq!(mgr.last_music_id(), Some(Music::ThemeA));
        assert_eq!(mgr.master_volume_left(), FULL_VOLUME);
        assert_eq!(mgr.sequencer.channels[0].data, THEME_A_CH1.to_vec());
    }

    #[test]
    fn switching_music_saves_and_resumes_position() {
        let mut mgr = manager();
        mgr.play_music(Music::ThemeA);
        mgr.update_frame();
        assert_eq!(mgr.sequencer.channels[0].ptr, 4, "mid-note after one tick");

        // Switching away snapshots the live position under ThemeA.
        mgr.play_music(Music::ThemeB);
        assert_eq!(mgr.sequencer.channels[0].ptr, 0, "ThemeB starts fresh");

        // Requesting ThemeA again resumes the snapshot instead of restarting.
        mgr.play_music(Music::ThemeA);
        assert_eq!(
            mgr.sequencer.channels[0].ptr, 4,
            "resume restores the saved command position"
        );
        assert_eq!(mgr.last_music_id(), Some(Music::ThemeA));
    }

    #[test]
    fn discard_saved_music_state_forces_restart() {
        let mut mgr = manager();
        mgr.play_music(Music::ThemeA);
        mgr.update_frame();
        mgr.play_music(Music::ThemeB);
        mgr.discard_saved_music_state(Music::ThemeA);
        mgr.play_music(Music::ThemeA);
        assert_eq!(mgr.sequencer.channels[0].ptr, 0);
    }

    #[test]
    fn play_music_with_fade_same_id_is_noop() {
        let mut mgr = manager();
        mgr.play_music(Music::ThemeA);
        mgr.play_music_with_fade(Music::ThemeA, 10);
        assert_eq!(mgr.fade_state(), FadeState::None);
    }

    #[test]
    fn fade_steps_volume_down_then_switches_music() {
        let mut mgr = manager();
        mgr.play_music(Music::ThemeA);
        mgr.play_music_with_fade(Music::ThemeB, 0);
        assert!(mgr.is_fading());

        // Volume 7 → 0, one step per frame at reload 0.
        for step in 0..7 {
            mgr.update_frame();
            assert_eq!(mgr.master_volume_left(), 6 - step);
        }

        // Next frame completes the fade and starts the queued track.
        mgr.update_frame();
        assert_eq!(mgr.fade_state(), FadeState::None);
        assert_eq!(mgr.last_music_id(), Some(Music::ThemeB));
        assert_eq!(mgr.master_volume_left(), FULL_VOLUME);
        assert!(mgr.is_music_playing());
    }

    #[test]
    fn fade_counter_delays_volume_steps() {
        let mut mgr = manager();
        mgr.play_music(Music::ThemeA);
        mgr.play_music_with_fade(Music::ThemeB, 2);
        assert_eq!(mgr.sequencer.fade_counter, 2);
        assert_eq!(mgr.fade_counter_reload, 2);

        mgr.update_frame(); // counter 2 → 1
        assert_eq!(mgr.master_volume_left(), FULL_VOLUME);
        mgr.update_frame(); // counter 1 → 0
        assert_eq!(mgr.master_volume_left(), FULL_VOLUME);
        mgr.update_frame(); // reload, step down
        assert_eq!(mgr.master_volume_left(), FULL_VOLUME - 1);
    }

    #[test]
    fn fade_out_without_queued_music_just_stops() {
        let mut mgr = manager();
        mgr.play_music(Music::ThemeA);
        mgr.fade_out(0);
        for _ in 0..8 {
            mgr.update_frame();
        }
        assert_eq!(mgr.fade_state(), FadeState::None);
        assert!(!mgr.is_music_playing());
    }

    #[test]
    fn fade_out_then_play_runs_hook_after_restart() {
        static ALT_CH1: &[u8] = &[0xDC, 0xC7, 0xE4, 0x24, 0xFF];

        let mut mgr = manager();
        mgr.play_music(Music::ThemeA);
        mgr.fade_out_then_play(
            Music::ThemeB,
            0,
            Some(Box::new(|seq| seq.override_channel_stream(CHAN1, ALT_CH1))),
        );
        assert!(mgr.is_fading());

        for _ in 0..8 {
            mgr.update_frame();
        }
        assert_eq!(mgr.last_music_id(), Some(Music::ThemeB));
        assert_eq!(
            mgr.sequencer.channels[CHAN1].data,
            ALT_CH1.to_vec(),
            "hook poked the channel override right after the restart"
        );
    }

    #[test]
    fn fade_out_then_play_without_music_plays_immediately() {
        static ALT_CH1: &[u8] = &[0xFF];

        let mut mgr = manager();
        mgr.fade_out_then_play(
            Music::ThemeA,
            10,
            Some(Box::new(|seq| seq.override_channel_stream(CHAN1, ALT_CH1))),
        );
        assert_eq!(mgr.fade_state(), FadeState::None);
        assert_eq!(mgr.last_music_id(), Some(Music::ThemeA));
        assert_eq!(mgr.sequencer.channels[CHAN1].data, ALT_CH1.to_vec());
    }

    #[test]
    fn play_sfx_detects_start_channel() {
        let mut mgr = manager();
        assert_eq!(mgr.sfx_start_channel(Sfx::Beep), Some(0));
        mgr.play_sfx(Sfx::Beep);
        assert!(mgr.is_sfx_playing());
        assert_eq!(mgr.sequencer.sfx_tempo, 0x0100);
        assert_eq!(mgr.sequencer.frequency_modifier, 0);
    }

    #[test]
    fn play_sfx_with_modifiers_sets_cry_parameters() {
        let mut mgr = manager();
        mgr.play_sfx_with_modifiers(Sfx::Beep, 0x20, 0x0140);
        assert!(mgr.is_sfx_playing());
        assert_eq!(mgr.sequencer.sfx_tempo, 0x0140);
        assert_eq!(mgr.sequencer.frequency_modifier, 0x20);
    }

    #[test]
    fn stop_all_clears_music_sfx_fade_and_saved_states() {
        let mut mgr = manager();
        mgr.play_music(Music::ThemeA);
        mgr.play_sfx(Sfx::Beep);
        mgr.play_music(Music::ThemeB); // saves ThemeA
        mgr.stop_all();
        assert!(!mgr.is_music_playing());
        assert!(!mgr.is_sfx_playing());
        assert_eq!(mgr.last_music_id(), None);
        assert_eq!(mgr.fade_state(), FadeState::None);
    }

    #[test]
    fn post_frame_hook_overrides_registers_after_sequencer() {
        let mut mgr = manager();
        mgr.apu.write_register(0xFF26, 0x80); // NR52 power on
        mgr.play_music(Music::ThemeA);
        // An alarm-style hook pokes the pulse-1 registers directly.
        mgr.set_post_frame_hook(Some(Box::new(|apu, _seq| {
            apu.write_register(0xFF10, 0);
            apu.write_register(0xFF11, 0xA0);
            apu.write_register(0xFF12, 0xE2);
            apu.write_register(0xFF13, 0x50);
            apu.write_register(0xFF14, 0x87);
        })));
        mgr.update_frame();
        assert_eq!(mgr.apu.read_register(0xFF12), 0xE2);
        assert_eq!(mgr.apu.ch1.freq_reg, 0x750);

        mgr.set_post_frame_hook(None);
        // With the hook gone nothing pokes the registers anymore: a manual
        // write survives the frame (the music stream has ended, so the
        // sequencer no longer touches channel 1 either).
        mgr.apu.write_register(0xFF12, 0x00);
        mgr.update_frame();
        assert_eq!(mgr.apu.read_register(0xFF12), 0x00);
    }
}

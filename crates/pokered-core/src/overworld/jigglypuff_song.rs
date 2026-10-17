//! PewterPokecenterJigglypuffText: inner PrintText/DONE, song, image-only
//! spin, then DoNotWaitForButtonPressAfterDisplayingText/TextScriptEnd.
//! Original scripts/PewterPokecenter.asm:19-84 at pokered fbcf7d0.
use super::screen::OverworldAudioRequest;
use super::script_bridge::ScriptEffect;
use super::{Direction, OverworldScreen};
use crate::alloc_prelude::*;
use dotzuki_engine::GameData;
use pokered_data::{maps::MapId, script_command::FieldTextPrompt, tilesets::TilesetId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase {
    Start,
    Printing,
    StopAllWait,
    SongSpin,
    PostSongWait,
    HoldARelease,
    CloseWait,
    Done,
}

const FACINGS: [Direction; 4] = [
    Direction::Down,
    Direction::Left,
    Direction::Up,
    Direction::Right,
];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JigglypuffSongState {
    // Reuse the real inner PrintText/DONE driver, including its preparation,
    // native pagination and final letter wait. No second display cursor.
    print: Box<ScriptEffect>,
    npc_id: String,
    map: Option<MapId>,
    npc_index: Option<usize>,
    phase: Phase,
    frames_remaining: u8,
    facing_index: u8,
}

impl JigglypuffSongState {
    pub fn new(text: String, npc_id: String) -> Self {
        Self {
            print: Box::new(ScriptEffect::PrintFieldText {
                text,
                retain_window: true,
                prompt: FieldTextPrompt::Done,
                prompt_frames_remaining: 0,
                prompt_last_buttons: None,
                leading_sound: None,
                leading_sound_started: false,
            }),
            npc_id,
            map: None,
            npc_index: None,
            phase: Phase::Start,
            frames_remaining: 0,
            facing_index: 0,
        }
    }

    /// This override belongs to the effect, never the NPC's logical facing.
    /// Close, reset and map replacement drop it automatically. In particular,
    /// no stale override is installed into a different map's slot 3.
    pub(super) fn image_override(&self, map: MapId) -> Option<(usize, Direction)> {
        (self.map == Some(map)
            && matches!(
                self.phase,
                Phase::SongSpin | Phase::PostSongWait | Phase::HoldARelease
            ))
        .then(|| {
            self.npc_index
                .map(|index| (index, FACINGS[usize::from(self.facing_index) % 4]))
        })
        .flatten()
    }
}

impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    pub(super) fn tick_jigglypuff_song(&mut self, state: &mut JigglypuffSongState) -> bool {
        match state.phase {
            Phase::Start => {
                self.begin_source_field();
                state.map = Some(self.state.current_map);
                state.npc_index = super::update::resolve_npc_index(
                    &state.npc_id,
                    &self.npc_states,
                    &self.map_script_config,
                );
                // This entry calls PrintText inside DisplayTextID's text_asm.
                // The inner helper performs BG3/font loading + PrintText3.
                self.pending_dialogue = None;
                self.last_script_dialogue = None;
                state.phase = Phase::Printing;
                return self.tick_jigglypuff_song(state);
            }
            Phase::Printing => {
                if self.tick_owned_field_text(&mut state.print) {
                    // DONE has returned to the caller, retaining the window.
                    // Stop all music/SFX once and yield before DelayFrames32.
                    self.audio_requests.push(OverworldAudioRequest::PlaySound {
                        sound_id: "SFX_STOP_ALL_MUSIC".into(),
                    });
                    state.frames_remaining = 32;
                    state.phase = Phase::StopAllWait;
                }
            }
            Phase::StopAllWait => {
                state.frames_remaining = state.frames_remaining.saturating_sub(1);
                if state.frames_remaining == 0 {
                    // The original locates the current ImageIndex in its
                    // rotating array, retaining that position for every loop.
                    let facing = state
                        .npc_index
                        .and_then(|index| {
                            self.npc_presentation
                                .pose(self.state.current_map, index)
                                .map(|pose| pose.image.facing)
                                .or_else(|| self.npc_states.get(index).map(|npc| npc.facing))
                        })
                        .unwrap_or(Direction::Down);
                    state.facing_index = FACINGS
                        .iter()
                        .position(|&value| value == facing)
                        .unwrap_or(0) as u8;
                    self.audio_requests.push(OverworldAudioRequest::PlayMusic {
                        music_id: "MUSIC_JIGGLYPUFF_SONG".into(),
                    });
                    state.frames_remaining = 24;
                    state.phase = Phase::SongSpin;
                    // Even a pre-dispatch false sample cannot skip the first
                    // full 24-frame block or the App's actual song start.
                }
            }
            Phase::SongSpin => {
                state.frames_remaining = state.frames_remaining.saturating_sub(1);
                if state.frames_remaining == 0 {
                    // Original checks CH1 OR CH2 only after DelayFrames24.
                    // The App sample is also scoped to JIGGLYPUFF_SONG:
                    // JSON snapshots do not restore the audio sequencer, so
                    // restored looping map music must never hold this wait.
                    if self.jigglypuff_song_playing {
                        state.facing_index = (state.facing_index + 1) % 4;
                        state.frames_remaining = 24;
                    } else {
                        state.phase = Phase::PostSongWait;
                        state.frames_remaining = 48;
                    }
                }
            }
            Phase::PostSongWait => {
                state.frames_remaining = state.frames_remaining.saturating_sub(1);
                if state.frames_remaining == 0 {
                    self.audio_requests
                        .push(OverworldAudioRequest::RestoreMapMusic {
                            map: self.state.current_map,
                        });
                    // DoNotWait bypasses WaitForTextScrollButtonPress, but
                    // TextScriptEnd still returns through HoldTextDisplayOpen.
                    self.pending_dialogue = self.last_script_dialogue.take();
                    if let Some(dialogue) = self.pending_dialogue.as_mut() {
                        dialogue.start_holding_open();
                    }
                    state.phase = Phase::HoldARelease;
                    return self.tick_jigglypuff_song(state);
                }
            }
            Phase::HoldARelease => {
                if self.tick_retained_field_outer_ack() {
                    self.clear_retained_field_text();
                    state.phase = Phase::CloseWait;
                }
            }
            Phase::CloseWait => state.phase = Phase::Done,
            Phase::Done => {}
        }
        state.phase == Phase::Done
    }
}

#[cfg(all(test, not(target_os = "none"), not(feature = "script-boa")))]
mod tests;

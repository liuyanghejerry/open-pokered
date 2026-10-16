//! SilphCo11F's explicit GBFadeOutToBlack / Delay3 / GBFadeInFromBlack caller.
//! The effect's installation writes Pal4; only subsequent host ticks wait.

use super::screen::OverworldScreen;
use super::script_bridge::ScriptEffect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase {
    FadeOut,
    Settle,
    FadeIn,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SilphRocketDepartureState {
    phase: Phase,
    step: u8,
    frames_remaining: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DepartureTick {
    Waiting,
    ApplyObjects,
    Complete,
}

impl Default for SilphRocketDepartureState {
    fn default() -> Self {
        Self::new()
    }
}

impl SilphRocketDepartureState {
    pub fn new() -> Self {
        Self {
            phase: Phase::FadeOut,
            step: 0,
            frames_remaining: 8,
        }
    }

    /// Index into the original FadePal1..4 table, not an already mapped shade.
    pub fn palette_index(&self) -> usize {
        match self.phase {
            Phase::FadeOut => usize::from(3 - self.step),
            Phase::Settle => 0,
            Phase::FadeIn => usize::from(self.step),
            Phase::Done => 3,
        }
    }

    pub(super) fn advance(&mut self) -> DepartureTick {
        if self.phase == Phase::Done {
            return DepartureTick::Complete;
        }
        self.frames_remaining -= 1;
        if self.frames_remaining != 0 {
            return DepartureTick::Waiting;
        }
        match self.phase {
            Phase::FadeOut if self.step < 3 => {
                self.step += 1;
                self.frames_remaining = 8;
            }
            Phase::FadeOut => {
                self.phase = Phase::Settle;
                self.frames_remaining = 3;
                // The host applies all writes in this same update, before a
                // snapshot/draw can observe the Settle state.
                return DepartureTick::ApplyObjects;
            }
            Phase::Settle => {
                self.phase = Phase::FadeIn;
                self.step = 0;
                self.frames_remaining = 8;
            }
            Phase::FadeIn if self.step < 3 => {
                self.step += 1;
                self.frames_remaining = 8;
            }
            Phase::FadeIn => {
                self.phase = Phase::Done;
                return DepartureTick::Complete;
            }
            Phase::Done => unreachable!(),
        }
        DepartureTick::Waiting
    }
}

impl<G: dotzuki_engine::GameData> OverworldScreen<G> {
    /// Rendering observes the serialized script owner; drawing never ticks it.
    pub fn silph_rocket_departure_palette_index(&self) -> Option<usize> {
        match self.active_script_effect.as_ref() {
            Some(ScriptEffect::SilphRocketDeparture { state }) => Some(state.palette_index()),
            _ => None,
        }
    }
}

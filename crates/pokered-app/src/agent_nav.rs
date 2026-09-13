//! M2 closed-loop local navigation: `agent_move_to` / `agent_interact` /
//! `agent_interact_with` on [`PokemonGame`].
//!
//! The executor pathfinds on [`pokered_agent::NavGrid`] (which reuses the
//! game's own collision), then walks ONE tile at a time with real
//! controller input — hold the direction until the tile-locked step
//! begins, coast until it settles, re-observe — replanning on mismatch
//! and aborting on interruption (battle, dialogue, script, map change).
//! It never blind-plays a whole path. Synchronous like `step_frames`:
//! runs inside the debug handler, no TCP involved (tests drive it
//! directly).

use crate::alloc_prelude::*;
use pokered_agent::{
    direction_between, find_approach, find_path, InteractOutcome, InteractResult, NavGrid,
    NavigationOutcome, NavigationResult, NpcObs, Position,
};
use pokered_core::game_state::GameScreen;
use pokered_core::overworld::player_movement::direction_delta;
use pokered_core::overworld::{Direction, MovementState, WarpFadeState};
use pokered_data::maps::MapId;
use pokered_renderer::input::{GbButton, InputState};

use crate::game::PokemonGame;

/// Overall walk budget (tiles) before giving up as `Blocked`.
const MAX_TILE_STEPS: u32 = 240;
/// Overall simulation budget (frames) before giving up as `Blocked`.
const MAX_FRAMES: u32 = 6000;
/// Frames to hold a direction waiting for a tile step to begin (covers
/// the initial turn-in-place frame; a bump never starts one → stuck).
const START_STEP_FRAMES: u32 = 30;
/// Frames to coast waiting for a step to settle back to Idle.
const DRAIN_FRAMES: u32 = 96;
/// Consecutive failed/mismatched tile attempts tolerated before `Blocked`
/// (a wandering NPC repeatedly pinching the path, spinner loops, …).
const MAX_MISMATCHES: u32 = 6;
/// Frames to wait for a dialogue/battle after an A press.
const INTERACT_FRAMES: u32 = 60;
/// Re-approach attempts for `interact_with` when a wandering NPC drifted
/// away from the tile the approach was planned against.
const APPROACH_RETRIES: u32 = 3;

/// Abort-condition mapping (pure; unit-tested). Priority: map change →
/// battle → dialogue → script. Returns `None` while the player simply
/// has control.
pub(crate) fn nav_interruption(
    map_changed: bool,
    battle: bool,
    dialogue: bool,
    script_busy: bool,
) -> Option<NavigationResult> {
    if map_changed {
        Some(NavigationResult::MapChanged)
    } else if battle {
        Some(NavigationResult::EnteredBattle)
    } else if dialogue {
        Some(NavigationResult::EnteredDialogue)
    } else if script_busy {
        Some(NavigationResult::Interrupted)
    } else {
        None
    }
}

fn button_for(dir: Direction) -> GbButton {
    match dir {
        Direction::Up => GbButton::Up,
        Direction::Down => GbButton::Down,
        Direction::Left => GbButton::Left,
        Direction::Right => GbButton::Right,
    }
}

/// Result of one tile-walk attempt.
enum TileWalk {
    /// The step settled; carries the actual arrival cell (may differ
    /// from the planned one — ledge overshoot, spinner slide, …).
    Done((u16, u16)),
    /// The step never started (bumped into a wall/NPC).
    Stuck,
    /// An interruption fired mid-step.
    Aborted(NavigationResult),
}

impl PokemonGame {
    /// Current abort state for navigation, or `None` while the player
    /// has plain overworld control relative to `start_map`.
    fn check_nav_interruption(&self, start_map: MapId) -> Option<NavigationResult> {
        nav_interruption(
            self.overworld.state.current_map != start_map
                || self.overworld.pending_warp.is_some()
                || !matches!(self.overworld.warp_fade_state, WarpFadeState::Idle),
            matches!(self.state.screen, GameScreen::Battle)
                || self.overworld.pending_wild_encounter.is_some()
                || self.overworld.pending_trainer_battle.is_some()
                || self.overworld.script_awaiting_battle,
            // A trainer sight-intro reads as dialogue-incoming, so the
            // caller can wait it out instead of treating it as an opaque
            // script interruption.
            self.overworld.pending_dialogue.is_some()
                || self.overworld.pending_choice.is_some()
                || self.overworld.trainer_encounter_pending(),
            self.overworld.active_script_effect_label().is_some()
                || !self.overworld.script_engine_idle(),
        )
    }

    /// NavGrid for the current map with live NPC occupancy.
    pub(crate) fn build_nav_grid(&self) -> Option<NavGrid> {
        let map = self.overworld.map_data.as_ref()?;
        let npcs: Vec<NpcObs> = self
            .overworld
            .npc_states
            .iter()
            .map(NpcObs::from)
            .collect();
        Some(NavGrid::build(map, &npcs))
    }

    pub(crate) fn step_with(&mut self, button: Option<GbButton>, frames: &mut u32) {
        let mut input = InputState::new();
        if let Some(button) = button {
            input.press(button);
        }
        self.update(&input);
        *frames += 1;
    }

    pub(crate) fn player_pos(&self) -> (u16, u16) {
        (
            self.overworld.state.player.x,
            self.overworld.state.player.y,
        )
    }

    /// Walk a single planned tile: hold the direction until the
    /// tile-locked step begins, then coast on neutral input until it
    /// settles — exactly one tile (two on a ledge hop), zero overshoot.
    fn walk_one_tile(
        &mut self,
        next: (u16, u16),
        start_map: MapId,
        frames: &mut u32,
    ) -> TileWalk {
        let from = self.player_pos();
        let Some(dir) = direction_between(from, next) else {
            return TileWalk::Done(from);
        };
        let button = button_for(dir);

        // Phase 1: hold until the step begins (position can't change
        // before the first walk frame; a turn consumes the first press).
        let mut held = 0;
        loop {
            if let Some(result) = self.check_nav_interruption(start_map) {
                return TileWalk::Aborted(result);
            }
            if self.player_pos() != from {
                return TileWalk::Done(self.player_pos());
            }
            if self.overworld.ledge_jump.is_some()
                || self.overworld.state.player.movement_state != MovementState::Idle
            {
                break;
            }
            if held >= START_STEP_FRAMES {
                return TileWalk::Stuck;
            }
            self.step_with(Some(button), frames);
            held += 1;
        }

        // Phase 2: coast until the step (or ledge hop) settles. The
        // settle check runs first so a script/encounter that fires ON
        // the arrival tile is reported by the outer loop's target check
        // (Reached wins on the target tile) rather than swallowed here.
        let mut coasted = 0;
        loop {
            if self.overworld.ledge_jump.is_none()
                && self.overworld.state.player.movement_state == MovementState::Idle
            {
                return TileWalk::Done(self.player_pos());
            }
            if let Some(result) = self.check_nav_interruption(start_map) {
                return TileWalk::Aborted(result);
            }
            if coasted >= DRAIN_FRAMES {
                return TileWalk::Done(self.player_pos());
            }
            self.step_with(None, frames);
            coasted += 1;
        }
    }

    /// Step neutral frames until the warp fade completes (bounded), so a
    /// `MapChanged` outcome reports the destination map already settled.
    pub(crate) fn settle_map_change(&mut self, frames: &mut u32) {
        for _ in 0..240 {
            if matches!(self.overworld.warp_fade_state, WarpFadeState::Idle)
                && self.overworld.pending_warp.is_none()
            {
                return;
            }
            self.step_with(None, frames);
        }
    }

    /// Closed-loop walk to (`target_x`, `target_y`) on the current map.
    /// Re-plans around live NPC positions after every tile; reports the
    /// first abort condition hit. Pure observation + real input — no
    /// game-state writes beyond what walking does anyway.
    pub fn agent_move_to(&mut self, target_x: u16, target_y: u16) -> NavigationOutcome {
        let start_map = self.overworld.state.current_map;
        let (sx, sy) = self.player_pos();
        let mut outcome = NavigationOutcome {
            result: NavigationResult::Blocked,
            steps: 0,
            frames: 0,
            start: Position {
                x: sx as i32,
                y: sy as i32,
            },
            target: Position {
                x: target_x as i32,
                y: target_y as i32,
            },
            final_pos: Position {
                x: sx as i32,
                y: sy as i32,
            },
            detail: None,
        };
        let mut mismatches = 0u32;

        while outcome.steps < MAX_TILE_STEPS && outcome.frames < MAX_FRAMES {
            let pos = self.player_pos();
            outcome.final_pos = Position {
                x: pos.0 as i32,
                y: pos.1 as i32,
            };
            let interruption = self.check_nav_interruption(start_map);
            // A map transition always wins — even when the target tile is
            // the warp itself (stepping onto it IS the point). Settle the
            // fade first so the outcome reports the destination map.
            if matches!(interruption, Some(NavigationResult::MapChanged)) {
                self.settle_map_change(&mut outcome.frames);
                outcome.result = NavigationResult::MapChanged;
                let pos = self.player_pos();
                outcome.final_pos = Position {
                    x: pos.0 as i32,
                    y: pos.1 as i32,
                };
                return outcome;
            }
            if pos == (target_x, target_y) {
                outcome.result = NavigationResult::Reached;
                return outcome;
            }
            // Other interruptions (battle/dialogue/script) lose to the
            // target check: an event that fires on the arrival tile is
            // the caller's to handle.
            if let Some(result) = interruption {
                outcome.result = result;
                return outcome;
            }

            let Some(grid) = self.build_nav_grid() else {
                outcome.detail = Some("no map loaded".to_string());
                return outcome;
            };
            if !grid.in_bounds(target_x, target_y) {
                outcome.detail = Some("target out of bounds".to_string());
                return outcome;
            }
            let Some(path) = find_path(&grid, pos, (target_x, target_y)) else {
                outcome.detail = Some("no path to target".to_string());
                return outcome;
            };
            let Some(&next) = path.first() else {
                // Empty path means start == goal, handled above.
                continue;
            };

            match self.walk_one_tile(next, start_map, &mut outcome.frames) {
                TileWalk::Done(actual) if actual == next => {
                    outcome.steps += 1;
                    mismatches = 0;
                }
                TileWalk::Done(actual) => {
                    outcome.steps += 1;
                    mismatches += 1;
                    outcome.detail = Some(format!(
                        "step toward ({},{}) landed at ({},{}); replanned",
                        next.0, next.1, actual.0, actual.1
                    ));
                }
                TileWalk::Stuck => {
                    mismatches += 1;
                    outcome.detail = Some(format!(
                        "blocked stepping toward ({},{})",
                        next.0, next.1
                    ));
                }
                TileWalk::Aborted(result) => {
                    if result == NavigationResult::MapChanged {
                        self.settle_map_change(&mut outcome.frames);
                    }
                    outcome.result = result;
                    let pos = self.player_pos();
                    outcome.final_pos = Position {
                        x: pos.0 as i32,
                        y: pos.1 as i32,
                    };
                    return outcome;
                }
            }
            if mismatches >= MAX_MISMATCHES {
                outcome.result = NavigationResult::Blocked;
                return outcome;
            }
        }
        outcome.detail = Some("budget exhausted".to_string());
        outcome
    }

    /// Turn in place to face `dir` (one press frame + settle). No-op
    /// when already facing — never walks.
    pub(crate) fn turn_to(&mut self, dir: Direction, frames: &mut u32) {
        if self.overworld.state.player.facing == dir {
            return;
        }
        self.step_with(Some(button_for(dir)), frames);
        for _ in 0..4 {
            self.step_with(None, frames);
        }
    }

    /// The tile the player currently faces.
    fn faced_tile(&self) -> (u16, u16) {
        let (dx, dy) = direction_delta(self.overworld.state.player.facing);
        let (x, y) = self.player_pos();
        (
            (x as i32 + dx as i32).max(0) as u16,
            (y as i32 + dy as i32).max(0) as u16,
        )
    }

    /// Whether pressing A facing `tile` can do something: a visible NPC
    /// stands there, a sign is posted there, or a hidden item is buried
    /// there (the game's own A-press checks run on the faced tile).
    fn interactable_on(&self, tile: (u16, u16)) -> bool {
        let on_npc = self
            .overworld
            .npc_states
            .iter()
            .any(|n| n.visible && (n.x, n.y) == tile);
        if on_npc {
            return true;
        }
        let map_id = self.overworld.state.current_map;
        if let Some(map_json) = pokered_data::map_data_loader::get_map_json(map_id) {
            if map_json
                .signs
                .iter()
                .any(|s| (s.x as u16, s.y as u16) == tile)
            {
                return true;
            }
        }
        pokered_data::hidden_items::find_hidden_item(map_id, tile.0 as u8, tile.1 as u8).is_some()
    }

    /// Press A once (tap + release) and run until a dialogue opens, a
    /// battle starts, a script takes over, the map changes, or nothing
    /// happens within [`INTERACT_FRAMES`].
    fn run_interaction(&mut self, frames: &mut u32, start_map: MapId) -> InteractResult {
        self.step_with(Some(GbButton::A), frames);
        self.step_with(None, frames);
        for _ in 0..INTERACT_FRAMES {
            if self.overworld.state.current_map != start_map
                || self.overworld.pending_warp.is_some()
                || !matches!(self.overworld.warp_fade_state, WarpFadeState::Idle)
            {
                return InteractResult::MapChanged;
            }
            if matches!(self.state.screen, GameScreen::Battle)
                || self.overworld.pending_wild_encounter.is_some()
                || self.overworld.pending_trainer_battle.is_some()
            {
                return InteractResult::Battle;
            }
            if self.overworld.pending_dialogue.is_some() || self.overworld.pending_choice.is_some()
            {
                return InteractResult::Dialogue;
            }
            if self.overworld.active_script_effect_label().is_some()
                || !self.overworld.script_engine_idle()
            {
                return InteractResult::Interrupted;
            }
            self.step_with(None, frames);
        }
        InteractResult::Nothing
    }

    fn interact_outcome(
        &self,
        result: InteractResult,
        frames: u32,
        navigation: Option<NavigationOutcome>,
        target: Option<String>,
        detail: Option<String>,
    ) -> InteractOutcome {
        let pos = self.player_pos();
        InteractOutcome {
            result,
            target,
            frames,
            navigation,
            final_pos: Position {
                x: pos.0 as i32,
                y: pos.1 as i32,
            },
            facing: format!("{:?}", self.overworld.state.player.facing),
            detail,
        }
    }

    /// Press A facing the adjacent interactable — the faced tile first,
    /// otherwise the player turns toward an adjacent visible NPC, sign,
    /// or hidden item (that priority) before pressing A.
    pub fn agent_interact(&mut self) -> InteractOutcome {
        let start_map = self.overworld.state.current_map;
        let mut frames = 0u32;

        // A post-battle/warp fade-IN tail reads as a map transition to
        // the entry check below — settle it first (same class of fix as
        // travel's settle_for_travel).
        self.settle_map_change(&mut frames);

        if let Some(result) = self.check_nav_interruption(start_map) {
            let result = match result {
                NavigationResult::EnteredBattle => InteractResult::Battle,
                NavigationResult::EnteredDialogue => InteractResult::Dialogue,
                NavigationResult::Interrupted => InteractResult::Interrupted,
                _ => InteractResult::MapChanged,
            };
            return self.interact_outcome(result, frames, None, None, None);
        }

        // Faced tile first; otherwise scan adjacent cells for something
        // to face (visible NPC > sign > hidden item, game order).
        let facing = self.overworld.state.player.facing;
        let dir = if self.interactable_on(self.faced_tile()) {
            Some(facing)
        } else {
            [
                Direction::Down,
                Direction::Up,
                Direction::Left,
                Direction::Right,
            ]
            .into_iter()
            .find(|&dir| {
                let (dx, dy) = direction_delta(dir);
                let (x, y) = self.player_pos();
                let tile = (
                    (x as i32 + dx as i32).max(0) as u16,
                    (y as i32 + dy as i32).max(0) as u16,
                );
                self.interactable_on(tile)
            })
        };
        if let Some(dir) = dir {
            self.turn_to(dir, &mut frames);
        }
        let result = self.run_interaction(&mut frames, start_map);
        self.interact_outcome(result, frames, None, None, None)
    }

    /// Resolve a `get_nearby` entity id to its current tile. `npc:{i}`
    /// requires a visible NPC; `sign:{i}` and `hidden:{table_index}`
    /// resolve against the current map's static data. Warps are not
    /// interaction targets (walking onto one is `move_to`'s job).
    fn resolve_interact_target(&self, id: &str) -> Result<(u16, u16), String> {
        let (kind, index) = id
            .split_once(':')
            .ok_or_else(|| format!("malformed entity id: '{id}'"))?;
        let index: usize = index
            .parse()
            .map_err(|_| format!("malformed entity id: '{id}'"))?;
        let map_id = self.overworld.state.current_map;
        match kind {
            "npc" => self
                .overworld
                .npc_states
                .iter()
                .find(|n| n.npc_index as usize == index)
                .filter(|n| n.visible)
                .map(|n| (n.x, n.y))
                .ok_or_else(|| format!("no visible npc with index {index}")),
            "sign" => {
                let map_handle = pokered_data::map_data_loader::get_map_json(map_id);
                map_handle
                    .and_then(|m| m.signs.get(index).map(|s| (s.x as u16, s.y as u16)))
                    .ok_or_else(|| format!("no sign with index {index} on this map"))
            }
            "hidden" => pokered_data::hidden_items::HIDDEN_ITEMS
                .get(index)
                .filter(|h| h.map == map_id)
                .map(|h| (h.x as u16, h.y as u16))
                .ok_or_else(|| format!("no hidden item {index} on this map")),
            "warp" => Err("interact_with does not target warps; use move_to".to_string()),
            _ => Err(format!("unknown entity kind in id: '{id}'")),
        }
    }

    /// Pathfind adjacent to a `get_nearby` entity, face it, and press A.
    /// Re-approaches (bounded) when a wandering NPC drifted between
    /// planning and arrival.
    pub fn agent_interact_with(&mut self, id: &str) -> InteractOutcome {
        let start_map = self.overworld.state.current_map;
        let mut frames = 0u32;

        // Settle any post-battle fade-IN tail before the entry check
        // (it would otherwise read as a map transition — see
        // agent_interact).
        self.settle_map_change(&mut frames);

        if let Some(result) = self.check_nav_interruption(start_map) {
            let result = match result {
                NavigationResult::EnteredBattle => InteractResult::Battle,
                NavigationResult::EnteredDialogue => InteractResult::Dialogue,
                NavigationResult::Interrupted => InteractResult::Interrupted,
                _ => InteractResult::MapChanged,
            };
            return self.interact_outcome(result, frames, None, Some(id.to_string()), None);
        }

        let mut navigation: Option<NavigationOutcome> = None;
        let mut attempts = 0u32;
        loop {
            attempts += 1;
            let tile = match self.resolve_interact_target(id) {
                Ok(tile) => tile,
                Err(detail) => {
                    return self.interact_outcome(
                        InteractResult::NotFound,
                        frames,
                        navigation,
                        Some(id.to_string()),
                        Some(detail),
                    )
                }
            };

            // Already adjacent? Skip navigation entirely.
            let pos = self.player_pos();
            let adjacent = (pos.0 as i32 - tile.0 as i32).abs() + (pos.1 as i32 - tile.1 as i32).abs()
                == 1;
            let Some(grid) = self.build_nav_grid() else {
                return self.interact_outcome(
                    InteractResult::Blocked,
                    frames,
                    navigation,
                    Some(id.to_string()),
                    Some("no map loaded".to_string()),
                );
            };
            let approach = if adjacent {
                Some((Vec::new(), direction_between(pos, tile).unwrap_or(Direction::Down)))
            } else {
                find_approach(&grid, pos, tile)
            };
            let Some((path, face_dir)) = approach else {
                return self.interact_outcome(
                    InteractResult::Blocked,
                    frames,
                    navigation,
                    Some(id.to_string()),
                    Some(format!("no approach path to {id}")),
                );
            };

            if let Some(&dest) = path.last() {
                let nav = self.agent_move_to(dest.0, dest.1);
                frames += nav.frames;
                if nav.result != NavigationResult::Reached {
                    let result = match nav.result {
                        NavigationResult::Blocked => InteractResult::Blocked,
                        NavigationResult::Interrupted => InteractResult::Interrupted,
                        NavigationResult::EnteredBattle => InteractResult::EnteredBattle,
                        NavigationResult::EnteredDialogue => InteractResult::EnteredDialogue,
                        NavigationResult::MapChanged => InteractResult::MapChanged,
                        NavigationResult::Reached => unreachable!(),
                    };
                    return self.interact_outcome(
                        result,
                        frames,
                        Some(nav),
                        Some(id.to_string()),
                        None,
                    );
                }
                navigation = Some(nav);
            }

            // A wandering NPC may have drifted mid-approach: re-resolve
            // and re-approach (bounded) until the target is adjacent.
            let current_tile = match self.resolve_interact_target(id) {
                Ok(tile) => tile,
                Err(detail) => {
                    return self.interact_outcome(
                        InteractResult::NotFound,
                        frames,
                        navigation,
                        Some(id.to_string()),
                        Some(detail),
                    )
                }
            };
            let pos = self.player_pos();
            let dx = current_tile.0 as i32 - pos.0 as i32;
            let dy = current_tile.1 as i32 - pos.1 as i32;
            if dx.abs() + dy.abs() == 1 {
                let face_dir = direction_between(pos, current_tile).unwrap_or(face_dir);
                self.turn_to(face_dir, &mut frames);
                let result = self.run_interaction(&mut frames, start_map);
                return self.interact_outcome(result, frames, navigation, Some(id.to_string()), None);
            }
            if attempts >= APPROACH_RETRIES {
                return self.interact_outcome(
                    InteractResult::Blocked,
                    frames,
                    navigation,
                    Some(id.to_string()),
                    Some(format!("{id} moved away during approach")),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interruption_priority_map_battle_dialogue_script() {
        assert_eq!(nav_interruption(false, false, false, false), None);
        assert_eq!(
            nav_interruption(true, true, true, true),
            Some(NavigationResult::MapChanged)
        );
        assert_eq!(
            nav_interruption(false, true, true, true),
            Some(NavigationResult::EnteredBattle)
        );
        assert_eq!(
            nav_interruption(false, false, true, true),
            Some(NavigationResult::EnteredDialogue)
        );
        assert_eq!(
            nav_interruption(false, false, false, true),
            Some(NavigationResult::Interrupted)
        );
    }
}

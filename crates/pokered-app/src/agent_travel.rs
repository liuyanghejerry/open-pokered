//! M3 cross-map travel: `agent_travel_to` on [`PokemonGame`].
//!
//! Executes a world-graph route end-to-end: per map-level leg it routes
//! at tile level ([`pokered_agent::find_tile_route`] — local collision
//! steps, warp teleports incl. LAST_MAP mats, goal-bound connection
//! crossings), drives the M2 per-tile walker, verifies every map
//! landing, and replans the remaining route from the actual map on
//! mismatch. Wild battles on grass routes are auto-resolved (RUN with a
//! fast lead — deterministic; falls back to FIGHT after failed runs);
//! trainer battles are fought with the lead's first move. Blackouts and
//! unresolvable battles abort the travel.

use pokered_agent::{
    find_tile_route, NavGrid, NavigationResult, TileCross, TileLeg, TravelOutcome, TravelResult,
    WorldGraph,
};
use pokered_core::battle::BattlePhase;
use pokered_core::game_state::GameScreen;
use pokered_core::overworld::Direction;
use pokered_data::maps::MapId;
use pokered_renderer::input::GbButton;

use crate::game::PokemonGame;

/// Graph-route replans after map mismatches before giving up.
const MAX_REPLANS: u32 = 6;
/// Retries per tile leg (battle/dialogue interruptions). Progress is
/// kept between attempts (each re-walk starts from the position the
/// interruption dropped us at), so this bounds the number of battles a
/// single grass leg can plausibly roll — a 131-tile forest leg at ~10%
/// per grass tile.
const MAX_LEG_ATTEMPTS: u32 = 12;
/// Frames to hold a direction waiting for a connection/warp crossing.
const CROSS_FRAMES: u32 = 90;
/// Action cap for the battle auto-resolver.
const BATTLE_MAX_ROUNDS: u32 = 300;
/// Frame cap for advancing a dialogue box.
const DIALOGUE_MAX_FRAMES: u32 = 900;

/// Result of the battle auto-resolver.
enum BattleResolution {
    /// Battle ended with the party standing (win or run).
    Resolved,
    /// Whole party fainted — the player whited out.
    Blackout,
    /// Round cap or a battle state the resolver doesn't drive.
    Failed,
}

/// Per-graph-leg execution result.
enum LegExec {
    Done,
    Abort(TravelResult, Option<String>),
    MapMismatch { expected: MapId, actual: MapId },
}

/// Result of a held-direction crossing press.
enum PressAcross {
    /// The map changed.
    Crossed,
    /// Nothing happened within the frame budget.
    NotFired,
    /// A battle/dialogue/script interrupted the press; the caller's
    /// settle loop resolves it and the attempt retries.
    Interrupted,
}

impl PokemonGame {
    /// Tap a button (one press frame + one release frame) so edge
    /// detectors see a fresh press each time.
    fn tap(&mut self, button: GbButton, frames: &mut u32) {
        self.step_with(Some(button), frames);
        self.step_with(None, frames);
    }

    /// Tap A to confirm a menu, then wait (bounded) for the battle phase
    /// to leave `before` — otherwise the next round's action races the
    /// transition and double-confirms whatever the cursor sits on.
    fn tap_a_and_wait_phase(&mut self, before: &BattlePhase, frames: &mut u32) {
        self.tap(GbButton::A, frames);
        for _ in 0..12 {
            if std::mem::discriminant(&self.battle.phase) != std::mem::discriminant(before)
                || !matches!(self.state.screen, GameScreen::Battle)
            {
                return;
            }
            self.step_with(None, frames);
        }
    }

    /// Advance any open dialogue with A taps (bounded). A trainer
    /// sight-intro is first waited out into its before-battle text (or
    /// straight into the battle). Returns early when a battle starts
    /// underneath.
    fn advance_dialogue(&mut self, frames: &mut u32) {
        // Trainer sight-intro ("!" + walk-up + storyline): wait for it to
        // produce the before-battle dialogue or the battle itself.
        for _ in 0..DIALOGUE_MAX_FRAMES {
            if !self.overworld.trainer_encounter_pending()
                || self.overworld.pending_dialogue.is_some()
                || matches!(self.state.screen, GameScreen::Battle)
                || self.overworld.pending_trainer_battle.is_some()
            {
                break;
            }
            self.step_with(None, frames);
        }
        let mut tapped = 0u32;
        let mut a_down = false;
        while self.overworld.pending_dialogue.is_some() && tapped < DIALOGUE_MAX_FRAMES {
            if matches!(self.state.screen, GameScreen::Battle) {
                return;
            }
            if a_down {
                self.step_with(Some(GbButton::A), frames);
            } else {
                self.step_with(None, frames);
            }
            a_down = !a_down;
            tapped += 1;
        }
    }

    /// Auto-resolve the current battle: RUN from wild battles (a fast
    /// lead makes this deterministic; two failures switch to fighting),
    /// FIGHT with the lead's first move otherwise. Advances text with A.
    fn auto_resolve_battle(&mut self, frames: &mut u32) -> BattleResolution {
        let mut rounds = 0u32;
        let mut run_attempts = 0u32;
        while matches!(self.state.screen, GameScreen::Battle) {
            if rounds >= BATTLE_MAX_ROUNDS {
                return BattleResolution::Failed;
            }
            match self.battle.phase.clone() {
                BattlePhase::PlayerMenu => {
                    if self.battle.is_wild && run_attempts < 2 {
                        // RUN is grid (1,1); FIGHT is (0,0).
                        while self.battle.battle_menu.col() < 1 {
                            self.tap(GbButton::Right, frames);
                        }
                        while self.battle.battle_menu.row() < 1 {
                            self.tap(GbButton::Down, frames);
                        }
                        run_attempts += 1;
                    } else {
                        while self.battle.battle_menu.col() > 0 {
                            self.tap(GbButton::Left, frames);
                        }
                        while self.battle.battle_menu.row() > 0 {
                            self.tap(GbButton::Up, frames);
                        }
                    }
                    self.tap_a_and_wait_phase(&BattlePhase::PlayerMenu, frames);
                }
                BattlePhase::MoveSelect => {
                    // Prefer the first usable DAMAGING move (a lead whose
                    // first slot is a status move would otherwise stall
                    // the resolver); fall back to slot 0.
                    let preferred = self
                        .battle
                        .move_menu
                        .as_ref()
                        .map(|mm| {
                            mm.moves()
                                .iter()
                                .position(|slot| {
                                    !slot.is_disabled
                                        && slot.current_pp > 0
                                        && pokered_data::move_data::MoveData::get(slot.move_id)
                                            .is_some_and(|data| data.power > 0)
                                })
                                .unwrap_or(0)
                        })
                        .unwrap_or(0);
                    let mut guard = 0;
                    while guard < 8 {
                        let cursor = self
                            .battle
                            .move_menu
                            .as_ref()
                            .map(|mm| mm.cursor())
                            .unwrap_or(preferred);
                        if cursor == preferred
                            || !matches!(self.battle.phase, BattlePhase::MoveSelect)
                        {
                            break;
                        }
                        if cursor > preferred {
                            self.tap(GbButton::Up, frames);
                        } else {
                            self.tap(GbButton::Down, frames);
                        }
                        guard += 1;
                    }
                    self.tap_a_and_wait_phase(&BattlePhase::MoveSelect, frames);
                }
                BattlePhase::ShowingText { .. } | BattlePhase::ShiftPrompt => {
                    // A advances text; ShiftPrompt's cursor defaults to
                    // NO ("don't switch"), matching Gen-1.
                    self.tap(GbButton::A, frames);
                }
                _ => {
                    // Animations/send-outs settle on their own; B backs
                    // out of any unexpected sub-menu.
                    self.tap(GbButton::B, frames);
                }
            }
            rounds += 1;
        }
        if !self.save_data.party.is_empty()
            && self.save_data.party.iter().all(|mon| mon.hp == 0)
        {
            BattleResolution::Blackout
        } else {
            BattleResolution::Resolved
        }
    }

    /// Settle anything currently owning the game before (re)trying a
    /// leg: drain leftover warp-fade tails, advance dialogue, resolve
    /// battles. `Some` result aborts the travel.
    fn settle_for_travel(
        &mut self,
        frames: &mut u32,
        battles: &mut u32,
    ) -> Option<(TravelResult, Option<String>)> {
        for _ in 0..4 {
            // A connection/warp crossing leaves a fade-IN tail behind;
            // drain it so the next leg doesn't read it as a transition.
            self.settle_map_change(frames);
            if self.overworld.pending_dialogue.is_some()
                || self.overworld.pending_choice.is_some()
                || self.overworld.trainer_encounter_pending()
            {
                self.advance_dialogue(frames);
                continue;
            }
            if self.overworld.pending_trainer_battle.is_some() {
                // The battle screen opens a frame or two after the
                // before-battle text closes — wait for it.
                for _ in 0..60 {
                    if matches!(self.state.screen, GameScreen::Battle) {
                        break;
                    }
                    self.step_with(None, frames);
                }
            }
            if matches!(self.state.screen, GameScreen::Battle) {
                *battles += 1;
                match self.auto_resolve_battle(frames) {
                    BattleResolution::Resolved => continue,
                    BattleResolution::Blackout => {
                        return Some((TravelResult::Blackout, None));
                    }
                    BattleResolution::Failed => {
                        return Some((
                            TravelResult::EnteredBattle,
                            Some("battle could not be auto-resolved".to_string()),
                        ));
                    }
                }
            }
            if self.overworld.active_script_effect_label().is_some()
                || !self.overworld.script_engine_idle()
            {
                return Some((
                    TravelResult::Interrupted,
                    Some("script/cutscene took control".to_string()),
                ));
            }
            return None;
        }
        Some((
            TravelResult::Interrupted,
            Some("could not settle dialogue/battle loop".to_string()),
        ))
    }

    /// Hold `dir` until the map changes (connection crossing, or a
    /// carpet/edge warp that needs the direction held at the
    /// step-completion frame), then coast until movement settles.
    /// `Interrupted` when a battle/dialogue/script takes over mid-press
    /// (a wild encounter CAN fire on the seam step through grass).
    fn press_direction_across(
        &mut self,
        dir: Direction,
        start_map: MapId,
        frames: &mut u32,
    ) -> PressAcross {
        let button = match dir {
            Direction::Up => GbButton::Up,
            Direction::Down => GbButton::Down,
            Direction::Left => GbButton::Left,
            Direction::Right => GbButton::Right,
        };
        for _ in 0..CROSS_FRAMES {
            if self.overworld.state.current_map != start_map {
                break;
            }
            if matches!(self.state.screen, GameScreen::Battle)
                || self.overworld.pending_wild_encounter.is_some()
                || self.overworld.pending_trainer_battle.is_some()
                || self.overworld.pending_dialogue.is_some()
                || self.overworld.pending_choice.is_some()
                || self.overworld.trainer_encounter_pending()
                || self.overworld.active_script_effect_label().is_some()
                || !self.overworld.script_engine_idle()
            {
                return PressAcross::Interrupted;
            }
            self.step_with(Some(button), frames);
        }
        if self.overworld.state.current_map == start_map {
            return PressAcross::NotFired;
        }
        // Coast to a settled Idle on the new map.
        for _ in 0..60 {
            if self.overworld.state.player.movement_state
                == pokered_core::overworld::MovementState::Idle
            {
                break;
            }
            self.step_with(None, frames);
        }
        PressAcross::Crossed
    }

    /// NavGrid for `map`: live NPC overlay on the current map, no
    /// overlay for transit maps (static NPCs wander anyway).
    fn travel_grid(&self, map: MapId) -> Option<NavGrid> {
        if pokered_data::map_data_loader::get_map_json(map).is_none() {
            return None;
        }
        if map == self.overworld.state.current_map {
            return self.build_nav_grid();
        }
        let (map_data, _) =
            pokered_core::overworld::map_data_loading::load_full_map_data_concrete(map);
        Some(NavGrid::build(&map_data, &[]))
    }

    /// Execute one tile-level leg (walk segment + crossing).
    fn execute_tile_leg(
        &mut self,
        tile_leg: &TileLeg,
        frames: &mut u32,
        battles: &mut u32,
    ) -> LegExec {
        for _ in 0..MAX_LEG_ATTEMPTS {
            if let Some((result, detail)) = self.settle_for_travel(frames, battles) {
                return LegExec::Abort(result, detail);
            }
            if let Some(&target) = tile_leg.tiles.last() {
                if self.player_pos() == target {
                    // Already on the crossing tile (arrival from a prior leg).
                } else {
                    let nav = self.agent_move_to(target.0, target.1);
                    *frames += nav.frames;
                    match nav.result {
                        NavigationResult::Reached => {}
                        NavigationResult::MapChanged => {
                            let actual = self.overworld.state.current_map;
                            if let Some(TileCross::Warp { to_map, .. }) = &tile_leg.cross {
                                // Expected only when this leg ends in a warp cross.
                                if actual == *to_map {
                                    return LegExec::Done;
                                }
                                return LegExec::MapMismatch {
                                    expected: *to_map,
                                    actual,
                                };
                            }
                            if actual == tile_leg.map {
                                // Spurious transition signal (fade-in
                                // tail): the walk never happened — retry.
                                continue;
                            }
                            // Crossed somewhere unintended mid-walk; the
                            // graph-leg verification sorts out whether
                            // it's the leg's target (replan otherwise).
                            return LegExec::MapMismatch {
                                expected: tile_leg.map,
                                actual,
                            };
                        }
                        NavigationResult::EnteredBattle => {
                            *battles += 1;
                            match self.auto_resolve_battle(frames) {
                                BattleResolution::Resolved => continue,
                                BattleResolution::Blackout => {
                                    return LegExec::Abort(TravelResult::Blackout, None);
                                }
                                BattleResolution::Failed => {
                                    return LegExec::Abort(
                                        TravelResult::EnteredBattle,
                                        Some("battle could not be auto-resolved".to_string()),
                                    );
                                }
                            }
                        }
                        NavigationResult::EnteredDialogue => {
                            self.advance_dialogue(frames);
                            continue;
                        }
                        NavigationResult::Interrupted => {
                            return LegExec::Abort(
                                TravelResult::Interrupted,
                                Some("script/cutscene took control".to_string()),
                            );
                        }
                        NavigationResult::Blocked => {
                            return LegExec::Abort(TravelResult::Blocked, nav.detail);
                        }
                    }
                }
            }
            // Crossing action.
            match &tile_leg.cross {
                None => return LegExec::Done,
                Some(TileCross::Warp { to_map, warp_index }) => {
                    let current = self.overworld.state.current_map;
                    if current == *to_map {
                        return LegExec::Done;
                    }
                    if current != tile_leg.map {
                        return LegExec::MapMismatch {
                            expected: *to_map,
                            actual: current,
                        };
                    }
                    // Approach + hold-through (nav_warp semantics): walk
                    // to a reachable adjacent cell, face the warp, and
                    // press through with the direction HELD at the
                    // completion frame — entry-fired doors work the same,
                    // and edge/carpet warps require exactly this.
                    let warp_tile = tile_leg
                        .tiles
                        .last()
                        .copied()
                        .unwrap_or_else(|| self.player_pos());
                    let Some(grid) = self.build_nav_grid() else {
                        return LegExec::Abort(
                            TravelResult::Blocked,
                            Some("no map loaded".to_string()),
                        );
                    };
                    let pos = self.player_pos();
                    let Some((path, face_dir)) =
                        pokered_agent::find_approach(&grid, pos, warp_tile)
                    else {
                        return LegExec::Abort(
                            TravelResult::Blocked,
                            Some(format!(
                                "no approach to warp {warp_index} at {warp_tile:?} on {:?}",
                                tile_leg.map
                            )),
                        );
                    };
                    if let Some(&dest) = path.last() {
                        let nav = self.agent_move_to(dest.0, dest.1);
                        *frames += nav.frames;
                        match nav.result {
                            NavigationResult::Reached => {}
                            NavigationResult::MapChanged => {
                                let actual = self.overworld.state.current_map;
                                if actual == *to_map {
                                    return LegExec::Done;
                                }
                                if actual == tile_leg.map {
                                    continue; // spurious transition — retry
                                }
                                return LegExec::MapMismatch {
                                    expected: *to_map,
                                    actual,
                                };
                            }
                            NavigationResult::EnteredBattle => {
                                *battles += 1;
                                match self.auto_resolve_battle(frames) {
                                    BattleResolution::Resolved => continue,
                                    BattleResolution::Blackout => {
                                        return LegExec::Abort(TravelResult::Blackout, None);
                                    }
                                    BattleResolution::Failed => {
                                        return LegExec::Abort(
                                            TravelResult::EnteredBattle,
                                            Some(
                                                "battle could not be auto-resolved".to_string(),
                                            ),
                                        );
                                    }
                                }
                            }
                            NavigationResult::EnteredDialogue => {
                                self.advance_dialogue(frames);
                                continue;
                            }
                            NavigationResult::Interrupted => {
                                return LegExec::Abort(
                                    TravelResult::Interrupted,
                                    Some("script/cutscene took control".to_string()),
                                );
                            }
                            NavigationResult::Blocked => {
                                return LegExec::Abort(TravelResult::Blocked, nav.detail);
                            }
                        }
                    }
                    self.turn_to(face_dir, frames);
                    match self.press_direction_across(face_dir, tile_leg.map, frames) {
                        PressAcross::Crossed => {
                            let actual = self.overworld.state.current_map;
                            if actual == *to_map {
                                return LegExec::Done;
                            }
                            return LegExec::MapMismatch {
                                expected: *to_map,
                                actual,
                            };
                        }
                        PressAcross::Interrupted => continue,
                        PressAcross::NotFired => {
                            return LegExec::Abort(
                                TravelResult::Blocked,
                                Some(format!(
                                    "warp to {:?} never fired from {:?}",
                                    to_map, tile_leg.map
                                )),
                            );
                        }
                    }
                }
                Some(TileCross::Connection { to_map, direction }) => {
                    match self.press_direction_across(*direction, tile_leg.map, frames) {
                        PressAcross::Crossed => {
                            let actual = self.overworld.state.current_map;
                            if actual == *to_map {
                                return LegExec::Done;
                            }
                            return LegExec::MapMismatch {
                                expected: *to_map,
                                actual,
                            };
                        }
                        PressAcross::Interrupted => continue,
                        PressAcross::NotFired => {
                            return LegExec::Abort(
                                TravelResult::Blocked,
                                Some(format!(
                                    "connection {:?} from {:?} never crossed",
                                    direction, tile_leg.map
                                )),
                            );
                        }
                    }
                }
            }
        }
        LegExec::Abort(
            TravelResult::Blocked,
            Some(format!("tile leg on {:?} did not converge", tile_leg.map)),
        )
    }

    /// Execute one map-level route leg.
    fn execute_graph_leg(
        &mut self,
        leg: &pokered_agent::RouteLeg,
        frames: &mut u32,
        battles: &mut u32,
    ) -> LegExec {
        // LAST_MAP warp legs resolve against the live tracked last map.
        let expected = if leg.dynamic_destination {
            self.overworld.last_map.unwrap_or(leg.to)
        } else {
            leg.to
        };
        let current = self.overworld.state.current_map;
        let tile_legs = match leg.kind {
            pokered_agent::RouteLegKind::Warp => {
                // Single tile leg straight to the warp tile.
                let Some(tile) = leg.from_pos else {
                    return LegExec::Abort(
                        TravelResult::Blocked,
                        Some("warp leg without a source tile".to_string()),
                    );
                };
                let warp_index = leg.warp_index.unwrap_or(0);
                vec![TileLeg {
                    map: current,
                    tiles: if self.player_pos() == (tile.x as u16, tile.y as u16) {
                        Vec::new()
                    } else {
                        vec![(tile.x as u16, tile.y as u16)]
                    },
                    cross: Some(TileCross::Warp {
                        to_map: expected,
                        warp_index,
                    }),
                }]
            }
            pokered_agent::RouteLegKind::Connection => {
                let pos = self.player_pos();
                match find_tile_route(
                    &|map| self.travel_grid(map),
                    current,
                    pos,
                    expected,
                    self.overworld.last_map,
                ) {
                    Some(legs) => legs,
                    None => {
                        return LegExec::Abort(
                            TravelResult::Blocked,
                            Some(format!(
                                "no tile route from {:?}({},{}) to {:?}",
                                current, pos.0, pos.1, expected
                            )),
                        );
                    }
                }
            }
        };
        for tile_leg in &tile_legs {
            match self.execute_tile_leg(tile_leg, frames, battles) {
                LegExec::Done => {}
                other => return other,
            }
        }
        // Verify the leg's destination map.
        let actual = self.overworld.state.current_map;
        if actual == expected {
            LegExec::Done
        } else {
            LegExec::MapMismatch { expected, actual }
        }
    }

    /// Travel cross-map to `dest`: world routing → tile-level execution
    /// per leg → landing verification → replan on mismatch.
    pub fn agent_travel_to(&mut self, dest: MapId) -> TravelOutcome {
        let mut frames = 0u32;
        let mut battles = 0u32;
        let mut replans = 0u32;
        let mut last_mismatch: Option<(MapId, MapId)> = None;
        let mut legs: Vec<pokered_agent::RouteLeg> = Vec::new();
        let mut legs_completed = 0usize;

        let finish = |game: &Self,
                      result: TravelResult,
                      legs: Vec<pokered_agent::RouteLeg>,
                      legs_completed: usize,
                      frames: u32,
                      battles: u32,
                      mismatch: Option<(MapId, MapId)>,
                      detail: Option<String>| {
            let (x, y) = game.player_pos();
            let current = game.overworld.state.current_map;
            TravelOutcome {
                result,
                legs,
                legs_completed,
                frames,
                battles,
                final_pos: pokered_agent::Position {
                    x: x as i32,
                    y: y as i32,
                },
                final_map: format!("{:?}", current),
                expected_map: mismatch.map(|(e, _)| format!("{:?}", e)),
                actual_map: mismatch.map(|(_, a)| format!("{:?}", a)),
                detail,
            }
        };

        loop {
            let current = self.overworld.state.current_map;
            if current == dest {
                return finish(
                    self,
                    TravelResult::Reached,
                    legs,
                    legs_completed,
                    frames,
                    battles,
                    None,
                    None,
                );
            }
            if pokered_data::map_data_loader::get_map_json(dest).is_none() {
                return finish(
                    self,
                    TravelResult::InvalidTarget,
                    legs,
                    legs_completed,
                    frames,
                    battles,
                    None,
                    Some(format!("no map data for {dest:?}")),
                );
            }
            if replans > MAX_REPLANS {
                return finish(
                    self,
                    if last_mismatch.is_some() {
                        TravelResult::MapMismatch
                    } else {
                        TravelResult::Blocked
                    },
                    legs,
                    legs_completed,
                    frames,
                    battles,
                    last_mismatch,
                    Some("replan budget exhausted".to_string()),
                );
            }
            let Some(route) = WorldGraph::shared().find_route(current, dest) else {
                return finish(
                    self,
                    TravelResult::Blocked,
                    legs,
                    legs_completed,
                    frames,
                    battles,
                    None,
                    Some(format!("no route from {current:?} to {dest:?}")),
                );
            };
            legs = route;
            legs_completed = 0;

            let mut mismatch: Option<(MapId, MapId)> = None;
            for leg in legs.clone() {
                match self.execute_graph_leg(&leg, &mut frames, &mut battles) {
                    LegExec::Done => legs_completed += 1,
                    LegExec::MapMismatch { expected, actual } => {
                        mismatch = Some((expected, actual));
                        break;
                    }
                    LegExec::Abort(result, detail) => {
                        return finish(
                            self, result, legs, legs_completed, frames, battles, None, detail,
                        );
                    }
                }
            }
            match mismatch {
                None => {
                    // All legs executed; the loop top confirms arrival.
                }
                some => {
                    last_mismatch = some;
                    replans += 1;
                }
            }
        }
    }
}

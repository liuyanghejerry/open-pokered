//! `update_frame()` method and all helper methods for the OverworldScreen.
//!
//! Contains the main game loop tick, script effect processing, NPC movement,
//! wild encounter checks, and map transition logic.

use super::screen::{
    self, BedroomDialogue, ConnectionNpcPreview, EmotionBubbleState, HealingMachineState,
    MapData, OverworldAudioRequest, OverworldGameDataRequest, OverworldScreen, OverworldSfxEvent,
    PendingConnection, PendingTrainerBattle, PendingWarp, PendingWildEncounter,
    TrainerEncounterIntro,
    PokedexEntryState, WARP_FADE_IN_FRAMES, WARP_FADE_OUT_FRAMES, WARP_FADE_OUT_WHITE_FRAMES,
    WarpFadeState, build_npc_runtime_states,
};
use crate::game_state::{GameScreen, ScreenAction};
use crate::overworld::{
    collision, doors_elevators, npc_interaction, npc_movement, player_movement, presentation,
    script_bridge, special_terrain, wild_encounters,
};
use crate::overworld::script_bridge::HealingMachinePhase;
use crate::overworld::spinner_paths::spinner_paths;
use dotzuki_engine::overworld::{
    Direction, MovementState, OverworldInput, PlayerState, TransportMode,
};
use dotzuki_engine::overworld::collision::CollisionProvider;
use dotzuki_engine::overworld::map_transitions::{
    calculate_connection_transition as engine_calculate_connection_transition,
};
use dotzuki_engine::tileset::TilesetTrait;
use dotzuki_engine::GameData;
use dotzuki_engine_script::{CommandResult, MapScriptConfig};
use pokered_data::blockset_data;
use pokered_data::impl_traits::PokemonMapData;
use pokered_data::map_data_loader::get_map_json;
use pokered_data::maps::MapId;
use pokered_data::tileset_data;
use pokered_data::tilesets::TilesetId;
use player_movement::{InputState as MovementInput, MoveResult};
use std::collections::VecDeque;

// ── Free helper functions ─────────────────────────────────────────

fn resolve_npc_index(
    npc_id: &str,
    npc_states: &[npc_movement::NpcRuntimeState],
    config: &MapScriptConfig,
) -> Option<usize> {
    if let Some(idx) = script_bridge::find_npc_index_by_id(npc_states, npc_id) {
        return Some(idx);
    }
    if let Some(npc_text_id) = config.npc_id_by_script_id(npc_id) {
        return npc_states.iter().position(|n| n.text_id == npc_text_id);
    }
    if let Some(npc_text_id) = config.npc_id_by_toggle(npc_id) {
        return npc_states.iter().position(|n| n.text_id == npc_text_id);
    }
    None
}

fn is_in_tile_bounds<T: TilesetTrait>(map: &MapData<T>, x: u16, y: u16) -> bool {
    let width_tiles = (map.width as u16) * 2;
    let height_tiles = (map.height as u16) * 2;
    x < width_tiles && y < height_tiles
}

/// Whether the player's standing tile at position `x`, `y` (player units —
/// 2 tiles per unit, same space as map.json warp coordinates) is walkable:
/// in bounds, not a counter tile, and passable. Mirrors the movement
/// collision rules; reused by the editor warp resolver (safe spawns).
pub fn is_script_walkable_tile<T: TilesetTrait>(map: &MapData<T>, x: u16, y: u16) -> bool {
    if !is_in_tile_bounds(map, x, y) {
        return false;
    }
    let ts = pokered_data::tilesets::resolve_concrete(&map.tileset);
    let provider = collision::PokemonCollisionProvider::new(map.id, ts);
    let tile = provider.get_tile_at_position(ts, &map.blocks, map.width, x, y);
    let header = tileset_data::get_tileset_header(ts);
    !header.is_counter_tile(tile) && provider.is_tile_passable(ts, tile)
}

fn plan_terrain_path<T: TilesetTrait>(
    map: &MapData<T>,
    start: (u16, u16),
    target: (u16, u16),
) -> Option<Vec<(u16, u16)>> {
    if start == target {
        return Some(Vec::new());
    }
    if !is_in_tile_bounds(map, start.0, start.1) || !is_in_tile_bounds(map, target.0, target.1) {
        return None;
    }
    if !is_script_walkable_tile(map, target.0, target.1) {
        return None;
    }

    let width_tiles = (map.width as usize) * 2;
    let height_tiles = (map.height as usize) * 2;
    let total = width_tiles * height_tiles;
    let mut visited = vec![false; total];
    let mut prev: Vec<Option<(u16, u16)>> = vec![None; total];
    let mut queue = VecDeque::new();

    let index_of = |x: u16, y: u16| -> usize { (y as usize) * width_tiles + x as usize };

    let start_idx = index_of(start.0, start.1);
    visited[start_idx] = true;
    queue.push_back(start);

    while let Some((x, y)) = queue.pop_front() {
        if (x, y) == target {
            break;
        }

        for dir in [
            Direction::Down,
            Direction::Up,
            Direction::Left,
            Direction::Right,
        ] {
            let (dx, dy) = player_movement::direction_delta(dir);
            let nx_i = x as i32 + dx as i32;
            let ny_i = y as i32 + dy as i32;
            if nx_i < 0 || ny_i < 0 {
                continue;
            }
            let nx = nx_i as u16;
            let ny = ny_i as u16;
            if !is_in_tile_bounds(map, nx, ny) {
                continue;
            }
            if !is_script_walkable_tile(map, nx, ny) {
                continue;
            }

            let nidx = index_of(nx, ny);
            if visited[nidx] {
                continue;
            }

            visited[nidx] = true;
            prev[nidx] = Some((x, y));
            queue.push_back((nx, ny));
        }
    }

    let target_idx = index_of(target.0, target.1);
    if !visited[target_idx] {
        return None;
    }

    let mut rev_path = Vec::new();
    let mut cur = target;
    while cur != start {
        rev_path.push(cur);
        let idx = index_of(cur.0, cur.1);
        cur = prev[idx]?;
    }
    rev_path.reverse();
    Some(rev_path)
}

/// L-shaped path: walk X first, then Y. Ignores terrain — used as
/// fallback when `plan_terrain_path` fails (e.g. door tiles).
fn plan_straight_path(start: (u16, u16), target: (u16, u16)) -> Vec<(u16, u16)> {
    let mut path = Vec::new();
    let (mut x, mut y) = start;
    while x != target.0 {
        x = if target.0 > x { x + 1 } else { x - 1 };
        path.push((x, y));
    }
    while y != target.1 {
        y = if target.1 > y { y + 1 } else { y - 1 };
        path.push((x, y));
    }
    path
}

/// Terrain-aware path that also handles unwalkable targets (e.g. door
/// tiles): route to the closest reachable walkable neighbor of the
/// target, then append the target itself as the final step. Returns
/// `None` only when even that is impossible.
fn plan_terrain_path_allow_target_step<T: TilesetTrait>(
    map: &MapData<T>,
    start: (u16, u16),
    target: (u16, u16),
) -> Option<Vec<(u16, u16)>> {
    if let Some(path) = plan_terrain_path(map, start, target) {
        return Some(path);
    }
    if is_script_walkable_tile(map, target.0, target.1) {
        // Walkable but unreachable — nothing better to try.
        return None;
    }
    let mut best: Option<Vec<(u16, u16)>> = None;
    for dir in [
        Direction::Down,
        Direction::Up,
        Direction::Left,
        Direction::Right,
    ] {
        let (dx, dy) = player_movement::direction_delta(dir);
        let nx = target.0 as i32 + dx as i32;
        let ny = target.1 as i32 + dy as i32;
        if nx < 0 || ny < 0 {
            continue;
        }
        let neighbor = (nx as u16, ny as u16);
        if let Some(mut path) = plan_terrain_path(map, start, neighbor) {
            if best.as_ref().map_or(true, |b| path.len() < b.len()) {
                path.push(target);
                best = Some(path);
            }
        }
    }
    best
}

fn path_u16_to_u8(path: &[(u16, u16)]) -> Option<Vec<(u8, u8)>> {
    path.iter()
        .map(|&(x, y)| Some((u8::try_from(x).ok()?, u8::try_from(y).ok()?)))
        .collect()
}

fn direction_toward_player(from_x: u16, from_y: u16, to_x: u16, to_y: u16) -> Option<Direction> {
    let dx = to_x as i32 - from_x as i32;
    let dy = to_y as i32 - from_y as i32;
    if dx == 0 && dy == 0 {
        return None;
    }
    if dx.abs() > dy.abs() {
        Some(if dx > 0 {
            Direction::Right
        } else {
            Direction::Left
        })
    } else {
        Some(if dy > 0 {
            Direction::Down
        } else {
            Direction::Up
        })
    }
}

fn get_npc_text_from_json(
    map_id: MapId,
    text_id: u8,
) -> Option<Vec<pokered_data::map_json::TextPageJson>> {
    let map_json = pokered_data::map_data_loader::get_map_json(map_id)?;
    let key = text_id.to_string();
    map_json.text.npc.get(&key).cloned()
}

fn get_sign_text_from_json(
    map_id: MapId,
    text_id: u8,
) -> Option<Vec<pokered_data::map_json::TextPageJson>> {
    let map_json = pokered_data::map_data_loader::get_map_json(map_id)?;
    let key = text_id.to_string();
    map_json.text.sign.get(&key).cloned()
}

pub(crate) fn resolve_warp_destination(dest_map: MapId, dest_warp_id: u8) -> Option<(u8, u8)> {
    let map_json = get_map_json(dest_map)?;
    let idx = dest_warp_id as usize;
    if idx < map_json.warps.len() {
        Some((map_json.warps[idx].x, map_json.warps[idx].y))
    } else {
        None
    }
}

pub(crate) fn execute_warp(
    map_data: &MapData,
    px: u8,
    py: u8,
    last_map: Option<MapId>,
) -> Option<(MapId, u8, u8)> {
    let transition = dotzuki_engine::overworld::map_transitions::check_warp_at(map_data, px, py)?;
    let dest_map = if transition.is_last_map {
        last_map?
    } else {
        transition.new_map
    };
    let (dx, dy) = resolve_warp_destination(dest_map, transition.dest_warp_id)?;
    Some((dest_map, dx, dy))
}

// ── OverworldScreen game-loop methods ─────────────────────────────

impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    /// Re-baseline the button edge detectors (`prev_*_pressed`) to the
    /// buttons currently held. Frontends must call this whenever control
    /// returns to the overworld from a sub-screen (START menu, bag, save…):
    /// those screens consumed the button press while the overworld was
    /// suspended, leaving `prev_*` stale — without the re-baseline the
    /// still-held press reads as a fresh edge on the first frame back (e.g.
    /// A on START-menu EXIT instantly talks to the facing NPC). The original
    /// avoids this structurally: home/joypad.asm recomputes hJoyPressed
    /// against hJoyReleased every frame, so a press consumed by one loop can
    /// never re-fire in another.
    pub fn sync_prev_input(&mut self, a: bool, b: bool, up: bool, down: bool) {
        self.prev_a_pressed = a;
        self.prev_b_pressed = b;
        self.prev_up_pressed = up;
        self.prev_down_pressed = down;
    }

    pub fn update_frame(&mut self, input: OverworldInput) -> ScreenAction {
        self.frame_counter = self.frame_counter.wrapping_add(1);
        self.sfx_event = OverworldSfxEvent::None;
        self.audio_requests.clear();

        // Push the configured text speed into any active dialogue (the frontend
        // sets `text_delay_frames` from the options menu every frame).
        let text_delay = self.text_delay_frames;
        if let Some(ref mut dlg) = self.pending_dialogue {
            dlg.set_text_delay_frames(text_delay);
        }

        // Edge detection for the A button must be computed BEFORE any early-return
        // paths (warp fade, script effects, door auto-step, etc.) so that
        // `prev_a_pressed` stays in sync with the physical key state every frame.
        // Without this, holding A across a dialogue-creation boundary causes the
        // first page to be skipped because the held state is mistaken for a new press.
        let a_just_pressed = input.a && !self.prev_a_pressed;
        let b_just_pressed = input.b && !self.prev_b_pressed;
        let up_just_pressed = input.up && !self.prev_up_pressed;
        let down_just_pressed = input.down && !self.prev_down_pressed;
        self.prev_a_pressed = input.a;
        self.prev_b_pressed = input.b;
        self.prev_up_pressed = input.up;
        self.prev_down_pressed = input.down;

        // Link presence (Cable Club): while a link session is connected and
        // the player is inside Colosseum/TradeCenter, the app sets
        // `link_opponent` and the room's opponent NPC (text id 1 — the map
        // config binding the scene's talkOpponent trigger uses) is pinned to
        // the remote player's position every frame — the original's
        // `TradeCenter_Script` moves the opponent sprite on map entry
        // (scripts/TradeCenter.asm:17-30). Stationary so the overworld's
        // wander logic never fights the override.
        if let Some(presence) = self.link_opponent {
            if let Some(npc) = self.npc_states.iter_mut().find(|n| n.text_id == 1) {
                npc.x = presence.x;
                npc.y = presence.y;
                npc.facing = presence.facing;
                npc.movement_type = dotzuki_engine::overworld::NpcMovementType::Stationary;
                npc.visible = true;
            }
        }

        // UpdateMovingBgTiles: water/flower tile animation ticks every frame
        // (vblank-driven in the original).
        self.tile_anim.tick();

        // ITEMFINDER dings: 4× (SFX_HEALING_MACHINE, SFX_PURCHASE), metered
        // one per ITEMFINDER_DING_FRAMES (ItemUseItemfinder's
        // PlaySoundWaitForCurrent loop, item_effects.asm:1928-1935).
        if let Some((remaining, mut frames)) = self.itemfinder_dings {
            if frames == 0 {
                let sound_id = if remaining % 2 == 0 {
                    "SFX_HEALING_MACHINE"
                } else {
                    "SFX_PURCHASE"
                };
                self.audio_requests.push(OverworldAudioRequest::PlaySound {
                    sound_id: sound_id.to_string(),
                });
                frames = crate::overworld::hidden_items::ITEMFINDER_DING_FRAMES;
                let remaining = remaining - 1;
                self.itemfinder_dings = if remaining == 0 {
                    None
                } else {
                    Some((remaining, frames))
                };
            } else {
                self.itemfinder_dings = Some((remaining, frames - 1));
            }
        }

        // FLASH white-out frames after a dark cave is lit. GBPalWhiteOutWithDelay3
        // blocks ALL gameplay for its 3-frame Delay3 (the overworld loop sits
        // inside the routine, home/fade.asm) — the renderer whites the screen
        // while `flash_lit_frames > 0` and movement/NPCs stay frozen here.
        if self.flash_lit_frames > 0 {
            self.flash_lit_frames -= 1;
            return ScreenAction::Continue;
        }

        // Naming screen open/submit white flash (GBPalWhiteOutWithDelay3,
        // naming_screen.asm:88/163) — same blocking Delay3 as above.
        if self.naming_flash_frames > 0 {
            self.naming_flash_frames -= 1;
            return ScreenAction::Continue;
        }

        // TELEPORT/DIG/ESCAPE ROPE spin-out (_LeaveMapAnim): freeze gameplay
        // while the player spins and rises off screen; the warp fade-out
        // (GBFadeOutToWhite) starts when the spin finishes.
        if let Some(mut spin) = self.teleport_spin.take() {
            if let Some(sfx) = spin.tick() {
                self.audio_requests.push(OverworldAudioRequest::PlaySound {
                    sound_id: presentation::teleport_spin_sfx(sfx).to_string(),
                });
            }
            if spin.is_done() {
                self.warp_fade_state = WarpFadeState::FadingOut {
                    frames_remaining: WARP_FADE_OUT_WHITE_FRAMES,
                };
            } else {
                self.teleport_spin = Some(spin);
            }
            return ScreenAction::Continue;
        }

        // ShakeElevator: freeze gameplay while the screen shakes.
        if let Some(mut shake) = self.elevator_shake.take() {
            if let Some(sfx) = shake.tick() {
                self.audio_requests.push(OverworldAudioRequest::PlaySound {
                    sound_id: presentation::elevator_shake_sfx(sfx).to_string(),
                });
            }
            if !shake.is_done() {
                self.elevator_shake = Some(shake);
            }
            return ScreenAction::Continue;
        }

        // S.S. Anne departure cutscene (VermilionDockSSAnneLeavesScript +
        // VermilionDock_EraseSSAnne, scripts/VermilionDock.asm): the ship
        // sails away with smoke puffs and an east view scroll. Freezes
        // gameplay like the original's blocking routine; the ship's map
        // blocks become water and the dock→ship warp is removed
        // (wNumberOfWarps--) when the erase phase begins.
        if let Some(mut dep) = self.ship_departure.take() {
            let prev_phase = dep.phase();
            if let Some(sfx) = dep.tick() {
                self.audio_requests.push(OverworldAudioRequest::PlaySound {
                    sound_id: presentation::ship_departure_sfx(sfx).to_string(),
                });
            }
            if dep.phase() == presentation::ShipDeparturePhase::Erase
                && prev_phase != presentation::ShipDeparturePhase::Erase
            {
                self.apply_ss_anne_departure_erase();
            }
            if !dep.is_done() {
                self.ship_departure = Some(dep);
            }
            return ScreenAction::Continue;
        }

        match self.warp_fade_state {
            WarpFadeState::FadingOut { frames_remaining } => {
                if frames_remaining <= 1 {
                    self.warp_fade_state = WarpFadeState::BlackScreen;
                } else {
                    self.warp_fade_state = WarpFadeState::FadingOut {
                        frames_remaining: frames_remaining - 1,
                    };
                }
                return ScreenAction::Continue;
            }
            WarpFadeState::BlackScreen => {
                self.commit_pending_warp();
                self.warp_fade_state = WarpFadeState::FadingIn {
                    frames_remaining: WARP_FADE_IN_FRAMES,
                };
                return ScreenAction::Continue;
            }
            WarpFadeState::FadingIn { frames_remaining } => {
                if frames_remaining <= 1 {
                    self.warp_fade_state = WarpFadeState::Idle;
                    // Fade complete; the next warp defaults to fade-to-black.
                    self.warp_fade_to_white = false;
                } else {
                    self.warp_fade_state = WarpFadeState::FadingIn {
                        frames_remaining: frames_remaining - 1,
                    };
                }
                return ScreenAction::Continue;
            }
            WarpFadeState::Idle => {}
        }

        // EnterMapAnim arrival spin-in (player_animations.asm:1-91): once
        // the fade-in-from-white completes, the player descends from off the
        // top of the screen and spins in place (FLY / TELEPORT / DIG / ESCAPE
        // ROPE / dungeon-warp arrivals). Gameplay stays frozen while it
        // runs, like the original's blocking routine.
        if let Some(mut anim) = self.enter_map_anim.take() {
            if let Some(sfx) = anim.tick() {
                self.audio_requests.push(OverworldAudioRequest::PlaySound {
                    sound_id: presentation::enter_map_spin_sfx(sfx).to_string(),
                });
            }
            if !anim.is_done() {
                self.enter_map_anim = Some(anim);
            }
            return ScreenAction::Continue;
        }

        // Elevator arrival shake (SilphCoElevatorShakeScript): starts once the
        // elevator's warp-to-floor has fully completed.
        if self.elevator_shake_pending
            && matches!(self.warp_fade_state, WarpFadeState::Idle)
            && self.pending_warp.is_none()
            && self.active_script_effect.is_none()
        {
            self.elevator_shake_pending = false;
            self.elevator_shake = Some(presentation::ElevatorShakeState::new(
                doors_elevators::elevator_shake_params(),
            ));
            return ScreenAction::Continue;
        }

        // ── Script engine tick ────────────────────────────────────────
        // While a FollowNpc effect is active, suppress warp triggers: the
        // player's shadow-walk trail can pass over a door warp tile (e.g.
        // Pallet Town's Oak escort crosses (12,11)), and warping mid-follow
        // freezes the script and drops the leftover effect at commit — the
        // hideObject cleanup right after followNpc never runs. The follow
        // finishes first; the scene then walks the player onto the door
        // tile deliberately (movePlayer) to trigger the warp safely.
        let effect_was_follow_npc = matches!(
            self.active_script_effect,
            Some(script_bridge::ScriptEffect::FollowNpc { .. })
        );
        if let Some(ref mut effect) = self.active_script_effect {
            let naming_was_open = self.pending_naming_screen.is_some();
            let done = Self::tick_active_effect(
                effect,
                a_just_pressed,
                b_just_pressed,
                input.a,
                up_just_pressed,
                down_just_pressed,
                &mut self.pending_dialogue,
                &mut self.pending_choice,
                &mut self.pending_pokedex_entry,
                &mut self.pending_naming_screen,
                &mut self.party_select_requested,
                &mut self.pending_emotion_bubble,
                &mut self.pending_healing_machine,
                &mut self.npc_states,
                &self.state.player,
                &mut self.scripted_player_path,
                self.map_data.as_ref(),
                &self.map_script_config,
                self.party_count,
                &mut self.audio_requests,
                self.state.current_map,
                &mut self.sfx_event,
                &mut self.ship_departure,
            );
            if !naming_was_open && self.pending_naming_screen.is_some() {
                // DisplayNamingScreen entry: GBPalWhiteOutWithDelay3 before the
                // screen is drawn (naming_screen.asm:88).
                self.naming_flash_frames = crate::naming_screen::NAMING_FLASH_FRAMES;
            }
            if done {
                // `giveItem` must report whether the bag had room (the original's
                // GiveItem carry flag). Capture the item id while `effect` is still
                // borrowed; the room check runs after the borrow ends, against the
                // frame-seeded bag snapshot (same source `hasItem` reads).
                let give_item_id = match effect {
                    script_bridge::ScriptEffect::GiveItem { item_id, .. } => Some(item_id.clone()),
                    _ => None,
                };
                let base_result = Self::finish_effect(effect);
                // StartBattle must SUSPEND the script (not resume it now): the
                // battle hasn't run yet, so the `await game.startBattle(...)`
                // result is delivered later by `resume_script_after_battle`.
                let awaiting_battle = matches!(
                    effect,
                    script_bridge::ScriptEffect::StartBattle { .. }
                        | script_bridge::ScriptEffect::StartWildBattle { .. }
                        | script_bridge::ScriptEffect::OldManTutorial
                );
                // ElevatorMenu also suspends: the app opens the floor menu and
                // delivers the chosen index via `resume_script_after_elevator`.
                let awaiting_elevator = matches!(
                    effect,
                    script_bridge::ScriptEffect::ElevatorMenu { .. }
                );
                // FilterBag likewise suspends until the app returns the chosen
                // item (or cancel) via `resume_script_after_filter_bag`.
                let awaiting_filter_bag = matches!(
                    effect,
                    script_bridge::ScriptEffect::FilterBag { .. }
                );
                // TradePokemon suspends too: the app plays the trade cutscene
                // (engine/movie/trade.asm), applies the party mutation, then
                // delivers whether the offered mon was held via
                // `resume_script_after_trade`.
                let awaiting_trade = matches!(
                    effect,
                    script_bridge::ScriptEffect::TradePokemon { .. }
                );
                let effect_done = self.active_script_effect.take();
                // The bag mutation itself is still queued by `apply_finished_effect`
                // below and applied by the app layer; here we only compute the
                // await result the scene branches on. `true` when the item can be
                // taken (already held, or a free slot exists), `false` when full.
                let result = if let Some(item_id) = give_item_id {
                    let held = self.script_bag_names.iter().any(|n| *n == item_id);
                    let has_room = held
                        || self.script_bag_names.len() < crate::items::inventory::BAG_ITEM_CAPACITY;
                    CommandResult::Bool(has_room)
                } else {
                    base_result
                };
                if let Some(ref eff) = effect_done {
                    log::info!(target: "pokered::overworld", "[Script] Effect done: {:?}", std::mem::discriminant(eff));
                }
                self.apply_finished_effect(effect_done);
                if awaiting_battle {
                    // Hold the script; it resumes when the battle ends.
                    self.script_awaiting_battle = true;
                } else if awaiting_elevator {
                    // Hold the script; it resumes when the app returns the
                    // chosen floor via `resume_script_after_elevator`.
                    self.script_awaiting_elevator = true;
                } else if awaiting_filter_bag {
                    // Hold the script; it resumes when the app returns the
                    // chosen item via `resume_script_after_filter_bag`.
                    self.script_awaiting_filter_bag = true;
                } else if awaiting_trade {
                    // Hold the script; it resumes when the app finishes the
                    // trade cutscene (or rejects the trade) via
                    // `resume_script_after_trade`.
                    self.script_awaiting_trade = true;
                } else if let Ok(Some(next_cmd)) = self.script_engine.signal_done(result) {
                    log::info!(target: "pokered::overworld", "[Script] Next command: {:?}", std::mem::discriminant(&next_cmd));
                    self.active_script_effect = Some(script_bridge::dispatch_command_with_names(
                        &next_cmd,
                        &self.player_name,
                        &self.rival_name,
                        &self.starter_display_name(),
                    ));
                }
                self.sync_flags_from_engine();
            }
            // NPC movement must continue during script effects so that
            // MoveNpc / StartNpcMove / AwaitNpcMove effects can complete.
            self.run_npc_movement_tick();

            // Scripted player path must also advance during script effects so
            // that MovePlayer can observe the path draining. Without this, the
            // MovePlayer effect waits for scripted_player_path to empty, but
            // the standalone path-following block (below) is unreachable while
            // active_script_effect is Some — causing a deadlock.
            let pos_before = (self.state.player.x, self.state.player.y);
            self.advance_scripted_player_path();
            // Only check warps when a step actually completed and the position
            // changed.  Checking on the frame that *starts* a walk would fire
            // on the old (warp-tile) position before the player has moved away,
            // causing an immediate re-warp (e.g. entering OaksLab at the door
            // tile and getting warped back to PalletTown).
            let pos_after = (self.state.player.x, self.state.player.y);
            if pos_before != pos_after && !effect_was_follow_npc {
                self.try_trigger_warp_at_player_position();
            }

            return ScreenAction::Continue;
        }
        if let Some(cmd) = self.script_engine.tick() {
            self.active_script_effect = Some(script_bridge::dispatch_command_with_names(
                &cmd,
                &self.player_name,
                &self.rival_name,
                &self.starter_display_name(),
            ));
            return ScreenAction::Continue;
        }

        // ── Cutscene management ────────────────────────────────────────
        // When a cutscene is active and the script engine is idle,
        // either start or end the cutscene.
        if self.cutscene_manager.needs_start() {
            if let Some(script_name) = self.cutscene_manager.current_script_name().map(|s| s.to_string()) {
                if self.script_engine.has_function(&script_name) {
                    self.script_engine.set_player_position(
                        self.state.player.x as u8,
                        self.state.player.y as u8,
                    );
                    if let Ok(Some(cmd)) = self.script_engine.call_function_no_args(&script_name) {
                        self.cutscene_manager.mark_started();
                        self.active_script_effect =
                            Some(script_bridge::dispatch_command_with_names(
                                &cmd,
                                &self.player_name,
                                &self.rival_name,
                                &self.starter_display_name(),
                            ));
                        self.sync_flags_from_engine();
                        return ScreenAction::Continue;
                    }
                }
            }
        } else if self.cutscene_manager.is_active()
            && self.active_script_effect.is_none()
            && self.script_engine.is_idle()
        {
            // Script finished → end current and start next if queued.
            let _next = self.cutscene_manager.end_cutscene();
            // If `end_cutscene` started a new cutscene, `needs_start` will
            // be true on the next frame and the logic above will kick in.
        }

        // Door exit auto-step (PlayerStepOutFromDoor / BIT_EXITING_DOOR).
        // When exiting_door is active, advance the walk animation ignoring real input.
        if self.state.exiting_door {
            if self.state.player.movement_state != MovementState::Idle {
                let step_done = player_movement::advance_step(&mut self.state);
                if step_done {
                    self.state.exiting_door = false;
                }
            } else {
                self.state.exiting_door = false;
            }
            return ScreenAction::Continue;
        }

        // Initiate the auto-step when standing_on_door is flagged after a warp.
        if self.state.standing_on_door {
            self.state.standing_on_door = false;
            self.state.player.facing = Direction::Down;
            self.state.player.movement_state = MovementState::Walking;
            self.state.walk_counter = player_movement::WALK_COUNTER_INIT;
            self.state.exiting_door = true;
            return ScreenAction::Continue;
        }

        // Scripted player movement — follow path ignoring real input.
        if !self.scripted_player_path.is_empty() {
            let pos_before = (self.state.player.x, self.state.player.y);
            self.advance_scripted_player_path();
            let pos_after = (self.state.player.x, self.state.player.y);
            if pos_before != pos_after {
                self.try_trigger_warp_at_player_position();
            }
            self.run_npc_movement_tick();
            return ScreenAction::Continue;
        }

        // While a dialogue box is active, consume A/B-button to advance pages;
        // block all movement and Start input.
        if let Some(ref mut dlg) = self.pending_dialogue {
            if dlg.holding_open() {
                // HoldTextDisplayOpen: keep text box open while A is held
                if !input.a || b_just_pressed {
                    self.pending_dialogue = None;
                }
            } else if a_just_pressed || b_just_pressed {
                if dlg.waiting_for_input() {
                    // Page fully revealed → advance to next page
                    if dlg.is_last_page() && a_just_pressed {
                        // Last page + A pressed → start holding open
                        dlg.start_holding_open();
                    } else if !dlg.advance() {
                        self.pending_dialogue = None;
                    }
                } else {
                    // Still typing → skip to full page reveal
                    dlg.skip_to_full_page();
                }
                self.sfx_event = OverworldSfxEvent::TextAdvance;
            } else {
                // No button pressed → advance typewriter
                dlg.reveal_next_char();
            }
            return ScreenAction::Continue;
        }

        // FishingAnim (player_animations.asm:378-469): the rod animation
        // freezes gameplay like the original's blocking DelayFrames loops
        // (the player sprite shake, the "!" bubble, and the rod piece are
        // rendered from this state). When it finishes, the rod response text
        // is queued — and on a bite the hooked mon's battle is armed so
        // home/overworld.asm's `.newBattle` fires once that text closes.
        if let Some(mut anim) = self.fishing_anim.take() {
            anim.tick();
            if anim.is_done() {
                if let Some(pf) = self.pending_fishing.take() {
                    use crate::overworld::fishing::{response_text, RodResponse};
                    let text = self.localize_message(response_text(pf.response));
                    self.pending_dialogue = Some(BedroomDialogue::from_message(&text));
                    if let RodResponse::Bite { species, level } = pf.response {
                        self.post_dialogue_battle = Some(PendingWildEncounter {
                            species,
                            level,
                            old_man: false,
                            hooked: true,
                        });
                    }
                }
            } else {
                self.fishing_anim = Some(anim);
            }
            return ScreenAction::Continue;
        }

        // Start the rod animation once the "You used the <ROD>!" text is
        // dismissed. The original (FishingInit, item_effects.asm:1906-1911)
        // prints the item-use text, then plays SFX_HEAL_AILMENT and waits
        // 80 frames, and only then runs the blocking FishingAnim. The cast
        // pause freezes gameplay like the original's DelayFrames; the
        // response (rolled at item use, 1869-1877) rides along until the
        // animation completes.
        if let Some(pf) = self.pending_fishing.take() {
            use crate::overworld::fishing::RodResponse;
            if self.fishing_cast_delay > 0 {
                self.fishing_cast_delay -= 1;
                if self.fishing_cast_delay > 0 {
                    self.pending_fishing = Some(pf);
                    return ScreenAction::Continue;
                }
                // The pause elapsed this frame: kick off FishingAnim.
                self.fishing_anim = Some(presentation::FishingAnimState::new(
                    self.state.player.facing,
                    matches!(pf.response, RodResponse::Bite { .. }),
                ));
                self.pending_fishing = Some(pf);
                return ScreenAction::Continue;
            }
            // First frame after the text closed: start the 80-frame pause
            // (FishingInit's `ld c, 80; call DelayFrames`) with the
            // pre-cast SFX_HEAL_AILMENT.
            self.fishing_cast_delay = 80;
            self.audio_requests.push(OverworldAudioRequest::PlaySound {
                sound_id: "SFX_HEAL_AILMENT".to_string(),
            });
            self.pending_fishing = Some(pf);
            return ScreenAction::Continue;
        }

        // FLASH lit a dark cave: the message has been dismissed — white out
        // the screen for a few frames (GBPalWhiteOutWithDelay3).
        if self.flash_pending_white && self.pending_dialogue.is_none() {
            self.flash_pending_white = false;
            self.flash_lit_frames = presentation::FLASH_WHITE_FRAMES;
        }

        // Safari Zone game-over eject: once the "SAFARI GAME is over!" message has
        // been dismissed (pending_dialogue cleared above), warp the player back to
        // the Safari Zone gate via a normal fade transition.
        if let Some(warp) = self.safari_eject_pending.take() {
            self.pending_warp = Some(warp);
            self.warp_fade_state = WarpFadeState::FadingOut {
                frames_remaining: WARP_FADE_OUT_FRAMES,
            };
            self.sfx_event = OverworldSfxEvent::GoOutside;
            return ScreenAction::Continue;
        }

        // Deferred post-dialogue warp (field TELEPORT): the "Warp to the last
        // #MON CENTER." text has been dismissed — play the leave-map spin
        // (_LeaveMapAnim escape-warp path); the fade starts when it finishes.
        if let Some(warp) = self.post_dialogue_warp.take() {
            self.pending_warp = Some(warp);
            self.warp_fade_to_white = true;
            self.teleport_spin = Some(presentation::TeleportSpinState::new(
                self.state.player.facing,
                presentation::TELEPORT_SPIN_FACINGS,
            ));
            return ScreenAction::Continue;
        }

        // Deferred post-dialogue wild battle (fishing rods): the rod's result
        // text ("Oh! It's a bite!") has been dismissed — hand the hooked mon
        // to the app as a pending wild encounter (home/overworld.asm's
        // `.newBattle`, reached when wCurOpponent != 0 after the item flow).
        if let Some(encounter) = self.post_dialogue_battle.take() {
            self.pending_wild_encounter = Some(encounter);
            return ScreenAction::Continue;
        }

        // Tick down a trainer-engage "!" bubble. Bubbles raised by script
        // effects (ShowEmotionBubble) are ticked by tick_active_effect, but
        // the trainer-LOS path below creates the bubble WITHOUT a script
        // effect — without this tick its countdown never moves and the
        // engage intro would wait on `frames_remaining == 0` forever.
        if let Some(ref mut bubble) = self.pending_emotion_bubble {
            if bubble.frames_remaining == 0 {
                self.pending_emotion_bubble = None;
            } else {
                bubble.frames_remaining -= 1;
            }
        }

        // Trainer line-of-sight detection: runs every frame during normal gameplay.
        // When a trainer spots the player, run the ENGAGE INTRO first
        // (CheckFightingMapTrainers, home/trainers.asm:129-159): "!" bubble →
        // trainer walks up to the player → THEN the battle. The intro state
        // replaces the instant-battle path; `pending_trainer_battle` is set
        // when the intro completes.
        if self.pending_trainer_battle.is_none() && self.trainer_encounter_intro.is_none() {
            if let Some(sighting) = npc_interaction::check_trainer_line_of_sight(
                &self.npc_states,
                &self.npc_pokemon_data,
                self.state.player.x,
                self.state.player.y,
            ) {
                let tc = pokered_data::trainer_data::TrainerClass::from_u8(sighting.trainer_class);
                let trainer_id =
                    pokered_data::trainer_data::make_trainer_id(tc, sighting.trainer_set);
                let end_battle_text = self
                    .npc_pokemon_data
                    .get(sighting.npc_index as usize)
                    .and_then(|d| d.end_battle_text.clone());
                // PlayTrainerMusic (home/trainers.asm:390-443): RIVAL1/2/3
                // keep the current music; gym leaders keep theirs too
                // (wGymLeaderNo — identified by class here); everyone else
                // gets MEET_EVIL / MEET_FEMALE / MEET_MALE by the lists in
                // data/trainers/encounter_types.asm.
                let music = crate::battle::trainer_encounter::encounter_music(tc);
                if let Some(id) = music {
                    let name = format!("{:?}", id); // enum Debug → name
                    self.audio_requests
                        .push(OverworldAudioRequest::PlayMusic { music_id: name });
                }
                // EXCLAMATION_BUBBLE over the trainer (predef EmotionBubble,
                // trainers.asm:132-135), held for 32 frames while input is
                // locked.
                self.pending_emotion_bubble = Some(EmotionBubbleState {
                    npc_id: String::new(), // resolved by index below
                    emotion: "exclamation".to_string(),
                    frames_remaining: 32,
                });
                // TrainerWalkUpToPlayer_Bank0: the trainer closes the sight
                // line to face the player. Compute the straight path.
                let npc = &self.npc_states[sighting.npc_index as usize];
                let mut path: Vec<(u8, u8)> = Vec::new();
                let (mut x, mut y) = (npc.x, npc.y);
                let (dx, dy) = (
                    self.state.player.x as i32 - npc.x as i32,
                    self.state.player.y as i32 - npc.y as i32,
                );
                // Walk along the dominant axis first (the original walks the
                // sight line: one axis), stopping one tile short of the player.
                let step_x = dx.signum();
                let step_y = dy.signum();
                let dist = (dx.abs() + dy.abs()).saturating_sub(1); // keep 1 tile gap
                for _ in 0..dist {
                    if dx.abs() >= dy.abs() {
                        x = (x as i32 + step_x) as u16;
                    } else {
                        y = (y as i32 + step_y) as u16;
                    }
                    path.push((x as u8, y as u8));
                }
                let npc_index = sighting.npc_index as usize;
                if !path.is_empty() {
                    npc_movement::start_scripted_move(
                        &mut self.npc_states[npc_index],
                        &path,
                    );
                }
                self.trainer_encounter_intro = Some(TrainerEncounterIntro {
                    trainer_id,
                    npc_index: sighting.npc_index,
                    end_battle_text,
                    rival_triplet_base: None,
                });
            }
        }

        // Advance the engage-intro: once the "!" bubble is gone AND the
        // walk-up finished, hand over to the actual pending battle.
        if let Some(intro) = self.trainer_encounter_intro.take() {
            let bubble_done = self
                .pending_emotion_bubble
                .as_ref()
                .map_or(true, |b| b.frames_remaining == 0);
            let walk_done = self
                .npc_states
                .get(intro.npc_index as usize)
                .map_or(true, |n| npc_movement::is_scripted_move_done(n));
            if bubble_done && walk_done {
                self.pending_trainer_battle = Some(PendingTrainerBattle {
                    trainer_id: intro.trainer_id,
                    npc_index: intro.npc_index,
                    end_battle_text: intro.end_battle_text,
                    rival_triplet_base: intro.rival_triplet_base,
                });
            } else {
                self.trainer_encounter_intro = Some(intro);
            }
        }

        // A-button: check signs first, then NPCs (matches original game priority).
        if a_just_pressed && self.state.player.movement_state == MovementState::Idle {
            // Check tile-based OnInteract triggers first
            {
                let (dx, dy) = player_movement::direction_delta(self.state.player.facing);
                let facing_x = (self.state.player.x as i32 + dx as i32).max(0) as u32;
                let facing_y = (self.state.player.y as i32 + dy as i32).max(0) as u32;
                let map_key = script_bridge::map_id_to_script_key(self.state.current_map);
                if let Some(fn_name) = self.trigger_manager.check_interact_mut(&map_key, facing_x, facing_y) {
                    if self.script_engine.has_function(&fn_name) {
                        self.script_engine.set_player_position(
                            self.state.player.x as u8,
                            self.state.player.y as u8,
                        );
                        if let Ok(Some(cmd)) = self.script_engine.call_function_no_args(&fn_name) {
                            self.active_script_effect =
                                Some(script_bridge::dispatch_command_with_names(
                                    &cmd,
                                    &self.player_name,
                                    &self.rival_name,
                                    &self.starter_display_name(),
                                ));
                            self.sync_flags_from_engine();
                            return ScreenAction::Continue;
                        }
                    }
                }
            }

            // Hidden items (home/overworld.asm:89-96): the hidden-event check
            // runs BEFORE the sign/sprite check, and a matched spot consumes
            // the A press even when the item was already taken (hItemAlreadyFound
            // = 0 loops the overworld without DisplayTextID).
            {
                let (dx, dy) = player_movement::direction_delta(self.state.player.facing);
                let facing_x = self.state.player.x as i32 + dx as i32;
                let facing_y = self.state.player.y as i32 + dy as i32;
                if facing_x >= 0 && facing_y >= 0 {
                    let (fx, fy) = (facing_x as u8, facing_y as u8);
                    if pokered_data::hidden_items::find_hidden_item(
                        self.state.current_map,
                        fx,
                        fy,
                    )
                    .is_some()
                    {
                        self.handle_hidden_item(fx, fy);
                        return ScreenAction::Continue;
                    }
                    // Hidden coins (Game Corner floor spots) — the ref checks
                    // them in the same hidden-event pass.
                    if self.handle_hidden_coin(fx, fy) {
                        return ScreenAction::Continue;
                    }
                }
            }

            if let Some(map) = &self.map_data {
                // Copy early to avoid borrow conflict with mutable self calls below.
                let map_tileset = map.tileset;
                let sign_tuples: Vec<(u8, u8, u8)> =
                    map.signs.iter().map(|s| (s.x, s.y, s.text_id)).collect();

                if let Some(sign_text_id) = npc_interaction::check_sign_interaction(
                    &sign_tuples,
                    self.state.player.x,
                    self.state.player.y,
                    self.state.player.facing,
                ) {
                    if self.try_call_script_sign_talk(sign_text_id) {
                        return ScreenAction::Continue;
                    }
                    if let Some(text_pages) =
                        get_sign_text_from_json(self.state.current_map, sign_text_id)
                    {
                        if !text_pages.is_empty() {
                            let pages = self.localize_text_pages(&text_pages);
                            self.pending_dialogue = Some(BedroomDialogue::from_text_pages(
                                &pages,
                                &self.player_name,
                                &self.rival_name,
                                &self.starter_display_name(),
                            ));
                            return ScreenAction::Continue;
                        }
                    }
                }

                let interaction = npc_interaction::try_interact(
                    &self.npc_states,
                    &self.npc_pokemon_data,
                    self.state.player.x,
                    self.state.player.y,
                    self.state.player.facing,
                    self.map_data.as_ref(),
                    &collision::PokemonCollisionProvider::new(self.state.current_map, map_tileset),
                );

                match interaction {
                    npc_interaction::InteractionResult::Talk { npc_index, text_id }
                    | npc_interaction::InteractionResult::AlreadyDefeated { npc_index, text_id } => {
                        self.npc_face_player(npc_index);
                        if self.try_call_script_npc_talk(text_id) {
                            return ScreenAction::Continue;
                        }
                        if self.try_show_npc_json_text(text_id) {
                            return ScreenAction::Continue;
                        }
                    }
                    npc_interaction::InteractionResult::TrainerBattle {
                        npc_index,
                        trainer_class,
                        trainer_set,
                    } => {
                        let face_dir =
                            player_movement::opposite_direction(self.state.player.facing);
                        if let Some(npc) = self
                            .npc_states
                            .iter_mut()
                            .find(|n| n.npc_index == npc_index)
                        {
                            npc.facing = face_dir;
                            let text_id = npc.text_id;
                            if self.try_call_script_npc_talk(text_id) {
                                return ScreenAction::Continue;
                            }
                            if self.try_show_npc_json_text(text_id) {
                                return ScreenAction::Continue;
                            }
                        }
                        // Start a trainer battle via pending_trainer_battle
                        let tc = pokered_data::trainer_data::TrainerClass::from_u8(trainer_class);
                        let trainer_id =
                            pokered_data::trainer_data::make_trainer_id(tc, trainer_set);
                        let end_battle_text = self
                            .npc_pokemon_data
                            .get(npc_index as usize)
                            .and_then(|d| d.end_battle_text.clone());
                        self.pending_trainer_battle = Some(PendingTrainerBattle {
                            trainer_id,
                            npc_index,
                            end_battle_text,
                            rival_triplet_base: None,
                        });
                    }
                    npc_interaction::InteractionResult::ItemPickup { npc_index, .. } => {
                        self.npc_face_player(npc_index);
                        let text_id = self
                            .npc_states
                            .iter()
                            .find(|n| n.npc_index == npc_index)
                            .map(|n| n.text_id)
                            .unwrap_or(0);
                        if text_id > 0 {
                            if self.try_call_script_npc_talk(text_id) {
                                return ScreenAction::Continue;
                            }
                            if self.try_show_npc_json_text(text_id) {
                                return ScreenAction::Continue;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Tick pending connection walk and apply the map swap when done.
        // During the walk the viewport stays in the old coordinate system
        // (smooth scroll); new-map NPCs are rendered via connection_npc_preview.
        if self.pending_connection.is_some() {
            if self.state.walk_counter > 0 {
                let dec = if self.state.player.transport == TransportMode::Biking
                    && self.state.player.bike_speedup_active
                {
                    2
                } else {
                    1
                };
                self.state.walk_counter = self.state.walk_counter.saturating_sub(dec);
            }
            if self.state.walk_counter == 0 {
                if let Some(pc) = self.pending_connection.take() {
                    self.state.player.movement_state = MovementState::Idle;
                    if pc.save_last_map {
                        self.last_map = Some(pc.old_map);
                    }
                    let new_map = pc.transition.new_map;
                    self.state.current_map = new_map;
                    // EnterMap on the new map: STRENGTH wears off, the dark-cave
                    // state follows the map, and city maps are marked visited.
                    self.strength_active = false;
                    self.tried_push_boulder = false;
                    self.boulder_dust_frames = 0;
                    self.dark_cave.enter_map(new_map);
                    if pokered_data::map_flags::is_city_map(new_map) {
                        self.game_data_requests
                            .push(OverworldGameDataRequest::MarkTownVisited { map: new_map });
                    }
                    let (map_data, npc_pokemon_data) =
                        crate::overworld::map_data_loading::load_full_map_data(new_map, self.game_data.tileset_provider());
                    self.map_data = Some(map_data);
                    self.npc_pokemon_data = npc_pokemon_data;
                    // LoadTilesetHeader: hTileAnimations follows the new tileset.
                    if let Some(ref md) = self.map_data {
                        self.tile_anim.set_tileset(presentation::tile_anim_kind(
                            pokered_data::tileset_data::get_tileset_header(md.tileset).animation,
                        ));
                    }
                    self.load_map_script(new_map);
                    self.audio_requests
                        .push(OverworldAudioRequest::PlayMapMusic { map: new_map });
                    let hidden_npc_ids = self.map_script_config.hidden_npc_ids();
                    self.npc_states = self
                        .map_data
                        .as_ref()
                        .map(|md| build_npc_runtime_states(&md.npcs, &self.npc_pokemon_data, &hidden_npc_ids))
                        .unwrap_or_default();
                    self.apply_hidden_object_flags();
                    self.state.player.x = pc.transition.new_x;
                    self.state.player.y = pc.transition.new_y;
                    // EnterMap: CheckForceBikeOrSurf — walking across the
                    // Route 16/18 road ends mounts and locks the bike; the
                    // gates release it (auto-dismount).
                    self.apply_map_entry_transport(new_map, self.state.player.x, self.state.player.y);
                    // CollisionCheckOnWater .stopSurfing across a connection:
                    // crossing onto a passable land tile ends the surf (the
                    // collision layer allowed the cross).
                    self.dismount_surf_if_on_land();
                    self.connection_npc_preview = None;
                }
            }
            self.run_npc_movement_tick();
            return ScreenAction::Continue;
        }

        if self.cutscene_manager.is_blocking() {
            self.run_npc_movement_tick();
            return ScreenAction::Continue;
        }

        if input.start {
            return ScreenAction::Transition(GameScreen::StartMenu);
        }

        let movement_input = MovementInput {
            up: input.up,
            down: input.down,
            left: input.left,
            right: input.right,
            a_button: input.a,
            b_button: input.b,
            start: input.start,
            select: input.select,
        };

        let get_tile_id_at_position =
            |blocks: &[u8], width: u8, tileset: G::Tileset, x: u16, y: u16| -> u8 {
                let block_x = (x / 2) as usize;
                let block_y = (y / 2) as usize;
                let sub_x = (x % 2) as usize;
                let sub_y = (y % 2) as usize;

                if block_x < width as usize {
                    let block_idx = block_y * (width as usize) + block_x;
                    if block_idx < blocks.len() {
                        let block_id = blocks[block_idx];
                        let concrete = pokered_data::tilesets::resolve_concrete(&tileset);
                        return blockset_data::block_tiles(concrete, block_id)
                            .map(|t| t[(sub_y * 2 + 1) * 4 + sub_x * 2])
                            .unwrap_or(0);
                    }
                }
                0
            };

        if let Some(map) = &self.map_data {
            let prev_x = self.state.player.x;
            let prev_y = self.state.player.y;

            let collision_provider = collision::PokemonCollisionProvider::new(map.id, map.tileset);

            let standing_tile = get_tile_id_at_position(
                &map.blocks,
                map.width,
                map.tileset,
                self.state.player.x,
                self.state.player.y,
            );

            // Spinner / arrow tiles (Facility & Gym tilesets, e.g. Viridian
            // Gym, Rocket Hideout). The original is TABLE-driven: each map's
            // script maps (x, y) → an RLE "dir × steps" list
            // (<Map>ArrowTilePlayerMovement + DecodeArrowMovementRLE,
            // map_objects.asm:5-27) which is fed as simulated input — the
            // player travels a STRAIGHT line per run while the sprite spins
            // (LoadSpinnerArrowTiles), plus SFX_ARROW_TILES on trigger.
            // B1F/B4F arrow tiles have no table (decorative only).
            let movement_input = if self.state.player.movement_state == MovementState::Idle
                && self.active_script_effect.is_none()
                && self.pending_warp.is_none()
                && self.scripted_player_path.is_empty()
                && special_terrain::is_spinner_tile(map.tileset, standing_tile)
            {
                let map_name = crate::overworld::script_bridge::map_id_to_script_key(map.id)
                    .to_string();
                let entry = spinner_paths(&map_name).iter().find(|&&(sx, sy, _)| {
                    sx as u16 == self.state.player.x && sy as u16 == self.state.player.y
                });
                if let Some(&(_, _, steps)) = entry {
                    // Build the straight-line tile path from the RLE steps.
                    let (mut x, mut y) = (self.state.player.x, self.state.player.y);
                    let mut path: Vec<(u16, u16)> = Vec::new();
                    for st in steps {
                        let (dx, dy) = player_movement::direction_delta(st.dir);
                        for _ in 0..st.steps {
                            x = (x as i32 + dx as i32).max(0) as u16;
                            y = (y as i32 + dy as i32).max(0) as u16;
                            path.push((x, y));
                        }
                    }
                    if !path.is_empty() {
                        self.sfx_event = OverworldSfxEvent::ArrowTiles;
                        for p in &path {
                            self.scripted_player_path.push_back(*p);
                        }
                    }
                }
                movement_input // no direction override: the path drives movement
            } else {
                movement_input
            };

            // Route 17 (Cycling Road) slope: JoypadOverworld
            // (home/overworld.asm:1826-1835) simulates a PAD_DOWN press every
            // frame while no trainer battle is active and the player holds no
            // d-pad / A / B button — the slope auto-walks the player downhill.
            // (The original checks BIT_TRAINER_BATTLE; the port's equivalent
            // is a sighted-but-not-yet-started battle.)
            let movement_input = if map.id == MapId::Route17
                && self.pending_trainer_battle.is_none()
                && !movement_input.up
                && !movement_input.down
                && !movement_input.left
                && !movement_input.right
                && !movement_input.a_button
                && !movement_input.b_button
            {
                MovementInput {
                    down: true,
                    ..movement_input
                }
            } else {
                movement_input
            };

            // DoBikeSpeedup (home/overworld.asm:377-388): the bike normally
            // steps at double speed; on Route 17 the speedup is cancelled
            // while UP/LEFT/RIGHT is held (fighting the slope) — the bike
            // then moves at walking speed.
            self.state.player.bike_speedup_active = !(map.id == MapId::Route17
                && (movement_input.up || movement_input.left || movement_input.right));

            let target_tile = if let Some(dir) = movement_input.direction_pressed() {
                let (dx, dy) = player_movement::direction_delta(dir);
                let target_x = ((self.state.player.x as i32) + dx as i32).max(0) as u16;
                let target_y = ((self.state.player.y as i32) + dy as i32).max(0) as u16;

                get_tile_id_at_position(&map.blocks, map.width, map.tileset, target_x, target_y)
            } else {
                standing_tile
            };

            // Build sprite positions for collision check.
            // Include NPC destinations (tiles they're walking toward) to prevent
            // race conditions where player walks into a tile an NPC is heading to.
            // This mirrors the occupied vector logic in npc_movement.rs.
            let npc_positions: Vec<collision::SpritePosition> = self
                .npc_states
                .iter()
                .filter(|npc| npc.visible)
                .flat_map(|npc| {
                    let cur = collision::SpritePosition { x: npc.x, y: npc.y };
                    if npc.walk_counter > 0 {
                        // NPC is walking - include destination tile
                        let (dx, dy) = player_movement::direction_delta(npc.facing);
                        let dest = collision::SpritePosition {
                            x: (npc.x as i32 + dx as i32).max(0) as u16,
                            y: (npc.y as i32 + dy as i32).max(0) as u16,
                        };
                        vec![cur, dest]
                    } else {
                        vec![cur]
                    }
                })
                .collect();

            let movement_before = self.state.player.movement_state;
            let transport_before = self.state.player.transport;

            let result = player_movement::process_frame(
                &mut self.state,
                &movement_input,
                map,
                standing_tile,
                target_tile,
                &npc_positions,
                &collision_provider,
            );

            // Surf dismount (CollisionCheckOnWater .stopSurfing): the engine
            // flipped the transport back to Walking after stepping ashore —
            // PlayDefaultMusic, i.e. resume the map's own music.
            if transport_before == TransportMode::Surfing
                && self.state.player.transport == TransportMode::Walking
            {
                let current_map = self.state.current_map;
                self.audio_requests
                    .push(OverworldAudioRequest::PlayMapMusic { map: current_map });
            }

            let mut turn_encounter_check: Option<(MapId, G::Tileset, u8, u8)> = None;
            // A step completes when the engine returns to Idle OR when the
            // position advanced — the engine chains straight into the next
            // step on the same frame when the direction stays held (no Idle
            // frame in between; process_frame's step-done branch calls
            // try_move immediately). Both cases must roll the encounter
            // check exactly once per TILE entered.
            let pos_changed = self.state.player.x != prev_x || self.state.player.y != prev_y;
            let step_completed = (movement_before == MovementState::Walking
                && self.state.player.movement_state == MovementState::Idle)
                || (movement_before == MovementState::Walking && pos_changed);

            // REPEL wore-off is detected where the counter now ticks — inside
            // check_wild_encounter_on_step's gate (the classic
            // TryDoWildEncounter placement, wild_encounters.asm:19-25 +
            // .lastRepelStep): the roll path consumes the step and reports the
            // >0 → 0 transition, whose "wore off" text REPLACES that step's
            // encounter roll. Steps that never reach the roll (warp tile,
            // script, cooldown, chained steps into another tile) never tick
            // repel and never wear it off.
            let mut repel_wore_off = false;

            let encounter_check_data = if step_completed
                && self.pending_wild_encounter.is_none()
            {
                let new_standing_tile = collision_provider.get_tile_at_position(
                    map.tileset, &map.blocks, map.width,
                    self.state.player.x,
                    self.state.player.y,
                );
                // The RATE anchor is the tile right of the standing tile
                // (screen (9,9), wild_encounters.asm:28-47). Off-map right:
                // the original still reads a BG tile (the map edge row) —
                // approximate with the standing tile so the roll (and the
                // REPEL tick inside its gate) still happens.
                let right_tile = if self.state.player.x + 1 < map.width as u16 {
                    collision_provider.get_tile_at_position(
                        map.tileset, &map.blocks, map.width,
                        self.state.player.x + 1,
                        self.state.player.y,
                    )
                } else {
                    new_standing_tile
                };
                Some((map.id, map.tileset, new_standing_tile, right_tile))
            } else {
                None
            };

            self.prev_movement_state = self.state.player.movement_state;
            let mut warped_or_edge = false;

            match result {
                MoveResult::Warped { warp_index: _ } => {
                    warped_or_edge = true;
                    if let Some((dest_map, warp_x, warp_y)) = execute_warp(
                        map,
                        self.state.player.x as u8,
                        self.state.player.y as u8,
                        self.last_map,
                    ) {
                        // PlayMapChangeSound: door tile → GoInside, otherwise → GoOutside
                        if doors_elevators::is_standing_on_door(map.tileset, standing_tile) {
                            self.sfx_event = OverworldSfxEvent::GoInside;
                        } else {
                            self.sfx_event = OverworldSfxEvent::GoOutside;
                        }
                        let save_last_map = special_terrain::is_outside_map(map.tileset);
                        self.pending_warp = Some(PendingWarp {
                            dest_map,
                            dest_x: warp_x,
                            dest_y: warp_y,
                            save_last_map,
                            // IsPlayerOnDungeonWarp (hidden_events.asm:1-17):
                            // stepping on a warp pad/hole sets BIT_DUNGEON_WARP
                            // → the arrival plays EnterMapAnim's spin-in.
                            // Door warps fade in without it.
                            arrival_spin: crate::overworld::special_terrain::check_warp_pad_or_hole(
                                map.tileset,
                                standing_tile,
                            ) != pokered_data::tileset_data::WarpPadOrHoleType::None,
                        });
                        self.warp_fade_state = WarpFadeState::FadingOut {
                            frames_remaining: WARP_FADE_OUT_FRAMES,
                        };
                    }
                }
                MoveResult::ReachedMapEdge => {
                    warped_or_edge = true;
                    if let Some(dir) = movement_input.direction_pressed() {
                        if let Some(transition) = engine_calculate_connection_transition(
                            map,
                            &PokemonMapData,
                            self.state.player.x,
                            self.state.player.y,
                            dir,
                        ) {
                            let save_last_map = special_terrain::is_outside_map(map.tileset);
                            self.pending_connection = Some(PendingConnection {
                                transition,
                                save_last_map,
                                old_map: self.state.current_map,
                            });
                            self.state.player.movement_state = MovementState::Walking;
                            self.state.walk_counter = player_movement::WALK_COUNTER_INIT;

                            // Pre-load destination NPCs so the renderer can draw
                            // them offset during the walk (scroll-in effect).
                            // The +delta accounts for the player being one step
                            // before the boundary while new_x/y is one step after.
                            let (dx, dy) = player_movement::direction_delta(dir);
                            let step_offset_x =
                                self.state.player.x as i32 + dx as i32 - transition.new_x as i32;
                            let step_offset_y =
                                self.state.player.y as i32 + dy as i32 - transition.new_y as i32;
                            let (dest_map, dest_pokemon_data) =
                                crate::overworld::map_data_loading::load_full_map_data(transition.new_map, self.game_data.tileset_provider());
                            let dest_key = script_bridge::map_id_to_script_key(transition.new_map);
                            let hidden = self
                                .script_loader
                                .get_config(&dest_key)
                                .map(|c| c.hidden_npc_ids())
                                .unwrap_or_default();
                            let preview_npcs =
                                build_npc_runtime_states(&dest_map.npcs, &dest_pokemon_data, &hidden);
                            self.connection_npc_preview = Some(ConnectionNpcPreview {
                                npcs: preview_npcs,
                                step_offset_x,
                                step_offset_y,
                            });
                        }
                    }
                }
                MoveResult::TurnedOnly => {
                    // holdIntermediateDirectionLoop → call NewBattle
                    // (home/overworld.asm:224-234): a completed 180° turn-in-
                    // place ROLLS the wild-encounter check (and burns a repel
                    // step) exactly like a completed step. Record it; the
                    // roll runs after the map borrow ends (same queue as the
                    // on-step check below).
                    self.bump_anim_counter = 0;
                    if self.pending_wild_encounter.is_none() && turn_encounter_check.is_none() {
                        turn_encounter_check = Some((
                            map.id,
                            map.tileset,
                            standing_tile,
                            if self.state.player.x + 1 < map.width as u16 {
                                collision_provider.get_tile_at_position(
                                    map.tileset,
                                    &map.blocks,
                                    map.width,
                                    self.state.player.x + 1,
                                    self.state.player.y,
                                )
                            } else {
                                standing_tile
                            },
                        ));
                    }
                }
                MoveResult::Blocked(_) => {
                    self.sfx_event = OverworldSfxEvent::Collision;
                    self.bump_anim_counter = self.bump_anim_counter.wrapping_add(1);
                }
                MoveResult::LedgeJump => {
                    self.sfx_event = OverworldSfxEvent::Ledge;
                    self.bump_anim_counter = 0;
                }
                _ => {
                    self.bump_anim_counter = 0;
                }
            }

            if !warped_or_edge
                && self.active_script_effect.is_none()
                && (self.state.player.x != prev_x || self.state.player.y != prev_y)
            {
                // Existing coord-event check (kept for backward compatibility)
                if let Some(fn_name) = self
                    .map_script_config
                    .coord_event_fn(self.state.player.x, self.state.player.y)
                {
                    let fn_name = fn_name.to_string();
                    if self.script_engine.has_function(&fn_name) {
                        self.script_engine.set_player_position(
                            self.state.player.x as u8,
                            self.state.player.y as u8,
                        );
                        if let Ok(Some(cmd)) = self.script_engine.call_function_no_args(&fn_name) {
                            self.active_script_effect =
                                Some(script_bridge::dispatch_command_with_names(
                                    &cmd,
                                    &self.player_name,
                                    &self.rival_name,
                                    &self.starter_display_name(),
                                ));
                        }
                        self.sync_flags_from_engine();
                    }
                }
            }

            // TriggerManager check (OnStep / OnEnter) — runs every frame
            // regardless of whether the player moved, because OnStep fires
            // each frame the player stands on a trigger tile.
            if self.active_script_effect.is_none() {
                let map_key = script_bridge::map_id_to_script_key(self.state.current_map);
                let triggered = self.trigger_manager.check_triggers(
                    &map_key,
                    self.state.player.x as u32,
                    self.state.player.y as u32,
                );
                for fn_name in &triggered {
                    if self.script_engine.has_function(fn_name) {
                        self.script_engine.set_player_position(
                            self.state.player.x as u8,
                            self.state.player.y as u8,
                        );
                        if let Ok(Some(cmd)) = self.script_engine.call_function_no_args(fn_name) {
                            self.active_script_effect =
                                Some(script_bridge::dispatch_command_with_names(
                                    &cmd,
                                    &self.player_name,
                                    &self.rival_name,
                                    &self.starter_display_name(),
                                ));
                            self.sync_flags_from_engine();
                            break; // Only handle one trigger per frame
                        }
                    }
                }
            }

            if repel_wore_off {
                // `map`'s borrow is dead here; the text replaces this step's
                // encounter roll (already suppressed above).
                self.show_repel_wore_off_message();
            }

            if let Some((map_id, tileset, standing_tile, right_tile)) = encounter_check_data {
                if self.check_wild_encounter_on_step(
                    map_id,
                    tileset,
                    standing_tile,
                    right_tile,
                    self.state.standing_on_warp,
                    self.active_script_effect.is_some(),
                ) {
                    repel_wore_off = true;
                }
            }

            // A completed 180° turn-in-place rolls too (NewBattle after the
            // intermediate-direction loop, home/overworld.asm:224-234).
            if let Some((map_id, tileset, standing_tile, right_tile)) = turn_encounter_check {
                self.check_wild_encounter_on_step(
                    map_id,
                    tileset,
                    standing_tile,
                    right_tile,
                    self.state.standing_on_warp,
                    self.active_script_effect.is_some(),
                );
            }

            // Safari Zone: decrement the step counter and end the game at zero.
            if step_completed {
                self.tick_safari_steps();
                // Day Care: a deposited Pokémon gains one experience point per
                // overworld step (original IncrementDayCareMonExp, called from
                // the per-step ApplyOutOfBattlePoisonDamage). The exp lives in
                // SaveData, so defer the increment to the app layer.
                self.game_data_requests
                    .push(OverworldGameDataRequest::TickDaycareExp);
            }
        } else {
            if let Some(dir) = movement_input.direction_pressed() {
                self.state.player.facing = dir;
                let (dx, dy) = player_movement::direction_delta(dir);
                self.state.player.x = (self.state.player.x as i32 + dx as i32).max(0) as u16;
                self.state.player.y = (self.state.player.y as i32 + dy as i32).max(0) as u16;
            }
        }

        // Boulder pushing (TryPushingBoulder): while STRENGTH is active and
        // the player holds the d-pad toward a boulder, slide it one tile.
        // Runs in the normal gameplay path only (dialogue/scripts return
        // earlier), matching the original's RunMapScript call site.
        self.tick_boulder_push(movement_input.direction_pressed());

        // Advance NPC movement every frame (DoMovementForAllSprites).
        // In the original game, NPC movement is frozen while a text box is displayed
        // (wFontLoaded / BIT_FONT_LOADED check in UpdateNPCSprite).
        // run_npc_movement_tick() gates on is_text_ui_active() internally.
        self.run_npc_movement_tick();

        ScreenAction::Continue
    }

    fn is_text_ui_active(&self) -> bool {
        self.pending_dialogue.is_some()
            || self.pending_choice.is_some()
            || self.pending_pokedex_entry.is_some()
            || self.pending_naming_screen.is_some()
    }

    fn run_npc_movement_tick(&mut self) {
        if self.is_text_ui_active() {
            return;
        }
        if let Some(ref map) = self.map_data {
            let rng_value = (self
                .frame_counter
                .wrapping_mul(1103515245)
                .wrapping_add(12345)
                >> 16) as u8;
            let player_dest = if self.state.player.movement_state != MovementState::Idle {
                let (dx, dy) = player_movement::direction_delta(self.state.player.facing);
                Some((
                    (self.state.player.x as i32 + dx as i32).max(0) as u16,
                    (self.state.player.y as i32 + dy as i32).max(0) as u16,
                ))
            } else {
                None
            };
            npc_movement::update_npc_movement(
                &mut self.npc_states,
                self.state.player.x,
                self.state.player.y,
                player_dest,
                map.width,
                map.height,
                rng_value,
                &map.blocks,
                map.tileset,
                &collision::PokemonCollisionProvider::new(self.state.current_map, map.tileset),
            );
        }
    }

    fn advance_scripted_player_path(&mut self) {
        if self.scripted_player_path.is_empty() {
            return;
        }
        if self.state.player.movement_state == MovementState::Idle {
            self.start_next_scripted_player_step();
        } else {
            let step_done = player_movement::advance_step(&mut self.state);
            if step_done && !self.scripted_player_path.is_empty() {
                self.start_next_scripted_player_step();
            }
        }
    }

    fn start_next_scripted_player_step(&mut self) {
        while let Some(&(tx, ty)) = self.scripted_player_path.front() {
            if self.state.player.x == tx && self.state.player.y == ty {
                self.scripted_player_path.pop_front();
                continue;
            }
            if let Some(dir) =
                direction_toward_player(self.state.player.x, self.state.player.y, tx, ty)
            {
                self.state.player.facing = dir;
                self.state.player.movement_state = MovementState::Walking;
                self.state.walk_counter = player_movement::WALK_COUNTER_INIT;
            }
            return;
        }
    }

    fn try_trigger_warp_at_player_position(&mut self) -> bool {
        if self.pending_warp.is_some() || !matches!(self.warp_fade_state, WarpFadeState::Idle) {
            return false;
        }

        let Some(map) = self.map_data.as_ref() else {
            return false;
        };

        let Some(_warp_idx) =
            collision::check_warp_at_position(self.state.player.x, self.state.player.y, map)
        else {
            return false;
        };

        let Some((dest_map, warp_x, warp_y)) = execute_warp(
            map,
            self.state.player.x as u8,
            self.state.player.y as u8,
            self.last_map,
        ) else {
            return false;
        };

        let standing_tile =
            collision::PokemonCollisionProvider::new(map.id, map.tileset)
                .get_tile_at_position(map.tileset, &map.blocks, map.width, self.state.player.x, self.state.player.y);
        if doors_elevators::is_standing_on_door(map.tileset, standing_tile) {
            self.sfx_event = OverworldSfxEvent::GoInside;
        } else {
            self.sfx_event = OverworldSfxEvent::GoOutside;
        }

        let save_last_map = special_terrain::is_outside_map(map.tileset);
        self.pending_warp = Some(PendingWarp {
            dest_map,
            dest_x: warp_x,
            dest_y: warp_y,
            save_last_map,
            // IsPlayerOnDungeonWarp (hidden_events.asm:1-17): stepping on a
            // warp pad/hole sets BIT_DUNGEON_WARP → the arrival plays
            // EnterMapAnim's spin-in. Door warps fade in without it.
            arrival_spin: crate::overworld::special_terrain::check_warp_pad_or_hole(
                map.tileset,
                standing_tile,
            ) != pokered_data::tileset_data::WarpPadOrHoleType::None,
        });
        self.warp_fade_state = WarpFadeState::FadingOut {
            frames_remaining: WARP_FADE_OUT_FRAMES,
        };
        true
    }

    /// `VermilionDock_EraseSSAnne` + `wNumberOfWarps--`
    /// (scripts/VermilionDock.asm:182-224, 122-123): at the end of the
    /// departure the ship's map blocks are replaced with the water block
    /// ($d) and the dock→ship warp is removed from the live map.
    ///
    /// The original fills BG rows 10-21 with the water tile and replaces
    /// the "lower half of the ship" blocks (5..8, 2). The port's
    /// VermilionDock (14×6 blocks) keeps the ship as the bottom block row
    /// (the 0x10/0x12 hull at (0..12, 5)) with the porthole strip at
    /// (5..8, 2) — the same two erase zones, mapped onto the port layout.
    /// Like the original's wOverworldMap edit, the change is transient:
    /// a map reload rebuilds the blocks and warps (the original's
    /// wNumberOfWarps is also reloaded on entry).
    fn apply_ss_anne_departure_erase(&mut self) {
        if self.state.current_map != pokered_data::maps::MapId::VermilionDock {
            return;
        }
        if let Some(map) = self.map_data.as_mut() {
            // Water block: `ld a, $d ; water block` (VermilionDock.asm:200).
            const WATER_BLOCK: u8 = 0x0d;
            // The ship hull (bottom block row) and the porthole strip.
            for bx in 0..=12u8 {
                map.set_block(bx, 5, WATER_BLOCK);
            }
            for bx in 5..=8u8 {
                map.set_block(bx, 2, WATER_BLOCK);
            }
            // wNumberOfWarps--: drop the dock→ship warp (warp_event 14, 2,
            // SS_ANNE_1F, 2) so the player cannot re-board during this visit.
            map.warps.retain(|w| !(w.x == 14 && w.y == 2));
        }
    }

    fn tick_active_effect(
        effect: &mut script_bridge::ScriptEffect,
        a_just_pressed: bool,
        b_just_pressed: bool,
        a_pressed: bool,
        up_pressed: bool,
        down_pressed: bool,
        pending_dialogue: &mut Option<BedroomDialogue>,
        pending_choice: &mut Option<script_bridge::PendingChoice>,
        pending_pokedex_entry: &mut Option<PokedexEntryState>,
        pending_naming_screen: &mut Option<crate::naming_screen::NamingScreenState>,
        party_select_requested: &mut bool,
        pending_emotion_bubble: &mut Option<EmotionBubbleState>,
        pending_healing_machine: &mut Option<HealingMachineState>,
        npc_states: &mut [npc_movement::NpcRuntimeState],
        player_state: &PlayerState,
        scripted_player_path: &mut VecDeque<(u16, u16)>,
        map_data: Option<&MapData<G::Tileset>>,
        map_script_config: &MapScriptConfig,
        party_count: u8,
        audio_requests: &mut Vec<OverworldAudioRequest>,
        current_map: MapId,
        sfx_event: &mut OverworldSfxEvent,
        ship_departure: &mut Option<presentation::ShipDepartureState>,
    ) -> bool {
        match effect {
            script_bridge::ScriptEffect::ShowDialogue { text } => {
                if pending_dialogue.is_none() {
                    *pending_dialogue = Some(script_bridge::text_to_dialogue(text));
                    false
                } else if pending_dialogue.as_ref().map_or(true, |d| d.is_done()) {
                    *pending_dialogue = None;
                    true
                } else {
                    if let Some(ref mut dlg) = pending_dialogue {
                        if dlg.holding_open() {
                            if !a_pressed || b_just_pressed {
                                *pending_dialogue = None;
                                return true;
                            }
                        } else if a_just_pressed || b_just_pressed {
                            if dlg.waiting_for_input() {
                                if dlg.is_last_page() && a_just_pressed {
                                    dlg.start_holding_open();
                                } else if !dlg.advance() {
                                    *pending_dialogue = None;
                                    return true;
                                }
                            } else {
                                dlg.skip_to_full_page();
                            }
                            *sfx_event = OverworldSfxEvent::TextAdvance;
                        } else {
                            dlg.reveal_next_char();
                        }
                    }
                    false
                }
            }
            script_bridge::ScriptEffect::ShowChoice {
                options,
                started,
                selected,
            } => {
                if !*started {
                    *pending_choice = Some(script_bridge::PendingChoice::new(options.clone()));
                    *started = true;
                    false
                } else if let Some(ref mut choice) = pending_choice {
                    if up_pressed {
                        choice.move_up();
                    } else if down_pressed {
                        choice.move_down();
                    }
                    if a_just_pressed {
                        *selected = choice.selected;
                        *pending_choice = None;
                        true
                    } else if b_just_pressed {
                        // B = cancel = last option (NO)
                        *selected = choice.options.len().saturating_sub(1) as u32;
                        *pending_choice = None;
                        true
                    } else {
                        false
                    }
                } else {
                    // pending_choice was cleared externally — treat as done
                    true
                }
            }
            script_bridge::ScriptEffect::Delay {
                frames: _,
                ref mut frames_remaining,
            } => {
                if *frames_remaining == 0 {
                    return true;
                }
                *frames_remaining -= 1;
                *frames_remaining == 0
            }
            script_bridge::ScriptEffect::PlayShipDeparture { ref mut started } => {
                if !*started {
                    *started = true;
                    *ship_departure = Some(presentation::ShipDepartureState::new());
                    false
                } else if ship_departure.is_some() {
                    // The animation is ticking in the presentation section
                    // above (which returns early); stay blocked until it
                    // completes and clears the state.
                    false
                } else {
                    true
                }
            }
            script_bridge::ScriptEffect::Immediate { .. } => true,
            script_bridge::ScriptEffect::SetJoyIgnore { .. } => true,
            script_bridge::ScriptEffect::ClearJoyIgnore => true,
            script_bridge::ScriptEffect::StartNpcMove { npc_id, path } => {
                if let Some(idx) = resolve_npc_index(npc_id, npc_states, map_script_config) {
                    npc_movement::start_scripted_move(&mut npc_states[idx], path);
                }
                true
            }
            script_bridge::ScriptEffect::MoveNpc {
                npc_id,
                path,
                started,
            } => {
                if !*started {
                    if let Some(idx) = resolve_npc_index(npc_id, npc_states, map_script_config) {
                        npc_movement::start_scripted_move(&mut npc_states[idx], path);
                    }
                    *started = true;
                    false
                } else {
                    let npc_idx = resolve_npc_index(npc_id, npc_states, map_script_config);
                    match npc_idx {
                        Some(idx) => npc_movement::is_scripted_move_done(&npc_states[idx]),
                        None => true,
                    }
                }
            }
            script_bridge::ScriptEffect::AwaitNpcMove { npc_id } => {
                let npc_idx = resolve_npc_index(npc_id, npc_states, map_script_config);
                match npc_idx {
                    Some(idx) => npc_movement::is_scripted_move_done(&npc_states[idx]),
                    None => true,
                }
            }
            script_bridge::ScriptEffect::MovePlayer { path, started } => {
                if !*started {
                    scripted_player_path.clear();
                    for &(x, y) in path.iter() {
                        scripted_player_path.push_back((x as u16, y as u16));
                    }
                    *started = true;
                    false
                } else {
                    scripted_player_path.is_empty()
                        && player_state.movement_state == MovementState::Idle
                }
            }
            script_bridge::ScriptEffect::MovePlayerRelative { steps, started } => {
                if !*started {
                    // Resolve cumulative deltas against the player's live
                    // position into absolute waypoints.
                    scripted_player_path.clear();
                    let mut cx = player_state.x as i32;
                    let mut cy = player_state.y as i32;
                    for &(dx, dy) in steps.iter() {
                        cx += dx as i32;
                        cy += dy as i32;
                        if cx >= 0 && cy >= 0 {
                            scripted_player_path.push_back((cx as u16, cy as u16));
                        }
                    }
                    *started = true;
                    false
                } else {
                    scripted_player_path.is_empty()
                        && player_state.movement_state == MovementState::Idle
                }
            }
            script_bridge::ScriptEffect::StartNpcMoveTo { npc_id, x, y } => {
                if let Some(map) = map_data {
                    if let Some(idx) = resolve_npc_index(npc_id, npc_states, map_script_config) {
                        let start = (npc_states[idx].x, npc_states[idx].y);
                        let target = (*x as u16, *y as u16);
                        if let Some(path) = plan_terrain_path(map, start, target) {
                            if let Some(path_u8) = path_u16_to_u8(&path) {
                                npc_movement::start_scripted_move(&mut npc_states[idx], &path_u8);
                            }
                        }
                    }
                }
                true
            }
            script_bridge::ScriptEffect::MoveNpcTo {
                npc_id,
                x,
                y,
                started,
            } => {
                if !*started {
                    if let Some(map) = map_data {
                        if let Some(idx) = resolve_npc_index(npc_id, npc_states, map_script_config)
                        {
                            let start = (npc_states[idx].x, npc_states[idx].y);
                            let target = (*x as u16, *y as u16);
                            if let Some(path) = plan_terrain_path(map, start, target) {
                                if let Some(path_u8) = path_u16_to_u8(&path) {
                                    npc_movement::start_scripted_move(
                                        &mut npc_states[idx],
                                        &path_u8,
                                    );
                                }
                            }
                        }
                    }
                    *started = true;
                    false
                } else {
                    let npc_idx = resolve_npc_index(npc_id, npc_states, map_script_config);
                    match npc_idx {
                        Some(idx) => npc_movement::is_scripted_move_done(&npc_states[idx]),
                        None => true,
                    }
                }
            }
            script_bridge::ScriptEffect::MovePlayerTo { x, y, started } => {
                if !*started {
                    scripted_player_path.clear();
                    if let Some(map) = map_data {
                        let start = (player_state.x, player_state.y);
                        let target = (*x as u16, *y as u16);
                        if let Some(path) = plan_terrain_path(map, start, target) {
                            for (px, py) in path {
                                scripted_player_path.push_back((px, py));
                            }
                        }
                    }
                    *started = true;
                    false
                } else {
                    scripted_player_path.is_empty()
                        && player_state.movement_state == MovementState::Idle
                }
            }
            script_bridge::ScriptEffect::FollowNpc {
                npc_id,
                target_x,
                target_y,
                phase,
            } => {
                use script_bridge::FollowNpcPhase;
                match phase {
                    FollowNpcPhase::StartNpc => {
                        let mut start_x = 0u16;
                        let mut start_y = 0u16;
                        if let Some(map) = map_data {
                            if let Some(idx) =
                                resolve_npc_index(npc_id, npc_states, map_script_config)
                            {
                                start_x = npc_states[idx].x;
                                start_y = npc_states[idx].y;
                                let target = (*target_x as u16, *target_y as u16);
                                let path = plan_terrain_path_allow_target_step(
                                    map,
                                    (start_x, start_y),
                                    target,
                                )
                                .unwrap_or_else(|| {
                                    plan_straight_path((start_x, start_y), target)
                                });
                                if let Some(path_u8) = path_u16_to_u8(&path) {
                                    npc_movement::start_scripted_move(
                                        &mut npc_states[idx],
                                        &path_u8,
                                    );
                                }
                            }
                        }
                        *phase = FollowNpcPhase::Following {
                            last_npc_x: start_x,
                            last_npc_y: start_y,
                            final_push_done: false,
                        };
                        false
                    }
                    FollowNpcPhase::Following {
                        last_npc_x,
                        last_npc_y,
                        final_push_done,
                    } => {
                        if let Some(idx) = resolve_npc_index(npc_id, npc_states, map_script_config)
                        {
                            let (nx, ny, npc_done) = {
                                let npc = &npc_states[idx];
                                (npc.x, npc.y, npc_movement::is_scripted_move_done(npc))
                            };

                            if nx != *last_npc_x || ny != *last_npc_y {
                                scripted_player_path.push_back((*last_npc_x, *last_npc_y));
                                *last_npc_x = nx;
                                *last_npc_y = ny;
                            }

                            if npc_done && !*final_push_done {
                                let pos = (nx, ny);
                                if scripted_player_path.back() != Some(&pos) {
                                    scripted_player_path.push_back(pos);
                                }
                                // NOTE: the NPC is NOT hidden here — scenes
                                // hide it explicitly (hideObject) if needed;
                                // auto-hiding would freeze any later moveNpc
                                // walk-off, since hidden NPCs don't advance
                                // scripted movement.
                                *final_push_done = true;
                            }
                            let player_done = scripted_player_path.is_empty()
                                && player_state.movement_state == MovementState::Idle;
                            if npc_done && player_done {
                                *phase = FollowNpcPhase::Done;
                                return true;
                            }
                        } else {
                            *phase = FollowNpcPhase::Done;
                            return true;
                        }
                        false
                    }
                    FollowNpcPhase::Done => true,
                }
            }
            script_bridge::ScriptEffect::ShowPokedexEntry { species, started } => {
                if !*started {
                    *pending_pokedex_entry = Some(PokedexEntryState {
                        species: species.clone(),
                        page: 0,
                        total_pages: 0,
                    });
                    *started = true;
                    false
                } else if pending_pokedex_entry.is_none() {
                    true
                } else if a_just_pressed || b_just_pressed {
                    if let Some(ref mut state) = pending_pokedex_entry {
                        let on_last_page =
                            state.total_pages == 0 || state.page + 1 >= state.total_pages;
                        if on_last_page {
                            *pending_pokedex_entry = None;
                            return true;
                        }
                        state.page += 1;
                    }
                    false
                } else {
                    false
                }
            }
            script_bridge::ScriptEffect::NamingScreen {
                species: _,
                naming_state,
                started,
                result_name,
            } => {
                if !*started {
                    let ns = crate::naming_screen::NamingScreenState::new(
                        crate::naming_screen::NamingScreenType::Pokemon,
                    );
                    *naming_state = Some(ns.clone());
                    *pending_naming_screen = Some(ns);
                    *started = true;
                    false
                } else if result_name.is_some() {
                    true
                } else {
                    pending_naming_screen.is_none()
                }
            }
            script_bridge::ScriptEffect::ChoosePartyPokemon {
                started,
                result_index,
            } => {
                if !*started {
                    // Ask the app layer to open the party selector (it owns the
                    // party). Selection resolves via `update_party_select_input`.
                    *party_select_requested = true;
                    *started = true;
                    false
                } else {
                    // Done once the selection (or cancel) has been recorded.
                    result_index.is_some()
                }
            }
            script_bridge::ScriptEffect::SetPartyNickname { .. } => true,
            script_bridge::ScriptEffect::ShowEmotionBubble {
                npc_id,
                emotion,
                frames_remaining,
                started,
            } => {
                if !*started {
                    *pending_emotion_bubble = Some(EmotionBubbleState {
                        npc_id: npc_id.clone(),
                        emotion: emotion.clone(),
                        frames_remaining: *frames_remaining,
                    });
                    *started = true;
                    false
                } else if let Some(ref mut bubble) = pending_emotion_bubble {
                    if bubble.frames_remaining == 0 {
                        *pending_emotion_bubble = None;
                        true
                    } else {
                        bubble.frames_remaining -= 1;
                        false
                    }
                } else {
                    true
                }
            }
            script_bridge::ScriptEffect::AnimateHealingMachine {
                phase,
                frames_remaining,
            } => {
                match phase {
                    HealingMachinePhase::FadeOutMusic => {
                        audio_requests.push(OverworldAudioRequest::FadeOutMusic);
                        *pending_healing_machine = Some(HealingMachineState {
                            phase: HealingMachinePhase::FadeOutMusic,
                            frames_remaining: 32,
                            pokeballs_visible: 0,
                            flash_active: false,
                        });
                        *phase = HealingMachinePhase::WaitForFadeOut;
                        *frames_remaining = 32;
                        false
                    }
                    HealingMachinePhase::WaitForFadeOut => {
                        if *frames_remaining == 0 {
                            let total = if party_count == 0 { 0 } else { party_count };
                            *phase = HealingMachinePhase::HealPartyMember {
                                member_index: 0,
                                total_members: total,
                            };
                            *frames_remaining = 30;
                            if total > 0 {
                                audio_requests.push(OverworldAudioRequest::PlaySound {
                                    sound_id: "SFX_HEALING_MACHINE".to_string(),
                                });
                            }
                            if let Some(ref mut state) = pending_healing_machine {
                                state.phase = HealingMachinePhase::HealPartyMember {
                                    member_index: 0,
                                    total_members: total,
                                };
                                state.frames_remaining = 30;
                                state.pokeballs_visible = if total > 0 { 1 } else { 0 };
                            }
                        } else {
                            *frames_remaining -= 1;
                            if let Some(ref mut state) = pending_healing_machine {
                                state.frames_remaining = *frames_remaining;
                            }
                        }
                        false
                    }
                    HealingMachinePhase::HealPartyMember {
                        member_index,
                        total_members,
                    } => {
                        if *frames_remaining == 0 {
                            if *member_index + 1 < *total_members {
                                *member_index += 1;
                                *frames_remaining = 30;
                                audio_requests.push(OverworldAudioRequest::PlaySound {
                                    sound_id: "SFX_HEALING_MACHINE".to_string(),
                                });
                                if let Some(ref mut state) = pending_healing_machine {
                                    state.phase = HealingMachinePhase::HealPartyMember {
                                        member_index: *member_index,
                                        total_members: *total_members,
                                    };
                                    state.frames_remaining = 30;
                                    state.pokeballs_visible = *member_index + 1;
                                }
                            } else {
                                let total = *total_members;
                                *phase = HealingMachinePhase::PlayHealedMusic;
                                if let Some(ref mut state) = pending_healing_machine {
                                    state.phase = HealingMachinePhase::PlayHealedMusic;
                                    state.pokeballs_visible = total;
                                }
                            }
                        } else {
                            *frames_remaining -= 1;
                            if let Some(ref mut state) = pending_healing_machine {
                                state.frames_remaining = *frames_remaining;
                            }
                        }
                        false
                    }
                    HealingMachinePhase::PlayHealedMusic => {
                        audio_requests.push(OverworldAudioRequest::PlayMusic {
                            music_id: "MUSIC_PKMN_HEALED".to_string(),
                        });
                        *phase = HealingMachinePhase::FlashSprite { flashes_remaining: 8 };
                        *frames_remaining = 10;
                        if let Some(ref mut state) = pending_healing_machine {
                            state.phase = HealingMachinePhase::FlashSprite { flashes_remaining: 8 };
                            state.frames_remaining = 10;
                            state.flash_active = true;
                        }
                        false
                    }
                    HealingMachinePhase::FlashSprite { flashes_remaining } => {
                        if *frames_remaining == 0 {
                            if *flashes_remaining > 0 {
                                *flashes_remaining -= 1;
                                *frames_remaining = 10;
                                if let Some(ref mut state) = pending_healing_machine {
                                    state.phase = HealingMachinePhase::FlashSprite {
                                        flashes_remaining: *flashes_remaining,
                                    };
                                    state.frames_remaining = 10;
                                    state.flash_active = !state.flash_active;
                                }
                            } else {
                                *phase = HealingMachinePhase::WaitForMusic;
                                *frames_remaining = 120;
                                if let Some(ref mut state) = pending_healing_machine {
                                    state.phase = HealingMachinePhase::WaitForMusic;
                                    state.frames_remaining = 120;
                                    state.flash_active = false;
                                }
                            }
                        } else {
                            *frames_remaining -= 1;
                            if let Some(ref mut state) = pending_healing_machine {
                                state.frames_remaining = *frames_remaining;
                            }
                        }
                        false
                    }
                    HealingMachinePhase::WaitForMusic => {
                        if *frames_remaining == 0 {
                            *phase = HealingMachinePhase::Done;
                            *pending_healing_machine = None;
                        } else {
                            *frames_remaining -= 1;
                            if let Some(ref mut state) = pending_healing_machine {
                                state.frames_remaining = *frames_remaining;
                            }
                        }
                        false
                    }
                    HealingMachinePhase::Done => {
                        *pending_healing_machine = None;
                        audio_requests.push(OverworldAudioRequest::PlayMapMusic {
                            map: current_map,
                        });
                        true
                    }
                }
            }
            _ => true,
        }
    }

    /// Resume a script suspended on `await game.startBattle(...)`, delivering the
    /// outcome ("win" | "lose" | "draw") as the await result. No-op when no script
    /// is waiting (e.g. sight-engaged trainer battles that were not script-driven).
    pub fn resume_script_after_battle(&mut self, outcome: &str) {
        if !self.script_awaiting_battle {
            return;
        }
        self.script_awaiting_battle = false;
        if let Ok(Some(next_cmd)) = self
            .script_engine
            .signal_done(CommandResult::Text(outcome.to_string()))
        {
            self.active_script_effect = Some(script_bridge::dispatch_command_with_names(
                &next_cmd,
                &self.player_name,
                &self.rival_name,
                &self.starter_display_name(),
            ));
        }
        self.sync_flags_from_engine();
    }

    /// Resume a script suspended on `await game.elevatorMenu(...)`, delivering
    /// the chosen floor index (0-based, or -1 if the player cancelled) as the
    /// await result. No-op when no script is waiting.
    pub fn resume_script_after_elevator(&mut self, floor: i32) {
        if !self.script_awaiting_elevator {
            return;
        }
        self.script_awaiting_elevator = false;
        if floor >= 0 {
            // A floor was chosen: ShakeElevator plays once the elevator warp
            // completes (SilphCoElevatorShakeScript — BIT_CUR_MAP_USED_ELEVATOR).
            self.elevator_shake_pending = true;
        }
        if let Ok(Some(next_cmd)) = self
            .script_engine
            .signal_done(CommandResult::Number(floor as f64))
        {
            self.active_script_effect = Some(script_bridge::dispatch_command_with_names(
                &next_cmd,
                &self.player_name,
                &self.rival_name,
                &self.starter_display_name(),
            ));
        }
        self.sync_flags_from_engine();
    }

    /// Resume a script suspended on `await game.filterBag(...)`, delivering the
    /// chosen item's const name ("" if the player cancelled). No-op when no
    /// script is waiting.
    pub fn resume_script_after_filter_bag(&mut self, item_name: &str) {
        if !self.script_awaiting_filter_bag {
            return;
        }
        self.script_awaiting_filter_bag = false;
        if let Ok(Some(next_cmd)) = self
            .script_engine
            .signal_done(CommandResult::Text(item_name.to_string()))
        {
            self.active_script_effect = Some(script_bridge::dispatch_command_with_names(
                &next_cmd,
                &self.player_name,
                &self.rival_name,
                &self.starter_display_name(),
            ));
        }
        self.sync_flags_from_engine();
    }

    /// Resume a script suspended on `await game.tradePokemon(...)`, delivering
    /// whether the trade happened (the party held the offered mon). Called by
    /// the app after the trade cutscene completes and the party mutation is
    /// applied — or immediately with `false` when the offered mon is absent.
    /// No-op when no script is waiting.
    pub fn resume_script_after_trade(&mut self, traded: bool) {
        if !self.script_awaiting_trade {
            return;
        }
        self.script_awaiting_trade = false;
        if let Ok(Some(next_cmd)) = self.script_engine.signal_done(CommandResult::Bool(traded)) {
            self.active_script_effect = Some(script_bridge::dispatch_command_with_names(
                &next_cmd,
                &self.player_name,
                &self.rival_name,
                &self.starter_display_name(),
            ));
        }
        self.sync_flags_from_engine();
    }

    fn finish_effect(effect: &script_bridge::ScriptEffect) -> CommandResult {
        match effect {
            script_bridge::ScriptEffect::ShowChoice { selected, .. } => {
                CommandResult::Number(*selected as f64)
            }
            script_bridge::ScriptEffect::Immediate { result } => result.clone(),
            script_bridge::ScriptEffect::NamingScreen {
                result_name,
                species,
                ..
            } => {
                let name = result_name
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| species.clone());
                CommandResult::Text(name)
            }
            script_bridge::ScriptEffect::ChoosePartyPokemon { result_index, .. } => {
                CommandResult::Number(result_index.unwrap_or(-1) as f64)
            }
            _ => CommandResult::Void,
        }
    }

    fn apply_finished_effect(&mut self, effect: Option<script_bridge::ScriptEffect>) {
        if let Some(eff) = effect {
            match eff {
                script_bridge::ScriptEffect::SetJoyIgnore { mask } => {
                    self.joy_ignore_mask = mask;
                }
                script_bridge::ScriptEffect::ClearJoyIgnore => {
                    self.joy_ignore_mask = 0;
                }
                script_bridge::ScriptEffect::FaceNpc { npc_id, direction } => {
                    if let Some(idx) =
                        resolve_npc_index(&npc_id, &self.npc_states, &self.map_script_config)
                    {
                        self.npc_states[idx].facing = direction;
                        self.npc_states[idx].scripted_frame = None;
                    }
                }
                script_bridge::ScriptEffect::FacePlayer { direction } => {
                    self.state.player.facing = direction;
                }
                script_bridge::ScriptEffect::ShowObject { object_index } => {
                    if let Some(npc) = self
                        .npc_states
                        .iter_mut()
                        .find(|n| n.npc_index == object_index)
                    {
                        npc.visible = true;
                    }
                }
                script_bridge::ScriptEffect::HideObject { object_index } => {
                    if let Some(npc) = self
                        .npc_states
                        .iter_mut()
                        .find(|n| n.npc_index == object_index)
                    {
                        npc.visible = false;
                    }
                }
                script_bridge::ScriptEffect::ShowObjectByName { toggle_id } => {
                    use pokered_data::toggleable_objects::{
                        set_object_shown, toggle_id_to_bit_index,
                    };
                    if let Some(npc_id) = self.map_script_config.npc_id_by_toggle(&toggle_id) {
                        if let Some(npc) = self.npc_states.iter_mut().find(|n| n.text_id == npc_id)
                        {
                            npc.visible = true;
                        }
                    }
                    // Clear the hidden flag in unified_flags
                    let flag_key = format!("__OBJ_HIDDEN_{}", toggle_id);
                    self.unified_flags.remove_flag(&flag_key);
                    let shown_key = format!("__OBJ_SHOWN_{}", toggle_id);
                    self.unified_flags.set_flag(&shown_key, true);
                    // Also update toggleable_object_flags for SRAM persistence
                    if let Some(bit_index) = toggle_id_to_bit_index(&toggle_id) {
                        set_object_shown(&mut self.toggleable_object_flags, bit_index);
                    }
                }
                script_bridge::ScriptEffect::HideObjectByName { toggle_id } => {
                    use pokered_data::toggleable_objects::{
                        set_object_hidden, toggle_id_to_bit_index,
                    };
                    if let Some(npc_id) = self.map_script_config.npc_id_by_toggle(&toggle_id) {
                        if let Some(npc) = self.npc_states.iter_mut().find(|n| n.text_id == npc_id)
                        {
                            npc.visible = false;
                        }
                    }
                    // Set the hidden flag in unified_flags
                    let flag_key = format!("__OBJ_HIDDEN_{}", toggle_id);
                    self.unified_flags.set_flag(&flag_key, true);
                    let shown_key = format!("__OBJ_SHOWN_{}", toggle_id);
                    self.unified_flags.remove_flag(&shown_key);
                    // Also update toggleable_object_flags for SRAM persistence
                    if let Some(bit_index) = toggle_id_to_bit_index(&toggle_id) {
                        set_object_hidden(&mut self.toggleable_object_flags, bit_index);
                    }
                }
                script_bridge::ScriptEffect::PlayMusic { music_id } => {
                    self.audio_requests
                        .push(OverworldAudioRequest::PlayMusic { music_id });
                }
                script_bridge::ScriptEffect::PlaySound { sound_id } => {
                    self.audio_requests
                        .push(OverworldAudioRequest::PlaySound { sound_id });
                }
                script_bridge::ScriptEffect::StopMusic => {
                    self.audio_requests.push(OverworldAudioRequest::StopMusic);
                }
                script_bridge::ScriptEffect::FadeOutMusic => {
                    self.audio_requests
                        .push(OverworldAudioRequest::FadeOutMusic);
                }
                script_bridge::ScriptEffect::StartBattle { trainer_id, rival_triplet_base } => {
                    self.pending_trainer_battle = Some(PendingTrainerBattle {
                        trainer_id,
                        npc_index: u8::MAX,
                        // Script-driven battles (gym leaders, rivals) show their
                        // own reward/quip text from the .scene.
                        end_battle_text: None,
                        rival_triplet_base,
                    });
                }
                script_bridge::ScriptEffect::StartWildBattle { species, level } => {
                    // Static/legendary battle: queue a catchable wild encounter
                    // for the app to start; the script is suspended (awaiting the
                    // outcome) exactly like StartBattle.
                    let normalized = species
                        .chars()
                        .enumerate()
                        .map(|(i, c)| {
                            if i == 0 {
                                c.to_ascii_uppercase()
                            } else {
                                c.to_ascii_lowercase()
                            }
                        })
                        .collect::<String>();
                    if let Ok(sp) = normalized.parse::<pokered_data::species::Species>() {
                        self.pending_wild_encounter =
                            Some(PendingWildEncounter { species: sp, level, old_man: false, hooked: false });
                    } else {
                        log::warn!(target: "pokered::overworld", "startWildBattle: unknown species '{}'", species);
                    }
                }
                script_bridge::ScriptEffect::OldManTutorial => {
                    // The Old-Man catch tutorial: a Lv5 WEEDLE, auto-played + guaranteed
                    // catch (the app reads `old_man` and sets `battle.is_old_man`). The
                    // script is suspended awaiting the outcome, like StartWildBattle.
                    self.pending_wild_encounter = Some(PendingWildEncounter {
                        species: pokered_data::species::Species::Weedle,
                        level: 5,
                        old_man: true,
                        hooked: false,
                    });
                }
                script_bridge::ScriptEffect::SetNpcPosition { npc_id, x, y } => {
                    if let Some(npc_id) = self.map_script_config.npc_id_by_toggle(&npc_id) {
                        if let Some(npc) = self.npc_states.iter_mut().find(|n| n.text_id == npc_id)
                        {
                            npc.x = x as u16;
                            npc.y = y as u16;
                        }
                    }
                }
                script_bridge::ScriptEffect::SetNpcFrame { npc_id, frame } => {
                    if let Some(idx) =
                        resolve_npc_index(&npc_id, &self.npc_states, &self.map_script_config)
                    {
                        self.npc_states[idx].scripted_frame = Some(frame);
                    }
                }
                script_bridge::ScriptEffect::Heal => {
                    self.heal_requested = true;
                    // SetLastBlackoutMap (engine/events/set_blackout_map.asm):
                    // a script-driven heal (Pokémon Center nurse, mom, …)
                    // records the map the player came in from as the blackout
                    // /Teleport target — except in Safari Zone rest houses,
                    // which the original explicitly skips
                    // (data/maps/rest_house_maps.asm).
                    let is_rest_house = matches!(
                        self.state.current_map,
                        MapId::SafariZoneWestRestHouse
                            | MapId::SafariZoneEastRestHouse
                            | MapId::SafariZoneNorthRestHouse
                    );
                    if !is_rest_house {
                        if let Some(map) = self.last_map {
                            self.game_data_requests
                                .push(OverworldGameDataRequest::SetBlackoutMap { map });
                        }
                    }
                }
                script_bridge::ScriptEffect::GivePokemon {
                    species,
                    nickname,
                    level,
                } => {
                    let normalized = species
                        .chars()
                        .enumerate()
                        .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() })
                        .collect::<String>();
                    if let Ok(sp) = normalized.parse::<pokered_data::species::Species>() {
                        self.pending_give_pokemon = Some(screen::PendingGivePokemon {
                            species: sp,
                            level,
                            nickname: nickname.clone(),
                        });
                    }
                }
                script_bridge::ScriptEffect::SetPartyNickname { index, nickname } => {
                    // Ignore an empty name (a cancelled naming screen) so the
                    // Pokémon keeps its current nickname. The app layer owns the
                    // party, so surface the write as a pending request.
                    if !nickname.is_empty() {
                        self.pending_set_nickname = Some((index, nickname));
                    }
                }
                script_bridge::ScriptEffect::GiveItem { item_id, quantity } => {
                    self.game_data_requests
                        .push(OverworldGameDataRequest::GiveItem {
                            item: item_id,
                            quantity,
                        });
                }
                script_bridge::ScriptEffect::TradePokemon {
                    offered,
                    received,
                    nickname,
                } => {
                    self.game_data_requests
                        .push(OverworldGameDataRequest::TradePokemon {
                            offered,
                            received,
                            nickname,
                        });
                }
                script_bridge::ScriptEffect::TakeItem { item_id, quantity } => {
                    self.game_data_requests
                        .push(OverworldGameDataRequest::TakeItem {
                            item: item_id,
                            quantity,
                        });
                }
                script_bridge::ScriptEffect::GiveMoney { amount } => {
                    self.game_data_requests
                        .push(OverworldGameDataRequest::GiveMoney { amount });
                }
                script_bridge::ScriptEffect::TakeMoney { amount } => {
                    self.game_data_requests
                        .push(OverworldGameDataRequest::TakeMoney { amount });
                }
                script_bridge::ScriptEffect::GiveCoins { amount } => {
                    self.game_data_requests
                        .push(OverworldGameDataRequest::GiveCoins { amount });
                }
                script_bridge::ScriptEffect::TakeCoins { amount } => {
                    self.game_data_requests
                        .push(OverworldGameDataRequest::TakeCoins { amount });
                }
                script_bridge::ScriptEffect::DepositDaycare { index } => {
                    self.game_data_requests
                        .push(OverworldGameDataRequest::DepositDaycare { index });
                }
                script_bridge::ScriptEffect::WithdrawDaycare => {
                    self.game_data_requests
                        .push(OverworldGameDataRequest::WithdrawDaycare);
                }
                script_bridge::ScriptEffect::PlayCry { species } => {
                    self.audio_requests
                        .push(OverworldAudioRequest::PlayCry { species });
                }
                script_bridge::ScriptEffect::GiveBadge { badge } => {
                    self.game_data_requests
                        .push(OverworldGameDataRequest::GiveBadge { badge });
                }
                script_bridge::ScriptEffect::ReplaceTileBlock { x, y, block_id } => {
                    // Mutate the live block grid in place. Collision + rendering
                    // read `blocks` every frame, so no re-render/invalidation is
                    // needed.
                    if let Some(map) = self.map_data.as_mut() {
                        map.set_block(x, y, block_id);
                    }
                }
                script_bridge::ScriptEffect::WarpTo { map, x, y } => {
                    // Scripted warpTo (elevator floors, Hall of Fame, etc.):
                    // queue a fade-out warp exactly like a door/edge transition.
                    if let Some(dest_map) =
                        pokered_data::map_data_loader::resolve_map_id(&map)
                    {
                        // Same rule as door warps: only outside maps update the
                        // Escape Rope/DIG return point, so warps from interior
                        // maps don't clobber last_map/last_map_entry.
                        let save_last_map = self
                            .map_data
                            .as_ref()
                            .map(|m| special_terrain::is_outside_map(m.tileset))
                            .unwrap_or(false);
                        self.pending_warp = Some(PendingWarp {
                            dest_map,
                            dest_x: x,
                            dest_y: y,
                            save_last_map,
                            // Script warps (elevator floors, Hall of Fame)
                            // use the plain fade — no EnterMapAnim.
                            arrival_spin: false,
                        });
                        self.warp_fade_state = WarpFadeState::FadingOut {
                            frames_remaining: WARP_FADE_OUT_FRAMES,
                        };
                        self.sfx_event = OverworldSfxEvent::GoOutside;
                    } else {
                        log::warn!(
                            target: "pokered::overworld",
                            "[Script] warpTo: unknown map '{}'",
                            map
                        );
                    }
                }
                script_bridge::ScriptEffect::FadeScreen { .. } => {}
                script_bridge::ScriptEffect::OpenShop { items } => {
                    self.pending_shop = Some(items);
                }
                script_bridge::ScriptEffect::OpenSlots { lucky } => {
                    self.pending_slots = Some(lucky);
                }
                script_bridge::ScriptEffect::ElevatorMenu { floors } => {
                    self.pending_elevator = Some(floors);
                }
                script_bridge::ScriptEffect::FilterBag { item_ids } => {
                    self.pending_filter_bag = Some(item_ids);
                }
                script_bridge::ScriptEffect::ShowDiploma => {
                    self.pending_diploma = true;
                }
                script_bridge::ScriptEffect::OpenPc { kind } => {
                    self.pending_pc = Some(kind);
                }
                script_bridge::ScriptEffect::LinkStart => {
                    self.link_start_requested = true;
                }
                script_bridge::ScriptEffect::HallOfFameCeremony => {
                    self.pending_hof_ceremony = true;
                }
                _ => {}
            }
        }
    }

    pub(crate) fn load_map_script(&mut self, map_id: MapId) {
        self.sync_flags_from_engine();

        let map_key = script_bridge::map_id_to_script_key(map_id);
        log::info!(target: "pokered::overworld", "[Script] Loading map script for {:?} (key: {})", map_id, map_key);
        // Resolve the scene ASTs before mutating the engine (native path).
        let shared_ast = self.shared_scene_ast();
        let map_ast = self.map_scene_ast(&map_key);
        // The engine is recreated per map; carry the selected script language
        // ("en"/"zh") over so a LanguageSelect choice survives map transitions
        // (bedroom → downstairs: mom's `@t` dialogue must stay Chinese).
        let script_lang = self.script_engine.script_lang().unwrap_or("en").to_string();
        self.script_engine = super::native_script::OverworldScriptEngine::new();
        self.script_engine.set_lang(&script_lang);

        match &mut self.script_engine {
            #[cfg(feature = "script-boa")]
            super::native_script::OverworldScriptEngine::Boa(engine) => {
                if let Some(shared_source) = self.script_loader.get_script("shared/pokecenter") {
                    log::info!(target: "pokered::overworld", "[Script] Loading shared/pokecenter module ({} bytes)", shared_source.len());
                    match engine.load_shared_module("pokecenter.js", shared_source) {
                        Ok(()) => log::info!(target: "pokered::overworld", "[Script] Shared pokecenter module loaded OK"),
                        Err(e) => log::warn!(target: "pokered::overworld", "[Script] Shared pokecenter module FAILED: {}", e),
                    }
                } else {
                    log::warn!(target: "pokered::overworld", "[Script] No shared/pokecenter module found in loader");
                }

                if let Some(source) = self.script_loader.get_script(&map_key) {
                    log::info!(target: "pokered::overworld", "[Script] Loading {} script ({} bytes)", map_key, source.len());
                    match engine.load_script(source) {
                        Ok(()) => log::info!(target: "pokered::overworld", "[Script] {} script loaded OK", map_key),
                        Err(e) => log::warn!(target: "pokered::overworld", "[Script] {} script FAILED: {}", map_key, e),
                    }
                } else {
                    log::warn!(target: "pokered::overworld", "[Script] No script found for key '{}'", map_key);
                }
            }
            super::native_script::OverworldScriptEngine::Native(engine) => {
                if let Some(scene) = &shared_ast {
                    engine.register_shared_scene(scene);
                }
                match map_ast {
                    Some(ref scene) => {
                        engine.load_map(&map_key, scene);
                        log::info!(target: "pokered::overworld", "[NativeScript] Loaded AST for {} ({} storylines)", map_key, scene.storylines.len());
                    }
                    None => log::warn!(target: "pokered::overworld", "[NativeScript] No scene AST found for key '{}'", map_key),
                }
            }
        }

        self.map_script_config = self
            .script_loader
            .get_config(&map_key)
            .cloned()
            .unwrap_or_default();

        log::info!(target: "pokered::overworld", "[Script] Config for {}: {} NPCs, {} signs, {} coord_events",
            map_key,
            self.map_script_config.npcs.len(),
            self.map_script_config.signs.len(),
            self.map_script_config.coord_events.len(),
        );

        self.active_script_effect = None;

        self.script_engine
            .seed_flags(&self.unified_flags.to_hashmap());

        if let Some(fn_name) = self.map_script_config.on_load() {
            log::info!(target: "pokered::overworld", "[Script] on_load function: {}", fn_name);
            if self.script_engine.has_function(fn_name) {
                log::info!(target: "pokered::overworld", "[Script] Calling on_load: {}", fn_name);
                self.script_engine
                    .set_player_position(self.state.player.x as u8, self.state.player.y as u8);
                if let Ok(Some(cmd)) = self.script_engine.call_function_no_args(fn_name) {
                    self.active_script_effect = Some(script_bridge::dispatch_command_with_names(
                        &cmd,
                        &self.player_name,
                        &self.rival_name,
                        &self.starter_display_name(),
                    ));
                }
                self.sync_flags_from_engine();
            } else {
                log::warn!(target: "pokered::overworld", "[Script] on_load function '{}' not found in module", fn_name);
            }
        }

        // ── Build triggers from config + script exports ─────────────
        self.setup_triggers_for_map(map_id);

        crate::debug_log::flush();
    }

    /// Populates the [`TriggerManager`] with entries derived from the current
    /// map's `script_config.json` and detected JS function-name patterns.
    ///
    /// The existing script system (`load_map_script`, `coord_event_fn`,
    /// `npc_talk_fn`, `on_load`) continues to work unchanged — the trigger
    /// manager runs in parallel as an additional unified dispatch layer.
    fn setup_triggers_for_map(&mut self, map_id: MapId) {
        use dotzuki_engine::metatile::TriggerType;
        use dotzuki_engine::trigger_manager::Trigger;

        let map_key = script_bridge::map_id_to_script_key(map_id);
        self.trigger_manager.remove_triggers_for_map(&map_key);
        self.trigger_manager.reset_fired_for_map(&map_key);

        // 1. Coord events → OnStep triggers
        for ce in &self.map_script_config.coord_events {
            if self.script_engine.has_function(&ce.trigger) {
                let id = format!("coord_{}", ce.name);
                self.trigger_manager.add_trigger(Trigger::single_tile(
                    id,
                    map_key.clone(),
                    TriggerType::OnStep,
                    ce.position.0 as u32,
                    ce.position.1 as u32,
                    ce.trigger.clone(),
                    ce.one_shot,
                ));
            }
        }

        // 2. on_load / enterMap → OnEnter trigger at player position
        if let Some(fn_name) = self.map_script_config.on_load() {
            if self.script_engine.has_function(fn_name) {
                self.trigger_manager.add_trigger(Trigger::single_tile(
                    format!("enter_{}", fn_name),
                    map_key.clone(),
                    TriggerType::OnEnter,
                    self.state.player.x as u32,
                    self.state.player.y as u32,
                    fn_name.to_string(),
                    true,
                ));
            }
        }

        // 3. NPC talk functions → OnInteract triggers at NPC positions
        for npc_cfg in &self.map_script_config.npcs {
            if let Some(ref talk_fn) = npc_cfg.talk {
                if self.script_engine.has_function(talk_fn) {
                    if let Some(npc) = self.npc_states.iter().find(|n| n.text_id == npc_cfg.id) {
                        self.trigger_manager.add_trigger(Trigger::single_tile(
                            format!("npc_talk_{}", npc_cfg.id),
                            map_key.clone(),
                            TriggerType::OnInteract,
                            npc.x as u32,
                            npc.y as u32,
                            talk_fn.clone(),
                            false,
                        ));
                    }
                }
            }
        }

        // 4. Sign talk functions → OnInteract triggers at sign positions
        for sign_cfg in &self.map_script_config.signs {
            if self.script_engine.has_function(&sign_cfg.talk) {
                if let Some(map) = &self.map_data {
                    if let Some(sign) = map.signs.iter().find(|s| s.text_id == sign_cfg.id) {
                        self.trigger_manager.add_trigger(Trigger::single_tile(
                            format!("sign_talk_{}", sign_cfg.id),
                            map_key.clone(),
                            TriggerType::OnInteract,
                            sign.x as u32,
                            sign.y as u32,
                            sign_cfg.talk.clone(),
                            false,
                        ));
                    }
                }
            }
        }

        log::info!(target: "pokered::overworld", "[Trigger] Built {} triggers for {}", self.trigger_manager.len(), map_key);
    }

    fn try_call_script_npc_talk(&mut self, text_id: u8) -> bool {
        if let Some(fn_name) = self.map_script_config.npc_talk_fn(text_id) {
            log::info!(target: "pokered::overworld", "[Script] NPC talk: text_id={}, fn_name={}", text_id, fn_name);
            if self.script_engine.has_function(fn_name) {
                log::info!(target: "pokered::overworld", "[Script] Calling {}", fn_name);
                self.script_engine
                    .set_player_position(self.state.player.x as u8, self.state.player.y as u8);
                if let Ok(Some(cmd)) = self.script_engine.call_function_no_args(fn_name) {
                    self.active_script_effect = Some(script_bridge::dispatch_command_with_names(
                        &cmd,
                        &self.player_name,
                        &self.rival_name,
                        &self.starter_display_name(),
                    ));
                    self.sync_flags_from_engine();
                    crate::debug_log::flush();
                    return true;
                }
                self.sync_flags_from_engine();
            } else {
                log::warn!(target: "pokered::overworld", "[Script] Function '{}' not found in module, falling back to JSON text", fn_name);
            }
        } else {
            log::info!(target: "pokered::overworld", "[Script] NPC talk: text_id={}, no script function mapped", text_id);
        }
        crate::debug_log::flush();
        false
    }

    fn try_call_script_sign_talk(&mut self, text_id: u8) -> bool {
        if let Some(fn_name) = self.map_script_config.sign_talk_fn(text_id) {
            if self.script_engine.has_function(fn_name) {
                self.script_engine
                    .set_player_position(self.state.player.x as u8, self.state.player.y as u8);
                if let Ok(Some(cmd)) = self.script_engine.call_function_no_args(fn_name) {
                    self.active_script_effect = Some(script_bridge::dispatch_command_with_names(
                        &cmd,
                        &self.player_name,
                        &self.rival_name,
                        &self.starter_display_name(),
                    ));
                    self.sync_flags_from_engine();
                    return true;
                }
                self.sync_flags_from_engine();
            }
        }
        false
    }

    fn npc_face_player(&mut self, npc_index: u8) {
        let face_dir = player_movement::opposite_direction(self.state.player.facing);
        if let Some(npc) = self
            .npc_states
            .iter_mut()
            .find(|n| n.npc_index == npc_index)
        {
            npc.facing = face_dir;
        }
    }

    fn try_show_npc_json_text(&mut self, text_id: u8) -> bool {
        if let Some(text_pages) = get_npc_text_from_json(self.state.current_map, text_id) {
            if !text_pages.is_empty() {
                let pages = self.localize_text_pages(&text_pages);
                self.pending_dialogue = Some(BedroomDialogue::from_text_pages(
                    &pages,
                    &self.player_name,
                    &self.rival_name,
                    &self.starter_display_name(),
                ));
                return true;
            }
        }
        false
    }

    fn check_wild_encounter_on_step(
        &mut self,
        map_id: MapId,
        tileset: G::Tileset,
        standing_tile: u8,
        right_tile: u8,
        standing_on_warp: bool,
        has_script_effect: bool,
    ) -> bool {        use crate::battle::wild::{EncounterContext, WildEncounterRandoms};
        use pokered_data::event_flags::EventFlag;
        use pokered_data::wild_data::GameVersion;
        use rand::Rng;

        // Original MtMoonB2F_Script: once the Super Nerd is beaten, wild
        // battles are disabled inside the fossil area (MtMoonB2FFossilAreaCoords:
        // x 11-14, y 5-8) via wStatusFlags4 BIT_NO_BATTLES.
        if map_id == MapId::MtMoonB2F
            && (11..=14).contains(&(self.state.player.x as u8))
            && (5..=8).contains(&(self.state.player.y as u8))
            && EventFlag::from_name("EVENT_BEAT_MT_MOON_EXIT_SUPER_NERD")
                .map_or(false, |f| self.unified_flags.check(f))
        {
            return false;
        }

        let version = GameVersion::Red;

        // The encounter-check gate (the classic TryDoWildEncounter call
        // conditions): only steps that may actually roll consume a REPEL
        // tick (wild_encounters.asm:19-25). The >0 → 0 transition is
        // returned so the caller replaces this step's roll with the
        // "wore off" text (.lastRepelStep).
        let roll_gated =
            !standing_on_warp && !has_script_effect && self.state.encounter_cooldown == 0;
        let mut wore_off = false;
        if roll_gated {
            if self.state.repel_steps > 0 {
                self.state.repel_steps -= 1;
                if self.state.repel_steps == 0 {
                    wore_off = true;
                }
            }
        }
        if wore_off {
            // .lastRepelStep: the "wore off" step never rolls an encounter.
            return true;
        }

        let encounter_roll = self.rng.gen_range(0u8..=255);
        let slot_roll = self.rng.gen_range(0u8..=255);

        let randoms = WildEncounterRandoms {
            encounter_roll,
            slot_roll,
        };

        let context = EncounterContext {
            repel_active: self.state.repel_steps > 0,
            // Gen-1 repel compares wild level against the LEAD (first) party
            // member's level. The OverworldScreen does not own the full party,
            // so the app layer keeps `party_lead_level` in sync from save data.
            party_lead_level: self.party_lead_level,
        };

        let result = wild_encounters::check_wild_encounter(
            map_id,
            tileset,
            standing_tile,
            right_tile,
            version,
            &randoms,
            &context,
            standing_on_warp,
            has_script_effect,
            self.state.encounter_cooldown,
        );

        if let crate::battle::wild::WildEncounterResult::Encounter { level, species } = result {
            self.pending_wild_encounter = Some(PendingWildEncounter { species, level, old_man: false, hooked: false });
        }
        wore_off
    }

    pub(crate) fn sync_flags_from_engine(&mut self) {
        let engine_flags = self.script_engine.get_all_flags();
        self.unified_flags.merge_from(&engine_flags);
    }

    /// Safari Zone step accounting, run once per completed step. Decrements the
    /// step counter while inside the zone and, when the step (or ball) counter
    /// reaches zero, ejects the player back to the gate with the game-over line.
    fn tick_safari_steps(&mut self) {
        if !self.safari_game_active {
            return;
        }
        if !pokered_data::map_flags::is_safari_zone_map(self.state.current_map) {
            return;
        }
        if self.safari_steps > 0 {
            self.safari_steps -= 1;
        }
        // Eject on empty step or ball counter. Only fire when nothing else is
        // mid-flight (a wild battle just triggered, an active script, or an open
        // text box); the check re-runs on the next step, so the eject is not lost.
        if (self.safari_steps == 0 || self.safari_balls == 0)
            && self.pending_wild_encounter.is_none()
            && self.active_script_effect.is_none()
            && self.pending_dialogue.is_none()
            && self.safari_eject_pending.is_none()
        {
            self.trigger_safari_game_over();
        }
    }

    /// End the Safari game: show the announcer's "game over" line and queue the
    /// eject warp back to the gate (fired once the message is dismissed).
    fn trigger_safari_game_over(&mut self) {
        self.end_safari_game();
        let msg = "PA: Ding-ding!\nYour SAFARI GAME is over!";
        self.pending_dialogue =
            Some(screen::BedroomDialogue::from_message(&self.localize_message(msg)));
        self.safari_eject_pending = Some(PendingWarp {
            dest_map: MapId::SafariZoneGate,
            dest_x: screen::SAFARI_GATE_RETURN_X,
            dest_y: screen::SAFARI_GATE_RETURN_Y,
            save_last_map: false,
            // Plain fade transition back to the gate.
            arrival_spin: false,
        });
    }
}

#[cfg(test)]
mod safari_timer_tests {
    use super::*;
    use pokered_data::impl_traits::PokemonRedData;

    fn screen_at(map: MapId) -> OverworldScreen<PokemonRedData> {
        OverworldScreen::new(map, None, PokemonRedData)
    }

    fn warp_to(ow: &mut OverworldScreen<PokemonRedData>, dest: MapId) {
        ow.pending_warp = Some(PendingWarp {
            dest_map: dest,
            dest_x: 0,
            dest_y: 0,
            save_last_map: false,
            arrival_spin: false,
        });
        ow.commit_pending_warp();
    }

    #[test]
    fn entering_zone_arms_the_game() {
        let mut ow = screen_at(MapId::SafariZoneGate);
        assert!(!ow.is_safari_game_active());
        warp_to(&mut ow, MapId::SafariZoneCenter);
        assert!(ow.is_safari_game_active());
        assert_eq!(ow.safari_steps_remaining(), screen::SAFARI_ZONE_STEP_COUNT);
        assert_eq!(ow.safari_balls_remaining(), screen::SAFARI_ZONE_BALL_COUNT);
    }

    #[test]
    fn leaving_zone_resets_the_game() {
        let mut ow = screen_at(MapId::SafariZoneGate);
        warp_to(&mut ow, MapId::SafariZoneCenter);
        assert!(ow.is_safari_game_active());
        warp_to(&mut ow, MapId::SafariZoneGate);
        assert!(!ow.is_safari_game_active());
        assert_eq!(ow.safari_steps_remaining(), 0);
    }

    #[test]
    fn each_step_decrements_the_counter() {
        let mut ow = screen_at(MapId::SafariZoneGate);
        warp_to(&mut ow, MapId::SafariZoneCenter);
        let before = ow.safari_steps_remaining();
        ow.tick_safari_steps();
        assert_eq!(ow.safari_steps_remaining(), before - 1);
    }

    #[test]
    fn zero_steps_ends_game_and_queues_eject() {
        let mut ow = screen_at(MapId::SafariZoneGate);
        warp_to(&mut ow, MapId::SafariZoneCenter);
        ow.safari_steps = 1;
        ow.tick_safari_steps();
        assert_eq!(ow.safari_steps_remaining(), 0);
        assert!(!ow.is_safari_game_active());
        assert!(ow.pending_dialogue.is_some());
        let warp = ow.safari_eject_pending.expect("eject queued");
        assert_eq!(warp.dest_map, MapId::SafariZoneGate);
    }

    #[test]
    fn zero_balls_ends_game() {
        let mut ow = screen_at(MapId::SafariZoneGate);
        warp_to(&mut ow, MapId::SafariZoneCenter);
        ow.safari_balls = 0;
        ow.tick_safari_steps();
        assert!(!ow.is_safari_game_active());
        assert!(ow.safari_eject_pending.is_some());
    }

    #[test]
    fn using_balls_counts_down() {
        let mut ow = screen_at(MapId::SafariZoneGate);
        warp_to(&mut ow, MapId::SafariZoneCenter);
        assert_eq!(ow.use_safari_ball(), screen::SAFARI_ZONE_BALL_COUNT - 1);
    }

    #[test]
    fn no_decrement_outside_zone() {
        let mut ow = screen_at(MapId::SafariZoneGate);
        // Manually pretend a game is active but we are on a non-zone map.
        ow.start_safari_game();
        let before = ow.safari_steps_remaining();
        ow.tick_safari_steps();
        assert_eq!(ow.safari_steps_remaining(), before);
    }
}

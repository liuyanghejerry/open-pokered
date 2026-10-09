//! Out-of-battle field-move (HM) dispatch — the party-menu effects of
//! CUT / FLY / SURF / STRENGTH / FLASH / DIG / TELEPORT / SOFTBOILED.
//!
//! Gen-1 references:
//! - engine/menus/start_sub_menus.asm — `StartMenu_Pokemon`
//!   `.outOfBattleMovePointers` (badge gates, per-move flow, messages,
//!   `.softboiled` target pick)
//! - engine/items/item_effects.asm — `ItemUseSurfboard` / `ItemUseEscapeRope`
//!   / `ItemUseMedicine` (the SOFTBOILED pseudo-item heal)
//! - engine/overworld/cut.asm — `UsedCut`
//! - engine/overworld/field_move_messages.asm — `PrintStrengthText` /
//!   `IsSurfingAllowed`
//!
//! The pure gating logic lives in [`super::hm_effects`] (unit-tested); this
//! module wires it to the live [`OverworldScreen`].

use crate::alloc_prelude::*;
use dotzuki_engine::overworld::types::TransportMode;
use dotzuki_engine::GameData;
use pokered_data::event_flags::EventFlag;
use pokered_data::items::ItemId;
use pokered_data::maps::MapId;
use pokered_data::moves::MoveId;
use pokered_data::tilesets::TilesetId;

use super::hm_effects::{self, BoulderPushResult, CutResult, FlashResult, FlyResult, StrengthResult, SurfResult};
use super::screen::{
    BedroomDialogue, OverworldAudioRequest, OverworldScreen, PendingCut, PendingWarp, WarpFadeState,
};
use super::collision::CollisionProvider;
use super::{collision, player_movement, presentation, special_terrain, Direction};
use crate::battle::state::Pokemon;

/// Gen-1 water tile ID ($14 — `IsNextTileShoreOrWater`).
const WATER_TILE: u8 = 0x14;
/// Eastern shoreline tiles that also allow starting to surf ($32 usual,
/// $48 Safari Zone), except on the Vermilion Dock (SHIP_PORT) tileset.
const SHORE_TILE_USUAL: u8 = 0x32;
const SHORE_TILE_SAFARI: u8 = 0x48;

/// "No! A new BADGE is required." (data/text/text_5.asm _NewBadgeRequiredText).
const NEW_BADGE_REQUIRED_TEXT: &str = "No! A new BADGE\nis required.";

/// What happened after the player chose a field move in the party menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldMoveOutcome {
    /// The flow completed on the spot: a message was queued in
    /// `pending_dialogue` when there was something to say (CUT/SURF/
    /// STRENGTH/FLASH/TELEPORT and all refusals), or the escape warp was
    /// queued directly (DIG, successful SURF dismount).
    Done,
    /// FLY: badge + outdoor checks passed; the caller should open the town
    /// map in fly-destination mode so the player can pick a target.
    OpenFlyMap,
    /// SOFTBOILED: the user is healthy enough; the caller should reopen the
    /// party menu in target-pick mode so the player can choose who to heal
    /// (Gen-1 `.softboiled` → `GoBackToPartyMenu`).
    ChooseSoftboiledTarget,
}

impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    /// Use a field move from the party menu.
    ///
    /// `mon` is the party member the move belongs to (its name is used in the
    /// Gen-1 message texts; its cry plays for STRENGTH). `obtained_badges`
    /// and `last_blackout_map` come from the persistent game data (the
    /// overworld does not own the save).
    pub fn use_field_move(
        &mut self,
        move_id: MoveId,
        mon: &Pokemon,
        obtained_badges: u8,
        last_blackout_map: MapId,
    ) -> FieldMoveOutcome {
        let mut name_buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
        let mon_name = mon.display_name(&mut name_buf);
        match move_id {
            MoveId::Cut => self.field_cut(obtained_badges, mon_name),
            MoveId::Fly => self.field_fly(obtained_badges, mon_name),
            MoveId::Surf => self.field_surf(obtained_badges, mon_name),
            MoveId::Strength => self.field_strength(obtained_badges, mon),
            MoveId::Flash => self.field_flash(obtained_badges),
            MoveId::Dig => self.field_dig(last_blackout_map),
            MoveId::Teleport => self.field_teleport(mon_name, last_blackout_map),
            MoveId::Softboiled => self.field_softboiled(mon),
            // Not a field move — the party menu never offers it.
            _ => self.field_message("This isn't the\ntime to use that!"),
        }
    }

    fn field_message(&mut self, text: &str) -> FieldMoveOutcome {
        self.pending_dialogue =
            Some(BedroomDialogue::from_message(&self.localize_message(text)));
        FieldMoveOutcome::Done
    }

    /// Tile the player is standing on, the tile directly ahead, and the
    /// coordinates of that ahead-tile — the inputs every field move checks.
    pub(crate) fn tiles_in_front(&self) -> Option<(u8, u8, u16, u16)> {
        let map = self.map_data.as_ref()?;
        let provider = collision::PokemonCollisionProvider::new(map.id, map.tileset);
        let (dx, dy) = player_movement::direction_delta(self.state.player.facing);
        let fx = (self.state.player.x as i32 + dx as i32).max(0) as u16;
        let fy = (self.state.player.y as i32 + dy as i32).max(0) as u16;
        let standing = provider.get_tile_at_position(
            map.tileset,
            &map.blocks,
            map.width,
            self.state.player.x,
            self.state.player.y,
        );
        let in_front =
            provider.get_tile_at_position(map.tileset, &map.blocks, map.width, fx, fy);
        Some((standing, in_front, fx, fy))
    }

    // ── CUT ──────────────────────────────────────────────────────────
    //
    // UsedCut (engine/overworld/cut.asm): needs the CASCADE badge and a
    // cuttable tree ($3D overworld / $50 gym) or grass ($52) in front.
    fn field_cut(&mut self, obtained_badges: u8, mon_name: &str) -> FieldMoveOutcome {
        let Some((_, tile_in_front, fx, fy)) = self.tiles_in_front() else {
            return self.field_message("There isn't\nanything to CUT!");
        };
        let map = self.map_data.as_ref().expect("map_data present");
        let current_block = collision::get_block_at(fx, fy, map.width, &map.blocks).unwrap_or(0);
        match hm_effects::use_cut(obtained_badges, map.tileset, tile_in_front, current_block) {
            CutResult::NoBadge => self.field_message(NEW_BADGE_REQUIRED_TEXT),
            CutResult::NothingToCut => self.field_message("There isn't\nanything to CUT!"),
            CutResult::CutTree { replacement_block } => {
                self.pending_cut = Some(PendingCut {
                    block_x: (fx / 2) as u8,
                    block_y: (fy / 2) as u8,
                    replacement_block: Some(replacement_block),
                    kind: presentation::CutAnimKind::Tree,
                });
                self.field_message(&format!("{} hacked\naway with CUT!", mon_name))
            }
            // Gen-1 grass cutting plays the same animation + text but does not
            // alter the map (grass blocks are not in CutTreeBlockSwaps).
            CutResult::CutGrass => {
                self.pending_cut = Some(PendingCut {
                    block_x: (fx / 2) as u8,
                    block_y: (fy / 2) as u8,
                    replacement_block: None,
                    kind: presentation::CutAnimKind::Grass,
                });
                self.field_message(&format!("{} hacked\naway with CUT!", mon_name))
            }
        }
    }

    // ── FLY ──────────────────────────────────────────────────────────
    //
    // start_sub_menus.asm .fly: THUNDER badge + CheckIfInOutsideMap, then
    // ChooseFlyDestination. The destination pick happens on the town map
    // screen; this only performs the gating and hands off.
    fn field_fly(&mut self, obtained_badges: u8, mon_name: &str) -> FieldMoveOutcome {
        let Some(map) = self.map_data.as_ref() else {
            return self.field_message(&format!("{} can't\nFLY here.", mon_name));
        };
        match hm_effects::use_fly(obtained_badges, map.tileset, None) {
            FlyResult::NoBadge => self.field_message(NEW_BADGE_REQUIRED_TEXT),
            FlyResult::CannotFlyHere => {
                self.field_message(&format!("{} can't\nFLY here.", mon_name))
            }
            // No destination chosen yet (we passed None): open the fly map.
            FlyResult::Cancelled | FlyResult::ChoseDestination { .. } => {
                FieldMoveOutcome::OpenFlyMap
            }
        }
    }

    // ── SURF ─────────────────────────────────────────────────────────
    //
    // start_sub_menus.asm .surf: SOUL badge + IsSurfingAllowed, then
    // ItemUseSurfboard (engine/items/item_effects.asm).
    fn field_surf(&mut self, obtained_badges: u8, mon_name: &str) -> FieldMoveOutcome {
        let Some((standing_tile, tile_in_front, _, _)) = self.tiles_in_front() else {
            return self.field_message(&format!("No SURFing on\n{}\nhere!", mon_name));
        };
        let map = self.map_data.as_ref().expect("map_data present");
        let tileset = map.tileset;
        let current_map = self.state.current_map;
        let already_surfing = self.state.player.transport == TransportMode::Surfing;
        let seafoam_b4f_boulders_done = self
            .unified_flags
            .check(EventFlag::EVENT_SEAFOAM4_BOULDER1_DOWN_HOLE)
            && self
                .unified_flags
                .check(EventFlag::EVENT_SEAFOAM4_BOULDER2_DOWN_HOLE);
        // IsNextTileShoreOrWater: a water tileset and a water/shore tile ahead.
        let is_facing_water = pokered_data::tileset_data::is_water_tileset(tileset)
            && (tile_in_front == WATER_TILE
                || (tileset != TilesetId::ShipPort
                    && (tile_in_front == SHORE_TILE_USUAL || tile_in_front == SHORE_TILE_SAFARI)));
        // BIT_ALWAYS_ON_BIKE — the Cycling Road forced-bike lock, set by
        // CheckForceBikeOrSurf on map entry (forced_bike.rs); while active,
        // IsSurfingAllowed refuses with the "Cycling is fun!" text.
        let forced_bike = self.forced_bike.active;
        match hm_effects::use_surf(
            obtained_badges,
            tileset,
            is_facing_water,
            already_surfing,
            forced_bike,
            current_map,
            seafoam_b4f_boulders_done,
            self.state.player.x as u8,
            self.state.player.y as u8,
        ) {
            SurfResult::NoBadge => self.field_message(NEW_BADGE_REQUIRED_TEXT),
            SurfResult::AlreadySurfing => self.try_stop_surfing(standing_tile, tile_in_front),
            SurfResult::NotFacingWater => {
                self.field_message(&format!("No SURFing on\n{}\nhere!", mon_name))
            }
            SurfResult::ForcedToRideBike => {
                self.field_message("Cycling is fun!\nForget SURFing!")
            }
            SurfResult::CurrentTooFast => {
                self.field_message("The current is\nmuch too fast!")
            }
            SurfResult::StartedSurfing => {
                // ItemUseSurfboard: TilePairCollisionsWater may still veto.
                let blocked = pokered_data::collision::check_tile_pair_collision(
                    tileset,
                    standing_tile,
                    tile_in_front,
                    true,
                );
                if blocked {
                    return self.field_message(&format!("No SURFing on\n{}\nhere!", mon_name));
                }
                self.state.player.transport = TransportMode::Surfing;
                self.defer_field_move_step();
                // PlayDefaultMusic: surfing music follows the transport mode.
                self.audio_requests
                    .push(OverworldAudioRequest::PlayMapMusic { map: current_map });
                self.field_message(&format!("{} got on\n{}!", self.player_name, mon_name))
            }
        }
    }

    /// ItemUseSurfboard .tryToStopSurfing: step off onto a passable land
    /// tile; no text on success, "There's no place to get off!" otherwise.
    fn try_stop_surfing(&mut self, standing_tile: u8, tile_in_front: u8) -> FieldMoveOutcome {
        let map = self.map_data.as_ref().expect("map_data present");
        let tileset = map.tileset;
        let current_map = self.state.current_map;
        let sprite_ahead = self.sprite_in_front_of_player().is_some();
        let pair_blocked =
            pokered_data::collision::check_tile_pair_collision(tileset, standing_tile, tile_in_front, true);
        let land_passable = pokered_data::collision::is_tile_passable(tileset, tile_in_front);
        if sprite_ahead || pair_blocked || !land_passable {
            return self.field_message("There's no place\nto get off!");
        }
        self.state.player.transport = TransportMode::Walking;
        self.defer_field_move_step();
        // PlayDefaultMusic: back to the map's own music.
        self.audio_requests
            .push(OverworldAudioRequest::PlayMapMusic { map: current_map });
        FieldMoveOutcome::Done
    }

    /// The Gen-1 `.makePlayerMoveForward` step: walk one tile ahead via the
    /// scripted-movement path (the tile was already validated by the caller).
    fn defer_field_move_step(&mut self) {
        self.pending_field_move_step = Some(presentation::FieldMoveStepState::new(
            self.state.player.x,
            self.state.player.y,
            self.state.player.facing,
        ));
        self.field_move_step_needs_restore = true;
    }

    /// First visible NPC (if any) standing on the tile the player faces.
    fn sprite_in_front_of_player(&self) -> Option<usize> {
        let (dx, dy) = player_movement::direction_delta(self.state.player.facing);
        let fx = (self.state.player.x as i32 + dx as i32).max(0) as u16;
        let fy = (self.state.player.y as i32 + dy as i32).max(0) as u16;
        self.npc_states
            .iter()
            .position(|n| n.visible && n.x == fx && n.y == fy)
    }

    // ── STRENGTH ─────────────────────────────────────────────────────
    //
    // PrintStrengthText (engine/overworld/field_move_messages.asm): sets
    // BIT_STRENGTH_ACTIVE and prints both texts; the mon's cry plays.
    fn field_strength(&mut self, obtained_badges: u8, mon: &Pokemon) -> FieldMoveOutcome {
        match hm_effects::use_strength(obtained_badges, self.strength_active) {
            StrengthResult::NoBadge => self.field_message(NEW_BADGE_REQUIRED_TEXT),
            // AlreadyActive re-prints the same texts in the original
            // (PrintStrengthText is unconditional).
            StrengthResult::Activated | StrengthResult::AlreadyActive => {
                self.strength_active = true;
                self.audio_requests.push(OverworldAudioRequest::PlayCry {
                    species: format!("{:?}", mon.species),
                });
                let mut name_buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
                let name = mon.display_name(&mut name_buf);
                self.field_message(&format!(
                    "{} used\nSTRENGTH.\n{} can\nmove boulders.",
                    name, name
                ))
            }
        }
    }

    // ── FLASH ────────────────────────────────────────────────────────
    //
    // start_sub_menus.asm .flash: BOULDER badge, then wMapPalOffset = 0 and
    // the "blinding FLASH" text (printed even outside dark caves).
    fn field_flash(&mut self, obtained_badges: u8) -> FieldMoveOutcome {
        match hm_effects::use_flash(obtained_badges, self.dark_cave.is_dark()) {
            FlashResult::NoBadge => self.field_message(NEW_BADGE_REQUIRED_TEXT),
            FlashResult::LitUpCave => {
                self.dark_cave.use_flash();
                // GBPalWhiteOutWithDelay3: the screen whites out for 3 frames
                // once the "blinding FLASH" text is dismissed.
                self.flash_pending_white = true;
                self.field_message("A blinding FLASH\nlights the area!")
            }
            FlashResult::AlreadyLit => {
                self.dark_cave.use_flash();
                self.field_message("A blinding FLASH\nlights the area!")
            }
        }
    }

    // ── DIG ──────────────────────────────────────────────────────────
    //
    // start_sub_menus.asm .dig (195-203): DIG is the ESCAPE_ROPE item effect
    // (no badge check); `wPseudoItemID` marks the "using Dig" state so
    // `ItemUseEscapeRope` skips the item removal — nothing is consumed when
    // used as a move. The warp target is the last Pokémon Center
    // (wLastBlackoutMap → FlyWarpDataPtr, special_warps.asm:76-80) — NOT the
    // dungeon entrance.
    fn field_dig(&mut self, last_blackout_map: MapId) -> FieldMoveOutcome {
        // Reuse the escape-rope flow; the `consumed` flag only tells bag-item
        // callers to remove the item, so it is ignored for the move.
        let _ = self.use_field_item(ItemId::EscapeRope, last_blackout_map);
        FieldMoveOutcome::Done
    }

    // ── TELEPORT ─────────────────────────────────────────────────────
    //
    // start_sub_menus.asm .teleport: outside maps only; prints
    // "Warp to the last #MON CENTER." and fly-warps to wLastBlackoutMap's
    // fly point once the text is dismissed.
    fn field_teleport(&mut self, mon_name: &str, last_blackout_map: MapId) -> FieldMoveOutcome {
        let outside = self
            .map_data
            .as_ref()
            .map(|m| special_terrain::is_outside_map(m.tileset))
            .unwrap_or(false);
        if !outside {
            return self.field_message(&format!("{} can't\nuse TELEPORT\nnow.", mon_name));
        }
        let dest = hm_effects::fly_destination_for_map(last_blackout_map)
            .or_else(|| hm_effects::fly_destination_for_map(MapId::PalletTown))
            .expect("Pallet Town always has a fly point");
        self.post_dialogue_warp = Some(PendingWarp {
            dest_map: dest.map,
            dest_x: dest.x,
            dest_y: dest.y,
            save_last_map: false,
            // ItemUseTeleport / ItemUseEscapeRope set BIT_FLY_WARP
            // (item_effects.asm:1509) → the arrival plays EnterMapAnim.
            arrival_spin: true,
        });
        self.field_message("Warp to the last\n#MON CENTER.")
    }

    // ── SOFTBOILED ───────────────────────────────────────────────────
    //
    // start_sub_menus.asm .softboiled (236-274): the user must have more than
    // 1/5 of its max HP left, otherwise "Not healthy enough."; then the party
    // menu reopens (ItemUseMedicine's `GoBackToPartyMenu` — the caller picks
    // the target via `PartyScreenMode::SoftboiledTarget`). The actual heal
    // (user loses 1/5 max HP, target gains it, capped at max) is applied by
    // the frontend through [`crate::items::bag_use::apply_softboiled`] so the
    // live party data is mutated. No PP is spent (field moves never are).
    fn field_softboiled(&mut self, mon: &Pokemon) -> FieldMoveOutcome {
        let cost = mon.max_hp / 5;
        if mon.hp <= cost {
            // _NotHealthyEnoughText (data/text/text_5.asm:55-58).
            return self.field_message("Not healthy\nenough.");
        }
        FieldMoveOutcome::ChooseSoftboiledTarget
    }

    /// Begin a fly-warp to an overworld destination — FLY's chosen town-map
    /// target. Mirrors the BIT_FLY_WARP handling in special_warps.asm: the
    /// `_LeaveMapAnim` first plays the full bird pickup/departure, then fades
    /// to white and lands the player at the map's fly point.
    pub fn fly_warp_to(&mut self, dest_map: MapId, dest_x: u8, dest_y: u8) {
        // BIT_USED_FLY (player_animations.asm:55-70): the arrival plays the
        // BIRD animation instead of the spin-in.
        self.pending_fly_arrival = true;
        self.pending_warp = Some(PendingWarp {
            dest_map,
            dest_x,
            dest_y,
            save_last_map: false,
            // The fly picker sets BIT_FLY_WARP (town_map.asm:214) → the
            // arrival plays EnterMapAnim.
            arrival_spin: true,
        });
        self.warp_fade_to_white = true;
        self.warp_fade_state = WarpFadeState::Idle;
        self.fly_departure = Some(presentation::LeaveMapFlyState::new());
        self.audio_requests.push(OverworldAudioRequest::StopMusic);
    }

    // ── Boulder pushing (STRENGTH) ────────────────────────────────────
    //
    // TryPushingBoulder (engine/overworld/push_boulder.asm), called once per
    // overworld frame from RunMapScript. While STRENGTH is active, facing a
    // boulder and holding the d-pad toward it for two consecutive checks
    // (BIT_TRIED_PUSH_BOULDER) slides it one tile — provided the tile beyond
    // is clear (CheckForCollisionWhenPushingBoulder).

    /// Per-frame boulder-push check. `held_direction` is the d-pad direction
    /// currently held (hJoyHeld), if any.
    pub(crate) fn tick_boulder_push(&mut self, held_direction: Option<Direction>) {
        if self.boulder_push.is_some() {
            self.advance_boulder_push();
            return;
        }
        if !self.strength_active {
            return;
        }
        let facing = self.state.player.facing;
        let (dx, dy) = player_movement::direction_delta(facing);
        let fx = (self.state.player.x as i32 + dx as i32).max(0) as u16;
        let fy = (self.state.player.y as i32 + dy as i32).max(0) as u16;
        let sprite_in_front = self
            .npc_states
            .iter()
            .position(|n| n.visible && n.x == fx && n.y == fy);
        let Some(npc_index) = sprite_in_front else {
            // ResetBoulderPushFlags: nothing in front of the player.
            self.tried_push_boulder = false;
            return;
        };
        let is_boulder =
            self.npc_states[npc_index].sprite_id == pokered_data::sprites::SpriteId::Boulder as u8;
        if !is_boulder {
            self.tried_push_boulder = false;
            return;
        }
        // The boulder's destination: one tile further in the facing
        // direction (GetTileTwoStepsInFrontOfPlayer).
        let beyond_x = (fx as i32 + dx as i32).max(0) as u16;
        let beyond_y = (fy as i32 + dy as i32).max(0) as u16;
        let boulder_blocked = self.boulder_push_blocked(beyond_x, beyond_y);
        let already_tried = self.tried_push_boulder;
        match hm_effects::try_push_boulder_with_direction(
            true,
            false,
            Some(npc_index as u8),
            is_boulder,
            already_tried,
            facing,
            held_direction,
            boulder_blocked,
        ) {
            BoulderPushResult::Pushed { direction } => {
                let (ddx, ddy) = player_movement::direction_delta(direction);
                let npc = &mut self.npc_states[npc_index];
                let destination = ((npc.x as i32 + ddx as i32).max(0) as u16,
                    (npc.y as i32 + ddy as i32).max(0) as u16);
                npc.facing = direction;
                npc.walk_counter = 16;
                self.tried_push_boulder = false;
                self.boulder_dust_frames = BOULDER_DUST_FRAMES;
                self.boulder_dust = presentation::BoulderDustState::inactive();
                self.boulder_push = Some(presentation::BoulderPushState {
                    npc_index, direction, destination, origin: (npc.x,npc.y),
                    anchor: (self.state.player.x, self.state.player.y), frame: 0,
                    switch_block: None, redraw_remaining: 0,
                });
                self.audio_requests.push(OverworldAudioRequest::PlaySound {
                    sound_id: "SFX_PUSH_BOULDER".to_string(),
                });

            }
            BoulderPushResult::NeedPushAgain => {
                // First contact — set BIT_TRIED_PUSH_BOULDER; the next frame
                // (still holding) completes the push.
                self.tried_push_boulder = true;
            }
            BoulderPushResult::BoulderBlocked => {
                // ResetBoulderPushFlags on collision beyond the boulder.
                self.tried_push_boulder = false;
            }
            BoulderPushResult::StrengthNotActive
            | BoulderPushResult::NoBoulderInFront
            | BoulderPushResult::NotABoulder
            | BoulderPushResult::NotPushingCorrectDirection => {}
        }
    }

    pub fn boulder_blocks_control(&self) -> bool {
        self.boulder_push.is_some_and(|p|p.frame<presentation::BoulderPushState::COMPLETION_FRAME)
    }

    /// Finish the retained LCD image even when START has taken over the UI.
    /// This never advances a blocking push or consumes player input.
    pub fn tick_boulder_presentation_during_ui(&mut self) {
        if self.boulder_push.is_some_and(|p|p.frame>=presentation::BoulderPushState::COMPLETION_FRAME) {
            self.advance_boulder_push();
        }
    }

    /// CheckForCollisionWhenPushingBoulder: the tile beyond the boulder at
    /// (`bx`, `by`) must be passable, free of sprites, not a stairs tile,
    /// and not an elevation change from the player's tile.
    pub(crate) fn advance_boulder_push(&mut self) {
        let Some(mut push) = self.boulder_push else { return; };
        if push.redraw_remaining != 0 {
            push.redraw_remaining -= 1;
            self.boulder_push = Some(push);
            return;
        }
        push.frame = push.frame.saturating_add(1);
        // TryWalking commits map coordinates before the first sprite pixel.
        if push.frame == 2 {
            self.npc_states[push.npc_index].x = push.destination.0;
            self.npc_states[push.npc_index].y = push.destination.1;
        }
        // 1F/2F check the destination on the next map-script iteration.
        // Their block replacement runs one iteration after setting the flag.
        if push.frame == 4 && matches!(self.state.current_map, MapId::VictoryRoad1F | MapId::VictoryRoad2F) {
            push.switch_block = self.activate_victory_road_switch(push.destination.0, push.destination.1);
        }
        if push.frame == 6 {
            if let Some((x, y, block)) = push.switch_block.take() {
                if let Some(map) = self.map_data.as_mut() { map.set_block(x, y, block); }
                if self.boulder_switch_needs_redraw(x, y) { push.redraw_remaining = 9; }
            }
        }
        self.boulder_dust_frames = BOULDER_DUST_FRAMES.saturating_sub(push.frame);
        let pixels = push.slide_pixels();
        if pixels < 16 {
            self.npc_states[push.npc_index].walk_counter = 16 - pixels;
        } else if self.npc_states[push.npc_index].walk_counter != 0 {
            let npc = &mut self.npc_states[push.npc_index];
            npc.x = push.destination.0; npc.y = push.destination.1; npc.walk_counter = 0;
        }
        if push.frame == presentation::BoulderPushState::DUST_FIRST_FRAME {
            self.boulder_dust = presentation::BoulderDustState::new(push.direction, push.anchor.0, push.anchor.1);
        } else if push.frame > presentation::BoulderPushState::DUST_FIRST_FRAME
            && push.frame < presentation::BoulderPushState::DUST_LAST_FRAME {
            self.boulder_dust.tick();
        } else if push.frame > presentation::BoulderPushState::DUST_LAST_FRAME {
            self.boulder_dust = presentation::BoulderDustState::inactive();
        }
        if push.frame == presentation::BoulderPushState::COMPLETION_FRAME {
            // DiscardButtonPresses clears hJoyHeld before resumed Joypad.
            self.sampled_player_input=dotzuki_engine::overworld::OverworldInput::new(false,false,false,false,false,false,false,false);
            self.commit_seafoam_boulder_hole(push.npc_index);
            if self.state.current_map == MapId::VictoryRoad3F {
                self.commit_boulder_landing(push.npc_index);
            }
            self.tried_push_boulder = false;
            self.audio_requests.push(OverworldAudioRequest::PlaySound { sound_id: "SFX_CUT".to_string() });
        }
        if push.frame == presentation::BoulderPushState::LAST_FRAME {
            self.boulder_dust_frames = 0;
            self.boulder_push = None;
        } else {
            self.boulder_push = Some(push);
        }
    }

    fn activate_victory_road_switch(&mut self, x: u16, y: u16) -> Option<(u8, u8, u8)> {
        let (name, bx, by, block) = victory_road_switch_for(self.state.current_map, x, y)?;
        let flag = pokered_data::event_flags::EventFlag::from_name(name)?;
        if self.unified_flags.check(flag) { return None; }
        self.unified_flags.set(flag);
        Some((bx, by, block))
    }

    /// ReplaceTileBlock uses a linear WRAM address interval, including the
    /// padded connection columns, rather than a rectangular viewport test.
    fn boulder_switch_needs_redraw(&self, x: u8, y: u8) -> bool {
        let Some(map) = self.map_data.as_ref() else { return false; };
        let stride = i32::from(map.width) + 6;
        let top = (i32::from(self.state.player.y) - 4).div_euclid(2) * stride
            + (i32::from(self.state.player.x) - 4).div_euclid(2);
        let address = i32::from(y) * stride + i32::from(x);
        (top..=top + 4 * stride + 6).contains(&address)
    }

    fn commit_boulder_landing(&mut self, npc_index: usize) {
        let (x,y) = (self.npc_states[npc_index].x,self.npc_states[npc_index].y);
                if self.state.current_map == MapId::VictoryRoad3F && (x, y) == (23, 15) {
                    self.npc_states[npc_index].visible = false;
                    self.unified_flags.set(pokered_data::event_flags::EventFlag::EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2);
                    pokered_data::toggleable_objects::set_object_hidden(&mut self.toggleable_object_flags, 0x7A);
                    pokered_data::toggleable_objects::set_object_shown(&mut self.toggleable_object_flags, 0x60);
                }
                // VictoryRoad boulder-on-switch detection (CheckBoulderCoords +
                // SetEvent in VictoryRoad1F/2F/3F DefaultScript): a boulder
                // pushed onto the floor switch sets the floor's ON_SWITCH event
                // and opens the path block (ReplaceTileBlock) — the audit's
                // talk/step approximations bypassed this check.
                if let Some((flag_name, block_x, block_y, open_block)) =
                    victory_road_switch_for(self.state.current_map, x, y)
                {
                    if let Some(flag) =
                        pokered_data::event_flags::EventFlag::from_name(flag_name)
                    {
                        self.unified_flags.set(flag);
                    }
                    if let Some(map) = self.map_data.as_mut() {
                        map.set_block(block_x, block_y, open_block);
                    }
                }
    }

    /// Seafoam scripts test BIT_PUSHED_BOULDER, set only after dust and
    /// graphics restoration. A stone remains visible throughout its slide.
    fn commit_seafoam_boulder_hole(&mut self, npc_index: usize) {
        let (x,y)=(self.npc_states[npc_index].x,self.npc_states[npc_index].y);
                // Seafoam Islands boulder-into-hole: pushing a boulder onto one
                // of the floor's hole tiles drops it through (the original hides
                // the object and sets the per-boulder DOWN_HOLE event; the lower
                // floor reveals its twin via those events).
                if let Some(flag_name) = seafoam_hole_flag_for(self.state.current_map, x, y)
                {
                    self.npc_states[npc_index].visible = false;
                    if let Some(flag) =
                        pokered_data::event_flags::EventFlag::from_name(flag_name)
                    {
                        self.unified_flags.set(flag);
                    }
                }
    }

    fn boulder_push_blocked(&self, bx: u16, by: u16) -> bool {
        let Some(map) = self.map_data.as_ref() else {
            return true;
        };
        if bx >= (map.width as u16) * 2 || by >= (map.height as u16) * 2 {
            return true;
        }
        let provider = collision::PokemonCollisionProvider::new(map.id, map.tileset);
        let beyond_tile =
            provider.get_tile_at_position(map.tileset, &map.blocks, map.width, bx, by);
        // Tile two steps ahead must be passable.
        if !pokered_data::collision::is_tile_passable(map.tileset, beyond_tile) {
            return true;
        }
        // Stairs tile ($15) blocks boulders.
        if beyond_tile == 0x15 {
            return true;
        }
        // Elevation check between the player's tile and the boulder's target.
        let standing_tile = provider.get_tile_at_position(
            map.tileset,
            &map.blocks,
            map.width,
            self.state.player.x,
            self.state.player.y,
        );
        if pokered_data::collision::check_tile_pair_collision(
            map.tileset,
            standing_tile,
            beyond_tile,
            false,
        ) {
            return true;
        }
        // No sprite at the destination.
        self.npc_states
            .iter()
            .any(|n| n.visible && n.x == bx && n.y == by)
    }
}

/// Frames of boulder-dust lockout after a successful push — the boulder's
/// one-tile slide plus the dust puff (BIT_BOULDER_DUST).
pub(crate) const BOULDER_DUST_FRAMES: u8 = presentation::BoulderPushState::LAST_FRAME + 1;

/// Map a boulder's resting tile in the Seafoam Islands to the original
/// EVENT_SEAFOAM{n}_BOULDER{m}_DOWN_HOLE flag, if that tile is one of the
/// floor's holes (Seafoam{n}HolesCoords). When a Strength boulder is pushed
/// onto a hole it falls through to the floor below.
/// VictoryRoad floor-switch coordinates and the block swap each one performs
/// when a Strength boulder rests on it (VictoryRoad1F/2F/3F DefaultScript
/// .SwitchCoords + the matching ReplaceTileBlock in the load scripts).
pub(crate) fn victory_road_switch_for(
    map_id: MapId,
    x: u16,
    y: u16,
) -> Option<(&'static str, u8, u8, u8)> {
    let switches: &[(u16, u16, &'static str, u8, u8, u8)] = match map_id {
        // dbmapcoord 17,13 -> block X=4, Y=6, open id $1d
        MapId::VictoryRoad1F => {
            &[(17, 13, "EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH", 4, 6, 29)]
        }
        // dbmapcoord 1,16 -> block X=3, Y=4, open id $15; 9,16 -> X=11, Y=7, $1d
        MapId::VictoryRoad2F => &[
            (1, 16, "EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1", 3, 4, 21),
            (9, 16, "EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH2", 11, 7, 29),
        ],
        // dbmapcoord 3,5 -> block X=3, Y=5, open id $1d
        MapId::VictoryRoad3F => {
            &[(3, 5, "EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH1", 3, 5, 29)]
        }
        _ => &[],
    };
    switches
        .iter()
        .find(|(sx, sy, _, _, _, _)| *sx == x && *sy == y)
        .map(|(_, _, flag, bx, by, id)| (*flag, *bx, *by, *id))
}

pub(crate) fn seafoam_hole_flag_for(map_id: MapId, x: u16, y: u16) -> Option<&'static str> {
    let holes: &[(u16, u16, &'static str)] = match map_id {
        // SeafoamIslands1F (Seafoam1HolesCoords 17,6 / 24,6)
        MapId::SeafoamIslands1F => &[
            (17, 6, "EVENT_SEAFOAM1_BOULDER1_DOWN_HOLE"),
            (24, 6, "EVENT_SEAFOAM1_BOULDER2_DOWN_HOLE"),
        ],
        // SeafoamIslandsB1F (Seafoam2HolesCoords 18,6 / 23,6)
        MapId::SeafoamIslandsB1F => &[
            (18, 6, "EVENT_SEAFOAM2_BOULDER1_DOWN_HOLE"),
            (23, 6, "EVENT_SEAFOAM2_BOULDER2_DOWN_HOLE"),
        ],
        // SeafoamIslandsB2F (Seafoam3HolesCoords 19,6 / 22,6)
        MapId::SeafoamIslandsB2F => &[
            (19, 6, "EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE"),
            (22, 6, "EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE"),
        ],
        // SeafoamIslandsB3F (Seafoam4HolesCoords 3,16 / 6,16)
        MapId::SeafoamIslandsB3F => &[
            (3, 16, "EVENT_SEAFOAM4_BOULDER1_DOWN_HOLE"),
            (6, 16, "EVENT_SEAFOAM4_BOULDER2_DOWN_HOLE"),
        ],
        _ => &[],
    };
    holes
        .iter()
        .find(|(hx, hy, _)| *hx == x && *hy == y)
        .map(|(_, _, flag)| *flag)
}

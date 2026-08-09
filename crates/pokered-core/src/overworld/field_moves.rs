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

use dotzuki_engine::overworld::types::TransportMode;
use dotzuki_engine::GameData;
use pokered_data::event_flags::EventFlag;
use pokered_data::items::ItemId;
use pokered_data::maps::MapId;
use pokered_data::moves::MoveId;
use pokered_data::tilesets::TilesetId;

use super::hm_effects::{self, BoulderPushResult, CutResult, FlashResult, FlyResult, StrengthResult, SurfResult};
use super::screen::{
    BedroomDialogue, OverworldAudioRequest, OverworldScreen, OverworldSfxEvent, PendingWarp,
    WarpFadeState, WARP_FADE_OUT_WHITE_FRAMES,
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
        let mon_name = mon.display_name();
        match move_id {
            MoveId::Cut => self.field_cut(obtained_badges, &mon_name),
            MoveId::Fly => self.field_fly(obtained_badges, &mon_name),
            MoveId::Surf => self.field_surf(obtained_badges, &mon_name),
            MoveId::Strength => self.field_strength(obtained_badges, mon),
            MoveId::Flash => self.field_flash(obtained_badges),
            MoveId::Dig => self.field_dig(last_blackout_map),
            MoveId::Teleport => self.field_teleport(&mon_name, last_blackout_map),
            MoveId::Softboiled => self.field_softboiled(mon),
            // Not a field move — the party menu never offers it.
            _ => self.field_message("This isn't the\ntime to use that!"),
        }
    }

    fn field_message(&mut self, text: &str) -> FieldMoveOutcome {
        self.pending_dialogue = Some(BedroomDialogue::from_message(text));
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
                let map = self.map_data.as_mut().expect("map_data present");
                map.set_block((fx / 2) as u8, (fy / 2) as u8, replacement_block);
                self.audio_requests
                    .push(OverworldAudioRequest::PlaySound {
                        sound_id: "SFX_CUT".to_string(),
                    });
                self.field_message(&format!("{} hacked\naway with CUT!", mon_name))
            }
            // Gen-1 grass cutting plays the same animation + text but does not
            // alter the map (grass blocks are not in CutTreeBlockSwaps).
            CutResult::CutGrass => {
                self.audio_requests
                    .push(OverworldAudioRequest::PlaySound {
                        sound_id: "SFX_CUT".to_string(),
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
                self.step_player_forward_onto_tile();
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
        self.step_player_forward_onto_tile();
        // PlayDefaultMusic: back to the map's own music.
        self.audio_requests
            .push(OverworldAudioRequest::PlayMapMusic { map: current_map });
        FieldMoveOutcome::Done
    }

    /// The Gen-1 `.makePlayerMoveForward` step: walk one tile ahead via the
    /// scripted-movement path (the tile was already validated by the caller).
    fn step_player_forward_onto_tile(&mut self) {
        let (dx, dy) = player_movement::direction_delta(self.state.player.facing);
        let tx = (self.state.player.x as i32 + dx as i32).max(0) as u16;
        let ty = (self.state.player.y as i32 + dy as i32).max(0) as u16;
        self.scripted_player_path.push_back((tx, ty));
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
                let name = mon.display_name();
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
    /// screen fades out (to white — `_LeaveMapAnim` ends in GBFadeOutToWhite)
    /// and the player lands at the map's fly point.
    pub fn fly_warp_to(&mut self, dest_map: MapId, dest_x: u8, dest_y: u8) {
        self.pending_warp = Some(PendingWarp {
            dest_map,
            dest_x,
            dest_y,
            save_last_map: false,
            // The fly picker sets BIT_FLY_WARP (town_map.asm:214) → the
            // arrival plays EnterMapAnim's spin-in.
            arrival_spin: true,
        });
        self.warp_fade_to_white = true;
        self.warp_fade_state = WarpFadeState::FadingOut {
            frames_remaining: WARP_FADE_OUT_WHITE_FRAMES,
        };
        self.sfx_event = OverworldSfxEvent::GoOutside;
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
        // The dust puff runs its own frame-stepped timeline (8 steps × 3
        // frames, `AnimateBoulderDust`) — independent of the lockout below.
        let dust_was_active = self.boulder_dust.is_active();
        self.boulder_dust.tick();
        // DoBoulderDustAnimation (engine/overworld/push_boulder.asm:89-103):
        // when the dust animation finishes, the original plays SFX_CUT once
        // (the boulder has reached its new tile); BIT_BOULDER_DUST is cleared
        // in the same routine, so the sound fires exactly once.
        if dust_was_active && !self.boulder_dust.is_active() {
            self.audio_requests
                .push(OverworldAudioRequest::PlaySound {
                    sound_id: "SFX_CUT".to_string(),
                });
        }
        // BIT_BOULDER_DUST: pushing is locked while the dust animation plays.
        if self.boulder_dust_frames > 0 {
            self.boulder_dust_frames -= 1;
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
                // MoveSprite: the boulder slides one tile in the push
                // direction, then the dust lockout (BIT_BOULDER_DUST).
                let (ddx, ddy) = player_movement::direction_delta(direction);
                let npc = &mut self.npc_states[npc_index];
                npc.x = (npc.x as i32 + ddx as i32).max(0) as u16;
                npc.y = (npc.y as i32 + ddy as i32).max(0) as u16;
                self.tried_push_boulder = false;
                self.boulder_dust_frames = BOULDER_DUST_FRAMES;
                // AnimateBoulderDust: the 2×2 smoke-puff block spawns at the
                // boulder's base (anchored to the player's tile — the same
                // spot the original computes the OAM block from).
                self.boulder_dust = presentation::BoulderDustState::new(
                    direction,
                    self.state.player.x,
                    self.state.player.y,
                );
                self.audio_requests
                    .push(OverworldAudioRequest::PlaySound {
                        sound_id: "SFX_PUSH_BOULDER".to_string(),
                    });
                // Seafoam Islands boulder-into-hole: pushing a boulder onto one
                // of the floor's hole tiles drops it through (the original hides
                // the object and sets the per-boulder DOWN_HOLE event; the lower
                // floor reveals its twin via those events).
                if let Some(flag_name) = seafoam_hole_flag_for(self.state.current_map, npc.x, npc.y)
                {
                    npc.visible = false;
                    if let Some(flag) =
                        pokered_data::event_flags::EventFlag::from_name(flag_name)
                    {
                        self.unified_flags.set(flag);
                    }
                }
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

    /// CheckForCollisionWhenPushingBoulder: the tile beyond the boulder at
    /// (`bx`, `by`) must be passable, free of sprites, not a stairs tile,
    /// and not an elevation change from the player's tile.
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
pub(crate) const BOULDER_DUST_FRAMES: u8 = 16;

/// Map a boulder's resting tile in the Seafoam Islands to the original
/// EVENT_SEAFOAM{n}_BOULDER{m}_DOWN_HOLE flag, if that tile is one of the
/// floor's holes (Seafoam{n}HolesCoords). When a Strength boulder is pushed
/// onto a hole it falls through to the floor below.
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

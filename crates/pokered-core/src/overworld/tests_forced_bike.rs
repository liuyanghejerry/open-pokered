//! Integration tests for the Cycling Road forced-bike lock (BIT_ALWAYS_ON_BIKE):
//! `ForcedBikeState` (forced_bike.rs) + the `OverworldScreen` map-entry hooks,
//! the BICYCLE item refusal and the SURF "Cycling is fun!" refusal.
//!
//! Gen-1 references: engine/overworld/player_state.asm `CheckForceBikeOrSurf`,
//! data/maps/force_bike_surf.asm, scripts/Route16Gate1F.asm /
//! scripts/Route18Gate1F.asm (`res BIT_ALWAYS_ON_BIKE`), engine/menus/
//! start_sub_menus.asm:374-379 (BICYCLE refusal) and engine/overworld/
//! field_move_messages.asm `IsSurfingAllowed` (SURF refusal).

use super::field_moves::FieldMoveOutcome;
use super::screen::{OverworldScreen, PendingWarp};
use super::{Direction, OverworldInput};
use dotzuki_engine::overworld::types::{MovementState, TransportMode};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;
use pokered_data::moves::MoveId;

const SOUL: u8 = 1 << 4; // BIT_SOULBADGE

fn screen_on(map: MapId) -> OverworldScreen<PokemonRedData> {
    OverworldScreen::new(map, None, PokemonRedData)
}

fn test_mon() -> crate::battle::state::Pokemon {
    crate::pokemon::stats::create_pokemon(pokered_data::species::Species::Squirtle, 5, [0xFF, 0xFF])
        .unwrap()
}

fn dialogue_text(screen: &OverworldScreen<PokemonRedData>) -> Option<String> {
    let dlg = screen.pending_dialogue.as_ref()?;
    let page = dlg.current()?;
    Some(format!("{}\n{}", page.line1, page.line2))
}

fn mount_on_road(screen: &mut OverworldScreen<PokemonRedData>, map: MapId, x: u16, y: u16) {
    screen.state.player.x = x;
    screen.state.player.y = y;
    screen.apply_map_entry_transport(map, x, y);
}

// ── Map-entry lifecycle (CheckForceBikeOrSurf) ─────────────────────────

#[test]
fn stepping_onto_the_road_mounts_the_bike() {
    for (map, x, y) in super::forced_bike::FORCED_BIKE_TILES {
        let mut screen = screen_on(MapId::PalletTown);
        mount_on_road(&mut screen, *map, *x as u16, *y as u16);
        assert!(
            screen.forced_bike.active,
            "{map:?} ({x},{y}) sets BIT_ALWAYS_ON_BIKE"
        );
        assert_eq!(
            screen.state.player.transport,
            TransportMode::Biking,
            "the player is riding on the road"
        );
    }
}

#[test]
fn off_road_tile_leaves_the_player_walking() {
    let mut screen = screen_on(MapId::PalletTown);
    mount_on_road(&mut screen, MapId::Route16, 24, 10);
    assert!(!screen.forced_bike.active);
    assert_eq!(screen.state.player.transport, TransportMode::Walking);
}

#[test]
fn the_lock_persists_across_the_whole_road() {
    let mut screen = screen_on(MapId::Route16);
    mount_on_road(&mut screen, MapId::Route16, 17, 10);
    assert!(screen.forced_bike.active);
    // Walking down into Route 17 keeps the lock (the asm `ret nz` while the
    // bit is set) — the player cannot get off anywhere on the road.
    mount_on_road(&mut screen, MapId::Route17, 5, 40);
    assert!(screen.forced_bike.active);
    assert_eq!(screen.state.player.transport, TransportMode::Biking);
    mount_on_road(&mut screen, MapId::Route18, 10, 4);
    assert!(screen.forced_bike.active);
}

#[test]
fn entering_a_gate_auto_dismounts() {
    let mut screen = screen_on(MapId::Route16);
    mount_on_road(&mut screen, MapId::Route16, 17, 10);
    assert_eq!(screen.state.player.transport, TransportMode::Biking);
    // Route16Gate1F_Script / Route18Gate1F_Script: `res BIT_ALWAYS_ON_BIKE`;
    // the GATE tileset is not bike-allowed, so walking is restored.
    mount_on_road(&mut screen, MapId::Route16Gate1F, 7, 8);
    assert!(!screen.forced_bike.active);
    assert_eq!(screen.state.player.transport, TransportMode::Walking);

    mount_on_road(&mut screen, MapId::Route18, 33, 8);
    assert!(screen.forced_bike.active);
    mount_on_road(&mut screen, MapId::Route18Gate1F, 0, 4);
    assert!(!screen.forced_bike.active);
    assert_eq!(screen.state.player.transport, TransportMode::Walking);
}

#[test]
fn warping_onto_the_road_mounts_the_bike() {
    let mut screen = screen_on(MapId::Route16Gate1F);
    screen.pending_warp = Some(PendingWarp {
        dest_map: MapId::Route16,
        dest_x: 17,
        dest_y: 10,
        save_last_map: false,
        arrival_spin: false,
    });
    screen.commit_pending_warp();
    assert!(screen.forced_bike.active);
    assert_eq!(screen.state.player.transport, TransportMode::Biking);
}

#[test]
fn warping_into_a_gate_dismounts_the_bike() {
    let mut screen = screen_on(MapId::Route16);
    mount_on_road(&mut screen, MapId::Route16, 17, 10);
    screen.pending_warp = Some(PendingWarp {
        dest_map: MapId::Route16Gate1F,
        dest_x: 7,
        dest_y: 8,
        save_last_map: false,
        arrival_spin: false,
    });
    screen.commit_pending_warp();
    assert!(!screen.forced_bike.active);
    assert_eq!(
        screen.state.player.transport,
        TransportMode::Walking,
        "warping off the road into the gate restores walking"
    );
}

#[test]
fn fly_warp_releases_the_forced_bike() {
    let mut screen = screen_on(MapId::Route16);
    mount_on_road(&mut screen, MapId::Route16, 17, 10);
    // FLY/DIG/TELEPORT/ESCAPE ROPE warps set arrival_spin — the asm's
    // HandleFlyWarpOrDungeonWarp resets the bit and the walk/bike state.
    screen.pending_warp = Some(PendingWarp {
        dest_map: MapId::PalletTown,
        dest_x: 5,
        dest_y: 6,
        save_last_map: false,
        arrival_spin: true,
    });
    screen.commit_pending_warp();
    assert!(!screen.forced_bike.active);
    assert_eq!(screen.state.player.transport, TransportMode::Walking);
}

#[test]
fn blackout_releases_the_forced_bike() {
    // DisplayPlayerBlackedOutText / battle core.asm:1160-1162 reset the bit —
    // the settlement writeback clears it when it queues the blackout warp.
    let mut screen = screen_on(MapId::Route16);
    mount_on_road(&mut screen, MapId::Route16, 17, 10);
    assert!(screen.forced_bike.active);

    let mut save = crate::save::SaveData::new();
    save.game_data.last_blackout_map = MapId::PalletTown as u8;
    let player = vec![test_mon()];
    let enemy = vec![test_mon()];
    let mut battle = crate::battle::BattleScreen::from_parties(
        false,
        &player,
        &enemy,
        Some(pokered_data::trainer_data::TrainerClass::Brock),
    );
    battle.map_id = MapId::PewterCity as u8; // not Oak's Lab
    battle.settlement = Some(crate::battle::settlement::BattleSettlement {
        outcome: crate::battle::settlement::BattleOutcome::Loss,
        money_gained: 0,
        money_lost: 50,
        payday_bonus: 0,
        exp_entries: vec![],
        level_ups: vec![],
        evolutions: vec![],
    });
    crate::battle::settlement::settle_battle_into_save(&mut battle, &mut save, &mut screen);

    assert!(
        !screen.forced_bike.active,
        "blackout clears BIT_ALWAYS_ON_BIKE"
    );
    assert_eq!(screen.state.player.transport, TransportMode::Walking);
    assert!(screen.pending_warp.is_some(), "the blackout warp is queued");
}

// ── Dismount / SURF refusals while locked ──────────────────────────────

#[test]
fn bicycle_item_cannot_get_off_while_forced() {
    let mut screen = screen_on(MapId::Route16);
    mount_on_road(&mut screen, MapId::Route16, 17, 10);
    assert_eq!(screen.state.player.transport, TransportMode::Biking);

    let consumed = screen.use_field_item(pokered_data::items::ItemId::Bicycle, MapId::PalletTown);
    assert!(!consumed, "BICYCLE is a key item");
    assert_eq!(
        screen.state.player.transport,
        TransportMode::Biking,
        "still riding — the forced lock refuses the get-off"
    );
    // CannotGetOffHereText (data/text/text_5.asm:70-73).
    assert_eq!(
        dialogue_text(&screen).as_deref(),
        Some("You can't get off\nhere.")
    );
}

#[test]
fn bicycle_item_works_after_leaving_the_road() {
    let mut screen = screen_on(MapId::Route16);
    mount_on_road(&mut screen, MapId::Route16, 17, 10);
    mount_on_road(&mut screen, MapId::Route16Gate1F, 7, 8);
    assert_eq!(screen.state.player.transport, TransportMode::Walking);
    screen.use_field_item(pokered_data::items::ItemId::Bicycle, MapId::PalletTown);
    assert_eq!(
        screen.state.player.transport,
        TransportMode::Biking,
        "a normal ride works once the lock is released"
    );
}

#[test]
fn surf_refused_with_cycling_is_fun_while_forced() {
    let mut screen = screen_on(MapId::Route16);
    mount_on_road(&mut screen, MapId::Route16, 17, 10);
    let mon = test_mon();
    let outcome = screen.use_field_move(
        MoveId::Surf,
        &mon,
        SOUL,
        MapId::PalletTown,
    );
    assert_eq!(outcome, FieldMoveOutcome::Done);
    // CyclingIsFunText (data/text/text_5.asm:28-31) — the SOUL badge check
    // passes, then IsSurfingAllowed sees BIT_ALWAYS_ON_BIKE.
    assert_eq!(
        dialogue_text(&screen).as_deref(),
        Some("Cycling is fun!\nForget SURFing!")
    );
    assert_eq!(
        screen.state.player.transport,
        TransportMode::Biking,
        "still on the bike"
    );
}

// ── Route 17 slope (JoypadOverworld PAD_DOWN + DoBikeSpeedup) ────────

/// Tick `update_frame` until the player's tile position changes; returns the
/// number of step-duration frames (the walk-initiation frame does not
/// decrement the counter, exactly like Gen-1's `TryWalking`).
fn frames_to_step(screen: &mut OverworldScreen<PokemonRedData>, input: &OverworldInput) -> u32 {
    let start = (screen.state.player.x, screen.state.player.y);
    let mut guard = 0;
    while screen.state.player.movement_state == MovementState::Idle {
        screen.update_frame(*input);
        guard += 1;
        assert!(guard < 60, "walk never started at {:?}", start);
    }
    let mut frames = 0;
    while (screen.state.player.x, screen.state.player.y) == start {
        screen.update_frame(*input);
        frames += 1;
        guard += 1;
        assert!(guard < 120, "player never moved from {:?}", start);
    }
    frames
}

#[test]
fn route17_slope_auto_walks_down_when_idle() {
    // JoypadOverworld (home/overworld.asm:1826-1835): on Route 17, with no
    // trainer battle and no d-pad/A/B held, a PAD_DOWN is simulated — the
    // slope drags the player downhill. The road block (4,4) is all 0x39 road
    // tiles, so (8,8) → (8,9) is a clean step.
    let mut screen = screen_on(MapId::Route17);
    screen.state.player.x = 8;
    screen.state.player.y = 8;
    let no_input = OverworldInput::new(false, false, false, false, false, false, false, false);
    let frames = frames_to_step(&mut screen, &no_input);
    assert_eq!(frames, 8, "forced-down walks at walking speed");
    assert_eq!(
        (screen.state.player.x, screen.state.player.y),
        (8, 9),
        "the slope forces DOWN"
    );
    assert_eq!(screen.state.player.facing, Direction::Down);
}

#[test]
fn route17_slope_held_button_cancels_the_auto_walk() {
    // The asm mask `PAD_CTRL_PAD | PAD_B | PAD_A` skips the simulation when
    // ANY direction or A/B is held.
    let mut screen = screen_on(MapId::Route17);
    screen.state.player.x = 8;
    screen.state.player.y = 8;
    let a_held = OverworldInput::new(false, false, false, false, true, false, false, false);
    for _ in 0..12 {
        screen.update_frame(a_held);
    }
    assert_eq!(
        (screen.state.player.x, screen.state.player.y),
        (8, 8),
        "holding A stops the slope auto-walk"
    );
}

#[test]
fn route17_slope_real_direction_wins_over_the_simulation() {
    let mut screen = screen_on(MapId::Route17);
    screen.state.player.x = 8;
    screen.state.player.y = 8;
    let up = OverworldInput::new(true, false, false, false, false, false, false, false);
    let frames = frames_to_step(&mut screen, &up);
    assert_eq!(frames, 8);
    assert_eq!(
        (screen.state.player.x, screen.state.player.y),
        (8, 7),
        "holding UP walks uphill instead of the forced DOWN"
    );
}

#[test]
fn route17_bike_speed_cancelled_while_going_uphill() {
    // DoBikeSpeedup (home/overworld.asm:377-388): on Route 17 the double
    // speed is skipped while UP/LEFT/RIGHT is held — the bike takes the
    // full walking duration (8 frames) per step.
    let mut screen = screen_on(MapId::Route17);
    screen.state.player.x = 8;
    screen.state.player.y = 12;
    screen.state.player.transport = TransportMode::Biking;
    let up = OverworldInput::new(true, false, false, false, false, false, false, false);
    let frames = frames_to_step(&mut screen, &up);
    assert_eq!(frames, 8, "bike at walking speed uphill on the slope");
    assert_eq!((screen.state.player.x, screen.state.player.y), (8, 11));
}

#[test]
fn route17_bike_double_speed_downhill() {
    // DOWN held (or idle): the speedup applies — 4 frames per step.
    let mut screen = screen_on(MapId::Route17);
    screen.state.player.x = 8;
    screen.state.player.y = 12;
    screen.state.player.transport = TransportMode::Biking;
    let down = OverworldInput::new(false, true, false, false, false, false, false, false);
    let frames = frames_to_step(&mut screen, &down);
    assert_eq!(frames, 4, "bike at double speed downhill on the slope");
    assert_eq!((screen.state.player.x, screen.state.player.y), (8, 13));
}

#[test]
fn off_slope_bike_keeps_double_speed() {
    // Off Route 17 the bike always steps at double speed — the Route 1 road
    // block (5,1) is 0x39 road tiles, so (10,2) is a clean step.
    let mut screen = screen_on(MapId::Route1);
    screen.state.player.x = 10;
    screen.state.player.y = 2;
    screen.state.player.transport = TransportMode::Biking;
    let up = OverworldInput::new(true, false, false, false, false, false, false, false);
    let frames = frames_to_step(&mut screen, &up);
    assert_eq!(frames, 4, "bike speedup stays on other maps");
    assert_eq!((screen.state.player.x, screen.state.player.y), (10, 1));
}

#[test]
fn off_slope_no_auto_walk() {
    // The PAD_DOWN simulation is Route-17-only: idling on Route 1 does not
    // move the player.
    let mut screen = screen_on(MapId::Route1);
    screen.state.player.x = 10;
    screen.state.player.y = 2;
    let no_input = OverworldInput::new(false, false, false, false, false, false, false, false);
    for _ in 0..12 {
        screen.update_frame(no_input);
    }
    assert_eq!(
        (screen.state.player.x, screen.state.player.y),
        (10, 2),
        "no forced movement off Route 17"
    );
}

// ── Seafoam Islands forced-SURF currents (wWalkBikeSurfState = 2) ──────

#[test]
fn seafoam_current_tiles_force_surfing() {
    // CheckForceBikeOrSurf (player_state.asm:57-82): entering a Seafoam
    // B3F/B4F strong-current tile forces the surf state — e.g. falling
    // through a floor hole into the water without having surfed.
    for (map, x, y) in super::forced_bike::FORCED_SURF_TILES {
        let mut screen = screen_on(MapId::PalletTown);
        mount_on_road(&mut screen, *map, *x as u16, *y as u16);
        assert_eq!(
            screen.state.player.transport,
            TransportMode::Surfing,
            "{map:?} ({x},{y}) forces surfing"
        );
        assert!(
            !screen.forced_bike.active,
            "{map:?} ({x},{y}) does not set the bike lock"
        );
    }
}

#[test]
fn seafoam_fall_warp_onto_current_forces_surfing() {
    // The B3F hole warps (DungeonWarpData) land the player on B4F (4,14)/
    // (5,14) — the same water the current sweeps. EnterMap → CheckForceBike-
    // OrSurf forces surf so the player is not walking on water.
    let mut screen = screen_on(MapId::SeafoamIslandsB3F);
    screen.pending_warp = Some(PendingWarp {
        dest_map: MapId::SeafoamIslandsB4F,
        dest_x: 4,
        dest_y: 14,
        save_last_map: false,
        arrival_spin: false,
    });
    screen.commit_pending_warp();
    assert_eq!(screen.state.player.transport, TransportMode::Surfing);
}

#[test]
fn surf_dismount_blocked_on_current_tiles() {
    // ItemUseSurfboard .tryToStopSurfing (item_effects.asm:689-715): the
    // dismount needs a passable land tile ahead with no water-pair collision.
    // The current tiles are water on every side — B4F (4,14) has water in
    // all four directions, B3F (18,7) faces water or the CAVERN $14→$05
    // pair — so the get-off is refused, exactly like the original.
    let mon = test_mon();
    for (map, x, y) in super::forced_bike::FORCED_SURF_TILES {
        let mut screen = screen_on(*map);
        screen.state.player.x = *x as u16;
        screen.state.player.y = *y as u16;
        screen.state.player.transport = TransportMode::Surfing;
        let outcome = screen.use_field_move(MoveId::Surf, &mon, SOUL, MapId::PalletTown);
        assert_eq!(outcome, FieldMoveOutcome::Done);
        assert_eq!(
            dialogue_text(&screen).as_deref(),
            Some("There's no place\nto get off!"),
            "{map:?} ({x},{y}) refuses the dismount"
        );
        assert_eq!(
            screen.state.player.transport,
            TransportMode::Surfing,
            "still surfing on the current tile"
        );
    }
}

#[test]
fn seafoam_current_releases_outside() {
    // The forced surf is a per-entry state (wWalkBikeSurfState), not a lock:
    // entering the current tile forces surf, but there is nothing to clear
    // afterwards — the transport follows the player's own actions once they
    // leave the tile/map.
    let mut screen = screen_on(MapId::SeafoamIslandsB4F);
    mount_on_road(&mut screen, MapId::SeafoamIslandsB4F, 4, 14);
    assert_eq!(screen.state.player.transport, TransportMode::Surfing);
    // Walking away from the current (e.g. after the sweep spits the player
    // out on land) or entering any other map leaves no forced state behind.
    assert_eq!(
        screen.forced_bike.enter_map(MapId::PalletTown, 5, 5),
        super::forced_bike::ForcedBikeMapEntry::Keep
    );
    assert!(!screen.forced_bike.active);
    // A normal entry elsewhere does not touch the transport.
    mount_on_road(&mut screen, MapId::Route1, 10, 2);
    assert_eq!(screen.state.player.transport, TransportMode::Surfing);
}

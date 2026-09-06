//! Regression tests: a walking NPC must never end up overlapping the player.
//!
//! Covers the residual overlap windows left after the current/destination
//! tile checks landed:
//! - the ledge-jump landing tile (two tiles from the jump origin) was
//!   unprotected, so a wander NPC could stroll into it mid-jump;
//! - autonomous (wander) movement kept rolling while map scripts /
//!   cutscenes owned the frame, letting NPCs reposition onto tiles a
//!   scripted player walk was about to enter (the original freezes all
//!   random NPC movement while a script runs).

use super::npc_movement::NpcRuntimeState;
use super::screen::OverworldScreen;
use super::{collision, Direction, MovementState, NpcMovementType, OverworldInput};
use dotzuki_engine::overworld::NpcWanderAxis;
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn neutral() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

/// The exact LCG run_npc_movement_tick derives its wander roll from.
fn rng_for_frame(frame_counter: u32) -> u8 {
    (frame_counter
        .wrapping_mul(1103515245)
        .wrapping_add(12345)
        >> 16) as u8
}

/// Pick a frame_counter whose NEXT update_frame rolls `want_dir`
/// (0=Down, 1=Up, 2=Left, 3=Right) for the NPC in slot `npc_index`.
fn frame_rolling_direction(npc_index: u8, want_dir: u8) -> u32 {
    (0..64u32)
        .find(|&fc| (rng_for_frame(fc + 1).wrapping_add(npc_index)) & 0x03 == want_dir)
        .expect("the LCG hits every direction within 64 frames")
}

/// PalletTown screen with the Fisher (slot 2, Wander/Any) relocated next to
/// the beach column at x=11, delay expired so its very next tick rolls.
fn screen_with_fisher_at(x: u16, y: u16) -> OverworldScreen {
    let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    let fisher = screen.npc_states.last_mut().unwrap();
    assert_eq!(fisher.npc_index, 2, "PalletTown slot 2 must be the Fisher");
    assert_eq!(fisher.movement_type, NpcMovementType::Wander);
    fisher.x = x;
    fisher.y = y;
    fisher.delay_counter = 0;
    fisher.walk_counter = 0;
    fisher.wander_axis = NpcWanderAxis::Any;
    screen
}

fn fisher(screen: &OverworldScreen) -> &NpcRuntimeState {
    screen.npc_states.last().unwrap()
}

/// Precondition: every tile the scenario relies on is walkable and warp-free.
fn assert_column_walkable(screen: &OverworldScreen, tiles: &[(u16, u16)]) {
    use dotzuki_engine::overworld::collision::CollisionProvider as _;
    let map = screen.map_data.as_ref().expect("map data loaded");
    let provider = collision::PokemonCollisionProvider::new(MapId::PalletTown, map.tileset);
    for &(x, y) in tiles {
        let tile = provider.get_tile_at_position(map.tileset, &map.blocks, map.width, x, y);
        assert!(
            pokered_data::collision::is_tile_passable(map.tileset, tile),
            "precondition: ({x},{y}) must be passable (tile {tile:#04x})"
        );
        assert!(
            collision::check_warp_at_position(x, y, map).is_none(),
            "precondition: ({x},{y}) must not be a warp"
        );
    }
}

/// Steering sanity: with the player far away and idle, the steered frame
/// really does start the Fisher walking LEFT (proves the rng steering and
/// that the target tile is enterable — the overlap tests are only
/// meaningful if the roll they gate would otherwise move).
#[test]
fn wander_roll_steering_moves_fisher_left() {
    let mut screen = screen_with_fisher_at(12, 14);
    assert_column_walkable(&screen, &[(11, 14), (12, 14)]);
    screen.state.player.x = 5;
    screen.state.player.y = 5;
    screen.frame_counter = frame_rolling_direction(2, 2); // Left
    screen.update_frame(neutral());

    let f = fisher(&screen);
    assert!(
        f.walk_counter > 0 && f.facing == Direction::Left,
        "steering sanity: Fisher should start walking left, at ({},{}), walking={}, facing={:?}",
        f.x,
        f.y,
        f.walk_counter,
        f.facing
    );
}

/// A ledge jump crosses TWO tiles; the landing tile must be treated as the
/// player's destination while airborne, or a wander NPC can walk into it
/// and the player lands on top of the NPC.
#[test]
fn wander_npc_cannot_step_onto_ledge_jump_landing() {
    let mut screen = screen_with_fisher_at(12, 14);
    assert_column_walkable(&screen, &[(11, 14)]);
    // Player mid-jump from (11,12) down over the ledge onto (11,14).
    screen.state.player.x = 11;
    screen.state.player.y = 12;
    screen.state.player.facing = Direction::Down;
    screen.state.player.movement_state = MovementState::Jumping;
    screen.state.walk_counter = 10;
    screen.frame_counter = frame_rolling_direction(2, 2); // Left → (11,14)
    screen.update_frame(neutral());

    let f = fisher(&screen);
    let entered_landing = (f.x == 11 && f.y == 14)
        || (f.walk_counter > 0 && f.facing == Direction::Left);
    assert!(
        !entered_landing,
        "NPC walked into the ledge-jump landing tile: at ({},{}), walking={}, facing={:?}",
        f.x,
        f.y,
        f.walk_counter,
        f.facing
    );
}

/// While a map script effect owns the frame, autonomous NPC movement is
/// frozen (the original's scripts run with random sprite updates halted);
/// the roll must not even start.
#[test]
fn wander_npc_frozen_while_script_effect_active() {
    let mut screen = screen_with_fisher_at(12, 14);
    screen.state.player.x = 8;
    screen.state.player.y = 12;
    screen.frame_counter = frame_rolling_direction(2, 2); // Left → (11,14)
    screen.active_script_effect = Some(super::script_bridge::ScriptEffect::Delay {
        frames: 30,
        frames_remaining: 30,
    });
    screen.update_frame(neutral());

    let f = fisher(&screen);
    assert!(
        f.x == 12 && f.y == 14 && f.walk_counter == 0,
        "wander NPC moved while a script effect was active: at ({},{}), walking={}",
        f.x,
        f.y,
        f.walk_counter
    );
}

/// A scripted player walk (cutscene movePlayer / spinner path) must not
/// end with the player standing on a wander NPC that strolled onto a
/// later waypoint while the walk was in progress.
#[test]
fn scripted_player_walk_never_overlaps_wander_npc() {
    let mut screen = screen_with_fisher_at(16, 15);
    assert_column_walkable(&screen, &[(17, 13), (17, 14), (17, 15)]);
    screen.state.player.x = 17;
    screen.state.player.y = 12;
    screen.state.player.facing = Direction::Down;
    screen.scripted_player_path.push_back((17, 13));
    screen.scripted_player_path.push_back((17, 14));
    screen.scripted_player_path.push_back((17, 15));
    // The very first tick rolls RIGHT, onto the FINAL waypoint (17,15) — a
    // tile neither the player's current position nor the first step's
    // destination covers.
    screen.frame_counter = frame_rolling_direction(2, 3);
    for _ in 0..80 {
        screen.update_frame(neutral());
        let f = fisher(&screen);
        assert!(
            !(f.x == screen.state.player.x && f.y == screen.state.player.y),
            "player ({},{}) overlapped by Fisher while walking the scripted path",
            screen.state.player.x,
            screen.state.player.y
        );
        if screen.scripted_player_path.is_empty()
            && screen.state.player.movement_state == MovementState::Idle
        {
            break;
        }
    }
    assert_eq!(
        (screen.state.player.x, screen.state.player.y),
        (17, 15),
        "scripted walk must complete"
    );
}

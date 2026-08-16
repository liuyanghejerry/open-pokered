use super::collision::*;
use super::player_movement::*;
use super::*;
use pokered_data::blockset_data;
use pokered_data::collision;
use pokered_data::maps::MapId;
use pokered_data::music::MusicId;

fn provider() -> PokemonCollisionProvider {
    PokemonCollisionProvider::new(MapId::PalletTown, TilesetId::Overworld)
}
use pokered_data::tilesets::TilesetId;

fn make_state() -> OverworldState {
    let mut s = OverworldState::new(MapId::PalletTown);
    s.player.x = 5;
    s.player.y = 5;
    s
}

fn make_map() -> MapData {
    MapData::new(
        MapId::PalletTown,
        10,
        9,
        TilesetId::Overworld,
        MusicId::PalletTown,
        vec![0; 90],
        vec![],
        vec![],
        vec![],
        MapConnections::default(),
    )
}

#[test]
fn test_try_move_passable() {
    let mut state = make_state();
    let result = try_move(
        &mut state,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x00,
        0x00,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, MoveResult::Walking);
    assert_eq!(state.player.movement_state, MovementState::Walking);
    assert_eq!(state.walk_counter, WALK_COUNTER_INIT);
    assert_eq!(state.player.facing, Direction::Down);
}

#[test]
fn test_try_move_blocked() {
    let mut state = make_state();
    let result = try_move(
        &mut state,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x00,
        0xFF,
        &[],
        0,
        &provider(),
    );
    assert!(matches!(
        result,
        MoveResult::Blocked(CollisionResult::TileBlocked)
    ));
    assert_eq!(state.player.movement_state, MovementState::Idle);
}

#[test]
fn test_try_move_turn_only() {
    let mut state = make_state();
    state.player.facing = Direction::Up;
    let result = try_move(
        &mut state,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x00,
        0xFF,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, MoveResult::TurnedOnly);
    assert_eq!(state.player.facing, Direction::Down);
}

#[test]
fn test_try_move_map_edge() {
    let mut state = make_state();
    state.player.x = 0;
    state.player.y = 0;
    let result = try_move(
        &mut state,
        Direction::Up,
        TilesetId::Overworld,
        10,
        9,
        0x00,
        0x00,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, MoveResult::ReachedMapEdge);
}

#[test]
fn test_try_move_ledge_jump() {
    let mut state = make_state();
    let result = try_move(
        &mut state,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x2C,
        0x37,
        &[],
        collision::PAD_DOWN,
        &provider(),
    );
    assert_eq!(result, MoveResult::LedgeJump);
    assert_eq!(state.player.movement_state, MovementState::Jumping);
    assert_eq!(state.walk_counter, WALK_COUNTER_INIT * 2);
}

#[test]
fn test_try_move_still_moving() {
    let mut state = make_state();
    state.player.movement_state = MovementState::Walking;
    let result = try_move(
        &mut state,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x00,
        0x00,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, MoveResult::StillMoving);
}

#[test]
fn test_advance_step_walking() {
    let mut state = make_state();
    state.player.movement_state = MovementState::Walking;
    state.player.facing = Direction::Down;
    state.walk_counter = WALK_COUNTER_INIT;

    for _ in 0..7 {
        assert!(!advance_step(&mut state));
    }
    assert!(advance_step(&mut state));
    assert_eq!(state.player.y, 6);
    assert_eq!(state.player.x, 5);
    assert_eq!(state.player.movement_state, MovementState::Idle);
}

#[test]
fn test_advance_step_biking_double_speed() {
    let mut state = make_state();
    state.player.movement_state = MovementState::Walking;
    state.player.facing = Direction::Right;
    state.player.transport = TransportMode::Biking;
    state.walk_counter = WALK_COUNTER_INIT;

    for _ in 0..3 {
        assert!(!advance_step(&mut state));
    }
    assert!(advance_step(&mut state));
    assert_eq!(state.player.x, 6);
    assert_eq!(state.player.movement_state, MovementState::Idle);
}

#[test]
fn test_advance_step_bike_speedup_suppressed() {
    // Gen-1 DoBikeSpeedup (home/overworld.asm:377-388): a game can cancel the
    // bike's double speed (the Route 17 slope cancels it while UP/LEFT/RIGHT
    // is held); the step then takes the walking duration (8 frames).
    let mut state = make_state();
    state.player.movement_state = MovementState::Walking;
    state.player.facing = Direction::Right;
    state.player.transport = TransportMode::Biking;
    state.player.bike_speedup_active = false;
    state.walk_counter = WALK_COUNTER_INIT;

    for _ in 0..7 {
        assert!(!advance_step(&mut state));
    }
    assert!(advance_step(&mut state));
    assert_eq!(state.player.x, 6);
    assert_eq!(state.player.movement_state, MovementState::Idle);
}

#[test]
fn test_advance_step_ledge_jump_moves_two_tiles() {
    let mut state = make_state();
    state.player.movement_state = MovementState::Jumping;
    state.player.facing = Direction::Down;
    state.walk_counter = WALK_COUNTER_INIT * 2;

    while !advance_step(&mut state) {}
    assert_eq!(state.player.y, 7);
    assert_eq!(state.player.movement_state, MovementState::Idle);
}

#[test]
fn test_advance_step_does_not_tick_repel() {
    // REPEL now ticks inside the game's encounter-check gate (the classic
    // TryDoWildEncounter placement), NOT per step — warp/script/cooldown
    // steps must not burn charges. advance_step leaves it untouched; the
    // explicit `tick_repel_step` helper does the decrement.
    let mut state = make_state();
    state.player.movement_state = MovementState::Walking;
    state.player.facing = Direction::Down;
    state.walk_counter = 1;
    state.repel_steps = 10;

    advance_step(&mut state);
    assert_eq!(state.repel_steps, 10, "advance_step does not tick repel");

    dotzuki_engine::overworld::player_movement::tick_repel_step(&mut state);
    assert_eq!(state.repel_steps, 9, "tick_repel_step does");
}

#[test]
fn test_advance_step_decrements_encounter_cooldown() {
    let mut state = make_state();
    state.player.movement_state = MovementState::Walking;
    state.player.facing = Direction::Down;
    state.walk_counter = 1;
    state.encounter_cooldown = 5;

    advance_step(&mut state);
    assert_eq!(state.encounter_cooldown, 4);
}

#[test]
fn test_advance_step_zero_counter_noop() {
    let mut state = make_state();
    state.walk_counter = 0;
    assert!(advance_step(&mut state));
    assert_eq!(state.player.x, 5);
    assert_eq!(state.player.y, 5);
}

#[test]
fn test_direction_delta() {
    assert_eq!(direction_delta(Direction::Down), (0, 1));
    assert_eq!(direction_delta(Direction::Up), (0, -1));
    assert_eq!(direction_delta(Direction::Left), (-1, 0));
    assert_eq!(direction_delta(Direction::Right), (1, 0));
}

#[test]
fn test_opposite_direction() {
    assert_eq!(opposite_direction(Direction::Down), Direction::Up);
    assert_eq!(opposite_direction(Direction::Up), Direction::Down);
    assert_eq!(opposite_direction(Direction::Left), Direction::Right);
    assert_eq!(opposite_direction(Direction::Right), Direction::Left);
}

#[test]
fn test_input_direction_priority() {
    let input = InputState {
        down: true,
        up: true,
        ..Default::default()
    };
    assert_eq!(input.direction_pressed(), Some(Direction::Down));

    let input2 = InputState {
        up: true,
        left: true,
        ..Default::default()
    };
    assert_eq!(input2.direction_pressed(), Some(Direction::Up));

    let input3 = InputState {
        left: true,
        right: true,
        ..Default::default()
    };
    assert_eq!(input3.direction_pressed(), Some(Direction::Left));
}

#[test]
fn test_input_no_direction() {
    let input = InputState::default();
    assert_eq!(input.direction_pressed(), None);
}

#[test]
fn test_input_to_pad_bits() {
    let input = InputState {
        down: true,
        ..Default::default()
    };
    assert_eq!(input.to_pad_bits(), collision::PAD_DOWN);

    let input2 = InputState {
        up: true,
        right: true,
        ..Default::default()
    };
    assert_eq!(
        input2.to_pad_bits(),
        collision::PAD_UP | collision::PAD_RIGHT
    );
}

#[test]
fn test_is_on_grass() {
    assert!(is_on_grass(0x52, TilesetId::Overworld));
    assert!(is_on_grass(0x20, TilesetId::Forest));
    assert!(!is_on_grass(0x00, TilesetId::Overworld));
    assert!(!is_on_grass(0x52, TilesetId::Mart));
}

#[test]
fn test_frames_per_step() {
    assert_eq!(frames_per_step(TransportMode::Walking), 8);
    assert_eq!(frames_per_step(TransportMode::Biking), 4);
    assert_eq!(frames_per_step(TransportMode::Surfing), 8);
}

#[test]
fn test_process_frame_no_input() {
    let mut state = make_state();
    let map = make_map();
    let input = InputState::default();
    let result = process_frame(&mut state, &input, &map, 0x00, 0x00, &[], &provider());
    assert_eq!(result, MoveResult::NoInput);
}

#[test]
fn test_process_frame_start_walk() {
    let mut state = make_state();
    let map = make_map();
    let input = InputState {
        down: true,
        ..Default::default()
    };
    let result = process_frame(&mut state, &input, &map, 0x00, 0x00, &[], &provider());
    assert_eq!(result, MoveResult::Walking);
    assert_eq!(state.player.movement_state, MovementState::Walking);
}

#[test]
fn test_process_frame_continue_walk() {
    let mut state = make_state();
    state.player.movement_state = MovementState::Walking;
    state.player.facing = Direction::Down;
    state.walk_counter = 5;
    let map = make_map();
    let input = InputState {
        down: true,
        ..Default::default()
    };
    let result = process_frame(&mut state, &input, &map, 0x00, 0x00, &[], &provider());
    assert_eq!(result, MoveResult::StillMoving);
    assert_eq!(state.walk_counter, 4);
}

#[test]
fn test_process_frame_warp_on_step_complete() {
    // Test two-phase warp: stepping onto a warp position on an indoor map
    // at the map edge with direction held should trigger the warp.
    // This simulates walking down to y=7 on a 4×4 block (8 tile) indoor map.
    let mut state = OverworldState::new(MapId::RedsHouse1F);
    state.player.x = 2;
    state.player.y = 6; // Will walk to y=7 (map edge for 4-block-tall map)
    state.player.movement_state = MovementState::Walking;
    state.player.facing = Direction::Down;
    state.walk_counter = 1;

    let mut map = MapData::new(
        MapId::RedsHouse1F,
        4,
        4,
        TilesetId::RedsHouse1,
        MusicId::PalletTown,
        vec![0x0F; 16], // All floor blocks
        vec![],
        vec![],
        vec![],
        MapConnections::default(),
    );
    map.warps.push(WarpPoint::new(2, 7, MapId::PalletTown, 0));
    // Direction held (Down) — player is at map edge facing out → ExtraWarpCheck function 1 passes
    let input = InputState {
        down: true,
        ..Default::default()
    };
    // The collision provider must match the map under test. The shared
    // `provider()` helper is built for PalletTown/Overworld, whose tileset sets
    // warp_front_check=true — that would route ExtraWarpCheck down the
    // warp-carpet-tile path instead of the FacingEdge path this indoor map
    // (RedsHouse1F) uses, and the warp would be missed.
    let house_provider = PokemonCollisionProvider::new(MapId::RedsHouse1F, TilesetId::RedsHouse1);
    let result = process_frame(&mut state, &input, &map, 0x00, 0x00, &[], &house_provider);
    assert_eq!(result, MoveResult::Warped { warp_index: 0 });
}

#[test]
fn test_process_frame_warp_tile_triggers_even_when_entered_sideways() {
    // Entering a warp tile from the side should still warp immediately.
    // This is important for stairs and similar warp tiles that are not always
    // approached from the map edge direction expected by ExtraWarpCheck.
    let mut state = OverworldState::new(MapId::RedsHouse1F);
    state.player.x = 0;
    state.player.y = 0;
    state.player.movement_state = MovementState::Walking;
    state.player.facing = Direction::Right;
    state.walk_counter = 1;

    // Find a block/quadrant whose collision tile is a RedsHouse warp tile.
    // get_tile_at_position reads indices [4, 6, 12, 14] for (sub_x, sub_y)=
    // (0,0), (1,0), (0,1), (1,1).
    let candidate_indices = [
        (4usize, 0u16, 0u16),
        (6usize, 1u16, 0u16),
        (12usize, 0u16, 1u16),
        (14usize, 1u16, 1u16),
    ];

    let (block_id, target_x, target_y) = (0u8..=u8::MAX)
        .find_map(|id| {
            let tiles = blockset_data::block_tiles(TilesetId::RedsHouse1, id)?;
            for (idx, x, y) in candidate_indices {
                if matches!(tiles[idx], 0x1A | 0x1C) {
                    return Some((id, x, y));
                }
            }
            None
        })
        .expect("expected a RedsHouse1 block with a warp tile in any collision quadrant");

    let map = MapData::new(
        MapId::RedsHouse1F,
        2,
        2,
        TilesetId::RedsHouse1,
        MusicId::PalletTown,
        vec![block_id; 4],
        vec![WarpPoint::new(target_x as u8, target_y as u8, MapId::PalletTown, 0)],
        vec![],
        vec![],
        MapConnections::default(),
    );

    let input = if target_x > 0 {
        state.player.x = target_x - 1;
        state.player.y = target_y;
        state.player.facing = Direction::Right;
        InputState {
            right: true,
            ..Default::default()
        }
    } else {
        state.player.x = target_x + 1;
        state.player.y = target_y;
        state.player.facing = Direction::Left;
        InputState {
            left: true,
            ..Default::default()
        }
    };
    let result = process_frame(&mut state, &input, &map, 0x00, 0x00, &[], &provider());
    assert_eq!(result, MoveResult::Warped { warp_index: 0 });
}

#[test]
fn test_full_walk_cycle() {
    let mut state = make_state();
    let map = make_map();
    let input_down = InputState {
        down: true,
        ..Default::default()
    };
    let no_input = InputState::default();

    // Start walking
    let r = process_frame(&mut state, &input_down, &map, 0x00, 0x00, &[], &provider());
    assert_eq!(r, MoveResult::Walking);

    // Advance 8 frames to complete walk (walk_counter 8→0)
    for _ in 0..8 {
        let r = process_frame(&mut state, &no_input, &map, 0x00, 0x00, &[], &provider());
        assert!(r == MoveResult::StillMoving || r == MoveResult::Walking);
    }

    // After 8 total frames, position should be updated
    assert_eq!(state.player.y, 6);
    assert_eq!(state.player.movement_state, MovementState::Idle);
}

#[test]
fn test_player_blocked_by_npc_destination() {
    let mut state = make_state();
    state.player.x = 5;
    state.player.y = 5;
    state.player.facing = Direction::Right;
    let map = make_map();

    let input_right = InputState {
        right: true,
        ..Default::default()
    };

    let npc_current = SpritePosition { x: 4, y: 5 };
    let npc_dest = SpritePosition { x: 6, y: 5 };

    let npc_positions_with_dest = vec![npc_current, npc_dest];

    let result = process_frame(
        &mut state,
        &input_right,
        &map,
        0x00,
        0x00,
        &npc_positions_with_dest,
        &provider(),
    );

    assert!(
        matches!(result, MoveResult::Blocked(CollisionResult::SpriteBlocked)),
        "Player should be blocked when walking toward tile that is an NPC's destination, got {:?}",
        result
    );
    assert_eq!(state.player.x, 5, "Player position should not change");
    assert_eq!(state.player.movement_state, MovementState::Idle);
}

#[test]
fn test_player_blocked_by_npc_current_position() {
    let mut state = make_state();
    state.player.x = 5;
    state.player.y = 5;
    state.player.facing = Direction::Right;
    let map = make_map();

    let input_right = InputState {
        right: true,
        ..Default::default()
    };

    let npc_pos = SpritePosition { x: 6, y: 5 };
    let npc_positions = vec![npc_pos];

    let result = process_frame(&mut state, &input_right, &map, 0x00, 0x00, &npc_positions, &provider());

    assert!(
        matches!(result, MoveResult::Blocked(CollisionResult::SpriteBlocked)),
        "Player should be blocked when walking toward tile occupied by NPC, got {:?}",
        result
    );
}

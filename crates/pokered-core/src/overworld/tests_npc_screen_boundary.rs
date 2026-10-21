//! Constructed foreground fixtures, separate from normal-boot replay evidence.
use super::*;
use dotzuki_engine::overworld::{CollisionProvider, NpcWanderAxis};
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};

fn field(direction: Direction, edge: bool) -> OverworldScreen {
    let mut field = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
    let (dx, dy, frame) = match direction {
        Direction::Down => (0, if edge { 4 } else { 3 }, 0),
        Direction::Up => (0, if edge { -4 } else { -3 }, 3),
        Direction::Left => (if edge { -4 } else { -3 }, 0, 2),
        Direction::Right => (if edge { 5 } else { 4 }, 0, 1),
    };
    let x = if direction == Direction::Up { 5 } else { 6 };
    field.state.player.x = (x - dx) as u16;
    let y = if direction == Direction::Up { 25 } else { 24 };
    field.state.player.y = (y - dy) as u16;
    field.frame_counter = frame;
    field.frame_counter = field.frame_counter.wrapping_sub(1);
    field.npc_states[1].visible = false;
    let npc = &mut field.npc_states[0];
    npc.x = x as u16;
    npc.y = y as u16;
    npc.walk_counter = 0;
    npc.delay_counter = 0;
    npc.step_direction = None;
    npc.scripted_path.clear();
    npc.wander_axis = NpcWanderAxis::Any;
    npc.movement_type = NpcMovementType::Wander;
    npc.visible = true;
    let (sx, sy) = player_movement::direction_delta(direction);
    let map = field.map_data.as_ref().unwrap();
    let provider = collision::PokemonCollisionProvider::new(MapId::Route1, map.tileset);
    let tile = provider.get_tile_at_position(
        map.tileset,
        &map.blocks,
        map.width,
        (x + i32::from(sx)) as u16,
        (y + i32::from(sy)) as u16,
    );
    assert!(
        provider.is_tile_passable(map.tileset, tile),
        "fixture destination {direction:?}"
    );
    field.publish_npc_frame(false);
    field
}

#[test]
fn ordinary_walkers_turn_and_wait_instead_of_starting_off_screen() {
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::Left,
        Direction::Right,
    ] {
        let mut field = field(direction, true);
        let origin = (field.npc_states[0].x, field.npc_states[0].y);
        field.update_frame(OverworldInput::none());
        let npc = &field.npc_states[0];
        assert_eq!(npc.facing, direction);
        assert_eq!((npc.x, npc.y), origin);
        assert_eq!(
            npc.walk_counter, 0,
            "{direction:?} must be rejected before animation"
        );
        assert_eq!(npc.step_direction, None);
        assert!(npc.delay_counter > 0, "rejected steps still start a wait");
        assert!(npc.visible);
        assert_eq!(npc.movement_type, NpcMovementType::Wander);
    }
}

#[test]
fn ordinary_walkers_one_tile_inside_each_edge_still_start() {
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::Left,
        Direction::Right,
    ] {
        let mut field = field(direction, false);
        field.update_frame(OverworldInput::none());
        let npc = &field.npc_states[0];
        assert_eq!(
            npc.walk_counter,
            npc_movement::NPC_WALK_FRAMES,
            "{direction:?}"
        );
        assert_eq!(npc.step_direction, Some(direction));
    }
}

#[test]
fn scripted_steps_bypass_the_ordinary_screen_boundary() {
    let mut field = field(Direction::Down, true);
    field.npc_states[0].scripted_path.push_back((6, 25));
    field.update_frame(OverworldInput::none());
    assert_eq!(
        field.npc_states[0].walk_counter,
        npc_movement::NPC_WALK_FRAMES
    );
    assert_eq!(field.npc_states[0].step_direction, Some(Direction::Down));
}

#[test]
fn existing_npc_stride_is_not_cancelled_at_the_boundary() {
    let mut field = field(Direction::Down, true);
    field.npc_states[0].walk_counter = 8;
    field.npc_states[0].step_direction = Some(Direction::Up);
    field.update_frame(OverworldInput::none());
    assert_eq!(field.npc_states[0].walk_counter, 7);
    assert_eq!(field.npc_states[0].step_direction, Some(Direction::Up));
}

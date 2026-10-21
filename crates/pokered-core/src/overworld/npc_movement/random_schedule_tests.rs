//! Controlled component inputs; these are not normal-boot gameplay recordings.
use super::*;
use crate::overworld::{collision::PokemonCollisionProvider, OverworldScreen};
use crate::save::sprite_data::SpriteData;
use dotzuki_engine::overworld::NpcWanderAxis;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};

fn walker(x: u16, y: u16) -> NpcRuntimeState {
    let mut n = load_map_npcs(pokered_data::npc_data::get_map_npcs(MapId::Route1))[0].clone();
    n.x = x;
    n.y = y;
    n.visible = true;
    n.movement_type = NpcMovementType::Wander;
    n.wander_axis = NpcWanderAxis::Any;
    n.range = 0;
    n.walk_counter = 0;
    n.delay_counter = 0;
    n.scripted_path.clear();
    n
}

fn counters() -> SpriteData {
    let mut saved = SpriteData::default();
    for row in &mut saved.second {
        row[2] = 8;
        row[3] = 8;
    }
    saved
}

fn tick(npcs: &mut [NpcRuntimeState], saved: &mut SpriteData, bytes: &[u8]) -> Vec<u8> {
    let field = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
    let map = field.map_data.as_ref().unwrap();
    let mut draws = Vec::new();
    update_npc_movement_with_rng(
        npcs,
        saved,
        6,
        24,
        None,
        map.width,
        map.height,
        &map.blocks,
        map.tileset,
        &PokemonCollisionProvider::new(MapId::Route1, map.tileset),
        || {
            let byte = bytes[draws.len()];
            draws.push(byte);
            byte
        },
        |_, n, event, random| match event {
            NpcMovementEvent::Finished => n.delay_counter = movement_delay(random()),
            NpcMovementEvent::Stationary => {
                advance_stationary_facing_with_rng(n, false, || (random(), random()))
            }
        },
    );
    draws
}

#[test]
fn successful_stride_draws_no_delay_until_its_final_pixel() {
    let mut npcs = vec![walker(5, 24)];
    let mut saved = counters();
    assert_eq!(tick(&mut npcs, &mut saved, &[0]), vec![0]);
    assert_eq!((npcs[0].walk_counter, npcs[0].delay_counter), (16, 0));
    for counter in (1..16).rev() {
        assert!(tick(&mut npcs, &mut saved, &[]).is_empty());
        assert_eq!((npcs[0].walk_counter, npcs[0].delay_counter), (counter, 0));
    }
    assert_eq!(tick(&mut npcs, &mut saved, &[146]), vec![146]);
    assert_eq!(
        (
            npcs[0].x,
            npcs[0].y,
            npcs[0].walk_counter,
            npcs[0].delay_counter
        ),
        (5, 25, 0, 18)
    );
    assert_eq!(
        saved.second[1][2], 9,
        "one displacement store for the whole stride"
    );
}

#[test]
fn failed_direction_draws_its_own_delay_and_zero_wraps() {
    for (byte, delay) in [(230, 102), (0, 256), (128, 256), (255, 127)] {
        let mut npcs = vec![walker(5, 24)];
        let mut saved = counters();
        // Right attempts the occupied player tile: direction 192 != delay byte.
        assert_eq!(tick(&mut npcs, &mut saved, &[192, byte]), vec![192, byte]);
        assert_eq!((npcs[0].walk_counter, npcs[0].delay_counter), (0, delay));
        assert_eq!(npcs[0].facing, Direction::Right);
        assert_eq!(npcs[0].step_direction, None);
        assert_eq!(&saved.second[1][2..4], &[8, 8]);
    }
}

#[test]
fn displacement_rejection_consumes_delay_before_the_next_slot_direction() {
    let mut npcs = vec![walker(5, 24), walker(7, 24)];
    let mut saved = counters();
    saved.second[1][2] = 0;
    assert_eq!(tick(&mut npcs, &mut saved, &[0, 230, 0]), vec![0, 230, 0]);
    assert_eq!((npcs[0].walk_counter, npcs[0].delay_counter), (0, 102));
    assert_eq!((npcs[1].walk_counter, npcs[1].delay_counter), (16, 0));
    assert_eq!(saved.second[1][2], 0);
    assert_eq!(saved.second[2][2], 9);
}

#[test]
fn screen_edge_rejection_draws_delay_inside_the_actor_attempt() {
    let mut npcs = vec![walker(5, 28), walker(7, 24)];
    let mut saved = counters();
    assert_eq!(tick(&mut npcs, &mut saved, &[0, 230, 0]), vec![0, 230, 0]);
    assert_eq!((npcs[0].walk_counter, npcs[0].delay_counter), (0, 102));
    assert_eq!(npcs[0].step_direction, None);
    assert_eq!(saved.second[1][2], 8);
    assert_eq!((npcs[1].walk_counter, npcs[1].delay_counter), (16, 0));
}

#[test]
fn completions_stationary_turns_and_ready_walkers_share_one_slot_order_stream() {
    let mut npcs = vec![walker(5, 24), walker(6, 23), walker(7, 24)];
    npcs[0].walk_counter = 1;
    npcs[0].step_direction = Some(Direction::Down);
    npcs[0].facing = Direction::Up;
    npcs[1].movement_type = NpcMovementType::Stationary;
    let mut saved = counters();
    assert_eq!(
        tick(&mut npcs, &mut saved, &[146, 192, 230, 0]),
        vec![146, 192, 230, 0]
    );
    assert_eq!(
        (npcs[0].y, npcs[0].delay_counter, npcs[0].facing),
        (25, 18, Direction::Up)
    );
    assert_eq!(
        (npcs[1].facing, npcs[1].delay_counter),
        (Direction::Right, 102)
    );
    assert_eq!((npcs[2].walk_counter, npcs[2].delay_counter), (16, 0));
}

#[test]
fn scripted_start_and_finish_never_consume_ordinary_movement_randomness() {
    let mut npcs = vec![walker(5, 24)];
    npcs[0].scripted_path.push_back((5, 25));
    let mut saved = counters();
    for _ in 0..17 {
        assert!(tick(&mut npcs, &mut saved, &[]).is_empty());
    }
    assert_eq!(
        (
            npcs[0].x,
            npcs[0].y,
            npcs[0].walk_counter,
            npcs[0].delay_counter
        ),
        (5, 25, 0, 0)
    );
    assert_eq!(&saved.second[1][2..4], &[8, 8]);
}

#[test]
fn hidden_offscreen_delayed_and_inflight_actors_do_not_draw_directions() {
    let mut npcs = vec![walker(5, 24), walker(5, 24), walker(5, 24), walker(5, 24)];
    npcs[0].visible = false;
    npcs[1].x = 0;
    npcs[2].delay_counter = 3;
    npcs[3].walk_counter = 8;
    npcs[3].step_direction = Some(Direction::Down);
    assert!(tick(&mut npcs, &mut counters(), &[]).is_empty());
    assert!(!npcs[0].visible);
    assert!(npcs[1].visible);
    assert_eq!(npcs[2].delay_counter, 2);
    assert_eq!(npcs[3].walk_counter, 7);
}

#[test]
fn forced_direction_still_consumes_a_direction_draw() {
    for (selector, direction, byte) in [
        (0xD0, Direction::Down, 192),
        (0xD1, Direction::Up, 0),
        (0xD2, Direction::Left, 192),
        (0xD3, Direction::Right, 0),
    ] {
        let mut npcs = vec![walker(5, 24)];
        npcs[0].range = selector;
        let mut hooks = PokeredMovementHooks {
            saved: &mut counters(),
            player: (6, 24),
            random: || byte,
            event: |_: usize,
                    _: &mut NpcRuntimeState,
                    _: NpcMovementEvent,
                    _: &mut dyn FnMut() -> u8| {},
        };
        assert_eq!(
            dotzuki_engine::overworld::NpcMovementHooks::choose_direction(&mut hooks, 0, &npcs[0]),
            Some(direction)
        );
        assert_eq!(
            tick(&mut npcs, &mut counters(), &[byte, 19]).first(),
            Some(&byte)
        );
    }
}

#[test]
fn live_foreground_uses_the_seeded_stream_at_direction_and_completion_only() {
    use rand::Rng;
    let mut field = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
    field.state.player.x = 6;
    field.state.player.y = 24;
    field.npc_states[1].visible = false;
    field.npc_states[0] = walker(5, 24);
    field.npc_states[0].range = 0xD0;
    field.set_rng_seed(42);
    let mut expected = field.rng.clone();
    let _: u8 = expected.gen();
    field.update_frame(crate::overworld::OverworldInput::none());
    assert_eq!(
        (
            field.npc_states[0].walk_counter,
            field.npc_states[0].delay_counter
        ),
        (16, 0)
    );
    assert_eq!(
        field.rng.clone().gen::<u64>(),
        expected.clone().gen::<u64>()
    );
    // Ordinary field movement runs once per two rendered frames.
    for _ in 0..32 {
        if field.npc_states[0].walk_counter == 1 {
            break;
        }
        field.update_frame(crate::overworld::OverworldInput::none());
        assert_eq!(
            field.rng.clone().gen::<u64>(),
            expected.clone().gen::<u64>()
        );
    }
    assert_eq!(field.npc_states[0].walk_counter, 1);
    assert_eq!(
        field.rng.clone().gen::<u64>(),
        expected.clone().gen::<u64>()
    );
    let delay = movement_delay(expected.gen());
    for _ in 0..2 {
        if field.npc_states[0].walk_counter == 0 {
            break;
        }
        field.update_frame(crate::overworld::OverworldInput::none());
    }
    assert_eq!(
        (field.npc_states[0].y, field.npc_states[0].delay_counter),
        (25, delay)
    );
    assert_eq!(field.rng.gen::<u64>(), expected.gen::<u64>());
}

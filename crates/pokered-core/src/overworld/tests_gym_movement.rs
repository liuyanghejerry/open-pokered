//! Held input must yield to door gates and arrow-tile movement.
use super::screen::OverworldScreen;
use super::{Direction, MovementState, OverworldInput};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn input(dir: Direction, a: bool) -> OverworldInput {
    OverworldInput::new(
        dir == Direction::Up,
        dir == Direction::Down,
        dir == Direction::Left,
        dir == Direction::Right,
        a,
        false,
        false,
        false,
    )
}

#[test]
fn viridian_gym_locked_while_holding_up_and_dismissing_dialogue() {
    let mut screen = OverworldScreen::new(MapId::ViridianCity, None, PokemonRedData);
    screen.state.player.x = 32;
    screen.state.player.y = 9;
    screen.state.player.facing = Direction::Up;
    for frame in 0..1200 {
        screen.update_frame(input(Direction::Up, frame % 20 == 0));
        assert_eq!(
            screen.state.current_map,
            MapId::ViridianCity,
            "entered locked gym at frame {frame}"
        );
        assert!(
            screen.pending_warp.is_none(),
            "locked door queued warp at frame {frame}"
        );
        assert!(
            screen.state.player.y >= 8,
            "walked through locked entrance at frame {frame}"
        );
    }
}

#[test]
fn spinner_takes_control_before_opposing_held_input() {
    for map in [
        MapId::ViridianGym,
        MapId::RocketHideoutB2F,
        MapId::RocketHideoutB3F,
    ] {
        let key = super::script_bridge::map_id_to_script_key(map);
        for &(x, y, steps) in super::spinner_paths::spinner_paths(&key) {
            let mut screen = OverworldScreen::new(map, None, PokemonRedData);
            screen.npc_states.clear();
            screen.state.player.x = x as u16;
            screen.state.player.y = y as u16;
            let opposite = super::player_movement::opposite_direction(steps[0].dir);
            screen.state.player.facing = opposite;
            screen.update_frame(input(opposite, false));
            assert!(
                !screen.scripted_player_path.is_empty(),
                "{map:?} ({x},{y}) failed to start"
            );
            assert_eq!(
                screen.state.player.facing, steps[0].dir,
                "{map:?} ({x},{y}) accepted opposing input"
            );
            for _ in 0..1000 {
                screen.update_frame(input(opposite, false));
                if screen.scripted_player_path.is_empty()
                    && screen.state.player.movement_state == MovementState::Idle
                {
                    break;
                }
            }
            let expected = steps.iter().fold((x as i32, y as i32), |(x, y), step| {
                let (dx, dy) = super::player_movement::direction_delta(step.dir);
                (
                    x + dx as i32 * step.steps as i32,
                    y + dy as i32 * step.steps as i32,
                )
            });
            assert_eq!(
                (screen.state.player.x as i32, screen.state.player.y as i32),
                expected,
                "{map:?} ({x},{y}) path endpoint"
            );
        }
    }
}

#[test]
fn walking_onto_spinner_with_held_input_starts_forced_path() {
    let mut screen = OverworldScreen::new(MapId::ViridianGym, None, PokemonRedData);
    screen.npc_states.clear();
    screen.state.player.x = 19;
    screen.state.player.y = 12;
    screen.state.player.facing = Direction::Up;
    for frame in 0..80 {
        screen.update_frame(input(Direction::Up, false));
        if !screen.scripted_player_path.is_empty() {
            return;
        }
        assert!(
            screen.state.player.y >= 11,
            "walked off arrow without activating it at frame {frame}"
        );
    }
    panic!("held input bypassed spinner");
}

#[test]
fn viridian_gym_gate_cannot_be_bypassed_from_adjacent_tiles() {
    for (x, y, direction) in [
        (31, 8, Direction::Right),
        (33, 8, Direction::Left),
        (31, 7, Direction::Right),
        (33, 7, Direction::Left),
        (32, 6, Direction::Down),
    ] {
        let mut screen = OverworldScreen::new(MapId::ViridianCity, None, PokemonRedData);
        screen.state.player.x = x;
        screen.state.player.y = y;
        screen.state.player.facing = direction;
        for frame in 0..300 {
            let direction = if screen.state.player.x == 32 && screen.state.player.y == 8 {
                Direction::Up
            } else {
                direction
            };
            screen.update_frame(input(direction, frame % 20 == 0));
            assert_eq!(
                screen.state.current_map,
                MapId::ViridianCity,
                "bypassed from ({x},{y})"
            );
            assert!(
                screen.pending_warp.is_none(),
                "warp from ({x},{y}), frame {frame}"
            );
        }
    }
}

#[test]
fn viridian_gym_opens_with_seven_badges_and_stays_open_with_eight() {
    for badges in [0x7f, 0xff] {
        let mut screen = OverworldScreen::new(MapId::ViridianCity, None, PokemonRedData);
        screen
            .script_engine
            .seed_number("obtainedBadges", badges as f64);
        screen.state.player.x = 32;
        screen.state.player.y = 9;
        screen.state.player.facing = Direction::Up;
        for _ in 0..120 {
            screen.update_frame(input(Direction::Up, false));
            if screen.state.current_map == MapId::ViridianGym {
                break;
            }
        }
        assert_eq!(
            screen.state.current_map,
            MapId::ViridianGym,
            "badges {badges:#x}"
        );
    }
}

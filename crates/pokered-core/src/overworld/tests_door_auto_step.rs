use super::screen::{OverworldScreen, PendingWarp};
use super::{Direction, MovementState};
use dotzuki_engine::overworld::OverworldInput;
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn neutral() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn arrive(map: MapId, x: u8, y: u8) -> OverworldScreen<PokemonRedData> {
    let mut screen = OverworldScreen::new(map, None, PokemonRedData);
    screen.pending_warp = Some(PendingWarp {
        dest_map: map,
        dest_x: x,
        dest_y: y,
        save_last_map: false,
        arrival_spin: false,
    });
    screen.commit_pending_warp();
    assert!(screen.state.standing_on_door);
    screen
}

#[test]
fn door_auto_step_stops_above_mansion_walls_and_returns_control() {
    for map in [
        MapId::CeladonMansion1F,
        MapId::CeladonMansion2F,
        MapId::CeladonMansion3F,
    ] {
        let mut screen = arrive(map, 2, 1);
        for _ in 0..32 {
            screen.update_frame(neutral());
        }
        assert_eq!(
            (screen.state.player.x, screen.state.player.y),
            (2, 1),
            "{map:?}"
        );
        assert_eq!(screen.state.player.facing, Direction::Down);
        assert_eq!(screen.state.player.movement_state, MovementState::Idle);
        assert_eq!(screen.state.walk_counter, 0);
        assert!(!screen.state.standing_on_door && !screen.state.exiting_door);

        // Control returns after the blocked automatic DOWN. Enter the open
        // square to the right with real directional input, then release it.
        for _ in 0..32 {
            screen.update_frame(OverworldInput::new(
                false, false, false, true, false, false, false, false,
            ));
            if screen.state.player.x == 3 {
                break;
            }
        }
        for _ in 0..16 {
            screen.update_frame(neutral());
        }
        assert_eq!(
            (screen.state.player.x, screen.state.player.y),
            (3, 1),
            "{map:?}"
        );
        assert_eq!(screen.state.player.movement_state, MovementState::Idle);
    }
}

#[test]
fn door_auto_step_checks_npcs_and_preserves_the_clear_exit_cadence() {
    for blocked in [false, true] {
        let mut screen = arrive(MapId::SilphCo5F, 26, 0);
        for npc in &mut screen.npc_states {
            npc.visible = false;
        }
        if blocked {
            screen.npc_states[0].visible = true;
            screen.npc_states[0].x = 26;
            screen.npc_states[0].y = 1;
            screen.npc_states[0].walk_counter = 0;
        }
        screen.update_frame(neutral());
        assert_eq!(screen.state.exiting_door, !blocked);
        assert_eq!(
            screen.state.walk_counter,
            if blocked {
                0
            } else {
                super::player_movement::WALK_COUNTER_INIT - 1
            }
        );
        // Removing a blocker after the input was consumed must not queue a
        // delayed automatic step. The original simulates DOWN only once.
        if blocked {
            screen.npc_states[0].visible = false;
        }
        for _ in 0..32 {
            screen.update_frame(neutral());
        }
        assert_eq!(
            (screen.state.player.x, screen.state.player.y),
            (26, if blocked { 0 } else { 1 })
        );
        assert!(!screen.state.standing_on_door && !screen.state.exiting_door);
    }
}

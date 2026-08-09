//! End-to-end elevator ride test: drive the real CeladonMartElevator scene
//! (embedded script) — press A on the panel, choose a floor via
//! `resume_script_after_elevator`, let the warp fade complete — and assert
//! the player lands on the ELEVATOR tile of the destination floor (1,1), not
//! on the stairs (12,1). The arrival shake (`ElevatorShakeState`) then plays
//! with the player inside the elevator.
//!
//! Regression for the off-by-one in the elevator scenes' `warpTo` targets:
//! they used the original disassembly's 1-based warp numbers resolved to
//! coordinates, but `map.json` warp lists (and the engine's `elevator_data`
//! warp ids) are 0-based — so the player was warped onto the stairs tile and
//! shook there instead of in the elevator.

use pokered_core::data::impl_traits::PokemonRedData;
use pokered_core::overworld::screen::WarpFadeState;
use pokered_core::overworld::{Direction, OverworldInput, OverworldScreen};
use pokered_data::maps::MapId;

fn neutral() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

/// A-button press — `new(up, down, left, right, a, b, start, select)`.
fn a_press() -> OverworldInput {
    OverworldInput::new(false, false, false, false, true, false, false, false)
}

#[test]
fn celadon_elevator_to_1f_lands_on_elevator_tile() {
    let mut screen = OverworldScreen::new(MapId::CeladonMartElevator, None, PokemonRedData);
    screen.state.player.x = 3;
    screen.state.player.y = 1;
    screen.state.player.facing = Direction::Up;
    screen.run_on_load();
    for _ in 0..3 {
        screen.update_frame(neutral());
    }

    // Open the panel: alternate press/release so the "Which floor?" dialogue
    // page advances, then `elevatorMenu` suspends the script.
    let mut opened = false;
    for i in 0..240 {
        screen.update_frame(if i % 2 == 0 { a_press() } else { neutral() });
        if screen.script_awaiting_elevator {
            opened = true;
            break;
        }
    }
    assert!(
        opened,
        "script should suspend awaiting the elevator floor choice"
    );
    assert!(screen.pending_elevator.is_some(), "floor list handed to the app");

    // Choose floor 0 (1F): the script resumes and queues `warpTo("CeladonMart1F", 1, 1)`.
    screen.resume_script_after_elevator(0);
    for _ in 0..10 {
        screen.update_frame(neutral());
    }
    assert!(
        screen.pending_warp.is_some() || !matches!(screen.warp_fade_state, WarpFadeState::Idle),
        "warp fade should be in progress"
    );

    // Run until the warp fade completes and the arrival shake has finished.
    let mut frames = 0;
    while frames < 600 {
        screen.update_frame(neutral());
        frames += 1;
        if screen.state.current_map == MapId::CeladonMart1F
            && matches!(screen.warp_fade_state, WarpFadeState::Idle)
            && screen.elevator_shake.is_none()
        {
            break;
        }
    }
    assert_eq!(screen.state.current_map, MapId::CeladonMart1F, "arrived on 1F");
    // The elevator tile on every Celadon Mart floor (map.json warp index 5 on
    // 1F / index 2 on 2F-5F, 0-based) — NOT the stairs at (12,1)/(16,1).
    assert_eq!(screen.state.player.x, 1, "player X should be the elevator tile");
    assert_eq!(screen.state.player.y, 1, "player Y should be the elevator tile");
}

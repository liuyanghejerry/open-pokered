//! End-to-end wiring test for the S.S. Anne departure cutscene: the
//! VermilionDock @load scene (scripts/VermilionDock.asm
//! VermilionDockSSAnneLeavesScript) sets EVENT_SS_ANNE_LEFT, swaps the
//! music, runs the blocking ship-sail animation (`playShipDeparture()` →
//! `ShipDepartureState`), applies the ship erase + warp removal
//! (VermilionDock_EraseSSAnne + wNumberOfWarps--), and then drives the
//! forced walk-out north off the dock.

use pokered_core::overworld::presentation::{
    ShipDeparturePhase, SHIP_DEPARTURE_TOTAL_FRAMES,
};
use pokered_core::overworld::{OverworldInput, OverworldScreen};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn idle() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

#[test]
fn ss_anne_departure_plays_once_and_erases_the_ship() {
    let mut screen = OverworldScreen::new(MapId::VermilionDock, None, PokemonRedData);
    // The departure gates on having HM01 (from the SS Anne captain) and
    // having arrived from the ship.
    screen.set_flag_live("EVENT_GOT_HM01", true);
    screen.state.player.x = 14;
    screen.state.player.y = 2;
    screen.state.player.facing = pokered_core::overworld::Direction::Up;

    screen.run_on_load();

    // The scene sets the flag FIRST (SetEventForceReuseHL), then starts
    // the animation.
    let mut departure_started = false;
    let mut flag_seen_early = false;
    let mut saw_horn = false;
    for _ in 0..SHIP_DEPARTURE_TOTAL_FRAMES + 64 {
        screen.update_frame(idle());
        if screen.ship_departure.is_some() {
            if !departure_started {
                departure_started = true;
                // EVENT_SS_ANNE_LEFT is set before the animation starts.
                flag_seen_early = screen
                    .script_flags()
                    .get("EVENT_SS_ANNE_LEFT")
                    .copied()
                    .unwrap_or(false);
            }
            if screen.ship_departure.as_ref().unwrap().phase()
                == ShipDeparturePhase::Scroll
            {
                saw_horn = true;
            }
        }
        if screen.pending_warp.is_some() {
            break;
        }
    }
    assert!(departure_started, "the scene must start the departure");
    assert!(
        flag_seen_early,
        "EVENT_SS_ANNE_LEFT is set before the animation (plays-once gate)"
    );
    assert!(
        saw_horn,
        "the animation must reach the scroll phase (horn SFX fires)"
    );
    assert!(
        screen.ship_departure.is_none(),
        "the animation must complete"
    );

    // Erase: the ship's blocks (bottom block row + porthole strip) are
    // water now, and the dock→ship warp (14,2) is gone (wNumberOfWarps--).
    let map = screen.map_data.as_ref().expect("map data");
    for bx in 0..=12u8 {
        assert_eq!(
            map.blocks[map.width as usize * 5 + bx as usize],
            0x0d,
            "block ({bx}, 5) must be the water block after the departure"
        );
    }
    for bx in 5..=8u8 {
        assert_eq!(
            map.blocks[map.width as usize * 2 + bx as usize],
            0x0d,
            "block ({bx}, 2) (porthole strip) must be water after the departure"
        );
    }
    assert!(
        !map.warps.iter().any(|w| w.x == 14 && w.y == 2),
        "the dock→ship warp must be removed"
    );

    // The cutscene then drives the walk-out: the player leaves north and
    // the walk-out flags are set.
    let mut walked_out = false;
    for _ in 0..400 {
        screen.update_frame(idle());
        if screen
            .script_flags()
            .get("EVENT_WALKED_OUT_OF_DOCK")
            .copied()
            .unwrap_or(false)
        {
            walked_out = true;
            break;
        }
    }
    assert!(walked_out, "the walk-out must run after the departure");
}

#[test]
fn ss_anne_departure_does_not_replay() {
    let mut screen = OverworldScreen::new(MapId::VermilionDock, None, PokemonRedData);
    screen.set_flag_live("EVENT_GOT_HM01", true);
    screen.set_flag_live("EVENT_SS_ANNE_LEFT", true);
    screen.state.player.x = 14;
    screen.state.player.y = 2;
    screen.run_on_load();
    // The gate `!getFlag(EVENT_SS_ANNE_LEFT)` skips the departure entirely.
    for _ in 0..60 {
        screen.update_frame(idle());
    }
    assert!(
        screen.ship_departure.is_none(),
        "the departure must not replay once EVENT_SS_ANNE_LEFT is set"
    );
}

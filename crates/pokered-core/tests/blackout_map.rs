//! End-to-end wiring test for `wLastBlackoutMap`: a script-driven heal (the
//! Pokémon Center nurse saying "Yes") must enqueue a `SetBlackoutMap`
//! request carrying the map the player entered the center from —
//! `SetLastBlackoutMap` (engine/events/set_blackout_map.asm), called from
//! `DisplayPokemonCenterDialogue_` (engine/events/pokecenter.asm:17).

use pokered_core::overworld::screen::OverworldGameDataRequest;
use pokered_core::overworld::{Direction, OverworldInput, OverworldScreen};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn none() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn press_a() -> OverworldInput {
    OverworldInput::new(false, false, false, false, true, false, false, false)
}

/// Talk to the Viridian nurse and accept healing; return the game-data
/// requests enqueued along the way.
fn heal_at_viridian_nurse() -> Vec<OverworldGameDataRequest> {
    let mut screen = OverworldScreen::new(MapId::ViridianPokecenter, None, PokemonRedData);
    // Simulate walking in from Viridian City (the warp system records the
    // outdoor map into `last_map` on entry).
    screen.last_map = Some(MapId::ViridianCity);
    // Nurse stands at (3,1); talk to her from below.
    screen.state.player.x = 3;
    screen.state.player.y = 2;
    screen.state.player.facing = Direction::Up;

    for i in 0..600 {
        // Advance dialogue and accept the default YES choice; idle in
        // between so text/delays can run.
        let input = if i % 6 == 0 { press_a() } else { none() };
        screen.update_frame(input);
        if screen
            .game_data_requests
            .iter()
            .any(|r| matches!(r, OverworldGameDataRequest::SetBlackoutMap { .. }))
        {
            break;
        }
    }
    std::mem::take(&mut screen.game_data_requests)
}

#[test]
fn nurse_heal_records_blackout_map() {
    let requests = heal_at_viridian_nurse();
    assert!(
        requests.contains(&OverworldGameDataRequest::SetBlackoutMap {
            map: MapId::ViridianCity
        }),
        "nurse heal must record wLastMap as the blackout target; got {requests:?}"
    );
}

//! End-to-end wiring test for the Hall of Fame ceremony: entering the
//! HallOfFame map runs its @load cutscene (OAK's congratulations), which
//! ends in `game.enterHallOfFame()` — the app-facing `pending_hof_ceremony`
//! flag that starts the roll-call movie + credits takeover
//! (scripts/HallOfFame.asm HallOfFameResetEventsAndSaveScript).

use pokered_core::overworld::{OverworldInput, OverworldScreen};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn none() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn press_a() -> OverworldInput {
    OverworldInput::new(false, false, false, false, true, false, false, false)
}

#[test]
fn hall_of_fame_scene_requests_ceremony() {
    let mut screen = OverworldScreen::new(MapId::HallOfFame, None, PokemonRedData);
    screen.run_on_load();
    let mut fired = false;
    for i in 0..5000 {
        // Advance OAK's dialogue; idle in between so the scripted walk and
        // text can run.
        let input = if i % 6 == 0 { press_a() } else { none() };
        screen.update_frame(input);
        if screen.pending_hof_ceremony {
            fired = true;
            break;
        }
    }
    assert!(
        fired,
        "the HallOfFame @load scene must end in game.enterHallOfFame()"
    );
    // The scene also resets the Elite 4 gauntlet for the rematch
    // (ResetEventRange INDIGO_PLATEAU_EVENTS_START..END).
    let flags = screen.script_flags();
    assert_eq!(
        flags.get("EVENT_BEAT_CHAMPION_RIVAL").copied().unwrap_or(false),
        false
    );
    // …and sets EVENT_HALL_OF_FAME_DEX_RATING (HoFDisplayPlayerStats).
    assert_eq!(
        flags
            .get("EVENT_HALL_OF_FAME_DEX_RATING")
            .copied()
            .unwrap_or(false),
        true
    );
}

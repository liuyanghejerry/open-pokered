//! End-to-end wiring tests for the Elite Four challenge flag:
//! - Entering LoreleisRoom sets EVENT_STARTED_ELITE_4
//!   (`LoreleiShowOrHideExitBlock` → `set BIT_STARTED_ELITE_4`,
//!   scripts/LoreleisRoom.asm:16-18).
//! - Re-entering IndigoPlateauLobby with the flag set resets the gauntlet
//!   (scripts/IndigoPlateauLobby.asm:9-14), so a failed run restarts from
//!   Lorelei.

use pokered_core::overworld::{OverworldInput, OverworldScreen};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn none() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn flag(screen: &OverworldScreen<PokemonRedData>, name: &str) -> bool {
    screen.script_flags().get(name).copied().unwrap_or(false)
}

fn set_flag(screen: &mut OverworldScreen<PokemonRedData>, name: &str) {
    let mut flags = screen.script_flags().clone();
    flags.insert(name.to_string(), true);
    screen.set_script_flags(flags);
}

#[test]
fn entering_loreleis_room_sets_started_elite_4() {
    let mut screen = OverworldScreen::new(MapId::LoreleisRoom, None, PokemonRedData);
    screen.run_on_load();
    for _ in 0..10 {
        screen.update_frame(none());
    }
    assert!(
        flag(&screen, "EVENT_STARTED_ELITE_4"),
        "LoreleisRoom @load must set EVENT_STARTED_ELITE_4"
    );
}

#[test]
fn lobby_resets_gauntlet_when_elite_4_started() {
    let mut screen = OverworldScreen::new(MapId::IndigoPlateauLobby, None, PokemonRedData);
    // Simulate a failed E4 run: challenge started, Lorelei beaten.
    set_flag(&mut screen, "EVENT_STARTED_ELITE_4");
    set_flag(&mut screen, "EVENT_BEAT_LORELEIS_ROOM_TRAINER_0");
    set_flag(&mut screen, "EVENT_AUTOWALKED_INTO_LORELEIS_ROOM");
    screen.run_on_load();
    for _ in 0..10 {
        screen.update_frame(none());
    }
    assert!(!flag(&screen, "EVENT_STARTED_ELITE_4"), "flag consumed");
    assert!(
        !flag(&screen, "EVENT_BEAT_LORELEIS_ROOM_TRAINER_0"),
        "Lorelei must be refightable"
    );
    assert!(
        !flag(&screen, "EVENT_AUTOWALKED_INTO_LORELEIS_ROOM"),
        "walk-in cutscene replays on the rematch"
    );
}

#[test]
fn lobby_leaves_flags_alone_when_elite_4_not_started() {
    let mut screen = OverworldScreen::new(MapId::IndigoPlateauLobby, None, PokemonRedData);
    set_flag(&mut screen, "EVENT_BEAT_LORELEIS_ROOM_TRAINER_0");
    screen.run_on_load();
    for _ in 0..10 {
        screen.update_frame(none());
    }
    // Without STARTED_ELITE_4 the original's `ret z` skips the reset.
    assert!(flag(&screen, "EVENT_BEAT_LORELEIS_ROOM_TRAINER_0"));
}

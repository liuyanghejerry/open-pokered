//! Tests for the Game Corner lucky slot-machine roll
//! (`GameCornerSelectLuckySlotMachine`, scripts/GameCorner.asm:8-22) and the
//! argument-less `openSlots()` resolution.

use super::screen::OverworldScreen;
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn screen_on(map: MapId) -> OverworldScreen<PokemonRedData> {
    OverworldScreen::new(map, None, PokemonRedData)
}

/// A lucky roll maps to sign textId 2..=8 (hidden event indices 0..6 of the
/// original's GAME_CORNER hidden-event table; index 5, "Someone's keys", is
/// the broken machine with no script here), and a roll of 0 means no lucky
/// machine this visit.
fn is_valid_lucky_sign(sign: Option<u8>) -> bool {
    matches!(sign, None | Some(2..=8))
}

#[test]
fn game_corner_map_load_rolls_a_valid_lucky_sign() {
    let mut saw_none = false;
    let mut saw_some = false;
    for _ in 0..60 {
        let mut screen = screen_on(MapId::GameCorner);
        screen.run_on_load();
        assert!(
            is_valid_lucky_sign(screen.lucky_slot_machine_sign),
            "roll out of range: {:?}",
            screen.lucky_slot_machine_sign
        );
        match screen.lucky_slot_machine_sign {
            None => saw_none = true,
            Some(_) => saw_some = true,
        }
    }
    assert!(saw_none, "a roll of 0 (no lucky machine) must be possible");
    assert!(saw_some, "a lucky machine must be rolled sometimes");
}

#[test]
fn non_game_corner_maps_do_not_roll() {
    let mut screen = screen_on(MapId::CeladonCity);
    screen.run_on_load();
    assert_eq!(screen.lucky_slot_machine_sign, None);
}

#[test]
fn default_open_slots_resolves_against_active_sign() {
    let mut screen = screen_on(MapId::GameCorner);
    screen.lucky_slot_machine_sign = Some(3);

    // The lucky sign itself.
    screen.active_sign_text_id = Some(3);
    assert!(screen.resolve_default_lucky_slots());

    // A different machine.
    screen.active_sign_text_id = Some(2);
    assert!(!screen.resolve_default_lucky_slots());

    // No sign context (e.g. the Beauty 2 NPC entry point).
    screen.active_sign_text_id = None;
    assert!(!screen.resolve_default_lucky_slots());

    // No lucky machine this visit (roll 0).
    screen.lucky_slot_machine_sign = None;
    screen.active_sign_text_id = Some(2);
    assert!(!screen.resolve_default_lucky_slots());
}

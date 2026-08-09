//! Tests for hidden items (engine/events/hidden_items.asm) and the ITEMFINDER
//! (engine/items/itemfinder.asm + ItemUseItemfinder in item_effects.asm).
//!
//! The Viridian Forest POTION at (1, 18) — table index 0 — is the fixture:
//! the player stands at (1, 17) facing Down so the tile in front is the item.

use super::screen::{OverworldAudioRequest, OverworldGameDataRequest, OverworldScreen};
use super::{Direction, OverworldInput};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::items::ItemId;
use pokered_data::maps::MapId;

fn screen_on(map: MapId) -> OverworldScreen<PokemonRedData> {
    OverworldScreen::new(map, None, PokemonRedData)
}

/// Player one tile above the Viridian Forest POTION, facing it.
fn screen_facing_viridian_potion() -> OverworldScreen<PokemonRedData> {
    let mut screen = screen_on(MapId::ViridianForest);
    screen.state.player.x = 1;
    screen.state.player.y = 17;
    screen.state.player.facing = Direction::Down;
    screen
}

fn press_a(screen: &mut OverworldScreen<PokemonRedData>) {
    let mut input = OverworldInput::none();
    input.a = true;
    screen.update_frame(input);
}

fn pending_dialogue_text(screen: &OverworldScreen<PokemonRedData>) -> String {
    screen
        .pending_dialogue
        .as_ref()
        .map(|d| {
            d.pages()
                .iter()
                .map(|p| format!("{}\n{}", p.line1, p.line2))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

#[test]
fn a_press_facing_hidden_item_finds_it() {
    let mut screen = screen_facing_viridian_potion();
    press_a(&mut screen);

    // "<PLAYER> found POTION!" (_FoundHiddenItemText, text_2.asm:751).
    let text = pending_dialogue_text(&screen);
    assert!(text.contains("found"), "shows the found text, got: {text:?}");
    assert!(text.contains("POTION"), "names the item, got: {text:?}");
    // The obtained flag is set (HiddenItems' FLAG_SET, hidden_items.asm:31-35).
    assert_eq!(screen.hidden_item_flags()[0] & 1, 1, "flag index 0 set");
    // The bag add is queued for the app layer (GiveItem).
    assert!(
        screen.game_data_requests.iter().any(|r| matches!(
            r,
            OverworldGameDataRequest::GiveItem { item, quantity: 1 } if item == "POTION"
        )),
        "queues the POTION give"
    );
    // SFX_GET_ITEM_2 plays on a successful take (hidden_items.asm:36-38).
    assert!(
        screen.audio_requests.iter().any(|r| matches!(
            r,
            OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_GET_ITEM_2"
        )),
        "plays the item-get jingle"
    );
}

#[test]
fn already_obtained_hidden_item_shows_nothing() {
    let mut screen = screen_facing_viridian_potion();
    let mut flags = *screen.hidden_item_flags();
    flags[0] |= 1; // index 0 = the Viridian Forest POTION
    screen.set_hidden_item_flags(flags);

    press_a(&mut screen);

    // HiddenItems returns on the flag test (hidden_items.asm:5-12): no text,
    // no item, no jingle — and the A press is consumed (no sign/NPC fallback).
    assert!(screen.pending_dialogue.is_none(), "nothing is shown");
    assert!(screen.game_data_requests.is_empty(), "no item is given");
    assert!(
        !screen.audio_requests.iter().any(|r| matches!(
            r,
            OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_GET_ITEM_2"
        )),
        "no jingle"
    );
}

#[test]
fn bag_full_keeps_item_and_flag_clear() {
    let mut screen = screen_facing_viridian_potion();
    // Fill the bag snapshot the screen uses to predict GiveItem (20 slots).
    screen.script_bag_names = (0..crate::items::inventory::BAG_ITEM_CAPACITY)
        .map(|i| format!("DUMMY_{i}"))
        .collect();

    press_a(&mut screen);

    let text = pending_dialogue_text(&screen);
    assert!(text.contains("found"), "the found text still shows first");
    assert!(
        text.contains("no more room"),
        "then the bag-full text (_HiddenItemBagFullText), got: {text:?}"
    );
    // GiveItem failed → flag NOT set, no give queued (hidden_items.asm:29-46).
    assert_eq!(screen.hidden_item_flags()[0] & 1, 0, "flag stays clear");
    assert!(screen.game_data_requests.is_empty(), "no item is given");
}

#[test]
fn a_press_off_hidden_tile_does_nothing() {
    let mut screen = screen_on(MapId::ViridianForest);
    screen.state.player.x = 5;
    screen.state.player.y = 17;
    screen.state.player.facing = Direction::Down;
    press_a(&mut screen);
    assert!(screen.pending_dialogue.is_none());
    assert!(screen.game_data_requests.is_empty());
    assert_eq!(screen.hidden_item_flags(), &[0; crate::save::game_data::HIDDEN_ITEMS_BYTES]);
}

#[test]
fn itemfinder_dings_when_item_nearby() {
    let mut screen = screen_facing_viridian_potion();

    let consumed = screen.use_field_item(ItemId::Itemfinder, MapId::PalletTown);
    assert!(!consumed, "the ITEMFINDER is a key item");
    let text = pending_dialogue_text(&screen);
    assert!(
        text.contains("Yes! ITEMFINDER"),
        "_ItemfinderFoundItemText (text_6.asm:119), got: {text:?}"
    );

    // ItemUseItemfinder plays SFX_HEALING_MACHINE + SFX_PURCHASE four times
    // (item_effects.asm:1928-1935); the port meters them out per frame.
    let mut sounds: Vec<String> = Vec::new();
    for _ in 0..(8 * super::hidden_items::ITEMFINDER_DING_FRAMES as usize + 2) {
        screen.update_frame(OverworldInput::none());
        for req in screen.audio_requests.drain(..) {
            if let OverworldAudioRequest::PlaySound { sound_id } = req {
                sounds.push(sound_id);
            }
        }
    }
    let expected: Vec<String> = (0..4)
        .flat_map(|_| ["SFX_HEALING_MACHINE".to_string(), "SFX_PURCHASE".to_string()])
        .collect();
    assert_eq!(sounds, expected, "four ding pairs, alternating");
}

#[test]
fn itemfinder_silent_when_nothing_nearby() {
    let mut screen = screen_on(MapId::PalletTown); // no hidden items at all
    let consumed = screen.use_field_item(ItemId::Itemfinder, MapId::PalletTown);
    assert!(!consumed);
    let text = pending_dialogue_text(&screen);
    assert!(
        text.contains("Nope! ITEMFINDER"),
        "_ItemfinderFoundNothingText (text_6.asm:125), got: {text:?}"
    );
    assert!(screen.itemfinder_dings.is_none(), "no dings scheduled");
}

#[test]
fn itemfinder_ignores_obtained_items() {
    let mut screen = screen_facing_viridian_potion();
    // Take both Viridian Forest items (indices 0 and 1) — nothing left nearby.
    let mut flags = *screen.hidden_item_flags();
    flags[0] |= 0b11;
    screen.set_hidden_item_flags(flags);

    screen.use_field_item(ItemId::Itemfinder, MapId::PalletTown);
    let text = pending_dialogue_text(&screen);
    assert!(
        text.contains("Nope! ITEMFINDER"),
        "obtained items are skipped (HiddenItemNear flag test), got: {text:?}"
    );
    assert!(screen.itemfinder_dings.is_none());
}

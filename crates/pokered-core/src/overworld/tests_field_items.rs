//! Tests for `OverworldScreen::use_field_item` — the bag ITEM-menu USE dispatch
//! (POKé FLUTE Snorlax trigger, BICYCLE ride toggle, ESCAPE ROPE dungeon warp).

use super::screen::{OverworldScreen, WarpFadeState};
use super::{Direction, OverworldInput};
use dotzuki_engine::overworld::types::TransportMode;
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::items::ItemId;
use pokered_data::maps::MapId;

fn screen_on(map: MapId) -> OverworldScreen<PokemonRedData> {
    OverworldScreen::new(map, None, PokemonRedData)
}

fn flag_set(screen: &OverworldScreen<PokemonRedData>, name: &str) -> bool {
    screen.script_flags().get(name).copied().unwrap_or(false)
}

#[test]
fn bicycle_toggles_transport_mode() {
    let mut screen = screen_on(MapId::Route1);
    assert_eq!(screen.state.player.transport, TransportMode::Walking);

    let consumed = screen.use_field_item(ItemId::Bicycle, MapId::PalletTown);
    assert!(!consumed, "the BICYCLE is a key item and is never consumed");
    assert_eq!(screen.state.player.transport, TransportMode::Biking);
    assert!(screen.pending_dialogue.is_some(), "shows a 'got on' message");

    screen.use_field_item(ItemId::Bicycle, MapId::PalletTown);
    assert_eq!(screen.state.player.transport, TransportMode::Walking);
}

#[test]
fn bicycle_refused_while_surfing() {
    let mut screen = screen_on(MapId::Route1);
    screen.state.player.transport = TransportMode::Surfing;
    screen.use_field_item(ItemId::Bicycle, MapId::PalletTown);
    assert_eq!(
        screen.state.player.transport,
        TransportMode::Surfing,
        "can't switch to the BICYCLE while SURFING"
    );
}

#[test]
fn poke_flute_sets_snorlax_fight_flag_on_route12() {
    let mut screen = screen_on(MapId::Route12);
    assert!(!flag_set(&screen, "EVENT_FIGHT_ROUTE12_SNORLAX"));

    let consumed = screen.use_field_item(ItemId::PokeFlute, MapId::PalletTown);
    assert!(!consumed, "the POKé FLUTE is a key item");
    assert!(
        flag_set(&screen, "EVENT_FIGHT_ROUTE12_SNORLAX"),
        "using the flute on Route 12 makes the SNORLAX battle-ready"
    );
    // PlayedFluteHadEffectText (engine/items/item_effects.asm:1801-1806):
    // the overworld jingle plays when the flute had an effect.
    assert!(
        screen.audio_requests.iter().any(|r| matches!(
            r,
            super::screen::OverworldAudioRequest::PlaySound { sound_id }
                if sound_id == "SFX_POKEFLUTE"
        )),
        "requests the SFX_POKEFLUTE jingle"
    );
}

#[test]
fn poke_flute_does_nothing_off_route() {
    let mut screen = screen_on(MapId::PalletTown);
    screen.use_field_item(ItemId::PokeFlute, MapId::PalletTown);
    assert!(!flag_set(&screen, "EVENT_FIGHT_ROUTE12_SNORLAX"));
    assert!(screen.pending_dialogue.is_some(), "still shows the 'nothing happened' line");
    // PlayedFluteNoEffectText has no sound in the original.
    assert!(
        !screen.audio_requests.iter().any(|r| matches!(
            r,
            super::screen::OverworldAudioRequest::PlaySound { sound_id }
                if sound_id == "SFX_POKEFLUTE"
        )),
        "no jingle when the flute had no effect"
    );
}

#[test]
fn escape_rope_refused_on_outside_map() {
    let mut screen = screen_on(MapId::Route1); // overworld tileset = outside
    screen.last_map = Some(MapId::Route2);
    screen.last_map_entry = Some((5, 5));

    let consumed = screen.use_field_item(ItemId::EscapeRope, MapId::PalletTown);
    assert!(!consumed, "not consumed when it can't be used");
    assert!(screen.pending_warp.is_none(), "no warp queued outside");
    assert!(matches!(screen.warp_fade_state, WarpFadeState::Idle));
}

#[test]
fn escape_rope_refused_on_ship_tileset() {
    // EscapeRopeTilesets (data/tilesets/escape_rope_tilesets.asm) lists only
    // FOREST/CEMETERY/CAVERN/FACILITY/INTERIOR — the SHIP tileset (SS Anne)
    // is NOT included, so the original refuses the item there.
    let mut screen = screen_on(MapId::SSAnne1F); // ship tileset
    let consumed = screen.use_field_item(ItemId::EscapeRope, MapId::PalletTown);
    assert!(!consumed);
    assert!(screen.pending_warp.is_none());
    assert!(
        screen.pending_dialogue.is_some(),
        "refusal message shown"
    );
}

#[test]
fn escape_rope_refused_in_agathas_room() {
    // ItemUseEscapeRope (item_effects.asm:1496-1498) also refuses in
    // AGATHAS_ROOM even though its CEMETERY tileset is in the list.
    let mut screen = screen_on(MapId::AgathasRoom);
    let consumed = screen.use_field_item(ItemId::EscapeRope, MapId::PalletTown);
    assert!(!consumed);
    assert!(screen.pending_warp.is_none());
}

#[test]
fn escape_rope_warps_to_last_pokemon_center() {
    // Gen-1: ItemUseEscapeRope sets BIT_FLY_WARP|BIT_ESCAPE_WARP
    // (item_effects.asm:1508-1510); LoadSpecialWarpData then warps to
    // wLastBlackoutMap's fly point (special_warps.asm:76-80) — the last
    // Pokémon Center the player healed at (SetLastBlackoutMap). NOT the
    // dungeon entrance.
    let mut screen = screen_on(MapId::MtMoon1F); // cavern tileset = interior
    let consumed = screen.use_field_item(ItemId::EscapeRope, MapId::CeruleanCity);
    assert!(consumed, "ESCAPE ROPE is a consumable");
    assert!(
        screen.pending_dialogue.is_none(),
        "the warp animation replaces any dialogue"
    );
    let warp = screen.pending_warp.as_ref().expect("a warp is queued");
    // Cerulean City's fly point (FlyWarpDataPtr — the same table FLY uses).
    assert_eq!(warp.dest_map, MapId::CeruleanCity);
    assert_eq!((warp.dest_x, warp.dest_y), (19, 18));
    // _LeaveMapAnim (BIT_ESCAPE_WARP): the spin-out plays first; the
    // fade-out-to-white starts when it finishes.
    assert!(screen.teleport_spin.is_some(), "spin-out plays first");
    assert!(screen.warp_fade_to_white, "escape warps fade to white");
    assert!(matches!(screen.warp_fade_state, WarpFadeState::Idle));
    let input = super::OverworldInput::new(
        false, false, false, false, false, false, false, false,
    );
    while screen.teleport_spin.is_some() {
        screen.update_frame(input);
    }
    assert!(matches!(
        screen.warp_fade_state,
        WarpFadeState::FadingOut { .. }
    ));
}

#[test]
fn escape_rope_without_any_heal_falls_back_to_pallet_town() {
    // wLastBlackoutMap starts at PALLET_TOWN (game_data.last_blackout_map
    // defaults to 0); the fly-point lookup then lands on Pallet Town.
    let mut screen = screen_on(MapId::MtMoon1F);
    let consumed = screen.use_field_item(ItemId::EscapeRope, MapId::PalletTown);
    assert!(consumed);
    let warp = screen.pending_warp.as_ref().expect("a warp is queued");
    assert_eq!(warp.dest_map, MapId::PalletTown);
    assert_eq!((warp.dest_x, warp.dest_y), (5, 6));
}

// -- REPEL (ItemUseRepel: 100 / 200 / 250 steps, consumed) -------------------

#[test]
fn repel_sets_100_steps_and_is_consumed() {
    let mut screen = screen_on(MapId::Route1);
    let consumed = screen.use_field_item(ItemId::Repel, MapId::PalletTown);
    assert!(consumed, "REPEL is a consumable");
    assert_eq!(screen.state.repel_steps, 100);
    assert!(screen.pending_dialogue.is_some(), "shows the 'used' text");
}

#[test]
fn super_and_max_repel_set_200_and_250_steps() {
    let mut screen = screen_on(MapId::Route1);
    assert!(screen.use_field_item(ItemId::SuperRepel, MapId::PalletTown));
    assert_eq!(screen.state.repel_steps, 200);

    let mut screen = screen_on(MapId::Route1);
    assert!(screen.use_field_item(ItemId::MaxRepel, MapId::PalletTown));
    assert_eq!(screen.state.repel_steps, 250);
}

#[test]
fn repel_wore_off_message_fires_on_last_step() {
    let mut screen = screen_on(MapId::Route1);
    screen.use_field_item(ItemId::Repel, MapId::PalletTown);
    screen.pending_dialogue = None; // clear the 'used' text
    screen.state.repel_steps = 1;

    // Simulate the per-step decrement happening in the movement update.
    screen.state.repel_steps = 0;
    assert!(
        screen.repel_wore_off(1),
        "the step that spends the last REPEL charge is detected"
    );
    screen.show_repel_wore_off_message();
    assert!(
        screen.pending_dialogue.is_some(),
        "the 'REPEL's effect wore off.' text is queued"
    );
}

#[test]
fn repel_wore_off_message_quiet_while_steps_remain() {
    let mut screen = screen_on(MapId::Route1);
    screen.state.repel_steps = 5;
    assert!(!screen.repel_wore_off(6));
    // No repel active before the step: no message either.
    screen.state.repel_steps = 0;
    assert!(!screen.repel_wore_off(0));
    assert!(screen.pending_dialogue.is_none());
}

/// Screen-level integration: using REPEL from the bag sets the step counter,
/// walking spends it, and the final step shows "REPEL's effect wore off."
/// (Original: ItemUseRepel + DisplayRepelWoreOffText on the last step.)
#[test]
fn repel_wears_off_after_final_step_with_message() {
    let mut screen = screen_on(MapId::PalletTown);
    // (10,3) with a clear tile north of it (the Pallet Town Oak tests walk
    // this column); one step up stays clear of the (10,1) coord event.
    screen.state.player.x = 10;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Up;

    assert!(screen.use_field_item(ItemId::Repel, MapId::PalletTown));
    assert_eq!(screen.state.repel_steps, 100);
    screen.pending_dialogue = None; // dismiss the "used the REPEL" text
    screen.state.repel_steps = 1; // the next step is the last

    let neutral = OverworldInput::new(false, false, false, false, false, false, false, false);
    let up = OverworldInput::new(true, false, false, false, false, false, false, false);
    for _ in 0..120 {
        let input = if screen.pending_dialogue.is_some() {
            neutral
        } else {
            up
        };
        screen.update_frame(input);
        if screen.pending_dialogue.is_some() {
            break;
        }
    }

    assert_eq!(screen.state.repel_steps, 0, "the step spent the last charge");
    assert_eq!(
        (screen.state.player.x, screen.state.player.y),
        (10, 2),
        "the player really walked a step"
    );
    assert!(
        screen.pending_dialogue.is_some(),
        "'REPEL's effect wore off.' shows on the final step"
    );
}


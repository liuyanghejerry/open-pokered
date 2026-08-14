//! Regression tests for toggleable-object persistence: the toggleId → SRAM
//! bit mapping (original TOGGLE_* constants) and the hide/show object cleanup
//! in scene scripts (Daisy's BluesHouse sprite swap, the TOWN MAP pickup).
//!
//! These tests drive a real `OverworldScreen` with the embedded `.scene`
//! scripts, the same sources the game compiles at runtime.

use super::screen::OverworldScreen;
use super::{Direction, OverworldInput};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;
use pokered_data::toggleable_objects::{is_object_hidden, toggle_id_to_bit_index};

fn neutral_input() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn up_input() -> OverworldInput {
    OverworldInput::new(true, false, false, false, false, false, false, false)
}

fn a_input() -> OverworldInput {
    OverworldInput::new(false, false, false, false, true, false, false, false)
}

fn flag_set(screen: &OverworldScreen<PokemonRedData>, name: &str) -> bool {
    screen.script_flags().get(name).copied().unwrap_or(false)
}

/// Pallet Town's `coordNorthExit` runs the original palletTownDaisy cleanup
/// after the Oak escort: it must hide Daisy's SITTING sprite and show her
/// WALKING sprite in BluesHouse (TOGGLE_DAISY_SITTING / TOGGLE_DAISY_WALKING).
/// The Pallet Town Girl/Fisher NPCs must NOT be touched — the original
/// toggles BluesHouse objects, not Pallet Town's.
#[test]
fn daisy_sprite_swap_writes_toggle_bits() {
    let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    // Post-escort state: Oak's block is skipped, Daisy's cleanup runs.
    screen.set_flag_live("EVENT_FOLLOWED_OAK_INTO_LAB", true);
    screen.set_flag_live("EVENT_GOT_TOWN_MAP", true);
    screen.set_flag_live("EVENT_ENTERED_BLUES_HOUSE", true);
    screen.state.player.x = 10;
    screen.state.player.y = 2;
    screen.state.player.facing = Direction::Up;

    // One step onto the (10,1) north-exit trigger tile, then keep ticking so
    // the setFlag + hideObject + showObject effects all finish applying.
    let mut stepped = false;
    for _ in 0..240 {
        let input = if !stepped {
            stepped = true;
            up_input()
        } else {
            neutral_input()
        };
        screen.update_frame(input);
    }

    assert!(
        flag_set(&screen, "EVENT_DAISY_WALKING"),
        "palletTownDaisy cleanup never ran"
    );
    let daisy1 = toggle_id_to_bit_index("BLUESHOUSE_DAISY1").unwrap();
    let daisy2 = toggle_id_to_bit_index("BLUESHOUSE_DAISY2").unwrap();
    assert!(
        is_object_hidden(screen.toggleable_object_flags(), daisy1),
        "Daisy sitting sprite must be hidden (TOGGLE_DAISY_SITTING)"
    );
    assert!(
        !is_object_hidden(screen.toggleable_object_flags(), daisy2),
        "Daisy walking sprite must be shown (TOGGLE_DAISY_WALKING)"
    );
    assert!(
        flag_set(&screen, "__OBJ_HIDDEN_BLUESHOUSE_DAISY1"),
        "runtime hide flag for Daisy sitting must be set"
    );
    assert!(
        flag_set(&screen, "__OBJ_SHOWN_BLUESHOUSE_DAISY2"),
        "runtime show flag for Daisy walking must be set"
    );

    // Loading BluesHouse with this SRAM must render sitting-Daisy hidden
    // and walking-Daisy visible.
    let mut house = OverworldScreen::new(MapId::BluesHouse, None, PokemonRedData);
    house.set_script_flags(screen.script_flags());
    house.set_toggleable_object_flags(*screen.toggleable_object_flags());
    house.apply_hidden_object_flags();
    assert!(!house.npc_states[0].visible, "sitting Daisy must be hidden");
    assert!(house.npc_states[1].visible, "walking Daisy must be visible");
}

/// Talking to Daisy with the POKeDEX gives the TOWN MAP and hides the map
/// object on the table (TOGGLE_TOWN_MAP). The pickup must persist across a
/// BluesHouse reload (SRAM toggle bit), not just for the current screen.
#[test]
fn town_map_pickup_hides_table_map() {
    let mut screen = OverworldScreen::new(MapId::BluesHouse, None, PokemonRedData);
    screen.set_flag_live("EVENT_GOT_POKEDEX", true);
    // Daisy sitting is at (2,3) facing right; stand west of her and talk.
    screen.state.player.x = 1;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Right;

    let mut got_map = false;
    let mut map_hidden = false;
    for frame in 0..1200 {
        let input =
            if screen.active_script_effect.is_some() || screen.pending_dialogue.is_some() {
                if frame % 20 == 0 {
                    a_input()
                } else {
                    neutral_input()
                }
            } else {
                a_input()
            };
        screen.update_frame(input);
        if flag_set(&screen, "EVENT_GOT_TOWN_MAP") {
            got_map = true;
        }
        if got_map && !screen.npc_states[2].visible {
            map_hidden = true;
            break;
        }
    }

    assert!(got_map, "Daisy never handed over the TOWN MAP");
    assert!(
        map_hidden,
        "the TOWN MAP object on the table must hide after the pickup"
    );
    let town_map_bit = toggle_id_to_bit_index("BLUESHOUSE_TOWN_MAP").unwrap();
    assert_eq!(town_map_bit, 0x29, "TOGGLE_TOWN_MAP sanity");
    assert!(
        is_object_hidden(screen.toggleable_object_flags(), town_map_bit),
        "TOWN MAP pickup must persist the hidden bit to SRAM"
    );
    assert!(
        flag_set(&screen, "__OBJ_HIDDEN_BLUESHOUSE_TOWN_MAP"),
        "runtime hide flag for the TOWN MAP must be set"
    );

    // A fresh BluesHouse restored from SRAM must start with the map hidden.
    let mut reload = OverworldScreen::new(MapId::BluesHouse, None, PokemonRedData);
    reload.set_script_flags(screen.script_flags());
    reload.set_toggleable_object_flags(*screen.toggleable_object_flags());
    reload.apply_hidden_object_flags();
    assert!(
        !reload.npc_states[2].visible,
        "TOWN MAP must stay hidden when BluesHouse is reloaded"
    );
}

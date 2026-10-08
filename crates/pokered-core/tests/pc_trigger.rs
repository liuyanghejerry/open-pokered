//! End-to-end wiring tests for the PC storage screens: the Pokémon Center PC
//! sign and Red's bedroom PC sign must open the PC via `game.openPC()` /
//! `game.openItemPC()` (engine/menus/pc.asm ActivatePC, players_pc.asm
//! PlayerPC).

use pokered_core::overworld::{Direction, OverworldInput, OverworldScreen};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn none() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn press_a() -> OverworldInput {
    OverworldInput::new(false, false, false, false, true, false, false, false)
}

/// Press A at the given tile/facing, then idle until the script finishes
/// dispatching; return the resulting `pending_pc`.
fn interact(save_map: MapId, x: u16, y: u16, facing: Direction) -> Option<String> {
    let mut screen = OverworldScreen::new(save_map, None, PokemonRedData);
    screen.state.player.x = x;
    screen.state.player.y = y;
    screen.state.player.facing = facing;
    screen.update_frame(press_a());
    for _ in 0..60 {
        screen.update_frame(none());
        if screen.pending_pc.is_some() {
            break;
        }
    }
    screen.pending_pc.take()
}

#[test]
fn viridian_pokecenter_pc_sign_opens_pc() {
    // Original: hidden_event 13,3 OpenPokemonCenterPC facing up
    // (data/events/hidden_events.asm:156).
    assert_eq!(
        interact(MapId::ViridianPokecenter, 13, 4, Direction::Up).as_deref(),
        Some("center")
    );
}

#[test]
fn pewter_pokecenter_pc_sign_opens_pc() {
    assert_eq!(
        interact(MapId::PewterPokecenter, 13, 4, Direction::Up).as_deref(),
        Some("center")
    );
}

#[test]
fn reds_bedroom_pc_sign_opens_item_pc() {
    // Original: hidden_event 0,1 OpenRedsPC facing up
    // (data/events/hidden_events.asm:137).
    assert_eq!(
        interact(MapId::RedsHouse2F, 0, 2, Direction::Up).as_deref(),
        Some("items")
    );
}

#[test]
fn billshouse_pc_shows_monitor_before_bill_is_saved() {
    // Original: hidden_event 1,4 BillsHousePC facing up
    // (data/events/hidden_events.asm:488). Before the Bill subplot the
    // monitor just shows the teleporter (_BillsHouseMonitorText), no PC.
    let mut screen = OverworldScreen::new(MapId::BillsHouse, None, PokemonRedData);
    screen.state.player.x = 1;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Up;
    screen.update_frame(press_a());
    for _ in 0..60 {
        screen.update_frame(none());
        if screen.pending_dialogue.is_some() || screen.pending_pc.is_some() {
            break;
        }
    }
    assert_eq!(screen.pending_pc.take(), None);
    let dialogue = screen.pending_dialogue.take();
    let text = dialogue
        .map(|d| {
            d.pages()
                .iter()
                .flat_map(|p| [p.line1.as_ref(), p.line2.as_ref()])
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    assert!(
        text.contains("TELEPORTER") && text.contains("monitor"),
        "expected monitor text, got: {text:?}"
    );
}

#[test]
fn billshouse_pc_keeps_monitor_after_separation_until_player_returns() {
    // BillsHousePC checks EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING, rather than
    // EVENT_MET_BILL or merely receiving the ticket, before showing the list.
    for ticket in [false, true] {
        let mut screen = bills_house_pc();
        screen.set_flag_live("EVENT_MET_BILL", true);
        screen.set_flag_live("EVENT_USED_CELL_SEPARATOR_ON_BILL", true);
        screen.set_flag_live("EVENT_GOT_SS_TICKET", ticket);
        screen.update_frame(press_a());
        for _ in 0..60 {
            screen.update_frame(none());
            if screen.pending_dialogue.is_some() {
                break;
            }
        }
        assert!(screen.pending_pc.is_none());
        assert!(screen.pending_choice.is_none());
        let dialogue = screen
            .pending_dialogue
            .as_ref()
            .expect("teleporter monitor");
        let text = dialogue
            .pages()
            .iter()
            .flat_map(|p| [p.line1.as_ref(), p.line2.as_ref()])
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            text.contains("TELEPORTER") && text.contains("monitor"),
            "{text}"
        );
    }
}

fn bills_house_pc() -> OverworldScreen {
    let mut screen = OverworldScreen::new(MapId::BillsHouse, None, PokemonRedData);
    screen.state.player.x = 1;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Up;
    screen
}

fn tap(screen: &mut OverworldScreen, input: OverworldInput) {
    screen.update_frame(none());
    screen.update_frame(input);
    screen.update_frame(none());
}

#[test]
fn billshouse_pc_after_return_browses_all_four_entries_and_retains_cursor() {
    let mut screen = bills_house_pc();
    for flag in [
        "EVENT_MET_BILL",
        "EVENT_USED_CELL_SEPARATOR_ON_BILL",
        "EVENT_GOT_SS_TICKET",
        "EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING",
    ] {
        screen.set_flag_live(flag, true);
    }
    screen.update_frame(press_a());
    // Advance the introductory text with input, through the real hidden-event
    // binding, rather than invoking the scene handler directly.
    for frame in 0..200 {
        assert!(
            screen.pending_pc.is_none(),
            "Bill's list must not open storage"
        );
        if screen.pending_choice.is_some() {
            break;
        }
        if let Some(dialogue) = screen.pending_dialogue.as_mut() {
            dialogue.skip_to_full_page();
        }
        screen.update_frame(if frame % 2 == 0 { press_a() } else { none() });
    }
    assert_eq!(
        screen
            .pending_choice
            .as_ref()
            .expect("Eevee family list")
            .options,
        ["EEVEE", "FLAREON", "JOLTEON", "VAPOREON", "CANCEL"]
    );
    for (index, species) in ["Eevee", "Flareon", "Jolteon", "Vaporeon"]
        .iter()
        .enumerate()
    {
        if index != 0 {
            tap(
                &mut screen,
                OverworldInput::new(false, true, false, false, false, false, false, false),
            );
        }
        assert_eq!(
            screen.pending_choice.as_ref().unwrap().selected,
            index as u32
        );
        tap(&mut screen, press_a());
        for _ in 0..20 {
            screen.update_frame(none());
            if screen.pending_pokedex_entry.is_some() {
                break;
            }
        }
        assert_eq!(
            screen
                .pending_pokedex_entry
                .as_ref()
                .expect("dex entry")
                .species,
            *species
        );
        tap(
            &mut screen,
            OverworldInput::new(false, false, false, false, false, true, false, false),
        );
        for _ in 0..20 {
            screen.update_frame(none());
            if screen.pending_choice.is_some() {
                break;
            }
        }
        assert!(screen.pending_pokedex_entry.is_none());
        assert_eq!(
            screen
                .pending_choice
                .as_ref()
                .expect("return to list")
                .selected,
            index as u32
        );
        assert!(screen.pending_pc.is_none());
    }
    tap(
        &mut screen,
        OverworldInput::new(false, false, false, false, false, true, false, false),
    );
    for _ in 0..20 {
        screen.update_frame(none());
    }
    assert!(screen.pending_choice.is_none());
    assert!(screen.pending_pokedex_entry.is_none());
    assert!(screen.pending_pc.is_none());
}

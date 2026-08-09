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
                .flat_map(|p| [p.line1, p.line2])
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
fn billshouse_pc_opens_bills_pc_after_bill_is_saved() {
    // After the Cell Separation System flow (EVENT_MET_BILL), the same PC
    // opens the direct BillsPc entry ("Switch on!" + Bill's #MON storage,
    // scripts/BillsHouse.asm BillsHousePCScript -> script_bills_pc).
    let mut screen = OverworldScreen::new(MapId::BillsHouse, None, PokemonRedData);
    screen.set_flag_live("EVENT_MET_BILL", true);
    screen.state.player.x = 1;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Up;
    screen.update_frame(press_a());
    for _ in 0..60 {
        screen.update_frame(none());
        if screen.pending_pc.is_some() {
            break;
        }
    }
    assert_eq!(screen.pending_pc.take().as_deref(), Some("bills"));
}

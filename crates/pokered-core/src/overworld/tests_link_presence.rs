//! Cable Club link-presence tests: the remote player's avatar override in
//! the Colosseum / Trade Center rooms (`OverworldScreen::link_opponent`).

use dotzuki_engine::overworld::{Direction, NpcMovementType};

use crate::link::{LinkOpponentPresence, LinkRole};
use crate::overworld::OverworldInput;
use crate::overworld::OverworldScreen;
use crate::data::maps::MapId;

fn neutral_input() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

/// While a link session is connected and the player is in a Cable Club
/// room, the app sets `link_opponent` and the room's opponent NPC (index 1)
/// is pinned to the remote player's spot every frame — the original's
/// `TradeCenter_Script` placement (scripts/TradeCenter.asm:17-30).
#[test]
fn link_presence_pins_opponent_npc() {
    let mut screen = OverworldScreen::new(MapId::Colosseum, None, crate::data::impl_traits::PokemonRedData);
    screen.run_on_load();

    // The map's placeholder opponent sits at its configured spot (2,2).
    let npc = screen
        .npc_states
        .iter()
        .find(|n| n.text_id == 1)
        .expect("Colosseum must define the opponent NPC");
    assert_eq!((npc.x, npc.y), (2, 2));

    // Host role: the remote player stands at the right end of the table,
    // facing left.
    screen.link_opponent = Some(LinkOpponentPresence::for_role(LinkRole::Host));
    screen.update_frame(neutral_input());

    let npc = screen
        .npc_states
        .iter()
        .find(|n| n.text_id == 1)
        .expect("opponent NPC still present");
    assert_eq!((npc.x, npc.y), (3, 2));
    assert_eq!(npc.facing, Direction::Left);
    assert_eq!(npc.movement_type, NpcMovementType::Stationary);
    assert!(npc.visible);

    // Guest role mirrors across the table.
    screen.link_opponent = Some(LinkOpponentPresence::for_role(LinkRole::Guest));
    screen.update_frame(neutral_input());
    let npc = screen
        .npc_states
        .iter()
        .find(|n| n.text_id == 1)
        .unwrap();
    assert_eq!((npc.x, npc.y), (1, 2));
    assert_eq!(npc.facing, Direction::Right);
}

/// Without the override the room keeps its placeholder NPC (no link session).
#[test]
fn link_presence_cleared_keeps_placeholder() {
    let mut screen = OverworldScreen::new(MapId::TradeCenter, None, crate::data::impl_traits::PokemonRedData);
    screen.run_on_load();
    screen.update_frame(neutral_input());

    let npc = screen
        .npc_states
        .iter()
        .find(|n| n.text_id == 1)
        .expect("TradeCenter must define the opponent NPC");
    assert_eq!((npc.x, npc.y), (2, 2));
}

/// `game.linkStart()` from the gameboy scene flags a request the app drains
/// (`take_link_start_request`), mirroring the party-select request pattern.
#[test]
fn link_start_request_flag_roundtrip() {
    let mut screen = OverworldScreen::new(MapId::Colosseum, None, crate::data::impl_traits::PokemonRedData);
    assert!(!screen.take_link_start_request());
    screen.link_start_requested = true;
    assert!(screen.take_link_start_request());
    assert!(!screen.take_link_start_request());
}

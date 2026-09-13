//! M2 closed-loop local navigation: `agent_move_to` / `agent_interact` /
//! `agent_interact_with` against a live Pallet Town overworld.

use pokered_agent::{InteractResult, NavigationResult};
use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::GameScreen;
use pokered_core::overworld::{Direction, OverworldScreen};
use pokered_core::save::SaveData;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};

/// Same construction idiom as agent_observation.rs: a fresh game in the
/// Pallet Town overworld at (x, y), facing down, empty save.
fn game_at_pallet(x: u16, y: u16) -> PokemonGame {
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red,
        None,
        None,
        None,
        false,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    game.save_data = SaveData::new();
    game.state.screen = GameScreen::Overworld;
    game.overworld = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    game.overworld.state.player.x = x;
    game.overworld.state.player.y = y;
    game.overworld.state.player.facing = Direction::Down;
    game
}

#[test]
fn move_to_reaches_open_tile_exactly() {
    // (5, 5) is the Red's-house door warp tile; the SE field at (16, 14)
    // is open ground. The walk must route around Blue's house.
    let mut game = game_at_pallet(5, 5);
    let outcome = game.agent_move_to(16, 14);
    assert_eq!(outcome.result, NavigationResult::Reached, "{:?}", outcome.detail);
    assert_eq!(
        (game.overworld.state.player.x, game.overworld.state.player.y),
        (16, 14)
    );
    assert!(outcome.steps > 0);
    assert!(outcome.frames > 0);
    // The walk never left the map.
    assert_eq!(game.overworld.state.current_map, MapId::PalletTown);
}

#[test]
fn move_to_reports_blocked_for_unwalkable_targets() {
    let mut game = game_at_pallet(5, 5);
    // House wall tile.
    let outcome = game.agent_move_to(6, 4);
    assert_eq!(outcome.result, NavigationResult::Blocked);
    assert_eq!(outcome.steps, 0);

    // Out-of-bounds tile.
    let outcome = game.agent_move_to(40, 40);
    assert_eq!(outcome.result, NavigationResult::Blocked);
    assert_eq!(outcome.detail.as_deref(), Some("target out of bounds"));

    // The Fisher's own tile is NPC-occupied — not a walkable goal.
    let fisher = game
        .overworld
        .npc_states
        .iter()
        .find(|n| n.npc_index == 2)
        .unwrap();
    let (fx, fy) = (fisher.x, fisher.y);
    let outcome = game.agent_move_to(fx, fy);
    assert_eq!(outcome.result, NavigationResult::Blocked);
}

#[test]
fn move_to_onto_warp_tile_reports_map_changed() {
    // Standing below the door, walking back up onto the warp tile warps
    // into Red's house — the documented MapChanged outcome.
    let mut game = game_at_pallet(5, 6);
    let outcome = game.agent_move_to(5, 5);
    assert_eq!(outcome.result, NavigationResult::MapChanged, "{:?}", outcome.detail);
    assert_eq!(game.overworld.state.current_map, MapId::RedsHouse1F);
}

#[test]
fn move_to_interrupted_by_oak_north_exit_event() {
    // The coord trigger at (10, 1) fires Oak's "It's unsafe!" event the
    // moment the player steps onto it. Reaching the trigger tile itself
    // reports Reached (events on the arrival tile are the caller's)…
    let mut game = game_at_pallet(10, 3);
    let outcome = game.agent_move_to(10, 1);
    assert_eq!(outcome.result, NavigationResult::Reached, "{:?}", outcome.detail);
    assert!(
        game.overworld.active_script_effect_label().is_some()
            || !game.overworld.script_engine_idle()
            || game.overworld.pending_dialogue.is_some(),
        "the Oak event must be running after stepping onto the trigger"
    );

    // …and a follow-up walk requested while the cutscene owns the game
    // aborts immediately as Interrupted (or EnteredDialogue while the
    // opening text is up), without walking anywhere.
    let outcome = game.agent_move_to(5, 5);
    assert!(
        matches!(
            outcome.result,
            NavigationResult::Interrupted | NavigationResult::EnteredDialogue
        ),
        "expected interruption, got {:?}",
        outcome.result
    );
    assert_eq!(outcome.steps, 0);
}

#[test]
fn interact_with_sign_opens_dialogue() {
    // sign:1 at (7, 9) — the PALLET TOWN sign. From (5, 5) the executor
    // navigates adjacent, faces it, and presses A.
    let mut game = game_at_pallet(5, 5);
    let outcome = game.agent_interact_with("sign:1");
    assert_eq!(outcome.result, InteractResult::Dialogue, "{:?}", outcome.detail);
    assert!(game.overworld.pending_dialogue.is_some());
    let navigation = outcome.navigation.expect("walked to the sign");
    assert_eq!(navigation.result, NavigationResult::Reached);
}

#[test]
fn interact_faces_adjacent_sign_and_opens_dialogue() {
    // Already adjacent to the (7, 9) sign but facing away: interact
    // turns toward it before pressing A.
    let mut game = game_at_pallet(7, 10);
    game.overworld.state.player.facing = Direction::Left;
    let outcome = game.agent_interact();
    assert_eq!(outcome.result, InteractResult::Dialogue, "{:?}", outcome.detail);
    assert!(game.overworld.pending_dialogue.is_some());
}

#[test]
fn interact_on_bare_ground_reports_nothing() {
    let mut game = game_at_pallet(16, 7);
    game.overworld.state.player.facing = Direction::Right;
    let outcome = game.agent_interact();
    assert_eq!(outcome.result, InteractResult::Nothing, "{:?}", outcome.detail);
    assert!(game.overworld.pending_dialogue.is_none());
}

#[test]
fn interact_with_reports_not_found() {
    let mut game = game_at_pallet(5, 5);
    // Oak (npc:0) is defaultHidden in Pallet Town — not talkable.
    let outcome = game.agent_interact_with("npc:0");
    assert_eq!(outcome.result, InteractResult::NotFound);
    // Warps are move_to's job.
    let outcome = game.agent_interact_with("warp:0");
    assert_eq!(outcome.result, InteractResult::NotFound);
    // Malformed / unknown ids.
    let outcome = game.agent_interact_with("bogus");
    assert_eq!(outcome.result, InteractResult::NotFound);
}

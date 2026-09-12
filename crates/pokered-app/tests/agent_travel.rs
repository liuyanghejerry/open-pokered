//! M3 world graph + cross-map travel: route queries and `agent_travel_to`
//! end-to-end across connections, gate warps, and wild-encounter grass.

use pokered_agent::{RouteLegKind, TravelResult, WorldGraph};
use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::GameScreen;
use pokered_core::overworld::{Direction, OverworldScreen};
use pokered_core::pokemon::stats::create_pokemon;
use pokered_core::save::SaveData;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId, species::Species};

/// A fresh game in the Pallet Town overworld with a level-100 lead (a
/// fast, one-hit-wonder that makes wild encounters auto-resolvable) and
/// the story flags that geographically gate the north road out of
/// Viridian pre-set (M3 is geography-only; the flags stand in for the
/// parcel delivery playthrough.py does first).
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

fn with_strong_lead(mut game: PokemonGame) -> PokemonGame {
    // Level-100 lead: fast (guaranteed RUN from wilds) and one-shots
    // trainers. Mewtwo's natural level-100 moveset is all-status, so
    // slot 0 is overwritten with a damaging move for the auto-resolver.
    let mut mon = create_pokemon(Species::Mewtwo, 100, [0x9a, 0x78]).unwrap();
    mon.moves[0] = pokered_data::moves::MoveId::PsychicM;
    game.save_data.party.add(mon).unwrap();
    game.overworld.party_count = game.save_data.party.count() as u8;
    game.overworld.party_lead_level = game.save_data.party.leader_level();
    // Story flags that gate the raw geography: Oak's Pallet north-exit
    // escort (needs the starter event done) and the Viridian parcel/coffee
    // gates. M3 is geography-only — these stand in for story progress.
    game.overworld.set_flag_live("EVENT_FOLLOWED_OAK_INTO_LAB", true);
    game.overworld.set_flag_live("EVENT_GOT_STARTER", true);
    game.overworld.set_flag_live("EVENT_GOT_POKEDEX", true);
    game.overworld.set_flag_live("EVENT_GOT_OAKS_PARCEL", true);
    game
}

#[test]
fn world_route_pallet_to_pewter_via_query() {
    let route = WorldGraph::shared()
        .find_route(MapId::PalletTown, MapId::PewterCity)
        .expect("route exists");
    let maps: Vec<&str> = route
        .iter()
        .map(|e| e.from_map.as_str())
        .chain(std::iter::once(route.last().unwrap().to_map.as_str()))
        .collect();
    assert_eq!(
        maps,
        ["PalletTown", "Route1", "ViridianCity", "Route2", "PewterCity"]
    );
    assert!(route.iter().all(|e| e.kind == RouteLegKind::Connection));
}

#[test]
fn travel_to_viridian_city_end_to_end() {
    // Two connection legs (Pallet → Route 1 → Viridian) with wild grass
    // on the way — the auto-resolver runs from encounters with the
    // level-100 lead.
    let mut game = with_strong_lead(game_at_pallet(10, 9));
    let outcome = game.agent_travel_to(MapId::ViridianCity);
    assert_eq!(
        outcome.result,
        TravelResult::Reached,
        "detail: {:?}, expected {:?} actual {:?}",
        outcome.detail,
        outcome.expected_map,
        outcome.actual_map
    );
    assert_eq!(game.overworld.state.current_map, MapId::ViridianCity);
    assert_eq!(outcome.legs_completed, outcome.legs.len());
    assert_eq!(outcome.legs.len(), 2);
}

#[test]
fn travel_to_unreachable_or_dataless_map_errors_cleanly() {
    let mut game = with_strong_lead(game_at_pallet(10, 9));
    // UnusedMap0B has no map data → invalid target, no panic, no walk.
    let outcome = game.agent_travel_to(MapId::UnusedMap0B);
    assert!(
        matches!(
            outcome.result,
            TravelResult::InvalidTarget | TravelResult::Blocked
        ),
        "got {:?}",
        outcome.result
    );
    assert_eq!(game.overworld.state.current_map, MapId::PalletTown);
}

#[test]
fn travel_to_pewter_city_end_to_end() {
    // The full stretch: Pallet → Route1 → Viridian → Route2 → forest
    // gates → Viridian Forest → Route 2 north → Pewter. Wild battles on
    // the routes and in the forest are auto-run/-fought; forest trainers
    // may add sight-line battles (fought with the level-100 lead).
    let mut game = with_strong_lead(game_at_pallet(10, 9));
    let outcome = game.agent_travel_to(MapId::PewterCity);
    assert_eq!(
        outcome.result,
        TravelResult::Reached,
        "detail: {:?}, expected {:?} actual {:?}, legs completed {}/{}",
        outcome.detail,
        outcome.expected_map,
        outcome.actual_map,
        outcome.legs_completed,
        outcome.legs.len()
    );
    assert_eq!(game.overworld.state.current_map, MapId::PewterCity);
    assert_eq!(outcome.legs_completed, outcome.legs.len());
    assert!(outcome.legs.len() >= 3);
}

#[test]
fn travel_to_current_map_is_trivially_reached() {
    let mut game = with_strong_lead(game_at_pallet(10, 9));
    let outcome = game.agent_travel_to(MapId::PalletTown);
    assert_eq!(outcome.result, TravelResult::Reached);
    assert_eq!(outcome.legs_completed, 0);
}

/// Regression for the intro-mirror overflow the auto-battler exposed: a
/// fast input driver can advance the core intro phase before the
/// send-out VFX poof completes; the stale intro mirror then ticked its
/// u8 frame counter past 255 while move animations owned the shared
/// player. `on_phase_change` now drops the mirror when the battle
/// leaves the intro — this drives a trainer battle with A taps far
/// beyond 255 frames.
#[test]
fn trainer_battle_fast_forward_does_not_overflow() {
    use dotzuki_app::InputState;
    use dotzuki_renderer::input::GbButton;

    let mut game = with_strong_lead(game_at_pallet(10, 9));
    game.debug_start_trainer_battle(pokered_data::trainer_data::TrainerClass::BugCatcher, 1);
    game.state.screen = GameScreen::Battle;
    for i in 0..3000 {
        let mut input = InputState::new();
        if i % 2 == 0 {
            input.press(GbButton::A);
        }
        game.update(&input);
        if game.state.screen != GameScreen::Battle {
            break;
        }
    }
    // The battle runs to completion (win) or is still legitimately
    // running — the point is no overflow panic occurred.
    assert!(matches!(
        game.state.screen,
        GameScreen::Battle | GameScreen::Overworld
    ));
}

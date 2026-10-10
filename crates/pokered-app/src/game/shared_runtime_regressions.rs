//! Runtime regression coverage migrated from the former TUI copy.

use super::*;
use pokered_core::game_state::{Lang, MainMenuChoice};
use pokered_renderer::input::InputState;

/// A game standing in the overworld as if a save was loaded via CONTINUE:
/// save-file position (5,5) on Viridian City, live overworld position
/// (30,20) on Pallet Town. Any rebuild of the overworld from the save data
/// becomes visible as a position reset.
fn game_at_overworld() -> PokemonGame {
    let mut game = PokemonGame::new(pokered_core::data::wild_data::GameVersion::Red);
    game.state.screen = GameScreen::Overworld;
    game.main_menu.last_choice = Some(MainMenuChoice::Continue);
    game.save_data.game_data.position.map_id = pokered_data::maps::MapId::ViridianCity as u8;
    game.save_data.game_data.position.x = 5;
    game.save_data.game_data.position.y = 5;
    game.overworld.state.current_map = pokered_data::maps::MapId::PalletTown;
    game.overworld.state.player.x = 30;
    game.overworld.state.player.y = 20;
    game
}

fn press(button: GbButton) -> InputState {
    let mut input = InputState::new();
    input.press(button);
    input
}

#[test]
fn rival_team_uses_saved_starter_after_lead_changes() {
    use pokered_data::species::Species;
    let mut game = game_at_overworld();
    game.save_data.game_data.player_starter = Species::Bulbasaur as u8;
    game.save_data
        .party
        .add(
            pokered_core::pokemon::stats::create_pokemon(Species::Pikachu, 30, [255, 255]).unwrap(),
        )
        .unwrap();
    game.start_trainer_battle("OPP_RIVAL2", Some(3));
    let battle = game.battle.battle_state.as_ref().unwrap();
    assert_eq!(
        battle.enemy.party.last().unwrap().species,
        Species::Charmeleon
    );
    assert_eq!(battle.enemy.party.len(), 5);
}

#[test]
fn pp_items_apply_selected_slot_through_game_flow() {
    use pokered_data::{items::ItemId, species::Species};
    for item in [ItemId::Ether, ItemId::MaxEther, ItemId::PpUp] {
        let mut game = game_at_overworld();
        let mut mon =
            pokered_core::pokemon::stats::create_pokemon(Species::Venusaur, 50, [255, 255])
                .unwrap();
        mon.pp[1] = 0;
        let first = mon.pp[0];
        game.save_data.party.add(mon).unwrap();
        game.save_data.game_data.bag.add_item(item, 1).unwrap();
        game.handle_transition(GameScreen::Bag);
        for button in [
            GbButton::A,
            GbButton::A,
            GbButton::A,
            GbButton::Down,
            GbButton::A,
        ] {
            game.update(&press(button));
        }
        assert!(
            game.save_data.party.get(0).unwrap().pp[1] > 0,
            "{item:?} must affect selected slot"
        );
        assert_eq!(game.save_data.party.get(0).unwrap().pp[0], first);
        assert_eq!(game.save_data.game_data.bag.item_quantity(item), 0);
    }
}

#[test]
fn new_game_clears_loaded_party() {
    let mut game = game_at_overworld();
    let mon = pokered_core::pokemon::stats::create_pokemon(
        pokered_data::species::Species::Bulbasaur,
        7,
        [0x9a, 0x78],
    )
    .unwrap();
    game.save_data.party.add(mon).unwrap();
    game.handle_transition(GameScreen::OakSpeech);
    assert!(game.save_data.party.is_empty());
}

#[test]
fn bag_field_item_use_returns_to_overworld_without_rebuild() {
    let mut game = game_at_overworld();
    let _ = game
        .save_data
        .game_data
        .bag
        .add_item(pokered_data::items::ItemId::Bicycle, 1);
    game.handle_transition(GameScreen::Bag);
    assert_eq!(game.state.screen, GameScreen::Bag);

    // StartMenu_Item directly uses BICYCLE, without USE/TOSS or a second A.
    game.update(&press(GbButton::A));
    assert_eq!(game.state.screen, GameScreen::Overworld);
    // The live overworld must survive (BICYCLE toggles riding in place);
    // the last-saved position is (5,5), so a rebuild would show up here.
    assert_eq!(game.overworld.state.player.x, 30);
    assert_eq!(game.overworld.state.player.y, 20);
    assert_eq!(
        game.overworld.state.current_map,
        pokered_data::maps::MapId::PalletTown
    );
    assert!(
        game.overworld.pending_dialogue.is_some(),
        "field-item result text must be shown on return"
    );
}

#[test]
fn bag_cancel_returns_to_start_menu() {
    let mut game = game_at_overworld();
    let _ = game
        .save_data
        .game_data
        .bag
        .add_item(pokered_data::items::ItemId::Bicycle, 1);
    game.handle_transition(GameScreen::Bag);
    game.update(&press(GbButton::B));
    assert_eq!(game.state.screen, GameScreen::StartMenu);
}

#[test]
fn fly_picks_destination_and_warps_without_overworld_rebuild() {
    let mut game = game_at_overworld();
    // Viridian City is a visited fly destination (a fresh save has none).
    game.save_data
        .game_data
        .mark_town_visited(pokered_data::maps::MapId::ViridianCity);
    game.pending_fly_map = true;
    game.handle_transition(GameScreen::TownMap);
    assert_eq!(
        game.town_map_screen.mode(),
        pokered_core::town_map_screen::TownMapMode::Fly,
        "FLY opens the town map in destination-picker mode"
    );

    // The shared runtime keeps the selected map visible for eight departure
    // frames, then returns to the live overworld without rebuilding it.
    game.update(&press(GbButton::A));
    assert_eq!(game.state.screen, GameScreen::TownMap);
    for _ in 1..pokered_core::overworld::presentation::FLY_DEPARTURE_TOWN_MAP_FRAMES {
        game.update(&InputState::new());
        assert_eq!(game.state.screen, GameScreen::TownMap);
        assert_eq!(game.overworld.state.player.x, 30, "overworld not rebuilt");
    }
    game.update(&InputState::new());
    assert_eq!(game.state.screen, GameScreen::Overworld);
    assert!(
        game.overworld.pending_warp.is_some(),
        "FlyTo must queue the warp on the live overworld"
    );
    assert_eq!(game.overworld.state.player.x, 30, "overworld not rebuilt");

    // Re-open in view mode (bag TOWN MAP item): B closes back to the
    // overworld directly (no pending_fly_map).
    game.town_map_screen = TownMapScreenState::new(game.overworld.state.current_map);
    game.state.screen = GameScreen::TownMap;
    game.update(&press(GbButton::B));
    assert_eq!(game.state.screen, GameScreen::Overworld);
}

// ── Slots / Elevator / FilterBag / Diploma / soft reset ──────────────
// End-to-end arms: the state machines themselves are tested in
// pokered-core (slots_screen / elevator_screen); these verify the shared runtime
// wiring (coin persistence, screen exit, script-resume delivery).

fn hold_all(buttons: &[GbButton]) -> InputState {
    let mut input = InputState::new();
    for b in buttons {
        input.press(*b);
    }
    input
}

#[test]
fn slots_full_lifecycle_persists_coins_and_exits() {
    let mut game = game_at_overworld();
    game.slots_screen = Some(SlotsScreen::new(false, 100, 1));
    game.state.screen = GameScreen::Slots;

    // A: deduct the bet and spin.
    game.update(&press(GbButton::A));
    let slots = game.slots_screen.as_ref().unwrap();
    assert_eq!(
        slots.phase,
        pokered_core::slots_screen::SlotsPhase::Spinning
    );
    assert_eq!(slots.coins, 99);
    assert_eq!(
        game.save_data.game_data.player_coins, 99,
        "running coin balance must persist to the save every frame"
    );

    // Keep A held: warm-up runs, each reel stops when allowed, a win pays
    // out one coin at a time, then all is done → Result.
    for _ in 0..20000 {
        game.update(&press(GbButton::A));
        let phase = game.slots_screen.as_ref().unwrap().phase;
        if phase != pokered_core::slots_screen::SlotsPhase::Spinning
            && phase != pokered_core::slots_screen::SlotsPhase::Payout
        {
            break;
        }
    }
    let slots = game.slots_screen.as_ref().unwrap();
    assert_eq!(slots.phase, pokered_core::slots_screen::SlotsPhase::Result);
    assert!(slots.reels_stopped.iter().all(|&s| s));
    assert_eq!(slots.payout_remaining, 0);
    assert_eq!(game.save_data.game_data.player_coins, slots.coins);

    // A on the result screen → bet selection again; B → exit to the
    // overworld (the coin balance is already persisted).
    game.update(&press(GbButton::A));
    assert_eq!(
        game.slots_screen.as_ref().unwrap().phase,
        pokered_core::slots_screen::SlotsPhase::BetSelect
    );
    game.update(&press(GbButton::B));
    assert_eq!(game.state.screen, GameScreen::Overworld);
    assert!(game.slots_screen.is_none());
}

#[test]
fn elevator_selects_floor_and_returns_to_overworld() {
    let mut game = game_at_overworld();
    game.elevator_screen = Some(ElevatorScreen::new(vec![
        "1F".into(),
        "2F".into(),
        "3F".into(),
    ]));
    game.state.screen = GameScreen::Elevator;

    // Down, Down → 3F (index 2); A confirms → script resumed + overworld.
    game.update(&press(GbButton::Down));
    game.update(&press(GbButton::Down));
    game.update(&press(GbButton::A));
    assert_eq!(game.state.screen, GameScreen::Overworld);
    assert!(game.elevator_screen.is_none());
}

#[test]
fn elevator_b_cancels_back_to_overworld() {
    let mut game = game_at_overworld();
    game.elevator_screen = Some(ElevatorScreen::new(vec!["1F".into()]));
    game.state.screen = GameScreen::Elevator;
    game.update(&press(GbButton::B));
    assert_eq!(game.state.screen, GameScreen::Overworld);
    assert!(game.elevator_screen.is_none());
}

#[test]
fn filter_bag_select_returns_to_overworld_with_item() {
    let mut game = game_at_overworld();
    game.elevator_screen = Some(ElevatorScreen::new(vec![
        "FRESH WATER".into(),
        "SODA POP".into(),
    ]));
    game.state.screen = GameScreen::FilterBag;

    // Down → SODA POP; A confirms → script resumed with the item name.
    game.update(&press(GbButton::Down));
    game.update(&press(GbButton::A));
    assert_eq!(game.state.screen, GameScreen::Overworld);
    assert!(game.elevator_screen.is_none());
}

#[test]
fn diploma_a_closes_to_overworld() {
    let mut game = game_at_overworld();
    game.state.screen = GameScreen::Diploma;
    game.update(&press(GbButton::A));
    assert_eq!(game.state.screen, GameScreen::Overworld);
}

#[test]
fn soft_reset_needs_16_frame_hold_then_title() {
    let mut game = game_at_overworld();
    let combo = hold_all(&[GbButton::A, GbButton::B, GbButton::Start, GbButton::Select]);

    // 15 frames of the combo: not reset yet. (The Start press opens the
    // START menu during the hold — the app behaves identically; the
    // soft-reset check only returns once the 16-frame threshold hits.)
    for _ in 0..15 {
        game.update(&combo);
    }
    assert_ne!(
        game.state.screen,
        GameScreen::TitleScreen,
        "a short hold must not soft reset"
    );
    assert_eq!(game.soft_reset_frames, 15);
    // The 16th frame triggers the reset → title screen.
    game.update(&combo);
    assert_eq!(
        game.state.screen,
        GameScreen::TitleScreen,
        "a 16-frame hold soft-resets to the title"
    );
    assert_eq!(
        game.soft_reset_frames, 0,
        "the hold counter clears on reset"
    );

    // Releasing the combo resets the hold counter: a fresh 15-frame hold
    // after a release must not reset again.
    let mut game2 = game_at_overworld();
    let released = InputState::new();
    game2.update(&released);
    for _ in 0..15 {
        game2.update(&combo);
    }
    assert_eq!(game2.soft_reset_frames, 15);
    assert_ne!(game2.state.screen, GameScreen::TitleScreen);
}
// ── Language pipeline sync ───────────────────────────────────────────
// The LanguageSelect screen re-syncs the overworld script engine and the
// battle message language (shared by all frontends); the boot flow now stops
// there instead of skipping it.

#[test]
fn language_select_toggles_language_and_syncs_pipelines() {
    let mut game = PokemonGame::new(pokered_core::data::wild_data::GameVersion::Red);
    // The boot flow must STOP at LanguageSelect now (splash exit is no
    // longer remapped to the intro): run the ~500-frame splash out.
    for _ in 0..600 {
        game.update(&InputState::new());
        if game.state.screen == GameScreen::LanguageSelect {
            break;
        }
    }
    assert_eq!(
        game.state.screen,
        GameScreen::LanguageSelect,
        "boot flow must land on the language-select screen"
    );
    assert_eq!(game.state.config.language, Lang::En);

    // Up toggles En → Zh and re-syncs the script engine + battle messages.
    game.update(&press(GbButton::Up));
    assert_eq!(game.state.config.language, Lang::Zh);
    assert_eq!(game.state.screen, GameScreen::LanguageSelect);
    assert_eq!(game.overworld.script_lang(), Some("zh"));
    assert!(game.battle.is_zh);

    // A confirms → intro scene; the choice persists.
    game.update(&press(GbButton::A));
    assert_eq!(game.state.screen, GameScreen::IntroScene);
    assert_eq!(game.overworld.script_lang(), Some("zh"));
    assert!(game.battle.is_zh);

    // Toggling back must restore English everywhere too.
    game.state.screen = GameScreen::LanguageSelect;
    game.update(&press(GbButton::Down));
    assert_eq!(game.state.config.language, Lang::En);
    assert_eq!(game.overworld.script_lang(), Some("en"));
    assert!(!game.battle.is_zh);
}

#[test]
fn overworld_rebuilds_keep_script_lang_in_sync() {
    let mut game = game_at_overworld();
    game.state.config.language = Lang::Zh;

    // Continue path: re-entering the overworld from the main menu
    // rebuilds the overworld; the script engine must come up Chinese.
    game.main_menu.last_choice = Some(MainMenuChoice::Continue);
    game.state.screen = GameScreen::MainMenu;
    game.handle_transition(GameScreen::Overworld);
    assert_eq!(game.overworld.script_lang(), Some("zh"));

    // NewGame path likewise.
    game.main_menu.last_choice = Some(MainMenuChoice::NewGame);
    game.state.screen = GameScreen::MainMenu;
    game.handle_transition(GameScreen::Overworld);
    assert_eq!(game.overworld.script_lang(), Some("zh"));

    // Default construction stays English.
    let fresh = PokemonGame::new(pokered_core::data::wild_data::GameVersion::Red);
    assert_eq!(fresh.overworld.script_lang(), Some("en"));
}

#[test]
fn new_game_seeds_toggleable_object_flags() {
    use pokered_data::toggleable_objects::{is_object_hidden, toggle_id_to_bit_index};
    let mut game = PokemonGame::new(pokered_core::data::wild_data::GameVersion::Red);
    game.main_menu.last_choice = Some(MainMenuChoice::NewGame);
    game.state.screen = GameScreen::MainMenu;
    game.handle_transition(GameScreen::OakSpeech); // resets the in-memory save
    game.handle_transition(GameScreen::Overworld);
    // InitializeToggleableObjectsFlags: PALLET_TOWN Oak starts hidden;
    // unseeded all-zero flags re-showed him on the first warp.
    let bit = toggle_id_to_bit_index("PALLET_TOWN_OBJ_1").unwrap();
    assert!(
        is_object_hidden(game.overworld.toggleable_object_flags(), bit),
        "NEW GAME must seed the fresh save's toggleable object flags into the overworld"
    );
}

#[test]
fn battle_starts_carry_language() {
    let mut game = game_at_overworld();

    game.state.config.language = Lang::Zh;
    game.start_wild_battle(pokered_data::species::Species::Rattata, 5);
    assert!(game.battle.is_zh, "wild battle must localize messages");

    game.start_trainer_battle("youngster1", None);
    assert!(game.battle.is_zh, "trainer battle must localize messages");

    game.state.config.language = Lang::En;
    game.start_wild_battle(pokered_data::species::Species::Rattata, 5);
    assert!(!game.battle.is_zh);
}

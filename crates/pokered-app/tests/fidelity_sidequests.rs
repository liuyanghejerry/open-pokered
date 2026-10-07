//! Real shared-runtime input tests for fidelity audit items 13–17.
use dotzuki_app::InputState;
use dotzuki_renderer::input::GbButton;
use pokered_app::PokemonGame;
use pokered_core::{
    game_state::{GameScreen, Lang},
    overworld::{Direction, OverworldScreen},
    pokemon::stats::create_pokemon,
    save::SaveData,
};
use pokered_data::{
    impl_traits::PokemonRedData, maps::MapId, species::Species, wild_data::GameVersion,
};

fn game(map: MapId, x: u16, y: u16) -> PokemonGame {
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
    game.state.config.language = Lang::En;
    game.overworld = OverworldScreen::new(map, None, PokemonRedData);
    game.overworld.state.player.x = x;
    game.overworld.state.player.y = y;
    game.overworld.state.player.facing = Direction::Up;
    idle(&mut game, 2);
    game
}
fn idle(g: &mut PokemonGame, frames: usize) {
    for _ in 0..frames {
        g.update(&InputState::new());
    }
}
fn tap(g: &mut PokemonGame, button: GbButton) {
    let mut input = InputState::new();
    input.press(button);
    g.update(&input);
    idle(g, 1);
}
fn until(g: &mut PokemonGame, predicate: impl Fn(&PokemonGame) -> bool) {
    for _ in 0..2000 {
        if predicate(g) {
            return;
        }
        if g.overworld.pending_dialogue.is_some() {
            tap(g, GbButton::A);
        } else {
            idle(g, 1);
        }
    }
    panic!(
        "flow stalled: {:?}",
        g.overworld.active_script_effect_value()
    );
}

#[test]
fn name_rater_rejects_foreign_ot_but_allows_own_mon_even_if_traded_marker_is_stale() {
    for (id_matches, name_matches) in [(true, true), (false, true), (true, false), (false, false)] {
        let mut g = game(MapId::NameRatersHouse, 5, 4);
        g.save_data.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        g.save_data.game_data.player_id = 42;
        let mut mon = create_pokemon(Species::MrMime, 20, [0, 0]).unwrap();
        mon.ot_id = if id_matches { 42 } else { 43 };
        mon.ot_name.fill(0x50);
        let ot = pokered_data::charmap::encode_string(if name_matches { "RED" } else { "BLUE" })
            .unwrap();
        mon.ot_name[..ot.len()].copy_from_slice(&ot);
        mon.is_traded = true; // strict OT fields, not the cached EXP marker
        g.save_data.party.add(mon).unwrap();
        idle(&mut g, 2);
        tap(&mut g, GbButton::A);
        until(&mut g, |g| g.overworld.pending_choice.is_some());
        tap(&mut g, GbButton::A);
        until(&mut g, |g| g.overworld.pending_party_select.is_some());
        tap(&mut g, GbButton::A);
        until(&mut g, |g| {
            g.state.screen == GameScreen::Overworld && g.overworld.pending_dialogue.is_some()
        });
        let mut saw_impeccable = false;
        for _ in 0..1200 {
            if let Some(d) = &g.overworld.pending_dialogue {
                saw_impeccable |= d
                    .pages()
                    .iter()
                    .any(|p| p.line1.contains("impeccable") || p.line2.contains("impeccable"));
            }
            if g.overworld.pending_choice.is_some() {
                break;
            }
            if g.overworld.active_script_effect_label().is_none() {
                break;
            }
            tap(&mut g, GbButton::A);
        }
        let own = id_matches && name_matches;
        assert_eq!(g.overworld.pending_choice.is_some(), own);
        assert_eq!(saw_impeccable, !own);
        assert!(!g.overworld.is_naming_screen_active());
    }
}

#[test]
fn coin_counter_and_gifts_preserve_original_saturation_in_shared_runtime() {
    use pokered_data::items::ItemId;
    for (npc, coins, expected, money_after) in [
        (2, 9950, 9999, 4000),
        (2, 9989, 9999, 4000),
        (2, 9990, 9990, 5000),
        (9, 9989, 9999, 5000),
        (9, 9990, 9990, 5000),
        (10, 9990, 9990, 5000),
        (10, 9991, 9999, 5000),
        (10, 9999, 9999, 5000),
    ] {
        // Stationary NPC positions are read from the actual map table.
        let (x, y) = match npc {
            2 => (5, 7),
            9 => (14, 12),
            10 => (17, 14),
            _ => unreachable!(),
        };
        let mut g = game(MapId::GameCorner, x, y);
        g.save_data.game_data.player_coins = coins;
        g.save_data.game_data.player_money = 5000;
        g.save_data
            .game_data
            .bag
            .add_item(ItemId::CoinCase, 1)
            .unwrap();
        tap(&mut g, GbButton::A);
        if npc == 2 {
            until(&mut g, |g| g.overworld.pending_choice.is_some());
            tap(&mut g, GbButton::A);
        }
        until(&mut g, |g| {
            g.overworld.active_script_effect_label().is_none()
        });
        idle(&mut g, 2);
        assert_eq!(
            g.save_data.game_data.player_coins, expected,
            "npc {npc}, coins {coins}"
        );
        assert_eq!(g.save_data.game_data.player_money, money_after);
        let bytes = pokered_core::save::sram_export::export_sram(&g.save_data);
        let loaded = pokered_core::save::sram_import::import_sram(&bytes).unwrap();
        assert_eq!(loaded.game_data.player_coins, expected);
    }
}

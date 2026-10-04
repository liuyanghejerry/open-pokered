//! Seeded native-menu regression only, never Jev collection credit.
//! The original Clerk 1 rejects purchases at >=9990, not >9949; the final
//! 50-coin purchase may saturate to 9999 and costs the full 1000 yen.

use dotzuki_app::InputState;
use dotzuki_engine::render_config::RenderConfig;
use dotzuki_renderer::input::GbButton;
use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::{GameScreen, MainMenuChoice};
use pokered_core::overworld::{Direction, OverworldScreen};
use pokered_core::pokemon::stats::create_pokemon;
use pokered_data::{impl_traits::PokemonRedData, items::ItemId, maps::MapId, species::Species};
use pokered_renderer::{FrameBuffer, Rgba};

fn fixture(coins: u16, money: u32, coin_case: bool) -> PokemonGame {
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
    game.state.screen = GameScreen::Overworld;
    game.main_menu.last_choice = Some(MainMenuChoice::Continue);
    game.overworld = OverworldScreen::new(MapId::GameCorner, None, PokemonRedData);
    game.overworld.state.player.x = 5;
    game.overworld.state.player.y = 8;
    game.overworld.state.player.facing = Direction::Up;
    game.save_data.game_data.player_coins = coins;
    game.save_data.game_data.player_money = money;
    if coin_case {
        game.save_data
            .game_data
            .bag
            .add_item(ItemId::CoinCase, 1)
            .unwrap();
    }
    game.save_data
        .party
        .add(create_pokemon(Species::Charmander, 5, [0x88, 0x88]).unwrap())
        .unwrap();
    game.save_data
        .game_data
        .pokedex
        .set_owned(Species::Charmander);
    for _ in 0..40 {
        game.update(&InputState::new());
    }
    game
}

fn tap(game: &mut PokemonGame, button: GbButton) {
    let mut input = InputState::new();
    input.press(button);
    game.update(&input);
    game.update(&InputState::new());
}

fn open_choice(game: &mut PokemonGame) {
    for _ in 0..400 {
        if game.overworld.pending_choice.is_some() {
            return;
        }
        tap(game, GbButton::A);
    }
    panic!("native coin/prize menu did not open");
}

fn choose(game: &mut PokemonGame, index: usize) {
    for _ in 0..index {
        tap(game, GbButton::Down);
    }
    tap(game, GbButton::A);
    // Reveal the result with neutral frames; do not dismiss the evidence text.
    for _ in 0..180 {
        game.update(&InputState::new());
    }
}

fn capture(game: &mut PokemonGame, name: &str) {
    if let Ok(dir) = std::env::var("GAME_CORNER_CAPTURE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        game.draw(&mut fb);
        fb.save_png(&std::path::Path::new(&dir).join(format!("{name}.png")))
            .unwrap();
    }
}

fn buy_case(coins: u16, money: u32, coin_case: bool, choice: usize, accepted: bool) {
    let mut game = fixture(coins, money, coin_case);
    open_choice(&mut game);
    choose(&mut game, choice);
    if coins == 9950 && money == 1000 && coin_case && choice == 0 {
        capture(&mut game, "jev-dex-coin-clerk-9950");
    }
    eprintln!("coin boundary: coins={coins}, money={money}, case={coin_case}, choice={choice}; frame={}, native coins={}, money={}",
        game.frame_count, game.save_data.game_data.player_coins, game.save_data.game_data.player_money);
    assert_eq!(
        game.save_data.game_data.player_coins,
        if accepted {
            coins.saturating_add(50).min(9999)
        } else {
            coins
        }
    );
    assert_eq!(
        game.save_data.game_data.player_money,
        if accepted { money - 1000 } else { money }
    );
    assert_eq!(game.save_data.party.count(), 1);
    assert!(!game.save_data.game_data.pokedex.is_owned(Species::Porygon));
}

#[test]
fn coin_clerk_9949_accepts_full_fifty() {
    buy_case(9949, 1000, true, 0, true);
}

#[test]
fn coin_clerk_9950_accepts_final_purchase() {
    buy_case(9950, 1000, true, 0, true);
}

#[test]
fn coin_clerk_9989_accepts_and_saturates() {
    buy_case(9989, 1000, true, 0, true);
}

#[test]
fn coin_clerk_9990_and_9999_refuse_without_charge() {
    for coins in [9990, 9999] {
        buy_case(coins, 1000, true, 0, false);
    }
}

#[test]
fn coin_clerk_missing_case_money_and_no_choice_preserve_resources() {
    buy_case(9950, 1000, false, 0, false);
    buy_case(9950, 999, true, 0, false);
    buy_case(9950, 1000, true, 1, false);
}

fn finish_dialogue(game: &mut PokemonGame) {
    for _ in 0..400 {
        if game.overworld.pending_dialogue.is_none() && game.overworld.script_engine_idle() {
            return;
        }
        tap(game, GbButton::A);
    }
    panic!("native result dialogue did not settle");
}

#[test]
fn coin_clerk_zero_balance_reaches_9999_through_200_paid_menus() {
    let mut game = fixture(0, 200000, true);
    for purchase in 1..=200u16 {
        open_choice(&mut game);
        choose(&mut game, 0);
        assert_eq!(
            game.save_data.game_data.player_coins,
            (purchase * 50).min(9999)
        );
        assert_eq!(
            game.save_data.game_data.player_money,
            200000 - u32::from(purchase) * 1000
        );
        finish_dialogue(&mut game);
    }
    assert_eq!(game.save_data.party.count(), 1);
    assert!(!game.save_data.game_data.pokedex.is_owned(Species::Porygon));
}

#[test]
fn final_coin_purchase_then_native_red_porygon_prize_receipt() {
    let mut game = fixture(9950, 1000, true);
    open_choice(&mut game);
    choose(&mut game, 0);
    assert_eq!(game.save_data.game_data.player_coins, 9999);
    assert_eq!(game.save_data.game_data.player_money, 0);
    finish_dialogue(&mut game);
    // Isolated test setup at the actual Red prize-vendor sign, then real menu
    // input/payment/delivery. No seeded Porygon, model call or formal-run write.
    game.overworld = OverworldScreen::new(MapId::GameCornerPrizeRoom, None, PokemonRedData);
    game.overworld.state.player.x = 4;
    game.overworld.state.player.y = 3;
    game.overworld.state.player.facing = Direction::Up;
    for _ in 0..40 {
        game.update(&InputState::new());
    }
    open_choice(&mut game);
    choose(&mut game, 2);
    for _ in 0..400 {
        if game.save_data.party.count() == 2 {
            break;
        }
        tap(&mut game, GbButton::A);
    }
    assert_eq!(game.save_data.game_data.player_coins, 0);
    assert_eq!(game.save_data.game_data.player_money, 0);
    assert_eq!(game.save_data.party.count(), 2);
    let mon = game.save_data.party.get(1).unwrap();
    assert_eq!(mon.species, Species::Porygon);
    assert_eq!(mon.level, 26);
    assert!(game.save_data.game_data.pokedex.is_owned(Species::Porygon));
}

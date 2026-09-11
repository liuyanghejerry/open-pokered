//! Capture populated menus: cargo run --release -p pokered-app --example ui_audit_capture -- <directory> [en]

use dotzuki_app::InputState;
use dotzuki_engine::render_config::RenderConfig;
use dotzuki_renderer::input::GbButton;
use pokered_app::{tools::apply_lang, PokemonGame};
use pokered_core::{
    data::wild_data::GameVersion,
    game_state::{GameScreen, Lang},
    items::shop::{MartState, ShopInventory},
    pokedex_screen::PokedexScreenInput,
    pokemon::stats::create_pokemon,
};
use pokered_data::{items::ItemId, species::Species};
use pokered_renderer::{FrameBuffer, Rgba};
fn shot(g: &mut PokemonGame, dir: &str, name: &str) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    g.draw(&mut fb);
    fb.save_png(std::path::Path::new(&format!("{dir}/{name}.png")))
        .unwrap();
}
fn press(g: &mut PokemonGame, b: GbButton) {
    let mut i = InputState::new();
    i.press(b);
    g.update(&i);
    i.release(b);
    g.update(&i);
}
fn main() {
    let dir = std::env::args().nth(1).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let mut g = PokemonGame::new(GameVersion::Red);
    apply_lang(
        &mut g,
        if std::env::args().nth(2).as_deref() == Some("en") {
            Lang::En
        } else {
            Lang::Zh
        },
    );
    g.overworld.state.player.x = 5;
    g.overworld.state.player.y = 6;
    g.frame_count = 10;
    for s in [
        Species::Venusaur,
        Species::Charizard,
        Species::Blastoise,
        Species::Pikachu,
        Species::Snorlax,
        Species::Chansey,
    ] {
        g.save_data
            .party
            .add(create_pokemon(s, 100, [255, 255]).unwrap())
            .unwrap();
        g.save_data.game_data.pokedex.set_seen(s);
        g.save_data.game_data.pokedex.set_owned(s);
    }
    for id in [
        ItemId::SuperPotion,
        ItemId::FullRestore,
        ItemId::MaxRepel,
        ItemId::PokeBall,
    ] {
        g.save_data.game_data.bag.add_item(id, 99).unwrap();
    }
    g.save_data.game_data.player_money = 999999;
    for (screen, name) in [
        (GameScreen::PartyScreen, "party"),
        (GameScreen::Bag, "bag"),
        (GameScreen::TrainerCard, "trainer-card"),
        (GameScreen::TownMap, "town-map"),
        (GameScreen::SaveMenu, "save"),
        (GameScreen::StartMenu, "start-menu"),
        (GameScreen::OptionsMenu, "options"),
    ] {
        g.handle_transition(screen);
        shot(&mut g, &dir, name);
    }
    g.handle_transition(GameScreen::PartyScreen);
    press(&mut g, GbButton::A);
    shot(&mut g, &dir, "party-actions");
    g.handle_transition(GameScreen::Bag);
    press(&mut g, GbButton::A);
    shot(&mut g, &dir, "bag-actions");
    press(&mut g, GbButton::Down);
    press(&mut g, GbButton::A);
    shot(&mut g, &dir, "bag-quantity");
    g.handle_transition(GameScreen::Pokedex);
    shot(&mut g, &dir, "dex-list");
    g.pokedex_screen.update_frame(PokedexScreenInput {
        down: true,
        ..Default::default()
    });
    g.pokedex_screen.update_frame(PokedexScreenInput {
        down: true,
        ..Default::default()
    });
    g.pokedex_screen.update_frame(PokedexScreenInput {
        a: true,
        ..Default::default()
    });
    shot(&mut g, &dir, "dex-menu");
    for (s, n) in [
        (Species::Venusaur, "dex-venusaur"),
        (Species::Charizard, "dex-charizard"),
        (Species::Snorlax, "dex-snorlax"),
    ] {
        g.debug_open_pokedex(s);
        shot(&mut g, &dir, n);
    }
    g.handle_transition(GameScreen::Shop(MartState::new(ShopInventory::new(vec![
        ItemId::SuperPotion,
        ItemId::FullRestore,
        ItemId::MaxRepel,
        ItemId::PokeBall,
    ]))));
    shot(&mut g, &dir, "mart-menu");
    press(&mut g, GbButton::A);
    shot(&mut g, &dir, "mart-buy");
    press(&mut g, GbButton::A);
    shot(&mut g, &dir, "mart-quantity");
    press(&mut g, GbButton::A);
    shot(&mut g, &dir, "mart-confirm");
    g.debug_start_wild_battle(Species::Charizard, 100);
    for _ in 0..1000 {
        if matches!(
            g.battle.phase,
            pokered_core::battle::BattlePhase::PlayerMenu
        ) {
            break;
        }
        press(&mut g, GbButton::A);
    }
    assert!(matches!(
        g.battle.phase,
        pokered_core::battle::BattlePhase::PlayerMenu
    ));
    shot(&mut g, &dir, "battle-menu");
    press(&mut g, GbButton::A);
    shot(&mut g, &dir, "battle-moves");

    // Additional edge-state checks, captured after the comparison sequence.
    g.handle_transition(GameScreen::MainMenu);
    shot(&mut g, &dir, "main");
    g.handle_transition(GameScreen::SaveMenu);
    g.save_menu.cursor = pokered_core::save_menu::YesNoChoice::No;
    shot(&mut g, &dir, "save-no");
    g.handle_transition(GameScreen::OptionsMenu);
    for (n, row, speed, anim, style) in [
        (
            "options-fast",
            pokered_core::options_menu::OptionsRow::TextSpeed,
            pokered_core::options_menu::TextSpeed::Fast,
            pokered_core::options_menu::BattleAnimation::On,
            pokered_core::options_menu::BattleStyle::Shift,
        ),
        (
            "options-slow",
            pokered_core::options_menu::OptionsRow::TextSpeed,
            pokered_core::options_menu::TextSpeed::Slow,
            pokered_core::options_menu::BattleAnimation::On,
            pokered_core::options_menu::BattleStyle::Shift,
        ),
        (
            "options-off",
            pokered_core::options_menu::OptionsRow::BattleAnimation,
            pokered_core::options_menu::TextSpeed::Medium,
            pokered_core::options_menu::BattleAnimation::Off,
            pokered_core::options_menu::BattleStyle::Shift,
        ),
        (
            "options-set",
            pokered_core::options_menu::OptionsRow::BattleStyle,
            pokered_core::options_menu::TextSpeed::Medium,
            pokered_core::options_menu::BattleAnimation::On,
            pokered_core::options_menu::BattleStyle::Set,
        ),
    ] {
        g.options_menu.row = row;
        g.options_menu.options.text_speed = speed;
        g.options_menu.options.battle_animation = anim;
        g.options_menu.options.battle_style = style;
        shot(&mut g, &dir, n);
    }
    g.handle_transition(GameScreen::Shop(MartState::new(ShopInventory::new(vec![
        ItemId::FullRestore,
    ]))));
    press(&mut g, GbButton::A);
    press(&mut g, GbButton::A);
    for _ in 0..98 {
        press(&mut g, GbButton::Up);
    }
    shot(&mut g, &dir, "mart-quantity-99");
    press(&mut g, GbButton::A);
    press(&mut g, GbButton::Down);
    shot(&mut g, &dir, "mart-confirm-no");
}

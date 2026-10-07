//! Run unchanged on master and the fix branch: CAPTURE_SIDE=before/after,
//! CAPTURE_DIR=<absolute path> cargo test -p pokered-app --test fidelity_story_capture -- --ignored.
use dotzuki_app::InputState;
use dotzuki_engine::render_config::RenderConfig;
use dotzuki_renderer::input::GbButton;
use pokered_app::{render::draw_overworld, PokemonGame};
use pokered_core::{
    game_state::{GameScreen, Lang},
    overworld::{Direction, OverworldScreen},
    save::SaveData,
};
use pokered_data::{
    impl_traits::PokemonRedData, maps::MapId, species::Species, wild_data::GameVersion,
};
use pokered_renderer::{
    resource::{AssetRoot, ResourceManager},
    FrameBuffer, Rgba,
};
fn idle(g: &mut PokemonGame, n: usize) {
    for _ in 0..n {
        g.update(&InputState::new());
    }
}
fn tap(g: &mut PokemonGame, b: GbButton) {
    let mut i = InputState::new();
    i.press(b);
    g.update(&i);
    idle(g, 1);
}
fn game(map: MapId, x: u16, y: u16) -> PokemonGame {
    let mut g = PokemonGame::new_with_options(
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
    g.save_data = SaveData::new();
    g.state.screen = GameScreen::Overworld;
    g.state.config.language = Lang::En;
    g.overworld = OverworldScreen::new(map, None, PokemonRedData);
    g.overworld.state.player.x = x;
    g.overworld.state.player.y = y;
    g.overworld.state.player.facing = Direction::Up;
    g.overworld.set_rng_seed(42);
    idle(&mut g, 2);
    g
}
fn close_dialogue(g: &mut PokemonGame) {
    for _ in 0..1500 {
        if g.overworld.pending_dialogue.is_none() {
            return;
        }
        tap(g, GbButton::A);
    }
    panic!("dialogue stalled");
}
fn menu(g: &mut PokemonGame) {
    tap(g, GbButton::A);
    for _ in 0..1500 {
        if g.overworld.pending_choice.is_some() {
            return;
        }
        if g.overworld.pending_dialogue.is_some() {
            tap(g, GbButton::A);
        } else {
            idle(g, 1);
        }
    }
    panic!("no menu");
}
fn shot(g: &mut PokemonGame, name: &str) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut rm = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
    draw_overworld(&mut g.overworld, &mut rm, &mut fb, Lang::En);
    let mut png = image::RgbaImage::new(160, 144);
    for y in 0..144 {
        for x in 0..160 {
            png.put_pixel(x, y, image::Rgba(fb.get_pixel(x, y).unwrap().to_array()));
        }
    }
    let dir = std::path::PathBuf::from(std::env::var("CAPTURE_DIR").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let side = std::env::var("CAPTURE_SIDE").unwrap();
    png.save(dir.join(format!("{name}-{side}.png"))).unwrap();
    eprintln!(
        "capture {name}-{side}: map={:?}, pos=({},{}), effect={:?}",
        g.overworld.state.current_map,
        g.overworld.state.player.x,
        g.overworld.state.player.y,
        g.overworld.active_script_effect_value()
    );
}
#[test]
#[ignore = "explicit before/after evidence capture"]
fn capture_audit_8_to_12() {
    let mut g = game(MapId::CeladonMansionRoofHouse, 4, 4);
    tap(&mut g, GbButton::A);
    idle(&mut g, 400);
    tap(&mut g, GbButton::A);
    idle(&mut g, 100);
    shot(&mut g, "gift-nickname");

    let mut g = game(MapId::Museum1F, 13, 5);
    let mut i = InputState::new();
    i.press(GbButton::Up);
    for _ in 0..8 {
        g.update(&i);
    }
    idle(&mut g, 40);
    shot(&mut g, "museum-back-counter");

    for (map, name) in [
        (MapId::ViridianSchoolHouse, "school-reading"),
        (MapId::CeladonMansionRoofHouse, "link-reading"),
    ] {
        let mut g = game(map, 3, 1);
        menu(&mut g);
        tap(&mut g, GbButton::A);
        idle(&mut g, 5);
        close_dialogue(&mut g);
        idle(&mut g, 10);
        shot(&mut g, name);
    }
    let mut g = game(MapId::CeladonMartRoof, 10, 2);
    g.save_data.game_data.player_money = 3000;
    menu(&mut g);
    shot(&mut g, "vending-menu");

    let mut g = game(MapId::OaksLab, 4, 2);
    g.overworld.state.player.facing = Direction::Right;
    for f in [
        "EVENT_GOT_POKEDEX",
        "EVENT_GOT_STARTER",
        "EVENT_BATTLED_RIVAL_IN_OAKS_LAB",
        "EVENT_PALLET_AFTER_GETTING_POKEBALLS",
    ] {
        g.overworld.set_flag_live(f, true);
    }
    for sp in [Species::Bulbasaur, Species::Charmander] {
        g.save_data.game_data.pokedex.set_seen(sp);
        g.save_data.game_data.pokedex.set_owned(sp);
    }
    for n in &mut g.overworld.npc_states {
        if n.text_id == 5 {
            n.visible = true;
        }
    }
    tap(&mut g, GbButton::A);
    for _ in 0..3 {
        idle(&mut g, 5);
        close_dialogue(&mut g);
    }
    idle(&mut g, 80);
    shot(&mut g, "oak-rating");
}

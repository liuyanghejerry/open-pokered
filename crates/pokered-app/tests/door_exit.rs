//! Actual doorway inputs through the shared Game, starting from controlled maps.
use dotzuki_app::InputState;
use dotzuki_engine::render_config::RenderConfig;
use dotzuki_renderer::input::GbButton;
use pokered_app::{render::draw_overworld, PokemonGame};
use pokered_core::{
    game_state::{GameScreen, Lang},
    overworld::{Direction, OverworldScreen},
    save::SaveData,
};
use pokered_data::{impl_traits::PokemonRedData, maps::MapId, wild_data::GameVersion};
use pokered_renderer::{
    resource::{AssetRoot, ResourceManager},
    FrameBuffer, Rgba,
};

fn game(map: MapId, outside: MapId, x: u16, y: u16) -> PokemonGame {
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
    g.overworld.last_map = Some(outside);
    g.overworld.set_flag_live("EVENT_GOT_STARTER", true);
    g.overworld
        .set_flag_live("EVENT_FOLLOWED_OAK_INTO_LAB", true);
    g.overworld.state.player.x = x;
    g.overworld.state.player.y = y;
    g.overworld.state.player.facing = Direction::Down;
    idle(&mut g, 2);
    g
}

fn idle(g: &mut PokemonGame, frames: usize) {
    for _ in 0..frames {
        g.update(&InputState::new());
    }
}

fn travel(g: &mut PokemonGame, button: GbButton, destination: MapId) {
    let mut input = InputState::new();
    input.press(button);
    for frame in 0..1000 {
        if g.overworld.state.current_map == destination {
            // Release on map change; subsequent movement must be the game's.
            eprintln!(
                "map={destination:?} arrived after {frame} updates at {:?}, effect={:?}",
                position(g),
                g.overworld.active_script_effect_label()
            );
            idle(g, 100);
            return;
        }
        g.update(&input);
    }
    panic!(
        "did not reach {destination:?}: map={:?}, pos={:?}, effect={:?}",
        g.overworld.state.current_map,
        position(g),
        g.overworld.active_script_effect_label()
    );
}

fn position(g: &PokemonGame) -> (u16, u16) {
    (g.overworld.state.player.x, g.overworld.state.player.y)
}

fn capture(g: &mut PokemonGame, name: &str) {
    let Ok(dir) = std::env::var("DOOR_CAPTURE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut resources = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_overworld(&mut g.overworld, &mut resources, &mut fb, Lang::En);
    image::RgbaImage::from_fn(160, 144, |x, y| {
        image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
    })
    .save(format!("{dir}/{name}.png"))
    .unwrap();
}

#[test]
fn outdoor_doors_step_down_after_npc_visibility_loads() {
    let mut failures = Vec::new();
    for (name, map, outside, start, expected, entry) in [
        (
            "gate-left",
            MapId::Route22Gate,
            MapId::Route22,
            (4, 6),
            (8, 6),
            (4, 7),
        ),
        (
            "gate-right",
            MapId::Route22Gate,
            MapId::Route22,
            (5, 6),
            (8, 6),
            (4, 7),
        ),
        (
            "home",
            MapId::RedsHouse1F,
            MapId::PalletTown,
            (2, 6),
            (5, 6),
            (2, 7),
        ),
        (
            "lab",
            MapId::OaksLab,
            MapId::PalletTown,
            (4, 10),
            (12, 12),
            (5, 11),
        ),
        (
            "center",
            MapId::ViridianPokecenter,
            MapId::ViridianCity,
            (3, 6),
            (23, 26),
            (3, 7),
        ),
        (
            "cerulean-champion",
            MapId::CeruleanPokecenter,
            MapId::CeruleanCity,
            (3, 6),
            (19, 18),
            (3, 7),
        ),
        (
            "saffron-rescue",
            MapId::SaffronPokecenter,
            MapId::SaffronCity,
            (3, 6),
            (9, 30),
            (3, 7),
        ),
        (
            "saffron-liberated",
            MapId::SaffronPokecenter,
            MapId::SaffronCity,
            (3, 6),
            (9, 30),
            (3, 7),
        ),
    ] {
        let mut g = game(map, outside, start.0, start.1);
        match name {
            "cerulean-champion" => g
                .overworld
                .set_flag_live("EVENT_HALL_OF_FAME_DEX_RATING", true),
            "saffron-rescue" => g.overworld.set_flag_live("EVENT_RESCUED_MR_FUJI", true),
            "saffron-liberated" => g
                .overworld
                .set_flag_live("EVENT_BEAT_SILPH_CO_GIOVANNI", true),
            _ => {}
        }
        travel(&mut g, GbButton::Down, outside);
        capture(&mut g, name);
        eprintln!("{name}: final={:?}, expected={expected:?}", position(&g));
        if position(&g) != expected {
            failures.push(name);
            continue;
        }
        assert_eq!(g.overworld.state.player.facing, Direction::Down);
        assert!(!g.overworld.state.exiting_door);
        assert!(!g.overworld.state.standing_on_door);
        // Turning back into the real door works without a compensating DOWN.
        travel(&mut g, GbButton::Up, map);
        assert_eq!(position(&g), entry);
    }
    assert!(
        failures.is_empty(),
        "missing automatic door step: {failures:?}"
    );
}

#[test]
fn stairs_do_not_add_a_door_step() {
    let mut g = game(MapId::RedsHouse2F, MapId::PalletTown, 6, 1);
    travel(&mut g, GbButton::Right, MapId::RedsHouse1F);
    assert_eq!(position(&g), (7, 1));
    assert!(!g.overworld.state.standing_on_door);
    assert!(!g.overworld.state.exiting_door);
}

#[test]
fn leaving_the_dock_keeps_the_two_step_departure_script() {
    let mut g = game(MapId::VermilionDock, MapId::VermilionCity, 14, 1);
    g.overworld.set_flag_live("EVENT_SS_ANNE_LEFT", true);
    g.overworld
        .set_flag_live("EVENT_WALKED_PAST_GUARD_AFTER_SS_ANNE_LEFT", false);
    travel(&mut g, GbButton::Up, MapId::VermilionCity);
    assert_eq!(position(&g), (18, 29));
    assert!(g
        .overworld
        .unified_flags()
        .get_flag("EVENT_WALKED_PAST_GUARD_AFTER_SS_ANNE_LEFT"));
    assert!(g.overworld.pending_dialogue.is_none());
    assert!(!g.overworld.state.standing_on_door);
    assert!(!g.overworld.state.exiting_door);
}

//! Controlled map fixtures driven through the shared Game with button input.
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

fn game(x: u16, badge: bool) -> PokemonGame {
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
    g.save_data.game_data.obtained_badges = u8::from(badge);
    g.state.screen = GameScreen::Overworld;
    g.state.config.language = Lang::En;
    g.overworld = OverworldScreen::new(MapId::Route22Gate, None, PokemonRedData);
    g.overworld.set_flag_live("EVENT_BEAT_BROCK", badge);
    g.overworld.state.player.x = x;
    g.overworld.state.player.y = 3;
    g.overworld.state.player.facing = Direction::Up;
    idle(&mut g, 2);
    g
}

fn idle(g: &mut PokemonGame, count: usize) {
    for _ in 0..count {
        g.update(&InputState::new());
    }
}

fn press(g: &mut PokemonGame, button: GbButton, count: usize) {
    let mut input = InputState::new();
    input.press(button);
    for _ in 0..count {
        g.update(&input);
    }
    idle(g, 1);
}

fn capture(g: &mut PokemonGame, name: &str) {
    let Ok(dir) = std::env::var("GATE_CAPTURE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut rm = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_overworld(&mut g.overworld, &mut rm, &mut fb, Lang::En);
    image::RgbaImage::from_fn(160, 144, |x, y| {
        image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
    })
    .save(format!("{dir}/{name}.png"))
    .unwrap();
}

fn finish_dialogue(g: &mut PokemonGame) {
    for _ in 0..2000 {
        if g.overworld.pending_dialogue.is_some() {
            press(g, GbButton::A, 1);
        } else {
            idle(g, 1);
        }
        if g.overworld.script_engine_idle()
            && g.overworld.active_script_effect_label().is_none()
            && g.overworld.pending_dialogue.is_none()
        {
            idle(g, 45);
            return;
        }
    }
    panic!("guard script did not finish");
}

#[test]
fn boulder_badge_crossing_announces_permission_in_both_lanes() {
    let mut missing = Vec::new();
    for x in [4, 5] {
        let mut g = game(x, true);
        press(&mut g, GbButton::Up, 8);
        idle(&mut g, 180);
        assert_eq!(
            (g.overworld.state.player.x, g.overworld.state.player.y),
            (x, 2)
        );
        capture(&mut g, &format!("badge-lane-{x}"));
        let text = g.overworld.pending_dialogue.as_ref().map(|d| {
            d.pages()
                .iter()
                .flat_map(|p| [&p.line1, &p.line2])
                .cloned()
                .collect::<Vec<_>>()
                .join(" ")
        });
        eprintln!("lane={x} dialogue={text:?}");
        if !text.is_some_and(|s| s.contains("BOULDERBADGE")) {
            missing.push(x);
        }
    }
    assert!(
        missing.is_empty(),
        "badge permission missing in lanes {missing:?}"
    );
}

#[test]
fn no_badge_direct_conversation_pushes_the_player_down() {
    let mut g = game(6, false);
    press(&mut g, GbButton::A, 1);
    idle(&mut g, 180);
    assert!(g.overworld.pending_dialogue.is_some());
    finish_dialogue(&mut g);
    capture(&mut g, "refused-direct-talk");
    assert_eq!(
        (g.overworld.state.player.x, g.overworld.state.player.y),
        (6, 4)
    );
    assert_eq!(g.overworld.state.player.facing, Direction::Down);
}

#[test]
fn gate_uses_badge_bits_and_refuses_both_lanes_on_every_attempt() {
    for x in [4, 5] {
        let mut g = game(x, false);
        // An imported save can carry the badge and gym result independently.
        g.overworld.set_flag_live("EVENT_BEAT_BROCK", true);
        for _ in 0..2 {
            press(&mut g, GbButton::Up, 8);
            idle(&mut g, 90);
            assert!(g.overworld.pending_dialogue.is_some());
            finish_dialogue(&mut g);
            assert_eq!(
                (g.overworld.state.player.x, g.overworld.state.player.y),
                (x, 3)
            );
            assert_eq!(g.overworld.state.player.facing, Direction::Down);
        }
    }
    let mut g = game(4, true);
    g.overworld.set_flag_live("EVENT_BEAT_BROCK", false);
    press(&mut g, GbButton::Up, 8);
    idle(&mut g, 90);
    assert!(g.overworld.pending_dialogue.is_some());
    finish_dialogue(&mut g);
    assert_eq!(g.overworld.state.player.y, 2);
}

fn travel(g: &mut PokemonGame, button: GbButton, map: MapId) {
    let mut input = InputState::new();
    input.press(button);
    for _ in 0..1000 {
        if g.overworld.state.current_map == map {
            idle(g, 100);
            eprintln!(
                "arrived {map:?} at ({}, {})",
                g.overworld.state.player.x, g.overworld.state.player.y
            );
            return;
        }
        g.update(&input);
    }
    panic!(
        "map transition did not reach {map:?}: current={:?} pos=({}, {}) effect={:?}",
        g.overworld.state.current_map,
        g.overworld.state.player.x,
        g.overworld.state.player.y,
        g.overworld.active_script_effect_label()
    );
}

#[test]
fn permission_survives_leaving_the_map_but_manual_talk_stays_available() {
    let mut g = game(4, true);
    press(&mut g, GbButton::Up, 8);
    idle(&mut g, 90);
    finish_dialogue(&mut g);
    travel(&mut g, GbButton::Down, MapId::Route22);
    // Release the destination warp tile before turning back into its doorway.
    press(&mut g, GbButton::Down, 8);
    idle(&mut g, 40);
    travel(&mut g, GbButton::Up, MapId::Route22Gate);
    for _ in 0..6 {
        if g.overworld.state.player.y <= 2 {
            break;
        }
        press(&mut g, GbButton::Up, 8);
        idle(&mut g, 40);
        assert!(g.overworld.pending_dialogue.is_none());
    }
    assert_eq!(g.overworld.state.player.y, 2);
    press(&mut g, GbButton::Right, 8);
    idle(&mut g, 40);
    press(&mut g, GbButton::Right, 8);
    press(&mut g, GbButton::A, 1);
    idle(&mut g, 90);
    assert!(g.overworld.pending_dialogue.is_some());
}

#[test]
fn first_manual_permission_plays_the_fanfare_and_persists_in_sram() {
    use pokered_core::save::{game_data::ROUTE22_GATE_SCRIPT_INDEX, sram_import::import_sram};
    let mut g = game(6, true);
    g.audio = Some(pokered_audio::output::AudioOutput::new_pcm());
    press(&mut g, GbButton::A, 1);
    let mut heard = false;
    for i in 0..2000 {
        heard |= g.audio.as_ref().unwrap().is_sfx_playing();
        if g.overworld.script_engine_idle() && g.overworld.pending_dialogue.is_none() {
            break;
        }
        if i % 4 == 0 {
            press(&mut g, GbButton::A, 1);
        } else {
            idle(&mut g, 1);
        }
    }
    assert!(
        heard,
        "permission must play GET_ITEM_1 while the dialogue owns control"
    );
    assert!(g.overworld.script_engine_idle());
    assert_eq!(g.overworld.state.player.y, 3);

    let path =
        std::env::temp_dir().join(format!("pokered-route22-guard-{}.sav", std::process::id()));
    g.save_to_path(&path).unwrap();
    let saved = std::fs::read(&path).unwrap();
    // Original SRAM: sMainData + (wRoute22GateCurScript - wMainDataStart).
    assert_eq!(saved[0x28ba], 2);
    assert_eq!(
        import_sram(&saved).unwrap().game_data.game_progress_flags[ROUTE22_GATE_SCRIPT_INDEX],
        2
    );
    let mut continued = PokemonGame::new_with_options(
        GameVersion::Red,
        Some(path.clone()),
        None,
        None,
        false,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    // Cold app and physical title/menu inputs, with no injected runtime flags.
    for frame in 0..1500 {
        if continued.state.screen == GameScreen::Overworld {
            break;
        }
        match frame % 32 {
            0 => press(&mut continued, GbButton::Start, 1),
            16 => press(&mut continued, GbButton::A, 1),
            _ => idle(&mut continued, 1),
        }
    }
    std::fs::remove_file(&path).unwrap();
    assert_eq!(continued.state.screen, GameScreen::Overworld);
    idle(&mut continued, 90);
    assert_eq!(continued.overworld.state.current_map, MapId::Route22Gate);
    press(&mut continued, GbButton::Left, 8);
    idle(&mut continued, 40);
    press(&mut continued, GbButton::Up, 8);
    idle(&mut continued, 90);
    assert_eq!(
        (
            continued.overworld.state.player.x,
            continued.overworld.state.player.y
        ),
        (5, 2)
    );
    assert!(continued.overworld.pending_dialogue.is_none());
}

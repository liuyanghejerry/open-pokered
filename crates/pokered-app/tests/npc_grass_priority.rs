//! Original NPC grass pixels and cached rendering across the priority boundary.
use dotzuki_engine::render_config::RenderConfig;
use pokered_app::{render::session::RenderSession, PokemonGame};
use pokered_core::game_state::GameScreen;
use pokered_core::overworld::{npc_presentation::NpcPresentation, Direction};
use pokered_data::{maps::MapId, species::Species, wild_data::GameVersion};
use pokered_renderer::{
    input::{GbButton, InputState},
    FrameBuffer, Rgba,
};

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn fixture(walking: bool) -> PokemonGame {
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
    game.set_seed(42);
    game.state.screen = GameScreen::Overworld;
    game.overworld.warp_to_map(MapId::Route1, 12, 22);
    for _ in 0..120 {
        game.update(&InputState::new());
    }
    game.save_data.party.clear();
    game.save_data
        .party
        .add(pokered_core::pokemon::stats::create_pokemon(Species::Mew, 50, [0x99; 2]).unwrap())
        .unwrap();
    game.overworld.party_lead_level = game.save_data.party.leader_level();
    game.overworld.state.repel_steps = 200;
    game.overworld.state.player.facing = Direction::Left;
    let npc = &mut game.overworld.npc_states[usize::from(walking)];
    npc.x = if walking { 15 } else { 14 };
    npc.y = 22;
    npc.facing = if walking {
        Direction::Right
    } else {
        Direction::Down
    };
    npc.step_direction = walking.then_some(Direction::Right);
    npc.walk_counter = if walking { 12 } else { 0 };
    npc.delay_counter = if walking { 16 } else { 127 };
    npc.visible = true;
    game.overworld.npc_presentation = NpcPresentation::default();
    game.overworld.publish_npc_frame(false);
    game
}

#[test]
fn stationary_npc_lower_half_matches_original_and_priority_only_change_invalidates_cache() {
    run(|| {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/npc-grass-standing-1390.json")).unwrap();
        let bytes: Vec<u8> = reference["pixels"]
            .as_str()
            .unwrap()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let mut game = fixture(false);
        let mut session = RenderSession::new();
        let mut cached = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        session.render(&mut game, &mut cached, &mut |_, _, _, _, _, _| {});
        let before = cached.packed().to_vec();
        let mut compared = 0;
        for frame in 0..40 {
            game.update(&InputState::new());
            assert_eq!(game.state.screen, GameScreen::Overworld);
            let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            game.draw(&mut full);
            session.render(&mut game, &mut cached, &mut |_, _, _, _, _, _| {});
            assert_eq!(
                cached.packed(),
                full.packed(),
                "priority publication frame {frame}"
            );
            let pose = game
                .overworld
                .npc_presentation
                .pose(game.overworld.state.current_map, 0)
                .unwrap();
            if !pose.image.grass_priority {
                continue;
            }
            assert_eq!((pose.x, pose.y), (96, 60));
            for y in 0..16u32 {
                for x in 0..16u32 {
                    let i = (y * 16 + x) as usize;
                    let index = (bytes[i / 4] >> (6 - 2 * (i % 4))) & 3;
                    assert_eq!(
                        full.get_index(96 + x, 60 + y).unwrap() as u8,
                        index,
                        "original stationary NPC frame {frame}, pixel {x},{y}"
                    );
                }
            }
            compared += 1;
        }
        assert!(
            compared >= 21,
            "must actually publish and compare grass priority"
        );
        assert_ne!(
            before,
            cached.packed(),
            "stationary priority changes visible pixels"
        );
    });
}

#[test]
fn walking_npc_and_player_keep_cached_grass_pixels_equal_to_full_frames() {
    run(|| {
        let mut game = fixture(true);
        let mut session = RenderSession::new();
        let mut cached = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        let mut input = InputState::new();
        let mut seen = [false; 2];
        for frame in 0..120 {
            input.begin_frame();
            if (8..24).contains(&frame) {
                input.press(GbButton::Down);
            } else {
                input.release(GbButton::Down);
            }
            game.update(&input);
            assert_eq!(game.state.screen, GameScreen::Overworld);
            let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            game.draw(&mut full);
            session.render(&mut game, &mut cached, &mut |_, _, _, _, _, _| {});
            assert_eq!(
                cached.packed(),
                full.packed(),
                "moving priority frame {frame}"
            );
            let pose = game
                .overworld
                .npc_presentation
                .pose(game.overworld.state.current_map, 1)
                .unwrap();
            seen[usize::from(pose.image.grass_priority)] = true;
        }
        assert!(seen[0] && seen[1], "fixture must cross the grass boundary");
    });
}

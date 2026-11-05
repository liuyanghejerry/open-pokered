//! Manual continuous-input capture. Keep identical on the base and candidate.
use dotzuki_engine::render_config::RenderConfig;
use pokered_app::{render::session::RenderSession, PokemonGame};
use pokered_core::{
    game_state::GameScreen,
    overworld::{npc_presentation::NpcPresentation, Direction, NpcMovementType},
};
use pokered_data::{maps::MapId, species::Species, wild_data::GameVersion};
use pokered_renderer::{
    input::{GbButton, InputState},
    FrameBuffer, Rgba,
};
use serde_json::json;

#[test]
#[ignore = "manual paired capture; requires NPC_GRASS_CAPTURE output directory"]
fn capture_npc_grass() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let output = std::path::PathBuf::from(std::env::var_os("NPC_GRASS_CAPTURE").unwrap());
            assert!(!output.exists(), "preserve earlier captures");
            for case in ["stand", "walk", "player"] {
                for repeat in 1..=2 {
                    let dir = output.join(format!("{case}-{repeat}"));
                    std::fs::create_dir_all(&dir).unwrap();
                    let mut g = PokemonGame::new_with_options(
                        GameVersion::Red,
                        Some(dir.join("session.sav")),
                        None,
                        None,
                        false,
                        None,
                        false,
                        true,
                        #[cfg(feature = "debug-server")]
                        None,
                    );
                    g.set_seed(42);
                    g.state.screen = GameScreen::Overworld;
                    g.overworld.warp_to_map(MapId::Route1, 12, 22);
                    for _ in 0..120 {
                        g.update(&InputState::new());
                    }
                    g.save_data.party.clear();
                    g.save_data
                        .party
                        .add(
                            pokered_core::pokemon::stats::create_pokemon(
                                Species::Mew,
                                50,
                                [0x99; 2],
                            )
                            .unwrap(),
                        )
                        .unwrap();
                    g.overworld.party_lead_level = g.save_data.party.leader_level();
                    g.overworld.state.repel_steps = 200;
                    g.overworld.state.player.facing = Direction::Left;
                    let walking = case != "stand";
                    let slot = usize::from(walking);
                    let npc = &mut g.overworld.npc_states[slot];
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
                    g.overworld.npc_presentation = NpcPresentation::default();
                    g.overworld.publish_npc_frame(false);
                    // Ordinary SRAM is an additional reproducible setup fixture;
                    // no mutation follows the start of the recorded window.
                    if !walking {
                        g.overworld.npc_states[slot].movement_type = NpcMovementType::Stationary;
                        g.save_to_path(&dir.join("fixture.sav")).unwrap();
                    }
                // Both implementations advance four physical frames after
                // fixture setup, before the input-aligned verdict window.
                for _ in 0..4 {
                    g.update(&InputState::new());
                }
                    let mut input = InputState::new();
                    let mut session = RenderSession::new();
                    let mut cached = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                    let mut rows = Vec::new();
                    for frame in -1i32..120 {
                        let pressed = case == "player" && (0..16).contains(&frame);
                        if frame >= 0 {
                            input.begin_frame();
                            if pressed {
                                input.press(GbButton::Down);
                            } else {
                                input.release(GbButton::Down);
                            }
                            g.update(&input);
                        }
                        assert_eq!(g.state.screen, GameScreen::Overworld);
                        let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                        g.draw(&mut full);
                        session.render(&mut g, &mut cached, &mut |_, _, _, _, _, _| {});
                        assert_eq!(full.packed(), cached.packed(), "{case} {repeat} {frame}");
                        full.save_png(&dir.join(format!("frame-{:03}.png", frame + 1)))
                            .unwrap();
                        rows.push(json!({"t": frame, "frame_count": g.frame_count,
                        "button": if pressed { Some("down") } else { None },
                        "map": format!("{:?}", g.overworld.state.current_map),
                        "player": g.overworld.state.player,
                        "npc": g.overworld.npc_states[slot],
                        "presentation": g.overworld.npc_presentation,
                        "pixels": full.packed()}));
                    }
                    std::fs::write(dir.join("frames.json"), serde_json::to_vec(&rows).unwrap())
                        .unwrap();
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

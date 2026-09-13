//! Deterministic fade captures, without a window or audio device.
//! cargo run --release -p pokered-app --example capture_intro_fades -- <output-dir>

use dotzuki_engine::render_config::RenderConfig;
use pokered_app::render::{draw_intro_scene, draw_title_screen};
use pokered_core::intro_scene::{IntroPhase, IntroSceneState};
use pokered_core::title_screen::{TitlePhase, TitleScreenState};
use pokered_data::wild_data::GameVersion;
use pokered_renderer::resource::{AssetRoot, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba};

fn main() {
    let output = std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&output).unwrap();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gfx");
    let mut resources = Some(ResourceManager::new(AssetRoot::new(root).unwrap()));
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);

    let mut intro = IntroSceneState::new();
    for _ in 0..2000 {
        if intro.phase == IntroPhase::FadeOut {
            break;
        }
        intro.update_frame(false);
    }
    assert_eq!(intro.phase, IntroPhase::FadeOut);
    for frame in [0, 12, 23] {
        intro.frame_counter = frame;
        draw_intro_scene(&intro, &mut resources, &mut fb);
        fb.save_png(&output.join(format!("intro-fade-{frame}.png")))
            .unwrap();
    }

    let mut title = TitleScreenState::new(GameVersion::Red);
    title.skip_to_waiting_for_input();
    title.update_frame(true);
    for _ in 0..200 {
        if title.phase == TitlePhase::FadeOut {
            break;
        }
        title.update_frame(false);
    }
    assert_eq!(title.phase, TitlePhase::FadeOut);
    for frame in [0, 8, 15] {
        title.frame_counter = frame;
        draw_title_screen(&title, false, &mut resources, &mut fb);
        fb.save_png(&output.join(format!("title-fade-{frame}.png")))
            .unwrap();
    }
}

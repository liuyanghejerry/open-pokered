use dotzuki_engine::render_config::RenderConfig;
use pokered_app::render::{draw_intro_scene, draw_title_screen};
use pokered_core::intro_scene::{IntroPhase, IntroSceneState};
use pokered_core::title_screen::{TitlePhase, TitleScreenState};
use pokered_data::wild_data::GameVersion;
use pokered_renderer::resource::{AssetRoot, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba};

fn resources() -> Option<ResourceManager> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gfx");
    let mut rm = ResourceManager::new(AssetRoot::new(root).unwrap());
    // Fail rather than silently testing the text fallback when assets are missing.
    rm.load_intro("gengar").unwrap();
    rm.load_intro("red_nidorino_1").unwrap();
    rm.load_title("pokemon_logo").unwrap();
    rm.load_title("player").unwrap();
    rm.load_pokemon_front("charmander").unwrap();
    rm.load_splash("copyright").unwrap();
    Some(rm)
}

fn framebuffer() -> FrameBuffer {
    FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE)
}

// The whole image must lighten together, including background tiles,
// sprites, text and letterbox bars. White pixels must never become darker.
fn assert_fade(normal: &FrameBuffer, faded: &FrameBuffer, step: u8) {
    for y in 0..normal.height() {
        for x in 0..normal.width() {
            let original = normal.get_pixel(x, y).unwrap();
            let expected = original.r.saturating_add(85 * step);
            assert_eq!(
                faded.get_pixel(x, y),
                Some(Rgba::rgb(expected, expected, expected)),
                "fade step {step}, pixel ({x}, {y})"
            );
        }
    }
}

#[test]
fn intro_fades_gengar_nidorino_and_bars_to_white() {
    let mut res = resources();
    let mut state = IntroSceneState::new();
    state.nidorino_base_x = 100;
    let mut normal = framebuffer();
    draw_intro_scene(&state, &mut res, &mut normal);
    assert_eq!(normal.get_pixel(0, 0), Some(Rgba::BLACK));
    let mut faded = framebuffer();
    state.phase = IntroPhase::FadeOut;
    for (frame, step) in [(0, 1), (7, 1), (8, 2), (15, 2), (16, 3), (23, 3)] {
        state.frame_counter = frame;
        draw_intro_scene(&state, &mut res, &mut faded);
        assert_fade(&normal, &faded, step);
    }
    state.phase = IntroPhase::MoveNidorinoRight;
    draw_intro_scene(&state, &mut res, &mut faded);
    assert_fade(&normal, &faded, 0);
}

#[test]
fn title_white_out_covers_pokemon_copyright_and_logo_for_three_frames() {
    let mut res = resources();
    let mut state = TitleScreenState::new(GameVersion::Red);
    state.skip_to_waiting_for_input();
    let mut normal = framebuffer();
    draw_title_screen(&state, false, &mut res, &mut normal);
    let mut faded = framebuffer();
    // title.asm:240-243 calls WaitForSoundToFinish, then
    // GBPalWhiteOutWithDelay3 (home/palettes.asm:11-18): instant all-white
    // registers for three frames, rather than GBFadeOutToWhite's ramp.
    assert!((0..normal.height()).any(|y| (0..normal.width()).any(|x|
        normal.get_pixel(x,y) != Some(Rgba::WHITE))));
    state.phase = TitlePhase::FadeOut;
    for frame in 0..3 {
        state.frame_counter = frame;
        draw_title_screen(&state, false, &mut res, &mut faded);
        for y in 0..144 {
            for x in 0..160 {
                assert_eq!(faded.get_pixel(x,y), Some(Rgba::WHITE),
                    "instant white-out frame {frame}, pixel ({x},{y})");
            }
        }
    }
    state.skip_to_waiting_for_input();
    draw_title_screen(&state, false, &mut res, &mut faded);
    assert_fade(&normal, &faded, 0);
}

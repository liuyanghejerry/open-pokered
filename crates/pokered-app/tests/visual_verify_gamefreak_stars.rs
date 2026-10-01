//! Matched production splash captures on the base and candidate branches.
use pokered_app::render::draw_gamefreak_splash;
use pokered_core::gamefreak_splash::{GameFreakSplashState, SplashInput};
use pokered_renderer::{
    resource::{AssetRoot, ResourceManager},
    FrameBuffer, Rgba,
};

#[test]
#[ignore = "writes matched PR screenshots to PR_SCREENSHOTS"]
fn capture_gamefreak_stars() {
    let output = std::path::PathBuf::from(std::env::var("PR_SCREENSHOTS").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    let mut resources = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
    let mut state = GameFreakSplashState::new();
    let mut fb = FrameBuffer::new(
        dotzuki_engine::render_config::RenderConfig::new(160, 144),
        Rgba::WHITE,
    );
    for frame in 0..=341 {
        for (target, name) in [
            (258, "big-star"),
            (284, "logo-first-flash"),
            (314, "small-stars-entry"),
            (317, "small-stars-next-step"),
            (338, "small-stars-step-1"),
            (341, "small-stars-step-2"),
        ] {
            if frame == target {
                draw_gamefreak_splash(&state, &mut resources, &mut fb);
                fb.save_png(&output.join(format!("{name}.png"))).unwrap();
            }
        }
        state.update_frame(SplashInput::none());
    }
}

//! Fixed map, position and input for the story PR's before/after evidence.
use dotzuki_engine::render_config::RenderConfig;
use pokered_app::render::draw_overworld;
use pokered_core::{game_state::Lang, overworld::{OverworldInput, OverworldScreen}};
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
use pokered_renderer::{resource::{AssetRoot, ResourceManager}, FrameBuffer, Rgba};

#[test]
#[ignore = "explicit before/after screenshot capture"]
fn capture_purified_zone_heal() {
    let side = std::env::var("CAPTURE_SIDE").expect("CAPTURE_SIDE=before or after");
    assert!(matches!(side.as_str(), "before" | "after"));
    let dir = std::env::var("CAPTURE_DIR").unwrap_or_else(|_| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/screenshots/fidelity-story-split")
            .to_string_lossy().into_owned()
    });
    std::fs::create_dir_all(&dir).unwrap();
    let mut screen = OverworldScreen::new(MapId::PokemonTower5F, None, PokemonRedData);
    screen.state.player.x = 10;
    screen.state.player.y = 8;
    // Isolate the real Heal effect from trainers, encounters and dialogue.
    // Both revisions receive the exact same authored command at the same tile.
    screen.reload_scene_with_config("PokemonTower5F", r#"
game_scene PokemonTower5F {
  @load { heal() }
}
    "#, Some(r#"{"onLoad":"PokemonTower5FOnLoad","coordEvents":[],"npcs":[],"signs":[]}"#)).unwrap();
    for frame in 1..=28 {
        screen.update_frame(OverworldInput::new(false,false,false,false,false,false,false,false));
        if frame == 2 { assert!(screen.heal_requested, "capture must execute Heal"); }
        if [8, 20].contains(&frame) {
            let mut resources = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
            let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
            draw_overworld(&mut screen, &mut resources, &mut fb, Lang::En);
            fb.save_png(&std::path::Path::new(&dir).join(format!("purified-zone-frame-{frame}-{side}.png"))).unwrap();
        }
    }
}

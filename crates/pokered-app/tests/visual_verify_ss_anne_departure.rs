//! Visual verification test for the S.S. Anne departure cutscene
//! (`VermilionDockSSAnneLeavesScript` + `VermilionDock_EraseSSAnne`,
//! scripts/VermilionDock.asm:33-123, 182-224).
//!
//! Drives the real dock @load scene (with EVENT_GOT_HM01 set) and renders
//! the actual game pipeline (`draw_overworld`) at each choreography stage:
//! the pre-departure dock (ship hull visible at the bottom), the scroll
//! with smoke puffs above the smokestack, and the post-erase empty dock.
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_ss_anne_departure -- --nocapture
//!
//! The output PNGs are saved in the current working directory.

use pokered_app::render::draw_overworld;
use pokered_core::game_state::Lang;
use pokered_core::overworld::presentation::{
    ShipDeparturePhase, SHIP_DEPARTURE_ERASE_FRAMES, SHIP_DEPARTURE_ITERATION_FRAMES,
    SHIP_DEPARTURE_TOTAL_FRAMES,
};
use pokered_core::overworld::{Direction, OverworldInput, OverworldScreen};
use pokered_core::data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;
use jrpg_engine::render_config::RenderConfig;
use pokered_renderer::{resource::ResourceManager, FrameBuffer, Rgba};

fn create_resource_manager() -> ResourceManager {
    let root = pokered_renderer::resource::AssetRoot::auto_detect()
        .expect("Cannot auto-detect asset root (gfx/ directory). Run from within the workspace project.");
    eprintln!("Asset root: {:?}", root.gfx_dir());
    ResourceManager::new(root)
}

fn render_frame(
    screen: &mut OverworldScreen,
    rm: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    fb.clear(Rgba::WHITE);
    draw_overworld(screen, rm, fb, Lang::default());
}

fn save_frame(fb: &FrameBuffer, filename: &str) {
    let mut img = image::RgbaImage::new(fb.width(), fb.height());
    for y in 0..fb.height() {
        for x in 0..fb.width() {
            if let Some(color) = fb.get_pixel(x, y) {
                let c = color.to_array();
                img.put_pixel(x, y, image::Rgba(c));
            }
        }
    }
    img.save(filename).expect("Failed to save PNG");
    eprintln!("  Saved: {}", filename);
}

fn tick_frames(screen: &mut OverworldScreen, n: u32) {
    let neutral = OverworldInput::new(false, false, false, false, false, false, false, false);
    for _ in 0..n {
        screen.update_frame(neutral);
    }
}

#[test]
fn render_ss_anne_departure_full_sequence() {
    let rm = create_resource_manager();
    let mut rm_opt = Some(rm);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);

    let mut screen = OverworldScreen::new(MapId::VermilionDock, None, PokemonRedData);
    screen.set_flag_live("EVENT_GOT_HM01", true);
    screen.state.player.x = 14;
    screen.state.player.y = 2;
    screen.state.player.facing = Direction::Up;
    screen.run_on_load();

    // A couple of frames for the scene to dispatch the departure effect.
    tick_frames(&mut screen, 3);
    assert!(
        screen.ship_departure.is_some(),
        "the departure animation must start"
    );

    // 1) Pre-departure pause: the ship hull is still at the dock.
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "ss_anne_departure_01_pause.png");

    // 2) Mid-scroll: ~3 iterations in, several smoke puffs over the
    //    smokestack, the view scrolled ~3 tiles.
    tick_frames(&mut screen, 3 * SHIP_DEPARTURE_ITERATION_FRAMES as u32);
    assert_eq!(
        screen.ship_departure.as_ref().unwrap().phase(),
        ShipDeparturePhase::Scroll
    );
    assert!(
        screen.ship_departure.as_ref().unwrap().puff_count() >= 3,
        "multiple smoke puffs must be live mid-scroll"
    );
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "ss_anne_departure_02_scroll_puffs.png");

    // 3) Late scroll: nearly all puffs live, view ~15 tiles scrolled.
    tick_frames(&mut screen, 4 * SHIP_DEPARTURE_ITERATION_FRAMES as u32);
    assert!(
        screen.ship_departure.as_ref().unwrap().puff_count() >= 6,
        "the late scroll must show most puffs"
    );
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "ss_anne_departure_03_scroll_end.png");

    // 4) Erase phase: the hull blocks are water (empty dock). Tick until
    //    the phase is reached (the scene dispatch adds a frame or two of
    //    offset before the state starts ticking).
    let mut ticks = 0;
    while screen.ship_departure.as_ref().map_or(false, |d| {
        d.phase() != ShipDeparturePhase::Erase && !d.is_done()
    }) && ticks < SHIP_DEPARTURE_TOTAL_FRAMES
    {
        tick_frames(&mut screen, 8);
        ticks += 8;
    }
    assert_eq!(
        screen.ship_departure.as_ref().unwrap().phase(),
        ShipDeparturePhase::Erase
    );
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "ss_anne_departure_04_erase.png");

    // 5) Post-cutscene: the walk-out has moved the player north off the
    //    dock; the ship stays gone for the rest of the visit.
    tick_frames(&mut screen, SHIP_DEPARTURE_ERASE_FRAMES as u32 + 24);
    assert!(
        screen.ship_departure.is_none(),
        "the cutscene must be finished"
    );
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "ss_anne_departure_05_done.png");

    eprintln!(
        "player at ({}, {}) — departure complete, ship erased",
        screen.state.player.x, screen.state.player.y
    );
}

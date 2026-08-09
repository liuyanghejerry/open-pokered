//! Visual verification test for the Hall of Fame roll-call and end-credits
//! renderers.
//!
//! Drives `HofCeremonyState` / `CreditsState` to their key phases and renders
//! each through the actual pipeline (`draw_hof_ceremony` / `draw_credits`),
//! saving PNGs for inspection.
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_hof_credits -- --nocapture

use jrpg_engine::render_config::RenderConfig;
use pokered_app::render::{draw_credits, draw_hof_ceremony};
use pokered_core::credits::{CreditsInput, CreditsPhase, CreditsState};
use pokered_core::game_state::Lang;
use pokered_core::hof_ceremony::{HofCeremonyState, HofEntry, HofPhase, HofPlayerStats};
use pokered_data::species::Species;
use pokered_data::wild_data::GameVersion;
use pokered_renderer::{resource::ResourceManager, FrameBuffer, Rgba};

fn create_resource_manager() -> ResourceManager {
    let root = pokered_renderer::resource::AssetRoot::auto_detect()
        .expect("gfx/ asset root not found; run scripts/fetch-gfx.sh");
    ResourceManager::new(root)
}

fn save_frame(fb: &FrameBuffer, filename: &str) {
    let img = image::RgbaImage::from_fn(fb.width(), fb.height(), |x, y| {
        let c = fb.get_pixel(x, y).unwrap_or(pokered_renderer::Rgba::WHITE);
        image::Rgba(c.to_array())
    });
    img.save(filename).expect("Failed to save PNG");
    eprintln!("Saved: {filename}");
}

fn ceremony() -> HofCeremonyState {
    HofCeremonyState::new(
        vec![
            HofEntry {
                species: Species::Charizard,
                level: 65,
                nickname: "CHARLIE".into(),
            },
            HofEntry {
                species: Species::Lapras,
                level: 60,
                nickname: "LORELEI".into(),
            },
        ],
        HofPlayerStats {
            name: "RED".into(),
            play_time_hours: 25,
            play_time_minutes: 30,
            money: 99999,
            dex_seen: 120,
            dex_owned: 81,
            rating: "Very good!\nGo fish for some\nmarine #MON!",
        },
    )
}

fn render_hof(hof: &HofCeremonyState, rm: &mut Option<ResourceManager>, name: &str) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_hof_ceremony(hof, rm, &mut fb, Lang::En);
    save_frame(&fb, name);
}

fn render_credits(roll: &CreditsState, rm: &mut Option<ResourceManager>, name: &str) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_credits(roll, rm, &mut fb);
    save_frame(&fb, name);
}

#[test]
fn render_hof_ceremony_phases() {
    let mut rm = Some(create_resource_manager());
    let mut hof = ceremony();

    // Advance to the first mon's scroll (mid-slide).
    while hof.phase() != HofPhase::MonScroll {
        hof.update_frame();
    }
    for _ in 0..24 {
        hof.update_frame();
    }
    assert_eq!(hof.phase(), HofPhase::MonScroll);
    render_hof(&hof, &mut rm, "/tmp/hof_1_scroll.png");

    while hof.phase() != HofPhase::MonInfo {
        hof.update_frame();
    }
    render_hof(&hof, &mut rm, "/tmp/hof_2_info.png");

    while hof.phase() != HofPhase::MonText {
        hof.update_frame();
    }
    render_hof(&hof, &mut rm, "/tmp/hof_3_text.png");

    let mut guard = 0;
    while hof.phase() != HofPhase::PlayerStats {
        hof.update_frame();
        guard += 1;
        assert!(guard < 10_000);
    }
    render_hof(&hof, &mut rm, "/tmp/hof_4_player.png");

    // Runs to completion.
    guard = 0;
    loop {
        if hof.update_frame() {
            break;
        }
        guard += 1;
        assert!(guard < 10_000);
    }
    assert_eq!(hof.phase(), HofPhase::Done);
}

#[test]
fn render_credits_phases() {
    let mut rm = Some(create_resource_manager());
    let mut roll = CreditsState::new(GameVersion::Red);

    // First screen fading in (#MON / RED VERSION STAFF).
    for _ in 0..10 {
        roll.update_frame(CreditsInput::none());
    }
    assert_eq!(roll.phase(), CreditsPhase::Hold);
    render_credits(&roll, &mut rm, "/tmp/credits_1_fade.png");

    // Fully faded in.
    for _ in 0..30 {
        roll.update_frame(CreditsInput::none());
    }
    render_credits(&roll, &mut rm, "/tmp/credits_2_hold.png");

    // Venusaur silhouette scrolling by.
    while roll.phase() != CreditsPhase::MonScroll {
        roll.update_frame(CreditsInput::none());
    }
    for _ in 0..20 {
        roll.update_frame(CreditsInput::none());
    }
    render_credits(&roll, &mut rm, "/tmp/credits_3_mon.png");

    // Run to THE END.
    let mut guard = 0;
    while roll.phase() != CreditsPhase::TheEnd {
        roll.update_frame(CreditsInput::none());
        guard += 1;
        assert!(guard < 100_000);
    }
    for _ in 0..20 {
        roll.update_frame(CreditsInput::none());
    }
    assert!(roll.the_end_visible());
    render_credits(&roll, &mut rm, "/tmp/credits_4_the_end.png");
}

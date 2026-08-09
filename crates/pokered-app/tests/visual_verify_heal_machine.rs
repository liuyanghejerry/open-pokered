//! Visual verification test for the Pokemon Center healing machine animation.
//!
//! Creates an OverworldScreen for a Pokemon Center map, sets up the healing
//! machine state, and renders using the ACTUAL game rendering pipeline
//! (draw_overworld). Saves the output as `heal_machine_frame.png`.
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_heal_machine -- --nocapture
//!
//! The output PNG is saved in the current working directory.

use pokered_app::render::draw_overworld;
use pokered_core::game_state::Lang;
use pokered_core::overworld::{HealingMachinePhase, HealingMachineState, OverworldScreen};
use pokered_data::maps::MapId;
use dotzuki_engine::render_config::RenderConfig;
use pokered_renderer::{resource::ResourceManager, FrameBuffer, Rgba};

/// Create a `ResourceManager` by auto-detecting the gfx/ asset root.
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

#[test]
fn render_healing_machine_full_sequence() {
    let rm = create_resource_manager();
    let mut rm_opt = Some(rm);

    // Create an OverworldScreen for Viridian Pokemon Center.
    let mut screen = OverworldScreen::new(MapId::ViridianPokecenter, None, pokered_core::data::impl_traits::PokemonRedData);
    // Position the player near the nurse counter.
    screen.state.player.x = 4;
    screen.state.player.y = 3;

    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);

    // ── Frame 1: Monitor only (no pokeballs yet) ────────────────
    screen.pending_healing_machine = Some(HealingMachineState {
        phase: HealingMachinePhase::HealPartyMember {
            member_index: 0,
            total_members: 0,
        },
        frames_remaining: 0,
        pokeballs_visible: 0,
        flash_active: false,
    });
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "heal_machine_frame_01_initial.png");

    // ── Frame 2: 3 pokeballs visible (mid-heal) ────────────────
    screen.pending_healing_machine = Some(HealingMachineState {
        phase: HealingMachinePhase::HealPartyMember {
            member_index: 3,
            total_members: 6,
        },
        frames_remaining: 15,
        pokeballs_visible: 3,
        flash_active: false,
    });
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "heal_machine_frame_02_3balls.png");

    // ── Frame 3: All 6 pokeballs visible ────────────────────────
    screen.pending_healing_machine = Some(HealingMachineState {
        phase: HealingMachinePhase::HealPartyMember {
            member_index: 6,
            total_members: 6,
        },
        frames_remaining: 10,
        pokeballs_visible: 6,
        flash_active: false,
    });
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "heal_machine_frame_03_6balls.png");

    // ── Frame 4: Flash effect active (palette swap) ─────────────
    screen.pending_healing_machine = Some(HealingMachineState {
        phase: HealingMachinePhase::FlashSprite {
            flashes_remaining: 5,
        },
        frames_remaining: 5,
        pokeballs_visible: 6,
        flash_active: true,
    });
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "heal_machine_frame_04_flash.png");

    eprintln!("\n✅ Saved 4 frames:");
    eprintln!("   1. heal_machine_frame_01_initial.png — monitor only, no balls");
    eprintln!("   2. heal_machine_frame_02_3balls.png  — 3 pokeballs visible");
    eprintln!("   3. heal_machine_frame_03_6balls.png  — all 6 pokeballs visible");
    eprintln!("   4. heal_machine_frame_04_flash.png   — flash effect active");
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

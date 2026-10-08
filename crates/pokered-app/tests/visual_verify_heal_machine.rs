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
use pokered_core::overworld::{Direction, HealingMachinePhase, HealingMachineState, OverworldScreen};
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
    draw_overworld(screen, rm, fb, Lang::En);
}

#[test]
fn render_healing_machine_full_sequence() {
    let rm = create_resource_manager();
    let mut rm_opt = Some(rm);

    // Create an OverworldScreen for Viridian Pokemon Center.
    let mut screen = OverworldScreen::new(MapId::ViridianPokecenter, None, pokered_core::data::impl_traits::PokemonRedData);
    // Position the player near the nurse counter.
    screen.state.player.x = 3;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Up;

    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);

    // ── Frame 1: Monitor only (no pokeballs yet) ────────────────
    screen.pending_healing_machine = Some(HealingMachineState {
        phase: HealingMachinePhase::HealPartyMember {
            member_index: 0,
            total_members: 6,
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

/// Original dbsprite coordinates are raw OAM: subtract X=8, Y=16.
/// Keep this oracle independent of renderer offsets and viewport constants.
#[test]
fn canonical_healing_overlay_matches_original_oam_and_palette() {
    use pokered_renderer::palette::Palette;
    use pokered_renderer::resource::AssetCategory;
    let mut screen = OverworldScreen::new(MapId::ViridianPokecenter, None, pokered_core::data::impl_traits::PokemonRedData);
    screen.state.player.x = 3; screen.state.player.y = 3; screen.state.player.facing = Direction::Up;
    let mut resources = Some(create_resource_manager());
    let tiles = resources.as_mut().unwrap().load_asset(AssetCategory::Overworld, "heal_machine.png").unwrap().tileset.clone();
    let mut background = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
    render_frame(&mut screen, &mut resources, &mut background);
    for flash in [false, true] {
        let colors = if flash { [Rgba::TRANSPARENT, Rgba::rgb(85,85,85), Rgba::WHITE, Rgba::BLACK] }
            else { [Rgba::TRANSPARENT, Rgba::WHITE, Rgba::rgb(85,85,85), Rgba::BLACK] };
        let palette = Palette::new(&colors);
        let mut expected = background.clone();
        for (x,y,tile,flip) in [(44,20,0,false),(40,27,1,false),(48,27,1,true),
            (40,32,1,false),(48,32,1,true),(40,37,1,false),(48,37,1,true)] {
            for dy in 0..8u32 { for dx in 0..8u32 {
                let source_x = if flip { 7-dx } else { dx };
                let color = tiles.get(tile).render_row(dy as usize, &palette)[source_x as usize];
                if color != Rgba::TRANSPARENT { expected.set_pixel(x+dx,y+dy,color); }
            } }
        }
        screen.pending_healing_machine = Some(HealingMachineState {
            phase: HealingMachinePhase::HealPartyMember { member_index: 6, total_members: 6 },
            frames_remaining: 10, pokeballs_visible: 6, flash_active: flash,
        });
        let mut actual = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
        render_frame(&mut screen, &mut resources, &mut actual);
        for y in 0..144 { for x in 0..160 {
            assert_eq!(actual.get_pixel(x,y),expected.get_pixel(x,y), "healing flash={flash} at {x},{y}");
        } }
    }
}

#[test]
#[ignore = "matched canonical healing-frame screenshot; not an animation recording"]
fn capture_healing_origin_90() {
    let output = std::path::PathBuf::from(std::env::var("FIDELITY_HEAL_ORIGIN_CAPTURE").unwrap());
    let mut screen = OverworldScreen::new(MapId::ViridianPokecenter, None, pokered_core::data::impl_traits::PokemonRedData);
    screen.state.player.x = 3; screen.state.player.y = 3; screen.state.player.facing = Direction::Up;
    screen.pending_healing_machine = Some(HealingMachineState {
        phase: HealingMachinePhase::HealPartyMember { member_index: 6, total_members: 6 },
        frames_remaining: 10, pokeballs_visible: 6, flash_active: false,
    });
    let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
    render_frame(&mut screen, &mut Some(create_resource_manager()), &mut fb);
    fb.save_png(&output).unwrap();
}

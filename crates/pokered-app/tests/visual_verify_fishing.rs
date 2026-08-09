//! Visual verification test for the player-side fishing rod animation
//! (`FishingAnim`, engine/overworld/player_animations.asm:378-469).
//!
//! Drives the real rod flow (OLD ROD always hooks MAGIKARP) and renders the
//! actual game pipeline (`draw_overworld`) at each choreography stage:
//! rod-out pose, bite shake, "!" bubble, result text — plus down- and
//! left-facing variants showing the rod piece offsets (the original's
//! DOWN/RIGHT pieces landed off-screen because its player was anchored to
//! the bottom edge; the port's centered player keeps them visible).
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_fishing -- --nocapture
//!
//! The output PNGs are saved in the current working directory.

use pokered_app::render::draw_overworld;
use pokered_core::game_state::Lang;
use pokered_core::overworld::{Direction, OverworldInput, OverworldScreen};
use pokered_core::data::impl_traits::PokemonRedData;
use pokered_data::blockset_data;
use pokered_data::items::ItemId;
use pokered_data::maps::MapId;
use pokered_data::tilesets::TilesetId;
use jrpg_engine::render_config::RenderConfig;
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

/// Find a blockset block whose player-readable tile equals the water tile
/// ($14) and position the player so `facing` points at that sub-tile of a
/// block placed at map block (5,5).
fn face_water_tile(screen: &mut OverworldScreen, facing: Direction) {
    let mut found = None;
    for block in 0u8..=255 {
        let Some(tiles) = blockset_data::block_tiles(TilesetId::Overworld, block) else {
            break;
        };
        for (sub_x, sub_y) in [(0u16, 0u16), (1, 0), (0, 1), (1, 1)] {
            let idx = ((sub_y * 2 + 1) * 4 + sub_x * 2) as usize;
            if tiles[idx] == 0x14 {
                found = Some((block, sub_x, sub_y));
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }
    let (block, sub_x, sub_y) = found.expect("blockset has a water block");
    screen
        .map_data
        .as_mut()
        .expect("map_data present")
        .set_block(5, 5, block);
    let water_x = 5u16 * 2 + sub_x;
    let water_y = 5u16 * 2 + sub_y;
    match facing {
        Direction::Up => {
            screen.state.player.x = water_x;
            screen.state.player.y = water_y + 1;
        }
        Direction::Left => {
            screen.state.player.x = water_x + 1;
            screen.state.player.y = water_y;
        }
        Direction::Down => {
            screen.state.player.x = water_x;
            screen.state.player.y = water_y.saturating_sub(1);
        }
        Direction::Right => {
            screen.state.player.x = water_x.saturating_sub(1);
            screen.state.player.y = water_y;
        }
    }
    screen.state.player.facing = facing;
}

/// Mash A every other frame until the current dialogue is gone, then tick
/// one extra frame so the dialogue-close follow-up (the rod animation start)
/// runs.
fn dismiss_dialogue(screen: &mut OverworldScreen) {
    let press_a = OverworldInput::new(false, false, false, false, true, false, false, false);
    let neutral = OverworldInput::new(false, false, false, false, false, false, false, false);
    for i in 0..600 {
        if screen.pending_dialogue.is_none() {
            screen.update_frame(neutral);
            return;
        }
        screen.update_frame(if i % 2 == 0 { press_a } else { neutral });
    }
    panic!("dialogue never dismissed");
}

fn tick_frames(screen: &mut OverworldScreen, n: u32) {
    let neutral = OverworldInput::new(false, false, false, false, false, false, false, false);
    for _ in 0..n {
        screen.update_frame(neutral);
    }
}

fn start_rod(screen: &mut OverworldScreen) {
    assert!(
        !screen.use_field_item(ItemId::OldRod, MapId::PalletTown),
        "rods are key items"
    );
    dismiss_dialogue(screen);
    // The original pauses 80 frames after the used-text closes before the
    // cast animation (FishingInit: PrintText → SFX → DelayFrames(80) →
    // FishingAnim; item_effects.asm:1906-1911).
    tick_frames(screen, 80);
    assert!(screen.fishing_anim.is_some(), "rod animation started");
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

#[test]
fn render_fishing_anim_full_sequence() {
    let rm = create_resource_manager();
    let mut rm_opt = Some(rm);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);

    // ── Up-facing bite: rod piece above the player (148,116) ─────────
    let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    face_water_tile(&mut screen, Direction::Up);
    start_rod(&mut screen);

    // Rod out: fishing pose + rod piece visible (anim frame ~10..109).
    tick_frames(&mut screen, 60);
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "fishing_frame_01_rod_out_up.png");

    // Bite shake: player + rod jitter ±1px (anim frames 110..139; frame
    // 111 is an even iteration → offset +1).
    tick_frames(&mut screen, 51);
    assert_eq!(
        screen.fishing_anim.as_ref().unwrap().phase(),
        pokered_core::overworld::presentation::FishingAnimPhase::Shake
    );
    assert_eq!(
        screen.fishing_anim.as_ref().unwrap().player_shake_offset(),
        1
    );
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "fishing_frame_02_shake_up.png");

    // "!" bubble over the player; the up-facing rod is hidden so it does not
    // overlap the bubble (anim frames 140..199).
    tick_frames(&mut screen, 30);
    assert_eq!(
        screen.fishing_anim.as_ref().unwrap().phase(),
        pokered_core::overworld::presentation::FishingAnimPhase::Bubble
    );
    assert!(!screen.fishing_anim.as_ref().unwrap().rod_visible());
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "fishing_frame_03_bubble_up.png");

    // Result text after the animation (FishingAnim.done → PrintText).
    tick_frames(&mut screen, 60);
    assert!(screen.fishing_anim.is_none());
    assert!(screen.pending_dialogue.is_some(), "result text queued");
    render_frame(&mut screen, &mut rm_opt, &mut fb);
    save_frame(&fb, "fishing_frame_04_result_text.png");

    // ── Left-facing rod piece at the player's bottom-left (offset 0,16) ─
    let mut left = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    face_water_tile(&mut left, Direction::Left);
    start_rod(&mut left);
    tick_frames(&mut left, 60);
    render_frame(&mut left, &mut rm_opt, &mut fb);
    save_frame(&fb, "fishing_frame_05_rod_out_left.png");

    // ── Down-facing rod piece below the player's feet (offset 20,35) ───
    let mut down = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    face_water_tile(&mut down, Direction::Down);
    start_rod(&mut down);
    tick_frames(&mut down, 60);
    render_frame(&mut down, &mut rm_opt, &mut fb);
    save_frame(&fb, "fishing_frame_06_rod_out_down.png");

    eprintln!("\nSaved 6 frames:");
    eprintln!("   1. fishing_frame_01_rod_out_up.png     — pose + rod above the head");
    eprintln!("   2. fishing_frame_02_shake_up.png       — bite shake (±1px jitter)");
    eprintln!("   3. fishing_frame_03_bubble_up.png      — \"!\" bubble, rod hidden");
    eprintln!("   4. fishing_frame_04_result_text.png    — \"Oh! It's a bite!\"");
    eprintln!("   5. fishing_frame_05_rod_out_left.png   — left-facing rod piece");
    eprintln!("   6. fishing_frame_06_rod_out_down.png   — down-facing rod piece");
}

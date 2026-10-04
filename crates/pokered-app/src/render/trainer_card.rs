//! Trainer card screen (`StartMenu_TrainerInfo` / `DrawTrainerInfo` /
//! `DrawBadges` — engine/menus/start_sub_menus.asm:453-565,
//! engine/menus/draw_badges.asm): player name, money, play time, the Red
//! front sprite, and the 8 gym badges in two rows of four (a slot shows the
//! gym leader's face until the badge is owned).

use crate::alloc_prelude::*;
use pokered_core::game_state::Lang;
use pokered_data::lang_data::ui_label;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::palette::GRAYSCALE_PALETTE;
use pokered_renderer::resource::{AssetCategory, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

/// Four equal-width cells inside the frame, with the number beside the icon.
const BADGE_ROW_Y: [u32; 2] = [88, 112];
const BADGE_ROW_X: [u32; 4] = [16, 48, 80, 112];

#[allow(clippy::too_many_arguments)]
pub fn draw_trainer_card(
    player_name: &str,
    money: u32,
    play_time_hours: u8,
    play_time_minutes: u8,
    obtained_badges: u8,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    fb.clear(Rgba::WHITE);
    let pal = &GRAYSCALE_PALETTE;
    let fg = Rgba::BLACK;
    let t = TILE_SIZE;

    if let Some(rm) = res.as_mut() {
        if let Ok(cached) = rm.load_asset(AssetCategory::Player, "red.png") {
            for row in 0..6 {
                for col in 1..5 {
                    blit_tile(
                        fb,
                        &cached.tileset,
                        (row * 7 + col) as usize,
                        120 + (col - 1) * 8,
                        8 + row * 8,
                        pal,
                    );
                }
            }
        }
        if let Ok(cached) = rm.load_asset(AssetCategory::TrainerCard, "trainer_info.png") {
            super::draw_trainer_info_box(fb, &cached.tileset, 0, 0, 18, 6);
            super::draw_trainer_info_box(fb, &cached.tileset, 1, 10, 16, 6);
            for y in 10..18 {
                for x in [0, 19] {
                    blit_tile(fb, &cached.tileset, 8, x * 8, y * 8, pal);
                }
            }
        }
        if let Ok(cached) = rm.load_asset(AssetCategory::TrainerCard, "circle_tile.png") {
            for x in [6, 13] {
                blit_tile(fb, &cached.tileset, 0, x * 8, 9 * 8, pal);
            }
        }
    }
    let is_zh = lang == Lang::Zh;
    for (label, value, x, y) in [
        ("NAME/", player_name.to_uppercase(), 56, 16),
        ("MONEY/", format!("${:06}", money), 64, 32),
        (
            "TIME/",
            format!("{}:{:02}", play_time_hours, play_time_minutes),
            72,
            48,
        ),
    ] {
        draw_text(ui_label(label, is_zh), 16, y, fg, fb);
        draw_text(&value, x, y, fg, fb);
    }
    // Fusion Pixel extends below the original eight-pixel heading row.
    // Clear its full ink area so the frame texture cannot show through.
    let heading = ui_label("BADGES", is_zh);
    let width = pokered_renderer::embedded_font::measure_text(heading);
    for y in 72..82 {
        for x in 56..56 + width { fb.set_pixel(x, y, Rgba::WHITE); }
    }
    draw_text(heading, 56, 72, fg, fb);

    // Badge rows: number tile beside the 2×2 face (unowned) or badge (owned)
    // graphic (GymLeaderFaceAndBadgeTileGraphics layout: face i at
    // tile i*8, its badge at +4).
    if let Some(ref mut rm) = res {
        let numbers = rm
            .load_asset(AssetCategory::TrainerCard, "badge_numbers.png")
            .map(|c| c.tileset.clone());
        let faces = rm
            .load_asset(AssetCategory::TrainerCard, "badges.png")
            .map(|c| c.tileset.clone());
        for i in 0..8u32 {
            let row = (i / 4) as usize;
            let col = (i % 4) as usize;
            let x = BADGE_ROW_X[col];
            let y = BADGE_ROW_Y[row];
            if let Ok(ref ts) = numbers {
                blit_tile(fb, ts, i as usize, x, y, pal);
            }
            if let Ok(ref ts) = faces {
                let owned = obtained_badges & (1 << i) != 0;
                let base = i * 8 + if owned { 4 } else { 0 };
                for k in 0..4u32 {
                    let dx = (k % 2) * t;
                    let dy = (k / 2) * t;
                    blit_tile(fb, ts, (base + k) as usize, x + dx, y + 8 + dy, pal);
                }
            }
        }
    }
}

/// Blit one 8×8 tile (by linear index, row-major within the tileset).
fn blit_tile(
    fb: &mut FrameBuffer,
    ts: &pokered_renderer::tile::TileSet,
    idx: usize,
    x: u32,
    y: u32,
    pal: &pokered_renderer::palette::Palette,
) {
    if idx >= ts.len() {
        return;
    }
    let tile = ts.get(idx);
    for row in 0..TILE_SIZE {
        let rgba_row = tile.render_row(row as usize, pal);
        for col in 0..TILE_SIZE {
            let c = rgba_row[col as usize];
            if c != Rgba::TRANSPARENT && x + col < fb.width() && y + row < fb.height() {
                fb.set_pixel(x + col, y + row, c);
            }
        }
    }
}

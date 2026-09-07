//! Trainer card screen (`StartMenu_TrainerInfo` / `DrawTrainerInfo` /
//! `DrawBadges` — engine/menus/start_sub_menus.asm:453-565,
//! engine/menus/draw_badges.asm): player name, money, play time, the Red
//! front sprite, and the 8 gym badges in two rows of four (a slot shows the
//! gym leader's face until the badge is owned).

use pokered_core::game_state::Lang;
use pokered_data::lang_data::ui_label;
use pokered_renderer::embedded_font::{draw_text, measure_text};
use pokered_renderer::palette::GRAYSCALE_PALETTE;
use pokered_renderer::resource::{AssetCategory, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::draw_text_box;

/// Four equal-width cells inside the frame, with the number beside the icon.
const BADGE_ROW_Y: [u32; 2] = [92, 118];
const BADGE_ROW_X: [u32; 4] = [13, 49, 85, 121];

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

    // Matching full-width frames. Seven interior rows keep the 56px trainer
    // sprite clear of the top card's bottom border.
    draw_text_box(fb, 0, 0, 18, 7, fg);
    draw_text_box(fb, 0, 10 * t, 18, 6, fg);

    // Red front sprite, upper right (DisplayPicCenteredOrUpperRight), unflipped.
    if let Some(ref mut rm) = res {
        if let Ok(cached) = rm.load_asset(AssetCategory::Player, "red.png") {
            let ts = cached.tileset.clone();
            let tiles_w = cached.source_size.0 / t;
            for idx in 0..ts.len() {
                let tx = (idx as u32) % tiles_w;
                let ty = (idx as u32) / tiles_w;
                blit_tile(fb, &ts, idx, 12 * t + tx * t, t + ty * t, pal);
            }
        }
    }

    let is_zh = lang == Lang::Zh;
    for (label, value, y) in [
        ("NAME/", player_name.to_uppercase(), 14),
        ("MONEY/", format!("${}", money), 32),
        (
            "TIME/",
            format!("{}:{:02}", play_time_hours, play_time_minutes),
            50,
        ),
    ] {
        draw_text(ui_label(label, is_zh), 12, y, fg, fb);
        draw_text(
            &value,
            96u32.saturating_sub(measure_text(&value)),
            y,
            fg,
            fb,
        );
    }

    // Center the localized heading in a cutout in the badge frame's top edge.
    let heading = ui_label("BADGES", is_zh);
    let heading_width = measure_text(heading);
    let heading_x = (fb.width() - heading_width) / 2;
    for y in 78..88 {
        for x in heading_x - 6..heading_x + heading_width + 6 {
            fb.set_pixel(x, y, Rgba::WHITE);
        }
    }
    draw_text(heading, heading_x, 78, fg, fb);

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
                blit_tile(fb, ts, i as usize, x, y + 4, pal);
            }
            if let Ok(ref ts) = faces {
                let owned = obtained_badges & (1 << i) != 0;
                let base = i * 8 + if owned { 4 } else { 0 };
                for k in 0..4u32 {
                    let dx = (k % 2) * t;
                    let dy = (k / 2) * t;
                    blit_tile(fb, ts, (base + k) as usize, x + 10 + dx, y + dy, pal);
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

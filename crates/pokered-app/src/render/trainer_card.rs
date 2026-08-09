//! Trainer card screen (`StartMenu_TrainerInfo` / `DrawTrainerInfo` /
//! `DrawBadges` — engine/menus/start_sub_menus.asm:453-565,
//! engine/menus/draw_badges.asm): player name, money, play time, the Red
//! front sprite, and the 8 gym badges in two rows of four (a slot shows the
//! gym leader's face until the badge is owned).

use pokered_core::game_state::Lang;
use pokered_data::lang_data::ui_label;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::palette::GRAYSCALE_PALETTE;
use pokered_renderer::resource::{AssetCategory, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::draw_text_box;

/// Badge slot layout from `DrawBadges`: number tiles at (2+i*4, 11) / (2+i*4,
/// 14), 2×2-tile face/badge graphics directly below each number.
const BADGE_ROW_Y: [u32; 2] = [11, 14];
const BADGE_ROW_X: [u32; 4] = [2, 6, 10, 14];

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

    // Card frames (TrainerInfo_DrawTextBox: 18-wide top box, 16-wide badge
    // box, both 6 interior rows tall).
    draw_text_box(fb, 0, 0, 18, 6, fg);
    draw_text_box(fb, t, 10 * t, 16, 6, fg);

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
    draw_text(ui_label("NAME/", is_zh), 2 * t, 2 * t, fg, fb);
    draw_text(&player_name.to_uppercase(), 7 * t, 2 * t, fg, fb);
    draw_text(ui_label("MONEY/", is_zh), 2 * t, 4 * t, fg, fb);
    draw_text(&format!("${}", money), 8 * t, 4 * t, fg, fb);
    draw_text(ui_label("TIME/", is_zh), 2 * t, 6 * t, fg, fb);
    draw_text(
        &format!("{}:{:02}", play_time_hours, play_time_minutes),
        9 * t,
        6 * t,
        fg,
        fb,
    );

    // "●BADGES●" header (asm uses circle tile $76 around the word).
    draw_circle(6 * t, 9 * t, fb);
    draw_text(ui_label("BADGES", is_zh), 6 * t + 10, 9 * t, fg, fb);
    draw_circle(6 * t + 48, 9 * t, fb);

    // Badge rows: number tile, then the 2×2 face (unowned) or badge (owned)
    // graphic below it (GymLeaderFaceAndBadgeTileGraphics layout: face i at
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
            let x = BADGE_ROW_X[col] * t;
            let y = BADGE_ROW_Y[row] * t;
            if let Ok(ref ts) = numbers {
                blit_tile(fb, ts, i as usize, x, y, pal);
            }
            if let Ok(ref ts) = faces {
                let owned = obtained_badges & (1 << i) != 0;
                let base = i * 8 + if owned { 4 } else { 0 };
                for k in 0..4u32 {
                    let dx = (k % 2) * t;
                    let dy = (k / 2) * t;
                    blit_tile(fb, ts, (base + k) as usize, x + dx, y + t + dy, pal);
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

/// Small filled circle (the original's $76 circle tile around "BADGES").
fn draw_circle(x: u32, y: u32, fb: &mut FrameBuffer) {
    const CIRCLE: [u8; 8] = [
        0b0011_1100,
        0b0111_1110,
        0b1111_1111,
        0b1111_1111,
        0b1111_1111,
        0b1111_1111,
        0b0111_1110,
        0b0011_1100,
    ];
    for (dy, bits) in CIRCLE.iter().enumerate() {
        for dx in 0..8u32 {
            if bits & (0x80 >> dx) != 0 {
                let px = x + dx;
                let py = y + dy as u32;
                if px < fb.width() && py < fb.height() {
                    fb.set_pixel(px, py, Rgba::BLACK);
                }
            }
        }
    }
}

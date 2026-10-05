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
    // Center the project font between the circles. CJK ink extends two
    // pixels below the ten-pixel Latin row; both must end before y=80.
    let heading = ui_label("BADGES", is_zh);
    let width = pokered_renderer::embedded_font::measure_text(heading);
    let heading_x = (fb.width().saturating_sub(width)) / 2;
    let heading_height = if is_zh { 12 } else { 10 };
    let heading_y = 80 - heading_height;
    for y in heading_y..80 {
        for x in heading_x..heading_x + width {
            fb.set_pixel(x, y, Rgba::WHITE);
        }
    }
    draw_text(heading, heading_x, heading_y, fg, fb);

    // Badge rows: number tile beside the 2×2 face (unowned) or badge (owned)
    // graphic (GymLeaderFaceAndBadgeTileGraphics layout: face i at
    // tile i*8, its badge at +4).
    if let Some(ref mut rm) = res {
        let faces = rm
            .load_asset(AssetCategory::TrainerCard, "badges.png")
            .map(|c| c.tileset.clone());
        for i in 0..8u32 {
            let row = (i / 4) as usize;
            let col = (i % 4) as usize;
            let x = BADGE_ROW_X[col];
            let y = BADGE_ROW_Y[row];
            draw_text(&(i + 1).to_string(), x, y.saturating_sub(2), fg, fb);
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

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render_config::RenderConfig;
    use pokered_renderer::resource::AssetRoot;

    #[test]
    fn badge_heading_is_centered_and_leaves_the_top_frame_intact() {
        let mut resources = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
        let tiles = resources
            .as_mut()
            .unwrap()
            .load_asset(AssetCategory::TrainerCard, "trainer_info.png")
            .unwrap()
            .tileset
            .clone();
        let mut border = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        super::super::draw_trainer_info_box(&mut border, &tiles, 1, 10, 16, 6);
        for lang in [Lang::En, Lang::Zh] {
            let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            draw_trainer_card("RED", 3000, 0, 0, 0, &mut resources, &mut fb, lang);
            let ink: Vec<_> = (64..80)
                .flat_map(|y| (56..104).map(move |x| (x, y)))
                .filter(|&(x, y)| fb.get_pixel(x, y) == Some(Rgba::BLACK))
                .collect();
            let left = ink.iter().map(|p| p.0).min().expect("heading ink");
            let right = ink.iter().map(|p| p.0).max().unwrap();
            assert!(
                (left as i32 + right as i32 - 159).abs() <= 4,
                "{lang:?} heading must be centered between the circles"
            );
            for y in 80..88 {
                for x in 56..104 {
                    assert_eq!(
                        fb.get_pixel(x, y),
                        border.get_pixel(x, y),
                        "{lang:?} heading must not erase or overlap the frame at ({x},{y})"
                    );
                }
            }
        }
    }
}

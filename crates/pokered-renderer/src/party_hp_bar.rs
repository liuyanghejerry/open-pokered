//! Party HP graphics with an open-font label; original letter tiles are skipped.
use crate::alloc_prelude::*;
use crate::{FbSurface, Rgba};
use dotzuki_renderer::asset_provider::ResourceProvider;
use dotzuki_renderer::battle_scene::{
    calc_hp_bar_pixels, TILE_HP_BAR_LEFT, TILE_HP_EMPTY, TILE_HP_END_CAP_BATTLE, TILE_HP_FULL,
    TILE_HP_PARTIAL_BASE,
};

pub fn draw_party_hp_bar(
    fb: &mut impl FbSurface,
    provider: &mut dyn ResourceProvider,
    x: u32,
    y: u32,
    hp: u16,
    max_hp: u16,
) -> Result<(), String> {
    let tiles = provider.load_asset_2bpp("font", "font_battle_extra.png")?;
    let mut remaining = calc_hp_bar_pixels(hp, max_hp);
    let mut ids = [TILE_HP_EMPTY; 8];
    ids[0] = TILE_HP_BAR_LEFT;
    ids[7] = TILE_HP_END_CAP_BATTLE;
    for id in &mut ids[1..7] {
        let pixels = remaining.min(8);
        remaining -= pixels;
        *id = match pixels {
            0 => TILE_HP_EMPTY,
            8 => TILE_HP_FULL,
            n => TILE_HP_PARTIAL_BASE + n as u8,
        };
    }
    for (column, id) in ids.into_iter().enumerate() {
        let tile = tiles.get((id - 0x62) as usize);
        for row in 0..8 {
            let colors = tile.render_row(row, &crate::palette::GRAYSCALE_PALETTE);
            for (pixel, color) in colors.into_iter().enumerate() {
                fb.set_pixel(
                    x + 12 + column as u32 * 8 + pixel as u32,
                    y + row as u32,
                    if crate::battle_text_tiles::is_text_pixel(id as usize, pixel) { Rgba::WHITE } else { color },
                );
            }
        }
    }
    crate::embedded_font::draw_text("HP", x, y.saturating_sub(2), Rgba::BLACK, fb);
    Ok(())
}

pub fn draw_party_hp_bar_indexed(
    fb: &mut crate::FrameBuffer,
    provider: &mut dyn ResourceProvider,
    x: u32,
    y: u32,
    hp: u16,
    max_hp: u16,
) -> Result<(), String> {
    let tiles = provider.load_asset_2bpp("font", "font_battle_extra.png")?;
    let mut remaining = calc_hp_bar_pixels(hp, max_hp);
    let mut ids = [TILE_HP_EMPTY; 8];
    ids[0] = TILE_HP_BAR_LEFT;
    ids[7] = TILE_HP_END_CAP_BATTLE;
    for id in &mut ids[1..7] {
        let pixels = remaining.min(8);
        remaining -= pixels;
        *id = match pixels {
            0 => TILE_HP_EMPTY,
            8 => TILE_HP_FULL,
            n => TILE_HP_PARTIAL_BASE + n as u8,
        };
    }
    #[cfg(all(target_os = "none", target_arch = "arm"))]
    {
        let width = fb.width() as usize;
        let height = fb.height() as usize;
        let indices = fb.indices_mut();
        for (column, id) in ids.into_iter().enumerate() {
            let tile = tiles.get((id - 0x62) as usize);
            let base_x = (x + 12) as usize + column * 8;
            for row in 0..8usize {
                let py = y as usize + row;
                if py >= height {
                    continue;
                }
                let row_off = py * width;
                for pixel in 0..8usize {
                    let px = base_x + pixel;
                    if px >= width {
                        continue;
                    }
                    indices[row_off + px] =
                        if crate::battle_text_tiles::is_text_pixel(id as usize, pixel) {
                            0
                        } else {
                            tile.pixels[row][pixel]
                        };
                }
            }
        }
    }
    #[cfg(not(all(target_os = "none", target_arch = "arm")))]
    {
        use dotzuki_renderer::palette::GbColor;
        for (column, id) in ids.into_iter().enumerate() {
            let tile = tiles.get((id - 0x62) as usize);
            let base_x = x + 12 + column as u32 * 8;
            for row in 0..8usize {
                for pixel in 0..8usize {
                    let shade = if crate::battle_text_tiles::is_text_pixel(id as usize, pixel) {
                        0
                    } else {
                        tile.pixels[row][pixel]
                    };
                    fb.set_pixel_index(
                        base_x + pixel as u32,
                        y + row as u32,
                        GbColor::from_u8(shade),
                    );
                }
            }
        }
    }
    crate::embedded_font::draw_text_indexed("HP", x, y.saturating_sub(2), Rgba::BLACK, fb);
    Ok(())
}

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

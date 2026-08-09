use pokered_core::town_map_screen::TownMapScreenState;
use pokered_data::map_names::map_name_str;
use pokered_data::town_map_data::{decode_town_map_tilemap, town_map_position, TOWN_MAP_WIDTH};
use pokered_renderer::embedded_font::{draw_text, fill_tile};
use pokered_renderer::palette::{Palette, GRAYSCALE_PALETTE};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::{blit_single_tile, draw_text_box};

/// Draw the Town Map viewer: the full 20×18 Kanto tilemap, a selection reticle
/// around the browse cursor's landmark, a flashing "you are here" marker at the
/// player's current location, and the highlighted landmark's name in a box.
///
/// Mirrors the native app's render/town_map.rs::draw_town_map. The map
/// background + reticle need the `ResourceManager`; without it (headless /
/// no gfx assets) the fallback still shows the "you are here" marker and the
/// landmark name box — the same fallback the app uses.
pub fn draw_town_map(
    state: &TownMapScreenState,
    res: &mut Option<ResourceManager>,
    frame_counter: u64,
    fb: &mut FrameBuffer,
) {
    fb.clear(Rgba::WHITE);

    let bg_pal = &GRAYSCALE_PALETTE;
    // Reticle palette: color 0 (the sprite's white background/hollow) is
    // transparent so the framed landmark tile shows through.
    let cursor_pal = Palette::new(&[
        Rgba::TRANSPARENT,
        GRAYSCALE_PALETTE.colors[1],
        GRAYSCALE_PALETTE.colors[2],
        GRAYSCALE_PALETTE.colors[3],
    ]);

    if let Some(ref mut rm) = res {
        // 1. Background map — one of 16 sheet tiles per cell, row-major.
        if let Ok(sheet) = rm.load_town_map("town_map") {
            let ts = sheet.tileset.clone();
            for (i, &tile) in decode_town_map_tilemap().iter().enumerate() {
                let tx = (i % TOWN_MAP_WIDTH) as u32;
                let ty = (i / TOWN_MAP_WIDTH) as u32;
                blit_single_tile(fb, &ts, tile as usize, tx * TILE_SIZE, ty * TILE_SIZE, bg_pal);
            }
        }

        // 2. Selection reticle (16×16) centered on the highlighted landmark.
        if let Some((sx, sy, _)) = town_map_position(state.selected_map()) {
            if let Ok(cursor) = rm.load_town_map("town_map_cursor") {
                let cts = cursor.tileset.clone();
                let bx = ((sx as u32) * TILE_SIZE).saturating_sub(TILE_SIZE / 2);
                let by = ((sy as u32) * TILE_SIZE).saturating_sub(TILE_SIZE / 2);
                blit_single_tile(fb, &cts, 0, bx, by, &cursor_pal);
                blit_single_tile(fb, &cts, 1, bx + TILE_SIZE, by, &cursor_pal);
                blit_single_tile(fb, &cts, 2, bx, by + TILE_SIZE, &cursor_pal);
                blit_single_tile(fb, &cts, 3, bx + TILE_SIZE, by + TILE_SIZE, &cursor_pal);
            }
        }
    }

    // 3. Flashing "you are here" marker at the player's current location.
    if (frame_counter / 16) % 2 == 0 {
        if let Some((px, py, _)) = town_map_position(state.current_map()) {
            fill_tile((px as u32) * TILE_SIZE, (py as u32) * TILE_SIZE, Rgba::BLACK, fb);
        }
    }

    // 4. Highlighted landmark's name, in a box along the bottom three rows.
    draw_text_box(fb, 0, 15 * TILE_SIZE, 18, 1, Rgba::BLACK);
    if let Some((_, _, name)) = town_map_position(state.selected_map()) {
        draw_text(map_name_str(name), TILE_SIZE, 16 * TILE_SIZE, Rgba::BLACK, fb);
    }
}

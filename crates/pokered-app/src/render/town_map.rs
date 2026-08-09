use pokered_core::game_state::Lang;
use pokered_core::town_map_screen::{TownMapMode, TownMapScreenState};
use pokered_data::map_names::{map_name_str, map_name_str_zh};
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
/// FLY mode (`LoadTownMap_Fly`, engine/items/town_map.asm:141-249) draws the
/// original's chrome: "To" at the top-left, the town name beside it, the
/// Pidgey bird sprite (the first 16×16 frame of gfx/sprites/bird.2bpp —
/// `BirdSprite`, tiles $04-$07) centered on the selected landmark, and the
/// ▲▼ cursor markers at the top-right (TownMapUpArrow at (18,0), the ▼ glyph
/// at (19,0) — decoord 18/19, 0 in the asm).
pub fn draw_town_map(
    state: &TownMapScreenState,
    res: &mut Option<ResourceManager>,
    frame_counter: u64,
    fb: &mut FrameBuffer,
    lang: Lang,
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

        if state.mode() == TownMapMode::Fly {
            // 3. FLY chrome (LoadTownMap_Fly): "To" at (0,0), the landmark
            // name at (3,0), the ▲▼ cursor markers at (18,0)/(19,0).
            draw_text(if lang == Lang::Zh { "去" } else { "To" }, 0, 0, Rgba::BLACK, fb);
            if let Some((_, _, name)) = town_map_position(state.selected_map()) {
                let label = if lang == Lang::Zh { map_name_str_zh(name) } else { map_name_str(name) };
                draw_text(label, 3 * TILE_SIZE, 0, Rgba::BLACK, fb);
            }
            if let Ok(arrow) = rm.load_town_map("up_arrow") {
                let ats = arrow.tileset.clone();
                // TownMapUpArrow (gfx/town_map/up_arrow.1bpp) is the '▲'
                // glyph (charmap.asm:85); the '▼' is the font's cursor glyph.
                blit_single_tile(fb, &ats, 0, 18 * TILE_SIZE, 0, bg_pal);
            }
            draw_text("▼", 19 * TILE_SIZE, 0, Rgba::BLACK, fb);

            // 4. The Pidgey bird sprite over the selected landmark — the
            // first 16×16 frame of gfx/sprites/bird.png (`BirdSprite` +
            // BIRD_BASE_TILE $04, engine/items/town_map.asm:146-149). Like
            // the player marker it is centered on the landmark (OAM
            // coords x*8+24/y*8+24 minus the 4px 16×16 offset →
            // top-left at x*8+4, y*8+4). White pixels stay transparent.
            if let Ok(bird) = rm.load_sprite("bird") {
                let bts = bird.tileset.clone();
                let bird_pal = Palette::new(&[
                    Rgba::TRANSPARENT,
                    GRAYSCALE_PALETTE.colors[1],
                    GRAYSCALE_PALETTE.colors[2],
                    GRAYSCALE_PALETTE.colors[3],
                ]);
                if let Some((sx, sy, _)) = town_map_position(state.selected_map()) {
                    let bx = (sx as u32) * TILE_SIZE + TILE_SIZE / 2;
                    let by = (sy as u32) * TILE_SIZE + TILE_SIZE / 2;
                    blit_single_tile(fb, &bts, 0, bx, by, &bird_pal);
                    blit_single_tile(fb, &bts, 1, bx + TILE_SIZE, by, &bird_pal);
                    blit_single_tile(fb, &bts, 2, bx, by + TILE_SIZE, &bird_pal);
                    blit_single_tile(fb, &bts, 3, bx + TILE_SIZE, by + TILE_SIZE, &bird_pal);
                }
            }
        }
    }

    // 5. Flashing "you are here" marker at the player's current location.
    if (frame_counter / 16) % 2 == 0 {
        if let Some((px, py, _)) = town_map_position(state.current_map()) {
            fill_tile((px as u32) * TILE_SIZE, (py as u32) * TILE_SIZE, Rgba::BLACK, fb);
        }
    }

    // 6. Highlighted landmark's name, in a box along the bottom three rows
    // (View mode only — the FLY screen shows the name in the top row with
    // the original's "To" prompt).
    if state.mode() == TownMapMode::View {
        draw_text_box(fb, 0, 15 * TILE_SIZE, 18, 1, Rgba::BLACK);
        if let Some((_, _, name)) = town_map_position(state.selected_map()) {
            let label = if lang == Lang::Zh { map_name_str_zh(name) } else { map_name_str(name) };
            draw_text(label, TILE_SIZE, 16 * TILE_SIZE, Rgba::BLACK, fb);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render_config::RenderConfig;
    use pokered_core::town_map_screen::TownMapScreenState;
    use pokered_data::maps::MapId;

    #[test]
    fn fly_mode_draws_to_prompt_bird_and_arrows() {
        let state = TownMapScreenState::new_fly(
            MapId::PalletTown,
            vec![MapId::PalletTown, MapId::ViridianCity],
        );
        let mut res = pokered_renderer::resource::AssetRoot::auto_detect()
            .ok()
            .map(pokered_renderer::resource::ResourceManager::new);
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        draw_town_map(&state, &mut res, 0, &mut fb, Lang::default());

        // "To" prompt at the top-left (LoadTownMap_Fly's ToText at (0,0)).
        let to_ink = (0..2 * TILE_SIZE).any(|dy| {
            (0..2 * TILE_SIZE).any(|dx| {
                fb.get_pixel(dx, dy).is_some_and(|c| c != Rgba::WHITE)
            })
        });
        assert!(to_ink, "the To prompt is drawn at the top-left");

        // ▲ (up_arrow asset) at (18,0) and ▼ at (19,0).
        let up_ink = (0..TILE_SIZE).any(|dy| {
            (0..TILE_SIZE).any(|dx| {
                fb.get_pixel(18 * TILE_SIZE + dx, dy)
                    .is_some_and(|c| c != Rgba::WHITE)
            })
        });
        let down_ink = (0..TILE_SIZE).any(|dy| {
            (0..TILE_SIZE).any(|dx| {
                fb.get_pixel(19 * TILE_SIZE + dx, dy)
                    .is_some_and(|c| c != Rgba::WHITE)
            })
        });
        assert!(up_ink, "the ▲ cursor marker is drawn at (18,0)");
        assert!(down_ink, "the ▼ cursor marker is drawn at (19,0)");

        // The bird sprite centered on the selected landmark (Pallet Town at
        // (3,15)-ish in the 20×18 map): ink in the 16×16 window around it
        // that is NOT the plain map tile — the bird adds pixels.
        if let Some((sx, sy, _)) = town_map_position(state.selected_map()) {
            let bx = (sx as u32) * TILE_SIZE + TILE_SIZE / 2;
            let by = (sy as u32) * TILE_SIZE + TILE_SIZE / 2;
            let bird_ink = (0..2 * TILE_SIZE).any(|dy| {
                (0..2 * TILE_SIZE).any(|dx| {
                    fb.get_pixel(bx + dx, by + dy).is_some_and(|c| c != Rgba::WHITE)
                })
            });
            assert!(bird_ink, "the bird sprite is drawn over the landmark");
        } else {
            panic!("Pallet Town has a town-map position");
        }

        let path = std::env::temp_dir().join("town_map_fly_test.png");
        fb.save_png(&path).expect("save fly map png");
    }
}

use crate::alloc_prelude::*;
use pokered_core::game_state::Lang;
use pokered_core::town_map_screen::{TownMapMode, TownMapScreenState};
use pokered_data::map_names::{map_name_str, map_name_str_zh};
use pokered_data::town_map_data::{decode_town_map_tilemap, town_map_position, TOWN_MAP_WIDTH};
use pokered_renderer::embedded_font::{draw_text, fill_tile, measure_text};
use pokered_renderer::palette::{Palette, GRAYSCALE_PALETTE};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::{blit_single_tile, draw_text_box};

/// Region changed by one phase of the current-location marker.
pub fn marker_damage(current_map: pokered_data::maps::MapId) -> Option<pokered_ui::DamageRect> {
    town_map_position(current_map).map(|(x, y, _)| {
        pokered_ui::DamageRect::new(
            (x as u32 + 2) * TILE_SIZE,
            (y as u32 + 1) * TILE_SIZE,
            TILE_SIZE,
            TILE_SIZE,
        )
    })
}

/// Regions changed by [`redraw_town_map_cursor`].
pub fn cursor_damage(
    state: &TownMapScreenState,
    previous_map: pokered_data::maps::MapId,
    lang: Lang,
) -> Option<[pokered_ui::DamageRect; 4]> {
    let (old_x, old_y, old_name) = town_map_position(previous_map)?;
    let (new_x, new_y, new_name) = town_map_position(state.selected_map())?;
    let marker = marker_damage(state.current_map())?;
    let label = if state.mode() == TownMapMode::Fly {
        pokered_ui::DamageRect::new(0, 0, 160, 2 * TILE_SIZE)
    } else {
        let overlaps_view_box = [(old_y, old_name), (new_y, new_name)]
            .iter()
            .any(|(y, _)| *y as u32 * TILE_SIZE + 5 + 16 > 15 * TILE_SIZE);
        let marker_is_behind_box = town_map_position(state.current_map())
            .is_some_and(|(_, y, _)| (y as u32 + 1) * TILE_SIZE >= 15 * TILE_SIZE);
        if overlaps_view_box || marker_is_behind_box {
            pokered_ui::DamageRect::new(0, 15 * TILE_SIZE, 160, 3 * TILE_SIZE)
        } else {
            let label_width = |name| {
                measure_text(if lang == Lang::Zh {
                    map_name_str_zh(name)
                } else {
                    map_name_str(name)
                })
            };
            pokered_ui::DamageRect::new(
                TILE_SIZE,
                16 * TILE_SIZE,
                label_width(old_name).max(label_width(new_name)).min(18 * TILE_SIZE),
                13,
            )
        }
    };
    Some([
        pokered_ui::DamageRect::new(
            old_x as u32 * TILE_SIZE + 12,
            old_y as u32 * TILE_SIZE + 5,
            16,
            16,
        ),
        pokered_ui::DamageRect::new(
            new_x as u32 * TILE_SIZE + 12,
            new_y as u32 * TILE_SIZE + 5,
            16,
            16,
        ),
        marker,
        label,
    ])
}

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
            for (i, &tile) in decode_town_map_tilemap().iter().enumerate() {
                let tx = (i % TOWN_MAP_WIDTH) as u32;
                let ty = (i / TOWN_MAP_WIDTH) as u32;
                blit_single_tile(
                    fb,
                    &sheet.tileset,
                    tile as usize,
                    tx * TILE_SIZE,
                    ty * TILE_SIZE,
                    bg_pal,
                );
            }
        }

        // 2. Selection reticle (16×16) centered on the highlighted landmark.
        // Entry coords are NOT screen tiles: the original converts them via
        // TownMapCoordsToOAMCoords (OAM x/y = v*8+24), WriteTownMapSpriteOAM
        // (net x−4, y−3) and the hardware OAM offset (x−8, y−16), so a 16×16
        // sprite's top-left lands at (x*8+12, y*8+5) — centered on the town
        // square baked into the tilemap at tile (x+2, y+1).
        if let Some((sx, sy, _)) = town_map_position(state.selected_map()) {
            if let Ok(cursor) = rm.load_town_map("town_map_cursor") {
                let bx = (sx as u32) * TILE_SIZE + 12;
                let by = (sy as u32) * TILE_SIZE + 5;
                blit_single_tile(fb, &cursor.tileset, 0, bx, by, &cursor_pal);
                blit_single_tile(
                    fb,
                    &cursor.tileset,
                    1,
                    bx + TILE_SIZE,
                    by,
                    &cursor_pal,
                );
                blit_single_tile(
                    fb,
                    &cursor.tileset,
                    2,
                    bx,
                    by + TILE_SIZE,
                    &cursor_pal,
                );
                blit_single_tile(
                    fb,
                    &cursor.tileset,
                    3,
                    bx + TILE_SIZE,
                    by + TILE_SIZE,
                    &cursor_pal,
                );
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
                // TownMapUpArrow (gfx/town_map/up_arrow.1bpp) is the '▲'
                // glyph (charmap.asm:85); the '▼' is the font's cursor glyph.
                blit_single_tile(fb, &arrow.tileset, 0, 18 * TILE_SIZE, 0, bg_pal);
            }
            draw_text("▼", 19 * TILE_SIZE, 0, Rgba::BLACK, fb);

            // 4. The Pidgey bird sprite over the selected landmark — the
            // first 16×16 frame of gfx/sprites/bird.png (`BirdSprite` +
            // BIRD_BASE_TILE $04, engine/items/town_map.asm:146-149). It
            // shares the reticle's OAM-derived anchor (top-left at
            // x*8+12, y*8+5). White pixels stay transparent.
            if let Ok(bird) = rm.load_sprite("bird") {
                let bird_pal = Palette::new(&[
                    Rgba::TRANSPARENT,
                    GRAYSCALE_PALETTE.colors[1],
                    GRAYSCALE_PALETTE.colors[2],
                    GRAYSCALE_PALETTE.colors[3],
                ]);
                if let Some((sx, sy, _)) = town_map_position(state.selected_map()) {
                    let bx = (sx as u32) * TILE_SIZE + 12;
                    let by = (sy as u32) * TILE_SIZE + 5;
                    blit_single_tile(fb, &bird.tileset, 0, bx, by, &bird_pal);
                    blit_single_tile(
                        fb,
                        &bird.tileset,
                        1,
                        bx + TILE_SIZE,
                        by,
                        &bird_pal,
                    );
                    blit_single_tile(
                        fb,
                        &bird.tileset,
                        2,
                        bx,
                        by + TILE_SIZE,
                        &bird_pal,
                    );
                    blit_single_tile(
                        fb,
                        &bird.tileset,
                        3,
                        bx + TILE_SIZE,
                        by + TILE_SIZE,
                        &bird_pal,
                    );
                }
            }
        }
    }

    // 5. Flashing "you are here" marker at the player's current location,
    // over the town square baked into the tilemap at tile (x+2, y+1).
    if (frame_counter / 16) % 2 == 0 {
        if let Some((px, py, _)) = town_map_position(state.current_map()) {
            fill_tile((px as u32) * TILE_SIZE + 16, (py as u32) * TILE_SIZE + 8, Rgba::BLACK, fb);
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

fn restore_town_map_marker_layers(
    state: &TownMapScreenState,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    lang: Lang,
    marker_tx: usize,
    marker_ty: usize,
) {
    if let Some(rm) = res.as_mut() {
        if let Some(&tile) = decode_town_map_tilemap()
            .get(marker_ty * TOWN_MAP_WIDTH + marker_tx)
        {
            if let Ok(sheet) = rm.load_town_map("town_map") {
                fill_tile(
                    marker_tx as u32 * TILE_SIZE,
                    marker_ty as u32 * TILE_SIZE,
                    Rgba::WHITE,
                    fb,
                );
                blit_single_tile(
                    fb,
                    &sheet.tileset,
                    tile as usize,
                    marker_tx as u32 * TILE_SIZE,
                    marker_ty as u32 * TILE_SIZE,
                    &GRAYSCALE_PALETTE,
                );
            }
        }

        let overlay_pal = Palette::new(&[
            Rgba::TRANSPARENT,
            GRAYSCALE_PALETTE.colors[1],
            GRAYSCALE_PALETTE.colors[2],
            GRAYSCALE_PALETTE.colors[3],
        ]);
        if let Some((sx, sy, _)) = town_map_position(state.selected_map()) {
            let bx = sx as u32 * TILE_SIZE + 12;
            let by = sy as u32 * TILE_SIZE + 5;
            if let Ok(cursor) = rm.load_town_map("town_map_cursor") {
                blit_single_tile(fb, &cursor.tileset, 0, bx, by, &overlay_pal);
                blit_single_tile(
                    fb,
                    &cursor.tileset,
                    1,
                    bx + TILE_SIZE,
                    by,
                    &overlay_pal,
                );
                blit_single_tile(
                    fb,
                    &cursor.tileset,
                    2,
                    bx,
                    by + TILE_SIZE,
                    &overlay_pal,
                );
                blit_single_tile(
                    fb,
                    &cursor.tileset,
                    3,
                    bx + TILE_SIZE,
                    by + TILE_SIZE,
                    &overlay_pal,
                );
            }
            if state.mode() == TownMapMode::Fly {
                draw_text(if lang == Lang::Zh { "去" } else { "To" }, 0, 0, Rgba::BLACK, fb);
                if let Some((_, _, name)) = town_map_position(state.selected_map()) {
                    let label = if lang == Lang::Zh {
                        map_name_str_zh(name)
                    } else {
                        map_name_str(name)
                    };
                    draw_text(label, 3 * TILE_SIZE, 0, Rgba::BLACK, fb);
                }
                if let Ok(arrow) = rm.load_town_map("up_arrow") {
                    blit_single_tile(fb, &arrow.tileset, 0, 18 * TILE_SIZE, 0, &GRAYSCALE_PALETTE);
                }
                draw_text("▼", 19 * TILE_SIZE, 0, Rgba::BLACK, fb);
                if let Ok(bird) = rm.load_sprite("bird") {
                    blit_single_tile(fb, &bird.tileset, 0, bx, by, &overlay_pal);
                    blit_single_tile(
                        fb,
                        &bird.tileset,
                        1,
                        bx + TILE_SIZE,
                        by,
                        &overlay_pal,
                    );
                    blit_single_tile(
                        fb,
                        &bird.tileset,
                        2,
                        bx,
                        by + TILE_SIZE,
                        &overlay_pal,
                    );
                    blit_single_tile(
                        fb,
                        &bird.tileset,
                        3,
                        bx + TILE_SIZE,
                        by + TILE_SIZE,
                        &overlay_pal,
                    );
                }
            }
        }
    }
}

/// Repaint only the flashing 8×8 current-location marker.
///
/// The marker is drawn after the selection reticle and FLY bird, so turning
/// it off must restore those layers as well as the underlying map tile.
pub fn redraw_town_map_marker(
    state: &TownMapScreenState,
    res: &mut Option<ResourceManager>,
    frame_counter: u64,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let Some((px, py, _)) = town_map_position(state.current_map()) else {
        return;
    };
    let marker_tx = px as usize + 2;
    let marker_ty = py as usize + 1;
    if state.mode() == TownMapMode::View && marker_ty as u32 * TILE_SIZE >= 15 * TILE_SIZE {
        // The bottom location-name box is drawn after the marker and covers
        // it completely, so its animation has no visible pixels to update.
        return;
    }
    if (frame_counter / 16) % 2 == 0 {
        fill_tile(
            marker_tx as u32 * TILE_SIZE,
            marker_ty as u32 * TILE_SIZE,
            Rgba::BLACK,
            fb,
        );
    } else {
        restore_town_map_marker_layers(state, res, fb, lang, marker_tx, marker_ty);
    }
}

fn restore_town_map_background_rect(
    rm: &mut ResourceManager,
    fb: &mut FrameBuffer,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) {
    let first_tx = x / TILE_SIZE;
    let first_ty = y / TILE_SIZE;
    let last_tx = (x + width.saturating_sub(1)) / TILE_SIZE;
    let last_ty = (y + height.saturating_sub(1)) / TILE_SIZE;
    let tilemap = decode_town_map_tilemap();
    let Ok(sheet) = rm.load_town_map("town_map") else {
        return;
    };
    for ty in first_ty..=last_ty.min(17) {
        for tx in first_tx..=last_tx.min((TOWN_MAP_WIDTH - 1) as u32) {
            let Some(&tile) = tilemap.get(ty as usize * TOWN_MAP_WIDTH + tx as usize) else {
                continue;
            };
            fill_tile(tx * TILE_SIZE, ty * TILE_SIZE, Rgba::WHITE, fb);
            blit_single_tile(
                fb,
                &sheet.tileset,
                tile as usize,
                tx * TILE_SIZE,
                ty * TILE_SIZE,
                &GRAYSCALE_PALETTE,
            );
        }
    }
}

/// Repaint the old/new selection area and its localized label.
pub fn redraw_town_map_cursor(
    state: &TownMapScreenState,
    previous_map: pokered_data::maps::MapId,
    res: &mut Option<ResourceManager>,
    frame_counter: u64,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    if let Some(rm) = res.as_mut() {
        if let Some((sx, sy, _)) = town_map_position(previous_map) {
            restore_town_map_background_rect(
                rm,
                fb,
                sx as u32 * TILE_SIZE + 12,
                sy as u32 * TILE_SIZE + 5,
                16,
                16,
            );
        }
        if state.mode() == TownMapMode::Fly {
            // The embedded font can extend one pixel below its nominal row.
            restore_town_map_background_rect(rm, fb, 0, 0, 160, 2 * TILE_SIZE);
        }
    }
    if let Some((px, py, _)) = town_map_position(state.current_map()) {
        let marker_tx = px as usize + 2;
        let marker_ty = py as usize + 1;
        restore_town_map_marker_layers(state, res, fb, lang, marker_tx, marker_ty);
        if (frame_counter / 16) % 2 == 0
            && (state.mode() != TownMapMode::View
                || marker_ty as u32 * TILE_SIZE < 15 * TILE_SIZE)
        {
            fill_tile(
                marker_tx as u32 * TILE_SIZE,
                marker_ty as u32 * TILE_SIZE,
                Rgba::BLACK,
                fb,
            );
        }
    }
    if state.mode() == TownMapMode::View {
        let label_for = |map| {
            town_map_position(map).map(|(_, _, name)| {
                if lang == Lang::Zh {
                    map_name_str_zh(name)
                } else {
                    map_name_str(name)
                }
            })
        };
        let previous_label = label_for(previous_map);
        let current_label = label_for(state.selected_map());
        let clear_width = previous_label
            .map_or(0, measure_text)
            .max(current_label.map_or(0, measure_text))
            .min(18 * TILE_SIZE);
        let reticle_overlaps_box = [previous_map, state.selected_map()]
            .iter()
            .filter_map(|&map| town_map_position(map))
            .any(|(_, y, _)| y as u32 * TILE_SIZE + 5 + 16 > 15 * TILE_SIZE);
        let marker_is_behind_box = town_map_position(state.current_map())
            .is_some_and(|(_, y, _)| (y as u32 + 1) * TILE_SIZE >= 15 * TILE_SIZE);
        if reticle_overlaps_box || marker_is_behind_box {
            // These layers are conceptually below the box. Re-establish the
            // complete box when restoring them touched its pixels.
            draw_text_box(fb, 0, 15 * TILE_SIZE, 18, 1, Rgba::BLACK);
        } else {
            // The box itself is static. Clear only the old/new label union
            // across its 13-pixel font bounds; keep the border intact.
            for y in 16 * TILE_SIZE..16 * TILE_SIZE + 13 {
                for x in TILE_SIZE..TILE_SIZE + clear_width {
                    fb.set_pixel(x, y, Rgba::WHITE);
                }
            }
        }
        if let Some(label) = current_label {
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

    fn assert_framebuffers_equal(actual: &FrameBuffer, expected: &FrameBuffer) {
        assert_eq!(actual.width(), expected.width());
        assert_eq!(actual.height(), expected.height());
        for y in 0..actual.height() {
            for x in 0..actual.width() {
                assert_eq!(
                    actual.get_pixel(x, y),
                    expected.get_pixel(x, y),
                    "framebuffer mismatch at ({x}, {y})",
                );
            }
        }
    }

    #[test]
    fn marker_repaint_matches_full_draw_in_view_and_fly_modes() {
        let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
        let cases = [
            TownMapScreenState::new(MapId::PalletTown),
            TownMapScreenState::new_fly(
                MapId::PalletTown,
                vec![MapId::PalletTown, MapId::ViridianCity],
            ),
        ];

        for state in cases {
            for lang in [Lang::En, Lang::Zh] {
                let mut res = pokered_renderer::resource::AssetRoot::auto_detect()
                    .ok()
                    .map(pokered_renderer::resource::ResourceManager::new);
                let mut actual = FrameBuffer::new(config, Rgba::WHITE);
                draw_town_map(&state, &mut res, 0, &mut actual, lang);
                redraw_town_map_marker(&state, &mut res, 16, &mut actual, lang);
                let mut expected = FrameBuffer::new(config, Rgba::WHITE);
                draw_town_map(&state, &mut res, 16, &mut expected, lang);
                assert_framebuffers_equal(&actual, &expected);

                redraw_town_map_marker(&state, &mut res, 32, &mut actual, lang);
                let mut expected = FrameBuffer::new(config, Rgba::WHITE);
                draw_town_map(&state, &mut res, 32, &mut expected, lang);
                assert_framebuffers_equal(&actual, &expected);
            }
        }
    }

    #[test]
    fn cursor_repaint_matches_full_draw_in_view_and_fly_modes() {
        let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
        for lang in [Lang::En, Lang::Zh] {
            let mut cases = [
                TownMapScreenState::new(MapId::PalletTown),
                TownMapScreenState::new(MapId::CinnabarIsland),
                TownMapScreenState::new_fly(
                    MapId::PalletTown,
                    vec![MapId::PalletTown, MapId::ViridianCity],
                ),
            ];
            for state in &mut cases {
                let mut res = pokered_renderer::resource::AssetRoot::auto_detect()
                    .ok()
                    .map(pokered_renderer::resource::ResourceManager::new);
                let mut actual = FrameBuffer::new(config, Rgba::WHITE);
                draw_town_map(state, &mut res, 0, &mut actual, lang);
                let previous = state.selected_map();
                let input = pokered_core::town_map_screen::TownMapScreenInput {
                    up: state.mode() == TownMapMode::Fly,
                    down: state.mode() == TownMapMode::View,
                    a: false,
                    b: false,
                };
                state.update_frame(input);
                redraw_town_map_cursor(state, previous, &mut res, 16, &mut actual, lang);

                let mut expected = FrameBuffer::new(config, Rgba::WHITE);
                draw_town_map(state, &mut res, 16, &mut expected, lang);
                assert_framebuffers_equal(&actual, &expected);
            }
        }
    }

    #[test]
    fn view_cursor_repaint_matches_full_draw_for_every_landmark() {
        let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
        for lang in [Lang::En, Lang::Zh] {
            let mut state = TownMapScreenState::new(
                pokered_data::town_map_data::TOWN_MAP_ORDER[0],
            );
            let mut res = pokered_renderer::resource::AssetRoot::auto_detect()
                .ok()
                .map(pokered_renderer::resource::ResourceManager::new);
            let mut actual = FrameBuffer::new(config, Rgba::WHITE);
            draw_town_map(&state, &mut res, 0, &mut actual, lang);

            for frame in 1..pokered_data::town_map_data::TOWN_MAP_ORDER.len() {
                let previous = state.selected_map();
                state.update_frame(pokered_core::town_map_screen::TownMapScreenInput {
                    down: true,
                    ..Default::default()
                });
                redraw_town_map_cursor(
                    &state,
                    previous,
                    &mut res,
                    frame as u64 * 16,
                    &mut actual,
                    lang,
                );

                let mut expected = FrameBuffer::new(config, Rgba::WHITE);
                draw_town_map(
                    &state,
                    &mut res,
                    frame as u64 * 16,
                    &mut expected,
                    lang,
                );
                assert_framebuffers_equal(&actual, &expected);
            }
        }
    }

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

        // The bird sprite centered on the selected landmark's town square
        // (16×16 top-left at x*8+12, y*8+5 — see draw_town_map): ink in the
        // 16×16 window around it that is NOT the plain map tile — the bird
        // adds pixels.
        if let Some((sx, sy, _)) = town_map_position(state.selected_map()) {
            let bx = (sx as u32) * TILE_SIZE + 12;
            let by = (sy as u32) * TILE_SIZE + 5;
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

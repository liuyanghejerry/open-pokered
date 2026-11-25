use crate::alloc_prelude::*;
use pokered_core::game_state::Lang;
use pokered_core::town_map_screen::{TownMapMode, TownMapScreenState};
use pokered_data::map_names::{map_name_str, map_name_str_zh};
use pokered_data::town_map_data::{decode_town_map_tilemap, town_map_position, TOWN_MAP_WIDTH};
use pokered_renderer::embedded_font::{draw_text, fill_tile};
use pokered_renderer::palette::{Palette, GRAYSCALE_PALETTE};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::{blit_single_tile, draw_text_box};

fn view_box_top(_lang: Lang) -> u32 {
    // The project font is 10px tall in both languages.
    14 * TILE_SIZE
}

fn view_box_height(lang: Lang) -> u32 {
    144 - view_box_top(lang)
}

fn draw_view_name_box(state: &TownMapScreenState, fb: &mut FrameBuffer, lang: Lang) {
    let top = view_box_top(lang);
    let interior_rows = view_box_height(lang) / TILE_SIZE - 2;
    draw_text_box(fb, 0, top, 18, interior_rows, Rgba::BLACK);
    if let Some((_, _, name)) = town_map_position(state.selected_map()) {
        let label = if lang == Lang::Zh { map_name_str_zh(name) } else { map_name_str(name) };
        draw_text(label, TILE_SIZE, top + TILE_SIZE, Rgba::BLACK, fb);
    }
}

const BLINK_HALF_PERIOD: u64 = 25;

#[inline]
fn town_map_blink_visible(frame_counter: u64) -> bool {
    (frame_counter % (BLINK_HALF_PERIOD * 2)) < BLINK_HALF_PERIOD
}

/// Region changed by one blink transition of the View-mode selection cursor
/// (`TownMapSpriteBlinkingAnimation`, `engine/items/town_map.asm`).
pub fn marker_damage(
    state: &TownMapScreenState,
    lang: Lang,
) -> Option<pokered_ui::DamageRect> {
    if state.mode() != TownMapMode::View {
        return None;
    }
    let (sx, sy, _) = town_map_position(state.selected_map())?;
    let bx = sx as u32 * TILE_SIZE + 12;
    let by = sy as u32 * TILE_SIZE + 5;
    let box_top = view_box_top(lang);
    if by >= box_top {
        return None;
    }
    let height = 16.min(box_top - by);
    Some(pokered_ui::DamageRect::new(bx, by, 16, height))
}

/// Regions changed by [`redraw_town_map_cursor`].
pub fn cursor_damage(
    state: &TownMapScreenState,
    previous_map: pokered_data::maps::MapId,
    lang: Lang,
) -> Option<[pokered_ui::DamageRect; 3]> {
    let (old_x, old_y, _) = town_map_position(previous_map)?;
    let (new_x, new_y, _) = town_map_position(state.selected_map())?;
    let label = if state.mode() == TownMapMode::Fly {
        pokered_ui::DamageRect::new(0, 0, 160, 2 * TILE_SIZE)
    } else {
        pokered_ui::DamageRect::new(0, view_box_top(lang), 160, view_box_height(lang))
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
        label,
    ])
}

fn draw_sprite_16x16(
    fb: &mut FrameBuffer,
    tileset: &pokered_renderer::tile::TileSet,
    bx: u32,
    by: u32,
    pal: &Palette,
) {
    blit_single_tile(fb, tileset, 0, bx, by, pal);
    blit_single_tile(fb, tileset, 1, bx + TILE_SIZE, by, pal);
    blit_single_tile(fb, tileset, 2, bx, by + TILE_SIZE, pal);
    blit_single_tile(fb, tileset, 3, bx + TILE_SIZE, by + TILE_SIZE, pal);
}

/// Draw the Town Map viewer (`DisplayTownMap` / `LoadTownMap_Fly`,
/// `engine/items/town_map.asm`):
/// - View mode draws the steady Player sprite (`wShadowOAMSprite36..39`) at
///   `current_map()`, the 25-frame blinking `TownMapCursor` reticle
///   (`wShadowOAMSprite04..07`) at `selected_map()`, and the landmark name box.
/// - FLY mode draws `"To"` + landmark name + `▲▼` in the top row, the steady
///   Player sprite (`Sprite36..39`) at `current_map()`, and the steady Pidgey
///   bird sprite (`Sprite32..35`) at `selected_map()`.
pub fn draw_town_map(
    state: &TownMapScreenState,
    res: &mut Option<ResourceManager>,
    frame_counter: u64,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    fb.clear(Rgba::WHITE);

    let bg_pal = &GRAYSCALE_PALETTE;
    let sprite_pal = Palette::new(&[
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

        // 2. Player sprite (`DrawPlayerOrBirdSprite` b=$0 → Sprite36..39,
        // kept visible continuously by `TownMapSpriteBlinkingAnimation`).
        if let Some((px, py, _)) = town_map_position(state.current_map()) {
            if let Ok(player) = rm.load_sprite("red") {
                let bx = (px as u32) * TILE_SIZE + 12;
                let by = (py as u32) * TILE_SIZE + 5;
                draw_sprite_16x16(fb, &player.tileset, bx, by, &sprite_pal);
            }
        }

        match state.mode() {
            TownMapMode::View => {
                // 3a. Blinking selection reticle (`wShadowOAMSprite04..07`,
                // 25 frames ON / 25 frames OFF). Lower OAM index overlays Red.
                if town_map_blink_visible(frame_counter) {
                    if let Some((sx, sy, _)) = town_map_position(state.selected_map()) {
                        if let Ok(cursor) = rm.load_town_map("town_map_cursor") {
                            let bx = (sx as u32) * TILE_SIZE + 12;
                            let by = (sy as u32) * TILE_SIZE + 5;
                            draw_sprite_16x16(fb, &cursor.tileset, bx, by, &sprite_pal);
                        }
                    }
                }
            }
            TownMapMode::Fly => {
                // 3b. FLY chrome (`LoadTownMap_Fly`): "To" at (0,0), landmark
                // name at (3,0), ▲▼ at (18,0)/(19,0), and steady Bird sprite
                // (`Sprite32..35`) over `selected_map()`.
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
                    blit_single_tile(fb, &arrow.tileset, 0, 18 * TILE_SIZE, 0, bg_pal);
                }
                draw_text("▼", 19 * TILE_SIZE, 0, Rgba::BLACK, fb);
                if let Some((sx, sy, _)) = town_map_position(state.selected_map()) {
                    if let Ok(bird) = rm.load_sprite("bird") {
                        let bx = (sx as u32) * TILE_SIZE + 12;
                        let by = (sy as u32) * TILE_SIZE + 5;
                        draw_sprite_16x16(fb, &bird.tileset, bx, by, &sprite_pal);
                    }
                }
            }
        }
    }

    // 4. Highlighted landmark's name box in View mode.
    if state.mode() == TownMapMode::View {
        draw_view_name_box(state, fb, lang);
    }
}

fn redraw_town_map_sprites_and_chrome(
    state: &TownMapScreenState,
    rm: &mut ResourceManager,
    frame_counter: u64,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let sprite_pal = Palette::new(&[
        Rgba::TRANSPARENT,
        GRAYSCALE_PALETTE.colors[1],
        GRAYSCALE_PALETTE.colors[2],
        GRAYSCALE_PALETTE.colors[3],
    ]);
    if let Some((px, py, _)) = town_map_position(state.current_map()) {
        if let Ok(player) = rm.load_sprite("red") {
            draw_sprite_16x16(
                fb,
                &player.tileset,
                px as u32 * TILE_SIZE + 12,
                py as u32 * TILE_SIZE + 5,
                &sprite_pal,
            );
        }
    }
    match state.mode() {
        TownMapMode::View => {
            if town_map_blink_visible(frame_counter) {
                if let Some((sx, sy, _)) = town_map_position(state.selected_map()) {
                    if let Ok(cursor) = rm.load_town_map("town_map_cursor") {
                        draw_sprite_16x16(
                            fb,
                            &cursor.tileset,
                            sx as u32 * TILE_SIZE + 12,
                            sy as u32 * TILE_SIZE + 5,
                            &sprite_pal,
                        );
                    }
                }
            }
        }
        TownMapMode::Fly => {
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
            if let Some((sx, sy, _)) = town_map_position(state.selected_map()) {
                if let Ok(bird) = rm.load_sprite("bird") {
                    draw_sprite_16x16(
                        fb,
                        &bird.tileset,
                        sx as u32 * TILE_SIZE + 12,
                        sy as u32 * TILE_SIZE + 5,
                        &sprite_pal,
                    );
                }
            }
        }
    }
}

/// Repaint only the blinking 16×16 selection reticle in View mode.
pub fn redraw_town_map_marker(
    state: &TownMapScreenState,
    res: &mut Option<ResourceManager>,
    frame_counter: u64,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    if state.mode() != TownMapMode::View {
        return;
    }
    let Some((sx, sy, _)) = town_map_position(state.selected_map()) else {
        return;
    };
    let bx = sx as u32 * TILE_SIZE + 12;
    let by = sy as u32 * TILE_SIZE + 5;
    let box_top = view_box_top(lang);
    if by >= box_top {
        return;
    }
    if let Some(rm) = res.as_mut() {
        restore_town_map_background_rect(rm, fb, bx, by, 16, 16);
        // Restore steady Player sprite if its 16×16 window overlaps the tile
        // neighborhood just wiped by `restore_town_map_background_rect`.
        if let Some((px, py, _)) = town_map_position(state.current_map()) {
            let pbx = px as u32 * TILE_SIZE + 12;
            let pby = py as u32 * TILE_SIZE + 5;
            if pbx.abs_diff(bx) < 24 && pby.abs_diff(by) < 24 {
                restore_town_map_background_rect(rm, fb, pbx, pby, 16, 16);
            }
        }
        redraw_town_map_sprites_and_chrome(state, rm, frame_counter, fb, lang);
    }
    draw_view_name_box(state, fb, lang);
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
        if let Some((nx, ny, _)) = town_map_position(state.selected_map()) {
            restore_town_map_background_rect(
                rm,
                fb,
                nx as u32 * TILE_SIZE + 12,
                ny as u32 * TILE_SIZE + 5,
                16,
                16,
            );
        }
        if let Some((px, py, _)) = town_map_position(state.current_map()) {
            restore_town_map_background_rect(
                rm,
                fb,
                px as u32 * TILE_SIZE + 12,
                py as u32 * TILE_SIZE + 5,
                16,
                16,
            );
        }
        if state.mode() == TownMapMode::Fly {
            // The embedded font can extend one pixel below its nominal row.
            restore_town_map_background_rect(rm, fb, 0, 0, 160, 2 * TILE_SIZE);
        }
        redraw_town_map_sprites_and_chrome(state, rm, frame_counter, fb, lang);
    }
    if state.mode() == TownMapMode::View {
        // Reserve 16px for the project font above the original bottom border.
        // Restore the box in complete-frame order.
        draw_view_name_box(state, fb, lang);
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
    fn view_name_leaves_space_for_the_project_font_above_the_bottom_border() {
        for lang in [Lang::En, Lang::Zh] {
        for map in [MapId::PalletTown, MapId::ViridianCity, MapId::LavenderTown] {
            let state = TownMapScreenState::new(map);
            let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            draw_town_map(&state, &mut None, 16, &mut fb, lang);
            assert_eq!(view_box_top(Lang::Zh), 112);
            assert_eq!(view_box_top(Lang::En), 112);
            for y in 134..136 { for x in 8..152 {
                assert_eq!(fb.get_pixel(x, y), Some(Rgba::WHITE), "glyph touches bottom border at {x},{y}");
            } }
            assert_eq!(fb.get_pixel(8, 138), Some(Rgba::BLACK), "restore original bottom border");
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
                for &frame in &[24u64, 25, 49, 50] {
                    redraw_town_map_marker(&state, &mut res, frame, &mut actual, lang);
                    let mut expected = FrameBuffer::new(config, Rgba::WHITE);
                    draw_town_map(&state, &mut res, frame, &mut expected, lang);
                    assert_framebuffers_equal(&actual, &expected);
                }
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
                redraw_town_map_cursor(state, previous, &mut res, 25, &mut actual, lang);

                let mut expected = FrameBuffer::new(config, Rgba::WHITE);
                draw_town_map(state, &mut res, 25, &mut expected, lang);
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
                    frame as u64 * 25,
                    &mut actual,
                    lang,
                );

                let mut expected = FrameBuffer::new(config, Rgba::WHITE);
                draw_town_map(
                    &state,
                    &mut res,
                    frame as u64 * 25,
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

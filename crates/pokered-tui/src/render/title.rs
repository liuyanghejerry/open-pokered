use pokered_core::data::wild_data::GameVersion;
use pokered_core::title_screen::TitleScreenState;
use pokered_data::layout_constants;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::layout;
use pokered_renderer::palette::{Palette, PaletteState, GRAYSCALE_PALETTE};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::{blit_tileset, species_to_sprite_name};

pub fn draw_title_screen(
    state: &TitleScreenState,
    is_copyright: bool,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    let fade = state.fade_progress();
    let bg_color = if fade > 0.0 {
        let v = (255.0 * (1.0 - fade)) as u8;
        Rgba::rgb(v, v, v)
    } else {
        Rgba::WHITE
    };
    fb.clear(bg_color);

    let bg_pal = &GRAYSCALE_PALETTE;
    // Color 0 = transparent so fb.clear(bg_color) shows through during fade
    let transparent_bg_pal = Palette::new(&[
        Rgba::TRANSPARENT,
        GRAYSCALE_PALETTE.colors[1],
        GRAYSCALE_PALETTE.colors[2],
        GRAYSCALE_PALETTE.colors[3],
    ]);
    let mut palette_state = PaletteState::new(GRAYSCALE_PALETTE);
    palette_state.obp0 = 0b11100100;
    let sprite_pal = &palette_state.obj_palette0();

    if let Some(ref mut rm) = res {
        if is_copyright {
            draw_text("©1995 GAME FREAK", 24, 64, Rgba::BLACK, fb);
            return;
        }

        if let Ok(logo) = rm.load_title("pokemon_logo") {
            let lw = logo.source_size.0;
            let tiles_per_row = lw / TILE_SIZE;
            let logo_ts = logo.tileset.clone();
            let lx = layout_constants::title_screen::LOGO_PIXEL_X;
            let logo_y = (layout_constants::title_screen::LOGO_PIXEL_Y as i32 - state.scroll_y).max(0) as u32;
            blit_tileset(fb, &logo_ts, lx, logo_y, tiles_per_row, &transparent_bg_pal);
        }

        if state.version_text_visible {
            if let Ok(version_tiles) = rm.load_title("red_version_tiles") {
                let version_w = version_tiles.source_size.0;
                let tiles_per_row = version_w / TILE_SIZE;
                let version_ts = version_tiles.tileset.clone();
                let final_vx = layout_constants::title_screen::version_centered_x(version_w);
                let offscreen_right_x = fb.width();
                let current_vx = if state.version_scroll_progress < 1.0 {
                    let progress = state.version_scroll_progress;
                    (offscreen_right_x as f32 * (1.0 - progress) + final_vx as f32 * progress)
                        as u32
                } else {
                    final_vx
                };
                blit_tileset(
                    fb,
                    &version_ts,
                    current_vx,
                    layout_constants::title_screen::VERSION_PIXEL_Y,
                    tiles_per_row,
                    &transparent_bg_pal,
                );
            } else {
                let version_text = match state.version {
                    GameVersion::Red => "Red Version",
                    GameVersion::Blue => "Blue Version",
                };
                let text_width = version_text.len() as u32 * TILE_SIZE;
                let final_vx = layout_constants::title_screen::version_centered_x(text_width);
                let offscreen_right_x = fb.width();
                let current_vx = if state.version_scroll_progress < 1.0 {
                    let progress = state.version_scroll_progress;
                    (offscreen_right_x as f32 * (1.0 - progress) + final_vx as f32 * progress)
                        as u32
                } else {
                    final_vx
                };
                draw_text(
                    version_text,
                    current_vx,
                    layout_constants::title_screen::VERSION_PIXEL_Y + 3,
                    Rgba::BLACK,
                    fb,
                );
            }
        }

        if let Ok(pokemon_sprite) =
            rm.load_pokemon_front(&species_to_sprite_name(&state.current_mon.to_string()))
        {
            let pw = pokemon_sprite.source_size.0;
            let ph = pokemon_sprite.source_size.1;
            let sprite_tiles_w = pw / TILE_SIZE;
            let sprite_tiles_h = ph / TILE_SIZE;

            let offset_x_tiles = (8 - sprite_tiles_w) / 2;
            let offset_y_tiles = 7 - sprite_tiles_h;
            let offset_x = offset_x_tiles * TILE_SIZE;
            let offset_y = offset_y_tiles * TILE_SIZE;

            let tiles_per_row = sprite_tiles_w;
            let pokemon_ts = pokemon_sprite.tileset.clone();

            let base_x = layout_constants::title_screen::POKEMON_PIXEL_X + offset_x;
            let draw_x = (base_x as i32 + state.mon_scroll_offset).max(0) as u32;
            let draw_y = layout_constants::title_screen::POKEMON_PIXEL_Y + offset_y;

            if (draw_x as i32) < fb.width() as i32 {
                blit_tileset(fb, &pokemon_ts, draw_x, draw_y, tiles_per_row, bg_pal);
            }
        }

        if state.player_visible {
            if let Ok(player_sprite) = rm.load_title("player") {
                let player_w = player_sprite.source_size.0;
                let tiles_per_row = player_w / TILE_SIZE;
                let player_ts = player_sprite.tileset.clone();
                let (player_x, player_y) = layout_constants::title_screen::player_screen_pos();
                blit_tileset(
                    fb,
                    &player_ts,
                    player_x,
                    player_y,
                    tiles_per_row,
                    sprite_pal,
                );
            }
        }

        if let Ok(copyright) = rm.load_splash("copyright") {
            let cw = copyright.source_size.0;
            let tiles_per_row = cw / TILE_SIZE;
            let copyright_ts = copyright.tileset.clone();
            let copyright_x = (fb.width() - cw) / 2;
            blit_tileset(
                fb,
                &copyright_ts,
                copyright_x,
                layout_constants::title_screen::COPYRIGHT_PIXEL_Y,
                tiles_per_row,
                bg_pal,
            );
        }
    } else {
        let phase_text = format!("Title Screen: {:?}", state.phase);
        draw_text(&phase_text, 10, 10, Rgba::BLACK, fb);
        draw_text("Press any button to continue", 10, 100, Rgba::BLACK, fb);
    }
}

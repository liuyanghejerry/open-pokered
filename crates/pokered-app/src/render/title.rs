use crate::alloc_prelude::*;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::title_screen::{TitlePhase, TitleScreenState, FADE_OUT_FRAMES};
use pokered_data::layout_constants;
use pokered_renderer::embedded_font::{draw_text, measure_text};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::screen_fade::apply_white_fade;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::species_to_sprite_name;
use pokered_renderer::tile::TileSet;

#[inline(never)]
#[cfg_attr(all(target_os = "none", target_arch = "arm"), link_section = ".iwram")]
fn blit_title_tiles(
    fb: &mut FrameBuffer,
    tiles: &TileSet,
    x: u32,
    y: u32,
    columns: u32,
    transparent: bool,
) {
    let mut column = 0;
    let mut py = y;
    for index in 0..tiles.len() {
        super::opening::blit(
            fb,
            (x + column * TILE_SIZE) as i32,
            py as i32,
            tiles.get(index),
            transparent,
        );
        column += 1;
        if column == columns {
            column = 0;
            py += TILE_SIZE;
        }
    }
}

pub fn draw_title_screen(
    state: &TitleScreenState,
    is_copyright: bool,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    fb.clear(Rgba::WHITE);

    if let Some(ref mut rm) = res {
        if is_copyright {
            let copyright_w = measure_text("©1995 GAME FREAK");
            draw_text(
                "©1995 GAME FREAK",
                (fb.width() - copyright_w) / 2,
                64,
                Rgba::BLACK,
                fb,
            );
            return;
        }

        draw_logo(state, rm, fb);

        draw_version(state, rm, fb);

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
            let pokemon_ts = &pokemon_sprite.tileset;

            let base_x = layout_constants::title_screen::POKEMON_PIXEL_X + offset_x;
            let draw_x = (base_x as i32 + state.mon_scroll_offset).max(0) as u32;
            let draw_y = layout_constants::title_screen::POKEMON_PIXEL_Y + offset_y;

            if (draw_x as i32) < fb.width() as i32 {
                blit_title_tiles(fb, &pokemon_ts, draw_x, draw_y, tiles_per_row, false);
            }
        }

        if state.player_visible {
            if let Ok(player_sprite) = rm.load_title("player") {
                let player_w = player_sprite.source_size.0;
                let tiles_per_row = player_w / TILE_SIZE;
                let player_ts = &player_sprite.tileset;
                let (player_x, player_y) = layout_constants::title_screen::player_screen_pos();
                blit_title_tiles(fb, &player_ts, player_x, player_y, tiles_per_row, true);
            }
        }

        if let Ok(copyright) = rm.load_splash("copyright") {
            let cw = copyright.source_size.0;
            let tiles_per_row = cw / TILE_SIZE;
            let copyright_ts = &copyright.tileset;
            let copyright_x = (fb.width() - cw) / 2;
            blit_title_tiles(
                fb,
                &copyright_ts,
                copyright_x,
                layout_constants::title_screen::COPYRIGHT_PIXEL_Y,
                tiles_per_row,
                false,
            );
        }
    } else {
        let phase_text = format!("Title Screen: {:?}", state.phase);
        draw_text(&phase_text, 10, 10, Rgba::BLACK, fb);
        draw_text("Press any button to continue", 10, 100, Rgba::BLACK, fb);
    }
    if state.phase == TitlePhase::FadeOut {
        apply_white_fade(fb, state.frame_counter, FADE_OUT_FRAMES);
    }
}

fn draw_version(state: &TitleScreenState, rm: &mut ResourceManager, fb: &mut FrameBuffer) {
    if state.version_text_visible {
        if let Ok(version_tiles) = rm.load_title("red_version_tiles") {
            let version_w = version_tiles.source_size.0;
            let tiles_per_row = version_w / TILE_SIZE;
            let version_ts = &version_tiles.tileset;
            let final_vx = layout_constants::title_screen::version_centered_x(version_w);
            let offscreen_right_x = fb.width();
            let current_vx = if state.version_scroll_progress < 1.0 {
                let progress = state.version_scroll_progress;
                (offscreen_right_x as f32 * (1.0 - progress) + final_vx as f32 * progress) as u32
            } else {
                final_vx
            };
            blit_title_tiles(
                fb,
                &version_ts,
                current_vx,
                layout_constants::title_screen::VERSION_PIXEL_Y,
                tiles_per_row,
                true,
            );
        } else {
            let version_text = match state.version {
                GameVersion::Red => "Red Version",
                GameVersion::Blue => "Blue Version",
            };
            let text_width = measure_text(version_text);
            let final_vx = layout_constants::title_screen::version_centered_x(text_width);
            let offscreen_right_x = fb.width();
            let current_vx = if state.version_scroll_progress < 1.0 {
                let progress = state.version_scroll_progress;
                (offscreen_right_x as f32 * (1.0 - progress) + final_vx as f32 * progress) as u32
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
}

/// The version banner occupies its own sixteen-pixel strip below the logo and
/// above both portraits. Redraw that strip while the other layers are static.
pub(super) fn redraw_version(
    state: &TitleScreenState,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    fb.fill_rect(
        0,
        layout_constants::title_screen::VERSION_PIXEL_Y,
        fb.width(),
        2 * TILE_SIZE,
        Rgba::WHITE,
    );
    if let Some(rm) = res {
        draw_version(state, rm, fb);
    }
}

fn draw_logo(state: &TitleScreenState, rm: &mut ResourceManager, fb: &mut FrameBuffer) {
        if let Ok(logo) = rm.load_title("pokemon_logo") {
            let lw = logo.source_size.0;
            let tiles_per_row = lw / TILE_SIZE;
            let logo_ts = &logo.tileset;
            let lx = layout_constants::title_screen::LOGO_PIXEL_X;
            let logo_y = (layout_constants::title_screen::LOGO_PIXEL_Y as i32 - state.scroll_y)
                .max(0) as u32;
            blit_title_tiles(fb, &logo_ts, lx, logo_y, tiles_per_row, true);
        }

}

pub(super) fn redraw_logo(state: &TitleScreenState, res: &mut Option<ResourceManager>, fb: &mut FrameBuffer) {
    fb.fill_rect(0, 0, fb.width(), 80, Rgba::WHITE);
    if let Some(rm) = res { draw_logo(state, rm, fb); }
}

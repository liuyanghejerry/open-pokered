use crate::alloc_prelude::*;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::title_screen::{TitlePhase, TitleScreenState};
use pokered_data::layout_constants;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::species_to_sprite_name;
use pokered_renderer::tile::TileSet;

#[inline(never)]
#[cfg_attr(all(target_os = "none", target_arch = "arm"), link_section = ".iwram")]
fn blit_title_tiles(
    fb: &mut FrameBuffer,
    tiles: &TileSet,
    x: i32,
    y: i32,
    columns: u32,
    transparent: bool,
) {
    let mut column = 0;
    let mut py = y;
    for index in 0..tiles.len() {
        super::opening::blit(
            fb,
            x + (column * TILE_SIZE) as i32,
            py,
            tiles.get(index),
            transparent,
        );
        column += 1;
        if column == columns {
            column = 0;
            py += TILE_SIZE as i32;
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
    if state.phase == TitlePhase::FadeOut {
        return;
    }

    if let Some(ref mut rm) = res {
        if is_copyright {
            super::opening::draw_copyright(rm, fb);
            return;
        }

        draw_logo(state, rm, fb);

        draw_version(state, rm, fb);

        if let Ok(pokemon_sprite) =
            rm.load_pokemon_front(&species_to_sprite_name(&state.current_mon.to_string()))
        {
            super::blit_front_pic(
                fb,
                &pokemon_sprite,
                layout_constants::title_screen::POKEMON_PIXEL_X as i32 + state.mon_scroll_offset,
                layout_constants::title_screen::POKEMON_PIXEL_Y as i32,
                false,
            );
        }

        if state.player_visible {
            if let Ok(player_sprite) = rm.load_title("player") {
                let player_w = player_sprite.source_size.0;
                let tiles_per_row = player_w / TILE_SIZE;
                let player_ts = &player_sprite.tileset;
                let (player_x, player_y) = layout_constants::title_screen::player_screen_pos();
                for index in 0..player_ts.len() {
                    let x = player_x as i32 + (index as u32 % tiles_per_row * TILE_SIZE) as i32;
                    let mut y = player_y as i32 + (index as u32 / tiles_per_row * TILE_SIZE) as i32;
                    if index == 10 {
                        y += 4 + state.ball_y_offset();
                    }
                    super::opening::blit(fb, x, y, player_ts.get(index), true);
                }
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
                copyright_x as i32,
                layout_constants::title_screen::COPYRIGHT_PIXEL_Y as i32,
                tiles_per_row,
                false,
            );
        }
    } else {
        let phase_text = format!("Title Screen: {:?}", state.phase);
        draw_text(&phase_text, 10, 10, Rgba::BLACK, fb);
        draw_text("Press any button to continue", 10, 100, Rgba::BLACK, fb);
    }
}

fn draw_version(state: &TitleScreenState, rm: &mut ResourceManager, fb: &mut FrameBuffer) {
    if !state.version_text_visible {
        return;
    }
    let asset = match state.version {
        GameVersion::Red => "red_version",
        GameVersion::Blue => "blue_version",
    };
    if let Ok(cached) = rm.load_title(asset) {
        let x = if state.phase == TitlePhase::VersionScroll {
            168 - state.frame_counter as i32 * 4
        } else {
            56
        };
        let indices: [Option<usize>; 8] = match state.version {
            GameVersion::Red => [
                Some(0),
                Some(1),
                None,
                Some(5),
                Some(6),
                Some(7),
                Some(8),
                Some(9),
            ],
            GameVersion::Blue => [
                Some(0),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(7),
            ],
        };
        for (column, index) in indices.into_iter().enumerate() {
            if let Some(index) = index {
                super::opening::blit(
                    fb,
                    x + column as i32 * 8,
                    64,
                    cached.tileset.get(index),
                    true,
                );
            }
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
        let logo_y = (layout_constants::title_screen::LOGO_PIXEL_Y as i32 - state.scroll_y);
        blit_title_tiles(fb, &logo_ts, lx as i32, logo_y, tiles_per_row, true);
    }
}

pub(super) fn redraw_logo(
    state: &TitleScreenState,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    fb.fill_rect(0, 0, fb.width(), 80, Rgba::WHITE);
    if let Some(rm) = res {
        draw_logo(state, rm, fb);
    }
}

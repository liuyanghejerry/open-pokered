mod battle;
mod credits;
mod diploma;
mod elevator;
mod evolution;
mod gamefreak_splash;
mod hof_ceremony;
mod intro;
mod menu;
mod oak;
mod opening;
mod overworld;
mod pc;
mod pokedex;
mod slots;
mod title;
mod town_map;
mod trainer_card;

pub use battle::{draw_battle, BattleVisualEffects};
pub use credits::draw_credits;
pub use diploma::draw_diploma;
pub use elevator::{draw_elevator, draw_filter_bag};
pub use evolution::draw_evolution;
pub use gamefreak_splash::draw_gamefreak_splash;
pub use hof_ceremony::draw_hof_ceremony;
pub use intro::draw_intro_scene;
pub use menu::{draw_bag, draw_main_menu, draw_mart, draw_options_menu, draw_party_screen, draw_save_menu, draw_start_menu, draw_stats_screen,
};
pub use oak::{draw_naming_screen, draw_oak_speech};
pub use overworld::draw_overworld;
pub use pc::draw_pc;
pub use pokedex::draw_pokedex_screen;
pub use slots::draw_slots;
pub use title::draw_title_screen;
pub use town_map::draw_town_map;
pub use trainer_card::draw_trainer_card;

use pokered_renderer::embedded_font::{box_tiles, draw_box_tile, fill_tile};
use pokered_renderer::palette::Palette;
use pokered_renderer::tile::TileSet;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

pub fn blit_tileset(
    fb: &mut FrameBuffer,
    tileset: &TileSet,
    x: u32,
    y: u32,
    tiles_per_row: u32,
    palette: &Palette,
) {
    let total = tileset.len();
    for idx in 0..total {
        let tile = tileset.get(idx);
        let tx = (idx as u32) % tiles_per_row;
        let ty = (idx as u32) / tiles_per_row;
        let px = x + tx * TILE_SIZE;
        let py = y + ty * TILE_SIZE;
        for row in 0..TILE_SIZE {
            let rgba_row = tile.render_row(row as usize, palette);
            for col in 0..TILE_SIZE {
                let sx = px + col;
                let sy = py + row;
                if sx < fb.width() && sy < fb.height() {
                    let c = rgba_row[col as usize];
                    if c != Rgba::TRANSPARENT {
                        fb.set_pixel(sx, sy, c);
                    }
                }
            }
        }
    }
}

pub fn draw_text_box(fb: &mut FrameBuffer, bx: u32, by: u32, bw: u32, bh: u32, color: Rgba) {
    let bg = Rgba::WHITE;
    let t = TILE_SIZE;

    draw_box_tile(&box_tiles::TOP_LEFT, &box_tiles::outside::TOP_LEFT, bx, by, color, bg, fb);
    for col in 0..bw {
        draw_box_tile(
            &box_tiles::HORIZONTAL,
            &box_tiles::outside::HORIZONTAL,
            bx + (1 + col) * t,
            by,
            color,
            bg,
            fb,
        );
    }
    draw_box_tile(&box_tiles::TOP_RIGHT, &box_tiles::outside::TOP_RIGHT, bx + (1 + bw) * t, by, color, bg, fb);

    for row in 0..bh {
        let y = by + (1 + row) * t;
        draw_box_tile(&box_tiles::VERTICAL_LEFT, &box_tiles::outside::VERTICAL_LEFT, bx, y, color, bg, fb);
        for col in 0..bw {
            fill_tile(bx + (1 + col) * t, y, bg, fb);
        }
        draw_box_tile(
            &box_tiles::VERTICAL_RIGHT,
            &box_tiles::outside::VERTICAL_RIGHT,
            bx + (1 + bw) * t,
            y,
            color,
            bg,
            fb,
        );
    }

    let bot_y = by + (1 + bh) * t;
    draw_box_tile(&box_tiles::BOTTOM_LEFT, &box_tiles::outside::BOTTOM_LEFT, bx, bot_y, color, bg, fb);
    for col in 0..bw {
        draw_box_tile(
            &box_tiles::HORIZONTAL_BOTTOM,
            &box_tiles::outside::HORIZONTAL_BOTTOM,
            bx + (1 + col) * t,
            bot_y,
            color,
            bg,
            fb,
        );
    }
    draw_box_tile(
        &box_tiles::BOTTOM_RIGHT,
        &box_tiles::outside::BOTTOM_RIGHT,
        bx + (1 + bw) * t,
        bot_y,
        color,
        bg,
        fb,
    );
}

pub fn draw_centered_sprite(
    fb: &mut FrameBuffer,
    tileset: &TileSet,
    sprite_w: u32,
    _sprite_h: u32,
    pal: &Palette,
) {
    let tiles_per_row = sprite_w / TILE_SIZE;
    let sx = 6 * TILE_SIZE;
    let sy = 32_u32;
    blit_tileset(fb, tileset, sx, sy, tiles_per_row, pal);
}

/// Map every framebuffer pixel through a GB palette byte (rBGP) — same
/// helper as pokered-app's `render::apply_gb_palette` (home/fade.asm).
pub(crate) fn apply_gb_palette(fb: &mut FrameBuffer, pal: &dotzuki_renderer::transition::FadePalette) {
    fb.apply_bgp(pal.bgp);
}

pub fn blit_single_tile(
    fb: &mut FrameBuffer,
    tileset: &TileSet,
    tile_idx: usize,
    px: u32,
    py: u32,
    palette: &Palette,
) {
    blit_single_tile_flipped(fb, tileset, tile_idx, px, py, palette, false);
}

pub fn blit_single_tile_flipped(
    fb: &mut FrameBuffer,
    tileset: &TileSet,
    tile_idx: usize,
    px: u32,
    py: u32,
    palette: &Palette,
    flip_horizontal: bool,
) {
    if tile_idx >= tileset.len() {
        return;
    }
    let tile = tileset.get(tile_idx);
    for row in 0..TILE_SIZE {
        let rgba_row = tile.render_row(row as usize, palette);
        for col in 0..TILE_SIZE {
            let src_col = if flip_horizontal {
                TILE_SIZE - 1 - col
            } else {
                col
            };
            let sx = px + col;
            let sy = py + row;
            if sx < fb.width() && sy < fb.height() {
                let c = rgba_row[src_col as usize];
                if c != Rgba::TRANSPARENT {
                    fb.set_pixel(sx, sy, c);
                }
            }
        }
    }
}

pub fn species_to_sprite_name(species_display: &str) -> String {
    let name = species_display
        .to_lowercase()
        .replace([' ', '-', '\''], "");
    // Mr. Mime is the only Gen-1 species whose gfx filename keeps punctuation
    // (`mr.mime.png` / `mr.mimeb.png`); the display name loses the dot.
    if name == "mrmime" {
        return "mr.mime".to_string();
    }
    name
}

#[cfg(test)]
mod tests {
    use super::species_to_sprite_name;

    #[test]
    fn mr_mime_keeps_dot() {
        assert_eq!(species_to_sprite_name("MrMime"), "mr.mime");
        assert_eq!(species_to_sprite_name("Mr. Mime"), "mr.mime");
        assert_eq!(species_to_sprite_name("MR.MIME"), "mr.mime");
    }

    #[test]
    fn other_special_names_strip_punctuation() {
        assert_eq!(species_to_sprite_name("NidoranF"), "nidoranf");
        assert_eq!(species_to_sprite_name("Farfetchd"), "farfetchd");
        assert_eq!(species_to_sprite_name("Bulbasaur"), "bulbasaur");
    }
}
/// LoadMonFrontSprite pads a tight picture into a 7×7 tile buffer: horizontal
/// placement rounds up to a whole tile, and the last row always rests at row 7.
/// LoadFlippedFrontSprite mirrors that *whole buffer*, including its padding.
pub(super) fn blit_front_pic(
    fb: &mut FrameBuffer,
    cached: &pokered_renderer::resource::CachedTileSet,
    x: i32,
    y: i32,
    flipped: bool,
) {
    let (width, height) = (
        cached.source_size.0 / TILE_SIZE,
        cached.source_size.1 / TILE_SIZE,
    );
    let pad_x = (8 - width) / 2;
    let pad_x = if flipped { 7 - width - pad_x } else { pad_x };
    let pad_y = 7 - height;
    for index in 0..cached.tileset.len() {
        let column = index as u32 % width;
        let row = index as u32 / width;
        let column = if flipped { width - 1 - column } else { column };
        fb.blit_gb_tile_indices(
            x + ((pad_x + column) * TILE_SIZE) as i32,
            y + ((pad_y + row) * TILE_SIZE) as i32,
            cached.tileset.get(index),
            false,
            flipped,
            false,
        );
    }
}

/// TrainerInfoTextBoxTileGraphics, also used by the diploma and Cable Club.
pub(super) fn draw_trainer_info_box(
    fb: &mut FrameBuffer,
    tiles: &TileSet,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) {
    use pokered_renderer::palette::GRAYSCALE_PALETTE;
    let tile = |fb: &mut FrameBuffer, id, tx, ty| {
        blit_single_tile(
            fb,
            tiles,
            id,
            (x + tx) * TILE_SIZE,
            (y + ty) * TILE_SIZE,
            &GRAYSCALE_PALETTE,
        );
    };
    tile(fb, 2, 0, 0);
    tile(fb, 4, width + 1, 0);
    tile(fb, 6, 0, height + 1);
    tile(fb, 7, width + 1, height + 1);
    for tx in 1..=width {
        tile(fb, 3, tx, 0);
        tile(fb, 0, tx, height + 1);
    }
    for ty in 1..=height {
        tile(fb, 5, 0, ty);
        tile(fb, 1, width + 1, ty);
    }
}

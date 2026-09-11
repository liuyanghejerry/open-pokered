//! Generic rendering helpers for blitting tilesets and drawing UI boxes.
//!
//! These functions operate on [`FrameBuffer`], [`TileSet`], and [`Palette`]
//! from `dotzuki-renderer` — no game-specific data types are involved.

use dotzuki_renderer::embedded_font::{box_tiles, draw_box_tile, fill_tile};
use dotzuki_renderer::palette::Palette;
use dotzuki_renderer::tile::TileSet;
use dotzuki_renderer::{FrameBuffer, Rgba};

/// Game Boy tile size in pixels (8×8).
const TILE_SIZE: u32 = 8;

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

    draw_box_tile(
        &box_tiles::TOP_LEFT,
        &box_tiles::outside::TOP_LEFT,
        bx,
        by,
        color,
        bg,
        fb,
    );
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
    draw_box_tile(
        &box_tiles::TOP_RIGHT,
        &box_tiles::outside::TOP_RIGHT,
        bx + (1 + bw) * t,
        by,
        color,
        bg,
        fb,
    );

    for row in 0..bh {
        let y = by + (1 + row) * t;
        draw_box_tile(
            &box_tiles::VERTICAL_LEFT,
            &box_tiles::outside::VERTICAL_LEFT,
            bx,
            y,
            color,
            bg,
            fb,
        );
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
    draw_box_tile(
        &box_tiles::BOTTOM_LEFT,
        &box_tiles::outside::BOTTOM_LEFT,
        bx,
        bot_y,
        color,
        bg,
        fb,
    );
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

pub fn draw_centered_sprite(
    fb: &mut FrameBuffer,
    tileset: &TileSet,
    sprite_w: u32,
    _sprite_h: u32,
    pal: &Palette,
) {
    let tiles_per_row = sprite_w / TILE_SIZE;
    let sx = (fb.width().saturating_sub(sprite_w)) / 2;
    let sy = 32_u32;
    blit_tileset(fb, tileset, sx, sy, tiles_per_row, pal);
}

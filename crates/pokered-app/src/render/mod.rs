mod battle;
mod battle_i18n;
mod diploma;
mod elevator;
mod credits;
mod evolution;
mod hof_ceremony;
mod gamefreak_splash;
mod intro;
mod link;
mod menu;
mod oak;
mod overworld;
mod pc;
mod pokedex;
mod slots;
mod title;
mod town_map;
mod trade;
mod trainer_card;

pub use battle::{draw_battle, BattleVisualEffects};
pub use battle_i18n::{trainer_class_zh, zh_battle_dialog};
pub use diploma::draw_diploma;
pub use elevator::{draw_elevator, draw_filter_bag};
pub use credits::draw_credits;
pub use evolution::draw_evolution;
pub use hof_ceremony::draw_hof_ceremony;
pub use gamefreak_splash::draw_gamefreak_splash;
pub use intro::draw_intro_scene;
pub use link::draw_link_flow;
pub use menu::{draw_bag, draw_main_menu, draw_mart, draw_options_menu, draw_party_screen, draw_save_menu, draw_start_menu, draw_stats_screen};
pub use oak::{draw_naming_screen, draw_oak_speech};
pub use pc::draw_pc;
pub use pokedex::draw_pokedex_screen;
pub use slots::draw_slots;
pub use overworld::draw_overworld;
pub use title::draw_title_screen;
pub use town_map::draw_town_map;
pub use trade::draw_trade;
pub use trainer_card::draw_trainer_card;

use pokered_renderer::embedded_font::{box_tiles, draw_box_tile, fill_tile};
use pokered_renderer::palette::Palette;
use pokered_renderer::tile::TileSet;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use jrpg_renderer::transition::FadePalette;

/// Map every framebuffer pixel through a GB palette byte (rBGP).
///
/// Pixels are assumed to be one of the four GRAYSCALE_PALETTE shades; any
/// other color is treated as black (shade 3). The palette byte packs four
/// 2-bit shade mappings with color 0 in the LOW bits — `dc a,b,c,d` in
/// home/fade.asm is (a<<6)|(b<<4)|(c<<2)|d, i.e. colors 3,2,1,0.
pub(crate) fn apply_gb_palette(fb: &mut FrameBuffer, pal: &FadePalette) {
    const SHADES: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];
    for px in fb.data.chunks_exact_mut(4) {
        let shade: u8 = match (px[0], px[1], px[2]) {
            (0xFF, 0xFF, 0xFF) => 0,
            (0xAA, 0xAA, 0xAA) => 1,
            (0x55, 0x55, 0x55) => 2,
            _ => 3,
        };
        let mapped = (pal.bgp >> (2 * shade)) & 3;
        let v = SHADES[mapped as usize];
        px[0] = v;
        px[1] = v;
        px[2] = v;
    }
}

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
    let sx = (fb.width().saturating_sub(sprite_w)) / 2;
    let sy = 32_u32;
    blit_tileset(fb, tileset, sx, sy, tiles_per_row, pal);
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

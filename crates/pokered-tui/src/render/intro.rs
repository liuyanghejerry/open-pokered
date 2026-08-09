use pokered_core::intro_scene::{GengarPose, IntroSceneState};
use pokered_data::layout_constants;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::layout;
use pokered_renderer::palette::{PaletteState, GRAYSCALE_PALETTE};
use pokered_renderer::resource::{AssetCategory, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

pub fn draw_intro_scene(
    state: &IntroSceneState,
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
    let mut palette_state = PaletteState::new(GRAYSCALE_PALETTE);
    palette_state.obp0 = 0b11100100;
    let sprite_pal = &palette_state.obj_palette0();

    if let Some(ref mut rm) = res {
        draw_gengar(state, rm, fb, bg_pal);
        draw_nidorino(state, rm, fb, sprite_pal);
        draw_black_bars(fb);
    } else {
        let phase_text = format!("Intro: {:?}", state.phase);
        draw_text(&phase_text, 10, 10, Rgba::BLACK, fb);
        draw_text("Gengar vs Nidorino", 10, 30, Rgba::BLACK, fb);
        let pos_text = format!(
            "Nido X:{} Y:{} scroll:{}",
            state.nidorino_base_x + state.nidorino_anim_dx,
            state.nidorino_base_y + state.nidorino_anim_dy,
            state.scroll_x
        );
        draw_text(&pos_text, 10, 50, Rgba::BLACK, fb);
        draw_text("Press any button to skip", 10, 100, Rgba::BLACK, fb);
    }
}

fn gengar_tilemap_name(pose: GengarPose) -> &'static str {
    match pose {
        GengarPose::Idle => "gengar_1.tilemap",
        GengarPose::RaiseArm => "gengar_2.tilemap",
        GengarPose::Slash => "gengar_3.tilemap",
    }
}

/// Maps .2bpp tile index (column-major, deduplicated with --preserve=0x19,0x76)
/// to the row-major PNG tile index used by TileSet.
/// Generated from gengar.png (168×56, 21×7 tiles) matching rgbgfx --columns --remove-duplicates.
#[rustfmt::skip]
const GENGAR_TILE_REMAP: [usize; 95] = [
    0, 21, 42, 84, 105, 126, 43, 64, 85, 106, 127, 44, 65, 86, 107, 128,
    45, 66, 87, 108, 46, 67, 88, 109, 130, 26, 47, 68, 89, 110, 131, 111,
    132, 49, 70, 91, 112, 133, 50, 71, 92, 113, 134, 30, 51, 72, 93, 114,
    10, 31, 73, 94, 32, 53, 74, 12, 33, 54, 96, 117, 138, 34, 55, 77,
    98, 119, 140, 78, 99, 120, 141, 37, 58, 79, 100, 121, 142, 38, 59, 80,
    101, 122, 60, 81, 102, 144, 61, 82, 103, 124, 145, 62, 83, 125, 146,
];

fn draw_gengar(
    state: &IntroSceneState,
    rm: &mut ResourceManager,
    fb: &mut FrameBuffer,
    pal: &pokered_renderer::palette::Palette,
) {
    let tilemap_file = gengar_tilemap_name(state.gengar_pose);
    let tilemap = match load_intro_tilemap(rm, tilemap_file) {
        Some(data) => data,
        None => return,
    };

    if let Ok(cached) = rm.load_intro("gengar") {
        let ts = cached.tileset.clone();
        let base_x = layout_constants::intro_scene::GENGAR_PIXEL_X as i32 - state.scroll_x;
        let base_y = layout_constants::intro_scene::GENGAR_PIXEL_Y as i32;
        let grid_w = layout_constants::intro_scene::GENGAR_TILES_W;

        for (map_idx, &tile_idx) in tilemap.iter().enumerate() {
            let grid_col = (map_idx as u32) % grid_w;
            let grid_row = (map_idx as u32) / grid_w;
            let px = base_x + (grid_col * TILE_SIZE) as i32;
            let py = base_y + (grid_row * TILE_SIZE) as i32;

            if px + TILE_SIZE as i32 <= 0 || px >= fb.width() as i32 {
                continue;
            }
            if py + TILE_SIZE as i32 <= 0 || py >= fb.height() as i32 {
                continue;
            }

            let remap_idx = tile_idx as usize;
            let tile_index = if remap_idx < GENGAR_TILE_REMAP.len() {
                GENGAR_TILE_REMAP[remap_idx]
            } else {
                continue;
            };
            if tile_index >= ts.len() {
                continue;
            }

            let tile = ts.get(tile_index);
            for row in 0..TILE_SIZE {
                let sy = py + row as i32;
                if sy < 0 || sy >= fb.height() as i32 {
                    continue;
                }
                let rgba_row = tile.render_row(row as usize, pal);
                for col in 0..TILE_SIZE {
                    let sx = px + col as i32;
                    if sx < 0 || sx >= fb.width() as i32 {
                        continue;
                    }
                    let c = rgba_row[col as usize];
                    if c != Rgba::TRANSPARENT {
                        fb.set_pixel(sx as u32, sy as u32, c);
                    }
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn load_intro_tilemap(_rm: &ResourceManager, filename: &str) -> Option<Vec<u8>> {
    let path = format!("intro/{}", filename);
    pokered_renderer::embedded::get_embedded_asset(&path).map(|bytes| bytes.to_vec())
}

#[cfg(not(target_arch = "wasm32"))]
fn load_intro_tilemap(rm: &ResourceManager, filename: &str) -> Option<Vec<u8>> {
    let path = rm.root().resolve(AssetCategory::Intro, filename);
    std::fs::read(path).ok()
}

fn nidorino_asset_name(sprite_set: u8) -> &'static str {
    match sprite_set {
        1 => "red_nidorino_2",
        2 => "red_nidorino_3",
        _ => "red_nidorino_1",
    }
}

fn draw_nidorino(
    state: &IntroSceneState,
    rm: &mut ResourceManager,
    fb: &mut FrameBuffer,
    pal: &pokered_renderer::palette::Palette,
) {
    let asset = nidorino_asset_name(state.nidorino_sprite_set);
    if let Ok(tiles) = rm.load_intro(asset) {
        let tw = tiles.source_size.0;
        let tiles_per_row = tw / TILE_SIZE;
        let ts = tiles.tileset.clone();

        // ASM: OAM grid starts at Y = baseCoordY + 8 (first row), screen = OAM_Y - 16
        //      OAM X starts at 0, screen = OAM_X - 8
        // Net offset from baseCoord to screen top-left: (-8, -8)
        let draw_x = state.nidorino_base_x + state.nidorino_anim_dx - 8;
        let draw_y = state.nidorino_base_y + state.nidorino_anim_dy - 8;

        let clip_top = layout_constants::intro_scene::FIGHT_AREA_PIXEL_Y as i32;
        let clip_bottom = (layout_constants::intro_scene::FIGHT_AREA_PIXEL_Y
            + layout_constants::intro_scene::FIGHT_AREA_PIXEL_H) as i32;

        let total_tiles = ts.len();
        for idx in 0..total_tiles {
            let tx = (idx as u32) % tiles_per_row;
            let ty = (idx as u32) / tiles_per_row;
            let px = draw_x + (tx * TILE_SIZE) as i32;
            let py = draw_y + (ty * TILE_SIZE) as i32;

            if py + TILE_SIZE as i32 <= clip_top || py >= clip_bottom {
                continue;
            }
            if px + TILE_SIZE as i32 <= 0 || px >= fb.width() as i32 {
                continue;
            }

            let tile = ts.get(idx);
            for row in 0..TILE_SIZE {
                let sy = py + row as i32;
                if sy < clip_top || sy >= clip_bottom || sy < 0 || sy >= fb.height() as i32 {
                    continue;
                }
                let rgba_row = tile.render_row(row as usize, pal);
                for col in 0..TILE_SIZE {
                    let sx = px + col as i32;
                    if sx < 0 || sx >= fb.width() as i32 {
                        continue;
                    }
                    let c = rgba_row[col as usize];
                    if c != Rgba::TRANSPARENT {
                        fb.set_pixel(sx as u32, sy as u32, c);
                    }
                }
            }
        }
    }
}

fn draw_black_bars(fb: &mut FrameBuffer) {
    fb.fill_rect(
        0,
        0,
        fb.width(),
        layout_constants::intro_scene::BLACK_BAR_TOP_PIXEL_H,
        Rgba::BLACK,
    );
    fb.fill_rect(
        0,
        layout_constants::intro_scene::BLACK_BAR_BOTTOM_PIXEL_Y,
        fb.width(),
        fb.height() - layout_constants::intro_scene::BLACK_BAR_BOTTOM_PIXEL_Y,
        Rgba::BLACK,
    );
}

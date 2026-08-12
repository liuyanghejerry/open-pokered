//! Demo JRPG map using the multi-layer rendering pipeline.
//!
//! Loaded via `--demo` CLI flag. Shows a ground layer (grass),
//! a decoration layer (trees), a player entity, and camera follow.

use dotzuki_engine::camera::{Camera, Rect, Vec2};
use dotzuki_engine::render::{MapLayer, MapRenderState};
use dotzuki_engine::tilemap::{Tilemap, TilemapEntry};
use image::{ImageBuffer, ImageEncoder, Rgba as ImgRgba, RgbaImage};
use pokered_renderer::input::{GbButton, InputState};
use pokered_renderer::layer_renderer::render_layers;
use pokered_renderer::tile::RgbaTileSet;
use pokered_renderer::window::GameLoop;
// True-color demo keeps the engine's RGBA FrameBuffer (full-color
// RgbaTileSet tilesets); the indexed facade would quantize it to 4 shades.
use dotzuki_renderer::{FrameBuffer, Rgba};

/// Tileset is 3 tiles of 8x8 pixels, laid out horizontally: 24×8 PNG.
const TILESET_COLS: u32 = 3;
const TILESET_PNG_W: u32 = TILESET_COLS * 8;
const TILESET_PNG_H: u32 = 8;

/// Generate an in-memory PNG bytes buffer containing the demo tileset.
///
/// Tile 0 – green grass (checker pattern)
/// Tile 1 – brown tree trunk
/// Tile 2 – player character (blue body, red hat)
fn generate_tileset_png() -> Vec<u8> {
    let mut img: RgbaImage = ImageBuffer::new(TILESET_PNG_W, TILESET_PNG_H);

    // Tile 0: grass (checkerboard of two green shades)
    for py in 0..8u32 {
        for px in 0..8u32 {
            let green = if (px + py) % 4 < 2 { 100 } else { 140 };
            img.put_pixel(px, py, ImgRgba([0, green as u8, 0, 255]));
        }
    }

    // Tile 1: tree trunk (brown upright rectangle, darker edges)
    let tx_base = 8u32;
    for py in 0..8u32 {
        for px in 0..8u32 {
            let is_edge = px == 0 || px == 7 || py == 7;
            let is_inner = px >= 2 && px <= 5 && py <= 6;
            let color = if is_edge {
                [80, 50, 10, 255]
            } else if is_inner {
                [120, 80, 20, 255]
            } else if py == 0 {
                [60, 120, 20, 255] // green top (leaves)
            } else {
                [0, 0, 0, 0] // transparent background
            };
            img.put_pixel(tx_base + px, py, ImgRgba(color));
        }
    }

    // Tile 2: player (blue body, red hat, small character)
    let px_base = 16u32;
    for py in 0..8u32 {
        for px in 0..8u32 {
            let color = if py <= 1 && px >= 2 && px <= 5 {
                [220, 30, 30, 255] // red hat
            } else if py == 2 && px >= 1 && px <= 6 {
                [255, 200, 150, 255] // face
            } else if py == 3 && (px == 2 || px == 5) {
                [30, 30, 30, 255] // eyes
            } else if py >= 3 && py <= 6 && px >= 2 && px <= 5 {
                [30, 30, 200, 255] // blue body
            } else {
                [0, 0, 0, 0] // transparent
            };
            img.put_pixel(px_base + px, py, ImgRgba(color));
        }
    }

    let mut png_bytes: Vec<u8> = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
    encoder
        .write_image(
            img.as_raw(),
            TILESET_PNG_W,
            TILESET_PNG_H,
            image::ExtendedColorType::Rgba8,
        )
        .expect("failed to encode tileset PNG");
    png_bytes
}

/// Maps Tiled GID (1-based: 0=empty, 1=grass, 2=tree) to tileset index (0-based).
fn tiled_gid_to_tileset_idx(gid: u16) -> Option<usize> {
    match gid {
        0 => None,   // empty cell
        1 => Some(0), // grass tile
        2 => Some(1), // tree tile
        _ => None,
    }
}

/// Parse a simple Tiled .tmx file with CSV layer data.
///
/// Returns `(vec_of_layers, map_width_tiles, map_height_tiles)`.
fn parse_tmx_simple(xml: &str) -> (Vec<MapLayer>, u16, u16) {
    use std::io::BufRead;

    // Extract <map width="..." height="...">
    let map_line = xml.lines().find(|l| l.trim().starts_with("<map ")).unwrap();
    let extract_attr = |attr: &str| -> u16 {
        let start = map_line.find(&format!("{}=\"", attr)).unwrap() + attr.len() + 2;
        let end = map_line[start..].find('"').unwrap();
        map_line[start..start + end].parse().unwrap()
    };
    let map_w = extract_attr("width");
    let map_h = extract_attr("height");

    let mut layers: Vec<MapLayer> = Vec::new();
    let mut z = 0i32;

    // Extract each <layer> ... </layer> block and its CSV data inside <data>
    let mut in_layer = false;
    let mut in_data = false;
    let mut csv_lines: Vec<String> = Vec::new();

    for line in xml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<layer ") {
            in_layer = true;
            csv_lines.clear();
        } else if trimmed.starts_with("</layer>") {
            in_layer = false;
            // Build the tilemap from collected CSV lines
            let flat: Vec<u16> = csv_lines
                .iter()
                .flat_map(|l| {
                    l.split(',')
                        .map(|s| s.trim().parse::<u16>().unwrap_or(0))
                        .collect::<Vec<_>>()
                })
                .collect();

            let mut tm = Tilemap::new(map_w, map_h);
            for y in 0..map_h {
                for x in 0..map_w {
                    let idx = y as usize * map_w as usize + x as usize;
                    let gid = flat.get(idx).copied().unwrap_or(0);
                    if let Some(ts_idx) = tiled_gid_to_tileset_idx(gid) {
                        tm.set(
                            x,
                            y,
                            TilemapEntry {
                                tile_id: ts_idx as u16,
                                ..Default::default()
                            },
                        );
                    }
                    // If gid is 0 (empty), leave the default entry (tile_id=0 = transparent)
                }
            }

            layers.push(MapLayer::new(tm, z));
            z += 1;
        } else if in_layer && trimmed.starts_with("<data") {
            in_data = true;
        } else if in_layer && trimmed.starts_with("</data>") {
            in_data = false;
        } else if in_data && !trimmed.is_empty() && !trimmed.starts_with('<') {
            csv_lines.push(trimmed.to_string());
        }
    }

    (layers, map_w, map_h)
}

const PLAYER_TILE_INDEX: usize = 2;

pub struct DemoGame {
    tileset: RgbaTileSet,
    render_state: MapRenderState,
    camera: Camera,
    /// Player position in world pixels (not tiles).
    player_x: f32,
    player_y: f32,
    map_pixels_w: f32,
    map_pixels_h: f32,
}

impl DemoGame {
    pub fn new() -> Self {
        // 1. Generate tileset PNG in memory, then load as RgbaTileSet
        let png_bytes = generate_tileset_png();
        let tileset = RgbaTileSet::from_rgba_png(&png_bytes)
            .expect("failed to load demo tileset from PNG");

        // 2. Parse TMX map data (embedded at compile time)
        let tmx_data = include_str!("../../../assets/demo/demo.tmx");
        let (layers, map_w_tiles, map_h_tiles) = parse_tmx_simple(tmx_data);

        let map_pixels_w = map_w_tiles as f32 * 8.0;
        let map_pixels_h = map_h_tiles as f32 * 8.0;

        // 3. Build render state
        let mut rs = MapRenderState::new();
        rs.background_color = (0, 0, 0, 255);
        for layer in layers {
            rs.add_layer(layer);
        }

        // 4. Set up camera
        let mut camera = Camera::new(160.0, 144.0);
        camera.smooth_factor = 0.15;
        // Camera bounds: we allow showing up to the map boundary + one extra
        // tile so the map doesn't abruptly cut off at screen edges.
        camera.clamp_to_bounds(Rect::new(
            -8.0,
            -8.0,
            map_pixels_w + 8.0,
            map_pixels_h + 8.0,
        ));

        // Player starts at center of map
        let start_x = map_pixels_w / 2.0;
        let start_y = map_pixels_h / 2.0;

        Self {
            tileset,
            render_state: rs,
            camera,
            player_x: start_x,
            player_y: start_y,
            map_pixels_w,
            map_pixels_h,
        }
    }

    /// Move the player by tile-sized steps, clamped to map bounds.
    fn move_player(&mut self, dx: f32, dy: f32) {
        let new_x = (self.player_x + dx).clamp(8.0, self.map_pixels_w - 8.0);
        let new_y = (self.player_y + dy).clamp(8.0, self.map_pixels_h - 8.0);
        self.player_x = new_x;
        self.player_y = new_y;
    }
}

impl GameLoop for DemoGame {
    // True-color demo: stays on the engine's RGBA FrameBuffer (the demo's
    // tilesets are full-color RgbaTileSet, not 2bpp GB tiles).
    type Fb = FrameBuffer;

    fn update(&mut self, input: &InputState) {
        let step = 8.0; // one tile per press

        if input.is_held(GbButton::Up) {
            self.move_player(0.0, -step);
        }
        if input.is_held(GbButton::Down) {
            self.move_player(0.0, step);
        }
        if input.is_held(GbButton::Left) {
            self.move_player(-step, 0.0);
        }
        if input.is_held(GbButton::Right) {
            self.move_player(step, 0.0);
        }

        // Camera follows player, centering the viewport on the player
        self.camera.follow_target(Vec2::new(
            self.player_x - 160.0 / 2.0,
            self.player_y - 144.0 / 2.0,
        ));
        self.camera.update(1.0 / 60.0);
    }

    fn draw(&mut self, fb: &mut FrameBuffer) {
        fb.clear(Rgba::BLACK);

        // ---- Render all map layers via render_layers ----
        let tile_color = |tile_id: u16, _palette: u8, px: u8, py: u8| -> Rgba {
            let tile = self.tileset.get(tile_id as usize);
            let row = tile.render_row(py as usize);
            row[px as usize]
        };

        render_layers(
            fb,
            &self.render_state.layers,
            self.camera.position.x as i32,
            self.camera.position.y as i32,
            fb.width(),
            fb.height(),
            tile_color,
        );

        // ---- Draw the player entity on top ----
        let player_screen = self.camera.world_to_screen(Vec2::new(self.player_x, self.player_y));
        let sx = player_screen.x as i32;
        let sy = player_screen.y as i32;

        let player_tile = self.tileset.get(PLAYER_TILE_INDEX);
        for ty in 0..8i32 {
            for tx in 0..8i32 {
                let row = player_tile.render_row(ty as usize);
                let color = row[tx as usize];
                if color.a > 0 {
                    fb.set_pixel((sx + tx) as u32, (sy + ty) as u32, color);
                }
            }
        }
    }
}

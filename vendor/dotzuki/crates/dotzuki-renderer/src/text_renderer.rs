use crate::palette::Palette;
use crate::tile::TileSet;
use crate::{FbSurface, TILE_SIZE};

/// A tile buffer representing the screen-space tilemap.
/// Width and height are in tiles; dynamically allocated.
#[derive(Debug, Clone)]
pub struct ScreenTileBuffer {
    pub tiles: Vec<u8>,
    pub width_tiles: u32,
    pub height_tiles: u32,
}

impl ScreenTileBuffer {
    pub fn new(width_tiles: u32, height_tiles: u32) -> Self {
        Self {
            tiles: vec![0x7F; (width_tiles * height_tiles) as usize],
            width_tiles,
            height_tiles,
        }
    }

    #[inline]
    pub fn get(&self, tx: u32, ty: u32) -> u8 {
        if tx >= self.width_tiles || ty >= self.height_tiles {
            return 0x7F;
        }
        self.tiles[(ty * self.width_tiles + tx) as usize]
    }

    #[inline]
    pub fn set(&mut self, tx: u32, ty: u32, tile_id: u8) {
        if tx < self.width_tiles && ty < self.height_tiles {
            self.tiles[(ty * self.width_tiles + tx) as usize] = tile_id;
        }
    }

    pub fn fill(&mut self, tile_id: u8) {
        self.tiles.fill(tile_id);
    }

    pub fn set_row(&mut self, ty: u32, row_data: &[u8]) {
        if ty >= self.height_tiles {
            return;
        }
        let start = (ty * self.width_tiles) as usize;
        let count = row_data.len().min(self.width_tiles as usize);
        self.tiles[start..start + count].copy_from_slice(&row_data[..count]);
    }

    pub fn copy_from_flat(&mut self, data: &[u8]) {
        let count = data.len().min(self.tiles.len());
        self.tiles[..count].copy_from_slice(&data[..count]);
    }

    pub fn render(&self, fb: &mut impl FbSurface, tileset: &TileSet, palette: &Palette) {
        let fb_w = fb.width();
        let fb_h = fb.height();
        for ty in 0..self.height_tiles {
            for tx in 0..self.width_tiles {
                let tile_id = self.get(tx, ty) as usize;
                let tile = tileset.get(tile_id);
                let screen_x = tx * TILE_SIZE;
                let screen_y = ty * TILE_SIZE;

                for row in 0..TILE_SIZE {
                    for col in 0..TILE_SIZE {
                        let px = screen_x + col;
                        let py = screen_y + row;
                        if px < fb_w && py < fb_h {
                            let color_idx = tile.get(row as usize, col as usize);
                            let rgba = palette.color(crate::palette::GbColor::from_u8(color_idx));
                            fb.set_pixel(px, py, rgba);
                        }
                    }
                }
            }
        }
    }

    pub fn render_region(
        &self,
        fb: &mut impl FbSurface,
        tileset: &TileSet,
        palette: &Palette,
        tx_start: u32,
        ty_start: u32,
        tw: u32,
        th: u32,
    ) {
        let fb_w = fb.width();
        let fb_h = fb.height();
        let tx_end = (tx_start + tw).min(self.width_tiles);
        let ty_end = (ty_start + th).min(self.height_tiles);

        for ty in ty_start..ty_end {
            for tx in tx_start..tx_end {
                let tile_id = self.get(tx, ty) as usize;
                let tile = tileset.get(tile_id);
                let screen_x = tx * TILE_SIZE;
                let screen_y = ty * TILE_SIZE;

                for row in 0..TILE_SIZE {
                    for col in 0..TILE_SIZE {
                        let px = screen_x + col;
                        let py = screen_y + row;
                        if px < fb_w && py < fb_h {
                            let color_idx = tile.get(row as usize, col as usize);
                            let rgba = palette.color(crate::palette::GbColor::from_u8(color_idx));
                            fb.set_pixel(px, py, rgba);
                        }
                    }
                }
            }
        }
    }
}

pub fn write_tiles_at(buf: &mut ScreenTileBuffer, start_tx: u32, start_ty: u32, tile_ids: &[u8]) {
    for (i, &tile_id) in tile_ids.iter().enumerate() {
        let tx = start_tx + i as u32;
        if tx >= buf.width_tiles {
            break;
        }
        buf.set(tx, start_ty, tile_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_tile_buffer_dynamic_sizes() {
        let buf = ScreenTileBuffer::new(20, 18);
        assert_eq!(buf.tiles.len(), 360);
        assert_eq!(buf.width_tiles, 20);
        assert_eq!(buf.height_tiles, 18);

        let buf2 = ScreenTileBuffer::new(30, 20);
        assert_eq!(buf2.tiles.len(), 600);

        let buf3 = ScreenTileBuffer::new(10, 10);
        assert_eq!(buf3.tiles.len(), 100);
    }

    #[test]
    fn test_screen_tile_buffer_set_get() {
        let mut buf = ScreenTileBuffer::new(20, 18);
        buf.set(3, 4, 0x42);
        assert_eq!(buf.get(3, 4), 0x42);
    }

    #[test]
    fn test_screen_tile_buffer_fill() {
        let mut buf = ScreenTileBuffer::new(20, 18);
        buf.fill(0x01);
        for ty in 0..18 {
            for tx in 0..20 {
                assert_eq!(buf.get(tx, ty), 0x01);
            }
        }
    }

    #[test]
    fn test_screen_tile_buffer_out_of_bounds() {
        let buf = ScreenTileBuffer::new(10, 10);
        assert_eq!(buf.get(10, 0), 0x7F);
        assert_eq!(buf.get(0, 10), 0x7F);
    }

    #[test]
    fn test_screen_tile_buffer_set_out_of_bounds() {
        let mut buf = ScreenTileBuffer::new(10, 10);
        buf.set(10, 0, 0xFF);
        buf.set(0, 10, 0xFF);
        assert_eq!(buf.get(5, 5), 0x7F);
    }

    #[test]
    fn test_screen_tile_buffer_set_row() {
        let mut buf = ScreenTileBuffer::new(10, 5);
        buf.set_row(2, &[0x10, 0x20, 0x30]);
        assert_eq!(buf.get(0, 2), 0x10);
        assert_eq!(buf.get(1, 2), 0x20);
        assert_eq!(buf.get(2, 2), 0x30);
        assert_eq!(buf.get(3, 2), 0x7F);
    }

    #[test]
    fn test_screen_tile_buffer_copy_from_flat() {
        let mut buf = ScreenTileBuffer::new(10, 5);
        buf.copy_from_flat(&[0xA0, 0xA1, 0xA2]);
        assert_eq!(buf.get(0, 0), 0xA0);
        assert_eq!(buf.get(1, 0), 0xA1);
        assert_eq!(buf.get(2, 0), 0xA2);
        assert_eq!(buf.get(3, 0), 0x7F);
    }
}

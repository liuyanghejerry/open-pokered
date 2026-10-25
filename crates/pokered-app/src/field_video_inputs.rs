//! Actual native UI raster cells. These handles/patterns are a separate
//! namespace from original map/VRAM tile IDs; never silently reinterpret them
//! as original font codes. Sampling does not transfer VRAM or return an IRQ.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub(super) enum FieldBackgroundError {
    WrongDimensions,
    TooManyUiTiles,
    Uninitialized,
    MapUnavailable,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(super) struct NativeUiTiles {
    pub tiles: Vec<u8>,
    pub patterns: Vec<[u8; 16]>,
    pub revision: u64,
}
impl Default for NativeUiTiles {
    fn default() -> Self {
        Self {
            tiles: Vec::with_capacity(360),
            patterns: Vec::with_capacity(256),
            revision: 0,
        }
    }
}
impl NativeUiTiles {
    /// Reuse handles across text reveal/blink frames. A failed sample rolls
    /// back newly interned patterns and leaves the prior complete map intact.
    pub fn sample(&mut self, frame: &FrameBuffer) -> Result<(), FieldBackgroundError> {
        if frame.width() != 160 || frame.height() != 144 {
            return Err(FieldBackgroundError::WrongDimensions);
        }
        let old_len = self.patterns.len();
        let mut tiles = Vec::with_capacity(360);
        for ty in 0..18 {
            for tx in 0..20 {
                let mut pattern = [0; 16];
                for row in 0..8 {
                    for column in 0..8 {
                        let shade = frame.get_index(tx * 8 + column, ty * 8 + row).unwrap() as u8;
                        pattern[row as usize * 2] |= (shade & 1) << (7 - column);
                        pattern[row as usize * 2 + 1] |= ((shade >> 1) & 1) << (7 - column);
                    }
                }
                let index = if let Some(index) = self.patterns.iter().position(|p| *p == pattern) {
                    index
                } else {
                    if self.patterns.len() == 256 {
                        self.patterns.truncate(old_len);
                        return Err(FieldBackgroundError::TooManyUiTiles);
                    }
                    let index = self.patterns.len();
                    self.patterns.push(pattern);
                    index
                };
                tiles.push(index as u8);
            }
        }
        if self.tiles != tiles {
            self.revision += 1;
            self.tiles = tiles;
        }
        Ok(())
    }
}
#[derive(serde::Serialize)]
pub(super) enum FieldBackgroundInput<'a> {
    NativeUiRaster(&'a NativeUiTiles),
    Map(&'a crate::render::MapTileSnapshot),
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_renderer::palette::GRAYSCALE_PALETTE;
    fn frame() -> FrameBuffer {FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE)}
    pub(super) fn assert_pixels(pool:&NativeUiTiles, frame:&FrameBuffer) {
        assert_eq!(pool.tiles.len(),360);
        for y in 0..144 {for x in 0..160 {
            let pattern=pool.patterns[usize::from(pool.tiles[(y/8*20+x/8)as usize])];
            let row=(y%8)as usize;let shift=7-x%8;
            let shade=((pattern[row*2]>>shift)&1)|(((pattern[row*2+1]>>shift)&1)<<1);
            assert_eq!(shade,frame.get_index(x,y).unwrap()as u8);
        }}
    }
    #[test]
    fn actual_native_pixel_planes_roundtrip_and_handles_survive_reveal_and_blink() {
        let mut frame=frame();let mut pool=NativeUiTiles::default();pool.sample(&frame).unwrap();
        let initial=pool.clone();assert_pixels(&pool,&frame);
        for y in 0..8 {for x in 0..8 {frame.set_pixel(x,y,GRAYSCALE_PALETTE.colors[((x+y)%4)as usize]);}}
        pool.sample(&frame).unwrap();assert_pixels(&pool,&frame);let changed=pool.clone();
        pool.sample(&frame).unwrap();assert_eq!(pool,changed);
        frame.clear(Rgba::WHITE);pool.sample(&frame).unwrap();assert_pixels(&pool,&frame);
        assert_eq!(pool.tiles,initial.tiles);assert_eq!(pool.patterns,changed.patterns);
    }
    #[test]
    fn capacity_failure_is_atomic_and_never_aliases_unrepresentable_ui_tiles() {
        let mut pool=NativeUiTiles::default();pool.sample(&frame()).unwrap();let before=pool.clone();
        let mut too_many=frame();
        for tile in 0..360u32 {for bit in 0..9 {
            if tile&(1<<bit)!=0 {too_many.set_pixel(tile%20*8+bit%8,tile/20*8+bit/8,Rgba::BLACK);}
        }}
        assert_eq!(pool.sample(&too_many),Err(FieldBackgroundError::TooManyUiTiles));assert_eq!(pool,before);
        let bad=FrameBuffer::new(RenderConfig::new(8,8),Rgba::WHITE);
        assert_eq!(pool.sample(&bad),Err(FieldBackgroundError::WrongDimensions));assert_eq!(pool,before);
    }
}

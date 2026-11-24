//! Actual native UI raster cells. These handles/patterns are a separate
//! namespace from original map/VRAM tile IDs; never silently reinterpret them
//! as original font codes. Sampling does not transfer VRAM or return an IRQ.
use super::*;

fn pattern_word(pattern: &[u8; 16], offset: usize) -> u32 {
    u32::from_ne_bytes(pattern[offset..offset + 4].try_into().unwrap())
}
fn pattern_eq(left: &[u8; 16], right: &[u8; 16]) -> bool {
    pattern_word(left, 0) == pattern_word(right, 0)
        && pattern_word(left, 4) == pattern_word(right, 4)
        && pattern_word(left, 8) == pattern_word(right, 8)
        && pattern_word(left, 12) == pattern_word(right, 12)
}

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
    #[serde(skip)]
    slots: [u16; 512],
}
impl Default for NativeUiTiles {
    fn default() -> Self {
        Self {
            tiles: Vec::new(),
            patterns: Vec::new(),
            revision: 0,
            slots: [u16::MAX; 512],
        }
    }
}
impl NativeUiTiles {
    pub fn clear(&mut self) {
        self.tiles.clear();
        self.patterns.clear();
        self.slots.fill(u16::MAX);
    }

    /// Reuse handles across text reveal/blink frames. A failed sample rolls
    /// back newly interned patterns and leaves the prior complete map intact.
    pub fn sample(&mut self, frame: &FrameBuffer) -> Result<(), FieldBackgroundError> {
        self.sample_pixels(frame.width(), frame.height(), |x, y| {
            frame.get_index(x, y).unwrap() as u8
        })
    }

    pub fn sample_dialog(&mut self, frame: &FrameBuffer) -> Result<(), FieldBackgroundError> {
        self.sample_rows(frame, 12, 18)
    }

    pub fn sample_rows(
        &mut self,
        frame: &FrameBuffer,
        first_row: u32,
        end_row: u32,
    ) -> Result<(), FieldBackgroundError> {
        if frame.width() != 160 || frame.height() != 144 {
            return Err(FieldBackgroundError::WrongDimensions);
        }
        if first_row >= end_row || end_row > 18 {
            return Err(FieldBackgroundError::WrongDimensions);
        }
        if first_row == 0 && self.tiles.is_empty() {
            self.tiles = vec![0; 360];
        }
        if self.tiles.len() != 360 {
            return Err(FieldBackgroundError::Uninitialized);
        }
        let old_len = self.patterns.len();
        self.patterns
            .reserve_exact(((end_row - first_row) * 20) as usize);
        let hash = |pattern: &[u8; 16]| {
            (pattern_word(pattern, 0)
                ^ pattern_word(pattern, 4).rotate_left(7)
                ^ pattern_word(pattern, 8).rotate_left(14)
                ^ pattern_word(pattern, 12).rotate_left(21)) as usize
                & 511
        };
        let mut slots = self.slots;
        let mut changed = false;
        let mut previous = None;
        for ty in first_row..end_row {
            for tx in 0..20 {
                let mut pattern = [0; 16];
                for row in 0..8 {
                    for column in 0..8 {
                        let shade = frame.get_index(tx * 8 + column, ty * 8 + row).unwrap() as u8;
                        if shade != 0 {
                            pattern[row as usize * 2] |= (shade & 1) << (7 - column);
                            pattern[row as usize * 2 + 1] |= ((shade >> 1) & 1) << (7 - column);
                        }
                    }
                }
                let index = match previous {
                    Some(index) if pattern_eq(&self.patterns[index], &pattern) => index,
                    _ => {
                        let mut slot = hash(&pattern);
                        loop {
                            if slots[slot] == u16::MAX {
                                if self.patterns.len() == 256 {
                                    self.patterns.truncate(old_len);
                                    return Err(FieldBackgroundError::TooManyUiTiles);
                                }
                                let index = self.patterns.len();
                                self.patterns.push(pattern);
                                slots[slot] = index as u16;
                                break index;
                            }
                            let index = usize::from(slots[slot]);
                            if pattern_eq(&self.patterns[index], &pattern) {
                                break index;
                            }
                            slot = (slot + 1) & 511;
                        }
                    }
                };
                previous = Some(index);
                let at = (ty * 20 + tx) as usize;
                if self.tiles[at] != index as u8 {
                    self.tiles[at] = index as u8;
                    changed = true;
                }
            }
        }
        if changed {
            self.revision += 1;
        }
        self.slots = slots;
        Ok(())
    }

    /// Sample a retained packed raster without replacing the displayed UI or
    /// allocating another full linear framebuffer on GBA.
    pub fn sample_retained(
        &mut self,
        frame: &dotzuki_renderer::RgbaIndexedFrameBuffer<dotzuki_renderer::palette::GbColor>,
    ) -> Result<(), FieldBackgroundError> {
        self.sample_pixels(frame.width(), frame.height(), |x, y| {
            frame.get_index(x, y).unwrap() as u8
        })
    }

    fn sample_pixels(
        &mut self,
        width: u32,
        height: u32,
        pixel: impl Fn(u32, u32) -> u8,
    ) -> Result<(), FieldBackgroundError> {
        if width != 160 || height != 144 {
            return Err(FieldBackgroundError::WrongDimensions);
        }
        let old_len = self.patterns.len();
        // Deduplicate in a bounded workspace before allocating. Reserving a
        // worst-case heap pool then shrinking it needs both allocations on
        // GBA, while current and saved UI namespaces already coexist.
        let mut patterns = [[0; 16]; 256];
        patterns[..old_len].copy_from_slice(&self.patterns);
        let hash = |pattern: &[u8; 16]| {
            (pattern_word(pattern, 0)
                ^ pattern_word(pattern, 4).rotate_left(7)
                ^ pattern_word(pattern, 8).rotate_left(14)
                ^ pattern_word(pattern, 12).rotate_left(21)) as usize
                & 511
        };
        let mut slots = self.slots;
        let mut pattern_count = old_len;
        let mut tiles = Vec::with_capacity(360);
        let mut previous = None;
        for ty in 0..18 {
            for tx in 0..20 {
                let mut pattern = [0; 16];
                for row in 0..8 {
                    for column in 0..8 {
                        let shade = pixel(tx * 8 + column, ty * 8 + row);
                        if shade != 0 {
                            pattern[row as usize * 2] |= (shade & 1) << (7 - column);
                            pattern[row as usize * 2 + 1] |= ((shade >> 1) & 1) << (7 - column);
                        }
                    }
                }
                let index = match previous {
                    Some(index) if pattern_eq(&patterns[index], &pattern) => index,
                    _ => {
                        let mut slot = hash(&pattern);
                        loop {
                            if slots[slot] == u16::MAX {
                                if pattern_count == patterns.len() {
                                    return Err(FieldBackgroundError::TooManyUiTiles);
                                }
                                let index = pattern_count;
                                patterns[index] = pattern;
                                slots[slot] = index as u16;
                                pattern_count += 1;
                                break index;
                            }
                            let index = usize::from(slots[slot]);
                            if pattern_eq(&patterns[index], &pattern) {
                                break index;
                            }
                            slot = (slot + 1) & 511;
                        }
                    }
                };
                previous = Some(index);
                tiles.push(index as u8);
            }
        }
        self.patterns.reserve_exact(pattern_count - old_len);
        self.patterns.extend_from_slice(&patterns[old_len..pattern_count]);
        if self.tiles != tiles {
            self.revision += 1;
            self.tiles = tiles;
        }
        self.slots = slots;
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
    fn clear_invalidates_cached_hash_slots() {
        let mut frame = frame();
        frame.set_pixel(0, 0, Rgba::BLACK);
        let mut pool = NativeUiTiles::default();
        pool.sample(&frame).unwrap();
        pool.clear();
        assert!(pool.tiles.is_empty() && pool.patterns.is_empty());
        pool.sample(&frame).unwrap();
        let mut expected = NativeUiTiles::default();
        expected.sample(&frame).unwrap();
        assert_eq!(pool.tiles, expected.tiles);
        assert_eq!(pool.patterns, expected.patterns);
        assert_eq!(pool.revision, 2);
        assert_pixels(&pool, &frame);
    }

    #[test]
    fn dialog_resample_matches_a_full_sample_and_preserves_other_rows() {
        let mut frame = frame();
        let mut pool = NativeUiTiles::default();
        pool.sample(&frame).unwrap();
        let before = pool.clone();
        for y in 96..144 {
            for x in 0..160 {
                frame.set_pixel(
                    x,
                    y,
                    GRAYSCALE_PALETTE.colors[((x / 8 + y / 8) % 4) as usize],
                );
            }
        }
        let mut expected = pool.clone();
        expected.sample(&frame).unwrap();
        pool.sample_dialog(&frame).unwrap();
        assert_eq!(pool.tiles, expected.tiles);
        assert_eq!(pool.patterns, expected.patterns);
        assert_eq!(pool.revision, expected.revision);
        assert_pixels(&pool, &frame);
        for ty in 0..12 {
            assert_eq!(
                &pool.tiles[ty * 20..ty * 20 + 20],
                &before.tiles[ty * 20..ty * 20 + 20]
            );
        }
        let mut uninitialized = NativeUiTiles::default();
        assert_eq!(
            uninitialized.sample_dialog(&frame),
            Err(FieldBackgroundError::Uninitialized)
        );
    }

    #[test]
    fn full_tile_namespace_roundtrips_and_rejects_one_more_pattern_atomically() {
        let mut frame = frame();
        for tile in 0..360u32 {
            for bit in 0..8 {
                if tile % 256 & (1 << bit) != 0 {
                    frame.set_pixel(tile % 20 * 8 + bit, tile / 20 * 8, Rgba::BLACK);
                }
            }
        }
        let mut pool = NativeUiTiles::default();
        pool.sample(&frame).unwrap();
        assert_eq!(pool.patterns.len(), 256);
        assert_pixels(&pool, &frame);
        let before = pool.clone();
        frame.set_pixel(0, 1, Rgba::BLACK);
        assert_eq!(pool.sample(&frame), Err(FieldBackgroundError::TooManyUiTiles));
        assert_eq!(pool, before);
        assert_eq!(
            pool.sample_rows(&frame, 0, 1),
            Err(FieldBackgroundError::TooManyUiTiles)
        );
        assert_eq!(pool, before);
        frame.set_pixel(0, 1, Rgba::WHITE);
        assert_pixels(&pool, &frame);
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

//! Read-through source bytes for shared map restoration. ResourceManager stays
//! the sole decoded-resource cache; this adapter borrows it and never clones
//! a sprite sheet/party or turns a cache hit into a completed transfer.
//! Source font bytes are transfer data only, never a text-rendering path.
//! Player-visible text continues to use the existing Fusion Pixel renderer.
use crate::alloc_prelude::format;
use core::cell::RefCell;
use dotzuki_renderer::tile::TileSet;
use pokered_core::overworld::field_restore::{PatternAssets, PatternSource};
use pokered_data::sprites::SpriteId;
use pokered_renderer::resource::AssetCategory;
use pokered_renderer::resource::ResourceManager;

pub struct NativePatternAssets<'a> {
    resources: RefCell<&'a mut ResourceManager>,
}
impl<'a> NativePatternAssets<'a> {
    pub fn new(resources: &'a mut ResourceManager) -> Self {
        Self {
            resources: RefCell::new(resources),
        }
    }
}
impl PatternAssets for NativePatternAssets<'_> {
    fn read_pattern_byte(&self, source: PatternSource, offset: u16) -> Option<u8> {
        #[cfg(target_os = "none")]
        return rom_pattern_byte(source, offset);
        #[cfg(not(target_os = "none"))]
        {
            let mut resources = self.resources.borrow_mut();
            let (cached, expected_tiles, one_bpp) = match source {
                PatternSource::Sprite(raw) => {
                    // SpriteId::None.sprite_name() is a rendering convenience
                    // that returns "red". It cannot repair an invalid raw source.
                    if raw == 0 {
                        return None;
                    }
                    let id = SpriteId::from_u8(raw)?;
                    (
                        resources.load_sprite(id.sprite_name()).ok()?,
                        if id.is_still() { 4 } else { 24 },
                        false,
                    )
                }
                PatternSource::WalkingPlayer => (resources.load_sprite("red").ok()?, 24, false),
                PatternSource::BicyclePlayer => {
                    (resources.load_sprite("red_bike").ok()?, 24, false)
                }
                PatternSource::SurfingPlayer => (resources.load_sprite("seel").ok()?, 24, false),
                // Pattern contents are uploaded as transfer data only. The normal
                // text renderer continues to use Fusion Pixel, not these glyphs.
                PatternSource::Font => (resources.load_font("font").ok()?, 128, true),
                PatternSource::TextBox => (resources.load_font("font_extra").ok()?, 32, false),
                PatternSource::Flower(frame @ 1..=3) => (
                    resources
                        .load_asset(
                            AssetCategory::Tileset,
                            &format!("flower/flower{}.png", frame),
                        )
                        .ok()?,
                    1,
                    false,
                ),
                PatternSource::Flower(_) => return None,
            };
            if cached.tileset.len() != expected_tiles {
                return None;
            }
            pattern_byte(&cached.tileset, offset, one_bpp)
        }
    }
}

// GBA already has immutable original tile bytes in ROM. Reading a transfer
// must not allocate a decoded sheet merely to reconstruct those same bytes.
#[cfg(target_os = "none")]
fn rom_pattern_byte(source: PatternSource, offset: u16) -> Option<u8> {
    let (dir, name, tiles, stored_one_bpp, output_one_bpp) = match source {
        PatternSource::Sprite(raw) => {
            if raw == 0 {
                return None;
            }
            let id = SpriteId::from_u8(raw)?;
            (
                "sprites",
                id.sprite_name(),
                if id.is_still() { 4 } else { 24 },
                false,
                false,
            )
        }
        PatternSource::WalkingPlayer => ("sprites", "red", 24, false, false),
        PatternSource::BicyclePlayer => ("sprites", "red_bike", 24, false, false),
        PatternSource::SurfingPlayer => ("sprites", "seel", 24, false, false),
        PatternSource::Font => ("font", "font", 128, true, true),
        PatternSource::TextBox => ("font", "font_extra", 32, true, false),
        PatternSource::Flower(1) => ("tilesets/flower", "flower1", 1, false, false),
        PatternSource::Flower(2) => ("tilesets/flower", "flower2", 1, false, false),
        PatternSource::Flower(3) => ("tilesets/flower", "flower3", 1, false, false),
        PatternSource::Flower(_) => return None,
    };
    let bytes = pokered_renderer::gba_assets::get_preconverted_asset(dir, name)?;
    stored_pattern_byte(bytes, offset, tiles, stored_one_bpp, output_one_bpp)
}

#[cfg(any(target_os = "none", test))]
fn stored_pattern_byte(
    bytes: &[u8],
    offset: u16,
    tiles: usize,
    stored_one_bpp: bool,
    output_one_bpp: bool,
) -> Option<u8> {
    let stored_stride = if stored_one_bpp { 8 } else { 16 };
    let output_stride = if output_one_bpp { 8 } else { 16 };
    if bytes.len() != tiles * stored_stride || usize::from(offset) >= tiles * output_stride {
        return None;
    }
    let offset = usize::from(offset);
    match (stored_one_bpp, output_one_bpp) {
        (true, false) => bytes.get(offset / 2).copied(),
        (false, true) => None, // no current caller may discard a 2bpp plane
        _ => bytes.get(offset).copied(),
    }
}

/// ResourceManager decoded palette indices preserve the original 2bpp layout.
/// Font's 1bpp source is reconstructed only if both bitplanes are identical;
/// no colored source is silently collapsed into a fake monochrome byte.
fn pattern_byte(tiles: &TileSet, offset: u16, one_bpp: bool) -> Option<u8> {
    let bytes_per_tile = if one_bpp { 8 } else { 16 };
    let tile_index = usize::from(offset) / bytes_per_tile;
    if tile_index >= tiles.len() {
        return None;
    }
    let row = (usize::from(offset) % bytes_per_tile) / if one_bpp { 1 } else { 2 };
    let plane = if one_bpp { 0 } else { usize::from(offset) & 1 };
    let tile = tiles.get(tile_index);
    let mut byte = 0;
    for column in 0..8 {
        let index = tile.get(row, column);
        if index > 3 || (one_bpp && index != 0 && index != 3) {
            return None;
        }
        byte |= ((index >> plane) & 1) << (7 - column);
    }
    Some(byte)
}
#[cfg(test)]
mod tests {
    use crate::alloc_prelude::Vec;

    #[test]
    fn rom_transfer_bytes_match_decoded_reencoded_tiles() {
        for one_bpp in [false, true] {
            let stride = if one_bpp { 8 } else { 16 };
            let bytes: Vec<u8> = (0..256).map(|n| n as u8).collect();
            let tiles = if one_bpp {
                TileSet::from_1bpp(&bytes)
            } else {
                TileSet::from_2bpp(&bytes)
            };
            for output_one_bpp in [false, true] {
                if output_one_bpp && !one_bpp {
                    continue;
                }
                let output_len = bytes.len() / stride * if output_one_bpp { 8 } else { 16 };
                for offset in 0..output_len {
                    assert_eq!(
                        stored_pattern_byte(
                            &bytes,
                            offset as u16,
                            bytes.len() / stride,
                            one_bpp,
                            output_one_bpp
                        ),
                        pattern_byte(&tiles, offset as u16, output_one_bpp)
                    );
                }
                assert_eq!(
                    stored_pattern_byte(
                        &bytes,
                        output_len as u16,
                        bytes.len() / stride,
                        one_bpp,
                        output_one_bpp
                    ),
                    None
                );
            }
        }
    }
    use super::*;
    #[test]
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    fn cached_real_resources_still_require_each_original_transfer_wait() {
        use pokered_core::overworld::field_restore::{
            CopyFormat, PatternCopy, PatternMemory, VideoCopyWork,
        };
        let mut resources =
            ResourceManager::new(pokered_renderer::resource::AssetRoot::auto_detect().unwrap());
        let assets = NativePatternAssets::new(&mut resources);
        let mut epoch = 0;
        for _ in 0..2 {
            // The second pass uses the same already-populated resource cache.
            // Retail1099 observes two waits for12, seventeen for128, five for32.
            for (source, tiles, format, waits) in [
                (PatternSource::WalkingPlayer, 12, CopyFormat::TwoBpp, 2),
                (PatternSource::Font, 128, CopyFormat::OneBppDouble, 17),
                (PatternSource::TextBox, 32, CopyFormat::TwoBpp, 5),
            ] {
                let mut work = VideoCopyWork::new(PatternCopy {
                    source,
                    source_offset: 0,
                    destination: 0x8800,
                    tiles,
                    format,
                });
                let mut memory = PatternMemory::default();
                let start = epoch;
                while !work.finished() {
                    work.arm(epoch).unwrap();
                    epoch += 1;
                    work.service(epoch, &mut memory, &assets).unwrap();
                    work.resume().unwrap();
                }
                assert_eq!(epoch - start, waits);
                for offset in 0..u16::from(tiles) * 16 {
                    let source_offset = offset
                        / if format == CopyFormat::OneBppDouble {
                            2
                        } else {
                            1
                        };
                    assert_eq!(
                        memory.read(0x8800 + offset),
                        assets.read_pattern_byte(source, source_offset)
                    );
                }
            }
        }
    }
    #[test]
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    fn moving_flower_assets_preserve_all_three_original_rom_tiles() {
        let root = pokered_renderer::resource::AssetRoot::auto_detect().unwrap();
        let mut resources = ResourceManager::new(root);
        let assets = NativePatternAssets::new(&mut resources);
        let expected = [
            [
                0x81, 0, 0, 0x18, 0, 0x24, 0x85, 0x5a, 0x1c, 0x42, 0x18, 0xa5, 0, 0x7e, 0x81, 0x18,
            ],
            [
                0x81, 0, 0, 0x0c, 0, 0x12, 0x82, 0x2d, 0x0e, 0xe1, 0x0c, 0x73, 0, 0x3e, 0x81, 0x18,
            ],
            [
                0x81, 0x18, 0, 0x24, 4, 0x5a, 0x9d, 0x42, 0x18, 0x24, 0, 0xdb, 0, 0x7e, 0x81, 0x18,
            ],
        ];
        for (frame, bytes) in expected.iter().enumerate() {
            let source = PatternSource::Flower(frame as u8 + 1);
            for (offset, byte) in bytes.iter().enumerate() {
                assert_eq!(assets.read_pattern_byte(source, offset as u16), Some(*byte));
            }
            assert_eq!(assets.read_pattern_byte(source, 16), None);
        }
        assert_eq!(assets.read_pattern_byte(PatternSource::Flower(0), 0), None);
        assert_eq!(assets.read_pattern_byte(PatternSource::Flower(4), 0), None);
    }
    #[test]
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    fn real_resource_font_and_textbox_decode_to_transfer_bytes_without_fallback() {
        // Exercise the production category decoder, not a synthetic TileSet.
        // Font PNG decoding must yield 0/3 so one-bpp expansion is lossless.
        let root = pokered_renderer::resource::AssetRoot::auto_detect()
            .expect("repository gfx is required by the data crate as well");
        let mut resources = ResourceManager::new(root);
        let assets = NativePatternAssets::new(&mut resources);
        for (source, bytes) in [
            (PatternSource::Font, 128 * 8),
            (PatternSource::TextBox, 32 * 16),
        ] {
            let actual: crate::alloc_prelude::Vec<_> = (0..bytes)
                .map(|offset| {
                    assets
                        .read_pattern_byte(source, offset)
                        .expect("real decoded source must preserve each bitplane")
                })
                .collect();
            assert!(actual.iter().any(|&byte| byte != 0));
            assert_eq!(assets.read_pattern_byte(source, bytes), None);
        }
        for source in [
            PatternSource::WalkingPlayer,
            PatternSource::BicyclePlayer,
            PatternSource::SurfingPlayer,
        ] {
            assert!(assets.read_pattern_byte(source, 383).is_some());
            assert_eq!(assets.read_pattern_byte(source, 384), None);
        }
        assert_eq!(assets.read_pattern_byte(PatternSource::Sprite(0), 0), None);
        assert_eq!(
            assets.read_pattern_byte(PatternSource::Sprite(255), 0),
            None
        );
    }
    #[test]
    fn decoded_tiles_recover_both_bitplanes_and_the_next_tile_without_padding() {
        let bytes: [u8; 32] = core::array::from_fn(|i| (i as u8).wrapping_mul(17).wrapping_add(3));
        let tiles = TileSet::from_2bpp(&bytes);
        for (offset, expected) in bytes.iter().enumerate() {
            assert_eq!(pattern_byte(&tiles, offset as u16, false), Some(*expected));
        }
        assert_eq!(pattern_byte(&tiles, 32, false), None);
    }
    #[test]
    fn font_double_input_uses_one_byte_per_row_and_rejects_a_colored_source() {
        let bytes: [u8; 16] = core::array::from_fn(|i| (i / 2) as u8 * 23);
        let tiles = TileSet::from_2bpp(&bytes);
        for offset in 0..8 {
            assert_eq!(
                pattern_byte(&tiles, offset, true),
                Some(bytes[usize::from(offset) * 2])
            );
        }
        assert_eq!(pattern_byte(&tiles, 8, true), None);
        let colored = TileSet::from_2bpp(&[0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(pattern_byte(&colored, 0, true), None);
    }
}

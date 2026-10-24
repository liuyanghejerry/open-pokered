use crate::alloc_prelude::*;
use crate::gen1_battle_anim::{render_gen1_oam_2bpp_palette_split, render_gen1_oam_palette_split};
use crate::palette::GRAYSCALE_PALETTE;
use crate::sprite::SpriteOamEntry;
use crate::tile::TileSet;
use crate::{FrameBuffer, RenderConfig, Rgba};

fn patterned_background() -> FrameBuffer {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    for y in 0..fb.height() {
        for x in 0..fb.width() {
            fb.set_pixel(
                x,
                y,
                GRAYSCALE_PALETTE.colors[((x / 3 + y / 5) % 4) as usize],
            );
        }
    }
    fb
}

#[test]
fn rom_oam_matches_decoded_sheets_for_all_tiles_and_object_attributes() {
    let background = patterned_background();
    let mut flipped = GRAYSCALE_PALETTE;
    flipped.colors.swap(1, 2);
    for name in ["move_anim_0", "move_anim_1"] {
        let bytes = crate::gba_assets::get_preconverted_asset("battle", name).unwrap();
        let decoded = TileSet::from_2bpp(bytes);
        // Include off-sheet IDs: the existing TileSet contract returns blank.
        for batch in 0..9 {
            for attributes in [0, 0x20, 0x40, 0x60, 0x80, 0xa0, 0xc0, 0xe0] {
                let entries: Vec<_> = (0..40)
                    .map(|i| {
                        SpriteOamEntry::new(
                            (i / 12) * 42 - 3,
                            // Overlapping, clipped and more than ten objects per line.
                            (i % 12) * 5 - 4,
                            (batch * 10 + i % 10) as u8,
                            attributes,
                        )
                    })
                    .collect();
                for split in [None, Some(0), Some(1), Some(45), Some(143), Some(144)] {
                    let mut expected = background.clone();
                    let mut actual = background.clone();
                    render_gen1_oam_palette_split(
                        &mut expected,
                        &entries,
                        &decoded,
                        &GRAYSCALE_PALETTE,
                        &flipped,
                        split,
                    );
                    render_gen1_oam_2bpp_palette_split(
                        &mut actual,
                        &entries,
                        bytes,
                        &GRAYSCALE_PALETTE,
                        &flipped,
                        split,
                    );
                    let mut expected_rgba = vec![0; 160 * 144 * 4];
                    let mut actual_rgba = expected_rgba.clone();
                    assert!(expected.to_rgba(&mut expected_rgba));
                    assert!(actual.to_rgba(&mut actual_rgba));
                    assert_eq!(
                        actual_rgba, expected_rgba,
                        "{name} batch={batch} attr={attributes} split={split:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn rom_oam_missing_tile_is_transparent() {
    let expected = patterned_background();
    let mut actual = expected.clone();
    let entries = [SpriteOamEntry::new(20, 20, 255, 0)];
    render_gen1_oam_2bpp_palette_split(
        &mut actual,
        &entries,
        &[],
        &GRAYSCALE_PALETTE,
        &GRAYSCALE_PALETTE,
        None,
    );
    let mut expected_rgba = vec![0; 160 * 144 * 4];
    let mut actual_rgba = expected_rgba.clone();
    assert!(expected.to_rgba(&mut expected_rgba));
    assert!(actual.to_rgba(&mut actual_rgba));
    assert_eq!(actual_rgba, expected_rgba);
}

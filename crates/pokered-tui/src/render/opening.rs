//! Native-index blits for the fixed grayscale opening artwork.
use pokered_renderer::{tile::Tile, FrameBuffer};

/// Clip once per tile; copy opaque rows directly and skip OBJ colour zero.
#[inline(never)]
#[cfg_attr(all(target_os = "none", target_arch = "arm"), link_section = ".iwram")]
pub(super) fn blit(fb: &mut FrameBuffer, x: i32, y: i32, tile: &Tile, transparent: bool) {
    #[cfg(not(all(target_os = "none", target_arch = "arm")))]
    fb.blit_gb_tile_indices(x, y, tile, transparent, false, false);
    #[cfg(all(target_os = "none", target_arch = "arm"))]
    {
        let (width, height) = (fb.width() as usize, fb.height() as usize);
        blit_linear(fb.indices_mut(), width, height, x, y, tile, transparent);
    }
}

#[cfg(any(test, all(target_os = "none", target_arch = "arm")))]
#[inline(always)]
fn blit_linear(
    dst: &mut [u8],
    width: usize,
    height: usize,
    x: i32,
    y: i32,
    tile: &Tile,
    transparent: bool,
) {
    let left = x.saturating_neg().clamp(0, 8) as usize;
    let top = y.saturating_neg().clamp(0, 8) as usize;
    let right = (width as i32).saturating_sub(x).clamp(0, 8) as usize;
    let bottom = (height as i32).saturating_sub(y).clamp(0, 8) as usize;
    if left >= right || top >= bottom {
        return;
    }
    // Clipping above proves each destination and source row is in bounds.
    for row in top..bottom {
        let offset = (y + row as i32) as usize * width + (x + left as i32) as usize;
        unsafe {
            let source = tile.pixels[row].as_ptr().add(left);
            let target = dst.as_mut_ptr().add(offset);
            if !transparent {
                core::ptr::copy_nonoverlapping(source, target, right - left);
            } else {
                for column in 0..right - left {
                    let shade = source.add(column).read();
                    if shade != 0 {
                        target.add(column).write(shade);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipped_native_blit_matches_reference_and_preserves_surroundings() {
        let mut tile = Tile::blank();
        for y in 0..8 {
            for x in 0..8 {
                tile.pixels[y][x] = ((x + y) % 4) as u8;
            }
        }
        for x in [-12, -7, -1, 0, 1, 8, 12, 15, 16, 20] {
            for y in [-9, -1, 0, 3, 8, 12] {
                for transparent in [false, true] {
                    let mut actual = [2u8; 16 * 12];
                    let mut expected = actual;
                    for row in 0..8 {
                        for column in 0..8 {
                            let (px, py) = (x + column, y + row);
                            let shade = tile.pixels[row as usize][column as usize];
                            if (0..16).contains(&px)
                                && (0..12).contains(&py)
                                && (!transparent || shade != 0)
                            {
                                expected[py as usize * 16 + px as usize] = shade;
                            }
                        }
                    }
                    blit_linear(&mut actual, 16, 12, x, y, &tile, transparent);
                    assert_eq!(actual, expected, "x={x} y={y} transparent={transparent}");
                }
            }
        }
    }
}

#[inline(never)]
#[cfg_attr(all(target_os = "none", target_arch = "arm"), link_section = ".iwram")]
pub(super) fn blit_image(
    fb: &mut FrameBuffer,
    image: &[u8],
    width: usize,
    height: usize,
    x: i32,
    y: i32,
    transparent: bool,
    clip_top: i32,
    clip_bottom: i32,
) {
    let left = x.saturating_neg().max(0).min(width as i32) as usize;
    let right = (fb.width() as i32 - x).clamp(0, width as i32) as usize;
    let top = (clip_top.max(0) - y).clamp(0, height as i32) as usize;
    let bottom = (clip_bottom.min(fb.height() as i32) - y).clamp(0, height as i32) as usize;
    if left >= right || top >= bottom {
        return;
    }
    #[cfg(all(target_os = "none", target_arch = "arm"))]
    let stride = fb.width() as usize;
    for row in top..bottom {
        let source = &image[row * width + left..row * width + right];
        #[cfg(all(target_os = "none", target_arch = "arm"))]
        {
            let start = (y + row as i32) as usize * stride + (x + left as i32) as usize;
            let target = &mut fb.indices_mut()[start..start + source.len()];
            if transparent {
                for (dst, &shade) in target.iter_mut().zip(source) {
                    if shade != 0 {
                        *dst = shade;
                    }
                }
            } else {
                target.copy_from_slice(source);
            }
        }
        #[cfg(not(all(target_os = "none", target_arch = "arm")))]
        for (column, &shade) in source.iter().enumerate() {
            if !transparent || shade != 0 {
                fb.set_pixel_index(
                    (x + (left + column) as i32) as u32,
                    (y + row as i32) as u32,
                    pokered_renderer::palette::GbColor::from_u8(shade),
                );
            }
        }
    }
}
/// CopyrightTextString uses custom tiles, with `next` advancing two tile rows.
pub(super) fn draw_copyright(
    resources: &mut pokered_renderer::resource::ResourceManager,
    fb: &mut FrameBuffer,
) {
    let copyright = resources
        .load_splash("copyright")
        .ok()
        .map(|c| c.tileset.clone());
    let gamefreak = resources
        .load_title("gamefreak_inc")
        .ok()
        .map(|c| c.tileset.clone());
    let (Some(copyright), Some(gamefreak)) = (copyright, gamefreak) else {
        return;
    };
    for (line, suffix) in [(0, 5..11), (1, 11..19), (2, 19..28)] {
        let prefix = [0, 1, 2, 1, 3, 1, 4, usize::MAX];
        for (column, id) in prefix.into_iter().chain(suffix).enumerate() {
            let tile = if id < 19 {
                Some(copyright.get(id))
            } else if id < 28 {
                Some(gamefreak.get(id - 19))
            } else {
                None
            };
            if let Some(tile) = tile {
                blit(fb, 16 + column as i32 * 8, 56 + line * 16, tile, false);
            }
        }
    }
}

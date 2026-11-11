//! Render the scanned OAM independently of the restored background portions.
//! Only the retained stationary sprite bank is read; native UI text remains
//! Fusion Pixel in the background rasters.
use super::*;
use pokered_core::overworld::display_device::ObjectRaster;

pub(super) fn draw_submenu_objects(
    raster: &ObjectRaster,
    patterns: &PatternMemory,
    fb: &mut FrameBuffer,
) {
    use dotzuki_renderer::palette::GbColor;
    for (index, span) in raster.spans().iter().enumerate() {
        if span.lcdc & 0x82 != 0x82 {
            continue;
        }
        let end = raster
            .spans()
            .get(index + 1)
            .map_or(144, |next| next.start_line);
        let height = if span.lcdc & 4 != 0 { 16 } else { 8 };
        // Hidden OAM is common while a menu owns the screen. Bound the rows
        // by actual OAM Y, retaining offscreen-X objects for ten-slot priority.
        let mut first_row = end;
        let mut last_row = span.start_line;
        for object in span.oam.chunks_exact(4) {
            let top = i16::from(object[0]) - 16;
            let first = top.max(i16::from(span.start_line));
            let last = (top + height).min(i16::from(end));
            if first < last {
                first_row = first_row.min(first as u8);
                last_row = last_row.max(last as u8);
            }
        }
        for y in first_row..last_row {
            let mut selected = [(0u8, 0usize); 10];
            let mut count = 0;
            for (slot, object) in span.oam.chunks_exact(4).enumerate() {
                let top = i16::from(object[0]) - 16;
                if top <= i16::from(y) && i16::from(y) < top + height {
                    selected[count] = (object[1], slot);
                    count += 1;
                    if count == 10 {
                        break;
                    }
                }
            }
            selected[..count].sort_unstable(); // DMG: X first, then OAM index.
            let mut occupied = [false; 160];
            for &(_, slot) in &selected[..count] {
                let object = &span.oam[slot * 4..slot * 4 + 4];
                let attributes = object[3];
                let mut row = (i16::from(y) - (i16::from(object[0]) - 16)) as u16;
                if attributes & 0x40 != 0 {
                    row = height as u16 - 1 - row;
                }
                let tile = if height == 16 {
                    object[2] & !1
                } else {
                    object[2]
                };
                let address = 0x8000 + u16::from(tile) * 16 + row * 2;
                let low = patterns.read(address).expect("valid object pattern row");
                let high = patterns
                    .read(address + 1)
                    .expect("valid object pattern row");
                let palette = span.palettes[usize::from(attributes & 0x10 != 0)];
                for column in 0..8i16 {
                    let x = i16::from(object[1]) - 8 + column;
                    if !(0..160).contains(&x) || occupied[x as usize] {
                        continue;
                    }
                    let bit = if attributes & 0x20 != 0 {
                        column
                    } else {
                        7 - column
                    };
                    let color = ((low >> bit) & 1) | (((high >> bit) & 1) << 1);
                    if color == 0 {
                        continue;
                    }
                    occupied[x as usize] = true;
                    // A hidden higher-priority OBJ pixel still wins against
                    // lower-priority objects; compare against the untouched BG.
                    if attributes & 0x80 != 0
                        && fb.get_index(x as u32, u32::from(y)) != Some(GbColor::White)
                    {
                        continue;
                    }
                    fb.set_pixel_index(
                        x as u32,
                        u32::from(y),
                        GbColor::from_u8((palette >> (color * 2)) & 3),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_core::overworld::display_device::DisplayDevice;

    #[test]
    fn scanned_objects_preserve_transparency_white_highlights_and_bg_priority() {
        let mut device = DisplayDevice::powered_off(0);
        device.write_port(0xff40, 0x91, 0xc3).unwrap();
        let mut oam = [0; 160];
        oam[..8].copy_from_slice(&[16, 8, 0, 0, 16, 16, 0, 0x80]);
        device
            .begin_native_object_scanout(&oam, [0xd0, 0xe0])
            .unwrap();
        let mut patterns = PatternMemory::default();
        for row in 0..8u16 {
            patterns
                .initialize_native_byte(0x8000 + row * 2, 0x55)
                .unwrap();
            patterns
                .initialize_native_byte(0x8001 + row * 2, 0x33)
                .unwrap();
        }
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::rgb(0x55, 0x55, 0x55));
        draw_submenu_objects(device.native_object_raster().unwrap(), &patterns, &mut fb);
        assert_eq!(fb.get_pixel(0, 0), Some(Rgba::rgb(0x55, 0x55, 0x55)));
        assert_eq!(fb.get_pixel(1, 0), Some(Rgba::WHITE));
        assert_eq!(fb.get_pixel(2, 0), Some(Rgba::rgb(0xaa, 0xaa, 0xaa)));
        assert_eq!(fb.get_pixel(3, 0), Some(Rgba::BLACK));
        assert!((8..16).all(|x| fb.get_pixel(x, 0) == Some(Rgba::rgb(0x55, 0x55, 0x55))));
    }
}

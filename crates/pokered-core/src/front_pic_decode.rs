//! Unflipped Gen-I front-picture decoding and source foreground work.
//!
//! The PC caller clears wSpriteFlipped. Work follows home/uncompress.asm at
//! pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c, including the
//! UncompressSpriteData wrapper and its return, excluding interrupts. The
//! virtual source buffers are at A188/A310, including pointer-carry branches.
//!
//! This is a pure asset component. It does not own a clock, schedule device
//! accesses or model an entire picture-load/VRAM transfer. Those integrations
//! are still required before the PC transition can use its work receipt.
const PLANE_BYTES: usize = 7 * 7 * 8;
// Constant basic blocks, written as their instruction timings. Conditional
// work below depends on the actual compressed stream and decoded state.
const STORE_POINTER_T: u64 = 4 + 16 + 16 + 4 + 16 + 16 + 16;
const RESET_POINTER_RETURN_T: u64 = 4 + 16 + 4 + 16 + 4 + 16 + 4 + 16 + 16;
const READ_BYTE_T: u64 = 16 + 4 + 16 + 4 + 8 + 4 + 4 + 16 + 4 + 16 + 4 + 16;
const READ_BIT_FAST_T: u64 = 16 + 4 + 12 + 16 + 16 + 4 + 16 + 8 + 16;
const READ_BIT_RELOAD_T: u64 = 16 + 4 + 8 + 24 + READ_BYTE_T + 16 + 8 + 16 + 16 + 4 + 16 + 8 + 16;
const WRAPPER_T: u64 = 4 + 12 + 16 + 4 + 12 + 16 + 8 + 16 + 4 + 16 + 24 + 12 + 12 + 16 + 16;
const CLEAR_BUFFERS_T: u64 =
    16 + 4 + (PLANE_BYTES as u64 * 2) * (4 + 8 + 8 + 4 + 4 + 12) - 4 + 12 + 16;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Truncated,
    Dimensions,
    RunLength,
}
#[derive(Debug, Default)]
pub struct Work {
    pub input_bits: usize,
    pub foreground_tcycles: u64,
    pub zero_packets: usize,
    pub literal_groups: usize,
    pub zero_groups: usize,
    pub run_prefix_ones: usize,
    pub mode: u8,
}
pub struct Picture {
    pub width_tiles: u8,
    pub height_tiles: u8,
    pub planes: [[u8; PLANE_BYTES]; 2],
    pub work: Work,
}
struct Bits<'a> {
    data: &'a [u8],
    cursor: usize,
    tcycles: u64,
}
impl Bits<'_> {
    fn bit(&mut self) -> Result<u8, Error> {
        let byte = self.data.get(self.cursor / 8).ok_or(Error::Truncated)?;
        let value = (byte >> (7 - self.cursor % 8)) & 1;
        self.tcycles += 24
            + if self.cursor % 8 == 0 {
                READ_BIT_RELOAD_T
            } else {
                READ_BIT_FAST_T
            };
        self.cursor += 1;
        Ok(value)
    }
    fn number(&mut self, count: usize) -> Result<usize, Error> {
        let mut value = 0;
        for _ in 0..count {
            value = (value << 1) | usize::from(self.bit()?);
        }
        Ok(value)
    }
}
fn fill_plane(
    bits: &mut Bits<'_>,
    plane: &mut [u8; PLANE_BYTES],
    width: usize,
    height: usize,
    source_plane: usize,
    last_plane: bool,
    work: &mut Work,
) -> Result<(), Error> {
    let total = width * height * 4;
    let mut cursor = 0;
    let mut literal = bits.bit()? != 0;
    bits.tcycles += 4 + if literal { 8 } else { 12 };
    while cursor < total {
        if literal {
            while cursor < total {
                let group = bits.number(2)? as u8;
                bits.tcycles += 4 + 8 + 4 + 4 + if group == 0 { 12 } else { 8 };
                if group == 0 {
                    break;
                }
                write_group(plane, height, cursor, group);
                bits.tcycles += write_move_work(cursor, source_plane, last_plane, width, height);
                cursor += 1;
                work.literal_groups += 1;
                if cursor < total {
                    bits.tcycles += 12;
                }
            }
        } else {
            bits.tcycles += 8;
            let mut ones = 0;
            loop {
                let bit = bits.bit()?;
                bits.tcycles += 4 + if bit == 0 { 12 } else { 8 };
                if bit == 0 {
                    break;
                }
                ones += 1;
                bits.tcycles += 4 + 12;
                if ones >= 16 {
                    return Err(Error::RunLength);
                }
            }
            bits.tcycles += sum(&[4, 4, 12, 4, 4, 12, 8, 4, 8, 16, 4, 8, 4]);
            let mut run = ((1usize << (ones + 1)) - 1) + bits.number(ones + 1)?;
            bits.tcycles += 48 * ones as u64 + 24 + sum(&[12, 8, 4, 4]);
            work.zero_packets += 1;
            work.run_prefix_ones += ones;
            while run != 0 && cursor < total {
                bits.tcycles +=
                    4 + 4 + write_move_work(cursor, source_plane, last_plane, width, height) + 4;
                work.zero_groups += 1;
                cursor += 1;
                if cursor == total {
                    break;
                }
                bits.tcycles += 8 + 4 + 4 + if (run - 1) >> 8 != 0 { 12 } else { 8 };
                if (run - 1) >> 8 == 0 {
                    bits.tcycles += 4 + 4;
                }
                run -= 1;
                bits.tcycles += if run != 0 { 12 } else { 8 };
            }
            if cursor < total {
                bits.tcycles += 12;
            }
        }
        literal = !literal;
    }
    Ok(())
}
fn write_group(plane: &mut [u8; PLANE_BYTES], height: usize, cursor: usize, group: u8) {
    let column = cursor / (height * 4);
    let remaining = cursor % (height * 4);
    let pair = remaining / height;
    let row = remaining % height;
    plane[column * height + row] |= group << (6 - pair * 2);
}
fn differential_decode(
    plane: &mut [u8; PLANE_BYTES],
    width: usize,
    height: usize,
    tcycles: &mut u64,
) {
    *tcycles += sum(&[
        4,
        16,
        16,
        24,
        STORE_POINTER_T,
        16,
        4,
        12,
        12,
        12,
        4,
        16,
        4,
        16,
        4,
        16,
        4,
        16,
        8,
    ]);
    const CODES: [[u8; 16]; 2] = [
        [0, 1, 3, 2, 7, 6, 4, 5, 15, 14, 12, 13, 8, 9, 11, 10],
        [15, 14, 12, 13, 8, 9, 11, 10, 0, 1, 3, 2, 7, 6, 4, 5],
    ];
    for row in 0..height {
        let mut last = 0;
        for column in 0..width {
            let index = column * height + row;
            let byte = plane[index];
            *tcycles += sum(&[16, 4, 16, 4, 8, 4, 8, 8]) + nibble_work(byte >> 4, last as u8);
            let high = CODES[last][usize::from(byte >> 4)];
            *tcycles += sum(&[8, 4, 4, 8]) + nibble_work(byte & 15, high);
            let low = CODES[usize::from(high & 1)][usize::from(byte & 15)];
            last = usize::from(low & 1);
            plane[index] = (high << 4) | low;
            *tcycles += sum(&[
                4,
                4,
                16,
                4,
                16,
                4,
                4,
                8,
                16,
                4,
                12,
                16,
                4,
                16,
                16,
                8,
                16,
                4,
                16,
                4,
                if column + 1 != width { 12 } else { 8 },
            ]);
        }
        *tcycles += sum(&[
            4,
            4,
            16,
            16,
            4,
            16,
            4,
            16,
            4,
            if row + 1 == height { 12 } else { 8 },
        ]);
        if row + 1 != height {
            *tcycles += sum(&[16, 4, 16, 4, 8, 24, STORE_POINTER_T, 12]);
        }
    }
    *tcycles += 4 + 16 + 16;
}
pub fn decode(data: &[u8]) -> Result<Picture, Error> {
    let dimensions = *data.first().ok_or(Error::Truncated)?;
    let (width, height) = (dimensions >> 4, dimensions & 15);
    if !(1..=7).contains(&width) || !(1..=7).contains(&height) {
        return Err(Error::Dimensions);
    }
    let mut bits = Bits {
        data,
        cursor: 8,
        tcycles: WRAPPER_T
            + sum(&[
                12,
                8,
                8,
                4,
                24,
                CLEAR_BUFFERS_T,
                8,
                16,
                8,
                16,
                4,
                16,
                16,
                16,
                24,
                READ_BYTE_T,
                4,
                8,
                4,
                4,
                4,
                16,
                4,
                8,
                8,
                4,
                4,
                4,
                16,
            ]),
    };
    let mut result = Picture {
        width_tiles: width,
        height_tiles: height,
        planes: [[0; PLANE_BYTES]; 2],
        work: Work::default(),
    };
    let order = usize::from(bits.bit()?);
    bits.tcycles += 16;
    let (w, h) = (usize::from(width), usize::from(height) * 8);
    bits.tcycles += fill_prefix(order, false);
    fill_plane(
        &mut bits,
        &mut result.planes[order],
        w,
        h,
        order,
        false,
        &mut result.work,
    )?;
    bits.tcycles += fill_prefix(order ^ 1, true);
    let first = bits.bit()?;
    bits.tcycles += 4 + if first == 0 { 12 } else { 8 };
    let mode = if first != 0 {
        let value = 1 + bits.bit()?;
        bits.tcycles += 4;
        value
    } else {
        0
    };
    bits.tcycles += 16;
    fill_plane(
        &mut bits,
        &mut result.planes[order ^ 1],
        w,
        h,
        order ^ 1,
        true,
        &mut result.work,
    )?;
    bits.tcycles += 16 + 8 + if mode == 2 { 16 } else { 12 };
    if mode == 0 {
        bits.tcycles += 4 + 12 + 12 + 24;
        differential_decode(&mut result.planes[0], w, h, &mut bits.tcycles);
        bits.tcycles += 12;
        differential_decode(&mut result.planes[1], w, h, &mut bits.tcycles);
    } else {
        let reset = reset_work(order);
        if mode == 2 {
            bits.tcycles += sum(&[24, reset, 16, 16, 4, 16, 16, 4, 16, 4, 24]);
            differential_decode(&mut result.planes[order ^ 1], w, h, &mut bits.tcycles);
            bits.tcycles += sum(&[24, reset, 12, 16, 16]);
        } else {
            bits.tcycles += 4 + 16;
        }
        bits.tcycles += sum(&[4, 16, 16, 24, reset, 16, 4, 16, 4, 24]);
        differential_decode(&mut result.planes[order], w, h, &mut bits.tcycles);
        bits.tcycles += sum(&[24, reset, 16, 4, 16, 4, 16, 4, 16, 4]);
        for column in 0..w {
            for row in 0..h {
                bits.tcycles += sum(&[
                    16,
                    4,
                    12,
                    8,
                    4,
                    8,
                    4,
                    8,
                    8,
                    16,
                    4,
                    16,
                    4,
                    16,
                    4,
                    if row + 1 != h { 12 } else { 8 },
                ]);
                let i = column * h + row;
                result.planes[order ^ 1][i] ^= result.planes[order][i];
            }
            bits.tcycles += sum(&[
                4,
                16,
                16,
                8,
                16,
                4,
                16,
                4,
                if column + 1 != w { 12 } else { 8 },
            ]);
        }
        bits.tcycles += 4 + 16 + 16;
    }
    result.work.foreground_tcycles = bits.tcycles;
    result.work.mode = mode;
    result.work.input_bits = bits.cursor;
    Ok(result)
}
impl Picture {
    /// Canonical 7x7 column-major 2bpp VRAM picture, as produced by source
    /// LoadUncompressedSpriteData + InterlaceMergeSpriteBuffers.
    pub fn aligned_front(&self) -> [u8; 784] {
        let mut output = [0; 784];
        let (w, h) = (
            usize::from(self.width_tiles),
            usize::from(self.height_tiles),
        );
        let (pad_x, pad_y) = ((8 - w) / 2, 7 - h);
        for column in 0..w {
            for row in 0..h {
                for y in 0..8 {
                    let source = column * h * 8 + row * 8 + y;
                    let destination = ((pad_x + column) * 7 + pad_y + row) * 16 + y * 2;
                    output[destination] = self.planes[0][source];
                    output[destination + 1] = self.planes[1][source];
                }
            }
        }
        output
    }
}
#[test]
fn source_151_front_pictures_match_every_original_vram_byte() {
    let bytes = include_bytes!("../../../docs/audits/hof-pc-load/front-codec-cases.bin");
    let mut cursor = 0;
    let mut count = 0;
    let mut modes = [0; 3];
    let mut seen = [false; 256];
    let mut sizes = [0; 8];
    while cursor < bytes.len() {
        let species = bytes[cursor];
        assert!(!seen[usize::from(species)], "duplicate source species");
        seen[usize::from(species)] = true;
        let size = u16::from_le_bytes([bytes[cursor + 1], bytes[cursor + 2]]) as usize;
        cursor += 3;
        let picture = decode(&bytes[cursor..cursor + size]).unwrap();
        cursor += size;
        assert_eq!(
            picture.aligned_front().as_slice(),
            &bytes[cursor..cursor + 784],
            "original species {species}"
        );
        modes[usize::from(picture.work.mode)] += 1;
        sizes[usize::from(picture.width_tiles)] += 1;
        assert_eq!(
            picture.work.zero_groups + picture.work.literal_groups,
            usize::from(picture.width_tiles) * usize::from(picture.height_tiles) * 8 * 8
        );
        cursor += 784;
        let expected_cycles = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
        assert_eq!(
            picture.work.foreground_tcycles, expected_cycles,
            "source foreground work species{species}"
        );
        cursor += 8;
        count += 1;
    }
    assert_eq!(count, 151);
    assert_eq!(modes, [0, 74, 77]);
    assert!(sizes[5..=7].iter().all(|n| *n > 0));
    eprintln!("151 species exact byte equality; modes={modes:?}; dimensions={sizes:?}");
}
#[test]
fn asset_errors_do_not_read_past_the_input() {
    assert!(matches!(decode(&[]), Err(Error::Truncated)));
    assert!(matches!(decode(&[0]), Err(Error::Dimensions)));
    assert!(matches!(decode(&[0x55]), Err(Error::Truncated)));
    assert!(matches!(
        decode(&[0x55, 0x3f, 0xff, 0xff]),
        Err(Error::RunLength) | Err(Error::Truncated)
    ));
}

#[test]
fn controlled_mode_zero_stream_decodes_two_zero_planes() {
    // Deliberately constructed 1x1-tile codec fixture, not a gameplay save.
    // Order0, two zero packets of32 groups, unpack mode0.
    let picture = decode(&[0x11, 0x3c, 0x13, 0xc1]).unwrap();
    assert_eq!(picture.work.mode, 0);
    assert_eq!(picture.work.zero_groups, 64);
    assert_eq!(picture.planes, [[0; PLANE_BYTES]; 2]);
    assert_eq!(picture.aligned_front(), [0; 784]);
}

// Foreground instruction work from the pinned home/uncompress.asm. The sums
// retain CALL/RET and conditional-branch costs. They are NOT timings fitted to
// species: inputs are compressed bits, output position, prior decoded bit and
// the source buffer selection. Device/IRQ scheduling is a separate owner.
fn sum(xs: &[u64]) -> u64 {
    xs.iter().sum()
}
fn fill_prefix(plane: usize, last: bool) -> u64 {
    sum(&[
        12,
        16,
        8,
        if plane == 0 { 12 } else { 8 },
        if plane == 0 { 0 } else { 12 },
        24,
        STORE_POINTER_T,
        16,
        8,
        if last { 8 } else { 12 },
    ])
}
fn reset_work(order: usize) -> u64 {
    sum(&[
        16,
        8,
        if order ^ 1 != 0 { 12 } else { 8 },
        12,
        12,
        if order ^ 1 != 0 { 0 } else { 12 },
        RESET_POINTER_RETURN_T,
    ])
}
fn write_move_work(index: usize, plane: usize, last: bool, width: usize, height: usize) -> u64 {
    let row = index % height;
    let pair = index / height % 4;
    let column = index / (height * 4);
    let offset = 3 - pair;
    let mut t = 24 + write_bits_work(offset);
    t += 24 + sum(&[16, 4, 16, 4, 4, if row + 1 == height { 12 } else { 8 }]);
    if row + 1 != height {
        let base = if plane == 0 { 0xa188 } else { 0xa310 };
        let carry = (base + column * height + row + 1) & 255 == 0;
        t += sum(&[16, 16, 4, 16, if carry { 8 } else { 20 }]);
        if carry {
            t += 16 + 4 + 16 + 16;
        }
        return t;
    }
    t += sum(&[4, 16, 16, 4, if offset == 0 { 12 } else { 8 }]);
    if offset != 0 {
        return t + sum(&[4, 16, 12, 8, 16, 8, 16, 16]);
    }
    t += sum(&[
        8,
        16,
        16,
        8,
        16,
        4,
        16,
        4,
        if column + 1 == width { 12 } else { 8 },
    ]);
    if column + 1 != width {
        return t + sum(&[16, 4, 16, 4, 8, 16, STORE_POINTER_T]);
    }
    t += sum(&[12, 4, 16, 16, 8, if last { 12 } else { 8 }]);
    t + if last { 16 } else { 8 + 8 + 16 + 16 }
}
fn nibble_work(value: u8, last: u8) -> u64 {
    let odd = value & 1 != 0;
    let previous = last & 1 != 0;
    sum(&[
        24,
        8,
        8,
        if odd { 8 } else { 12 },
        if odd { 8 } else { 0 },
        4,
        16,
        4,
        12,
        8,
        4,
        if previous { 12 } else { 8 },
        16,
        4,
        16,
        if previous { 0 } else { 12 },
        4,
        4,
        4,
        4,
        12,
        8,
        8,
        if odd { 12 } else { 8 },
        if odd { 0 } else { 8 },
        8,
        4,
        16,
    ])
}

fn write_bits_work(offset: usize) -> u64 {
    let head = 4 + 16 + 4; // LD E,A; LD A,[bit offset]; AND A
    let tail = 16 + 4 + 16 + 4 + 8 + 4 + 8 + 16;
    head + tail
        + match offset {
            0 => 12,
            1 => 8 + 8 + 12 + 8 + 8 + 12,
            2 => 8 + 8 + 8 + 12 + 8,
            3 => 8 + 8 + 8 + 8 + 8 + 8 + 12,
            _ => unreachable!("two-bit output offset"),
        }
}

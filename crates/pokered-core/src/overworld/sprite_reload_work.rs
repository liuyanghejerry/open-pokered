//! CPU work performed while ReloadMapSpriteTilePatterns has the LCD disabled.
//!
//! This is an instruction-cycle model of InitMapSprites, including its
//! bank-switch return and the call to EnableLCD. It does not measure host CPU
//! time or prescribe a number of LCD frames: the caller must combine this work
//! with the emulated CPU/PPU phase. Pixel data is never copied by this model.

use pokered_data::sprite_set_data::{
    MapSpriteSetRef, SplitDirection, SpriteSetId, MAP_SPRITE_SETS, SPLIT_MAP_SPRITE_SETS,
};

/// Original sprite slots include the player at index zero. Indoor slots up to
/// `count` retain their picture IDs even when hidden. Outdoors the fixed sprite
/// set is loaded, then all fifteen slots are searched to assign VRAM slots.
/// Returns None for an invalid sprite configuration rather than inventing a
/// frame delay. All cycle sums below follow engine/overworld/map_sprites.asm.
pub fn sprite_reload_cycles(map: u8, x: u8, y: u8, pictures: &[u8; 16], count: u8) -> Option<u32> {
    if count > 15 {
        return None;
    }
    let mut cycles = 24 + 16 + 8; // CALL InitOutsideMapSprites; map read/compare
    if let Some(map_set) = MAP_SPRITE_SETS.get(map as usize) {
        cycles += 8 + 12 + 4 + 4 + 12 + 8 + 8;
        let set = match *map_set {
            MapSpriteSetRef::Direct(set) => {
                cycles += 12;
                set
            }
            MapSpriteSetRef::Split(index) => {
                cycles += 24;
                let (work, set) = split_set(index, x, y);
                cycles += work;
                set
            }
        };
        // Font bit was cleared and wSpriteSetID reset before entering.
        cycles += 4 + 16 + 8 + 8 + 16 + 4 + 8;
        // Load the eleven-element fixed set into RAM. Both carry paths through
        // pointer arithmetic cost twelve cycles; no ROM address is required.
        cycles += 4 + 16 + 4 + 4 + 8 + 4 + 8 + 8 + 4 + 4 + 12 + 4 + 4 + 12;
        cycles += 12 + 8 + 8 + 12;
        let set_pictures = set.sprites();
        let mut loaded = [0u8; 16];
        loaded[0] = 1;
        for (dst, src) in loaded[1..12].iter_mut().zip(set_pictures) {
            *dst = *src as u8;
        }
        cycles += 11 * 80 - 4 + 8 + 4 * 44 - 4 + 16 + 16 + 8 + 16 + 24;
        cycles += load_tile_patterns(&loaded, 11)?;
        cycles += 12 + 16 + 12 + 8 + 15 * 44 - 4 + 12;
        for (index, &picture) in pictures.iter().enumerate().skip(1) {
            cycles += 8 + 8 + 4;
            if picture == 0 {
                cycles += 12;
            } else {
                cycles += 8 + 4 + 12;
                let slot = set_pictures.iter().position(|&p| p as u8 == picture)?;
                cycles += (slot as u32 + 1) * 36 - 4 + 4;
            }
            cycles += 16 + 4 + 8 + 4 + 4 + 4 + 8 + 12 + 8 + 4 + 4 + 4;
            cycles += if index < 15 { 12 } else { 8 };
        }
        cycles += 4 + 16 + 20; // SCF; RET; taken RET C at InitMapSprites
    } else {
        cycles += 20 + 8 + 12 + 12 + 16 * 60 - 4;
        cycles += load_tile_patterns(pictures, count as usize)?;
    }
    // Bankswitch return: POP BC; LD A,B; bank store/select; RET; CALL EnableLCD.
    Some(cycles + 12 + 4 + 12 + 16 + 16 + 24)
}

fn split_set(index: u8, x: u8, y: u8) -> (u32, SpriteSetId) {
    if index == 7 {
        // Route 20 uses four horizontal intervals and two different Y divides.
        let mut cycles = 8 + 12 + 12 + 8 + 8 + 8;
        if x < 43 {
            return (cycles + 20, SpriteSetId::PalletViridian);
        }
        cycles += 8 + 8 + 8 + 8;
        if x >= 62 {
            return (cycles + 20, SpriteSetId::Fuchsia);
        }
        cycles += 8 + 8 + 8 + 8 + if x >= 55 { 12 } else { 8 + 8 };
        let dividing_y = if x >= 55 { 8 } else { 13 };
        cycles += 16 + 4 + 8;
        if y < dividing_y {
            (cycles + 20, SpriteSetId::Fuchsia)
        } else {
            (cycles + 8 + 8 + 16, SpriteSetId::PalletViridian)
        }
    } else {
        let split = SPLIT_MAP_SPRITE_SETS[index as usize];
        let east_west = split.direction == SplitDirection::EastWest;
        let coord = if east_west { x } else { y };
        let before = coord < split.coordinate;
        let cycles = 8
            + 8
            + 12
            + 8
            + 4
            + 8
            + 8
            + 4
            + 4
            + 12
            + 8
            + 8
            + 8
            + 4
            + if east_west { 12 } else { 8 }
            + 16
            + if east_west { 0 } else { 12 }
            + 4
            + if before { 12 } else { 8 + 8 }
            + 8
            + 16;
        (
            cycles,
            if before {
                split.set_north_or_west
            } else {
                split.set_south_or_east
            },
        )
    }
}

fn load_tile_patterns(pictures: &[u8; 16], count: usize) -> Option<u32> {
    if count == 0 {
        return Some(16 + 4 + 8 + 16);
    }
    let mut cycles = 16 + 4 + 12 + 4 + 8 + 12 + 4 + 12 + 16 * 48 - 4 + 12;
    let mut bases = *pictures;
    let mut four_tile_count = 0;
    for index in 1..=count {
        let picture = pictures[index];
        if !(1..=72).contains(&picture) {
            return None;
        }
        cycles += 12;
        let mut duplicate = None;
        for (previous, &loaded) in pictures.iter().enumerate().take(index).skip(1) {
            cycles += 40;
            if loaded == picture {
                cycles += 8 + 8 + 16;
                duplicate = Some(previous);
                break;
            }
            cycles += 8 + 8 + 12 + 4 + 8 + 4 + 12;
        }
        if let Some(previous) = duplicate {
            cycles += 8 + 8 + 8;
            bases[index] = bases[previous];
        } else {
            cycles += 44 + 12 + 8;
            let mut slot = 1u8;
            for &base in &bases[1..index] {
                cycles += 32 + 8 + 8;
                if base >= 11 {
                    cycles += 12;
                    continue;
                }
                cycles += 8 + 4;
                if base < slot {
                    cycles += 12;
                    continue;
                }
                cycles += 8 + 4 + 12;
                slot = base;
            }
            cycles += 36 + 4 + 4 + 16 + 8 + 4 + 8;
            if picture < 61 {
                cycles += 12 + 12;
                slot += 1;
            } else {
                cycles += 8 + 12 + 12 + 8 + 12;
                slot = 11 + four_tile_count;
            }
            bases[index] = slot;
            // Table-pointer carry paths have the same cost. ReadSpriteSheetData
            // takes 68 cycles; all walking sheets are 192 bytes, still sheets 64.
            cycles += 8
                + 12
                + 4
                + 4
                + 4
                + 4
                + 16
                + 16
                + 12
                + 12
                + 4
                + 4
                + 12
                + 16
                + 24
                + 68
                + 16
                + 16
                + 16
                + 12
                + 12
                + 12
                + 8;
            if slot < 11 {
                cycles += 8 + 4 + 4 + (slot as u32 - 1) * 24 - 4 + 12;
            } else {
                cycles += 12 + 12 + 12 + 4;
                if four_tile_count != 0 {
                    cycles += 12;
                } else {
                    cycles += 8 + 12 + 4 + 12;
                    four_tile_count = 1;
                }
            }
            cycles += 12 + 12 + 12 + 16 + 16 + 4 + 4 + 12 + 4 + 16 + 8 + 8;
            let bytes = if picture < 61 { 192 } else { 64 };
            // CopyData: 52 cycles per byte; FarCopyData2 adds 172.
            cycles += 4 + 8 + 24 + 52 * bytes + 172 + 12 + 12 + 12 + 8;
            if slot < 11 {
                cycles += 8
                    + 16
                    + 24
                    + 68
                    + 16
                    + 8
                    + 4
                    + 4
                    + 12
                    + 16
                    + 8
                    + 8
                    + 12
                    + 12
                    + 8
                    + 16
                    + 4
                    + 4
                    + 12
                    + 24
                    + 52 * bytes
                    + 172
                    + 12;
            } else {
                cycles += 12;
            }
            cycles += 12 + 12 + 12;
        }
        cycles += 4 + 8 + 4 + 4 + if index < count { 16 } else { 12 };
    }
    Some(cycles + 12 + 8 + 16 * 44 - 4 + 16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct OriginalWork {
        kind: crate::alloc_prelude::String,
        map: u8,
        x: u8,
        y: u8,
        count: u8,
        pictures: [u8; 16],
        cycles: u32,
    }

    #[test]
    fn reload_work_matches_original_instructions_and_actual_capture_entries() {
        // Five real menu-return entries plus 496 diagnostic entries executing
        // the original ROM routine. The latter are not gameplay evidence.
        let cases: crate::alloc_prelude::Vec<OriginalWork> = serde_json::from_str(include_str!(
            "../../tests/fixtures/sprite-reload-work-143.json"
        ))
        .unwrap();
        assert_eq!(cases.len(), 501);
        for case in cases {
            assert_eq!(
                sprite_reload_cycles(case.map, case.x, case.y, &case.pictures, case.count),
                Some(case.cycles),
                "{}",
                case.kind
            );
        }
    }

    #[test]
    fn malformed_slots_do_not_create_a_delay() {
        assert_eq!(sprite_reload_cycles(37, 0, 0, &[0; 16], 16), None);
        assert_eq!(sprite_reload_cycles(37, 0, 0, &[0; 16], 1), None);
        assert_eq!(sprite_reload_cycles(0, 0, 0, &[255; 16], 1), None);
    }
}

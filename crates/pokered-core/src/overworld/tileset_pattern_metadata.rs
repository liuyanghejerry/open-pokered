//! LoadTilesetTilePatternData always reads $600 bytes, including bytes past
//! the trimmed graphics. Metadata comes from the pinned retail Red symbols
//! and Makefile's --trim-whitespace / RedsHouse --preserve=$48 rules.
//! ROM SHA256: 5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TilesetPatternMetadata {
    pub bank: u8,
    pub address: u16,
    pub graphics_bytes: u16,
}

pub fn tileset_pattern_metadata(raw: u8) -> Option<TilesetPatternMetadata> {
    let &(bank, address, graphics_bytes) = TILESETS.get(usize::from(raw))?;
    Some(TilesetPatternMetadata { bank, address, graphics_bytes })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TilesetPatternByte {
    Graphics { tileset: u8, offset: u16 },
    Blockset { tileset: u8, offset: u16 },
    Vram(u16),
}

/// Resolve ownership without padding short PNGs or treating $8000 as the
/// next ROM bank. The VRAM branch must be read on the live instruction bus.
pub fn tileset_pattern_byte(raw: u8, offset: u16) -> Option<TilesetPatternByte> {
    if offset >= 0x600 { return None; }
    let metadata = tileset_pattern_metadata(raw)?;
    let address = metadata.address.checked_add(offset)?;
    if address >= 0x8000 { return Some(TilesetPatternByte::Vram(address)); }
    if offset < metadata.graphics_bytes {
        return Some(TilesetPatternByte::Graphics { tileset: raw, offset });
    }
    let offset = offset - metadata.graphics_bytes;
    // RedsHouse's 73 graphics tiles + 19 blocks leave 64 bytes of House_GFX
    // in the fixed-size read. Both house floors share this source address.
    if matches!(raw, 1 | 4) && offset >= 304 {
        Some(TilesetPatternByte::Graphics { tileset: 8, offset: offset - 304 })
    } else {
        Some(TilesetPatternByte::Blockset { tileset: raw, offset })
    }
}

const TILESETS: [(u8, u16, u16); 24] = [
    (0x19, 0x4000, 1504), // Overworld
    (0x19, 0x4de0, 1168), // RedsHouse1
    (0x1a, 0x4dbf, 1536), // Mart
    (0x1a, 0x63ff, 1536), // Forest
    (0x19, 0x4de0, 1168), // RedsHouse2
    (0x1a, 0x407f, 1536), // Dojo
    (0x1a, 0x4dbf, 1536), // Pokecenter
    (0x1a, 0x407f, 1536), // Gym
    (0x19, 0x53a0, 1504), // House
    (0x1a, 0x560f, 1520), // ForestGate
    (0x1a, 0x560f, 1520), // Museum
    (0x1b, 0x7d60, 400),  // Underground
    (0x1a, 0x560f, 1520), // Gate
    (0x1b, 0x6390, 1440), // Ship
    (0x19, 0x6610, 1504), // ShipPort
    (0x1b, 0x4000, 1472), // Cemetery
    (0x19, 0x6d60, 1520), // Interior
    (0x1b, 0x4ca0, 1056), // Cavern
    (0x1b, 0x58c0, 1504), // Lobby
    (0x19, 0x5bb0, 1504), // Mansion
    (0x1b, 0x6d10, 1472), // Lab
    (0x1b, 0x7670, 1200), // Club
    (0x1a, 0x71ff, 1536), // Facility
    (0x19, 0x76f0, 1120), // Plateau
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trimmed_graphics_cross_real_resource_and_vram_boundaries() {
        use TilesetPatternByte::*;
        assert_eq!(tileset_pattern_byte(0, 1504), Some(Blockset { tileset: 0, offset: 0 }));
        for id in [1, 4] {
            assert_eq!(tileset_pattern_byte(id, 1471), Some(Blockset { tileset: id, offset: 303 }));
            assert_eq!(tileset_pattern_byte(id, 1472), Some(Graphics { tileset: 8, offset: 0 }));
            assert_eq!(tileset_pattern_byte(id, 1535), Some(Graphics { tileset: 8, offset: 63 }));
        }
        assert_eq!(tileset_pattern_byte(11, 400), Some(Blockset { tileset: 11, offset: 0 }));
        assert_eq!(tileset_pattern_byte(11, 671), Some(Blockset { tileset: 11, offset: 271 }));
        assert_eq!(tileset_pattern_byte(11, 672), Some(Vram(0x8000)));
        assert_eq!(tileset_pattern_byte(11, 1535), Some(Vram(0x835f)));
        assert_eq!(tileset_pattern_byte(11, 1536), None);
        assert_eq!(tileset_pattern_byte(24, 0), None);
    }
}

//! Source SpriteSheetPointerTable metadata, not graphics or executable ROM.
//! Pinned Red table: bank5:$7b27,72 entries from data/sprites/sprites.asm.
//! ROM SHA256: 5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteSheetMetadata {
    pub address: u16,
    pub bytes: u8,
    pub bank: u8,
}
pub fn sprite_sheet_metadata(raw: u8) -> Option<SpriteSheetMetadata> {
    let index = raw.checked_sub(1)? as usize;
    let &(address, bytes, bank) = SHEETS.get(index)?;
    Some(SpriteSheetMetadata {
        address,
        bytes,
        bank,
    })
}
const SHEETS: [(u16, u8, u8); 72] = [
    (0x4180, 192, 5), // raw1
    (0x4300, 192, 5), // raw2
    (0x4480, 192, 5), // raw3
    (0x4600, 192, 5), // raw4
    (0x4780, 192, 5), // raw5
    (0x4900, 192, 5), // raw6
    (0x4a80, 192, 5), // raw7
    (0x4c00, 192, 5), // raw8
    (0x4d80, 192, 5), // raw9
    (0x4f00, 192, 5), // raw10
    (0x5080, 192, 5), // raw11
    (0x5200, 192, 5), // raw12
    (0x5380, 192, 5), // raw13
    (0x5500, 192, 5), // raw14
    (0x5680, 192, 5), // raw15
    (0x5800, 192, 5), // raw16
    (0x5980, 192, 5), // raw17
    (0x5b00, 192, 5), // raw18
    (0x5c80, 192, 5), // raw19
    (0x5e00, 192, 5), // raw20
    (0x5f80, 192, 5), // raw21
    (0x6040, 192, 5), // raw22
    (0x61c0, 192, 5), // raw23
    (0x6340, 192, 5), // raw24
    (0x64c0, 192, 5), // raw25
    (0x6640, 192, 5), // raw26
    (0x67c0, 192, 5), // raw27
    (0x6940, 192, 5), // raw28
    (0x6ac0, 192, 5), // raw29
    (0x6c40, 192, 5), // raw30
    (0x4000, 192, 4), // raw31
    (0x4000, 192, 4), // raw32
    (0x4180, 192, 4), // raw33
    (0x4300, 192, 4), // raw34
    (0x4480, 192, 4), // raw35
    (0x4540, 192, 4), // raw36
    (0x4600, 192, 4), // raw37
    (0x46c0, 192, 4), // raw38
    (0x4780, 192, 4), // raw39
    (0x4840, 192, 4), // raw40
    (0x4900, 192, 4), // raw41
    (0x49c0, 192, 4), // raw42
    (0x4a80, 192, 4), // raw43
    (0x4b40, 192, 4), // raw44
    (0x4c00, 192, 4), // raw45
    (0x4cc0, 192, 4), // raw46
    (0x4d80, 192, 4), // raw47
    (0x4f00, 192, 4), // raw48
    (0x5080, 192, 4), // raw49
    (0x5080, 192, 4), // raw50
    (0x6dc0, 192, 5), // raw51
    (0x6e80, 192, 5), // raw52
    (0x6f40, 192, 5), // raw53
    (0x7000, 192, 5), // raw54
    (0x7000, 192, 5), // raw55
    (0x70c0, 192, 5), // raw56
    (0x7240, 192, 5), // raw57
    (0x73c0, 192, 5), // raw58
    (0x7540, 192, 5), // raw59
    (0x76c0, 192, 5), // raw60
    (0x5140, 64, 4),  // raw61
    (0x5180, 64, 4),  // raw62
    (0x51c0, 64, 4),  // raw63
    (0x5200, 64, 4),  // raw64
    (0x5240, 64, 4),  // raw65
    (0x5280, 64, 4),  // raw66
    (0x52c0, 64, 4),  // raw67
    (0x5300, 64, 4),  // raw68
    (0x5300, 64, 4),  // raw69
    (0x5340, 64, 4),  // raw70
    (0x5340, 64, 4),  // raw71
    (0x5340, 64, 4),  // raw72
];

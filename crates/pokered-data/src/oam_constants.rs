pub const OAM_COUNT: usize = 40;
pub const OBJ_SIZE: usize = 4;
pub const OAM_Y_OFS: u8 = 16;
pub const OAM_X_OFS: u8 = 8;
pub const SCREEN_HEIGHT_PX: u8 = 144;
pub const SCREEN_WIDTH_PX: u8 = 160;

/// Tile size in pixels (Game Boy: 8×8 tiles).
pub const TILE_SIZE_PX: u32 = 8;

pub const BIT_END_OF_OAM_DATA: u8 = 0;
pub const BIT_SPRITE_UNDER_GRASS: u8 = 1;
pub const FACING_END: u8 = 1 << BIT_END_OF_OAM_DATA;
pub const UNDER_GRASS: u8 = 1 << BIT_SPRITE_UNDER_GRASS;
pub const OAM_XFLIP: u8 = 1 << 5;
/// Game Boy OAM attribute bit 7: sprite renders behind non-zero BG pixels.
/// Equivalent to OAM_PRIO in the original assembly.
pub const OAM_BG_PRIORITY: u8 = 0x80;

pub const NUM_SPRITESTATEDATA_STRUCTS: usize = 16;
pub const SPRITESTATEDATA_LENGTH: usize = 16;

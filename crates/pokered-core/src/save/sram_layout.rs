// SRAM bank layout constants for original Game Boy .sav file format.
// Layout (from layout.link):
//   SRAM Bank 0: "Sprite Buffers" — sprite buffers + Hall of Fame
//   SRAM Bank 1: "Save Data"      — padding $598 + sGameData + checksum
//   SRAM Bank 2: "Saved Boxes 1"  — boxes 1-6 + checksums
//   SRAM Bank 3: "Saved Boxes 2"  — boxes 7-12 + checksums

use super::game_data::*;
use super::ser_pokemon::*;

pub const SRAM_BANK_SIZE_LAYOUT: usize = 0x2000;
pub const SAV_FILE_SIZE: usize = SRAM_BANK_SIZE_LAYOUT * 4;

// Bank 0 offsets — SPRITEBUFFERSIZE = 7*7*8 = 392
pub const SPRITE_BUFFER_SIZE: usize = 7 * 7 * 8;
// sHallOfFame = 3 sprite buffers + $100 padding
pub const HOF_OFFSET: usize = SPRITE_BUFFER_SIZE * 3 + 0x100;
pub const HOF_MON_ENTRY_SIZE: usize = 16;
pub const HOF_TEAM_ENTRY_SIZE: usize = 6 * HOF_MON_ENTRY_SIZE;
pub const HOF_CAPACITY: usize = 50;
pub const HOF_TOTAL_SIZE: usize = HOF_TEAM_ENTRY_SIZE * HOF_CAPACITY;

// Bank 1 offsets — $598 padding then sGameData
pub const SAVE_DATA_PADDING: usize = 0x598;
pub const GAME_DATA_OFFSET: usize = SAVE_DATA_PADDING;
pub const PLAYER_NAME_OFFSET: usize = GAME_DATA_OFFSET;
pub const MAIN_DATA_OFFSET: usize = PLAYER_NAME_OFFSET + NAME_LENGTH;

// Sub-region sizes derived from wram.asm struct definitions
pub const SPRITE_DATA_REGION_SIZE: usize = SPRITE_DATA_SIZE;
pub const PARTY_DATA_SIZE: usize =
    1 + 7 + 6 * PARTY_STRUCT_SIZE + 6 * NAME_LENGTH + 6 * NAME_LENGTH;
pub const BOX_DATA_SIZE: usize =
    1 + 21 + 20 * BOX_STRUCT_SIZE + 20 * NAME_LENGTH + 20 * NAME_LENGTH;

// Banks 2-3: 6 boxes each + checksum block
pub const BOXES_PER_BANK: usize = 6;
pub const BOX_BANK_CHECKSUM_SIZE: usize = 1 + BOXES_PER_BANK;

pub const MONS_PER_BOX: usize = 20;
pub const SRAM_PARTY_LENGTH: usize = 6;

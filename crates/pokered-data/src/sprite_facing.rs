use crate::oam_constants::{FACING_END, OAM_XFLIP, UNDER_GRASS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OamTemplate {
    pub y_offset: u8,
    pub x_offset: u8,
    pub attributes: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteTilePattern {
    pub tiles: [u8; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteFacingEntry {
    pub tile_pattern: &'static SpriteTilePattern,
    pub oam_template: &'static [OamTemplate; 4],
}

pub const STANDING_DOWN: SpriteTilePattern = SpriteTilePattern {
    tiles: [0x00, 0x01, 0x02, 0x03],
};
pub const WALKING_DOWN: SpriteTilePattern = SpriteTilePattern {
    tiles: [0x80, 0x81, 0x82, 0x83],
};
pub const STANDING_UP: SpriteTilePattern = SpriteTilePattern {
    tiles: [0x04, 0x05, 0x06, 0x07],
};
pub const WALKING_UP: SpriteTilePattern = SpriteTilePattern {
    tiles: [0x84, 0x85, 0x86, 0x87],
};
pub const STANDING_LEFT: SpriteTilePattern = SpriteTilePattern {
    tiles: [0x08, 0x09, 0x0A, 0x0B],
};
pub const WALKING_LEFT: SpriteTilePattern = SpriteTilePattern {
    tiles: [0x88, 0x89, 0x8A, 0x8B],
};

pub const NORMAL_OAM: [OamTemplate; 4] = [
    OamTemplate {
        y_offset: 0,
        x_offset: 0,
        attributes: 0x00,
    },
    OamTemplate {
        y_offset: 0,
        x_offset: 8,
        attributes: 0x00,
    },
    OamTemplate {
        y_offset: 8,
        x_offset: 0,
        attributes: UNDER_GRASS,
    },
    OamTemplate {
        y_offset: 8,
        x_offset: 8,
        attributes: UNDER_GRASS | FACING_END,
    },
];

pub const FLIPPED_OAM: [OamTemplate; 4] = [
    OamTemplate {
        y_offset: 0,
        x_offset: 8,
        attributes: OAM_XFLIP,
    },
    OamTemplate {
        y_offset: 0,
        x_offset: 0,
        attributes: OAM_XFLIP,
    },
    OamTemplate {
        y_offset: 8,
        x_offset: 8,
        attributes: OAM_XFLIP | UNDER_GRASS,
    },
    OamTemplate {
        y_offset: 8,
        x_offset: 0,
        attributes: OAM_XFLIP | UNDER_GRASS | FACING_END,
    },
];

pub const SPRITE_FACING_TABLE: [SpriteFacingEntry; 32] = [
    // Sprites $1-$9: full directional sprites (16 entries)
    // Facing down
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &WALKING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &WALKING_DOWN,
        oam_template: &FLIPPED_OAM,
    },
    // Facing up
    SpriteFacingEntry {
        tile_pattern: &STANDING_UP,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &WALKING_UP,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_UP,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &WALKING_UP,
        oam_template: &FLIPPED_OAM,
    },
    // Facing left
    SpriteFacingEntry {
        tile_pattern: &STANDING_LEFT,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &WALKING_LEFT,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_LEFT,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &WALKING_LEFT,
        oam_template: &NORMAL_OAM,
    },
    // Facing right (reuses left tiles with flipped OAM)
    SpriteFacingEntry {
        tile_pattern: &STANDING_LEFT,
        oam_template: &FLIPPED_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &WALKING_LEFT,
        oam_template: &FLIPPED_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_LEFT,
        oam_template: &FLIPPED_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &WALKING_LEFT,
        oam_template: &FLIPPED_OAM,
    },
    // Sprites $a-$b: immobile sprites (16 entries, all same)
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
    SpriteFacingEntry {
        tile_pattern: &STANDING_DOWN,
        oam_template: &NORMAL_OAM,
    },
];

pub fn facing_table_index(image_index: u8) -> usize {
    let is_unchanging = image_index >= 0xA0;
    if is_unchanging {
        let base = (image_index & 0x0F) as usize;
        base + 16
    } else {
        (image_index & 0x0F) as usize
    }
}

pub const TILES_PER_SPRITE: usize = 12;
pub const FOUR_TILE_SPRITE_A: u8 = 0x0A;
pub const FOUR_TILE_SPRITE_B: u8 = 0x0B;
pub const FOUR_TILE_SPRITE_B_OFFSET: u8 = 0x0A * 12 + 4;

pub fn sprite_tile_base_offset(image_index: u8) -> u8 {
    let sprite_num = (image_index >> 4) & 0x0F;
    if sprite_num == FOUR_TILE_SPRITE_B {
        FOUR_TILE_SPRITE_B_OFFSET
    } else {
        sprite_num.wrapping_mul(TILES_PER_SPRITE as u8)
    }
}

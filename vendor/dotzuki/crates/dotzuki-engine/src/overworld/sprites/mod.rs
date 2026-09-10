pub mod collision;
pub mod oam;
pub mod update;

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// Generic constants (Game Boy hardware limits / sprite protocol)
// ---------------------------------------------------------------------------

pub const NUM_SPRITESTATEDATA_STRUCTS: usize = 16;
pub const OAM_COUNT: usize = 40;
pub const OAM_Y_OFS: u8 = 16;
pub const OAM_X_OFS: u8 = 8;
pub const OAM_BG_PRIORITY: u8 = 0x80;
pub const OAM_XFLIP: u8 = 1 << 5;
pub const FACING_END: u8 = 1 << 0;
pub const UNDER_GRASS: u8 = 1 << 1;
pub const GRASS_PRIORITY: u8 = 0x80;
pub const IMAGE_INDEX_OFFSCREEN: u8 = 0xFF;
pub const MOVEMENT_WALK: u8 = 0xFE;
pub const MOVEMENT_STAY: u8 = 0xFF;

// Facing direction constants
pub const FACING_DOWN: u8 = 0x00;
pub const FACING_UP: u8 = 0x04;
pub const FACING_LEFT: u8 = 0x08;
pub const FACING_RIGHT: u8 = 0x0C;

// ---------------------------------------------------------------------------
// MovementStatus enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MovementStatus {
    Uninitialized = 0,
    Ready = 1,
    Delayed = 2,
    Moving = 3,
}

impl MovementStatus {
    pub fn from_byte(b: u8) -> Self {
        match b & 0x7F {
            0 => Self::Uninitialized,
            1 => Self::Ready,
            2 => Self::Delayed,
            3 => Self::Moving,
            _ => Self::Uninitialized,
        }
    }

    pub fn face_player_bit(b: u8) -> bool {
        b & (1 << 7) != 0
    }
}

// ---------------------------------------------------------------------------
// Sprite-facing types (moved from pokered-data::sprite_facing)
// ---------------------------------------------------------------------------

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

// Tile patterns for each direction
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

// OAM layout templates for normal and flipped sprites
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

// Sprite facing table (32 entries: 16 full-direction sprites + 16 immobile sprites)
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

pub const TILES_PER_SPRITE: usize = 12;
pub const FOUR_TILE_SPRITE_A: u8 = 0x0A;
pub const FOUR_TILE_SPRITE_B: u8 = 0x0B;
pub const FOUR_TILE_SPRITE_B_OFFSET: u8 = 0x0A * 12 + 4;

pub fn facing_table_index(image_index: u8) -> usize {
    let is_unchanging = image_index >= 0xA0;
    if is_unchanging {
        let base = (image_index & 0x0F) as usize;
        base + 16
    } else {
        (image_index & 0x0F) as usize
    }
}

pub fn sprite_tile_base_offset(image_index: u8) -> u8 {
    let sprite_num = (image_index >> 4) & 0x0F;
    if sprite_num == FOUR_TILE_SPRITE_B {
        FOUR_TILE_SPRITE_B_OFFSET
    } else {
        sprite_num.wrapping_mul(TILES_PER_SPRITE as u8)
    }
}

// ---------------------------------------------------------------------------
// Core sprite data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OamEntry {
    pub y: u8,
    pub x: u8,
    pub tile_id: u8,
    pub attributes: u8,
}

#[derive(Debug, Clone)]
pub struct ShadowOam {
    pub entries: [OamEntry; OAM_COUNT],
}

impl Default for ShadowOam {
    fn default() -> Self {
        Self {
            entries: [OamEntry::default(); OAM_COUNT],
        }
    }
}

impl ShadowOam {
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = OamEntry::default();
        }
    }

    pub fn hide_all(&mut self, screen_height: u8) {
        for entry in &mut self.entries {
            entry.y = OAM_Y_OFS + screen_height;
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SpriteStateData1 {
    pub picture_id: u8,
    pub movement_status: u8,
    pub image_index: u8,
    pub y_step_vector: i8,
    pub y_pixels: u8,
    pub x_step_vector: i8,
    pub x_pixels: u8,
    pub intra_anim_frame_counter: u8,
    pub anim_frame_counter: u8,
    pub facing_direction: u8,
    pub y_adjusted: u8,
    pub x_adjusted: u8,
    pub collision_data: u8,
    pub field_0d: u8,
    pub collision_sprite_lo: u8,
    pub collision_sprite_hi: u8,
}

impl SpriteStateData1 {
    pub fn is_active(&self) -> bool {
        self.picture_id != 0
    }

    pub fn is_visible(&self) -> bool {
        self.image_index != IMAGE_INDEX_OFFSCREEN
    }

    pub fn movement_status(&self) -> MovementStatus {
        MovementStatus::from_byte(self.movement_status)
    }

    pub fn faces_player(&self) -> bool {
        MovementStatus::face_player_bit(self.movement_status)
    }

    pub fn clear_collision(&mut self) {
        self.collision_data = 0;
        self.collision_sprite_lo = 0;
        self.collision_sprite_hi = 0;
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SpriteStateData2 {
    pub walk_anim_counter: u8,
    pub field_01: u8,
    pub y_displacement: u8,
    pub x_displacement: u8,
    pub map_y: u8,
    pub map_x: u8,
    pub movement_byte1: u8,
    pub grass_priority: u8,
    pub movement_delay: u8,
    pub orig_facing_direction: u8,
    pub field_0a: u8,
    pub field_0b: u8,
    pub field_0c: u8,
    pub picture_id: u8,
    pub image_base_offset: u8,
    pub field_0f: u8,
}

impl SpriteStateData2 {
    pub fn init_displacement(&mut self) {
        self.y_displacement = 8;
        self.x_displacement = 8;
    }

    pub fn has_grass_priority(&self) -> bool {
        self.grass_priority & GRASS_PRIORITY != 0
    }
}

pub struct SpriteTable {
    pub data1: [SpriteStateData1; NUM_SPRITESTATEDATA_STRUCTS],
    pub data2: [SpriteStateData2; NUM_SPRITESTATEDATA_STRUCTS],
    pub shadow_oam: ShadowOam,
    pub oam_count: usize,
    pub ledge_or_fishing: bool,
}

impl Default for SpriteTable {
    fn default() -> Self {
        Self {
            data1: [SpriteStateData1::default(); NUM_SPRITESTATEDATA_STRUCTS],
            data2: [SpriteStateData2::default(); NUM_SPRITESTATEDATA_STRUCTS],
            shadow_oam: ShadowOam::default(),
            oam_count: 0,
            ledge_or_fishing: false,
        }
    }
}

impl SpriteTable {
    pub fn clear_all(&mut self) {
        self.data1 = [SpriteStateData1::default(); NUM_SPRITESTATEDATA_STRUCTS];
        self.data2 = [SpriteStateData2::default(); NUM_SPRITESTATEDATA_STRUCTS];
        self.shadow_oam.clear();
        self.oam_count = 0;
    }

    pub fn player_data1(&self) -> &SpriteStateData1 {
        &self.data1[0]
    }

    pub fn player_data1_mut(&mut self) -> &mut SpriteStateData1 {
        &mut self.data1[0]
    }

    pub fn player_data2(&self) -> &SpriteStateData2 {
        &self.data2[0]
    }

    pub fn player_data2_mut(&mut self) -> &mut SpriteStateData2 {
        &mut self.data2[0]
    }

    pub fn active_sprite_count(&self) -> usize {
        self.data1.iter().filter(|s| s.is_active()).count()
    }
}

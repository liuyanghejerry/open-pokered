use super::{SpriteTable, NUM_SPRITESTATEDATA_STRUCTS};

const COLLISION_THRESHOLD_STATIC: u8 = 7;
const COLLISION_THRESHOLD_MOVING: u8 = 9;

const COLLISION_BIT_DOWN: u8 = 1 << 0;
const COLLISION_BIT_UP: u8 = 1 << 1;
const COLLISION_BIT_LEFT: u8 = 1 << 2;
const COLLISION_BIT_RIGHT: u8 = 1 << 3;

pub const SPRITE_COLLISION_BIT_TABLE: [u16; NUM_SPRITESTATEDATA_STRUCTS] = [
    1 << 0,
    1 << 1,
    1 << 2,
    1 << 3,
    1 << 4,
    1 << 5,
    1 << 6,
    1 << 7,
    1 << 8,
    1 << 9,
    1 << 10,
    1 << 11,
    1 << 12,
    1 << 13,
    1 << 14,
    1 << 15,
];

pub fn detect_collisions_for_sprite(table: &mut SpriteTable, sprite_idx: usize) {
    if sprite_idx >= NUM_SPRITESTATEDATA_STRUCTS {
        return;
    }
    let sd1 = &table.data1[sprite_idx];
    if !sd1.is_active() || !sd1.is_visible() {
        return;
    }

    table.data1[sprite_idx].clear_collision();

    let y_adj = table.data1[sprite_idx].y_pixels.wrapping_add(4);
    let x_adj = table.data1[sprite_idx].x_pixels.wrapping_add(4);

    let y_step = table.data1[sprite_idx].y_step_vector;
    let x_step = table.data1[sprite_idx].x_step_vector;

    let y_threshold = if y_step != 0 {
        COLLISION_THRESHOLD_MOVING
    } else {
        COLLISION_THRESHOLD_STATIC
    };
    let x_threshold = if x_step != 0 {
        COLLISION_THRESHOLD_MOVING
    } else {
        COLLISION_THRESHOLD_STATIC
    };

    for other_idx in 0..NUM_SPRITESTATEDATA_STRUCTS {
        if other_idx == sprite_idx {
            continue;
        }

        let other = &table.data1[other_idx];
        if !other.is_active() || !other.is_visible() {
            continue;
        }

        let other_y = other.y_pixels.wrapping_add(4);
        let other_x = other.x_pixels.wrapping_add(4);

        let dy = y_adj.wrapping_sub(other_y);
        let dx = x_adj.wrapping_sub(other_x);

        let dy_abs = if dy > 127 {
            256u16 - dy as u16
        } else {
            dy as u16
        };
        let dx_abs = if dx > 127 {
            256u16 - dx as u16
        } else {
            dx as u16
        };

        if dy_abs >= y_threshold as u16 || dx_abs >= x_threshold as u16 {
            continue;
        }

        let collision_dir = if dy > 127 {
            COLLISION_BIT_UP
        } else if dy > 0 {
            COLLISION_BIT_DOWN
        } else if dx > 127 {
            COLLISION_BIT_LEFT
        } else {
            COLLISION_BIT_RIGHT
        };

        let sprite = &mut table.data1[sprite_idx];
        sprite.collision_data |= collision_dir;

        let bit = SPRITE_COLLISION_BIT_TABLE[other_idx];
        sprite.collision_sprite_lo |= (bit & 0xFF) as u8;
        sprite.collision_sprite_hi |= ((bit >> 8) & 0xFF) as u8;
    }
}

pub fn detect_all_collisions(table: &mut SpriteTable) {
    for i in 0..NUM_SPRITESTATEDATA_STRUCTS {
        detect_collisions_for_sprite(table, i);
    }
}

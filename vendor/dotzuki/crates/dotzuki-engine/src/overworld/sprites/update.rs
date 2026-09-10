use super::{
    collision::detect_collisions_for_sprite, oam::prepare_oam_data, MovementStatus, SpriteTable,
    MOVEMENT_STAY, MOVEMENT_WALK, NUM_SPRITESTATEDATA_STRUCTS,
};

pub fn update_sprites(table: &mut SpriteTable, screen_height: u8) {
    update_player_sprite(table);

    for i in 1..NUM_SPRITESTATEDATA_STRUCTS {
        update_npc_sprite(table, i);
    }

    detect_all_sprite_collisions(table);

    prepare_oam_data(table, screen_height);
}

fn update_player_sprite(table: &mut SpriteTable) {
    let sd1 = &table.data1[0];
    if !sd1.is_active() {
        return;
    }

    update_sprite_image_index(table, 0);
}

fn update_npc_sprite(table: &mut SpriteTable, idx: usize) {
    let sd1 = &table.data1[idx];
    if !sd1.is_active() {
        return;
    }

    let status = sd1.movement_status();
    match status {
        MovementStatus::Uninitialized => {}
        MovementStatus::Ready => {
            try_start_npc_movement(table, idx);
        }
        MovementStatus::Delayed => {
            update_movement_delay(table, idx);
        }
        MovementStatus::Moving => {
            continue_npc_movement(table, idx);
        }
    }

    update_sprite_image_index(table, idx);
}

fn try_start_npc_movement(table: &mut SpriteTable, idx: usize) {
    let sd2 = &table.data2[idx];
    if sd2.movement_byte1 == MOVEMENT_STAY {
        return;
    }

    if sd2.movement_byte1 == MOVEMENT_WALK {
        let sd2 = &mut table.data2[idx];
        if sd2.movement_delay > 0 {
            sd2.movement_delay -= 1;
            return;
        }
    }
}

fn update_movement_delay(table: &mut SpriteTable, idx: usize) {
    let sd2 = &mut table.data2[idx];
    if sd2.movement_delay > 0 {
        sd2.movement_delay -= 1;
    } else {
        table.data1[idx].movement_status =
            (table.data1[idx].movement_status & 0x80) | MovementStatus::Ready as u8;
    }
}

fn continue_npc_movement(table: &mut SpriteTable, idx: usize) {
    let sd1 = &mut table.data1[idx];

    sd1.y_pixels = (sd1.y_pixels as i16 + sd1.y_step_vector as i16) as u8;
    sd1.x_pixels = (sd1.x_pixels as i16 + sd1.x_step_vector as i16) as u8;

    let sd2 = &mut table.data2[idx];
    if sd2.walk_anim_counter > 0 {
        sd2.walk_anim_counter -= 1;
    } else {
        sd1.y_step_vector = 0;
        sd1.x_step_vector = 0;
        sd1.movement_status = (sd1.movement_status & 0x80) | MovementStatus::Ready as u8;
    }
}

fn update_sprite_image_index(table: &mut SpriteTable, idx: usize) {
    let sd1 = &table.data1[idx];
    let sd2 = &table.data2[idx];

    if sd2.image_base_offset == 0 && sd1.picture_id == 0 {
        return;
    }

    let facing = sd1.facing_direction;
    let anim_frame = sd1.anim_frame_counter & 0x03;

    let facing_base: u8 = match facing & 0x0C {
        0x00 => 0x00,
        0x04 => 0x04,
        0x08 => 0x08,
        0x0C => 0x0C,
        _ => 0x00,
    };

    let image_idx = (sd2.image_base_offset << 4) | facing_base | anim_frame;
    table.data1[idx].image_index = image_idx;
}

fn detect_all_sprite_collisions(table: &mut SpriteTable) {
    for i in 0..NUM_SPRITESTATEDATA_STRUCTS {
        if table.data1[i].is_active() {
            detect_collisions_for_sprite(table, i);
        }
    }
}

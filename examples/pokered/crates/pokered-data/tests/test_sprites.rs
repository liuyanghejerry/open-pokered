use pokered_data::oam_constants::*;
use pokered_data::sprite_facing::*;
use pokered_data::sprite_state_constants::*;

#[test]
fn oam_constants_values() {
    assert_eq!(OAM_COUNT, 40);
    assert_eq!(OBJ_SIZE, 4);
    assert_eq!(OAM_Y_OFS, 16);
    assert_eq!(OAM_X_OFS, 8);
    assert_eq!(SCREEN_HEIGHT_PX, 144);
    assert_eq!(SCREEN_WIDTH_PX, 160);
    assert_eq!(NUM_SPRITESTATEDATA_STRUCTS, 16);
    assert_eq!(SPRITESTATEDATA_LENGTH, 16);
}

#[test]
fn oam_bit_flags() {
    assert_eq!(FACING_END, 0x01);
    assert_eq!(UNDER_GRASS, 0x02);
    assert_eq!(OAM_XFLIP, 0x20);
}

#[test]
fn sprite_facing_table_length() {
    assert_eq!(SPRITE_FACING_TABLE.len(), 32);
}

#[test]
fn sprite_facing_table_directional_entries() {
    let down_stand = &SPRITE_FACING_TABLE[0];
    assert_eq!(down_stand.tile_pattern.tiles, [0x00, 0x01, 0x02, 0x03]);
    assert_eq!(down_stand.oam_template[0].attributes, 0x00);

    let up_stand = &SPRITE_FACING_TABLE[4];
    assert_eq!(up_stand.tile_pattern.tiles, [0x04, 0x05, 0x06, 0x07]);

    let left_stand = &SPRITE_FACING_TABLE[8];
    assert_eq!(left_stand.tile_pattern.tiles, [0x08, 0x09, 0x0A, 0x0B]);

    let right_stand = &SPRITE_FACING_TABLE[12];
    assert_eq!(right_stand.tile_pattern.tiles, [0x08, 0x09, 0x0A, 0x0B]);
    assert_ne!(right_stand.oam_template[0].attributes & OAM_XFLIP, 0);
}

#[test]
fn sprite_facing_table_immobile_entries() {
    for i in 16..32 {
        let entry = &SPRITE_FACING_TABLE[i];
        assert_eq!(entry.tile_pattern.tiles, [0x00, 0x01, 0x02, 0x03]);
    }
}

#[test]
fn facing_table_index_directional() {
    assert_eq!(facing_table_index(0x00), 0);
    assert_eq!(facing_table_index(0x01), 1);
    assert_eq!(facing_table_index(0x0F), 15);
    assert_eq!(facing_table_index(0x10), 0);
    assert_eq!(facing_table_index(0x1F), 15);
}

#[test]
fn facing_table_index_immobile() {
    assert_eq!(facing_table_index(0xA0), 16);
    assert_eq!(facing_table_index(0xAF), 31);
    assert_eq!(facing_table_index(0xB0), 16);
}

#[test]
fn sprite_tile_base_offset_standard() {
    assert_eq!(sprite_tile_base_offset(0x00), 0);
    assert_eq!(sprite_tile_base_offset(0x10), 12);
    assert_eq!(sprite_tile_base_offset(0x20), 24);
    assert_eq!(sprite_tile_base_offset(0x90), 108);
}

#[test]
fn sprite_tile_base_offset_four_tile_b() {
    assert_eq!(sprite_tile_base_offset(0xB0), FOUR_TILE_SPRITE_B_OFFSET);
    assert_eq!(FOUR_TILE_SPRITE_B_OFFSET, 0x0A * 12 + 4);
}

#[test]
fn sprite_state_constants_field_offsets() {
    assert_eq!(SPRITESTATEDATA1_PICTUREID, 0x00);
    assert_eq!(SPRITESTATEDATA1_MOVEMENTSTATUS, 0x01);
    assert_eq!(SPRITESTATEDATA1_IMAGEINDEX, 0x02);
    assert_eq!(SPRITESTATEDATA1_FACINGDIRECTION, 0x09);
    assert_eq!(SPRITESTATEDATA1_COLLISIONDATA, 0x0C);
    assert_eq!(SPRITESTATEDATA1_0F, 0x0F);

    assert_eq!(SPRITESTATEDATA2_WALKANIMATIONCOUNTER, 0x00);
    assert_eq!(SPRITESTATEDATA2_MAPY, 0x04);
    assert_eq!(SPRITESTATEDATA2_MAPX, 0x05);
    assert_eq!(SPRITESTATEDATA2_GRASSPRIORITY, 0x07);
    assert_eq!(SPRITESTATEDATA2_PICTUREID, 0x0D);
    assert_eq!(SPRITESTATEDATA2_IMAGEBASEOFFSET, 0x0E);
}

#[test]
fn facing_direction_constants() {
    assert_eq!(FACING_DOWN, 0x00);
    assert_eq!(FACING_UP, 0x04);
    assert_eq!(FACING_LEFT, 0x08);
    assert_eq!(FACING_RIGHT, 0x0C);
}

#[test]
fn movement_constants() {
    assert_eq!(MOVEMENT_WALK, 0xFE);
    assert_eq!(MOVEMENT_STAY, 0xFF);
    assert_eq!(DIR_ANY, 0x00);
    assert_eq!(DIR_NONE, 0xFF);
}

#[test]
fn people_event_flags() {
    assert_eq!(TRAINER, 1 << 6);
    assert_eq!(ITEM, 1 << 7);
}

#[test]
fn normal_oam_template_structure() {
    assert_eq!(NORMAL_OAM[0].y_offset, 0);
    assert_eq!(NORMAL_OAM[0].x_offset, 0);
    assert_eq!(NORMAL_OAM[0].attributes, 0x00);

    assert_eq!(NORMAL_OAM[1].x_offset, 8);

    assert_eq!(NORMAL_OAM[2].y_offset, 8);
    assert_ne!(NORMAL_OAM[2].attributes & UNDER_GRASS, 0);

    assert_ne!(NORMAL_OAM[3].attributes & FACING_END, 0);
}

#[test]
fn flipped_oam_template_structure() {
    assert_ne!(FLIPPED_OAM[0].attributes & OAM_XFLIP, 0);
    assert_eq!(FLIPPED_OAM[0].x_offset, 8);
    assert_eq!(FLIPPED_OAM[1].x_offset, 0);
}

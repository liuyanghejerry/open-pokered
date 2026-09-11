use super::collision::{
    detect_all_collisions, detect_collisions_for_sprite, SPRITE_COLLISION_BIT_TABLE,
};
use super::oam::prepare_oam_data;
use super::update::update_sprites;
use super::*;

fn make_active_sprite(table: &mut SpriteTable, idx: usize, y: u8, x: u8) {
    table.data1[idx].picture_id = 1;
    table.data1[idx].image_index = 0x00;
    table.data1[idx].y_pixels = y;
    table.data1[idx].x_pixels = x;
    table.data1[idx].movement_status = MovementStatus::Ready as u8;
    table.data2[idx].image_base_offset = 0;
}

#[test]
fn sprite_state_data1_default_is_inactive() {
    let sd1 = SpriteStateData1::default();
    assert!(!sd1.is_active());
    assert_eq!(sd1.image_index, 0);
}

#[test]
fn sprite_state_data1_active_and_visible() {
    let mut sd1 = SpriteStateData1::default();
    sd1.picture_id = 1;
    sd1.image_index = 0x10;
    assert!(sd1.is_active());
    assert!(sd1.is_visible());
}

#[test]
fn sprite_state_data1_offscreen() {
    let mut sd1 = SpriteStateData1::default();
    sd1.picture_id = 1;
    sd1.image_index = IMAGE_INDEX_OFFSCREEN;
    assert!(sd1.is_active());
    assert!(!sd1.is_visible());
}

#[test]
fn movement_status_from_byte() {
    assert_eq!(MovementStatus::from_byte(0), MovementStatus::Uninitialized);
    assert_eq!(MovementStatus::from_byte(1), MovementStatus::Ready);
    assert_eq!(MovementStatus::from_byte(2), MovementStatus::Delayed);
    assert_eq!(MovementStatus::from_byte(3), MovementStatus::Moving);
    assert_eq!(
        MovementStatus::from_byte(0x80),
        MovementStatus::Uninitialized
    );
    assert_eq!(MovementStatus::from_byte(0x81), MovementStatus::Ready);
}

#[test]
fn face_player_bit() {
    assert!(!MovementStatus::face_player_bit(0x01));
    assert!(MovementStatus::face_player_bit(0x81));
    assert!(MovementStatus::face_player_bit(0x80));
}

#[test]
fn shadow_oam_clear() {
    let mut oam = ShadowOam::default();
    oam.entries[0].y = 50;
    oam.entries[0].x = 60;
    oam.clear();
    assert_eq!(oam.entries[0].y, 0);
    assert_eq!(oam.entries[0].x, 0);
}

#[test]
fn shadow_oam_hide_all() {
    let mut oam = ShadowOam::default();
    oam.entries[0].y = 50;
    oam.hide_all(144);
    assert_eq!(oam.entries[0].y, OAM_Y_OFS + 144);
    assert_eq!(oam.entries[39].y, OAM_Y_OFS + 144);
}

#[test]
fn sprite_table_default_empty() {
    let table = SpriteTable::default();
    assert_eq!(table.active_sprite_count(), 0);
    assert_eq!(table.oam_count, 0);
}

#[test]
fn sprite_table_active_count() {
    let mut table = SpriteTable::default();
    table.data1[0].picture_id = 1;
    table.data1[3].picture_id = 5;
    assert_eq!(table.active_sprite_count(), 2);
}

#[test]
fn sprite_table_player_accessors() {
    let mut table = SpriteTable::default();
    table.data1[0].picture_id = 1;
    assert_eq!(table.player_data1().picture_id, 1);
    table.player_data1_mut().x_pixels = 80;
    assert_eq!(table.data1[0].x_pixels, 80);
}

#[test]
fn prepare_oam_no_sprites() {
    let mut table = SpriteTable::default();
    prepare_oam_data(&mut table, 144);
    assert_eq!(table.oam_count, 0);
    for i in 0..OAM_COUNT {
        assert_eq!(table.shadow_oam.entries[i].y, 160);
    }
}

#[test]
fn prepare_oam_single_sprite() {
    let mut table = SpriteTable::default();
    make_active_sprite(&mut table, 0, 64, 80);
    prepare_oam_data(&mut table, 144);
    assert_eq!(table.oam_count, 4);

    assert_eq!(table.shadow_oam.entries[0].y, 64 + 16);
    assert_eq!(table.shadow_oam.entries[0].x, 80 + 8);

    assert_eq!(table.shadow_oam.entries[1].y, 64 + 16);
    assert_eq!(table.shadow_oam.entries[1].x, 80 + 8 + 8);

    assert_eq!(table.shadow_oam.entries[2].y, 64 + 16 + 8);
    assert_eq!(table.shadow_oam.entries[2].x, 80 + 8);

    assert_eq!(table.shadow_oam.entries[3].y, 64 + 16 + 8);
    assert_eq!(table.shadow_oam.entries[3].x, 80 + 8 + 8);
}

#[test]
fn prepare_oam_skips_inactive_sprites() {
    let mut table = SpriteTable::default();
    make_active_sprite(&mut table, 2, 64, 80);
    prepare_oam_data(&mut table, 144);
    assert_eq!(table.oam_count, 4);
}

#[test]
fn prepare_oam_skips_offscreen_sprites() {
    let mut table = SpriteTable::default();
    table.data1[0].picture_id = 1;
    table.data1[0].image_index = IMAGE_INDEX_OFFSCREEN;
    prepare_oam_data(&mut table, 144);
    assert_eq!(table.oam_count, 0);
}

#[test]
fn prepare_oam_preserves_last_entries_during_ledge() {
    let mut table = SpriteTable::default();
    table.ledge_or_fishing = true;

    table.shadow_oam.entries[36].y = 42;
    table.shadow_oam.entries[37].y = 43;
    table.shadow_oam.entries[38].y = 44;
    table.shadow_oam.entries[39].y = 45;

    prepare_oam_data(&mut table, 144);

    assert_eq!(table.shadow_oam.entries[36].y, 42);
    assert_eq!(table.shadow_oam.entries[37].y, 43);
    assert_eq!(table.shadow_oam.entries[38].y, 44);
    assert_eq!(table.shadow_oam.entries[39].y, 45);
}

#[test]
fn collision_no_sprites() {
    let mut table = SpriteTable::default();
    detect_all_collisions(&mut table);
    for i in 0..NUM_SPRITESTATEDATA_STRUCTS {
        assert_eq!(table.data1[i].collision_data, 0);
    }
}

#[test]
fn collision_two_overlapping_sprites() {
    let mut table = SpriteTable::default();
    make_active_sprite(&mut table, 0, 64, 80);
    make_active_sprite(&mut table, 1, 64, 80);

    detect_collisions_for_sprite(&mut table, 0);

    assert_ne!(table.data1[0].collision_data, 0);
    assert_eq!(
        table.data1[0].collision_sprite_lo & (SPRITE_COLLISION_BIT_TABLE[1] as u8),
        SPRITE_COLLISION_BIT_TABLE[1] as u8
    );
}

#[test]
fn collision_far_apart_sprites() {
    let mut table = SpriteTable::default();
    make_active_sprite(&mut table, 0, 0, 0);
    make_active_sprite(&mut table, 1, 100, 100);

    detect_collisions_for_sprite(&mut table, 0);

    assert_eq!(table.data1[0].collision_data, 0);
}

#[test]
fn collision_bit_table_values() {
    for i in 0..NUM_SPRITESTATEDATA_STRUCTS {
        assert_eq!(SPRITE_COLLISION_BIT_TABLE[i], 1 << i);
    }
}

#[test]
fn sprite_data2_init_displacement() {
    let mut sd2 = SpriteStateData2::default();
    sd2.init_displacement();
    assert_eq!(sd2.y_displacement, 8);
    assert_eq!(sd2.x_displacement, 8);
}

#[test]
fn sprite_data2_grass_priority() {
    let mut sd2 = SpriteStateData2::default();
    assert!(!sd2.has_grass_priority());
    sd2.grass_priority = GRASS_PRIORITY;
    assert!(sd2.has_grass_priority());
}

#[test]
fn update_sprites_full_cycle() {
    let mut table = SpriteTable::default();
    make_active_sprite(&mut table, 0, 64, 80);
    table.data1[0].facing_direction = FACING_DOWN;

    update_sprites(&mut table, 144);

    assert_eq!(table.oam_count, 4);
    assert!(table.shadow_oam.entries[0].y > 0);
}

#[test]
fn sprite_table_clear_all() {
    let mut table = SpriteTable::default();
    make_active_sprite(&mut table, 0, 64, 80);
    make_active_sprite(&mut table, 5, 32, 40);
    table.oam_count = 8;

    table.clear_all();

    assert_eq!(table.active_sprite_count(), 0);
    assert_eq!(table.oam_count, 0);
}

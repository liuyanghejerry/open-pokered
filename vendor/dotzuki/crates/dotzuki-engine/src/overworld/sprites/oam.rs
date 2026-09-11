use super::{
    facing_table_index, sprite_tile_base_offset, OamEntry, SpriteTable, FACING_END,
    NUM_SPRITESTATEDATA_STRUCTS, OAM_BG_PRIORITY, OAM_COUNT, OAM_X_OFS, OAM_Y_OFS,
    SPRITE_FACING_TABLE, UNDER_GRASS,
};

const OAM_ENTRIES_PER_SPRITE: usize = 4;
const PRESERVED_OAM_ENTRIES: usize = 4;

pub fn prepare_oam_data(table: &mut SpriteTable, screen_height: u8) {
    let preserve_count = if table.ledge_or_fishing {
        PRESERVED_OAM_ENTRIES
    } else {
        0
    };
    let max_oam = OAM_COUNT - preserve_count;
    let mut oam_idx = 0;

    for sprite_idx in 0..NUM_SPRITESTATEDATA_STRUCTS {
        if oam_idx + OAM_ENTRIES_PER_SPRITE > max_oam {
            break;
        }

        let sd1 = &table.data1[sprite_idx];
        if !sd1.is_active() || !sd1.is_visible() {
            continue;
        }

        let sd2 = &table.data2[sprite_idx];
        let facing_idx = facing_table_index(sd1.image_index);
        if facing_idx >= SPRITE_FACING_TABLE.len() {
            continue;
        }

        let entry = &SPRITE_FACING_TABLE[facing_idx];
        let tile_base = sprite_tile_base_offset(sd1.image_index);
        let grass_attr = if sd2.has_grass_priority() {
            OAM_BG_PRIORITY
        } else {
            0
        };

        write_oam_block(
            &mut table.shadow_oam.entries,
            &mut oam_idx,
            sd1.y_pixels,
            sd1.x_pixels,
            tile_base,
            entry.tile_pattern,
            entry.oam_template,
            grass_attr,
        );
    }

    table.oam_count = oam_idx;

    for i in oam_idx..max_oam {
        table.shadow_oam.entries[i].y = OAM_Y_OFS + screen_height;
    }
}

fn write_oam_block(
    oam: &mut [OamEntry; OAM_COUNT],
    oam_idx: &mut usize,
    sprite_y: u8,
    sprite_x: u8,
    tile_base: u8,
    tile_pattern: &super::SpriteTilePattern,
    oam_template: &[super::OamTemplate; 4],
    grass_attr: u8,
) {
    for i in 0..OAM_ENTRIES_PER_SPRITE {
        let tmpl = &oam_template[i];
        let entry = &mut oam[*oam_idx];

        entry.y = sprite_y.wrapping_add(OAM_Y_OFS).wrapping_add(tmpl.y_offset);
        entry.x = sprite_x.wrapping_add(OAM_X_OFS).wrapping_add(tmpl.x_offset);
        entry.tile_id = tile_base.wrapping_add(tile_pattern.tiles[i]);

        let mut attr = tmpl.attributes;
        if attr & UNDER_GRASS != 0 {
            attr = (attr & !UNDER_GRASS) | grass_attr;
        }
        attr &= !FACING_END;
        entry.attributes = attr;

        *oam_idx += 1;
    }
}

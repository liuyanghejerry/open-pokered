use super::game_data::{
    GameData, MapConnection, MAX_BG_EVENTS, MAX_OBJECT_EVENTS, MAX_WARP_EVENTS,
};
use super::ser_pokemon::serialize_name;

fn push_u16_be(buf: &mut Vec<u8>, val: u16) {
    buf.push((val >> 8) as u8);
    buf.push((val & 0xFF) as u8);
}

fn push_bcd_money(buf: &mut Vec<u8>, money: u32) {
    let capped = money.min(999999);
    let d5 = (capped / 100000) as u8;
    let d4 = ((capped / 10000) % 10) as u8;
    let d3 = ((capped / 1000) % 10) as u8;
    let d2 = ((capped / 100) % 10) as u8;
    let d1 = ((capped / 10) % 10) as u8;
    let d0 = (capped % 10) as u8;
    buf.push((d5 << 4) | d4);
    buf.push((d3 << 4) | d2);
    buf.push((d1 << 4) | d0);
}

fn push_bcd_coins(buf: &mut Vec<u8>, coins: u16) {
    let capped = coins.min(9999);
    let d3 = (capped / 1000) as u8;
    let d2 = ((capped / 100) % 10) as u8;
    let d1 = ((capped / 10) % 10) as u8;
    let d0 = (capped % 10) as u8;
    buf.push((d3 << 4) | d2);
    buf.push((d1 << 4) | d0);
}

fn serialize_map_connection(conn: &MapConnection, buf: &mut Vec<u8>) {
    buf.push(conn.connected_map);
    push_u16_be(buf, conn.strip_src);
    push_u16_be(buf, conn.strip_dest);
    buf.push(conn.strip_length);
    buf.push(conn.connected_width);
    buf.push(conn.y_alignment);
    buf.push(conn.x_alignment);
    push_u16_be(buf, conn.view_pointer);
}

fn serialize_inventory(
    inv: &crate::items::inventory::Inventory,
    buf: &mut Vec<u8>,
    capacity: usize,
) {
    let count = inv.count() as u8;
    buf.push(count);
    for i in 0..inv.count() {
        if let Some((item, qty)) = inv.get(i) {
            buf.push(item as u8);
            buf.push(qty);
        }
    }
    buf.push(0xFF);
    let items_written = inv.count() * 2 + 1;
    let total_slots = capacity * 2 + 1;
    for _ in items_written..total_slots {
        buf.push(0);
    }
}

/// Serialize GameData into a byte buffer matching the exact WRAM layout
/// that the original game stores in SRAM (sGameData region).
///
/// This must produce a byte-identical stream to what sram_deser_game_data
/// reads, including all padding/unused bytes that exist in WRAM.
pub fn serialize_game_data_into(data: &GameData, buf: &mut Vec<u8>) {
    // -- Pokedex owned/seen flags (19+19 = 38 bytes) --
    buf.extend_from_slice(data.pokedex.owned_flags());
    buf.extend_from_slice(data.pokedex.seen_flags());

    // -- Bag inventory (count + 20*2 slots + terminator) --
    serialize_inventory(&data.bag, buf, 20);

    // -- Money (3 bytes BCD) --
    push_bcd_money(buf, data.player_money);

    // -- Rival name (11 bytes) --
    serialize_name(&data.rival_name, buf);

    // -- Options (1 byte) --
    buf.push(data.options.to_byte());

    // -- Badges + unused byte (2 bytes) --
    buf.push(data.obtained_badges);
    buf.push(0); // _unused_badges

    // -- Letter printing delay (1 byte) --
    buf.push(data.letter_printing_delay_flags);

    // -- Player ID (2 bytes BE) --
    push_u16_be(buf, data.player_id);

    // -- Map music/bank/pal (3 bytes) --
    buf.push(data.map_music_sound_id);
    buf.push(data.map_music_rom_bank);
    buf.push(data.map_pal_offset);

    // -- Current map + wCurrentTileBlockMapViewPointer (1 + 2 bytes) --
    buf.push(data.position.map_id);
    push_u16_be(buf, 0); // _tile_block_ptr (WRAM pointer, unused in save)

    // -- Player position (4 bytes) --
    buf.push(data.position.y);
    buf.push(data.position.x);
    buf.push(data.position.y_block);
    buf.push(data.position.x_block);

    // -- Last map + unused last_map_width (2 bytes) --
    buf.push(data.last_map);
    buf.push(0); // _unused_last_map_width

    // -- Map header (10 bytes) --
    buf.push(data.map_header.tileset);
    buf.push(data.map_header.height);
    buf.push(data.map_header.width);
    push_u16_be(buf, data.map_header.data_ptr);
    push_u16_be(buf, data.map_header.text_ptr);
    push_u16_be(buf, data.map_header.script_ptr);
    buf.push(data.map_header.connections);

    // -- Connections: ALWAYS serialize all 4 (4×11 = 44 bytes) --
    // In WRAM, all 4 connection structs are always present regardless of mask.
    for i in 0..4 {
        serialize_map_connection(&data.connections[i], buf);
    }

    // -- Sprite set (11 bytes) + sprite_set_id (1 byte) --
    buf.extend_from_slice(&data.sprite_set);
    buf.push(data.sprite_set_id);

    // -- _obj_data_ptr_temp (2 bytes) + 2 bytes padding --
    push_u16_be(buf, 0); // _obj_data_ptr_temp
    buf.push(0); // padding
    buf.push(0); // padding

    // -- Map background tile (1 byte) --
    buf.push(data.map_background_tile);

    // -- Warps: count + 32 entries × 4 bytes + destination_warp_id --
    buf.push(data.num_warps);
    for i in 0..MAX_WARP_EVENTS {
        let w = &data.warp_entries[i];
        buf.push(w.y);
        buf.push(w.x);
        buf.push(w.warp_id);
        buf.push(w.map_id);
    }
    buf.push(data.destination_warp_id);

    // -- 128 bytes padding (wMapDataStart region in WRAM) --
    buf.extend_from_slice(&[0u8; 128]);

    // -- Signs: count + coords (16×2) + text_ids (16) --
    buf.push(data.num_signs);
    for i in 0..MAX_BG_EVENTS {
        buf.push(data.sign_coords[i].0);
        buf.push(data.sign_coords[i].1);
    }
    for i in 0..MAX_BG_EVENTS {
        buf.push(data.sign_text_ids[i]);
    }

    // -- Sprites: count + _y_offset + _x_offset + sprite data --
    buf.push(data.num_sprites);
    buf.push(0); // _y_offset
    buf.push(0); // _x_offset
    for i in 0..MAX_OBJECT_EVENTS {
        buf.push(data.map_sprite_data[i].0);
        buf.push(data.map_sprite_data[i].1);
    }
    for i in 0..MAX_OBJECT_EVENTS {
        buf.push(data.map_sprite_extra_data[i].0);
        buf.push(data.map_sprite_extra_data[i].1);
    }

    // -- Map dimensions + _map_view_vram_ptr --
    buf.push(data.current_map_height2);
    buf.push(data.current_map_width2);
    push_u16_be(buf, 0); // _map_view_vram_ptr

    // -- Player direction fields (3 bytes) --
    buf.push(data.player_moving_direction);
    buf.push(data.player_last_stop_direction);
    buf.push(data.player_direction);

    // -- Tileset fields --
    buf.push(data.tileset_bank);
    push_u16_be(buf, data.tileset_blocks_ptr);
    push_u16_be(buf, data.tileset_gfx_ptr);
    push_u16_be(buf, data.tileset_collision_ptr);
    buf.extend_from_slice(&data.tileset_talking_over_tiles);
    buf.push(data.grass_tile);
    buf.extend_from_slice(&[0u8; 4]); // 4 bytes padding after grass_tile

    // -- PC items inventory --
    serialize_inventory(&data.pc_items, buf, 50);

    // -- Current box num + 1 byte padding --
    buf.push(data.current_box_num);
    buf.push(0); // padding

    // -- Num HoF teams + unused_map_var --
    buf.push(data.num_hof_teams);
    buf.push(0); // _unused_map_var

    // -- Player coins (2 bytes BCD) --
    push_bcd_coins(buf, data.player_coins);

    // -- Toggleable object flags --
    buf.extend_from_slice(&data.toggleable_object_flags);

    // -- Post-toggleable padding region: 7 + 1 + 33 + 1 = 42 bytes --
    buf.extend_from_slice(&[0u8; 7]); // skip(7)
    buf.push(0); // _saved_sprite_image_index
    buf.extend_from_slice(&[0u8; 16 * 2 + 1]); // _toggleable_list (33 bytes)
    buf.push(0); // skip(1)

    // -- Game progress flags --
    buf.extend_from_slice(&data.game_progress_flags);

    // -- 56 bytes padding after game_progress_flags --
    buf.extend_from_slice(&[0u8; 56]);

    // -- Hidden items and coins --
    buf.extend_from_slice(&data.obtained_hidden_items);
    buf.extend_from_slice(&data.obtained_hidden_coins);

    // -- Walk/bike/surf state + 10 bytes padding --
    buf.push(data.walk_bike_surf_state);
    buf.extend_from_slice(&[0u8; 10]);

    // -- Town visited flags --
    buf.extend_from_slice(&data.town_visited_flags);

    // -- Safari steps (2 bytes BE) --
    push_u16_be(buf, data.safari_steps);

    // -- Fossil item + fossil mon + 2 bytes padding --
    buf.push(data.fossil_item);
    buf.push(data.fossil_mon);
    buf.push(0); // padding
    buf.push(0); // padding

    // -- Enemy mon/trainer class + _jumping_y_index --
    buf.push(data.enemy_mon_or_trainer_class);
    buf.push(0); // _jumping_y_index

    // -- Rival starter + 1 byte padding --
    buf.push(data.rival_starter);
    buf.push(0); // padding

    // -- Player starter + _boulder_sprite --
    buf.push(data.player_starter);
    buf.push(0); // _boulder_sprite

    // -- Last blackout map + destination map + _unused_player_byte --
    buf.push(data.last_blackout_map);
    buf.push(data.destination_map);
    buf.push(0); // _unused_player_byte

    // -- Tile in front of boulder, dungeon warp dest, which dungeon warp --
    buf.push(data.tile_in_front_of_boulder);
    buf.push(data.dungeon_warp_dest_map);
    buf.push(data.which_dungeon_warp);

    // -- _unused_card_key_gate + 8 bytes padding --
    buf.push(0); // _unused_card_key_gate
    buf.extend_from_slice(&[0u8; 8]);

    // -- Status flags with WRAM padding (15 bytes total) --
    // Layout: [flags0, pad, flags1, pad, flags2, flags3, flags4, pad, flags5, pad, flags6, flags7, flags8, pad, flags9]
    buf.push(data.status_flags[0]);
    buf.push(0); // padding
    buf.push(data.status_flags[1]);
    buf.push(0); // padding
    buf.push(data.status_flags[2]);
    buf.push(data.status_flags[3]);
    buf.push(data.status_flags[4]);
    buf.push(0); // padding
    buf.push(data.status_flags[5]);
    buf.push(0); // padding
    buf.push(data.status_flags[6]);
    buf.push(data.status_flags[7]);
    buf.push(data.status_flags[8]);
    buf.push(0); // padding
    buf.push(data.status_flags[9]);

    // -- Completed in-game trade flags + 2 bytes padding --
    push_u16_be(buf, data.completed_in_game_trade_flags);
    buf.push(0); // padding
    buf.push(0); // padding

    // -- Warped from warp/map + 2 bytes padding --
    buf.push(data.warped_from_warp);
    buf.push(data.warped_from_map);
    buf.push(0); // padding
    buf.push(0); // padding

    // -- Card key door Y/X + 2 bytes padding --
    buf.push(data.card_key_door_y);
    buf.push(data.card_key_door_x);
    buf.push(0); // padding
    buf.push(0); // padding

    // -- Trash can locks + 2 bytes padding --
    buf.push(data.first_lock_trash_can);
    buf.push(data.second_lock_trash_can);
    buf.push(0); // padding
    buf.push(0); // padding

    // -- Event flags --
    buf.extend_from_slice(&data.event_flags);

    // -- Grass encounters: rate + mons data + 8 bytes padding --
    buf.push(data.grass_rate);
    buf.extend_from_slice(&data.grass_mons);
    buf.extend_from_slice(&[0u8; 8]); // padding after grass mons

    // -- Water encounters: rate + mons data --
    buf.push(data.water_rate);
    buf.extend_from_slice(&data.water_mons);

    // -- Trainer header ptr + 6 bytes padding --
    push_u16_be(buf, data.trainer_header_ptr);
    buf.extend_from_slice(&[0u8; 6]);

    // -- Opponent after wrong answer + cur map script + 7 bytes padding --
    buf.push(data.opponent_after_wrong_answer);
    buf.push(data.cur_map_script);
    buf.extend_from_slice(&[0u8; 7]);

    // -- Play time (5 bytes) --
    buf.push(data.play_time.hours);
    buf.push(if data.play_time.maxed { 0xFF } else { 0x00 });
    buf.push(data.play_time.minutes);
    buf.push(data.play_time.seconds);
    buf.push(data.play_time.frames);

    // -- Safari zone game over + num safari balls --
    buf.push(data.safari_zone_game_over);
    buf.push(data.num_safari_balls);

    // -- Daycare: in_use flag, then name, then OT name, then box struct --
    buf.push(if data.daycare.in_use { 1 } else { 0 });
    serialize_name(&data.daycare_mon_name, buf);
    serialize_name(&data.daycare_mon_ot, buf);
    serialize_daycare_mon(&data.daycare, buf);
}

fn serialize_daycare_mon(dc: &super::game_data::DayCareMon, buf: &mut Vec<u8>) {
    buf.push(dc.species);
    push_u16_be(buf, dc.hp);
    buf.push(dc.box_level);
    buf.push(dc.status);
    buf.push(dc.type1);
    buf.push(dc.type2);
    buf.push(dc.catch_rate);
    for &m in &dc.moves {
        buf.push(m);
    }
    push_u16_be(buf, dc.ot_id);
    buf.push(((dc.exp >> 16) & 0xFF) as u8);
    buf.push(((dc.exp >> 8) & 0xFF) as u8);
    buf.push((dc.exp & 0xFF) as u8);
    push_u16_be(buf, dc.hp_exp);
    push_u16_be(buf, dc.attack_exp);
    push_u16_be(buf, dc.defense_exp);
    push_u16_be(buf, dc.speed_exp);
    push_u16_be(buf, dc.special_exp);
    buf.push((dc.dvs >> 8) as u8);
    buf.push((dc.dvs & 0xFF) as u8);
    for &p in &dc.pp {
        buf.push(p);
    }
}

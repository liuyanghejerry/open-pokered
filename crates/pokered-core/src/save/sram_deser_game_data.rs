use super::game_data::*;
use super::serialization::SaveError;
use super::sram_deser::SramReader;
use crate::options_menu::GameOptions;
use crate::pokemon::pokedex::Pokedex;

pub fn deserialize_game_data(reader: &mut SramReader) -> Result<GameData, SaveError> {
    let mut data = GameData::new();

    let owned = reader.read_bytes(19)?;
    let seen = reader.read_bytes(19)?;
    data.pokedex = Pokedex::from_flags(owned, seen);

    data.bag = reader.read_inventory(20)?;
    data.player_money = reader.read_bcd_money()?;
    data.rival_name = reader.read_name()?;
    data.options = GameOptions::from_byte(reader.read_u8()?);
    data.obtained_badges = reader.read_u8()?;
    let _unused_badges = reader.read_u8()?;
    data.letter_printing_delay_flags = reader.read_u8()?;
    data.player_id = reader.read_u16_be()?;
    data.map_music_sound_id = reader.read_u8()?;
    data.map_music_rom_bank = reader.read_u8()?;
    data.map_pal_offset = reader.read_u8()?;

    let cur_map = reader.read_u8()?;
    let _tile_block_ptr = reader.read_u16_be()?;
    let y = reader.read_u8()?;
    let x = reader.read_u8()?;
    let y_block = reader.read_u8()?;
    let x_block = reader.read_u8()?;
    data.position = MapPosition {
        map_id: cur_map,
        y,
        x,
        y_block,
        x_block,
    };

    data.last_map = reader.read_u8()?;
    let _unused_last_map_width = reader.read_u8()?;

    data.map_header = MapHeader {
        tileset: reader.read_u8()?,
        height: reader.read_u8()?,
        width: reader.read_u8()?,
        data_ptr: reader.read_u16_be()?,
        text_ptr: reader.read_u16_be()?,
        script_ptr: reader.read_u16_be()?,
        connections: reader.read_u8()?,
    };

    // In raw WRAM, all 4 connection headers are always present (4×11=44 bytes).
    // The connections mask only indicates which ones contain valid data.
    for i in 0..4 {
        data.connections[i] = reader.read_map_connection()?;
    }

    let sprite_set_bytes = reader.read_bytes(SPRITE_SET_LENGTH)?;
    data.sprite_set.copy_from_slice(sprite_set_bytes);
    data.sprite_set_id = reader.read_u8()?;

    let _obj_data_ptr_temp = reader.read_u16_be()?;
    reader.skip(2)?;
    data.map_background_tile = reader.read_u8()?;

    data.num_warps = reader.read_u8()?;
    for i in 0..MAX_WARP_EVENTS {
        data.warp_entries[i] = WarpEntry {
            y: reader.read_u8()?,
            x: reader.read_u8()?,
            warp_id: reader.read_u8()?,
            map_id: reader.read_u8()?,
        };
    }
    data.destination_warp_id = reader.read_u8()?;

    reader.skip(128)?;

    data.num_signs = reader.read_u8()?;
    for i in 0..MAX_BG_EVENTS {
        let sy = reader.read_u8()?;
        let sx = reader.read_u8()?;
        data.sign_coords[i] = (sy, sx);
    }
    for i in 0..MAX_BG_EVENTS {
        data.sign_text_ids[i] = reader.read_u8()?;
    }

    data.num_sprites = reader.read_u8()?;
    let _y_offset = reader.read_u8()?;
    let _x_offset = reader.read_u8()?;
    for i in 0..MAX_OBJECT_EVENTS {
        let a = reader.read_u8()?;
        let b = reader.read_u8()?;
        data.map_sprite_data[i] = (a, b);
    }
    for i in 0..MAX_OBJECT_EVENTS {
        let a = reader.read_u8()?;
        let b = reader.read_u8()?;
        data.map_sprite_extra_data[i] = (a, b);
    }

    data.current_map_height2 = reader.read_u8()?;
    data.current_map_width2 = reader.read_u8()?;
    let _map_view_vram_ptr = reader.read_u16_be()?;

    data.player_moving_direction = reader.read_u8()?;
    data.player_last_stop_direction = reader.read_u8()?;
    data.player_direction = reader.read_u8()?;

    data.tileset_bank = reader.read_u8()?;
    data.tileset_blocks_ptr = reader.read_u16_be()?;
    data.tileset_gfx_ptr = reader.read_u16_be()?;
    data.tileset_collision_ptr = reader.read_u16_be()?;
    let talking_tiles = reader.read_bytes(3)?;
    data.tileset_talking_over_tiles
        .copy_from_slice(talking_tiles);
    data.grass_tile = reader.read_u8()?;
    reader.skip(4)?;

    data.pc_items = reader.read_inventory(50)?;
    data.current_box_num = reader.read_u8()?;
    reader.skip(1)?;
    data.num_hof_teams = reader.read_u8()?;
    let _unused_map_var = reader.read_u8()?;
    data.player_coins = reader.read_bcd_coins()?;

    let toggle_bytes = reader.read_bytes(TOGGLEABLE_OBJECT_BYTES)?;
    data.toggleable_object_flags.copy_from_slice(toggle_bytes);

    reader.skip(7)?;
    let _saved_sprite_image_index = reader.read_u8()?;
    let _toggleable_list = reader.read_bytes(16 * 2 + 1)?;
    reader.skip(1)?;

    let gp_bytes = reader.read_bytes(GAME_PROGRESS_FLAGS_SIZE)?;
    data.game_progress_flags = gp_bytes.to_vec();

    reader.skip(56)?;

    let hidden_items = reader.read_bytes(HIDDEN_ITEMS_BYTES)?;
    data.obtained_hidden_items.copy_from_slice(hidden_items);
    let hidden_coins = reader.read_bytes(HIDDEN_COINS_BYTES)?;
    data.obtained_hidden_coins.copy_from_slice(hidden_coins);

    data.walk_bike_surf_state = reader.read_u8()?;
    reader.skip(10)?;

    let town_bytes = reader.read_bytes(CITY_VISITED_BYTES)?;
    data.town_visited_flags.copy_from_slice(town_bytes);

    data.safari_steps = reader.read_u16_be()?;
    data.fossil_item = reader.read_u8()?;
    data.fossil_mon = reader.read_u8()?;
    reader.skip(2)?;
    data.enemy_mon_or_trainer_class = reader.read_u8()?;
    let _jumping_y_index = reader.read_u8()?;
    data.rival_starter = reader.read_u8()?;
    reader.skip(1)?;
    data.player_starter = reader.read_u8()?;
    let _boulder_sprite = reader.read_u8()?;
    data.last_blackout_map = reader.read_u8()?;
    data.destination_map = reader.read_u8()?;
    let _unused_player_byte = reader.read_u8()?;
    data.tile_in_front_of_boulder = reader.read_u8()?;
    data.dungeon_warp_dest_map = reader.read_u8()?;
    data.which_dungeon_warp = reader.read_u8()?;
    let _unused_card_key_gate = reader.read_u8()?;
    reader.skip(8)?;

    // WRAM status flags region is 15 bytes with padding:
    // [flags1, ds1, beatGym, ds1, flags2, flags3, flags4, ds1, flags5, ds1, flags6, flags7, elite4, ds1, movement]
    // We extract the 12 meaningful bytes into status_flags[0..12], skipping padding.
    data.status_flags[0] = reader.read_u8()?;
    reader.skip(1)?;
    data.status_flags[1] = reader.read_u8()?;
    reader.skip(1)?;
    data.status_flags[2] = reader.read_u8()?;
    data.status_flags[3] = reader.read_u8()?;
    data.status_flags[4] = reader.read_u8()?;
    reader.skip(1)?;
    data.status_flags[5] = reader.read_u8()?;
    reader.skip(1)?;
    data.status_flags[6] = reader.read_u8()?;
    data.status_flags[7] = reader.read_u8()?;
    data.status_flags[8] = reader.read_u8()?;
    reader.skip(1)?;
    data.status_flags[9] = reader.read_u8()?;

    data.completed_in_game_trade_flags = reader.read_u16_be()?;
    reader.skip(2)?;
    data.warped_from_warp = reader.read_u8()?;
    data.warped_from_map = reader.read_u8()?;
    reader.skip(2)?;
    data.card_key_door_y = reader.read_u8()?;
    data.card_key_door_x = reader.read_u8()?;
    reader.skip(2)?;
    data.first_lock_trash_can = reader.read_u8()?;
    data.second_lock_trash_can = reader.read_u8()?;
    reader.skip(2)?;

    let ef_bytes = reader.read_bytes(NUM_EVENTS_BYTES)?;
    data.event_flags = ef_bytes.to_vec();

    data.grass_rate = reader.read_u8()?;
    let grass_data = reader.read_bytes(WILDDATA_LENGTH - 1)?;
    data.grass_mons.copy_from_slice(grass_data);
    reader.skip(8)?;
    data.water_rate = reader.read_u8()?;
    let water_data = reader.read_bytes(WILDDATA_LENGTH - 1)?;
    data.water_mons.copy_from_slice(water_data);

    data.trainer_header_ptr = reader.read_u16_be()?;
    reader.skip(6)?;
    data.opponent_after_wrong_answer = reader.read_u8()?;
    data.cur_map_script = reader.read_u8()?;
    reader.skip(7)?;

    data.play_time = PlayTime {
        hours: reader.read_u8()?,
        maxed: reader.read_u8()? != 0,
        minutes: reader.read_u8()?,
        seconds: reader.read_u8()?,
        frames: reader.read_u8()?,
    };

    data.safari_zone_game_over = reader.read_u8()?;
    data.num_safari_balls = reader.read_u8()?;

    let daycare_in_use = reader.read_u8()?;
    data.daycare.in_use = daycare_in_use != 0;
    data.daycare_mon_name = reader.read_name()?;
    data.daycare_mon_ot = reader.read_name()?;
    deserialize_daycare_box_struct(reader, &mut data.daycare)?;

    Ok(data)
}

fn deserialize_daycare_box_struct(
    reader: &mut SramReader,
    dc: &mut DayCareMon,
) -> Result<(), SaveError> {
    dc.species = reader.read_u8()?;
    dc.hp = reader.read_u16_be()?;
    dc.box_level = reader.read_u8()?;
    dc.status = reader.read_u8()?;
    dc.type1 = reader.read_u8()?;
    dc.type2 = reader.read_u8()?;
    dc.catch_rate = reader.read_u8()?;
    for i in 0..4 {
        dc.moves[i] = reader.read_u8()?;
    }
    dc.ot_id = reader.read_u16_be()?;
    dc.exp = reader.read_exp24()?;
    dc.hp_exp = reader.read_u16_be()?;
    dc.attack_exp = reader.read_u16_be()?;
    dc.defense_exp = reader.read_u16_be()?;
    dc.speed_exp = reader.read_u16_be()?;
    dc.special_exp = reader.read_u16_be()?;
    dc.dvs = reader.read_u16_be()?;
    for i in 0..4 {
        dc.pp[i] = reader.read_u8()?;
    }
    Ok(())
}

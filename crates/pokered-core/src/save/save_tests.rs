use crate::battle::state::{decode_name, encode_name, Pokemon, StatusCondition, NAME_TEXT_BUF};
use crate::pokemon::party::Party;
use crate::pokemon::pc_box::PcBox;
use crate::save::game_data::{GameData, PlayTime, GAME_PROGRESS_FLAGS_SIZE, NUM_EVENTS_BYTES};
use crate::save::hall_of_fame::{HallOfFame, HofMon, HofTeam, HOF_TEAM_CAPACITY};
use crate::save::ser_pokemon::*;
use crate::save::serialization::*;
use crate::save::SaveData;
use crate::save_menu::calc_checksum;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn make_test_pokemon(species: Species, level: u8) -> Pokemon {
    Pokemon {
        species,
        nickname: [0x50; 11],
        level,
        hp: 100,
        max_hp: 100,
        attack: 50,
        defense: 40,
        speed: 60,
        special: 55,
        type1: PokemonType::Normal,
        type2: PokemonType::Normal,
        moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        pp: [35, 0, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xAB, 0xCD],
        stat_exp: [100, 200, 300, 400, 500],
        total_exp: 1000,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

#[test]
fn test_save_data_new_defaults() {
    let save = SaveData::new();
    assert!(save.player_name.is_empty());
    assert_eq!(save.tile_animations, 0);
    assert_eq!(save.game_data.player_money, 0);
    assert_eq!(save.game_data.obtained_badges, 0);
    assert_eq!(save.party.count(), 0);
    assert_eq!(save.hall_of_fame.team_count(), 0);
}

#[test]
fn test_save_data_clear() {
    let mut save = SaveData::new();
    save.player_name = vec![0x80, 0x81, 0x82];
    save.game_data.player_money = 99999;
    save.game_data.obtained_badges = 0xFF;
    save.tile_animations = 5;
    save.clear();
    assert!(save.player_name.is_empty());
    assert_eq!(save.game_data.player_money, 0);
    assert_eq!(save.game_data.obtained_badges, 0);
    assert_eq!(save.tile_animations, 0);
}

#[test]
fn test_checksum_empty_save() {
    let save = SaveData::new();
    let cksum = save.compute_checksum();
    assert!(save.validate_checksum(cksum));
}

#[test]
fn test_checksum_changes_with_data() {
    let mut save = SaveData::new();
    let cksum1 = save.compute_checksum();
    save.game_data.player_money = 50000;
    let cksum2 = save.compute_checksum();
    assert_ne!(cksum1, cksum2);
}

#[test]
fn test_checksum_validate_bad() {
    let save = SaveData::new();
    let cksum = save.compute_checksum();
    assert!(!save.validate_checksum(cksum.wrapping_add(1)));
}

#[test]
fn test_calc_checksum_matches_asm() {
    let data = [0x01, 0x02, 0x03, 0x04];
    let sum: u8 = data.iter().fold(0u8, |a, &b| a.wrapping_add(b));
    let expected = !sum;
    assert_eq!(calc_checksum(&data), expected);
}

#[test]
fn test_status_to_byte_roundtrip() {
    let cases = vec![
        StatusCondition::None,
        StatusCondition::Sleep(3),
        StatusCondition::Poison,
        StatusCondition::Burn,
        StatusCondition::Freeze,
        StatusCondition::Paralysis,
    ];
    for status in cases {
        let b = status_to_byte(&status);
        let back = byte_to_status(b);
        assert_eq!(status, back);
    }
}

#[test]
fn test_serialize_box_mon_size() {
    let mon = make_test_pokemon(Species::Pikachu, 25);
    let mut buf = Vec::new();
    serialize_box_mon(&mon, &mut buf);
    assert_eq!(buf.len(), BOX_STRUCT_SIZE);
}

#[test]
fn test_serialize_party_mon_size() {
    let mon = make_test_pokemon(Species::Pikachu, 25);
    let mut buf = Vec::new();
    serialize_party_mon(&mon, &mut buf);
    assert_eq!(buf.len(), PARTY_STRUCT_SIZE);
}

#[test]
fn test_deserialize_box_mon_roundtrip() {
    let mon = make_test_pokemon(Species::Charmander, 16);
    let mut buf = Vec::new();
    serialize_box_mon(&mon, &mut buf);
    let restored = deserialize_box_mon(&buf).unwrap();
    assert_eq!(restored.species, mon.species);
    assert_eq!(restored.level, mon.level);
    assert_eq!(restored.hp, mon.hp);
    assert_eq!(restored.total_exp, mon.total_exp);
    assert_eq!(restored.dv_bytes, mon.dv_bytes);
    assert_eq!(restored.moves[0], mon.moves[0]);
}

#[test]
fn test_deserialize_party_mon_roundtrip() {
    let mon = make_test_pokemon(Species::Blastoise, 36);
    let mut buf = Vec::new();
    serialize_party_mon(&mon, &mut buf);
    let restored = deserialize_party_mon(&buf).unwrap();
    assert_eq!(restored.species, mon.species);
    assert_eq!(restored.level, mon.level);
    assert_eq!(restored.max_hp, mon.max_hp);
    assert_eq!(restored.attack, mon.attack);
    assert_eq!(restored.defense, mon.defense);
    assert_eq!(restored.speed, mon.speed);
    assert_eq!(restored.special, mon.special);
}

#[test]
fn test_deserialize_box_mon_too_short() {
    let buf = [0u8; BOX_STRUCT_SIZE - 1];
    assert_eq!(deserialize_box_mon(&buf), Err(SaveError::DataTooShort));
}

#[test]
fn test_deserialize_party_mon_too_short() {
    let buf = [0u8; PARTY_STRUCT_SIZE - 1];
    assert_eq!(deserialize_party_mon(&buf), Err(SaveError::DataTooShort));
}

#[test]
fn test_serialize_name_padding() {
    let mut buf = Vec::new();
    let name = vec![0x80, 0x81, 0x82];
    serialize_name(&name, &mut buf);
    assert_eq!(buf.len(), 11);
    assert_eq!(buf[0], 0x80);
    assert_eq!(buf[1], 0x81);
    assert_eq!(buf[2], 0x82);
    assert_eq!(buf[3], 0x50);
    for i in 4..11 {
        assert_eq!(buf[i], 0x50);
    }
}

#[test]
fn test_serialize_name_empty() {
    let mut buf = Vec::new();
    serialize_name(&[], &mut buf);
    assert_eq!(buf.len(), 11);
    assert_eq!(buf[0], 0x50);
}

#[test]
fn test_deserialize_name_with_terminator() {
    let data = [
        0x80, 0x81, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let name = deserialize_name(&data);
    assert_eq!(name, vec![0x80, 0x81]);
}

#[test]
fn test_serialize_sprite_data_size() {
    let mut buf = Vec::new();
    serialize_sprite_data_into(&mut buf);
    assert_eq!(buf.len(), SPRITE_DATA_SIZE);
    assert!(buf.iter().all(|&b| b == 0));
}

#[test]
fn test_serialize_party_empty() {
    let party = Party::new();
    let mut buf = Vec::new();
    serialize_party_into(&party, &mut buf);
    assert_eq!(buf[0], 0);
    assert_eq!(buf[1], 0xFF);
}

#[test]
fn test_serialize_box_empty() {
    let box_data = PcBox::new();
    let mut buf = Vec::new();
    serialize_box_into(&box_data, &mut buf);
    assert_eq!(buf[0], 0);
    assert_eq!(buf[1], 0xFF);
}

#[test]
fn test_game_data_badge_operations() {
    let mut gd = GameData::new();
    assert_eq!(gd.badge_count(), 0);
    assert!(!gd.has_badge(0));
    gd.set_badge(0);
    assert!(gd.has_badge(0));
    assert_eq!(gd.badge_count(), 1);
    gd.set_badge(7);
    assert!(gd.has_badge(7));
    assert_eq!(gd.badge_count(), 2);
}

#[test]
fn test_game_data_badge_out_of_range() {
    let mut gd = GameData::new();
    gd.set_badge(8);
    assert!(!gd.has_badge(8));
    assert_eq!(gd.badge_count(), 0);
}

#[test]
fn test_play_time_total_seconds() {
    let pt = PlayTime {
        hours: 1,
        maxed: false,
        minutes: 30,
        seconds: 45,
        frames: 0,
    };
    assert_eq!(pt.total_seconds(), 3600 + 1800 + 45);
}

#[test]
fn test_play_time_tick_basic() {
    let mut pt = PlayTime::new();
    // 60 ticks = 1 second
    for _ in 0..60 {
        pt.tick();
    }
    assert_eq!(pt.frames, 0);
    assert_eq!(pt.seconds, 1);
    assert_eq!(pt.minutes, 0);
    assert_eq!(pt.hours, 0);
    assert!(!pt.maxed);
}

#[test]
fn test_play_time_tick_rollover_minutes() {
    let mut pt = PlayTime::new();
    // 60 seconds = 1 minute
    for _ in 0..60 * 60 {
        pt.tick();
    }
    assert_eq!(pt.frames, 0);
    assert_eq!(pt.seconds, 0);
    assert_eq!(pt.minutes, 1);
    assert_eq!(pt.hours, 0);
}

#[test]
fn test_play_time_tick_rollover_hours() {
    let mut pt = PlayTime::new();
    // 60 minutes = 1 hour
    for _ in 0..60 * 60 * 60 {
        pt.tick();
    }
    assert_eq!(pt.seconds, 0);
    assert_eq!(pt.minutes, 0);
    assert_eq!(pt.hours, 1);
    assert!(!pt.maxed);
}

#[test]
fn test_play_time_tick_maxed_at_255_hours() {
    let mut pt = PlayTime {
        hours: 254,
        maxed: false,
        minutes: 59,
        seconds: 59,
        frames: 59,
    };
    pt.tick(); // should trigger hour rollover to 255 and max
    assert_eq!(pt.hours, 255);
    assert_eq!(pt.frames, 59);
    assert!(pt.maxed);

    // Further ticks should be no-ops
    pt.tick();
    assert_eq!(pt.hours, 255);
    assert_eq!(pt.frames, 59);
    assert!(pt.maxed);
}

#[test]
fn test_play_time_tick_single_frame() {
    let mut pt = PlayTime::new();
    pt.tick();
    assert_eq!(pt.frames, 1);
    assert_eq!(pt.seconds, 0);
}

#[test]
fn test_hof_push_team() {
    let mut hof = HallOfFame::new();
    assert_eq!(hof.team_count(), 0);
    let mut team = HofTeam::new();
    team.add_mon(HofMon::new(25, 50, &[0x80, 0x81]));
    hof.push_team(team);
    assert_eq!(hof.team_count(), 1);
    assert_eq!(hof.get_team(0).unwrap().mons().len(), 1);
}

#[test]
fn test_hof_capacity_evicts_oldest() {
    let mut hof = HallOfFame::new();
    for i in 0..HOF_TEAM_CAPACITY + 5 {
        let mut team = HofTeam::new();
        team.add_mon(HofMon::new(i as u8, 50, &[]));
        hof.push_team(team);
    }
    assert_eq!(hof.team_count(), HOF_TEAM_CAPACITY);
    let first = hof.get_team(0).unwrap();
    assert_eq!(first.mons()[0].species, 5);
}

#[test]
fn test_hof_clear() {
    let mut hof = HallOfFame::new();
    let team = HofTeam::new();
    hof.push_team(team);
    hof.clear();
    assert_eq!(hof.team_count(), 0);
}

#[test]
fn test_hof_team_max_mons() {
    let mut team = HofTeam::new();
    for i in 0..10 {
        team.add_mon(HofMon::new(i, 50, &[]));
    }
    assert_eq!(team.mons().len(), 6);
}

#[test]
fn test_save_error_display() {
    assert_eq!(
        format!("{}", SaveError::DataTooShort),
        "save data too short"
    );
    assert_eq!(format!("{}", SaveError::BadChecksum), "bad checksum");
    assert_eq!(format!("{}", SaveError::InvalidData), "invalid save data");
}

#[test]
fn test_species_from_index_id() {
    assert_eq!(Species::from_index_id(25), Species::Pikachu);
    assert_eq!(Species::from_index_id(0), Species::None);
    assert_eq!(Species::from_index_id(255), Species::None);
}

#[test]
fn test_pokemon_type_from_id() {
    assert_eq!(PokemonType::from_id(0x00), PokemonType::Normal);
    assert_eq!(PokemonType::from_id(0x14), PokemonType::Fire);
    assert_eq!(PokemonType::from_id(0xFF), PokemonType::Normal);
}

#[test]
fn test_move_id_from_id() {
    assert_eq!(MoveId::from_id(0x01), MoveId::Pound);
    assert_eq!(MoveId::from_id(0x00), MoveId::None);
    assert_eq!(MoveId::from_id(0xFF), MoveId::None);
}

#[test]
fn test_game_data_serialization_deterministic() {
    let gd = GameData::new();
    let mut buf1 = Vec::new();
    let mut buf2 = Vec::new();
    gd.serialize_into(&mut buf1);
    gd.serialize_into(&mut buf2);
    assert_eq!(buf1, buf2);
}

#[test]
fn test_save_data_default_eq_new() {
    let a = SaveData::new();
    let b = SaveData::default();
    assert_eq!(a.compute_checksum(), b.compute_checksum());
}

#[test]
fn test_game_data_event_flags_size() {
    let gd = GameData::new();
    assert_eq!(gd.event_flags.len(), NUM_EVENTS_BYTES);
    assert_eq!(gd.game_progress_flags.len(), GAME_PROGRESS_FLAGS_SIZE);
}

#[test]
fn test_position_sram_roundtrip() {
    use crate::save::game_data::MapPosition;
    use crate::save::sram_export::export_sram;
    use crate::save::sram_import::import_sram;

    let mut save = SaveData::new();
    save.player_name = vec![0x80, 0x81, 0x82, 0x50];

    save.game_data.position = MapPosition {
        map_id: 0,
        y: 5,
        x: 3,
        y_block: 1,
        x_block: 1,
    };
    save.game_data.player_direction = 4;
    save.game_data.player_last_stop_direction = 4;
    save.game_data.player_moving_direction = 4;
    save.game_data.current_map_height2 = 18;
    save.game_data.current_map_width2 = 20;

    let sram = export_sram(&save);
    let restored = import_sram(&sram).expect("roundtrip import should succeed");

    assert_eq!(restored.game_data.position.map_id, 0, "map_id mismatch");
    assert_eq!(restored.game_data.position.x, 3, "x mismatch");
    assert_eq!(restored.game_data.position.y, 5, "y mismatch");
    assert_eq!(restored.game_data.position.x_block, 1, "x_block mismatch");
    assert_eq!(restored.game_data.position.y_block, 1, "y_block mismatch");
    assert_eq!(restored.game_data.player_direction, 4, "direction mismatch");
}

#[test]
fn test_position_sram_roundtrip_route1() {
    use crate::save::game_data::MapPosition;
    use crate::save::sram_export::export_sram;
    use crate::save::sram_import::import_sram;

    let mut save = SaveData::new();
    save.player_name = vec![0x87, 0x84, 0x83, 0x50];

    save.game_data.position = MapPosition {
        map_id: 12,
        y: 27,
        x: 4,
        y_block: 1,
        x_block: 0,
    };
    save.game_data.player_direction = 0;
    save.game_data.current_map_height2 = 36;
    save.game_data.current_map_width2 = 20;

    let sram = export_sram(&save);
    let restored = import_sram(&sram).expect("roundtrip import should succeed");

    assert_eq!(restored.game_data.position.map_id, 12);
    assert_eq!(restored.game_data.position.x, 4);
    assert_eq!(restored.game_data.position.y, 27);
    assert_eq!(restored.game_data.player_direction, 0);
}

#[test]
fn test_party_sram_roundtrip() {
    use crate::save::sram_export::export_sram;
    use crate::save::sram_import::import_sram;

    let mut save = SaveData::new();
    save.player_name = vec![0x80, 0x81, 0x50];

    let mon = make_test_pokemon(Species::Bulbasaur, 5);
    save.party.add(mon).unwrap();

    assert_eq!(save.party.count(), 1, "party should have 1 member before save");

    let sram = export_sram(&save);
    let restored = import_sram(&sram).expect("roundtrip import should succeed");

    assert_eq!(restored.party.count(), 1, "party should have 1 member after load");
    let mon = restored.party.get(0).unwrap();
    assert_eq!(mon.species, Species::Bulbasaur);
    assert_eq!(mon.level, 5);
}

#[test]
fn test_party_sram_roundtrip_multiple() {
    use crate::save::sram_export::export_sram;
    use crate::save::sram_import::import_sram;

    let mut save = SaveData::new();
    save.player_name = vec![0x80, 0x81, 0x50];

    save.party.add(make_test_pokemon(Species::Charmander, 5)).unwrap();
    save.party.add(make_test_pokemon(Species::Squirtle, 5)).unwrap();
    save.party.add(make_test_pokemon(Species::Bulbasaur, 5)).unwrap();

    assert_eq!(save.party.count(), 3, "party should have 3 members before save");

    let sram = export_sram(&save);
    let restored = import_sram(&sram).expect("roundtrip import should succeed");

    assert_eq!(restored.party.count(), 3, "party should have 3 members after load");
    assert_eq!(restored.party.get(0).unwrap().species, Species::Charmander);
    assert_eq!(restored.party.get(1).unwrap().species, Species::Squirtle);
    assert_eq!(restored.party.get(2).unwrap().species, Species::Bulbasaur);
}

#[test]
fn test_deserialize_mon_ot_id_and_pp_ups_roundtrip() {
    // MON_OTID (struct offsets 12-13) and the PP bytes' high-2-bit PP-Up counts
    // must round-trip through the box/party struct serialization.
    let mut mon = make_test_pokemon(Species::Pikachu, 25);
    mon.ot_id = 0xBEEF;
    mon.moves = [MoveId::Tackle, MoveId::Thunder, MoveId::QuickAttack, MoveId::None];
    mon.pp = [35, 10, 30, 0];
    mon.pp_ups = [3, 1, 0, 0];

    let mut buf = Vec::new();
    serialize_box_mon(&mon, &mut buf);
    // The packed bytes are exactly what the original layout expects.
    assert_eq!(buf[12], 0xBE);
    assert_eq!(buf[13], 0xEF);
    assert_eq!(buf[29], 35 | (3 << 6));
    assert_eq!(buf[30], 10 | (1 << 6));
    assert_eq!(buf[31], 30);

    let restored = deserialize_box_mon(&buf).unwrap();
    assert_eq!(restored.ot_id, 0xBEEF);
    assert_eq!(restored.pp, [35, 10, 30, 0]);
    assert_eq!(restored.pp_ups, [3, 1, 0, 0]);
}

#[test]
fn test_party_sram_roundtrip_preserves_ot_and_nickname() {
    use crate::save::sram_export::export_sram;
    use crate::save::sram_import::import_sram;

    let mut save = SaveData::new();
    save.player_name = vec![0x80, 0x81, 0x50];

    // A nicknamed, traded mon: OT id/name + nickname + PP-Ups all set.
    let mut traded = make_test_pokemon(Species::Pikachu, 25);
    traded.ot_id = 0x1234;
    traded.ot_name = encode_name("RED");
    traded.set_nickname("SPARKY");
    traded.pp_ups = [2, 0, 0, 0];
    save.party.add(traded).unwrap();

    // An untouched own mon: no nickname, no OT data.
    save.party.add(make_test_pokemon(Species::Bulbasaur, 5)).unwrap();

    let sram = export_sram(&save);
    let restored = import_sram(&sram).expect("roundtrip import should succeed");

    let m0 = restored.party.get(0).unwrap();
    assert_eq!(m0.ot_id, 0x1234, "OT id must survive the round-trip");
    let mut buf = [0u8; NAME_TEXT_BUF];
    assert_eq!(decode_name(&m0.ot_name, &mut buf), "RED", "OT name must survive");
    assert_eq!(decode_name(&m0.nickname, &mut buf), "SPARKY", "nickname must survive");
    assert_eq!(m0.pp_ups, [2, 0, 0, 0], "PP-Ups must survive");
    assert!(
        m0.is_traded,
        "OT id != player id (0) → traded flag derived on import"
    );

    let m1 = restored.party.get(1).unwrap();
    assert_eq!(m1.ot_id, 0);
    assert!(!m1.is_traded, "ot_id 0 (unknown) counts as own");
    assert_eq!(m1.ot_name, [0x50; 11], "blank OT name stays unset");
    assert_eq!(
        m1.nickname, [0x50; 11],
        "a stored species-name nickname decodes back to unset"
    );
}

#[test]
fn test_box_sram_roundtrip_preserves_ot_and_nickname() {
    use crate::save::sram_export::export_sram;
    use crate::save::sram_import::import_sram;

    let mut save = SaveData::new();
    save.player_name = vec![0x80, 0x81, 0x50];

    let mut boxed = make_test_pokemon(Species::Eevee, 30);
    boxed.ot_id = 0x00FF;
    boxed.ot_name = encode_name("BLUE");
    boxed.set_nickname("VOLT");
    boxed.pp_ups = [1, 1, 0, 0];
    save.current_box.deposit(boxed).unwrap();

    let sram = export_sram(&save);
    let restored = import_sram(&sram).expect("roundtrip import should succeed");

    let m = restored.current_box.get(0).expect("box mon present");
    assert_eq!(m.ot_id, 0x00FF);
    let mut buf = [0u8; NAME_TEXT_BUF];
    assert_eq!(decode_name(&m.ot_name, &mut buf), "BLUE");
    assert_eq!(decode_name(&m.nickname, &mut buf), "VOLT");
    assert_eq!(m.pp_ups, [1, 1, 0, 0]);
}

/// The editor tooling (`export-snapshot`/`import-snapshot`, pokered-runner-web)
/// persists `SaveData` as JSON. Chunk C moved party/boxes/HoF to fixed-capacity
/// arrays; this locks the legacy JSON shapes (plain arrays of active entries —
/// no `mons`/`count` wrappers) so snapshots stay round-trip compatible.
#[test]
fn test_snapshot_json_keeps_legacy_shapes() {
    let mut save = SaveData::new();
    save.player_name = vec![0x80, 0x81];
    save.party.add(make_test_pokemon(Species::Pikachu, 25)).unwrap();
    save.party.add(make_test_pokemon(Species::Bulbasaur, 5)).unwrap();
    save.current_box
        .deposit(make_test_pokemon(Species::Charmander, 10))
        .unwrap();
    let mut team = HofTeam::new();
    team.add_mon(HofMon::new(25, 50, &[0x8F, 0x88, 0x8A, 0x80]));
    save.hall_of_fame.push_team(team);

    let json = serde_json::to_string(&save).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();

    // `party`: a plain 2-element array (the old Vec shape). Species serialize
    // as their enum-variant names.
    let party = value["party"].as_array().expect("party is an array");
    assert_eq!(party.len(), 2);
    assert_eq!(party[0]["species"], "Pikachu");
    assert_eq!(party[1]["species"], "Bulbasaur");

    // `current_box`: a plain 1-element array.
    let box_data = value["current_box"].as_array().expect("current_box is an array");
    assert_eq!(box_data.len(), 1);
    assert_eq!(box_data[0]["species"], "Charmander");

    // `pc_storage.boxes`: 12 plain arrays.
    let boxes = value["pc_storage"]["boxes"]
        .as_array()
        .expect("pc_storage.boxes is an array");
    assert_eq!(boxes.len(), 12);
    assert!(boxes.iter().all(|b| b.as_array().unwrap().is_empty()));

    // `hall_of_fame`: a 1-team array; team is a 1-mon array; nickname stays a
    // byte array without the 0x50 padding.
    let hof = value["hall_of_fame"].as_array().expect("hof is an array");
    assert_eq!(hof.len(), 1);
    let team_json = hof[0].as_array().expect("team is an array");
    assert_eq!(team_json.len(), 1);
    assert_eq!(team_json[0]["species"], 25);
    assert_eq!(team_json[0]["level"], 50);
    assert_eq!(
        team_json[0]["nickname"].as_array().unwrap(),
        &[serde_json::Value::from(0x8F), serde_json::Value::from(0x88),
          serde_json::Value::from(0x8A), serde_json::Value::from(0x80)]
    );

    // Full round-trip: JSON → SaveData → JSON is stable.
    let back: SaveData = serde_json::from_str(&json).unwrap();
    assert_eq!(back.party.count(), 2);
    assert_eq!(back.current_box.count(), 1);
    assert_eq!(back.hall_of_fame.team_count(), 1);
    assert_eq!(back.hall_of_fame.get_team(0).unwrap().mons()[0].nickname_bytes(), &[0x8F, 0x88, 0x8A, 0x80]);
    assert_eq!(serde_json::to_string(&back).unwrap(), json);
}

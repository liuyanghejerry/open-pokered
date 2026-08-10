use crate::battle::state::{Pokemon, StatusCondition};
use crate::pokemon::party::Party;
use crate::pokemon::pc_box::PcBox;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

use super::game_data::NAME_LENGTH;
use super::serialization::SaveError;

pub const NUM_MOVES: usize = 4;
pub const BOX_STRUCT_SIZE: usize = 33;
pub const PARTY_STRUCT_SIZE: usize = 44;
pub const SPRITE_STATE_SIZE: usize = 16;
pub const NUM_SPRITE_STRUCTS: usize = 16;
pub const SPRITE_DATA_SIZE: usize = SPRITE_STATE_SIZE * NUM_SPRITE_STRUCTS * 2;

pub fn status_to_byte(status: &StatusCondition) -> u8 {
    match status {
        StatusCondition::None => 0,
        StatusCondition::Sleep(n) => *n & 0x07,
        StatusCondition::Poison => 1 << 3,
        StatusCondition::Burn => 1 << 4,
        StatusCondition::Freeze => 1 << 5,
        StatusCondition::Paralysis => 1 << 6,
    }
}

pub fn byte_to_status(b: u8) -> StatusCondition {
    if b & 0x07 != 0 {
        StatusCondition::Sleep(b & 0x07)
    } else if b & (1 << 3) != 0 {
        StatusCondition::Poison
    } else if b & (1 << 4) != 0 {
        StatusCondition::Burn
    } else if b & (1 << 5) != 0 {
        StatusCondition::Freeze
    } else if b & (1 << 6) != 0 {
        StatusCondition::Paralysis
    } else {
        StatusCondition::None
    }
}

fn push_u16_be(buf: &mut Vec<u8>, val: u16) {
    buf.push((val >> 8) as u8);
    buf.push((val & 0xFF) as u8);
}

fn read_u16_be(data: &[u8], offset: usize) -> u16 {
    ((data[offset] as u16) << 8) | data[offset + 1] as u16
}

pub fn serialize_box_mon(mon: &Pokemon, buf: &mut Vec<u8>) {
    buf.push(mon.species as u8);
    push_u16_be(buf, mon.hp);
    buf.push(mon.level);
    buf.push(status_to_byte(&mon.status));
    buf.push(mon.type1 as u8);
    buf.push(mon.type2 as u8);
    buf.push(0);
    for i in 0..NUM_MOVES {
        buf.push(mon.moves[i] as u8);
    }
    // MON_OTID — the original-trainer ID (drives traded-mon obedience).
    push_u16_be(buf, mon.ot_id);
    buf.push(((mon.total_exp >> 16) & 0xFF) as u8);
    buf.push(((mon.total_exp >> 8) & 0xFF) as u8);
    buf.push((mon.total_exp & 0xFF) as u8);
    for i in 0..5 {
        push_u16_be(buf, mon.stat_exp[i]);
    }
    buf.push(mon.dv_bytes[0]);
    buf.push(mon.dv_bytes[1]);
    for i in 0..NUM_MOVES {
        // PP byte: low 6 bits = current PP, high 2 bits = PP-Ups applied (PP_MASK
        // / the mon struct's PP-up packing in the original).
        buf.push((mon.pp[i] & 0x3F) | ((mon.pp_ups[i] & 0x03) << 6));
    }
}

pub fn serialize_party_mon(mon: &Pokemon, buf: &mut Vec<u8>) {
    serialize_box_mon(mon, buf);
    buf.push(mon.level);
    push_u16_be(buf, mon.max_hp);
    push_u16_be(buf, mon.attack);
    push_u16_be(buf, mon.defense);
    push_u16_be(buf, mon.speed);
    push_u16_be(buf, mon.special);
}

pub fn serialize_name(name: &[u8], buf: &mut Vec<u8>) {
    let mut padded = [0x50u8; NAME_LENGTH];
    let len = name.len().min(NAME_LENGTH - 1);
    padded[..len].copy_from_slice(&name[..len]);
    padded[len] = 0x50;
    buf.extend_from_slice(&padded);
}

/// The all-caps species name the original stores as a mon's "nickname" when no
/// real nickname was given (`display_name`'s fallback).
pub fn species_default_name(species: Species) -> String {
    pokered_data::lang_data::species_name(species, false).to_string()
}

/// Encode a decoded-text name to the charmap bytes `serialize_name` expects.
/// Unencodable text (or `None`) degrades to a blank (0x50-terminated) entry.
fn encode_name_or_blank(name: Option<&str>) -> Vec<u8> {
    name.and_then(pokered_data::charmap::encode_string)
        .unwrap_or_default()
}

/// The nickname bytes for SRAM: the real nickname if set, else the species
/// name (the original always stores a name — never a blank nickname).
fn nickname_bytes(mon: &Pokemon) -> Vec<u8> {
    if mon.has_nickname() {
        mon.nickname.to_vec()
    } else {
        encode_name_or_blank(Some(&species_default_name(mon.species)))
    }
}

pub fn serialize_party_into(party: &Party, buf: &mut Vec<u8>) {
    let count = party.count() as u8;
    buf.push(count);
    for mon in party.iter() {
        buf.push(mon.species as u8);
    }
    buf.push(0xFF);
    let species_written = count as usize + 1;
    for _ in species_written..7 {
        buf.push(0);
    }
    for mon in party.iter() {
        serialize_party_mon(mon, buf);
    }
    for _ in party.count()..6 {
        buf.extend_from_slice(&[0u8; PARTY_STRUCT_SIZE]);
    }
    // OT-name table, then nickname table (6 × NAME_LENGTH each).
    for mon in party.iter() {
        serialize_name(&mon.ot_name, buf);
    }
    for _ in party.count()..6 {
        serialize_name(&[], buf);
    }
    for mon in party.iter() {
        serialize_name(&nickname_bytes(mon), buf);
    }
    for _ in party.count()..6 {
        serialize_name(&[], buf);
    }
}

pub fn serialize_box_into(box_data: &PcBox, buf: &mut Vec<u8>) {
    let count = box_data.count() as u8;
    buf.push(count);
    for mon in box_data.iter() {
        buf.push(mon.species as u8);
    }
    buf.push(0xFF);
    let species_written = count as usize + 1;
    for _ in species_written..21 {
        buf.push(0);
    }
    for mon in box_data.iter() {
        serialize_box_mon(mon, buf);
    }
    for _ in box_data.count()..20 {
        buf.extend_from_slice(&[0u8; BOX_STRUCT_SIZE]);
    }
    for mon in box_data.iter() {
        serialize_name(&mon.ot_name, buf);
    }
    for _ in box_data.count()..20 {
        serialize_name(&[], buf);
    }
    for mon in box_data.iter() {
        serialize_name(&nickname_bytes(mon), buf);
    }
    for _ in box_data.count()..20 {
        serialize_name(&[], buf);
    }
}

pub fn serialize_sprite_data_into(buf: &mut Vec<u8>) {
    buf.extend_from_slice(&[0u8; SPRITE_DATA_SIZE]);
}

pub fn deserialize_box_mon(data: &[u8]) -> Result<Pokemon, SaveError> {
    if data.len() < BOX_STRUCT_SIZE {
        return Err(SaveError::DataTooShort);
    }
    let species = Species::from_index_id(data[0]);
    let hp = read_u16_be(data, 1);
    let box_level = data[3];
    let status = byte_to_status(data[4]);
    let type1 = PokemonType::from_id(data[5]);
    let type2 = PokemonType::from_id(data[6]);
    let moves = [
        MoveId::from_id(data[8]),
        MoveId::from_id(data[9]),
        MoveId::from_id(data[10]),
        MoveId::from_id(data[11]),
    ];
    // MON_OTID (offsets 12-13).
    let ot_id = read_u16_be(data, 12);
    let exp = ((data[14] as u32) << 16) | ((data[15] as u32) << 8) | data[16] as u32;
    let stat_exp = [
        read_u16_be(data, 17),
        read_u16_be(data, 19),
        read_u16_be(data, 21),
        read_u16_be(data, 23),
        read_u16_be(data, 25),
    ];
    let dv_bytes = [data[27], data[28]];
    // PP bytes: low 6 bits = current PP, high 2 bits = PP-Ups.
    let pp = [
        data[29] & 0x3F,
        data[30] & 0x3F,
        data[31] & 0x3F,
        data[32] & 0x3F,
    ];
    let pp_ups = [data[29] >> 6, data[30] >> 6, data[31] >> 6, data[32] >> 6];

    Ok(Pokemon {
        species,
        nickname: [0x50; 11],
        level: box_level,
        hp,
        max_hp: hp,
        attack: 0,
        defense: 0,
        speed: 0,
        special: 0,
        type1,
        type2,
        moves,
        pp,
        pp_ups,
        status,
        dv_bytes,
        stat_exp,
        total_exp: exp,
        is_traded: false,
        ot_id,
        ot_name: [0x50; 11],
    })
}

pub fn deserialize_party_mon(data: &[u8]) -> Result<Pokemon, SaveError> {
    if data.len() < PARTY_STRUCT_SIZE {
        return Err(SaveError::DataTooShort);
    }
    let mut mon = deserialize_box_mon(&data[..BOX_STRUCT_SIZE])?;
    let off = BOX_STRUCT_SIZE;
    mon.level = data[off];
    mon.max_hp = read_u16_be(data, off + 1);
    mon.attack = read_u16_be(data, off + 3);
    mon.defense = read_u16_be(data, off + 5);
    mon.speed = read_u16_be(data, off + 7);
    mon.special = read_u16_be(data, off + 9);
    Ok(mon)
}

pub fn deserialize_name(data: &[u8]) -> Vec<u8> {
    let mut name = Vec::new();
    for &b in data.iter().take(NAME_LENGTH) {
        if b == 0x50 {
            break;
        }
        name.push(b);
    }
    name
}

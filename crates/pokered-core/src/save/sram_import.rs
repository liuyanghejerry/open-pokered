// Import original Game Boy .sav files (32KB SRAM dump) into Rust SaveData.
//
// SRAM layout (4 banks × 8KB):
//   Bank 0: 3 sprite buffers + $100 padding + Hall of Fame data
//   Bank 1: $598 padding + sGameData (player name + main data + sprite data + party + current box + tile_animations) + checksum
//   Bank 2: Boxes 1-6 + checksums
//   Bank 3: Boxes 7-12 + checksums

use super::game_data::NAME_LENGTH;
use super::hall_of_fame::{HallOfFame, HofMon, HofTeam};
use super::ser_pokemon::{
    deserialize_box_mon, deserialize_name, deserialize_party_mon, BOX_STRUCT_SIZE,
    PARTY_STRUCT_SIZE, SPRITE_DATA_SIZE,
};
use super::serialization::SaveError;
use super::sram_deser::SramReader;
use super::sram_deser_game_data::deserialize_game_data;
use super::sram_layout::*;
use super::SaveData;
use crate::pokemon::party::Party;
use crate::pokemon::pc_box::{PcBox, PcStorage};
use crate::save_menu::calc_checksum;

/// Import a raw 32KB Game Boy .sav file into a `SaveData`.
///
/// Validates checksums for bank 1 (main data) and banks 2-3 (PC boxes).
/// Returns `Err(SaveError::DataTooShort)` if data is not 32KB.
/// Returns `Err(SaveError::BadChecksum)` if any checksum fails.
pub fn import_sram(data: &[u8]) -> Result<SaveData, SaveError> {
    if data.len() < SAV_FILE_SIZE {
        return Err(SaveError::DataTooShort);
    }

    let bank0 = &data[0..SRAM_BANK_SIZE_LAYOUT];
    let bank1 = &data[SRAM_BANK_SIZE_LAYOUT..SRAM_BANK_SIZE_LAYOUT * 2];
    let bank2 = &data[SRAM_BANK_SIZE_LAYOUT * 2..SRAM_BANK_SIZE_LAYOUT * 3];
    let bank3 = &data[SRAM_BANK_SIZE_LAYOUT * 3..SRAM_BANK_SIZE_LAYOUT * 4];

    validate_bank1_checksum(bank1)?;
    validate_box_bank_checksum(bank2)?;
    validate_box_bank_checksum(bank3)?;

    let hall_of_fame = parse_hall_of_fame(bank0)?;
    let (player_name, game_data, party, current_box, tile_animations) = parse_bank1(bank1)?;
    let mut pc_storage = PcStorage::new();
    parse_box_bank(bank2, &mut pc_storage, 0)?;
    parse_box_bank(bank3, &mut pc_storage, 6)?;

    // Populate script_flags from SRAM event_flags bytes so the unified
    // HashMap has all typed EventFlag entries immediately after import.
    let mut unified = crate::overworld::event_flags::EventFlags::new();
    unified.load_event_bytes(&game_data.event_flags);
    let script_flags = unified.to_hashmap();

    let mut save = SaveData {
        player_name,
        game_data,
        party,
        current_box,
        pc_storage,
        hall_of_fame,
        tile_animations,
        script_flags,
    };
    derive_traded_flags(&mut save);
    Ok(save)
}

/// Derive each stored mon's `is_traded` from the OT-ID comparison the original
/// performs on the fly (`wPartyMon1OTID` vs `wPlayerID`) — the flag drives the
/// 1.5× traded EXP bonus; obedience re-does the comparison itself. `ot_id == 0`
/// (legacy saves / unstamped mons) counts as own.
fn derive_traded_flags(save: &mut SaveData) {
    let player_id = save.game_data.player_id;
    for mon in save.party.iter_mut() {
        mon.is_traded = crate::battle::obedience::is_traded_for(mon.ot_id, player_id);
    }
    for mon in save.current_box.iter_mut() {
        mon.is_traded = crate::battle::obedience::is_traded_for(mon.ot_id, player_id);
    }
    for i in 0..12 {
        if let Ok(b) = save.pc_storage.get_box_mut(i) {
            for mon in b.iter_mut() {
                mon.is_traded = crate::battle::obedience::is_traded_for(mon.ot_id, player_id);
            }
        }
    }
}

// Checksum covers sGameData ($598) to sGameDataEnd (byte before last)
fn validate_bank1_checksum(bank1: &[u8]) -> Result<(), SaveError> {
    let checksum_offset = SRAM_BANK_SIZE_LAYOUT - 1;
    let stored_checksum = bank1[checksum_offset];
    let game_data_region = &bank1[GAME_DATA_OFFSET..checksum_offset];
    let computed = calc_checksum(game_data_region);

    if computed != stored_checksum {
        return Err(SaveError::BadChecksum);
    }
    Ok(())
}

// All-boxes checksum: first byte after 6 boxes' data, covers offset 0..boxes_end
fn validate_box_bank_checksum(bank: &[u8]) -> Result<(), SaveError> {
    let boxes_total_size = BOXES_PER_BANK * BOX_DATA_SIZE;
    if bank.len() < boxes_total_size + 1 {
        return Err(SaveError::DataTooShort);
    }

    let stored_checksum = bank[boxes_total_size];
    let computed = calc_checksum(&bank[..boxes_total_size]);

    if computed != stored_checksum {
        return Err(SaveError::BadChecksum);
    }
    Ok(())
}

fn parse_hall_of_fame(bank0: &[u8]) -> Result<HallOfFame, SaveError> {
    let mut hof = HallOfFame::new();

    if bank0.len() < HOF_OFFSET + HOF_TOTAL_SIZE {
        return Err(SaveError::DataTooShort);
    }

    let hof_data = &bank0[HOF_OFFSET..HOF_OFFSET + HOF_TOTAL_SIZE];

    for team_idx in 0..HOF_CAPACITY {
        let team_start = team_idx * HOF_TEAM_ENTRY_SIZE;
        let team_data = &hof_data[team_start..team_start + HOF_TEAM_ENTRY_SIZE];

        if team_data[0] == 0x00 {
            break;
        }

        let mut team = HofTeam::new();
        for mon_idx in 0..6 {
            let mon_start = mon_idx * HOF_MON_ENTRY_SIZE;
            let species = team_data[mon_start];
            if species == 0x00 || species == 0xFF {
                break;
            }
            let level = team_data[mon_start + 1];
            let nickname_data = &team_data[mon_start + 2..mon_start + HOF_MON_ENTRY_SIZE];
            let nickname = deserialize_name(nickname_data);
            team.add_mon(HofMon::new(species, level, nickname));
        }
        if !team.mons.is_empty() {
            hof.push_team(team);
        }
    }

    Ok(hof)
}

/// Parse bank 1: player_name + game_data + sprite_data(skip) + party + current_box + tile_animations
fn parse_bank1(
    bank1: &[u8],
) -> Result<(Vec<u8>, super::game_data::GameData, Party, PcBox, u8), SaveError> {
    let mut reader = SramReader::new(&bank1[GAME_DATA_OFFSET..]);

    let player_name = reader.read_name()?;
    let game_data = deserialize_game_data(&mut reader)?;
    reader.skip(SPRITE_DATA_SIZE)?;
    let party = parse_party(&mut reader)?;
    let current_box = parse_box(&mut reader)?;
    let tile_animations = reader.read_u8()?;

    Ok((player_name, game_data, party, current_box, tile_animations))
}

// Party: count(1) + species(7) + 6×party_struct(44) + 6×OT(11) + 6×nick(11) = 404 bytes
fn parse_party(reader: &mut SramReader) -> Result<Party, SaveError> {
    let count = reader.read_u8()? as usize;
    let count = count.min(SRAM_PARTY_LENGTH);

    let _species_data = reader.read_bytes(7)?;
    let all_structs = reader.read_bytes(6 * PARTY_STRUCT_SIZE)?;
    let all_ot_names = reader.read_bytes(6 * NAME_LENGTH)?;
    let all_nicknames = reader.read_bytes(6 * NAME_LENGTH)?;

    let mut mons = Vec::with_capacity(count);
    for i in 0..count {
        let struct_start = i * PARTY_STRUCT_SIZE;
        let struct_data = &all_structs[struct_start..struct_start + PARTY_STRUCT_SIZE];
        let mut mon = deserialize_party_mon(struct_data)?;

        let ot_start = i * NAME_LENGTH;
        let ot_name = deserialize_name(&all_ot_names[ot_start..ot_start + NAME_LENGTH]);

        let nick_start = i * NAME_LENGTH;
        let nickname = deserialize_name(&all_nicknames[nick_start..nick_start + NAME_LENGTH]);

        apply_name_tables(&mut mon, &ot_name, &nickname);
        mons.push(mon);
    }

    Ok(Party::from(mons))
}

/// Apply the OT-name / nickname table entries to a freshly deserialized mon.
/// A blank OT name stays `None`; a nickname equal to the species name (what the
/// original stores for an unnamed mon) decodes back to `None`.
fn apply_name_tables(mon: &mut crate::battle::state::Pokemon, ot_name: &[u8], nickname: &[u8]) {
    let ot = pokered_data::charmap::decode_string(ot_name);
    if !ot.is_empty() {
        mon.ot_name = Some(ot);
    }
    let nick = pokered_data::charmap::decode_string(nickname);
    if !nick.is_empty() && nick != super::ser_pokemon::species_default_name(mon.species) {
        mon.set_nickname(nick);
    }
}

// Box: count(1) + species(21) + 20×box_struct(33) + 20×OT(11) + 20×nick(11) = 1122 bytes
fn parse_box(reader: &mut SramReader) -> Result<PcBox, SaveError> {
    let count = reader.read_u8()? as usize;
    let count = count.min(MONS_PER_BOX);

    let _species_data = reader.read_bytes(21)?;
    let all_structs = reader.read_bytes(20 * BOX_STRUCT_SIZE)?;
    let all_ot_names = reader.read_bytes(20 * NAME_LENGTH)?;
    let all_nicknames = reader.read_bytes(20 * NAME_LENGTH)?;

    let mut pc_box = PcBox::new();
    for i in 0..count {
        let struct_start = i * BOX_STRUCT_SIZE;
        let struct_data = &all_structs[struct_start..struct_start + BOX_STRUCT_SIZE];
        let mut mon = deserialize_box_mon(struct_data)?;

        let ot_start = i * NAME_LENGTH;
        let ot_name = deserialize_name(&all_ot_names[ot_start..ot_start + NAME_LENGTH]);

        let nick_start = i * NAME_LENGTH;
        let nickname = deserialize_name(&all_nicknames[nick_start..nick_start + NAME_LENGTH]);

        apply_name_tables(&mut mon, &ot_name, &nickname);
        let _ = pc_box.deposit(mon);
    }

    Ok(pc_box)
}

/// Parse 6 boxes from a box bank into PcStorage at the given starting box index.
fn parse_box_bank(
    bank: &[u8],
    storage: &mut PcStorage,
    start_box_index: usize,
) -> Result<(), SaveError> {
    let mut reader = SramReader::new(bank);

    for i in 0..BOXES_PER_BANK {
        let box_index = start_box_index + i;
        let parsed_box = parse_box(&mut reader)?;

        if let Ok(target) = storage.get_box_mut(box_index) {
            *target = parsed_box;
        }
    }

    Ok(())
}

/// Import SRAM without checksum validation (for testing or corrupted saves).
/// Import SRAM without checksum validation (for testing or corrupted saves).
pub fn import_sram_no_checksum(data: &[u8]) -> Result<SaveData, SaveError> {
    if data.len() < SAV_FILE_SIZE {
        return Err(SaveError::DataTooShort);
    }

    let bank0 = &data[0..SRAM_BANK_SIZE_LAYOUT];
    let bank1 = &data[SRAM_BANK_SIZE_LAYOUT..SRAM_BANK_SIZE_LAYOUT * 2];
    let bank2 = &data[SRAM_BANK_SIZE_LAYOUT * 2..SRAM_BANK_SIZE_LAYOUT * 3];
    let bank3 = &data[SRAM_BANK_SIZE_LAYOUT * 3..SRAM_BANK_SIZE_LAYOUT * 4];

    let hall_of_fame = parse_hall_of_fame(bank0)?;
    let (player_name, game_data, party, current_box, tile_animations) = parse_bank1(bank1)?;
    let mut pc_storage = PcStorage::new();
    parse_box_bank(bank2, &mut pc_storage, 0)?;
    parse_box_bank(bank3, &mut pc_storage, 6)?;

    let mut unified = crate::overworld::event_flags::EventFlags::new();
    unified.load_event_bytes(&game_data.event_flags);
    let script_flags = unified.to_hashmap();

    let mut save = SaveData {
        player_name,
        game_data,
        party,
        current_box,
        pc_storage,
        hall_of_fame,
        tile_animations,
        script_flags,
    };
    derive_traded_flags(&mut save);
    Ok(save)
}

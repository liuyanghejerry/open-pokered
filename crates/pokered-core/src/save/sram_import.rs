// Import original Game Boy .sav files (32KB SRAM dump) into Rust SaveData.
//
// SRAM layout (4 banks × 8KB):
//   Bank 0: 3 sprite buffers + $100 padding + Hall of Fame data
//   Bank 1: $598 padding + sGameData (player name + main data + sprite data + party + current box + tile_animations) + checksum
//   Bank 2: Boxes 1-6 + checksums
//   Bank 3: Boxes 7-12 + checksums

use crate::alloc_prelude::*;
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

/// Serialized length of the canonical bank-1 checksummed region
/// (name 11 + main + sprite + party + box + tile 1). Every field is
/// fixed-width, so the value is identical for all saves — deriving it from
/// a blank `GameData` avoids constructing a full `SaveData`, which costs
/// 29 KB of stack and overflowed the GBA's 64 KiB EWRAM stack during boot
/// media probing.
pub fn canonical_region_len() -> usize {
    use super::sram_layout::{BOX_DATA_SIZE, PARTY_DATA_SIZE, SPRITE_DATA_REGION_SIZE};
    let mut probe = Vec::new();
    super::ser_game_data::serialize_game_data_into(&crate::save::game_data::GameData::new(), &mut probe);
    11 + probe.len() + SPRITE_DATA_REGION_SIZE + PARTY_DATA_SIZE + BOX_DATA_SIZE + 1
}

/// Import a raw 32KB Game Boy .sav file into a `SaveData`.
///
/// Validates checksums for bank 1 (main data) and banks 2-3 (PC boxes).
/// Returns `Err(SaveError::DataTooShort)` if data is not 32KB.
/// Returns `Err(SaveError::BadChecksum)` if any checksum fails.
// Not inlined: the 29 KB `SaveData` return slot must live in the caller's
// frame. With fat LTO the whole chain inlined into `game_main`, whose frame
// then exceeded the GBA's 64 KiB EWRAM stack and corrupted it at entry.
#[inline(never)]
pub fn import_sram(data: &[u8]) -> Result<SaveData, SaveError> {
    let mut save = SaveData::new();
    import_sram_into(data, &mut save)?;
    Ok(save)
}

/// Import into a caller-owned `SaveData`. The GBA boot path passes the game's
/// resident save slot directly: the 29 KB return slot of [`import_sram`] no
/// longer exists as a stack temporary, which the 64 KiB EWRAM stack cannot
/// absorb on top of the parse frames.
#[inline(never)]
pub fn import_sram_into(data: &[u8], out: &mut SaveData) -> Result<(), SaveError> {
    if data.len() < SAV_FILE_SIZE {
        return Err(SaveError::DataTooShort);
    }

    let bank0 = &data[0..SRAM_BANK_SIZE_LAYOUT];
    let bank1 = &data[SRAM_BANK_SIZE_LAYOUT..SRAM_BANK_SIZE_LAYOUT * 2];
    let bank2 = &data[SRAM_BANK_SIZE_LAYOUT * 2..SRAM_BANK_SIZE_LAYOUT * 3];
    let bank3 = &data[SRAM_BANK_SIZE_LAYOUT * 3..SRAM_BANK_SIZE_LAYOUT * 4];

    let valid_box_banks = validate_box_bank_checksum(bank2).is_ok()
        && validate_box_bank_checksum(bank3).is_ok();
    let box_initialization_bit = bank1[0x084c] & 0x80 != 0;
    let legacy_native = uses_legacy_native_layout(bank1);
    // The ROM only initializes box banks on the first CHANGE BOX. A valid
    // initial save can contain arbitrary power-on SRAM in banks 2 and 3.
    if !valid_box_banks && (box_initialization_bit || legacy_native) {
        return Err(SaveError::BadChecksum);
    }
    import_bank1_into(bank1, out)?;
    // A new ROM playthrough can retain checksum-valid boxes from a previous
    // trainer. Bit 7, not checksum validity, decides whether those banks exist.
    let boxes_initialized=valid_box_banks && (box_initialization_bit || legacy_native);
    // Every box slot is fully overwritten by parse_box_bank, so the resident
    // storage needs no reset (its 18 KB reset temporary would defeat the
    // point of importing in place).
    if boxes_initialized {
        parse_box_bank(bank2, &mut out.pc_storage, 0, legacy_native)?;
        parse_box_bank(bank3, &mut out.pc_storage, 6, legacy_native)?;
    } else { out.pc_storage.clear(); }
    out.hall_of_fame = parse_hall_of_fame(bank0, legacy_native)?;
    finish_import(out);
    Ok(())
}

/// Load stable SRAM bank by bank, using one 8 KiB buffer instead of a full
/// 32 KiB image. The reader must fill the requested bank completely and
/// keep the medium unchanged until this function returns.
#[inline(never)]
pub fn import_sram_banks_into(
    mut read_bank: impl FnMut(usize, &mut [u8]),
    out: &mut SaveData,
) -> Result<(), SaveError> {
    let mut bank = vec![0u8; SRAM_BANK_SIZE_LAYOUT];
    // Check both box banks before changing the resident save, matching the
    // whole-image import's behavior on corrupt media.
    let mut valid_box_banks = true;
    for index in [2, 3] {
        read_bank(index, &mut bank);
        valid_box_banks &= validate_box_bank_checksum(&bank).is_ok();
    }
    read_bank(1, &mut bank);
    let box_initialization_bit=bank[0x084c]&0x80!=0;
    let legacy_native = uses_legacy_native_layout(&bank);
    if !valid_box_banks && (box_initialization_bit || legacy_native) {
        return Err(SaveError::BadChecksum);
    }
    import_bank1_into(&bank, out)?;
    let boxes_initialized=valid_box_banks && (box_initialization_bit || legacy_native);
    if boxes_initialized {
        for index in [2, 3] {
            read_bank(index, &mut bank);
            parse_box_bank(&bank, &mut out.pc_storage, (index - 2) * BOXES_PER_BANK, legacy_native)?;
        }
    } else { out.pc_storage.clear(); }
    read_bank(0, &mut bank);
    out.hall_of_fame = parse_hall_of_fame(&bank, legacy_native)?;
    finish_import(out);
    Ok(())
}

fn uses_legacy_native_layout(bank: &[u8]) -> bool {
    validate_canonical_bank1(bank).is_err()
        || (recent_native_layout(bank) && !party_layout_matches(bank, 0x0f2c, false))
}

#[inline(never)]
fn import_bank1_into(bank: &[u8], out: &mut SaveData) -> Result<bool, SaveError> {
    // A previous native checksum lies *inside* the longer ROM region. With
    // zero tail padding it also makes the ROM checksum zero, so checksum alone
    // cannot identify that layout. Validate the party header/struct positions.
    let legacy_native = uses_legacy_native_layout(bank);
    let mut bank1_owned;
    let bank1: &[u8] = {
        let b = bank;
        if legacy_native {
            // Legacy (pre-2026-08) Rust format: transform to canonical.
            bank1_owned = migrate_legacy_bank1(b).ok_or(SaveError::BadChecksum)?;
            // Fix the transformed bank's checksum at the canonical spot so
            // the standard validation below passes.
            let canonical_len = canonical_region_len();
            let region = &bank1_owned[GAME_DATA_OFFSET..GAME_DATA_OFFSET + canonical_len];
            bank1_owned[GAME_DATA_OFFSET + canonical_len] = crate::save_menu::calc_checksum(region);
            &bank1_owned
        } else {
            b
        }
    };
    validate_bank1_checksum(bank1)?;
    let (player_name, game_data, party, current_box, tile_animations) = parse_bank1(bank1)?;
    out.player_name = player_name;
    out.game_data = game_data;
    out.party = party;
    out.current_box = current_box;
    out.tile_animations = tile_animations;
    out.imported_legacy_native = legacy_native;
    out.imported_legacy_json = false;
    Ok(legacy_native)
}

fn finish_import(out: &mut SaveData) {
    // wCurrentBoxNum (save.asm:382-384: menu index | $80; GetBoxSRAMLocation
    // masks with BOX_NUM_MASK) — restore the trainer's last-open box so a
    // save→load round-trip keeps Bill's PC where it was left.
    let saved_box = (out.game_data.current_box_num & 0x7F) as usize;
    if saved_box < 12 {
        let _ = out.pc_storage.change_box(saved_box);
    }
    // Bank 1 is the live current box. The inactive-bank copy can be older
    // until ChangeBox; restore it before Bill's PC reads storage.
    out.sync_current_box_to_storage();
    derive_traded_flags(out);
}

/// Derive each stored mon's `is_traded` from the OT-ID comparison the original
/// performs on the fly (`wPartyMon1OTID` vs `wPlayerID`) — the flag drives the
/// 1.5× traded EXP bonus; obedience re-does the comparison itself. `ot_id == 0`
/// (legacy saves / unstamped mons) counts as own.
pub fn derive_traded_flags(save: &mut SaveData) {
    let player_id = save.game_data.player_id;
    for mon in save.party.iter_mut() {
        mon.is_traded = crate::battle::obedience::is_traded_for_with_name(mon.ot_id, player_id, &mon.ot_name);
    }
    for mon in save.current_box.iter_mut() {
        mon.is_traded = crate::battle::obedience::is_traded_for_with_name(mon.ot_id, player_id, &mon.ot_name);
    }
    for i in 0..12 {
        if let Ok(b) = save.pc_storage.get_box_mut(i) {
            for mon in b.iter_mut() {
                mon.is_traded = crate::battle::obedience::is_traded_for_with_name(mon.ot_id, player_id, &mon.ot_name);
            }
        }
    }
}

// Checksum covers sGameData from $598 to sGameDataEnd; sMainDataCheckSum
// sits DIRECTLY after the region (ram/sram.asm "Save Data" — no trailing
// alignment). The region length is re-derived from the serializer so the
// two can never drift.
fn validate_bank1_checksum(bank1: &[u8]) -> Result<(), SaveError> {
    validate_canonical_bank1(bank1).or_else(|_| {
        // Legacy fallback: pre-2026-08 Rust saves stored the checksum at the
        // bank's last byte over a 455-byte-shorter layout. Recognizing them
        // here lets callers route through the migration before parsing.
        let legacy_offset = SRAM_BANK_SIZE_LAYOUT - 1;
        let legacy = &bank1[GAME_DATA_OFFSET..legacy_offset];
        if calc_checksum(legacy) == bank1[legacy_offset] {
            Ok(())
        } else {
            Err(SaveError::BadChecksum)
        }
    })
}

fn validate_canonical_bank1(bank1: &[u8]) -> Result<(), SaveError> {
    // The canonical region = name(11) + main + sprite + party + box + tile(1).
    let canonical_len = canonical_region_len();
    let checksum_offset = GAME_DATA_OFFSET + canonical_len;
    if bank1.len() <= checksum_offset {
        return Err(SaveError::DataTooShort);
    }
    let region = &bank1[GAME_DATA_OFFSET..checksum_offset];
    if calc_checksum(region) != bank1[checksum_offset] {
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

fn parse_hall_of_fame(bank0: &[u8], legacy_native: bool) -> Result<HallOfFame, SaveError> {
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
            team.add_mon(HofMon::new(if legacy_native { species } else { pokered_data::species::Species::from_rom_id(species) as u8 }, level, &nickname));
        }
        if team.count() > 0 {
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
    let current_box = parse_box(&mut reader, false)?;
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
/// A blank OT name stays unset; a nickname equal to the species name (what the
/// original stores for an unnamed mon) decodes back to unset. The raw SRAM
/// charmap bytes are kept as-is (never re-encoded — decode→encode is not
/// identity for e.g. the 0x70 quote glyphs).
fn apply_name_tables(mon: &mut crate::battle::state::Pokemon, ot_name: &[u8], nickname: &[u8]) {
    let ot = pokered_data::charmap::decode_string(ot_name);
    if !ot.is_empty() {
        mon.ot_name = to_name_bytes(ot_name);
    }
    let nick = pokered_data::charmap::decode_string(nickname);
    if !nick.is_empty() && nick != super::ser_pokemon::species_default_name(mon.species) {
        mon.nickname = to_name_bytes(nickname);
    }
}

/// Copy SRAM name-table bytes into the fixed in-memory form, padding with
/// 0x50 terminators.
fn to_name_bytes(name: &[u8]) -> [u8; 11] {
    let mut out = [0x50u8; 11];
    let len = name.len().min(out.len() - 1);
    out[..len].copy_from_slice(&name[..len]);
    out
}

// Box: count(1) + species(21) + 20×box_struct(33) + 20×OT(11) + 20×nick(11) = 1122 bytes
fn parse_box(reader: &mut SramReader, legacy_native: bool) -> Result<PcBox, SaveError> {
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
        let mut canonical_struct = [0u8; BOX_STRUCT_SIZE];
        canonical_struct.copy_from_slice(struct_data);
        if legacy_native {
            canonical_struct[0] = pokered_data::species::Species::from_index_id(canonical_struct[0]).to_rom_id();
        }
        let mut mon = deserialize_box_mon(&canonical_struct)?;

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
    legacy_native: bool,
) -> Result<(), SaveError> {
    let mut reader = SramReader::new(bank);

    for i in 0..BOXES_PER_BANK {
        let box_index = start_box_index + i;
        let parsed_box = parse_box(&mut reader, legacy_native)?;

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

    let hall_of_fame = parse_hall_of_fame(bank0, false)?;
    let (player_name, game_data, party, current_box, tile_animations) = parse_bank1(bank1)?;
    let mut pc_storage = PcStorage::new();
    parse_box_bank(bank2, &mut pc_storage, 0, false)?;
    parse_box_bank(bank3, &mut pc_storage, 6, false)?;

    let mut save = SaveData {
        imported_legacy_native: false,
        imported_legacy_json: false,
        player_name,
        game_data,
        party,
        current_box,
        pc_storage,
        hall_of_fame,
        tile_animations,
    };
    finish_import(&mut save);
    Ok(save)
}

fn party_layout_matches(bank: &[u8], start: usize, native_dex: bool) -> bool {
    let count = bank[start] as usize;
    if count > 6 || bank[start + 1 + count] != 0xFF { return false; }
    for i in 0..count {
        let at = start + 8 + i * PARTY_STRUCT_SIZE;
        let species = bank[at];
        if species == 0 || bank[start + 1 + i] != species { return false; }
        if native_dex && species > 151 { return false; }
        if !native_dex && pokered_data::species::Species::from_rom_id(species) == pokered_data::species::Species::None { return false; }
        if !(1..=100).contains(&bank[at + 33]) { return false; }
        if native_dex && bank[at + 3] != bank[at + 33] { return false; }
    }
    true
}

fn recent_native_layout(bank: &[u8]) -> bool {
    let old_end = GAME_DATA_OFFSET + canonical_region_len() - 88;
    calc_checksum(&bank[GAME_DATA_OFFSET..old_end]) == bank[old_end]
        && party_layout_matches(bank, 0x0f2c - 88, true)
}

// Legacy native saves used dex-order species bytes. Two layouts exist:
// the most recent omitted ten Day Care stat-exp bytes and 80 progress padding
// bytes; older saves kept Day Care complete but omitted 375 UNION bytes and
// the 80 progress bytes, with their checksum at the bank end.
// Validate their own checksum before migrating; never repair corrupt ROM data.
fn migrate_legacy_bank1(bank1: &[u8]) -> Option<Vec<u8>> {
    use super::ser_game_data::serialize_game_data_into;
    use pokered_data::species::Species;
    let canonical_len = canonical_region_len();
    let old_len = canonical_len.checked_sub(88)?;
    let old_ck = GAME_DATA_OFFSET + old_len;
    let recent = calc_checksum(&bank1[GAME_DATA_OFFSET..old_ck]) == bank1[old_ck];
    let end_ck = SRAM_BANK_SIZE_LAYOUT - 1;
    if !recent && calc_checksum(&bank1[GAME_DATA_OFFSET..end_ck]) != bank1[end_ck] {
        return None;
    }
    let blank = super::game_data::GameData::new();
    let mut main = Vec::new();
    serialize_game_data_into(&blank, &mut main);
    let main_len = main.len();
    let base = GAME_DATA_OFFSET;
    let mut out = bank1.to_vec();
    let mut marker = blank.clone();
    marker.game_progress_flags.fill(0xA5);
    let mut marked = Vec::new();
    serialize_game_data_into(&marker, &mut marked);
    let progress_end = marked.windows(marker.game_progress_flags.len())
        .position(|w| w.iter().all(|&b| b == 0xA5))? + marker.game_progress_flags.len();
    out.splice(base + NAME_LENGTH + progress_end..base + NAME_LENGTH + progress_end,
        core::iter::repeat(0).take(80));
    if recent {
        // The previous writer padded the 50-byte wild-data branch as though
        // it were 48 bytes, so remove its two surplus UNION bytes.
        let union_end = base + NAME_LENGTH + blank_offset_after_water(&main) + 375;
        out.drain(union_end..union_end + 2);
        // Insert at MON_HP_EXP, before DVs/PP, not after them.
        let at = base + NAME_LENGTH + main_len - 33 + 17;
        out.splice(at..at, core::iter::repeat(0).take(10));
    } else {
        let at = base + NAME_LENGTH + blank_offset_after_water(&main);
        out.splice(at..at, core::iter::repeat(0).take(375));
    }
    out.truncate(SRAM_BANK_SIZE_LAYOUT);
    let main_base = base + NAME_LENGTH;
    // The starter/fossil values are species bytes in SRAM too.
    for field in 0..3 {
        let mut marked = blank.clone();
        match field { 0 => marked.fossil_mon = 1, 1 => marked.rival_starter = 1, _ => marked.player_starter = 1 }
        let mut probe = Vec::new();
        serialize_game_data_into(&marked, &mut probe);
        let offset = probe.iter().zip(&main).position(|(a,b)| a != b)?;
        let at = main_base + offset;
        out[at] = Species::from_index_id(out[at]).to_rom_id();
    }
    let mut trade_marker = blank.clone();
    trade_marker.completed_in_game_trade_flags = 1;
    let mut trade_probe = Vec::new();
    serialize_game_data_into(&trade_marker, &mut trade_probe);
    let trade_offset = trade_probe.iter().zip(&main).position(|(a,b)| a != b)?;
    out.swap(main_base + trade_offset, main_base + trade_offset + 1);
    let dc = main_base + main_len - 33;
    out[dc] = Species::from_index_id(out[dc]).to_rom_id();
    let party = main_base + main_len + SPRITE_DATA_SIZE;
    for i in 0..usize::from(out[party]).min(6) {
        let at = party + 8 + i * PARTY_STRUCT_SIZE;
        out[at] = Species::from_index_id(out[at]).to_rom_id();
        out[party + 1 + i] = Species::from_index_id(out[party + 1 + i]).to_rom_id();
    }
    let bx = party + PARTY_DATA_SIZE;
    for i in 0..usize::from(out[bx]).min(20) {
        let at = bx + 22 + i * BOX_STRUCT_SIZE;
        out[at] = Species::from_index_id(out[at]).to_rom_id();
        out[bx + 1 + i] = Species::from_index_id(out[bx + 1 + i]).to_rom_id();
    }
    Some(out)
}

/// Byte offset (inside serialized MAIN data) just past the water-encounter
/// table — located by pattern: the blank probe's grass/water rates are 0 and
/// the tables are zero, so instead count from the event-flags length.
fn blank_offset_after_water(probe: &[u8]) -> usize {
    use super::game_data::{NUM_EVENTS_BYTES, WILDDATA_LENGTH};
    // event_flags end position: everything before them is version-stable.
    // Find it by the probe: serialize GameData with a sentinel event flag.
    let mut marked = super::game_data::GameData::default();
    marked.event_flags = vec![0xA5; NUM_EVENTS_BYTES];
    let mut buf = Vec::new();
    super::ser_game_data::serialize_game_data_into(&marked, &mut buf);
    let flags_at = buf
        .windows(NUM_EVENTS_BYTES)
        .position(|w| w.iter().all(|&b| b == 0xA5))
        .expect("sentinel event flags must serialize contiguously");
    flags_at + NUM_EVENTS_BYTES + (WILDDATA_LENGTH + 8) + WILDDATA_LENGTH
}

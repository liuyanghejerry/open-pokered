use crate::save::ser_pokemon::*;
use crate::save::serialization::SaveError;
use crate::save::sram_import::{import_sram, import_sram_no_checksum};
use crate::save::sram_layout::*;
use crate::save_menu::calc_checksum;

fn make_empty_sram() -> Vec<u8> {
    vec![0u8; SAV_FILE_SIZE]
}

fn write_bank1_checksum(sram: &mut [u8]) {
    let bank1_start = SRAM_BANK_SIZE_LAYOUT;
    let checksum_offset = bank1_start + SRAM_BANK_SIZE_LAYOUT - 1;
    let game_data_region = &sram[bank1_start + GAME_DATA_OFFSET..checksum_offset];
    let cksum = calc_checksum(game_data_region);
    sram[checksum_offset] = cksum;
}

fn write_box_bank_checksum(sram: &mut [u8], bank_index: usize) {
    let bank_start = bank_index * SRAM_BANK_SIZE_LAYOUT;
    let boxes_total_size = BOXES_PER_BANK * BOX_DATA_SIZE;
    let box_region = &sram[bank_start..bank_start + boxes_total_size];
    let cksum = calc_checksum(box_region);
    sram[bank_start + boxes_total_size] = cksum;
}

fn make_valid_sram() -> Vec<u8> {
    let mut sram = make_empty_sram();
    write_bank1_checksum(&mut sram);
    write_box_bank_checksum(&mut sram, 2);
    write_box_bank_checksum(&mut sram, 3);
    sram
}

fn bank1_game_data_offset() -> usize {
    SRAM_BANK_SIZE_LAYOUT + GAME_DATA_OFFSET
}

#[test]
fn test_import_too_short() {
    let data = vec![0u8; SAV_FILE_SIZE - 1];
    assert!(matches!(import_sram(&data), Err(SaveError::DataTooShort)));
}

#[test]
fn test_import_empty_sram_valid_checksums() {
    let sram = make_valid_sram();
    let result = import_sram(&sram);
    assert!(result.is_ok());
    let save = result.unwrap();
    // All-zeros SRAM: name bytes are 0x00 (no 0x50 terminator), so all 11 bytes are read
    assert_eq!(save.player_name, vec![0u8; 11]);
    assert_eq!(save.party.count(), 0);
    assert_eq!(save.game_data.player_money, 0);
    assert_eq!(save.hall_of_fame.team_count(), 0);
    assert_eq!(save.tile_animations, 0);
}

#[test]
fn test_import_bad_bank1_checksum() {
    let mut sram = make_valid_sram();
    let checksum_offset = SRAM_BANK_SIZE_LAYOUT + SRAM_BANK_SIZE_LAYOUT - 1;
    sram[checksum_offset] ^= 0xFF;
    assert!(matches!(import_sram(&sram), Err(SaveError::BadChecksum)));
}

#[test]
fn test_import_bad_bank2_checksum() {
    let mut sram = make_valid_sram();
    let bank2_start = SRAM_BANK_SIZE_LAYOUT * 2;
    let boxes_total_size = BOXES_PER_BANK * BOX_DATA_SIZE;
    sram[bank2_start + boxes_total_size] ^= 0xFF;
    assert!(matches!(import_sram(&sram), Err(SaveError::BadChecksum)));
}

#[test]
fn test_import_bad_bank3_checksum() {
    let mut sram = make_valid_sram();
    let bank3_start = SRAM_BANK_SIZE_LAYOUT * 3;
    let boxes_total_size = BOXES_PER_BANK * BOX_DATA_SIZE;
    sram[bank3_start + boxes_total_size] ^= 0xFF;
    assert!(matches!(import_sram(&sram), Err(SaveError::BadChecksum)));
}

#[test]
fn test_import_no_checksum_ignores_bad_checksum() {
    let sram = make_empty_sram();
    let result = import_sram_no_checksum(&sram);
    assert!(result.is_ok());
}

#[test]
fn test_import_player_name() {
    let mut sram = make_empty_sram();
    let name_offset = bank1_game_data_offset();
    sram[name_offset] = 0x80;
    sram[name_offset + 1] = 0x81;
    sram[name_offset + 2] = 0x82;
    sram[name_offset + 3] = 0x50;

    write_bank1_checksum(&mut sram);
    write_box_bank_checksum(&mut sram, 2);
    write_box_bank_checksum(&mut sram, 3);

    let save = import_sram(&sram).unwrap();
    assert_eq!(save.player_name, vec![0x80, 0x81, 0x82]);
}

#[test]
fn test_import_player_id() {
    let mut sram = make_empty_sram();
    let name_offset = bank1_game_data_offset();

    // Player name: 11 bytes (NAME_LENGTH)
    // Then game data starts. We need to find player_id offset.
    // From sram_deser_game_data: pokedex_owned(19) + pokedex_seen(19) + bag(1+20*2+1=42)
    // + money(3) + rival_name(11) + options(1) + badges(1) + unused_badges(1)
    // + letter_delay(1) + player_id(2 bytes, big-endian)
    // = 11 + 19 + 19 + 42 + 3 + 11 + 1 + 1 + 1 + 1 = 109 offset from game_data_start
    let player_id_offset = name_offset + 11 + 19 + 19 + 42 + 3 + 11 + 1 + 1 + 1 + 1;
    sram[player_id_offset] = 0x12;
    sram[player_id_offset + 1] = 0x34;

    write_bank1_checksum(&mut sram);
    write_box_bank_checksum(&mut sram, 2);
    write_box_bank_checksum(&mut sram, 3);

    let save = import_sram(&sram).unwrap();
    assert_eq!(save.game_data.player_id, 0x1234);
}

#[test]
fn test_import_hof_empty() {
    let sram = make_valid_sram();
    let save = import_sram(&sram).unwrap();
    assert_eq!(save.hall_of_fame.team_count(), 0);
}

#[test]
fn test_import_hof_one_team() {
    let mut sram = make_valid_sram();

    let hof_start = HOF_OFFSET;
    // Team 0, Mon 0: species=0x99 (Pikachu index), level=25, nickname "PIKA" + 0x50
    sram[hof_start] = 0x54; // species (Pikachu internal index)
    sram[hof_start + 1] = 25; // level
    sram[hof_start + 2] = 0x8F; // 'P'
    sram[hof_start + 3] = 0x88; // 'I'
    sram[hof_start + 4] = 0x8A; // 'K'
    sram[hof_start + 5] = 0x80; // 'A'
    sram[hof_start + 6] = 0x50; // terminator

    write_bank1_checksum(&mut sram);
    write_box_bank_checksum(&mut sram, 2);
    write_box_bank_checksum(&mut sram, 3);

    let save = import_sram(&sram).unwrap();
    assert_eq!(save.hall_of_fame.team_count(), 1);
    let team = save.hall_of_fame.get_team(0).unwrap();
    assert_eq!(team.mons.len(), 1);
    assert_eq!(team.mons[0].species, 0x54);
    assert_eq!(team.mons[0].level, 25);
    assert_eq!(team.mons[0].nickname, vec![0x8F, 0x88, 0x8A, 0x80]);
}

#[test]
fn test_import_pc_storage_empty() {
    let sram = make_valid_sram();
    let save = import_sram(&sram).unwrap();
    assert_eq!(save.pc_storage.total_stored(), 0);
    for i in 0..12 {
        assert_eq!(save.pc_storage.get_box(i).unwrap().count(), 0);
    }
}

#[test]
fn test_import_checksum_algorithm_matches() {
    let data = vec![0x01, 0x02, 0x03, 0x04];
    let cksum = calc_checksum(&data);
    let sum: u8 = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    assert_eq!(cksum, !sum);
}

#[test]
fn test_import_sav_file_size() {
    assert_eq!(SAV_FILE_SIZE, 32768);
    assert_eq!(SRAM_BANK_SIZE_LAYOUT, 0x2000);
}

#[test]
fn test_import_box_data_size() {
    let expected = 1 + 21 + 20 * BOX_STRUCT_SIZE + 20 * 11 + 20 * 11;
    assert_eq!(BOX_DATA_SIZE, expected);
}

#[test]
fn test_import_party_data_size() {
    let expected = 1 + 7 + 6 * PARTY_STRUCT_SIZE + 6 * 11 + 6 * 11;
    assert_eq!(PARTY_DATA_SIZE, expected);
}

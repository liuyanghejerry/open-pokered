use super::hall_of_fame::{HallOfFame, HOF_MON_SIZE, HOF_TEAM_SIZE};
use super::ser_pokemon::serialize_box_into;
use super::sram_layout::*;
use super::SaveData;
use crate::save_menu::calc_checksum;

pub fn export_sram(save: &SaveData) -> Vec<u8> {
    let mut sram = vec![0u8; SAV_FILE_SIZE];

    write_bank0(&save.hall_of_fame, &mut sram[0..SRAM_BANK_SIZE_LAYOUT]);
    write_bank1(
        save,
        &mut sram[SRAM_BANK_SIZE_LAYOUT..SRAM_BANK_SIZE_LAYOUT * 2],
    );
    write_box_bank(
        save,
        0,
        &mut sram[SRAM_BANK_SIZE_LAYOUT * 2..SRAM_BANK_SIZE_LAYOUT * 3],
    );
    write_box_bank(
        save,
        6,
        &mut sram[SRAM_BANK_SIZE_LAYOUT * 3..SRAM_BANK_SIZE_LAYOUT * 4],
    );

    sram
}

fn write_bank0(hof: &HallOfFame, bank: &mut [u8]) {
    let mut offset = HOF_OFFSET;
    for team in hof.iter() {
        for (mon_idx, mon) in team.mons.iter().enumerate() {
            let base = offset + mon_idx * HOF_MON_SIZE;
            bank[base] = mon.species;
            bank[base + 1] = mon.level;
            let name_slot = &mut bank[base + 2..base + HOF_MON_SIZE];
            name_slot.fill(0x50);
            let len = mon.nickname.len().min(HOF_MON_SIZE - 3);
            name_slot[..len].copy_from_slice(&mon.nickname[..len]);
            name_slot[len] = 0x50;
        }
        offset += HOF_TEAM_SIZE;
    }
}

fn write_bank1(save: &SaveData, bank: &mut [u8]) {
    let checksummed = save.serialize_checksummed_region();
    let dest = &mut bank[GAME_DATA_OFFSET..GAME_DATA_OFFSET + checksummed.len()];
    dest.copy_from_slice(&checksummed);

    let checksum = calc_checksum(&checksummed);
    bank[SRAM_BANK_SIZE_LAYOUT - 1] = checksum;
}

fn write_box_bank(save: &SaveData, start_box_index: usize, bank: &mut [u8]) {
    let mut buf = Vec::new();
    for i in 0..BOXES_PER_BANK {
        let box_idx = start_box_index + i;
        if let Ok(pc_box) = save.pc_storage.get_box(box_idx) {
            serialize_box_into(pc_box, &mut buf);
        } else {
            serialize_box_into(&crate::pokemon::pc_box::PcBox::new(), &mut buf);
        }
    }

    let boxes_total = BOXES_PER_BANK * BOX_DATA_SIZE;
    let copy_len = buf.len().min(boxes_total);
    bank[..copy_len].copy_from_slice(&buf[..copy_len]);

    let all_boxes_checksum = calc_checksum(&bank[..boxes_total]);
    bank[boxes_total] = all_boxes_checksum;

    for i in 0..BOXES_PER_BANK {
        let box_start = i * BOX_DATA_SIZE;
        let box_end = box_start + BOX_DATA_SIZE;
        let individual_checksum = calc_checksum(&bank[box_start..box_end]);
        bank[boxes_total + 1 + i] = individual_checksum;
    }
}

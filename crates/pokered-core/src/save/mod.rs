pub mod game_data;
pub mod hall_of_fame;
pub mod ser_game_data;
pub mod ser_pokemon;
pub mod serialization;
pub mod sram_deser;
pub mod sram_deser_game_data;
pub mod sram_export;
pub mod sram_import;
pub mod sram_layout;

#[cfg(test)]
mod save_tests;
#[cfg(test)]
mod sram_import_tests;
#[cfg(test)]
mod daycare_tests;

use crate::pokemon::party::Party;
use crate::pokemon::pc_box::{PcBox, PcStorage};
use game_data::{DayCareMon, GameData};
use hall_of_fame::HallOfFame;
use serde::{Deserialize, Serialize};

use crate::save_menu::calc_checksum;

pub use serialization::{SaveError, SRAM_BANK_SIZE};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveData {
    pub player_name: Vec<u8>,
    pub game_data: GameData,
    pub party: Party,
    pub current_box: PcBox,
    pub pc_storage: PcStorage,
    pub hall_of_fame: HallOfFame,
    pub tile_animations: u8,
}

impl SaveData {
    pub fn new() -> Self {
        Self {
            player_name: Vec::new(),
            game_data: GameData::new(),
            party: Party::new(),
            current_box: PcBox::new(),
            pc_storage: PcStorage::new(),
            hall_of_fame: HallOfFame::new(),
            tile_animations: 0,
        }
    }

    /// Deposit the party member at `index` (0-based) into the Day Care. Removes
    /// it from the party and stores it off-party in `game_data.daycare`, where
    /// it gains experience while the player walks. No-op if `index` is out of
    /// range, the mon knows an HM move, or it is the player's last Pokémon
    /// (the original refuses all three). Mirrors `MoveMon PARTY_TO_DAYCARE`.
    pub fn deposit_daycare(&mut self, index: u8) {
        use crate::pokemon::move_learning::is_hm_move;
        use pokered_data::pokemon_data::get_base_stats;
        let idx = index as usize;
        let ok = self
            .party
            .get(idx)
            .map(|m| !m.moves.iter().any(|mv| is_hm_move(*mv)))
            .unwrap_or(false)
            && self.party.count() > 1;
        if !ok {
            return;
        }
        let Ok(mon) = self.party.remove(idx) else {
            return;
        };
        let catch_rate = get_base_stats(mon.species)
            .map(|b| b.catch_rate)
            .unwrap_or(0);
        let player_id = self.game_data.player_id;
        let mut name_buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
        let name = mon.display_name(&mut name_buf);
        let dc = &mut self.game_data.daycare;
        dc.in_use = true;
        dc.species = mon.species as u8;
        dc.hp = mon.hp;
        dc.box_level = mon.level;
        dc.status = 0;
        // Types/catch-rate are re-derived from the species on withdrawal, so
        // only the growth-relevant fields need to round-trip exactly.
        dc.type1 = 0;
        dc.type2 = 0;
        dc.catch_rate = catch_rate;
        dc.moves = [
            mon.moves[0] as u8,
            mon.moves[1] as u8,
            mon.moves[2] as u8,
            mon.moves[3] as u8,
        ];
        dc.ot_id = player_id;
        dc.exp = mon.total_exp;
        dc.hp_exp = mon.stat_exp[0];
        dc.attack_exp = mon.stat_exp[1];
        dc.defense_exp = mon.stat_exp[2];
        dc.speed_exp = mon.stat_exp[3];
        dc.special_exp = mon.stat_exp[4];
        dc.dvs = u16::from_be_bytes(mon.dv_bytes);
        dc.pp = mon.pp;
        self.game_data.daycare_mon_name =
            pokered_data::charmap::encode_string(&name).unwrap_or_default();
    }

    /// Withdraw the Day Care Pokémon back into the party at its grown level:
    /// level/stats are recomputed from the accumulated experience, HP is
    /// restored to max, and any level-up moves learned since the deposit level
    /// are taught (this preserves TM/HM-taught moves, unlike the original
    /// `WriteMonMoves` rebuild). STAT EXP IS LOST — the original's
    /// `wDayCareMon` is a 33-byte box_struct with no stat-exp fields
    /// (ram/wram.asm:2221), so a deposit/withdraw round-trip resets effort to
    /// zero (the famous Day-Care EV wipe). No-op if nothing is deposited or
    /// the party is full.
    pub fn withdraw_daycare(&mut self) {
        use crate::battle::experience::growth::level_from_exp;
        use crate::pokemon::move_learning::process_level_up_moves;
        use crate::pokemon::stats::{create_pokemon, recalculate_stats};
        use pokered_data::moves::MoveId;
        use pokered_data::pokemon_data::get_base_stats;
        use pokered_data::species::Species;
        if !self.game_data.daycare.in_use || self.party.is_full() {
            return;
        }
        let dc = self.game_data.daycare; // DayCareMon: Copy
        let species = Species::from_index_id(dc.species);
        if let Some(base) = get_base_stats(species) {
            let new_level = level_from_exp(base.growth_rate, dc.exp).clamp(1, 100);
            if let Some(mut mon) = create_pokemon(species, new_level, dc.dvs.to_be_bytes()) {
                mon.total_exp = dc.exp;
            let box_moves = [
                MoveId::from_id(dc.moves[0]),
                MoveId::from_id(dc.moves[1]),
                MoveId::from_id(dc.moves[2]),
                MoveId::from_id(dc.moves[3]),
            ];
            if box_moves.iter().any(|m| *m != MoveId::None) {
                mon.moves = box_moves;
                mon.pp = dc.pp;
            }
            process_level_up_moves(&mut mon, dc.box_level, new_level);
                recalculate_stats(&mut mon);
                mon.hp = mon.max_hp;
                let name = pokered_data::charmap::decode_string(&self.game_data.daycare_mon_name);
                if !name.is_empty() && name != crate::save::ser_pokemon::species_default_name(species) {
                    mon.set_nickname(&name);
                }
                let _ = self.party.add(mon);
            }
        }
        self.game_data.daycare = DayCareMon::default();
        self.game_data.daycare_mon_name.clear();
        self.game_data.daycare_mon_ot.clear();
    }

    pub fn player_id(&self) -> u16 {
        self.game_data.player_id
    }

    pub fn validate_checksum(&self, stored_checksum: u8) -> bool {
        let data = self.serialize_checksummed_region();
        calc_checksum(&data) == stored_checksum
    }

    pub fn compute_checksum(&self) -> u8 {
        let data = self.serialize_checksummed_region();
        calc_checksum(&data)
    }

    pub fn serialize_checksummed_region(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Pad to NAME_LENGTH (11 bytes) — deserializer's read_name() always reads 11.
        ser_pokemon::serialize_name(&self.player_name, &mut buf);
        ser_game_data::serialize_game_data_into(&self.game_data, &mut buf);
        ser_pokemon::serialize_sprite_data_into(&mut buf);
        ser_pokemon::serialize_party_into(&self.party, &mut buf);
        ser_pokemon::serialize_box_into(&self.current_box, &mut buf);
        buf.push(self.tile_animations);
        buf
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }
}

impl Default for SaveData {
    fn default() -> Self {
        Self::new()
    }
}

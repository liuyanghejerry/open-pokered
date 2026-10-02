use crate::alloc_prelude::*;
pub mod game_data;
#[cfg(target_os = "none")]
pub mod gba_sram;
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
mod daycare_tests;
#[cfg(test)]
mod save_tests;
#[cfg(test)]
mod sram_import_tests;

use crate::pokemon::party::Party;
use crate::pokemon::pc_box::{PcBox, PcStorage};
use game_data::{DayCareMon, GameData};
use hall_of_fame::HallOfFame;
use serde::{Deserialize, Serialize};

use crate::save_menu::calc_checksum;

pub use serialization::{SaveError, SRAM_BANK_SIZE};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveData {
    /// Import provenance only: old native SRAM did not persist system aliases.
    /// Its companion may supply those bits once, before the next canonical save.
    #[serde(skip)]
    pub imported_legacy_native: bool,
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
            imported_legacy_native: false,
            player_name: Vec::new(),
            game_data: GameData::new(),
            party: Party::new(),
            current_box: PcBox::new(),
            pc_storage: PcStorage::new(),
            hall_of_fame: HallOfFame::new(),
            tile_animations: 0,
        }
    }

    /// Keep the bank-1 live box and the storage menu's current slot together.
    /// `current_box` is authoritative at save/load/capture boundaries, just
    /// like the original's wBoxData (inactive SRAM boxes may be stale).
    pub fn sync_current_box_to_storage(&mut self) {
        *self.pc_storage.current_box_mut() = self.current_box;
    }

    pub fn sync_current_box_from_storage(&mut self) {
        self.current_box = *self.pc_storage.current_box();
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
        if !ok || self.game_data.daycare.in_use {
            return;
        }
        let Ok(mon) = self.party.remove(idx) else {
            return;
        };
        let catch_rate = get_base_stats(mon.species)
            .map(|b| b.catch_rate)
            .unwrap_or(0);
        let mut name_buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
        let name = mon.display_name(&mut name_buf);
        let dc = &mut self.game_data.daycare;
        dc.in_use = true;
        dc.species = mon.species as u8;
        dc.hp = mon.hp;
        dc.box_level = mon.level;
        dc.status = ser_pokemon::status_to_byte(&mon.status);
        // Day Care copies the complete box struct, including current types.
        dc.type1 = mon.type1 as u8;
        dc.type2 = mon.type2 as u8;
        dc.catch_rate = catch_rate;
        dc.moves = [
            mon.moves[0] as u8,
            mon.moves[1] as u8,
            mon.moves[2] as u8,
            mon.moves[3] as u8,
        ];
        dc.ot_id = mon.ot_id;
        dc.exp = mon.total_exp;
        dc.hp_exp = mon.stat_exp[0];
        dc.attack_exp = mon.stat_exp[1];
        dc.defense_exp = mon.stat_exp[2];
        dc.speed_exp = mon.stat_exp[3];
        dc.special_exp = mon.stat_exp[4];
        dc.dvs = u16::from_be_bytes(mon.dv_bytes);
        dc.pp = core::array::from_fn(|i| (mon.pp[i] & 0x3F) | ((mon.pp_ups[i] & 3) << 6));
        self.game_data.daycare_mon_ot = mon.ot_name.to_vec();
        self.game_data.daycare_mon_name =
            pokered_data::charmap::encode_string(&name).unwrap_or_default();
    }

    /// Withdraw at the grown level. The original copies the full box struct
    /// and OT/name tables, computes stats with stat experience, then learns
    /// moves after the deposit level by shifting full move/PP slots left.
    /// HP returns to max; existing status and existing PP are preserved.
    pub fn withdraw_daycare(&mut self) {
        use crate::battle::experience::growth::level_from_exp;
        use crate::pokemon::move_learning::learn_daycare_moves;
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
                    mon.pp = dc.pp.map(|p| p & 0x3F);
                    mon.pp_ups = dc.pp.map(|p| p >> 6);
                }
                mon.stat_exp = [dc.hp_exp, dc.attack_exp, dc.defense_exp, dc.speed_exp, dc.special_exp];
                mon.ot_id = dc.ot_id;
                mon.is_traded = crate::battle::obedience::is_traded_for(dc.ot_id, self.game_data.player_id);
                mon.status = ser_pokemon::byte_to_status(dc.status);
                mon.type1 = pokered_data::types::PokemonType::from_id(dc.type1);
                mon.type2 = pokered_data::types::PokemonType::from_id(dc.type2);
                mon.ot_name.fill(0x50);
                let ot = &self.game_data.daycare_mon_ot;
                let len = ot.len().min(mon.ot_name.len());
                mon.ot_name[..len].copy_from_slice(&ot[..len]);
                learn_daycare_moves(&mut mon, dc.box_level, new_level);
                recalculate_stats(&mut mon);
                mon.hp = mon.max_hp;
                let name = pokered_data::charmap::decode_string(&self.game_data.daycare_mon_name);
                if !name.is_empty()
                    && name != crate::save::ser_pokemon::species_default_name(species)
                {
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
        // Keep the large inline PC and Hall-of-Fame arrays in place. Building
        // a complete `SaveData::new()` temporary here can exhaust the GBA's
        // stack when this runs inside the top-level update state machine.
        self.player_name.clear();
        self.game_data = GameData::new();
        self.party.clear();
        self.current_box.clear();
        self.pc_storage.clear();
        self.hall_of_fame.clear();
        self.tile_animations = 0;
    }
}

impl Default for SaveData {
    fn default() -> Self {
        Self::new()
    }
}

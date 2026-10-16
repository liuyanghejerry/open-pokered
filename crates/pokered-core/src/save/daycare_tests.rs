//! Tests for the Day Care deposit/withdraw/EXP-growth mechanic and the
//! Game Corner coin balance, driven through [`SaveData`]/[`GameData`].

use super::SaveData;
use crate::alloc_prelude::*;
use crate::battle::experience::growth::exp_for_level;
use crate::pokemon::stats::create_pokemon;
use pokered_data::moves::MoveId;
use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::Species;

fn mon(species: Species, level: u8) -> crate::battle::state::Pokemon {
    create_pokemon(species, level, [0x9A, 0x78]).expect("valid species")
}

fn growth_of(species: Species) -> pokered_data::species::GrowthRate {
    get_base_stats(species).unwrap().growth_rate
}

#[test]
fn deposit_removes_mon_and_stores_growth_fields() {
    let mut save = SaveData::new();
    let _ = save.party.add(mon(Species::Bulbasaur, 5));
    let _ = save.party.add(mon(Species::Charmander, 7));
    assert_eq!(save.party.count(), 2);

    save.deposit_daycare(0);
    assert_eq!(save.party.count(), 1, "deposited mon leaves the party");
    assert!(save.game_data.daycare.in_use);
    assert_eq!(save.game_data.daycare.species, Species::Bulbasaur as u8);
    assert_eq!(save.game_data.daycare.box_level, 5);
    // The remaining party member is the one we did not deposit.
    assert_eq!(save.party.get(0).unwrap().species, Species::Charmander);
}

#[test]
fn deposit_refuses_the_players_last_pokemon() {
    let mut save = SaveData::new();
    let _ = save.party.add(mon(Species::Bulbasaur, 5));
    save.deposit_daycare(0);
    assert!(!save.game_data.daycare.in_use, "cannot deposit your last mon");
    assert_eq!(save.party.count(), 1);
}

#[test]
fn deposit_refuses_a_mon_that_knows_an_hm_move() {
    let mut save = SaveData::new();
    let mut cutter = mon(Species::Bulbasaur, 5);
    cutter.moves[0] = MoveId::Cut; // HM01
    let _ = save.party.add(cutter);
    let _ = save.party.add(mon(Species::Charmander, 7));
    save.deposit_daycare(0);
    assert!(!save.game_data.daycare.in_use, "HM-move holders are rejected");
    assert_eq!(save.party.count(), 2);
}

#[test]
fn exp_ticks_only_while_in_use_and_is_capped() {
    let mut save = SaveData::new();
    // No deposit yet: ticking is a no-op.
    save.game_data.tick_daycare_exp();
    assert_eq!(save.game_data.daycare.exp, 0);

    let _ = save.party.add(mon(Species::Bulbasaur, 5));
    let _ = save.party.add(mon(Species::Charmander, 7));
    save.deposit_daycare(0);
    let start = save.game_data.daycare.exp;
    for _ in 0..100 {
        save.game_data.tick_daycare_exp();
    }
    assert_eq!(save.game_data.daycare.exp, start + 100, "one EXP per step");

    // Cap at the species' level-100 experience.
    let cap = exp_for_level(growth_of(Species::Bulbasaur), 100);
    save.game_data.daycare.exp = cap - 1;
    save.game_data.tick_daycare_exp();
    save.game_data.tick_daycare_exp();
    assert_eq!(save.game_data.daycare.exp, cap, "exp never exceeds level-100");
}

#[test]
fn withdraw_returns_the_mon_grown_with_full_hp() {
    let mut save = SaveData::new();
    let _ = save.party.add(mon(Species::Bulbasaur, 5));
    let _ = save.party.add(mon(Species::Charmander, 7));
    save.deposit_daycare(0);

    // Simulate a long walk: raise the deposited mon's experience to level 15.
    save.game_data.daycare.exp = exp_for_level(growth_of(Species::Bulbasaur), 15);

    save.withdraw_daycare();
    assert!(!save.game_data.daycare.in_use, "day care is emptied on withdraw");
    assert_eq!(save.party.count(), 2, "the mon returns to the party");
    let back = save
        .party
        .to_vec()
        .into_iter()
        .find(|m| m.species == Species::Bulbasaur)
        .expect("Bulbasaur is back");
    assert_eq!(back.level, 15, "returned at its grown level");
    assert!(back.hp > 0 && back.hp == back.max_hp, "HP restored to full");
}

#[test]
fn withdraw_is_a_noop_when_party_is_full() {
    let mut save = SaveData::new();
    for lvl in 1..=6 {
        let _ = save.party.add(mon(Species::Pidgey, lvl));
    }
    // Force a deposited mon into the box directly.
    let _ = save.party.add(mon(Species::Bulbasaur, 5)); // rejected: party full
    save.game_data.daycare.in_use = true;
    save.game_data.daycare.species = Species::Rattata as u8;
    save.game_data.daycare.box_level = 3;
    save.game_data.daycare.exp = exp_for_level(growth_of(Species::Rattata), 3);

    save.withdraw_daycare();
    assert!(save.game_data.daycare.in_use, "cannot withdraw into a full party");
    assert_eq!(save.party.count(), 6);
}

#[test]
fn coins_credit_caps_at_9999_and_debit_saturates() {
    let mut save = SaveData::new();
    save.game_data.give_coins(500);
    assert_eq!(save.game_data.player_coins, 500);
    save.game_data.give_coins(60000); // would overflow / exceed cap
    assert_eq!(save.game_data.player_coins, 9999);
    save.game_data.take_coins(10000);
    assert_eq!(save.game_data.player_coins, 0, "debit saturates at zero");
}

/// Original wDayCareMon is a complete box_struct (ram/wram.asm:2221;
/// macros/ram.asm:7-25), including all five stat-exp fields. _MoveMon copies
/// the whole record and recalculates returning stats with effort enabled
/// (engine/pokemon/add_mon.asm:409-413,496-514).
#[test]
fn withdraw_preserves_stat_exp_for_returned_pokemon() {
    let mut save = SaveData::new();
    let deposited_effort = [1000, 2000, 3000, 4000, 5000];
    let resident_effort = [6000, 7000, 8000, 9000, 10000];
    let mut deposited = mon(Species::Bulbasaur, 20);
    deposited.stat_exp = deposited_effort;
    let deposited_exp = deposited.total_exp;
    let mut resident = mon(Species::Charmander, 7);
    resident.stat_exp = resident_effort;
    save.party.add(deposited).unwrap();
    save.party.add(resident).unwrap();
    save.deposit_daycare(0);

    // Growth while deposited (1 exp per step in the original).
    save.game_data.daycare.exp = save.game_data.daycare.exp.saturating_add(300);

    save.withdraw_daycare();
    assert_eq!(save.party.count(), 2);
    let resident = save.party.get(0).expect("resident stayed in the party");
    assert_eq!(resident.species, Species::Charmander);
    assert_eq!(resident.stat_exp, resident_effort);
    let back = save.party.get(1).expect("returned mon is appended to the party");
    assert_eq!(back.species, Species::Bulbasaur);
    assert_eq!(back.stat_exp, deposited_effort, "the full box record preserves stat exp");
    assert_eq!(back.total_exp, deposited_exp + 300, "experience growth is kept");
}

#[test]
fn daycare_evolved_magikarp_keeps_its_full_box_record_and_catch_byte_through_sram() {
    use super::ser_pokemon::{serialize_box_mon, BOX_STRUCT_SIZE};
    use super::sram_layout::{MAIN_DATA_OFFSET, SRAM_BANK_SIZE_LAYOUT};

    let mut evolved = mon(Species::Magikarp, 20);
    assert_eq!(evolved.catch_rate, Some(255));
    evolved.stat_exp = [1000, 2000, 3000, 4000, 5000];
    evolved.pp_ups[0] = 2;
    evolved.pp[0] = 7;
    crate::battle::settlement::evolution::apply_evolution(&mut evolved, Species::Gyarados);
    assert_eq!(evolved.species, Species::Gyarados);
    assert_eq!(get_base_stats(Species::Gyarados).unwrap().catch_rate, 45);
    assert_eq!(evolved.catch_rate, Some(255));

    let mut expected = Vec::new();
    serialize_box_mon(&evolved, &mut expected);
    assert_eq!(BOX_STRUCT_SIZE, 33);
    assert_eq!(expected.len(), 33);
    assert_eq!(expected[7], 255, "byte 7 belongs to this evolved instance");

    let mut save = SaveData::new();
    save.party.add(evolved).unwrap();
    save.party.add(mon(Species::Charmander, 7)).unwrap();
    save.deposit_daycare(0);
    assert!(save.game_data.daycare.in_use);
    assert_eq!(save.game_data.daycare.catch_rate, 255);

    let mut main_data = Vec::new();
    super::ser_game_data::serialize_game_data_into(&save.game_data, &mut main_data);
    let record_start = main_data.len() - BOX_STRUCT_SIZE;
    assert_eq!(&main_data[record_start..], expected.as_slice());
    let sram = super::sram_export::export_sram(&save);
    let sram_start = SRAM_BANK_SIZE_LAYOUT + MAIN_DATA_OFFSET + record_start;
    assert_eq!(&sram[sram_start..sram_start + BOX_STRUCT_SIZE], expected.as_slice());

    let mut restored = super::sram_import::import_sram(&sram).unwrap();
    assert_eq!(restored.game_data.daycare.catch_rate, 255);
    restored.withdraw_daycare();
    assert!(!restored.game_data.daycare.in_use);
    assert_eq!(restored.party.count(), 2);
    assert_eq!(restored.party.get(0).unwrap().species, Species::Charmander);
    let returned = restored.party.get(1).unwrap();
    assert_eq!(returned.species, Species::Gyarados);
    assert_eq!(returned.catch_rate, Some(255));
    assert_eq!(returned.stat_exp, evolved.stat_exp);
    assert_eq!(returned.pp_ups, evolved.pp_ups);
    assert_eq!(returned.pp, evolved.pp);
    let mut returned_bytes = Vec::new();
    serialize_box_mon(returned, &mut returned_bytes);
    assert_eq!(returned_bytes, expected);
}

#[test]
fn daycare_preserves_explicit_catch_bytes_and_defaults_only_legacy_absence() {
    for (saved, expected) in [(Some(0), 0), (Some(173), 173), (Some(45), 45), (None, 45)] {
        let mut save = SaveData::new();
        let mut deposited = mon(Species::Gyarados, 20);
        deposited.catch_rate = saved;
        assert_eq!(deposited.catch_rate, saved);
        save.party.add(deposited).unwrap();
        save.party.add(mon(Species::Charmander, 7)).unwrap();
        save.deposit_daycare(0);
        assert_eq!(save.game_data.daycare.catch_rate, expected, "saved={saved:?}");

        let sram = super::sram_export::export_sram(&save);
        let mut restored = super::sram_import::import_sram(&sram).unwrap();
        assert_eq!(restored.game_data.daycare.catch_rate, expected, "saved={saved:?}");
        restored.withdraw_daycare();
        let returned = restored.party.get(1).unwrap();
        assert_eq!(returned.species, Species::Gyarados);
        assert_eq!(returned.catch_rate, Some(expected), "saved={saved:?}");
    }
}

#[test]
fn name_rater_requires_both_ot_name_and_id_including_zero_id() {
    let mut save = SaveData::new();
    save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
    save.game_data.player_id = 42;
    let mut pokemon = mon(Species::MrMime, 20);
    pokemon.ot_id = 42;
    pokemon.ot_name.fill(0x50);
    pokemon.ot_name[..3].copy_from_slice(&save.player_name[..3]);
    save.party.add(pokemon).unwrap();
    assert!(save.party_mon_can_rename(0));
    save.party.get_mut(0).unwrap().ot_id = 0;
    assert!(!save.party_mon_can_rename(0));
    save.party.get_mut(0).unwrap().ot_id = 42;
    save.party.get_mut(0).unwrap().ot_name[0] ^= 1;
    assert!(!save.party_mon_can_rename(0));
    assert!(!save.party_mon_can_rename(6));
}

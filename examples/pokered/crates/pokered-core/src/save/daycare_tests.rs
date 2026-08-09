//! Tests for the Day Care deposit/withdraw/EXP-growth mechanic and the
//! Game Corner coin balance, driven through [`SaveData`]/[`GameData`].

use super::SaveData;
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

use crate::alloc_prelude::*;
use super::party::Party;
use super::pc_box::*;
use crate::battle::state::{Pokemon, StatusCondition};
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn make_pokemon(species: Species, level: u8) -> Pokemon {
    Pokemon {
        species,
        nickname: [0x50; 11],
        level,
        hp: 50,
        max_hp: 50,
        attack: 40,
        defense: 40,
        speed: 40,
        special: 40,
        type1: PokemonType::Normal,
        type2: PokemonType::Normal,
        moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        pp: [35, 0, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xAA, 0xAA],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

#[test]
fn new_box_is_empty() {
    let b = PcBox::new();
    assert!(b.is_empty());
    assert_eq!(b.count(), 0);
    assert!(!b.is_full());
}

#[test]
fn default_box_is_empty() {
    let b = PcBox::default();
    assert!(b.is_empty());
}

#[test]
fn deposit_and_withdraw() {
    let mut b = PcBox::new();
    let idx = b.deposit(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    assert_eq!(idx, 0);
    assert_eq!(b.count(), 1);

    let mon = b.withdraw(0).unwrap();
    assert_eq!(mon.species, Species::Bulbasaur);
    assert!(b.is_empty());
}

#[test]
fn deposit_up_to_twenty() {
    let mut b = PcBox::new();
    for i in 0..MONS_PER_BOX {
        assert!(b
            .deposit(make_pokemon(Species::Pidgey, (i + 1) as u8))
            .is_ok());
    }
    assert!(b.is_full());
    assert_eq!(
        b.deposit(make_pokemon(Species::Rattata, 5)),
        Err(BoxError::BoxFull)
    );
}

#[test]
fn withdraw_out_of_bounds() {
    let mut b = PcBox::new();
    b.deposit(make_pokemon(Species::Pikachu, 25)).unwrap();
    assert_eq!(b.withdraw(5), Err(BoxError::IndexOutOfBounds));
}

#[test]
fn release_is_withdraw() {
    let mut b = PcBox::new();
    b.deposit(make_pokemon(Species::Charmander, 10)).unwrap();
    let mon = b.release(0).unwrap();
    assert_eq!(mon.species, Species::Charmander);
    assert!(b.is_empty());
}

#[test]
fn box_species_list() {
    let mut b = PcBox::new();
    b.deposit(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    b.deposit(make_pokemon(Species::Squirtle, 8)).unwrap();
    assert_eq!(
        b.species_list(),
        vec![Species::Bulbasaur, Species::Squirtle]
    );
}

#[test]
fn box_find_species() {
    let mut b = PcBox::new();
    b.deposit(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    b.deposit(make_pokemon(Species::Charmander, 10)).unwrap();
    assert_eq!(b.find_species(Species::Charmander), Some(1));
    assert_eq!(b.find_species(Species::Mew), None);
}

#[test]
fn box_get_and_get_mut() {
    let mut b = PcBox::new();
    b.deposit(make_pokemon(Species::Pikachu, 25)).unwrap();
    assert_eq!(b.get(0).unwrap().level, 25);
    b.get_mut(0).unwrap().level = 30;
    assert_eq!(b.get(0).unwrap().level, 30);
    assert!(b.get(1).is_none());
}

#[test]
fn box_iter() {
    let mut b = PcBox::new();
    b.deposit(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    b.deposit(make_pokemon(Species::Charmander, 10)).unwrap();
    let species: Vec<Species> = b.iter().map(|p| p.species).collect();
    assert_eq!(species, vec![Species::Bulbasaur, Species::Charmander]);
}

#[test]
fn storage_new_has_12_empty_boxes() {
    let storage = PcStorage::new();
    assert_eq!(storage.box_count(), NUM_BOXES);
    assert_eq!(storage.current_box_index(), 0);
    assert_eq!(storage.total_stored(), 0);
    assert!(storage.current_box().is_empty());
}

#[test]
fn storage_default() {
    let storage = PcStorage::default();
    assert_eq!(storage.box_count(), NUM_BOXES);
}

#[test]
fn storage_change_box() {
    let mut storage = PcStorage::new();
    storage.change_box(5).unwrap();
    assert_eq!(storage.current_box_index(), 5);

    storage.change_box(11).unwrap();
    assert_eq!(storage.current_box_index(), 11);

    assert_eq!(storage.change_box(12), Err(BoxError::InvalidBoxNumber));
}

#[test]
fn storage_deposit_and_withdraw_current() {
    let mut storage = PcStorage::new();
    let idx = storage
        .deposit_to_current(make_pokemon(Species::Bulbasaur, 5))
        .unwrap();
    assert_eq!(idx, 0);
    assert_eq!(storage.total_stored(), 1);

    let mon = storage.withdraw_from_current(0).unwrap();
    assert_eq!(mon.species, Species::Bulbasaur);
    assert_eq!(storage.total_stored(), 0);
}

#[test]
fn storage_deposit_across_boxes() {
    let mut storage = PcStorage::new();
    storage
        .deposit_to_current(make_pokemon(Species::Bulbasaur, 5))
        .unwrap();

    storage.change_box(3).unwrap();
    storage
        .deposit_to_current(make_pokemon(Species::Charmander, 10))
        .unwrap();

    assert_eq!(storage.total_stored(), 2);
    assert_eq!(storage.get_box(0).unwrap().count(), 1);
    assert_eq!(storage.get_box(3).unwrap().count(), 1);
}

#[test]
fn storage_get_box() {
    let storage = PcStorage::new();
    assert!(storage.get_box(0).is_ok());
    assert!(storage.get_box(11).is_ok());
    assert_eq!(storage.get_box(12), Err(BoxError::InvalidBoxNumber));
}

#[test]
fn storage_deposit_from_party() {
    let mut storage = PcStorage::new();
    let mut party = Party::new();
    party.add(make_pokemon(Species::Pikachu, 25)).unwrap();
    party.add(make_pokemon(Species::Bulbasaur, 5)).unwrap();

    let box_idx = storage.deposit_from_party(&mut party, 1).unwrap();
    assert_eq!(box_idx, 0);
    assert_eq!(party.count(), 1);
    assert_eq!(party.leader().unwrap().species, Species::Pikachu);
    assert_eq!(
        storage.current_box().get(0).unwrap().species,
        Species::Bulbasaur
    );
}

#[test]
fn storage_deposit_from_party_cannot_remove_last() {
    let mut storage = PcStorage::new();
    let mut party = Party::new();
    party.add(make_pokemon(Species::Pikachu, 25)).unwrap();

    let result = storage.deposit_from_party(&mut party, 0);
    assert!(result.is_err());
    assert_eq!(party.count(), 1);
}

#[test]
fn storage_deposit_from_party_box_full() {
    let mut storage = PcStorage::new();
    for i in 0..MONS_PER_BOX {
        storage
            .deposit_to_current(make_pokemon(Species::Pidgey, (i + 1) as u8))
            .unwrap();
    }

    let mut party = Party::new();
    party.add(make_pokemon(Species::Pikachu, 25)).unwrap();
    party.add(make_pokemon(Species::Bulbasaur, 5)).unwrap();

    let result = storage.deposit_from_party(&mut party, 1);
    assert_eq!(result, Err(BoxError::BoxFull));
    assert_eq!(party.count(), 2);
}

#[test]
fn storage_withdraw_to_party() {
    let mut storage = PcStorage::new();
    storage
        .deposit_to_current(make_pokemon(Species::Bulbasaur, 5))
        .unwrap();

    let mut party = Party::new();
    party.add(make_pokemon(Species::Pikachu, 25)).unwrap();

    let party_idx = storage.withdraw_to_party(0, &mut party).unwrap();
    assert_eq!(party_idx, 1);
    assert_eq!(party.count(), 2);
    assert_eq!(party.get(1).unwrap().species, Species::Bulbasaur);
    assert!(storage.current_box().is_empty());
}

#[test]
fn storage_withdraw_to_party_party_full() {
    let mut storage = PcStorage::new();
    storage
        .deposit_to_current(make_pokemon(Species::Bulbasaur, 5))
        .unwrap();

    let mut party = Party::new();
    for i in 0..6 {
        party
            .add(make_pokemon(Species::Pidgey, (i + 1) as u8))
            .unwrap();
    }

    let result = storage.withdraw_to_party(0, &mut party);
    assert_eq!(result, Err(BoxError::BoxFull));
    assert_eq!(storage.current_box().count(), 1);
}

#[test]
fn storage_current_box_mut() {
    let mut storage = PcStorage::new();
    storage
        .deposit_to_current(make_pokemon(Species::Pikachu, 25))
        .unwrap();
    storage.current_box_mut().get_mut(0).unwrap().level = 50;
    assert_eq!(storage.current_box().get(0).unwrap().level, 50);
}

#[test]
fn storage_get_box_mut() {
    let mut storage = PcStorage::new();
    storage.change_box(7).unwrap();
    storage
        .deposit_to_current(make_pokemon(Species::Mew, 100))
        .unwrap();

    storage.get_box_mut(7).unwrap().get_mut(0).unwrap().level = 5;
    assert_eq!(storage.get_box(7).unwrap().get(0).unwrap().level, 5);
}

// The real legal-run Hypno gained stat EXP without gaining a level. Its party
// cache was still 129 HP; the Gen-I box record reconstructs a 130-HP maximum.
fn trained_hypno() -> Pokemon {
    let mut mon = super::stats::create_pokemon(Species::Hypno, 38, [147, 142]).unwrap();
    mon.total_exp = 59108;
    mon.stat_exp = [7864, 12837, 7647, 13972, 6908];
    mon.hp = 129;
    mon.max_hp = 129;
    mon.moves = [
        MoveId::Headbutt, MoveId::Hypnosis, MoveId::Disable, MoveId::PsychicM,
    ];
    mon.pp = [15, 20, 20, 10];
    mon.ot_id = 25506;
    mon.ot_name[..4].copy_from_slice(&[0x91, 0x84, 0x83, 0x50]); // RED
    mon
}

#[test]
fn deposit_rebuilds_box_stats_without_healing_or_changing_the_record() {
    use crate::save::ser_pokemon::{deserialize_box_mon, serialize_box_mon};
    let mon = trained_hypno();
    let mut original_record = Vec::new();
    serialize_box_mon(&mon, &mut original_record);
    let mut b = PcBox::new();
    b.deposit(mon).unwrap();
    let stored = *b.get(0).unwrap();
    assert_eq!((stored.hp, stored.max_hp), (129, 130));
    assert_eq!(
        (stored.attack, stored.defense, stored.speed, stored.special),
        (77, 68, 73, 111)
    );
    let mut stored_record = Vec::new();
    serialize_box_mon(&stored, &mut stored_record);
    assert_eq!(stored_record, original_record);
    let mut restored = deserialize_box_mon(&stored_record).unwrap();
    restored.nickname = stored.nickname;
    restored.ot_name = stored.ot_name;
    assert_eq!(restored, stored);
}

#[test]
fn legacy_json_box_rebuilds_stats_without_changing_hp_or_box_level() {
    let mon = trained_hypno();
    let json = serde_json::to_string(&vec![mon]).unwrap();
    let b: PcBox = serde_json::from_str(&json).unwrap();
    let mut expected = mon;
    expected.max_hp = 130;
    expected.attack = 77;
    expected.defense = 68;
    expected.speed = 73;
    expected.special = 111;
    assert_eq!(b.get(0), Some(&expected));
}

#[test]
fn withdraw_recomputes_level_from_exp_and_stats_without_healing() {
    use crate::battle::experience::growth::exp_for_level;
    use pokered_data::pokemon_data::get_base_stats;
    let mut mon = trained_hypno();
    let base = get_base_stats(mon.species).unwrap();
    mon.total_exp = exp_for_level(base.growth_rate, 39);
    mon.status = StatusCondition::Sleep(3);
    mon.pp_ups = [1, 2, 3, 0];
    let mut storage = PcStorage::new();
    storage.deposit_to_current(mon).unwrap();
    // A box STATS view uses the stored box level, not the XP-derived level.
    assert_eq!(storage.current_box().get(0).unwrap().level, 38);
    let mut party = Party::new();
    storage.withdraw_to_party(0, &mut party).unwrap();
    let restored = *party.get(0).unwrap();
    let mut expected = mon;
    expected.level = 39;
    let (hp, atk, def, spd, spc) = crate::battle::experience::stats::calc_all_stats(
        base, mon.dv_bytes, &mon.stat_exp, 39,
    );
    expected.max_hp = hp;
    expected.attack = atk;
    expected.defense = def;
    expected.speed = spd;
    expected.special = spc;
    assert_eq!(restored, expected);
    assert_eq!(restored.hp, 129);
}

#[test]
fn withdraw_from_current_rebuilds_stats_and_does_not_revive() {
    let mut mon = trained_hypno();
    mon.hp = 0;
    mon.status = StatusCondition::Poison;
    let mut storage = PcStorage::new();
    storage.deposit_to_current(mon).unwrap();
    // Model an older in-memory box, including a stale derived-stat cache.
    *storage.current_box_mut().get_mut(0).unwrap() = mon;
    let restored = storage.withdraw_from_current(0).unwrap();
    assert_eq!((restored.hp, restored.max_hp), (0, 130));
    assert_eq!(restored.status, StatusCondition::Poison);
    assert_eq!(restored.total_exp, mon.total_exp);
    assert_eq!(restored.dv_bytes, mon.dv_bytes);
    assert_eq!(restored.stat_exp, mon.stat_exp);
    assert_eq!(restored.moves, mon.moves);
    assert_eq!(restored.pp, mon.pp);
    assert_eq!(restored.ot_name, mon.ot_name);
}

#[test]
fn box_stat_rebuild_preserves_fainting_status_pp_and_identity_at_level_limits() {
    use crate::save::ser_pokemon::{deserialize_box_mon, serialize_box_mon};
    for level in [1, 5, 38, 99, 100] {
        for hp in [0, 1] {
            let mut mon = super::stats::create_pokemon(Species::Hypno, level, [147, 142]).unwrap();
            mon.stat_exp = [7864, 12837, 7647, 13972, 6908];
            mon.hp = hp;
            mon.status = StatusCondition::Paralysis;
            mon.pp = [1, 0, 2, 3];
            mon.pp_ups = [1, 2, 3, 0];
            mon.ot_id = 25506;
            mon.is_traded = true;
            mon.nickname[..4].copy_from_slice(&[0x80, 0x81, 0x82, 0x50]);
            let mut record = Vec::new();
            serialize_box_mon(&mon, &mut record);
            let expected_stats = deserialize_box_mon(&record).unwrap();
            let mut b = PcBox::new();
            b.deposit(mon).unwrap();
            let stored = *b.get(0).unwrap();
            let mut regenerated = Vec::new();
            serialize_box_mon(&stored, &mut regenerated);
            assert_eq!(regenerated, record);
            assert_eq!(stored.max_hp, expected_stats.max_hp);
            assert_eq!(stored.nickname, mon.nickname);
            assert_eq!(stored.is_traded, mon.is_traded);
            let mut storage = PcStorage::new();
            storage.deposit_to_current(mon).unwrap();
            assert_eq!(storage.withdraw_from_current(0).unwrap(), stored);
        }
    }
}

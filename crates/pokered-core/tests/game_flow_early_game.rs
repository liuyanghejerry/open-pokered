//! Integration tests: Early game flow
//! Route 1 wild encounters, Viridian City, Oak's Parcel, Pokedex, buying Pokeballs

mod game_flow_helpers;

use game_flow_helpers::*;
use pokered_core::battle::state::*;
use pokered_core::battle::turn::execute_turn;
use pokered_core::items::inventory::Inventory;
use pokered_core::items::shop::{buy_price, try_buy, BuyResult};
use pokered_core::overworld::event_flags::EventFlags;
use pokered_core::overworld::map_loading::get_map_name;
use pokered_core::pokemon::party::Party;
use pokered_core::pokemon::pokedex::Pokedex;
use pokered_data::event_flags::EventFlag;
use pokered_data::items::ItemId;
use pokered_data::maps::MapId;
use pokered_data::move_data::MoveData;
use pokered_data::moves::{MoveEffect, MoveId};
use pokered_data::species::Species;
use pokered_data::types::PokemonType;
use pokered_data::wild_data::{wild_data, GameVersion};

/// Route 1 wild encounter table (Red) has only Pidgey and Rattata, levels 2-5
#[test]
fn route1_wild_encounters_red_only_pidgey_rattata() {
    let data = wild_data(GameVersion::Red);
    let route1 = data.iter().find(|d| d.name == "Route1").unwrap();

    assert!(
        route1.grass.encounter_rate > 0,
        "Route 1 should have grass encounters"
    );
    assert!(
        route1.water.mons.is_empty(),
        "Route 1 has no water encounters"
    );

    for mon in &route1.grass.mons {
        assert!(
            mon.species == Species::Pidgey || mon.species == Species::Rattata,
            "Route 1 should only have Pidgey/Rattata, found {:?}",
            mon.species
        );
        assert!(
            mon.level >= 2 && mon.level <= 5,
            "Route 1 wild levels should be 2-5, found {}",
            mon.level
        );
    }
}

/// Route 1 encounter table has exactly 10 slots (Game Boy encounter table format)
#[test]
fn route1_encounter_table_has_10_slots() {
    let data = wild_data(GameVersion::Red);
    let route1 = data.iter().find(|d| d.name == "Route1").unwrap();
    assert_eq!(
        route1.grass.mons.len(),
        10,
        "Encounter table should have 10 slots"
    );
}

/// Blue version Route 1 also has only Pidgey and Rattata
#[test]
fn route1_wild_encounters_blue_same_species() {
    let data = wild_data(GameVersion::Blue);
    let route1 = data.iter().find(|d| d.name == "Route1").unwrap();

    for mon in &route1.grass.mons {
        assert!(
            mon.species == Species::Pidgey || mon.species == Species::Rattata,
            "Blue Route 1 should also only have Pidgey/Rattata, found {:?}",
            mon.species
        );
    }
}

/// Wild battle on Route 1: player's starter vs a wild Pidgey
#[test]
fn route1_wild_battle_starter_vs_pidgey() {
    let player = make_starter_charmander();
    let wild_pidgey = make_pokemon(
        Species::Pidgey,
        3,
        15,
        10,
        10,
        12,
        8,
        PokemonType::Normal,
        PokemonType::Flying,
        [MoveId::Gust, MoveId::None, MoveId::None, MoveId::None],
        [35, 0, 0, 0],
    );

    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![wild_pidgey]);
    state.player.selected_move = MoveId::Scratch;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Gust;
    state.enemy.selected_move_index = 0;

    let scratch_data = MoveData {
        id: MoveId::Scratch,
        effect: MoveEffect::NoAdditionalEffect,
        power: 40,
        move_type: PokemonType::Normal,
        accuracy: 255,
        pp: 35,
    };
    let gust_data = MoveData {
        id: MoveId::Gust,
        effect: MoveEffect::NoAdditionalEffect,
        power: 40,
        move_type: PokemonType::Normal,
        accuracy: 255,
        pp: 35,
    };

    let randoms = turn_randoms_player_first();
    execute_turn(&mut state, &scratch_data, &gust_data, &randoms);

    // Charmander (speed 13) is faster than Pidgey (speed 12), should hit first
    // At least one side took damage
    assert!(
        state.player.active_mon().hp < state.player.active_mon().max_hp
            || state.enemy.active_mon().hp < state.enemy.active_mon().max_hp,
        "At least one side should take damage in wild battle"
    );
}

/// Event flags: getting starter sets the EVENT_GOT_STARTER flag
#[test]
fn event_flag_got_starter() {
    let mut flags = EventFlags::new();
    assert!(!flags.check(EventFlag::EVENT_GOT_STARTER));

    flags.set(EventFlag::EVENT_GOT_STARTER);
    assert!(flags.check(EventFlag::EVENT_GOT_STARTER));
}

/// Event flags: battled rival sets EVENT_BATTLED_RIVAL_IN_OAKS_LAB
#[test]
fn event_flag_battled_rival_in_lab() {
    let mut flags = EventFlags::new();
    flags.set(EventFlag::EVENT_GOT_STARTER);
    flags.set(EventFlag::EVENT_BATTLED_RIVAL_IN_OAKS_LAB);

    assert!(flags.check(EventFlag::EVENT_GOT_STARTER));
    assert!(flags.check(EventFlag::EVENT_BATTLED_RIVAL_IN_OAKS_LAB));
}

/// Event flags: got Pokedex from Oak
#[test]
fn event_flag_got_pokedex() {
    let mut flags = EventFlags::new();
    flags.set(EventFlag::EVENT_GOT_STARTER);
    flags.set(EventFlag::EVENT_BATTLED_RIVAL_IN_OAKS_LAB);
    flags.set(EventFlag::EVENT_GOT_POKEDEX);

    assert!(flags.check(EventFlag::EVENT_GOT_POKEDEX));
    assert_eq!(flags.count_set(), 3);
}

/// Oak's Parcel can be added to inventory and is a key item
#[test]
fn oaks_parcel_inventory_management() {
    let mut bag = Inventory::new_bag();
    assert!(bag.is_empty());

    bag.add_item(ItemId::OaksParcel, 1).unwrap();
    assert!(bag.has_item(ItemId::OaksParcel, 1));
    assert_eq!(bag.count(), 1);

    // Deliver parcel (remove it)
    bag.remove_item(ItemId::OaksParcel, 1).unwrap();
    assert!(!bag.has_item(ItemId::OaksParcel, 1));
    assert!(bag.is_empty());
}

/// Buying Pokeballs from Viridian Mart after delivering parcel
#[test]
fn buy_pokeballs_from_mart() {
    let mut bag = Inventory::new_bag();
    let mut money: u32 = 3000;

    // Pokeball costs 200
    let price = buy_price(ItemId::PokeBall, 5).unwrap();
    assert_eq!(price, 1000, "5 Pokeballs should cost 1000");

    let result = try_buy(ItemId::PokeBall, 5, &mut money, &mut bag);
    assert!(matches!(result, BuyResult::Success { .. }));
    assert_eq!(money, 2000);
    assert!(bag.has_item(ItemId::PokeBall, 5));
}

#[test]
fn buy_potions_from_mart() {
    let mut bag = Inventory::new_bag();
    let mut money: u32 = 3000;

    let price = buy_price(ItemId::Potion, 3).unwrap();
    assert_eq!(price, 900, "3 Potions should cost 900 (300 each)");

    let result = try_buy(ItemId::Potion, 3, &mut money, &mut bag);
    assert!(matches!(result, BuyResult::Success { .. }));
    assert_eq!(money, 2100);
    assert!(bag.has_item(ItemId::Potion, 3));
}

#[test]
fn buy_fails_insufficient_money() {
    let mut bag = Inventory::new_bag();
    let mut money: u32 = 100;

    let result = try_buy(ItemId::PokeBall, 5, &mut money, &mut bag);
    assert!(matches!(result, BuyResult::NotEnoughMoney));
    assert_eq!(money, 100, "Money should not change on failed purchase");
    assert!(bag.is_empty());
}

/// Map name lookups work for early game areas
#[test]
fn early_game_map_names() {
    assert_eq!(get_map_name(MapId::PalletTown), "PALLET TOWN");
    assert_eq!(get_map_name(MapId::Route1), "ROUTE 1");
    assert_eq!(get_map_name(MapId::ViridianCity), "VIRIDIAN CITY");
}

/// Pokedex tracking after Route 1 encounters
#[test]
fn pokedex_after_route1_encounters() {
    let mut dex = Pokedex::new();

    // Player chose Charmander, saw wild Pidgey and Rattata
    dex.set_owned(Species::Charmander);
    dex.set_seen(Species::Pidgey);
    dex.set_seen(Species::Rattata);

    assert_eq!(dex.owned_count(), 1);
    assert_eq!(dex.seen_count(), 3); // owned counts as seen too
    assert!(dex.is_owned(Species::Charmander));
    assert!(dex.is_seen(Species::Pidgey));
    assert!(dex.is_seen(Species::Rattata));
    assert!(!dex.is_owned(Species::Pidgey));
}

/// After catching a Pidgey, it becomes owned
#[test]
fn pokedex_after_catching_pidgey() {
    let mut dex = Pokedex::new();
    dex.set_owned(Species::Charmander);
    dex.set_seen(Species::Pidgey);
    dex.set_seen(Species::Rattata);

    // Catch Pidgey
    dex.set_owned(Species::Pidgey);
    assert_eq!(dex.owned_count(), 2);
    assert_eq!(dex.seen_count(), 3);
    assert!(dex.is_owned(Species::Pidgey));
}

/// Adding a caught Pokemon to party
#[test]
fn add_caught_pokemon_to_party() {
    let starter = make_starter_charmander();
    let mut party = Party::from_pokemon(vec![starter]).unwrap();
    assert_eq!(party.count(), 1);

    let pidgey = make_pokemon(
        Species::Pidgey,
        3,
        15,
        10,
        10,
        12,
        8,
        PokemonType::Normal,
        PokemonType::Flying,
        [MoveId::Gust, MoveId::None, MoveId::None, MoveId::None],
        [35, 0, 0, 0],
    );

    let idx = party.add(pidgey).unwrap();
    assert_eq!(idx, 1);
    assert_eq!(party.count(), 2);
    assert_eq!(party.get(1).unwrap().species, Species::Pidgey);
}

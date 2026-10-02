// Audit observations, not regression tests for desired behavior.
// Run by temporarily copying to crates/pokered-core/tests/audit_systems_probe.rs.
use pokered_core::items::{inventory::Inventory, shop::{try_buy, try_sell, BuyResult}};
use pokered_core::pokemon::{stats::{create_pokemon, recalculate_stats}, pc_box::PcBox, move_learning::replace_move};
use pokered_core::save::{SaveData, ser_game_data::serialize_game_data_into, ser_pokemon::{serialize_box_mon, deserialize_box_mon}};
use pokered_data::{items::ItemId, species::Species, moves::MoveId};

#[test]
fn full_bag_buy_is_not_atomic() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 95).unwrap();
    for id in 1..=40 {
        let item = ItemId::from_id(id);
        if item != ItemId::Potion && !bag.has_item(item,1) {
            bag.add_item(item, 1).unwrap();
        }
        if bag.count() == 20 { break; }
    }
    assert_eq!(bag.count(),20);
    let mut money = 10000;
    let result = try_buy(ItemId::Potion,10,&mut money,&mut bag);
    println!("full bag: result={result:?}, Potion95→{}, money10000→{money}",bag.item_quantity(ItemId::Potion));
    assert_eq!(result,BuyResult::BagFull);
    assert_eq!(bag.item_quantity(ItemId::Potion),99);
    assert_eq!(money,10000);
}

#[test]
fn mart_money_not_capped() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Nugget, 1).unwrap();
    let mut money = 999999;
    let result = try_sell(0,1,&mut money,&mut bag);
    println!("sell Nugget at cap: result={result:?}, money={money}");
    assert!(money > 999999);
}

#[test]
fn daycare_tail_is_23_not_33_bytes() {
    let mut save = SaveData::new();
    save.game_data.daycare.species = 0xA5;
    let mut main = Vec::new();
    serialize_game_data_into(&save.game_data, &mut main);
    let marker = main.iter().rposition(|b| *b==0xA5).unwrap();
    println!("main len={}, daycare struct tail={}, canonical region={}",main.len(),main.len()-marker,save.serialize_checksummed_region().len());
    assert_eq!(main.len()-marker,23);
}

#[test]
fn sram_species_is_dex_id_instead_of_original_internal_id() {
    let mon = create_pokemon(Species::Pikachu,25,[0x99,0x88]).unwrap();
    let mut bytes = Vec::new();
    serialize_box_mon(&mon,&mut bytes);
    assert_eq!(bytes[0],25);
    bytes[0]=0x54; // Original PIKACHU internal species ID.
    let wrong = deserialize_box_mon(&bytes).unwrap();
    println!("Pikachu exports species={:02x}; original 54 imports as {:?}",25,wrong.species);
    assert_eq!(wrong.species,Species::Doduo);
}

#[test]
fn daycare_loses_effort_ot_and_ppups_without_save_reload() {
    let mut save = SaveData::new();
    save.game_data.player_id = 1234;
    let mut mon = create_pokemon(Species::Pikachu,25,[0x99,0x88]).unwrap();
    mon.stat_exp=[10000;5]; mon.ot_id=5678; mon.is_traded=true;
    mon.ot_name=pokered_core::battle::state::encode_name("ALICE");
    mon.pp_ups[0]=3;
    save.party.add(mon).unwrap();
    save.party.add(create_pokemon(Species::Pidgey,10,[0x99,0x88]).unwrap()).unwrap();
    save.deposit_daycare(0);
    println!("daycare deposited OT={}, OT name length={}",save.game_data.daycare.ot_id,save.game_data.daycare_mon_ot.len());
    save.withdraw_daycare();
    let received=save.party.get(1).unwrap();
    println!("daycare returned stat_exp={:?}, OT={}, traded={}, PP Ups={:?}",received.stat_exp,received.ot_id,received.is_traded,received.pp_ups);
    assert_eq!(received.stat_exp,[0;5]);
    assert_eq!(received.ot_id,0);
    assert_eq!(received.pp_ups,[0;4]);
}

#[test]
fn daycare_full_moves_silently_skip_grown_level_moves() {
    let mut save = SaveData::new();
    let mut mon=create_pokemon(Species::Pikachu,25,[0x99,0x88]).unwrap();
    mon.moves=[MoveId::Thundershock,MoveId::Growl,MoveId::ThunderWave,MoveId::QuickAttack];
    save.party.add(mon).unwrap();
    save.party.add(create_pokemon(Species::Pidgey,10,[0x99,0x88]).unwrap()).unwrap();
    save.deposit_daycare(0);
    save.game_data.daycare.exp=27000; // Medium Fast level30; L26 Swift.
    save.withdraw_daycare();
    let got=save.party.get(1).unwrap();
    println!("daycare Lv25→{} moves={:?}, Swift absent={}",got.level,got.moves,!got.moves.contains(&MoveId::Swift));
    assert_eq!(got.level,30);
    assert!(!got.moves.contains(&MoveId::Swift));
}

#[test]
fn pc_withdraw_does_not_recalculate_effort_stats() {
    let mut mon=create_pokemon(Species::Pikachu,50,[0x99,0x88]).unwrap();
    let old_attack=mon.attack;
    mon.stat_exp=[65535;5];
    let mut expected=mon;
    recalculate_stats(&mut expected);
    let mut pc=PcBox::new();
    pc.deposit(mon).unwrap();
    let got=pc.withdraw(0).unwrap();
    println!("box trick: old attack={old_attack}, actual={}, recomputed={}",got.attack,expected.attack);
    assert_eq!(got.attack,old_attack);
    assert_ne!(got.attack,expected.attack);
}

#[test]
fn learned_move_inherits_old_slot_ppups() {
    let mut mon=create_pokemon(Species::Pikachu,25,[0x99,0x88]).unwrap();
    mon.pp_ups[0]=3;
    replace_move(&mut mon,0,MoveId::Thunderbolt);
    println!("replacement move {:?}, PP={}, PP Ups={}",mon.moves[0],mon.pp[0],mon.pp_ups[0]);
    assert_eq!(mon.pp_ups[0],3);
}

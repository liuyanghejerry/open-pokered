use pokered_core::items::{
    inventory::Inventory,
    shop::{try_buy, try_sell, BuyResult},
};
use pokered_core::pokemon::{
    move_learning::replace_move,
    pc_box::PcBox,
    stats::{create_pokemon, recalculate_stats},
};
use pokered_core::save::{
    ser_game_data::serialize_game_data_into,
    ser_pokemon::{deserialize_box_mon, serialize_box_mon},
    sram_export::export_sram,
    sram_import::import_sram,
    SaveData,
};
use pokered_data::{items::ItemId, moves::MoveId, species::Species};

#[test]
fn full_bag_purchase_failure_is_atomic() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 95).unwrap();
    for id in 1..=40 {
        let item = ItemId::from_id(id);
        if item != ItemId::Potion && !bag.has_item(item, 1) {
            bag.add_item(item, 1).unwrap();
        }
        if bag.count() == 20 {
            break;
        }
    }
    let before = bag.clone();
    let mut money = 10000;
    assert_eq!(
        try_buy(ItemId::Potion, 10, &mut money, &mut bag),
        BuyResult::BagFull
    );
    assert_eq!(bag, before);
    assert_eq!(money, 10000);
    assert!(matches!(
        try_buy(ItemId::Potion, 4, &mut money, &mut bag),
        BuyResult::Success { .. }
    ));
    assert_eq!(bag.item_quantity(ItemId::Potion), 99);
    // Duplicate stacks are legal. The ROM stops on overflow of the first
    // stack when full, even if a later duplicate has space.
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 100).unwrap();
    for id in 1..=40 {
        let item = ItemId::from_id(id);
        if item != ItemId::Potion && !bag.has_item(item, 1) {
            bag.add_item(item, 1).unwrap();
        }
        if bag.count() == 20 {
            break;
        }
    }
    let before = bag.clone();
    assert!(bag.add_item(ItemId::Potion, 1).is_err());
    assert_eq!(bag, before);
}

#[test]
fn sale_caps_bcd_money() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Nugget, 1).unwrap();
    let mut money = 999998;
    try_sell(0, 1, &mut money, &mut bag);
    assert_eq!(money, 999999);
    assert!(bag.is_empty());
}

#[test]
fn original_rom_species_ids_cover_every_species() {
    assert_eq!(Species::Pikachu.to_rom_id(), 0x54);
    assert_eq!(Species::Bulbasaur.to_rom_id(), 0x99);
    assert_eq!(Species::Rhydon.to_rom_id(), 1);
    for dex in 1..=151 {
        let s = Species::from_index_id(dex);
        assert_eq!(Species::from_rom_id(s.to_rom_id()), s);
    }
    let mon = create_pokemon(Species::Pikachu, 25, [0x99, 0x88]).unwrap();
    let mut bytes = Vec::new();
    serialize_box_mon(&mon, &mut bytes);
    assert_eq!(bytes[0], 0x54);
    assert_eq!(
        deserialize_box_mon(&bytes).unwrap().species,
        Species::Pikachu
    );
}

#[test]
fn daycare_preserves_full_box_struct_through_sram() {
    let mut save = SaveData::new();
    save.game_data.player_id = 1234;
    let mut mon = create_pokemon(Species::Pikachu, 25, [0x99, 0x88]).unwrap();
    mon.stat_exp = [10000, 20000, 30000, 40000, 50000];
    mon.ot_id = 5678;
    mon.is_traded = true;
    mon.ot_name = pokered_core::battle::state::encode_name("ALICE");
    mon.pp_ups[0] = 3;
    mon.pp[0] = 7;
    mon.status = pokered_core::battle::state::StatusCondition::Poison;
    save.party.add(mon).unwrap();
    save.party
        .add(create_pokemon(Species::Pidgey, 10, [0x99, 0x88]).unwrap())
        .unwrap();
    save.deposit_daycare(0);
    let mut save = import_sram(&export_sram(&save)).unwrap();
    save.withdraw_daycare();
    let got = save.party.get(1).unwrap();
    assert_eq!(got.stat_exp, mon.stat_exp);
    assert_eq!(got.ot_id, 5678);
    assert!(got.is_traded);
    assert_eq!(got.ot_name, mon.ot_name);
    assert_eq!(got.pp_ups, mon.pp_ups);
    assert_eq!(got.pp, mon.pp);
    assert_eq!(got.status, mon.status);
    assert_eq!(got.hp, got.max_hp);
}

#[test]
fn daycare_full_moves_shift_oldest_move_and_its_pp() {
    let mut save = SaveData::new();
    let mut mon = create_pokemon(Species::Pikachu, 25, [0x99, 0x88]).unwrap();
    mon.moves = [
        MoveId::Thundershock,
        MoveId::Growl,
        MoveId::ThunderWave,
        MoveId::QuickAttack,
    ];
    mon.pp = [4, 5, 6, 7];
    mon.pp_ups = [3, 2, 1, 0];
    save.party.add(mon).unwrap();
    save.party
        .add(create_pokemon(Species::Pidgey, 10, [0x99, 0x88]).unwrap())
        .unwrap();
    save.deposit_daycare(0);
    save.game_data.daycare.exp = 27000;
    save.withdraw_daycare();
    let got = save.party.get(1).unwrap();
    assert_eq!(
        got.moves,
        [
            MoveId::Growl,
            MoveId::ThunderWave,
            MoveId::QuickAttack,
            MoveId::Swift
        ]
    );
    assert_eq!(&got.pp[..3], &[5, 6, 7]);
    assert_eq!(got.pp_ups, [2, 1, 0, 0]);
}

#[test]
fn pc_box_trick_recomputes_stats_from_effort() {
    let mut mon = create_pokemon(Species::Pikachu, 50, [0x99, 0x88]).unwrap();
    mon.stat_exp = [65535; 5];
    let mut expected = mon;
    recalculate_stats(&mut expected);
    let mut pc = PcBox::new();
    pc.deposit(mon).unwrap();
    let got = pc.withdraw(0).unwrap();
    assert_eq!(got.attack, expected.attack);
    assert_eq!(got.max_hp, expected.max_hp);
    assert_eq!(got.hp, mon.hp);
}

#[test]
fn replacing_move_clears_previous_pp_ups() {
    let mut mon = create_pokemon(Species::Pikachu, 25, [0x99, 0x88]).unwrap();
    mon.pp_ups[0] = 3;
    replace_move(&mut mon, 0, MoveId::Thunderbolt);
    assert_eq!(mon.pp_ups[0], 0);
    assert_eq!(mon.pp[0], 15);
}

// Independent offsets from pret fbcf7d0, assembled with RGBDS 1.0.1 by the
// audit. This fixture does not use the Rust serializer to construct layout.
fn original_sram() -> Vec<u8> {
    let mut bytes = vec![0u8; 32768];
    bytes[0x2598..0x25a3].fill(0x50);
    bytes[0x2f2c] = 1;
    bytes[0x2f2d] = 0x54;
    bytes[0x2f2e] = 0xff;
    let at = 0x2f34;
    bytes[at] = 0x54;
    bytes[at + 1..at + 3].copy_from_slice(&50u16.to_be_bytes());
    bytes[at + 3] = 25;
    bytes[at + 12..at + 14].copy_from_slice(&1234u16.to_be_bytes());
    bytes[at + 14..at + 17].copy_from_slice(&[0, 0x3d, 9]);
    bytes[at + 17..at + 19].copy_from_slice(&54321u16.to_be_bytes());
    bytes[at + 27..at + 29].copy_from_slice(&[0x99, 0x88]);
    bytes[at + 33] = 25;
    for i in 0..5 {
        bytes[at + 34 + i * 2..at + 36 + i * 2].copy_from_slice(&60u16.to_be_bytes());
    }
    bytes[0x29b9..0x29bb].copy_from_slice(&123u16.to_be_bytes()); // wSafariSteps
    bytes[0x29e3] = 2; // completed NPC trade 1: MARCEL, byte-oriented bitset
    bytes[0x2914] = 0x35;
    bytes[0x2963] = 0x7a; // original progress tail must survive
    bytes[0x2cf3] = 7; // wNumSafariBalls
    bytes[0x3523] = pokered_core::save_menu::calc_checksum(&bytes[0x2598..0x3523]);
    // First CHANGE BOX has not initialized banks 2/3 yet.
    bytes[0x4000..].fill(0xa7);
    bytes
}

#[test]
fn original_offsets_and_uninitialized_box_banks_import() {
    let save = import_sram(&original_sram()).unwrap();
    assert!(!save.imported_legacy_native);
    assert_eq!(save.party.get(0).unwrap().species, Species::Pikachu);
    assert_eq!(save.party.get(0).unwrap().stat_exp[0], 54321);
    assert_eq!(save.game_data.safari_steps, 123);
    assert_eq!(save.game_data.num_safari_balls, 7);
    assert_eq!(save.game_data.completed_in_game_trade_flags, 2);
    let mut main = Vec::new();
    serialize_game_data_into(&save.game_data, &mut main);
    assert_eq!(main.len(), 1929);
    assert_eq!(save.serialize_checksummed_region().len(), 3979);
    let exported = export_sram(&save);
    assert_eq!(exported[0x2f2c], 1);
    assert_eq!(exported[0x2f34], 0x54);
    assert_eq!(exported[0x2914], 0x35);
    assert_eq!(exported[0x2963], 0x7a);
    assert_eq!(
        exported[0x3523],
        pokered_core::save_menu::calc_checksum(&exported[0x2598..0x3523])
    );
}

#[test]
fn original_new_game_ignores_previous_playthroughs_valid_box_banks() {
    let mut previous = SaveData::new();
    previous
        .pc_storage
        .get_box_mut(7)
        .unwrap()
        .deposit(create_pokemon(Species::Pikachu, 25, [0x99, 0x88]).unwrap())
        .unwrap();
    let previous = export_sram(&previous);
    assert_ne!(previous[0x284c] & 0x80, 0);
    let mut bytes = original_sram();
    bytes[0x4000..0x8000].copy_from_slice(&previous[0x4000..0x8000]);
    assert_eq!(bytes[0x284c] & 0x80, 0); // new ROM game, no first CHANGE BOX yet
    let save = import_sram(&bytes).unwrap();
    for index in 0..12 {
        assert_eq!(save.pc_storage.get_box(index).unwrap().count(), 0);
    }
}

#[test]
fn previous_native_save_migrates_layout_and_species() {
    let mut bytes = original_sram();
    bytes[0x2f2d] = 25;
    bytes[0x2f34] = 25;
    bytes.swap(0x29e3, 0x29e4);
    // Original daycare HPExp offset, then the omitted ds78 in progress flags.
    bytes.drain(0x2d1c..0x2d26);
    bytes.splice(0x2cdc..0x2cdc, [0u8; 2]);
    bytes.drain(0x2914..0x2964);
    bytes.resize(32768, 0);
    // Preserve bank boundaries: only bank1 layout shifted in historical saves.
    let mut old = original_sram();
    old[0x2000..0x4000].copy_from_slice(&bytes[0x2000..0x4000]);
    old[0x34cb] = pokered_core::save_menu::calc_checksum(&old[0x2598..0x34cb]);
    let save = import_sram(&old).unwrap();
    assert!(save.imported_legacy_native);
    assert_eq!(save.party.get(0).unwrap().species, Species::Pikachu);
    assert_eq!(save.party.get(0).unwrap().ot_id, 1234);
    assert_eq!(save.game_data.safari_steps, 123);
}

#[test]
fn safari_and_original_status_aliases_resume_allowances() {
    let mut data = pokered_core::save::game_data::GameData::new();
    data.safari_steps = 123;
    data.num_safari_balls = 7;
    data.status_flags[0] = 0x38;
    let mut ow = pokered_core::overworld::OverworldScreen::new(
        pokered_data::maps::MapId::SafariZoneCenter,
        None,
        pokered_data::impl_traits::PokemonRedData,
    );
    ow.set_flag_live("EVENT_IN_SAFARI_ZONE", true);
    ow.restore_system_save_state(&data);
    assert!(ow.is_safari_game_active());
    assert_eq!(ow.safari_steps_remaining(), 123);
    assert_eq!(ow.safari_balls_remaining(), 7);
    assert!(ow.unified_flags().get_flag("EVENT_GOT_SUPER_ROD"));
    ow.use_safari_ball();
    ow.write_system_save_state(&mut data);
    assert_eq!(data.num_safari_balls, 6);
    assert_eq!(data.status_flags[0] & 0x38, 0x38);
    data.event_flags = ow.unified_flags().as_bytes().to_vec();
    let mut save = SaveData::new();
    save.game_data = data;
    let save = import_sram(&export_sram(&save)).unwrap();
    let mut resumed = pokered_core::overworld::OverworldScreen::new(
        pokered_data::maps::MapId::SafariZoneCenter,
        None,
        pokered_data::impl_traits::PokemonRedData,
    );
    resumed.set_event_flags_bytes(&save.game_data.event_flags);
    resumed.restore_system_save_state(&save.game_data);
    assert!(resumed.is_safari_game_active());
    assert_eq!(resumed.safari_steps_remaining(), 123);
    assert_eq!(resumed.safari_balls_remaining(), 6);
}

#[test]
fn every_original_npc_trade_completion_bit_survives_sram() {
    let names = [
        "TERRY",
        "MARCEL",
        "CHIKUCHIKU",
        "SAILOR",
        "DUX",
        "MARC",
        "LOLA",
        "DORIS",
        "CRINKLES",
        "SPOT",
    ];
    for index in 0..10 {
        let mut bytes = original_sram();
        bytes[0x29e3..0x29e5].copy_from_slice(&(1u16 << index).to_le_bytes());
        bytes[0x3523] = pokered_core::save_menu::calc_checksum(&bytes[0x2598..0x3523]);
        let mut save = import_sram(&bytes).unwrap();
        let mut ow = pokered_core::overworld::OverworldScreen::new(
            pokered_data::maps::MapId::Route2TradeHouse,
            None,
            pokered_data::impl_traits::PokemonRedData,
        );
        ow.restore_system_save_state(&save.game_data);
        for (at, name) in names.iter().enumerate() {
            assert_eq!(
                ow.unified_flags()
                    .get_flag(&format!("EVENT_TRADED_FOR_{name}")),
                at == index
            );
        }
        assert_eq!(
            ow.unified_flags()
                .get_flag("EVENT_GOT_LICKITUNG_FROM_TRADE"),
            index == 5
        );
        ow.write_system_save_state(&mut save.game_data);
        let output = export_sram(&save);
        assert_eq!(&output[0x29e3..0x29e5], &(1u16 << index).to_le_bytes());
    }
}

#[test]
fn sram_flags_are_authoritative_and_only_old_native_recovers_companion_aliases() {
    for legacy in [false, true] {
        let mut save = SaveData::new();
        save.imported_legacy_native = legacy;
        save.game_data.completed_in_game_trade_flags = 2;
        save.game_data.status_flags[0] = 8;
        save.game_data.event_flags[0x25 / 8] |= 1 << (0x25 % 8); // original EVENT_GOT_POKEDEX
        let mut extras = pokered_core::hash_compat::HashMap::default();
        for (name, value) in [
            ("EVENT_GOT_POKEDEX", false),
            ("EVENT_TRADED_FOR_MARCEL", false),
            ("EVENT_TRADED_FOR_SAILOR", true),
            ("EVENT_GOT_GOOD_ROD", true),
            ("RUNTIME_UNKNOWN", true),
        ] {
            extras.insert(name.to_string(), value);
        }
        let mut ow = pokered_core::overworld::OverworldScreen::new(
            pokered_data::maps::MapId::CeruleanPokecenter,
            None,
            pokered_data::impl_traits::PokemonRedData,
        );
        ow.restore_loaded_save_flags(&save, Some(extras));
        assert!(ow.unified_flags().get_flag("EVENT_GOT_POKEDEX"));
        assert!(ow.unified_flags().get_flag("EVENT_TRADED_FOR_MARCEL"));
        assert!(ow.unified_flags().get_flag("EVENT_GOT_OLD_ROD"));
        assert_eq!(
            ow.unified_flags().get_flag("EVENT_TRADED_FOR_SAILOR"),
            legacy
        );
        assert_eq!(ow.unified_flags().get_flag("EVENT_GOT_GOOD_ROD"), legacy);
        assert!(ow.unified_flags().get_flag("RUNTIME_UNKNOWN"));
        ow.write_system_save_state(&mut save.game_data);
        let reloaded = import_sram(&export_sram(&save)).unwrap();
        assert!(!reloaded.imported_legacy_native);
        assert_eq!(
            reloaded.game_data.completed_in_game_trade_flags,
            if legacy { 10 } else { 2 }
        );
        assert_eq!(
            reloaded.game_data.status_flags[0] & 0x38,
            if legacy { 24 } else { 8 }
        );
    }
}

#[test]
fn full_party_capture_is_visible_in_pc_and_survives_save() {
    for active_box in 0..12 {
        let mut save = SaveData::new();
        let mon = create_pokemon(Species::Pikachu, 25, [0x99, 0x88]).unwrap();
        for _ in 0..6 {
            save.party.add(mon).unwrap();
        }
        for index in 0..12 {
            if index != active_box {
                save.pc_storage
                    .get_box_mut(index)
                    .unwrap()
                    .deposit(
                        create_pokemon(Species::Pidgey, 10 + index as u8, [0x99, 0x88]).unwrap(),
                    )
                    .unwrap();
            }
        }
        save.pc_storage.change_box(active_box).unwrap();
        save.game_data.current_box_num = active_box as u8 | 0x80;
        save.sync_current_box_from_storage();
        let mut battle = pokered_core::battle::BattleScreen::new(true);
        battle.captured_mon = Some(create_pokemon(Species::Abra, 15, [0x88, 0x88]).unwrap());
        let mut ow = pokered_core::overworld::OverworldScreen::new(
            pokered_data::maps::MapId::Route1,
            None,
            pokered_data::impl_traits::PokemonRedData,
        );
        pokered_core::battle::settlement::settle_battle_into_save(&mut battle, &mut save, &mut ow);
        assert_eq!(save.current_box.count(), 1);
        assert_eq!(save.pc_storage.current_box().count(), 1);
        assert_eq!(ow.box_count, 1);
        let mut restored = import_sram(&export_sram(&save)).unwrap();
        assert_eq!(restored.pc_storage.current_box_index(), active_box);
        assert_eq!(
            restored.pc_storage.current_box().get(0).unwrap().species,
            Species::Abra
        );
        let got = restored.pc_storage.withdraw_from_current(0).unwrap();
        restored.sync_current_box_from_storage();
        assert_eq!(got.species, Species::Abra);
        assert_eq!(restored.current_box.count(), 0);
        for index in 0..12 {
            restored.pc_storage.change_box(index).unwrap();
            restored.sync_current_box_from_storage();
            assert_eq!(
                restored.current_box.count(),
                usize::from(index != active_box)
            );
            if index != active_box {
                assert_eq!(restored.current_box.get(0).unwrap().level, 10 + index as u8);
            }
        }
    }
}

#[test]
fn colosseum_heals_when_battle_returns() {
    let mut save = SaveData::new();
    let mut mon = create_pokemon(Species::Pikachu, 25, [0x99, 0x88]).unwrap();
    mon.hp = 1;
    mon.pp[0] = 0;
    mon.status = pokered_core::battle::state::StatusCondition::Poison;
    save.party.add(mon).unwrap();
    let mut battle = pokered_core::battle::BattleScreen::new(false);
    battle.link_mode = true;
    let mut ow = pokered_core::overworld::OverworldScreen::new(
        pokered_data::maps::MapId::Colosseum,
        None,
        pokered_data::impl_traits::PokemonRedData,
    );
    pokered_core::battle::settlement::settle_battle_into_save(&mut battle, &mut save, &mut ow);
    let got = save.party.get(0).unwrap();
    assert_eq!(got.hp, got.max_hp);
    assert!(got.status.is_none());
    assert!(got.pp[0] > 0);
}

#[test]
fn every_original_main_data_boundary_matches_rgbds_symbols() {
    let mut gd = pokered_core::save::game_data::GameData::new();
    gd.game_progress_flags.fill(0xA5);
    gd.safari_steps = 0x1234;
    gd.num_safari_balls = 7;
    gd.status_flags[0] = 0x38;
    gd.completed_in_game_trade_flags = 2;
    gd.grass_rate = 0xFA;
    gd.grass_mons.fill(0xAB);
    gd.water_rate = 0xFB;
    gd.water_mons.fill(0xCD);
    gd.trainer_header_ptr = 0x5678;
    gd.daycare.species = Species::Pikachu as u8;
    gd.daycare.hp_exp = 54321;
    let mut bytes = vec![0; 0x25a3];
    serialize_game_data_into(&gd, &mut bytes);
    assert_eq!(bytes.len(), 0x2d2c); // sSpriteData starts here
    assert_eq!(&bytes[0x289c..0x2914], &[0xA5; 120]);
    assert_eq!(&bytes[0x2914..0x2964], &[0; 80]); // final two scripts + ds78
    assert_eq!(&bytes[0x29b9..0x29bb], &[0x12, 0x34]);
    assert_eq!(bytes[0x29d4], 0x38);
    assert_eq!(&bytes[0x29e3..0x29e5], &[2, 0]);
    assert_eq!(bytes[0x2b33], 0xFA);
    assert_eq!(&bytes[0x2b34..0x2b48], &[0xAB; 20]);
    assert_eq!(bytes[0x2b50], 0xFB);
    assert_eq!(&bytes[0x2b51..0x2b65], &[0xCD; 20]);
    assert_eq!(&bytes[0x2cdc..0x2cde], &[0x56, 0x78]);
    assert_eq!(bytes[0x2cf3], 7);
    assert_eq!(bytes[0x2d0b], 0x54);
    assert_eq!(&bytes[0x2d1c..0x2d1e], &54321u16.to_be_bytes());
}

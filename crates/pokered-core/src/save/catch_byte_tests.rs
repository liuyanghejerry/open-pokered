use super::ser_pokemon::{
    deserialize_box_mon, deserialize_party_mon, serialize_box_mon, serialize_party_mon,
};
use crate::alloc_prelude::*;
use crate::battle::state::Pokemon;
use crate::pokemon::pc_box::PcBox;
use crate::pokemon::stats::create_pokemon;
use pokered_data::species::Species;

// First mon from actual original ordinary SAVE used by audit851. Its native
// ordinary START/SAVE/YES replay previously changed only byte7 45 -> 0.
const SOURCE_PARTY: [u8; 44] = [
    153, 0, 1, 12, 0, 22, 3, 45, 33, 45, 0, 0, 0, 0, 0, 3, 205, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 154,
    120, 35, 40, 0, 0, 12, 0, 35, 0, 18, 0, 19, 0, 17, 0, 22,
];
fn party_bytes(mon: &Pokemon) -> Vec<u8> {
    let mut b = Vec::new();
    serialize_party_mon(mon, &mut b);
    b
}
#[test]
fn catch_byte_party_box_and_json_roundtrip_keep_zero_base_and_nonbase_values() {
    for value in [0, 45, 173] {
        let mut expected = SOURCE_PARTY;
        expected[7] = value;
        let mon = deserialize_party_mon(&expected).unwrap();
        assert_eq!(mon.catch_rate, Some(value));
        assert_eq!(party_bytes(&mon), expected);
        let box_mon = deserialize_box_mon(&expected[..33]).unwrap();
        let mut bytes = Vec::new();
        serialize_box_mon(&box_mon, &mut bytes);
        assert_eq!(bytes, expected[..33]);
        let json = serde_json::to_string(&mon).unwrap();
        let from_json: Pokemon = serde_json::from_str(&json).unwrap();
        assert_eq!(party_bytes(&from_json), expected);
    }
}
#[test]
fn catch_byte_legacy_json_absence_derives_base_and_new_native_stores_base() {
    let native = create_pokemon(Species::Bulbasaur, 12, [0x9a, 0x78]).unwrap();
    assert_eq!(native.catch_rate, Some(45));
    assert_eq!(party_bytes(&native)[7], 45);
    let mut json = serde_json::to_value(native).unwrap();
    json.as_object_mut().unwrap().remove("catch_rate");
    let legacy: Pokemon = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(legacy.catch_rate, None);
    assert_eq!(party_bytes(&legacy)[7], 45);
    json["catch_rate"] = serde_json::json!(0);
    let explicit: Pokemon = serde_json::from_value(json).unwrap();
    assert_eq!(explicit.catch_rate, Some(0));
    assert_eq!(party_bytes(&explicit)[7], 0);
}
#[test]
fn catch_byte_pc_stat_engine_and_evolution_preserve_instance_value() {
    for value in [0, 173] {
        let mut mon = create_pokemon(Species::Magikarp, 20, [0x9a, 0x78]).unwrap();
        mon.catch_rate = Some(value);
        let mut pc = PcBox::new();
        pc.deposit(mon).unwrap();
        let mut moved = pc.withdraw(0).unwrap();
        assert_eq!(moved.catch_rate, Some(value));
        crate::pokemon::stats::recalculate_stats(&mut moved);
        assert_eq!(moved.catch_rate, Some(value));
        let instance = moved.to_monster_instance();
        moved.apply_monster_instance(&instance);
        assert_eq!(moved.catch_rate, Some(value));
        crate::battle::settlement::evolution::apply_evolution(&mut moved, Species::Gyarados);
        assert_eq!(party_bytes(&moved)[7], value);
    }
    let mut legacy = create_pokemon(Species::Magikarp, 20, [0x9a, 0x78]).unwrap();
    legacy.catch_rate = None;
    crate::battle::settlement::evolution::apply_evolution(&mut legacy, Species::Gyarados);
    assert_eq!(legacy.catch_rate, Some(255));
    assert_eq!(party_bytes(&legacy)[7], 255);
    let mut via_engine = create_pokemon(Species::Magikarp, 20, [0x9a, 0x78]).unwrap();
    via_engine.catch_rate = None;
    let mut instance = via_engine.to_monster_instance();
    instance.species = Species::Gyarados;
    via_engine.apply_monster_instance(&instance);
    assert_eq!(via_engine.catch_rate, Some(255));
}
#[test]
fn catch_byte_sram_party_and_box_remain_exact_after_export_import_export() {
    let mut save = super::SaveData::new();
    let mut party = create_pokemon(Species::Bulbasaur, 12, [0x9a, 0x78]).unwrap();
    party.catch_rate = Some(0);
    save.party.add(party).unwrap();
    let mut boxed = create_pokemon(Species::Pikachu, 15, [0x9a, 0x78]).unwrap();
    boxed.catch_rate = Some(173);
    save.current_box.deposit(boxed).unwrap();
    let sram = super::sram_export::export_sram(&save);
    let loaded = super::sram_import::import_sram(&sram).unwrap();
    assert_eq!(loaded.party.get(0).unwrap().catch_rate, Some(0));
    assert_eq!(loaded.current_box.get(0).unwrap().catch_rate, Some(173));
    let again = super::sram_export::export_sram(&loaded);
    assert_eq!(sram, again);
}

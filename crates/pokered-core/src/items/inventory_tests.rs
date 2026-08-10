use super::inventory::*;
use pokered_data::items::ItemId;

#[test]
fn new_bag_has_correct_capacity() {
    let bag = Inventory::new_bag();
    assert_eq!(bag.capacity(), BAG_ITEM_CAPACITY);
    assert_eq!(bag.count(), 0);
    assert!(bag.is_empty());
    assert!(!bag.is_full());
}

#[test]
fn new_pc_has_correct_capacity() {
    let pc = Inventory::new_pc();
    assert_eq!(pc.capacity(), PC_ITEM_CAPACITY);
    assert!(pc.is_empty());
}

#[test]
fn add_single_item() {
    let mut bag = Inventory::new_bag();
    assert!(bag.add_item(ItemId::Potion, 5).is_ok());
    assert_eq!(bag.count(), 1);
    assert_eq!(bag.get(0), Some((ItemId::Potion, 5)));
}

#[test]
fn add_stacks_same_item() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 10).unwrap();
    bag.add_item(ItemId::Potion, 5).unwrap();
    assert_eq!(bag.count(), 1);
    assert_eq!(bag.get(0), Some((ItemId::Potion, 15)));
}

#[test]
fn add_different_items_separate_slots() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 3).unwrap();
    bag.add_item(ItemId::Antidote, 2).unwrap();
    assert_eq!(bag.count(), 2);
    assert_eq!(bag.get(0), Some((ItemId::Potion, 3)));
    assert_eq!(bag.get(1), Some((ItemId::Antidote, 2)));
}

#[test]
fn add_overflow_at_99_spills_to_new_slot() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 95).unwrap();
    bag.add_item(ItemId::Potion, 10).unwrap();
    assert_eq!(bag.count(), 2);
    assert_eq!(bag.get(0), Some((ItemId::Potion, 99)));
    assert_eq!(bag.get(1), Some((ItemId::Potion, 6)));
}

#[test]
fn add_exactly_99() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 99).unwrap();
    assert_eq!(bag.count(), 1);
    assert_eq!(bag.get(0), Some((ItemId::Potion, 99)));
}

#[test]
fn add_zero_quantity_fails() {
    let mut bag = Inventory::new_bag();
    assert_eq!(
        bag.add_item(ItemId::Potion, 0),
        Err(InventoryError::ZeroQuantity)
    );
}

#[test]
fn bag_full_rejects_new_item() {
    let mut bag = Inventory::new_bag();
    for i in 0..BAG_ITEM_CAPACITY {
        let item = match i % 3 {
            0 => ItemId::Potion,
            1 => ItemId::Antidote,
            _ => ItemId::BurnHeal,
        };
        bag.add_item(item, 1).unwrap();
    }
    let mut bag2 = Inventory::new_bag();
    let items = [
        ItemId::Potion,
        ItemId::SuperPotion,
        ItemId::HyperPotion,
        ItemId::MaxPotion,
        ItemId::FullRestore,
        ItemId::Antidote,
        ItemId::BurnHeal,
        ItemId::IceHeal,
        ItemId::Awakening,
        ItemId::ParlyzHeal,
        ItemId::FullHeal,
        ItemId::Revive,
        ItemId::MaxRevive,
        ItemId::Ether,
        ItemId::MaxEther,
        ItemId::Elixer,
        ItemId::MaxElixer,
        ItemId::HpUp,
        ItemId::Protein,
        ItemId::Iron,
    ];
    for &item in &items {
        bag2.add_item(item, 1).unwrap();
    }
    assert!(bag2.is_full());
    assert_eq!(
        bag2.add_item(ItemId::Carbos, 1),
        Err(InventoryError::InventoryFull)
    );
}

#[test]
fn bag_full_but_can_stack_existing() {
    let mut bag = Inventory::new_bag();
    let items = [
        ItemId::Potion,
        ItemId::SuperPotion,
        ItemId::HyperPotion,
        ItemId::MaxPotion,
        ItemId::FullRestore,
        ItemId::Antidote,
        ItemId::BurnHeal,
        ItemId::IceHeal,
        ItemId::Awakening,
        ItemId::ParlyzHeal,
        ItemId::FullHeal,
        ItemId::Revive,
        ItemId::MaxRevive,
        ItemId::Ether,
        ItemId::MaxEther,
        ItemId::Elixer,
        ItemId::MaxElixer,
        ItemId::HpUp,
        ItemId::Protein,
        ItemId::Iron,
    ];
    for &item in &items {
        bag.add_item(item, 1).unwrap();
    }
    assert!(bag.is_full());
    assert!(bag.add_item(ItemId::Potion, 5).is_ok());
    assert_eq!(bag.get(0), Some((ItemId::Potion, 6)));
}

#[test]
fn remove_item_at_partial() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 10).unwrap();
    bag.remove_item_at(0, 3).unwrap();
    assert_eq!(bag.get(0), Some((ItemId::Potion, 7)));
    assert_eq!(bag.count(), 1);
}

#[test]
fn remove_item_at_all_removes_slot() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 5).unwrap();
    bag.add_item(ItemId::Antidote, 3).unwrap();
    bag.remove_item_at(0, 5).unwrap();
    assert_eq!(bag.count(), 1);
    assert_eq!(bag.get(0), Some((ItemId::Antidote, 3)));
}

#[test]
fn remove_item_at_shifts_subsequent_slots() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 5).unwrap();
    bag.add_item(ItemId::Antidote, 3).unwrap();
    bag.add_item(ItemId::BurnHeal, 1).unwrap();
    bag.remove_item_at(1, 3).unwrap();
    assert_eq!(bag.count(), 2);
    assert_eq!(bag.get(0), Some((ItemId::Potion, 5)));
    assert_eq!(bag.get(1), Some((ItemId::BurnHeal, 1)));
}

#[test]
fn remove_item_at_not_enough_fails() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 3).unwrap();
    assert_eq!(
        bag.remove_item_at(0, 5),
        Err(InventoryError::NotEnoughItems)
    );
}

#[test]
fn remove_item_at_oob_fails() {
    let mut bag = Inventory::new_bag();
    assert_eq!(
        bag.remove_item_at(0, 1),
        Err(InventoryError::IndexOutOfBounds)
    );
}

#[test]
fn remove_item_at_zero_fails() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 5).unwrap();
    assert_eq!(bag.remove_item_at(0, 0), Err(InventoryError::ZeroQuantity));
}

#[test]
fn remove_item_by_id() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 10).unwrap();
    bag.add_item(ItemId::Antidote, 5).unwrap();
    bag.remove_item(ItemId::Antidote, 2).unwrap();
    assert_eq!(bag.item_quantity(ItemId::Antidote), 3);
}

#[test]
fn remove_item_by_id_not_found() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 10).unwrap();
    assert_eq!(
        bag.remove_item(ItemId::Antidote, 1),
        Err(InventoryError::ItemNotFound)
    );
}

#[test]
fn toss_item_works() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 10).unwrap();
    bag.toss_item(0, 10).unwrap();
    assert!(bag.is_empty());
}

// -- Key-item toss guard ("That's too important to toss!") -------------------

#[test]
fn key_items_cannot_be_tossed() {
    for item in [
        ItemId::Bicycle,
        ItemId::SilphScope,
        ItemId::PokeFlute,
        ItemId::TownMap,
        ItemId::Hm01,
        ItemId::Hm03,
        ItemId::BoulderBadge,
    ] {
        let mut bag = Inventory::new_bag();
        bag.add_item(item, 1).unwrap();
        assert_eq!(
            bag.toss_item(0, 1),
            Err(InventoryError::KeyItem),
            "{item:?} must refuse tossing"
        );
        assert_eq!(bag.item_quantity(item), 1, "{item:?} stays in the bag");
    }
}

#[test]
fn tms_and_medicine_are_tossable() {
    for item in [ItemId::Tm34, ItemId::Potion, ItemId::FireStone, ItemId::Nugget] {
        let mut bag = Inventory::new_bag();
        bag.add_item(item, 2).unwrap();
        assert!(bag.toss_item(0, 1).is_ok(), "{item:?} is tossable");
        assert_eq!(bag.item_quantity(item), 1);
    }
}

#[test]
fn is_tossable_matches_toss_guard() {
    assert!(!super::inventory::is_tossable(ItemId::Bicycle));
    assert!(!super::inventory::is_tossable(ItemId::Hm05));
    assert!(!super::inventory::is_tossable(ItemId::OaksParcel));
    assert!(super::inventory::is_tossable(ItemId::Potion));
    assert!(super::inventory::is_tossable(ItemId::Tm01));
    assert!(super::inventory::is_tossable(ItemId::RareCandy));
}

#[test]
fn swap_items() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 5).unwrap();
    bag.add_item(ItemId::Antidote, 3).unwrap();
    bag.swap(0, 1).unwrap();
    assert_eq!(bag.get(0), Some((ItemId::Antidote, 3)));
    assert_eq!(bag.get(1), Some((ItemId::Potion, 5)));
}

#[test]
fn swap_same_index_fails() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 5).unwrap();
    assert_eq!(bag.swap(0, 0), Err(InventoryError::SameIndex));
}

#[test]
fn swap_oob_fails() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 5).unwrap();
    assert_eq!(bag.swap(0, 5), Err(InventoryError::IndexOutOfBounds));
}

#[test]
fn has_item_true() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 10).unwrap();
    assert!(bag.has_item(ItemId::Potion, 10));
    assert!(bag.has_item(ItemId::Potion, 1));
}

#[test]
fn has_item_false() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 5).unwrap();
    assert!(!bag.has_item(ItemId::Potion, 6));
    assert!(!bag.has_item(ItemId::Antidote, 1));
}

#[test]
fn has_item_across_multiple_slots() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 99).unwrap();
    bag.add_item(ItemId::Potion, 50).unwrap();
    assert!(bag.has_item(ItemId::Potion, 149));
    assert!(!bag.has_item(ItemId::Potion, 150));
}

#[test]
fn item_quantity_sums_across_slots() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 99).unwrap();
    bag.add_item(ItemId::Potion, 30).unwrap();
    assert_eq!(bag.item_quantity(ItemId::Potion), 129);
}

#[test]
fn find_item_returns_first_slot() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Antidote, 3).unwrap();
    bag.add_item(ItemId::Potion, 5).unwrap();
    assert_eq!(bag.find_item(ItemId::Potion), Some(1));
    assert_eq!(bag.find_item(ItemId::BurnHeal), None);
}

#[test]
fn use_item_decrements_and_returns_id() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 3).unwrap();
    let id = bag.use_item(0).unwrap();
    assert_eq!(id, ItemId::Potion);
    assert_eq!(bag.get(0), Some((ItemId::Potion, 2)));
}

#[test]
fn use_item_removes_slot_at_zero() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 1).unwrap();
    bag.use_item(0).unwrap();
    assert!(bag.is_empty());
}

#[test]
fn use_item_oob_fails() {
    let mut bag = Inventory::new_bag();
    assert_eq!(bag.use_item(0), Err(InventoryError::IndexOutOfBounds));
}

#[test]
fn clear_empties_inventory() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 10).unwrap();
    bag.add_item(ItemId::Antidote, 5).unwrap();
    bag.clear();
    assert!(bag.is_empty());
    assert_eq!(bag.count(), 0);
}

#[test]
fn pc_capacity_50_slots() {
    let mut pc = Inventory::new_pc();
    let items = [
        ItemId::Potion,
        ItemId::SuperPotion,
        ItemId::HyperPotion,
        ItemId::MaxPotion,
        ItemId::FullRestore,
        ItemId::Antidote,
        ItemId::BurnHeal,
        ItemId::IceHeal,
        ItemId::Awakening,
        ItemId::ParlyzHeal,
        ItemId::FullHeal,
        ItemId::Revive,
        ItemId::MaxRevive,
        ItemId::Ether,
        ItemId::MaxEther,
        ItemId::Elixer,
        ItemId::MaxElixer,
        ItemId::HpUp,
        ItemId::Protein,
        ItemId::Iron,
        ItemId::Carbos,
        ItemId::Calcium,
        ItemId::RareCandy,
        ItemId::Nugget,
        ItemId::EscapeRope,
        ItemId::Repel,
        ItemId::SuperRepel,
        ItemId::MaxRepel,
        ItemId::GuardSpec,
        ItemId::DireHit,
        ItemId::XAttack,
        ItemId::XDefend,
        ItemId::XSpeed,
        ItemId::XSpecial,
        ItemId::XAccuracy,
        ItemId::PokeDoll,
        ItemId::FreshWater,
        ItemId::SodaPop,
        ItemId::Lemonade,
        ItemId::PpUp,
        ItemId::MasterBall,
        ItemId::UltraBall,
        ItemId::GreatBall,
        ItemId::PokeBall,
        ItemId::MoonStone,
        ItemId::FireStone,
        ItemId::ThunderStone,
        ItemId::WaterStone,
        ItemId::LeafStone,
        ItemId::Coin,
    ];
    assert_eq!(items.len(), 50);
    for &item in &items {
        pc.add_item(item, 1).unwrap();
    }
    assert!(pc.is_full());
    assert_eq!(
        pc.add_item(ItemId::SafariBall, 1),
        Err(InventoryError::InventoryFull)
    );
}

#[test]
fn items_slice_matches_internal_state() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 5).unwrap();
    bag.add_item(ItemId::Antidote, 3).unwrap();
    let items = bag.items();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0], (ItemId::Potion, 5));
    assert_eq!(items[1], (ItemId::Antidote, 3));
}

#[test]
fn serde_roundtrip_preserves_shape() {
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 5).unwrap();
    bag.add_item(ItemId::Antidote, 3).unwrap();
    let json = serde_json::to_string(&bag).unwrap();
    // Same JSON shape as the pre-fixed-capacity derive: items list + capacity.
    assert_eq!(
        json,
        r#"{"items":[["Potion",5],["Antidote",3]],"capacity":20}"#
    );
    let back: Inventory<BAG_ITEM_CAPACITY> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, bag);
    assert_eq!(back.capacity(), BAG_ITEM_CAPACITY);
}

#[test]
fn serde_loads_legacy_json_shape() {
    // Old saves written by the Vec-backed format carry the same shape
    // (`capacity` matches the const generic) and must load unchanged.
    let json = r#"{"items":[["Potion",99],["PokeBall",10]],"capacity":20}"#;
    let bag: Inventory<BAG_ITEM_CAPACITY> = serde_json::from_str(json).unwrap();
    assert_eq!(bag.count(), 2);
    assert_eq!(bag.get(0), Some((ItemId::Potion, 99)));
    assert_eq!(bag.get(1), Some((ItemId::PokeBall, 10)));
}

#[test]
fn serde_rejects_more_items_than_capacity() {
    let json = r#"{"items":[["Potion",1],["Antidote",1],["PokeBall",1]],"capacity":2}"#;
    let err = serde_json::from_str::<Inventory<2>>(json).unwrap_err();
    assert!(err.is_data());
}

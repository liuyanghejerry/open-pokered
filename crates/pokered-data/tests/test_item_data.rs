use pokered_data::item_data::{get_item_data, ITEM_DATA, TM_PRICES};
use pokered_data::items::{ItemId, HM_MOVES, NUM_HMS, NUM_TMS, TM_MOVES};
use strum::IntoEnumIterator;

#[test]
fn item_data_has_83_entries() {
    assert_eq!(ITEM_DATA.len(), 83);
}

#[test]
fn tm_prices_has_50_entries() {
    assert_eq!(TM_PRICES.len(), 50);
}

#[test]
fn tm_moves_has_50_entries() {
    assert_eq!(TM_MOVES.len(), NUM_TMS as usize);
}

#[test]
fn hm_moves_has_5_entries() {
    assert_eq!(HM_MOVES.len(), NUM_HMS as usize);
}

#[test]
fn every_item_has_data() {
    for item in ItemId::iter() {
        if item == ItemId::NoItem {
            assert!(get_item_data(item).is_none());
            continue;
        }
        let val = item as u8;
        if val > 83 {
            continue;
        }
        let data =
            get_item_data(item).unwrap_or_else(|| panic!("Missing item data for {:?}", item));
        assert_eq!(data.id, item);
    }
}

#[test]
fn item_data_ordered() {
    for (i, item) in ITEM_DATA.iter().enumerate() {
        let expected = (i + 1) as u8;
        assert_eq!(
            item.id as u8, expected,
            "ITEM_DATA[{}] has id {:?} ({}), expected {}",
            i, item.id, item.id as u8, expected
        );
    }
}

#[test]
fn item_names_nonempty() {
    for item in ITEM_DATA.iter() {
        assert!(!item.name.is_empty(), "{:?} has empty name", item.id);
    }
}

#[test]
fn key_items_consistency() {
    let key_items_with_price: Vec<_> = ITEM_DATA
        .iter()
        .filter(|item| item.is_key_item && item.price > 0)
        .collect();
    assert_eq!(
        key_items_with_price.len(),
        1,
        "Only SafariBall should be a key item with nonzero price"
    );
    assert_eq!(key_items_with_price[0].id, ItemId::SafariBall);
}

#[test]
fn tm_prices_all_positive() {
    for (i, &price) in TM_PRICES.iter().enumerate() {
        assert!(price > 0, "TM{:02} has price 0", i + 1);
    }
}

#[test]
fn spot_check_master_ball() {
    let mb = get_item_data(ItemId::MasterBall).unwrap();
    assert_eq!(mb.name, "MASTER BALL");
    assert_eq!(mb.price, 0);
    assert!(!mb.is_key_item);
}

#[test]
fn spot_check_poke_ball_price() {
    let pb = get_item_data(ItemId::PokeBall).unwrap();
    assert_eq!(pb.price, 200);
}

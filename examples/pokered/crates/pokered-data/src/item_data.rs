use crate::items::ItemId;
use crate::items::ShopId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemData {
    pub id: ItemId,
    pub name: &'static str,
    pub price: u16,
    pub is_key_item: bool,
}

/// Get item data by ItemId. Returns None for NoItem.
pub fn get_item_data(id: ItemId) -> Option<&'static ItemData> {
    // Editor-injected runtime override shadows the baseline.
    if let Some(ov) = crate::runtime_overrides::item_override(id) {
        return Some(ov);
    }
    let idx = id as usize;
    if idx == 0 || idx > ITEM_DATA.len() {
        None
    } else {
        Some(&ITEM_DATA[idx - 1])
    }
}

include!(concat!(env!("OUT_DIR"), "/item_data_gen.rs"));

// ── TM prices ──────────────────────────────────────────────────────────

/// TM prices in Pokédollars (thousands * 1000)
pub const TM_PRICES: [u16; 50] = [
    3000, // TM01
    2000, // TM02
    2000, // TM03
    1000, // TM04
    3000, // TM05
    4000, // TM06
    2000, // TM07
    4000, // TM08
    3000, // TM09
    4000, // TM10
    2000, // TM11
    1000, // TM12
    4000, // TM13
    5000, // TM14
    5000, // TM15
    5000, // TM16
    3000, // TM17
    2000, // TM18
    3000, // TM19
    2000, // TM20
    5000, // TM21
    5000, // TM22
    5000, // TM23
    2000, // TM24
    5000, // TM25
    4000, // TM26
    5000, // TM27
    2000, // TM28
    4000, // TM29
    1000, // TM30
    2000, // TM31
    1000, // TM32
    1000, // TM33
    2000, // TM34
    4000, // TM35
    2000, // TM36
    2000, // TM37
    5000, // TM38
    2000, // TM39
    4000, // TM40
    2000, // TM41
    2000, // TM42
    5000, // TM43
    2000, // TM44
    2000, // TM45
    4000, // TM46
    3000, // TM47
    4000, // TM48
    4000, // TM49
    2000, // TM50
];

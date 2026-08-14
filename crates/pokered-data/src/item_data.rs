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
///
/// TM/HM ids ($C4–$FA) are synthesized from `TMHM_DATA` — the reference
/// generates "TM01".."TM50" / "HM01".."HM05" names at runtime
/// (home/names.asm:51-91) and prices from `data/items/tm_prices.asm`.
pub fn get_item_data(id: ItemId) -> Option<&'static ItemData> {
    // Editor-injected runtime override shadows the baseline.
    if let Some(ov) = crate::runtime_overrides::item_override(id) {
        return Some(ov);
    }
    let idx = id as usize;
    if idx == 0 {
        return None;
    }
    if idx <= ITEM_DATA.len() {
        return Some(&ITEM_DATA[idx - 1]);
    }
    // TM01..TM50 = $C9..$FA, HM01..HM05 = $C4..$C8.
    if (0xC9..=0xFA).contains(&idx) {
        return Some(&TMHM_DATA[idx - 0xC9]);
    }
    if (0xC4..=0xC8).contains(&idx) {
        return Some(&TMHM_DATA[50 + (idx - 0xC4)]);
    }
    None
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
/// Synthesized TM/HM item definitions (the reference generates these at
/// runtime in home/names.asm:51-91; TMs get their price from tm_prices.asm,
/// HMs are key items with no price). Indices: [0..50) = TM01..TM50,
/// [50..55) = HM01..HM05.
pub const TMHM_DATA: [ItemData; 55] = [
    ItemData { id: ItemId::Tm01, name: "TM01", price: 3000, is_key_item: false },
    ItemData { id: ItemId::Tm02, name: "TM02", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm03, name: "TM03", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm04, name: "TM04", price: 1000, is_key_item: false },
    ItemData { id: ItemId::Tm05, name: "TM05", price: 3000, is_key_item: false },
    ItemData { id: ItemId::Tm06, name: "TM06", price: 4000, is_key_item: false },
    ItemData { id: ItemId::Tm07, name: "TM07", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm08, name: "TM08", price: 4000, is_key_item: false },
    ItemData { id: ItemId::Tm09, name: "TM09", price: 3000, is_key_item: false },
    ItemData { id: ItemId::Tm10, name: "TM10", price: 4000, is_key_item: false },
    ItemData { id: ItemId::Tm11, name: "TM11", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm12, name: "TM12", price: 1000, is_key_item: false },
    ItemData { id: ItemId::Tm13, name: "TM13", price: 4000, is_key_item: false },
    ItemData { id: ItemId::Tm14, name: "TM14", price: 5000, is_key_item: false },
    ItemData { id: ItemId::Tm15, name: "TM15", price: 5000, is_key_item: false },
    ItemData { id: ItemId::Tm16, name: "TM16", price: 5000, is_key_item: false },
    ItemData { id: ItemId::Tm17, name: "TM17", price: 3000, is_key_item: false },
    ItemData { id: ItemId::Tm18, name: "TM18", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm19, name: "TM19", price: 3000, is_key_item: false },
    ItemData { id: ItemId::Tm20, name: "TM20", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm21, name: "TM21", price: 5000, is_key_item: false },
    ItemData { id: ItemId::Tm22, name: "TM22", price: 5000, is_key_item: false },
    ItemData { id: ItemId::Tm23, name: "TM23", price: 5000, is_key_item: false },
    ItemData { id: ItemId::Tm24, name: "TM24", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm25, name: "TM25", price: 5000, is_key_item: false },
    ItemData { id: ItemId::Tm26, name: "TM26", price: 4000, is_key_item: false },
    ItemData { id: ItemId::Tm27, name: "TM27", price: 5000, is_key_item: false },
    ItemData { id: ItemId::Tm28, name: "TM28", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm29, name: "TM29", price: 4000, is_key_item: false },
    ItemData { id: ItemId::Tm30, name: "TM30", price: 1000, is_key_item: false },
    ItemData { id: ItemId::Tm31, name: "TM31", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm32, name: "TM32", price: 1000, is_key_item: false },
    ItemData { id: ItemId::Tm33, name: "TM33", price: 1000, is_key_item: false },
    ItemData { id: ItemId::Tm34, name: "TM34", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm35, name: "TM35", price: 4000, is_key_item: false },
    ItemData { id: ItemId::Tm36, name: "TM36", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm37, name: "TM37", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm38, name: "TM38", price: 5000, is_key_item: false },
    ItemData { id: ItemId::Tm39, name: "TM39", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm40, name: "TM40", price: 4000, is_key_item: false },
    ItemData { id: ItemId::Tm41, name: "TM41", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm42, name: "TM42", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm43, name: "TM43", price: 5000, is_key_item: false },
    ItemData { id: ItemId::Tm44, name: "TM44", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm45, name: "TM45", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Tm46, name: "TM46", price: 4000, is_key_item: false },
    ItemData { id: ItemId::Tm47, name: "TM47", price: 3000, is_key_item: false },
    ItemData { id: ItemId::Tm48, name: "TM48", price: 4000, is_key_item: false },
    ItemData { id: ItemId::Tm49, name: "TM49", price: 4000, is_key_item: false },
    ItemData { id: ItemId::Tm50, name: "TM50", price: 2000, is_key_item: false },
    ItemData { id: ItemId::Hm01, name: "HM01", price: 0, is_key_item: true },
    ItemData { id: ItemId::Hm02, name: "HM02", price: 0, is_key_item: true },
    ItemData { id: ItemId::Hm03, name: "HM03", price: 0, is_key_item: true },
    ItemData { id: ItemId::Hm04, name: "HM04", price: 0, is_key_item: true },
    ItemData { id: ItemId::Hm05, name: "HM05", price: 0, is_key_item: true },
];
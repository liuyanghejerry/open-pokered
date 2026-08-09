use jrpg_engine::items::Inventory as EngineInventory;
use pokered_data::items::ItemId;
use serde::{Deserialize, Serialize};

pub const MAX_ITEM_QUANTITY: u8 = 99;
pub const BAG_ITEM_CAPACITY: usize = 20;
pub const PC_ITEM_CAPACITY: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryError {
    InventoryFull,
    ItemNotFound,
    NotEnoughItems,
    IndexOutOfBounds,
    SameIndex,
    ZeroQuantity,
    QuantityOverflow,
    /// Key items (BICYCLE, SILPH SCOPE, HMs, badges…) cannot be tossed
    /// ("That's too important to toss!").
    KeyItem,
}

/// Gen-1 tossability check: key items, badges, and HMs are "too important to
/// toss" (the original gates the bag TOSS action on the item's key-item
/// flag; HMs live above the item table and are always key).
pub fn is_tossable(item: ItemId) -> bool {
    if item.is_key_item() || item.is_badge() {
        return false;
    }
    !matches!(
        pokered_data::items::item_kind(item),
        jrpg_engine::items::ItemKind::Custom(pokered_data::items::CustomKind::Hm)
            | jrpg_engine::items::ItemKind::KeyItem
    )
}

/// Gen1 item inventory: wraps the engine's generic `Inventory<ItemId>` while
/// preserving pokered-specific capacity limits, overflow-spill semantics, and
/// the existing public API.
///
/// Bag = capacity 20, PC = capacity 50, max 99 per slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    inner: EngineInventory<ItemId>,
}

impl Inventory {
    /// Create an empty inventory with the given slot capacity and the standard
    /// 99-per-slot limit.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: EngineInventory::with_capacity(capacity, MAX_ITEM_QUANTITY as u32),
        }
    }

    pub fn new_bag() -> Self {
        Self::new(BAG_ITEM_CAPACITY)
    }

    pub fn new_pc() -> Self {
        Self::new(PC_ITEM_CAPACITY)
    }

    /// Number of occupied slots (not total item count).
    pub fn count(&self) -> usize {
        self.inner.items.len()
    }

    pub fn capacity(&self) -> usize {
        self.inner.max_slots.unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.inner.items.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.inner.is_full()
    }

    /// Get the (item, quantity) pair at `index`.  Quantity is returned as
    /// `u8` for backward compatibility (quantities never exceed 99 in Gen 1).
    pub fn get(&self, index: usize) -> Option<(ItemId, u8)> {
        self.inner.items.get(index).map(|&(id, qty)| (id, qty as u8))
    }

    /// Raw access to internal `(ItemId, u32)` slots (used by save
    /// serialization).
    pub fn items(&self) -> &[(ItemId, u32)] {
        &self.inner.items
    }

    /// Add `quantity` copies of `item` with Gen-1 overflow semantics: if an
    /// existing slot reaches 99, excess spills into a new slot.
    pub fn add_item(&mut self, item: ItemId, quantity: u8) -> Result<(), InventoryError> {
        if quantity == 0 {
            return Err(InventoryError::ZeroQuantity);
        }

        let mut remaining = quantity as u32;

        // Fill existing slots first
        for slot in self.inner.items.iter_mut() {
            if slot.0 == item && remaining > 0 {
                let space = (MAX_ITEM_QUANTITY as u32).saturating_sub(slot.1);
                if space > 0 {
                    let add = remaining.min(space);
                    slot.1 += add;
                    remaining -= add;
                }
            }
            if remaining == 0 {
                return Ok(());
            }
        }

        // Create new slots for remaining
        while remaining > 0 {
            if self.inner.is_full() {
                return Err(InventoryError::InventoryFull);
            }
            let add = remaining.min(MAX_ITEM_QUANTITY as u32);
            self.inner.items.push((item, add));
            remaining -= add;
        }

        Ok(())
    }

    /// Remove `quantity` from the slot at `index`.  If the slot becomes
    /// empty it is removed entirely (subsequent slots shift left).
    pub fn remove_item_at(&mut self, index: usize, quantity: u8) -> Result<(), InventoryError> {
        if quantity == 0 {
            return Err(InventoryError::ZeroQuantity);
        }
        if index >= self.inner.items.len() {
            return Err(InventoryError::IndexOutOfBounds);
        }
        let current_qty = self.inner.items[index].1;
        let qty = quantity as u32;
        if qty > current_qty {
            return Err(InventoryError::NotEnoughItems);
        }
        let new_qty = current_qty - qty;
        if new_qty == 0 {
            self.inner.items.remove(index);
        } else {
            self.inner.items[index].1 = new_qty;
        }
        Ok(())
    }

    /// Remove `quantity` of `item` from the first matching slot.
    pub fn remove_item(&mut self, item: ItemId, quantity: u8) -> Result<(), InventoryError> {
        if quantity == 0 {
            return Err(InventoryError::ZeroQuantity);
        }
        let index = self
            .inner
            .items
            .iter()
            .position(|&(id, _)| id == item)
            .ok_or(InventoryError::ItemNotFound)?;
        self.remove_item_at(index, quantity)
    }

    /// Toss `quantity` of the item at `index`. Key items (incl. HMs) refuse
    /// with [`InventoryError::KeyItem`] — the caller shows "That's too
    /// important to toss!" (_TooImportantToTossText).
    pub fn toss_item(&mut self, index: usize, quantity: u8) -> Result<(), InventoryError> {
        if let Some(&(id, _)) = self.inner.items.get(index) {
            if !is_tossable(id) {
                return Err(InventoryError::KeyItem);
            }
        }
        self.remove_item_at(index, quantity)
    }

    /// Swap the two slots at indices `a` and `b`.
    pub fn swap(&mut self, a: usize, b: usize) -> Result<(), InventoryError> {
        if a == b {
            return Err(InventoryError::SameIndex);
        }
        let len = self.inner.items.len();
        if a >= len || b >= len {
            return Err(InventoryError::IndexOutOfBounds);
        }
        self.inner.items.swap(a, b);
        Ok(())
    }

    /// Returns `true` if at least `quantity` of `item` is owned (summed
    /// across all slots, matching Gen-1 behaviour).
    pub fn has_item(&self, item: ItemId, quantity: u8) -> bool {
        let total: u32 = self
            .inner
            .items
            .iter()
            .filter(|&&(id, _)| id == item)
            .map(|&(_, qty)| qty)
            .sum();
        total >= quantity as u32
    }

    /// `has_item` by const name (e.g. "FRESH_WATER", "TM34"). Unknown names are
    /// simply absent. Used by the filtered-bag menu to show only carried items.
    pub fn has_item_const(&self, const_name: &str) -> bool {
        pokered_data::items::ItemId::from_const_name(const_name)
            .map_or(false, |id| self.has_item(id, 1))
    }

    /// Total quantity of `item` across all slots.
    pub fn item_quantity(&self, item: ItemId) -> u16 {
        self.inner
            .items
            .iter()
            .filter(|&&(id, _)| id == item)
            .map(|&(_, qty)| qty as u16)
            .sum()
    }

    /// Index of the first slot containing `item`, if any.
    pub fn find_item(&self, item: ItemId) -> Option<usize> {
        self.inner.items.iter().position(|&(id, _)| id == item)
    }

    /// Use one unit of the item at `index` (decrement by 1; remove slot if
    /// quantity reaches 0).  Returns the [`ItemId`] that was used.
    pub fn use_item(&mut self, index: usize) -> Result<ItemId, InventoryError> {
        if index >= self.inner.items.len() {
            return Err(InventoryError::IndexOutOfBounds);
        }
        let item_id = self.inner.items[index].0;
        self.remove_item_at(index, 1)?;
        Ok(item_id)
    }

    /// Remove all items.
    pub fn clear(&mut self) {
        self.inner.items.clear();
    }
}

// ── Serde support ──────────────────────────────────────────────────────────
//
// Manual implementations keep the same JSON shape as the previous derive-
// based version: `{ "items": […], "capacity": 20 }`.

impl Serialize for Inventory {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Inventory", 2)?;
        state.serialize_field("items", &self.inner.items)?;
        state.serialize_field("capacity", &self.inner.max_slots.unwrap_or(0))?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Inventory {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "Inventory")]
        struct InventoryData {
            items: Vec<(ItemId, u32)>,
            capacity: usize,
        }
        let data = InventoryData::deserialize(deserializer)?;
        Ok(Inventory {
            inner: EngineInventory {
                items: data.items,
                max_slots: Some(data.capacity),
                max_per_slot: Some(MAX_ITEM_QUANTITY as u32),
            },
        })
    }
}

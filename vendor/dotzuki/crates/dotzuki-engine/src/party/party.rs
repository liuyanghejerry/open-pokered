//! Generic ordered party container, storage boxes, and a box store.
//!
//! Capacities are always **supplied by the game** (never hardcoded to 6 / 12 /
//! etc.). The engine only enforces the capacity it was given.

use super::{MonsterInstance, MonsterProvider};

/// Error returned when adding to a full [`Party`] or [`StorageBox`].
///
/// Carries the rejected monster back to the caller so nothing is lost.
#[derive(Debug, PartialEq, Eq)]
pub struct PartyFull<P: MonsterProvider>(pub MonsterInstance<P>);

/// An ordered party of monsters with a provider/param-defined capacity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Party<P: MonsterProvider> {
    members: Vec<MonsterInstance<P>>,
    capacity: usize,
}

impl<P: MonsterProvider> Party<P> {
    /// Create an empty party with the given capacity (supplied by the game,
    /// **not** hardcoded).
    pub fn new(capacity: usize) -> Self {
        Self {
            members: Vec::new(),
            capacity,
        }
    }

    /// The capacity this party was created with.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of monsters currently in the party.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether the party has no members.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Whether the party is at capacity.
    pub fn is_full(&self) -> bool {
        self.members.len() >= self.capacity
    }

    /// Append a monster. Returns [`PartyFull`] (carrying the monster back) if
    /// the party is already full.
    pub fn add(&mut self, m: MonsterInstance<P>) -> Result<(), PartyFull<P>> {
        if self.is_full() {
            return Err(PartyFull(m));
        }
        self.members.push(m);
        Ok(())
    }

    /// Remove and return the monster at `index`, shifting later members down.
    pub fn remove(&mut self, index: usize) -> Option<MonsterInstance<P>> {
        if index < self.members.len() {
            Some(self.members.remove(index))
        } else {
            None
        }
    }

    /// Swap the monsters at indices `a` and `b`. Out-of-range indices are
    /// ignored.
    pub fn swap(&mut self, a: usize, b: usize) {
        if a < self.members.len() && b < self.members.len() {
            self.members.swap(a, b);
        }
    }

    /// Borrow the monster at `index`.
    pub fn get(&self, index: usize) -> Option<&MonsterInstance<P>> {
        self.members.get(index)
    }

    /// Mutably borrow the monster at `index`.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut MonsterInstance<P>> {
        self.members.get_mut(index)
    }

    /// Index of the first non-fainted member (lead selection), if any.
    pub fn first_able(&self) -> Option<usize> {
        self.members.iter().position(|m| !m.is_fainted())
    }

    /// Whether every member has fainted (battle loss / whiteout). A party with
    /// no members counts as all-fainted.
    pub fn all_fainted(&self) -> bool {
        !self.members.is_empty() && self.members.iter().all(|m| m.is_fainted())
    }

    /// Iterate over the party members in order.
    pub fn iter(&self) -> impl Iterator<Item = &MonsterInstance<P>> {
        self.members.iter()
    }

    /// Mutably iterate over the party members in order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut MonsterInstance<P>> {
        self.members.iter_mut()
    }
}

/// A single storage box with a game-defined capacity. Same add/remove/len/
/// is_full surface as [`Party`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageBox<P: MonsterProvider> {
    members: Vec<MonsterInstance<P>>,
    capacity: usize,
}

impl<P: MonsterProvider> StorageBox<P> {
    /// Create an empty box with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            members: Vec::new(),
            capacity,
        }
    }

    /// The capacity this box was created with.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of monsters stored.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether the box is empty.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Whether the box is at capacity.
    pub fn is_full(&self) -> bool {
        self.members.len() >= self.capacity
    }

    /// Store a monster, returning it back via [`PartyFull`] if the box is full.
    pub fn add(&mut self, m: MonsterInstance<P>) -> Result<(), PartyFull<P>> {
        if self.is_full() {
            return Err(PartyFull(m));
        }
        self.members.push(m);
        Ok(())
    }

    /// Remove and return the monster at `index`.
    pub fn remove(&mut self, index: usize) -> Option<MonsterInstance<P>> {
        if index < self.members.len() {
            Some(self.members.remove(index))
        } else {
            None
        }
    }

    /// Borrow the monster at `index`.
    pub fn get(&self, index: usize) -> Option<&MonsterInstance<P>> {
        self.members.get(index)
    }

    /// Mutably borrow the monster at `index`.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut MonsterInstance<P>> {
        self.members.get_mut(index)
    }

    /// Iterate over the stored monsters.
    pub fn iter(&self) -> impl Iterator<Item = &MonsterInstance<P>> {
        self.members.iter()
    }
}

/// A collection of storage boxes plus a "currently selected" box, with both the
/// box count and per-box capacity supplied by the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoxStore<P: MonsterProvider> {
    boxes: Vec<StorageBox<P>>,
    current: usize,
}

impl<P: MonsterProvider> BoxStore<P> {
    /// Create `box_count` boxes, each with `box_capacity` (both supplied).
    pub fn new(box_count: usize, box_capacity: usize) -> Self {
        Self {
            boxes: (0..box_count)
                .map(|_| StorageBox::new(box_capacity))
                .collect(),
            current: 0,
        }
    }

    /// Number of boxes.
    pub fn box_count(&self) -> usize {
        self.boxes.len()
    }

    /// Index of the currently selected box.
    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Borrow the currently selected box.
    pub fn current(&self) -> &StorageBox<P> {
        &self.boxes[self.current]
    }

    /// Mutably borrow the currently selected box.
    pub fn current_mut(&mut self) -> &mut StorageBox<P> {
        &mut self.boxes[self.current]
    }

    /// Borrow a specific box by index.
    pub fn get(&self, index: usize) -> Option<&StorageBox<P>> {
        self.boxes.get(index)
    }

    /// Mutably borrow a specific box by index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut StorageBox<P>> {
        self.boxes.get_mut(index)
    }

    /// Select the box at `index`. Out-of-range indices are ignored.
    pub fn switch(&mut self, index: usize) {
        if index < self.boxes.len() {
            self.current = index;
        }
    }
}

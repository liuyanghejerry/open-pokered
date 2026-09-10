//! Generic, game-agnostic party / monster model (milestone **P0a**).
//!
//! This module lifts the party/monster/box model out of any specific game into
//! the engine. The engine knows *nothing* game-specific: species, moves, items,
//! stat formulas, EXP curves, and evolution conditions are all supplied by the
//! game through provider traits, mirroring the existing
//! [`crate::GameData`] / [`crate::battle::BattleProvider`] /
//! [`crate::battle::ItemProvider`](crate::items::ItemProvider) pattern.
//!
//! The engine offers *mechanism* (storing instances, driving level-up /
//! evolution / party transitions); the game supplies *policy* (the numbers).
//!
//! There is **no `rand` dependency**: if a game needs randomness (e.g. for
//! DV/IV generation) it computes the value itself and hands the result to the
//! engine, which only ever stores it.

mod monster;
mod party;

pub use monster::{LevelUp, MonsterInstance, MonsterStatus, MoveSlot};
pub use party::{BoxStore, Party, PartyFull, StorageBox};

use core::fmt::Debug;

/// Game-supplied definition of how monsters work.
///
/// This is the master provider for the party model, analogous to
/// [`crate::GameData`]. It binds the game's concrete id types and supplies the
/// stat-calculation hooks. The engine stores instances and drives transitions;
/// the game decides every number.
pub trait MonsterProvider {
    /// Opaque species identifier (e.g. a numeric dex id newtype/enum).
    type SpeciesId: Copy + Eq + Debug;
    /// Opaque move identifier.
    type MoveId: Copy + Eq + Debug;
    /// Per-instance "genetics" the game uses in stat calc (Gen-1 DVs, modern
    /// IVs). The engine treats this as opaque and just stores it. `PartialEq +
    /// Eq` are required so [`MonsterInstance`] / [`Party`] can derive them
    /// (handy for round-trip identity tests and snapshot comparison).
    type Genetics: Clone + Debug + Default + PartialEq + Eq;
    /// Accumulated training data (Gen-1 stat-exp, modern EVs). Opaque to the
    /// engine.
    type Training: Clone + Debug + Default + PartialEq + Eq;
    /// The set of stats this game uses. Must be index-mappable / comparable.
    type Stat: Copy + Eq + Debug;

    /// Base stat value for a species + stat.
    fn base_stat(&self, species: Self::SpeciesId, stat: Self::Stat) -> u16;

    /// Compute a single derived stat from base / level / genetics / training.
    ///
    /// This is where the Gen-1 formula (or any other) lives — in the **game**.
    fn calc_stat(
        &self,
        species: Self::SpeciesId,
        stat: Self::Stat,
        level: u8,
        genetics: &Self::Genetics,
        training: &Self::Training,
    ) -> u16;

    /// All stats the game iterates over (for full recalculation).
    fn stats(&self) -> &[Self::Stat];

    /// Which [`Self::Stat`] represents HP. The engine cannot guess this, so the
    /// game declares it; [`MonsterInstance::max_hp`] uses it.
    fn hp_stat(&self) -> Self::Stat;

    /// Max number of moves a monster can know (Gen-1 = 4).
    fn max_moves(&self) -> usize;
}

/// A small map from `P::Stat` to `u16`.
///
/// Because `P::Stat` is opaque to the engine, this is stored as a `Vec` kept in
/// the provider's [`MonsterProvider::stats`] order. (We deliberately do *not*
/// require an `EnumMap` here — the game's stat enum is unknown to the engine, so
/// a `Vec` keyed by the provider's stat order is the pragmatic choice.)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatSet<P: MonsterProvider> {
    values: Vec<(P::Stat, u16)>,
}

impl<P: MonsterProvider> StatSet<P> {
    /// One zeroed entry per `provider.stats()`, in provider order.
    pub fn zeroed(provider: &P) -> Self {
        Self {
            values: provider.stats().iter().map(|&s| (s, 0)).collect(),
        }
    }

    /// Get the value for `stat`, or `0` if the stat is not present.
    pub fn get(&self, stat: P::Stat) -> u16 {
        self.values
            .iter()
            .find(|(s, _)| *s == stat)
            .map(|(_, v)| *v)
            .unwrap_or(0)
    }

    /// Set the value for `stat`. If the stat is not already present (e.g. the
    /// set was built before the stat existed) it is appended.
    pub fn set(&mut self, stat: P::Stat, value: u16) {
        if let Some(entry) = self.values.iter_mut().find(|(s, _)| *s == stat) {
            entry.1 = value;
        } else {
            self.values.push((stat, value));
        }
    }

    /// Iterate over `(stat, value)` pairs in provider order.
    pub fn iter(&self) -> impl Iterator<Item = (P::Stat, u16)> + '_ {
        self.values.iter().map(|(s, v)| (*s, *v))
    }
}

/// Total EXP / leveling hooks. Layered on top of [`MonsterProvider`] so the
/// same id types are reused.
pub trait ExpProvider: MonsterProvider {
    /// Total EXP required to *be* at `level` for this species' growth group.
    fn exp_for_level(&self, species: Self::SpeciesId, level: u8) -> u32;

    /// Maximum attainable level (Gen-1 = 100).
    fn max_level(&self) -> u8;

    /// Decide the new *current* HP when the monster levels up and its max HP
    /// changes from `old_max_hp` to `new_max_hp`.
    ///
    /// This is the engine's **HP-growth policy hook**. The default is the
    /// game-agnostic "preserve absolute HP, clamp to the new max" behavior
    /// (matching [`MonsterInstance::recalc_stats`]). Games that want the Gen-1
    /// behavior — grow current HP by the max-HP delta — override this to return
    /// `current_hp + (new_max_hp - old_max_hp)`.
    ///
    /// The engine clamps the returned value to `new_max_hp` regardless, so an
    /// override cannot produce HP above the new maximum.
    fn levelup_current_hp(&self, old_max_hp: u16, new_max_hp: u16, current_hp: u16) -> u16 {
        let _ = old_max_hp;
        current_hp.min(new_max_hp)
    }

    /// Learn any moves the monster gains by reaching `level`, mutating its
    /// `moves` list, and return the ids of the moves that were newly learned.
    ///
    /// This is the engine's **move-learning hook**. It is called once per level
    /// crossed (in ascending order) during [`MonsterInstance::gain_exp`]. The
    /// default learns nothing (returns an empty `Vec`) so the engine never needs
    /// to know any move data. Games override this to consult their learnset and
    /// insert moves into `moves` using whatever slot/PP rules they use; the ids
    /// returned here are surfaced in [`LevelUp::learned_moves`].
    fn learn_moves_on_levelup(
        &self,
        species: Self::SpeciesId,
        level: u8,
        moves: &mut Vec<MoveSlot<Self>>,
    ) -> Vec<Self::MoveId>
    where
        Self: Sized,
    {
        let _ = (species, level, moves);
        Vec::new()
    }
}

/// Evolution hooks. Layered on top of [`MonsterProvider`].
///
/// The trigger carries an opaque, game-defined item id ([`Self::EvoItem`]) so
/// the engine never couples to a concrete item system.
pub trait EvolutionProvider: MonsterProvider {
    /// Opaque item identifier used by item-based evolutions.
    type EvoItem: Copy + Eq + Debug;

    /// Decide what species (if any) this instance evolves into *right now*,
    /// given a trigger. The game encodes all conditions; the engine just
    /// applies the result.
    fn evolution_target(
        &self,
        inst: &MonsterInstance<Self>,
        trigger: EvolutionTrigger<Self::EvoItem>,
    ) -> Option<Self::SpeciesId>
    where
        Self: Sized;
}

/// What caused an evolution check. Parameterized by the game's opaque item id
/// so the engine stays decoupled from any item system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvolutionTrigger<Item> {
    /// Triggered by gaining a level.
    LevelUp,
    /// Triggered by using an evolution item.
    Item(Item),
    /// Triggered by trading.
    Trade,
}

#[cfg(test)]
mod tests;

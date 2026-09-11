//! [`MonsterInstance`]: a generic, provider-driven monster instance.

use super::{EvolutionProvider, EvolutionTrigger, ExpProvider, MonsterProvider, StatSet};
use crate::battle::{BattleProvider, BattlerState, EnumMap};

/// Engine-level status condition for a stored monster.
///
/// This is intentionally a small, game-agnostic enum; games map their own
/// status model onto it. Battle-only / volatile statuses live in the battle
/// layer (the battle `BattlerState` carries a provider-defined `Status`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonsterStatus {
    /// No status condition.
    Healthy,
    /// Asleep for the given number of remaining turns.
    Sleep(u8),
    /// Poisoned.
    Poison,
    /// Burned.
    Burn,
    /// Frozen.
    Freeze,
    /// Paralyzed.
    Paralysis,
}

impl Default for MonsterStatus {
    fn default() -> Self {
        MonsterStatus::Healthy
    }
}

/// A single known move with its current and bonus PP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveSlot<P: MonsterProvider> {
    /// The move identifier.
    pub move_id: P::MoveId,
    /// Current PP.
    pub pp: u8,
    /// Number of PP Ups applied (game decides what this means).
    pub pp_up: u8,
}

/// Result of an EXP gain: what happened to the monster's level.
///
/// Parameterized by the [`MonsterProvider`] only so it can carry the game's
/// opaque move ids in [`Self::learned_moves`]; the engine never inspects them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelUp<P: MonsterProvider> {
    /// How many levels were gained (0 if none).
    pub levels_gained: u8,
    /// Level before the EXP was applied.
    pub old_level: u8,
    /// Level after the EXP was applied.
    pub new_level: u8,
    /// Moves newly learned across the levels crossed, in ascending-level order
    /// (populated by [`ExpProvider::learn_moves_on_levelup`]; empty by default).
    pub learned_moves: Vec<P::MoveId>,
}

impl<P: MonsterProvider> Default for LevelUp<P> {
    fn default() -> Self {
        LevelUp {
            levels_gained: 0,
            old_level: 0,
            new_level: 0,
            learned_moves: Vec::new(),
        }
    }
}

impl<P: MonsterProvider> LevelUp<P> {
    /// Whether any level was gained.
    pub fn gained(&self) -> bool {
        self.levels_gained > 0
    }
}

/// A generic monster instance: species, level, exp, computed stats, status, and
/// known moves. All numbers are delegated to the [`MonsterProvider`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonsterInstance<P: MonsterProvider> {
    /// The species.
    pub species: P::SpeciesId,
    /// Current level.
    pub level: u8,
    /// Total accumulated experience.
    pub exp: u32,
    /// Per-instance genetics (opaque to the engine).
    pub genetics: P::Genetics,
    /// Accumulated training (opaque to the engine).
    pub training: P::Training,
    /// Cached computed stats, keyed by the provider's stat order.
    pub stats: StatSet<P>,
    /// Current HP.
    pub current_hp: u16,
    /// Status condition.
    pub status: MonsterStatus,
    /// Known moves.
    pub moves: Vec<MoveSlot<P>>,
}

impl<P: MonsterProvider> MonsterInstance<P> {
    /// Construct at a level. The caller supplies genetics/training; stats are
    /// computed from the provider and `current_hp` is initialized to full.
    pub fn new(
        provider: &P,
        species: P::SpeciesId,
        level: u8,
        genetics: P::Genetics,
        training: P::Training,
    ) -> Self {
        let mut inst = Self {
            species,
            level,
            exp: 0,
            genetics,
            training,
            stats: StatSet::zeroed(provider),
            current_hp: 0,
            status: MonsterStatus::Healthy,
            moves: Vec::new(),
        };
        inst.recalc_stats(provider);
        inst.current_hp = inst.max_hp(provider);
        inst
    }

    /// Recompute every stat from the provider.
    ///
    /// Preserves the absolute `current_hp`, clamped to the new max HP (this
    /// matches Gen-1 recalculation behavior, where current HP is *not* scaled
    /// by ratio on a stat recompute).
    pub fn recalc_stats(&mut self, provider: &P) {
        for &stat in provider.stats() {
            let value = provider.calc_stat(
                self.species,
                stat,
                self.level,
                &self.genetics,
                &self.training,
            );
            self.stats.set(stat, value);
        }
        let max = self.max_hp(provider);
        if self.current_hp > max {
            self.current_hp = max;
        }
    }

    /// Max HP, using the provider-declared HP stat.
    pub fn max_hp(&self, provider: &P) -> u16 {
        self.stats.get(provider.hp_stat())
    }

    /// Whether the monster has fainted (0 HP).
    pub fn is_fainted(&self) -> bool {
        self.current_hp == 0
    }
}

impl<P: ExpProvider> MonsterInstance<P> {
    /// Add EXP, advancing level while the threshold for the next level is met.
    ///
    /// Recomputes stats on each level gained. Caps at the provider's
    /// [`ExpProvider::max_level`]. Returns a summary of what happened.
    ///
    /// Note: the engine drives the *mechanism* (cross thresholds, recalc). Any
    /// game-specific quirks (e.g. the Gen-1 experience underflow) live in the
    /// game's [`ExpProvider::exp_for_level`] implementation and in how the game
    /// chooses to call this method.
    pub fn gain_exp(&mut self, provider: &P, amount: u32) -> LevelUp<P> {
        let old_level = self.level;
        self.exp = self.exp.saturating_add(amount);

        let max_level = provider.max_level();
        // Clamp accumulated exp to never exceed the max-level threshold.
        let max_exp = provider.exp_for_level(self.species, max_level);
        if self.exp > max_exp {
            self.exp = max_exp;
        }

        let mut learned_moves: Vec<P::MoveId> = Vec::new();

        while self.level < max_level {
            let next = self.level + 1;
            let needed = provider.exp_for_level(self.species, next);
            if self.exp >= needed {
                // Capture max HP before the recalc so the HP-growth policy hook
                // can apply the game's delta rule (Gen-1: grow by the increase).
                let old_max_hp = self.max_hp(provider);
                self.level = next;
                // Recompute stats for the new level. `recalc_stats` clamps
                // `current_hp` to the new max; the policy hook below then sets
                // the authoritative value (and is re-clamped), so the interim
                // clamp here is harmless.
                self.recalc_stats(provider);
                let new_max_hp = self.max_hp(provider);
                let new_hp = provider.levelup_current_hp(old_max_hp, new_max_hp, self.current_hp);
                self.current_hp = new_hp.min(new_max_hp);

                // Learn any moves gained at this level (default: none).
                let mut gained =
                    provider.learn_moves_on_levelup(self.species, next, &mut self.moves);
                learned_moves.append(&mut gained);
            } else {
                break;
            }
        }

        LevelUp {
            levels_gained: self.level - old_level,
            old_level,
            new_level: self.level,
            learned_moves,
        }
    }
}

impl<P: EvolutionProvider> MonsterInstance<P> {
    /// If the provider says we evolve, switch species and recalc stats.
    ///
    /// Returns the new species id if evolution happened, otherwise `None`
    /// (the provider returning `None` is how evolution is canceled).
    pub fn try_evolve(
        &mut self,
        provider: &P,
        trigger: EvolutionTrigger<P::EvoItem>,
    ) -> Option<P::SpeciesId> {
        let target = provider.evolution_target(self, trigger)?;
        self.species = target;
        self.recalc_stats(provider);
        Some(target)
    }
}

impl<P: MonsterProvider> MonsterInstance<P> {
    /// Build a [`BattlerState`] snapshot for the battle engine.
    ///
    /// This is the seam the later battle-migration milestone builds on. The
    /// engine's [`BattlerState`] is generic over a single [`BattleProvider`]
    /// `B`, whose associated `Species` / `Move` / `Stat` types are *independent*
    /// of this monster's [`MonsterProvider`]. To stay fully game-agnostic the
    /// caller supplies the conversions:
    ///
    /// - `map_species`: `P::SpeciesId -> B::Species`
    /// - `map_move`: `P::MoveId -> B::Move`
    /// - `map_stat`: `P::Stat -> B::Stat`
    ///
    /// Stat stages start neutral, status defaults to `None`, and the live HP /
    /// level / species / stats / moves are carried over. Status is intentionally
    /// **not** mapped here (it is a battle-provider-defined `Option<B::Status>`);
    /// the caller can set `bs.status` afterwards if needed. This keeps the
    /// mapping total and the engine decoupled from any concrete status model.
    pub fn to_battler<B, FS, FM, FST>(
        &self,
        provider: &P,
        mut map_species: FS,
        mut map_move: FM,
        mut map_stat: FST,
    ) -> BattlerState<B>
    where
        B: BattleProvider,
        B::Stat: Copy + PartialEq,
        FS: FnMut(P::SpeciesId) -> B::Species,
        FM: FnMut(P::MoveId) -> B::Move,
        FST: FnMut(P::Stat) -> B::Stat,
    {
        let mut stats: EnumMap<B::Stat, u16> = EnumMap::new();
        for (stat, value) in self.stats.iter() {
            stats.set(map_stat(stat), value);
        }
        let moves: Vec<B::Move> = self.moves.iter().map(|m| map_move(m.move_id)).collect();
        let species = map_species(self.species);
        let max_hp = self.max_hp(provider);
        BattlerState::new(species, self.current_hp, max_hp, stats, moves)
    }
}

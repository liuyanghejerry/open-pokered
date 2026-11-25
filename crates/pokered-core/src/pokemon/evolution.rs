use crate::alloc_prelude::*;
use crate::battle::settlement::evolution::{
    apply_evolution, check_item_evolution, check_level_evolution, check_trade_evolution,
};
use crate::battle::state::Pokemon;
use crate::pokemon::move_learning::{try_learn_move, LearnMoveResult};
use crate::pokemon::pokedex::Pokedex;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolutionTrigger {
    LevelUp,
    Trade,
    Item(ItemId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvolutionResult {
    pub from: Species,
    pub to: Species,
    pub trigger: EvolutionTrigger,
}

pub fn check_evolution(pokemon: &Pokemon, trigger: EvolutionTrigger) -> Option<Species> {
    match trigger {
        EvolutionTrigger::LevelUp => check_level_evolution(pokemon.species, pokemon.level),
        EvolutionTrigger::Trade => check_trade_evolution(pokemon.species, pokemon.level),
        EvolutionTrigger::Item(item) => check_item_evolution(pokemon.species, pokemon.level, item),
    }
}

pub fn try_evolve(pokemon: &mut Pokemon, trigger: EvolutionTrigger) -> Option<EvolutionResult> {
    let from = pokemon.species;
    let to = check_evolution(pokemon, trigger)?;
    apply_evolution(pokemon, to);
    Some(EvolutionResult { from, to, trigger })
}

pub fn evolve_party_after_battle(party: &mut [Pokemon]) -> Vec<EvolutionResult> {
    let mut results = Vec::new();
    for mon in party.iter_mut() {
        if mon.hp > 0 {
            if let Some(result) = try_evolve(mon, EvolutionTrigger::LevelUp) {
                results.push(result);
            }
        }
    }
    results
}

/// Apply a CONFIRMED evolution (the evolution cutscene finished without a
/// B-cancel) — the post-`EvolveMon` half of `EvolutionAfterBattle`
/// (engine/pokemon/evos_moves.asm:136-236):
///
/// - species swap + stat recalc + HP-delta adjustment ([`apply_evolution`];
///   `CalcStats` + the max-HP-delta add at evos_moves.asm:174-207);
/// - `RenameEvolvedMon` (evos_moves.asm:262-291): a mon whose "nickname" is
///   just its old species name is renamed to the new species name; real
///   nicknames are kept. A `None` nickname needs nothing — `display_name`
///   already follows the species;
/// - `LearnMoveFromLevelUp` at the mon's current level (evos_moves.asm:65, 341-379) —
///   in Gen 1 the evolved form learns ONLY the first learnset entry matching
///   `mon.level` (and stops immediately if that first matching entry is already
///   known). When all four slots are full, that single blocked move is returned
///   so the caller can run the forget-a-move replace flow ([`replace_move_guarded`]);
/// - Pokédex seen + owned for the new species (the two
///   `Evolution_FlagAction` calls at evos_moves.asm:218-228).
///
/// Returns the new species' level-up move the mon could not learn because
/// all four move slots are full (empty when nothing was blocked).
pub fn finalize_evolution(mon: &mut Pokemon, pokedex: &mut Pokedex, to: Species) -> Vec<MoveId> {
    let from = mon.species;
    apply_evolution(mon, to);
    // RenameEvolvedMon: a mon whose "nickname" is just its old species name
    // is renamed to the new species name; real nicknames are kept. An unset
    // nickname needs nothing — display_name already follows the species.
    let mut name_buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
    if mon.has_nickname()
        && crate::battle::state::decode_name(&mon.nickname, &mut name_buf)
            .eq_ignore_ascii_case(pokered_data::lang_data::species_name(from, false))
    {
        mon.set_nickname(pokered_data::lang_data::species_name(to, false));
    }
    let mut blocked = Vec::new();
    if let Some(move_id) = crate::pokemon::move_learning::first_move_at_level_for(to, mon.level, &mon.moves) {
        if matches!(try_learn_move(mon, move_id), LearnMoveResult::MoveSlotsFull) {
            blocked.push(move_id);
        }
    }
    pokedex.set_seen(to);
    pokedex.set_owned(to);
    blocked
}

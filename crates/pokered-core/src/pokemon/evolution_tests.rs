use super::evolution::*;
use crate::battle::state::{Pokemon, StatusCondition};
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn make_pokemon(species: Species, level: u8) -> Pokemon {
    Pokemon {
        species,
        nickname: None,
        level,
        hp: 50,
        max_hp: 50,
        attack: 40,
        defense: 40,
        speed: 40,
        special: 40,
        type1: PokemonType::Normal,
        type2: PokemonType::Normal,
        moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        pp: [35, 0, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xAA, 0xAA],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: None,
    }
}

#[test]
fn level_evolution_bulbasaur_at_16() {
    let mut mon = make_pokemon(Species::Bulbasaur, 16);
    let result = try_evolve(&mut mon, EvolutionTrigger::LevelUp);
    assert!(result.is_some());
    let r = result.unwrap();
    assert_eq!(r.from, Species::Bulbasaur);
    assert_eq!(r.to, Species::Ivysaur);
    assert_eq!(r.trigger, EvolutionTrigger::LevelUp);
    assert_eq!(mon.species, Species::Ivysaur);
}

#[test]
fn level_evolution_bulbasaur_at_15_no_evolve() {
    let mut mon = make_pokemon(Species::Bulbasaur, 15);
    let result = try_evolve(&mut mon, EvolutionTrigger::LevelUp);
    assert!(result.is_none());
    assert_eq!(mon.species, Species::Bulbasaur);
}

#[test]
fn trade_evolution_kadabra() {
    let mut mon = make_pokemon(Species::Kadabra, 30);
    let result = try_evolve(&mut mon, EvolutionTrigger::Trade);
    assert!(result.is_some());
    let r = result.unwrap();
    assert_eq!(r.from, Species::Kadabra);
    assert_eq!(r.to, Species::Alakazam);
    assert_eq!(mon.species, Species::Alakazam);
}

#[test]
fn trade_evolution_pikachu_no_evolve() {
    let mut mon = make_pokemon(Species::Pikachu, 50);
    let result = try_evolve(&mut mon, EvolutionTrigger::Trade);
    assert!(result.is_none());
    assert_eq!(mon.species, Species::Pikachu);
}

#[test]
fn item_evolution_pikachu_thunder_stone() {
    let mut mon = make_pokemon(Species::Pikachu, 25);
    let result = try_evolve(&mut mon, EvolutionTrigger::Item(ItemId::ThunderStone));
    assert!(result.is_some());
    let r = result.unwrap();
    assert_eq!(r.from, Species::Pikachu);
    assert_eq!(r.to, Species::Raichu);
    assert_eq!(mon.species, Species::Raichu);
}

#[test]
fn item_evolution_wrong_item() {
    let mut mon = make_pokemon(Species::Pikachu, 25);
    let result = try_evolve(&mut mon, EvolutionTrigger::Item(ItemId::FireStone));
    assert!(result.is_none());
    assert_eq!(mon.species, Species::Pikachu);
}

#[test]
fn no_evolution_for_mew() {
    let mut mon = make_pokemon(Species::Mew, 100);
    assert!(try_evolve(&mut mon, EvolutionTrigger::LevelUp).is_none());
    assert!(try_evolve(&mut mon, EvolutionTrigger::Trade).is_none());
    assert!(try_evolve(&mut mon, EvolutionTrigger::Item(ItemId::MoonStone)).is_none());
}

#[test]
fn check_evolution_without_applying() {
    let mon = make_pokemon(Species::Bulbasaur, 16);
    let target = check_evolution(&mon, EvolutionTrigger::LevelUp);
    assert_eq!(target, Some(Species::Ivysaur));
}

#[test]
fn evolve_party_after_battle_only_alive() {
    let mut party = vec![
        make_pokemon(Species::Bulbasaur, 16),
        make_pokemon(Species::Charmander, 16),
        make_pokemon(Species::Squirtle, 16),
    ];
    party[1].hp = 0;

    let results = evolve_party_after_battle(&mut party);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].from, Species::Bulbasaur);
    assert_eq!(results[0].to, Species::Ivysaur);
    assert_eq!(results[1].from, Species::Squirtle);
    assert_eq!(results[1].to, Species::Wartortle);

    assert_eq!(party[0].species, Species::Ivysaur);
    assert_eq!(party[1].species, Species::Charmander);
    assert_eq!(party[2].species, Species::Wartortle);
}

#[test]
fn evolve_party_no_evolutions() {
    let mut party = vec![
        make_pokemon(Species::Bulbasaur, 10),
        make_pokemon(Species::Mew, 100),
    ];
    let results = evolve_party_after_battle(&mut party);
    assert!(results.is_empty());
}

#[test]
fn evolution_updates_stats() {
    let mut mon = make_pokemon(Species::Bulbasaur, 16);
    let old_max_hp = mon.max_hp;
    try_evolve(&mut mon, EvolutionTrigger::LevelUp);
    assert_ne!(mon.max_hp, old_max_hp);
}

#[test]
fn eevee_fire_stone_evolution() {
    let mut mon = make_pokemon(Species::Eevee, 25);
    let result = try_evolve(&mut mon, EvolutionTrigger::Item(ItemId::FireStone));
    assert!(result.is_some());
    assert_eq!(mon.species, Species::Flareon);
}

#[test]
fn eevee_water_stone_evolution() {
    let mut mon = make_pokemon(Species::Eevee, 25);
    let result = try_evolve(&mut mon, EvolutionTrigger::Item(ItemId::WaterStone));
    assert!(result.is_some());
    assert_eq!(mon.species, Species::Vaporeon);
}

#[test]
fn eevee_thunder_stone_evolution() {
    let mut mon = make_pokemon(Species::Eevee, 25);
    let result = try_evolve(&mut mon, EvolutionTrigger::Item(ItemId::ThunderStone));
    assert!(result.is_some());
    assert_eq!(mon.species, Species::Jolteon);
}

// -- finalize_evolution (the post-cutscene application) ---------------------

#[test]
fn finalize_swaps_species_and_marks_dex_seen_and_owned() {
    let mut mon = make_pokemon(Species::Bulbasaur, 16);
    let mut dex = crate::pokemon::pokedex::Pokedex::new();
    finalize_evolution(&mut mon, &mut dex, Species::Ivysaur);
    assert_eq!(mon.species, Species::Ivysaur);
    // evos_moves.asm:222-228 — BOTH flag actions run (owned, then seen).
    assert!(dex.is_owned(Species::Ivysaur));
    assert!(dex.is_seen(Species::Ivysaur));
    // Types come from the new species (Grass/Poison for Ivysaur).
    assert_eq!(mon.type1, PokemonType::Grass);
}

#[test]
fn finalize_adjusts_hp_by_max_hp_delta() {
    let mut mon = make_pokemon(Species::Bulbasaur, 16);
    mon.hp = 30;
    mon.max_hp = 40;
    let old_max = mon.max_hp;
    let mut dex = crate::pokemon::pokedex::Pokedex::new();
    finalize_evolution(&mut mon, &mut dex, Species::Ivysaur);
    let delta = mon.max_hp - old_max;
    assert_eq!(mon.hp, 30 + delta, "hp grows by the max-hp delta");
}

/// RenameEvolvedMon (evos_moves.asm:262-291): a mon nicknamed with its OLD
/// species name (i.e. not a real nickname) is renamed to the new species;
/// real nicknames survive.
#[test]
fn finalize_renames_only_species_default_nicknames() {
    let mut dex = crate::pokemon::pokedex::Pokedex::new();

    // No nickname: display_name follows the species automatically.
    let mut mon = make_pokemon(Species::Bulbasaur, 16);
    finalize_evolution(&mut mon, &mut dex, Species::Ivysaur);
    assert_eq!(mon.display_name(), "IVYSAUR");

    // "Nickname" == old species name → renamed to the new species name.
    let mut mon = make_pokemon(Species::Bulbasaur, 16);
    mon.set_nickname("BULBASAUR".to_string());
    finalize_evolution(&mut mon, &mut dex, Species::Ivysaur);
    assert_eq!(mon.display_name(), "IVYSAUR");

    // Real nickname → kept.
    let mut mon = make_pokemon(Species::Bulbasaur, 16);
    mon.set_nickname("SPROUT".to_string());
    finalize_evolution(&mut mon, &mut dex, Species::Ivysaur);
    assert_eq!(mon.display_name(), "SPROUT");
}

/// LearnMoveFromLevelUp at the current level (evos_moves.asm:212): the
/// evolved form learns its new species' moves at that level — Abra evolves
/// into Kadabra at 16, and Kadabra's learnset has Confusion at 16.
#[test]
fn finalize_learns_new_species_moves_at_current_level() {
    let mut mon = make_pokemon(Species::Abra, 16);
    mon.moves = [MoveId::Teleport, MoveId::None, MoveId::None, MoveId::None];
    let mut dex = crate::pokemon::pokedex::Pokedex::new();
    finalize_evolution(&mut mon, &mut dex, Species::Kadabra);
    assert!(
        mon.moves.contains(&MoveId::Confusion),
        "Kadabra learns Confusion at level 16 on evolution, got {:?}",
        mon.moves
    );
}

/// A full moveset blocks the evolution learn: the new species' level-up move
/// is returned (not silently dropped) for the forget-a-move prompt
/// (learn_move.asm:26-29 → TryingToLearn).
#[test]
fn finalize_returns_blocked_move_when_moveset_is_full() {
    let mut mon = make_pokemon(Species::Abra, 16);
    mon.moves = [
        MoveId::Teleport,
        MoveId::Tackle,
        MoveId::Growl,
        MoveId::Psywave,
    ];
    let mut dex = crate::pokemon::pokedex::Pokedex::new();
    let blocked = finalize_evolution(&mut mon, &mut dex, Species::Kadabra);
    assert!(
        blocked.contains(&MoveId::Confusion),
        "Kadabra's level-16 Confusion is blocked by the full moveset: {blocked:?}"
    );
    assert!(
        !mon.moves.contains(&MoveId::Confusion),
        "the blocked move is NOT silently force-learned"
    );
}

/// replace_move_guarded (learn_move.asm:160-181): swapping works, and the
/// HM guard (IsMoveHM → HMCantDeleteText) applies to level-up learning too.
#[test]
fn replace_move_guarded_refuses_hms() {
    let mut mon = make_pokemon(Species::Abra, 16);
    mon.moves = [
        MoveId::Teleport,
        MoveId::Tackle,
        MoveId::Growl,
        MoveId::Psywave,
    ];
    use crate::pokemon::move_learning::{replace_move_guarded, ReplaceMoveError};
    // An HM move in the slot cannot be forgotten — even for a level-up learn.
    mon.moves[0] = MoveId::Cut;
    assert_eq!(
        replace_move_guarded(&mut mon, 0, MoveId::Confusion),
        Err(ReplaceMoveError::HmCantDelete)
    );
    assert_eq!(mon.moves[0], MoveId::Cut, "the HM move survives");
    // A normal move can be replaced (returns the forgotten move).
    let old = replace_move_guarded(&mut mon, 1, MoveId::Confusion).expect("slot 1 replaceable");
    assert_eq!(old, MoveId::Tackle);
    assert_eq!(mon.moves[1], MoveId::Confusion);
    assert!(mon.pp[1] > 0, "PP follows the new move");
    // Empty slots are refused.
    mon.moves[3] = MoveId::None;
    assert_eq!(
        replace_move_guarded(&mut mon, 3, MoveId::Confusion),
        Err(ReplaceMoveError::InvalidSlot)
    );
}

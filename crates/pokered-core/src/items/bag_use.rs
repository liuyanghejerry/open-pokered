//! Bag ITEM-menu USE flow for party-targeted items.
//!
//! The overworld bag's USE action splits items into three kinds
//! ([`classify_bag_use`]):
//!
//! * [`BagUseKind::Field`] — handled directly by
//!   `OverworldScreen::use_field_item` (POKé FLUTE, BICYCLE, ESCAPE ROPE,
//!   REPEL, the fishing rods…).
//! * [`BagUseKind::OnPokemon`] — opens the party screen; on selection the
//!   caller runs [`apply_item_to_pokemon`], which drives the same concrete
//!   effect functions ([`crate::items::healing`], [`status_cure`],
//!   [`pp_restore`], [`vitamins`]) that [`super::use_engine`] adapts to the
//!   engine's item driver, plus evolution stones / Rare Candy evolutions
//!   (deferred to the cutscene via [`ItemApplyOutcome::EvolutionPending`]) and
//!   TM/HM teaching ([`teach_tm`] / [`teach_hm`]).
//! * [`BagUseKind::NotTime`] — refused ("OAK: This isn't the time to use
//!   that!") by `use_field_item`'s fallback arm.
//!
//! Message texts follow the original wording (data/text/text_6.asm /
//! text_7.asm), line-broken for the two-line overworld text box.

use crate::battle::state::Pokemon;
use crate::items::healing::{use_healing_item, HealResult};
use crate::items::pp_restore::{use_pp_restore, PpRestoreResult};
use crate::items::status_cure::{use_status_cure, StatusCureResult};
use crate::items::vitamins::{use_rare_candy, use_vitamin, VitaminResult};
use crate::pokemon::evolution::{check_evolution, EvolutionTrigger};
use crate::pokemon::move_learning::{
    can_learn_hm, can_learn_tm, hm_to_move, is_hm_move, replace_move, teach_hm, teach_tm, tm_to_move,
    LearnMoveResult, TeachError, NUM_MOVES,
};
use crate::pokemon::pokedex::Pokedex;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;

/// `_ItemUseNoEffectText` / `_VitaminNoEffectText` (data/text/text_6.asm).
pub const NO_EFFECT_MESSAGE: &str = "It won't have any\neffect.";

/// How the bag's USE action handles an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagUseKind {
    /// Opens the party screen and applies to the chosen party member.
    OnPokemon,
    /// Field effect handled by `OverworldScreen::use_field_item`.
    Field,
    /// Cannot be used from the bag right now.
    NotTime,
}

/// Classify an item for the bag's USE action.
pub fn classify_bag_use(item: ItemId) -> BagUseKind {
    if is_party_target_item(item) {
        BagUseKind::OnPokemon
    } else if is_field_item(item) {
        BagUseKind::Field
    } else {
        BagUseKind::NotTime
    }
}

/// Items whose field effect lives in `OverworldScreen::use_field_item`.
fn is_field_item(item: ItemId) -> bool {
    matches!(
        item,
        ItemId::PokeFlute
            | ItemId::Bicycle
            | ItemId::TownMap
            | ItemId::EscapeRope
            | ItemId::Repel
            | ItemId::SuperRepel
            | ItemId::MaxRepel
            | ItemId::Itemfinder
            | ItemId::OldRod
            | ItemId::GoodRod
            | ItemId::SuperRod
    )
}

/// Items applied to a chosen party Pokémon (ItemUsePtrTable medicine /
/// vitamin / stone entries, plus TM/HM machines).
fn is_party_target_item(item: ItemId) -> bool {
    if machine_of(item).is_some() || is_evolution_stone(item) {
        return true;
    }
    matches!(
        item,
        // HP healing / revive / full-restore.
        ItemId::Potion
            | ItemId::SuperPotion
            | ItemId::HyperPotion
            | ItemId::MaxPotion
            | ItemId::FullRestore
            | ItemId::FreshWater
            | ItemId::SodaPop
            | ItemId::Lemonade
            | ItemId::Revive
            | ItemId::MaxRevive
            // Status cures.
            | ItemId::Antidote
            | ItemId::BurnHeal
            | ItemId::IceHeal
            | ItemId::Awakening
            | ItemId::ParlyzHeal
            | ItemId::FullHeal
            // PP restore.
            | ItemId::Ether
            | ItemId::MaxEther
            | ItemId::Elixer
            | ItemId::MaxElixer
            | ItemId::PpUp
            // Vitamins & Rare Candy.
            | ItemId::HpUp
            | ItemId::Protein
            | ItemId::Iron
            | ItemId::Carbos
            | ItemId::Calcium
            | ItemId::RareCandy
    )
}

/// The five evolution stones (ItemUseEvoStone).
pub fn is_evolution_stone(item: ItemId) -> bool {
    matches!(
        item,
        ItemId::MoonStone
            | ItemId::FireStone
            | ItemId::ThunderStone
            | ItemId::WaterStone
            | ItemId::LeafStone
    )
}

/// A TM (consumed on use) or HM (reusable) machine and its 1-based number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineKind {
    Tm(u8),
    Hm(u8),
}

impl MachineKind {
    pub fn is_tm(self) -> bool {
        matches!(self, MachineKind::Tm(_))
    }

    pub fn move_id(self) -> Option<MoveId> {
        match self {
            MachineKind::Tm(n) => tm_to_move(n),
            MachineKind::Hm(n) => hm_to_move(n),
        }
    }

    fn can_learn(self, species: pokered_data::species::Species) -> bool {
        match self {
            MachineKind::Tm(n) => can_learn_tm(species, n),
            MachineKind::Hm(n) => can_learn_hm(species, n),
        }
    }
}

/// Map a bag item to its machine (TM01..=TM50 = $C9..=$FA, HM01..=HM05 =
/// $C4..=$C8), if it is one.
pub fn machine_of(item: ItemId) -> Option<MachineKind> {
    let id = item as u8;
    if (ItemId::Tm01 as u8..=ItemId::Tm50 as u8).contains(&id) {
        Some(MachineKind::Tm(id - ItemId::Tm01 as u8 + 1))
    } else if (ItemId::Hm01 as u8..=ItemId::Hm05 as u8).contains(&id) {
        Some(MachineKind::Hm(id - ItemId::Hm01 as u8 + 1))
    } else {
        None
    }
}

/// Result of applying a bag item to a party Pokémon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemApplyOutcome {
    /// The effect applied. `consume` says whether the caller removes one copy
    /// from the bag (false for HMs).
    Used { message: String, consume: bool },
    /// Nothing happened; the item stays in the bag. `message` explains why.
    NoEffect { message: String },
    /// TM/HM: the Pokémon already knows four moves. The caller must run the
    /// forget-a-move flow and then call [`finish_tm_hm_replace`].
    NeedsMoveReplace { move_id: MoveId },
    /// The item triggers an evolution (stone / Rare Candy level-up). NOTHING
    /// has been applied to the species yet — the caller must run the
    /// evolution cutscene (`crate::evolution_screen`) and, on success,
    /// `pokemon::evolution::finalize_evolution`; on a B-cancel the mon is
    /// unchanged. `pre_text` is an optional message shown before "What? X is
    /// evolving!" (Rare Candy's "grew to level X!" — the original prints it
    /// before `TryEvolvingMon`). `force` is `wForceEvolution`: true for
    /// stones (uncancellable), false for Rare Candy.
    EvolutionPending {
        pre_text: Option<String>,
        from: pokered_data::species::Species,
        to: pokered_data::species::Species,
        force: bool,
        consume: bool,
    },
}

fn used(message: String, consume: bool) -> ItemApplyOutcome {
    ItemApplyOutcome::Used { message, consume }
}

fn no_effect() -> ItemApplyOutcome {
    ItemApplyOutcome::NoEffect {
        message: NO_EFFECT_MESSAGE.to_string(),
    }
}

fn move_display(move_id: MoveId) -> &'static str {
    pokered_data::lang_data::move_name(move_id, false)
}

/// Apply a party-targeted bag item to `mon`, mutating it in place.
///
/// Evolution stones and Rare Candy level-up evolutions are NOT applied here:
/// they return [`ItemApplyOutcome::EvolutionPending`] and the caller drives
/// the evolution cutscene, applying `pokemon::evolution::finalize_evolution`
/// (which also updates `pokedex`) only when the cutscene confirms.
///
/// Dispatch order mirrors [`super::use_engine::apply_to_pokemon`] (healing →
/// status cure → PP restore → vitamin → Rare Candy) with evolution stones and
/// TM/HM machines appended; both front-ends share the same concrete effect
/// functions, and use_engine's parity tests lock that shared behavior.
pub fn apply_item_to_pokemon(item: ItemId, mon: &mut Pokemon, pokedex: &mut Pokedex) -> ItemApplyOutcome {
    // Dex updates for evolutions happen in finalize_evolution, not here.
    let _ = &pokedex;
    let mut name_buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
    // Healing (HP / revive / full-restore).
    match use_healing_item(mon, item) {
        HealResult::Healed { hp_restored } => {
            return used(
                format!("{}'s HP was\nrestored by {}!", mon.display_name(&mut name_buf), hp_restored),
                true,
            )
        }
        HealResult::Revived { .. } => {
            return used(format!("{} was\nrevitalized!", mon.display_name(&mut name_buf)), true)
        }
        HealResult::AlreadyFullHp | HealResult::NotFainted => return no_effect(),
        HealResult::NotApplicable => {}
    }
    // Status cures.
    match use_status_cure(mon, item) {
        StatusCureResult::Cured => {
            return used(format!("{} was cured\nof its status!", mon.display_name(&mut name_buf)), true)
        }
        StatusCureResult::NoEffect => return no_effect(),
        StatusCureResult::NotApplicable => {}
    }
    // PP restore (whole-item ethers / elixirs target the first move slot in
    // the field, matching `use_engine::apply_to_pokemon`).
    match use_pp_restore(mon, item, 0) {
        PpRestoreResult::Restored { .. } | PpRestoreResult::AllRestored { .. } => {
            return used("PP was\nrestored!".to_string(), true)
        }
        PpRestoreResult::PpUpApplied { move_index, .. } => {
            return used(
                format!("{}'s PP\nincreased!", move_display(mon.moves[move_index])),
                true,
            )
        }
        PpRestoreResult::NoEffect => return no_effect(),
        PpRestoreResult::NotApplicable => {}
    }
    // Vitamins.
    match use_vitamin(mon, item) {
        VitaminResult::Applied { .. } => {
            return used(format!("{}'s stats\nrose!", mon.display_name(&mut name_buf)), true)
        }
        VitaminResult::NoEffect => return no_effect(),
        VitaminResult::NotApplicable => {}
    }
    // Rare Candy: level-up + move learning now; the level-up evolution check
    // runs through the cutscene (ItemUseRareCandy → TryEvolvingMon with
    // wForceEvolution = 0, item_effects.asm:1409-1411).
    if item == ItemId::RareCandy {
        let name = mon.display_name(&mut name_buf);
        return match use_rare_candy(mon) {
            Some(result) => {
                let msg = format!("{} grew to\nlevel {}!", name, result.new_level);
                match check_evolution(mon, EvolutionTrigger::LevelUp) {
                    Some(to) => ItemApplyOutcome::EvolutionPending {
                        pre_text: Some(msg),
                        from: mon.species,
                        to,
                        force: false,
                        consume: true,
                    },
                    None => used(msg, true),
                }
            }
            None => no_effect(),
        };
    }
    // Evolution stones (ItemUseEvoStone): the cutscene is FORCED
    // (wForceEvolution = TRUE, item_effects.asm:776-778 — B cannot cancel).
    if is_evolution_stone(item) {
        if mon.hp == 0 {
            return no_effect();
        }
        return match check_evolution(mon, EvolutionTrigger::Item(item)) {
            Some(to) => ItemApplyOutcome::EvolutionPending {
                pre_text: None,
                from: mon.species,
                to,
                force: true,
                consume: true,
            },
            None => no_effect(),
        };
    }
    // TM/HM machines (ItemUseTMHM).
    if let Some(machine) = machine_of(item) {
        if mon.hp == 0 {
            return no_effect();
        }
        let name = mon.display_name(&mut name_buf);
        let result = match machine {
            MachineKind::Tm(n) => teach_tm(mon, n),
            MachineKind::Hm(n) => teach_hm(mon, n),
        };
        let move_id = machine.move_id().unwrap_or(MoveId::None);
        return match result {
            Ok(LearnMoveResult::Learned { .. }) => used(
                format!("{} learned\n{}!", name, move_display(move_id)),
                machine.is_tm(),
            ),
            Ok(LearnMoveResult::MoveSlotsFull) => ItemApplyOutcome::NeedsMoveReplace { move_id },
            Ok(LearnMoveResult::AlreadyKnown) => ItemApplyOutcome::NoEffect {
                message: format!("{} already\nknows {}!", name, move_display(move_id)),
            },
            // _MonCannotLearnMachineMoveText (data/text/text_6.asm).
            Err(TeachError::Incompatible) => ItemApplyOutcome::NoEffect {
                message: format!(
                    "{} is not\ncompatible with\n{}.\nIt can't learn\n{}.",
                    name,
                    move_display(move_id),
                    move_display(move_id)
                ),
            },
            Err(TeachError::AlreadyKnown) => ItemApplyOutcome::NoEffect {
                message: format!("{} already\nknows {}!", name, move_display(move_id)),
            },
            Err(TeachError::InvalidTmHm) | Err(TeachError::MoveSlotsFull) => no_effect(),
        };
    }
    no_effect()
}

/// The SOFTBOILED field-move heal (start_sub_menus.asm `.softboiled` →
/// ItemUseMedicine's pseudo-item path, engine/items/item_effects.asm:1003-1074,
/// 1186-1222): the *user* loses 1/5 of its max HP (truncating division, `b=2;
/// call Divide`), the *target* gains that same amount capped at its max HP.
///
/// Refusals match the original: the user needs HP > max/5 ("Not healthy
/// enough." — checked before the target pick); a fainted or full-HP target is
/// "It won't have any effect." (`.healingItemNoEffect`, and the target can't
/// be the user itself — the party screen loops while that is picked). No PP
/// and no item is consumed.
pub fn apply_softboiled(user: &mut Pokemon, target: &mut Pokemon) -> ItemApplyOutcome {
    let cost = user.max_hp / 5;
    if user.hp <= cost {
        return ItemApplyOutcome::NoEffect {
            // _NotHealthyEnoughText (data/text/text_5.asm:55-58).
            message: "Not healthy\nenough.".to_string(),
        };
    }
    if target.hp == 0 || target.hp >= target.max_hp {
        return no_effect();
    }
    user.hp -= cost;
    let old_hp = target.hp;
    target.hp = target.hp.saturating_add(cost).min(target.max_hp);
    // POTION_MSG (data/text/text_2.asm:1384-1390): "{name} recovered by {N}!"
    let mut name_buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
    used(
        format!("{} recovered by\n{}!", target.display_name(&mut name_buf), target.hp - old_hp),
        false,
    )
}

/// Finish teaching a TM/HM after the player picked a move to forget
/// ([`ItemApplyOutcome::NeedsMoveReplace`]). Replaces the move in
/// `forget_slot` and reports whether the machine is consumed (TMs yes, HMs
/// no).
pub fn finish_tm_hm_replace(item: ItemId, mon: &mut Pokemon, forget_slot: usize) -> ItemApplyOutcome {
    let Some(machine) = machine_of(item) else {
        return no_effect();
    };
    let Some(move_id) = machine.move_id() else {
        return no_effect();
    };
    if forget_slot >= NUM_MOVES
        || mon.moves[forget_slot] == MoveId::None
        || mon.moves.contains(&move_id)
        || !machine.can_learn(mon.species)
    {
        return no_effect();
    }
    // Gen-1 learn_move.asm: HMCantDeleteText — "HM techniques can't be
    // deleted!"
    if is_hm_move(mon.moves[forget_slot]) {
        return ItemApplyOutcome::NoEffect {
            message: "HM techniques\ncan't be deleted!".to_string(),
        };
    }
    let old_move = mon.moves[forget_slot];
    replace_move(mon, forget_slot, move_id);
    let mut name_buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
    used(
        format!(
            "{} forgot\n{}...\nand learned\n{}!",
            mon.display_name(&mut name_buf),
            move_display(old_move),
            move_display(move_id)
        ),
        machine.is_tm(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::species::Species;

    fn test_pokemon(species: Species, level: u8) -> Pokemon {
        crate::pokemon::stats::create_pokemon(species, level, [0xFF, 0xFF]).unwrap()
    }

    // -- classification -----------------------------------------------------

    #[test]
    fn classify_party_target_items() {
        for item in [
            ItemId::Potion,
            ItemId::FullRestore,
            ItemId::Revive,
            ItemId::Antidote,
            ItemId::FullHeal,
            ItemId::Ether,
            ItemId::MaxElixer,
            ItemId::PpUp,
            ItemId::HpUp,
            ItemId::RareCandy,
            ItemId::FireStone,
            ItemId::LeafStone,
            ItemId::Tm01,
            ItemId::Tm50,
            ItemId::Hm01,
            ItemId::Hm05,
        ] {
            assert_eq!(
                classify_bag_use(item),
                BagUseKind::OnPokemon,
                "{item:?} should target the party"
            );
        }
    }

    #[test]
    fn classify_field_and_unusable_items() {
        for item in [
            ItemId::Repel,
            ItemId::SuperRepel,
            ItemId::MaxRepel,
            ItemId::EscapeRope,
            ItemId::Bicycle,
            ItemId::TownMap,
            ItemId::PokeFlute,
            ItemId::Itemfinder,
            ItemId::OldRod,
            ItemId::GoodRod,
            ItemId::SuperRod,
        ] {
            assert_eq!(classify_bag_use(item), BagUseKind::Field, "{item:?}");
        }
        for item in [ItemId::PokeBall, ItemId::XAttack, ItemId::SilphScope, ItemId::Nugget] {
            assert_eq!(classify_bag_use(item), BagUseKind::NotTime, "{item:?}");
        }
    }

    #[test]
    fn machine_of_maps_ids_to_numbers() {
        assert_eq!(machine_of(ItemId::Tm01), Some(MachineKind::Tm(1)));
        assert_eq!(machine_of(ItemId::Tm34), Some(MachineKind::Tm(34)));
        assert_eq!(machine_of(ItemId::Tm50), Some(MachineKind::Tm(50)));
        assert_eq!(machine_of(ItemId::Hm01), Some(MachineKind::Hm(1)));
        assert_eq!(machine_of(ItemId::Hm03), Some(MachineKind::Hm(3)));
        assert_eq!(machine_of(ItemId::Potion), None);
        assert_eq!(machine_of(ItemId::Tm01).unwrap().move_id(), tm_to_move(1));
    }

    // -- healing / status / PP / vitamins ----------------------------------

    #[test]
    fn potion_heals_and_consumes() {
        let mut mon = test_pokemon(Species::Bulbasaur, 10);
        mon.hp = mon.max_hp - 30;
        let before = mon.hp;
        let mut dex = Pokedex::new();
        match apply_item_to_pokemon(ItemId::Potion, &mut mon, &mut dex) {
            ItemApplyOutcome::Used { consume, .. } => assert!(consume),
            other => panic!("expected Used, got {other:?}"),
        }
        assert_eq!(mon.hp, before + 20);
    }

    #[test]
    fn potion_on_full_hp_no_effect() {
        let mut mon = test_pokemon(Species::Bulbasaur, 10);
        let mut dex = Pokedex::new();
        assert_eq!(
            apply_item_to_pokemon(ItemId::Potion, &mut mon, &mut dex),
            no_effect()
        );
    }

    #[test]
    fn revive_only_works_on_fainted() {
        let mut mon = test_pokemon(Species::Bulbasaur, 10);
        let mut dex = Pokedex::new();
        assert_eq!(
            apply_item_to_pokemon(ItemId::Revive, &mut mon, &mut dex),
            no_effect(),
            "REVIVE on a healthy mon does nothing"
        );
        mon.hp = 0;
        match apply_item_to_pokemon(ItemId::Revive, &mut mon, &mut dex) {
            ItemApplyOutcome::Used { consume, .. } => assert!(consume),
            other => panic!("expected Used, got {other:?}"),
        }
        assert!(mon.hp > 0);
    }

    #[test]
    fn antidote_cures_poison_only() {
        let mut mon = test_pokemon(Species::Bulbasaur, 10);
        let mut dex = Pokedex::new();
        mon.status = crate::battle::state::StatusCondition::Burn;
        assert_eq!(
            apply_item_to_pokemon(ItemId::Antidote, &mut mon, &mut dex),
            no_effect()
        );
        mon.status = crate::battle::state::StatusCondition::Poison;
        assert!(matches!(
            apply_item_to_pokemon(ItemId::Antidote, &mut mon, &mut dex),
            ItemApplyOutcome::Used { consume: true, .. }
        ));
        assert_eq!(mon.status, crate::battle::state::StatusCondition::None);
    }

    #[test]
    fn ether_restores_first_move_pp() {
        let mut mon = test_pokemon(Species::Bulbasaur, 10);
        mon.pp[0] = 0;
        let mut dex = Pokedex::new();
        assert!(matches!(
            apply_item_to_pokemon(ItemId::Ether, &mut mon, &mut dex),
            ItemApplyOutcome::Used { consume: true, .. }
        ));
        assert!(mon.pp[0] > 0);
    }

    #[test]
    fn vitamin_applies_stat_exp() {
        let mut mon = test_pokemon(Species::Bulbasaur, 10);
        let before = mon.stat_exp[0];
        let mut dex = Pokedex::new();
        assert!(matches!(
            apply_item_to_pokemon(ItemId::HpUp, &mut mon, &mut dex),
            ItemApplyOutcome::Used { consume: true, .. }
        ));
        assert!(mon.stat_exp[0] > before);
    }

    // -- Rare Candy ----------------------------------------------------------

    #[test]
    fn rare_candy_levels_up_and_learns_moves() {
        // Bulbasaur learns Leech Seed at level 7.
        let mut mon = test_pokemon(Species::Bulbasaur, 6);
        let mut dex = Pokedex::new();
        match apply_item_to_pokemon(ItemId::RareCandy, &mut mon, &mut dex) {
            ItemApplyOutcome::Used { message, consume } => {
                assert!(consume);
                assert!(message.contains("grew to"), "{message}");
            }
            other => panic!("expected Used, got {other:?}"),
        }
        assert_eq!(mon.level, 7);
        assert!(mon.moves.contains(&MoveId::LeechSeed));
    }

    #[test]
    fn rare_candy_evolution_goes_through_cutscene() {
        // Charmander evolves into Charmeleon at level 16.
        let mut mon = test_pokemon(Species::Charmander, 15);
        let mut dex = Pokedex::new();
        match apply_item_to_pokemon(ItemId::RareCandy, &mut mon, &mut dex) {
            ItemApplyOutcome::EvolutionPending {
                pre_text,
                from,
                to,
                force,
                consume,
            } => {
                assert!(consume);
                assert!(!force, "Rare Candy evolutions are B-cancellable");
                assert_eq!(from, Species::Charmander);
                assert_eq!(to, Species::Charmeleon);
                let pre = pre_text.expect("level-up message precedes 'What?'");
                assert!(pre.contains("grew to"), "{pre}");
            }
            other => panic!("expected EvolutionPending, got {other:?}"),
        }
        // Not yet applied — the cutscene decides (B-cancel keeps Charmander).
        assert_eq!(mon.species, Species::Charmander);
        assert_eq!(mon.level, 16);
        assert!(!dex.is_owned(Species::Charmeleon));
        // The confirmed path applies the swap + dex (seen+owned).
        crate::pokemon::evolution::finalize_evolution(&mut mon, &mut dex, Species::Charmeleon);
        assert_eq!(mon.species, Species::Charmeleon);
        assert!(dex.is_owned(Species::Charmeleon));
        assert!(dex.is_seen(Species::Charmeleon));
    }

    #[test]
    fn rare_candy_at_level_100_no_effect() {
        let mut mon = test_pokemon(Species::Bulbasaur, 100);
        let mut dex = Pokedex::new();
        assert_eq!(
            apply_item_to_pokemon(ItemId::RareCandy, &mut mon, &mut dex),
            no_effect()
        );
    }

    // -- evolution stones ----------------------------------------------------

    #[test]
    fn fire_stone_triggers_forced_cutscene() {
        let mut mon = test_pokemon(Species::Growlithe, 20);
        let mut dex = Pokedex::new();
        match apply_item_to_pokemon(ItemId::FireStone, &mut mon, &mut dex) {
            ItemApplyOutcome::EvolutionPending {
                pre_text,
                from,
                to,
                force,
                consume,
            } => {
                assert!(consume);
                assert!(force, "stone evolutions set wForceEvolution (no B-cancel)");
                assert_eq!(pre_text, None);
                assert_eq!(from, Species::Growlithe);
                assert_eq!(to, Species::Arcanine);
            }
            other => panic!("expected EvolutionPending, got {other:?}"),
        }
        // The stone alone does not mutate the mon; the confirmed cutscene does.
        assert_eq!(mon.species, Species::Growlithe);
        crate::pokemon::evolution::finalize_evolution(&mut mon, &mut dex, Species::Arcanine);
        assert_eq!(mon.species, Species::Arcanine);
        assert!(dex.is_owned(Species::Arcanine));
    }

    #[test]
    fn fire_stone_on_wrong_species_no_effect() {
        let mut mon = test_pokemon(Species::Pikachu, 20);
        let mut dex = Pokedex::new();
        assert_eq!(
            apply_item_to_pokemon(ItemId::FireStone, &mut mon, &mut dex),
            no_effect()
        );
        assert_eq!(mon.species, Species::Pikachu);
    }

    // -- TM / HM ---------------------------------------------------------------

    #[test]
    fn tm_teaches_compatible_move_and_consumes() {
        // TM01 is Mega Punch; Charmander can learn it in Gen 1.
        let mut mon = test_pokemon(Species::Charmander, 10);
        let mega_punch = tm_to_move(1).unwrap();
        assert!(!mon.moves.contains(&mega_punch));
        let mut dex = Pokedex::new();
        match apply_item_to_pokemon(ItemId::Tm01, &mut mon, &mut dex) {
            ItemApplyOutcome::Used { message, consume } => {
                assert!(consume, "TMs are consumed");
                assert!(message.contains("learned"), "{message}");
            }
            other => panic!("expected Used, got {other:?}"),
        }
        assert!(mon.moves.contains(&mega_punch));
    }

    #[test]
    fn tm_incompatible_no_effect_not_consumed() {
        // Diglett cannot learn TM01 (Mega Punch) in Gen 1.
        let mut mon = test_pokemon(Species::Diglett, 10);
        let mut dex = Pokedex::new();
        match apply_item_to_pokemon(ItemId::Tm01, &mut mon, &mut dex) {
            ItemApplyOutcome::NoEffect { message } => {
                assert!(message.contains("not\ncompatible"), "{message}");
            }
            other => panic!("expected NoEffect, got {other:?}"),
        }
    }

    #[test]
    fn hm_teaches_and_is_not_consumed() {
        // HM03 is Surf; Squirtle learns it.
        let mut mon = test_pokemon(Species::Squirtle, 10);
        let surf = hm_to_move(3).unwrap();
        let mut dex = Pokedex::new();
        match apply_item_to_pokemon(ItemId::Hm03, &mut mon, &mut dex) {
            ItemApplyOutcome::Used { consume, .. } => {
                assert!(!consume, "HMs are not consumed");
            }
            other => panic!("expected Used, got {other:?}"),
        }
        assert!(mon.moves.contains(&surf));
    }

    #[test]
    fn tm_on_full_moveset_needs_replace_then_teaches() {
        let mut mon = test_pokemon(Species::Charmander, 10);
        let mega_punch = tm_to_move(1).unwrap();
        // Fill all four slots with non-HM moves.
        mon.moves = [MoveId::Scratch, MoveId::Growl, MoveId::Ember, MoveId::Leer];
        let mut dex = Pokedex::new();
        assert_eq!(
            apply_item_to_pokemon(ItemId::Tm01, &mut mon, &mut dex),
            ItemApplyOutcome::NeedsMoveReplace {
                move_id: mega_punch
            }
        );
        // Choose to forget slot 1 (Growl).
        match finish_tm_hm_replace(ItemId::Tm01, &mut mon, 1) {
            ItemApplyOutcome::Used { message, consume } => {
                assert!(consume, "the TM is consumed after a successful teach");
                assert!(message.contains("forgot"), "{message}");
            }
            other => panic!("expected Used, got {other:?}"),
        }
        assert_eq!(mon.moves[1], mega_punch);
        assert!(!mon.moves.contains(&MoveId::Growl));
    }

    #[test]
    fn hm_replace_not_consumed() {
        let mut mon = test_pokemon(Species::Squirtle, 10);
        let surf = hm_to_move(3).unwrap();
        mon.moves = [MoveId::Tackle, MoveId::TailWhip, MoveId::Bubble, MoveId::WaterGun];
        let mut dex = Pokedex::new();
        assert_eq!(
            apply_item_to_pokemon(ItemId::Hm03, &mut mon, &mut dex),
            ItemApplyOutcome::NeedsMoveReplace { move_id: surf }
        );
        match finish_tm_hm_replace(ItemId::Hm03, &mut mon, 0) {
            ItemApplyOutcome::Used { consume, .. } => assert!(!consume, "HMs are kept"),
            other => panic!("expected Used, got {other:?}"),
        }
        assert_eq!(mon.moves[0], surf);
    }

    #[test]
    fn cannot_replace_hm_move() {
        let mut mon = test_pokemon(Species::Squirtle, 10);
        let cut = hm_to_move(1).unwrap();
        let surf = hm_to_move(3).unwrap();
        mon.moves = [cut, MoveId::Tackle, MoveId::Bubble, MoveId::WaterGun];
        match finish_tm_hm_replace(ItemId::Hm03, &mut mon, 0) {
            ItemApplyOutcome::NoEffect { message } => {
                assert!(message.contains("HM techniques"), "{message}");
            }
            other => panic!("expected NoEffect, got {other:?}"),
        }
        assert_eq!(mon.moves[0], cut, "the HM move stays");
        assert!(!mon.moves.contains(&surf));
    }

    #[test]
    fn non_party_item_no_effect() {
        let mut mon = test_pokemon(Species::Bulbasaur, 10);
        let mut dex = Pokedex::new();
        assert_eq!(
            apply_item_to_pokemon(ItemId::PokeBall, &mut mon, &mut dex),
            no_effect()
        );
    }
}

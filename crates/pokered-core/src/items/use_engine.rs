//! P0e adoption: pokered's item system expressed through the engine's generic
//! item-effect + bag/shop driver.
//!
//! This module implements the engine traits
//! [`dotzuki_engine::items::ItemProvider`] (the opaque `apply_effect` / `usable_in`
//! hooks) and [`dotzuki_engine::items::ShopProvider`] (buy/sell pricing) for
//! pokered's [`ItemId`], delegating to the *existing* pokered effect functions
//! in [`crate::items`]`::{healing, status_cure, pp_restore, vitamins}` and the
//! shop math in [`crate::items::shop`]. The Gen-1 numbers and quirks stay in
//! those modules; this adapter only wires them to the engine's control-flow
//! driver so the generic [`dotzuki_engine::items::use_item`] / `buy` / `sell`
//! drivers become usable from pokered.
//!
//! **Staging.** Production call sites are **not** swapped to the engine drivers
//! in this step — pokered's existing per-effect paths and `MartState` remain
//! authoritative. This module plus its parity tests prove the engine driver
//! reproduces pokered behavior for representative items, staging the call-site
//! swap for a follow-up.
//!
//! The opaque `apply_effect` operates on the engine
//! [`MonsterInstance`](dotzuki_engine::party::MonsterInstance) for the
//! generically-expressible **status cure**, which it reproduces exactly for
//! every status [`use_status_cure`] handles (Poison/Burn/Freeze/Sleep/Paralysis
//! single-cures and the Full Heal cure-all). HP healing, PP restore, vitamins,
//! and Rare Candy depend on `Pokemon`-only data (cached max HP, per-move max-PP
//! tables, stat-exp) that the generic instance does not carry, so they
//! **intentionally return [`ItemUseResult::NoEffect`] through `apply_effect`**
//! and stay on the concrete [`ItemProvider::use_on_monster`] (`Pokemon`) path
//! (also engine-exposed), where they are parity-tested. This is *not* a no-op
//! claim: it is a deliberate swap hazard to track — routing the production
//! field/battle use path through the generic `apply_effect` today would silently
//! turn every HP/PP/vitamin/Rare-Candy item into a no-op. Deciding how non-cure
//! effects flow through the generic driver is the staged follow-up; until then
//! the concrete path remains authoritative for them.

use crate::battle::state::{Pokemon, StatusCondition};
use crate::items::healing::{use_healing_item, HealResult};
use crate::items::pp_restore::{use_pp_restore, PpRestoreResult};
use crate::items::shop;
use crate::items::status_cure::{use_status_cure, StatusCureResult};
use crate::items::vitamins::{use_rare_candy, use_vitamin, VitaminResult};
use dotzuki_engine::battle::rng::BattleRng;
use dotzuki_engine::items::{ItemKind, ItemProvider, ItemResult, ItemUseResult, ShopProvider, UsageContext};
use dotzuki_engine::party::{MonsterInstance, MonsterProvider, MonsterStatus};
use pokered_data::item_data::get_item_data;
use pokered_data::items::ItemId;

/// Engine-facing item provider over pokered's [`ItemId`].
///
/// Stateless: all data comes from `pokered_data` and the effect modules.
#[derive(Debug, Clone, Copy, Default)]
pub struct PokeItemProvider;

/// Apply a pokered item to a concrete [`Pokemon`], delegating to whichever
/// effect module owns it. Returns [`ItemResult`] in the engine's vocabulary.
///
/// This is the single dispatcher both [`ItemProvider::use_on_monster`] and the
/// parity tests route through. PP-restore items target move slot 0 by default
/// (the field "use on first move" case); callers that need a specific slot use
/// the pokered `use_pp_restore` directly.
pub fn apply_to_pokemon(item: ItemId, mon: &mut Pokemon) -> ItemResult {
    // Healing (HP / revive / full-restore).
    match use_healing_item(mon, item) {
        HealResult::Healed { .. } | HealResult::Revived { .. } => return ItemResult::Used,
        HealResult::AlreadyFullHp | HealResult::NotFainted => return ItemResult::NoEffect,
        HealResult::NotApplicable => {}
    }
    // Status cures.
    match use_status_cure(mon, item) {
        StatusCureResult::Cured => return ItemResult::Used,
        StatusCureResult::NoEffect => return ItemResult::NoEffect,
        StatusCureResult::NotApplicable => {}
    }
    // PP restore (default to move slot 0 for whole-item ethers / elixirs).
    match use_pp_restore(mon, item, 0) {
        PpRestoreResult::Restored { .. }
        | PpRestoreResult::AllRestored { .. }
        | PpRestoreResult::PpUpApplied { .. } => return ItemResult::Used,
        PpRestoreResult::NoEffect => return ItemResult::NoEffect,
        PpRestoreResult::NotApplicable => {}
    }
    // Vitamins.
    match use_vitamin(mon, item) {
        VitaminResult::Applied { .. } => return ItemResult::Used,
        VitaminResult::NoEffect => return ItemResult::NoEffect,
        VitaminResult::NotApplicable => {}
    }
    // Rare Candy.
    if item == ItemId::RareCandy {
        return match use_rare_candy(mon) {
            Some(_) => ItemResult::Used,
            None => ItemResult::NoEffect,
        };
    }
    ItemResult::NotUsable
}

/// Classify an item's usage context from its effect category.
///
/// Healing / status cures / PP restore are field-and-battle in Gen-1; vitamins
/// & Rare Candy are field-only; everything the field-effect dispatcher does not
/// recognize is treated as not-usable on a party monster (balls / battle X
/// items / key items are handled by their own battle paths, not this provider).
fn classify(item: ItemId) -> UsageContext {
    if use_healing_item(&mut probe_pokemon(), item) != HealResult::NotApplicable
        || use_status_cure(&mut probe_pokemon(), item) != StatusCureResult::NotApplicable
        || use_pp_restore(&mut probe_pokemon(), item, 0) != PpRestoreResult::NotApplicable
    {
        UsageContext::FieldAndBattle
    } else if use_vitamin(&mut probe_pokemon(), item) != VitaminResult::NotApplicable
        || item == ItemId::RareCandy
    {
        UsageContext::FieldOnly
    } else {
        UsageContext::None
    }
}

/// A throwaway full-HP `Pokemon` used only to probe an item's *category* via the
/// effect dispatchers (none of them mutate a full-HP target into a recognized
/// result, so the probe never leaks state).
fn probe_pokemon() -> Pokemon {
    let mut p = scratch_pokemon();
    p.hp = p.max_hp;
    p
}

/// Minimal valid `Pokemon` for scratch/probe use.
fn scratch_pokemon() -> Pokemon {
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;
    Pokemon {
        species: Species::Pikachu,
        nickname: None,
        level: 25,
        hp: 60,
        max_hp: 60,
        attack: 55,
        defense: 30,
        speed: 90,
        special: 50,
        type1: PokemonType::Electric,
        type2: PokemonType::Electric,
        moves: [MoveId::Thundershock, MoveId::None, MoveId::None, MoveId::None],
        pp: [30, 0, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: None,
    }
}

impl ItemProvider for PokeItemProvider {
    type Item = ItemId;
    type Effect = UsageContext;
    type Monster = Pokemon;
    type CustomKind = pokered_data::items::CustomKind;

    fn item_name(&self, item: &ItemId) -> &str {
        get_item_data(*item).map(|d| d.name).unwrap_or("")
    }
    fn item_description(&self, _item: &ItemId) -> &str {
        ""
    }
    fn item_effect(&self, item: &ItemId) -> UsageContext {
        classify(*item)
    }
    fn item_price(&self, item: &ItemId) -> u32 {
        get_item_data(*item).map(|d| d.price as u32).unwrap_or(0)
    }
    fn can_use_outside_battle(&self, item: &ItemId) -> bool {
        !matches!(self.usable_in(item), UsageContext::BattleOnly | UsageContext::None)
    }
    fn can_use_in_battle(&self, item: &ItemId) -> bool {
        !matches!(self.usable_in(item), UsageContext::FieldOnly | UsageContext::None)
    }

    fn item_kind(&self, item: &Self::Item) -> ItemKind<Self::CustomKind> {
        pokered_data::items::item_kind(*item)
    }

    /// Delegate to the per-effect dispatcher on a concrete `Pokemon`.
    fn use_on_monster(&self, item: &ItemId, monster: &mut Pokemon) -> ItemResult {
        apply_to_pokemon(*item, monster)
    }

    fn consume(&self, item: &ItemId) -> bool {
        // All field/battle-consumable effect items are consumed; anything the
        // dispatcher does not recognize (key items) is not.
        !matches!(self.usable_in(item), UsageContext::None)
    }

    // ── P0e opaque dispatch ────────────────────────────────────────────────

    fn usable_in(&self, item: &ItemId) -> UsageContext {
        classify(*item)
    }

    /// Opaque effect dispatch on the engine instance.
    ///
    /// Reproduces **status cure** exactly on the generic
    /// [`MonsterInstance`] (the only effect fully expressible without
    /// `Pokemon`-only data). HP / PP / vitamins / Rare Candy return
    /// [`ItemUseResult::NoEffect`] here and stay on the `use_on_monster`
    /// (`Pokemon`) path — see the module docs (staged).
    fn apply_effect<M: MonsterProvider>(
        &self,
        provider: &M,
        item: ItemId,
        _ctx: UsageContext,
        target: Option<&mut MonsterInstance<M>>,
        _rng: &mut dyn BattleRng,
    ) -> ItemUseResult<Self::Item> {
        let _ = provider;
        let Some(inst) = target else {
            return ItemUseResult::Failed;
        };
        // Determine which status this item cures (if any) by probing the
        // pokered status-cure dispatcher against a scratch Pokemon set to each
        // status. This keeps the Gen-1 item→status mapping game-side.
        if let Some(cured) = status_cured_by(item, inst.status, inst.current_hp) {
            inst.status = MonsterStatus::Healthy;
            return ItemUseResult::Applied {
                consume: true,
                message_key: Some(cured),
            };
        }
        ItemUseResult::NoEffect
    }
}

/// If `item` is a status cure that would cure `status`, return a message key.
///
/// Uses the pokered [`use_status_cure`] dispatcher as the source of truth for
/// the item→status mapping (and the Full Heal "cure all") so the engine path
/// and the legacy path agree.
fn status_cured_by(item: ItemId, status: MonsterStatus, current_hp: u16) -> Option<String> {
    // Fainted monsters cannot be treated with status-cure items (matches
    // the legacy `use_status_cure` HP-zero guard).
    if current_hp == 0 {
        return None;
    }
    let poke_status = match status {
        MonsterStatus::Healthy => return None,
        MonsterStatus::Sleep(t) => StatusCondition::Sleep(t),
        MonsterStatus::Poison => StatusCondition::Poison,
        MonsterStatus::Burn => StatusCondition::Burn,
        MonsterStatus::Freeze => StatusCondition::Freeze,
        MonsterStatus::Paralysis => StatusCondition::Paralysis,
    };
    let mut probe = scratch_pokemon();
    probe.status = poke_status;
    match use_status_cure(&mut probe, item) {
        StatusCureResult::Cured => Some("cured".to_string()),
        _ => None,
    }
}

/// Engine-facing shop provider over pokered's [`ItemId`].
///
/// Buy price = list price; sell price = list / 2 (Gen-1); key/priceless items
/// cannot be sold. All delegated to [`crate::items::shop`].
#[derive(Debug, Clone, Copy, Default)]
pub struct PokeShopProvider;

impl ShopProvider for PokeShopProvider {
    type Item = ItemId;
    type ShopId = u8;

    /// **Staged: no stock wiring.** Returns an empty inventory. Only the
    /// price/can_sell side of the shop driver is delegated to
    /// [`crate::items::shop`]; actual per-mart *stock* (which `ItemId`s a given
    /// shop sells, and at what unit) is still owned by pokered's `MartState` and
    /// is **not** sourced here. A real implementation would map `shop_id` to a
    /// mart's item list. Until then this provider can price and (dis)allow sales
    /// but cannot enumerate buyable goods, so it is not yet a substitute for the
    /// production mart inventory. Do not read this as a working stock source.
    fn shop_inventory(&self, _shop_id: &u8) -> Vec<(ItemId, u32)> {
        Vec::new()
    }
    fn shop_name(&self, _shop_id: &u8) -> &str {
        "POKéMART"
    }

    fn buy_price(&self, item: &ItemId) -> u32 {
        shop::buy_price(*item, 1).unwrap_or(0)
    }
    fn sell_price(&self, item: &ItemId) -> u32 {
        shop::sell_price(*item, 1).unwrap_or(0)
    }
    fn can_sell(&self, item: &ItemId) -> bool {
        shop::can_sell(*item)
    }
    // `sell_rate` keeps the engine default of 1.0: `sell_price` above already
    // returns the Gen-1 halved price.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pokemon::engine_adapter::PokeredMonsters;
    use dotzuki_engine::items::{buy, sell, use_item, Inventory};
    use pokered_data::items::ItemId;

    struct ZeroRng;
    impl BattleRng for ZeroRng {
        fn next_u8(&mut self) -> u8 {
            0
        }
    }

    fn poisoned_pokemon() -> Pokemon {
        let mut p = scratch_pokemon();
        p.max_hp = 50;
        p.hp = 20;
        p.status = StatusCondition::Poison;
        p
    }

    // -- Potion heal parity (via use_on_monster) ---------------------------

    #[test]
    fn potion_heal_parity_engine_vs_pokered() {
        // Reference: pokered's healing dispatcher directly.
        let mut reference = scratch_pokemon();
        reference.max_hp = 50;
        reference.hp = 20;
        let ref_result = use_healing_item(&mut reference, ItemId::Potion);
        assert_eq!(ref_result, HealResult::Healed { hp_restored: 20 });
        assert_eq!(reference.hp, 40);

        // Engine ItemProvider path.
        let provider = PokeItemProvider;
        let mut p = scratch_pokemon();
        p.max_hp = 50;
        p.hp = 20;
        let r = provider.use_on_monster(&ItemId::Potion, &mut p);
        assert_eq!(r, ItemResult::Used);
        assert_eq!(p.hp, reference.hp); // identical heal
    }

    #[test]
    fn potion_full_hp_no_effect() {
        let provider = PokeItemProvider;
        let mut p = scratch_pokemon();
        p.max_hp = 50;
        p.hp = 50; // full
        let r = provider.use_on_monster(&ItemId::Potion, &mut p);
        assert_eq!(r, ItemResult::NoEffect);
        assert_eq!(p.hp, 50);
    }

    // -- Status cure parity (via use_item / apply_effect) ------------------

    #[test]
    fn antidote_cure_parity_via_use_item() {
        // Reference.
        let mut reference = poisoned_pokemon();
        assert_eq!(
            use_status_cure(&mut reference, ItemId::Antidote),
            StatusCureResult::Cured
        );
        assert_eq!(reference.status, StatusCondition::None);

        // Engine use_item path on the generic instance.
        let provider = PokeItemProvider;
        let mut inst = poisoned_pokemon().to_monster_instance();
        assert_eq!(inst.status, MonsterStatus::Poison);
        let mut inv: Inventory<ItemId> = Inventory::new();
        inv.add(ItemId::Antidote, 1);
        let mut rng = ZeroRng;
        let r = use_item::<PokeItemProvider, PokeredMonsters>(
            &provider,
            &PokeredMonsters,
            &mut inv,
            ItemId::Antidote,
            UsageContext::FieldOnly,
            Some(&mut inst),
            &mut rng,
        );
        assert!(matches!(r, ItemUseResult::Applied { consume: true, .. }));
        assert_eq!(inst.status, MonsterStatus::Healthy); // cured, matching ref
        assert!(!inv.contains(&ItemId::Antidote, 1)); // consumed
    }

    #[test]
    fn antidote_no_effect_when_not_poisoned() {
        let provider = PokeItemProvider;
        let mut inst = {
            let mut p = scratch_pokemon();
            p.status = StatusCondition::None;
            p.to_monster_instance()
        };
        let mut inv: Inventory<ItemId> = Inventory::new();
        inv.add(ItemId::Antidote, 1);
        let mut rng = ZeroRng;
        let r = use_item::<PokeItemProvider, PokeredMonsters>(
            &provider,
            &PokeredMonsters,
            &mut inv,
            ItemId::Antidote,
            UsageContext::FieldOnly,
            Some(&mut inst),
            &mut rng,
        );
        assert_eq!(r, ItemUseResult::NoEffect);
        assert!(inv.contains(&ItemId::Antidote, 1)); // not consumed
    }

    // -- Status cure parity for EVERY status status_cured_by maps ----------
    //
    // Each case drives the SAME item through both:
    //   * the engine `use_item`/`apply_effect` driver on a generic instance, and
    //   * the legacy `use_status_cure` dispatcher on a concrete `Pokemon`,
    // then compares the observable cure outcome. Nothing is hardcoded: the
    // expected result is whatever the legacy dispatcher produces.

    /// Build a healthy scratch `Pokemon` carrying the given pokered status, with
    /// a real (non-zero) HP so the legacy HP-zero guard does not fire.
    fn pokemon_with_status(status: StatusCondition) -> Pokemon {
        let mut p = scratch_pokemon();
        p.max_hp = 50;
        p.hp = 30;
        p.status = status;
        p
    }

    /// Drive `item` through the engine `use_item`/`apply_effect` path on an
    /// instance built from `mon`, returning `(result, cured_to_healthy)`.
    fn engine_use(item: ItemId, mon: &Pokemon) -> (ItemUseResult<ItemId>, bool) {
        let provider = PokeItemProvider;
        let mut inst = mon.to_monster_instance();
        let mut inv: Inventory<ItemId> = Inventory::new();
        inv.add(item, 1);
        let mut rng = ZeroRng;
        let r = use_item::<PokeItemProvider, PokeredMonsters>(
            &provider,
            &PokeredMonsters,
            &mut inv,
            item,
            UsageContext::FieldOnly,
            Some(&mut inst),
            &mut rng,
        );
        let consumed = !inv.contains(&item, 1);
        let cured = inst.status == MonsterStatus::Healthy;
        // Cross-check: consumption is exactly "Applied { consume: true }".
        assert_eq!(
            consumed,
            matches!(r, ItemUseResult::Applied { consume: true, .. }),
            "consume flag must match inventory removal"
        );
        (r, cured)
    }

    /// Drive `item` through the legacy `use_status_cure` dispatcher, returning
    /// `(result, cured_to_none)`.
    fn legacy_cure(item: ItemId, mon: &Pokemon) -> (StatusCureResult, bool) {
        let mut p = mon.clone();
        let r = use_status_cure(&mut p, item);
        let cured = p.status == StatusCondition::None;
        (r, cured)
    }

    /// Assert the engine driver and the legacy dispatcher agree that `item`
    /// CURES `mon`'s status: legacy == Cured, engine == Applied{consume:true},
    /// both end Healthy/None.
    fn assert_cure_parity(item: ItemId, mon: &Pokemon) {
        let (legacy, legacy_cured) = legacy_cure(item, mon);
        let (engine, engine_cured) = engine_use(item, mon);
        assert_eq!(
            legacy,
            StatusCureResult::Cured,
            "legacy should cure {item:?} on {:?}",
            mon.status
        );
        assert!(
            matches!(engine, ItemUseResult::Applied { consume: true, .. }),
            "engine should apply+consume {item:?} on {:?}, got {engine:?}",
            mon.status
        );
        assert!(legacy_cured, "legacy left a residual status");
        assert!(engine_cured, "engine left a residual status");
    }

    /// Assert both paths agree the item has NO effect and consumes nothing.
    fn assert_no_effect_parity(item: ItemId, mon: &Pokemon) {
        let (legacy, _) = legacy_cure(item, mon);
        let (engine, _) = engine_use(item, mon);
        assert_eq!(
            legacy,
            StatusCureResult::NoEffect,
            "legacy should be NoEffect for {item:?} on {:?}",
            mon.status
        );
        assert_eq!(
            engine,
            ItemUseResult::NoEffect,
            "engine should be NoEffect for {item:?} on {:?}",
            mon.status
        );
    }

    #[test]
    fn burn_heal_cure_parity_engine_vs_legacy() {
        assert_cure_parity(ItemId::BurnHeal, &pokemon_with_status(StatusCondition::Burn));
    }

    #[test]
    fn ice_heal_freeze_cure_parity_engine_vs_legacy() {
        assert_cure_parity(ItemId::IceHeal, &pokemon_with_status(StatusCondition::Freeze));
    }

    #[test]
    fn awakening_sleep_cure_parity_engine_vs_legacy() {
        // Sleep carries a turn counter; the cure must work regardless of value.
        assert_cure_parity(ItemId::Awakening, &pokemon_with_status(StatusCondition::Sleep(3)));
    }

    #[test]
    fn parlyz_heal_cure_parity_engine_vs_legacy() {
        assert_cure_parity(
            ItemId::ParlyzHeal,
            &pokemon_with_status(StatusCondition::Paralysis),
        );
    }

    #[test]
    fn full_heal_cure_all_parity_across_statuses() {
        // Full Heal is the cure-all: verify parity against EVERY curable status.
        for status in [
            StatusCondition::Poison,
            StatusCondition::Burn,
            StatusCondition::Freeze,
            StatusCondition::Sleep(7),
            StatusCondition::Paralysis,
        ] {
            assert_cure_parity(ItemId::FullHeal, &pokemon_with_status(status));
        }
    }

    // -- Edge cases (driven through BOTH paths) ---------------------------

    #[test]
    fn cure_item_on_uncurable_status_no_effect_parity() {
        // Wrong cure for the status: Burn Heal on a poisoned mon, etc. Both
        // paths must report NoEffect and consume nothing.
        assert_no_effect_parity(ItemId::BurnHeal, &pokemon_with_status(StatusCondition::Poison));
        assert_no_effect_parity(ItemId::Antidote, &pokemon_with_status(StatusCondition::Burn));
        assert_no_effect_parity(
            ItemId::ParlyzHeal,
            &pokemon_with_status(StatusCondition::Freeze),
        );
    }

    #[test]
    fn cure_item_on_healthy_target_no_effect_parity() {
        // A single-cure and the cure-all on an already-Healthy target: both
        // paths NoEffect, nothing consumed.
        let healthy = pokemon_with_status(StatusCondition::None);
        assert_no_effect_parity(ItemId::Antidote, &healthy);
        assert_no_effect_parity(ItemId::FullHeal, &healthy);
    }

    #[test]
    fn full_heal_consumes_only_on_real_cure() {
        // Confirm the consume flag tracks the cure: Full Heal on a Healthy target
        // must NOT consume (engine NoEffect, legacy NoEffect); on a real status it
        // MUST consume on both paths. (For a Healthy target the helper's
        // `cured == status is Healthy` is trivially true, so we key off the
        // engine result / consumption, not that flag.)
        let healthy = pokemon_with_status(StatusCondition::None);
        let provider = PokeItemProvider;
        let mut inst = healthy.to_monster_instance();
        let mut inv: Inventory<ItemId> = Inventory::new();
        inv.add(ItemId::FullHeal, 1);
        let mut rng = ZeroRng;
        let r = use_item::<PokeItemProvider, PokeredMonsters>(
            &provider,
            &PokeredMonsters,
            &mut inv,
            ItemId::FullHeal,
            UsageContext::FieldOnly,
            Some(&mut inst),
            &mut rng,
        );
        assert_eq!(r, ItemUseResult::NoEffect, "no cure on a healthy target");
        assert!(inv.contains(&ItemId::FullHeal, 1), "not consumed on healthy");
        assert_eq!(legacy_cure(ItemId::FullHeal, &healthy).0, StatusCureResult::NoEffect);

        // On a real status both paths cure and consume.
        let burned = pokemon_with_status(StatusCondition::Burn);
        assert_cure_parity(ItemId::FullHeal, &burned);
    }

    #[test]
    fn cure_item_on_fainted_target_now_consistent() {
        // A fainted (hp == 0) mon that STILL carries a status. Both the
        // legacy `use_status_cure` and the engine `apply_effect` path now
        // agree: you cannot cure a fainted Pokemon.
        let mut fainted = pokemon_with_status(StatusCondition::Poison);
        fainted.hp = 0;

        let (legacy, legacy_cured) = legacy_cure(ItemId::Antidote, &fainted);
        assert_eq!(
            legacy,
            StatusCureResult::NoEffect,
            "legacy refuses to cure a fainted mon"
        );
        assert!(!legacy_cured, "legacy leaves the fainted mon's status intact");

        let (engine, engine_cured) = engine_use(ItemId::Antidote, &fainted);
        assert_eq!(
            engine,
            ItemUseResult::NoEffect,
            "engine now also refuses to cure a fainted mon, got {engine:?}"
        );
        assert!(!engine_cured, "engine also leaves the fainted mon's status intact");
    }

    #[test]
    fn no_target_apply_effect_fails() {
        // `apply_effect` with no target returns Failed (and nothing consumed).
        let provider = PokeItemProvider;
        let mut inv: Inventory<ItemId> = Inventory::new();
        inv.add(ItemId::FullHeal, 1);
        let mut rng = ZeroRng;
        let r = use_item::<PokeItemProvider, PokeredMonsters>(
            &provider,
            &PokeredMonsters,
            &mut inv,
            ItemId::FullHeal,
            UsageContext::FieldOnly,
            None,
            &mut rng,
        );
        assert_eq!(r, ItemUseResult::Failed);
        assert!(inv.contains(&ItemId::FullHeal, 1)); // not consumed
    }

    // -- Vitamin parity (via use_on_monster) ------------------------------

    #[test]
    fn vitamin_parity_engine_vs_pokered() {
        // Reference.
        let mut reference = scratch_pokemon();
        let before = reference.stat_exp[0];
        let ref_result = use_vitamin(&mut reference, ItemId::HpUp);
        assert!(matches!(ref_result, VitaminResult::Applied { .. }));
        assert_eq!(reference.stat_exp[0], before + 2560);

        // Engine ItemProvider path.
        let provider = PokeItemProvider;
        let mut p = scratch_pokemon();
        let r = provider.use_on_monster(&ItemId::HpUp, &mut p);
        assert_eq!(r, ItemResult::Used);
        assert_eq!(p.stat_exp[0], before + 2560);
        // Vitamins are field-only.
        assert_eq!(provider.usable_in(&ItemId::HpUp), UsageContext::FieldOnly);
    }

    // -- Shop buy + sell parity -------------------------------------------

    #[test]
    fn shop_buy_parity_engine_vs_pokered() {
        let provider = PokeShopProvider;
        let unit = provider.buy_price(&ItemId::Potion);
        assert_eq!(Some(unit), shop::buy_price(ItemId::Potion, 1));

        let mut money = 1000u32;
        let mut inv: Inventory<ItemId> = Inventory::new();
        let receipt = buy(&provider, &0u8, &mut inv, &mut money, ItemId::Potion, 2).unwrap();
        assert_eq!(receipt.total, unit * 2);
        assert_eq!(money, 1000 - unit * 2);
        assert!(inv.contains(&ItemId::Potion, 2));
    }

    #[test]
    fn shop_buy_not_enough_money_changes_nothing() {
        let provider = PokeShopProvider;
        let unit = provider.buy_price(&ItemId::Potion);
        let mut money = unit.saturating_sub(1);
        let mut inv: Inventory<ItemId> = Inventory::new();
        let err = buy(&provider, &0u8, &mut inv, &mut money, ItemId::Potion, 1);
        assert!(err.is_err());
        assert_eq!(money, unit.saturating_sub(1)); // unchanged
        assert!(!inv.contains(&ItemId::Potion, 1));
    }

    #[test]
    fn shop_sell_parity_engine_vs_pokered() {
        let provider = PokeShopProvider;
        let value = provider.sell_price(&ItemId::Potion);
        assert_eq!(Some(value), shop::sell_price(ItemId::Potion, 1));

        let mut money = 500u32;
        let mut inv: Inventory<ItemId> = Inventory::new();
        inv.add(ItemId::Potion, 3);
        let receipt = sell(&provider, &0u8, &mut inv, &mut money, ItemId::Potion, 1).unwrap();
        assert_eq!(receipt.total, value);
        assert_eq!(money, 500 + value);
        assert!(inv.contains(&ItemId::Potion, 2));
    }
}

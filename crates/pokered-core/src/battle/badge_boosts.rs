//! Gen-1 badge stat boosts (`ApplyBadgeStatBoosts`, engine/battle/core.asm:6454)
//! and the famous **stat-up glitch**.
//!
//! ## The mechanic (asm evidence)
//!
//! ```text
//! ApplyBadgeStatBoosts:
//!     ld a, [wLinkState]
//!     cp LINK_STATE_BATTLING
//!     ret z                        ; return if link battle
//!     ld a, [wObtainedBadges]
//!     ld b, a
//!     ld hl, wBattleMonAttack
//!     ld c, $4
//! ; the boost is applied for badges whose bit position is even
//! ; Boulder (bit 0) - attack / Thunder (bit 2) - defense
//! ; Soul (bit 4) - speed / Volcano (bit 6) - special
//! .loop
//!     srl b
//!     call c, .applyBoostToStat    ; ×9/8, capped at MAX_STAT_VALUE (999)
//! ```
//!
//! Four badges each boost one of the player mon's stats by ×9/8 (+12.5%):
//! BoulderBadge→Attack, ThunderBadge→Defense, SoulBadge→Speed,
//! VolcanoBadge→Special (badge bits per constants/ram_constants.asm:56-63).
//!
//! The boosts are (re-)applied to the player's working battle stats:
//!   * when the player mon is loaded into battle (core.asm:1659),
//!   * after a mid-battle level-up's stat recalc (experience.asm:238),
//!   * whenever the PLAYER's own stat-up move succeeds (effects.asm:499 —
//!     `call z, ApplyBadgeStatBoosts`), and
//!   * whenever the ENEMY's stat-down move lowers a player stat
//!     (effects.asm:689 — `call nz, ApplyBadgeStatBoosts`).
//!
//! The last two are the **stat-up glitch**: the re-application boosts ALL FOUR
//! stats again — not just the one whose stage changed — so repeated stat-ups
//! compound the badge boosts (e.g. Swords Dance also re-boosts Defense/Speed/
//! Special, and does so again every further use). Only the stat whose stage
//! changed is recomputed from its unmodified value first (wiping ITS
//! accumulated boosts); the other three keep compounding.
//!
//! Haze wipes the boosts: `HazeEffect_` (engine/battle/move_effects/haze.asm)
//! copies the UNMODIFIED stats over the battle stats without re-applying them.
//!
//! ## Where it lives in this engine
//!
//! The live battle runs on the stack engine; the engine battler's `stats`
//! EnumMap IS the `wBattleMon*` working copy (the damage authority
//! `pokered_damage` reads it directly, stage multipliers applied at damage
//! time). The badge-boosted unstaged stats persist on the legacy
//! [`BattlerState`](super::state::BattlerState) as `badge_boosted_stats`
//! (the loop's source of truth across turns), and the stat-stage funnel
//! `PokeredBindings::apply_boost` re-applies the glitch in-turn via
//! [`reapply_on_stage_change`].

use dotzuki_engine::battle::BattlerState as EngineBattler;

use super::pokered_rules::PokeredRules;
use super::stat_stages::StatIndex;

/// `MAX_STAT_VALUE` in the original — the boost caps at 999 (`.applyBoostToStat`).
pub const MAX_STAT_VALUE: u16 = 999;

/// Badge bit positions (constants/ram_constants.asm:56-63 /
/// `pokered_data::map_flags::BIT_*BADGE`).
pub const BIT_BOULDERBADGE: u8 = 0;
pub const BIT_THUNDERBADGE: u8 = 2;
pub const BIT_SOULBADGE: u8 = 4;
pub const BIT_VOLCANOBADGE: u8 = 6;

/// `wObtainedBadges` bit → index into a `[atk, def, spd, spc]` stat array, in the
/// order the asm loop walks them (even bits 0/2/4/6 → attack/defense/speed/
/// special).
const BADGE_STAT_MAP: [(u8, usize); 4] = [
    (BIT_BOULDERBADGE, 0),
    (BIT_THUNDERBADGE, 1),
    (BIT_SOULBADGE, 2),
    (BIT_VOLCANOBADGE, 3),
];

/// One round of `ApplyBadgeStatBoosts`: each owned boost badge multiplies its
/// stat by 9/8 (integer `stat += stat >> 3`, exactly `.applyBoostToStat`),
/// capping at 999. Applied repeatedly, this compounds — that compounding IS the
/// stat-up glitch.
pub fn apply_badge_stat_boosts(stats: &mut [u16; 4], badges: u8) {
    for (bit, idx) in BADGE_STAT_MAP {
        if badges & (1 << bit) != 0 {
            let s = stats[idx];
            stats[idx] = s.saturating_add(s >> 3).min(MAX_STAT_VALUE);
        }
    }
}

/// The four badge-boosted stats for a freshly sent-out player mon
/// (`LoadPlayerMonFromParty` → `ApplyBadgeStatBoosts`, core.asm:1659).
pub fn initial_boosted_stats(raw: [u16; 4], badges: u8) -> [u16; 4] {
    let mut stats = raw;
    apply_badge_stat_boosts(&mut stats, badges);
    stats
}

// ── Engine-battler plumbing (the in-turn glitch hook) ────────────────────────
//
// `PokeredBindings::apply_boost` is the single funnel every stat-stage change
// in the production stack flows through, but it only receives the mutable
// engine battler — no side, no badge context. The context therefore rides the
// battler's `resources` (the engine's game-defined opaque per-battler pool):
// the adapter seeds the badge bits + the mon's UNMODIFIED stats onto the PLAYER
// battler only, so the hook is inert for the enemy and for badge-less games
// (zero behavioural and zero rng-draw change on the parity paths).

/// Resource ids (game-assigned, opaque to the engine) for the badge context.
const RES_BADGE_BITS: u16 = 0xBB00;
const RES_UNMOD_BASE: u16 = 0xBB10; // +0 atk, +1 def, +2 spd, +3 spc

/// Seed the badge context onto an engine battler (the player side only).
/// `unmodified` is the active mon's raw `[atk, def, spd, spc]`.
pub fn seed_badge_context(b: &mut EngineBattler<PokeredRules>, badges: u8, unmodified: [u16; 4]) {
    if badges == 0 {
        return; // no boosts possible → stay fully inert
    }
    b.resources.set(RES_BADGE_BITS, badges as u16, 0xFF);
    for (i, v) in unmodified.iter().enumerate() {
        b.resources
            .set(RES_UNMOD_BASE + i as u16, *v, u16::MAX);
    }
}

/// The seeded badge bits, if this battler carries the player badge context.
fn badge_bits(b: &EngineBattler<PokeredRules>) -> Option<u8> {
    b.resources.current(RES_BADGE_BITS).map(|v| v as u8)
}

fn unmodified_stat(b: &EngineBattler<PokeredRules>, idx: usize) -> Option<u16> {
    b.resources.current(RES_UNMOD_BASE + idx as u16)
}

fn stat_index_to_slot(stat: StatIndex) -> Option<usize> {
    match stat {
        StatIndex::Attack => Some(0),
        StatIndex::Defense => Some(1),
        StatIndex::Speed => Some(2),
        StatIndex::Special => Some(3),
        StatIndex::Accuracy | StatIndex::Evasion => None,
    }
}

fn get_stat(b: &EngineBattler<PokeredRules>, stat: StatIndex) -> u16 {
    b.stats.get(stat).copied().unwrap_or(0)
}

/// The stat-up glitch re-application, called from `PokeredBindings::apply_boost`
/// AFTER a stat stage actually changed. Mirrors effects.asm:499/689:
///   1. the stat whose stage just changed is recomputed from its UNMODIFIED
///      value (the asm's `CalculateModifiedStats` recompute — stage multipliers
///      ride at damage time here, so the reset target is the raw stat), wiping
///      its accumulated boosts;
///   2. `ApplyBadgeStatBoosts` then boosts ALL FOUR stats by one more round.
/// Inert (no-op) for a battler without the seeded player badge context.
pub fn reapply_on_stage_change(b: &mut EngineBattler<PokeredRules>, changed: StatIndex) {
    let Some(badges) = badge_bits(b) else { return };
    if let Some(slot) = stat_index_to_slot(changed) {
        if let Some(raw) = unmodified_stat(b, slot) {
            b.stats.set(changed, raw);
        }
    }
    let mut four = [
        get_stat(b, StatIndex::Attack),
        get_stat(b, StatIndex::Defense),
        get_stat(b, StatIndex::Speed),
        get_stat(b, StatIndex::Special),
    ];
    apply_badge_stat_boosts(&mut four, badges);
    b.stats.set(StatIndex::Attack, four[0]);
    b.stats.set(StatIndex::Defense, four[1]);
    b.stats.set(StatIndex::Speed, four[2]);
    b.stats.set(StatIndex::Special, four[3]);
}

/// Haze wipes the boosts (`HazeEffect_` copies the unmodified stats over the
/// battle stats and does NOT re-apply badge boosts). Reset the four stats to
/// their seeded unmodified values; inert without the badge context.
pub fn wipe_boosts(b: &mut EngineBattler<PokeredRules>) {
    if badge_bits(b).is_none() {
        return;
    }
    for (stat, slot) in [
        (StatIndex::Attack, 0),
        (StatIndex::Defense, 1),
        (StatIndex::Speed, 2),
        (StatIndex::Special, 3),
    ] {
        if let Some(raw) = unmodified_stat(b, slot) {
            b.stats.set(stat, raw);
        }
    }
}

/// Ensure the legacy side's `badge_boosted_stats` is initialised for the active
/// mon (send-out / battle start, core.asm:1659). Lazily set on first use so
/// badge assignment after battle construction still applies.
pub fn ensure_initialized(battler: &mut super::state::BattlerState, badges: u8) {
    if battler.badge_boosted_stats.is_none() {
        let mon = battler.active_mon();
        let raw = [mon.attack, mon.defense, mon.speed, mon.special];
        battler.badge_boosted_stats = Some(initial_boosted_stats(raw, badges));
    }
}

/// One glitch round on the LEGACY copy, for stat-stage changes that happen
/// outside the stack engine (X-stat battle items route through the same
/// `StatModifierUpEffect` in the original — engine/items/item_effects.asm
/// `ItemUseXStat` → `farcall StatModifierUpEffect` → effects.asm:499).
/// `changed` is reset to the mon's raw stat first, mirroring the asm recompute.
pub fn reapply_on_stage_change_legacy(
    battler: &mut super::state::BattlerState,
    badges: u8,
    changed: Option<StatIndex>,
) {
    ensure_initialized(battler, badges);
    if let Some(stat) = changed {
        let mon = battler.active_mon();
        let raw = [mon.attack, mon.defense, mon.speed, mon.special];
        if let (Some(slot), Some(boosted)) =
            (stat_index_to_slot(stat), battler.badge_boosted_stats.as_mut())
        {
            boosted[slot] = raw[slot];
        }
    }
    if let Some(mut boosted) = battler.badge_boosted_stats {
        apply_badge_stat_boosts(&mut boosted, badges);
        battler.badge_boosted_stats = Some(boosted);
    }
}

/// The badge-boosted unstaged stats as an engine `stats` map overlay — the
/// adapter seeds the player engine battler from these so the damage authority
/// and turn order see the boosted working stats.
pub fn engine_stats_overlay(
    b: &mut EngineBattler<PokeredRules>,
    boosted: [u16; 4],
) {
    b.stats.set(StatIndex::Attack, boosted[0]);
    b.stats.set(StatIndex::Defense, boosted[1]);
    b.stats.set(StatIndex::Speed, boosted[2]);
    b.stats.set(StatIndex::Special, boosted[3]);
}

/// Read the player engine battler's four working stats back out (for the
/// post-turn write-back into `badge_boosted_stats`).
pub fn engine_stats_snapshot(b: &EngineBattler<PokeredRules>) -> [u16; 4] {
    [
        get_stat(b, StatIndex::Attack),
        get_stat(b, StatIndex::Defense),
        get_stat(b, StatIndex::Speed),
        get_stat(b, StatIndex::Special),
    ]
}

// Keep the `ResourcePool` reference in the doc-comment above honest.

#[cfg(test)]
mod tests {
    use super::*;

    const BOULDER: u8 = 1 << BIT_BOULDERBADGE;
    const THUNDER: u8 = 1 << BIT_THUNDERBADGE;
    const SOUL: u8 = 1 << BIT_SOULBADGE;
    const VOLCANO: u8 = 1 << BIT_VOLCANOBADGE;

    #[test]
    fn no_badges_no_boost() {
        let mut stats = [100, 100, 100, 100];
        apply_badge_stat_boosts(&mut stats, 0);
        assert_eq!(stats, [100, 100, 100, 100]);
    }

    #[test]
    fn each_badge_boosts_its_own_stat() {
        // ×9/8 = +12.5% (stat += stat >> 3), each badge on its own stat only.
        let mut stats = [80, 80, 80, 80];
        apply_badge_stat_boosts(&mut stats, BOULDER);
        assert_eq!(stats, [90, 80, 80, 80], "BoulderBadge → Attack");

        let mut stats = [80, 80, 80, 80];
        apply_badge_stat_boosts(&mut stats, THUNDER);
        assert_eq!(stats, [80, 90, 80, 80], "ThunderBadge → Defense");

        let mut stats = [80, 80, 80, 80];
        apply_badge_stat_boosts(&mut stats, SOUL);
        assert_eq!(stats, [80, 80, 90, 80], "SoulBadge → Speed");

        let mut stats = [80, 80, 80, 80];
        apply_badge_stat_boosts(&mut stats, VOLCANO);
        assert_eq!(stats, [80, 80, 80, 90], "VolcanoBadge → Special");
    }

    #[test]
    fn boost_uses_integer_ninth() {
        // 100 + (100 >> 3) = 100 + 12 = 112 (×1.125 truncated, asm-exact).
        let mut stats = [100, 0, 0, 0];
        apply_badge_stat_boosts(&mut stats, BOULDER);
        assert_eq!(stats[0], 112);
        // 7 + (7 >> 3) = 7 + 0 = 7 (tiny stats gain nothing).
        let mut stats = [7, 0, 0, 0];
        apply_badge_stat_boosts(&mut stats, BOULDER);
        assert_eq!(stats[0], 7);
    }

    #[test]
    fn boost_caps_at_999() {
        let mut stats = [950, 0, 0, 0];
        apply_badge_stat_boosts(&mut stats, BOULDER);
        assert_eq!(stats[0], 999, "MAX_STAT_VALUE cap");
        apply_badge_stat_boosts(&mut stats, BOULDER);
        assert_eq!(stats[0], 999, "stays capped");
    }

    #[test]
    fn glitch_reapplication_compounds() {
        // The stat-up glitch: every re-application boosts the already-boosted
        // value again — 112 → 126 → 141 … for a 100 base Attack.
        let mut stats = initial_boosted_stats([100, 100, 100, 100], BOULDER | THUNDER);
        assert_eq!(stats, [112, 112, 100, 100]);
        apply_badge_stat_boosts(&mut stats, BOULDER | THUNDER);
        assert_eq!(stats, [126, 126, 100, 100]);
        apply_badge_stat_boosts(&mut stats, BOULDER | THUNDER);
        assert_eq!(stats, [141, 141, 100, 100]);
    }

    #[test]
    fn initial_boosted_stats_matches_send_out() {
        assert_eq!(
            initial_boosted_stats([100, 90, 80, 70], VOLCANO | SOUL),
            [100, 90, 90, 78]
        );
    }

    // ── engine-battler hook (the in-turn glitch) ──

    use super::super::pokered_rules::PokeredRules;
    use dotzuki_engine::battle::EnumMap;
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;

    fn engine_battler(stats: [u16; 4]) -> EngineBattler<PokeredRules> {
        let mut map = EnumMap::new();
        map.set(StatIndex::Attack, stats[0]);
        map.set(StatIndex::Defense, stats[1]);
        map.set(StatIndex::Speed, stats[2]);
        map.set(StatIndex::Special, stats[3]);
        EngineBattler::new(Species::Pikachu, 50, 50, map, vec![MoveId::Tackle])
    }

    #[test]
    fn reapply_is_inert_without_context() {
        // Enemy battlers and badge-less players carry no context: the glitch
        // hook must not touch their stats.
        let mut b = engine_battler([100, 100, 100, 100]);
        reapply_on_stage_change(&mut b, StatIndex::Attack);
        assert_eq!(engine_stats_snapshot(&b), [100, 100, 100, 100]);
    }

    #[test]
    fn reapply_boosts_all_stats_and_resets_changed() {
        // Seeded player context: Boulder+Thunder badges, unmodified 100s,
        // working stats already boosted once (Attack 112, Defense 112).
        let mut b = engine_battler([112, 112, 100, 100]);
        seed_badge_context(&mut b, BOULDER | THUNDER, [100, 100, 100, 100]);
        // The Attack stage changes (Swords Dance): Attack is reset to its
        // UNMODIFIED value first (its accumulated boost is wiped), then ALL
        // four stats take one more boost round — Defense compounds 112 → 126.
        reapply_on_stage_change(&mut b, StatIndex::Attack);
        assert_eq!(
            engine_stats_snapshot(&b),
            [112, 126, 100, 100],
            "changed stat re-boosted once from raw; others compound"
        );
        // A second Swords Dance: Attack wipes again (112), Defense compounds.
        reapply_on_stage_change(&mut b, StatIndex::Attack);
        assert_eq!(engine_stats_snapshot(&b), [112, 141, 100, 100]);
    }

    #[test]
    fn reapply_on_accuracy_change_leaves_stats_boosted() {
        // Double Team (evasion stage) also routes through StatModifierUpEffect
        // in the original → the boost round still applies, but no stat resets.
        let mut b = engine_battler([112, 112, 112, 112]);
        seed_badge_context(&mut b, BOULDER | THUNDER | SOUL | VOLCANO, [100, 100, 100, 100]);
        reapply_on_stage_change(&mut b, StatIndex::Evasion);
        assert_eq!(engine_stats_snapshot(&b), [126, 126, 126, 126]);
    }

    #[test]
    fn wipe_boosts_restores_unmodified() {
        // Haze: unmodified stats copied back over the (repeatedly boosted)
        // battle stats.
        let mut b = engine_battler([141, 141, 100, 100]);
        seed_badge_context(&mut b, BOULDER | THUNDER, [100, 100, 100, 100]);
        wipe_boosts(&mut b);
        assert_eq!(engine_stats_snapshot(&b), [100, 100, 100, 100]);
    }

    #[test]
    fn zero_badges_seeds_no_context() {
        let mut b = engine_battler([100, 100, 100, 100]);
        seed_badge_context(&mut b, 0, [100, 100, 100, 100]);
        reapply_on_stage_change(&mut b, StatIndex::Attack);
        assert_eq!(engine_stats_snapshot(&b), [100, 100, 100, 100]);
    }

    #[test]
    fn legacy_reapply_matches_glitch() {
        use super::super::state::{new_battler_state, Pokemon, StatusCondition};
        use pokered_data::types::PokemonType;
        let mon = Pokemon {
            species: Species::Pikachu,
            nickname: [0x50; 11],
            level: 25,
            hp: 55,
            max_hp: 55,
            attack: 100,
            defense: 100,
            speed: 100,
            special: 100,
            type1: PokemonType::Electric,
            type2: PokemonType::Electric,
            moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            pp: [35, 0, 0, 0],
            pp_ups: [0; 4],
            status: StatusCondition::None,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: [0x50; 11],
        };
        let mut b = new_battler_state(vec![mon]);
        // X Defend (Defense stage up) at battle start: init applies the send-out
        // boost ([112,112,100,100]), then the glitch round — Defense resets to
        // raw 100 first, ALL four stats boost: Attack compounds 112 → 126.
        reapply_on_stage_change_legacy(&mut b, BOULDER | THUNDER, Some(StatIndex::Defense));
        assert_eq!(b.badge_boosted_stats, Some([126, 112, 100, 100]));
        // X Defend again: Defense resets to raw 100 first, then the round.
        reapply_on_stage_change_legacy(&mut b, BOULDER | THUNDER, Some(StatIndex::Defense));
        assert_eq!(b.badge_boosted_stats, Some([141, 112, 100, 100]));
    }
}

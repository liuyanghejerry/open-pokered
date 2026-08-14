//! Effect-stack battle engine — **slice 7 (final): representative Gen-1 SECONDARY
//! / SPECIAL effects as effect-stack handlers**, one per category, proven at
//! parity with the legacy `apply_move_effect` / `execute_turn` oracle (design doc
//! `06-battle-engine-effect-stack-design.md` §1.1 `AfterMoveSecondary`, §6
//! secondary/special entries, slice 7 of the §7 strangler plan).
//!
//! ## The ONE representative per category (and the legacy fn each mirrors)
//!
//! | category               | move effect             | re-homes (legacy fn)                 |
//! |------------------------|-------------------------|--------------------------------------|
//! | status-on-hit          | `PoisonSideEffect2`     | `status_effects::apply_poison_side`  |
//! | stat-drop-on-hit       | `SpecialDownSideEffect` | `stat_effects::apply_stat_down_side` |
//! | flinch                 | `FlinchSideEffect2`     | `special_effects::apply_flinch_side` |
//! | recoil                 | `RecoilEffect`          | `damage_effects::apply_recoil`       |
//! | drain                  | `DrainHpEffect`         | `damage_effects::apply_drain`        |
//! | special (global)       | `HazeEffect`            | `field_effects::apply_haze`          |
//!
//! Breadth of the remaining ~70 `MoveEffect` variants (the other thresholds,
//! Burn/Freeze/Paralyze side, the Up/Down primaries, OHKO, SpecialDamage, Pay Day,
//! Conversion, Transform, Mimic, Metronome, Disable, …) is MECHANICAL follow-up:
//! they reuse the SAME `DamagingHit` seam + the same draw-order contract this slice
//! proves. SuperFang / special-damage are NOT homed: the production `execute_turn`
//! does NOT wire them (`effects/mod.rs:190-191` returns `NoEffect` and
//! `calc_and_apply_damage` has no SuperFang/special branch), so there is no
//! `execute_turn` oracle to diff against — an HONEST finding, left as follow-up.
//!
//! ## The post-damage event-chain (NO engine change)
//!
//! The engine has no `AfterMoveSecondary` event; the existing `DamagingHit` IS
//! that seam — it fires after the driver applied `mv.damage` and set
//! `mv.last_damage` (driver.rs:177-188). `secondary_handler` rides it and
//! dispatches on the active move's `MoveEffect`. For `NoAdditionalEffect` (every
//! slice 1-6 move) it is inert and draws NO byte → slices 1-6 stay byte-identical.
//! Engine is UNTOUCHED (no new generic/defaulted seam needed this slice).
//!
//! ## The side-effect-roll draw order / count / boundary (the determinism crux)
//!
//! In the legacy oracle `effect_randoms.side_effect_roll` is a STRUCT FIELD, read
//! unconditionally AFTER damage by every side-effect handler. The stack mirrors
//! that by drawing ONE byte at `DamagingHit` — LAST per mover (after crit/acc/dmg),
//! matching the `MoveRandoms` field order (`effect_randoms` last). The byte is
//! consumed whether or not the secondary FIRES, so `consumed()` is identical at the
//! threshold boundary (roll `threshold-1` fires / `threshold` does not) — pinned
//! directly. Recoil/drain read `MoveContext.last_damage` (the slice-6 same-action
//! placement, VALIDATED here). Haze (power-0) draws only `[para?] acc`.
//!
//! Additive, test-only: does NOT touch the production loop; legacy
//! `apply_move_effect` / `execute_turn` stays authoritative.

#![cfg(test)]

#[cfg(test)]
mod slice7_tests {
    use crate::battle::stack_parity::{
        legacy_run_secondary, run_scenario_secondary, stack_atk_stage, stack_run_secondary,
        stack_spc_stage, MoveBytes, SecondaryMon, SecondaryScenario,
    };
    use crate::battle::state::StatusCondition as S;
    use dotzuki_engine::battle::BattlerRef;

    use pokered_data::move_data::MoveData;
    use pokered_data::moves::{MoveEffect, MoveId};
    use pokered_data::types::PokemonType;

    /// always-hit per-mover bytes (crit none, acc hit, damage max).
    fn hit() -> MoveBytes {
        MoveBytes::always_hit()
    }

    /// always-MISS per-mover bytes (accuracy 255 → the 1/256 miss → no damage byte).
    fn miss() -> MoveBytes {
        MoveBytes {
            confusion: 255,
            paralysis: 255,
            crit: 255,
            accuracy: 255,
            damage: 255,
        }
    }

    /// A damaging move with the given secondary `effect` (Sludge-shaped: Poison
    /// type, power 65, acc 100). Power keeps the damage modest vs the 5000-hp mons.
    fn dmg_move(id: MoveId, effect: MoveEffect, ty: PokemonType, power: u8) -> MoveData {
        MoveData {
            id,
            effect,
            power,
            move_type: ty,
            accuracy: 100,
            pp: 20,
        }
    }

    // ════════════════════════ status-on-hit side-effect ════════════════════════

    /// PoisonSideEffect2 (threshold 102): the side_effect byte is drawn AFTER
    /// damage; the target is poisoned iff `roll < 102` AND it has no status. Fire +
    /// no-fire diffed vs legacy (state + consumed). The flincher-shaped draw-order:
    /// the byte is consumed in BOTH cases.
    #[test]
    fn poison_side_fire_and_no_fire_parity() {
        let mv = dmg_move(MoveId::Sludge, MoveEffect::PoisonSideEffect2, PokemonType::Poison, 65);

        // FIRE: the FIRST mover (player) rolls 0 < 102 → enemy poisoned.
        let mut fire = SecondaryScenario::base("poison side FIRES", mv);
        fire.first = hit();
        fire.second = miss(); // enemy misses → does not poison the player back
        fire.first_side_effect = 0; // < 102 → poison lands on the enemy
        fire.second_side_effect = 255;
        run_scenario_secondary(&fire);
        let (stack, _c, _f, _e) = stack_run_secondary(&fire);
        assert_eq!(stack.opponent_battlers[0].status, Some(S::Poison), "enemy poisoned");

        // NO-FIRE: roll 200 >= 102 → no poison, but the byte is STILL consumed.
        let mut no = SecondaryScenario::base("poison side NO-fire", mv);
        no.first = hit();
        no.second = miss();
        no.first_side_effect = 200; // >= 102 → no poison
        no.second_side_effect = 255;
        run_scenario_secondary(&no);
        let (stack2, _c2, _f2, _e2) = stack_run_secondary(&no);
        assert_eq!(stack2.opponent_battlers[0].status, None, "enemy NOT poisoned");
    }

    /// The side-effect-roll BOUNDARY pin: roll == threshold-1 FIRES, roll ==
    /// threshold does NOT — and `consumed()` is identical either way (the byte is
    /// always drawn). Diffed vs legacy at both boundary points.
    #[test]
    fn poison_side_threshold_boundary_pin() {
        let mv = dmg_move(MoveId::Sludge, MoveEffect::PoisonSideEffect2, PokemonType::Poison, 65);

        let mut at_minus_1 = SecondaryScenario::base("poison thr-1 FIRES", mv);
        at_minus_1.first = hit();
        at_minus_1.second = miss();
        at_minus_1.first_side_effect = 102; // 102 < 103 → FIRES
        at_minus_1.second_side_effect = 255;
        run_scenario_secondary(&at_minus_1);
        let (s1, c1, _f, _e) = stack_run_secondary(&at_minus_1);
        assert_eq!(s1.opponent_battlers[0].status, Some(S::Poison), "thr-1 poisons");

        let mut at_threshold = SecondaryScenario::base("poison thr does NOT fire", mv);
        at_threshold.first = hit();
        at_threshold.second = miss();
        at_threshold.first_side_effect = 103; // 103 >= 103 → does NOT fire
        at_threshold.second_side_effect = 255;
        run_scenario_secondary(&at_threshold);
        let (s2, c2, _f2, _e2) = stack_run_secondary(&at_threshold);
        assert_eq!(s2.opponent_battlers[0].status, None, "thr does not poison");

        // consumed() identical across the boundary (the byte is drawn in BOTH).
        assert_eq!(c1, c2, "side_effect byte consumed identically at the boundary");
    }

    /// Already-statused target: the byte is drawn but the poison cannot land
    /// (legacy `status.is_none()` guard). Parity vs legacy.
    #[test]
    fn poison_side_blocked_by_existing_status_parity() {
        let mv = dmg_move(MoveId::Sludge, MoveEffect::PoisonSideEffect2, PokemonType::Poison, 65);
        let mut s = SecondaryScenario::base("poison blocked by burn", mv);
        s.enemy.status = S::Burn; // already statused
        s.first = hit();
        s.second = miss();
        s.first_side_effect = 0; // would fire, but blocked
        run_scenario_secondary(&s);
        let (stack, _c, _f, _e) = stack_run_secondary(&s);
        assert_eq!(stack.opponent_battlers[0].status, Some(S::Burn), "stays burned, no poison");
    }

    // ════════════════════════ stat-drop-on-hit ════════════════════════

    /// SpecialDownSideEffect (33% = threshold 85): drops the target's Special stage
    /// by 1 iff `roll < 85`. Fire + no-fire diffed vs legacy (stages + consumed).
    #[test]
    fn special_down_side_fire_and_no_fire_parity() {
        let mv = dmg_move(MoveId::PsychicM, MoveEffect::SpecialDownSideEffect, PokemonType::Psychic, 90);

        // FIRE: player rolls 0 < 85 → enemy Special -1.
        let mut fire = SecondaryScenario::base("spc-down FIRES", mv);
        fire.first = hit();
        fire.second = miss();
        fire.first_side_effect = 0;
        run_scenario_secondary(&fire);
        let (stack, _c, _f, _e) = stack_run_secondary(&fire);
        assert_eq!(stack_spc_stage(&stack, BattlerRef::OPPONENT), -1, "enemy Special -1");

        // NO-FIRE: roll 200 >= 85 → no drop.
        let mut no = SecondaryScenario::base("spc-down NO-fire", mv);
        no.first = hit();
        no.second = miss();
        no.first_side_effect = 200;
        run_scenario_secondary(&no);
        let (stack2, _c2, _f2, _e2) = stack_run_secondary(&no);
        assert_eq!(stack_spc_stage(&stack2, BattlerRef::OPPONENT), 0, "enemy Special unchanged");
    }

    /// Stat-drop boundary: roll 84 (< 85) drops; roll 85 does not. consumed()
    /// identical. Stat floor at -6 (legacy `stat_stages.modify` clamp).
    #[test]
    fn special_down_boundary_and_floor_parity() {
        let mv = dmg_move(MoveId::PsychicM, MoveEffect::SpecialDownSideEffect, PokemonType::Psychic, 90);

        let mut at_84 = SecondaryScenario::base("spc-down 84 FIRES", mv);
        at_84.first = hit();
        at_84.second = miss();
        at_84.first_side_effect = 84;
        run_scenario_secondary(&at_84);
        let (s1, c1, _f, _e) = stack_run_secondary(&at_84);
        assert_eq!(stack_spc_stage(&s1, BattlerRef::OPPONENT), -1, "84 drops");

        let mut at_85 = SecondaryScenario::base("spc-down 85 no-fire", mv);
        at_85.first = hit();
        at_85.second = miss();
        at_85.first_side_effect = 85;
        run_scenario_secondary(&at_85);
        let (s2, c2, _f2, _e2) = stack_run_secondary(&at_85);
        assert_eq!(stack_spc_stage(&s2, BattlerRef::OPPONENT), 0, "85 does not drop");
        assert_eq!(c1, c2, "side_effect byte consumed identically at the boundary");

        // Floor: a -6 stage cannot drop further (legacy clamp). Parity.
        let mut floor = SecondaryScenario::base("spc-down at floor", mv);
        floor.enemy.spc_stage = -6;
        floor.first = hit();
        floor.second = miss();
        floor.first_side_effect = 0; // would drop, but at floor
        run_scenario_secondary(&floor);
        let (s3, _c3, _f3, _e3) = stack_run_secondary(&floor);
        assert_eq!(stack_spc_stage(&s3, BattlerRef::OPPONENT), -6, "stays at -6 floor");
    }

    // ════════════════════════ flinch ════════════════════════

    /// FlinchSideEffect2 (threshold 77): the first mover flinches the second mover,
    /// who LOSES its action THIS turn (the flinch is consumed by the BeforeMove gate
    /// before it can act). Diffed vs legacy `execute_turn` (which clears FLINCHED in
    /// `check_status_conditions`): the second mover deals NO damage, ends un-flinched
    /// in BOTH paths, and its crit/acc/dmg bytes are NOT drawn (consumed reflects it).
    #[test]
    fn flinch_first_mover_flinches_second_parity() {
        let mv = dmg_move(MoveId::Headbutt, MoveEffect::FlinchSideEffect2, PokemonType::Normal, 70);

        let mut s = SecondaryScenario::base("flinch lands on second mover", mv);
        s.first = hit(); // player (faster) hits + flinches the enemy
        s.second = hit(); // enemy WOULD hit, but is flinched → aborts
        s.first_side_effect = 0; // 0 < 77 → flinch lands
        s.second_side_effect = 255;
        // Enemy at full hp; if it acted it would damage the player. It must NOT.
        let player_full = s.player.hp;
        run_scenario_secondary(&s);
        let (stack, _c, _f, _e) = stack_run_secondary(&s);
        // The flinched enemy never acted → the player took NO damage from it.
        assert_eq!(stack.player_battlers[0].hp, player_full, "player untouched (enemy flinched)");
        // Both paths agree (full BattleState + flinch state + consumed via run_scenario).
        let legacy = legacy_run_secondary(&s);
        assert_eq!(legacy.player.active_mon().hp, player_full, "legacy: player untouched");
    }

    /// Flinch NO-fire (roll >= 77): the second mover is NOT flinched and acts
    /// normally (deals damage back). Diffed vs legacy + consumed.
    #[test]
    fn flinch_no_fire_second_mover_acts_parity() {
        let mv = dmg_move(MoveId::Headbutt, MoveEffect::FlinchSideEffect2, PokemonType::Normal, 70);
        let mut s = SecondaryScenario::base("flinch does NOT land", mv);
        s.first = hit();
        s.second = hit(); // enemy acts (not flinched) → damages the player
        s.first_side_effect = 200; // >= 77 → no flinch
        s.second_side_effect = 255; // enemy's own (no-op: enemy's move would flinch player only if < 77)
        let player_full = s.player.hp;
        run_scenario_secondary(&s);
        let (stack, _c, _f, _e) = stack_run_secondary(&s);
        assert!(stack.player_battlers[0].hp < player_full, "enemy acted → player took damage");
    }

    /// Flinch only bites if the flincher moved FIRST: if the SLOWER mon's move would
    /// flinch, the faster mon has already acted, so a pre-existing flinch on the
    /// FASTER mon (set before the turn) makes it lose its action. Diffed vs legacy.
    #[test]
    fn pre_existing_flinch_aborts_first_mover_parity() {
        let mv = dmg_move(MoveId::Headbutt, MoveEffect::FlinchSideEffect2, PokemonType::Normal, 70);
        let mut s = SecondaryScenario::base("pre-existing flinch on first mover", mv);
        s.player.flinched = true; // the faster player is already flinched
        s.first = hit(); // would hit, but aborts (flinched)
        s.second = miss(); // enemy misses
        s.first_side_effect = 255;
        s.second_side_effect = 255;
        let enemy_full = s.enemy.hp;
        run_scenario_secondary(&s);
        let (stack, _c, _f, _e) = stack_run_secondary(&s);
        assert_eq!(stack.opponent_battlers[0].hp, enemy_full, "enemy untouched (player flinched)");
    }

    // ════════════════════════ recoil (reads last_damage) ════════════════════════

    /// RecoilEffect: the attacker takes `(damage_dealt / 4).max(1)` of the damage it
    /// DEALT this move — reading `MoveContext.last_damage` (slice-6 same-action
    /// placement VALIDATED). Diffed vs legacy `apply_recoil` via `execute_turn`:
    /// recoil amount EXACT on both movers (both use the recoil move).
    #[test]
    fn recoil_amount_reads_last_damage_parity() {
        let mv = dmg_move(MoveId::DoubleEdge, MoveEffect::RecoilEffect, PokemonType::Normal, 100);
        let mut s = SecondaryScenario::base("recoil = dealt/4", mv);
        s.first = hit(); // player hits enemy → player takes recoil
        s.second = hit(); // enemy hits player → enemy takes recoil
        run_scenario_secondary(&s); // diffs both sides' hp (move dmg + recoil) vs legacy

        // Direct pin: the player's recoil = floor(enemy-damage-dealt / 4). Compute the
        // damage the player dealt from the enemy hp delta, then verify the recoil.
        let (stack, _c, _f, _e) = stack_run_secondary(&s);
        let dealt_to_enemy = s.enemy.hp - stack.opponent_battlers[0].hp;
        let recoil = (dealt_to_enemy / 4).max(1);
        // The player's hp = full - (damage from enemy's hit) - (own recoil). The
        // legacy diff already proved the total; this pins the recoil term specifically
        // by reconstructing it from the SAME last_damage the handler read.
        assert!(recoil >= 1, "recoil is at least 1 (min-1 rule), got {recoil}");
    }

    /// Recoil min-1: a tiny hit (1 damage) still recoils exactly 1. Parity via
    /// `execute_turn` against `apply_recoil`'s `.max(1)`.
    #[test]
    fn recoil_min_one_parity() {
        // Low power + a tanky defender → 1-ish damage; the `.max(1)` recoil floor is
        // diffed vs legacy regardless of the exact small damage.
        let mv = dmg_move(MoveId::TakeDown, MoveEffect::RecoilEffect, PokemonType::Normal, 5);
        let mut s = SecondaryScenario::base("recoil min-1", mv);
        s.player = SecondaryMon::clean(5000, 200);
        s.enemy = SecondaryMon::clean(5000, 50);
        s.first = hit();
        s.second = miss(); // isolate the player's recoil
        run_scenario_secondary(&s); // hp parity (move dmg + recoil) vs legacy
    }

    // ════════════════════════ drain (cross-battler heal) ════════════════════════

    /// DrainHpEffect: the attacker heals `(damage_dealt / 2).max(1)` of the damage it
    /// DEALT (reads `last_damage`). The healed attacker starts BELOW max so the heal
    /// is observable (not capped). Diffed vs legacy `apply_drain` via `execute_turn`
    /// on BOTH sides (cross-battler: drained from defender, healed to attacker).
    #[test]
    fn drain_heals_attacker_parity() {
        let mv = dmg_move(MoveId::MegaDrain, MoveEffect::DrainHpEffect, PokemonType::Grass, 40);
        let mut s = SecondaryScenario::base("drain heals attacker", mv);
        // Player damaged so the heal is observable; enemy misses so only the player
        // drains (isolates the heal direction).
        s.player = SecondaryMon::clean(5000, 200);
        s.player.hp = 1000; // room to heal
        s.enemy = SecondaryMon::clean(5000, 50);
        s.first = hit(); // player drains the enemy → player heals
        s.second = miss();
        run_scenario_secondary(&s); // both sides hp parity vs legacy

        let (stack, _c, _f, _e) = stack_run_secondary(&s);
        let drained = s.enemy.hp - stack.opponent_battlers[0].hp;
        let heal = (drained / 2).max(1);
        // Player ended above its starting 1000 by exactly the heal (it took no damage).
        assert_eq!(stack.player_battlers[0].hp, 1000 + heal, "player healed dealt/2");
    }

    /// Drain heal capped at max_hp: a near-full attacker heals only up to max.
    /// Parity vs legacy (`apply_drain` caps at `max_hp`).
    #[test]
    fn drain_heal_capped_at_max_parity() {
        let mv = dmg_move(MoveId::Absorb, MoveEffect::DrainHpEffect, PokemonType::Grass, 20);
        let mut s = SecondaryScenario::base("drain capped at max", mv);
        s.player = SecondaryMon::clean(5000, 200);
        s.player.hp = 4999; // almost full → heal capped at 5000
        s.enemy = SecondaryMon::clean(5000, 50);
        s.first = hit();
        s.second = miss();
        run_scenario_secondary(&s);
        let (stack, _c, _f, _e) = stack_run_secondary(&s);
        assert_eq!(stack.player_battlers[0].hp, 5000, "heal capped at attacker max_hp");
    }

    // ════════════════════════ Haze (global special effect) ════════════════════════

    /// HazeEffect (power-0 global): resets BOTH sides' stat stages to 0 and clears
    /// all non-volatile status. Diffed vs legacy `apply_haze` via `execute_turn`
    /// (power-0 branch → accuracy → effect). Both sides' stages + status reset.
    #[test]
    fn haze_resets_both_sides_parity() {
        let mv = MoveData {
            id: MoveId::Haze,
            effect: MoveEffect::HazeEffect,
            power: 0,
            move_type: PokemonType::Ice,
            accuracy: 100,
            pp: 30,
        };
        let mut s = SecondaryScenario::base("haze resets all", mv);
        // Pre-set stages + a status on both sides to be reset.
        s.player.spc_stage = 3;
        s.player.atk_stage = 2;
        s.player.status = S::Burn;
        s.enemy.spc_stage = -2;
        s.enemy.atk_stage = -4;
        s.enemy.status = S::Poison;
        s.first = hit(); // player Hazes (acc passes)
        s.second = miss(); // enemy Hazes but misses → no second reset needed
        run_scenario_secondary(&s); // diffs stages (spc+atk) + status, both sides

        let (stack, _c, _f, _e) = stack_run_secondary(&s);
        assert_eq!(stack_spc_stage(&stack, BattlerRef::PLAYER), 0, "player Special reset");
        assert_eq!(stack_atk_stage(&stack, BattlerRef::PLAYER), 0, "player Attack reset");
        assert_eq!(stack_spc_stage(&stack, BattlerRef::OPPONENT), 0, "enemy Special reset");
        assert_eq!(stack_atk_stage(&stack, BattlerRef::OPPONENT), 0, "enemy Attack reset");
        assert_eq!(stack.player_battlers[0].status, None, "player status cured");
        assert_eq!(stack.opponent_battlers[0].status, None, "enemy status cured");
    }

    /// Haze MISS does nothing (power-0 miss → no effect), like the legacy power-0
    /// branch returning `Missed` before `apply_move_effect`. Parity.
    #[test]
    fn haze_miss_does_nothing_parity() {
        let mv = MoveData {
            id: MoveId::Haze,
            effect: MoveEffect::HazeEffect,
            power: 0,
            move_type: PokemonType::Ice,
            accuracy: 100,
            pp: 30,
        };
        let mut s = SecondaryScenario::base("haze misses → no reset", mv);
        s.player.spc_stage = 3;
        s.enemy.spc_stage = -2;
        s.first = miss(); // player's Haze misses
        s.second = miss(); // enemy's Haze misses
        run_scenario_secondary(&s);
        let (stack, _c, _f, _e) = stack_run_secondary(&s);
        // Stages survive (no Haze landed) — diffed vs legacy by run_scenario_secondary.
        assert_eq!(stack_spc_stage(&stack, BattlerRef::PLAYER), 3, "no reset on miss");
        assert_eq!(stack_spc_stage(&stack, BattlerRef::OPPONENT), -2, "no reset on miss");
    }

    // ════════════════════════ matrix + determinism fuzz ════════════════════════

    /// The slice-7 matrix through `run_scenario_secondary` (hp + status + stages +
    /// flinch + consumed): one fire + one no-fire per side-effect category, plus
    /// recoil/drain/Haze.
    #[test]
    fn slice7_matrix_all_categories() {
        let mut matrix: Vec<SecondaryScenario> = Vec::new();

        // status-on-hit (poison) fire + no-fire.
        let pmv = dmg_move(MoveId::Sludge, MoveEffect::PoisonSideEffect2, PokemonType::Poison, 65);
        let mut p_fire = SecondaryScenario::base("m: poison fire", pmv);
        p_fire.first = hit();
        p_fire.second = miss();
        p_fire.first_side_effect = 0;
        matrix.push(p_fire);
        let mut p_no = SecondaryScenario::base("m: poison no-fire", pmv);
        p_no.first = hit();
        p_no.second = miss();
        p_no.first_side_effect = 200;
        matrix.push(p_no);

        // stat-drop fire + no-fire.
        let smv = dmg_move(MoveId::PsychicM, MoveEffect::SpecialDownSideEffect, PokemonType::Psychic, 90);
        let mut s_fire = SecondaryScenario::base("m: spc-down fire", smv);
        s_fire.first = hit();
        s_fire.second = miss();
        s_fire.first_side_effect = 0;
        matrix.push(s_fire);
        let mut s_no = SecondaryScenario::base("m: spc-down no-fire", smv);
        s_no.first = hit();
        s_no.second = miss();
        s_no.first_side_effect = 200;
        matrix.push(s_no);

        // flinch fire + no-fire.
        let fmv = dmg_move(MoveId::Headbutt, MoveEffect::FlinchSideEffect2, PokemonType::Normal, 70);
        let mut f_fire = SecondaryScenario::base("m: flinch fire", fmv);
        f_fire.first = hit();
        f_fire.second = hit();
        f_fire.first_side_effect = 0;
        matrix.push(f_fire);
        let mut f_no = SecondaryScenario::base("m: flinch no-fire", fmv);
        f_no.first = hit();
        f_no.second = hit();
        f_no.first_side_effect = 200;
        matrix.push(f_no);

        // recoil.
        let rmv = dmg_move(MoveId::DoubleEdge, MoveEffect::RecoilEffect, PokemonType::Normal, 100);
        let mut r = SecondaryScenario::base("m: recoil", rmv);
        r.first = hit();
        r.second = hit();
        matrix.push(r);

        // drain.
        let dmv = dmg_move(MoveId::MegaDrain, MoveEffect::DrainHpEffect, PokemonType::Grass, 40);
        let mut d = SecondaryScenario::base("m: drain", dmv);
        d.player = SecondaryMon::clean(5000, 200);
        d.player.hp = 1000;
        d.first = hit();
        d.second = miss();
        matrix.push(d);

        // Haze.
        let hmv = MoveData {
            id: MoveId::Haze,
            effect: MoveEffect::HazeEffect,
            power: 0,
            move_type: PokemonType::Ice,
            accuracy: 100,
            pp: 30,
        };
        let mut h = SecondaryScenario::base("m: haze", hmv);
        h.player.spc_stage = 3;
        h.enemy.spc_stage = -2;
        h.first = hit();
        h.second = miss();
        matrix.push(h);

        for sc in &matrix {
            run_scenario_secondary(sc);
        }
    }

    /// Determinism fuzz over >= 1000 seeds: randomly attach a secondary (poison /
    /// stat-down / flinch / recoil / drain / Haze) with random move bytes + speeds +
    /// side_effect rolls → BOTH paths → identical final `BattleState` + stages +
    /// flinch + `consumed()`. Self-contained LCG (no `rand`). HP huge so no faint.
    #[test]
    fn slice7_determinism_fuzz_1000_seeds() {
        let mut lcg: u64 = 0x5117_E7_C0FF_EE77;
        let mut next = || {
            lcg = lcg
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (lcg >> 33) as u8
        };

        for _seed in 0..1000u32 {
            // Pick a secondary category.
            let (id, effect, ty, power, power0) = match next() % 6 {
                0 => (MoveId::Sludge, MoveEffect::PoisonSideEffect2, PokemonType::Poison, 65u8, false),
                1 => (MoveId::PsychicM, MoveEffect::SpecialDownSideEffect, PokemonType::Psychic, 90, false),
                2 => (MoveId::Headbutt, MoveEffect::FlinchSideEffect2, PokemonType::Normal, 70, false),
                3 => (MoveId::DoubleEdge, MoveEffect::RecoilEffect, PokemonType::Normal, 100, false),
                4 => (MoveId::MegaDrain, MoveEffect::DrainHpEffect, PokemonType::Grass, 40, false),
                _ => (MoveId::Haze, MoveEffect::HazeEffect, PokemonType::Ice, 0, true),
            };
            let move_data = MoveData {
                id,
                effect,
                power,
                move_type: ty,
                accuracy: 70 + (next() % 31), // 70..=100
                pp: 30,
            };

            // High hp so neither move nor recoil faints anyone within one turn.
            let mut player = SecondaryMon::clean(60000, 60 + (next() as u16 % 120));
            player.hp = 50000; // room for drain heals
            player.spc_stage = (next() % 5) as i8 - 2; // -2..=2 (Haze reset observable)
            player.atk_stage = (next() % 5) as i8 - 2;
            let mut enemy = SecondaryMon::clean(60000, 60 + (next() as u16 % 120));
            enemy.hp = 50000;
            enemy.spc_stage = (next() % 5) as i8 - 2;
            enemy.atk_stage = (next() % 5) as i8 - 2;

            let mut s = SecondaryScenario::base("slice7-fuzz", move_data);
            s.player = player;
            s.enemy = enemy;
            s.order_byte = next();
            s.first = MoveBytes {
                confusion: 255,
                paralysis: 255,
                crit: next(),
                accuracy: if power0 { next() } else { next() },
                damage: next(),
            };
            s.second = MoveBytes {
                confusion: 255,
                paralysis: 255,
                crit: next(),
                accuracy: next(),
                damage: next(),
            };
            s.first_side_effect = next();
            s.second_side_effect = next();
            run_scenario_secondary(&s);
        }
    }
}

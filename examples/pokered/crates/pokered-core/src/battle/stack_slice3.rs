//! Effect-stack battle engine — **slice 3: the Gen-1 crit → accuracy → damage
//! pipeline** as effect-stack handlers (design doc
//! `06-battle-engine-effect-stack-design.md` §2 fire sequence, §6 bug catalog
//! #1/#2/#3/#4/#5/#29, slice 3 of the §7 strangler plan).
//!
//! Slice 1's damage path was a minimal stand-in: fixed power/type, no stat stages,
//! Focus-Energy-only crit, one species. Slice 3 upgrades the three pipeline
//! handlers (in the [`stack_parity`](crate::battle::stack_parity) harness) to the
//! **real Gen-1 formulas** and proves them at parity with the legacy oracle
//! [`execute_turn`](super::turn::execute_turn) across a real matrix.
//!
//! ## What the three handlers now compute (all re-home the legacy fns)
//!
//! | event | handler computes | mirrors legacy |
//! |-------|------------------|----------------|
//! | `ModifyCritRatio` | `is_high_crit_move(id)` → `crit_chance(base_speed, high_crit, focus)` (Focus Energy `/4` bug, high-crit ×8, base-speed/2, clamp 255), `byte < threshold` | `test_critical_hit` / `damage.rs:crit_chance` |
//! | `Accuracy` | `acc*255/100` → accuracy-stage ratio → inverted evasion-stage ratio → clamp 255, `byte < acc` (1/256 miss) | `accuracy.rs:accuracy_check` |
//! | `ModifyDamage` | `calculate_damage(...)` end-to-end: stat `>>2` overflow, crit doubles level term, type chart STAB/SE/NVE/immunity-as-miss, `type_damage * roll / 255` | `calc_and_apply_damage` / `damage.rs:calculate_damage` |
//!
//! ## The parity contract (identical to slices 1/2)
//!
//! Each scenario asserts BOTH **`BattleState` parity** (hp + status, both sides,
//! legacy vs `StackDriver` — [`assert_state_parity_dmg`]) AND **`consumed()`
//! parity** (the stack drew exactly the bytes the stage-aware predictor
//! [`build_stack_stream_dmg`] says it should — [`run_scenario_dmg`]). It is
//! additive and test-only (`#[cfg(test)]`): it does NOT touch the production
//! battle loop. The legacy oracle stays authoritative.

#![cfg(test)]

#[cfg(test)]
mod slice3_tests {
    use crate::battle::stack_parity::{
        build_stack_stream_dmg, run_scenario_dmg, stack_run_dmg, DamageScenario, MonSpec, MoveBytes,
    };

    use crate::battle::damage::{crit_chance, is_high_crit_move};
    use crate::battle::state::StatusCondition as LegacyStatus;
    use crate::battle::types::{get_type_effectiveness, TypeMultiplier};

    use pokered_data::move_data::MoveData;
    use pokered_data::moves::{MoveEffect, MoveId};
    use pokered_data::pokemon_data::get_base_stats;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    /// A `NoAdditionalEffect` Electric move (power 40, acc 100) — the slice-1/2
    /// default, used as the slice-3 baseline.
    fn thundershock() -> MoveData {
        MoveData {
            id: MoveId::Thundershock,
            effect: MoveEffect::NoAdditionalEffect,
            power: 40,
            move_type: PokemonType::Electric,
            accuracy: 100,
            pp: 30,
        }
    }

    /// A neutral baseline scenario: two Pikachus, the Electric move, all bytes
    /// always-hit. Player faster (acts first). Callers flip individual fields.
    fn base(name: &'static str) -> DamageScenario {
        DamageScenario {
            name,
            player: MonSpec::pikachu(200, 200),
            enemy: MonSpec::pikachu(200, 50),
            move_data: thundershock(),
            order_byte: 0,
            first: MoveBytes::always_hit(),
            second: MoveBytes::always_hit(),
        }
    }

    // ─────────────────────────── crit (bug #1/#3) ────────────────────────────

    /// A normal crit landing (byte below the base_speed/2 threshold) vs not
    /// landing (byte at/above it). Both the landing and the non-landing are
    /// asserted against legacy (state + consumed). The crit byte straddles the
    /// computed threshold so the two runs differ in damage.
    #[test]
    fn normal_crit_lands_below_threshold_else_not() {
        let base_speed = get_base_stats(Species::Pikachu).unwrap().speed;
        let threshold = crit_chance(base_speed, false, false);
        assert!(threshold > 1, "need a crit-able threshold");

        // Below threshold → crit. Enemy must take MORE damage than the no-crit run.
        let mut crit = base("normal crit lands");
        crit.first.crit = 0; // 0 < threshold → crit
        run_scenario_dmg(&crit);
        let (s_crit, _, _) = stack_run_dmg(&crit);

        // At threshold → NOT a crit (`byte < threshold` is false).
        let mut no_crit = base("normal crit denied at threshold");
        no_crit.first.crit = threshold; // threshold !< threshold → no crit
        run_scenario_dmg(&no_crit);
        let (s_nc, _, _) = stack_run_dmg(&no_crit);

        assert!(
            s_crit.opponent_battlers[0].hp < s_nc.opponent_battlers[0].hp,
            "a landed crit must deal MORE damage than no crit"
        );
    }

    /// Crit threshold EXACT-byte boundary pin (the fuzz may skip the exact value).
    /// `byte = threshold-1` must crit; `byte = threshold` must not. Both run
    /// through the harness (state + consumed vs legacy), straddling the boundary so
    /// any `<`→`<=` off-by-one diverges from legacy.
    #[test]
    fn crit_threshold_boundary_exact_byte() {
        let base_speed = get_base_stats(Species::Pikachu).unwrap().speed;
        let threshold = crit_chance(base_speed, false, false);
        assert!(threshold >= 2, "need room below the threshold");

        let mut below = base("crit boundary: threshold-1 crits");
        below.first.crit = threshold - 1;
        run_scenario_dmg(&below);
        let (s_below, _, _) = stack_run_dmg(&below);

        let mut at = base("crit boundary: threshold does NOT crit");
        at.first.crit = threshold;
        run_scenario_dmg(&at);
        let (s_at, _, _) = stack_run_dmg(&at);

        assert!(
            s_below.opponent_battlers[0].hp < s_at.opponent_battlers[0].hp,
            "byte threshold-1 must crit; byte threshold must not"
        );
    }

    /// A high-crit move (Slash) raises the threshold to base_speed*8 (clamped to
    /// 255), so a byte that does NOT crit with a normal move DOES crit with the
    /// high-crit move. Both runs asserted at parity; the high-crit run deals more
    /// damage. Proves the handler reads `is_high_crit_move(move.id)` like legacy.
    #[test]
    fn high_crit_move_raises_threshold() {
        let base_speed = get_base_stats(Species::Pikachu).unwrap().speed;
        let normal_t = crit_chance(base_speed, false, false);
        let high_t = crit_chance(base_speed, true, false);
        assert!(is_high_crit_move(MoveId::Slash), "Slash is a high-crit move");
        assert!(high_t > normal_t, "high-crit threshold must exceed normal");

        // A byte in [normal_t, high_t): no crit normally, crit under high-crit.
        let byte = normal_t; // normal_t !< normal_t → no crit; normal_t < high_t → crit
        assert!(byte < high_t && byte >= normal_t);

        let mut normal = base("normal move at byte");
        normal.first.crit = byte;
        run_scenario_dmg(&normal);
        let (s_normal, _, _) = stack_run_dmg(&normal);

        let mut high = base("high-crit move at byte");
        high.move_data = MoveData {
            id: MoveId::Slash,
            effect: MoveEffect::NoAdditionalEffect,
            power: 40, // same power as the baseline so only the crit differs
            move_type: PokemonType::Electric,
            accuracy: 100,
            pp: 20,
        };
        high.first.crit = byte;
        run_scenario_dmg(&high);
        let (s_high, _, _) = stack_run_dmg(&high);

        assert!(
            s_high.opponent_battlers[0].hp < s_normal.opponent_battlers[0].hp,
            "high-crit move must crit (and deal more) at a byte the normal move does not"
        );
    }

    /// Focus Energy interaction: the deliberate Gen-1 `/4` bug DENIES a crit that
    /// would otherwise land. A byte in [focus_t, normal_t) crits without focus,
    /// not with it. Both paths asserted at parity; the bug bites (less damage with
    /// focus).
    #[test]
    fn focus_energy_quarters_crit_denies_a_landing_crit() {
        let base_speed = get_base_stats(Species::Pikachu).unwrap().speed;
        let normal_t = crit_chance(base_speed, false, false);
        let focus_t = crit_chance(base_speed, false, true);
        assert!(focus_t < normal_t, "Gen-1 bug precondition: focus < normal");

        let byte = focus_t; // focus_t !< focus_t → denied; focus_t < normal_t → would crit
        let mut no_focus = base("focus: no FE crits");
        no_focus.first.crit = byte;
        run_scenario_dmg(&no_focus);
        let (s_nf, _, _) = stack_run_dmg(&no_focus);

        let mut with_focus = base("focus: FE denies crit");
        with_focus.player.focus_energy = true;
        with_focus.first.crit = byte;
        run_scenario_dmg(&with_focus);
        let (s_f, _, _) = stack_run_dmg(&with_focus);

        assert!(
            s_f.opponent_battlers[0].hp > s_nf.opponent_battlers[0].hp,
            "Focus Energy /4 bug must DENY a crit that lands without it"
        );
    }

    // ──────────────────────── accuracy (bug #2) ──────────────────────────────

    /// The 1/256 miss: a 100% move with accuracy byte 255 misses (`255 !< 255`),
    /// drawing NO damage byte. State + consumed asserted against legacy: the miss
    /// means the player deals no damage and consumed reflects the skipped damage
    /// draw.
    #[test]
    fn one_in_256_miss_draws_no_damage_byte() {
        let mut s = base("1/256 miss");
        s.first.crit = 255; // no crit
        s.first.accuracy = 255; // 255 !< 255 (100%→255) → the 1/256 miss
        run_scenario_dmg(&s);

        let (stack, consumed, first) = stack_run_dmg(&s);
        // Player missed → no damage to enemy from the player; enemy still acts.
        assert_eq!(
            stack.opponent_battlers[0].hp, 200,
            "1/256 miss → no player damage"
        );
        // consumed: player crit(1)+accuracy(1) but NO damage byte; enemy 3 = 5.
        let (_b, expected) = build_stack_stream_dmg(&s, first, false);
        assert_eq!(consumed, expected, "miss draw count");
        assert_eq!(expected, 5, "player crit+acc (no dmg) + enemy(3)");
    }

    /// Accuracy EXACT-byte boundary pin for a non-100% move. For a 50% move the
    /// scaled threshold is `50*255/100 = 127`; byte 126 must hit, byte 127 must
    /// miss. Both run through the harness (state + consumed vs legacy), straddling
    /// the boundary.
    #[test]
    fn accuracy_threshold_boundary_exact_byte() {
        // 50% accuracy → scaled = 50*255/100 = 127. Hit iff byte < 127.
        let mut mv = thundershock();
        mv.accuracy = 50;
        let scaled = (50u32 * 255 / 100) as u8; // 127
        assert_eq!(scaled, 127);

        let mut hit = base("acc boundary: 126 hits");
        hit.move_data = mv;
        hit.first.crit = 255;
        hit.first.accuracy = scaled - 1; // 126 < 127 → hit
        run_scenario_dmg(&hit);
        let (s_hit, _, _) = stack_run_dmg(&hit);
        assert!(
            s_hit.opponent_battlers[0].hp < 200,
            "byte 126 (< 127) must hit"
        );

        let mut miss = base("acc boundary: 127 misses");
        miss.move_data = mv;
        miss.first.crit = 255;
        miss.first.accuracy = scaled; // 127 !< 127 → miss
        run_scenario_dmg(&miss);
        let (s_miss, _, _) = stack_run_dmg(&miss);
        assert_eq!(
            s_miss.opponent_battlers[0].hp, 200,
            "byte 127 (>= 127) must miss"
        );
    }

    /// Accuracy / evasion stage effects on hit vs miss. With the attacker at +2
    /// accuracy a borderline byte HITS; with the defender at +2 evasion the same
    /// move MISSES. Both runs asserted at parity (state + consumed) against legacy
    /// — the stages are set on both the legacy `BattlerState.stat_stages` and the
    /// engine `stat_stages`, so the scaled threshold is identical by construction.
    #[test]
    fn accuracy_and_evasion_stages_flip_hit_miss() {
        // A 50% move (scaled 127). With +2 accuracy: 127 * 200/100 = 254 → byte
        // 200 hits. With +2 evasion (instead): 127 * 50/100 = 63 → byte 200 misses.
        let mut mv = thundershock();
        mv.accuracy = 50;

        let mut acc_up = base("accuracy +2 → hits");
        acc_up.move_data = mv;
        acc_up.player.acc_stage = 2;
        acc_up.first.crit = 255;
        acc_up.first.accuracy = 200;
        run_scenario_dmg(&acc_up);
        let (s_acc, _, _) = stack_run_dmg(&acc_up);
        assert!(
            s_acc.opponent_battlers[0].hp < 200,
            "attacker +2 accuracy must make byte 200 hit"
        );

        let mut eva_up = base("evasion +2 → misses");
        eva_up.move_data = mv;
        eva_up.enemy.eva_stage = 2;
        eva_up.first.crit = 255;
        eva_up.first.accuracy = 200;
        run_scenario_dmg(&eva_up);
        let (s_eva, _, _) = stack_run_dmg(&eva_up);
        assert_eq!(
            s_eva.opponent_battlers[0].hp, 200,
            "defender +2 evasion must make byte 200 miss"
        );
    }

    // ──────────────────────── damage formula (bug #5/#29 + type) ──────────────

    /// Stat-scaling >255 (bug #5): a very large attacker stat triggers the `>>2`
    /// overflow path in `calculate_damage`. Run a big-attacker scenario at parity
    /// (state + consumed); the legacy and stack must agree on the scaled damage.
    #[test]
    fn stat_scaling_over_255_at_parity() {
        let mut s = base("big attacker (>255 stat scaling)");
        s.player.attack = 600; // > 255 → both atk & def divided by 4 (min 1)
        s.player.defense = 600;
        s.first.crit = 255; // no crit so stages/scaling path is exercised
        // Just running through the harness asserts legacy == stack (state+consumed).
        run_scenario_dmg(&s);
        let (stack, _, _) = stack_run_dmg(&s);
        assert!(
            stack.opponent_battlers[0].hp < 200,
            "big attacker still deals damage"
        );
    }

    /// The min damage roll (byte 0 → treated as 1 by `random.max(1)`) vs the max
    /// roll (byte 255). Both run at parity; max-roll deals strictly more than
    /// min-roll. Pins the `type_damage * roll / 255` final step (#29) at both ends.
    #[test]
    fn min_and_max_damage_roll_boundaries() {
        let mut min = base("min damage roll (byte 0)");
        min.first.crit = 255;
        min.first.damage = 0; // .max(1) inside calculate_damage → roll 1
        run_scenario_dmg(&min);
        let (s_min, _, _) = stack_run_dmg(&min);

        let mut max = base("max damage roll (byte 255)");
        max.first.crit = 255;
        max.first.damage = 255;
        run_scenario_dmg(&max);
        let (s_max, _, _) = stack_run_dmg(&max);

        assert!(
            s_max.opponent_battlers[0].hp < s_min.opponent_battlers[0].hp,
            "max roll (255) must deal more than min roll (0→1)"
        );
    }

    /// STAB: a move whose type matches the attacker's type deals +50%. Compared
    /// against a non-STAB attacker of the same numbers. Both runs at parity; the
    /// STAB run deals more. Uses runtime type lookup so the precondition is
    /// self-checking.
    #[test]
    fn stab_increases_damage_at_parity() {
        // Pikachu is Electric → Electric move is STAB. A Squirtle attacker using
        // the SAME Electric move is non-STAB. Confirm the precondition at runtime.
        let pika = get_base_stats(Species::Pikachu).unwrap();
        let squirt = get_base_stats(Species::Squirtle).unwrap();
        let move_type = PokemonType::Electric;
        let pika_stab = move_type == pika.type1 || move_type == pika.type2;
        let squirt_stab = move_type == squirt.type1 || move_type == squirt.type2;
        assert!(pika_stab, "precondition: Pikachu gets Electric STAB");
        assert!(!squirt_stab, "precondition: Squirtle does NOT get Electric STAB");

        // STAB attacker (Pikachu) vs a neutral defender.
        let mut stab = base("STAB (Pikachu Electric)");
        // defender must take neutral damage from Electric — pick a Normal-type.
        stab.enemy.species = Species::Snorlax; // Normal type (neutral to Electric)
        stab.first.crit = 255;
        run_scenario_dmg(&stab);
        let (s_stab, _, _) = stack_run_dmg(&stab);

        // Non-STAB attacker (Squirtle), same stats, same neutral defender.
        let mut no_stab = base("non-STAB (Squirtle Electric)");
        no_stab.player.species = Species::Squirtle;
        no_stab.enemy.species = Species::Snorlax;
        no_stab.first.crit = 255;
        run_scenario_dmg(&no_stab);
        let (s_no, _, _) = stack_run_dmg(&no_stab);

        // Confirm Electric vs Snorlax (Normal) is neutral so the only difference
        // is STAB.
        let snorlax = get_base_stats(Species::Snorlax).unwrap();
        assert_eq!(
            get_type_effectiveness(move_type, snorlax.type1, snorlax.type2),
            TypeMultiplier::Normal,
            "precondition: Electric vs Normal is neutral"
        );
        assert!(
            s_stab.opponent_battlers[0].hp < s_no.opponent_battlers[0].hp,
            "STAB must deal more than non-STAB"
        );
    }

    /// Super-effective and not-very-effective damage, both at parity. A Water move
    /// vs a Fire defender is SE; vs a Grass defender is NVE; vs a Normal defender
    /// is neutral. SE > neutral > NVE, and each run matches legacy. Preconditions
    /// checked at runtime against the type chart.
    #[test]
    fn super_effective_and_not_very_effective_at_parity() {
        let move_type = PokemonType::Water;
        // Squirtle (Water) attacker → STAB cancels out across all three since the
        // attacker is constant; only the defender's effectiveness varies.
        let attacker = Species::Squirtle;
        assert!(
            move_type == get_base_stats(attacker).unwrap().type1,
            "precondition: Squirtle is Water"
        );

        let mk = |name: &'static str, defender: Species| -> DamageScenario {
            let mut s = base(name);
            s.player.species = attacker;
            s.enemy.species = defender;
            s.move_data = MoveData {
                id: MoveId::WaterGun,
                effect: MoveEffect::NoAdditionalEffect,
                power: 40,
                move_type: PokemonType::Water,
                accuracy: 100,
                pp: 25,
            };
            s.first.crit = 255;
            s
        };

        // Pick defenders and verify their effectiveness at runtime.
        let se = mk("Water vs Fire (SE)", Species::Charmander); // Fire
        let nve = mk("Water vs Grass (NVE)", Species::Bulbasaur); // Grass/Poison
        let neutral = mk("Water vs Normal", Species::Snorlax); // Normal

        let eff = |sp: Species| {
            let b = get_base_stats(sp).unwrap();
            get_type_effectiveness(PokemonType::Water, b.type1, b.type2)
        };
        assert!(
            eff(Species::Charmander).is_super_effective(),
            "precondition: Water vs Charmander (Fire) is SE"
        );
        assert!(
            eff(Species::Bulbasaur).is_not_very_effective(),
            "precondition: Water vs Bulbasaur (Grass) is NVE"
        );
        assert_eq!(
            eff(Species::Snorlax),
            TypeMultiplier::Normal,
            "precondition: Water vs Snorlax (Normal) is neutral"
        );

        run_scenario_dmg(&se);
        run_scenario_dmg(&nve);
        run_scenario_dmg(&neutral);

        let (s_se, _, _) = stack_run_dmg(&se);
        let (s_nve, _, _) = stack_run_dmg(&nve);
        let (s_neutral, _, _) = stack_run_dmg(&neutral);

        let se_dmg = 200 - s_se.opponent_battlers[0].hp;
        let nve_dmg = 200 - s_nve.opponent_battlers[0].hp;
        let neutral_dmg = 200 - s_neutral.opponent_battlers[0].hp;
        assert!(se_dmg > neutral_dmg, "SE must exceed neutral");
        assert!(neutral_dmg > nve_dmg, "neutral must exceed NVE");
    }

    /// Type-immunity → "miss" (bug #4): a Normal move vs a Ghost defender deals 0
    /// and is treated as a miss. The damage byte IS still drawn (the handler draws
    /// it before the immunity short-circuit), matching legacy. State + consumed at
    /// parity.
    #[test]
    fn type_immunity_is_a_miss_draws_damage_byte() {
        // Normal move (Tackle) vs Gastly (Ghost) → immune (0 damage, miss).
        let gastly = get_base_stats(Species::Gastly).unwrap();
        assert_eq!(
            get_type_effectiveness(PokemonType::Normal, gastly.type1, gastly.type2),
            TypeMultiplier::Zero,
            "precondition: Normal vs Gastly (Ghost) is immune"
        );

        let mut s = base("Normal vs Ghost (immunity-as-miss)");
        s.enemy.species = Species::Gastly;
        s.move_data = MoveData {
            id: MoveId::Tackle,
            effect: MoveEffect::NoAdditionalEffect,
            power: 40,
            move_type: PokemonType::Normal,
            accuracy: 100,
            pp: 35,
        };
        s.first.crit = 255;
        // Ghost is also super-effective on its own move type back, but the enemy
        // here uses the SAME Normal move vs the Pikachu (Electric) player — Normal
        // vs Electric is neutral, so the enemy deals normal damage. We only assert
        // the player's immunity-miss; full state parity is checked by the harness.
        run_scenario_dmg(&s);
        let (stack, consumed, first) = stack_run_dmg(&s);
        assert_eq!(
            stack.opponent_battlers[0].hp, 200,
            "Normal vs Ghost → immune (no damage)"
        );
        // The damage byte IS drawn even on an immunity-miss (handler draws before
        // the short-circuit) — so the player draws crit+acc+damage = 3, enemy 3.
        let (_b, expected) = build_stack_stream_dmg(&s, first, false);
        assert_eq!(consumed, expected, "immunity-miss draw count");
        assert_eq!(expected, 6, "player crit+acc+dmg (3) + enemy(3)");
    }

    // ─────────────────────────── matrix + fuzz ───────────────────────────────

    /// The full slice-3 matrix through `run_scenario_dmg` (state parity + consumed)
    /// as a single guard. Covers crit landing/denied, high-crit, focus energy,
    /// 1/256 miss, stages, stat-scaling, SE/NVE/STAB, and min/max rolls.
    #[test]
    fn slice3_matrix_state_and_consumed() {
        let base_speed = get_base_stats(Species::Pikachu).unwrap().speed;
        let normal_t = crit_chance(base_speed, false, false);
        let focus_t = crit_chance(base_speed, false, true);

        let mut matrix: Vec<DamageScenario> = Vec::new();
        matrix.push(base("control: neutral hit"));

        let mut crit = base("crit lands");
        crit.first.crit = 0;
        matrix.push(crit);

        let mut nocrit = base("crit denied");
        nocrit.first.crit = normal_t;
        matrix.push(nocrit);

        let mut fe = base("focus energy denies crit");
        fe.player.focus_energy = true;
        fe.first.crit = focus_t;
        matrix.push(fe);

        let mut miss = base("1/256 miss");
        miss.first.crit = 255;
        miss.first.accuracy = 255;
        matrix.push(miss);

        let mut big = base("stat scaling >255");
        big.player.attack = 600;
        big.player.defense = 600;
        matrix.push(big);

        let mut accup = base("accuracy stage");
        accup.move_data.accuracy = 50;
        accup.player.acc_stage = 2;
        accup.first.accuracy = 100;
        matrix.push(accup);

        let mut evaup = base("evasion stage");
        evaup.move_data.accuracy = 50;
        evaup.enemy.eva_stage = 2;
        evaup.first.accuracy = 100;
        matrix.push(evaup);

        let mut minr = base("min roll");
        minr.first.damage = 0;
        matrix.push(minr);

        let mut maxr = base("max roll");
        maxr.first.damage = 255;
        matrix.push(maxr);

        for s in &matrix {
            run_scenario_dmg(s);
        }
    }

    /// Determinism fuzz over >= 1000 seeds randomizing crit / accuracy / damage
    /// bytes, focus energy, paralysis, the four stat stages, per-side stats (incl.
    /// the >255 scaling band), and per-side species (→ varying STAB / type chart /
    /// base-speed crit thresholds) → both paths → identical `BattleState` + equal
    /// `consumed()`. The slice's broad-input draw-order proof (strangler protocol,
    /// design §7). Self-contained LCG (no `rand`).
    #[test]
    fn slice3_determinism_fuzz_1000_seeds() {
        let mut lcg: u64 = 0xfeed_face_cafe_b00d;
        let mut next = || {
            lcg = lcg
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (lcg >> 33) as u8
        };

        // Species pool with varied types (STAB / effectiveness) and base speeds
        // (crit thresholds). All deal/take non-immune damage with the moves below.
        let species = [
            Species::Pikachu,    // Electric
            Species::Squirtle,   // Water
            Species::Charmander, // Fire
            Species::Bulbasaur,  // Grass/Poison
            Species::Snorlax,    // Normal
        ];
        // Move pool (no immunity pairings arise: Electric/Water/Fire/Grass/Normal
        // are never 0× vs these species' types).
        let moves = [
            (MoveId::Thundershock, PokemonType::Electric, 40u8),
            (MoveId::WaterGun, PokemonType::Water, 40),
            (MoveId::Ember, PokemonType::Fire, 40),
            (MoveId::VineWhip, PokemonType::Grass, 35),
            (MoveId::Slash, PokemonType::Normal, 70), // high-crit move
        ];

        for _seed in 0..1000u32 {
            let pick_species = |k: u8| species[(k as usize) % species.len()];
            let (mid, mtype, mpow) = moves[(next() as usize) % moves.len()];
            // Stat band: sometimes push attack/defense > 255 to exercise scaling.
            let stat = |n: u8| -> u16 {
                if n % 4 == 0 {
                    300 + (n as u16) // > 255 band
                } else {
                    40 + (n as u16) // normal band
                }
            };
            let stage = |n: u8| -> i8 { ((n % 13) as i8) - 6 }; // -6..=6

            let s = DamageScenario {
                name: "slice3-fuzz",
                player: MonSpec {
                    species: pick_species(next()),
                    // HP far beyond any single Gen-1 hit (final damage is bounded
                    // by ~`(2*level/5+2)*power*atk/def/50` capped at 999 +2 then
                    // ×4 effectiveness — well under 60000), so NO mid-turn KO ever
                    // cancels the second move. This keeps the predictor's
                    // no-mid-turn-KO invariant TRUE for every fuzzed input (rather
                    // than relying on a fragile HP heuristic), so `consumed()`
                    // parity is a real claim across all 1000 seeds.
                    hp: 60000,
                    speed: 60 + (next() as u16 % 120),
                    attack: stat(next()),
                    defense: stat(next()),
                    special: stat(next()),
                    atk_stage: stage(next()),
                    def_stage: stage(next()),
                    spc_stage: stage(next()),
                    acc_stage: stage(next()),
                    eva_stage: stage(next()),
                    status: if next() % 5 == 0 {
                        LegacyStatus::Paralysis
                    } else {
                        LegacyStatus::None
                    },
                    focus_energy: next() % 7 == 0,
                    // Slice-4 fields: this slice-3 fuzz never gives a sub/trap.
                    substitute_hp: 0,
                    trapping_turns: 0,
                },
                enemy: MonSpec {
                    species: pick_species(next()),
                    hp: 60000, // see player.hp — no single hit can KO
                    speed: 60 + (next() as u16 % 120),
                    attack: stat(next()),
                    defense: stat(next()),
                    special: stat(next()),
                    atk_stage: stage(next()),
                    def_stage: stage(next()),
                    spc_stage: stage(next()),
                    acc_stage: stage(next()),
                    eva_stage: stage(next()),
                    status: if next() % 5 == 0 {
                        LegacyStatus::Paralysis
                    } else {
                        LegacyStatus::None
                    },
                    focus_energy: next() % 7 == 0,
                    substitute_hp: 0,
                    trapping_turns: 0,
                },
                move_data: MoveData {
                    id: mid,
                    effect: MoveEffect::NoAdditionalEffect,
                    power: mpow,
                    move_type: mtype,
                    accuracy: 70 + (next() % 31), // 70..=100
                    pp: 20,
                },
                order_byte: next(),
                first: MoveBytes {
                    confusion: 255,
                    paralysis: next(),
                    crit: next(),
                    accuracy: next(),
                    damage: next(),
                },
                second: MoveBytes {
                    confusion: 255,
                    paralysis: next(),
                    crit: next(),
                    accuracy: next(),
                    damage: next(),
                },
            };
            run_scenario_dmg(&s);
        }
    }
}

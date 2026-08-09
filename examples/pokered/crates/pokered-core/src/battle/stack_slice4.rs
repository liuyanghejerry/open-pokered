//! Effect-stack battle engine — **slice 4: Substitute + partial-trap as
//! cross-battler effect-stack handlers** (design doc
//! `06-battle-engine-effect-stack-design.md` §3 `pair_mut` +
//! `EffectStateKind::{Substitute{hp}, Trapping{turns_left}}`, §6 #28 Substitute /
//! #16 partial-trap, slice 4 of the §7 strangler plan).
//!
//! This is the FIRST slice to exercise the engine's cross-battler
//! [`pair_mut`](jrpg_engine::battle::stack::BattleCtx::pair_mut) with a REAL
//! handler (not the synthetic slice-1 probe). The Substitute interceptor reads the
//! ATTACKER's context and writes the DEFENDER's volatile through a single
//! `pair_mut(source, target)` borrow — and because mover and defender are on
//! opposite sides in single battle, it hits the **cross-side raw-pointer branch**
//! (ctx.rs:150-158), the sole engine `unsafe`, now under genuine load.
//!
//! ## What the two handlers do (re-home the legacy oracle)
//!
//! | handler | event | re-homes | borrow exercise |
//! |---------|-------|----------|-----------------|
//! | `substitute_interceptor` | `ModifyDamage` (`order:2000`, after damage) | `move_execution.rs:288-300` absorb/break | `pair_mut(source,target)` — CROSS-SIDE |
//! | `trapped_gate` | `BeforeMove` (`order:30`) | `status_checks.rs:60-64` trapped forfeit | reads `effects` arena (no draw) |
//!
//! ## The parity contract (extends slices 1-3)
//!
//! Each scenario asserts BOTH **`BattleState`** (both sides' hp + status) AND the
//! **volatile** (each side's substitute hp + up-flag + trap turns) AND
//! **`consumed()`** — all legacy vs `StackDriver` — via
//! [`run_scenario_sub`](crate::battle::stack_parity::run_scenario_sub). Additive,
//! test-only: it does NOT touch the production battle loop; the legacy oracle stays
//! authoritative.
//!
//! ## Determinism (the slice's stress goal)
//!
//! Substitute changes WHERE damage lands (the volatile vs real hp) but draws NO
//! rng of its own — the damage byte is drawn by the base `damage_handler` BEFORE
//! the interceptor fires — so `consumed()` and the per-mover draw order
//! (crit→accuracy→damage) are IDENTICAL to a non-substitute hit. The trapped gate
//! likewise draws nothing (the legacy trapped check reads no byte), so a trapped
//! mover simply contributes zero bytes.

#![cfg(test)]

#[cfg(test)]
mod slice4_tests {
    use crate::battle::stack_parity::{
        run_scenario_sub, stack_run_sub, DamageScenario, MonSpec, MoveBytes,
    };

    use pokered_data::move_data::MoveData;
    use pokered_data::moves::{MoveEffect, MoveId};
    use pokered_data::types::PokemonType;

    /// A `NoAdditionalEffect` Electric move (power 40, acc 100) — the slice-1/2/3
    /// baseline. Substitute/trap are modeled as PRE-SET volatiles (the harness move
    /// stays additive-effect-free), so this baseline move is reused unchanged.
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

    /// Neutral baseline: two Pikachus, the Electric move, always-hit bytes. Player
    /// faster (acts first). HP kept high so no mid-turn KO ever cancels the second
    /// move (keeps the predictor's no-mid-turn-KO invariant true).
    fn base(name: &'static str) -> DamageScenario {
        DamageScenario {
            name,
            player: MonSpec::pikachu(500, 200),
            enemy: MonSpec::pikachu(500, 50),
            move_data: thundershock(),
            order_byte: 0,
            first: MoveBytes::always_hit(),
            second: MoveBytes::always_hit(),
        }
    }

    /// Measure the damage the player's first hit deals to the enemy with NO
    /// substitute (so sub-hp boundary scenarios can be built around the real value
    /// rather than a hard-coded constant). Returns the player→enemy damage.
    fn measure_player_damage(template: &DamageScenario) -> u16 {
        let mut probe = template.clone();
        probe.enemy.substitute_hp = 0;
        // Make the enemy NOT act (it would damage the player but not the enemy),
        // and freeze enemy hp by only reading the post-turn enemy hp delta. The
        // enemy still acts here, but we only read ENEMY hp (the player's target),
        // which only the player touches.
        let (stack, _c, _f) = crate::battle::stack_parity::stack_run_dmg(&probe);
        500 - stack.opponent_battlers[0].hp
    }

    // ───────────────────────── Substitute (#28) ──────────────────────────────

    /// Substitute absorbs a normal hit: the defender's REAL hp is unchanged, the
    /// substitute hp is REDUCED by exactly the damage, and `consumed()` equals the
    /// non-substitute case. Full state + volatile + consumed asserted vs legacy.
    #[test]
    fn substitute_absorbs_normal_hit() {
        let dmg = measure_player_damage(&base("sub probe"));
        assert!(dmg > 0, "baseline must deal damage");

        // Give the enemy a Substitute with MORE hp than one hit → it survives.
        let mut s = base("substitute absorbs a normal hit");
        s.enemy.substitute_hp = dmg + 50;
        // The enemy hits the player normally (no sub on the player), so the player
        // takes real damage; the enemy's real hp is protected by its sub.
        run_scenario_sub(&s);

        let (_stack, _c, _f, effects) = stack_run_sub(&s);
        // The enemy's substitute survives with (sub_hp - dmg) and its real hp is full.
        let sub_left = effects
            .iter()
            .find_map(|e| match &e.kind {
                crate::battle::stack_parity::PocKind::Substitute { hp }
                    if e.host == jrpg_engine::battle::BattlerRef::OPPONENT =>
                {
                    Some(*hp)
                }
                _ => None,
            })
            .expect("enemy sub must survive a non-overkill hit");
        assert_eq!(sub_left, (dmg + 50) - dmg, "sub reduced by exactly the damage");
    }

    /// Substitute breaks on overkill: a hit larger than the sub hp sets the sub to
    /// 0 (volatile removed / flag cleared) and the defender's real hp is STILL
    /// untouched (the overkill does NOT spill into real hp in Gen 1). State +
    /// volatile (the break) + consumed asserted vs legacy.
    #[test]
    fn substitute_breaks_on_overkill() {
        let dmg = measure_player_damage(&base("overkill probe"));
        assert!(dmg >= 2, "need room below the damage for a 1-hp sub");

        let mut s = base("substitute breaks on overkill");
        s.enemy.substitute_hp = 1; // 1 < dmg → overkill → break
        run_scenario_sub(&s);

        let (stack, _c, _f, effects) = stack_run_sub(&s);
        // The sub is GONE (no Substitute volatile for the enemy) and real hp full.
        let still_up = effects.iter().any(|e| {
            e.host == jrpg_engine::battle::BattlerRef::OPPONENT
                && matches!(e.kind, crate::battle::stack_parity::PocKind::Substitute { .. })
        });
        assert!(!still_up, "overkill must BREAK the substitute (volatile removed)");
        assert_eq!(
            stack.opponent_battlers[0].hp, 500,
            "overkill must NOT spill into the defender's real hp"
        );
    }

    /// A hit that EXACTLY depletes the sub (`damage == sub_hp`). Legacy uses
    /// `damage >= sub_hp` → BREAK, so an exact-depletion hit breaks the sub (not
    /// "survives at 0"). The boundary pin below straddles this with `±1`.
    #[test]
    fn substitute_exact_depletion_breaks() {
        let dmg = measure_player_damage(&base("exact probe"));
        assert!(dmg > 0);

        let mut s = base("substitute exactly depleted breaks");
        s.enemy.substitute_hp = dmg; // damage == sub_hp → `>=` → BREAK
        run_scenario_sub(&s);

        let (stack, _c, _f, effects) = stack_run_sub(&s);
        let still_up = effects.iter().any(|e| {
            e.host == jrpg_engine::battle::BattlerRef::OPPONENT
                && matches!(e.kind, crate::battle::stack_parity::PocKind::Substitute { .. })
        });
        assert!(!still_up, "damage == sub_hp must BREAK (legacy uses `>=`)");
        assert_eq!(stack.opponent_battlers[0].hp, 500, "real hp untouched");
    }

    /// **Boundary pin**: damage == sub_hp exactly (break vs survive). Builds three
    /// scenarios around the measured damage: `sub = dmg-1` (BREAK, overkill),
    /// `sub = dmg` (BREAK, exact `>=`), `sub = dmg+1` (SURVIVE at 1). Each runs
    /// through `run_scenario_sub` (state + volatile + consumed vs legacy), so a
    /// `>=`→`>` off-by-one in the absorb/break logic diverges from the legacy
    /// oracle and fails. The fuzz may skip these exact values; this pins them.
    #[test]
    fn substitute_break_boundary_exact_byte() {
        let dmg = measure_player_damage(&base("boundary probe"));
        assert!(dmg >= 2, "need room below the damage");

        // sub = dmg - 1 → overkill → BREAK.
        let mut below = base("boundary: sub = dmg-1 breaks");
        below.enemy.substitute_hp = dmg - 1;
        run_scenario_sub(&below);
        let (_s, _c, _f, eff_below) = stack_run_sub(&below);
        assert!(
            !sub_up(&eff_below),
            "sub_hp = dmg-1 must BREAK (overkill)"
        );

        // sub = dmg → exact depletion → BREAK (legacy `damage >= sub_hp`).
        let mut at = base("boundary: sub = dmg breaks (exact)");
        at.enemy.substitute_hp = dmg;
        run_scenario_sub(&at);
        let (_s2, _c2, _f2, eff_at) = stack_run_sub(&at);
        assert!(
            !sub_up(&eff_at),
            "sub_hp = dmg must BREAK (legacy uses `>=`, not `>`)"
        );

        // sub = dmg + 1 → survives at 1.
        let mut above = base("boundary: sub = dmg+1 survives");
        above.enemy.substitute_hp = dmg + 1;
        run_scenario_sub(&above);
        let (_s3, _c3, _f3, eff_above) = stack_run_sub(&above);
        assert!(sub_up(&eff_above), "sub_hp = dmg+1 must SURVIVE");
    }

    /// Substitute + crit: a critical hit absorbs into the sub exactly like a normal
    /// hit (the crit only changes the damage NUMBER, not WHERE it lands), and draws
    /// the same byte count. The crit's larger damage may break a sub a normal hit
    /// would not — asserted at parity (state + volatile + consumed).
    #[test]
    fn substitute_plus_crit_absorbs_into_sub() {
        // Measure the NON-crit and crit damage to size a sub that survives a normal
        // hit but BREAKS under crit.
        let mut nocrit_tpl = base("sub+crit: no-crit probe");
        nocrit_tpl.first.crit = 255; // no crit
        let normal_dmg = measure_player_damage(&nocrit_tpl);

        let mut crit_tpl = base("sub+crit: crit probe");
        crit_tpl.first.crit = 0; // guaranteed crit (0 < threshold)
        let crit_dmg = measure_player_damage(&crit_tpl);
        assert!(crit_dmg > normal_dmg, "crit must out-damage a normal hit");

        // Sub sized between the two: normal hit survives, crit breaks.
        let mut s = base("substitute + crit breaks a sub a normal hit would not");
        s.first.crit = 0; // crit
        s.enemy.substitute_hp = normal_dmg + 1; // < crit_dmg (since crit_dmg ~2x)
        assert!(s.enemy.substitute_hp <= crit_dmg, "crit must overkill this sub");
        run_scenario_sub(&s);
        let (stack, _c, _f, eff) = stack_run_sub(&s);
        assert!(!sub_up(&eff), "crit must break the under-sized sub");
        assert_eq!(stack.opponent_battlers[0].hp, 500, "crit overkill does not spill to real hp");
    }

    /// Substitute "bypass": a type-immunity hit (Normal vs Ghost) deals 0 damage,
    /// so it neither absorbs into nor breaks the sub — the sub is untouched (Gen 1
    /// has no general sub-bypass move in this slice; the natural 0-damage case is
    /// immunity). Asserts the sub survives unchanged and the damage byte is still
    /// drawn (matching legacy / the slice-3 immunity-miss draw count).
    #[test]
    fn substitute_untouched_by_zero_damage_immunity() {
        let mut s = base("substitute untouched by immunity (0 damage)");
        // Player uses a Normal move vs a Ghost enemy → immune (0 damage).
        s.enemy.species = pokered_data::species::Species::Gastly; // Ghost
        s.move_data = MoveData {
            id: MoveId::Tackle,
            effect: MoveEffect::NoAdditionalEffect,
            power: 40,
            move_type: PokemonType::Normal,
            accuracy: 100,
            pp: 35,
        };
        s.first.crit = 255;
        s.enemy.substitute_hp = 100; // a healthy sub
        run_scenario_sub(&s);
        let (_stack, _c, _f, eff) = stack_run_sub(&s);
        // The sub still has its full 100 hp (immunity dealt 0, so no absorb).
        let sub_left = eff
            .iter()
            .find_map(|e| match &e.kind {
                crate::battle::stack_parity::PocKind::Substitute { hp }
                    if e.host == jrpg_engine::battle::BattlerRef::OPPONENT =>
                {
                    Some(*hp)
                }
                _ => None,
            })
            .expect("immune hit must leave the sub up");
        assert_eq!(sub_left, 100, "a 0-damage (immune) hit must not touch the sub");
    }

    // ───────────────────────── partial-trap (#16) ────────────────────────────

    /// Partial-trap (in-turn): a mon whose OPPONENT is using a trapping move CANNOT
    /// act this turn — it draws no bytes and deals no damage. The trap turn counter
    /// is preserved at parity (the in-turn gate does not decrement it; that is the
    /// cross-turn `apply_trapping` lifecycle, deferred to slice 6). State + trap
    /// counter + consumed asserted vs the legacy trapped gate.
    #[test]
    fn partial_trap_forfeits_trapped_mon_action() {
        // The PLAYER is using a trapping move (turns left = 2) → the ENEMY (its
        // opponent) is trapped and cannot act. The player still acts normally.
        let mut s = base("partial-trap: trapped enemy forfeits its action");
        s.player.trapping_turns = 2;
        // Make the player FASTER so it acts first (and deals damage); the enemy is
        // trapped and contributes zero bytes / zero damage.
        s.player.speed = 200;
        s.enemy.speed = 50;
        run_scenario_sub(&s);

        let (stack, _c, _f, _eff) = stack_run_sub(&s);
        // The player took NO damage (the trapped enemy could not act).
        assert_eq!(
            stack.player_battlers[0].hp, 500,
            "trapped enemy must deal no damage (it could not act)"
        );
        // The enemy took the player's hit (it is not protected).
        assert!(
            stack.opponent_battlers[0].hp < 500,
            "the trapping player still acts and damages the trapped enemy"
        );
    }

    /// Partial-trap consumed() proof: the trapped mover draws ZERO bytes (the
    /// trapped gate reads no rng), so the turn's byte count is exactly the acting
    /// mover's draws. Asserted directly (state + consumed vs legacy via the harness).
    #[test]
    fn partial_trap_trapped_mover_draws_nothing() {
        // Enemy uses a trapping move → the PLAYER is trapped. Enemy acts first
        // (faster) and draws crit+acc+damage = 3; the player draws nothing.
        let mut s = base("partial-trap: trapped player draws nothing");
        s.enemy.trapping_turns = 3;
        s.enemy.speed = 200;
        s.player.speed = 50;
        run_scenario_sub(&s); // run_scenario_sub asserts consumed() vs the predictor

        let (stack, consumed, _f, _eff) = stack_run_sub(&s);
        assert_eq!(
            stack.opponent_battlers[0].hp, 500,
            "trapped player must deal no damage"
        );
        // Enemy crit(1)+acc(1)+damage(1) = 3; player trapped = 0. No order tie
        // (enemy faster), so no order byte.
        assert_eq!(consumed, 3, "only the acting (enemy) mover draws; trapped player draws 0");
    }

    /// Substitute AND partial-trap together: the trapping mover's hit lands on the
    /// trapped mon, and if the trapped mon's defender (the trapper) has a sub it
    /// absorbs — exercising both volatiles in one turn at parity.
    #[test]
    fn substitute_and_partial_trap_combined() {
        let mut s = base("substitute + partial-trap combined");
        // Player is trapping the enemy AND the player itself has a substitute up.
        s.player.trapping_turns = 2;
        s.player.substitute_hp = 100;
        s.player.speed = 200;
        s.enemy.speed = 50;
        // The enemy is trapped (cannot act), so the player's sub is never tested by
        // the enemy; this asserts the combined volatile bookkeeping stays at parity
        // (the player's sub survives untouched, the enemy is forfeited).
        run_scenario_sub(&s);
        let (stack, _c, _f, eff) = stack_run_sub(&s);
        assert_eq!(stack.player_battlers[0].hp, 500, "trapped enemy dealt no damage");
        let player_sub = eff
            .iter()
            .find_map(|e| match &e.kind {
                crate::battle::stack_parity::PocKind::Substitute { hp }
                    if e.host == jrpg_engine::battle::BattlerRef::PLAYER =>
                {
                    Some(*hp)
                }
                _ => None,
            })
            .expect("player sub must remain (enemy could not test it)");
        assert_eq!(player_sub, 100, "player's untested sub stays at full hp");
    }

    // ─────────────────────────── matrix + fuzz ───────────────────────────────

    /// The slice-4 matrix through `run_scenario_sub` (state + volatile + consumed)
    /// as a single guard: sub survive / break / exact, sub+crit, immunity-untouched,
    /// trap-both-directions, and the combined case.
    #[test]
    fn slice4_matrix_state_volatile_consumed() {
        let dmg = measure_player_damage(&base("matrix probe"));
        let mut matrix: Vec<DamageScenario> = Vec::new();

        matrix.push(base("control: no volatile"));

        let mut survive = base("sub survives");
        survive.enemy.substitute_hp = dmg + 100;
        matrix.push(survive);

        let mut brk = base("sub breaks");
        brk.enemy.substitute_hp = 1;
        matrix.push(brk);

        let mut exact = base("sub exact");
        exact.enemy.substitute_hp = dmg;
        matrix.push(exact);

        let mut crit = base("sub + crit");
        crit.first.crit = 0;
        crit.enemy.substitute_hp = dmg + 100;
        matrix.push(crit);

        let mut both_subs = base("both sides sub");
        both_subs.player.substitute_hp = dmg + 100;
        both_subs.enemy.substitute_hp = dmg + 100;
        matrix.push(both_subs);

        let mut trap_enemy = base("trap enemy");
        trap_enemy.player.trapping_turns = 2;
        matrix.push(trap_enemy);

        let mut trap_player = base("trap player");
        trap_player.enemy.trapping_turns = 4;
        matrix.push(trap_player);

        for s in &matrix {
            run_scenario_sub(s);
        }
    }

    /// Determinism fuzz over >= 1000 seeds randomly giving EITHER side a Substitute
    /// (with a sub hp band that straddles the break boundary) and/or a partial-trap,
    /// plus random crit/accuracy/damage bytes and speeds → both paths → identical
    /// `BattleState` + volatile + `consumed()`. The slice's broad-input
    /// cross-battler / absorb-break / draw-order proof. Self-contained LCG (no
    /// `rand`). HP is 60000 so no single hit KOs (keeps the no-mid-turn-KO
    /// invariant true and `consumed()` a real claim).
    #[test]
    fn slice4_determinism_fuzz_1000_seeds() {
        let mut lcg: u64 = 0x5117_ce04_d00d_face;
        let mut next = || {
            lcg = lcg
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (lcg >> 33) as u8
        };

        for _seed in 0..1000u32 {
            // Sub hp band: small enough that a hit can break it, large enough that
            // it can survive — straddling the boundary across seeds.
            let sub_band = |n: u8| -> u16 {
                if n % 3 == 0 {
                    0 // no sub
                } else {
                    1 + (n as u16 % 80) // 1..=80 hp (a 40-power hit lands near here)
                }
            };
            let trap_band = |n: u8| -> u8 {
                if n % 4 == 0 {
                    1 + (n % 5) // trapping, 1..=5 turns
                } else {
                    0 // not trapping
                }
            };

            let mut p = MonSpec::pikachu(60000, 60 + (next() as u16 % 120));
            p.substitute_hp = sub_band(next());
            p.trapping_turns = trap_band(next());

            let mut e = MonSpec::pikachu(60000, 60 + (next() as u16 % 120));
            e.substitute_hp = sub_band(next());
            e.trapping_turns = trap_band(next());

            let s = DamageScenario {
                name: "slice4-fuzz",
                player: p,
                enemy: e,
                move_data: MoveData {
                    id: MoveId::Thundershock,
                    effect: MoveEffect::NoAdditionalEffect,
                    power: 40,
                    move_type: PokemonType::Electric,
                    accuracy: 70 + (next() % 31), // 70..=100
                    pp: 30,
                },
                order_byte: next(),
                first: MoveBytes {
                    confusion: 255,
                    paralysis: 255,
                    crit: next(),
                    accuracy: next(),
                    damage: next(),
                },
                second: MoveBytes {
                    confusion: 255,
                    paralysis: 255,
                    crit: next(),
                    accuracy: next(),
                    damage: next(),
                },
            };
            run_scenario_sub(&s);
        }
    }

    /// Helper: does the ENEMY still have a Substitute volatile in the post-turn
    /// arena? (the engine equivalent of the legacy `HAS_SUBSTITUTE_UP` flag).
    fn sub_up(effects: &[jrpg_engine::battle::stack::EffectState<crate::battle::stack_parity::PocData>]) -> bool {
        effects.iter().any(|e| {
            e.host == jrpg_engine::battle::BattlerRef::OPPONENT
                && matches!(e.kind, crate::battle::stack_parity::PocKind::Substitute { .. })
        })
    }
}

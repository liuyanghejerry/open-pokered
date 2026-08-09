//! Effect-stack battle engine — **slice 2: the Gen-1 `BeforeMove` status gate**
//! (design doc `06-battle-engine-effect-stack-design.md` §6 bugs #8/#10/#12/#13,
//! slice 2 of the §7 strangler plan).
//!
//! Slice 1 proved the byte-stream draw-order shim and the move pipeline
//! (crit→accuracy→damage) at parity, with paralysis as the single `BeforeMove`
//! gate. Slice 2 re-homes the **rest of the Gen-1 pre-move status gate** as
//! ordered effect handlers and proves them at parity with the legacy oracle
//! [`check_status_conditions`](super::status_checks::check_status_conditions)
//! (via [`execute_turn`](super::turn::execute_turn)).
//!
//! ## What this slice adds (all on `Event::BeforeMove`, in the harness)
//!
//! | status     | handler `order` | ASM position | rng |
//! |------------|-----------------|--------------|-----|
//! | Sleep      | 10              | 1            | none (counter decrement) |
//! | Freeze     | 20              | 2            | none (no Gen-1 thaw) |
//! | Confusion  | 70              | 7            | 1 byte iff confused & not snapping out |
//! | Paralysis  | 90              | 9            | 1 byte iff paralyzed |
//!
//! Trapped (3), Flinch (4), Recharge (5), Disabled-counter (6) and
//! Disabled-move (8) are **STUBBED** — not wired in this slice. The relative
//! `order` of the four implemented handlers reproduces their ASM positions
//! exactly (`Sleep < Freeze < Confusion < Paralysis`), and the gaps in the
//! numbering are reserved so a later slice slots the stubs in without
//! renumbering.
//!
//! ## The parity contract (identical to slice 1)
//!
//! Every scenario asserts BOTH:
//! * **`BattleState` parity** — hp + status on both sides, legacy `execute_turn`
//!   vs `StackDriver` ([`assert_state_parity`]), and
//! * **`consumed()` parity** — the stack drew exactly the bytes the predictor
//!   ([`build_stack_stream`]) says it should, in the ASM / `MoveRandoms` order
//!   (confusion BEFORE paralysis), via [`run_scenario`].
//!
//! It is additive and test-only (`#[cfg(test)]`): it does NOT touch the
//! production battle loop. The legacy oracle stays authoritative.

#![cfg(test)]

#[cfg(test)]
mod slice2_tests {
    use crate::battle::stack_parity::{
        assert_state_parity, build_stack_stream, first_mover, legacy_run, order_is_tie,
        run_scenario, stack_run, MoveBytes, Scenario,
    };

    use crate::battle::state::StatusCondition as LegacyStatus;

    /// A neutral, no-status scenario template both movers ALWAYS-HIT; callers
    /// flip individual fields. Player is faster (acts first) unless overridden.
    fn base(name: &'static str) -> Scenario {
        Scenario {
            name,
            player_speed: 200,
            enemy_speed: 50,
            player_hp: 200,
            enemy_hp: 200,
            player_status: LegacyStatus::None,
            enemy_status: LegacyStatus::None,
            player_focus_energy: false,
            player_confused_turns: 0,
            enemy_confused_turns: 0,
            order_byte: 0,
            first: MoveBytes::always_hit(),
            second: MoveBytes::always_hit(),
        }
    }

    // ───────────────────────── Sleep (ASM #1, no draw, bug #8) ───────────────

    /// A sleeping mon loses its turn each turn the counter is > 0, and the
    /// counter decrements with NO rng draw — proven turn-by-turn against the
    /// legacy oracle for `Sleep(3) → Sleep(2) → Sleep(1) → (woke, lost turn)`.
    /// Each turn asserts BOTH state parity AND `consumed()` (sleep adds zero
    /// bytes, so the only draws are the enemy's crit/acc/damage).
    #[test]
    fn sleep_decrements_no_draw_then_wakes_and_loses_turn() {
        // Sleep(3): asleep, decrements to Sleep(2). Player (asleep) deals no
        // damage; enemy acts and damages the sleeping player.
        for (start, expect_after) in [
            (3u8, Some(LegacyStatus::Sleep(2))),
            (2u8, Some(LegacyStatus::Sleep(1))),
            // Sleep(1): wakes this turn but STILL loses the turn (bug #8).
            (1u8, None),
        ] {
            let mut s = base("sleep multi-turn");
            s.player_status = LegacyStatus::Sleep(start);

            let legacy = legacy_run(&s);
            let (stack, consumed, first) = stack_run(&s);

            // BattleState parity (hp + status both sides).
            assert_state_parity(&s, &legacy, &stack);

            // consumed() parity: sleep draws nothing; only the enemy's 3 bytes
            // (crit, acc, damage). No order byte (player faster → no tie).
            let (_b, expected) = build_stack_stream(&s, first, order_is_tie(&s));
            assert_eq!(consumed, expected, "[{}] sleep draw count", s.name);
            assert_eq!(expected, 3, "[{}] only enemy's 3 bytes drawn", s.name);

            // The sleeping player dealt NO damage (lost its turn); enemy is full.
            assert_eq!(stack.opponent_battlers[0].hp, 200, "sleeper deals no damage");
            assert_eq!(
                legacy.enemy.active_mon().hp,
                200,
                "legacy: sleeper deals no damage"
            );

            // Status decremented identically on both paths.
            assert_eq!(stack.player_battlers[0].status, expect_after, "stack status");
            assert_eq!(
                legacy.player.active_mon().status,
                expect_after.unwrap_or(LegacyStatus::None),
                "legacy status"
            );
        }
    }

    /// After fully waking (counter reached 0 last turn), the mon acts normally —
    /// the gate adds no draw and the move proceeds. Parity + consumed.
    #[test]
    fn awake_mon_acts_normally() {
        let s = base("awake acts");
        // No sleep → full pipeline both sides; faster player acts, deals damage.
        run_scenario(&s);
        let (stack, _c, _f) = stack_run(&s);
        assert!(
            stack.opponent_battlers[0].hp < 200,
            "awake player must deal damage"
        );
    }

    // ───────────────────────── Freeze (ASM #2, no draw, bug #10) ─────────────

    /// A frozen mon ALWAYS cannot move and Gen 1 has no per-turn thaw — so the
    /// gate draws NO byte and the status persists. The frozen player never acts;
    /// only the enemy's 3 bytes are drawn. Parity + consumed, both sides.
    #[test]
    fn freeze_always_blocks_no_draw() {
        let mut s = base("frozen player blocked");
        s.player_status = LegacyStatus::Freeze;

        let legacy = legacy_run(&s);
        let (stack, consumed, first) = stack_run(&s);

        assert_state_parity(&s, &legacy, &stack);

        let (_b, expected) = build_stack_stream(&s, first, order_is_tie(&s));
        assert_eq!(consumed, expected, "freeze draw count");
        assert_eq!(expected, 3, "only enemy's 3 bytes (freeze draws none)");

        // Frozen player dealt no damage; status still Freeze on both paths.
        assert_eq!(stack.opponent_battlers[0].hp, 200, "frozen deals no damage");
        assert_eq!(
            stack.player_battlers[0].status,
            Some(LegacyStatus::Freeze),
            "stack: still frozen"
        );
        assert_eq!(
            legacy.player.active_mon().status,
            LegacyStatus::Freeze,
            "legacy: still frozen"
        );
    }

    // ───────────────────────── Paralysis (ASM #9, bug #11/#12) ───────────────

    /// Full paralysis (`paralysis_roll < 63`): the gate draws ONE byte and aborts
    /// the move. Acts (`>= 63`): draws ONE byte and proceeds. Both outcomes AND
    /// `consumed()` asserted against legacy.
    #[test]
    fn paralysis_full_para_vs_acts_draw_and_outcome() {
        // Fully paralyzed: byte 0 < 63 → no damage from player. 1 para byte
        // drawn, then enemy's 3. consumed = 4.
        {
            let mut s = base("para full");
            s.player_status = LegacyStatus::Paralysis;
            // player is paralyzed → speed ÷4. Keep player faster anyway (200/4=50
            // ties enemy 50 → would draw an order byte). Bump player speed so it
            // stays clearly first even after ÷4 and NO tie byte is drawn.
            s.player_speed = 1000; // /4 = 250 > 50, no tie
            s.first.paralysis = 0; // < 63 → fully paralyzed
            let legacy = legacy_run(&s);
            let (stack, consumed, first) = stack_run(&s);
            assert_state_parity(&s, &legacy, &stack);
            let (_b, expected) = build_stack_stream(&s, first, order_is_tie(&s));
            assert_eq!(consumed, expected, "para-full draw count");
            assert_eq!(expected, 4, "1 para byte (player) + 3 (enemy)");
            assert_eq!(stack.opponent_battlers[0].hp, 200, "para player no damage");
        }
        // Paralyzed but acts: byte 200 >= 63 → player acts and damages enemy.
        // 1 para byte + crit + acc + damage (player) = 4, then enemy 3 = 7.
        {
            let mut s = base("para acts");
            s.player_status = LegacyStatus::Paralysis;
            s.player_speed = 1000;
            s.first.paralysis = 200; // >= 63 → acts
            let legacy = legacy_run(&s);
            let (stack, consumed, first) = stack_run(&s);
            assert_state_parity(&s, &legacy, &stack);
            let (_b, expected) = build_stack_stream(&s, first, order_is_tie(&s));
            assert_eq!(consumed, expected, "para-acts draw count");
            assert_eq!(expected, 7, "para(1)+crit+acc+dmg (player) + 3 (enemy)");
            assert!(
                stack.opponent_battlers[0].hp < 200,
                "acting para player must damage enemy"
            );
        }
    }

    /// Boundary pin for the paralysis threshold (`< 63`). The fixed-seed fuzz
    /// never draws the exact byte 63, so a `< 63 → < 64` off-by-one would slip
    /// through. Drive byte 62 (must block) and byte 63 (must act) through the
    /// harness against the legacy oracle: both outcome and `consumed()` match,
    /// and the two values straddle the threshold so any boundary mutation
    /// diverges from legacy.
    #[test]
    fn paralysis_threshold_boundary_62_blocks_63_acts() {
        // byte 62 < 63 → fully paralyzed (blocked, no enemy damage).
        {
            let mut s = base("para boundary 62 blocks");
            s.player_status = LegacyStatus::Paralysis;
            s.player_speed = 1000; // stays first after ÷4; no tie byte
            s.first.paralysis = 62;
            let legacy = legacy_run(&s);
            let (stack, consumed, first) = stack_run(&s);
            assert_state_parity(&s, &legacy, &stack);
            let (_b, expected) = build_stack_stream(&s, first, order_is_tie(&s));
            assert_eq!(consumed, expected, "para-62 draw count");
            assert_eq!(
                stack.opponent_battlers[0].hp, 200,
                "byte 62 (< 63) must fully paralyze → no enemy damage"
            );
        }
        // byte 63 is NOT < 63 → acts (damages enemy).
        {
            let mut s = base("para boundary 63 acts");
            s.player_status = LegacyStatus::Paralysis;
            s.player_speed = 1000;
            s.first.paralysis = 63;
            let legacy = legacy_run(&s);
            let (stack, consumed, first) = stack_run(&s);
            assert_state_parity(&s, &legacy, &stack);
            let (_b, expected) = build_stack_stream(&s, first, order_is_tie(&s));
            assert_eq!(consumed, expected, "para-63 draw count");
            assert!(
                stack.opponent_battlers[0].hp < 200,
                "byte 63 (>= 63) must act → enemy takes damage"
            );
        }
    }

    // ───────────────────────── Confusion (ASM #7, bug #13) ───────────────────

    /// Confusion self-hit (`confusion_roll < 128`): the gate draws ONE confusion
    /// byte, applies the typeless 40-power self-hit, and aborts the move. The
    /// self-hit damage and the abort (no enemy damage from the confused mon) must
    /// match legacy, AND `consumed()` must match (1 confusion byte, no crit/acc/
    /// damage for the confused mover).
    #[test]
    fn confusion_self_hit_draws_one_byte_and_aborts() {
        let mut s = base("confused self-hit");
        s.player_confused_turns = 3; // confused, will decrement to 2 (> 0)
        s.first.confusion = 0; // < 128 → self-hit

        let legacy = legacy_run(&s);
        let (stack, consumed, first) = stack_run(&s);

        assert_state_parity(&s, &legacy, &stack);

        let (_b, expected) = build_stack_stream(&s, first, order_is_tie(&s));
        assert_eq!(consumed, expected, "confusion self-hit draw count");
        // 1 confusion byte (player, self-hit aborts) + enemy's 3 = 4.
        assert_eq!(expected, 4, "confusion(1) + enemy(3)");

        // Confused player took self-hit damage (parity already asserted on hp).
        assert!(
            stack.player_battlers[0].hp < 200,
            "confused player must take self-hit damage"
        );
        // The confused player's move was aborted → enemy untouched by it.
        assert_eq!(
            stack.opponent_battlers[0].hp,
            legacy.enemy.active_mon().hp,
            "enemy hp parity"
        );
    }

    /// Confused but acts (`confusion_roll >= 128`): the gate draws ONE confusion
    /// byte, the mon does NOT hit itself, and the move proceeds normally. Both
    /// outcome AND `consumed()` asserted.
    #[test]
    fn confusion_acts_through_draws_one_byte() {
        let mut s = base("confused but acts");
        s.player_confused_turns = 3;
        s.first.confusion = 200; // >= 128 → acts normally

        let legacy = legacy_run(&s);
        let (stack, consumed, first) = stack_run(&s);

        assert_state_parity(&s, &legacy, &stack);

        let (_b, expected) = build_stack_stream(&s, first, order_is_tie(&s));
        assert_eq!(consumed, expected, "confusion-acts draw count");
        // confusion(1) + crit + acc + dmg (player) = 4, then enemy 3 = 7.
        assert_eq!(expected, 7, "confusion(1)+crit+acc+dmg + enemy(3)");

        // Player acted and damaged the enemy; took NO self-hit. "No self-hit" is
        // proven by comparing to an unconfused control: the acting player's HP
        // equals the control's (only the enemy's retaliation, no extra damage).
        assert!(
            stack.opponent_battlers[0].hp < 200,
            "acting confused player must damage enemy"
        );
        let (control, _c, _f) = stack_run(&base("acts control"));
        assert_eq!(
            stack.player_battlers[0].hp,
            control.player_battlers[0].hp,
            "acting confused player took only retaliation, no self-hit"
        );
    }

    /// Confusion snap-out: `confused_turns_left` decrements to 0 → confusion
    /// ends, NO byte drawn, mon acts the same turn. The legacy gate clears
    /// `status1::CONFUSED` and falls through WITHOUT reading the confusion byte;
    /// the stack must match (volatile removed, zero confusion draw).
    #[test]
    fn confusion_snaps_out_no_draw_and_acts() {
        let mut s = base("confusion snaps out");
        s.player_confused_turns = 1; // decrements to 0 → snap out, no byte
        s.first.confusion = 0; // would self-hit IF drawn — but it must NOT be drawn

        let legacy = legacy_run(&s);
        let (stack, consumed, first) = stack_run(&s);

        assert_state_parity(&s, &legacy, &stack);

        let (_b, expected) = build_stack_stream(&s, first, order_is_tie(&s));
        assert_eq!(consumed, expected, "snap-out draw count");
        // NO confusion byte: player crit+acc+dmg = 3, enemy 3 = 6.
        assert_eq!(expected, 6, "no confusion draw on snap-out: player(3)+enemy(3)");

        // The player ACTED (snapped out) and damaged the enemy; took NO self-hit
        // (the self-hit byte was never read). Compared to an unconfused control,
        // the player's HP is identical (only the enemy retaliation), proving the
        // confusion self-hit byte (0) was NOT consumed.
        assert!(
            stack.opponent_battlers[0].hp < 200,
            "snapped-out player must act and damage enemy"
        );
        let (control, _c, _f) = stack_run(&base("snap-out control"));
        assert_eq!(
            stack.player_battlers[0].hp,
            control.player_battlers[0].hp,
            "no self-hit on snap-out (byte not drawn) — only retaliation"
        );
    }

    // ────────────── Ordering: confusion byte BEFORE paralysis byte ───────────

    /// A mon that is BOTH paralyzed AND confused must read the confusion byte
    /// (ASM #7, `order:70`) BEFORE the paralysis byte (ASM #9, `order:90`). This
    /// pins the gate ordering: if the two handlers fired in the wrong order the
    /// confusion byte would be read at the paralysis offset (and vice-versa),
    /// flipping the outcome and breaking parity with the legacy oracle (which
    /// always reads `confusion_roll` before `paralysis_roll`).
    ///
    /// Trap: confusion byte 200 (>=128 → acts, no self-hit), paralysis byte 0
    /// (<63 → fully paralyzed). Correct order → confusion passes, paralysis
    /// aborts → player deals no damage. Swapped order → the para handler would
    /// read 200 (acts) and the confusion handler would read 0 (self-hit) → player
    /// takes self-damage and is NOT para-blocked → different state.
    #[test]
    fn confusion_drawn_before_paralysis() {
        let mut s = base("para + confused: confusion byte first");
        s.player_status = LegacyStatus::Paralysis;
        s.player_speed = 1000; // stays first even after ÷4, no tie byte
        s.player_confused_turns = 3; // confused, decrements to 2 (> 0) → draws
        s.first.confusion = 200; // >= 128 → confusion passes (acts)
        s.first.paralysis = 0; // < 63 → full para aborts

        let legacy = legacy_run(&s);
        let (stack, consumed, first) = stack_run(&s);

        // Parity is the strong claim: stack matches legacy exactly. With the
        // correct order the player is fully paralyzed (after passing confusion)
        // and deals no damage and takes no self-hit; legacy agrees.
        assert_state_parity(&s, &legacy, &stack);

        let (_b, expected) = build_stack_stream(&s, first, order_is_tie(&s));
        assert_eq!(consumed, expected, "confusion-then-para draw count");
        // confusion(1, acts) + paralysis(1, full-para aborts) for player = 2,
        // then enemy's 3 = 5.
        assert_eq!(expected, 5, "confusion(1)+para(1) (player) + enemy(3)");

        // Correct-order semantic checks (these trap a swapped fire order):
        //
        // (a) Paralysis aborted the move (byte 0 read at the PARA offset) → the
        //     player dealt NO damage, enemy untouched. If the handlers were
        //     swapped, the para handler would read 200 (acts) and the player
        //     would damage the enemy here.
        assert_eq!(
            stack.opponent_battlers[0].hp, 200,
            "paralysis byte (0) read at para offset → fully paralyzed, no damage"
        );
        // (b) Confusion did NOT self-hit (byte 200 read at the CONFUSION offset).
        //     Proven by comparing to a para-only (unconfused) control: the
        //     player's HP is the SAME (only the enemy's retaliation, no extra
        //     self-hit). A swapped order would read 0 at confusion → self-hit →
        //     strictly less HP than the control.
        let mut para_only = base("para-only control");
        para_only.player_status = LegacyStatus::Paralysis;
        para_only.player_speed = 1000;
        para_only.first.paralysis = 0;
        let (control, _c, _f) = stack_run(&para_only);
        assert_eq!(
            stack.player_battlers[0].hp,
            control.player_battlers[0].hp,
            "confusion byte (200) read at confusion offset → no self-hit (HP == para-only control)"
        );
        assert!(
            stack.player_battlers[0].hp < 200,
            "the (slower) enemy still retaliates against the blocked player"
        );
    }

    // ────────────── Combined turn: one side statused, other moves ────────────

    /// A combined turn where the SECOND mover is the statused one: the enemy acts
    /// first (faster) and damages the player, then the (slower) confused player's
    /// gate fires — proving the per-mover draw interleave is correct (enemy's
    /// crit/acc/damage drawn FIRST, then the player's confusion byte). Asserts
    /// state parity AND the interleaved `consumed()`.
    #[test]
    fn combined_turn_statused_second_mover_interleave() {
        let mut s = base("enemy acts, player confused-self-hit second");
        // Enemy faster → acts first.
        s.player_speed = 50;
        s.enemy_speed = 200;
        s.player_confused_turns = 3;
        // `first` MoveBytes is the FIRST mover (enemy here) — always hit.
        s.first = MoveBytes::always_hit();
        // `second` MoveBytes is the SECOND mover (the confused player).
        s.second.confusion = 0; // < 128 → self-hit, aborts

        let legacy = legacy_run(&s);
        let (stack, consumed, first) = stack_run(&s);

        assert_state_parity(&s, &legacy, &stack);

        let (_b, expected) = build_stack_stream(&s, first, order_is_tie(&s));
        assert_eq!(consumed, expected, "interleave draw count");
        // enemy(3) first, then player confusion(1, self-hit aborts) = 4.
        assert_eq!(expected, 4, "enemy(3) then player confusion(1)");

        // Enemy damaged the player; the confused player also took self-hit
        // damage and dealt none to the enemy.
        assert!(stack.player_battlers[0].hp < 200, "player took enemy damage");
        assert_eq!(stack.opponent_battlers[0].hp, 200, "confused player dealt none");
    }

    /// The full slice-2 matrix through `run_scenario` (state parity + consumed)
    /// as a single guard, mirroring slice 1's `parity_matrix_state_and_consumed`.
    /// Covers every implemented status and the no-status control.
    #[test]
    fn slice2_matrix_state_and_consumed() {
        let mut matrix: Vec<Scenario> = Vec::new();

        matrix.push(base("control: no status"));

        let mut sleep = base("sleep blocks");
        sleep.player_status = LegacyStatus::Sleep(3);
        matrix.push(sleep);

        let mut freeze = base("freeze blocks");
        freeze.player_status = LegacyStatus::Freeze;
        matrix.push(freeze);

        let mut para_full = base("para full");
        para_full.player_status = LegacyStatus::Paralysis;
        para_full.player_speed = 1000;
        para_full.first.paralysis = 0;
        matrix.push(para_full);

        let mut para_acts = base("para acts");
        para_acts.player_status = LegacyStatus::Paralysis;
        para_acts.player_speed = 1000;
        para_acts.first.paralysis = 200;
        matrix.push(para_acts);

        let mut conf_hit = base("confusion self-hit");
        conf_hit.player_confused_turns = 3;
        conf_hit.first.confusion = 0;
        matrix.push(conf_hit);

        let mut conf_acts = base("confusion acts");
        conf_acts.player_confused_turns = 3;
        conf_acts.first.confusion = 200;
        matrix.push(conf_acts);

        let mut conf_snap = base("confusion snap-out");
        conf_snap.player_confused_turns = 1;
        conf_snap.first.confusion = 0;
        matrix.push(conf_snap);

        let mut both = base("para + confused");
        both.player_status = LegacyStatus::Paralysis;
        both.player_speed = 1000;
        both.player_confused_turns = 3;
        both.first.confusion = 200;
        both.first.paralysis = 0;
        matrix.push(both);

        for s in &matrix {
            run_scenario(s);
            // `first_mover` is part of the harness contract; make sure the probe
            // agrees (it is also asserted inside `stack_run`).
            let _ = first_mover(s);
        }
    }

    /// Determinism fuzz over >= 1000 seeds mixing ALL slice-2 statuses (none,
    /// sleep, freeze, paralysis, confusion, and the para+confused combo) with
    /// random gate bytes → both paths → identical `BattleState` + equal
    /// `consumed()`. This is the slice's broad-input draw-order proof (the
    /// strangler per-slice protocol, design §7). Self-contained LCG (no `rand`).
    #[test]
    fn slice2_determinism_fuzz_1000_seeds() {
        let mut lcg: u64 = 0x0bad_c0de_dead_beef;
        let mut next = || {
            lcg = lcg
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (lcg >> 33) as u8
        };

        for seed in 0..1000u32 {
            // Pick a status family for each side from the seed. Sleep counters
            // are kept >= 1 so the mon is actually asleep.
            let pick_status = |k: u8, sleep_ctr: u8| -> LegacyStatus {
                match k % 5 {
                    0 => LegacyStatus::None,
                    1 => LegacyStatus::Sleep((sleep_ctr % 6) + 1),
                    2 => LegacyStatus::Freeze,
                    3 => LegacyStatus::Paralysis,
                    _ => LegacyStatus::None, // confusion handled separately below
                }
            };
            let pk = next();
            let ek = next();
            let pstatus = pick_status(pk, next());
            let estatus = pick_status(ek, next());
            // Confusion (volatile) layered independently: ~1/3 of sides confused,
            // turns in 1..=4 so both the self-hit/acts and the snap-out path fire.
            let pconf = if next() % 3 == 0 { 1 + next() % 4 } else { 0 };
            let econf = if next() % 3 == 0 { 1 + next() % 4 } else { 0 };

            let s = Scenario {
                name: "slice2-fuzz",
                // Keep both above 30 HP so the second mover always acts (the
                // predictor assumes no mid-turn KO, matching slice 1's fuzz).
                player_speed: 60 + (next() as u16 % 120),
                enemy_speed: 60 + (next() as u16 % 120),
                player_hp: 200,
                enemy_hp: 200,
                player_status: pstatus,
                enemy_status: estatus,
                player_focus_energy: seed % 7 == 0,
                player_confused_turns: pconf,
                enemy_confused_turns: econf,
                order_byte: next(),
                first: MoveBytes {
                    confusion: next(),
                    paralysis: next(),
                    crit: next(),
                    accuracy: next() % 200, // bias toward hits
                    damage: next().max(1),
                },
                second: MoveBytes {
                    confusion: next(),
                    paralysis: next(),
                    crit: next(),
                    accuracy: next() % 200,
                    damage: next().max(1),
                },
            };
            run_scenario(&s);
        }
    }
}

//! Effect-stack POC — **slice 1 parity scenarios** (design doc
//! `06-battle-engine-effect-stack-design.md` §8).
//!
//! The generic plumbing (the [`PocData`] provider + handlers, the
//! byte-vector ⇄ `TurnRandoms` ⇄ `ScriptedRng` shim, the `BattleState`
//! differential oracle, the consumed-count predictor, and the standing
//! draw-order guard) now lives in the **reusable harness**
//! [`crate::battle::stack_parity`]. Later slices (2–7) add scenarios there
//! without rebuilding the plumbing.
//!
//! This file is now just the **slice-1 scenario set** plus the original five
//! parity tests, **re-pointed** to call the harness (no duplicated machinery).
//! It is additive and test-only (`#[cfg(test)]`): it does NOT touch the
//! production battle loop (`turn.rs` / `mod.rs`). The legacy
//! [`execute_turn`](super::turn::execute_turn) stays the oracle.
//!
//! It proves the three structural risks the POC must retire:
//!
//! * **BORROW** — handlers are zero-capture `fn` pointers driving the engine's
//!   `StackDriver` through `&mut BattleCtx`; the engine-side tests prove
//!   `pair_mut` (both branches) + a Counter-shaped handler compile with no
//!   `RefCell`/`Rc`.
//! * **DETERMINISM** — the same byte vector fed to (a) pokered's legacy
//!   `execute_turn` (pre-rolled `TurnRandoms` struct) and (b) the `StackDriver`
//!   (streamed `ScriptedRng`) yields **identical** `BattleState` (hp/status both
//!   sides) AND equal `rng.consumed()`, with **crit drawn before accuracy**
//!   (pinned by the standing guard in the harness).
//! * **FIDELITY** — the paralysis `BeforeMove` gate, the Poison residual
//!   (`order:10`, min-1, per-mover), the first-mover-faint short-circuit, and
//!   the Focus Energy `/4` crit (deliberate Gen-1 bug #1) all match pokered.

#![cfg(test)]

#[cfg(test)]
mod parity_tests {
    use crate::battle::stack_parity::{
        assert_crit_drawn_before_accuracy, build_stack_stream, engine_battler, first_mover,
        legacy_run, order_is_tie, run_scenario, stack_run, MoveBytes, PocData, Scenario,
    };

    use jrpg_engine::battle::rng::ScriptedRng;
    use jrpg_engine::battle::stack::{EffectState, StackDriver};
    use jrpg_engine::battle::{BattleAction, BattleState as EngineState};

    use pokered_data::moves::MoveId;
    use pokered_data::pokemon_data::get_base_stats;
    use pokered_data::species::Species;

    use crate::battle::state::StatusCondition as LegacyStatus;

    /// The slice-1 scenario matrix (normal hit, full-para, para-acts, poison
    /// residual, focus-energy crit denial, clean miss). Each is a value handed
    /// to the harness' `run_scenario`.
    fn matrix() -> Vec<Scenario> {
        vec![
            Scenario {
                name: "normal hit both sides",
                player_speed: 100,
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
            },
            Scenario {
                name: "player fully paralyzed (move skipped)",
                player_speed: 100,
                enemy_speed: 50,
                player_hp: 200,
                enemy_hp: 200,
                player_status: LegacyStatus::Paralysis,
                enemy_status: LegacyStatus::None,
                player_focus_energy: false,
                player_confused_turns: 0,
                enemy_confused_turns: 0,
                order_byte: 0,
                first: MoveBytes {
                    confusion: 255,
                    paralysis: 0, // < 63 → fully paralyzed
                    crit: 255,
                    accuracy: 0,
                    damage: 255,
                },
                second: MoveBytes::always_hit(),
            },
            Scenario {
                name: "player paralyzed but acts",
                player_speed: 200, // stays first even after ÷4
                enemy_speed: 10,
                player_hp: 200,
                enemy_hp: 200,
                player_status: LegacyStatus::Paralysis,
                enemy_status: LegacyStatus::None,
                player_focus_energy: false,
                player_confused_turns: 0,
                enemy_confused_turns: 0,
                order_byte: 0,
                first: MoveBytes {
                    confusion: 255,
                    paralysis: 200, // >= 63 → acts
                    crit: 255,
                    accuracy: 0,
                    damage: 255,
                },
                second: MoveBytes::always_hit(),
            },
            Scenario {
                name: "enemy poisoned residual after its move",
                player_speed: 100,
                enemy_speed: 50,
                player_hp: 200,
                enemy_hp: 200,
                player_status: LegacyStatus::None,
                enemy_status: LegacyStatus::Poison,
                player_focus_energy: false,
                player_confused_turns: 0,
                enemy_confused_turns: 0,
                order_byte: 0,
                first: MoveBytes::always_hit(),
                second: MoveBytes::always_hit(),
            },
            Scenario {
                name: "focus energy /4 crit (Gen-1 bug) — crit denied",
                player_speed: 100,
                enemy_speed: 50,
                player_hp: 200,
                enemy_hp: 200,
                player_status: LegacyStatus::None,
                enemy_status: LegacyStatus::None,
                player_focus_energy: true,
                player_confused_turns: 0,
                enemy_confused_turns: 0,
                order_byte: 0,
                // crit byte at a value that WOULD crit without focus energy but
                // is denied by the /4 bug (proves the bug fires identically).
                first: MoveBytes {
                    confusion: 255,
                    paralysis: 255,
                    crit: 12, // base_speed/2 / 4 threshold; see assertion
                    accuracy: 0,
                    damage: 255,
                },
                second: MoveBytes::always_hit(),
            },
            Scenario {
                name: "a clean miss (accuracy fails, no damage byte)",
                player_speed: 100,
                enemy_speed: 50,
                player_hp: 200,
                enemy_hp: 200,
                player_status: LegacyStatus::None,
                enemy_status: LegacyStatus::None,
                player_focus_energy: false,
                player_confused_turns: 0,
                enemy_confused_turns: 0,
                order_byte: 0,
                first: MoveBytes {
                    confusion: 255,
                    paralysis: 255,
                    crit: 255,
                    // 100% acc → scaled 255; byte 255 is NOT < 255 → the Gen-1
                    // 1/256 miss. No damage byte drawn on the first mover.
                    accuracy: 255,
                    damage: 255,
                },
                second: MoveBytes::always_hit(),
            },
        ]
    }

    #[test]
    fn parity_matrix_state_and_consumed() {
        for s in matrix() {
            run_scenario(&s);
        }
    }

    /// The STANDING DRAW-ORDER GUARD: crit MUST be drawn BEFORE accuracy
    /// (design §4, bug-critical). Delegates to the harness, which traps the fire
    /// order via distinct stream offsets — it FAILS loudly if
    /// `ModifyCritRatio`/`Accuracy` are swapped in `driver.rs`. See
    /// [`crate::battle::stack_parity::assert_crit_drawn_before_accuracy`].
    #[test]
    fn crit_is_drawn_before_accuracy() {
        assert_crit_drawn_before_accuracy();
    }

    /// FIDELITY: the deliberate Gen-1 Focus Energy `/4` crit bug (#1). With a
    /// crit byte BETWEEN the focus threshold and the normal threshold, the move
    /// crits WITHOUT focus energy but is DENIED a crit WITH it (the bug reduces,
    /// not raises, crit rate). Both stack and legacy must agree on the denial.
    #[test]
    fn focus_energy_quarters_crit_gen1_bug() {
        let base_speed = get_base_stats(Species::Pikachu).unwrap().speed;
        let normal = crate::battle::damage::crit_chance(base_speed, false, false);
        let focus = crate::battle::damage::crit_chance(base_speed, false, true);
        assert!(
            focus < normal,
            "Gen-1 bug precondition: focus threshold {focus} must be < normal {normal}"
        );
        // A byte that crits normally (< normal) but NOT under focus (>= focus).
        let crit_byte = focus; // focus <= crit_byte < normal  (since focus < normal)
        assert!(crit_byte < normal, "byte must crit without focus energy");

        let base = Scenario {
            name: "focus-energy-bug",
            player_speed: 100,
            enemy_speed: 50,
            player_hp: 200,
            enemy_hp: 200,
            player_status: LegacyStatus::None,
            enemy_status: LegacyStatus::None,
            player_focus_energy: false,
            player_confused_turns: 0,
            enemy_confused_turns: 0,
            order_byte: 0,
            first: MoveBytes {
                confusion: 255,
                paralysis: 255,
                crit: crit_byte,
                accuracy: 0,
                damage: 255,
            },
            second: MoveBytes::always_hit(),
        };

        // WITHOUT focus energy → crit (more damage).
        let no_focus = base.clone();
        let legacy_nf = legacy_run(&no_focus);
        let (stack_nf, _, _) = stack_run(&no_focus);
        assert_eq!(
            legacy_nf.enemy.active_mon().hp,
            stack_nf.opponent_battlers[0].hp,
            "no-focus crit damage parity"
        );

        // WITH focus energy → NO crit (less damage = the bug).
        let mut with_focus = base;
        with_focus.player_focus_energy = true;
        let legacy_f = legacy_run(&with_focus);
        let (stack_f, _, _) = stack_run(&with_focus);
        assert_eq!(
            legacy_f.enemy.active_mon().hp,
            stack_f.opponent_battlers[0].hp,
            "focus crit-denied damage parity"
        );

        // The bug bites: focus energy DENIED the crit, so the enemy took LESS
        // damage (higher remaining hp) than the no-focus crit case. Both paths.
        assert!(
            legacy_f.enemy.active_mon().hp > legacy_nf.enemy.active_mon().hp,
            "Gen-1 bug: Focus Energy must DENY a crit that lands without it"
        );
        assert!(
            stack_f.opponent_battlers[0].hp > stack_nf.opponent_battlers[0].hp,
            "stack must reproduce the Focus Energy crit-denial bug"
        );
    }

    /// FIDELITY: the first-mover-faint / first-move-KO short-circuit cancels the
    /// second move (design §2 step 2d, `turn.rs:48-60`). The player KO's the
    /// enemy; the enemy must NOT then act. Both paths must agree the player took
    /// zero retaliation damage (still full HP).
    #[test]
    fn first_move_ko_cancels_second_move() {
        let s = Scenario {
            name: "player KOs enemy → enemy's move cancelled",
            player_speed: 100,
            enemy_speed: 50,
            player_hp: 200,
            enemy_hp: 1, // one hit KO's the enemy
            player_status: LegacyStatus::None,
            enemy_status: LegacyStatus::None,
            player_focus_energy: false,
            player_confused_turns: 0,
            enemy_confused_turns: 0,
            order_byte: 0,
            first: MoveBytes::always_hit(),
            second: MoveBytes::always_hit(),
        };
        let legacy = legacy_run(&s);
        // The stack stream: no order byte (no tie), first mover (player)
        // crit/acc/damage = 3 bytes; enemy is KO'd → second move cancelled → no
        // further draws.
        let provider = PocData;
        let mut state = EngineState::new(
            vec![engine_battler(Species::Pikachu, s.player_hp, s.player_speed, s.player_status)],
            vec![engine_battler(Species::Pikachu, s.enemy_hp, s.enemy_speed, s.enemy_status)],
        );
        let mut effects: Vec<EffectState<PocData>> = Vec::new();
        let actions = [
            BattleAction::<PocData>::Fight { move_: MoveId::Thundershock },
            BattleAction::<PocData>::Fight { move_: MoveId::Thundershock },
        ];
        let mut rng = ScriptedRng::new(vec![255, 0, 255]); // crit, acc(hit), dmg
        let result =
            StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);

        // Both: enemy fainted, player untouched, second move cancelled.
        assert_eq!(legacy.enemy.active_mon().hp, 0, "legacy: enemy KO'd");
        assert_eq!(state.opponent_battlers[0].hp, 0, "stack: enemy KO'd");
        assert_eq!(legacy.player.active_mon().hp, 200, "legacy: player untouched");
        assert_eq!(state.player_battlers[0].hp, 200, "stack: player untouched");
        assert!(result.second_cancelled, "stack must cancel the second move");
        // The cancelled second move drew NO further bytes: exactly 3 consumed.
        assert_eq!(rng.consumed(), 3, "no draws for the cancelled second move");
    }

    /// A determinism fuzz over >= 1000 seeds: random byte vectors → both paths →
    /// identical BattleState + equal stack consumed(). Covers the slice's draw
    /// order under broad input (design §8 GO criterion). Drives the reusable
    /// harness' `run_scenario`.
    #[test]
    fn determinism_fuzz_1000_seeds() {
        // A tiny deterministic LCG so the fuzz itself draws no `rand` (the
        // engine must never link rand; the test crate may, but we keep it self
        // contained and reproducible).
        let mut lcg: u64 = 0x1234_5678_9abc_def0;
        let mut next = || {
            lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (lcg >> 33) as u8
        };

        let statuses = [
            LegacyStatus::None,
            LegacyStatus::Paralysis,
            LegacyStatus::Poison,
        ];

        for seed in 0..1000u32 {
            let pstatus = statuses[(seed as usize) % statuses.len()];
            let estatus = statuses[((seed as usize) / 3) % statuses.len()];
            let focus = seed % 5 == 0;
            let s = Scenario {
                name: "fuzz",
                // keep both above 30 HP so the second mover always acts (the
                // shim's consumed() predictor assumes no mid-turn KO).
                player_speed: 60 + (next() as u16 % 120),
                enemy_speed: 60 + (next() as u16 % 120),
                player_hp: 200,
                enemy_hp: 200,
                player_status: pstatus,
                enemy_status: estatus,
                player_focus_energy: focus,
                player_confused_turns: 0,
                enemy_confused_turns: 0,
                order_byte: next(),
                first: MoveBytes {
                    confusion: 255,
                    paralysis: next(),
                    crit: next(),
                    accuracy: next() % 200, // bias toward hits to exercise damage
                    damage: next().max(1),
                },
                second: MoveBytes {
                    confusion: 255,
                    paralysis: next(),
                    crit: next(),
                    accuracy: next() % 200,
                    damage: next().max(1),
                },
            };
            run_scenario(&s);
        }
    }

    /// Slice-1 reusability smoke: a sibling test module pattern — later slices
    /// import the harness and call its helpers directly (here `build_stack_stream`
    /// + `first_mover` + `order_is_tie`) to assert the predicted byte count, with
    /// NO local plumbing. Proves the harness is reusable as designed.
    #[test]
    fn harness_predictor_is_reusable() {
        let s = Scenario {
            name: "reuse-smoke",
            player_speed: 100,
            enemy_speed: 100, // exact tie → order byte drawn
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
        };
        let first = first_mover(&s);
        let (bytes, expected) = build_stack_stream(&s, first, order_is_tie(&s));
        // tie ⇒ 1 order byte + 2 movers × (crit, acc, dmg) = 1 + 6 = 7.
        assert_eq!(expected, 7, "predicted byte count");
        assert_eq!(bytes.len(), expected);
        // And the full scenario still passes through the harness end-to-end.
        run_scenario(&s);
    }
}

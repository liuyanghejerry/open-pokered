//! P0 — the **production RNG shim + two-mover + AI differential** scenarios
//! (migration blueprint `15` §4 / §5 P0).
//!
//! This is the make-or-break prerequisite for the whole pokered battle
//! migration. Every prior slice (1-7) drives a two-mover turn whose ENEMY move
//! is fixed; none exercises the one draw the production loop interleaves: the AI
//! `pick_enemy_move` `rand::random()`. The blueprint flagged that AI-draw
//! interleave as the **top unproven hazard** (risk #1) — "can silently desync
//! every turn".
//!
//! ## The finding (the load-bearing P0 result)
//!
//! Reading the production live loop `BattleScreen::execute_turn_with_move`
//! (`mod.rs:1760-1885`):
//!   * `pick_enemy_move` is called at `mod.rs:1766` — drawing the AI byte(s)
//!     (`rand::random::<u8>()` trainer pick `mod.rs:794`, or
//!     `rand::random::<usize>() % len` wild fallback `mod.rs:803`) —
//!   * STRICTLY BEFORE `generate_turn_randoms()` at `mod.rs:1784` (which rolls
//!     `order_random`, then first_mover, then second_mover MoveRandoms).
//!
//! So the AI draw is a clean PREFIX of the turn's RNG stream, NOT an interleave
//! mid-`TurnRandoms`. **A prefixed draw IS reproducible by a streamed `BattleRng`**
//! — this is the migration-safe outcome (NOT a NO-GO). These scenarios prove it:
//! the harness lays the AI byte(s) at the front of ONE shared byte vector, draws
//! them via the re-homed REAL AI code on BOTH the legacy-oracle path and the
//! stack path, runs the rest of the turn, and asserts IDENTICAL resulting
//! `BattleState` AND identical `consumed()` (AI count + turn count).
//!
//! Additive + test-only (`#[cfg(test)]`); does NOT touch the production battle
//! loop. The legacy [`execute_turn`](super::turn::execute_turn) stays the oracle;
//! the real AI code (`move_choice_layers`/`choose_moves`/`pick_move`) is re-homed,
//! not reimplemented.

#![cfg(test)]

#[cfg(test)]
mod p0_ai_tests {
    use crate::battle::stack_parity::{
        ai_draw_count_pub, build_ai_turn_stream_pub, first_mover_ai_pub, harness_pick_enemy_move,
        order_is_tie_ai_pub, run_scenario_ai, AiMonSpec, AiScenario,
    };

    use crate::battle::state::StatusCondition as LegacyStatus;
    use jrpg_engine::battle::rng::ScriptedRng;
    use pokered_data::moves::MoveId;
    use pokered_data::trainer_data::TrainerClass;

    use crate::battle::stack_parity::MoveBytes;

    /// A wild (`trainer_class = None`) scenario base: the AI draws the ONE
    /// fallback byte, picks among the enemy's available moves, both movers act.
    fn wild(name: &'static str) -> AiScenario {
        AiScenario {
            name,
            player: AiMonSpec::solo(200, 100),
            enemy: AiMonSpec::solo(200, 50),
            player_move: MoveId::Thundershock,
            trainer_class: None,
            ai_byte: 0,
            order_byte: 0,
            first: MoveBytes::always_hit(),
            second: MoveBytes::always_hit(),
        }
    }

    /// A two-move enemy set so the wild fallback's `byte % len` selects between
    /// distinct slots (slot 0 Thundershock, slot 1 QuickAttack = +1 priority).
    fn two_move_enemy(hp: u16, speed: u16) -> AiMonSpec {
        AiMonSpec::multi(
            hp,
            speed,
            [MoveId::Thundershock, MoveId::QuickAttack, MoveId::None, MoveId::None],
            [30, 30, 0, 0],
        )
    }

    /// The ≥20-scenario P0 matrix: player-faster, enemy-faster, speed-tie (order
    /// byte both below and at/above 128), a miss, a faint short-circuit, a crit, a
    /// paralysis gate, and the AI choosing among two moves (incl. a priority move
    /// that flips turn order). Each runs BOTH paths on the SAME shared byte vector
    /// and asserts state + consumed() parity (the AI draw at the production ordinal).
    fn matrix() -> Vec<AiScenario> {
        let mut v = Vec::new();

        // 1. wild, player faster, normal hit both sides.
        v.push(wild("wild: player faster, normal hit"));

        // 2. wild, enemy faster.
        {
            let mut s = wild("wild: enemy faster");
            s.player = AiMonSpec::solo(200, 50);
            s.enemy = AiMonSpec::solo(200, 100);
            v.push(s);
        }

        // 3. speed-tie, order byte < 128 → player first.
        {
            let mut s = wild("wild: speed tie, order byte 0 -> player first");
            s.player = AiMonSpec::solo(200, 80);
            s.enemy = AiMonSpec::solo(200, 80);
            s.order_byte = 0;
            v.push(s);
        }

        // 4. speed-tie, order byte 127 (boundary below 128) → player first.
        {
            let mut s = wild("wild: speed tie, order byte 127 -> player first");
            s.player = AiMonSpec::solo(200, 80);
            s.enemy = AiMonSpec::solo(200, 80);
            s.order_byte = 127;
            v.push(s);
        }

        // 5. speed-tie, order byte 128 (boundary at 128) → enemy first.
        {
            let mut s = wild("wild: speed tie, order byte 128 -> enemy first");
            s.player = AiMonSpec::solo(200, 80);
            s.enemy = AiMonSpec::solo(200, 80);
            s.order_byte = 128;
            v.push(s);
        }

        // 6. speed-tie, order byte 255 (high) → enemy first.
        {
            let mut s = wild("wild: speed tie, order byte 255 -> enemy first");
            s.player = AiMonSpec::solo(200, 80);
            s.enemy = AiMonSpec::solo(200, 80);
            s.order_byte = 255;
            v.push(s);
        }

        // 7. a clean miss (first mover): accuracy byte 255 → the 1/256 Gen-1 miss.
        {
            let mut s = wild("wild: first mover misses (1/256 bug)");
            s.first = MoveBytes {
                confusion: 255,
                paralysis: 255,
                crit: 255,
                accuracy: 255, // 100% acc -> scaled 255; 255 !< 255 -> miss
                damage: 255,
            };
            v.push(s);
        }

        // 8. a clean miss (second mover).
        {
            let mut s = wild("wild: second mover misses");
            s.second = MoveBytes {
                confusion: 255,
                paralysis: 255,
                crit: 255,
                accuracy: 255,
                damage: 255,
            };
            v.push(s);
        }

        // 9. a crit (first mover crit byte 0 → guaranteed crit).
        {
            let mut s = wild("wild: first mover crits");
            s.first = MoveBytes {
                confusion: 255,
                paralysis: 255,
                crit: 0, // below threshold -> crit
                accuracy: 0,
                damage: 255,
            };
            v.push(s);
        }

        // 10. player fully paralyzed (first mover): paralysis byte < 63, move skipped.
        {
            let mut s = wild("wild: player fully paralyzed, move skipped");
            s.player = AiMonSpec {
                status: LegacyStatus::Paralysis,
                ..AiMonSpec::solo(200, 100)
            };
            s.first = MoveBytes {
                confusion: 255,
                paralysis: 0, // < 63 -> fully paralyzed
                crit: 255,
                accuracy: 0,
                damage: 255,
            };
            v.push(s);
        }

        // 11. player paralyzed but acts (paralysis byte >= 63).
        {
            let mut s = wild("wild: player paralyzed but acts");
            s.player = AiMonSpec {
                status: LegacyStatus::Paralysis,
                ..AiMonSpec::solo(200, 200)
            };
            s.enemy = AiMonSpec::solo(200, 10);
            s.first = MoveBytes {
                confusion: 255,
                paralysis: 200, // >= 63 -> acts
                crit: 255,
                accuracy: 0,
                damage: 255,
            };
            v.push(s);
        }

        // 12. enemy poisoned → residual after its move (status persistence parity).
        {
            let mut s = wild("wild: enemy poisoned residual");
            s.enemy = AiMonSpec {
                status: LegacyStatus::Poison,
                ..AiMonSpec::solo(200, 50)
            };
            v.push(s);
        }

        // 13. AI byte selects slot 0 (Thundershock) of a two-move enemy.
        {
            let mut s = wild("wild: AI fallback picks slot 0 (len=2, byte 0)");
            s.enemy = two_move_enemy(200, 50);
            s.ai_byte = 0; // 0 % 2 == 0 -> slot 0 (Thundershock, priority 0)
            v.push(s);
        }

        // 14. AI byte selects slot 1 (QuickAttack, +1 priority) — enemy now moves
        //     FIRST despite being slower (the AI pick flips turn order). Both paths
        //     must agree on the move AND the resulting order.
        {
            let mut s = wild("wild: AI fallback picks slot 1 (QuickAttack flips order)");
            s.enemy = two_move_enemy(200, 50); // slower, but QuickAttack = +1
            s.ai_byte = 1; // 1 % 2 == 1 -> slot 1 (QuickAttack)
            v.push(s);
        }

        // 15. faint short-circuit: player KOs the enemy (enemy hp 1) → enemy's AI-
        //     chosen move is cancelled (the second mover does not act).
        {
            let mut s = wild("wild: player KOs enemy -> second move cancelled");
            s.enemy = AiMonSpec::solo(1, 50);
            v.push(s);
        }

        // 16. trainer (BugCatcher = Layer1 only): the AI PICK byte path (mod.rs:794),
        //     drawn FIRST. Single-move enemy so the pick is deterministic.
        {
            let mut s = wild("trainer BugCatcher: AI pick byte (Layer1)");
            s.trainer_class = Some(TrainerClass::BugCatcher);
            v.push(s);
        }

        // 17. trainer (BugCatcher) with a two-move enemy, AI byte 0.
        {
            let mut s = wild("trainer BugCatcher: two-move enemy, AI byte 0");
            s.trainer_class = Some(TrainerClass::BugCatcher);
            s.enemy = two_move_enemy(200, 50);
            s.ai_byte = 0;
            v.push(s);
        }

        // 18. trainer (BugCatcher) with a two-move enemy, AI byte 1.
        {
            let mut s = wild("trainer BugCatcher: two-move enemy, AI byte 1");
            s.trainer_class = Some(TrainerClass::BugCatcher);
            s.enemy = two_move_enemy(200, 50);
            s.ai_byte = 1;
            v.push(s);
        }

        // 19. trainer (Misty = Layer1+Layer3) — the type-effectiveness AI path,
        //     two-move enemy, AI byte 0.
        {
            let mut s = wild("trainer Misty: Layer1+Layer3 AI, byte 0");
            s.trainer_class = Some(TrainerClass::Misty);
            s.enemy = two_move_enemy(200, 50);
            s.ai_byte = 0;
            v.push(s);
        }

        // 20. trainer (Misty) two-move enemy, AI byte 1.
        {
            let mut s = wild("trainer Misty: Layer1+Layer3 AI, byte 1");
            s.trainer_class = Some(TrainerClass::Misty);
            s.enemy = two_move_enemy(200, 50);
            s.ai_byte = 1;
            v.push(s);
        }

        // 21. trainer + speed-tie at/above 128 → enemy first (AI byte then order byte).
        {
            let mut s = wild("trainer BugCatcher: speed tie, order 200 -> enemy first");
            s.trainer_class = Some(TrainerClass::BugCatcher);
            s.player = AiMonSpec::solo(200, 80);
            s.enemy = AiMonSpec::solo(200, 80);
            s.order_byte = 200;
            v.push(s);
        }

        // 22. trainer + a miss on the first mover (AI prefix + miss interleave).
        {
            let mut s = wild("trainer BugCatcher: first mover misses");
            s.trainer_class = Some(TrainerClass::BugCatcher);
            s.first = MoveBytes {
                confusion: 255,
                paralysis: 255,
                crit: 255,
                accuracy: 255,
                damage: 255,
            };
            v.push(s);
        }

        v
    }

    /// The ≥20-scenario P0 differential: every scenario runs BOTH the legacy
    /// `execute_turn` oracle and the `StackDriver` on the SAME shared byte vector
    /// (AI prefix ++ turn fire order) and asserts IDENTICAL resulting state AND
    /// identical consumed() — the AI-draw interleave proof.
    #[test]
    fn p0_two_mover_plus_ai_matrix() {
        let m = matrix();
        assert!(m.len() >= 20, "P0 requires >= 20 scenarios, got {}", m.len());
        for s in m {
            run_scenario_ai(&s);
        }
    }

    /// THE KEY-RISK PROOF (the scenario that exercises the AI-draw interleave):
    /// a two-move enemy where the AI byte selects slot 1 = QuickAttack (+1
    /// priority), so the AI pick FLIPS turn order. Both paths must agree the AI
    /// picked QuickAttack, that the enemy therefore moved first, AND consume the
    /// IDENTICAL byte count (1 AI byte + the turn bytes). This is the single
    /// scenario where the AI draw both (a) lands at the production ordinal and
    /// (b) materially changes the turn outcome — proving the interleave matches.
    #[test]
    fn p0_ai_pick_flips_turn_order_and_consumed_matches() {
        let s = AiScenario {
            name: "AI pick QuickAttack flips order (key-risk proof)",
            player: AiMonSpec::solo(200, 100), // faster on raw speed
            enemy: two_move_enemy(200, 50),    // slower, but QuickAttack = +1
            player_move: MoveId::Thundershock,
            trainer_class: None,
            ai_byte: 1, // 1 % 2 == 1 -> slot 1 = QuickAttack
            order_byte: 0,
            first: MoveBytes::always_hit(),
            second: MoveBytes::always_hit(),
        };

        // The AI re-home picks QuickAttack on byte 1 (proven directly).
        let bs = {
            use crate::battle::state::{new_battle_state, BattleType, Pokemon};
            use pokered_data::pokemon_data::get_base_stats;
            use pokered_data::species::Species;
            let base = get_base_stats(Species::Pikachu).unwrap();
            let mk = |spec: &AiMonSpec| Pokemon {
                species: spec.species,
                nickname: None,
                level: 50,
                hp: spec.hp,
                max_hp: spec.hp,
                attack: 100,
                defense: 80,
                speed: spec.speed,
                special: 80,
                type1: base.type1,
                type2: base.type2,
                moves: spec.moves,
                pp: spec.pp,
                pp_ups: [0; 4],
                status: spec.status,
                dv_bytes: [0xFF, 0xFF],
                stat_exp: [0; 5],
                total_exp: 0,
                is_traded: false, ot_id: 0, ot_name: None,
            };
            new_battle_state(BattleType::Wild, vec![mk(&s.player)], vec![mk(&s.enemy)])
        };
        let mut probe = ScriptedRng::new(vec![1u8]);
        let (picked, idx) = harness_pick_enemy_move(&bs, None, &mut probe);
        assert_eq!(picked, MoveId::QuickAttack, "AI byte 1 must pick slot 1");
        assert_eq!(idx, 1);
        assert_eq!(probe.consumed(), 1, "wild fallback draws exactly one byte");

        // The full differential: state + consumed parity (the interleave proof).
        run_scenario_ai(&s);

        // And the order flip is real: with QuickAttack chosen, the enemy (slower
        // raw speed) moves FIRST on both paths.
        let first = first_mover_ai_pub(&s, MoveId::QuickAttack);
        assert!(
            matches!(first, jrpg_engine::battle::stack::FirstMover::Opponent),
            "QuickAttack pick must make the slower enemy move first"
        );
        // Cross-check: if the AI had picked slot 0 (Thundershock, priority 0), the
        // faster player would move first — proving the AI byte changes the outcome.
        let first0 = first_mover_ai_pub(&s, MoveId::Thundershock);
        assert!(
            matches!(first0, jrpg_engine::battle::stack::FirstMover::Player),
            "Thundershock pick would keep the faster player first"
        );
    }

    /// STANDING PIN #1 (non-negotiable): the order byte is drawn FIRST, exactly
    /// ONCE, even on a speed tie. With the AI prefix in front, the order byte sits
    /// at ordinal `ai_count` and is consumed exactly once on a tie (and never on a
    /// non-tie). We assert the predicted turn stream begins with the order byte on
    /// a tie, contains it exactly once, and the AI prefix never duplicates it.
    #[test]
    fn pin_order_byte_drawn_first_exactly_once_even_on_tie() {
        // A speed tie (both 80) with a distinctive order byte.
        let s = AiScenario {
            name: "pin: order byte once on tie",
            player: AiMonSpec::solo(200, 80),
            enemy: AiMonSpec::solo(200, 80),
            player_move: MoveId::Thundershock,
            trainer_class: None,
            ai_byte: 42,
            order_byte: 77,
            first: MoveBytes::always_hit(),
            second: MoveBytes::always_hit(),
        };
        let enemy_move = MoveId::Thundershock;
        let tie = order_is_tie_ai_pub(&s, enemy_move);
        assert!(tie, "the pin requires an exact speed tie");

        let first = first_mover_ai_pub(&s, enemy_move);
        let (turn_bytes, _n) = build_ai_turn_stream_pub(&s, first, tie);

        // The order byte is the FIRST byte of the turn (post-AI) stream …
        assert_eq!(turn_bytes[0], s.order_byte, "order byte must be drawn first");
        // … and appears EXACTLY ONCE in the turn stream (the slice's per-mover
        // bytes are all 255/0 from `always_hit`, distinct from 77).
        let occurrences = turn_bytes.iter().filter(|&&b| b == s.order_byte).count();
        assert_eq!(
            occurrences, 1,
            "order byte must be drawn exactly once even on a tie (got {occurrences})"
        );

        // Non-tie sweep: at every NON-tie speed pair the turn stream must NOT
        // begin with an order byte (it is not drawn at all), proving the tie is the
        // sole order-draw site.
        let mut nontie = s.clone();
        nontie.player = AiMonSpec::solo(200, 100);
        nontie.enemy = AiMonSpec::solo(200, 50);
        let nt_tie = order_is_tie_ai_pub(&nontie, enemy_move);
        assert!(!nt_tie, "speeds differ -> not a tie");
        let nt_first = first_mover_ai_pub(&nontie, enemy_move);
        let (nt_bytes, _m) = build_ai_turn_stream_pub(&nontie, nt_first, nt_tie);
        // First turn byte is a per-mover byte (crit/para), never the order byte's slot.
        assert_ne!(
            nt_bytes.first().copied(),
            Some(nontie.order_byte),
            "no order byte may be drawn on a non-tie"
        );

        // End-to-end: the tie scenario still passes the full differential.
        run_scenario_ai(&s);
    }

    /// STANDING PIN #2 (non-negotiable): crit is drawn BEFORE accuracy, EVEN with
    /// the AI prefix in front of the stream. The trap: crit byte 0 (→ crit if
    /// drawn at the crit offset) + accuracy byte 255 (→ the 1/256 miss if drawn at
    /// the accuracy offset). Correct fire order = a critical MISS (no damage);
    /// swapped = a non-crit HIT. The stack must match the legacy oracle (critical
    /// miss), and the AI byte sitting in front must NOT shift the crit/accuracy
    /// relative order. We assert the enemy is untouched (the critical miss) on both
    /// paths — which only holds if crit was drawn before accuracy AFTER the AI byte.
    #[test]
    fn pin_crit_drawn_before_accuracy_even_with_ai_prefix() {
        let s = AiScenario {
            name: "pin: crit before accuracy (AI prefix)",
            player: AiMonSpec::solo(200, 100), // player faster -> first mover
            enemy: AiMonSpec::solo(200, 50),
            player_move: MoveId::Thundershock,
            trainer_class: None,
            ai_byte: 99, // a distinctive AI prefix byte
            order_byte: 0,
            first: MoveBytes {
                confusion: 255,
                paralysis: 255,
                crit: 0,       // crit offset: 0 -> crit IF drawn here
                accuracy: 255, // accuracy offset: 255 -> the 1/256 miss IF drawn here
                damage: 255,
            },
            second: MoveBytes::always_hit(),
        };
        // The full differential must pass (state parity). Correct fire order makes
        // the first mover's hit a CRITICAL MISS → the enemy takes no damage from it.
        run_scenario_ai(&s);

        // Explicit semantic check on the stack: the enemy must be UNTOUCHED by the
        // player's (first) move — only the enemy's (second) move damaged the
        // player. If accuracy had been drawn before crit, the 0 byte would read as
        // a HIT and the enemy would have taken damage.
        use crate::battle::stack_parity::stack_run_ai;
        let (stack, _consumed, _first, _mv) = stack_run_ai(&s);
        assert_eq!(
            stack.opponent_battlers[0].hp, s.enemy.hp,
            "crit-before-accuracy: the critical-miss first move must leave the enemy untouched"
        );
    }

    /// Inert-handler invariant: a `NoAdditionalEffect` AI scenario draws NO extra
    /// secondary byte; consumed() is exactly AI + turn bytes (the side-effect
    /// handler returns immediately without reading a byte). Pinned via the matrix
    /// (every scenario asserts consumed()), re-stated here as a focused check that
    /// the AI prefix does not perturb the per-mover byte count.
    #[test]
    fn inert_handlers_keep_consumed_invariant_with_ai_prefix() {
        // Wild, both act, no status: AI 1 byte + 2 movers × (crit, acc, dmg) = 7.
        let s = wild("inert: 1 AI + 6 turn = 7");
        let enemy_move = MoveId::Thundershock;
        let tie = order_is_tie_ai_pub(&s, enemy_move);
        let first = first_mover_ai_pub(&s, enemy_move);
        let (_turn, turn_n) = build_ai_turn_stream_pub(&s, first, tie);
        let ai_n = ai_draw_count_pub(
            s.enemy.moves,
            s.enemy.pp,
            s.trainer_class,
            s.ai_byte,
            &s,
        );
        assert_eq!(ai_n, 1, "wild fallback draws exactly one AI byte");
        assert_eq!(turn_n, 6, "two movers × (crit, acc, dmg) = 6 turn bytes");
        // And the differential confirms the stack actually consumed 7 total.
        run_scenario_ai(&s);
    }
}

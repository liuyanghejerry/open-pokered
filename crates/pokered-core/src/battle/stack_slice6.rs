//! Effect-stack battle engine — **slice 6: multi-turn lock-in (Thrash / Fly /
//! Hyper-Beam recharge) + Counter + Bide as effect-stack handlers** (design doc
//! `06-battle-engine-effect-stack-design.md` §3.1 `LockedMove`/`TwoTurn`/`Bide`,
//! §6 #14/#15/#17/#18/#20, §9 open question, slice 6 of the §7 strangler plan).
//!
//! ## The §9 open question — RESOLVED here (and recorded in the doc)
//!
//! *Where does `MoveContext.last_damage` live, and where does locked-move / Bide
//! state live?*
//!
//! * **Cross-turn locked state → the `EffectState` ARENA.** Thrash's lock counter,
//!   Fly's charge flag, Hyper Beam's recharge flag, and Bide's accumulator all
//!   persist across turns reusing one arena (mirroring slice 5's Toxic ramp). A
//!   live volatile recorded on a PRIOR turn hijacks THIS turn's action through the
//!   engine's generic, defaulted [`EffectProvider::forced_action`] seam
//!   (`MoveGate::ForcedAction`-shaped) — the canonical proof that a per-turn
//!   `[Action; 2]` input is insufficient.
//! * **`last_damage` for Counter → a PER-BATTLER arena scratch, not `mv`.** The
//!   driver's `MoveContext.last_damage` is reset PER MOVER. Counter (−1 priority)
//!   reads the damage IT took when the OPPONENT moved — a DIFFERENT `MoveContext`.
//!   So the cross-action read must live per-battler in the arena
//!   ([`PocKind::DamageTaken`]), reset at the start of each turn. (Bide folds the
//!   same scratch into its accumulator.) The recommended "`last_damage` as
//!   `MoveContext` scratch" answer holds ONLY for SAME-action reads (recoil,
//!   drain); the cross-action reactive reads (Counter/Bide) need the arena. This
//!   is the canonical §9 finding.
//!
//! ## Counter makes `pair_mut` GENUINELY load-bearing
//!
//! [`counter_handler`](crate::battle::stack_parity) reflects `2 × physical damage
//! taken` by taking `ctx.pair_mut(source, target)` — it READS the Counter user's
//! (`source`) own per-turn state + live hp while WRITING the opponent's
//! (`target`) hp through the SAME paired `&mut`, then zeroes `ctx.mv.damage` so the
//! driver's auto-apply does not double-hit. Drop the paired write and Counter does
//! nothing — unlike slice 4's Substitute (whose absorb routed through the arena),
//! this is the "mutate target while reading source" the design (§3.2) says
//! `pair_mut` is for.
//!
//! ## Oracle scope — the HONEST design finding
//!
//! The legacy `execute_turn` (turn.rs) CANNOT drive multi-turn lock-in (it takes a
//! fixed `[move; 2]` each turn, never re-issues a locked move) and has NO Counter
//! damage at all (`MoveEffect` has no `CounterEffect`). So lock-in/Counter cannot
//! be diffed against `execute_turn` MULTI-TURN — exactly why the engine needed the
//! cross-turn `forced_action` seam. The slice therefore:
//!   * DIFFS the per-turn damage MATH (a forced strike = a normal move) vs the
//!     legacy single-turn `execute_turn` oracle (`legacy_single_turn_damage`);
//!   * pins the lock-in EFFECT math (Bide ×2, Thrash end-confusion, charge/strike,
//!     recharge skip) — re-homing the legacy `multi_turn_effects.rs` unit-test
//!     constants exactly;
//!   * DIRECT-PINS Counter's 2× reflection (= 2× the diffable opponent damage),
//!     while the diffable opponent hit IS asserted vs the legacy oracle.
//!
//! Additive, test-only: it does NOT touch the production battle loop; the legacy
//! `multi_turn_effects.rs` / `execute_turn` stay authoritative.

#![cfg(test)]

#[cfg(test)]
mod slice6_tests {
    use crate::battle::stack_parity::{
        legacy_single_turn_damage, stack_bide, stack_confused, stack_lock_turns,
        stack_recharging, stack_run_lockin, stack_twoturn_charging, Lockin, LockinScenario,
        MoveBytes,
    };
    use dotzuki_engine::battle::BattlerRef;

    use pokered_data::move_data::MoveData;
    use pokered_data::moves::{MoveEffect, MoveId};
    use pokered_data::types::PokemonType;

    /// A Normal-type PHYSICAL move (power 40, acc 100) — the carrier for Counter
    /// (Counter only reflects PHYSICAL damage) and the lock-in damage math.
    fn tackle() -> MoveData {
        MoveData {
            id: MoveId::Tackle,
            effect: MoveEffect::NoAdditionalEffect,
            power: 40,
            move_type: PokemonType::Normal,
            accuracy: 100,
            pp: 35,
        }
    }

    /// An Electric SPECIAL move (power 40) — for the "Counter ignores special" test.
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

    /// always-MISS per-mover bytes (accuracy 255 → 1/256 miss → no damage byte).
    fn miss() -> MoveBytes {
        MoveBytes { confusion: 255, paralysis: 255, crit: 255, accuracy: 255, damage: 255 }
    }

    // ═══════════════════════════ COUNTER (bug #20) ═══════════════════════════

    /// Counter reflects 2× the PHYSICAL damage the Counter user took this turn.
    /// The opponent (faster + priority 0) hits the Counter user (−1 priority, last)
    /// with Tackle; Counter then reflects 2× onto the opponent — via the load-
    /// bearing `pair_mut`. The diffable opponent hit is asserted vs the legacy
    /// oracle; the reflection is direct-pinned (= 2× that diffable damage).
    #[test]
    fn counter_reflects_twice_physical_damage_taken() {
        let mut s = LockinScenario::base("counter 2x physical");
        s.move_data = tackle(); // opponent's physical move (records DamageTaken)
        s.player_choice = MoveId::Counter; // player (−1) goes LAST
        s.enemy_choice = MoveId::Tackle; // opponent (0) goes FIRST, deals damage
        s.player_hp = 5000;
        s.enemy_hp = 5000;
        // Opponent is faster anyway (player_speed default 200 > enemy 50), but
        // Counter's −1 priority guarantees it moves last regardless.
        s.enemy_speed = 50;
        s.player_speed = 200;

        // The diffable opponent damage (one Tackle hit on a clean Pikachu).
        let tackle_dmg = legacy_single_turn_damage(&tackle(), 5000, 5000);
        assert!(tackle_dmg > 0, "Tackle must deal damage for the test to be meaningful");

        let (stack, _consumed, _eff) = stack_run_lockin(&s);

        // Counter user took the opponent's Tackle.
        assert_eq!(
            stack.player_battlers[0].hp,
            5000 - tackle_dmg,
            "Counter user took the opponent's physical hit (diffable vs legacy)"
        );
        // Counter reflected 2× that damage onto the opponent (direct pin — NO
        // legacy oracle for Counter; expected = 2× the diffable opponent damage).
        assert_eq!(
            stack.opponent_battlers[0].hp,
            5000 - tackle_dmg * 2,
            "Counter reflected 2x the physical damage taken (pair_mut)"
        );
    }

    /// Counter deals NOTHING when the damage taken was SPECIAL (Thundershock is
    /// Electric → special in Gen-1): only the opponent's hit lands, no reflection.
    #[test]
    fn counter_ignores_special_damage() {
        let mut s = LockinScenario::base("counter vs special = 0");
        s.move_data = thundershock(); // SPECIAL → Counter must not reflect
        s.player_choice = MoveId::Counter;
        s.enemy_choice = MoveId::Thundershock;
        s.player_hp = 5000;
        s.enemy_hp = 5000;

        let ts_dmg = legacy_single_turn_damage(&thundershock(), 5000, 5000);
        let (stack, _c, _e) = stack_run_lockin(&s);

        assert_eq!(
            stack.player_battlers[0].hp,
            5000 - ts_dmg,
            "Counter user took the special hit (diffable vs legacy)"
        );
        assert_eq!(
            stack.opponent_battlers[0].hp, 5000,
            "Counter does NOT reflect special damage (opponent untouched)"
        );
    }

    /// Counter deals NOTHING when the user took NO damage this turn (opponent
    /// missed): no DamageTaken recorded → Counter fails.
    #[test]
    fn counter_with_no_damage_taken_fails() {
        let mut s = LockinScenario::base("counter no damage = 0");
        s.move_data = tackle();
        s.player_choice = MoveId::Counter;
        s.enemy_choice = MoveId::Tackle;
        s.first = miss(); // first mover (opponent) MISSES → no DamageTaken
        s.second = miss();
        s.player_hp = 5000;
        s.enemy_hp = 5000;

        let (stack, _c, _e) = stack_run_lockin(&s);
        assert_eq!(stack.player_battlers[0].hp, 5000, "opponent missed → no damage taken");
        assert_eq!(
            stack.opponent_battlers[0].hp, 5000,
            "Counter fails with no damage taken (opponent untouched)"
        );
    }

    // ═══════════════════════════ BIDE (bug #18) ══════════════════════════════

    /// Bide accumulates the PHYSICAL+special damage taken over 2 storing turns and
    /// unleashes 2× on turn 3 (re-homes `apply_bide`: ×2, NOT ×3). The Bide user
    /// stores while the opponent hits it each turn; on the unleash turn the
    /// accumulated damage ×2 hits the opponent. Cross-turn via the persistent arena.
    #[test]
    fn bide_accumulates_two_turns_unleashes_twice() {
        let mut s = LockinScenario::base("bide accumulate-2 unleash-2x");
        s.move_data = tackle(); // opponent hits the Bide user with Tackle each turn
        s.player_choice = MoveId::Bide; // player bides (forced each turn)
        s.enemy_choice = MoveId::Tackle;
        // Bide stores for 3 residual ticks (≙ legacy `apply_bide`: set num_attacks
        // _left then decrement to 0 over 3 calls), unleashing on the 3rd. Each tick
        // folds the damage taken that turn into the accumulator.
        s.player_lockin = Lockin::Bide { turns_left: 3, accumulated: 0 };
        s.player_hp = 60000; // survive the accumulating hits
        s.enemy_hp = 60000;
        s.player_speed = 50; // SLOWER so the opponent hits BEFORE Bide's residual
        s.enemy_speed = 200;
        s.turns = 3;

        let tackle_dmg = legacy_single_turn_damage(&tackle(), 60000, 60000);
        assert!(tackle_dmg > 0);

        let (stack, _c, eff) = stack_run_lockin(&s);

        // Turns 1 & 2: opponent's Tackle is folded into the accumulator (the Bide
        // user takes it each turn). Turn 3: the opponent hits again, that damage is
        // folded too (apply_bide folds the turn-3 damage before unleashing), then
        // the accumulator ×2 is released. Over 3 turns the Bide user took 3 hits.
        // The Bide volatile is gone after unleash.
        assert!(stack_bide(&eff, BattlerRef::PLAYER).is_none(), "Bide cleared after unleash");
        // The Bide user took the opponent's hits each turn (diffable per-hit).
        let user_loss = 60000 - stack.player_battlers[0].hp;
        assert_eq!(user_loss, tackle_dmg * 3, "Bide user took 3 opponent hits");
        // Unleash = 2 × accumulated. Accumulated = the damage taken over the storing
        // turns folded in by `bide_residual` (turns 1,2, and the turn-3 fold before
        // unleash) = 3 × tackle_dmg. Released = 2 × that.
        let accumulated = tackle_dmg * 3;
        assert_eq!(
            stack.opponent_battlers[0].hp,
            60000 - accumulated * 2,
            "Bide unleashed 2x the accumulated damage (×2 not ×3, bug #18)"
        );
    }

    /// Bide while STORING does not attack on the user's own move turn (the storing
    /// turns deal nothing TO the opponent until unleash). After 1 storing turn the
    /// volatile is still live with turns_left decremented and accumulated > 0.
    #[test]
    fn bide_stores_without_attacking_until_unleash() {
        let mut s = LockinScenario::base("bide stores, no early attack");
        s.move_data = tackle();
        s.player_choice = MoveId::Bide;
        s.enemy_choice = MoveId::Tackle;
        s.player_lockin = Lockin::Bide { turns_left: 2, accumulated: 0 };
        s.player_hp = 60000;
        s.enemy_hp = 60000;
        s.player_speed = 50;
        s.enemy_speed = 200;
        s.turns = 1; // only the FIRST storing turn

        let tackle_dmg = legacy_single_turn_damage(&tackle(), 60000, 60000);
        let (stack, _c, eff) = stack_run_lockin(&s);

        // After 1 turn: Bide still live (turns_left 2 → 1), accumulated = 1 hit.
        let (acc, left) = stack_bide(&eff, BattlerRef::PLAYER).expect("Bide still storing");
        assert_eq!(left, 1, "turns_left decremented 2 → 1");
        assert_eq!(acc, tackle_dmg, "accumulated = the 1 hit taken this turn");
        // Opponent UNTOUCHED while storing (no early Bide attack).
        assert_eq!(stack.opponent_battlers[0].hp, 60000, "Bide does not attack while storing");
    }

    // ═══════════════════════ THRASH lock-in (bug #17) ═══════════════════════

    /// Thrash forces the locked move 2–3 turns (here a pre-rolled 2), IGNORING the
    /// chosen action, then self-confuses on fatigue. Proves the engine's
    /// `forced_action` seam re-issues the lock across turns. The per-turn damage is
    /// cross-checked vs the legacy single-turn oracle (a forced strike = a normal
    /// move).
    #[test]
    fn thrash_locks_two_turns_then_self_confuses() {
        let mut s = LockinScenario::base("thrash lock 2 then confuse");
        s.move_data = tackle();
        // The player CHOOSES Thundershock each turn, but the Thrash lock FORCES
        // Tackle — proving the chosen action is ignored.
        s.player_choice = MoveId::Thundershock;
        s.enemy_choice = MoveId::Thundershock; // opponent harmless-ish (special, weak)
        s.player_lockin = Lockin::Locked {
            move_: MoveId::Tackle,
            turns_left: 2,
            confuse_on_end: true,
        };
        s.player_hp = 60000;
        s.enemy_hp = 60000;
        s.player_speed = 200; // player (locked) acts first
        s.enemy_speed = 50;
        s.turns = 2; // exhaust the 2-turn lock

        let tackle_dmg = legacy_single_turn_damage(&tackle(), 60000, 60000);
        let (stack, _c, eff) = stack_run_lockin(&s);

        // The lock is exhausted (turns_left 2 → 0 → removed) and the player
        // self-confused (Thrash fatigue, bug #17).
        assert_eq!(stack_lock_turns(&eff, BattlerRef::PLAYER), None, "lock exhausted + removed");
        assert!(stack_confused(&eff, BattlerRef::PLAYER), "self-confused after Thrash ends");
        // The FORCED Tackle ran each turn (chosen Thundershock was ignored): the
        // opponent took 2 Tackle hits, NOT 2 Thundershock hits. (Cross-check the
        // forced strike deals the legacy Tackle damage, not the chosen move's.)
        let ts_dmg = legacy_single_turn_damage(&thundershock(), 60000, 60000);
        let opp_loss = 60000 - stack.opponent_battlers[0].hp;
        // Opponent loss = 2 forced Tackle hits (the player's contribution) MINUS
        // nothing else from the player; the enemy also acted (Thundershock) but
        // that hits the PLAYER, not itself. So opponent_loss is purely the player's
        // 2 forced Tackle hits.
        assert_eq!(opp_loss, tackle_dmg * 2, "forced Tackle ran twice (chosen move ignored)");
        assert_ne!(
            opp_loss, ts_dmg * 2,
            "the CHOSEN Thundershock did NOT run (forced_action overrode it)"
        );
    }

    /// Boundary pin: a 3-turn Thrash lock stays locked through turn 2 (turns_left
    /// 3 → 2 → 1) and is NOT yet confused — the roll-picks-lock-length boundary.
    #[test]
    fn thrash_three_turn_lock_boundary() {
        let mut s = LockinScenario::base("thrash 3-turn lock boundary");
        s.move_data = tackle();
        s.player_choice = MoveId::Thundershock;
        s.enemy_choice = MoveId::Thundershock;
        s.player_lockin = Lockin::Locked {
            move_: MoveId::Tackle,
            turns_left: 3,
            confuse_on_end: true,
        };
        s.player_hp = 60000;
        s.enemy_hp = 60000;
        s.player_speed = 200;
        s.enemy_speed = 50;
        s.turns = 2; // only 2 of the 3 locked turns

        let (_stack, _c, eff) = stack_run_lockin(&s);
        // After 2 of 3 turns: still locked (turns_left 3 → 1), NOT confused yet.
        assert_eq!(stack_lock_turns(&eff, BattlerRef::PLAYER), Some(1), "3-turn lock → 1 left");
        assert!(!stack_confused(&eff, BattlerRef::PLAYER), "not confused while still locked");
    }

    // ═══════════════════════ FLY two-turn (bug #15) ═════════════════════════

    /// Fly: charge turn (forced `Nothing`, no damage) then strike turn (forced
    /// `Fight{Fly}`, deals damage). Proves the two-turn charge→strike lifecycle via
    /// the cross-turn arena + `forced_action`.
    #[test]
    fn fly_charges_then_strikes() {
        let mut s = LockinScenario::base("fly charge then strike");
        s.move_data = tackle(); // the strike's damage move (power 40 physical)
        s.player_choice = MoveId::Thundershock; // ignored — Fly forces the action
        s.enemy_choice = MoveId::Thundershock;
        s.player_lockin = Lockin::TwoTurn { move_: MoveId::Tackle, invulnerable: true };
        s.player_hp = 60000;
        s.enemy_hp = 60000;
        s.player_speed = 200;
        s.enemy_speed = 50;

        let tackle_dmg = legacy_single_turn_damage(&tackle(), 60000, 60000);

        // ── Turn 1 (charge): opponent UNTOUCHED by the player (forced Nothing). ──
        let mut t1 = s.clone();
        t1.turns = 1;
        let (st1, _c1, eff1) = stack_run_lockin(&t1);
        // On the charge turn the player did Nothing → opponent took NO player hit.
        // (The opponent did act with Thundershock against the player, but that's
        // separate; the opponent's OWN hp is what we assert untouched-by-player.)
        // After turn 1 the volatile flipped to the strike turn (charging=false).
        assert_eq!(stack_twoturn_charging(&eff1, BattlerRef::PLAYER), Some(false), "charge→strike flip");
        assert_eq!(st1.opponent_battlers[0].hp, 60000, "no damage on the charge turn (forced Nothing)");

        // ── Both turns: turn-2 strike deals the Tackle damage; volatile cleared. ──
        let mut t2 = s.clone();
        t2.turns = 2;
        let (st2, _c2, eff2) = stack_run_lockin(&t2);
        assert_eq!(stack_twoturn_charging(&eff2, BattlerRef::PLAYER), None, "two-turn move complete");
        assert_eq!(
            st2.opponent_battlers[0].hp,
            60000 - tackle_dmg,
            "strike turn dealt the forced move's damage"
        );
    }

    // ═══════════════ HYPER BEAM recharge (bug #14) — skips a turn ═══════════

    /// Hyper Beam recharge: the recharge volatile forces `Nothing` next turn (the
    /// mon skips), proving cross-turn action override. A mon WITH a recharge
    /// volatile deals no damage this turn; afterwards the volatile is cleared.
    #[test]
    fn hyper_beam_recharge_skips_the_turn() {
        let mut s = LockinScenario::base("hyper beam recharge skip");
        s.move_data = tackle();
        s.player_choice = MoveId::Tackle; // CHOSEN Tackle — but recharge forces Nothing
        s.enemy_choice = MoveId::Thundershock;
        s.player_lockin = Lockin::Recharge; // must recharge THIS turn
        s.player_hp = 60000;
        s.enemy_hp = 60000;
        s.player_speed = 200;
        s.enemy_speed = 50;
        s.turns = 1;

        let (stack, consumed, eff) = stack_run_lockin(&s);
        // The recharging player did NOTHING (chosen Tackle ignored) → opponent
        // untouched by the player. The recharge volatile is cleared after the skip.
        assert_eq!(stack.opponent_battlers[0].hp, 60000, "recharge → no attack (chosen Tackle ignored)");
        assert!(!stack_recharging(&eff, BattlerRef::PLAYER), "recharge cleared after the skip");
        // consumed: the player drew NOTHING (forced Nothing → no crit/acc/dmg);
        // only the opponent's Thundershock drew its 3 bytes (crit/acc/dmg).
        assert_eq!(consumed, 3, "recharging mover draws no bytes (only the opponent's 3)");
    }

    /// After the recharge skip, the mon can act again next turn (volatile gone →
    /// `forced_action` no longer fires → the chosen action runs).
    #[test]
    fn after_recharge_mon_acts_again() {
        let mut s = LockinScenario::base("recharge then act");
        s.move_data = tackle();
        s.player_choice = MoveId::Tackle;
        s.enemy_choice = MoveId::Thundershock;
        s.player_lockin = Lockin::Recharge;
        s.player_hp = 60000;
        s.enemy_hp = 60000;
        s.player_speed = 200;
        s.enemy_speed = 50;
        s.turns = 2; // turn 1 recharge skip, turn 2 acts

        let tackle_dmg = legacy_single_turn_damage(&tackle(), 60000, 60000);
        let (stack, _c, eff) = stack_run_lockin(&s);
        assert!(!stack_recharging(&eff, BattlerRef::PLAYER), "recharge cleared");
        // Turn 2 the player's CHOSEN Tackle ran → opponent took exactly one hit.
        assert_eq!(
            stack.opponent_battlers[0].hp,
            60000 - tackle_dmg,
            "after recharge, the chosen Tackle ran (one hit, turn 2 only)"
        );
    }

    // ══════════════ ForcedAction proof + the matrix + the fuzz ══════════════

    /// The keystone proof: a locked move IGNORES the per-turn chosen action. The
    /// player CHOOSES a move that would deal a DIFFERENT amount than the forced
    /// one; the forced move's damage proves `forced_action` overrode the choice.
    #[test]
    fn locked_move_ignores_chosen_action() {
        // Choose Thundershock (special, ≠ Tackle damage), lock Tackle.
        let mut s = LockinScenario::base("locked move ignores choice");
        s.move_data = tackle();
        s.player_choice = MoveId::Thundershock; // would-be choice
        s.enemy_choice = MoveId::Splash; // opponent does ~nothing meaningful
        s.player_lockin = Lockin::Locked {
            move_: MoveId::Tackle,
            turns_left: 1,
            confuse_on_end: false,
        };
        s.player_hp = 60000;
        s.enemy_hp = 60000;
        s.player_speed = 200;
        s.enemy_speed = 50;
        s.turns = 1;

        let tackle_dmg = legacy_single_turn_damage(&tackle(), 60000, 60000);
        let ts_dmg = legacy_single_turn_damage(&thundershock(), 60000, 60000);
        assert_ne!(tackle_dmg, ts_dmg, "the two moves MUST differ for this proof");

        let (stack, _c, _eff) = stack_run_lockin(&s);
        let opp_loss = 60000 - stack.opponent_battlers[0].hp;
        assert_eq!(opp_loss, tackle_dmg, "the FORCED Tackle ran, not the chosen move");
        assert_ne!(opp_loss, ts_dmg, "the CHOSEN Thundershock did NOT run (proves ForcedAction)");
    }

    /// The slice-6 matrix: drive every lock-in shape across its natural turn count
    /// and assert the cross-turn volatile state + the opponent's resulting hp.
    #[test]
    fn slice6_matrix_lockin_state() {
        // Thrash 2-turn → confused.
        let mut thrash = LockinScenario::base("m: thrash");
        thrash.move_data = tackle();
        thrash.player_lockin = Lockin::Locked { move_: MoveId::Tackle, turns_left: 2, confuse_on_end: true };
        thrash.player_hp = 60000;
        thrash.enemy_hp = 60000;
        thrash.turns = 2;
        let (_s, _c, eff) = stack_run_lockin(&thrash);
        assert!(stack_confused(&eff, BattlerRef::PLAYER));
        assert_eq!(stack_lock_turns(&eff, BattlerRef::PLAYER), None);

        // Fly charge then strike.
        let mut fly = LockinScenario::base("m: fly");
        fly.move_data = tackle();
        fly.player_lockin = Lockin::TwoTurn { move_: MoveId::Fly, invulnerable: true };
        fly.player_hp = 60000;
        fly.enemy_hp = 60000;
        fly.turns = 2;
        let (_s2, _c2, eff2) = stack_run_lockin(&fly);
        assert_eq!(stack_twoturn_charging(&eff2, BattlerRef::PLAYER), None, "fly complete");

        // Recharge skip then clear.
        let mut hb = LockinScenario::base("m: recharge");
        hb.move_data = tackle();
        hb.player_lockin = Lockin::Recharge;
        hb.player_hp = 60000;
        hb.enemy_hp = 60000;
        hb.turns = 1;
        let (_s3, _c3, eff3) = stack_run_lockin(&hb);
        assert!(!stack_recharging(&eff3, BattlerRef::PLAYER));

        // Bide accumulate then unleash.
        let mut bide = LockinScenario::base("m: bide");
        bide.move_data = tackle();
        bide.player_choice = MoveId::Bide;
        bide.enemy_choice = MoveId::Tackle;
        bide.player_lockin = Lockin::Bide { turns_left: 3, accumulated: 0 };
        bide.player_hp = 60000;
        bide.enemy_hp = 60000;
        bide.player_speed = 50;
        bide.enemy_speed = 200;
        bide.turns = 3;
        let (_s4, _c4, eff4) = stack_run_lockin(&bide);
        assert!(stack_bide(&eff4, BattlerRef::PLAYER).is_none(), "bide unleashed + cleared");
    }

    /// Determinism fuzz over >= 1000 seeds: run a random lock-in shape with random
    /// speeds/bytes/hp for a random turn count and assert the run is reproducible
    /// (same seed → same final state). Self-contained LCG (no `rand`). HP huge so
    /// no faint mid-run. This pins the cross-turn lock-in machinery as deterministic.
    #[test]
    fn slice6_determinism_fuzz_1000_seeds() {
        let mut lcg: u64 = 0x5104_6510_C0FF_EE00;
        let mut next = || {
            lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (lcg >> 33) as u8
        };

        for _seed in 0..1000u32 {
            let shape = next() % 5;
            let mut s = LockinScenario::base("slice6-fuzz");
            s.move_data = tackle();
            s.player_hp = 60000;
            s.enemy_hp = 60000;
            s.player_speed = 60 + (next() as u16 % 120);
            s.enemy_speed = 60 + (next() as u16 % 120);
            s.first = MoveBytes {
                confusion: 255,
                paralysis: 255,
                crit: next(),
                accuracy: next(),
                damage: next(),
            };
            s.second = MoveBytes {
                confusion: 255,
                paralysis: 255,
                crit: next(),
                accuracy: next(),
                damage: next(),
            };
            s.turns = 1 + (next() as u32 % 3); // 1..=3
            match shape {
                0 => {
                    s.player_lockin = Lockin::Locked {
                        move_: MoveId::Tackle,
                        turns_left: 2 + (next() % 2),
                        confuse_on_end: true,
                    };
                }
                1 => {
                    s.player_lockin = Lockin::TwoTurn { move_: MoveId::Tackle, invulnerable: next() % 2 == 0 };
                }
                2 => s.player_lockin = Lockin::Recharge,
                3 => {
                    s.player_choice = MoveId::Bide;
                    s.enemy_choice = MoveId::Tackle;
                    s.player_lockin = Lockin::Bide { turns_left: 2, accumulated: 0 };
                    s.player_speed = 50;
                    s.enemy_speed = 200;
                }
                _ => {
                    // Counter shape.
                    s.player_choice = MoveId::Counter;
                    s.enemy_choice = MoveId::Tackle;
                    s.turns = 1;
                }
            }

            // Reproducibility: same scenario twice → identical final state.
            let (a, ca, _ea) = stack_run_lockin(&s);
            let (b, cb, _eb) = stack_run_lockin(&s);
            assert_eq!(a.player_battlers[0].hp, b.player_battlers[0].hp, "deterministic player hp");
            assert_eq!(
                a.opponent_battlers[0].hp, b.opponent_battlers[0].hp,
                "deterministic opponent hp"
            );
            assert_eq!(ca, cb, "deterministic consumed()");
        }
    }
}

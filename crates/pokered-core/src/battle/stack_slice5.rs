//! Effect-stack battle engine — **slice 5: Gen-1 end-of-turn RESIDUALS as effect
//! handlers** (burn/poison flat tick, Toxic uncapped ramp, Leech Seed cross-
//! battler drain+heal), proven at parity with the legacy `apply_all_residual`
//! oracle (design doc `06-battle-engine-effect-stack-design.md` §3.3 Toxic bug #6,
//! §6 #6/#7, §2 per-mover interleave, slice 5 of the §7 strangler plan).
//!
//! ## The three residual handlers + their ASM `order` (re-home the oracle)
//!
//! | handler          | event      | re-homes               | `order` / arena id |
//! |------------------|------------|------------------------|--------------------|
//! | `poison_residual`| `Residual` | `residual.rs:29-31` flat `/16` min 1 (burn+poison) | `order:10` (status route) |
//! | `toxic_residual` | `Residual` | `residual.rs:33-42` ramp `base*counter`, UNCAPPED (#6) | `order:10`, arena `140` |
//! | `leech_residual` | `Residual` | `residual.rs:53-78` drain host + heal opponent | `order:30`, arena `150` |
//!
//! ASM order = **status damage FIRST, then leech** (`residual.rs:84`). Burn/poison
//! fire via the engine's non-volatile-status residual route; Toxic + Leech fire via
//! the engine's generic arena-residual pass (design §3.4: every live effect on a
//! battler contributes its `Residual` handler). The "status then leech" sequence is
//! pinned WITHOUT any engine gen-1 branch: the game stamps Toxic's arena id `140` <
//! Leech's `150`, and Leech's `order:30` > the status-damage `order:10`.
//!
//! ## Toxic counter (bug #6) — modeled in the arena, ramp DIFFED vs legacy
//!
//! The counter lives in `PocKind::Toxic{counter}` (≙ legacy `BattlerState.
//! toxic_counter`). It increments each tick and damage = `floor(maxHP/16).max(1) *
//! counter`, UNCAPPED. Because the ramp is cross-turn state a single `execute_turn`
//! cannot drive, [`ResidualScenario`](crate::battle::stack_parity::ResidualScenario)
//! runs MULTIPLE turns reusing ONE persistent arena, and the counter (1,2,3,…) is
//! asserted equal to the legacy `toxic_counter` every run.
//!
//! ## Leech Seed — the genuinely load-bearing `pair_mut`
//!
//! Slice 4 reported its Substitute absorb did NOT *require* `pair_mut`. Leech Seed
//! DOES: it drains the host's real `hp` AND heals the opponent's real `hp` in one
//! tick — two distinct battlers' real hp, both writes essential — through a single
//! cross-side `ctx.pair_mut(host, opponent)`. Drop either write and the residual is
//! wrong. This is the first load-bearing cross-battler residual.
//!
//! ## Determinism — residuals draw NO rng
//!
//! Gen-1 residuals are fixed `/16` math (no roll), so adding them leaves
//! `consumed()` UNCHANGED vs the no-residual run. Every scenario asserts this: the
//! consumed count is predicted as `bytes/turn × turns` from the SAME crit→accuracy
//! →damage predictor used by slices 3/4 (which knows nothing of residuals), so a
//! residual that secretly drew a byte diverges and fails.
//!
//! Additive, test-only: it does NOT touch the production battle loop; the legacy
//! `apply_all_residual` / `execute_turn` stays authoritative.

#![cfg(test)]

#[cfg(test)]
mod slice5_tests {
    use crate::battle::stack_parity::{
        legacy_run_residual, run_scenario_residual, stack_run_residual, stack_seeded,
        stack_toxic_counter, ResidualMon, ResidualScenario,
    };
    use crate::battle::state::StatusCondition as S;
    use dotzuki_engine::battle::BattlerRef;

    use pokered_data::move_data::MoveData;
    use pokered_data::moves::{MoveEffect, MoveId};
    use pokered_data::types::PokemonType;

    /// The slice baseline move (Electric, power 40, acc 100). Reused unchanged.
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

    /// always-hit per-mover bytes (crit none, acc hit, damage max).
    fn hit() -> crate::battle::stack_parity::MoveBytes {
        crate::battle::stack_parity::MoveBytes::always_hit()
    }

    /// always-MISS per-mover bytes (accuracy 255 → the 1/256 miss, so no move
    /// damage and NO damage byte drawn). Isolates residual-only HP change.
    fn miss() -> crate::battle::stack_parity::MoveBytes {
        crate::battle::stack_parity::MoveBytes {
            confusion: 255,
            paralysis: 255,
            crit: 255,
            accuracy: 255, // 255 >= scaled 255 → MISS
            damage: 255,
        }
    }

    /// Neutral one-turn scenario: two clean Pikachus, both miss (so only residuals
    /// move HP). Player faster (acts first). High HP so no faint. Caller mutates
    /// the residual flags + turns.
    fn base(name: &'static str) -> ResidualScenario {
        ResidualScenario {
            name,
            player: ResidualMon::clean(5000, 200),
            enemy: ResidualMon::clean(5000, 50),
            move_data: thundershock(),
            order_byte: 0,
            first: miss(),
            second: miss(),
            turns: 1,
        }
    }

    // ───────────────────────── burn / poison flat tick ───────────────────────

    /// Burn ticks flat `floor(maxHP/16).max(1)` after the mon acts; state +
    /// consumed (no residual byte) asserted vs legacy `apply_all_residual`.
    #[test]
    fn burn_tick_parity() {
        let mut s = base("burn flat tick");
        s.player.status = S::Burn; // maxHP 5000 → 5000/16 = 312
        run_scenario_residual(&s);
        // Direct pin: the burned player lost exactly 5000/16 (both miss → no move dmg).
        let (stack, _c, _f, _e) = stack_run_residual(&s);
        assert_eq!(stack.player_battlers[0].hp, 5000 - 5000 / 16, "burn = floor(maxHP/16)");
    }

    /// Poison ticks identically to burn (same flat `/16`). Parity + consumed.
    #[test]
    fn poison_tick_parity() {
        let mut s = base("poison flat tick");
        s.enemy.status = S::Poison;
        run_scenario_residual(&s);
        let (stack, _c, _f, _e) = stack_run_residual(&s);
        assert_eq!(stack.opponent_battlers[0].hp, 5000 - 5000 / 16, "poison = floor(maxHP/16)");
    }

    /// Burn min-1: maxHP=10 → floor(10/16)=0 → clamped to 1. Parity vs legacy.
    #[test]
    fn burn_min_one_parity() {
        let mut s = base("burn min 1");
        s.player = ResidualMon::clean(10, 200);
        s.player.status = S::Burn;
        // enemy must not KO the player; both miss → only the 1-hp burn lands.
        run_scenario_residual(&s);
        let (stack, _c, _f, _e) = stack_run_residual(&s);
        assert_eq!(stack.player_battlers[0].hp, 9, "burn min-1 on a 10-hp mon");
    }

    // ───────────────────────── Toxic ramp (bug #6) ───────────────────────────

    /// Toxic multi-turn ramp: counter 1,2,3 over 3 turns, damage 312,624,936
    /// (base 5000/16=312, ×counter). The ramp + counter are DIFFED vs the legacy
    /// `toxic_counter` each turn; consumed() unchanged (no residual byte).
    #[test]
    fn toxic_multi_turn_ramp_parity() {
        let mut s = base("toxic 3-turn ramp");
        s.player.badly_poisoned = true;
        s.player.toxic_counter = 0;
        s.player.status = S::None; // badly-poisoned flag set directly (ramp arm)
        s.turns = 3;
        run_scenario_residual(&s); // asserts hp + counter (both sides) + consumed

        let (stack, _c, _f, effects) = stack_run_residual(&s);
        // counter ramped to 3.
        assert_eq!(stack_toxic_counter(&effects, BattlerRef::PLAYER), Some(3), "counter 1,2,3");
        // cumulative toxic damage = 312 + 624 + 936 = 1872.
        let base = 5000u16 / 16;
        let cumulative = base + base * 2 + base * 3;
        assert_eq!(
            stack.player_battlers[0].hp,
            5000 - cumulative,
            "toxic ramp cumulative = base*(1+2+3)"
        );
    }

    /// Toxic with a NON-zero initial counter ramps from there (uncapped multiply
    /// over several ticks): start counter 3 → ticks 4,5,6. Diffed vs legacy.
    #[test]
    fn toxic_uncapped_multiplier_parity() {
        let mut s = base("toxic uncapped from counter 3");
        s.player.badly_poisoned = true;
        s.player.toxic_counter = 3;
        s.player.status = S::None;
        s.turns = 3;
        run_scenario_residual(&s);

        let (stack, _c, _f, effects) = stack_run_residual(&s);
        assert_eq!(stack_toxic_counter(&effects, BattlerRef::PLAYER), Some(6), "3 → 4,5,6");
        let base = 5000u16 / 16;
        let cumulative = base * 4 + base * 5 + base * 6;
        assert_eq!(stack.player_battlers[0].hp, 5000 - cumulative, "uncapped base*(4+5+6)");
    }

    /// Toxic ramp with a Poison status (the FIXED oracle, D7): a mon that is
    /// BOTH Poison-status AND badly-poisoned takes the toxic RAMP — the original
    /// checks the BADLY_POISONED bit inside the shared HP-decrease routine
    /// (core.asm:577-589), so the flat `/16` arm never applies. (The pre-fix
    /// oracle matched its flat Burn|Poison arm first, leaving the ramp dead —
    /// that was the bug.) Exactly ONE chip lands per turn: the flat Poison
    /// status residual skips while the Toxic volatile is live. Parity vs legacy.
    #[test]
    fn toxic_ramps_despite_poison_status_parity() {
        let mut s = base("toxic ramps despite poison status");
        s.player.status = S::Poison; // a badly-poisoned mon's status IS Poison
        s.player.badly_poisoned = true;
        s.player.toxic_counter = 0;
        s.turns = 2;
        run_scenario_residual(&s);

        let (stack, _c, _f, effects) = stack_run_residual(&s);
        // counter ramps 1,2 — damage base*1 + base*2, NOT flat 2×base.
        assert_eq!(
            stack_toxic_counter(&effects, BattlerRef::PLAYER),
            Some(2),
            "toxic ramps even with a Poison status (D7)"
        );
        let base = 5000u16 / 16;
        assert_eq!(stack.player_battlers[0].hp, 5000 - (base + base * 2), "ramped, not flat");
    }

    // ───────────────────────── Leech Seed (cross-battler) ────────────────────

    /// Leech Seed drain+heal: the seeded host loses `floor(maxHP/16).max(1)` and
    /// its OPPONENT heals by the same — BOTH battlers' hp change, asserted, via the
    /// load-bearing `pair_mut`. Parity vs legacy + consumed unchanged.
    #[test]
    fn leech_seed_drain_and_heal_parity() {
        let mut s = base("leech drain+heal");
        s.player.seeded = true;
        // Damage the enemy first so the heal is observable (not capped at max).
        s.enemy = ResidualMon::clean(5000, 50);
        s.enemy.hp = 3000; // room to heal
        run_scenario_residual(&s);

        let (stack, _c, _f, _e) = stack_run_residual(&s);
        let drain = 5000u16 / 16;
        assert_eq!(stack.player_battlers[0].hp, 5000 - drain, "host drained floor(maxHP/16)");
        // enemy healed by the drained amount (3000 + drain), capped at max 5000.
        assert_eq!(stack.opponent_battlers[0].hp, 3000 + drain, "opponent healed by drain");
    }

    /// Leech heal capped at the opponent's max_hp: a near-full opponent heals only
    /// up to max. Parity vs legacy (`(hp+drain).min(max_hp)`).
    #[test]
    fn leech_heal_capped_at_max_parity() {
        let mut s = base("leech heal capped");
        s.player.seeded = true;
        s.enemy = ResidualMon::clean(5000, 50);
        s.enemy.hp = 4999; // drain 312 → would overflow; cap at 5000
        run_scenario_residual(&s);
        let (stack, _c, _f, _e) = stack_run_residual(&s);
        assert_eq!(stack.opponent_battlers[0].hp, 5000, "heal capped at opponent max_hp");
    }

    // ───────────────────── status + leech combined (ASM order) ───────────────

    /// Status damage FIRST, then leech (the ASM order, `residual.rs:84`): a burned
    /// AND seeded mon ticks burn `/16` then leech-drains `/16` (host loses both,
    /// opponent heals the drained amount). Parity vs legacy + the order is the only
    /// thing that makes the numbers line up.
    #[test]
    fn status_and_leech_combined_asm_order_parity() {
        let mut s = base("burn + leech, ASM order");
        s.player.status = S::Burn;
        s.player.seeded = true;
        s.enemy = ResidualMon::clean(5000, 50);
        s.enemy.hp = 3000;
        run_scenario_residual(&s);

        let (stack, _c, _f, _e) = stack_run_residual(&s);
        let chunk = 5000u16 / 16;
        // host: burn (status, first) + leech drain (second) = 2 chunks.
        assert_eq!(stack.player_battlers[0].hp, 5000 - chunk * 2, "burn then leech, both /16");
        // opponent healed by the leech drain only.
        assert_eq!(stack.opponent_battlers[0].hp, 3000 + chunk, "opponent healed by leech drain");
    }

    /// Toxic + leech on the SAME mon (status-damage then leech): toxic ramps and
    /// fires (arena id 140) BEFORE leech (arena id 150) — and the Gen-1 Toxic ×
    /// Leech Seed bug (D11) makes the LEECH drain scale with the ramping counter
    /// (the counter increments a SECOND time per turn on the leech tick). Parity
    /// vs legacy.
    #[test]
    fn toxic_and_leech_combined_order_parity() {
        let mut s = base("toxic + leech, arena order");
        s.player.badly_poisoned = true;
        s.player.toxic_counter = 0;
        s.player.status = S::None;
        s.player.seeded = true;
        s.enemy = ResidualMon::clean(5000, 50);
        s.enemy.hp = 3000;
        s.turns = 2;
        run_scenario_residual(&s);

        let (stack, _c, _f, effects) = stack_run_residual(&s);
        assert_eq!(stack_toxic_counter(&effects, BattlerRef::PLAYER), Some(4), "counter ++ twice per turn");
        let base = 5000u16 / 16;
        // turn1: toxic base*1 + leech base*2 ; turn2: toxic base*3 + leech base*4.
        let host_loss = (base * 1 + base * 2) + (base * 3 + base * 4);
        assert_eq!(stack.player_battlers[0].hp, 5000 - host_loss, "toxic ramp + SCALED leech (D11)");
        // opponent healed by the SCALED leech drain each turn (base*2 + base*4).
        assert_eq!(stack.opponent_battlers[0].hp, 3000 + base * 6, "opponent healed by scaled drains");
    }

    // ───────────────── residual KO cancels the second move ───────────────────

    /// A residual that KOs the FIRST mover cancels the second move (StackDriver §2
    /// step 2d, already built in slice 1 — re-proven under a RESIDUAL KO). The
    /// first mover is at exactly its poison tick of hp → poison KOs it after it
    /// acts → the second mover never acts → it draws NO bytes. `consumed()` reflects
    /// the cancel, asserted directly AND at parity with the legacy oracle.
    #[test]
    fn residual_ko_of_first_mover_cancels_second_move() {
        // Player faster (first), poisoned, at hp == its poison tick → poison KOs it.
        let mut s = base("residual KO cancels second move");
        s.player = ResidualMon::clean(160, 200); // tick = 160/16 = 10
        s.player.status = S::Poison;
        s.player.hp = 10; // exactly one poison tick → faints after acting
        s.enemy = ResidualMon::clean(5000, 50);
        s.first = hit(); // player's move lands (then poison KOs the player)
        s.second = hit();
        s.turns = 1;

        let legacy = legacy_run_residual(&s);
        let (stack, consumed, _first, _e) = stack_run_residual(&s);

        // Player fainted from its own poison residual (both paths).
        assert_eq!(legacy.player.active_mon().hp, 0, "legacy: poison KO'd the first mover");
        assert_eq!(stack.player_battlers[0].hp, 0, "stack: poison KO'd the first mover");
        // The enemy (second mover) NEVER acted → it dealt no damage to the player
        // (player is at 0 from poison, not from an enemy hit) AND it took only the
        // first mover's single hit, with NO retaliation.
        // consumed: ONLY the first mover's crit+accuracy+damage = 3 (no order tie,
        // player faster). The canceled second mover draws 0.
        assert_eq!(consumed, 3, "second move canceled by first-mover residual KO → 3 bytes");
        // State parity: enemy hp identical in both paths.
        assert_eq!(
            legacy.enemy.active_mon().hp,
            stack.opponent_battlers[0].hp,
            "enemy hp parity after the canceled second move"
        );
    }

    /// Boundary pin: a residual tick that EXACTLY reaches 0 (faint) vs leaves 1.
    /// At hp == tick → faints; at hp == tick+1 → survives at 1. Both at parity.
    #[test]
    fn residual_exact_faint_vs_survive_boundary() {
        let tick = 160u16 / 16; // 10

        // hp == tick → poison faints the mon exactly.
        let mut faint = base("residual exact-faint boundary");
        faint.player = ResidualMon::clean(160, 200);
        faint.player.status = S::Poison;
        faint.player.hp = tick;
        faint.enemy = ResidualMon::clean(5000, 50);
        faint.first = miss();
        faint.second = miss();
        let legacy_f = legacy_run_residual(&faint);
        let (stack_f, _c, _f, _e) = stack_run_residual(&faint);
        assert_eq!(legacy_f.player.active_mon().hp, 0, "legacy: hp==tick faints");
        assert_eq!(stack_f.player_battlers[0].hp, 0, "stack: hp==tick faints");

        // hp == tick+1 → survives at exactly 1.
        let mut survive = base("residual survive-at-1 boundary");
        survive.player = ResidualMon::clean(160, 200);
        survive.player.status = S::Poison;
        survive.player.hp = tick + 1;
        survive.enemy = ResidualMon::clean(5000, 50);
        survive.first = miss();
        survive.second = miss();
        // survives → both movers act → run_scenario_residual's faint-free path holds.
        run_scenario_residual(&survive);
        let (stack_s, _c2, _f2, _e2) = stack_run_residual(&survive);
        assert_eq!(stack_s.player_battlers[0].hp, 1, "hp==tick+1 survives at 1");
    }

    // ─────────────────────────── matrix + fuzz ───────────────────────────────

    /// The slice-5 matrix through `run_scenario_residual` (state + toxic ramp +
    /// consumed): burn, poison, toxic ramp, toxic-from-counter, leech, status+leech,
    /// toxic+leech, both-sides-seeded.
    #[test]
    fn slice5_matrix_state_counter_consumed() {
        let mut matrix: Vec<ResidualScenario> = Vec::new();

        matrix.push(base("control: clean"));

        let mut burn = base("burn");
        burn.player.status = S::Burn;
        matrix.push(burn);

        let mut psn = base("poison");
        psn.enemy.status = S::Poison;
        matrix.push(psn);

        let mut tox = base("toxic ramp 4t");
        tox.player.badly_poisoned = true;
        tox.turns = 4;
        matrix.push(tox);

        let mut tox2 = base("toxic from counter 2");
        tox2.enemy.badly_poisoned = true;
        tox2.enemy.toxic_counter = 2;
        tox2.turns = 3;
        matrix.push(tox2);

        let mut leech = base("leech");
        leech.player.seeded = true;
        leech.enemy.hp = 3000;
        matrix.push(leech);

        let mut combo = base("burn + leech");
        combo.player.status = S::Burn;
        combo.player.seeded = true;
        combo.enemy.hp = 3000;
        matrix.push(combo);

        let mut tox_leech = base("toxic + leech");
        tox_leech.player.badly_poisoned = true;
        tox_leech.player.seeded = true;
        tox_leech.enemy.hp = 3000;
        tox_leech.turns = 2;
        matrix.push(tox_leech);

        let mut both_seed = base("both sides seeded");
        both_seed.player.seeded = true;
        both_seed.enemy.seeded = true;
        both_seed.player.hp = 4000;
        both_seed.enemy.hp = 4000;
        matrix.push(both_seed);

        for s in &matrix {
            run_scenario_residual(s);
        }
    }

    /// Determinism fuzz over >= 1000 seeds randomly applying burn/poison/toxic/leech
    /// to either side, with random move bytes + speeds → both paths → identical
    /// final `BattleState` + toxic-counter ramp + `consumed()` (residuals add NO
    /// bytes). Self-contained LCG (no `rand`). HP huge so no faint within the few
    /// turns (keeps the faint-free consumed() claim true).
    #[test]
    fn slice5_determinism_fuzz_1000_seeds() {
        let mut lcg: u64 = 0xD1CE_5EED_F00D_BA11;
        let mut next = || {
            lcg = lcg
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (lcg >> 33) as u8
        };

        for _seed in 0..1000u32 {
            // Pick a residual config per side. Any status/badly_poisoned combo is
            // valid input — the (fixed) oracle ramps whenever the BADLY_POISONED
            // flag is set, and we diff vs it. Status is
            // None/Burn/Poison only (no gating status, so per-turn byte count is
            // constant → the faint-free consumed() oracle holds).
            let pick_status = |n: u8| match n % 4 {
                0 => S::Burn,
                1 => S::Poison,
                _ => S::None,
            };
            // max_hp 60000 → base /16 = 3750. To stay FAINT-FREE (the consumed()
            // oracle requires it) across up to 2 turns with the toxic ramp, cap the
            // start counter at 0..=1. With the Gen-1 Toxic × Leech Seed bug (D11)
            // reproduced, a badly-poisoned + seeded mon's counter increments TWICE
            // per turn — worst case from counter 1: toxic base*2 + leech base*3 on
            // turn 1, toxic base*4 + leech base*5 on turn 2 = 14*base = 52,500.
            // hp 59,000 keeps every side alive. Move damage (40-power) is
            // negligible at this hp.
            let mut p = ResidualMon::clean(60000, 60 + (next() as u16 % 120));
            p.status = pick_status(next());
            p.badly_poisoned = next() % 3 == 0;
            p.toxic_counter = next() % 2; // 0..=1
            p.seeded = next() % 3 == 0;

            let mut e = ResidualMon::clean(60000, 60 + (next() as u16 % 120));
            e.status = pick_status(next());
            e.badly_poisoned = next() % 3 == 0;
            e.toxic_counter = next() % 2; // 0..=1
            e.seeded = next() % 3 == 0;
            // Heal room so leech heals are observable (not always capped); max_hp
            // stays 60000 so the /16 base is unchanged.
            e.hp = 59000;
            p.hp = 59000;

            let s = ResidualScenario {
                name: "slice5-fuzz",
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
                first: crate::battle::stack_parity::MoveBytes {
                    confusion: 255,
                    paralysis: 255,
                    crit: next(),
                    accuracy: next(),
                    damage: next(),
                },
                second: crate::battle::stack_parity::MoveBytes {
                    confusion: 255,
                    paralysis: 255,
                    crit: next(),
                    accuracy: next(),
                    damage: next(),
                },
                turns: 1 + (next() as u32 % 2), // 1..=2 turns (faint-free ramp)
            };
            run_scenario_residual(&s);
            // Cross-check the seeded volatile presence is mirrored (both paths agree
            // a seeded mon stays seeded — leech does not consume itself in Gen-1).
            let (_st, _c, _fm, eff) = stack_run_residual(&s);
            if s.player.seeded {
                assert!(stack_seeded(&eff, BattlerRef::PLAYER), "player stays seeded");
            }
            if s.enemy.seeded {
                assert!(stack_seeded(&eff, BattlerRef::OPPONENT), "enemy stays seeded");
            }
        }
    }
}

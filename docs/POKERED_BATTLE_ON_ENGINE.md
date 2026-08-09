# pokered on the battle engine — a case study

How the **pokered** example game maps Gen-1 Pokémon battles onto the generic
`jrpg-engine` effect-stack. This is the concrete companion to the
[Battle Engine Developer Guide](./BATTLE_ENGINE_GUIDE.md): the guide teaches the
*engine*; this shows a real game *using* it, with file pointers.

> **Status (read first).** The engine + the whole pokered move set are migrated and
> **differentially proven** on the stack — but the migration provider currently
> lives **test-side** (`#![cfg(test)]`) as a side-by-side parity proof against the
> legacy `apply_move_effect` / `execute_turn` oracle, which stays the shipping
> dispatcher. Wiring the stack into the live battle loop is the remaining, human-
> gated step; see [`engine-gap-analysis/17-p6-production-flip-plan.md`](./engine-gap-analysis/17-p6-production-flip-plan.md).
> So: the *mapping below is real and green*; it is not yet the production path.

All paths are under `crates/pokered-core/src/battle/`.

---

## 1. The provider — `PokeredRules`

A game tells the engine about itself by implementing three traits on one type,
`PokeredRules` (in [`pokered_rules/mod.rs`](../crates/pokered-core/src/battle/pokered_rules/mod.rs)):

| Trait | What pokered binds | Notes |
|---|---|---|
| `BattleProvider` | `Move=MoveId`, `Status=StatusCondition`, `Stat=StatIndex`, `Species`, `Type=PokemonType` … + `calculate_damage` | `calculate_damage` stays the **single damage authority** — the real Gen-1 formula (STAB + chart + ×roll/255) in `battle::damage`. The stack's `ModifyDamage` hook (`pokered_damage`) just calls it and writes `ctx.mv.damage`. |
| `EffectProvider` | `EffectStateKind=PokeVolatile`; `effect_for_move` / `effect_for_status` / `effect_for_volatile`; `turn_order_rank`; `forced_action` | `PokeVolatile` is the typed arena state (FocusEnergy, Substitute, LeechSeed, Toxic{counter}, Confused{turns}, Disable{slot,turns}, the lock-in markers …). `turn_order_rank` re-homes the priority table + paralysis ÷4 speed. `forced_action` re-issues a locked move (Thrash/Fly/Hyper-Beam recharge/Bide). |
| `RulesProvider` | `Bindings=PokeredBindings`; `compiled()` / `bindings()` | The bridge to the `jrpg-rules` RON loader (§3). |

The engine learns **no Pokémon concepts** from this — only a table of hooks and a
damage function. `PokeVolatile` is opaque to it.

---

## 2. The four move tiers (how each kind of move is expressed)

The 165 moves split by *how much* is data vs. native Rust
(blueprint [`15`](./engine-gap-analysis/15-pokered-migration-blueprint.md)):

1. **Pure RON data** (~48) — pure damage, self-Boost, Recover, Drain, Recoil, Splash.
   A `rules.ron` record with closed-`Op` hooks; nothing pokered-specific in Rust.
2. **RON shell + reusable primitive** (~98) — side-status, foe stat-down, multi-hit,
   special/fixed damage. The declarative skeleton is RON; the rider uses a generic
   primitive (`HasVolatile`, `MoveTypeIsDefenderType`, `RepeatHits`, `SetDamage`,
   the nested-veto `TryBoost` driver).
3. **Native `&'static Effect`** (~14) — turn-spanning / cross-effect state
   (Bide/Charge/Fly/Trapping/Thrash/HyperBeam, Substitute/LeechSeed/Rage, field
   flags, Haze, Disable, Counter). Hand-written handlers + an arena slot.
4. **Native data-reach** (~5, kept native by decision, *not* script) —
   Transform / Metronome / Mimic / Mirror Move / Conversion: arbitrary logic over
   the species / move table.

The native handlers live in
[`pokered_rules/p5_native.rs`](../crates/pokered-core/src/battle/pokered_rules/p5_native.rs).

---

## 3. No-code moves — `rules.ron`

[`pokered_rules/rules.ron`](../crates/pokered-core/src/battle/pokered_rules/rules.ron)
authors the data-tier moves as `EffectRecord`s whose hooks carry a list of closed
primitive `Op`s (`DealMoveDamage`, `Boost`, `HealFraction`, `InflictStatus`,
`VetoIf`, `ApplyTypeChart`, `RepeatHits`, `SetDamage`, …) plus a `chance:[n,d]`
secondary gate. The `jrpg-rules` loader parses it (baked via `include_str!` for
release; hot-reload from disk under the `hot-reload` feature) and registers each
record as a runtime `Effect` that calls `interpret()` keyed by `EffectId` — **zero
engine change**. `PokeredBindings` resolves the game-specific lookups the ops need
(stat indices, type chart → `(1,1)` because the chart already rode the damage
authority). See guide [§5](./BATTLE_ENGINE_GUIDE.md#5-no-code-authoring-with-rulesron-the-jrpg-rules-loader).

---

## 4. Status residual & pre-move gates (Option A)

The non-damage Gen-1 status mechanics, all on the stack at legacy parity:

- **Residual chips** — `effect_for_status(Burn|Poison)` returns a `Residual`-hooked
  effect chipping `(max/16).max(1)`; `effect_for_volatile(Toxic|LeechSeed)` ticks the
  ramp / drain. The driver's per-mover residual fires status **then** volatiles in
  arena order. Poison's flat chip **skips** when a `Toxic` volatile is live (one chip,
  not two). (`p5_native.rs` `burn_residual` / `poison_residual` / `toxic_residual` /
  `leech_residual`.)
- **Pre-move gates** — `sleep_gate` (#8 wake-loses-turn), `freeze_gate` (#10), the
  `confusion_gate` (50% typeless 40-power self-hit), `paralysis_gate` (25% full-para).
  Attached as `BeforeMove` hooks on every move effect at orders `10/20/70/90` — the
  ASM `MoveRandoms` draw order (confusion byte before paralysis, both before crit);
  `run_event` short-circuits on the first `Fail`. Each is inert (no rng) when the
  mover has no status, so non-status turns are byte-identical.

Every one preserves its deliberate Gen-1 bug (the parity tests encode the numbers).

---

## 5. Turn narration — the `TurnLog` → battle text

When the production loop is routed through the stack, it will call
`StackDriver::execute_turn_logged` and translate the returned `TurnLog` (guide
[§2.11](./BATTLE_ENGINE_GUIDE.md#211-narrating-a-turn--the-turnlog)). pokered's
translator (`pokered_rules/tests.rs`) reproduces the production per-turn text
**exactly**:

- `move_announcement(...)` → `format_move_outcome`'s block (used / Critical hit! /
  effectiveness / missed / cannot-move). **Effectiveness is re-derived game-side**
  (`effectiveness_category`, via the type chart) — the engine log carries the damage
  amount, not the category.
- `translate_turn(log, state)` walks the whole log → the production lines: each
  `MoveUsed` → the announcement; `Blocked` → the cannot-move reason (from the mover's
  status, matching `format_cannot_move`); `Fainted` → "{name} fainted!". Names are
  UPPERCASE species, "Enemy "-prefixed on side 1. `Damaged`/`Healed`/`Status`/
  `StatChanged` carry no text in this reimpl (HP-bar/flags handle them).

Proven end-to-end on real `TurnLog`s (`translate_turn_*` tests).

---

## 6. How it's proven — the differential harness

The migration's correctness rests on a **differential parity** harness
(`pokered_rules/tests.rs`, `p5_tests.rs`): for each scenario it runs the SAME setup
through (a) the legacy oracle (`execute_turn` / `apply_move_effect` — the shipping
code) and (b) the stack (`StackDriver` on `PokeredRules`), and asserts an
**identical `BattleState`** (hp + status + every stat stage, both sides) **and an
identical `rng.consumed()`** (byte count + draw order). `build_stream` predicts the
stack's lazy draw order so a drift fails loudly. Result: **pokered-core 1689/0,
jrpg-engine 328/0** (the engine stays game-agnostic — no `rand`, no `if gen==1`, no
Pokémon types).

This is why the mapping is trustworthy even though it isn't the production path yet:
the stack provably does what the legacy code does, move for move, byte for byte.

---

## 7. What's left

Routing the live `BattleScreen` loop through the stack — productionizing the
provider, the `BattleState ↔ EngineState` adapter, the RNG shim, guarded routing,
then playtest + the irreversible cut. Specified step-by-step in
[`engine-gap-analysis/17-p6-production-flip-plan.md`](./engine-gap-analysis/17-p6-production-flip-plan.md).

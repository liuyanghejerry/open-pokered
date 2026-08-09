# Fidelity Gaps & Completion Plan

Audit of this Rust reimplementation vs. the Gen-1 original (2026-07-14), plus the
agreed phased plan to close the gaps. **This file is the resume point** — if the
work is picked up in a fresh session, read this first.

## The dominant pattern (read first)

`pokered-core` has an **extensive, well-tested pure-logic layer** for almost
every system. The gap is overwhelmingly at the **app glue layer**
(`crates/pokered-app/src/game.rs`): the logic exists and is
unit-tested but is **not called from real play**. Most "missing" features are one
glue hookup away, not a from-scratch build. Exceptions needing genuinely new work:
real link networking, and the end-game / cutscene screens.

Path shorthand: **CORE** = `crates/pokered-core/src` ·
**DATA** = `crates/pokered-data/src` ·
**APP** = `crates/pokered-app/src` ·
**RENDER** = `crates/pokered-renderer/src` /
`crates/jrpg-renderer/src`.

Fidelity policy: reproduce Gen-1 behaviour **including deliberate bugs**. Verify
with `cargo test -p <crate>` (per-crate; a workspace run can false-fail on feature
unification) and the debug-server harness (`--features debug-server --debug-port`;
`get_state` now exposes `battle_message`/`coins`/`money`).

---

## Agreed plan & ordering (user, 2026-07-14)

1. **Phase 1** — core-loop critical gaps **+ battle system** (do first).
2. **Phase 2** — the "logic exists but unwired" systems.
3. **Phase 3** — graphics gaps **#3, #4, #5, #6 only** (font gap #1 and menu-border
   gap #2 are **explicitly deferred / out of scope**).

Work through phases in order; commit incrementally; keep the user in the loop
between phases (stop + summarize after Phase 1 before starting Phase 2).

---

## Phase 1 — Core loop + battle system

### 1A. Battle→save integration (highest leverage; unblocks the most)

- **Battle results never persist.** Battle is built from a *clone*:
  `from_parties(true, &player_party, …)` with `player_party = self.save_data.party.to_vec()`
  (APP/game.rs:1186 wild, :1215 trainer). The post-battle settlement consumer
  (APP/game.rs:1027-1093) writes back **only** money + blackout + trainer-defeated.
  → Wire back the mutated battle party (`self.battle.battle_state.player.party`,
  CORE/battle/mod.rs BattlerState.party) into `save_data.party` after every battle:
  EXP, level-ups, learned moves, evolutions, HP, status, PP. Handle fainted mons,
  party reordering, and the `Loss` heal path. `BattleScreen` has no party accessor
  today (only `player_party_size`, mod.rs:524) — add one.
- **Catch adds nothing.** `Captured` only sets `BattleOver{won:true}` + "Caught!"
  (CORE/battle/mod.rs:1607-1613); `BattleOutcome::Captured` is dead. → On catch, add
  the caught wild mon to party (or PC box if full) and set Pokédex owned. The caught
  mon's data (species/level/DVs/HP/status) is the enemy battler.
- **Pokédex never populated in play.** `Pokedex::set_seen`/`set_owned`
  (CORE/pokemon/pokedex.rs:41-70) have **zero non-test callers**. → Set SEEN on
  encounter/battle-start (wild + trainer reveal), OWNED on catch/receive/evolve.
- **Evolution end-to-end** falls out of 1A: `settlement.evolutions`
  (CORE/battle/settlement/settle.rs:72-90) is computed on the clone but never read.
  Writing back the party persists it; also surface the "X evolved into Y!" text +
  learn the evolved form's learnset (`process_evolutions`). Stone/trade evo is Phase 2.

### 1B. Battle-system completeness

Live turn loop = `BattleScreen` → `execute_turn_with_move` (CORE/battle/mod.rs:1773)
→ `StackDriver::execute_turn_logged` with `PokeredRules`. What effects exist is
defined by `record_id_for_move` (CORE/battle/pokered_rules/mod.rs:627-689) +
`rules.ron`. ~70/165 moves wired; the rest fall through to `_ => "move.tackle"`
(pure damage). A **complete** legacy dispatcher `apply_move_effect`
(CORE/battle/effects/mod.rs:125-237) handles all 165 but is retired (do NOT revive
it as the main path — CLAUDE.md; expand the stack engine instead). `p5_native.rs`
handlers exist for many effects but nothing fires their `Event::Custom(EV_*)` live.

- **Wire the ~95 unwired move effects** on the stack engine (rules.ron +
  record_id_for_move + any new Ops), grouped by category: multi-turn charge
  (Fly/Dig/Solar Beam/Sky Attack/Razor Wind/Skull Bash), Hyper Beam recharge,
  binding (Wrap/Bind/Fire Spin/Clamp), Counter, Mirror Move, Metronome, Transform,
  Substitute, Rest, Conversion, Haze, Disable, Mimic, Bide, Rage, Thrash/Petal Dance,
  Explosion/Self-Destruct (self-KO + defence halving; damage.rs hardcodes
  `is_explode_effect:false` mod.rs:1066), Focus Energy, Light Screen/Reflect
  (mod.rs:1065 hardcodes false), Leech Seed, Toxic, Dream Eater, Pay Day (coins),
  Jump Kick crash, and all sleep/confusion/flinch inducers.
- **Enable sleep/confusion/badly-poison(Toxic)/flinch infliction** in the live loop
  (currently only poison/burn/freeze/paralysis are inflictable; no `"sleep"` binding
  mod.rs:517-525).
- **Battle text for secondary effects.** `translate_turn` (CORE/battle/pokered_rules/runtime.rs:146-177)
  emits text only for MoveUsed/Crit/Missed/Blocked/Fainted — no "was poisoned",
  "fell asleep", "rose/fell", "hurt by poison", "regained health", recoil, absorb.
- **Damage-formula fidelity (non-deliberate divergences to fix):**
  - Missing **217/255 minimum-roll floor** — `pokered_damage` draws unconstrained
    `[1,255]` (mod.rs:1027; damage.rs:157-158). Variance too wide.
  - **High-crit multiplier 2× too high** — uses `base_speed*8`, original is
    `(base_speed/2)*8` (damage.rs:35).
  - **Burn doesn't halve Attack** (no burn term in CORE/battle/damage.rs).
  - **Turn order ignores Speed stat STAGES** — `effective_speed` reads base only
    (pokered_rules/mod.rs:1355-1362); Agility/String Shot don't change order.
- **Trainer AI item usage + switching** — fully coded (CORE/battle/trainer_ai/ai_action.rs:12-31/55)
  but **no live caller**; gym leaders/E4 never heal or use X-items or switch.
  Layer-2 AI encouragement disabled by a hardcoded `0` (mod.rs:816).
- **Forced switch (Whirlwind/Roar/Teleport)** logic exists (CORE/battle/escape/mod.rs:124-169)
  but unwired in the stack path.
- Legacy `apply_move_effect` runs live only for the enemy's post-failed-escape attack
  (mod.rs:1753) and post-switch (mod.rs:1956) — an inconsistency to reconcile once
  the stack path is complete.

Deliberate Gen-1 bugs already correct (keep): 1/256 miss, OHKO level-immunity,
Ghost→Psychic 0-dmg, permanent freeze, crit-uses-base-speed, ±6 stat clamp,
Focus-Energy /4 crit (once reachable), catch-rate short-circuits.

**Safari battle mode** (Ball/Bait/Rock/Run, no attacking) is test-only
(`SafariBattleMenuState`) — Safari encounters run as ordinary wild battles. Old-Man
tutorial catch is a dialogue stub. Ghost Marowak has no "?"/uncatchable state.

---

## Phase 2 — Logic exists, not wired

- **Field-move menu** — party submenu is only STATS/SWITCH/CANCEL
  (CORE/party_screen.rs:25-26). Add a field-move (HM) option; this unblocks all of:
- **HM/field moves** (CORE/overworld/hm_effects.rs, all unit-tested, no live callers):
  CUT, SURF (never assigns `TransportMode::Surfing`; `collision on_water` never true),
  STRENGTH (+ `try_push_boulder`), FLY (+ make Town Map's A warp — town_map_screen.rs
  is view-only), FLASH (`DarkCaveState`/`use_flash` never instantiated; caves never
  drawn dark), DIG (move), TELEPORT.
- **TM/HM as bag items** — `ItemId` has no TM01-50/HM01-05 variants (DATA/items.rs:24-111
  ends at MaxElixer=0x53). Add them so `giveItem("TMxx")` works → unblocks Game Corner
  TM vendor + all TM/HM gifts + teaching from the bag. `TmId`/`HmId`/`TM_MOVES`/`HM_MOVES`
  already exist for move-teaching.
- **PC / storage box** — full deposit/withdraw/box/release/PC-menu logic
  (CORE/pokemon/pc_box.rs, pc_menu.rs) with **no screen + no overworld entry** (no
  GameScreen::PC; Bill's PC stubbed maps/BillsHouse/script.scene). Add a PC screen +
  wire the overworld PC object. Add HP/stat recompute on withdraw.
- **Evolution UI** — stone-evolution has no item-use case (CORE/items/use_engine.rs:61-116);
  no evolution animation/screen; no B-cancel. (Level-up evo handled by 1A writeback.)
- **Options honored** — all three rows are now wired end-to-end (2026-08-02):
  `text_speed` drives the dialogue typewriter (1/3/5 frames per char,
  `PrintLetterDelay`; CORE/overworld/screen.rs `BedroomDialogue`), `battle_style`
  gates the SHIFT "Will you change #MON?" prompt in trainer battles
  (`ReplaceFaintedEnemyMon`; CORE/battle/mod.rs `ShiftPrompt`/`ShiftSwitchSelect`),
  and both are written back from the options screen → config → save data in APP
  and TUI. (`battle_animation` wired earlier — see 2026-07-21 progress entry.)
- **Field-item effects** — Repel (counter wired to movement but `repel_steps` never set
  in play), evolution stones, Itemfinder/hidden items (`obtained_hidden_items` bitfield
  only), fishing rods (encounter data exists, no consumer). Also route out-of-battle
  healing/PP-restore/vitamins/Rare Candy (CORE/items/{healing,pp_restore,vitamins}.rs —
  wired only into battle) to a field/party entry point.
- **Key-item TOSS guard** (original refuses "TOO IMPORTANT TO TOSS!").
- **SRAM per-mon fidelity** — the SRAM path drops nicknames/OT/OT-ID/PP-Ups
  (CORE/save/ser_pokemon.rs:65,114-119,192).
- **End-game suite** — Hall-of-Fame recording (`push_team`/`add_mon` test-only) +
  parade screen, credits sequence, diploma, post-game E4 rematch reset
  (`last_blackout_map`/`EVENT_STARTED_ELITE_4` never set in play → League restart broken).
  Continue-screen save-info panel is dead code (CORE/main_menu.rs:81,144-147,197).
- **Safari bait/rock** + Old-Man tutorial + true Ghost reveal (also touches 1B).
- **Cable club / link** — only an in-process mpsc mock (CORE/link/transport.rs); real
  networking is genuinely large / out of single-player scope — defer unless requested.

---

## Phase 3 — Graphics (#3, #4, #5, #6 only; font #1 + borders #2 deferred)

Runtime is faithful 4-shade grayscale. Move animations ARE genuinely ported (203
moves, battle_anim/); overworld + battle HUD + battle intro + Gengar intro are faithful.

- **#3 — HP-bar drain (absent); catch/wobble animations (DONE).** HUD draws HP instantly
  (RENDER render/battle.rs:1674-1690); no display-HP tween/per-pixel bar drain with
  sound (core has no animated display-HP field, CORE/battle/mod.rs:516-522). ~~Catch
  wobble is text-only~~ — **DONE**: the core queues `BattleAnimEvent`s
  (`BattleScreen::take_anim_event`) and both frontends stage the faithful
  TossBallAnimation choreography (toss variant by ball → poof → hide-pic →
  wNumShakes shakes with SFX_TINK → poof → show-pic on breakout), incl.
  ghost dodge ($10), old-man tutorial and Safari balls.
- **#4 — "logic-ported-but-never-rendered" effects.** Wire renderers for: HM05 Flash
  cave-light (caves never drawn dark), elevator shake (`elevator_shake_params`),
  Teleport/Dig spin-out (`teleport_spin_direction`/`TELEPORT_SPIN_ORDER`), and the
  faint white-out (`GBFadeOutToWhite`). The faithful palette-fade module
  `crates/jrpg-renderer/src/transition.rs` is a clean port with no live callers — use it.
- **#5 — missing cutscene screens.** GameFreak "shooting-star" splash, end credits
  roll, Hall-of-Fame roll-call parade, S.S. Anne ship-sail (VermilionDock stub).
- **#6 — overworld tile animation + move special-effects.** Water/flower animated tiles
  (no animated-tile system; `tile_animations` is only a save byte) remain absent. ~~Replace
  the generic gray-particle approximations for Substitute doll / petals / leaves /
  droplets / Transform / Minimize with their specific sprites~~ — **DONE 2026-07-21**
  (shared `jrpg-renderer/src/battle_anim/effects.rs`; see progress log).

Comparison harness: `tools/compare.py` pixel-diffs vs PyBoy but needs a user ROM, has
no baseline/CI gate (manual). `visual-verify` skill covers only the heal machine.

---

## Progress log

- 2026-07-14: audit complete; plan agreed. Prior PR #86 already shipped Day Care,
  Game Corner prizes/coins, and per-trainer EndBattleText.
- 2026-07-14: **Phase 1A DONE.** Battle→save writeback (the battle ran on a party
  clone; now `battle_state.player.party` is written back to `save_data.party`
  after every battle — EXP/level-ups/learned-moves/evolutions/HP/status/PP all
  persist; **live-verified** exp 156250→156535 stuck after a trainer battle).
  Catch: added `BattleScreen.captured_mon`, set on a successful throw; the app
  adds it to the party (or the current PC box if full) + Pokédex owned
  (unit-tested end-to-end via a Master Ball). Fixed a second pre-existing bug:
  `battle.player_bag` was **never populated** from the save (empty in-battle bag
  → no items/balls usable) — now copied in + synced back. Pokédex seen on
  wild/trainer encounter; owned on catch/gift/trade. Debug: added
  `StartWildBattle` command + `battle_message`/`battle_phase`/`experience`/
  `coins`/`money` to `get_state`. Note: the `run_frames` debug path throttles the
  battle intro animation, so battle-menu automation is impractical live — the
  catch flow is covered by `battle/catch_tests.rs` instead. TUI battle path still
  discards results (separate pre-existing gap; native app is primary).
- 2026-07-14: **Phase 1B batches 1–3 DONE** (low-risk, all tested; a 5-agent
  workflow first mapped the effect-stack engine + produced the batch plan).
  - Batch 1 — damage/turn-order fidelity: 217/255 damage-roll floor, high-crit
    ratio `(base_speed/2)*8`, burn halves physical Attack (`DamageParams.attacker_burned`),
    Speed stat-stage in turn order. Backed by the stack_parity + turn_order oracles;
    3 new damage tests.
  - Batch 2 — secondary-effect battle text: `translate_turn` arms for
    StatusInflicted/StatusCured/StatChanged ("was poisoned!", "STAT rose!",
    "woke up!", …). No engine change; 1 new text test.
  - Batch 3 — Group-A move wiring (data-only): Flamethrower/FirePunch→burn,
    IcePunch→freeze, Thunderbolt/Thundershock/Thunderpunch→para, Lick→para(30%),
    StunSpore/Glare→paralyze, PoisonGas→poison. High-crit moves confirmed to need
    no wiring.
  - **DEFERRED (need engine-crate work / higher risk — paused for review):**
    Batch 4 (new `jrpg-rules` ops StatusWithDuration/InflictVolatile/AwardResource
    + sleep binding + flinch gate → unlocks sleep/confusion/Toxic/flinch infliction
    and ~20 more move riders + Rest/DreamEater/LeechSeed/PayDay), Batch 5 (Tier-2
    residual/recoil/drain/volatile text — needs HP-change cause tagging in
    jrpg-engine), Batch 6 (complex multi-turn: charge Fly/Dig, binding, Counter,
    Bide, Substitute, screens, Explosion self-KO, Transform/Metronome/Mirror-Move),
    Batch 7 (trainer AI item-use/switch — unverified `ai_action` seam). Full plan
    in the workflow output; batches are ordered by risk with per-batch test notes.
- 2026-07-15: **Batch 4 + a Batch-6 subset DONE on `feat/battle-effects`** (PR #87).
  Built the decoupling seam the user asked for and rode it for a whole class of
  moves — the engine / `jrpg-rules` stay Pokémon-unaware; only STRING vocabulary
  crosses. The seam: `Op::InflictStatus`/`InflictVolatile{kind:String, amount:
  AmountSpec}` + predicates `Not`/`TargetHasAnyStatus`; `RuleBindings` gained
  defaulted, RNG-free `set_status_with_amount` / `make_volatile` / `has_any_status`;
  the engine draws all entropy (AmountSpec → resolved u16) and hands the pure
  binding the number, the game assigns meaning. `install_effect` on the generic
  ctx pushes the opaque `P::EffectStateKind` (= `PokeVolatile`). Moves now live on
  the stack, each as a `rules.ron` record + a `record_id_for_move` arm:
  - **Sleep** (Hypnosis/Sing/SleepPowder/LovelyKiss/Spore) — RngRange(1,7) duration.
  - **Confusion** (Confuse Ray/Supersonic; Psybeam/Confusion/Dizzy Punch riders) —
    RngMask(3,+2) turns; BeforeMove confusion gate.
  - **Flinch** (Bite/Bone Club/Hyper Fang 10%; Stomp/Rolling Kick/Headbutt/Low Kick
    ~30%) — new `flinch_gate` (Fail → skip move), RNG-free.
  - **Leech Seed**, **Toxic** (VetoIf TargetHasAnyStatus + poison + Toxic ramp
    volatile), **Rest** (RemoveStatus + 2-turn sleep + full heal).
  - **Focus Energy** (self-volatile; pokered_crit reads it incl. the Gen-1 /4 crit
    BUG), **Explosion/Self-Destruct** (SetHp(Source,0) self-KO + defence halving).
  - **Light Screen / Reflect** (self-volatile; pokered_damage doubles the defender's
    relevant defence), **Mist** (self-volatile; consumer was already wired via
    `effect_for_volatile`'s TryBoost veto vs `bridge_foe_stat_down`).
  Also fixed a latent infinite loop in the merged Batch-1 damage-roll (bounded the
  217-floor retry so an exhausted ScriptedRng can't spin). ~13 decoupled-stack
  integration tests added; full jrpg-rules + jrpg-engine + pokered-core suites green.
  - **STILL DEFERRED — need NEW engine seams (bigger design calls, flag for review):**
    - **Dream Eater** — a *pre-damage* veto (deal 0 to a non-sleeping target); the
      current seam only rides *post-*DamagingHit.
    - **Batch 6 forced-action moves** — charge (Fly/Dig/SolarBeam/Sky Attack/Razor
      Wind/Skull Bash), lock-in (Thrash/Petal Dance/Rage), binding (Wrap/Bind/Fire
      Spin/Clamp), **Bide**, **Counter** (reactive 2× physical), **Disable**
      (move-lock), **Transform/Metronome/Mirror Move** (copy/replay), **Haze**
      (clear both sides' stages+volatiles), **Substitute** (setter volatile exists;
      HP-proxy consumer not wired). These want a "commit an action for a later turn"
      / "react to damage taken" engine seam — deliberately not bolted onto the
      post-hit record path.
    - **Batch 7 trainer AI** item-use/switching (dead `ai_action` path).
- 2026-07-15: **Counter DONE on the live path** (the reactive seam, user-picked
  "Counter first"). Promoted from the test-only `stack_slice6`/`stack_parity` POC:
  new opaque `PokeVolatile::DamageTaken{amount, counterable}` (per-turn scratch,
  no legacy backing → auto-dropped each turn, no reset); native `record_damage_taken`
  on every record's DamagingHit stamps the defender; `counter_handler` on the
  `special.counter` record reflects 2× via the load-bearing `pair_mut`. Faithful
  Gen-1: `counterable` = NORMAL/FIGHTING type only (Earthquake/Rock are physical but
  NOT countered — a fidelity gain over the POC's `is_physical`). -1 priority was
  already wired. 3 live tests; engine + jrpg-rules untouched. Deviation logged: models
  per-turn Counter, not the cross-turn persistent-variable bug.
  - **Forced-action lock-in family is a confirmed LIVE GAP** (next up): Bide, Thrash/
    Petal Dance, charge (Fly/Dig/Solar Beam/Sky Attack/Razor Wind/Skull Bash), Hyper
    Beam recharge, binding (Wrap/Bind/Fire Spin/Clamp) are UNIMPLEMENTED in the live
    stack path — only the test-only POC drives them. The legacy state already carries
    the backing (`status2::STORING_ENERGY`/`USING_TRAPPING_MOVE`, `num_attacks_left`,
    `bide_accumulated_damage`) and the engine's generic `forced_action` seam + the POC
    handlers exist. Promoting needs a live-loop change: `execute_turn_with_move` must
    resolve the locked/forced move GAME-SIDE (before setting `CURRENT_MOVES`) so the
    native handlers use the forced move's data, plus volatile↔legacy round-trip and the
    residual lifecycle (accumulate/unleash/decrement/clear) as stack handlers. This is
    the first change to the PRODUCTION battle loop (higher risk than the additive work).
- 2026-07-16: **Hyper Beam recharge DONE on the live path** (user-picked "charge +
  recharge first"; recharge is the simplest forced-action move). Established the
  forced-action integration pattern for the whole family: new opaque
  `PokeVolatile::Recharge` + `PokeredRules::forced_action` (Recharge → `Nothing`, so
  the engine skips the move — a forced `Nothing` resolves nothing, sidestepping the
  `CURRENT_MOVES` mismatch); native `hyperbeam_recharge_install` on DamagingHit (arms
  the flag unless the target fainted — the Gen-1 KO-skips-recharge quirk); round-trip
  `Recharge ↔ status2::NEEDS_TO_RECHARGE`; and the live-loop lifecycle in
  `execute_turn_with_move` (snapshot-who-entered-recharging → consume the flag after
  write-back so the mon isn't trapped; "MON must recharge!" narrated game-side since a
  forced Nothing logs no event). 3 ScriptedRng engine tests + round-trip + a real
  3-turn end-to-end lifecycle test through the production loop. engine/jrpg-rules
  untouched. **Next: charge moves (Fly/Dig/Solar Beam)** — reuse this pattern but add
  the charge→strike two-turn state (turn-1 damage veto + `CURRENT_MOVES` reconciliation
  for the forced strike) and semi-invulnerability (opponent accuracy), a bigger chunk.
- 2026-07-16: **Charge moves DONE — the "charge + recharge first" batch is COMPLETE.**
  All six two-turn chargers (Fly, Dig, Solar Beam, Razor Wind, Skull Bash, Sky Attack)
  on the live path via `PokeVolatile::Charging{move, invulnerable}`. The three native
  pipeline handlers branch charge-vs-strike on the volatile's presence: the GATHER
  turn installs it and draws nothing (crit/accuracy skip, 0 damage); the STRIKE turn
  (forced by `forced_action`) removes it and lands the hit. Semi-invulnerability lives
  in `pokered_accuracy` (miss a mid-Fly/Dig target, keyed on the MOVE since Dig shares
  `ChargeEffect`), with the Gen-1 exceptions (Gust/Thunder→Fly, Earthquake/Fissure→Dig);
  Solar Beam & co. charge without invuln. Round-trip `Charging ↔ CHARGING_UP/INVULNERABLE`
  (move recovered from `selected_move`); live-loop `CURRENT_MOVES` reconciliation pins
  the charging move on the strike turn (menu ignored) so the native handlers use its
  data; charge text ("flew up high!"…) narrated game-side. 6 engine + round-trip + a
  real charge→strike lifecycle test (also proves the strike ignores the menu index).
  engine/jrpg-rules untouched. **Remaining forced-action family: Thrash/Petal Dance
  lock-in, Bide, binding (Wrap/Bind/Fire Spin/Clamp)** — all reuse this same pattern
  (`forced_action` + Charging-style volatile + round-trip + residual lifecycle).
- 2026-07-17: **Group A (all forced-action / multi-turn moves) COMPLETE.** On top of
  recharge + charge, added on the live decoupled stack: **Thrash/Petal Dance**
  (`LockedMove` — 2–3 use lock + end-confuse), **Rage** (`Rage` — lock + Attack-up on
  being hit), **Wrap/Bind/Fire Spin/Clamp** (`Trapping` — 2–5 turn lock + the FOE is
  bound via `forced_action → Nothing`), and **Bide** (`Bide` — no-damage store folding
  the per-turn `DamageTaken` scratch, then unleash ×2 via `bide_residual` + `pair_mut`).
  Each is a lock volatile + a `forced_action` arm + a legacy round-trip
  (THRASHING_ABOUT/USING_RAGE/USING_TRAPPING_MOVE/STORING_ENERGY + num_attacks_left +
  bide_accumulated_damage) + the generalized `move_is_locked()` reconciliation. ~10 new
  ScriptedRng engine tests + the Thrash live lifecycle. engine/jrpg-rules untouched.
  **Remaining: group B (copy/replay) — Transform, Mimic, Metronome, Mirror Move.**
  Transform/Mimic are self-contained state copies; **Metronome & Mirror Move need a new
  "resolve an arbitrary move mid-turn" mechanism** the pipeline lacks (the p5_native
  prototypes exist but ride unwired Custom events). Plus odds-and-ends still open:
  Disable, Haze, Conversion, Pay Day, Substitute (create/absorb), Dream Eater sleep-gate,
  Jump Kick crash, Whirlwind/Roar/Teleport (switch/flee).
- 2026-07-17: **Group B (copy/replay) COMPLETE** on branch `feat/battle-copy-moves`
  (a workflow first mapped all 4 moves + the two mechanisms, then a synthesis produced
  the ordered plan — all zero-engine-change, game-side). Implemented:
  **Transform** (opaque one-shot `PokeVolatile::Transformed` + `transform_install` copies
  species/stats/stat-stages/moves onto the engine battler; `write_party` persists it into
  the legacy Pokémon ONCE — species/stats/types/moves/PP=5/status3::TRANSFORMED — avoiding
  the PP-reset trap); **live last-move tracking** (read the driver's MoveUsed events into
  `bs.*.last_move_used` — groundwork); **Mimic** (game-side reconciliation overwrites
  `moves[selected_move_index]`=foe's prior last move, PP=5; matches the oracle's
  last-move copy, not random); **Metronome + Mirror Move** (a pre-driver
  `resolve_called_move` substitutes the picked/mirrored move into BOTH channels — the
  Fight action + `set_current_move` — flattening the "call another move" recursion; nested
  calls bounded; failed Mirror Move → Nothing). Deferred: the optional `resolve_called_move`
  EffectProvider seam (only needed for caller-priority fidelity of a picked Quick Attack /
  Counter). Also **fixed a real pre-existing bug**: a Fly/Dig strike that MISSES left the
  mon stuck charging+invulnerable (pokered_damage doesn't run on an accuracy miss) — new
  `charge_miss_land` OnMiss handler clears it. ~12 new tests; p5 differential parity green;
  engine/jrpg-rules untouched; suite stable across repeated runs.
  **Remaining battle gaps (all now smaller than A/B):** Disable, Haze, Conversion, Pay Day,
  Substitute (create/absorb), Dream Eater sleep-gate, Jump Kick crash, Whirlwind/Roar/
  Teleport (switch/flee), Struggle recoil.
- 2026-07-18: **Misc batch — low-difficulty tier DONE** on `feat/battle-misc-moves` (a
  workflow mapped the 9 remaining moves + synthesized the plan). Shipped, each with tests:
  **Struggle** (recoil.struggle record, 1/2 recoil), **Dream Eater** (dream.eater record;
  damage lands, drain vetoed unless target asleep — matches the oracle, not a pre-damage
  gate), **Jump Kick / Hi Jump Kick** (native `jump_kick_crash` on OnMiss → 1-HP crash),
  **Pay Day** (game-side reconciliation → 2×level into total_payday_money), **Haze**
  (native `haze_reset` on DamagingHit — resets both sides' stages/status + clears
  confusion/LeechSeed/Toxic/FocusEnergy, PRESERVING screens/Mist/Substitute/lock-ins),
  **Whirlwind/Roar/Teleport** (game-side phase override → flee a WILD battle). All
  zero-engine-change; p5_*_parity green. **Remaining (medium/high, per the plan):**
  Conversion (opaque `TypeOverride` volatile + route type derivation through an
  `effective_types` helper + Transform-style round-trip), Disable (round-trip + Residual
  tick + SET reconcile + enemy-AI veto; the `PokeVolatile::Disable` variant already
  exists), Substitute core (create + ModifyDamage absorb + the already-live veto). The
  ONLY deferred item needing a genuine new engine seam: Substitute vs the direct-mutate
  ops (Super Fang / OHKO / multi-hit), which need a defaulted `EffectProvider::redirect_damage`
  in the game-agnostic jrpg-rules interp.
- 2026-07-18: **Substitute phase 1 DONE** on `feat/battle-substitute` (create + formula
  absorb) — the FIRST engine-touching move, but the touch is a single generic line, not a
  Pokémon leak. **Engine (jrpg-engine only):** the driver now FIRES the already-reserved
  `Event::Damage` fold in `resolve_action` (driver.rs, between the effectiveness fold and
  `take_damage`) — the pre-designed "damage-application" seam (its doc literally says
  "Substitute absorb, Disguise, Endure/Sturdy floor-to-1"). ADDITIVE + DEFAULTED: with no
  subscriber the fold returns the relay unchanged (no rng, no state change), so jrpg-engine
  / jrpg-rules / minimon / firered / every pokered stack-parity slice stay **byte-identical**
  (416 + 47 + 39 + full pokered-core suites green). This is the generic `TryDamage`/DamageSink
  primitive the user asked for — any game can now intercept incoming HP loss (shield / decoy /
  redirect) by subscribing a `Damage` handler; the engine stays game-agnostic. **Pokered
  (game-side, zero further engine change):** `substitute_install` (DamagingHit, keyed
  SubstituteEffect) raises the doll costing `max_hp/4` HP with the Gen-1 bug #28 (`hp==cost`
  succeeds → user at 0); `substitute_absorb` (the new `Event::Damage` hook) routes the hit
  into the doll's `SubstituteHp` pool, breaks it at `dmg >= hp` with NO overflow to the mon,
  and returns `Set(Damage(0))`. Round-trip (`SubstituteHp` ↔ `HAS_SUBSTITUTE_UP`/`substitute_hp`)
  and the stat/status/flinch VETO behind a sub were already live (effect_for_volatile TryBoost +
  the side-effect HasVolatile guards). 5 new tests (create + bug #28 + absorb + break + a live
  end-to-end absorb through `execute_turn_with_move`).
- 2026-07-18: **Substitute phase 2 DONE — the direct-mutate ops vs the doll** (same
  `feat/battle-substitute` branch). The deferred `redirect_damage` seam, built. **Engine
  (jrpg-rules only, one defaulted binding):** `RuleBindings::redirect_hp_loss(ctx, who, source,
  amount) -> bool` (default `false`); the interpreter calls it before the FOUR direct-mutate hp
  ops (`DamageFraction` recoil, `SetHp` OHKO/Explosion, `DamageCurrentHpFraction` Super Fang,
  `RepeatHits` multi-hit) — these apply HP OUTSIDE the driver's `Event::Damage` fold, so they
  needed their own interception point. Returning `true` means the game routed the loss to a sink
  and the interp SKIPS the direct write. DEFAULTED ⇒ jrpg-rules (47) + minimon (39) stay
  byte-identical; the non-sub path of every pokered special/OHKO/multi-hit move is unchanged.
  **Pokered:** `PokeredBindings::redirect_hp_loss` routes the loss into the doll via the shared
  `absorb_into_substitute` (the same core the `Event::Damage` absorb uses), EXCEPT self-inflicted
  loss (`who == source`: recoil, Explosion self-KO) which bypasses the mover's OWN doll. This
  reproduces the oracle exactly: **Super Fang** deals `mon_curHP/2` (read from the MON, not the
  doll) into the doll; **OHKO** breaks the doll (the mon survives); **multi-hit** sends every hit
  to the doll (#1 via Event::Damage, #2..N via redirect). 4 new tests (Super Fang / OHKO /
  multi-hit vs a sub + the recoil-bypasses-own-doll exemption). Plus **create/break battle text**
  ("put in a SUBSTITUTE!" / "'s SUBSTITUTE broke!", the absorb silent as in Gen-1). Leech Seed /
  residuals correctly drain the mon behind a sub (they never fire Event::Damage / the interp ops).
  **All 24 audited missing moves are now DONE.**
  - **Adversarial review (11-agent workflow)** caught 4 real regressions the wiring introduced,
    all FIXED: (1) recoil/drain dealt 0 through a sub — the `Event::Damage` fold zeroed
    `ctx.mv.{damage,last_damage}`, which the DamagingHit riders (recoil/drain `LastDamage`,
    multi-hit `damage`) read; the driver now keeps the pre-fold number for downstream while
    applying the folded amount to hp (still byte-identical with no subscriber). (2) multi-hit
    re-hits dealt 0 vs a sub (same root cause). (3) OHKO failed to break a doll whose HP exceeds
    the mon's current HP — `SetHp(_,0)` now signals an unconditional break. (4) SLEEP was wrongly
    vetoed by a sub — Gen-1 sleep goes THROUGH a Substitute (oracle `apply_sleep` has no sub
    check), so the `status.sleep` RON veto was removed. 5 more tests (recoil/drain, multi-hit
    two-hit-total, OHKO-below-mon-hp, sleep-through-sub).
- 2026-07-18: **Battle-loop cleanup** on `feat/battle-loop-cleanup` (off `feat/battle-substitute`).
  - **Free-turn reconciliation:** the two single-enemy-free-move paths (after a failed escape /
    after a player switch) were the LAST live callers of the legacy `move_execution::execute_move`
    / `apply_move_effect`; they now run on the stack via `run_enemy_free_turn_stack` (player action
    = Nothing) with the full enemy-side reconciliations. `execute_turn_with_move` is untouched
    (self-contained helper), so lifecycle tests stay byte-identical; the legacy dispatcher is now
    ONLY the differential-test oracle.
  - **Substitute micro-notes CLOSED:** Counter behind a sub reflects the REAL absorbed damage (the
    phase-2 driver fix already restores the pre-fold number; verified + tested, matches Gen-1's
    pre-absorb wDamage). The same-turn create+break narration gap is fixed with a per-turn
    `SUB_CREATED` flag (set by substitute_install, cleared by clear_current_moves) so both
    "put in a SUBSTITUTE!" and "'s SUBSTITUTE broke!" show even when a doll is raised and broken
    the same turn. 3 tests; full pokered-core suite (1806) green.
- 2026-07-18: **Secondary-effect battle text (residual cause tags)** on `feat/battle-secondary-text`.
  The FIDELITY §1B "hurt by poison / burn / Leech Seed" gap needed the anticipated "HP-change cause
  tagging in jrpg-engine" — done as ONE generic, defaulted engine field. **Engine (jrpg-engine):**
  new `HpChangeCause<P>{ Status(P::Status), Volatile }` + a `cause: Option<HpChangeCause<P>>` on
  `TurnEvent::Damaged`/`Healed` (`None` = move damage, today's meaning). The driver's
  `residual_and_faint` now snapshot-diffs EACH residual SOURCE on its own (status → `Status(s)`,
  each volatile → `Volatile`) instead of once per phase, so per-source HP deltas carry their cause.
  ADDITIVE: STATE + `consumed()` are byte-identical (the parity oracle checks those, not the log);
  only MULTI-source residual logs split into tagged events. jrpg-engine (416) + jrpg-rules (47) +
  minimon (39) unchanged; ~7 `matches!(Damaged{..})` test patterns gained `..`. **Pokered:**
  `translate_turn` narrates the reliable `Status` residuals — `Status(Poison)` → "is hurt by
  POISON!", `Status(Burn)` → "is hurt by its BURN!". A **17-agent adversarial review** showed the
  generic `Volatile` tag is too weak to narrate faithfully: it can't tell a self-tick (Toxic /
  Leech) from a cross-battler unleash (Bide → opponent), and Leech's "sapped" cue vanishes when the
  seeder's heal clamps to 0 at full HP — so `Volatile`-caused HP loss is deliberately LEFT SILENT
  (as it was before this change — no wrong text). Faithful Toxic/Leech text needs a finer,
  per-volatile engine cause (`Volatile(P::EffectStateKind)`, requiring `TurnEvent<P: EffectProvider>`)
  — deferred. Recoil is Gen-1-SILENT (no text) so intentionally omitted. 3 tests (poison / burn /
  the "a Volatile sap is NOT mislabeled poison" guard). Full suite (1809) green. **Remaining
  battle-system TODOs:** #8 TUI writeback, #1 trainer-AI item/switch, #2 AI layer-2 encouragement,
  #5 Old-Man/Ghost, #4 Safari mode; plus faithful Toxic/Leech/drain residual text (finer cause).
- 2026-07-18: **Medium tier DONE — Conversion + Disable** on `feat/battle-conversion-disable`
  (a workflow first mapped every seam; adversarial review followed). Both zero-engine-change,
  game-side; p5_*_parity + the full 1798-test pokered-core suite green.
  - **Conversion** (变身术): new opaque `PokeVolatile::TypeOverride{type1,type2}` + a new
    `effective_types(ctx, who)` that consults the arena override before `species_types`, wired
    into the damage formula's ATTACKER types (STAB) + DEFENDER types (effectiveness/immunity,
    fed into `calculate_damage`) and the self-type-immunity quirk (`move_type_is_defender_type`).
    `conversion_install` (DamagingHit, keyed ConversionEffect) copies the TARGET's effective
    types onto the user — mirrors legacy `apply_conversion`. Persistence rides NEW **battle-only**
    `BattlerState::conversion_type1/2` fields (round-tripped by build_volatiles/write_party),
    NOT the persistent `Pokemon.type1/2` — this deliberately AVOIDS the Transform-style trap
    where the battle→save writeback (`save_data.party = from_pokemon(bs.party.clone())`) would
    permanently retype a mon. Reset in `reset_volatile_status` so Conversion clears on switch.
    **Known limitation (documented):** the `has_type` binding (the `VetoIf(HasType(..))`
    status-move type-immunity predicate) gets only `&EngineBattler` — no ctx/ref — so it stays
    species-based; a Conversion does not alter *status-move* type immunity (an extremely narrow
    interaction; covering it needs an engine trait-sig change).
  - **Disable** (定身法): reuses the existing `PokeVolatile::Disable{slot,turns}` + the legacy
    `disabled_move`/`disabled_turns_left` BattlerState carrier (the two live veto consumers —
    the player move menu + the smart trainer AI — already read it, so the round-trip makes them
    work for free). `disable_install` (DamagingHit, keyed DisableEffect) disables the target's
    last-used move (rode in via a new `LAST_MOVE_LIVE` thread-local primed pre-turn from
    `bs.*.last_move_used`, symmetric with CURRENT_MOVES) for `(rng&7)+1` turns. Two BeforeMove
    gates — `disable_decrement_gate` (order 50) + `disable_veto_gate` (order 80) — reproduce the
    Gen-1 ASM step order (decrement step 6, confusion step 7, block step 8), and run_event's
    short-circuit gives the "asleep ⇒ no decrement" interaction for free. build_volatiles/
    write_party round-trip the `Disable` volatile ↔ the legacy scalar fields. The oracle's
    PP>0 guard IS reproduced: the loop primes LAST_MOVE_LIVE via `disable_target_last_move`,
    which yields `None` for an out-of-PP last move so `disable_install` no-ops like the oracle.
    **Documented minor divergences:** slot is the compacted-engine index (== the full-array
    `disabled_move` for a gapless moveset, the Gen-1 norm — no interior move-slot gaps occur in
    normal play); the last-move is the PRE-turn value, so a *slower* Disable user vs a target
    that changed its move this turn disables the prior move, and a turn-1 slower Disable is a
    no-op (Disable normally locks a *repeated* move and a *faster* user is always exact).
  - **Adversarial review (17-agent workflow)** found only LOW/cosmetic items; fixed: the
    effectiveness NARRATION now honours a defender's Conversion override (was species-based,
    a text/damage mismatch) via an `effects`-aware `translate_turn`; the PP>0 guard above.
    Added tests for defender-side Conversion immunity, override-aware narration, the faster-
    disabler same-turn decrement, the PP guard, and the switch-reset.
  - **Remaining: ONLY Substitute** (create + ModifyDamage absorb + the already-live veto; plus
    the ONE deferred engine seam: `EffectProvider::redirect_damage` for Substitute vs the
    direct-mutate ops — Super Fang / OHKO / multi-hit). *(Substitute later shipped on
    `feat/battle-substitute`; the whole move roster is now complete — see the earlier entries.)*
- 2026-07-18: **Trainer-AI item/switch NOW LIVE + layer-2 reclassified** on `feat/battle-trainer-ai`
  (PR #92; a 6-agent workflow first mapped ALL remaining battle TODOs vs the current code, then a
  3-lens adversarial review hardened the wiring — see the review bullet below). Zero engine /
  `jrpg-rules` change; entirely game-side. Full pokered-core suite (1834 lib + every integration
  bin, incl. the stack-parity oracle + the P0 AI differential `stack_p0_ai.rs`) green.
  - **#1 Trainer-AI item use + switching wired to the production loop.** The per-class routines +
    `trainer_ai_config` (all unit-tested in `trainer_ai/ai_action.rs`, but with ZERO live callers
    before) now drive real play: new `BattleScreen.enemy_ai_count` (Gen-1 `wAICount`, seeded per
    class in `from_parties`, reset on each enemy send-out and on an AI switch); `take_enemy_ai_action`
    → `enemy_ai_action_inner(rand)` runs the decision and APPLIES it to the legacy `bs.enemy` BEFORE
    `engine_state_from_legacy`, so the mutation — heal HP / cure status / +1 stat STAGE (X-items) /
    `PROTECTED_BY_MIST` (Guard Spec) / switch to the first alive non-active mon — is carried into the
    stack turn by the existing legacy↔engine adapter. When it fires, the enemy SPENDS ITS TURN as
    `BattleAction::Nothing` (no attack) and the AI narration leads the turn text — so gym leaders /
    Elite Four finally heal, cure, boost, Guard-Spec, and switch. **The key insight: no new engine /
    `jrpg-rules` seam is needed** — "the enemy does Nothing this turn" makes the whole action a
    pre-turn mutation the existing adapter already round-trips (HP/status/stat-stages/Mist/active
    mon). `wAICount` follows Gen-1's PER-CONSULTATION cadence: one charge spent every turn the AI is
    consulted (count > 0) regardless of whether the routine acted — a DoNothing roll or a rolled-but-
    impossible switch still spends it, so a routine may only act within the first `count` turns a mon
    is out. Conservative guards: skip the AI (spending NO charge) while the enemy is locked
    (`move_is_locked`) or recharging. 16 tests (per-routine apply: Brock Full-Heal cure, CooltrainerF /
    Full-Restore heal, Bruno X-Defend stage, Giovanni Guard-Spec Mist, Juggler switch + budget reset,
    the per-turn budget cadence, canonical-species narration, the count / wild / locked guards, the
    three speed-ordered placement cases, + a live-loop end-to-end cure through `execute_turn_with_move`).
    Scoped to `execute_turn_with_move`;
    the enemy free-turn after a *player* switch (`run_enemy_free_turn_stack`) is a deliberate Phase-2
    follow-up (there the player already does Nothing, so a fired AI item wastes the whole round).
  - **Adversarial review (3-lens × verify workflow, 12 agents).** Cleared two suspected regressions
    (the switch-target scan MATCHES Gen-1 `EnemySendOut` — first alive non-active from slot 0; the
    extra RNG byte on non-firing turns is inert vs the unseeded live `ThreadRng`). Fixed three
    confirmed findings: **per-turn `wAICount` cadence** (was per-action → trainers acted too often /
    too late), **failed-switch charge stays spent** (was refunded), and **canonical species narration**
    (`species_name`, not the strum variant — "MR.MIME" not "MRMIME").
  - **Speed-ordered placement DONE** (the review's #1, the medium finding — now fixed, not just
    documented). `decide_enemy_ai_action` (spends the budget, no mutation) is split from
    `apply_enemy_ai_action` (mutate + narrate), and `execute_turn_with_move` PLACES the action by
    `turn_order::determine_order` (from the selected moves, as Gen-1 computes it before `TrainerAI`):
    ENEMY-first → apply BEFORE the turn (the player then hits the healed/boosted/switched enemy);
    PLAYER-first → DEFER to after the player's move and CANCEL on a KO (Gen-1 skips `TrainerAI` when the
    player already fainted the enemy — no more negated KOs). No refund is needed on the wasted charge:
    a fainting mon re-seeds its `wAICount` on the next send-out, so an over-spend on a KO'd mon never
    matters. 3 new tests (player-first KO cancels the heal; player-first non-KO heals after the move;
    enemy-first heals before it). **Residual remaining edge (documented, needs a turn-split engine
    seam):** for a PLAYER-first heal, the player's move + end-of-turn residuals run in ONE StackDriver
    call, so a poisoned/burned enemy near residual-lethal HP faints before the deferred heal (Gen-1
    heals between the move and the residual). Narrow — only bites a statused enemy; a non-statused enemy
    (the common case) is now exactly faithful. Net: trainers previously NEVER used items.
  - **#2 AI layer-2 encouragement is NOT a gap** (the mapping verified against pret/pokered): the
    hardcoded `0` fed to `choose_moves` in `pick_enemy_move` is FAITHFUL — `wAILayer2Encouragement`
    is never set to 1 in Gen-1 (its only writer `ReplaceFaintedEnemyMon` zeroes it; WRAM default 0),
    so `AIMoveChoiceModification2` (`cp $1 / ret nz`) is dead code and Layer2 classes behave as
    Layer1(+Layer3). Reclassified from a gap; self-documented with a named `AI_LAYER2_ENCOURAGEMENT:
    u8 = 0` const + a warning NOT to flip it (that would be a non-faithful "smarter AI" house-rule,
    against the fidelity policy — not a fix).
  **Remaining battle-system TODOs:** #8 TUI battle writeback, #5 Old-Man tutorial catch + Ghost
  Marowak "?"/uncatchable overlay (the scripted Marowak battle is already wired), #4 Safari battle
  mode (Ball/Bait/Rock/Run — genuinely new, HIGH difficulty); plus faithful Toxic/Leech/drain
  residual text (a small ADDITIVE per-volatile engine cause — the mapping found the bound-flip is a
  no-op for every current caller).
- 2026-07-18: **Faithful Toxic / Leech Seed residual text DONE** on `feat/battle-residual-text` (the
  deferred "finer per-volatile cause" from the secondary-text entry — now built). The earlier generic
  `HpChangeCause::Volatile` marker was too weak to narrate (couldn't tell Toxic from Leech from Bide),
  so volatile-caused HP loss was left SILENT. Fixed by carrying the game's opaque per-volatile token.
  **Engine (jrpg-engine only, additive):** `HpChangeCause::Volatile` → `Volatile(P::EffectStateKind)`,
  which tightens `HpChangeCause` / `TurnEvent` / `TurnLog` from `BattleProvider` to `EffectProvider` —
  a **no-op for every caller** (both `execute_turn` + `execute_turn_logged` and all their helpers are
  ALREADY `<P: EffectProvider>`; `Snap<P>` stays on `BattleProvider`). The driver's residual loop now
  captures each source's `kind` and tags both the actor's `Damaged` and the opponent's paired `Healed`
  (Leech's drain-to-source) with it. **DEFAULTED / byte-identical:** jrpg-engine (416) + jrpg-rules (47)
  + minimon (39) all green — only the log payload grows; STATE + `consumed()` (what the parity oracle
  checks) are unchanged. **Pokered:** `translate_turn` narrates `Volatile(Toxic)` → "is hurt by POISON!"
  (a badly-poisoned mon's ramp chips via the Toxic VOLATILE — the plain-Poison status residual SKIPS
  when Toxic is live, `poison_residual`'s "one chip, not two", so its text lives on the Volatile arm,
  not Status) and `Volatile(LeechSeed)` → "'s HEALTH is sapped by LEECH SEED!" (narrated on the DRAINED
  mon; the seeder's paired `Healed` stays silent — Gen-1 prints one line). Bide's cross-battler unleash
  and other volatiles stay silent (their own move flow narrates). 3 tests (leech sap narrates + not
  mislabeled poison; Toxic ramp = exactly ONE poison line, proving no status+volatile double; Toxic +
  Leech both attributed distinctly — the case positional inference couldn't resolve). This closes the
  last battle-text gap. **Remaining battle-system TODOs:** #8 TUI writeback, #5 Old-Man/Ghost, #4 Safari
  mode (drain-MOVE recovery text — Absorb/Mega Drain — is a separate game-side `Healed(cause:None)` item,
  and Gen-1 is silent there anyway).
- 2026-07-18: **TUI battle writeback DONE** (`#8`) on `feat/battle-tui-writeback`. The TUI frontend
  silently DISCARDED every battle result — its `handle_transition` post-battle arm was an empty
  comment, so EXP/levels/HP/status/PP, catch, Pokédex, money, blackout, and trainer-defeated were all
  lost. Root-fixed by EXTRACTING the native app's ~120-line inline settlement into a shared
  `pokered_core::battle::settlement::settle_battle_into_save(&mut BattleScreen, &mut SaveData, &mut
  OverworldScreen) -> Option<&'static str>` (the outcome string for script resume), so both frontends
  run ONE fidelity-sensitive implementation instead of drifting copy-paste. The native call site now
  just calls the helper (behaviour-identical — the app still owns its own audio + `resume_script`
  tail); the TUI's empty arm calls it too. Also fixed the TUI's MISSING pre-battle setup (the reason a
  naive port would WIPE the bag): `start_wild_battle`/`start_trainer_battle` now copy `map_id` +
  `player_bag` into the battle and `pokedex.set_seen` the encounter — mirroring the native. 7 new
  pokered-core unit tests on the helper (win money, blackout, the Oak's-Lab-Rival1 no-blackout special
  case, catch→party, catch→PC-box-when-full, party-mutation writeback, and the bag-not-wiped guard).
  Full pokered-core suite (1828) green; app + TUI build. **Remaining battle-system TODOs:** #5
  Old-Man/Ghost, #4 Safari mode.
- 2026-07-18: **Ghost Marowak "?"/uncatchable overlay DONE** (`#5`b) on `feat/battle-ghost-marowak`.
  The scripted Marowak battle was already content-wired; what was missing is the Gen-1 GHOST
  presentation for a Pokémon-Tower wild encounter met WITHOUT the Silph Scope. New `BattleScreen.is_ghost`
  (default false); the app's `start_wild_battle` sets it when `current_map ∈ PokemonTower1F..=7F` &&
  the bag lacks `SilphScope`, and GUARDS `pokedex.set_seen` behind `!is_ghost` (a GHOST is unidentified —
  the species must not register as seen). `use_ball` early-returns "The GHOST is dodging your POKé BALLs!"
  (uncatchable; the ball is not consumed). Renderers (native + TUI) override the enemy NAME → "GHOST"
  (so the name-tile draw + the "Wild GHOST appeared!" line agree), and the native renderer loads the
  `gfx/battle/ghost.png` sprite (via `load_battle("ghost")`, a different asset category than
  `load_pokemon_front`) instead of the real species front. 2 pokered-core tests (default-not-ghost;
  a ball at a 1-HP GHOST is dodged with no capture). Full pokered-core suite (1823) green; app + TUI
  build. **Reading (low-risk, documented):** the GHOST keeps real HP/stats and is a normal fight you
  simply can't catch or identify (Gen-1's uncatchable+unidentified, not an unwinnable auto-flee); ball-
  dodge does not consume the player's turn; exact dodge wording is best-effort. **Old-Man tutorial
  (`#5`a) DEFERRED** — it needs a net-new scripted/auto-play battle mode (the driver picks the action)
  for a zero-gameplay cosmetic; the ViridianCity dialogue stub stands. **Remaining battle-system TODO:
  #4 Safari mode.**
- 2026-08-02: **Ghost Marowak reveal + ghost-battle mechanics DONE** (supersedes the "normal fight"
  reading above — the original IS an unwinnable fight without the scope). Ported from
  `engine/battle/common_text.asm` (PrintBeginningBattleText), `engine/battle/core.asm`
  (PrintGhostText/IsGhostBattle/TryRunningFromBattle) and `engine/battle/ghost_marowak_anim.asm`:
  ghost intro texts "Enemy GHOST appeared!" + "Darn! The GHOST can't be ID'd!" (new IntroPhase
  GhostCantID); the 6F Marowak battle now starts REGARDLESS of the Silph Scope
  (PokemonTower6F/script.scene drops its hasItem gate, matching scripts/PokemonTower6F.asm:36);
  WITH the scope the battle gets `BattleScreen.ghost_marowak_reveal` (derived in
  start_wild_battle from tower map + Marowak species + scope — exactly the original's
  `cp RESTLESS_SOUL`, since `RESTLESS_SOUL EQU MAROWAK`) → new IntroPhase GhostUnveil plays
  "SILPH SCOPE unveiled the GHOST's identity!" while the (previously dead) GhostMarowakReveal
  render anim runs (flash 8×, fade out ghost, fade in Marowak) with SFX_SILPH_SCOPE, then the
  intro loops back to a normal "Wild MAROWAK appeared!". Ghost-battle turns: the player's move
  fails with "<MON> is too scared to move!" (skipped when the mon is asleep/frozen, like the
  original's FRZ/SLP check; no PP spent), the GHOST never attacks ("GHOST: Get out... Get
  out..."), and RUN always succeeds. 7 new pokered-core tests. Battle texts stay English-only,
  matching every existing battle message.
- 2026-07-18: **Safari Zone battle mode DONE** (`#4`) on `feat/battle-safari-mode`. Safari encounters
  now run the real BALL / BAIT / ROCK / RUN mode instead of an ordinary wild battle — ported DIRECTLY
  from the disassembly (`/Users/liuyanghe02/develop/pokered-worktree`): `engine/items/item_effects.asm`
  (`ItemUseBait` / `ItemUseRock` / `BaitRockCommon`), `engine/battle/safari_zone.asm`
  (`PrintSafariZoneBattleText`), and the Safari branch of `engine/battle/core.asm` (the flee check).
  **CORE mechanics** — new `battle/safari.rs` `SafariState` (live catch rate + eating/anger counters +
  ball count) with the EXACT Gen-1 rules: bait halves the catch rate & clears anger & raises the eating
  counter by `rand(1..=5)`; rock doubles the catch rate (cap 255) & clears eating & raises the anger
  counter; per-turn upkeep decrements bait-then-anger (restoring the base catch rate when anger wears
  off); the flee roll is `b = (speed & 0xFF) * 2` (immediate run if the low byte > 127), `÷4` while
  eating, `×2` (cap 255) while angry, run iff `random < b`; the 1..=5 roll uses the `Random & 7`
  rejection loop. 14 mechanics tests verify each against the ASM. **Integration** — `BattleScreen`
  gains `is_safari` + `safari: Option<SafariState>` + a `safari_menu`; the PlayerMenu input drives the
  Ball/Bait/Rock/Run menu and `resolve_safari_action` (action → out-of-balls check → upkeep → flee, the
  ASM order); a catch sets `captured_mon` (the app adds it), RUN/flee/out-of-balls end the battle. App:
  `start_wild_battle` switches to Safari mode when `is_safari_zone_map && is_safari_game_active`, seeding
  `SafariState` from the species catch rate + `safari_balls_remaining`; the post-battle writeback folds
  the balls thrown back into the overworld game (its 0-ball game-over / eject keys off this). Rendering:
  a new `battle_safari.gui` (BALL/BAIT/ROCK/RUN 2×2) + `menus::battle_safari::draw`, dispatched on
  `is_safari`. 3 integration tests (RUN escapes, BAIT halves the rate live, a BALL is consumed). Full
  pokered-core suite (1838) green; ui + app + tui build. **Documented minor gaps:** the TUI renderer
  still shows the normal FIGHT labels for a Safari battle (secondary frontend; the input/mechanics are
  shared and correct); the remaining ball count isn't drawn in the menu box; the ROCK/BAIT battle
  animations aren't wired. **All Phase-1B battle-system TODOs are now complete.**
- 2026-07-18: **Old-Man catch tutorial DONE** (`#5`a — was deferred) on `feat/battle-oldman-tutorial`,
  built faithfully from the disassembly (`engine/battle/core.asm` `DisplayBattleMenu` BATTLE_TYPE_OLD_MAN
  branch + `scripts/ViridianCity.asm`). The Viridian Old Man's "NO, show me how to catch" branch now
  runs the real **auto-played demo battle** instead of a dialogue stub. New `BattleScreen.is_old_man`:
  the PlayerMenu phase AUTO-PLAYS `resolve_old_man_tutorial` (no input) — narrates the OLD MAN's POKé
  BALL throw + a scripted, GUARANTEED catch of the Lv5 WEEDLE, which is a DEMO so `captured_mon` stays
  `None` (nothing joins the party, matching Gen-1). The app renderer shows the player as "OLD MAN".
  **Tutorial-battle API** (the "no API" blocker the ViridianCity stub cited): new `ScriptCommand::
  OldManTutorial` → `game.oldManTutorial()` → `ScriptEffect::OldManTutorial` → a pending WEEDLE-Lv5
  encounter tagged `old_man` (new `PendingWildEncounter.old_man`); the app consumer sets
  `battle.is_old_man` and suspends/resumes the script like `startWildBattle`. `ViridianCity.scene` calls
  `game.oldManTutorial()` between the two dialogue lines. 1 core test (the demo catches but keeps
  nothing). Full pokered-core suite (1822) green; app + TUI build. **Documented minor gaps:** the TUI
  frontend's encounter consumer doesn't set `is_old_man` (secondary frontend — no auto-play/name there);
  the old-man BACK sprite (`core.asm:6199`) isn't swapped (name only). **Every Phase-1 battle-system TODO
  — 1A + all of 1B — is now complete.**
- 2026-07-18: **Field-move menu + all 7 HM/field moves wired DONE** (Phase-2 start) on
  `feat/phase2-field-moves`, ported from the disassembly. The party submenu is now dynamic —
  [field moves…, STATS, SWITCH, CANCEL] (Gen-1 `GetMonFieldMoves`, engine/menus/text_box.asm:509)
  listing known field moves in moveset order with NO badge check on display (Gen-1 checks on
  use); the box grows 2 rows/move and shifts left for STRENGTH/TELEPORT per
  `FieldMoveDisplayData`. New `FIELD_MOVE_TABLE` (data/moves/field_moves.asm port) +
  `PartyScreenAction::UseFieldMove` → `OverworldScreen::use_field_move` dispatching all seven
  with the original messages. Badge gates verified from engine/menus/start_sub_menus.asm
  (`.outOfBattleMovePointers`): CUT→CASCADE, FLY→THUNDER, SURF→SOUL, STRENGTH→RAINBOW,
  FLASH→BOULDER; DIG/TELEPORT ungated; "No! A new BADGE is required." on failure. **SURF**:
  `IsSurfingAllowed` + `ItemUseSurfboard` — jrpg-engine gains a defaulted `is_water_tile`
  provider op + `CollisionResult::StopSurfing` (`CollisionCheckOnWater`, home/overworld.asm:1888:
  water→passable, passable-land→StopSurfing, else blocked), so `on_water` is finally reachable
  and water is crossable only by surfing; dismount guard "There's no place to get off!".
  **CUT**: `UsedCut` ($3D overworld / $50 gym tree, $52 grass text+SFX only,
  `CutTreeBlockSwaps`). **STRENGTH**: `BIT_STRENGTH_ACTIVE`, reset on every map load
  (`ResetUsingStrengthOutOfBattleBit` via `EnterMap`); boulder push ported from
  engine/overworld/push_boulder.asm (push-twice flag, held-direction match, destination
  passable + no pair-collision + not stairs + no sprite, `SFX_PUSH_BOULDER`, dust lockout tick
  before the map script). **FLY**: town-map fly mode (`LoadTownMap_Fly`) — 11 city maps gated by
  a new `wTownVisitedFlag` bitfield (marked on map load for city maps < `FIRST_ROUTE_MAP`,
  `BuildFlyLocationsList` order, UP=next wrap quirk intact); landing = `FlyWarpDataPtr` (matched
  the existing `FLY_DESTINATIONS` exactly). **FLASH** clears `wMapPalOffset`; `DarkCaveState`
  set on entering Rock Tunnel 1F/B1F. **DIG** reuses the EscapeRope flow (Gen-1 `.dig` literally
  loads ESCAPE_ROPE); **TELEPORT** warps to the last Pokémon Center. Tests: 30 field-move
  integration + 7 party-menu + 6 town-map + 6 surf collision + 4 table — one real bug caught
  (boulder destination evaluated the boulder's own tile instead of the tile beyond). Full
  pokered-core (1917 lib) + jrpg-engine (416) + data/ui/tui suites green; app + tui build.
  **Documented minor gaps:** TUI FLY picker absent (no town-map screen there; the other six
  moves work via the shared dispatcher); dark-cave RENDERING is Phase-3 renderer work (the state
  is fully wired); TELEPORT lands at Pallet Town until the end-game suite wires
  `SetLastBlackoutMap` (`last_blackout_map` is never set in play — Gen-1-identical before the
  first heal); DIG keeps the project's pre-existing Gen-2+ entrance semantics (Gen-1 warps to
  the last PokéCenter — deliberate, flagged); Cycling Road forced-bike (`BIT_ALWAYS_ON_BIKE`)
  not ported (`use_surf`'s `forced_bike` always false); boulder push slides instantly (no dust
  animation / per-map boulder persistence); the fly map reuses the town-map viewer (no bird
  sprite / "TO>" prompt — cosmetic); SOFTBOILED (9th Gen-1 field move) excluded — a healing
  effect, separate scope.
- 2026-08-04: **Cycling Road forced bike + Softboiled + SS Anne + boulder dust + TUI old-man DONE** on `feat/field-polish` (see the audit doc's Wave-9 section for full details). Supersedes the following open items in this entry: Cycling Road forced-bike (`BIT_ALWAYS_ON_BIKE` — auto-mount on the 4 lock tiles Route16/18, gate dismount, blackout/fly-warp release, "You can't get off here." / "Cycling is fun! Forget SURFing!"); SOFTBOILED as the 9th field move (HP>max/5 gate, user −1/5 max HP transfer capped, no PP/item consumed, "Not healthy enough."); SS Anne ship-sail animation (VermilionDock_EraseSSAnne port: 120f pause → hull wipe → SFX_SS_ANNE_HORN → 8×128f column scroll with smoke puffs → erase + warp removal; flag-first plays-once; transient per visit, faithful); boulder dust particles (AnimateBoulderDust: 8 steps × 3f, 2×2 smoke block, OBP1 flash, drift opposite the push, horizontal-push 3-of-4 quirk); TUI old-man tutorial (`is_old_man` + "OLD MAN" name) + old-man back sprite swap (`OldManPicBack`, core.asm:6202 — intro silhouette only, faithful).
- 2026-08-04 (field-residuals): **Bike sprite + boulder persistence verify + dust SFX_CUT DONE** on `feat/field-residuals`. Bike sprite: both frontends now draw `red_bike.png` while `TransportMode::Biking` (LoadBikePlayerSpriteGraphics → RedBikeSprite, home/overworld.asm:1977-1990, gfx/sprites.asm:34; same 6-frame layout as red.png; app render smoke test `biking_renders_red_bike_sheet_not_red`). Boulder persistence: **verified faithful — the original does NOT persist boulder positions** (LoadMapHeader re-reads every object's Y/X from ROM on each map load, home/overworld.asm `.loadSpriteLoop`), so pushed boulders reset on re-entry in the original too; the *consequences* (boulder into Seafoam hole / onto Victory Road 3F switch) persist via saved event flags + `wToggleableObjectFlags` (Main Data save section, ram/wram.asm:1913; HideObject/ShowObject + CheckSpriteAvailability→IsObjectHidden, movement.asm:478-490; BIT_PUSHED_BOULDER handshake, push_boulder.asm:97 → SeafoamIslands1F/B1F/B2F/B3F.asm + VictoryRoad3F.asm), which the port already reproduces with DOWN_HOLE/ON_SWITCH event flags + per-floor @load scenes. Known small deviation (pre-existing, noted in audit doc): the port's Seafoam B1F/B2F boulders are visible from first arrival, whereas the original starts them hidden until the upper floor's boulder falls through. SFX_CUT at dust completion: `DoBoulderDustAnimation` (push_boulder.asm:89-103) plays SFX_CUT once when the 8×3f dust animation ends; port fires it on the dust's completion tick (`tick_boulder_push`), covered by `boulder_dust_completion_plays_sfx_cut_once`.
- 2026-08-04 (field-residuals): **Movement/field-move residuals DONE** on `feat/field-residuals`. DIG: the earlier note "DIG keeps Gen-2+ entrance semantics (deliberate)" is **rescinded — the asm proves Gen-1 DIG warps to the last PokéCenter**: start_sub_menus.asm `.dig` (195-199) loads ESCAPE_ROPE as a pseudo-item into ItemUseEscapeRope (item_effects.asm:1492-1528, sets BIT_FLY_WARP|BIT_ESCAPE_WARP), and LoadSpecialWarpData resolves BIT_ESCAPE_WARP to `wLastBlackoutMap`'s fly point (special_warps.asm:76-80; SetLastBlackoutMap stores the pre-Center map at heal time, set_blackout_map.asm:1-23 — Safari rest houses excluded). The port's EscapeRope + DIG now warp to the last-healed map's FLY_DESTINATIONS point; eligibility matches EscapeRopeTilesets exactly (FOREST/CEMETERY/CAVERN/FACILITY/INTERIOR — SS Anne/GATE/PC etc. now refuse, plus the explicit AGATHAS_ROOM refusal); DIG still consumes nothing. Deviation kept: the refusal text stays the port's "Can't use that here." (original: "OAK: <PLAYER>! This isn't the time to use that!"). Route 17 slope: JoypadOverworld's forced PAD_DOWN simulation (home/overworld.asm:1826-1835 — whole map, no tile list; gated on BIT_TRAINER_BATTLE and no d-pad/A/B held) + DoBikeSpeedup (377-388: double speed cancelled while UP/LEFT/RIGHT held on Route 17) ported — engine `PlayerState.bike_speedup_active` knob + update.rs input injection. Seafoam forced-SURF currents: the 4 remaining ForcedBikeOrSurfMaps entries (B3F (18,7)/(19,7), B4F (4,14)/(5,14)) now force `wWalkBikeSurfState = 2` on map entry (surf transport, no bike bit); dismount stays terrain-blocked exactly like the original (water-surrounded tiles / CAVERN $14→$05 pair); no persistent lock to release. Remaining (noted in audit doc): the B3F MOVE_OBJECT/DEFAULT current-sweep scripts (scene-layer RLE at (15,8) and (18,7)/(19,7)) are still not ported; the B4F sweeps (currentWest/currentSouthEast) already exist.

- 2026-07-21: **Battle-animation sound system wired** (MoveSoundTable from data/moves/sfx.asm
  transcribed to `pokered-data/src/move_sfx.rs` — 165 moves + 1 table-tail row, verified 0-diff
  by `workspace/scripts/verify_move_sfx_data.py`). Move SFX now play **per animation command**
  (GetMoveSound + PlaySound in PlayAnimation/PlaySubanimation, engine/battle/animations.asm)
  instead of once per " used " message; GROWL/ROAR play the whose-turn mon's cry
  (IsCryMove/GetCryData). The BATTLE ANIMATION option is wired in both frontends: when off,
  the animation and its per-command sounds are skipped and replaced by the original's
  DelayFrames(30) + applying-attack feedback (`.animationsDisabled`).
  **Follow-up (same day):** the pitch/tempo modifier bytes are now **applied** —
  the disassembly gates them on `Audio1_IsCry`, so they only ever affect cries:
  `pokered-data/src/cries.rs` ports `data/pokemon/cries.asm` (verified 0-diff by
  `workspace/scripts/verify_cry_data.py`), `AudioManager::play_cry` applies
  `wFrequencyModifier`/`wTempoModifier` (note-frequency add + cry tempo
  `0x0080 + mod`), and GROWL/ROAR add their command row's bytes on top — this
  also closed the pre-existing per-species cry pitch/length gap (all cry play
  sites go through `play_species_cry`). `WaitForSoundToFinish` is modeled:
  both frontends defer the animation start while an SFX is still playing
  (`BattleVisualEffects.sfx_playing` + `pending_anim_start`).
- 2026-07-21: **Battle-animation data + engine + renderer audited against the
  disassembly** (reference: pret/pokered @ fbcf7d0e1, read-only). New script
  `workspace/scripts/verify_battle_anim_data.py` byte-compares all four data tables
  (`BASE_COORDS` 177, `FRAME_BLOCK_DATA` 122, `SUBANIM_DATA` 86, `MOVE_ANIM_DATA`
  203) with the asm — 6 transcription errors fixed (FrameBlock34/37 flag bits, a
  stray 16th tile in FrameBlock62 that the original count byte excludes), and the
  frame-block offsets are now pixel-exact (61 sub-tile dbsprite offsets previously
  floored to whole tiles). Engine fixes in `battle_anim/player.rs`: a transpose
  bug that rendered every frame block with X/Y offsets swapped; `SlideMonOff`
  mis-mapped to `SlidePlayerMonHalfOff` (now a full slide-off, `e=8` vs `e=4`);
  `FlashScreenLong` 16→48 frames, `ShakeScreen` 4px/8f→8px/72f; per-command
  `sound` exposed on `AnimTickResult`; the 24-entry per-frame hook table
  (`AnimationIdSpecialEffects`) implemented (HyperBeam flash-every-4, RockSlide
  shakes, Explosion hides attacker, Growl OAM doubling; ball-toss/trade hooks
  need capture/trade flow state and are stubbed with notes);
  `ShareMoveAnimations` (enemy AMNESIA→CONF_ANIM, REST→SLP_ANIM). Renderer: new
  shared `jrpg-renderer/src/battle_anim/effects.rs` consumed by both
  `pokered-app` and `pokered-tui` — faithful Substitute doll (MonsterSprite
  tiles), Minimize blob, Squish, Transform, mon-local ShakeBackAndForth,
  enemy-HUD-area shake, spiral/upward balls and petal/leaf/droplet particles
  drawn from the real move_anim tiles (replacing generic gray particles); fixed
  a pre-existing bug where sub-animation OAM tile ids (absolute, +$31) indexed
  the 80-tile anim tilesets directly so **all** sub-animations rendered blank.
  Visual spot-check (Substitute/Minimize/Explosion/PetalDance/WaterGun/Thunder/
  Earthquake/Teleport + enemy Scratch mirror path) captured and reviewed.
  Residual deviations: FallingObjects' out-of-range movement byte
  approximated (original reads code bytes — original bug); ball-toss/trade
  per-frame hooks need capture/trade flow state (stubbed with notes). Fixed
  in the follow-up: ShakeEnemyHud now shakes the enemy mon with the HUD
  strip (BG scroll), SlideMonDownAndHide has its own two-step 7×5/7×3
  row-crop, BounceUpAndDown is the original 5× slide-down cycle.
- 2026-08-04: **Cycling Road forced-bike (`BIT_ALWAYS_ON_BIKE`) DONE** on
  `feat/field-polish` — the `CheckForceBikeOrSurf` lock is ported as
  `pokered-core/src/overworld/forced_bike.rs` (`ForcedBikeState` +
  `FORCED_BIKE_TILES` = the four `ForcedBikeOrSurfMaps` bike tiles: Route 16
  (17,10)/(17,11), Route 18 (33,8)/(33,9); the Seafoam forced-SURF entries are
  the currents mechanic, separate). `OverworldScreen::apply_map_entry_transport`
  runs the check on all three map-entry paths (`new`, `commit_pending_warp`,
  the connection-walk swap): entering a forced tile mounts the bike, entering
  either gate (`Route16Gate1F_Script` / `Route18Gate1F_Script` start with
  `res BIT_ALWAYS_ON_BIKE`) auto-dismounts, and the lock persists across the
  whole road; FLY/DIG/TELEPORT/ESCAPE ROPE arrivals (`HandleFlyWarpOrDungeonWarp`)
  and blackout (`DisplayPlayerBlackedOutText` / battle core.asm:1160-1162 —
  cleared in `settle_battle_into_save`'s Loss path) release it. While locked:
  the BICYCLE item prints "You can't get off here." and SURF prints "Cycling
  is fun! Forget SURFing!" (`use_surf`'s `forced_bike` is now live).
  Tests: 17 forced-bike tests (5 state-machine unit + 11 screen-level incl.
  warp/gate/blackout/fly lifecycles and both refusals, + the pre-existing
  `surf_forced_bike` gating test). **Documented minor gaps:** the
  Route 17 slope — the forced-down auto-ride (`JoypadOverworld` simulating
  PAD_DOWN when idle) and `DoBikeSpeedup`'s double-speed — is still unwired;
  the renderer draws the walking "red" sprite for Biking (no bike frames —
  graphics-only, matches the pre-existing no-bike-sprite state); the TUI's
  `PlayMapMusic` doesn't key off transport (bike music is app-only).
- 2026-08-04: **SOFTBOILED as the 9th Gen-1 field move DONE** on
  `feat/field-polish`. Added to `FIELD_MOVE_TABLE` (data/moves/field_moves.asm:
  entry 9, name index 9, leftmost tile $08 — no badge gate, `.softboiled`).
  Party-menu flow (start_sub_menus.asm:236-274): user needs HP > max/5 else
  "Not healthy enough."; the menu then reopens in target-pick mode
  (`PartyScreenMode::SoftboiledTarget`, A on the user itself loops like the
  original's `ItemUseMedicine`; B returns to the normal party menu); the heal
  (`bag_use::apply_softboiled`, mirroring the POTION pseudo-item path in
  item_effects.asm:1003-1074) takes 1/5 of the USER's max HP (truncating
  Divide), gives it to the target capped at max HP, refuses fainted/full-HP
  targets ("It won't have any effect."), consumes nothing (no item, no PP),
  and narrates the Gen-1 party-menu text "{name} recovered by {N}!". Wired in
  both frontends (app + TUI: `pending_softboiled_user` +
  `PartyScreenAction::SoftboiledTargetChosen`; `Party::get_two_mut` for the
  two-mon mutation). Tests: 12 (table/badge/leftmost, party-menu listing +
  target-pick flow, screen-level refusal texts, heal math incl. the 99/5
  truncation and the max-HP cap).

# Unified Game DSL — Fused Design

This branch fuses the two earlier migration attempts
(`feat/dsl-scene-migration` and `feat/migrate-pokered-stories-to-dsl`) into one
design that takes the strengths of each and drops their weaknesses.

| | scene branch | stories branch | **this (fused)** |
|---|---|---|---|
| Runtime actually runs `.scene` | ❌ deferred | ✅ | ✅ (inherited from stories) |
| `@speaker` prefix fidelity (`System:` bug) | ✅ empty-speaker | ❌ `System:` leaks | ✅ empty-speaker, applied to content |
| First-class `@if/@else`, `@choice` | ✅ | deprecated for `@run` | ✅ first-class; `@run` = bounded escape hatch |
| pokered-editor edits `.scene` | ❌ | basic highlight | ✅ highlight + **validation + compiled-JS preview** |
| Binding source of truth | `script_config.json` | `script_config.json` | **`.scene` (`@trigger`)**, JSON regenerated + drift-tested |

## Design principles

1. **First-class control flow, bounded escape hatch.** `@if/@else`, `@choice`,
   `@speaker`, bare commands are the DSL's domain language. `@run { …JS… }` is
   the *escape hatch* for genuinely irregular logic (e.g. member access /
   dynamic math the small condition sublanguage can't express) — not the main
   road. Conditions stay a small, analyzable sublanguage (flags, vars,
   comparisons, `&& || !`, calls) so the storyline conflict-detector and
   validators can reason about them.

2. **`@speaker("")` is the narrator form.** An empty speaker compiles to a bare
   `game.showText("…")` with no prefix. Prefix-less original dialogue uses it,
   so the `System: `/`": "` prefix can never leak into the textbox.
   (`compile_speaker`, `js_storyline.rs`.)

3. **The `.scene` is the single source of truth for bindings.** Each storyline
   declares its routing/binding inline:
   ```
   @storyline("talkOak") {
     @trigger(map = "PalletTown", npc = 1, toggle = "PALLET_TOWN_OBJ_1",
              script = "PALLETTOWN_OAK", hidden = true)
     …
   }
    @storyline("coordNorthExit") {
      @trigger(map = "PalletTown", name = "northExit1")
      @trigger(map = "PalletTown", name = "northExit2")
      …
    }
   ```
   A storyline may carry **several** `@trigger` lines when multiple objects map
   to one handler (e.g. OaksLab's two POKéDEX balls → `talkPokedex`).
   `script_config.json` is regenerated from the `.scene`
   (`config_gen::compile_scene_to_config`, `bin/gen_map_config`), and
   `tests/config_roundtrip.rs` asserts the generated bindings match the
   committed config for **all 248 maps** — the no-drift guarantee.

4. **The runtime runs the DSL.** `.scene` files are compiled to JS at load time
   (`pokered-data::scene_loader`) with a `.js` fallback and hot-reload
   (inherited from the stories cutover).

5. **The editor edits the DSL directly.** `crates/jrpg-web` exposes a
   `compile_scene` WASM bridge; the pokered-editor adds a CodeMirror linter
   (inline compile errors) and a live compiled-JS preview pane.

## Migration tooling

- `tools/dsl_migration/unify_scenes.py` — rewrites the `.scene` corpus onto the
  fused design: `@speaker("System") → @speaker("")`, regenerates every
  `@trigger` from `script_config.json` (full npc/sign/coord binding incl.
  toggle/script/hidden, all coord tiles), injects `no_talk` storylines for
  toggled objects with no dialogue handler.
- `crates/jrpg-engine-dsl/src/bin/gen_map_config.rs` — regenerate
  `script_config.json` from the `.scene`.

## Verification

- All 248 `.scene` compile (`gen_map_config`: 248 ok, 0 failed).
- `config_roundtrip`: 0 binding mismatches across 248 maps.
- `scene_onload_resolves_at_runtime`: every map with a non-empty `@load` body
  has a committed `onLoad` that equals its compiled `<Scene>OnLoad` export, so
  the runtime's `has_function(onLoad)` lookup actually resolves (guards the
  map-entry-script regression; see below).
- 0 `@speaker("System")` remain (the `System:` regression is gone from content
  *and* codegen; `test_speaker_empty_name_no_prefix`).
- `cargo test -p jrpg-engine-dsl` is fully green (lib 225, codegen 31,
  integration 17, build 4, error-quality 14, snapshots 8, config round-trip 2).
- `cargo build` of `jrpg-engine-dsl`, `pokered-data`, `pokered-app`, `jrpg-web`
  all pass. Editor: `vue-tsc --noEmit` + `npm run build` pass; WASM bridge
  exercised end-to-end in Node.

## Resolved (were follow-ups)

- **onLoad name alignment — FIXED.** The compiled `@load` fn is `<Scene>OnLoad`,
  but the committed configs named it `enterMap` / `scriptDefault` / camelCase
  `<scene>OnLoad`, so the runtime's `has_function(onLoad)` lookup missed it and
  the map-entry script silently never fired. This actually broke the early-game
  story maps. The 5 maps with real `@load` logic — **PalletTown, OaksLab,
  ViridianMart, BluesHouse, ViridianCity** — now have `onLoad` set to
  `<Scene>OnLoad`, and `scene_onload_resolves_at_runtime` guards every
  non-empty-`@load` map against re-regression. (The ~240 maps with an empty
  `@load {}` keep the legacy `enterMap` name harmlessly — it resolves to a
  no-op either way.)
- **Snapshot tests now real — FIXED.** `.gitignore`'s save-state pattern
  `*.sn*` was also matching `*.snap`, so no `insta` reference snapshot was ever
  committed (the tests couldn't gate anything in CI). Added `!*.snap` to
  re-include the references (pending `*.snap.new` stays ignored), refreshed the
  3 stale snapshots to current codegen (`storyline_main`, `let gold`), and
  committed all 8 references. `cargo test -p jrpg-engine-dsl` is green.
- **Condition/argument calls were not namespaced — FIXED.** A call inside an
  `@if` condition or a command argument (`getFlag(...)`) compiled to a *bare*
  `getFlag(...)`. Since all APIs live on the `game` object (no bare globals),
  that throws `ReferenceError` at runtime — and the engine silently swallows
  the rejection, so the storyline just quietly does nothing. This made
  first-class `@if (getFlag(...))` non-functional, which is *why* PalletTown
  fell back to `@run`. `compile_expression` now namespaces call callees to
  `game.` (matching the `game["name"](...)` that statements already used);
  `test_condition_calls_are_game_namespaced` guards it.
- **PalletTown's `@run` anti-pattern — REMOVED.** PalletTown was the only map
  in the 249-map corpus using `@run` (7 blocks), an artifact of its hand
  refactor. With conditions now working, its 7 `@run` blocks were rewritten as
  first-class `@if` + bare commands. Verified behavior-preserving: the
  canonical operation stream (ordered `game.*` calls, `if`/`else` structure,
  string + hex-normalized numeric args) is byte-identical before/after. The
  corpus now contains **zero** `@run` blocks; PalletTown is the template for
  porting further story content.

## Known follow-ups

- **Member access in conditions (10 maps).** Conditions like
  `@if (getFlag(EVENT.BEAT_BROCK))` / `@if (hasAllButEarthBadge || …)` still
  reference un-inlined constants/getters (`EVENT.BEAT_BROCK`,
  `hasAllButEarthBadge`) that don't exist at runtime — a *separate* bug from the
  call-namespacing one (the converter should have inlined `EVENT.X` →
  `"EVENT_X"`, as it did in storyline bodies). The namespacing fix corrects the
  `getFlag` callee but not these arguments; these 10 maps' conditions remain
  broken until the constants are inlined.
- **Full behavioral ScriptCommand diff.** Bindings are round-trip-verified, the
  onLoad wiring is guarded, PalletTown's conversion is operation-stream-verified,
  and the only remaining body change elsewhere is the `@speaker` prefix; a
  per-map ScriptCommand-sequence diff against the original `.js.bak` would
  further harden the migration.

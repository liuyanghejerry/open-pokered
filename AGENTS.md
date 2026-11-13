# AGENTS.md

## Project

A portable Rust reimplementation of Pokémon Red/Blue game logic. The generic
JRPG engine (`dotzuki-*`) lives in a separate repository, consumed through pinned
Cargo Git dependencies. Make generic engine changes there.

## Code navigation

If `.codegraph/` exists, use `codegraph_explore` or
`codegraph explore "<symbol names or question>"` before searching or reading code.
If both are unavailable, state that and use `rg` and direct reads.

## Development workflows

The repository root is the main Cargo workspace; `crates/pokered-gba` is a
standalone workspace. Fetch graphics assets before building.

See the [development guide](docs/development.md):

- [Workspace and assets](docs/development.md#workspace-location)
- [Builds and tests: native, WASM, GBA](docs/development.md#common-commands)
- [Debugging, snapshots, screenshots, TCP control](docs/development.md#debug-cli)
- [Agent experiments and autonomous playthroughs](docs/development.md#agent-experiments)
- [Editor, publishing, data verification](docs/development.md#tooling-outside-cargo)

## Architecture and data

- **Text:** use Fusion Pixel fonts on every frontend and in diagnostic captures;
  account for font metrics in layouts. Original font tiles, PK/MN ligatures,
  and naming underscores must not render text. Non-text graphics are exempt.
  See [font policy](crates/pokered-renderer/fonts/POKERED.md) and [font notices](crates/pokered-renderer/fonts/NOTICE.md).
- **Shared runtime:** frontends, including TUI, share `pokered-app::PokemonGame`.
  Keep game flow and rendering fixes there. `RenderSession` owns caches and
  `Reuse` / `Full` / `Damage` decisions; hosts handle platform presentation.
  Incremental rendering must match full rendering.
- **Core:** keep `pokered-core` deterministic and free of I/O, GPU, and platform
  calls. Rendering, audio output, and windowing belong outside core.
- **Battles:** production uses `StackDriver`, adapted in
  `pokered-core/src/battle/pokered_rules/runtime.rs`. Legacy battle code is a
  test oracle; do not add behavior there.
- **Layouts:** prefer v2 (`pokered_ui::v2`) for new screens. Static/GBA and
  dynamic/editor rendering share `.gui` definitions in
  `crates/pokered-data/ui_layouts/`, including cursor positions and glyphs.
  Keep runtime bindings semantic, without duplicated layout coordinates.
- **Scripts:** map `script.scene` files are authoritative. Use the native AST
  interpreter by default; `script-boa` is a development/fallback feature.
  `script.js.bak` files are historical backups.
- **Generated data:** edit map JSON, `.scene`, and `.gui` sources, then regenerate
  through `pokered-data/build.rs`; do not hand-edit generated tables, ASTs, or layouts.
- **Translations:** use `@t("english", "中文")` and preserve English text
  byte-for-byte. Check with `scripts/verify_scene_translations.py` against HEAD.
- **Fidelity:** preserve deliberate Gen I bugs, including Focus Energy's reduced
  crit rate and the 1/256 miss. Consult core tests before changing behavior.
- **Saves:** initialize boot/CONTINUE from the loaded `SaveData` and
  `SaveFileSummary`. Verify persistence in an independent process.

## Cross-platform constraints

- Keep all `dotzuki-*` revisions synchronized across the main and GBA workspaces;
  update both lockfiles. Manifests and lockfiles define the current revision.
  Do not commit local path overrides.
- Preserve shared crates' `no_std` support; gate hosted dependencies and I/O.
  Use `pokered-platform` primitives. Bare-metal synchronization requires a
  single-threaded executor, no interrupt access, and no recursive locking or
  initialization. See [platform contracts](crates/pokered-platform/src/lib.rs).
- Preserve GBA performance and memory budgets. Change the pinned toolchain or
  reviewed baseline only with measurements and rationale, never to bypass a
  failing gate. Diagnostic ROM features are not shipping defaults. Follow
  [GBA CI](.github/workflows/gba-performance.yml) and
  [performance guidance](docs/gba-performance.md).

## Validation and evidence

- **Generated audit output:** do not commit raw frame recordings, compressed logs,
  run reports, or checksum inventories. Keep them in `.task-tmp/` or outside the
  repository, then delete them after validation. Retain concise findings, replay
  generators, necessary regression-test fixtures, and required before/after
  screenshots. Do not use `git add -f` to bypass these artifact exclusions.

- Select coverage using [playthrough-regression](.agents/skills/playthrough-regression/SKILL.md):
  crate tests, affected subsystem scenarios/BDD, and progression playthroughs.
  Report tested targets, features, commands, and limits. Host checks do not
  validate WASM/GBA; per-crate passes do not resolve workspace feature failures.
- A fresh full clear requires NEW GAME, Hall of Fame, credits, ending autosave,
  and CONTINUE in a separate process. Label seeded, assisted, and resumed runs.
- For timing/audio changes, retain original-source provenance and reproducible
  trace/fixture generators. Unit traces or cycle totals alone do not prove
  runtime, animation, or physical-audio equivalence. State remaining gaps;
  see this [scoped evidence example](docs/audits/hof-pc-load/audio-live-bus-integration.md).

## Project skills

Use relevant skills in `.agents/skills/`; `.claude/skills` links to that directory.

- [pokered-debug](.agents/skills/pokered-debug/SKILL.md) — CLI and TCP debugging.
- [key-animation-differential](.agents/skills/key-animation-differential/SKILL.md) — original/current animation comparisons.
- [typesafe-ai](.agents/skills/typesafe-ai/SKILL.md) — typed semantic judgments.
- [pokered-save-editor](.agents/skills/pokered-save-editor/SKILL.md) — snapshots and save editing.
- [visual-verify](.agents/skills/visual-verify/SKILL.md) — healing-machine rendering and OAM coordinates.
- [playthrough-regression](.agents/skills/playthrough-regression/SKILL.md) — regression coverage and checkpoint semantics.

## Visual PR evidence

Every PR changing on-screen output requires matched **before/after screenshots**.
This covers layouts, fonts/assets, palettes, rendering, animations, and transitions;
changes without on-screen effects are exempt.

Commit captures under `docs/screenshots/`, label them `前` / `后`, and embed
absolute raw URLs pinned to the PR branch. Explain any surface that cannot be
captured headless. Follow the [capture and embedding procedure](docs/development.md#visual-pr-evidence).

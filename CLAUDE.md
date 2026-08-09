# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

A native **Rust reimplementation** of Pokémon Red/Blue. It is *not* a Game Boy emulator and *not* byte-identical to the original ROM — it reproduces the game *logic* (battles, overworld, events, menus, audio) as portable Rust.

This repo is **game-only**: the generic **JRPG engine** (the `jrpg-*` crates) lives in a separate repository and is consumed here as a **Cargo git dependency** (see `Cargo.toml` — `jrpg-engine`, `jrpg-engine-dsl`, `jrpg-renderer`, `jrpg-app`, etc. are `{ git = ..., tag = "v0.1.0" }` deps). To iterate on engine code, work in the engine repo and bump the tag + `cargo update` here.

The original RGBDS assembly disassembly has been **removed**. `ANALYSIS.md` at the repo root is kept as a historical reference to the original assembly codebase.

## Workspace location — read this first

The Cargo workspace root is the **repo root**. Run every `cargo` command from there:

```bash
cd /path/to/open-pokered
```

The repo layout:

```
Cargo.toml                 # Workspace root (members: pokered crates + scene_apply bin)
crates/
├── pokered-data/          # All static data; maps/<MapName>/ holds map.json, map.blk,
│                          #   script.scene (+ script_config.json), ui_layouts/*.gui
├── pokered-core/          # Pure game logic, NO I/O (battle/, overworld/, events/,
│                          #   items/, pokemon/, save/, link/, slots/, screen state
│                          #   machines: title, oak_speech, main_menu, naming_screen…)
├── pokered-renderer/      # 160×144 framebuffer rendering, battle anims, fonts
├── pokered-ui/            # UI engine + menus (pluggable Painter backends)
├── pokered-audio/         # Game Boy APU emulation (4 channels), sequencer, SFX
├── pokered-app/           # ★ The native binary `pokered-app` + debug CLI
├── pokered-tui/           # Terminal UI frontend (same pokered-ui code)
├── pokered-ui-preview/    # WASM shim for pokered-editor WYSIWYG layout preview
├── pokered-layout-preview/# WASM layout preview for the editor (mock data + custom:hp_bar + DSL compile bridge)
├── pokered-web/           # Full game for WASM/browser (wgpu/pixels)
├── pokered-runner-web/    # Headless WASM bridge for the editor's Play activity
├── pokered-debug-server/  # TCP debug server (JSON-line protocol); `debug-server` feature
├── pokered-android/       # Android shell (cdylib, winit + JNI)
├── pokered-ios/           # iOS shell (staticlib)
└── scene_apply/           # Story-translation helper: .scene → script_config.json
tools/pokered-editor/      # Pokémon-specific Vue/Vite editor suite + Electron shell
scripts/                   # Python data-extraction/verification helpers + fetch-gfx.sh
docs/                      # Reference notes (NPC dialogue transcripts, move anims, fidelity)
```

The game's graphics live at `gfx/` (PNG/2bpp asset dumps from the original game, consumed by conversion tools, embedded at build time, and loaded at runtime). This directory is **not committed** — it is a byte-for-byte copy of [`pret/pokered`](https://github.com/pret/pokered)'s `gfx/` tree, fetched on demand and gitignored. **Run `scripts/fetch-gfx.sh` once after cloning** — it is required before *any* build, because `pokered-data` embeds `gfx/blocksets/*.bst` via `include_bytes!` (a missing `gfx/` fails compilation, not just wasm/android/ios packaging). See `docs/gfx-assets.md`.

## Architecture notes

Key invariants — preserve these when editing:

- **`pokered-core` is pure logic with no I/O, no GPU, no platform calls.** Rendering, audio output, and windowing live in `pokered-renderer` / `pokered-audio` / `pokered-app`. Keep `core` deterministic and testable.
- **Battles run on the generic effect-stack engine** (`jrpg_engine::battle::stack::StackDriver`, consumed via the git dep); the production glue is `pokered-core/src/battle/pokered_rules/runtime.rs`. The legacy battle code remains only as a test-side parity oracle. Don't add new battle behavior to the legacy path.
- **Menus are mid-migration from v1 to v2.** pokered screens are moving from hardcoded v1 layouts in `pokered-ui` onto the v2 layout engine (`jrpg-renderer`'s `layout_engine`, driven by the `.gui` layouts in `pokered-data/ui_layouts/`, bridged via `pokered_ui::v2`). Main menu, battle main menu, options, and stats page 1 are already live on v2; shared custom elements (e.g. `custom:hp_bar`) are declared in `ui_layouts/components.gui` and registered game-side. Prefer v2 for new screen work.
- **Map scripts are `.scene` DSL, compiled to JS.** Each map dir has `script.scene` (the source of truth; old `script.js.bak` files are historical backups). `jrpg-engine-dsl` (git dep) compiles `.scene` → JavaScript, which runs on the Boa-based `jrpg-engine-script` engine. **Script loading:** `pokered-data`'s `build.rs` compiles every `script.scene` at build time and embeds the JS plus each `script_config.json` (`pokered_data::embedded_scenes`); `OverworldScreen` registers those tables by default, so all frontends (native, TUI, wasm) work with no flags. Passing `--scripts-dir <maps dir>` switches to disk loading with hot reload for dev.
- **Bilingual text** is supported everywhere via `@t("english", "中文")`; `scripts/verify_scene_translations.py` checks every `maps/*/script.scene` preserves the English text byte-for-byte against git HEAD.

## Common commands

```bash
# Build / run the game (native)
cargo run --release --bin pokered-app          # release recommended; debug is much slower

# Tests
cargo test                                      # whole workspace
cargo test -p pokered-core                      # one crate
cargo test test_damage_calculation              # one test by name
cargo test -- --nocapture                       # show println! output
# NOTE: a workspace-wide run unifies cargo features and can fail feature-gated
# suites that pass per-crate — re-run `cargo test -p <crate>` before assuming
# a real failure.

# Benchmarks (Criterion)
cargo bench -p pokered-core

# WASM
rustup target add wasm32-unknown-unknown        # once
cargo install cargo-run-wasm                    # once
cargo run-wasm -p pokered-web                   # build + serve in browser
```

## Debug CLI (`pokered-app` subcommands)

`pokered-app` is also a headless test/debug harness. This is the fastest way to reach a specific game state without playing through it:

```bash
# Skip intro and warp straight to a map (PascalCase map names, optional x,y)
cargo run --release --bin pokered-app -- run --skip-intro --warp CeruleanCity,14,8

# Load a save (.sav) or JSON snapshot
cargo run --release --bin pokered-app -- run --save pokered.sav --warp PalletTown,10,14
cargo run --release --bin pokered-app -- run --snapshot snapshot.json --skip-intro

# Save <-> JSON snapshot round-trip
cargo run --release --bin pokered-app -- export-snapshot --input pokered.sav -o snapshot.json
cargo run --release --bin pokered-app -- import-snapshot --input pokered.sav

# Headless capture / inspection (no window)
cargo run --release --bin pokered-app -- screenshot --screen battle -o out.png -f 10
cargo run --release --bin pokered-app -- screenshot-all -o screenshots/
cargo run --release --bin pokered-app -- dump-state --screen overworld -f 30   # JSON to stdout
cargo run --release --bin pokered-app -- battle --config sample_battle.json -s result.png

# Per-module debug logging (writes pokered-debug.log)
cargo run --release --bin pokered-app -- --debug-modules save,overworld,battle run
# modules: save overworld battle menu audio warp event render all

# TCP debug server (requires the `debug-server` feature)
cargo run --release --bin pokered-app --features debug-server -- run --debug-port 9000

# Headless mode (no window): for CI + scripted driving. Combine with the
# debug server; the `step_frames` command gives synchronous, deterministic
# frame control (unlike `run_frames`, which only schedules on the real-time loop).
cargo run --release --bin pokered-app --features debug-server -- run --headless --debug-port 9000 --skip-intro --warp PalletTown,10,5
```

Debug-server protocol (JSON-line over TCP): `get_state`, `get_position`,
`get_party`, `get_bag`, `get_flags`, `get_npcs`, `warp`, `press`,
`press_sequence`, `run_frames`, `step_frames`, `save`, `set_flag`,
`give_item`, `give_pokemon`, `start_wild_battle`. `get_state` also reports
`active_script_effect`, `script_awaiting_battle`, `player_movement_state`,
`dialogue`, and battle phase/message. A minimal Python client lives at
`scripts/debug_drive.py`.

Screen targets: `copyright title main-menu oak overworld battle start-menu options save`.

## Project skills

This repo ships Claude Code skills under `.claude/skills/` — invoke them when relevant:

- **pokered-debug** — full reference for the debug CLI above.
- **pokered-save-editor** — JSON snapshot format + the Save Editor GUI.
- **visual-verify** — render-correctness verification (e.g. the Pokémon Center heal machine); OAM→screen coordinate reference.

## Tooling outside Cargo

- `tools/pokered-editor/` — Pokémon-specific Vue 3/Vite editor suite (`pnpm install && pnpm dev`, http://localhost:5173): map editor, save editor, trainer/Pokémon/move data editors, UI layout editor (WASM-backed WYSIWYG preview, understands `.gui` DSL), map script editor, pixel editor. Also includes an **AI assistant** (chat with read/propose tools, reviewable change proposals, scene/gui/data generation, AI sprite generation) and an **Electron shell** (`pnpm electron:dev`, `pnpm electron:pack`).
  - The layout-preview WASM bridge is `jrpg-web` (engine crate) — not in this repo. `pnpm build:wasm` skips it when absent; supply the pkg via `JRPG_WASM_ROOT` or a checkout of the engine repo.
- `tools/asm2music.py` / `tools/asm2sfx.py` — convert pokered `audio/music|sfx/*.asm` to Rust byte tables.
- `tools/dsl_migration/` — historical scripts that converted legacy `script.js` map scripts to `.scene` DSL.
- `scripts/verify_battle_anim_data.py` / `verify_move_sfx_data.py` / `verify_cry_data.py` — byte-exact auditors that diff the battle-animation tables, the move SFX table, and the cry table against a local pret/pokered disassembly checkout (path is a CLI arg). Run them after touching any of these data files; all must report 0 diffs.

## Fidelity notes

The rewrite intentionally reproduces original Gen I mechanics *including known bugs* (e.g. Focus Energy reducing crit rate, 1/256 miss). Don't "fix" these when they appear deliberate — match original behavior, and check `pokered-core` tests, which encode expected outcomes.

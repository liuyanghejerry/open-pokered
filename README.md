# Pokémon Red/Blue — Rust Reimplementation

A faithful reimplementation of Pokémon Red and Blue in Rust, based on the [pret/pokered](https://github.com/pret/pokered) disassembly. This repo is **game-only**: the generic **JRPG engine** lives in a separate repository and is consumed here as a Cargo **git dependency** (`dotzuki-engine`, `dotzuki-engine-dsl`, `dotzuki-engine-script`, `dotzuki-rules`, `dotzuki-renderer`, `dotzuki-ui`, `dotzuki-audio`, `dotzuki-app`, `dotzuki-tui` — see `Cargo.toml`, all pinned to a `v0.1.0` tag of the engine repo).

## Getting started

```bash
# 1. Fetch the game graphics (NOT committed; REQUIRED for any build)
scripts/fetch-gfx.sh

# 2. Build / run
cargo run --release --bin pokered-app

# 3. Test
cargo test
```

The engine is consumed as a Cargo git dependency: `ssh://git@github.com/liuyanghejerry/dotzuki.git`, pinned to tag `v0.1.0` (see the `Cargo.toml` of each crate). After pulling a new engine tag, bump the `tag` and run `cargo update`.

## What this repo contains

- `crates/` — the game itself (`pokered-data` (248 maps, species/move/item tables), `pokered-core` (pure logic), `pokered-renderer`, `pokered-ui`, `pokered-audio` (GB APU emulation), `pokered-app` (native binary + debug CLI), `pokered-tui`, `pokered-ui-preview`) plus platform shells: `pokered-web` (WASM), `pokered-runner-web` (editor Play bridge), `pokered-layout-preview` (editor layout-preview WASM: per-menu mock data, `custom:hp_bar`, DSL compile bridge), `pokered-debug-server`, `pokered-android`, `pokered-ios`, plus `scene_apply` (story-translation helper bin)
- `tools/pokered-editor/` — the Vue/Vite editor suite (maps, saves, data, UI layouts, AI assistant) + Electron shell
- `android/` / `ios/` — mobile build projects
- `scripts/` — Python data-extraction/verification helpers
- `docs/` — NPC dialogue transcripts, move animation data, fidelity audits

## Not included

- The **jrpg engine** — separate repo, consumed via git dependency.
- The game **gfx assets** — fetched from pret/pokered by `scripts/fetch-gfx.sh` (copyrighted; not redistributed).

See `CLAUDE.md` for the full developer guide (build commands, debug CLI, skills).

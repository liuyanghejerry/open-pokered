# Pokémon Red/Blue — Rust Reimplementation

<p align="center">
  <img src="docs/screenshots/readme/title.png" alt="Title screen" width="480">
</p>

<p align="center">
  <a href="https://liuyanghejerry.github.io/open-pokered/"><strong>▶ Play the web version</strong></a>
  ·
  <a href="https://liuyanghejerry.github.io/open-pokered/editor/"><strong>Open the web editor</strong></a>
</p>

<p align="center">
  No installation required — play the full bilingual game or explore the editor directly in your browser.
</p>

A faithful reimplementation of **Pokémon Red and Blue** in **Rust**, built from the [pret/pokered](https://github.com/pret/pokered) disassembly — playable on **desktop, web, Android, and iOS** from a single codebase, fully playable in **English and 中文**, and shipping with a full **visual editor suite** for maps, data, UI layouts, and saves.

This repo is **game-only**: the generic **JRPG engine** lives in a separate repository and is consumed here as a Cargo **git dependency** (`dotzuki-engine`, `dotzuki-engine-dsl`, `dotzuki-engine-script`, `dotzuki-rules`, `dotzuki-renderer`, `dotzuki-ui`, `dotzuki-audio`, `dotzuki-app`, `dotzuki-tui` — see `crates/*/Cargo.toml`, all pinned to the `v0.6.0` tag of the engine repo).

## Try it in your browser

**Play now: [https://liuyanghejerry.github.io/open-pokered/](https://liuyanghejerry.github.io/open-pokered/)**

The web version is the complete game compiled to WebAssembly. It runs in a modern desktop or mobile browser, supports both English and 中文, and keeps your save in browser storage so you can continue later on the same device.

Want to inspect or modify the game instead? The visual editor is also available online:

**Open the editor: [https://liuyanghejerry.github.io/open-pokered/editor/](https://liuyanghejerry.github.io/open-pokered/editor/)**

Use it to browse maps, Pokémon data, trainers, moves, UI layouts, and live previews without setting up a local development environment.

## Screenshots

<p align="center">
  <img src="docs/screenshots/readme/overworld.png" alt="Overworld" width="360">
  <img src="docs/screenshots/readme/battle.png" alt="Battle" width="360">
</p>
<p align="center">
  <img src="docs/screenshots/readme/town-map.png" alt="Town map" width="360">
  <img src="docs/screenshots/readme/oak.png" alt="Professor Oak" width="360">
</p>

### 中文截图

The whole game is bilingual (`@t("english", "中文")` everywhere) — here is the same tour in Chinese:

<p align="center">
  <img src="docs/screenshots/readme/zh/title.png" alt="标题画面（中文）" width="360">
  <img src="docs/screenshots/readme/zh/overworld.png" alt="地图场景（中文）" width="360">
</p>
<p align="center">
  <img src="docs/screenshots/readme/zh/battle.png" alt="战斗画面（中文）" width="360">
  <img src="docs/screenshots/readme/zh/oak.png" alt="大木博士（中文）" width="360">
</p>

Screenshots are regenerated with `scripts/capture_readme_screenshots.sh` (headless captures via `pokered-app screenshot --lang en|zh`, with enough frames for each screen to finish loading).

## Highlights

- **Faithful to the original** — all 248 maps, 151 species, moves, items, and game logic rebuilt from the disassembly, with ongoing fidelity audits (`docs/FIDELITY_GAPS.md`)
- **Playable instantly** — a full WebAssembly build runs directly in the browser
- **Cross-platform** — native desktop app, web, Android, and iOS from one codebase
- **Bilingual** — every screen and line of dialogue in English and 中文, switchable in-game
- **Authentic audio** — Game Boy APU emulation (`pokered-audio`)
- **Hackable by design** — per-map `.scene` event scripts on a native DSL interpreter, a DSL for UI layouts, and JSON-driven game data
- **Full editor suite** — map editor, Pokémon/move/trainer data editors, UI layout editor with live preview, save editor, sprite tools, and an AI assistant

## Built for hacking — and for AI collaboration

Splitting the game from the engine is deliberate. Everything that makes the game *inspectable*, *drivable*, and *rebuildable* lives in data rather than in Rust — which opens the whole game up to customization, and makes it unusually approachable for AI agents: no hidden state, no realtime loop you can't control, no behavior you can't trace back to a file.

### See and drive the running game

A JSON-line TCP debug protocol (`run --headless --debug-port 9000`, `debug-server` feature) exposes the entire running game: `get_state`, `get_position`, `get_party`, `get_bag`, `get_flags`, `get_npcs` to *see* (state includes the active script effect, the current dialogue text, and the battle phase/message), and `warp`, `press`, `press_sequence`, `set_flag`, `give_item`, `give_pokemon`, `start_wild_battle`, `save` to *act*. `step_frames` advances the game synchronously — deterministic, frame-exact control with no realtime flakiness. `scripts/debug_drive.py` is a minimal client:

```python
from debug_drive import DebugClient

d = DebugClient(9000)
d.cmd(cmd="warp", map="PalletTown", x=10, y=5)
d.cmd(cmd="press_sequence", buttons=["up"] * 40)
d.cmd(cmd="step_frames", count=40)      # returns when the frames are done
print(d.cmd(cmd="get_state")["data"])   # position, party, flags, dialogue, ...
```

### Machine-readable state in, verifiable pixels out

`screenshot`, `screenshot-all`, `dump-state` and `--headless` mode render any screen without a window — every screenshot on this page is produced this way (`scripts/capture_readme_screenshots.sh`). This repo's own PR policy (`AGENTS.md`) requires before/after captures for any visual change: the capture loop isn't a demo, it's the development workflow.

### Content is data, not code

- **Maps** — all 248 maps are JSON (`crates/pokered-data/maps/`): warps, NPCs, signs, wild encounters, map connections, per-map text
- **Game data** — species, moves and trainers are JSON files too (`pokemon/`, `moves/`, `trainers/`)
- **Behavior** — per-event logic is a `.scene` DSL (`crates/pokered-data/maps/*/script.scene`), compiled to an AST at build time and run on the engine's native interpreter, with bilingual dialogue inline (`@t("english", "中文")`). A Boa JS engine remains as a dev fallback behind the `script-boa` feature
- **Battles** — a battle is a JSON config (`sample_battle.json`): parties, levels, movesets, trainer class

A romhack — new maps, new NPCs, re-scripted events, rebalanced data — is a directory of file edits, not an engine fork.

### UI is a DSL, not renderer code

Menus and screens are declared in a `.gui` layout DSL (28 files in `crates/pokered-data/ui_layouts/`, reference in `docs/GAME_UI_DSL.md`) with a compile bridge and live preview in the editor — excerpt from `bag.gui`:

```gui
screen Bag {
  text(@t("ITEM", "道具")) { rect = {tx: 7, ty: 1, tw: 6, th: 1} }
  flex_list("{bag_items}") {
    rect = {tx: 1, ty: 4, tw: 18, th: 13}
    cursor = {tile: 223, position: "left"}
  }
}
```

### Why AI agents work well here

Read state → drive inputs deterministically → verify the pixels. That loop maps one-to-one onto an agent's working cycle, and the repo ships agent-facing tooling built on it (debug and visual-verification skills under `.claude/skills/`). Fidelity audits (`docs/FIDELITY_GAPS.md`) and the screenshots above are products of the same loop: features get implemented, driven, and visually verified end to end — by humans or by agents.

## Editor Suite

**Try it online: [https://liuyanghejerry.github.io/open-pokered/editor/](https://liuyanghejerry.github.io/open-pokered/editor/)**

`tools/pokered-editor/` is a Vue/Vite application (with an Electron shell) for editing every aspect of the game. The hosted web version is ideal for exploring the data and layouts; local and Electron builds are available for the complete development workflow. Edits made on the hosted version are stored in your browser, and the **🚀 Publish game** action bundles the game engine plus all your edits into a single self-contained HTML file — download it and anyone can play your hack in a browser, no installation required.

<p align="center">
  <img src="docs/screenshots/readme/editor-map.png" alt="Map editor" width="720">
</p>
<p align="center">
  <img src="docs/screenshots/readme/editor-pokemon.png" alt="Pokémon editor" width="720">
</p>
<p align="center">
  <img src="docs/screenshots/readme/editor-layout.png" alt="Layout editor" width="720">
</p>
<p align="center">
  <img src="docs/screenshots/readme/editor-playtest.png" alt="Live playtest inside the editor" width="720">
</p>

```bash
cd tools/pokered-editor
pnpm install
pnpm dev        # or: pnpm electron:dev
```

## Getting started

If you only want to play or explore, use the hosted [game](https://liuyanghejerry.github.io/open-pokered/) and [editor](https://liuyanghejerry.github.io/open-pokered/editor/). To build locally:

```bash
# 1. Fetch the game graphics (NOT committed; REQUIRED for any build)
scripts/fetch-gfx.sh

# 2. Build / run
cargo run --release --bin pokered-app

# 3. Test
cargo test
```

The engine is consumed as a Cargo git dependency: `https://github.com/liuyanghejerry/dotzuki`, pinned to tag `v0.6.0` (see the `Cargo.toml` of each crate). After pulling a new engine tag, bump the `tag` and run `cargo update`.

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

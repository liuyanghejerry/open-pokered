# Running pokered-app

How to launch the game and where it looks for its resource (asset) directories.

All commands are run from the workspace root (`workspace/`).

## Launching

### Via Cargo (recommended for development)

```bash
# Windowed game, full intro
cargo run -p pokered-app -- run

# Release build (smooth framerate)
cargo run --release -p pokered-app -- run

# Skip the intro and warp straight to a map (PascalCase map name, optional x,y)
cargo run -p pokered-app -- run --skip-intro --warp PalletTown
cargo run -p pokered-app -- run --skip-intro --warp CeruleanCity,14,8

# Load a save or a JSON snapshot
cargo run -p pokered-app -- run --save pokered.sav --warp PalletTown,10,14
cargo run -p pokered-app -- run --snapshot snapshot.json --skip-intro
```

### Via the built binary directly

```bash
cargo build -p pokered-app                       # or --release
./target/debug/pokered-app run --skip-intro --warp PalletTown
```

> The binary works from **any** working directory — see [Resource directories](#resource-directories)
> for how it locates `gfx/` and the map data.

### Headless / debug subcommands

These never open a window:

```bash
# Capture a screen to PNG
./target/debug/pokered-app screenshot --screen overworld -o out.png -f 10
./target/debug/pokered-app screenshot-all -o screenshots/

# Dump game state as JSON
./target/debug/pokered-app dump-state --screen overworld -f 0

# Save <-> JSON snapshot
./target/debug/pokered-app export-snapshot --input pokered.sav -o snapshot.json

# Run a configured battle (optionally to a PNG instead of a window)
./target/debug/pokered-app battle --config sample_battle.json -s result.png
```

### Useful global flags

| Flag | Purpose |
|------|---------|
| `--debug-modules save,overworld,battle` | Per-module debug logging → `pokered-debug.log`. Modules: `save overworld battle menu audio warp event render all` |
| `--scripts-dir <PATH>` | Directory of per-map script folders (`script.js` / `script_config.json`). Defaults to the bundled map data. Only used without the `embedded-scripts` feature. |
| `--demo` | Run the jrpg-engine multi-layer demo map instead of Pokémon Red. |
| `--watch` | Hot-reload `.tmx` / `.png` / `.js` under `assets/` (debug builds). |

## Resource directories

The game needs two resource directories at runtime (unless built with the
embedded features below):

| Resource | Default location | Contents |
|----------|------------------|----------|
| **Graphics** (`gfx/`) | `examples/pokered/gfx/` (sibling of the example's `crates/`) | tilesets, sprites, fonts, … |
| **Map data** (`maps/`) | `examples/pokered/crates/pokered-data/maps/` | per-map `map.json` / `map.blk` / `script.js` |

> **First-time setup (required):** `gfx/` is not committed to this repo — it is
> fetched from [`pret/pokered`](https://github.com/pret/pokered) (pinned commit).
> Run `scripts/fetch-gfx.sh` from the worktree root once after cloning. This is
> required before *any* build: `pokered-data` `include_bytes!`s
> `gfx/blocksets/*.bst`, so a missing `gfx/` fails compilation (not just
> wasm/android/ios packaging, which also embed `gfx/` at build time).
> See `docs/gfx-assets.md`.

### How they are located (in order)

**Graphics — `AssetRoot::auto_detect()`**
1. `POKERED_GFX_DIR` environment variable, if set and a valid directory.
2. `examples/pokered/gfx/` baked relative to the crate manifest
   (`CARGO_MANIFEST_DIR/../../gfx`) — resolves for a locally-built binary from
   any working directory.
3. `gfx/` in the current working directory, then walking **up to 5 parent
   directories** from the cwd.
4. `gfx/` next to the executable.

**Map data — `find_maps_directory()`** (filesystem mode only)
1. `POKERED_MAPS_DIR` environment variable, if set and a valid directory.
2. A set of cwd-relative candidates (`examples/pokered/crates/pokered-data/maps`,
   `crates/pokered-data/maps`, `maps`, …).
3. The compile-time crate manifest path (`CARGO_MANIFEST_DIR/maps`) — baked in at
   build time, so it resolves even when the binary is launched directly.

If graphics cannot be found, the overworld falls back to **text-only placeholder
rendering**. If map data cannot be found, map loading panics with
`No map.json found for <Map>`.

### Specifying the resource directories explicitly

Point the game at any location with environment variables. This is the most
reliable option when launching the binary from an unrelated directory or from a
packaged build:

```bash
POKERED_GFX_DIR=/path/to/gfx \
POKERED_MAPS_DIR=/path/to/maps \
./target/debug/pokered-app run --skip-intro --warp PalletTown
```

Either variable may be set independently; an unset (or invalid) variable simply
falls back to auto-detection.

### Embedding resources into the binary

To build a self-contained binary that needs no external directories, enable the
embedding features (heavier compile, no runtime lookup):

```bash
cargo run -p pokered-app --features embedded-map-data,embedded-scripts -- run
```

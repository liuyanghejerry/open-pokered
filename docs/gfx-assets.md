# `gfx/` — Original-Game Graphics Assets (fetched from upstream)

The pokered example's `gfx/` directory (`workspace/examples/pokered/gfx/`,
a sibling of its `crates/`) holds the Pokémon Red/Blue graphics assets the Rust
project consumes (PNG sources plus a handful of `.bst`/`.tilemap`/`.rle` data
files). **These files are not committed to this repository.** They are a
byte-for-byte copy of the [`pret/pokered`](https://github.com/pret/pokered)
`gfx/` tree (verified: identical git blob SHAs for every shared file), so rather
than vendor ~2.7 MB of assets here we fetch them on demand.

## Getting the assets

Run the bootstrap script once after cloning (and again if you bump the pin):

```bash
scripts/fetch-gfx.sh            # populate gfx/ if missing (idempotent)
scripts/fetch-gfx.sh --force    # re-sync even if gfx/ already exists
```

It does a blobless, sparse, single-commit checkout of only pret/pokered's
`gfx/` subtree — it does **not** download the rest of that repo. The upstream
commit is pinned in the script (`PRET_PIN`) for reproducibility; bump it there
to pull newer upstream assets.

`gfx/` is listed in the repo-root `.gitignore`, so the fetched files stay
untracked. Compiled artifacts (`*.2bpp`/`*.1bpp`/`*.pic`) are likewise ignored.

## How the assets are consumed

They are read:

- at **compile time, unconditionally**, by `pokered-data`, which
  `include_bytes!`s `gfx/blocksets/*.bst` (see `blockset_data.rs`). **This makes
  `gfx/` a hard build dependency: a missing `gfx/` fails `cargo build -p
  pokered-data` (and anything depending on it) — so run the fetch before *any*
  build, not just packaging.**
- at **runtime** on native targets, from disk, by `pokered-renderer`'s
  `ResourceManager` (`AssetRoot::auto_detect()` finds `gfx/` via
  `POKERED_GFX_DIR`, the path baked relative to the crate manifest, a `gfx/` in
  or above the working dir, or next to the executable);
- at **build time** for embedded targets (wasm/android/ios), where
  `pokered-renderer/build.rs` `include_bytes!`s every PNG/tilemap into the
  binary (silently embedding nothing if `gfx/` is absent);
- optionally at build time by `pokered-data/build.rs` for custom tilesets
  (`gfx/blocksets/*.bst`, gated on `tileset_extras.json`);
- by the conversion tools and editors (`tools/asset-converter`,
  `tools/pokered-editor`, `tools/jrpg-editor`).

## Asset Subdirectories
| Subdirectory | Content |
|---|---|
| `pokemon/` | Front/back sprites per species (`front/`, `back/`, `front_rg/`) |
| `trainers/` | Trainer class portrait sprites |
| `sprites/` | Overworld character sprites (walking frames) |
| `tilesets/` | Map tileset graphics |
| `blocksets/` | Map block definitions (`.bst` files — 4×4 tile blocks) |
| `overworld/` | Overworld UI elements (player, items, cut tree, etc.) |
| `emotes/` | Overworld emote bubbles (!, ?, heart, etc.) |
| `font/` | Text font tiles, badge letters |
| `icons/` | Party menu mon type icons |
| `title/` | Title screen graphics (Red/Blue variants) |
| `intro/` | Intro sequence frames (Nidorino/Gengar fight) |
| `splash/` | Game Freak splash screen |
| `battle/` | Battle UI elements (HP bar, status icons) |
| `pokedex/` | Pokédex screen graphics |
| `town_map/` | Town map tileset |
| `trade/` | Trade animation graphics |
| `trainer_card/` | Trainer card elements |
| `credits/` | Credits sequence graphics |
| `sgb/` | Super Game Boy border graphics |
| `slots/` | Slot machine graphics |
| `player/` | Player sprites (cycling, surfing, fishing frames) |

See `ANALYSIS.md` at the repo root for the historical assembly graphics pipeline.

# Third-Party Notices

This repository bundles or consumes third-party material. The repository's own
original code is licensed under MIT (see `LICENSE`); the items below are NOT
covered by that license and each is governed by its own terms.

## Game content derived from pret/pokered

The game text (map/NPC dialogue, menus), music/SFX data, and all map/overworld
data in `crates/pokered-data`, `crates/pokered-audio`, `crates/pokered-core`,
and `docs/` are derived from the disassembly
[pret/pokered](https://github.com/pret/pokered) of Pokémon Red/Blue. That
repository carries no license; the underlying content remains the copyright of
Nintendo / Game Freak. It is included here for preservation and educational
purposes only; the author of this repository has no authority to grant any
license to it.

Runtime graphics (`gfx/`) are **not** committed: they are fetched from
pret/pokered by `scripts/fetch-gfx.sh` (pinned to a specific upstream commit)
at build time. See `docs/gfx-assets.md`.

## Fusion Pixel Font

`crates/pokered-renderer/fonts/` bundles the Fusion Pixel Font (10px mono
`latin` / `zh_hans` variants):

- Author: TakWolf, Copyright (c) 2022
- License: SIL Open Font License 1.1 — see
  `crates/pokered-renderer/fonts/LICENSE-OFL` and
  `crates/pokered-renderer/fonts/NOTICE.md` (which also lists the upstream
  fonts the project merges from).

## dotzuki engine

The generic JRPG engine (`dotzuki-*` crates) is consumed as a Cargo git
dependency from https://github.com/liuyanghejerry/dotzuki, licensed under
Apache-2.0 / MIT at your option. See the engine repository for license files.

## Pokemon trademarks

"Pokémon", "Pokémon Red" and related names are trademarks of Nintendo. This
project is a non-commercial reimplementation for study and preservation; it is
not affiliated with, endorsed by, or connected to Nintendo or Game Freak.

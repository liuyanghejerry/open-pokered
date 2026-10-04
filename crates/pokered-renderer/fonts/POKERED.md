# Dedicated Pokémon UI tiles

Ordinary English and Chinese text uses the project's existing Fusion Pixel
font and metrics through `dotzuki_renderer::embedded_font`. There is no
original-game alphabet or numeric font bank in this directory.

These small UI graphics are row-major eight-byte 1bpp tiles extracted without
resizing from pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`:

| Embedded bitmap | Original PNG | Original Git blob | Tiles |
|---|---|---|---|
| `pokered-box-tiles.bin` (six tiles) | `gfx/font/font_extra.png` | `243acda0dcd16b45305d97a5bc7964d697b63c46` | `$79`–`$7E` border |
| `pokered-pkmn-tiles.bin` (two tiles) | `gfx/font/font.png` | `34e86b4a683cb214b92f71e462ed372b19bfb984` | `$E1/$E2` PK/MN menu graphic |
| `pokered-naming-underscores.bin` (two tiles) | `gfx/font/font_battle_extra.png` | `d00ae7bc377b0672d9823306409e579f09952093` | `$76/$77` naming slots |

Each set bit represents a black pixel, with bit7 at the left. The extracted
tiles preserve the original border, menu graphic and naming-slot fixes
without replacing the project's text glyphs. `home/text.asm:1–45` uses the
same horizontal tile for both edges and the same vertical tile for both
sides, including opaque white backgrounds. Naming first loads
`HpBarAndStatusGraphics` at `$62` (`engine/menus/naming_screen.asm:93`),
overriding the underscore tile IDs.

The shared data text measurement and pagination use Fusion Pixel's actual
glyph advances. Authored line breaks and page boundaries remain hard;
overlong rows wrap at the 144px interior before two-line pagination.

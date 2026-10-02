# Original Pokémon Red tile font

These files contain row-major eight-byte 1bpp tiles converted without resizing from pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`:

| Embedded bitmap | Original PNG | Original Git blob | VRAM first tile |
|---|---|---|---|
| `pokered-font.bin` | `gfx/font/font.png` (128×64) | `34e86b4a683cb214b92f71e462ed372b19bfb984` | `$80` |
| `pokered-font-extra.bin` | `gfx/font/font_extra.png` (128×16) | `243acda0dcd16b45305d97a5bc7964d697b63c46` | `$60` |
| `pokered-number-symbol.bin` (one tile only) | `gfx/font/font_battle_extra.png` | `d00ae7bc377b0672d9823306409e579f09952093` | `$74` (the status/Pokédex `№` glyph) |
| `pokered-naming-underscores.bin` (two tiles only) | `gfx/font/font_battle_extra.png` (120×16) | `d00ae7bc377b0672d9823306409e579f09952093` | `$76/$77` (source loaded at `$62`) |

The conversion visits tile rows, tile columns, then eight scanlines. Black PNG pixels become set bits with bit7 at the left. The original font/extra glyphs use black and white pixels, so these stencils preserve them exactly. The last file extracts only the black-and-white normal/raised naming underscores; `engine/menus/naming_screen.asm:93` first loads `HpBarAndStatusGraphics`, overriding those tile IDs.

Mapping follows `constants/charmap.asm` and `home/load_font.asm`. `home/text.asm:1–45` writes complete background tiles for its border: the same `$7A` horizontal tile at top and bottom, the same `$7C` vertical tile on both sides, opaque white tile backgrounds, and `$79/$7B/$7D/$7E` corners. Fusion Pixel remains the bitmap fallback for characters absent from the original map. Glyph width is chosen by character, independent of the selected UI language: original charmap characters and printable ASCII advance 8px; Chinese retains Fusion Pixel's 10px advance. Unsupported printable ASCII uses its Fusion bitmap in an 8px cell.

`pokered-data::text_layout` supplies this same measurement to the core's overworld/battle pagination and the UI's drawing wrappers. Authored newlines and page boundaries remain hard; an overlong row wraps at the 144px interior before two-line pagination, including after placeholder expansion. Latin uses eighteen original cells per row. Mixed Chinese/ASCII rows use the sum of actual glyph advances. Chinese stats ID/OT fields use 12px vertical spacing so their wider ASCII values do not collide with Chinese labels.

Scope: the renderer restores glyphs and eight-pixel English advances. It does not reinterpret plain Unicode apostrophe sequences as the ROM's optional compressed contraction tiles (`'d`, `'s`, etc.); those require text encoding/state-machine work. The raw glyph API still exposes every font tile, including PK/MN ligatures.

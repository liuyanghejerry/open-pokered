# Font policy and retained border graphics

All rendered text uses Fusion Pixel (SIL Open Font License 1.1; see
`NOTICE.md`), including English/Chinese menus, battle names/levels/status/HP,
party HP labels, badge numbers, PK/MN labels, naming underscores, version
labels and copyright text. Diagnostic captures and the GBA frontend follow
the same policy. Original-game letter and number tiles are never used for
text rendering; battle text tile cells are blanked before drawing Fusion
Pixel with its actual glyph advances.

The only original-game bitmap embedded in this directory is
`pokered-box-tiles.bin`: six non-text border tiles ($79–$7E), extracted without
resizing from pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`gfx/font/font_extra.png` (Git blob `243acda0dcd16b45305d97a5bc7964d697b63c46`).
Each tile is row-major eight-byte 1bpp, bit 7 at the left. Borders, HP bars,
icons, sprites and logo artwork remain graphics; they are not font glyphs.

Text measurement and pagination use Fusion Pixel's actual glyph advances.
Authored line breaks and page boundaries remain hard; overlong rows wrap
at the 144px interior before two-line pagination.

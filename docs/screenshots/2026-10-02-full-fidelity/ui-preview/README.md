# Battle Bag font-width regression

The same native `render_layout("battle_bag", "", 0, 0)` mock and frame 0 is captured before (`6fb9812`) and after the quantity fix (`04286e4`). The mock contains POTION×3, SUPER POTION×1 and ANTIDOTE×2, cursor 0. The 8px original font pushed SUPER POTION's quantity beyond the right border. The fix gives quantities their own right-aligned columns, measures the remaining name area in pixels, and marks shortened names with the original ellipsis tile.

| Before | After |
|---|---|
| ![Before](before-battle-bag.png) | ![After](after-battle-bag.png) |

[capture-manifest.json](capture-manifest.json) records the source/library/image hashes, common mock state, and all 14 reviewed current preview golden hashes. The capture uses the actual native package's serde-enabled dependency graph; standalone rustc omits only WASM export annotations. Thirteen unchanged render hashes match the actual Cargo preview test executable. The corrected Battle Bag receives its new hash.

Original assembly basis: `home/list_menu.asm:364` starts the name; `479–491` separately writes × followed by a two-digit `PrintNumber`. This fix retains the authored Battle Bag box and reserves its three rightmost interior columns (16–18), as the ordinary Bag renderer already does. It does not claim that this authored layout reproduces every original item-menu coordinate.

The production `FrameBufferPainter` pixel regression tests English/Chinese names, SUPER POTION/THUNDERSTONE and quantities 1/99. They match every pixel of the original ×/digit tiles, a clear tile separating the name, and the untouched original right-border bitmap. The regression fails on the earlier production row renderer and passes after the fix. The full 57 menu tests and the updated 58 native preview tests pass through raw-rustc checks without entering the shared Cargo queue.

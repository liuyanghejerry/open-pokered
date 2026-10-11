# Pokédex mixed-button navigation

The original first moves a visible row in HandleMenuInput, then processes watched buttons in HandlePokedexListMenu. UP/DOWN win over RIGHT/LEFT/A in that caller. For seen001–025, DOWN+A moves to row1 and scroll1 (selected003), staying in the list. Side-menu UP wins over DOWN even at the first item. One production file restores those priorities.

13 original emulator scenarios and 13 actual native host scenarios each repeat twice byte exactly. Native fixtures seed the public PokemonGame menu state, feed simultaneous InputState keys through actual game.update, and draw with the real Pokédex renderer. These are controlled menu fixtures, not TCP multi-key inputs, natural START inputs or full playthroughs. Master c6f40c2 and candidate use the identical helper and locked dependencies. The three DOWN/RIGHT/A single-key controls retain exact PNG bytes. Inventory remains stable.

Four before/after pairs cover DOWN+A, UP+RIGHT, A+RIGHT and an opposite-direction side-menu case. English text and UI artwork use the current master renderer on both sides. Fonts, Chinese, IME and dialogue layout are unchanged.

78 frozen app/core debug-enabled programs: 3691 passed, 0 failed, 76 ignored. Fresh m01–m03 and separate-process s07 SAVE/CONTINUE pass. The evidence archive contains deduplicated frames, metadata, scripts, source excerpts and full test logs, verified by byte-for-byte readback. It contains no ROM, SRAM, emulator state, private snapshot or executable. Reconstruct PNG paths using png-files.json and png-blobs.

Held repeat, initial guards, cry waits, entry/area input behavior and broader story/function fidelity remain under separate audit. No whole-game equivalence is claimed.

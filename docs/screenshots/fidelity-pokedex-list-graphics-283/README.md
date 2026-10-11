# Pokédex CONTENTS graphics fidelity

One production file restores the original two-line English rows, owned marker position, full-height divider tiles and hollow inactive list cursor. CONTENTS stays at its original (1,1). Existing font rendering and Chinese text coordinates stay unchanged.

Four before/after pairs show the initial list, moved cursor, side menu and return. Before uses the frozen tested PR172 binary with proved entire `crates` tree equality to actual master `7a6191e`; after uses this candidate's explicitly built debug CLI. These are controlled real START/POKEDEX inputs with seen001–025 and owned001/003/005, not a natural full playthrough. Original emulator and both native sides repeat twice byte exactly. Inventory remains stable and no automatic save is written.

Original `PrintNumber` advances the tile pointer from column1 to4; `dec hl` writes the owned marker at3, and the subsequent `inc hl` places the name at4. Title is (1,1), numbers are one tile above each name. `LoadPokedexTilePatterns` and `HandlePokedexListMenu` supply the UI tiles. Resource/fallback regression requires actual assets to load; existing list/side-menu cursor tests compare full frames to incremental repaint for both languages.

78 frozen app/core debug-enabled test programs: 3687 passed, 0 failed, 76 ignored. Fresh m01–m03 and separate-process s07 SAVE/CONTINUE also pass. `verification.json` records commits, scope and archive hash. `evidence.zip` contains byte-checked frames, metadata, source excerpts, scripts and logs; it contains no ROM, SRAM, emulator state, private snapshot or executable. PNG paths are reconstructed via `png-files.json` and deduplicated `png-blobs`.

Input guards, held repeat, combined-key handling and broader story/function fidelity remain under separate audit. No whole-game equivalence is claimed.

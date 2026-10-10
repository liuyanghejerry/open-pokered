# PC message playback and field text mode

PC messages now reveal letters at the configured speed, accelerate while A/B is held, and preserve authored paragraphs and caller-specific PROMPT/DONE/WAITBUTTON endings. The initial generic PC confirmation waits for its AB sound before displaying the menu. Rating text retains its bottom line at CONT, selects the original seven owned-count sound tiers and waits for the sound before the final acknowledgement. Shutdown waits for its sound; direct player/Bill PC exit clears the shared no-delay bit while generic main-menu logoff preserves it.

The owning runtime test follows the actual bicycle-shop B cancellation, then opens and closes the bedroom or generic PC through real input. Core tests cover list/confirmation mode writes, slow letters, held-button acceleration, protected confirmation, paragraph delay, automatic rating question, rating tier boundaries and shutdown with a busy audio backend. All final a7 native suites pass: core 2748, data 263, app 220 (33 ignored capture tests), agent 54.

Matched screenshots use actual PC entry +10 hardware frames, identical 151-frame post-entry input sequences and the same four-package default feature union. Both sides were recorded twice: each 152-PNG clip and its state JSON repeat byteexact. Master 31b1eda production code is unchanged, with test-only capture helpers; absent legacy text-mode/glyph counters are null, not invented values. After a7: first glyph at +3, second at +8. This proves the visible whole-page/letter playback difference; it does not claim absolute original/native CPU or initialization timing parity.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-pc-text-174/opening-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-pc-text-174/opening-after.png)

Original source: home/text.asm, home/print_text.asm, home/joypad2.asm, engine/menus/pc.asm, engine/menus/oaks_pc.asm, engine/events/pokedex_rating.asm, text/pokedex_ratings.asm and audio/pokedex_rating_sfx.asm in pret/pokered fbcf7d0e. Original observations retained from audit 172: two repeated bedroom-PC clips clear True to False, generic main-PC clips preserve True, and generic slow-PC clips preserve False with gradual opening text. Those clips use a different player name and are not glyph/frame-aligned RGB equivalence proof. ROMs are not distributed.

Evidence archive contains complete native and original clips, source hashes, frozen-binary manifests, exact final sources, build/test/GBA logs and tooling. The initial add_pc_text_clock_174.py is preparation, not the entire final transform; final sources include subsequent corrections. Rejected a6 suite had two failures caused by the initial activation wait marker leaking into replaced messages; a7 clears it on message replacement and passes all suites. Master capture a1 placed the helper in the wrong test module and failed to compile; a2 corrected its test-only location and passed.

GBA uses 53 unique source hashes matching the final worktree; all 31 metrics passed the unchanged original budget.

Remaining audit scope: PC PCM/music-bank equivalence; CONT intermediate half-row scrolling; clocks for special confirmation phases; global text initialization/closing/arrow timing; previous movement and exact-head full playthrough plus independent SAVE/CONTINUE gates. This batch is progress, not a statement that all fidelity differences are resolved. Font, Chinese, pinyin and dialogue layout remain excluded.

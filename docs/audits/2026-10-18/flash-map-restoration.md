# FLASH map restoration

Before: `89ff9da6`, after text acknowledgement, the runtime sets a three-frame white counter and immediately returns to the overworld.

After: the party caller retains control through the map restoration plan from `engine/menus/start_sub_menus.asm` / `engine/gfx/palettes.asm` in pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`. It clears/publishes sprite OAM, reloads player/map sprite patterns, font/textbox transfer data, restores the background and palettes, and then returns to the overworld. Resource cache hits still execute transfers; movement cannot bypass the pending work. Text uses the existing Fusion Pixel renderer.

`flash-map-restoration-evidence.json` records exact sources, executable hashes, ordinary save hash, two independent 240-frame replays per revision, inputs, image hashes, and regression results. Screenshots use identical input and native capture indices: 159 (text), 174 (restore in progress), 194 (sprite publication), 200 (returned map). Initial fixture: RockTunnel1F `(15,3)`, seed 42, two Bulbasaur, first knows FLASH. SAVE preparation is a test fixture, not a fresh playthrough claim. Runtime input only: normal CONTINUE, START, Pokemon, first party member, FLASH; A at recorded indices 0/1 and 160/161. No warp or runtime mutation is used.

Tests also exercise outdoor/indoor repeat use, held movement, transfer/cache behavior, snapshot replay, legacy snapshot compatibility, ordinary loading, reset, and actor publication. Constructed PrepareOAM fixtures explicitly identify diagnostic RAM preparation and do not stand in for ordinary gameplay evidence.

## Remaining limits

This restores a missing player-visible sequence. It does **not** establish complete cycle/raster parity. The source LCD-off/restart capture ticks have variable cycle lengths; raw 35 vs 33 white capture counts cannot be interpreted as two physical frames. The current native restoration executes display/copy children with a shared display clock, but its physical-edge waits are not a full common interrupt handler / audio / RETI implementation. The accompanying timing JSON records that limitation and supersedes the earlier raw-count inference. Further normalized timing verification and the broader story/function audit remain open.

## Performance follow-up

The first PR head failed the original GBA performance gate: display updates walked every scanline even during an unchanged frame. The shortcut compared the window latch as if it remained set during VBlank; the device correctly clears it at VBlank entry. The corrected predicate distinguishes visible/AdvanceLy/VBlank phases and skips inspecting OAM when OBJ rendering is disabled. This preserves the complete display state while restoring the steady-period shortcut; no performance baseline or CI limits were changed. The follow-up actual Game replay has the same 240 PNGs and complete observed state records as the preceding after captures. `flash-map-restoration-performance.json` records its exact source/build and limits. Original CI must pass again on the latest head.

## GBA runtime and retained renderer follow-up

The actual cached renderer could reuse a party frame while FLASH text or whiteout changed. FLASH now owns a full redraw through map handoff; eight indoor/facing scenarios compare the cached renderer against full rendering through text, whiteout, return and subsequent idle frames.

A seeded GBA controller then exercised actual START → Pokemon → FLASH input twice on the same party/map/cache. It exposed a real heap failure: two retained linear images left insufficient room for the 8 KB pattern memory. The party image now doubles as staging for the final map redraw, and the retained map uses 5,760 packed bytes. GBA pattern transfers read immutable original ROM bytes directly rather than decoding and reconstructing a sprite/font sheet. A byte-equivalence test covers both storage formats and textbox plane expansion. Unpacking the retained map uses grouped plane conversion, exhaustively tested across every 65,536 plane-pair combination including display-palette preservation.

Both GBA calls complete, restore the same position and party, and recover the same 49,784-byte largest free block; measured painted stack margin is 11,888 bytes. Native desktop/debug-server application tests pass 398 cases with 39 existing ignored cases. A fresh ordinary-SAVE 240-frame native replay has identical PNGs and complete observed records to the preceding validated replay. These are separate scopes: seeded GBA execution is not a NEW GAME or ordinary-SAVE playthrough claim.

**Still draft:** measured GBA calls take about 8.6 seconds including diagnostic work. Stage profiling identifies restoration updates as the dominant cost; heap probing is a small part. Grouped unpacking reduced render cost, but runtime performance still needs correction. Green general-purpose CI and successful repeated return do not establish acceptable FLASH speed or original cycle/raster/common-IRQ parity. Exact identities and measurements are in `flash-gba-runtime.json`.

Review images were refreshed from current master `74cd6ff1` (before) and the same ordinary SAVE/input sequence on candidate `d3261d10` (after). Each fresh replay captures 240 consecutive frames. The four current after images are byte-identical to the earlier committed after images; the before images were checked/replaced from the fresh master capture. Native CLI executable hashes are recorded in `flash-gba-runtime.json`.

### Further runtime profiling

Candidate `460b0072` reduces the same two GBA calls to about 5.60 / 5.58 seconds, with the same 45 logical updates, 49,784-byte returned heap block and 11,848-byte painted stack margin. Immutable ROM references are retained within transfers. A device advance strictly before its next event updates only elapsed time and phase remainder. The exclusive pattern-bus borrow retains successful device/memory validation; checked mutations preserve those invariants, every instruction still checks its budget, and a new borrow or snapshot restarts validation. Source bytes, access order, exact event boundaries, malformed state, DMA, LCD writes and snapshot resumption are compared against checked paths.

Full core tests pass 3,200 cases; desktop/debug-server application tests pass 399 cases with 39 existing ignores. A fresh ordinary-SAVE 240-frame replay has identical PNGs and complete observed records to the preceding capture; all four committed after images remain byte-identical. The initial GBA timing above is historical; `performance_followup_1171` in the runtime JSON records this newer exact build. The remaining restoration cost is still too high, so the PR remains a draft and does not claim original physical timing parity.

### Idle performance gate follow-up

The next CI head exposed a one-tick Pokédex regression (119, original limit 118). Candidate `0c8fe584` retains the checked idle-period eligibility only while the device remains untouched. The time owner's device is private; mutable access and elapsed source instructions invalidate the cache, and deserialization always restarts unchecked. Saved input cannot supply trusted eligibility. A checked interval preserves the exact period and source/device synchronization proof.

All original local GBA performance thresholds pass, including Pokédex update 113 ticks; neither baseline nor limits change. Core library unit tests pass 3,202 cases; application library tests with desktop/debug-server pass 399 with 39 existing ignores. Both actual seeded FLASH calls still return in 45 logical updates with the same heap/stack margins, around 5.60 / 5.58 seconds. The ordinary-SAVE 240-frame PNGs, complete observed records and committed after screenshots are unchanged. This fixes the general performance gate, while the excessive FLASH runtime remains open.

### Exact idle waits and visible scanlines

Candidate `9079b6b9` reduces the same two GBA calls to 3.270 / 3.253 seconds (previously about 5.6 seconds), still 45 logical updates and the same returned 49,784-byte heap block / 11,848-byte painted stack margin. A passive forecast advances an idle wait to the actual next VBlank edge or the owner budget, whichever comes first. Unsupported/dynamic states retain per-event waiting. The ordinary device observer still processes events; settled mode2 intervals with OBJ/window/STAT/DMA disabled batch complete 456-dot visible lines and stop before the final visible line. No source instruction, transfer, DMA publication or interrupt return is omitted or invented.

Forecast and batched-line endpoints are compared against single-T-cycle execution, including full state, startup, partial budgets, scroll and window latches. Core tests pass 3,206 cases; application tests pass 399 with 39 existing ignores, including eight cached/full-renderer map/facing cases. The unchanged original GBA performance gate passes all thresholds (Pokédex 113, limit 118). A newly built native executable replays the ordinary SAVE/input sequence for 240 frames: all PNGs and complete observed records match the preceding verified replay, and all four committed after images remain byte-identical. Exact identities and scope are in `performance_followup_1174` in the runtime JSON.

The earlier timings above are historical. FLASH remains too slow at about 3.25 seconds, so this is still a draft; source physical palette/raster/common-interrupt parity and broader coverage remain unproven.

### Uniform white-frame reuse

Candidate `f6db6d73` retains only consecutive uniform FLASH white frames; text, map handoff and other takeovers invalidate reuse. No second snapshot is retained. Eight map/facing cached/full comparisons also require actual white reuse and verify black-screen interruption restores a fresh white frame. All 399 application tests pass (39 existing ignores). The unchanged original GBA performance gate passes all thresholds. Two seeded GBA calls now take 2.969 / 2.985 seconds with the same 45 logical updates and heap/stack return margins. A fresh ordinary-SAVE native replay from this exact candidate again has all 240 PNGs and complete observed records equal to the preceding replay; all committed after screenshots are byte-identical. The runtime JSON records both executable and ROM identities. This improvement still does not resolve the excessive runtime or establish original physical timing parity; the PR remains a draft.

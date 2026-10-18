# FLASH map restoration

Before: `89ff9da6`, after text acknowledgement, the runtime sets a three-frame white counter and immediately returns to the overworld.

After: the party caller retains control through the map restoration plan from `engine/menus/start_sub_menus.asm` / `engine/gfx/palettes.asm` in pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`. It clears/publishes sprite OAM, reloads player/map sprite patterns, font/textbox transfer data, restores the background and palettes, and then returns to the overworld. Resource cache hits still execute transfers; movement cannot bypass the pending work. Text uses the existing Fusion Pixel renderer.

`flash-map-restoration-evidence.json` records exact sources, executable hashes, ordinary save hash, two independent 240-frame replays per revision, inputs, image hashes, and regression results. Screenshots use identical input and native capture indices: 159 (text), 174 (restore in progress), 194 (sprite publication), 200 (returned map). Initial fixture: RockTunnel1F `(15,3)`, seed 42, two Bulbasaur, first knows FLASH. SAVE preparation is a test fixture, not a fresh playthrough claim. Runtime input only: normal CONTINUE, START, Pokemon, first party member, FLASH; A at recorded indices 0/1 and 160/161. No warp or runtime mutation is used.

Tests also exercise outdoor/indoor repeat use, held movement, transfer/cache behavior, snapshot replay, legacy snapshot compatibility, ordinary loading, reset, and actor publication. Constructed PrepareOAM fixtures explicitly identify diagnostic RAM preparation and do not stand in for ordinary gameplay evidence.

## Remaining limits

This restores a missing player-visible sequence. It does **not** establish complete cycle/raster parity. The source LCD-off/restart capture ticks have variable cycle lengths; raw 35 vs 33 white capture counts cannot be interpreted as two physical frames. The current native restoration executes display/copy children with a shared display clock, but its physical-edge waits are not a full common interrupt handler / audio / RETI implementation. The accompanying timing JSON records that limitation and supersedes the earlier raw-count inference. Further normalized timing verification and the broader story/function audit remain open.

## Performance follow-up

The first PR head failed the original GBA performance gate: display updates walked every scanline even during an unchanged frame. The shortcut compared the window latch as if it remained set during VBlank; the device correctly clears it at VBlank entry. The corrected predicate distinguishes visible/AdvanceLy/VBlank phases and skips inspecting OAM when OBJ rendering is disabled. This preserves the complete display state while restoring the steady-period shortcut; no performance baseline or CI limits were changed. The follow-up actual Game replay has the same 240 PNGs and complete observed state records as the preceding after captures. `flash-map-restoration-performance.json` records its exact source/build and limits. Original CI must pass again on the latest head.

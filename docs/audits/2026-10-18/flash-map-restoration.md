# FLASH map restoration

Current implementation: `b51b5b3f`, composed with master `2106f360`. Status: **PARTIAL**. The PR remains a draft: two fresh seeded GBA runs take about 2.633/2.616 seconds; original physical timing and the common IRQ/audio/RETI path remain unproven. Historical measurements below belong to their named builds; the current result is `current_followup_1188` in `flash-gba-runtime.json`.

Current-master verification: core library 3,225 tests passed; debug-server application library 408 passed with 40 existing ignores. Both the base and candidate were independently captured twice for 240 consecutive frames. Every observed record and PNG hash matches its repeat; both sides share the ordinary SAVE, seed 42, initial image, frame 775 and complete normalized command tape. The base executable was built at `9c0aed53`; all game crate and Cargo source bytes match master `2106f360` (no rebuilt-source relabeling). The candidate executable was built at `b51b5b3f`.

The new [verification bundle](flash-restoration-current-master/verification.json) includes a bootable prepared SAVE, normalized commands and compressed full observed frame records. Captures use the current menu `ready_for_input` observation before selecting Pokemon. The first attempted after capture referenced an obsolete diagnostic field and stopped; correcting only that script produced the two retained successful runs.

At index 159 both sides show the acknowledged FLASH text; at 174 the base has returned to the map while the candidate is white. At 194 the candidate shows the map while sprite work remains pending. **Index 200 is still a transfer phase, not completion.** The candidate returns at 204; the final comparison now uses index 220, with the player visible on both sides. These are matched native host-frame indexes, not proof of original physical-frame parity.

Fresh GBA runs on the composed source return after 45 logical updates, each with a 49,784-byte largest free block and 11,836-byte painted stack margin. Only the five measured inlining attributes were retained. A hidden-OBJ batching experiment passed its state tests but did not improve runtime, so it was rejected and is absent from this branch.

## Player-visible behavior

Before, confirming FLASH text returned directly to the overworld with only a three-frame white counter. After, the party caller retains control through sprite OAM clear/publication, player/map sprite pattern reloads, font/textbox transfer data, background and palette restoration, then returns to the overworld. Cache hits still execute transfers; movement cannot bypass pending work. The source plan is `engine/menus/start_sub_menus.asm` / `engine/gfx/palettes.asm` in pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`. Player text continues to use Fusion Pixel; original glyph bytes are transfer data only.

## Historical native evidence (`19c27230` / `74cd6ff1`)

The ordinary SAVE fixture starts in RockTunnel1F `(15,3)`, seed 42, two Bulbasaur, first knows FLASH. Runtime input is normal CONTINUE → START → Pokemon → first party member → FLASH, with A at recorded indices 0/1 and 160/161. No warp or runtime state setters are used. This prepared SAVE is not evidence of a fresh playthrough.

The historical executable from `19c27230` captured 240 consecutive frames. All PNGs and complete observed records matched its preceding verified replay. Its before run used master `74cd6ff1` with the same SAVE and inputs. The screenshots committed now have been replaced by the current-master comparison described above; historical PNG hashes remain in the historical evidence JSON. `flash-map-restoration-evidence.json` preserves the initial independent repeated captures; `performance_followup_1175` in `flash-gba-runtime.json` records the newest executable hash, ordinary SAVE hash and image hashes.

CLI capture uses full Game.draw. Separately, eight indoor/map-facing scenarios compare RenderSession against full rendering through typing, whiteout, map return and eight subsequent idle frames. Only consecutive uniform white frames reuse rendering; text, map handoff and other takeovers invalidate reuse. Tests explicitly check black-screen interruption followed by a freshly rendered white frame.

Core unit tests pass 3,209 cases; desktop/debug-server application tests pass 399 with 39 existing ignores. Coverage includes repeated indoor/outdoor use, held movement, transfers/cache, snapshots, legacy snapshots, ordinary loading/reset and actor publication. Constructed PrepareOAM RAM fixtures remain diagnostic evidence only.

## Historical GBA execution and memory (`19c27230`)

A temporary controller seeds party/badge/map, then drives actual START → Pokemon → FLASH inputs twice without clearing caches. On `19c27230`, calls take 2.700/2.699 seconds, both return after 45 logical updates, recover the same 49,784-byte largest free block, and retain 11,848 bytes of painted EWRAM stack margin. These are seeded diagnostic runs, not NEW GAME or ordinary-SAVE playthrough proof. Exact source, controller patch and ROM hashes are in the runtime JSON.

The retained party image doubles as staging for the final map redraw; the retained map uses 5,760 packed bytes. This avoids the original two-linear-buffer heap failure. GBA pattern reads use immutable original ROM slices rather than allocating decoded sheets to reconstruct bytes. Plane conversion is exhaustively compared across 65,536 bitplane pairs including display-palette preservation.

All original local GBA performance thresholds pass; baseline and limits are unchanged. Historical exact builds/timings in the JSON show the progression from 8.6 seconds through 5.6, 3.25, 2.97 and 2.785 to 2.70 seconds; earlier values are not current results.

## Timing-preserving optimizations

An untouched idle period reuses checked eligibility only while its private device remains unchanged; mutation and source instructions invalidate it, and deserialization cannot supply trusted eligibility. Idle edge waits use a passive next-VBlank forecast bounded by the owner budget. Settled visible scanlines with OBJ/window/STAT/DMA disabled batch whole 456-dot lines, stopping before the last visible line. Forecast and line endpoints are checked against single-T-cycle execution and full device state.

An exclusive pattern-bus borrow retains validated memory/device state and immutable source references. The new LCD polling lease similarly retains read/plain-instruction validation while preserving every instruction budget and original port-read cycle. Writes invalidate that receipt and use the checked path. LCD work, device, elapsed time and divider are compared after every instruction, with full-state write/grant/snapshot checks and malformed-state/error-order controls. Reserved device advances now use their checked u8 instruction duration; all phases, LCD-off/OBJ/window/DMA and partial/end boundaries are compared against the fully checked time observer. No source instruction, access, hardware edge or interrupt return is removed or fabricated.

## Remaining limits

This restores a missing sequence; it does not establish complete cycle/raster parity. LCD-off/restart makes the original capture ticks vary in physical duration, so raw 35/33 white capture counts do not prove a two-physical-frame difference. Matched palette boundaries and normalized physical timing still need verification. The native physical-edge waits are not a full common interrupt/audio/RETI implementation. Excessive GBA runtime, those timing questions, the broader story/function audit and final latest-master fresh playthrough remain open.

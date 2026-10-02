# GBA evidence after preserving the project fonts

This component validation uses production source **`c51209a492f804f58486607919c9ba0c62eb2eb7`**. It restores the project's Fusion Pixel fonts. It predates the final descender layout and Fossil Room lazy-decoding repair, so these results do not describe the final PR head. The earlier original-font-bank performance results remain separately identified in `gba-performance-repair.md`.

## Build and performance

All three GBA release configurations compiled with `nightly-2025-12-07`, `--locked --offline`, and `CARGO_INCREMENTAL=0`: ordinary release, `perf-benchmark`, and `memory-scenarios`. The ROMs were frozen before the shared nested target changed. Their complete hashes and persistent local paths appear in [the manifest](final-font-preserved-gba/manifest.json).

mGBA **0.10.5**, revision `26b7884bc25a5933960f3cdcd98bac1ae14d42e2`, ran the unchanged CI scripts with hardware-timer metrics, synchronization disabled, and an isolated display/configuration directory. Its Ubuntu Noble release archive matched CI's SHA-256 `0bbf1e7ca511cd4b443239b97546f699df72211241a1db9177e331866031d8e9`.

The existing baseline, 15% allowance, and 25-tick absolute floor were unchanged. **All 31 performance gates passed across seven scenes.**

| Scene | Samples | Renders | Update average | Draw per frame | Maximum draw |
| --- | ---: | ---: | ---: | ---: | ---: |
| Intro/title | 250 | 115 | 109 | 1156 | 10260 |
| Oak dialogue | 600 | 307 | 192 | 2964 | 8881 |
| Overworld idle | 1400 | 0 | 342 | 0 | 0 |
| Overworld movement | 240 | 26 | 352 | 282 | 2986 |
| Wild battle entry | 500 | 369 | 196 | 2709 | 8914 |
| Trainer battle entry | 500 | 224 | 211 | 2447 | 10216 |
| Pokédex entry | 400 | 1 | 96 | 41 | 16414 |

All numbers are emulated GBA hardware-timer ticks. Oak's update average is 192 against the unchanged exact limit 192.05; its draw average is 2964 against 3004.95. These fixed-frame windows contain changed original-game timing and are not a claim that every semantic animation phase is identical to the old baseline. The script still requires actual samples and renders for the six scenes that draw. The idle scene deliberately permits zero renders.

Evidence: [metrics](final-font-preserved-gba/performance.json), [all comparisons](final-font-preserved-gba/performance-compare.log), [mGBA log](final-font-preserved-gba/performance-mgba.log), and [27 passing Python gate tests](final-font-preserved-gba/tool-tests.log).

## Stress failure reproduced independently of the fonts

The unchanged broad stress script ran with its original 600-second timeout and 4096-byte heap/stack requirements. It completed these four scenes:

| Scene | Completed count |
| --- | ---: |
| English/Chinese Pokédex entries | 302 |
| Map transitions | 248 |
| Oak's Parcel delivery | 1 |
| Oak Pokédex evaluation bands | 16 |

It then failed in **Cinnabar Lab Fossil Room**, while talking to scientist 1. The log records `free=28720B stack=13224B` before the talk and then `memory allocation of 0 bytes failed` at `alloc.rs:439`. The script exits on that allocator panic, before the fossil scene can pass. The lowest previously observed contiguous heap was 7824 bytes; those observations do not measure the later transient allocation peak. Slots, menus, moves, evolution, trade, ending, full-PC traversal, and SRAM completion are not validated by this failed run.

The scenario creates a full party, all 12 PC boxes, and 50 Hall of Fame teams, then warps to `(5,3)`, faces up, and uses real A presses. It carries no fossil, so the expected conversation is the greeting followed by the no-fossil response. The failure occurs before the first conversation command is emitted.

The production path fully decodes the `talkScientist1` statement list at `native_script.rs:990`, then loads it at `:1069`. `Interpreter::load_function` clones the statements into its execution tree; the interpreter also clones the current outer `If` when ticking. The function's 17490-byte JSON includes all three selection paths and all three completed-revival paths, even when only the no-fossil dialogue is needed. The measured failure and these overlapping trees support splitting the native entry into small lazy branches and continuing from the player's actual filtered-bag choice. They do not establish the precise allocation that first exhausted the allocator.

Evidence: [stress gate output](final-font-preserved-gba/stress-gate.log) and [complete mGBA log](final-font-preserved-gba/stress-mgba.log).

## Original failing CI revision

GitHub run `37011906210`, job `110853395344`, was built from the pre-font-reversion `a4eb175` revision. Its performance and 40-encounter Route 22 steps passed; step 14, “Stress graphics, maps, animations, full PC and SRAM,” failed after 210 seconds. REST annotations only say exit code 1; the remote log/artifact domains were inaccessible under the session's network policy.

An isolated checkout of `a4eb175` was rebuilt and run with the same mGBA release and unchanged broad stress script. It reproduced the same allocator panic in `talkScientist1`, after the same four scenes passed. Its fossil entry reported `free=28512B stack=13232B`. This supplies local runtime evidence for the failed CI step without attributing an unseen remote panic message to GitHub.

Evidence: [pre-font-reversion gate output](final-font-preserved-gba/prefont-ci-stress-gate.log), [mGBA log](final-font-preserved-gba/prefont-ci-stress-mgba.log), and source/ROM metadata in the manifest. The fixed-source stress and final performance results must be recorded separately after the lazy-decoding and final layout changes are integrated.

# Final GBA validation

**Production source `eebde2fc3a4f78c38b4c7b5d49e0db2b830cb235` passed the complete GBA CI job.** This revision includes the final retained-font layout and Fossil Room lazy-decoding repair. The original Fusion Pixel font, performance baseline, allowances, memory floors, sample requirements, and timeouts were retained.

The evidence deliberately distinguishes local emulator measurements from GitHub's official step conclusions. See [the manifest](final-checked-gba/manifest.json) for ROM hashes, commands, source hashes, and evidence-file hashes. The earlier `final-font-preserved-gba.md` records the pre-repair `c51209a` failure and remains historical evidence.

## Official final CI

[GitHub run 37023355401, job 110891720787](https://github.com/liuyanghejerry/open-pokered/actions/runs/37023355401/job/110891720787) reports `head_sha=eebde2fc3a4f78c38b4c7b5d49e0db2b830cb235`, `status=completed`, and `conclusion=success`. [The archived REST response](final-checked-gba/official-ci-jobs.json) records successful builds and all gates:

| Step | Original workflow gate | Result |
| --- | --- | --- |
| 10 | Hardware-timer performance comparison | PASS |
| 12 | 40 Route 22 encounters, heap/stack floors, SRAM save | PASS |
| 14 | Full graphics, maps, animations, PC and SRAM stress | PASS |
| 16 | Connection, encounter and 20 move-animation frame timing | PASS |
| 18 | Opening, naming and doorways with slow ROM access | PASS |
| 20 | Unskipped Gengar/title opening with slow ROM access | PASS |
| 21 | Upload performance evidence | PASS |

The remote raw logs/artifacts were inaccessible under the session's network policy. Therefore the official run supplies source-linked gate conclusions, not claimed raw minimum margins, individual timer rows, remote ROM hashes, or timings. Those unavailable measurements are not substituted with local values.

## What the successful stress gate establishes

The unchanged `scripts/gba_memory.py --suite scenarios` requires each scene in order with its exact count, then `memory: ALL PASS` and actual heap/stack measurements. It rejects a panic/allocation error, missing or repeated scenes, either observed margin below **4096 bytes**, or failure to complete within **600 seconds**. Its successful official exit therefore establishes all thirteen cases:

| Scene | Required count |
| --- | ---: |
| English/Chinese Pokédex entries | 302 |
| Map transitions | 248 |
| Oak's Parcel | 1 |
| Oak evaluation bands | 16 |
| Fossil Room conversation | 1 |
| Slot sessions | 3 |
| Menu scenes | 48 |
| Move animations, both sides | 330 |
| Evolutions | 3 |
| Trades | 3 |
| Ending | 1 |
| Hall of Fame PC traversal | 300 |
| Full SRAM save/reload | 1 |

This includes the Fossil conversation that actually panicked on both `a4eb175` and `c51209a`. The final official successful gate covers the subsequent scenes that those failed runs never reached. It does not supply an exact official minimum heap/stack value or separately exercise every fossil-selection branch; the latter has native production tests.

`gba_frame_timing.py` requires at least two valid samples per prescribed scene, a completed marker, monotonic logic ticks, and at most one update between sampled draws. The official gates cover 23 move/connection scenes, six hardware scenes, and three opening scenes. The slow-cart mode uses the existing reviewed startup patch in an isolated temporary ROM copy; no shipped ROM or threshold changed.

## Independent local measurements

The same final production source compiled locally with `nightly-2025-12-07`, `--locked --offline --release`, and `CARGO_INCREMENTAL=0`. Ordinary release, `perf-benchmark`, `repro-route22`, and `memory-scenarios` all compiled and were frozen before the nested build target was reused. Their four local ROM hashes and persistent paths appear in the manifest.

The matching mGBA 0.10.5 Ubuntu Noble archive matched CI's SHA-256 `0bbf1e7ca511cd4b443239b97546f699df72211241a1db9177e331866031d8e9`. One emulator ran at a time with synchronization disabled and isolated configuration/SRAM fixtures.

All **31 performance gates passed across seven scenes**, against the unchanged baseline with its **15% or 25-tick** allowance:

| Scene | Samples | Renders | Update average | Draw per frame | Maximum draw |
| --- | ---: | ---: | ---: | ---: | ---: |
| intro-title-v1 | 250 | 115 | 109 | 1156 | 10260 |
| oak-dialogue-v1 | 600 | 307 | 192 | 2964 | 8881 |
| overworld-idle-v1 | 1400 | 0 | 346 | 0 | 0 |
| overworld-movement-v1 | 240 | 26 | 356 | 282 | 2986 |
| battle-entry-v1 | 500 | 369 | 196 | 2709 | 8914 |
| trainer-battle-entry-v1 | 500 | 224 | 211 | 2447 | 10216 |
| pokedex-entry-v1 | 400 | 1 | 96 | 41 | 16414 |

All measurements are emulated GBA hardware-timer ticks. Oak's update average is 192 against the exact unchanged limit 192.05; its draw average is 2964 against 3004.95. These fixed-frame windows are not a claim that every semantic animation phase matches the old baseline. [Metrics](final-checked-gba/performance.json), [all 31 comparisons](final-checked-gba/performance-compare.log), and [the local mGBA log](final-checked-gba/performance-mgba.log) preserve the actual measurements.

The local Route 22 test completed **40/40 encounters** with `save=ok`, minimum observed contiguous heap **23816 bytes**, and untouched stack **13888 bytes**. It took **420.488 seconds**, within the original 600-second limit. The five original before/after-parcel and final-battle position variants each ran eight times. [Gate output](final-checked-gba/route22-gate.log) and [the complete local mGBA log](final-checked-gba/route22-mgba.log) preserve the measurements.

All **27 Python gate-tool tests passed**. Once the same-source official CI had passed the remaining gates, the redundant local stress run was intentionally terminated; its compiled ROM is retained, but no local thirteen-scene or pacing completion is claimed. No further duplicate builds or emulation were performed. The positive final stress/pacing conclusions above come from the successful official CI job.

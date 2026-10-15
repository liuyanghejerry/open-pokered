# Ordinary capture draws from the saved battle RNG

Ordinary ball throws used the global hosted RNG, outside `set_seed` and runtime
snapshots. Their Rand2 argument was also evaluated before Rand1 and before the
branches that skip Rand2. Restoring the same battle and pressing the same buttons
could capture the wild Pokémon or fail.

Canonical ordinary capture now uses the existing `BattleScreen.rng` through one
mutable draw closure. The original `ItemUseBall.loop` draws Rand1 **before** the
Master Ball check. Rejection sampling accepts Great Ball bytes ≤200 and Ultra
Ball bytes ≤150. Status underflow, catch-rate failure and W>255 decide without
Rand2; otherwise the comparison draws Rand2 once. The capture arithmetic,
comparison thresholds, shakes, item consumption and continuation are unchanged.
No second RNG, state owner, snapshot field or renderer change is introduced.

The existing direct-byte and roll1/fixed-Rand2 APIs remain compatible, including
Safari callers. This correction applies to canonical ordinary capture only.
Safari, disobedience and other global RNG bypasses are separate work.

## Actual native replay and ordinary SAVE

The preboot fixture contains two healthy legal level-12 Rattata, normal learned
moves, HP31 each, two Poké Balls and money3000. The existing all-owned Pokédex is
an explicit fixture convenience, not proof of a campaign playthrough. CLI map
bootstrap and seed19 occur before the first input; ordinary walking produces a
natural level-3 Rattata encounter. No in-battle state/RNG/outcome edits are used.
Each trial has a private executable directory, port and save file.

At frame1064 the real battle BAG selects the Poké Ball. Each fork restores the
runtime slot, independently reserializes it and checks the initial hash before
exactly A1 followed by119 neutral frames.

| Compiled source | Repeats × forks | Initial hash | First-throw outcomes | Posthashes |
| --- | --- | --- | --- | --- |
| clean master `4c364111` BEFORE | 2 × 12 | `8dc0cc43a3a51430` in both | 5/7, then7/5 caught/failed | 2 |
| `19eac5ae` AFTER | 2 × 12 | same hash as BEFORE | failed in all24 | 1 |

All24 AFTER results have posthash `a80294c9ae258834`. This is a reproducible first
throw, not a claim that all balls always fail. The final real fork in each trial
continues through ordinary battle/escape inputs and START → SAVE. Only these two
saves per build are counted; adaptive finishing inputs are not compared as an
identical prefix.

All four complete32768-byte SRAMs have valid checksums. Both AFTER SRAMs are
byte-identical, SHA256
`e7e4c5a10fc88b41010dec8ac9c1e358dd705901230ff026b8537ca81559252a`.
Every old88 party byte remains equal to construction, including HP31/31,
status0/0, PP and stats. The ball quantity is1. Frozen-binary ordinary SAVE imports
also protect old boxes, Pokédex, player identity, money3000, bag, PC items,
collected items/coins and event/object flags. BEFORE's final fork captures and
has party3; AFTER's final fork fails and finishes with party2. Full SRAM equality
between BEFORE and AFTER is therefore not claimed.

## Original-ROM controls

Two independently prepared natural encounters each supply four full hardware
snapshot forks. A legal healthy Rattata12 pair, Poké Ball2, matching all-owned
Pokédex, fast option and initial hardware RNG bytes are constructed before the
first input. Ordinary walking naturally encounters Rattata level2, HP13 and
catch-rate255; native level3 and original RNG/DVs/clock are not equated.

All eight actual source throws catch. Read-only hooks at the checked CALL-return
addresses record Rand1=232 followed by conditional Rand2=36. Actual nickname NO
and ordinary SAVE complete every fork. All eight input action lists and complete
SRAMs are identical, SHA256
`7e1bc6b5700216a611285ee9c0323e5266e8a400270f27adb6a129899ea95141`.
The old88 party bytes, old box, dex and money are protected; ball2→1 and party2→3.

The primary ASM is `engine/items/item_effects.asm`, reference
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`. Its bytes match the pinned reference
archive. The extracted working reference lacks its own `.git`; an ancestor
`git rev-parse` would refer to the host project and is not original provenance.
This actual source matrix covers normal-ball success/Rand1→Rand2 only. Other
branches below are primary-source rules checked with deterministic tests.

## Validation and source identities

Seven new core tests check Master Ball's one draw, Rand1/Rand2 order, Great/Ultra
redraws, exact skipped Rand2 branches, W==255, inclusive thresholds, canonical
success/failure outcomes and consumption, snapshots, trainer/full-box refusal,
and ghost/restless-soul dodges. One application test drives actual menu buttons
and replays seeded and entropy-initialized persistent streams12 times each.
A standalone ordinary-menu probe against the fresh4c BEFORE libraries fails as
expected: Master Ball leaves the saved owner at word-position0 instead of1.

Frozen source `19eac5aee7689c058bfe5afaf905a5c9c74b59e8` passed:

- Complete core2896; full app debug296/default285, with39/37 existing ignores.
- Agent determinism9 in both feature configurations; explicit debug-server build.
- All seven unchanged GBA performance gates twice, with identical full metric
  JSONs. Trainer update232 ticks, idle382 and Oak draw2994.

Implementation commit is `0d097141`. Clean4c BEFORE was frozen before master
`f0787e19` landed; that master commit changes only a test fixture and docs.
AFTER has a separate merge, build, source manifest and actual captures on f078.
The historical05f6 native proof and original read-only proof keep their own
identities. Current AFTER binary SHA256 is
`354c438d2eb8e7c170b5903f49e6e96512cf59e06723d99df79a21797b06a327`.
No remote CI or full playthrough pass is claimed.

## Evidence

`verification.json` is the compact report. `archive.json` records hashes, counts,
exact metadata names and selected artifact roots. `evidence.zip` retains exact
original bytes using deduplicated blobs and an entry manifest. It combines two
explicit immutable issue926 proof archives with the exact new before/after
artifact directories and metadata allowlist; no shared metadata-prefix glob is
used. ROMs and executable binaries are excluded.

```bash
python3 extract_evidence.py evidence.zip --verify-only
python3 extract_evidence.py evidence.zip /tmp/capture-rng-evidence
```

Retained harness failures are not counted as successful controls: initial native
wrong-BAG navigation and incomplete failure-caption classification, a source
syntax typo before emulator startup, and an unnecessary ambiguous serde library
selection in the standalone probe setup. Corrected drivers/checks pass.

This is a pure game logic/test change, excluded from AGENTS.md's renderer
screenshot requirement. Actual PNGs remain archived. No cross-provider capture
probability, RNG-byte stream, whole-SRAM, frame or audio parity is claimed.

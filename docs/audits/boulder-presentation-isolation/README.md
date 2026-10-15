# Isolate the boulder presentation regression from wild encounters

The boulder presentation test occasionally entered a random Seafoam wild battle before the push started. The first direction sample after its idle setup takes the turn branch, which calls the ordinary wild-encounter check. The test used an entropy-seeded overworld and did not exclude encounters.

The fixture now sets the existing one-step encounter cooldown before pressing Down. The ten-frame push-start budget and every slide, dust, graphics-restore and `control_ready` assertion remain unchanged. There is no production logic change, no longer wait and no weaker assertion.

## Evidence

Base/source: `05f6e76c2e8fc6c0a64dbf6486316a073fa20347`.

- An unchanged frozen 910 integration test binary passed 40 isolated repeats. This binary keeps its own `c489a663` identity; it is not described as clean master.
- A fresh independent clean-master build passed its full app suite. A test-only diagnostic build also passed three full suites. Those passes did not resolve the reported intermittent failure.
- A subsequent diagnostic isolated batch reproduced the failure on its **32nd execution (zero-based repeat 31)**. At the first Down update the screen changed to `Battle`; the overworld frame stayed at 121 for the rest of the ten updates. The player remained at `(18,9)` and the boulder remained at `(18,10)`.
- Replaying that exact captured pre-state twice reproduced the old failure twice. Changing only the fixture cooldown made both replays complete the existing push presentation assertions.
- The source original's `home/overworld.asm` turn handler calls `NewBattle`; `engine/battle/wild_encounters.asm` allows indoor encounters in a cave. This audit does not propose changing that gameplay behavior.

The raw snapshots retain their original RNG states. Replays are diagnostic unit-test setup, not a claim of natural campaign play or ordinary SAVE coverage. No engine setters were added. Default save/sidecar files were absent next to all nine investigated libtest executables. No app-lib or renderer tests installed runtime map/block/wild overrides in this scope; a shared override race was not established.

## Validation

- `cargo test -p pokered-core --lib boulder`: 31 passed.
- `cargo test -p pokered-app --features debug-server --lib`: 289 passed, 39 existing ignored.
- `cargo test -p pokered-app --lib`: 278 passed, 37 existing ignored.
- Final unchanged compiled fixture: 64 isolated entropy-initialized repeats passed.

This change lives in the `#[cfg(all(test, feature = "debug-server"))]` test module. It cannot change a production frame, so before/after player screenshots are not applicable. GBA runtime source, performance limits and baselines are unchanged; no new local hardware-timer measurement is claimed. Remote CI remains a separate exact-head gate.

`evidence.zip` is content-deduplicated; `archive.json` records its SHA-256 and path manifest. It includes only explicit 922 diagnostic/replay paths and their directly used metadata, including the failed diagnostic build. Executables and unrelated PR evidence are excluded. Use `extract_evidence.py` to verify/extract it.

# NPC movement and viewport timing

The original chooses a new random NPC step only when the player walk counter
was zero before `AdvancePlayerSprite`. An idle delay still counts down and an
already running NPC step keeps moving. Using the decremented player counter
starts the NPC on the player's final walking update; using the new first-step
counter incorrectly prevents a simultaneous first step. The regression covers
both boundaries, a waiting NPC, and an already moving NPC using original RAM.

NPC OAM coordinates use the previous background viewport: VBlank writes SCX/SCY
before the next PrepareOAMData/DMA transfer. The port now latches this viewport
separately and stores it in JSON snapshots. A mid-scroll capture/restore and
30 subsequent hardware frames produce the same poses and viewport states.
Older JSON without this optional field still loads.

## Matched visual evidence

`ready-walk-{before,after}.png`: hardware t12, NPC waiting beside a moving player.
`moving-scroll-{before,after}.png`: hardware t10, NPC already moving as the
player scrolls the map. Before is actual master 31b1eda with only the ignored
capture helper; after is this branch. Same SRAM, controlled actor coordinates,
120 idle frames, four priming frames, and Down held on hardware frames 0..15.
The PR also changes field cadence, so equal priming input does not imply equal
resulting pretrigger NPC counters on master. Frame numbers are never offset to
make the images agree.

`ready-walk-previous-head.png` isolates the ready-gate failure on 7b4cc08.
`moving-scroll-before-camera-fix.png` isolates the two-pixel viewport error
before the camera correction. These supplementary pictures are not substituted
for the actual-master comparison.

The original valid standing NPC is at raw Y44, not the Y48 used by the earlier
131 source fixture; Y48 belongs to the downward moving case. Source 134 corrects
this coordinate and has two byte-identical RAM/PNG recordings. The extracted
counter fixture was unaffected. Superseded failed test logs remain in the
archive instead of being relabeled as passing evidence.

The original and latest port match the NPC's opaque pose and complete RGB
strip x48..64/y20..72 for every hardware frame -1..18 in the ready and already
moving cases (40 compared frames). Full recorded repeat pairs are also exact;
see native-verification.json. Random direction after a zero-delay NPC starts
moving is deliberately excluded from the direction/image oracle: no matching
PRNG stream is claimed. Counter/phase/intra-frame boundaries still compare.
These checks do not establish full-screen equivalence or scripted/boulder timing.

## Verification and remaining gates

- Core/app default suites: 3,646 passed, 80 groups, 53 ignored, no failures.
- Debug-server app tests: recorded separately in regression-verification.json.
- Existing three-case menu/NPC source oracle and snapshot replay passed in the
  core/app run; the new player-walk oracle passed all three cases.
- GBA: all 31 metrics within the unchanged checked-in performance baseline.
- The 7b4cc08 fresh full playthrough reached m16, then actually lost to Surge's
  Raichu at m17. This is a failed mainline validation, not a completed run.
- Victory Road 2F switch/OAM timing, other screen-restoration paths, a fresh
  latest-head m01..m49 run and independent Continue remain required before merge.

recordings.zip contains repeated original/native/master captures, raw state
rows, reproduction scripts, capture-helper source and compressed build/test
logs. provenance.json pins source, input scope and file hashes. Fonts, Chinese,
pinyin input and dialogue layout remain outside this audit's requested scope.

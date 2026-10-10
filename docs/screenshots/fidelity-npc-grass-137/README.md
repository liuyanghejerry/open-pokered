# NPC grass priority

The port drew the whole NPC over grass. Original `CheckSpriteAvailability`
retains the **top-right** footprint tile before NPC movement. It preserves the
flag while the player's old walk counter is nonzero. Y aligns to 16 px, X to
8 px. The flag applies to the lower two OAM tiles and reaches the LCD through
the same delayed queue as position/image. Snapshots and rendering keys now
preserve that flag.

Priority rendering saves the actual 16x8 background before actors are drawn.
It restores nonzero BG only on opaque pixels of the NPC's lower half, including
unaligned steps and animated terrain. Damage stays within the actor rectangle.
The original oracle compares cached/full frames while priming a stationary
actor, so a priority-only change must invalidate a previously cached picture.

`standing-before.png` / `standing-after.png` use actual master 31b1eda and this
branch with the same native fixture, actor placement, 120 idle frames, four
primes and subsequent idle input at hardware t=-1. `standing-previous-head.png`
isolates the defect on 2e03e6b. Master's prime counters differ because the full
PR changes cadence; no frame is shifted to obtain agreement.

The original fixture reaches Route1 via an actual connection and rock-wall
detour to player (12,22). Repel=200/lead level=50 suppress wild encounters only,
without a battle-stat fidelity claim or BG/map/ROM edit. Map dimensions are
20x36 cells. NPC0 is controlled at (14,22), Down, waiting 127. NPC1 starts at
origin (15,22), target (16,22), Right, remaining 12, phase1/intra0. Both prime
four HW frames. Native future waits 16/127 match the original chosen waits
only for these cases; general PRNG alignment is not claimed.

Verification:

- 21 standing, 31 NPC-walking and 31 player/NPC-walking frames compare original
  counters, phase, image, priority and complete NPC-region pixels: 83 exact.
- 95 full/cached framebuffer comparisons (83 oracle + 12 prime frames) exact.
- Three mid-snapshot JSON replays of 30 HW preserve presentation; older nested
  poses missing the optional flag deserialize successfully.
- Repeated original/native RAM rows and PNGs are byte-identical.
- Old implementation: 53 wrong pixels in the lower eight rows on all 21 frames.
- Full core/app debug-server suites: 3,660 passed, 80 groups, 57 ignored, zero failures.
- All 31 configured GBA metrics pass the unchanged original performance baseline;
  five production files are identical in native and GBA checkouts.

Earlier blocked-path, invalid grass-placement, compiler and phase-zero probe
failures are retained and excluded from passing evidence. The initial suspicion
of a wild battle was disproved by the screen and wIsInBattle=0. The detour is
recorded. Bottom-left/whole-cell sampling failed the walking boundary; the
source's top-right tile is the rule implemented.

This does not establish all scripted/connection/talk NPC behavior. Victory Road
2F timing, other graphics restores, latest-head full mainline/Continue and
matching CI remain merge gates. Fonts/language/pinyin/dialogue layout remain
outside the user's requested audit scope.

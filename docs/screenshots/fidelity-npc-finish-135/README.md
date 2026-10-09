# NPC standing image after a step

`CheckSpriteAvailability` preserves a walking NPC's image while the player
walks. A waiting NPC takes a different branch: `UpdateSpriteMovementDelay`
always falls through `NotYetMoving` and updates the standing image, including
the tick where its delay becomes zero. The port incorrectly applied the first
branch's image preservation to both. In the controlled original scenario the
NPC finishes at t9 and must update its image at t11; the port remained at image3
instead of image0. The failing regression is preserved alongside the fix.

The presentation state now retains the pre-update delay, so a final 1->0 delay
still takes the waiting branch. JSON snapshots preserve it. Legacy snapshots
without that optional cache seed it from the restored logical actor before the
first update. Both delay13 and delay1 legacy restores match current snapshots
for30 hardware frames, including the first standing-image update.

## Evidence

Before/after are actual master31b1eda / this branch, same road SRAM, actor setup,
input, priming sequence and hardware t13. `finish-t13-previous-head.png` also
isolates the defect on5d32403. The full PR changes movement cadence, so the
resulting master NPC counters after equal priming input differ. No capture was
shifted in time to obtain agreement.

Original source: pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c.
The controlled actor starts at origin19,29, targetMap23,34, remaining6,
phase2/intra2/rawY54, then four actual idle hardware frames prime OAM/LCD.
Native preloads future delay13; the original selects13 at completion. This
controls the test's wait, without claiming an aligned general PRNG stream.
The player holds Down on hardware0..15, then releases.

- 20 first-step hardware frames compare player/NPC counters, animation and image.
- All20 NPC opaque poses and complete RGB strip x48..64/y20..72 match original.
- Two original161-frame and native/master101-frame recordings per case have
  byte-identical RAM rows and PNGs. Latest captures were rerun after the legacy
  snapshot fix; recordings.zip contains reproduction scripts and helper source.
- Full core/app with debug-server:3658 passed,80 groups,55 ignored,zero failures.
- Latest production GBA Timer2:31 metrics within the unchanged original baseline.

This fixes the waiting-image branch, without establishing equivalence for all
NPC facing/collision/grass behavior. Victory Road2F timing, other graphics
restores, latest-head full mainline/Continue and matching CI remain merge gates.

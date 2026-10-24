# GBA animation memory margin

The unchanged broad memory gate on PRs #381 and #382 found only 3,839 bytes
of contiguous heap during enemy Hyper Beam, below its 4,096-byte requirement.
Replaying the downloaded #381 CI ROM locally reproduced the same low at
frame 86882. A local rebuild passed with a different ROM, so the previous
local result was insufficient to clear the CI failure.

Each decoded move-animation sheet occupies 5,120 bytes. GBA animation OAM
now reads its existing ROM-resident 2bpp sheet directly and expands one
eight-pixel row on the stack. The shared compositor retains clipping,
flips, transparent pixels, background priority, ten objects per scanline,
X-coordinate priority and mid-frame object-palette writes.

## Validation

- Renderer: 403 tests pass, including comparison of both real sheets across
  all tile IDs, object attributes, clipping and palette-split boundaries.
- Application: 393 tests pass; 38 existing opt-in tests remain ignored.
- The original 13 GBA memory scenarios pass on the standalone master-based
  change and in combination with #382. Both retain at least 7,832 bytes of
  contiguous heap and 8,708 bytes of untouched stack. The suite includes
  all 330 player/enemy move-animation cases, full PC and SRAM checks.
- The original 7 GBA performance scenarios pass with the existing thresholds.
- Actual mGBA captures compare the master tree's CI ROM with the candidate:
  enemy Hyper Beam has the same ordered 65 framebuffer changes, including
  its palette flashes (58 unique images). Repeated identical hardware frames
  are collapsed for this pixel comparison; the captures contain 1,111 and
  1,115 hardware frames, so this is not a claim of identical timing.

The committed before/after PNGs show the same 51st framebuffer change in that
animation. ROM hashes, commands and detailed results are in `verification.json`.
The before ROM comes from the successful #380 CI artifact: its source tree
equals the merged master tree `2748125f698fd766b145fdc503b0e63c5954b134`.

This is prepared-state regression evidence. It does not establish a new
ordinary NEW GAME playthrough. Original scenario coverage, memory limits,
performance thresholds and CI workflows are unchanged. Required CI runtime
checks must complete successfully before this change is merged.

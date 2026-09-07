# PR #62 regression corrections

Before: `master` at `cf265a790305cea78e0a20e43bb867c88d9e023f`.
After: `fix/pr62-overworld-regressions`.

The same `capture_comparisons` harness renders both revisions headlessly at
160×144. Screenshots are raw framebuffer output, with no image edits.

```sh
CAPTURE_SIDE=before cargo test -p pokered-app --test pr62_regressions capture_comparisons -- --ignored
CAPTURE_SIDE=after cargo test -p pokered-app --test pr62_regressions capture_comparisons -- --ignored
```

The harness was introduced for this correction, so copy the test file onto the
base checkout before running its capture command.

- `fly-15`: side wings in flight, overlapping an NPC (player OAM priority).
- `fly-18`: opposite wing pose, same hidden player.
- `fly-33`: last coordinate, bird aligned with the player's landing position.
- `current-0`: neither B2F boulder down, (15,8) arrival + 60 idle frames.
- `current-3`: both B2F boulders down, identical arrival + 60 idle frames.

For FLY the harness uses the real Route1 → PalletTown warp and then freezes
animation frames 15/18/33. For the current it uses the real warp/scene-loading
path with fixed flag values. All captures use English and identical inputs.
The TUI shares the corrected visibility rule but approximates FLY by hiding the
player; the native framebuffer comparisons above show the full sprite animation.

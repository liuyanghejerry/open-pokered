# Player lower-half grass priority

The player stands facing down at PalletTown (10,1), with no movement or
dialogue. The fixture suppresses the already-audited Oak trigger and hides
NPCs so both captures exercise exactly the same player/background scene.

`before.png` was captured on master `be54dcbde23b7507eb1127bd8fead7ab302f2348`.
`after.png` uses the identical fixture on the isolated grass-priority fix.
The capture helper was present in both builds and changes no production code.
Both state records match. Changed pixels are confined to X65..77, Y68..71.

The original lower two player OAM entries have priority bit 0x80 when
standing in grass. The previous renderer repainted only the background tile
starting at Y72, missing priority pixels at Y68..71. The fix clips nonzero
background pixels from all overlapping tiles to the entire lower sprite half.

Reproduce from a workspace checkout with graphics installed:

```sh
GRASS_CAPTURE_DIR=/tmp/player-grass-capture cargo test -p pokered-app --lib capture_player_grass_priority -- --ignored --nocapture
```

`reference-player.png` is the 16x16 player crop from an actual retail Red
NEW GAME, ordinary inputs to the northern Pallet exit, during Oak's approach.
Its 256 pixels exactly match the fixed screenshot's player region. This is
player-region evidence; the complete reference map frame is not claimed equal.
ROM/source identities, screenshot hashes and comparison bounds are recorded
in `verification.json`. No ROM or emulator save is included.

Validation: the isolated branch's full app library suite passes, including
the regression for lower-half clipping and transparent background color zero.
An earlier continuous-frame integration audit compared both northern-exit
positions twice: all 374 player-region frames matched the original after this
fix. Script clocks, Oak routes and other integration changes are outside this PR.

# Story fidelity split: matched healing captures

Base: `cb131bb` (current master, including #110 Chinese pagination).
Source changes: PR #111 at `0e6c00562b7348e044c2580e5ecb3b61e0b10d83`.

The capture harness at `docs/audits/fidelity-story-split/capture.rs` was temporarily
copied to `crates/pokered-app/tests/capture_story_fidelity.rs` and run on the base
and this branch with identical input. It creates PokemonTower5F at (10, 8),
injects the same single `heal()` command through the native scene interpreter,
and captures idle frames 8 and 20. This isolates the purified-zone heal effect
from trainer movement, encounters, and subsequent dialogue. The capture
asserts that the Heal command actually ran. Frame 20 shows the added white
palette hold; the base immediately keeps rendering the map.

Run from the workspace root:

```sh
cp docs/audits/fidelity-story-split/capture.rs crates/pokered-app/tests/capture_story_fidelity.rs
CAPTURE_SIDE=after cargo test -p pokered-app --features debug-server \
  --test capture_story_fidelity -- --ignored
```

For the base, copy this same capture harness into its test directory and use
`CAPTURE_SIDE=before CAPTURE_DIR=<absolute output directory>` with the same
command. Captures are newly generated against the current master; no images
from the original combined PR were reused.

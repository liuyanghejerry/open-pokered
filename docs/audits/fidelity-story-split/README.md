# Story split validation

Base: `cb131bb`; original combined source: `0e6c005`.
All results below were run anew on this extracted branch.

| Check | Result |
|---|---|
| `cargo test -p pokered-core --lib` | 2650 passed |
| `cargo test -p pokered-audio --lib` | 97 passed |
| `cargo test -p pokered-app --features debug-server --lib` | 112 passed, including the real no-audio CHAN1 WaitMusic regression |
| Native debug-server binary build | passed |
| `cargo check -p pokered-tui` | passed |
| `gen_event_graph` | regenerated from the split's own scenes; 248 maps, 1345 storylines; existing Daycare dynamic-argument and VermilionGym run_js unknown constructs remain |
| Fixed capture on master and branch | 1 capture test passed on each revision; matching frames 8 and 20 |
| Fresh power-on playthrough to m49 | two attempts blocked at m03 by `NavError: warped out of RedsHouse1F -> RedsHouse2F`; fresh master m03 reproduces the same failure |
| Full seeded subsystem scenarios | 10/11 passed; s10-npcs blocked by the same house navigation failure, reproduced by master `scenarios.py --only s10` |

The seeded scenarios include capture, blackout, forced switching, menus,
options and a save/CONTINUE round-trip in a fresh process. These results do
not establish a full story playthrough; m49 remains unverified here.

The before/after harness is saved as `capture.rs`; copy it temporarily to
`crates/pokered-app/tests/capture_story_fidelity.rs` to reproduce it. See
`docs/screenshots/fidelity-story-split/README.md` for the fixed state and
capture commands. No old screenshots or historical test results were reused.

# Five-PR local integration verification

This report describes the **combined local integration branch**, not any
individual PR's validation. No sixth PR is published. The exact reviewed
branch heads and freshly run test counts are in `validation.json`.

The base is current master `cb131bb` (including #110 Chinese pagination).
The combined source being split is #111 at `0e6c005`.

## Verification

| Suite | Passed | Failed | Ignored |
|---|---:|---:|---:|
| Complete pokered-core crate | 3076 | 0 | 1 doctest |
| pokered-app lib, binary and integration tests with debug-server | 361 | 0 | 10 explicit screenshot captures |
| Complete pokered-ui crate | 102 | 0 | 0 |
| Complete pokered-renderer crate | 395 | 0 | 0 |
| Navigation unit tests | 36 | 0 | 0 |

After the final presentation update, the restored English 28/29-glyph
battle pagination regression also passed as a focused test (1 passed). The
update added only that test and capture documentation, with no production
changes; the table above records the preceding complete suites.

The combined TUI passed `cargo check -p pokered-tui`, including both the
systems party selector and presentation's party icon resources.

Commands were run from this worktree. A shared Cargo build cache was used;
Cargo rebuilt the integration's own source paths. These are new integration
results, not reused individual-PR test results.

```sh
cargo test -p pokered-core
cargo test -p pokered-core battle_pages_keep_hard_rows_and_pixel_overflow --lib
cargo test -p pokered-app --features debug-server --lib --tests
cargo test -p pokered-ui
cargo test -p pokered-renderer
cargo check -p pokered-tui
PYTHONPATH=scripts python3 -m unittest \
  scripts.test_playthrough_navigation scripts.test_playthrough_interactions
cargo run -p pokered-agent --bin gen_event_graph
```

Full story completion is not established by these suites. Individual story,
navigation and presentation records describe their fresh-playthrough scope
and the master-reproduced m03 house navigation limitation.

## Merge sequence and conflict handling

Recommended order using the **final heads** of the five PRs:

1. battle (`fix/fidelity-battle-split`)
2. story (`fix/fidelity-story-split`)
3. systems (`fix/fidelity-save-link-split`)
4. presentation (`fix/fidelity-presentation-split`)
5. navigation (`fix/fidelity-navigation-split`)

Battle, story and navigation merged without conflicts. The systems merge
required resolution in app/game.rs, overworld/screen.rs, overworld/update.rs
and tui/game.rs. `restore_loaded_save_flags` is authoritative: its internal
`restore_system_save_state` replaces the earlier explicit restoration call.
Keep the story's bag quantity and sequencer status inputs, the systems'
provenance/PC storage/link behavior, and all distinct regression tests.
The identical externally suspended trade test need only remain once.

The presentation production merge was conflict-free. A later systems
increment added the missing TUI party-selector drawing, and withdrew an
accidentally shared ordinary PartyScreen resource hunk. Applying that
increment *after* presentation temporarily reverted its resource arguments;
the integration explicitly retained presentation's `resources.as_mut()` and
`frame_count`. Merging the final systems head *before* presentation avoids
that incremental-history side effect.

## Generated graph and source audit

Both story and systems independently regenerated their own checked-in graph.
Their graph changes merged automatically. Running `gen_event_graph` against
all combined scenes produced 3278 edges, 248 maps and 1345 storylines, with
**no tracked graph diff**. The resulting graph is byte-for-byte identical to
#111's graph. The generator's existing Daycare dynamic-argument and VermilionGym
run_js unknown constructs remain one occurrence each.

After any conflict resolution affecting scenes, regenerate the graph with
`cargo run -p pokered-agent --bin gen_event_graph`; do not choose either
branch's whole generated file without checking the merged scenes.

The source audit checked changed-file ownership and then inspected remaining
source-to-union code differences. Save serialization/import, snapshot fields
and battle settlement/writeback match the combined source exactly, including
current-box synchronization and persistent runtime flags. The app's settled
battle outcome still reaches `resume_script_after_battle`. The audit found
and corrected the TUI party-selector drawing omission in the systems PR.

Expected source differences retain #110 Chinese pagination, protected names,
Chinese description/layout tests, and the systems' additional TUI trade movie
support. Original combined audit documents and its old screenshots are not
reused. The audit also restored the original English battle-pagination regression
test in the presentation PR and verified it against the combined branch.

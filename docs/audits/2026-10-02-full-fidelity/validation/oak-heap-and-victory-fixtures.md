# Final integration: Oak parcel heap and Victory Road flag assertions

The integrated library run exposed one outdated flag assertion and one real
lazy-AST size regression. Neither fix changes the original gameplay sequence.

## Victory Road

Original `scripts/VictoryRoad2F.asm:17-19` clears only
`EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH` on entry. The current
`maps/VictoryRoad2F/script.scene:24` already performs that exact reset.
The event name maps to canonical bit `0x917` in
`crates/pokered-data/src/event_flags.rs:1009`.

`OverworldScreen::script_flags()` delegates to `EventFlags::to_hashmap()`;
`crates/pokered-core/src/overworld/event_flags.rs:238-245` exports only set
canonical bits. Therefore a correctly cleared bit is absent, not `Some(false)`.
The regression now reads the canonical flag API and still asserts that both
second-floor switches remain set. No false-valued extra alias was introduced.

## Oak parcel scene

The restored rival arrival/departure paths add three nested player-row
conditionals to the parcel branch. Its serialized lazy function grew from
10,941 bytes in the earlier build to 13,880 bytes, exceeding the existing
12 KiB limit intended to bound GBA AST allocation. The limit remains unchanged.

The build now emits specialized parcel functions for row 1, row 3, and all
other rows. The player does not move while this modal scene runs: all movement
commands target the rival NPC. Selecting the arrival and departure paths from
the initial player row preserves command order, dialogue, and flags while
omitting the unselected AST branches.

A standalone Rust probe compiled the exact `select_parcel_row` function from
`build.rs` against cached DSL/JSON dependencies, without running Cargo or
acquiring the shared build lock. On the integrated 13,880-byte parcel AST it
produced:

| Player row | Bytes | Statements |
| --- | ---: | ---: |
| 1 | 10,374 | 35 |
| 2 / other | 10,476 | 35 |
| 3 | 10,578 | 35 |

All three remained below 12 KiB and contained no `getPlayerY` calls. The
embedded-data regression checks the three emitted branches against the same
budget. The existing production lazy-versus-full-scene test now compares all
64 flag combinations, three bag states, both languages, and player rows
0/1/2/3/4, plus every Pokédex rating count. Root integration runs the Cargo
checks after this commit; the standalone probe verifies pruning and byte size,
not the complete game execution.

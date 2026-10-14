# Playthrough failure fixture capture

Fresh run 866 reached m01–m15, then failed m16 after Route6 training:
`heal_pokecenter` observed Ivysaur29 HP43/79 at VermilionPokecenter(3,3)
without an open nurse dialogue/choice. This change improves its diagnostics;
it does not fix or declare that gameplay failure resolved.

The driver requested `export_fixture`, which existed only in the test-only
stdio transport. The production TCP protocol rejected it, and one broad
exception handler silently discarded the missing fixture. `save_state`
returns only an in-memory slot/hash, and `save` would overwrite a persistent
resume checkpoint. Neither supplied a portable read-only fixture.

The TCP `export_fixture` command now returns SaveData JSON through the same
live-state clone used by SAVE, without updating the session or writing SRAM.
The failure collector independently records the original traceback, state,
NPCs, flags, agent observation, and fixture. Each unsuccessful command or
file write is recorded in `failure-capture.json` and stderr. A rejected
capture removes that artifact's stale copy from a reused directory. Older
binaries still retain successful observations and report the unsupported
export explicitly.

## Validation

- All 5 protocol tests pass, including export wire parse/serialization.
- The app's read-only export regression passes in both library and binary
  test targets. It checks the live location, injured HP/PP/money, unchanged
  stored SaveData, complete overworld/battle snapshots, frame, pending input
  queue, pending frame budget, and existing SRAM bytes.
- Three Python contracts pass: old protocol/partial failure with stale
  fixture cleanup, successful unwrapped SaveData output, and disconnected
  transport with the original traceback retained.
- Debug-server app build passes. This was scoped application testing;
  unrelated application suites were not rerun or claimed green.
- Two private real TCP exercises exported at frame120, captured all six
  diagnostic files, and booted the exported fixture successfully at the
  same map/coordinates with HP43/79, PP5/40/10/10, and money2081. Frame,
  complete observed state, runtime snapshot hash `b3cba103d1f1846a`, PNG hash,
  and pre-existing checkpoint SRAM bytes remained unchanged. No SAVE,
  debug SAVE, restore, or live state patch was used in these exercises.

Frozen build: clean `d2eface67ce5c632cbc18972891cda5ef6716728`,
`debug-server`, dev opt0/debug0, SHA-256
`ebd19ae8cd767403ee8901f5d4ea597a48141f2710a67e4b63ee24a4b2ad1adb`.
The later documentation commit does not relabel the compiled source.
[Build identity](build-identity.json), [verification](verification.json),
and [constructed fixture](fixture.json) retain the exact inputs and results.

A reproduction can boot `fixture.json` with `run --headless --speed 0
--skip-intro --snapshot fixture.json --debug-port PORT --no-audio --lang en`,
step120 frames, then send `{"cmd":"export_fixture"}`. The response's `data`
is loadable using `--snapshot`; `get_state`, `get_flags`, and
`get_agent_state` provide the separate runtime observations.

## Healing diagnosis remains open

Same frozen906 binary (`b50130e04db566195707c8c48fa57746fe71d34d8f9c0a3c62883e19c901c2bb`):
8 driven phase-offset controls, 4 default-speed real-time controls, and
2 controls after real Route6 wild Oddish13 fights all executed the original
W866 navigation/healing helper successfully and verified ordinary SAVE
HP79/79 and restored PP35/40/10/10. The prior wild battles reduced HP43→42
and TacklePP5→3 before healing.

These controls use explicit preboot construction (DVs29/98, HP statExp576,
other statExp0, EXP21022, matching story flags/money) rather than the missing
whole failure state. They rule out a reproducible simple injury or single
prior-wild-battle failure in those controls, but do not establish the original
long-run trigger. No numerical rule or timing budget was changed.

SaveData preserves persistent game state, not a full runtime/RNG snapshot.
During an interrupted battle, stored party values may still be battle-start
values; the separate state/agent observations expose live battle values.
Commands do not freeze a free-running process, so real-time artifact files
must not be assumed to describe one identical frame. These are debug tooling
changes with no on-screen output change.

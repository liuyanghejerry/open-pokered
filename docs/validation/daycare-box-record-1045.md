# Day Care box record

Day Care now preserves the Pokémon instance's stored `MON_CATCH_RATE` byte
when depositing and withdrawing it. Previously, deposit substituted the
current species' default, and withdrawal retained the constructor's default.
That loses valid instance data: a Magikarp evolved into Gyarados retains its
stored value of 255, while Gyarados's species default is 45.

Deposit uses the existing `effective_catch_rate` helper, preserving explicit
values including zero. Only an absent field in legacy native data uses the
species fallback. Withdrawal explicitly restores the byte from the Day Care
record. This preserves the existing save layout and applies no capture-rate
calculation change. The stored byte and the original battle's separate
`wEnemyMonActualCatchRate` have different roles; this validation establishes
record persistence and serialization, not an ordinary capture-probability
effect.

The original reference is `pret/pokered` revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`. Its `box_struct` contains the catch
byte and all five stat-EXP fields (`macros/ram.asm:7–25`), and `wDayCareMon`
uses that struct (`ram/wram.asm:2221`). `_MoveMon` copies all 33 box-record
bytes and recalculates returning stats with effort enabled
(`engine/pokemon/add_mon.asm:409–413,496–514`).

## Regression coverage

Validated integrated source: `98bad965d0f36a0bc412c8889e7d89950fede4bc`, based on
actual master `e323f16e1de75fdd8b94dc347f70cfbd82fa7b4e`, using engine
`e533a3d23a0cdbed1524a1ac91184733da4a6f7e`.

- `daycare_evolved_magikarp_keeps_its_full_box_record_and_catch_byte_through_sram`
  performs the actual evolution helper, confirms the retained 255 against the
  new species default of 45, and compares all 33 serialized bytes before
  deposit, in main data, in the exported SRAM buffer, and after SRAM import
  and withdrawal. Nonzero effort, current PP and PP Ups travel with the record.
- `daycare_preserves_explicit_catch_bytes_and_defaults_only_legacy_absence`
  covers explicit `Some(0)`, `Some(173)`, `Some(45)` and legacy `None` through
  deposit, SRAM export/import and withdrawal. Only `None` triggers the species
  fallback of 45 for the Gyarados fixture; explicit zero remains zero.
- `withdraw_preserves_stat_exp_for_returned_pokemon` corrects an old test
  that claimed an effort wipe and inspected the remaining party member.
  Production already preserved effort. The corrected test checks Charmander
  at resident index 0 and Bulbasaur at returned index 1, using different
  nonzero effort arrays, and checks the returned EXP gain exactly.

On the original source `99b2ee07352f6e37161380b418bd987d920c49c4`, based on
`e6150de4cf06bfd8a8bbbaf3e65ceb8ecb6e0ce5`, the first focused
`save::daycare_tests` module run passed **11 tests**. The complete
`pokered-core` suite then passed **3383 tests**: 2971 unit tests and 412
integration tests, with zero failures and one ignored doctest. Both runs used
a new dedicated target for this source, and the source, engine and GFX freezes
matched before and after. The Cargo process groups and runner exited. The
initial documentation-only commit preserved that validated source.

## Integration with actual master

The integrated source above includes merged PR #279. The two Day Care source
files are byte-identical to the original validated source; their patch against
actual master is unchanged. One additional complete `pokered-core` run passed
**3403 tests**: 2991 unit tests and 412 integration tests, with zero failures
and one ignored doctest. This includes all 11 Day Care tests and the inherited
FIGHT, capture and flinch regressions. The run reused this worktree's dedicated
target. The source, engine and GFX freezes matched before and after, and the
runner and Cargo process groups exited. Subsequent documentation changes
preserve these integrated non-document source bytes.

These checks use core fixtures and in-memory SRAM export/import. They do not
establish live Link interoperability, cold SRAM CONTINUE, a native Game
playthrough, App/CLI/GBA builds or official CI results. This pure persistence
logic change falls under the repository's screenshot exception. Day Care
dialogue changes and the separate Transform capture reader fix are outside
this patch.

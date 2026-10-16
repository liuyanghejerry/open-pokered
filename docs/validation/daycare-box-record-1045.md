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

Historical local integrated test source:
`98bad965d0f36a0bc412c8889e7d89950fede4bc`, based on then-master
`e323f16e1de75fdd8b94dc347f70cfbd82fa7b4e`, using engine
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

## Historical integration with master

The integrated source above includes merged PR #279. The two Day Care source
files are byte-identical to the original validated source; their patch against
actual master is unchanged. One additional complete `pokered-core` run passed
**3403 tests**: 2991 unit tests and 412 integration tests, with zero failures
and one ignored doctest. This includes all 11 Day Care tests and the inherited
FIGHT, capture and flinch regressions. The run reused this worktree's dedicated
target. The source, engine and GFX freezes matched before and after, and the
runner and Cargo process groups exited. The documentation-only publication
`79dff1a7` preserved these integrated non-document source bytes.

These local checks use core fixtures and in-memory SRAM export/import. They do not
establish live Link interoperability, cold SRAM CONTINUE, a native Game
playthrough, App/CLI/GBA builds or official CI results. This pure persistence
logic change falls under the repository's screenshot exception. Day Care
dialogue changes and the separate Transform capture reader fix are outside
this patch.

## Update after PR283 merged

The original publication `79dff1a7` passed all three original-attempt-1
workflows on historical base `e323f16e`: [CI](https://github.com/liuyanghejerry/open-pokered/actions/runs/38827973267),
[GBA ROM](https://github.com/liuyanghejerry/open-pokered/actions/runs/38827973089),
and [GBA performance](https://github.com/liuyanghejerry/open-pokered/actions/runs/38827973152).
The repository requires an up-to-date branch (`strict=true`), so the updated
head needs new naturally triggered CI; the old green checks remain attached
to their original head and base.

A normal merge of actual master `a393d6465a24355cfeab5d2d48eed6dbf9cfb288` produced
`eeab46ae2260eda4512f0db900e3493e7674deea`, without conflicts or manual source
changes. Both Day Care Rust files remain byte-identical to `79dff1a7`;
their production patch against the new base is also byte-identical to the
old PR patch. The already merged dialogue, confusion and Transform fixes
are inherited from the new base.

The base's non-document bytes equal the PR281+PR282+PR283 prefix
`8cd4ea152b67679fe9e10d707fddecb768799f87`. Every non-document byte of this
integrated candidate equals the complete private four-PR combination
`fb6f73dd79328c30dc345c214fa305498b4abc76`. That combination actually passed
**3411 core tests** (2999 unit + 412 integration, zero failures, one ignored
doctest), with identical source/engine/GFX freezes before and after. Its raw
result SHA-256 is
`4434b19e6555d0233fe4a4c4413b4aacacf1ca499f9038b5cb7c73f1084a93d4`,
and its independent terminal review is
`36c9a3226a66e4e7670a5d212ff1b80957ef9c3ec000b5b6581cc16e8edd5f17`.
This remains the earlier private-combination run, not a new execution on
this updated head. The historical 3403-test run likewise keeps its actual
source identity, `98bad965`.

The commit following that merge only updates this record. No Cargo, native
Game, App/CLI/GBA, cold SRAM CONTINUE or live Link run was added for this base
update. Official CI will evaluate the new published head separately.

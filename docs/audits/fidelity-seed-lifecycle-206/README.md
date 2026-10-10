# Explicit field seed survives session reconstruction

Actual master25c56ca drops a seed supplied through PokemonGame::set_seed before
NEW GAME or CONTINUE: both recreate OverworldScreen with a default random stream.
Existing determinism tests set their seed after constructing the field, so miss
that lifecycle. The fix reapplies self.seed only to each reconstructed overworld,
before Continue on-load callbacks. It does not call set_seed on the whole game,
reset the battle stream, or reseed returns from in-game menus/battles. With no
explicit seed the original default behavior is retained. GBA NEW GAME reuses its
existing overworld; its shared CONTINUE reconstruction receives the same guard.

Three new tests exercise the actual shared handle_transition paths. NEW GAME
and CONTINUE test seeds0/42/MAX against the original explicit field stream, while
START return verifies the consumed stream continues rather than restarting.
Correct pre-fix run executes3 tests: both reconstruction tests FAIL, return PASS.
After repair all8 integration tests PASS, including existing replay/fork, battle,
mid-script restore and unchanged-query map-warp tests. No test skips were used.

Two invalid initial observations are retained: a1 accidentally supplied Cargo
build options to the test harness (Unrecognized offline), and a2 selected an old
shared-target test binary, executing0 matching tests. Neither is bug evidence or
passing validation. a3 freezes the actual worktree-built original-production
binary and executes all3 new cases. Final a1 no-run output supplies the absolute
executable path, which is frozen/hash-recorded before all8 tests run.

Archive includes complete negative/positive logs, final source and patch, source
and frozen binary hashes; no executable, ROM or save is distributed. This is pure
RNG lifecycle logic/tooling and adds no renderer/UI/asset changes; paired visual
screenshots are not applicable. Current-head CI/GBA performance must independently
pass unchanged budgets before merge. This does not prove original ROM RNG parity,
all constructor NPC randomness, every script RNG state, or full-game replay from
power-on. Wider deterministic preparation and fidelity audits remain pending.

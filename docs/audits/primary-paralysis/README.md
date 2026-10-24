# Primary paralysis rules

Thunder Wave incorrectly rejected Electric targets, while primary paralysis
moves also incorrectly rejected Substitute and same-type targets. This repair
extracts the gameplay correction from [PR #107](https://github.com/liuyanghejerry/open-pokered/pull/107)
(`dca9b2363dbb59626c32aa982b254eb42c3a2b7c`) onto master `af6a12f8`.

The original [`ParalyzeEffect_`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/move_effects/paralyze.asm)
rejects an existing status, checks both target types for Ground only when the
move is Electric, and then performs its hit test. It has no Substitute or
same-type rejection. The damaging secondary paralysis path has separate rules.

Thunder Wave now has its own Ground guard. Stun Spore and Glare use the primary
paralysis rule without the secondary-effect guards. The test-only legacy oracle
and its RNG-stream predictor were corrected alongside the production rules.

The production-stack regression covers eight move/species combinations, each
with and without Substitute: Thunder Wave against Zapdos, Pikachu, Onix and
Nidoking; Stun Spore against Bulbasaur and Onix; Glare against Snorlax and Gastly.
On unchanged master it failed at Thunder Wave against Zapdos (`None` instead of
`Paralysis`). All 16 combinations pass with the repair, along with the existing
two-sided Substitute differential test.

Local verification:

- `cargo test -p pokered-core --locked --offline`: 3,660 passed, 0 failed,
  one existing ignored doctest.
- `cargo +nightly-2025-12-07 build --release --features perf-benchmark --locked --offline`
  from `crates/pokered-gba`: passed.
- All seven original GBA performance scenarios passed in mGBA 0.10.5, with full
  render/audio, fresh SRAM, and unchanged thresholds. Trainer battle entry is
  231 ticks against the original 232.3-tick limit.

[verification.json](verification.json) records source hashes, the failing
assertion, commands, and all GBA metrics. The full-core test preceded only a
related explanatory comment correction in `rules.ron`; the GBA build and
performance run used the final source bytes recorded there.

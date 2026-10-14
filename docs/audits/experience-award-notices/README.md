# Experience award attribution

Original `GainedText` in `engine/battle/experience.asm:342–365` chooses
`with EXP.ALL,` for the second EXP.ALL pass, including traded recipients.
The participant pass uses `a boosted` for traded recipients and otherwise
the ordinary wording. The original text fragments are in
`data/text/text_2.asm:1208–1224`, revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.

The native game previously displayed the ordinary wording for every award.
The fix carries the award kind with each gained notice and formats these two
qualifiers. Amounts, stat EXP, recipient order and level-up processing keep
their existing behavior. The existing Chinese localizer recognizes the two
new templates so they do not become mixed English/Chinese messages.

## Actual battle and save verification

Four cases use a real Viridian Forest Bug Catcher battle against Weedle6 and
Caterpie6, followed by the ordinary START → SAVE interaction. Each case was
repeated twice on the original and twice on each native build: original8,
native-before8 and native-after8. The healthy lead is Bulbasaur20; the bench
is Bulbasaur5 at HP1. Traded cases use OT7 and Cascade badge2, preventing
disobedience without a stat boost. EXP.ALL remains in the bag throughout.

| Case | Final lead / bench EXP | First enemy notices after repair |
| --- | --- | --- |
| Ordinary | 5593 / 135 | ordinary66 |
| EXP.ALL | 5558 / 167 | ordinary33, EXP.ALL16, EXP.ALL16 |
| Traded | 5659 / 135 | boosted99 |
| Traded + EXP.ALL | 5606 / 167 | boosted49, EXP.ALL24, EXP.ALL16 |

All eight native before/after pairs have identical fixtures, inputs,
capture frames, final party/bag/money and complete 32 KiB SRAM. The 88 party
bytes also match the corresponding original save in every case. Each
provider's two runs have identical inputs, observed states, PNGs and SRAM.
Money ends at 3060; Cut PP goes from30 to28. The source hooks observe actual
`PrintText` with HL pointing to `GainedText`, including the EXP.ALL-pass and
traded-boost flags; text-data labels are not treated as executable hooks.

Native before is clean master `018bd130ff44045e993c1cd992285fffda70341e`;
after is clean `98c98a880230f58641496b37b21df8909ab13a18`. Both are dev
opt0/debug0 builds with `debug-server`. Exact binary and PNG hashes, native
messages, source award flags and repeated-run checks are in
[verification.json](verification.json). These are captured source identities,
not a claim that a later documentation or merge commit was rebuilt.

The two paired screenshots show the same native screen, input prefix and
frame: traded at1208, EXP.ALL's first whole-party award at1476. Original
timing is independent; these checks do not claim ROM/native frame or RNG
alignment. Setups are constructed before input. Original map setup uses
EnterMap before the driver starts; verification performs no battle/EXP RAM
writes. Earlier historical source/native captures from audit872 remain
separate from the freshly built master-before pairs here.

## Reproduction and limits

Build `pokered-app` with `--features debug-server`, then run the audit-only
[native_capture.py](native_capture.py):

```bash
python3 docs/audits/experience-award-notices/native_capture.py \
  traded-exp-all 1 after /absolute/path/pokered-app COMPILED_SOURCE_COMMIT
```

Positional arguments are case (`solo`, `exp-all`, `traded`,
`traded-exp-all`), repeat, label, binary and compiled source identity.
`POKERED_EXP_NOTICE_EVIDENCE_DIR` optionally selects an evidence directory.
The driver constructs its scenario from [fixture-base.json](fixture-base.json)
before input, starts at the forest trainer's approach, drives the actual
battle, pauses each experience message with neutral frames, and saves through
the normal menu. Linux dependencies and headless debug capture are the same
as the repository's debug tooling. The original capture requires the private
reference ROM and prepared emulator map state; neither is distributed here.

Two production BattleScreen regression tests cover all four award variants
and EXP.ALL precedence/amount/order. Full core2864, battle-text10 and the
debug application build passed. The initial unit fixture omitted the enemy's
HP0 prerequisite and failed; fixing the fixture produced the reported pass.
Failed logs remain in the private audit directory. This audit does not
exercise level-ups, learning, evolution, simultaneous fainting or full mainline
completion. The current fresh mainline audit separately stopped at m16 and
must not be reported as a complete pass.

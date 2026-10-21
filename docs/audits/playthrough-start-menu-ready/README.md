# Wait for START-menu input readiness in the playthrough driver

The latest master first-clear run stopped at m18 on Route9 (4,8): the driver
expected a party menu but was still in START at the remembered Pokemon cursor.
The run passed m01–m17; source and binary hashes remained unchanged. It is a
failed full-clear attempt, not a passing m49 result.

`open_start` used to accept the menu's existence/cursor as sufficient and sent
its single A frame before `ready_for_input`. Original `home/start_menu.asm`
also calls DrawStartMenu before HandleMenuInput. The game's existing input
lock correctly rejects that early key; no game logic is changed here.

The helper now advances neutral frames until the explicit readiness flag is
true, before either navigation or confirmation. The wait is bounded to120
frames. Unexpected menus and timeout remain errors; the original assertion
that field moves must open a party menu is unchanged. There is no repeated
confirm, state setter, skipped milestone, or relaxed success condition.

The physical-input replay uses the existing NPC audit SRAM, normal boot and
CONTINUE. It first selects Pokemon and closes the menus normally to establish
the remembered cursor, then reopens at absolute frames3000..3003. For all four
phases the exact historical helper (`cfac0eae`) confirms while not ready and
returns still at START. A subsequent ordinary A after readiness opens party.
For all four phases the corrected helper confirms only when ready and returns
in the party menu. Both use the same unchanged game binary. Gzip records retain
all physical timelines and readonly observations; `comparison.json` highlights
the first confirm and result. This is a tool regression, not animation parity.

```sh
python3 -m unittest discover -s scripts -p 'test_playthrough*.py'
# 112 tests, including7 new readiness/error-boundary cases
python3 docs/audits/playthrough-start-menu-ready/replay.py \
  --binary /path/to/pokered-app --output /new/fixed-output --expect fixed
python3 docs/audits/playthrough-start-menu-ready/replay.py \
  --binary /path/to/pokered-app --output /new/base-output --expect base \
  --helper-ref cfac0eaef93dcd93ce911a3c58d3d5c7d7fb7838
```

No new screenshots are required: this changes the verification driver only,
not the game's on-screen behavior. The completed m49/HOF/credits/independent
CONTINUE verdict must still be established by a later fresh run.

# Choice input readiness regression

The m39 failure was a driver error: `AskFieldYesNo` already drew YES/NO
while `input_enabled=false`. The driver spent A before the first input poll,
then correctly rejected the still-open decision as an unfinished cutscene.
`choose()` now waits before cursor movement/confirmation and moves directly
toward the target index, supporting clamped as well as wrapping menus.

`evidence.json` records the original failure and six successful real-machine
answers using the unchanged normal CLI binary, seed42, speed0. All six gates
opened while their linked trainers stayed unbeaten. `fixture.json` is the
persistent SaveData exported at the real failure; it is not a full runtime/RNG
snapshot or an ordinary SAVE. Processes were stopped by the diagnostic owner.
Neither this fixture run nor the 105 Python tests proves a fresh full route.

From this checkout, build a normal pokered-app with desktop/debug-server, then:

```bash
python3 docs/validation/playthrough-choice-1114/replay.py \
  --binary /absolute/path/to/pokered-app --artifacts /new/artifact/directory
```

The replay boots through Continue and uses observations, ordinary button input
and dialogue advancement only. It does not set runtime flags, items, party,
position or SAVE via debug commands. A snapshot is loaded only before boot.
This package changes automation; it does not change game code or rendering.

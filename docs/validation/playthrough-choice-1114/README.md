# Playthrough choice regressions

The m39 driver confirmed YES/NO while `input_enabled=false`, so A was
ignored. `choose()` now waits before cursor movement and confirmation, and
moves directly toward the target index on clamped or wrapping menus.
The separate m44 Indigo nurse call now selects the actual HEAL label.

`evidence.json` records the original failures, all six real quiz machines
opening their gates with trainers unbeaten, and the committed m44 tail
healing the party and buying supplies. Both portable replay cases passed
using the same unchanged normal CLI binary, seed42 and speed0. The 105
Python regression tests also passed.

`fixture.json` and `indigo-fixture.json` contain persistent SaveData exported
at the actual failures. They are neither full runtime/RNG snapshots nor
ordinary SAVEs. The Indigo replay starts at the already-reached lobby and
executes the committed m44 tail; it does not replay Victory Road. Neither
fixture run proves a fresh full route. Diagnostic owners stopped the Games
after assertions; exit -15 is not natural game completion.

Build a normal pokered-app with desktop/debug-server, then run each case
with its own new artifact directory:

```bash
python3 docs/validation/playthrough-choice-1114/replay.py \
  --binary /absolute/path/to/pokered-app --case quiz --artifacts /new/quiz-directory
python3 docs/validation/playthrough-choice-1114/replay.py \
  --binary /absolute/path/to/pokered-app --case indigo --artifacts /new/indigo-directory
```

The replay boots through Continue and uses observations, ordinary button
input and dialogue advancement. It does not set runtime flags, items,
party, position or SAVE through debug commands. Persistent snapshots are
loaded only before boot. This package changes automation, game code and
rendering are unchanged.

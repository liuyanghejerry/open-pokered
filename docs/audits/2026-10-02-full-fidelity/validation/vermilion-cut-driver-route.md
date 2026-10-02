# Vermilion CUT driver recovery

Fresh attempt 3 passed m16 (the actual Captain/HM01 dialogue), then m17 failed
while searching from VermilionCity (15,17) to the gym. The facing and tree
coordinate were correct: downward tile (15,18) is cuttable `$3D`, in block `$35`.
The game's original/current block swap is `$35` → `$4C`.

This was a driver synchronization/cache error, not a CUT gameplay bug:

- Original `engine/overworld/cut.asm:49–60` prints UsedCutText before replacing
  the block and running AnimCut. Current `overworld/update.rs:903–916` likewise
  applies pending CUT on the next update after the dialogue closes.
- Debug `control_ready` (`pokered-app/src/game.rs:6540`) covers dialogue, scripts
  and warps. It does not wait for pending CUT or its 18-frame animation.
  The driver returned before that update, then failed its path search without
  advancing another frame. The controlled input probe confirms the tree is
  still `$35` immediately after dialogue completion.
- `Game.st()` only refreshed live map blocks when `smart_moves` was enabled;
  the ordinary continuous driver retained the tree's pre-CUT collision bytes.

The driver now advances 24 ordinary neutral frames after CUT dialogue and
refreshes its planning copy from every overworld observation. Battle observations
use a placeholder map name, so they are excluded from that cache update. Map
re-entry observations also restore a regrown tree in the planning copy.

Validation: 58 navigation/recovery/exploration unit tests pass, including
ordinary-driver CUT opening, tree regrowth and battle-placeholder cache safety.
The supplied final native binary was driven from a separate SaveData fixture via
CONTINUE, using real party-menu input, CUT, and walking. The tree changed to `$4C`
and the player reached **VermilionGym (4,17)**, frame 589 → 1039. All 38 recorded
commands are read-only observations or normal input/frame advancement. No warp,
flags, item injection or collision bypass is used during the route. This is a
controlled seeded route regression, not a fresh first-clear claim.

Reproduce from the repository root:

```sh
python3 docs/audits/2026-10-02-full-fidelity/validation/vermilion-cut-driver-route.py \
  --fixture docs/audits/2026-10-02-full-fidelity/validation/vermilion-cut-driver-fixture.json \
  --binary /path/to/pokered-app
PYTHONPATH=scripts python3 -m unittest scripts/test_playthrough_navigation.py \
  scripts/test_playthrough_recovery.py scripts/test_exploration_playthrough.py
```

The fixture contains one level-20 Ivysaur already taught CUT, Boulder/Cascade
badges, and position VermilionCity (15,17). Event flags are default. No product
source changed, no Cargo run was needed, and fresh attempt 4 was not started.

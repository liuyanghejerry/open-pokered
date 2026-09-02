# playthrough-regression — Milestone Playthrough as E2E Regression Test

Use this skill to regression-test the game engine end-to-end.
`scripts/playthrough.py` drives a headless game instance **from a real
power-on through story milestones m01–m10 using button input plus
debug-server observation only** — no `--skip-intro`, no `--warp`, no state
seeding. A green run proves the real engine paths (intro flow, warps, map
scripts, battles, story flags) still work after a change; a red run localizes
the regression to the surface a milestone exercises.

## Quick Start

```bash
# 1. Build the binary the driver launches. MUST be the debug profile
#    (the driver spawns target/debug/pokered-app directly) and MUST
#    carry the debug-server feature — without it the game only prints a
#    warning and the driver dies with ConnectionRefusedError after ~15s.
cargo build --bin pokered-app --features debug-server

# 2. List milestones
python3 scripts/playthrough.py --list

# 3. Run the chain scoped to your diff (stops after the named milestone)
python3 scripts/playthrough.py --until m06 2>&1 | tee /tmp/pt.log

# 4. Full run = the m01–m10 chain
python3 scripts/playthrough.py 2>&1 | tee /tmp/pt.log
```

Python 3 stdlib only — no venv, no pip. Default debug port is 9020
(`--port` to change). Success is the final line
`PLAYTHROUGH REACHED REQUESTED MILESTONE`.

## Scoping: which milestone guards which surface

Always run at least up to the last milestone that touches your diff — the
chain replays from power-on every time (that is the point), earlier
milestones are cheap; the m09 forest and the m10 grind dominate wall time.

| Milestone | Exercises | Run when the diff touches |
|-----------|-----------|---------------------------|
| m01 | language-select → title → main menu → NEW GAME | intro/screen flow, input accept windows |
| m02 | Oak speech, default naming, spawn | text advance, ShrinkPlayer, spawn state (`player_name=RED`, `RedsHouse2F`) |
| m03 | house stairs + door mat to Pallet Town | interior warps, exit-mat edge semantics (`extra_warp_check`) |
| m04 | Oak interception → OaksLab | map-edge NPC triggers, cutscene handoff, scripted walk |
| m05 | starter ball → dex preview → YES | `ShowPokedexEntry` screens, choice menus, party creation (`party_count=1`) |
| m06 | rival challenge battle | LOS/trainer battles, battle loop (PlayerMenu→FIGHT→move), post-battle scripts, money |
| m07 | Route 1 crossing → mart parcel | map connections, cross-map BFS driving, `@load` on-enter cutscenes, item flow |
| m08 | parcel delivery → POKéDEX | scripted delivery, event-flag flip changing Oak's dialogue branch |
| m09 | gate houses → forest → Pewter | gate warp chains (`last_map` semantics), forest trainer LOS fights |
| m10 | grind to L13 → Brock → badge | wild encounters, grass, Pokecenter heal flow, gym challenge; success = `EVENT_BEAT_BROCK` flag (3 internal attempts) |

Rule of thumb: pure data edits (moves, stats, text) → m05/m06/m10 suffice;
anything in warp/collision/script/flag code → run the full chain.

## Reading the output

Each milestone prints an evidence line plus its wall time (real values from
a verified run — m01–m03 take seconds; m09's forest battles and m10's grind
dominate a full run):

```
== m02: Oak speech + default names → bedroom
[m02] frame=1921 screen=overworld map=RedsHouse2F pos=(3,6) party=0 money=0 effect=None
   done (3.8s wall)
```

The evidence fields are the regression contract for that milestone
(`map=` where we ended up, `party=` party size, `money=` battle payouts,
`effect=` pending script effect). A mismatch in any field is a finding even
when the milestone's own asserts pass.

## Failure triage

| Symptom | Meaning | First suspect |
|---------|---------|---------------|
| `AssertionError` with a state dump (`assert s["map_name"] == "OaksLab"`) | engine state contradicts the milestone contract | strongest regression signal — check the diff in scripts/warp/flag code for that map |
| `NavError: no path … / did not converge` | planned route no longer walks | collision/warp regression, or `tools/map_data.json` went stale vs engine |
| `NavError: warp at (x,y) never fired` / `unexpected warp target` | warp didn't trigger or fired elsewhere | exit-mat / `last_map` semantics changed (screen.rs, special_terrain) |
| `condition '…' not reached in N frames` | expected screen/dialogue never appeared | cutscene or screen-flow regression |
| `NavError: cutscene blocked on choice […]` | script opened an unexpected choice | script ordering/branching change |
| `ConnectionRefusedError` at startup | binary lacks the `debug-server` feature or is stale | rebuild per Quick Start step 1 |

Triage loop for a real failure:

```bash
# 1. Reproduce with driver internals (nav plans, battle phases, pinch logic)
PT_DEBUG=1 python3 scripts/playthrough.py --until mNN 2>&1 | tee /tmp/pt-debug.log

# 2. Visual evidence (attach to the bug report)
python3 scripts/playthrough.py --until mNN --record-video /tmp/repro.mp4
# or PNG frames: --record /tmp/frames/ (assemble: ffmpeg -framerate 240 -i frame-%06d.png -r 60 out.mp4)
```

Then diff-read the suspected subsystem with the milestone table above. The
game's own stderr goes to `$TMPDIR/pokered-run-*/game.log`, but the driver
deletes that directory on a normal milestone failure — the log survives
only when the driver itself died before cleanup. For a persistent game log,
launch the game manually with `--debug-modules warp,event,overworld` (see
the pokered-debug skill) and drive the scenario with
`scripts/debug_drive.py`.

## Fresh runs vs `--resume`

- **Fresh run** (default): throwaway save, boots from power-on, writes no
  checkpoints. This is the *verdict* mode — a green fresh run is the result
  you report, and because m01–m03 take ~10 s total, the default iteration
  loop after an engine fix is simply a fresh scoped run.
- **`--resume`**: uses `scripts/.playthrough.sav` +
  `scripts/.playthrough.marker`; milestones the marker marks complete are
  skipped, each newly completed milestone checkpoints the save, and the
  driver taps the restored game through CONTINUE back into the saved
  overworld before the first executed milestone (`resume_reentry`).
  **Known limitation (verified 2026-09-02):** mid-chain continuation into
  m04 currently diverges — on a CONTINUE-restored PalletTown the player
  cannot step off the checkpoint tile (no script running, no NPC at the
  blocking tile), while the identical step works in a fresh run. That
  restore-vs-walk-in collision divergence is an open engine finding, not a
  driver bug to work around. Until it is fixed, treat `--resume` as
  checkpoint-recording only; don't gate a verdict on a resumed run.
- Reset the chain with `rm scripts/.playthrough.sav scripts/.playthrough.marker`.

## Flakes vs regressions

Battle RNG and wandering NPC patrols make runs near-deterministic, not
bit-exact. The driver self-heals (re-localization after drift, blackout
retry, m10 takes 3 gym attempts). Policy:

1. A failure is a regression only if it **reproduces on re-run** of the
   same scope. One clean pass on retry = flake; note it, move on.
2. Two failures in three attempts = treat as a regression.
3. m10 is the flakiest by construction (RNG battles + grind); m01–m08
   should be effectively deterministic — a failure there is a regression
   until proven otherwise.
4. **Never weaken the driver to make a test pass.** It encodes verified
   engine behavior (edge-triggered menus, door-mat/exit-mat semantics,
   `last_map` tracking). When driver and engine disagree, suspect the
   engine diff first, then `tools/map_data.json` staleness.

## Adding a milestone (m11+)

1. Write `def m11_xxx(g):` modeled on `m09_to_pewter` — navigate, then
   `assert` the engine state (call `g.evidence("m11")` so the contract is
   visible in logs).
2. Append to `MILESTONES` (`scripts/playthrough.py:1337`).
3. Encode any engine quirk you had to respect (input edge-triggering,
   staged warp chains, patrol bands) as a comment at the exact line —
   these comments are the engine-behavior record; delete driver workarounds
   only when the engine actually changed.
4. Verify the whole chain end-to-end fresh, then update the scoping table
   above and the list in `--list`.

## What this test does NOT cover

- Content past Pewter City (no m11+ yet).
- Rendering/audio correctness — use the screenshot CLI and the
  visual-verify skill; remember the AGENTS.md before/after screenshot rule
  for visual PRs.
- Save-format round trips — use `export-snapshot` / `import-snapshot` and
  the pokered-save-editor skill.
- Fine-grained battle math — covered by `pokered-core` unit tests instead.

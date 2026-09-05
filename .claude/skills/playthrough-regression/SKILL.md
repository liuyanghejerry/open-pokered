# playthrough-regression — Milestone Playthrough as E2E Regression Test

Use this skill to regression-test the game engine end-to-end.
`scripts/playthrough.py` drives a headless game instance **from a real
power-on through story milestones m01–m10 using button input plus
debug-server observation only** — no `--skip-intro`, no `--warp`, no state
seeding. A green run proves the real engine paths (intro flow, warps, map
scripts, battles, story flags) still work after a change; a red run localizes
the regression to the surface a milestone exercises. Its sibling
`scripts/scenarios.py` tests subsystems **outside** the linear chain
(battle/items/save/menus) by seeding state through the debug protocol —
see "Beyond the chain: the scenario suite".

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

## Beyond the chain: the scenario suite

`scripts/scenarios.py` is the non-linear sibling of the milestone chain.
Each scenario **seeds** a minimal state through the debug protocol's write
commands (`give_pokemon`, `give_item`, `start_wild_battle`, `warp`,
`save`) and drives ONE subsystem with real button input, asserting only
what the protocol reads back. A milestone proves "the game can be played
this far"; a scenario proves "this subsystem behaves as specified" — in
seconds, with no walk to get there.

```bash
python3 scripts/scenarios.py --list
python3 scripts/scenarios.py                 # full suite, ~1 min
python3 scripts/scenarios.py --only s05      # one scenario (id prefix ok)
python3 scripts/scenarios.py --skip s05      # all but the RNG-heavy one
```

| Scenario | Asserts |
|----------|---------|
| s01-bag-seed | `give_item` lands in `get_bag` with quantities; unknown items are rejected |
| s02-party-seed | `give_pokemon` appends in order; `party_count`; leader is first |
| s03-wild-run | `start_wild_battle` + RUN → overworld, player tile unmoved |
| s04-wild-win | winning a wild battle grants experience (`get_party` delta) |
| s05-wild-catch | battle BAG → Poké Ball → catch: party+1, correct species, balls spent |
| s06-blackout | total party KO → whiteout to the home fly point, party fully healed |
| s07-save-roundtrip | `save` cmd → separate fresh boot → CONTINUE restores map/pos/party/bag/money exactly |
| s08-start-menu | START menu opens; first entry opens the party screen; EXIT returns control |
| s09-options | OPTIONS text-speed toggle changes `text_speed_delay_frames` and persists across menu reopen |
| s10-npcs | `get_npcs` reports live, field-sane NPCs on the entered map |

Engine quirks the scenarios encode (same contract as the milestone
comments — don't weaken them without an engine change):

- **Battle menu is 2x2: FIGHT TL / PKMN TR / BAG BL / RUN BR** — BAG is
  `down,left` (`battle_loop` pins the other two corners with
  `up+left`/`down+right`).
- **A catch parks the dex-registration screen** over the overworld;
  dismiss with b/a taps before asserting `screen=overworld`.
- **New games start at ¥0** in this engine, so the blackout money
  penalty is not assertable headless yet.
- **Whiteout lands on PalletTown (5,6)** — the home fly point — not the
  bedroom.
- The battle BAG menu has no protocol snapshot (unlike FIGHT's
  `battle_moves`), so BagSelect is driven blind against
  `battle_phase == "BagSelect"`.

s05 is the only RNG scenario (catch rolls; ~0.03% all-miss with 20
balls). The chain's flake policy applies unchanged.

### BDD form: the same tests as acceptance specs

`scripts/bdd.py` runs the same kind of seeded subsystem tests written as
**Gherkin feature files** under `scripts/features/` — for when you want
the acceptance behavior readable (and writable) without reading driver
code:

```bash
python3 scripts/bdd.py --list            # parse + list all scenarios
python3 scripts/bdd.py                   # full suite (~1 min)
python3 scripts/bdd.py --only catch      # substring filter on names
```

The runner is stdlib-only and understands English Gherkin keywords plus
zh-CN (`功能/场景/假如/当/那么/而且/但是` — see
`features/blackout.zh.feature`). Scenarios from other languages or
constructs (Outline, tables, Background) fail the parse loudly instead
of being silently ignored.

The three layers stay strictly separated:

- **features/*.feature** — the acceptance specs (what must hold).
- **scripts/bdd_steps.py** — the step vocabulary: regex-matched
  Given/When/Then bodies that call the validated primitives. A new BDD
  test is usually *only* a new .feature file; add a step only when the
  vocabulary genuinely lacks the concept.
- **scripts/scenarios.py / playthrough.py** — the primitives
  (`boot_starter`, `throw_balls_until_caught`, `battle_loop`, …). The
  BDD layer never re-implements driver behavior.

Gotchas learned from the first suite: `And`/`But` inherit the previous
keyword (a driving step after a `Then` must be written as `When`);
free text under a title is a description only until the block's first
step, after which unparseable lines are errors; step text starting
with a keyword (`Then are written…`) will be parsed as a step. Each
scenario runs against its own fresh game — same isolation, same flake
policy as scenarios.py.

### Constructed saves: boot straight into an arbitrary state

Protocol seeding (give_*/start_wild_battle) needs a running game and
can't reach money, badges, event flags, the Pokédex, or exact party
movesets. `scripts/save_builder.py` builds **snapshot JSONs** offline —
`SaveBuilder` mutates a canonical template (a real save exported from a
freshly booted game, because SaveData fields carry no serde defaults —
partial snapshots don't deserialize), and the engine boots it via
`run --snapshot` (`Game(save_path=…, snapshot=…)` in playthrough.py;
the save file remains the in-session write target):

```bash
# CLI one-shot
python3 scripts/save_builder.py -o /tmp/state.json \
    --party Charizard:36 --money 65000 --flag EVENT_BEAT_BROCK \
    --item POKE_BALL:20 --map PalletTown:5,6 --badges 0x0F

# finished-first-playthrough preset (champion team, 8 badges,
# story-complete flags, full dex, home spawn)
python3 scripts/save_builder.py -o /tmp/champion.json --preset champion

# module
sb = SaveBuilder(); sb.champion(); sb.write("/tmp/champion.json")
```

The builder parses the repo's own data sources (build.rs SPECIES_ORDER,
pokemon/*.json base stats, moves/*.json PP, item_list.json, maps/*/
map.json ids, event_flags.rs bit indexes) and computes Gen-1 stats with
the same formula as the engine's create_pokemon — Bulbasaur L5 comes
out 20/10/10/10/12 and Charizard L36 has 109 max HP, field-for-field.

**Flag names must come from `event_flags.rs`** — the builder rejects
unknown names. Some script-gated flags (`EVENT_BEAT_SS_ANNE_RIVAL`,
`EVENT_GOT_EEVEE`, `EVENT_GOT_LAPRAS`,
`EVENT_GAVE_SAFFRON_GUARDS_DRINK`) are runtime sidecar flags, NOT part
of the SRAM bitset a snapshot can carry; set those live with `set_flag`
after boot if a test needs them. The champion preset documents its
deliberate exclusions inline (see `CHAMPION_FLAGS` in save_builder.py):
legendaries stay catchable, item pickups stay collectable, and flags
that would break the world when set (`EVENT_LANCES_ROOM_LOCK_DOOR`
closes Lance's door, `EVENT_IN_PURIFIED_ZONE` disables the heal zone)
are left clear.

In BDD features, the `constructed save` / `champion save` vocabulary
covers the flow:

```gherkin
Given a champion save
When the game boots from the save
And the player walks north out of Pallet Town
Then the player is on Route1
And no story script intercepts the player
```

Boot from a snapshot goes through the same intro → CONTINUE path as a
saved game (`resume_reentry`), and constructed event flags reach the
live flag store (visible to `get_flags`). See
`features/construct.feature` and `features/champion.feature` for the
executable specs. Runtime-only extras (`__OBJ_HIDDEN_*`) are NOT part
of a snapshot — set those with `set_flag` after boot.

### Adding a scenario (s11+)

Seed with the protocol's write commands, drive with `g.d.drive` taps,
assert against `get_state` / `get_party` / `get_bag` / `get_flags` /
`get_npcs`. Register with `@scenario("s11-name", "one-line contract")`
and add a row to the table above. Never assert on engine internals: if
the protocol cannot observe it, extend the debug server first — the
scenario layer is deliberately black-box. If the test reads like an
acceptance rule ("given X, when Y, then Z"), prefer a BDD feature file
(`scripts/bdd.py`) over another s-scenario.

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
  skipped, each newly completed milestone checkpoints the save, and
  `resume_reentry` taps the restored game through CONTINUE back into the
  saved overworld before the first executed milestone. `--until mNN` also
  stops at a milestone that is merely *already satisfied*, so resuming
  past a fixed point stays cheap.
- Reset the chain with `rm scripts/.playthrough.sav scripts/.playthrough.marker`.
- Never report a resumed run as the final verdict — it replays against a
  save written by an older binary. Finish with a fresh run.

## Driver footguns (learned the hard way)

- **Stale games hijack fixed ports.** The driver spawns
  `pokered-app --debug-port …`; a leaked instance from a previous crashed
  run keeps that port busy and the next run silently "plays" the old
  game, failing in ways that look like engine bugs. The driver probes a
  free port by default (`--port` is an override) and kills the spawned
  game if connecting fails. If you suspect a hijack:
  `ps aux | grep pokered-app`.
- **The headless game keeps simulating in real time between commands.**
  Extra driver round trips can shift frame timing by a frame or two, and
  some transitions are frame-exact — e.g. arriving from the RedsHouse2F
  stairs occasionally bounces straight back to 2F when the timing lands
  wrong. Don't add polling or settlements inside `nav_to`'s loop to fix
  one milestone; handle the script in that milestone (see
  `m04_oak_intercept`) where the perturbation is scoped and expected.
- A single flaky failure is often one of the above, not a regression: see
  the flake policy below before blaming the engine.

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
- Save-format *tooling* interop — use `export-snapshot` /
  `import-snapshot` and the pokered-save-editor skill; the in-engine
  save → CONTINUE round trip itself is covered by s07.
- Fine-grained battle math — covered by `pokered-core` unit tests instead.

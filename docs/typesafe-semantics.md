# TypeSafe semantic judgments

TypeSafe's System One model (`jev-latest`) answers *typed questions* — pick
one of a set, is this true, how much — and returns probabilities rather than
generated text. There is nothing to parse, so the answer is either usable or
absent; a malformed reply is not a failure mode.

Two places in this repo need semantic understanding rather than computation
or a lookup, and previously approximated it with string work:

| Call site | Before | Now |
| --- | --- | --- |
| Agent goal → entity targeting (`policies.LocalExplorer`) | `"oak" in npc_name.lower()`, driven by a hand-maintained `NPC_HINTS` table | candidates enumerated in code from `get_nearby`; a Choice question picks the one the goal names |
| Dialogue assertions (`bdd_steps.py`) | `'I came here with some friends!' in text` | a Noul question asks whether the line *conveys* the claim |

## Setup

The key lives in the repo-root `.env` (gitignored — never commit it):

```
TYPESAFE_API_KEY=apikey_...
```

`typesafe.from_env()` reads it automatically; exporting the variable works
too. Optional overrides: `TYPESAFE_BASE_URL`, `TYPESAFE_DEFAULT_MODEL`.

Without a key nothing breaks: `SemanticJudge.from_env()` returns a disabled
judge, every judgment returns `None`, and each call site falls back to its
previous deterministic behaviour. That is what keeps CI and offline runs
unchanged.

## Using it

```bash
# Unit checks — mock HTTP only, no key, no game, no network
python3 -m unittest scripts.test_openpokered_typesafe

# Live probe: the same judgments against the real API, with the baseline
# the old string rule would have produced
python3 scripts/openpokered/typesafe_probe.py --compare-dialogue

# Semantic BDD scenarios (need a key, hence a separate directory)
python3 scripts/bdd.py --dir scripts/features_semantic
```

In a feature file:

```gherkin
When the player talks to the object ahead
Then the dialogue should convey that the speaker came to the forest with friends
And the dialogue should not convey that the speaker came alone
```

In the agent:

```python
from openpokered.semantics import SemanticJudge
explorer = LocalExplorer(seed=1, judge=SemanticJudge.from_env())
```

## The judgment-driven policy (T2J)

`judgment_agent.py` replaces a script rather than sharpening one. The
scripted tiers encode "where does this goal point?" as typed data:
`oracle.py`'s `FLAG_STRATEGIES` maps an exact flag name to a hand-written
strategy with a hardcoded `travel_to:<map>` inside, and `NPC_HINTS` maps a
task id to a name substring. To add a goal you write another entry.

T2J derives it instead. Per decision, code lists the actions executable
from where the agent stands — walk to a place the world graph reaches, or
interact with an entity the observation layer reports — and one Choice
question picks among them. The goal is the task's own natural-language
`name`, so nothing is keyed to a flag, an id, or a house style.

```bash
python3 scripts/openpokered/run_judgment.py all --compare
```

`--compare` runs `LocalExplorer` (the mechanical T2 baseline) on the same
task and seed, so the difference is attributable to the policy. Measured
across the nine task specs at seed 42:

| policy | tasks solved | env-steps | judgments |
| --- | --- | --- | --- |
| T2 `LocalExplorer` | 0/9 | 650 | — |
| T2J `JudgmentAgent` | 3/9 | 407 | 242 |

With the scripted oracle also in the table (seed 42, `--compare` runs all
three):

| policy | tasks solved | env-steps | judgment requests |
| --- | --- | --- | --- |
| T3 `Oracle` (hardcoded strategy per flag) | 8/8 | 10 | — |
| T2J `JudgmentAgent` | 4/9 | 401 | 203 |
| T2 `LocalExplorer` (mechanical sweep) | 0/9 | 650 | — |

`get-pokedex` is `oracle: false` in its spec — the oracle is not expected
to solve it, so it is excluded from the oracle's tally rather than scored
as a failure.

Per task, `OK (steps)` / `FAIL (steps)`:

| task | T3 oracle | T2J | T2 base |
| --- | --- | --- | --- |
| acquire-potion | OK (2) | FAIL (40) | FAIL (40) |
| beat-brock | OK (1) | FAIL (120) | FAIL (120) |
| beat-one-trainer | OK (1) | FAIL (80) | FAIL (80) |
| get-pokedex | n/a | FAIL (200) | FAIL (200) |
| get-starter | OK (2) | FAIL (60) | FAIL (60) |
| reach-pewter-city | OK (1) | OK (4) | FAIL (30) |
| reach-viridian-city | OK (1) | OK (2) | FAIL (20) |
| talk-to-oak | OK (1) | OK (18) | FAIL (40) |
| win-wild-battle | OK (1) | OK (2) | FAIL (60) |

`fallbacks=0` throughout: no judgment request failed, so every difference
is decision quality rather than transport.

**What is actually limiting it.** An earlier run ended two tasks in
`no_actions_left`, which read as "the agent ran out of things to try", so
the candidate set was given an escalating hop radius
(`--hop-budget 1` widening to `--max-hop-budget 3` once the narrow set is
exhausted). Measured, **that was the wrong diagnosis**: `esc=0` on all
nine tasks — the widening never fired, because the binding constraints
are elsewhere:

- **the judgment budget** — `get-pokedex` spent its 60-judgment cap, and
  with `--max-judgments 400` it spent 160 judgments and 200 steps without
  solving anything. Raising the cap just moves it to `max_steps`. It is
  not short of actions; it is short of a plan, and it is the task the
  hand-written oracle also declines;
- **the task step budget** — `acquire-potion` (40), `get-starter` (60),
  `beat-one-trainer` (80) and `beat-brock` (120) all end at `max_steps`,
  with the judgment count well under the cap.

So the escalation is correct and tested, but it did not move the number:
of the four tasks T2J solves, three were already solved before it, and
the one it added (`reach-pewter-city`) came from the *visited* annotation
in the place descriptions, not from widening. The lesson is worth keeping:
a plausible-sounding failure reason is not a diagnosis, and `run_judgment`
now separates `judgment_cap` from `no_actions_left` so the two cannot be
confused again.

**Where the gap actually is.** The oracle's advantage is not its step
loop — it is that a maintainer already worked out *which* map and *which*
entity each goal needs, and wrote that down.

### The script index, and a wrong first cut

That knowledge turns out to be mechanical. The scene semantics the debug
API already exposes record, per storyline, which flags it `sets` and
which items it `gives`, so "which map satisfies this goal" is a lookup.
`ScriptIndex` builds that index once per run (248 maps, ~5 s) and
annotates the candidate places with it — code does the lookup, the
judgment still chooses.

The first cut attached those facts only to candidates inside the local
hop radius. **An ablation showed it changed no decision at all**: same
4/9, same steps, same judgment count, the only difference 357 input
tokens. The reason is structural — a goal's map is usually several hops
out (`EVENT_BEAT_BROCK` is set in PewterGym, five hops from PalletTown),
so the answer never reached the candidate list.

Offering the goal maps *regardless of distance* is what fixed it, and the
switch that made the first cut visible is kept as `--no-place-facts`.
Re-measured with both arms:

| task | with facts | without facts |
| --- | --- | --- |
| beat-brock | 60 steps, 7435 frames, **5 battles**, `judgment_cap` | 120 steps, 2703 frames, **1 battle**, `max_steps` |
| reach-pewter-city | OK, 4 steps | OK, 6 steps |
| other seven | identical | identical |
| **total** | **4/9** | **4/9** |

So the facts are demonstrably used — a trace shows the agent going
`PalletTown → Route1 → travel_to:PewterGym` directly off the annotation,
where without them it never leaves the starting area — and they make
`reach-pewter-city` more efficient. But they do not move the pass count.
The cost is real: 222 judgment requests and 119 608 input tokens versus
205 and 114 213.

### The bottleneck the facts exposed

With the destination solved, `beat-brock` *reached* PewterGym and then
looped: `interact_with:warp:0` (its own door) → `travel_to:PewterCity` →
`interact_with:warp:2` (the door back in) → `travel_to:PewterGym` → …

Tracing it turned up four separate defects, all of them in what code
offered rather than in what the model chose:

1. **Warps were interaction targets.** `interact_with` only resolves a
   warp from its own tile, so offering one produces a no-op that costs a
   decision and is then retired. Leaving a map is already `travel_to`.
2. **The candidate filter was adjacency.** `interactable` means
   `distance <= 2` step units (`nearby.rs`), so filtering on it hid every
   trainer across a room. `agent_interact_with` navigates to an id
   (`agent_nav.rs:522`), so distance was never a reason to withhold one.
   This is what made a gym leader stop being a candidate at all.
3. **A gym leader had no name to match.** PewterGym's entry is
   `{"spriteName": "SuperNerd", "trainerClass": "Brock"}` — Gen 1 reuses
   sprites — and `nearby.rs` labelled non-`is_trainer` NPCs by sprite
   name, so the entity an agent needs to find Brock was called
   "SuperNerd". The goal says "Defeat Brock"; nothing on offer did.
   Fixed in the observation layer: prefer `trainer_class` when the map
   data carries one, since `is_trainer` answers "does this NPC challenge
   you on sight", a different question from what to call them.
4. **Retirement was cleared on every map change.** For an agent
   oscillating between two maps, that re-armed every failure on arrival,
   so it retried the same dead action indefinitely. Retirements are now
   keyed by `(map, action)` and never cleared.

Re-measured, same nine task specs at seed 42:

| policy | solved | env-steps | judgment requests |
| --- | --- | --- | --- |
| T3 `Oracle` | 8/8 | 10 | — |
| T2J `JudgmentAgent` | **6/9** | 281 | 169 |
| T2 `LocalExplorer` | 0/9 | 650 | — |

Restricted to the eight tasks the oracle is asked to solve, that run shows
T2J going from **3/8 to 6/8**, picking up `acquire-potion` and
`beat-one-trainer`. Input tokens rose to 148 444, because a wider
candidate set is a longer question.

### The judgment is stochastic, and that invalidates single-run comparisons

Those numbers are **one run per task**, and they should not be read as a
measurement. Three identical requests produce different probabilities:

```
call 0: interact_with:npc:0  p=0.67   travel_to:ViridianCity  p=0.31
call 1: interact_with:npc:0  p=0.69   travel_to:ViridianCity  p=0.28
call 2: interact_with:npc:0  p=0.66   travel_to:ViridianCity  p=0.31
```

Same model (`jev-1.13.0`), same state, same question — a spread of about
three points. The game is deterministic given the action sequence
(`--speed 0`, `--seed 42`; four repeats of `win-wild-battle` gave
byte-identical traces, 1047 frames each), so a wobble only matters when
two options are close. When it is, the argmax flips, and a single flipped
decision at the start of a run sends the deterministic game down a
different path entirely.

`win-wild-battle` demonstrates it: it passed in one run and failed in the
next, in both state arms, with identical code on the thin path. The frames
tell the story — 873 when it passed, 1047 when it failed. That is a coin
flip on an early near-tie, not a change in the policy.

So the honest statement is: **the four vocabulary defects above are real
and evidenced by traces** (Brock is now named and targeted; the agent no
longer shuttles through doors), but **the size of their effect on the pass
count is not measured**. Every single-run delta reported in this document
before this section — 3/8, 4/9, 6/8 — carries that uncertainty.

Two consequences worth acting on:

- **Compare policies over repeats or seeds, never one run.** `--runs N`
  repeats a seed, which measures exactly this noise.
- **`--act-margin` exists for this.** A nonzero margin makes the policy
  decline to act on a near-tie and take the deterministic action instead,
  which removes the coin flip at the cost of occasional timidity. It
  defaults to 0 (off) because it has not been measured at scale yet.

### Rich state: measured, and it did not help

The state handed to each judgment used to be the map, the position, the
mode, and a party *count*. It now assembles what a player would actually
look at — every party member's species, level, HP and status, the bag, the
badge count, which maps have been visited, and which actions have already
failed here — and hands that over as one named object
(`story_state`, ablated with `--thin-state`).

Measured over three repeats of all nine tasks, which is the minimum
needed given the noise above:

| state | runs solved | judgment requests | input tokens |
| --- | --- | --- | --- |
| rich (`story_state`) | **15/27** | 594 | 565 641 |
| thin (map/position/mode/count) | **15/27** | 568 | 473 920 |

**No difference, at 19% more input tokens.** The mechanism works and is
tested; the hypothesis behind it does not hold. The likely reason is that
the decision was never bottlenecked on what the party or the bag knows:
the candidate list already carries the actionable information, and every
remaining failure observed here is a vocabulary or navigation problem
(a trainer the agent cannot walk to), not a missing fact about the party.

This is worth keeping even though it did not pay off. It is the control
that makes the place-facts result meaningful, it costs one flag to ablate,
and if a future task does hinge on party condition or inventory the state
is already there. But it should not be described as an improvement, and
the default could reasonably flip to thin.

**What still fails.** `get-starter` fails at the task's own 60-step
budget, and `get-pokedex` is `oracle: false` — the hand-written reference
declines it too.

`beat-brock` reaches the gym, correctly targets `npc:0` (now labelled
`Brock`), and gets wedged. Instrumented, the sequence is:

1. `interact_with:npc:0` from the entrance works — it walks from (4,13)
   to (4,6), where the junior trainer intercepts it
   (`navigation.result: entered_dialogue`). That is correct game
   behaviour: the trainer blocks the corridor to Brock.
2. From then on the player **never moves again**. Every action reports
   `before == after` with `invalid: false`, so each one is retired and the
   agent oscillates between the gym and the city until its budget runs
   out. All five gym actions end up retired.
3. The game state at that moment looks clean — `screen: overworld`,
   `script_running: false`, `script_awaiting_battle: false`,
   `dialogue_state: null`, `player_movement_state: "Idle"`.
4. `interact_with` on any *distant* target then returns
   `result: "dialogue"` with `frames: 0` and no movement. It does not
   report `blocked`, so the agent cannot tell "the path is shut" from
   "something happened".

So the remaining gap is narrower than "navigation inside a building", and
the open question is *why the interaction layer reports `dialogue` with
zero frames rather than `blocked`* — that single misleading result is what
makes the wedge invisible to the policy. Two candidates, neither yet
confirmed: the client's `skip_dialogue` during interception may dismiss
the trainer's challenge before the battle is scheduled, and
`agent_nav`'s blocked-path path may be returning the wrong
`InteractResult`. This needs a look at `agent_nav.rs` with the wedge state
reproduced under it, not more policy work.

**A caveat on the oracle column.** In a combined `--compare` run the
oracle scored 7/8, failing `get-starter`; run on its own it scores 8/8,
and 3/3 on that task alone. The two runs differ by 19 frames
(432 vs 451), so this is timing sensitivity under load, not a regression
from the changes here. Worth knowing before treating the oracle as a
fixed bar.

## Design rules

These come from the TypeSafe skill and are load-bearing:

- **Code owns the enumeration, the policy, and the action; the model owns
  only the judgment.** Candidates come from `get_nearby`, thresholds live in
  code, and the model can only pick an id it was offered. A candidate that
  is never listed cannot be chosen, so the scan radius has to cover the map.
- **A failed call degrades, it never invents an answer.** `None` from a
  judgment means "use your deterministic path", never "assume no".
- **Pass the state a reader would have.** A line does not say where the
  speaker is standing; a claim that leans on that ("came *here* with
  friends") is judged unsupported without it. `conveys(..., context=...)`
  exists for exactly this. Measured: the same claim scored p=0.26 with the
  line alone and p=0.96 once the map went in.
- **One narrow judgment per question.** Ask several independent ones in a
  single request (`decide()` does this for target + action) — they run in
  parallel and cannot see each other's answers.
- **An inconclusive answer is not a pass.** `Judgment.resolved` is false in
  the 0.4–0.6 band and the BDD steps fail on it rather than reporting an
  undecided claim as verified.

## What this does not do

- **Pixels.** The model reads text and JSON, so the 14 `visual_verify_*.rs`
  dumps and `visual_oracle.py` still need a human eye. Nothing here changes
  the `AGENTS.md` before/after screenshot rule.
- **Replace the deterministic oracles.** `verify_*_data.py` byte-exactness,
  flag/item counts and SHA-256 fixture pins stay exact — a semantic check is
  for claims about meaning, not a substitute for a precise assertion.
- **Auto-pass anything.** Every use is additive: `features_semantic/` is
  opt-in, `LocalExplorer` is unchanged unless a `judge=` is passed, and the
  semantic dialogue steps are new vocabulary in `bdd_steps.py` that no
  existing feature uses.

## Cost

Requests are one round trip each and ~0.8 s. The 13-request probe above
cost 4777 input / 393 output tokens.

T2J is the expensive consumer: one request per overworld decision, and a
decision that changes nothing still costs one. The nine-task comparison
above spent 242 requests / 133 031 input / 22 022 output tokens, of which
`beat-one-trainer` (60 judgments) and `get-pokedex` (60) are two thirds —
both are runs that exhaust `--max-judgments` without solving the task. The
input side dominates because every request re-sends the candidate list, so
**keeping the candidate set small is the main cost lever**, not the number
of decisions.

`SemanticJudge` tracks `calls`, `input_tokens`, `output_tokens` and
`errors`; `LocalExplorer` tracks `semantic_calls` / `semantic_hits`;
`JudgmentAgent` tracks `judgments` / `fallbacks` — which is how you tell
"the goal never needed a judgment" from "the judgment never landed" from
"the judgment landed and was ignored".

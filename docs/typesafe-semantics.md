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
cost 4777 input / 393 output tokens. `SemanticJudge` tracks `calls`,
`input_tokens`, `output_tokens` and `errors` for run reports;
`LocalExplorer` tracks `semantic_calls` / `semantic_hits`, which is how you
tell "the goal never needed a judgment" from "the judgment never landed".

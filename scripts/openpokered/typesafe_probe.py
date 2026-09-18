#!/usr/bin/env python3
"""Live TypeSafe probe — the checks in test_openpokered_typesafe.py run
against a stub; this runs the same judgments against the real API and
prints the numbers, including the cases where substring matching gives a
different answer from the semantic one.

Needs a key (`TYPESAFE_API_KEY`, or the repo-root `.env`). Costs a handful
of requests; not part of CI or of any test suite.

Usage:
    python3 scripts/openpokered/typesafe_probe.py
    python3 scripts/openpokered/typesafe_probe.py --compare-dialogue
"""
import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from openpokered.semantics import (NO_MATCH, SemanticJudge,  # noqa: E402
                                   render_candidate)

# A goal phrased the way a task name reads, over the entities an OaksLab
# scan actually returns. The id-slug/hint pair the policy used to need is
# nowhere in here.
CANDIDATES = [
    {"id": "npc:0", "kind": "npc", "name": "Oak",
     "position": {"x": 5, "y": 3}, "distance": 4, "interactable": True},
    {"id": "npc:1", "kind": "npc", "name": "Rival",
     "position": {"x": 8, "y": 6}, "distance": 7, "interactable": True},
    {"id": "item:2", "kind": "item", "name": "POTION",
     "position": {"x": 2, "y": 9}, "distance": 9, "interactable": True},
]

TARGETING = [
    # (goal, what the old "name contains substring" rule would do)
    ("Hear Oak's starter offer in his lab", "hit: npc:0"),
    ("speak to the professor who hands out POKeMON", "miss: no hint matches"),
    ("trade with the trainer who owns the POTION", "miss: no hint matches"),
    ("buy a bicycle from the shopkeeper", "miss: no hint matches"),
]

# Verbatim from content_regression.py's forest-npc case, plus probes of
# what substring matching cannot express.
DIALOGUE = [
    ("I came here with some friends! They're out for POKeMON fights!",
     ["the speaker arrived with other people",
      "the speaker's companions are battling",
      "the speaker is selling something",
      "the speaker came alone"]),
    ("I cam here with som friends! Theyre out for POKeMON fights!",
     ["the speaker arrived with other people"]),
]


def _line(label, detail):
    print(f"  {label:<46} {detail}")


def check_targeting(judge):
    print("\n== goal -> entity targeting ==")
    for goal, baseline in TARGETING:
        selection = judge.ask_for_entity(goal, CANDIDATES)
        if selection is None:
            _line(goal, f"NO ANSWER (errors={judge.errors})")
            continue
        picked = selection.choice
        label = "no match" if picked == NO_MATCH else \
            render_candidate(next(e for e in CANDIDATES if e["id"] == picked))
        _line(goal, f"-> {picked}  ({label})")
        _line("", f"margin={selection.margin:.3f} conf={selection.confidence:.3f}")
        _line("", f"old rule would have: {baseline}")


def check_dialogue(judge):
    print("\n== dialogue claims ==")
    for text, claims in DIALOGUE:
        print(f"\n  text: {text!r}")
        for claim in claims:
            verdict = judge.conveys(text, claim)
            if verdict is None:
                _line(claim, f"NO ANSWER (errors={judge.errors})")
                continue
            mark = "yes" if verdict.holds else "no "
            band = "" if verdict.resolved else "  [inconclusive band]"
            _line(claim, f"{mark}  p={verdict.probability:.3f}{band}")


def check_equivalence(judge):
    print("\n== same-meaning (translation/refactor drift) ==")
    pairs = [
        ("I came here with some friends!", "I came here with some friends!"),
        ("I came here with some friends!",
         "I came here\nwith some friends!"),          # reflow only
        ("I came here with some friends!",
         "I arrived here along with several friends."),  # reworded, same meaning
        ("I came here with some friends!",
         "I came here by myself."),                   # changed meaning
    ]
    for left, right in pairs:
        verdict = judge.same_meaning(left, right)
        if verdict is None:
            _line("pair", f"NO ANSWER (errors={judge.errors})")
            continue
        mark = "same" if verdict.holds else "DIFF"
        _line(f"{right!r}"[:46], f"{mark}  p={verdict.probability:.3f}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--compare-dialogue", action="store_true",
                    help="also run the dialogue and equivalence checks")
    args = ap.parse_args()

    judge = SemanticJudge.from_env()
    if not judge.enabled:
        print("no TypeSafe key: set TYPESAFE_API_KEY (or put it in .env)",
              file=sys.stderr)
        return 1

    check_targeting(judge)
    if args.compare_dialogue:
        check_dialogue(judge)
        check_equivalence(judge)

    print(f"\nrequests={judge.calls} in={judge.input_tokens} "
          f"out={judge.output_tokens} errors={len(judge.errors)}")
    for err in judge.errors:
        print(f"  ! {err}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

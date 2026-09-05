#!/usr/bin/env python3
"""Gherkin BDD runner for open-pokered — stdlib only.

Acceptance specs live as .feature files under scripts/features/, written
in Gherkin (English keywords, or zh-CN: 功能/场景/假如/当/那么/而且/但是).
Step *bodies* live in scripts/bdd_steps.py as regex-matched definitions
on top of the scenario/playthrough primitives, so a new BDD test is
usually just a new .feature file.

Each Scenario runs against its own freshly spawned headless game (same
isolation as scenarios.py): Given seeds state through the debug
protocol's write commands, When drives with real button input, Then
asserts only what the protocol reads back.

Usage:
    python3 scripts/bdd.py [--list] [--only NAME] [--dir DIR]
"""
import argparse
import re
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

DEFAULT_DIR = ROOT / "scripts" / "features"

# (keyword, kind) in match order — "Scenario Outline" must be tested
# before "Scenario"; `None` kind = recognized but unsupported.
KEYWORDS = [
    ("Scenario Outline", None),
    ("Examples", None),
    ("Background", None),
    ("Feature", "feature"), ("功能", "feature"),
    ("Scenario", "scenario"), ("场景", "scenario"),
    ("Given", "given"), ("假如", "given"), ("假设", "given"),
    ("When", "when"), ("当", "when"),
    ("Then", "then"), ("那么", "then"),
    ("And", "and"), ("而且", "and"), ("并且", "and"),
    ("But", "but"), ("但是", "but"),
]


class Step:
    def __init__(self, kind, text, file, line):
        self.kind, self.text, self.file, self.line = kind, text, file, line

    def __str__(self):
        return f"{self.kind.capitalize()} {self.text}"


class Scenario:
    def __init__(self, name, file):
        self.name, self.file, self.steps = name, file, []

    def label(self):
        return f"{self.file} :: {self.name}"


def parse_feature(path):
    """Parse one .feature into (feature name, [Scenario]). Raises a
    parse error with file:line for anything unsupported (outline,
    tables, docstrings) instead of silently misreading it."""
    feature = None
    scenarios = []
    cur = None
    last_kind = None
    for lineno, raw in enumerate(
            path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        for kw, kind in KEYWORDS:
            if not line.startswith(kw):
                continue
            rest = line[len(kw):].lstrip("：: ")
            if kind is None:
                raise SystemExit(f"{path}:{lineno}: unsupported Gherkin "
                                 f"construct '{kw}' (plain Scenarios only)")
            if kind in ("feature", "scenario"):
                if not rest:
                    raise SystemExit(f"{path}:{lineno}: empty {kw} title")
                if kind == "feature":
                    feature = rest
                else:
                    cur = Scenario(rest, path.name)
                    scenarios.append(cur)
                last_kind = None
            else:
                if cur is None:
                    raise SystemExit(f"{path}:{lineno}: step outside a "
                                     f"Scenario")
                if kind in ("and", "but"):
                    if last_kind is None:
                        raise SystemExit(f"{path}:{lineno}: '{kw}' with no "
                                         f"previous Given/When/Then")
                    kind = last_kind
                if not rest:
                    raise SystemExit(f"{path}:{lineno}: empty step text")
                cur.steps.append(Step(kind, rest, path.name, lineno))
                last_kind = kind
            break
        else:
            # Free text under a Feature:/Scenario: title is its
            # description — accepted while the block has no steps yet
            # (after that, an unparseable line is a typo, fail loudly).
            block = cur if cur is not None else None
            if block is None or not block.steps:
                continue
            raise SystemExit(f"{path}:{lineno}: cannot parse line "
                             f"(tables/docstrings unsupported): {line!r}")
    if not scenarios:
        raise SystemExit(f"{path}: no scenarios found")
    return feature, scenarios


def run_scenario(sc, bdd_steps):
    """Execute one scenario against a fresh game. Returns (ok, detail)."""
    ctx = bdd_steps.Context()
    t0 = time.time()
    try:
        print(f"== {sc.label()}", flush=True)
        for step in sc.steps:
            fn, kwargs = bdd_steps.match(step)
            print(f"    {step}", flush=True)
            fn(ctx, **kwargs)
        print(f"   PASS ({time.time()-t0:.1f}s)", flush=True)
        return True, None
    except Exception as e:
        print(f"   FAIL ({time.time()-t0:.1f}s): {type(e).__name__}: {e}",
              flush=True)
        return False, f"{sc.label()}: {type(e).__name__}: {e}"
    finally:
        ctx.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=str(DEFAULT_DIR),
                    help="directory of .feature files")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--only", default=None,
                    help="substring filter on scenario name")
    args = ap.parse_args()

    import bdd_steps  # noqa: PLC0415 — registers the step vocabulary

    feats = sorted(Path(args.dir).glob("*.feature"))
    if not feats:
        print(f"no .feature files under {args.dir}", file=sys.stderr)
        return sys.exit(1)

    all_scenarios = []
    for f in feats:
        _, scs = parse_feature(f)
        all_scenarios.extend(scs)

    if args.list:
        for sc in all_scenarios:
            print(sc.label())
        return

    picked = [sc for sc in all_scenarios
              if args.only is None or args.only in sc.name]
    if not picked:
        print("no scenarios selected", file=sys.stderr)
        return sys.exit(1)

    fails = []
    for sc in picked:
        ok, detail = run_scenario(sc, bdd_steps)
        if not ok:
            fails.append(detail)
    n = len(picked)
    print(f"BDD {n - len(fails)}/{n} SCENARIOS PASS"
          + (f" (failed: {len(fails)})" if fails else ""))
    return sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()

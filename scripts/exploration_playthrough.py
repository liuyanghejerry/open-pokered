#!/usr/bin/env python3
"""Milestone checkpoint exploration driver.

This is the non-completion sibling of ``playthrough.py``.  The main route is
still driven from power-on, but every completed milestone is saved as a
throwaway checkpoint.  Selected probes boot a copy of that checkpoint and
explore from it, so optional content and rejected gate attempts cannot change
the state used by the next milestone.

The probe manifest is deliberately declarative.  ``blocked`` probes use
real navigation followed by a real directional input at a story gate;
``destination`` probes walk to an optional destination and may interact with
one object.  A fixed random seed chooses a small sample of destinations at
each checkpoint.  Navigation/setup exhaustion is reported as ``inconclusive``
because it is not sufficient evidence of a game-content defect.

Examples:
    python3 scripts/exploration_playthrough.py --list
    python3 scripts/exploration_playthrough.py --until m10 --seed 73 \
        --samples 2 --artifacts /tmp/pokered-exploration
    python3 scripts/exploration_playthrough.py --only route22-gate-opens-after-brock
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import random
import shutil
import subprocess
import tempfile
import time

from content_regression import Session, require, ROOT
from playthrough import (
    BIN,
    DELTA,
    Game,
    MAPS,
    MILESTONES,
    NavError,
    resume_reentry,
)


MANIFEST = Path(__file__).with_name("exploration_probes.json")
VALID_DIRECTIONS = set(DELTA)
VALID_OPS = {"nav_to", "nav_to_map", "nav_warp"}


class ProbeIncomplete(RuntimeError):
    """The probe could not reach its observation point deterministically."""


class ProbeFailure(AssertionError):
    """The game contradicted a probe's explicit oracle."""


def _milestone_names():
    return {mid for mid, _, _ in MILESTONES}


def _check_coord(value, label):
    if (not isinstance(value, list) or len(value) != 2
            or not all(isinstance(v, int) for v in value)):
        raise ValueError(f"{label} must be [x, y]")


def load_manifest():
    """Load and validate the external exploration contract.

    Validation is intentionally strict: a malformed route must fail before a
    long milestone run starts, and a probe cannot silently introduce a direct
    debug warp (which would defeat the purpose of this mode).
    """
    manifest = json.loads(MANIFEST.read_text())
    if manifest.get("version") != 1:
        raise ValueError("unsupported exploration manifest version")
    known_milestones = _milestone_names()
    known_maps = set(MAPS)
    seen = set()
    by_checkpoint = {}
    for checkpoint, group in manifest.get("checkpoints", {}).items():
        if checkpoint not in known_milestones:
            raise ValueError(f"unknown checkpoint {checkpoint}")
        if not isinstance(group, dict):
            raise ValueError(f"checkpoint {checkpoint} must be an object")
        for kind in ("blocked", "destinations"):
            entries = group.get(kind, [])
            if not isinstance(entries, list):
                raise ValueError(f"{checkpoint}.{kind} must be a list")
            for probe in entries:
                if not isinstance(probe, dict) or not probe.get("id"):
                    raise ValueError(f"invalid probe under {checkpoint}.{kind}")
                probe_id = probe["id"]
                if probe_id in seen:
                    raise ValueError(f"duplicate probe id {probe_id}")
                seen.add(probe_id)
                expected_kind = "blocked" if kind == "blocked" else "destination"
                if probe.get("kind") != expected_kind:
                    raise ValueError(f"{probe_id} kind does not match {kind}")
                route = probe.get("route", [])
                if not isinstance(route, list) or not route:
                    raise ValueError(f"{probe_id} needs a non-empty route")
                for step in route:
                    if not isinstance(step, dict) or step.get("op") not in VALID_OPS:
                        raise ValueError(f"{probe_id} has an invalid route step")
                    op = step["op"]
                    if op == "nav_to_map":
                        if step.get("map") not in known_maps:
                            raise ValueError(f"{probe_id} has unknown map {step.get('map')}")
                    elif op == "nav_to":
                        if step.get("map") is not None and step.get("map") not in known_maps:
                            raise ValueError(f"{probe_id} has unknown nav_to map")
                    elif op == "nav_warp":
                        if step.get("from_map") not in known_maps:
                            raise ValueError(f"{probe_id} has unknown from_map")
                        if step.get("to_map") is not None and step.get("to_map") not in known_maps:
                            raise ValueError(f"{probe_id} has unknown to_map")
                    if "x" not in step:
                        raise ValueError(f"{probe_id} route step lacks x")
                    if "y" not in step:
                        raise ValueError(f"{probe_id} route step lacks y")
                if kind == "blocked":
                    attempt = probe.get("attempt", {})
                    if attempt.get("direction") not in VALID_DIRECTIONS:
                        raise ValueError(f"{probe_id} has invalid attempt direction")
                    expected = probe.get("expected", {})
                    if expected.get("map") not in known_maps:
                        raise ValueError(f"{probe_id} has invalid expected map")
                    _check_coord(expected.get("position"), f"{probe_id}.expected.position")
                else:
                    visit = probe.get("visit", {})
                    if visit.get("direction") not in VALID_DIRECTIONS:
                        raise ValueError(f"{probe_id} has invalid visit direction")
                    expected = probe.get("expected", {})
                    if expected.get("map") not in known_maps:
                        raise ValueError(f"{probe_id} has invalid visit map")
                    _check_coord(expected.get("position"), f"{probe_id}.expected.position")
        by_checkpoint[checkpoint] = group
    manifest["by_checkpoint"] = by_checkpoint
    manifest["probe_ids"] = seen
    manifest["probe_checkpoint"] = {
        probe["id"]: checkpoint
        for checkpoint, group in by_checkpoint.items()
        for kind in ("blocked", "destinations")
        for probe in group.get(kind, [])
    }
    return manifest


def _text_from_state(state):
    effect = state.get("script_effect") or {}
    dialogue = state.get("dialogue_state") or {}
    return effect.get("text") or dialogue.get("text")


def _validate_selection(args, manifest):
    selected_ids = set(args.only.split(",")) if args.only else None
    if selected_ids and not selected_ids <= manifest["probe_ids"]:
        unknown = sorted(selected_ids - manifest["probe_ids"])
        raise ValueError(f"unknown probe id(s): {', '.join(unknown)}")
    if selected_ids and args.until:
        limit = Game.milestone_index(args.until)
        beyond_until = sorted(
            probe_id for probe_id in selected_ids
            if Game.milestone_index(manifest["probe_checkpoint"][probe_id]) > limit
        )
        if beyond_until:
            raise ValueError(
                f"probe(s) occur after --until {args.until}: "
                f"{', '.join(beyond_until)}"
            )


class ExplorationSession(Session):
    """A content-regression Session booted from a saved checkpoint."""

    def boot_checkpoint(self, checkpoint):
        shutil.copy2(checkpoint, self.output / "checkpoint.sav")
        shutil.copy2(checkpoint, self.save)
        super().boot(reload=True)

    def snapshot(self, label):
        """Record the public evidence needed to reproduce a finding."""
        data = self.observe(label)
        return data


def settle_dialogue(session, max_rounds=300):
    """Advance active dialogue/effects and return all visible English text."""
    texts = []
    for _ in range(max_rounds):
        state = session.g.st()
        text = _text_from_state(state)
        if text and (not texts or texts[-1] != text):
            texts.append(text)
        if state.get("choice") is not None:
            return " ".join(texts), state["choice"]
        if state.get("dialogue_state") is not None:
            if (state.get("dialogue_state") or {}).get("waiting_for_input"):
                session.g.tap("a", 12)
            else:
                session.g.step(4)
            continue
        if state.get("screen") == "battle":
            return " ".join(texts), None
        if state.get("script_running") or state.get("active_script_effect"):
            session.g.step(4)
            continue
        return " ".join(texts), None
    raise ProbeIncomplete("dialogue/effect did not settle")


def run_route(session, route):
    """Execute manifest route steps using only walking/navigation helpers."""
    for step in route:
        op = step["op"]
        try:
            if op == "nav_to_map":
                prior_smart_moves = getattr(session.g, "smart_moves", False)
                if step.get("allow_ledges"):
                    session.g.smart_moves = True
                try:
                    session.g.nav_to_map(step["x"], step["y"], step["map"],
                                         avoid_grass=step.get("avoid_grass", True))
                finally:
                    session.g.smart_moves = prior_smart_moves
            elif op == "nav_to":
                session.g.nav_to(step["x"], step["y"], step.get("map"))
            elif op == "nav_warp":
                session.g.nav_warp(step["x"], step["y"], step["from_map"],
                                   step.get("to_map"),
                                   approach=step.get("approach", "walk"))
            else:  # load_manifest() already rejects this; defensive only.
                raise ProbeIncomplete(f"unsupported route operation {op}")
        except NavError as error:
            raise ProbeIncomplete(f"route setup stalled at {step}: {error}") from error


def assert_common_expected(session, expected, text):
    state = session.g.st()
    observed = (state["map_name"], state["player_x"], state["player_y"])
    target = (expected["map"], *expected["position"])
    if observed != target:
        raise ProbeFailure(f"expected position {target}, observed {observed}")
    for needle in expected.get("dialogue_contains", []):
        if needle.lower() not in text.lower():
            raise ProbeFailure(f"expected dialogue {needle!r}, got {text!r}")
    flags = session.cmd(cmd="get_flags")
    for flag, value in expected.get("flags", {}).items():
        if bool(flags.get(flag)) != bool(value):
            raise ProbeFailure(f"flag {flag} expected {value}, observed {flags.get(flag)}")
    if "money" in expected and state["money"] != expected["money"]:
        raise ProbeFailure(f"money expected {expected['money']}, observed {state['money']}")


def run_blocked_gate(session, probe):
    run_route(session, probe["route"])
    before = session.snapshot("before blocked gate attempt")
    attempt = probe["attempt"]
    session.g.d.drive([attempt["direction"]] * attempt.get("frames", 40),
                      frames=attempt.get("frames", 40) + 8)
    text, choice = settle_dialogue(session)
    if choice is not None:
        raise ProbeFailure(f"blocked gate opened an unexpected choice: {choice}")
    after = session.snapshot("after blocked gate attempt")
    assert_common_expected(session, probe["expected"], text)
    return {"before": before, "after": after, "dialogue": text}


def run_destination(session, probe):
    run_route(session, probe["route"])
    visit = probe["visit"]
    session.g.face(visit["direction"])
    before = session.snapshot("before optional destination")
    session.g.tap("a", 12)
    text, choice = settle_dialogue(session)
    if choice is not None:
        wanted = visit.get("choice")
        if wanted is None:
            raise ProbeFailure(f"destination opened an unplanned choice: {choice}")
        options = [str(option).upper() for option in choice["options"]]
        if str(wanted).upper() not in options:
            raise ProbeFailure(f"choice {wanted!r} absent from {choice['options']}")
        session.g.choose(wanted)
        more, leftover = settle_dialogue(session)
        text = " ".join(part for part in (text, more) if part)
        if leftover is not None:
            raise ProbeFailure(f"choice remained open after selecting {wanted}")
    after = session.snapshot("after optional destination")
    assert_common_expected(session, probe["expected"], text)
    expected_bag = probe["expected"].get("bag", {})
    bag = {item["item"]: item["qty"] for item in session.cmd(cmd="get_bag")}
    for item, value in expected_bag.items():
        if bag.get(item, 0) != value:
            raise ProbeFailure(f"bag {item} expected {value}, observed {bag.get(item, 0)}")
    return {"before": before, "after": after, "dialogue": text}


def run_probe(session, probe):
    if probe["kind"] == "blocked":
        return run_blocked_gate(session, probe)
    if probe["kind"] == "destination":
        return run_destination(session, probe)
    raise ProbeIncomplete(f"unknown probe kind {probe['kind']}")


class ExplorationRunner:
    def __init__(self, args, manifest, output):
        self.args = args
        self.manifest = manifest
        self.output = output
        self.main_save = output / "mainline.sav"
        _validate_selection(args, manifest)
        self.selected_ids = set(args.only.split(",")) if args.only else None
        self.g = Game(save_path=self.main_save)
        self.results = []
        self.rng = random.Random(args.seed)

    def _checkpoint_path(self, mid):
        path = self.output / "checkpoints" / f"{mid}.sav"
        path.parent.mkdir(parents=True, exist_ok=True)
        response = self.g.d.cmd(cmd="save")
        require(response["ok"], response)
        shutil.copy2(self.main_save, path)
        return path

    def _choose(self, mid):
        group = self.manifest["by_checkpoint"].get(mid, {})
        blocked = list(group.get("blocked", []))
        destinations = list(group.get("destinations", []))
        if self.selected_ids is not None:
            probes = [probe for probe in blocked + destinations
                      if probe["id"] in self.selected_ids]
        else:
            count = min(self.args.samples, len(destinations))
            probes = blocked + self.rng.sample(destinations, count)
        return probes

    def _run_checkpoint(self, mid, checkpoint, probes):
        entry = {"checkpoint": mid, "selected": [p["id"] for p in probes],
                 "probes": []}
        for probe in probes:
            probe_dir = self.output / "probes" / mid / probe["id"]
            probe_dir.parent.mkdir(parents=True, exist_ok=True)
            session = ExplorationSession(probe_dir)
            result = {"id": probe["id"], "checkpoint": mid,
                      "kind": probe["kind"], "status": "pass",
                      "contract": probe}
            began = time.monotonic()
            try:
                session.boot_checkpoint(checkpoint)
                session.g.smart_moves = Game.milestone_index(mid) >= Game.milestone_index("m11")
                result["evidence"] = run_probe(session, probe)
            except ProbeIncomplete as error:
                result.update(status="inconclusive", error=f"{type(error).__name__}: {error}")
            except Exception as error:
                result.update(status="fail", error=f"{type(error).__name__}: {error}")
                if session.g:
                    try:
                        result["failure_state"] = session.g.st()
                        result["failure_flags"] = session.cmd(cmd="get_flags")
                        result["failure_bag"] = session.cmd(cmd="get_bag")
                    except Exception as capture_error:
                        result["capture_error"] = str(capture_error)
            finally:
                session.close()
            result["wall_seconds"] = round(time.monotonic() - began, 3)
            (probe_dir / "result.json").write_text(
                json.dumps(result, ensure_ascii=False, indent=2) + "\n")
            entry["probes"].append(result)
            self.results.append(result)
            print(json.dumps({k: result[k] for k in ("checkpoint", "id", "status", "wall_seconds")},
                             ensure_ascii=False), flush=True)
        return entry

    def run(self):
        try:
            self.g.smart_moves = False
            self.g.wait("screen=language-select", 1800)
            # Reuse the canonical milestones' real power-on flow.  Calling the
            # functions directly keeps this mode a sibling, not a second route.
            for mid, desc, fn in MILESTONES:
                print(f"== {mid}: {desc}", flush=True)
                began = time.monotonic()
                self.g.smart_moves = Game.milestone_index(mid) >= Game.milestone_index("m11")
                if mid == "m05":
                    fn(self.g, self.args.starter)
                else:
                    fn(self.g)
                self.g.smart_moves = Game.milestone_index(mid) >= Game.milestone_index("m11")
                checkpoint = self._checkpoint_path(mid)
                probes = self._choose(mid)
                checkpoint_result = self._run_checkpoint(mid, checkpoint, probes)
                self._write_checkpoint_result(checkpoint_result)
                print(f"   mainline done ({time.monotonic() - began:.1f}s wall); "
                      f"explored {len(probes)} probe(s)", flush=True)
                if self.args.until == mid:
                    break
            return self._report()
        finally:
            self.g.close()

    def _write_checkpoint_result(self, entry):
        path = self.output / "checkpoints" / f"{entry['checkpoint']}.json"
        path.write_text(json.dumps(entry, ensure_ascii=False, indent=2) + "\n")

    def _report(self):
        counts = Counter(result["status"] for result in self.results)
        milestone_ids = [mid for mid, _, _ in MILESTONES
                         if not self.args.until or
                         Game.milestone_index(mid) <= Game.milestone_index(self.args.until)]
        checkpoint_coverage = {}
        for mid in milestone_ids:
            group = self.manifest["by_checkpoint"].get(mid, {})
            available = (len(group.get("blocked", [])) +
                         len(group.get("destinations", [])))
            selected = sum(1 for result in self.results
                           if result["checkpoint"] == mid)
            checkpoint_coverage[mid] = {
                "available": available,
                "selected": selected,
            }
        report = {
            "mode": "checkpoint-exploration",
            "engine_commit": subprocess.check_output(
                ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
            "binary_sha256": hashlib.sha256(BIN.read_bytes()).hexdigest(),
            "manifest_sha256": hashlib.sha256(MANIFEST.read_bytes()).hexdigest(),
            "seed": self.args.seed,
            "samples_per_checkpoint": self.args.samples,
            "until": self.args.until,
            "milestones": milestone_ids,
            "checkpoint_coverage": checkpoint_coverage,
            "uncovered_checkpoints": [mid for mid, coverage in checkpoint_coverage.items()
                                       if coverage["selected"] == 0],
            "results": self.results,
            "summary": dict(counts),
        }
        (self.output / "report.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        print(f"Report: {self.output / 'report.json'}")
        return report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true", help="list checkpoints and probes")
    parser.add_argument("--only", help="comma-separated probe IDs; run them at their checkpoints")
    parser.add_argument("--until", help="stop the mainline after this milestone")
    parser.add_argument("--starter", default="bulbasaur",
                        choices=["bulbasaur", "squirtle", "charmander"])
    parser.add_argument("--seed", type=int, default=1,
                        help="deterministic destination-sampling seed")
    parser.add_argument("--samples", type=int, default=1,
                        help="random destinations per checkpoint (blocked probes always run)")
    parser.add_argument("--artifacts", type=Path,
                        help="new directory for checkpoints, protocol traces and report")
    args = parser.parse_args(argv)
    manifest = load_manifest()
    if args.list:
        for checkpoint, group in manifest["by_checkpoint"].items():
            for kind in ("blocked", "destinations"):
                for probe in group.get(kind, []):
                    print(f"{checkpoint}: {probe['id']} ({probe['kind']}) - {probe.get('goal', '')}")
        return 0
    if args.samples < 0:
        parser.error("--samples must be non-negative")
    if args.until and args.until not in _milestone_names():
        parser.error(f"unknown milestone: {args.until}")
    try:
        _validate_selection(args, manifest)
    except ValueError as error:
        parser.error(str(error))
    output = args.artifacts or Path(tempfile.mkdtemp(prefix="pokered-exploration-"))
    require(not output.exists() or not any(output.iterdir()),
            "artifacts must be new or empty to preserve evidence")
    output.mkdir(parents=True, exist_ok=True)
    (output / "manifest.json").write_bytes(MANIFEST.read_bytes())
    runner = ExplorationRunner(args, manifest, output)
    report = runner.run()
    # Inconclusive means that a route or stochastic search budget did not
    # produce enough evidence; it is intentionally visible in report.json
    # but must not turn a useful exploratory sample into a content failure.
    return int(report["summary"].get("fail", 0))


if __name__ == "__main__":
    raise SystemExit(main())

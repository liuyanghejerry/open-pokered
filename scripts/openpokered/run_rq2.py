#!/usr/bin/env python3
"""RQ2 matrix runner (WP3): hierarchical planner + world-model ablation.

Conditions (design brief RQ2): button-level / skill-level (LLM tiers —
blocked on the exhausted HF quota, referenced from the RQ1 artifacts),
hierarchical planner (event-graph decomposition over M4 graph.json,
executed with T2 skills, travel_to allowed), oracle planner (scripted,
referenced from the calibration artifacts), and the ablation: the same
planner on a seeded 30%-edge-deleted event graph, ≥3 independent
deletion samples per task×seed.

    python3 scripts/openpokered/run_rq2.py                    # full matrix
    python3 scripts/openpokered/run_rq2.py --aggregate-only   # rebuild tables

Budgets follow the calibration convention (2× oracle median frames,
floor 2000); runs are scripted (no LLM) in `--speed 0` driven-only mode.
Resume: cells with an existing JSONL row are skipped (`--rerun-failed`
re-runs failures). Artifacts under target/agent/runs/rq2/ (gitignored).
"""
import argparse
import json
import statistics
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from openpokered.env import OpenPokeredEnv  # noqa: E402
from openpokered.metrics import RUNS_DIR, RunMetrics  # noqa: E402
from openpokered.planner import HierarchicalPlanner, ablate, load_graph  # noqa: E402
from openpokered.run_rq1 import (MATRIX_TASKS, SEEDS,  # noqa: E402
                              oracle_median_frames, oracle_reference_rows)
from openpokered.tasks import load_task  # noqa: E402

RQ2_DIR = RUNS_DIR / "rq2"
TASKS_DIR = Path(__file__).resolve().parent / "tasks"

ABLATION_FRACTION = 0.3
ABLATION_SAMPLES = (0, 1, 2)
BUDGET_FLOOR = 2000


def frame_budget(task_id):
    median = oracle_median_frames(task_id) or 7500
    return int(max(BUDGET_FLOOR, 2 * median))


def run_cell(path, task, seed, condition, graph, ablation_sample=None):
    budget = frame_budget(task["id"])
    task = dict(task)
    task["seed"] = seed
    task["max_steps"] = 100000
    metrics = RunMetrics(task_id=task["id"], seed=seed, tier=condition,
                         policy=HierarchicalPlanner.POLICY_NAME)
    metrics.extra["frame_budget"] = budget
    metrics.extra["graph_edges"] = len(graph.edges)
    if ablation_sample is not None:
        metrics.extra["ablation_fraction"] = ABLATION_FRACTION
        metrics.extra["ablation_sample"] = ablation_sample
    env = OpenPokeredEnv(speed=0)
    planner = HierarchicalPlanner(graph, seed=seed)
    try:
        env.reset(task)
        metrics.start(env.frame_count())
        success, reason = planner.run(env, task, budget)
        metrics.env_steps = env.env_steps
        metrics.invalid_actions = env.invalid_actions
        metrics.battles = planner.battles
        metrics.battles_won = planner.battles_won
        metrics.extra["plan_steps"] = len(planner.plan_steps)
        metrics.extra["candidates_tried"] = planner.candidates_tried
        metrics.finish(env.frame_count(), success, reason)
    except Exception as e:
        metrics.finish(env.frame_count(), False, f"{type(e).__name__}: {e}")
    finally:
        env.close()
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "a") as f:
        f.write(json.dumps(metrics.to_dict()) + "\n")
    tag = f" sample={ablation_sample}" if ablation_sample is not None else ""
    status = "OK " if metrics.success else "FAIL"
    print(f"[{status}] {condition} {metrics.task_id} seed={seed}{tag} "
          f"frames={metrics.frames_elapsed}/{budget} steps={metrics.env_steps} "
          f"battles={metrics.battles} cands={metrics.extra.get('candidates_tried')} "
          f"reason={metrics.failure_reason!r}", flush=True)
    return metrics


def _done_keys(out_dir):
    """Resume keys from one-file-per-cell JSONL names: stem → recorded
    seeds. Ablation cells carry the sample index in the stem
    (`<task>#s<k>`), so samples never shadow each other."""
    done = {}
    if not out_dir.is_dir():
        return done
    for path in sorted(out_dir.glob("*.jsonl")):
        done[path.stem] = {r["seed"] for r in RunMetrics.read_runs(path)}
    return done


def _rate(rows):
    if not rows:
        return "—"
    return f"{sum(1 for r in rows if r.get('success'))}/{len(rows)}"


def _median_frames(rows):
    wins = [r["frames_elapsed"] for r in rows if r.get("success")]
    return int(statistics.median(wins)) if wins else None


def _no_path_rate(rows):
    if not rows:
        return "—"
    np = sum(1 for r in rows
             if r.get("failure_reason", "").startswith(("no_path", "goal")))
    return f"{np}/{len(rows)}"


def aggregate(hier_rows, abl_rows, oracle_rows, rq1_blocked, wall_s):
    lines = [
        "# RQ2 — planning conditions",
        "",
        "Hierarchical planner over the M4 event graph (3213 edges; "
        "`sets`/`gives` producer search → travel_to + storyline trigger + "
        "auto-battle, requires-as-annotation). Ablation: seeded 30% edge "
        "deletion, samples 0-2 per task×seed. Budgets = 2× oracle median "
        f"frames (floor {BUDGET_FLOOR}). All runs scripted, --speed 0.",
        "",
        "| condition | success | median frames (wins) | no_path rate | "
        "model calls | tokens |",
        "|---|---|---|---|---|---|",
    ]
    for label, rows, note in (
            ("button-level (RQ1 T1 LLM)", rq1_blocked.get("T1", []),
             "blocked: HF quota"),
            ("skill-level (RQ1 T2 LLM)", rq1_blocked.get("T2", []),
             "blocked: HF quota"),
            ("hierarchical", hier_rows, ""),
            ("ablation-30%", abl_rows, "")):
        if note:
            attempted = len(rows)
            lines.append(f"| {label} | **blocked** ({note}; "
                         f"{attempted} attempted cell(s) before the wall) "
                         "| — | — | — | — |")
            continue
        fr = _median_frames(rows)
        lines.append(f"| {label} | **{_rate(rows)}** | "
                     f"{fr if fr is not None else '—'} | {_no_path_rate(rows)} "
                     "| 0 | 0 |")
    wins = [r for r in oracle_rows if r.get("success")]
    o_fr = int(statistics.median(r["frames_elapsed"] for r in wins)) if wins else None
    lines.append(f"| oracle (scripted, ref) | **{len(wins)}/{len(oracle_rows)}** "
                 f"| {o_fr if o_fr is not None else '—'} | 0/{len(oracle_rows)} "
                 "| 0 | 0 |")
    lines += ["", "## Per-task success (won / attempted)", "",
              "| task | hierarchical | ablation-30% | oracle (ref) |",
              "|---|---|---|---|"]
    by_task = {}
    for r in hier_rows:
        by_task.setdefault(r["task_id"], {}).setdefault("h", []).append(r)
    for r in abl_rows:
        by_task.setdefault(r["task_id"], {}).setdefault("a", []).append(r)
    or_by_task = {}
    for r in oracle_rows:
        or_by_task.setdefault(r["task_id"], []).append(r)
    for task_id in MATRIX_TASKS:
        cells = by_task.get(task_id, {})
        ors = or_by_task.get(task_id, [])
        lines.append(f"| {task_id} | {_rate(cells.get('h', []))} "
                     f"| {_rate(cells.get('a', []))} | {_rate(ors)} |")
    lines += ["", "## Failure attribution", "",
              "| task | condition | seed | sample | frames | reason |",
              "|---|---|---|---|---|---|"]
    for rows, cond in ((hier_rows, "hierarchical"), (abl_rows, "ablation-30%")):
        for r in rows:
            if r.get("success"):
                continue
            lines.append(f"| {r['task_id']} | {cond} | {r['seed']} "
                         f"| {r.get('extra', {}).get('ablation_sample', '—')} "
                         f"| {r['frames_elapsed']} | {r['failure_reason']} |")
    lines += ["", f"Total wall-clock: {wall_s:.1f}s.", ""]
    return "\n".join(lines)


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--condition", choices=["hierarchical", "ablation-30%"],
                   default=None)
    p.add_argument("--task", default=None)
    p.add_argument("--seed", type=int, default=None)
    p.add_argument("--rerun-failed", action="store_true")
    p.add_argument("--aggregate-only", action="store_true")
    args = p.parse_args(argv)

    t0 = time.time()
    if args.aggregate_only:
        hier = _latest_rows(RQ2_DIR / "hierarchical")
        abl = _latest_rows(RQ2_DIR / "ablation-30%")
        summary = aggregate(hier, abl, oracle_reference_rows(),
                            _rq1_blocked(), time.time() - t0)
        (RQ2_DIR / "SUMMARY.md").write_text(summary + "\n")
        print(summary)
        return 0

    graph = load_graph()
    print(f"event graph: {len(graph.edges)} edges; "
          f"ablation fraction {ABLATION_FRACTION}", flush=True)
    tasks = [args.task] if args.task else MATRIX_TASKS
    seeds = [args.seed] if args.seed is not None else list(SEEDS)
    conditions = [args.condition] if args.condition else ["hierarchical",
                                                         "ablation-30%"]
    for condition in conditions:
        out_dir = RQ2_DIR / condition
        out_dir.mkdir(parents=True, exist_ok=True)
        done = _done_keys(out_dir)
        for task_id in tasks:
            task = load_task(TASKS_DIR / f"{task_id}.json")
            for seed in seeds:
                samples = (ABLATION_SAMPLES if condition == "ablation-30%"
                           else (None,))
                for sample in samples:
                    stem = (task_id if sample is None
                            else f"{task_id}#s{sample}")
                    if seed in done.get(stem, set()) and not args.rerun_failed:
                        print(f"[skip] {condition} {task_id} seed={seed} "
                              f"sample={sample}", flush=True)
                        continue
                    g = graph
                    if condition == "ablation-30%":
                        g = ablate(graph, ABLATION_FRACTION,
                                   seed * 1000 + sample)
                    run_cell(out_dir / f"{stem}.jsonl", task, seed,
                             condition, g, ablation_sample=sample)

    wall_s = time.time() - t0
    hier = _latest_rows(RQ2_DIR / "hierarchical")
    abl = _latest_rows(RQ2_DIR / "ablation-30%")
    summary = aggregate(hier, abl, oracle_reference_rows(),
                        _rq1_blocked(), wall_s)
    (RQ2_DIR / "SUMMARY.md").write_text(summary + "\n")
    print()
    print(summary)
    return 0


def _rows_from(directory):
    rows = []
    if not directory.is_dir():
        return rows
    for path in sorted(directory.glob("*.jsonl")):
        rows.extend(RunMetrics.read_runs(path))
    return rows


def _latest_rows(directory):
    """Latest row per cell — --rerun-failed appends, so a cell can have
    several rows; the last one is the current truth."""
    by_cell = {}
    for r in _rows_from(directory):
        key = (r["task_id"], r["seed"],
               r.get("extra", {}).get("ablation_sample"))
        by_cell[key] = r
    return list(by_cell.values())


def _rq1_blocked():
    out = {}
    for tier in ("T1", "T2"):
        rows = []
        tier_dir = RUNS_DIR / "rq1" / tier.lower()
        if tier_dir.is_dir():
            for path in sorted(tier_dir.glob("*.jsonl")):
                rows.extend(RunMetrics.read_runs(path))
        out[tier] = rows
    return out


if __name__ == "__main__":
    sys.exit(main())

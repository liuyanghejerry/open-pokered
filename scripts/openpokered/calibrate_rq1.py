#!/usr/bin/env python3
"""RQ1 calibration matrix runner.

"Same Game / Same Initial State / Same Objective": every cell spawns the
same headless seeded game via OpenPokeredEnv on the same task spec; only the
agent tier changes.

- T3 semantic + world model: the M6 rule-based oracle (travel_to, world
  graph, script semantics). ALL runnable tasks; tier-1 tasks × 3 seeds,
  tier-2/3 tasks × 1 seed (M6 proved those at 3 seeds already).
- T2 symbolic + local navigation: LocalExplorer (policies.py) — no
  travel_to/world graph/route/semantics. reach-viridian-city,
  acquire-potion, talk-to-oak × 3 seeds. Budget: 2× the T3 median
  frames for that task; exceeding it records `step_budget`.
- T1 controller buttons only: ButtonRandomWalk — position/mode
  observation, press/drive actions, 20k-frame cap (`frame_cap`).
  reach-viridian-city × 3 seeds.

Artifacts: appends one JSONL row per run to
`target/agent/runs/rq1/<task>__<tier>.jsonl` (RunMetrics with tier +
policy fields), and rewrites `target/agent/runs/rq1/SUMMARY.md` with the
pivot table. Spot-checks determinism: re-runs one T3 and one T2 cell
and compares frame counts (tagged extra.spot_check, excluded from the
pivot).

    python3 scripts/openpokered/calibrate_rq1.py            # full matrix
    python3 scripts/openpokered/calibrate_rq1.py --only T3  # one tier
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
from openpokered.oracle import Oracle, OracleError  # noqa: E402
from openpokered.policies import ButtonRandomWalk, LocalExplorer  # noqa: E402
from openpokered.tasks import goal_satisfied, load_task  # noqa: E402

RQ1_DIR = RUNS_DIR / "rq1"
TASKS_DIR = Path(__file__).resolve().parent / "tasks"

SEEDS = (1, 42, 777)
T3_TIER1_TASKS = ["reach-viridian-city", "reach-pewter-city", "talk-to-oak",
                  "acquire-potion", "win-wild-battle"]
T3_TIER23_TASKS = ["get-starter", "beat-one-trainer", "beat-brock"]
T2_TASKS = ["reach-viridian-city", "acquire-potion", "talk-to-oak"]
T1_TASKS = ["reach-viridian-city"]
T1_FRAME_CAP = 20000
T2_MIN_BUDGET = 2000

POLICY_BY_TIER = {"T3": "oracle", "T2": LocalExplorer.POLICY_NAME,
                  "T1": ButtonRandomWalk.POLICY_NAME}


def run_cell(task, seed, tier, out_dir, frame_budget=None, spot_check=False):
    """One (task, tier, seed) run → RunMetrics, appended to the jsonl."""
    task = dict(task)
    task["seed"] = seed
    if tier != "T3":
        task["max_steps"] = 100000  # the frame budget is the limiter
    metrics = RunMetrics(task_id=task["id"], seed=seed, tier=tier,
                         policy=POLICY_BY_TIER[tier])
    if spot_check:
        metrics.extra["spot_check"] = True
    # speed=0 (driven-only): the frame budget and the determinism guard
    # both assume wall-clock-insensitive frame accounting.
    env = OpenPokeredEnv(speed=0)
    try:
        env.reset(task)
        metrics.start(env.frame_count())
        if tier == "T3":
            success, reason = _run_oracle(env, task, metrics)
        else:
            policy = (LocalExplorer(seed) if tier == "T2"
                      else ButtonRandomWalk(seed))
            success, reason = policy.run(env, task, frame_budget or 30000)
            metrics.battles = policy.battles
            metrics.battles_won = policy.battles_won
        metrics.env_steps = env.env_steps
        metrics.invalid_actions = env.invalid_actions
        if tier == "T3":
            metrics.battles = env.battles
            metrics.battles_won = env.battles_won
            metrics.blackouts = env.blackouts
            metrics.recovery_events = env.recovery_events
        metrics.finish(env.frame_count(), success, reason)
    except Exception as e:  # spawn/crash classes count as failures too
        metrics.env_steps = env.env_steps
        metrics.finish(env.frame_count(), False, f"{type(e).__name__}: {e}")
    finally:
        env.close()
    metrics.write(out_dir)
    status = "OK " if metrics.success else "FAIL"
    print(f"[{status}] {tier} {metrics.task_id} seed={seed} "
          f"steps={metrics.env_steps} frames={metrics.frames_elapsed} "
          f"battles={metrics.battles} invalid={metrics.invalid_actions} "
          f"wall={metrics.wall_clock_s}s reason={metrics.failure_reason!r}",
          flush=True)
    return metrics


def _run_oracle(env, task, metrics):
    try:
        result = Oracle(env).run(task)
    except OracleError as e:
        return False, f"oracle: {e}"
    success = goal_satisfied(task["goal"], env)
    if isinstance(result, tuple) and len(result) == 3:
        metrics.extra["strategy"] = result[2].get("strategy")
    return success, "" if success else "goal not satisfied after oracle run"


def _median_frames(runs):
    frames = [m.frames_elapsed for m in runs if m.success]
    return statistics.median(frames) if frames else None


def summarize(all_runs, spot_results, wall_s, out_path):
    """Pivot: task × tier → success rate, median frames/steps/battles."""
    cells = {}
    for m in all_runs:
        cells.setdefault((m.task_id, m.tier), []).append(m)
    task_order = sorted({m.task_id for m in all_runs},
                        key=lambda t: (t not in T3_TIER1_TASKS, t))

    def cell_text(task, tier):
        runs = cells.get((task, tier))
        if not runs:
            return "—"
        wins = [m for m in runs if m.success]
        rate = f"{len(wins)}/{len(runs)}"
        if not wins:
            reasons = sorted({m.failure_reason.split(":")[0] for m in runs})
            return f"**{rate}** ({', '.join(reasons)})"
        fr = statistics.median(m.frames_elapsed for m in wins)
        st = statistics.median(m.env_steps for m in wins)
        ba = statistics.median(m.battles for m in runs)
        return f"**{rate}** · {int(fr)}f · {st:.0f}st · {ba:.0f}b"

    lines = [
        "# RQ1 calibration — oracle upper bound + tier baselines",
        "",
        f"Matrix: T3 oracle (semantic + world model) · T2 local explorer "
        f"(symbolic, no world model) · T1 button random walk (controller only).",
        f"Seeds: {', '.join(map(str, SEEDS))}. T2 budget = 2× T3 median frames "
        f"(min {T2_MIN_BUDGET}); T1 cap = {T1_FRAME_CAP} frames.",
        "All runs headless with `--speed 0` (driven-only: game frames "
        "advance only inside synchronous debug commands).",
        "Cell: **success rate** · median frames (successes) · median env steps "
        "· median battles.",
        "",
        "| task | T1 buttons | T2 local nav | T3 oracle |",
        "|---|---|---|---|",
    ]
    for task in task_order:
        lines.append(f"| {task} | {cell_text(task, 'T1')} "
                     f"| {cell_text(task, 'T2')} | {cell_text(task, 'T3')} |")
    lines += [
        "",
        "## Failure attribution",
        "",
        "| task | tier | seed | env steps | frames | battles | invalid | reason |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for m in all_runs:
        if m.success:
            continue
        lines.append(f"| {m.task_id} | {m.tier} | {m.seed} | {m.env_steps} "
                     f"| {m.frames_elapsed} | {m.battles} "
                     f"| {m.invalid_actions} | {m.failure_reason} |")
    lines += ["", "## Determinism spot-check", ""]
    for label, first, second in spot_results:
        same = first.frames_elapsed == second.frames_elapsed
        lines.append(f"- {label}: frames {first.frames_elapsed} vs "
                     f"{second.frames_elapsed} → "
                     f"{'identical' if same else 'DRIFT'}")
    lines += ["", f"Total wall-clock: {wall_s:.1f}s "
                  f"({len(all_runs)} matrix runs + "
                  f"{len(spot_results)} spot-checks).", ""]
    out_path.write_text("\n".join(lines))
    return "\n".join(lines)


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--only", choices=["T1", "T2", "T3"], default=None)
    p.add_argument("--skip-spot-check", action="store_true")
    args = p.parse_args(argv)

    t0 = time.time()
    all_runs = []
    t3_medians = {}

    # ── T3 oracle: upper bound across the whole task set ────────────
    if args.only in (None, "T3"):
        for task_id in T3_TIER1_TASKS:
            task = load_task(TASKS_DIR / f"{task_id}.json")
            runs = [run_cell(task, seed, "T3", RQ1_DIR) for seed in SEEDS]
            all_runs += runs
            t3_medians[task_id] = _median_frames(runs)
        for task_id in T3_TIER23_TASKS:
            task = load_task(TASKS_DIR / f"{task_id}.json")
            all_runs.append(run_cell(task, 42, "T3", RQ1_DIR))

    # ── T2 local explorer ────────────────────────────────────────────
    if args.only in (None, "T2"):
        for task_id in T2_TASKS:
            task = load_task(TASKS_DIR / f"{task_id}.json")
            median = t3_medians.get(task_id)
            if median is None:  # --only T2: derive the budget from jsonl
                median = _t3_median_from_runs(task_id)
            budget = int(max(T2_MIN_BUDGET, 2 * (median or 15000)))
            print(f"[budget] T2 {task_id}: {budget} frames "
                  f"(T3 median {median})", flush=True)
            for seed in SEEDS:
                all_runs.append(run_cell(task, seed, "T2", RQ1_DIR,
                                         frame_budget=budget))

    # ── T1 button random walk ────────────────────────────────────────
    if args.only in (None, "T1"):
        for task_id in T1_TASKS:
            task = load_task(TASKS_DIR / f"{task_id}.json")
            for seed in SEEDS:
                all_runs.append(run_cell(task, seed, "T1", RQ1_DIR,
                                         frame_budget=T1_FRAME_CAP))

    # ── determinism spot-check: same cell twice more ─────────────────
    spot_results = []
    if not args.skip_spot_check and args.only in (None, "T3"):
        task = load_task(TASKS_DIR / "reach-viridian-city.json")
        first = next((m for m in all_runs
                      if m.task_id == "reach-viridian-city"
                      and m.tier == "T3" and m.seed == 42), None)
        second = run_cell(task, 42, "T3", RQ1_DIR, spot_check=True)
        if first is not None:
            spot_results.append(("T3 reach-viridian-city seed=42", first, second))
    if not args.skip_spot_check and args.only in (None, "T2"):
        task = load_task(TASKS_DIR / "reach-viridian-city.json")
        first = next((m for m in all_runs
                      if m.task_id == "reach-viridian-city"
                      and m.tier == "T2" and m.seed == 42), None)
        budget = int(max(T2_MIN_BUDGET,
                         2 * (t3_medians.get("reach-viridian-city") or 15000)))
        second = run_cell(task, 42, "T2", RQ1_DIR, frame_budget=budget,
                          spot_check=True)
        if first is not None:
            spot_results.append(("T2 reach-viridian-city seed=42", first, second))

    wall_s = time.time() - t0
    summary = summarize(all_runs, spot_results, wall_s, RQ1_DIR / "SUMMARY.md")
    print()
    print(summary)
    failed = [m for m in all_runs if not m.success and m.tier == "T3"]
    return 1 if failed else 0


def _t3_median_from_runs(task_id):
    path = RQ1_DIR / f"{task_id}__T3.jsonl"
    if not path.exists():
        return None
    runs = [r for r in RunMetrics.read_runs(path)
            if r.get("success") and not r.get("extra", {}).get("spot_check")]
    frames = [r["frames_elapsed"] for r in runs]
    return statistics.median(frames) if frames else None


if __name__ == "__main__":
    sys.exit(main())

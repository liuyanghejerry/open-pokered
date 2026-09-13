#!/usr/bin/env python3
"""openpoke task runner (M6): load a task spec, launch a headless seeded
game, run the rule-based oracle, record metrics, print a summary.

    python3 scripts/openpoke/run_task.py scripts/openpoke/tasks/beat-brock.json
    python3 scripts/openpoke/run_task.py scripts/openpoke/tasks/reach-pewter-city.json --seed 7
    python3 scripts/openpoke/run_task.py scripts/openpoke/tasks/beat-brock.json --runs 3 --seeds 1,42,777
    python3 scripts/openpoke/run_task.py <task> --determinism-check

Exit code 0 = success (all runs), 1 = failure.
"""
import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from openpoke.env import OpenPokeEnv  # noqa: E402
from openpoke.metrics import RunMetrics  # noqa: E402
from openpoke.oracle import Oracle, OracleError  # noqa: E402
from openpoke.tasks import load_task, goal_satisfied  # noqa: E402


def run_once(task, seed, binary=None, write_metrics=True, quiet=False):
    task = dict(task)
    task["seed"] = seed
    env = OpenPokeEnv(binary=binary)
    metrics = RunMetrics(task_id=task["id"], seed=seed)
    try:
        env.reset(task)
        metrics.start(env.frame_count())
        oracle = Oracle(env)
        result = oracle.run(task)
        success = goal_satisfied(task["goal"], env)
        metrics.env_steps = env.env_steps
        metrics.invalid_actions = env.invalid_actions
        metrics.battles = env.battles
        metrics.battles_won = env.battles_won
        metrics.blackouts = env.blackouts
        metrics.recovery_events = env.recovery_events
        if isinstance(result, tuple) and len(result) == 3:
            metrics.extra["strategy"] = result[2].get("strategy")
        metrics.finish(env.frame_count(), success,
                       "" if success else "goal not satisfied after oracle run")
    except OracleError as e:
        metrics.env_steps = env.env_steps
        metrics.finish(env.frame_count(), False, f"oracle: {e}")
    except Exception as e:
        metrics.env_steps = env.env_steps
        metrics.finish(env.frame_count(), False, f"{type(e).__name__}: {e}")
    finally:
        env.close()
    path = metrics.write() if write_metrics else None
    if not quiet:
        m = metrics
        status = "OK " if m.success else "FAIL"
        print(f"[{status}] {m.task_id} seed={m.seed} steps={m.env_steps} "
              f"frames={m.frames_elapsed} battles={m.battles} blackouts={m.blackouts} "
              f"recoveries={m.recovery_events} invalid={m.invalid_actions} "
              f"wall={m.wall_clock_s}s reason={m.failure_reason!r}")
        if path:
            print(f"       metrics → {path}")
    return metrics


def determinism_check(task, seed, binary=None):
    """Same seed + save/restore replay: the two traces must match."""
    task = dict(task)
    task["seed"] = seed
    traces = []
    for phase in ("straight", "forked"):
        env = OpenPokeEnv(binary=binary)
        try:
            env.reset(task)
            env.client.save_state(0)
            oracle = Oracle(env)
            oracle.run(task)
            traces.append((env.env_steps, env.frame_count(),
                           goal_satisfied(task["goal"], env)))
        finally:
            env.close()
    # Fork: restore mid-state and re-run from the same point.
    env = OpenPokeEnv(binary=binary)
    try:
        env.reset(task)
        env.client.save_state(0)
        env.client.restore_state(0)
        oracle = Oracle(env)
        oracle.run(task)
        traces.append((env.env_steps, env.frame_count(),
                       goal_satisfied(task["goal"], env)))
    finally:
        env.close()
    ok = all(t[2] for t in traces)
    same_steps = len({t[0] for t in traces}) == 1
    print(f"[determinism] traces: {traces} — steps equal: {same_steps}, all goals met: {ok}")
    return ok and same_steps


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("task", help="task spec JSON (or 'all' for the whole tiered set)")
    p.add_argument("--seed", type=int, default=None, help="override the spec seed")
    p.add_argument("--seeds", type=str, default=None, help="comma list for multi-seed runs")
    p.add_argument("--runs", type=int, default=1)
    p.add_argument("--binary", default=None)
    p.add_argument("--determinism-check", action="store_true")
    p.add_argument("--quiet", action="store_true")
    args = p.parse_args(argv)

    from openpoke.tasks import load_tasks_dir
    if args.task == "all":
        tasks = load_tasks_dir()
    else:
        tasks = [load_task(args.task)]

    if args.determinism_check:
        task = tasks[0]
        seed = args.seed if args.seed is not None else task["seed"]
        ok = determinism_check(task, seed, binary=args.binary)
        return 0 if ok else 1

    seeds = []
    if args.seeds:
        seeds = [int(s) for s in args.seeds.split(",")]
    elif args.seed is not None:
        seeds = [args.seed]
    results = []
    for task in tasks:
        if not task.get("oracle", True):
            if not args.quiet:
                print(f"[skip] {task['id']} (oracle:false)")
            continue
        run_seeds = seeds or [task["seed"]] * max(1, args.runs)
        for seed in run_seeds:
            results.append(run_once(task, seed, binary=args.binary, quiet=args.quiet))
    failed = [m for m in results if not m.success]
    if not args.quiet:
        print(f"── {len(results) - len(failed)}/{len(results)} runs succeeded")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

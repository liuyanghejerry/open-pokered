#!/usr/bin/env python3
"""Run task specs with the judgment-driven policy (T2J).

Same task specs, same seeds, same headless env as `run_task.py`; the
difference is the policy. `run_task.py` runs the rule-based oracle, which
resolves each goal through a hardcoded per-flag strategy.
`judgment_agent.JudgmentAgent` gets the task's natural-language name and
works out the rest: code lists the actions that are executable from where
it stands, a TypeSafe judgment picks one, and the same code executes it.

    python3 scripts/openpokered/run_judgment.py talk-to-oak
    python3 scripts/openpokered/run_judgment.py all --hop-budget 2
    python3 scripts/openpokered/run_judgment.py all --compare

`--compare` also runs `LocalExplorer` (the mechanical T2 baseline) on the
same task and seed, so the difference is attributable to the policy.

Needs a TypeSafe key (`TYPESAFE_API_KEY`, or the repo-root `.env`) and a
debug-server binary:

    cargo build --bin pokered-app --features debug-server

Exit code 0 = all runs succeeded, 1 = at least one failed.
"""
import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from openpokered.env import OpenPokeredEnv  # noqa: E402
from openpokered.judgment_agent import JudgmentAgent  # noqa: E402
from openpokered.metrics import RunMetrics  # noqa: E402
from openpokered.policies import LocalExplorer  # noqa: E402
from openpokered.oracle import Oracle, OracleError  # noqa: E402
from openpokered.semantics import SemanticJudge  # noqa: E402
from openpokered.tasks import goal_satisfied, load_task, load_tasks_dir  # noqa: E402

DEFAULT_FRAME_BUDGET = 20000


# Exploration mode: no task goal. The policy chooses what to pursue from
# the story objectives still unsatisfied, and the run ends when none are
# left. The party is the same generous one the task specs use, so a wild
# battle on the way does not decide the run.
EXPLORE_TASK = {
    "id": "explore-the-story",
    "name": "Explore the story from where you stand",
    "explore": True,
    "seed": 42,
    "initial_state": {"warp": "PalletTown,10,6"},
    "setup": {"party": [{"species": "Charizard", "level": 100}]},
    # `OpenPokeredEnv.step` assesses this on every step, so exploration
    # still needs one; a flag nothing sets keeps it from ever firing, and
    # the policy's own stopping condition ends the run instead.
    "goal": {"type": "flag", "id": "EVENT_EXPLORATION_HAS_NO_GOAL"},
    "max_steps": 400,
}


def _run_policy(task, seed, policy_factory, binary, maps_dir, frame_budget):
    task = dict(task)
    task["seed"] = seed
    env = OpenPokeredEnv(binary=binary, maps_dir=maps_dir, speed=0)
    metrics = RunMetrics(task_id=task["id"], seed=seed)
    policy = policy_factory()
    try:
        env.reset(task)
        metrics.start(env.frame_count())
        ok, reason = policy.run(env, task, frame_budget)
        # Exploration has no goal to satisfy: the policy owns the stopping
        # condition (nothing left of the story), so its verdict is the
        # verdict. Asking `goal_satisfied` here would fail every run.
        success = ok if task.get("explore") else goal_satisfied(task["goal"], env)
        metrics.env_steps = env.env_steps
        metrics.invalid_actions = env.invalid_actions
        metrics.battles = env.battles
        metrics.battles_won = env.battles_won
        metrics.blackouts = env.blackouts
        metrics.recovery_events = env.recovery_events
        metrics.finish(env.frame_count(), success,
                       "" if success else (reason or "goal not satisfied"))
    except Exception as e:
        metrics.env_steps = env.env_steps
        metrics.finish(env.frame_count(), False, f"{type(e).__name__}: {e}")
    finally:
        env.close()
    return metrics, policy


def _run_oracle(task, seed, binary, maps_dir, frame_budget):
    """The scripted reference: a hardcoded strategy per goal flag.

    This is the bar the judgment policy has to clear, and the only fair
    comparison — `LocalExplorer` is a deliberately weak baseline, so
    beating it says little. `Oracle.run` takes no frame budget (it works
    in its own step units), so `frame_budget` is unused here.
    """
    task = dict(task)
    task["seed"] = seed
    env = OpenPokeredEnv(binary=binary, maps_dir=maps_dir, speed=0)
    metrics = RunMetrics(task_id=task["id"], seed=seed)
    try:
        env.reset(task)
        metrics.start(env.frame_count())
        Oracle(env).run(task)
        success = goal_satisfied(task["goal"], env)
        metrics.env_steps = env.env_steps
        metrics.invalid_actions = env.invalid_actions
        metrics.battles = env.battles
        metrics.battles_won = env.battles_won
        metrics.blackouts = env.blackouts
        metrics.recovery_events = env.recovery_events
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
    return metrics


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("task", nargs="?", default="all",
                   help="task spec JSON (or 'all'); unused with --explore")
    p.add_argument("--explore", action="store_true",
                   help="no task goal: the policy picks what to pursue from "
                        "the story objectives still unsatisfied")
    p.add_argument("--seed", type=int, default=None, help="override the spec seed")
    p.add_argument("--runs", type=int, default=1)
    p.add_argument("--binary", default=None)
    p.add_argument("--maps-dir", default=None)
    p.add_argument("--hop-budget", type=int, default=1,
                   help="initial world-graph hop radius for candidate places")
    p.add_argument("--max-hop-budget", type=int, default=3,
                   help="radius the candidate set may widen to once the "
                        "narrow set is exhausted")
    p.add_argument("--frame-budget", type=int, default=DEFAULT_FRAME_BUDGET)
    p.add_argument("--max-judgments", type=int, default=60)
    p.add_argument("--no-place-facts", action="store_true",
                   help="ablation: do not annotate candidate places with what "
                        "their scripts do about the goal")
    p.add_argument("--thin-state", action="store_true",
                   help="ablation: send only map/position/mode/party-count to "
                        "the judgment instead of the full story state")
    p.add_argument("--act-margin", type=float, default=0.0,
                   help="decline to act when the top two options are closer "
                        "than this margin (0 = always act on the winner)")
    p.add_argument("--compare", action="store_true",
                   help="also run the LocalExplorer baseline and the rule-based "
                        "oracle on each task/seed")
    p.add_argument("--quiet", action="store_true")
    args = p.parse_args(argv)

    judge = SemanticJudge.from_env()
    if not judge.enabled:
        print("no TypeSafe key: set TYPESAFE_API_KEY (or put it in .env)",
              file=sys.stderr)
        return 1

    if args.explore:
        tasks = [dict(EXPLORE_TASK, seed=args.seed if args.seed is not None
                      else EXPLORE_TASK["seed"])]
    else:
        tasks = load_tasks_dir() if args.task == "all" else [load_task(args.task)]

    rows = []
    for task in tasks:
        seeds = [args.seed if args.seed is not None else task["seed"]] \
            * max(1, args.runs)
        for seed in seeds:
            t0 = time.time()
            m, policy = _run_policy(
                task, seed,
                lambda: JudgmentAgent(judge, seed=seed,
                                      hop_budget=args.hop_budget,
                                      max_hop_budget=args.max_hop_budget,
                                      max_judgments=args.max_judgments,
                                      place_facts=not args.no_place_facts,
                                      rich_state=not args.thin_state,
                                      act_margin=args.act_margin,
                                      explore=args.explore),
                args.binary, args.maps_dir, args.frame_budget)
            m.extra["judgments"] = policy.judgments
            m.extra["fallbacks"] = policy.fallbacks
            m.extra["escalations"] = policy.escalations
            m.extra["undecided"] = policy.undecided
            m.extra["judge_calls"] = judge.calls
            m.write()
            if not args.quiet:
                print(f"[{'OK ' if m.success else 'FAIL'}] T2J {m.task_id} "
                      f"seed={seed} steps={m.env_steps} frames={m.frames_elapsed} "
                      f"judgments={policy.judgments} esc={policy.escalations} "
                      f"fallbacks={policy.fallbacks} undecided={policy.undecided} "
                      f"battles={m.battles} "
                      f"wall={time.time() - t0:.1f}s "
                      f"reason={m.failure_reason!r}")
            rows.append({"task": task["id"], "seed": seed, "tier": "T2J",
                         "ok": m.success, "steps": m.env_steps})

            if not args.compare:
                continue
            m2, _ = _run_policy(
                task, seed, lambda: LocalExplorer(seed=seed),
                args.binary, args.maps_dir, args.frame_budget)
            if not args.quiet:
                print(f"[{'OK ' if m2.success else 'FAIL'}] T2  {m2.task_id} "
                      f"seed={seed} steps={m2.env_steps} "
                      f"frames={m2.frames_elapsed} battles={m2.battles} "
                      f"reason={m2.failure_reason!r}")
            rows.append({"task": task["id"], "seed": seed, "tier": "T2 base",
                         "ok": m2.success, "steps": m2.env_steps})

            # The oracle is the bar that matters, and the task specs that
            # opt out of it (`oracle: false`) must not be scored as if it
            # had failed — it was never asked to run.
            if task.get("oracle", True):
                m3 = _run_oracle(task, seed, args.binary, args.maps_dir,
                                 args.frame_budget)
                if not args.quiet:
                    print(f"[{'OK ' if m3.success else 'FAIL'}] T3  {m3.task_id} "
                          f"seed={seed} steps={m3.env_steps} "
                          f"frames={m3.frames_elapsed} battles={m3.battles} "
                          f"reason={m3.failure_reason!r}")
                rows.append({"task": task["id"], "seed": seed, "tier": "T3 oracle",
                             "ok": m3.success, "steps": m3.env_steps})
            else:
                rows.append({"task": task["id"], "seed": seed, "tier": "T3 oracle",
                             "ok": None, "steps": 0})

    if rows:
        # Count per (task, tier) rather than last-write-wins. With
        # `--runs N` a single result is exactly what the judgment's own
        # variance makes unreliable, so the report has to carry the rate.
        table = {}
        for r in rows:
            cell = table.setdefault(r["task"], {}).setdefault(
                r["tier"], {"runs": 0, "wins": 0, "steps": 0, "skipped": False})
            if r["ok"] is None:
                cell["skipped"] = True     # e.g. a task marked `oracle: false`
                continue
            cell["runs"] += 1
            cell["wins"] += 1 if r["ok"] else 0
            cell["steps"] += r["steps"]
        tiers = sorted({t for cells in table.values() for t in cells})

        print("\n── policies ──")
        for tier in tiers:
            cells = [c[tier] for c in table.values() if tier in c]
            runs = sum(c["runs"] for c in cells)
            wins = sum(c["wins"] for c in cells)
            steps = sum(c["steps"] for c in cells)
            if not runs:
                print(f"  {tier:<10} not run")
                continue
            print(f"  {tier:<10} {wins}/{runs} runs succeeded, "
                  f"{steps} env-steps total")

        print("\n── per task (succeeded / runs) ──")
        print("  " + f"{'task':<22}" + "".join(f"{t:<20}" for t in tiers))
        for name in sorted(table):
            cells = []
            for tier in tiers:
                c = table[name].get(tier)
                if c is None:
                    cells.append("—")
                elif c["skipped"] and not c["runs"]:
                    cells.append("n/a")
                else:
                    avg = c["steps"] / c["runs"]
                    cells.append(f"{c['wins']}/{c['runs']} ({avg:.0f} steps)")
            print("  " + f"{name:<22}" + "".join(f"{x:<20}" for x in cells))

    failed = [r for r in rows if r["tier"] == "T2J" and not r["ok"]]
    if not args.quiet:
        print(f"\n── judgment cost: {judge.calls} requests, "
              f"{judge.input_tokens} in / {judge.output_tokens} out tokens")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

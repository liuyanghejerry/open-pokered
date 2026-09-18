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
        success = goal_satisfied(task["goal"], env)
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
    p.add_argument("task", help="task spec JSON (or 'all')")
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
                                      act_margin=args.act_margin),
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

    if args.compare:
        tiers = {}
        for r in rows:
            if r["ok"] is None:
                continue
            n, wins, total = tiers.get(r["tier"], (0, 0, 0))
            tiers[r["tier"]] = (n + 1, wins + (1 if r["ok"] else 0),
                                total + r["steps"])
        print("\n── policy comparison ──")
        for tier, (n, wins, total) in sorted(tiers.items()):
            print(f"  {tier:<10} {wins}/{n} succeeded, {total} env-steps total")

        by_task = {}
        for r in rows:
            by_task.setdefault(r["task"], {})[r["tier"]] = r
        print("\n── per task ──")
        print(f"  {'task':<22} {'T3 oracle':<16} {'T2J':<16} {'T2 base':<16}")
        for name in sorted(by_task):
            cells = []
            for tier in ("T3 oracle", "T2J", "T2 base"):
                r = by_task[name].get(tier)
                if r is None or r["ok"] is None:
                    cells.append("n/a")
                else:
                    cells.append(f"{'OK' if r['ok'] else 'FAIL'} ({r['steps']} steps)")
            print(f"  {name:<22} {cells[0]:<16} {cells[1]:<16} {cells[2]:<16}")

    failed = [r for r in rows if r["tier"] == "T2J" and not r["ok"]]
    if not args.quiet:
        done = [r for r in rows if r["tier"] == "T2J"]
        print(f"── T2J {len(done) - len(failed)}/{len(done)} runs succeeded "
              f"({judge.calls} judgment requests, {judge.input_tokens} in / "
              f"{judge.output_tokens} out tokens)")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

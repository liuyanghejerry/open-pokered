#!/usr/bin/env python3
"""LLM smoke runner (WP1): one real LLM-driven run of a task spec.

    python3 scripts/openpoke/run_llm.py scripts/openpoke/tasks/reach-viridian-city.json --tier T2 --seed 42

Spawns the seeded headless game in driven-only mode (`--speed 0`, same as
the RQ1 calibration matrix), drives it with the tier's LLM policy, and
appends the run to `target/agent/runs/rq1/smoke/<task>__<tier>.jsonl`
plus a short SUMMARY.md. Credentials per llm_agent.resolve_credentials
(OPENAI_BASE_URL+OPENAI_API_KEY, else HF_TOKEN); model via OPENPOKE_MODEL
or the probed default.
"""
import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from openpoke.env import OpenPokeEnv  # noqa: E402
from openpoke.llm_agent import (ButtonLlmAgent, ChatClient, SkillLlmAgent,  # noqa: E402
                                WorldModelLlmAgent, default_model,
                                resolve_credentials)
from openpoke.metrics import RUNS_DIR, RunMetrics  # noqa: E402
from openpoke.tasks import load_task  # noqa: E402

TIER_CLASSES = {"T1": ButtonLlmAgent, "T2": SkillLlmAgent,
                "T3": WorldModelLlmAgent}
SMOKE_DIR = RUNS_DIR / "rq1" / "smoke"


def run_llm_cell(task, seed, tier, client, model, frame_budget, out_dir,
                 max_model_calls=60):
    task = dict(task)
    task["seed"] = seed
    task["max_steps"] = 100000  # frame budget is the limiter
    cls = TIER_CLASSES[tier]
    metrics = RunMetrics(task_id=task["id"], seed=seed, tier=tier,
                         policy=cls.POLICY_NAME)
    metrics.extra["model"] = model
    env = OpenPokeEnv(speed=0)
    policy = cls(client, seed, max_model_calls=max_model_calls)
    try:
        env.reset(task)
        metrics.start(env.frame_count())
        success, reason = policy.run(env, task, frame_budget)
        metrics.env_steps = env.env_steps
        metrics.invalid_actions = env.invalid_actions
        metrics.battles = policy.battles
        metrics.battles_won = policy.battles_won
        metrics.model_calls = policy.model_calls
        metrics.model_tokens_prompt = policy.prompt_tokens
        metrics.model_tokens_completion = policy.completion_tokens
        metrics.extra["parse_failures"] = policy.parse_failures
        metrics.extra["degraded_actions"] = policy.degraded_actions
        metrics.finish(env.frame_count(), success, reason)
    except Exception as e:
        metrics.finish(env.frame_count(), False, f"{type(e).__name__}: {e}")
    finally:
        env.close()
    path = metrics.write(out_dir)
    status = "OK " if metrics.success else "FAIL"
    print(f"[{status}] {tier} {metrics.task_id} seed={seed} "
          f"frames={metrics.frames_elapsed} steps={metrics.env_steps} "
          f"battles={metrics.battles} calls={metrics.model_calls} "
          f"tokens={metrics.model_tokens_prompt}/{metrics.model_tokens_completion} "
          f"parse_fail={metrics.extra['parse_failures']} "
          f"degraded={metrics.extra['degraded_actions']} "
          f"reason={metrics.failure_reason!r}")
    print(f"       metrics → {path}")
    return metrics


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("task", help="task spec JSON")
    p.add_argument("--tier", choices=sorted(TIER_CLASSES), default="T2")
    p.add_argument("--seed", type=int, default=None)
    p.add_argument("--frames", type=int, default=6000,
                   help="frame budget for the run")
    p.add_argument("--model", default=None)
    p.add_argument("--max-model-calls", type=int, default=60)
    p.add_argument("--out", default=None)
    args = p.parse_args(argv)

    base_url, api_key, source = resolve_credentials()
    model = args.model or default_model()
    print(f"llm: source={source} model={model}")
    client = ChatClient(base_url, api_key, model)

    task = load_task(args.task)
    seed = args.seed if args.seed is not None else task["seed"]
    out_dir = Path(args.out) if args.out else SMOKE_DIR
    metrics = run_llm_cell(task, seed, args.tier, client, model,
                           args.frames, out_dir,
                           max_model_calls=args.max_model_calls)

    summary = (out_dir / "SUMMARY.md")
    out_dir.mkdir(parents=True, exist_ok=True)
    summary.write_text(
        "# LLM smoke (WP1)\n\n"
        f"- source: {source} · model: `{model}`\n"
        f"- task: {metrics.task_id} · tier {args.tier} · seed {seed} · "
        f"--speed 0 (driven-only)\n"
        f"- success: {metrics.success} · reason: "
        f"{metrics.failure_reason!r}\n"
        f"- frames: {metrics.frames_elapsed} · env steps: "
        f"{metrics.env_steps} · battles: {metrics.battles}\n"
        f"- model calls: {metrics.model_calls} · tokens: "
        f"{metrics.model_tokens_prompt} prompt / "
        f"{metrics.model_tokens_completion} completion\n"
        f"- parse failures: {metrics.extra['parse_failures']} · degraded "
        f"actions: {metrics.extra['degraded_actions']}\n")
    print(f"       summary → {summary}")
    return 0 if metrics.success else 1


if __name__ == "__main__":
    sys.exit(main())

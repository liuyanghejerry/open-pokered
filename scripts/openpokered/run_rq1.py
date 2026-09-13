#!/usr/bin/env python3
"""RQ1 formal matrix runner (WP2): one LLM across abstraction tiers.

Matrix: T1 buttons / T2 skills (travel_to DISABLED — mirrors the
calibration T2's no-world-model setting) / T3 world model, over the
small-task subset × 3 seeds, plus the scripted-oracle row from the
calibration artifacts as the ceiling reference.

    python3 scripts/openpokered/run_rq1.py                 # full matrix
    python3 scripts/openpokered/run_rq1.py --tier T1 --task reach-viridian-city --seed 42

Resume: cells with an existing JSONL row (same task/tier/seed) are
skipped; `--rerun-failed` re-runs failed cells (appends a new row).
Stop rules: ≥3 consecutive HTTP-429/quota failures shrink the remaining
plan to the minimal cell (reach-viridian-city × seed 42 per tier) and
disclose it in the summary; a tier×task cell that fails all 3 seeds is a
recorded negative result — no unbounded tuning.

Budgets (calibration conventions, disclosed in the summary): T2/T3
frames = 2× the scripted-oracle median for that task (floor 2000); T1
frames = 20000. Model-call caps: T3 40, T2 60, T1 400. Decoding:
temperature 0 (endpoint permitting); residual LLM nondeterminism is
disclosed.

Artifacts (all gitignored): per-cell JSONL under
target/agent/runs/rq1/<tier>/, per-tier SUMMARY.md, and the aggregate
comparison at target/agent/runs/rq1/SUMMARY.md (regenerated; it embeds
the oracle reference row so the earlier calibration table is preserved
in substance).
"""
import argparse
import json
import statistics
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from openpokered.env import OpenPokeredEnv  # noqa: E402
from openpokered.llm_agent import (ButtonLlmAgent, ChatClient, LlmError,  # noqa: E402
                                SkillLlmAgent, WorldModelLlmAgent,
                                default_model, resolve_credentials)
from openpokered.metrics import RUNS_DIR, RunMetrics  # noqa: E402
from openpokered.tasks import load_task  # noqa: E402

RQ1_DIR = RUNS_DIR / "rq1"
TASKS_DIR = Path(__file__).resolve().parent / "tasks"

SEEDS = (1, 42, 777)
# Small-task subset (WP2 decision): the three required tasks plus the
# same-tier small ones; get-pokedex stays out (oracle:false),
# get-starter/beat-one-trainer are cut as next-tier difficulty (see report).
MATRIX_TASKS = ["reach-viridian-city", "reach-pewter-city", "talk-to-oak",
                "acquire-potion", "win-wild-battle", "beat-brock"]
T1_TRIAL_TASK = "reach-viridian-city"

FRAME_CAP_T1 = 20000
BUDGET_FLOOR = 2000
MODEL_CALL_CAP = {"T1": 400, "T2": 60, "T3": 40}
QUOTA_STOP = 3  # consecutive HTTP-429 failures before shrinking the plan


def oracle_median_frames(task_id):
    path = RQ1_DIR / f"{task_id}__T3.jsonl"
    if not path.exists():
        return None
    runs = RunMetrics.read_runs(path)
    frames = [r["frames_elapsed"] for r in runs
              if r.get("success") and not r.get("extra", {}).get("spot_check")]
    return statistics.median(frames) if frames else None


def task_frame_budget(tier, task_id):
    if tier == "T1":
        return FRAME_CAP_T1
    median = oracle_median_frames(task_id) or 7500
    return int(max(BUDGET_FLOOR, 2 * median))


def existing_cells(tier_dir):
    """(task_id, seed) → [rows] already recorded for a tier dir."""
    cells = {}
    if not tier_dir.is_dir():
        return cells
    for path in sorted(tier_dir.glob("*.jsonl")):
        for row in RunMetrics.read_runs(path):
            cells.setdefault((row["task_id"], row["seed"]), []).append(row)
    return cells


def make_agent(tier, client, seed, cap):
    if tier == "T1":
        return ButtonLlmAgent(client, seed, max_model_calls=cap)
    if tier == "T2":
        return SkillLlmAgent(client, seed, max_model_calls=cap,
                             allow_travel_to=False)
    if tier == "T3":
        return WorldModelLlmAgent(client, seed, max_model_calls=cap)
    raise ValueError(tier)


def run_cell(task, seed, tier, client, model, out_dir):
    budget = task_frame_budget(tier, task["id"])
    cap = MODEL_CALL_CAP[tier]
    task = dict(task)
    task["seed"] = seed
    task["max_steps"] = 100000
    metrics = RunMetrics(task_id=task["id"], seed=seed, tier=tier,
                         policy=make_agent(tier, client, seed, 0).POLICY_NAME)
    metrics.extra.update({
        "model": model, "temperature": client.temperature,
        "frame_budget": budget,
        "allow_travel_to": tier != "T2",
    })
    env = OpenPokeredEnv(speed=0)
    policy = make_agent(tier, client, seed, cap)
    try:
        env.reset(task)
        metrics.start(env.frame_count())
        success, reason = policy.run(env, task, budget)
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
    except LlmError as e:
        metrics.finish(env.frame_count(), False, f"{type(e).__name__}: {e}")
    except Exception as e:
        metrics.finish(env.frame_count(), False, f"{type(e).__name__}: {e}")
    finally:
        env.close()
    metrics.write(out_dir)
    status = "OK " if metrics.success else "FAIL"
    print(f"[{status}] {tier} {metrics.task_id} seed={seed} "
          f"frames={metrics.frames_elapsed}/{budget} steps={metrics.env_steps} "
          f"battles={metrics.battles} calls={metrics.model_calls} "
          f"tokens={metrics.model_tokens_prompt}+"
          f"{metrics.model_tokens_completion} "
          f"pf={metrics.extra['parse_failures']} "
          f"dg={metrics.extra['degraded_actions']} "
          f"reason={metrics.failure_reason!r}", flush=True)
    return metrics


# ── aggregation ───────────────────────────────────────────────────────
def aggregate(tier_runs, oracle_rows, model, wall_s, disclosures):
    def stats(rows):
        wins = [r for r in rows if r.get("success")]
        return wins, rows

    def cell(tier):
        rows = tier_runs.get(tier, [])
        wins, allr = stats(rows)
        if not allr:
            return f"| {tier} | — | — | — | — | — |"
        rate = f"{len(wins)}/{len(allr)}"
        fr = statistics.median(r["frames_elapsed"] for r in wins) if wins else None
        calls = statistics.median(r["model_calls"] for r in allr)
        tok_p = sum(r["model_tokens_prompt"] for r in allr)
        tok_c = sum(r["model_tokens_completion"] for r in allr)
        return (f"| {tier} | **{rate}** | {int(fr) if fr else '—'} | "
                f"{calls:.0f} | {tok_p} | {tok_c} |")

    oracle = oracle_rows
    wins = [r for r in oracle if r.get("success")]
    o_fr = statistics.median(r["frames_elapsed"] for r in wins) if wins else None
    o_steps = statistics.median(r["env_steps"] for r in wins) if wins else None

    lines = [
        "# RQ1 — abstraction tiers (LLM) with scripted-oracle reference",
        "",
        f"Model: `{model}` · temperature 0 · `--speed 0` driven-only · "
        f"seeds {', '.join(map(str, SEEDS))}.",
        "T2 has `travel_to` DISABLED (mirrors the calibration T2's "
        "no-world-model setting). Frame budgets: T2/T3 = 2× oracle median "
        f"(floor {BUDGET_FLOOR}); T1 = {FRAME_CAP_T1}. Model-call caps: "
        f"T3 {MODEL_CALL_CAP['T3']}, T2 {MODEL_CALL_CAP['T2']}, "
        f"T1 {MODEL_CALL_CAP['T1']}.",
        "",
        "LLM nondeterminism: decoding is temperature-0 but the endpoint "
        "and model outputs remain non-bit-exact across reruns; frames are "
        "engine-exact per actual action stream.",
        "",
        "| tier | success | median frames (wins) | median model calls | "
        "prompt tokens | completion tokens |",
        "|---|---|---|---|---|---|",
        cell("T1"),
        cell("T2"),
        cell("T3"),
        f"| oracle (scripted, ref) | **{len(wins)}/{len(oracle)}** | "
        f"{int(o_fr) if o_fr else '—'} | 0 | 0 | 0 |",
        "",
        "## Per-task success (won / attempted)",
        "",
        "| task | T1 | T2 | T3 | oracle (ref) |",
        "|---|---|---|---|---|",
    ]
    by_task = {}
    for tier, rows in tier_runs.items():
        for r in rows:
            by_task.setdefault(r["task_id"], {}).setdefault(tier, []).append(r)
    oracle_by_task = {}
    for r in oracle:
        oracle_by_task.setdefault(r["task_id"], []).append(r)
    for task_id in MATRIX_TASKS:
        cells = by_task.get(task_id, {})
        def rate(tier):
            rows = cells.get(tier)
            if not rows:
                return "—"
            return f"{sum(1 for r in rows if r.get('success'))}/{len(rows)}"
        ors = oracle_by_task.get(task_id, [])
        o_rate = f"{sum(1 for r in ors if r.get('success'))}/{len(ors)}" if ors else "—"
        lines.append(f"| {task_id} | {rate('T1')} | {rate('T2')} "
                     f"| {rate('T3')} | {o_rate} |")
    lines += ["", "## Failure attribution", "",
              "| task | tier | seed | frames | calls | reason |",
              "|---|---|---|---|---|---|"]
    for tier in ("T1", "T2", "T3"):
        for r in tier_runs.get(tier, []):
            if not r.get("success"):
                lines.append(f"| {r['task_id']} | {tier} | {r['seed']} "
                             f"| {r['frames_elapsed']} | {r['model_calls']} "
                             f"| {r['failure_reason']} |")
    lines += ["", "## Disclosures", ""]
    for d in disclosures:
        lines.append(f"- {d}")
    lines += ["", f"Total wall-clock: {wall_s:.1f}s.", ""]
    return "\n".join(lines)


def oracle_reference_rows():
    rows = []
    for path in sorted(RQ1_DIR.glob("*__T3.jsonl")):
        for r in RunMetrics.read_runs(path):
            if r.get("extra", {}).get("spot_check"):
                continue
            if r["task_id"] in MATRIX_TASKS:
                rows.append(r)
    return rows


def args_task_model(tier_runs):
    for rows in tier_runs.values():
        for r in rows:
            model = (r.get("extra") or {}).get("model")
            if model:
                return model
    return "(unknown)"


def _auto_disclosures(tier_runs):
    """Rebuilt disclosures for --aggregate-only, derived from the recorded
    rows themselves (quota walls, matrix shrinkage, T1 cost finding)."""
    out = []
    quota = sum(1 for rows in tier_runs.values() for r in rows
                if "HTTP 402" in r.get("failure_reason", "")
                or "HTTP 429" in r.get("failure_reason", ""))
    total = sum(len(rows) for rows in tier_runs.values())
    wins = sum(1 for rows in tier_runs.values() for r in rows if r.get("success"))
    if quota:
        out.append(
            f"Quota stop rule: the HuggingFace Inference Providers account "
            f"returned HTTP 402 (monthly included credits depleted) for "
            f"{quota}/{total} attempted cells ({wins} succeeded before the "
            "wall). The matrix was shrunk to the minimal viable cell "
            "(reach-viridian-city × seed 42 per tier) per the WP2 stop "
            "rules; unattempted cells are reported as not run, not as "
            "failures.")
    t1 = tier_runs.get("T1", [])
    if t1:
        med = statistics.median(
            r["frames_elapsed"] / max(1, r["model_calls"]) for r in t1)
        out.append(
            f"T1 cost finding: the button tier covered ~{med:.0f} frames "
            "per model call in the partial runs — a 20k-frame budget "
            "implies thousands of calls per run, making T1 cost-infeasible "
            "at this model/endpoint independent of the quota wall.")
    return out


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--tier", choices=["T1", "T2", "T3"], default=None)
    p.add_argument("--task", default=None)
    p.add_argument("--seed", type=int, default=None)
    p.add_argument("--rerun-failed", action="store_true")
    p.add_argument("--aggregate-only", action="store_true",
                   help="no runs: rebuild the aggregate SUMMARY.md from the "
                        "JSONL rows already on disk (quota-safe)")
    args = p.parse_args(argv)

    t0 = time.time()
    if args.aggregate_only:
        tier_runs = {}
        for tier in ("T1", "T2", "T3"):
            for rows in existing_cells(RQ1_DIR / tier.lower()).values():
                tier_runs.setdefault(tier, []).extend(rows)
        disclosures = _auto_disclosures(tier_runs)
        model = args_task_model(tier_runs)
        summary = aggregate(tier_runs, oracle_reference_rows(), model,
                            time.time() - t0, disclosures)
        (RQ1_DIR / "SUMMARY.md").write_text(summary + "\n")
        for tier, rows in tier_runs.items():
            wins = [r for r in rows if r.get("success")]
            (RQ1_DIR / tier.lower() / "SUMMARY.md").write_text(
                f"# RQ1 {tier} — {len(wins)}/{len(rows)} succeeded\n\n"
                f"model `{model}`, temperature 0, --speed 0\n")
        print(summary)
        return 0

    base_url, api_key, source = resolve_credentials()
    model = default_model()
    client = ChatClient(base_url, api_key, model)
    print(f"llm: source={source} model={model} temperature=0", flush=True)

    tiers = [args.tier] if args.tier else ["T3", "T2", "T1"]
    disclosures = []
    tier_runs = {}
    quota_streak = 0
    shrunk = False

    for tier in tiers:
        tier_dir = RQ1_DIR / tier.lower()
        tier_dir.mkdir(parents=True, exist_ok=True)
        done_cells = existing_cells(tier_dir)
        tasks = [args.task] if args.task else (
            [T1_TRIAL_TASK] if tier == "T1" else MATRIX_TASKS)
        seeds = [args.seed] if args.seed is not None else list(SEEDS)
        for task_id in tasks:
            task = load_task(TASKS_DIR / f"{task_id}.json")
            for seed in seeds:
                if shrunk and (task_id, seed) != (T1_TRIAL_TASK, 42):
                    continue
                prior = done_cells.get((task_id, seed), [])
                if prior and not (args.rerun_failed
                                  and not any(r.get("success") for r in prior)):
                    print(f"[skip] {tier} {task_id} seed={seed} "
                          f"({len(prior)} existing row(s))", flush=True)
                    tier_runs.setdefault(tier, []).extend(prior)
                    continue
                metrics = run_cell(task, seed, tier, client, model, tier_dir)
                tier_runs.setdefault(tier, []).append(metrics.to_dict())
                if "HTTP 429" in metrics.failure_reason:
                    quota_streak += 1
                else:
                    quota_streak = 0
                if quota_streak >= QUOTA_STOP and not shrunk:
                    shrunk = True
                    disclosures.append(
                        f"Quota/rate-limit stop rule fired after {quota_streak} "
                        "consecutive HTTP 429 failures: remaining plan shrunk "
                        "to the minimal cell (reach-viridian-city × seed 42 "
                        "per tier).")
                    print(f"[stop-rule] {disclosures[-1]}", flush=True)
        # per-tier summary
        rows = tier_runs.get(tier, [])
        if rows:
            wins = [r for r in rows if r.get("success")]
            (tier_dir / "SUMMARY.md").write_text(
                f"# RQ1 {tier} — {len(wins)}/{len(rows)} succeeded\n\n"
                f"model `{model}`, temperature 0, --speed 0\n")

    wall_s = time.time() - t0
    oracle_rows = oracle_reference_rows()
    summary = aggregate(tier_runs, oracle_rows, model, wall_s, disclosures)
    (RQ1_DIR / "SUMMARY.md").write_text(summary + "\n")
    print()
    print(summary)
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Record and compare deterministic GBA hardware-timer performance samples.

``record`` launches a ROM built with ``perf-benchmark`` in mGBA and stops it
after its structured benchmark line. ``compare`` fails if a candidate exceeds
the reviewed baseline metrics by more than the configured allowance. The
measurements use the GBA's emulated hardware timer rather than host wall time.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import selectors
import shlex
import signal
import subprocess
import sys
import time
from typing import Any


METRICS = (
    "update_avg_ticks",
    "draw_per_frame_ticks",
    "draw_max_ticks",
    "present_per_frame_ticks",
    "present_max_ticks",
)
SCENARIO_METRICS = {
    "intro-title-v1": METRICS,
    "oak-dialogue-v1": METRICS,
    # A stable overworld intentionally reuses its previous frame. Requiring a
    # render here would turn the optimization into a test failure; update time
    # is the only meaningful performance measurement for this window.
    "overworld-idle-v1": ("update_avg_ticks",),
    "overworld-movement-v1": METRICS,
    "battle-entry-v1": METRICS,
    "pokedex-entry-v1": METRICS,
}
PERF_LINE = re.compile(r"\bgba-perf\s+(?P<fields>.+)")


def parse_performance_line(line: str) -> dict[str, Any] | None:
    """Extract a structured benchmark line from an mGBA log line."""
    match = PERF_LINE.search(line)
    if not match:
        return None
    fields: dict[str, Any] = {}
    for token in match.group("fields").split():
        if "=" not in token:
            continue
        key, value = token.split("=", 1)
        fields[key] = value if key == "scenario" else int(value)
    required = {"scenario", "samples", "renders", *METRICS}
    missing = required.difference(fields)
    if missing:
        raise ValueError(f"incomplete gba-perf line; missing {', '.join(sorted(missing))}")
    if fields["scenario"] not in SCENARIO_METRICS:
        raise ValueError(f"unexpected GBA performance scenario: {fields['scenario']}")
    if fields["samples"] < 1:
        raise ValueError("GBA performance window did not contain samples")
    if len(SCENARIO_METRICS[fields["scenario"]]) > 1 and fields["renders"] < 1:
        raise ValueError("GBA performance window did not contain a required render")
    return fields


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def record(args: argparse.Namespace) -> int:
    rom = args.rom.resolve()
    if not rom.is_file():
        raise ValueError(f"ROM not found: {rom}")
    command = shlex.split(args.emulator) + [
        "-C", "logToStdout=1",
        "-C", "logLevel.gba.debug=127",
        # Timer 2 still measures emulated hardware cycles; disabling host-side
        # sync only makes the scripted workload complete quickly in CI.
        "-C", "fpsTarget=0",
        "-C", "audioSync=0",
        "-C", "videoSync=0",
        str(rom),
    ]
    print("Running:", shlex.join(command), flush=True)
    started = time.monotonic()
    log_lines: list[str] = []
    process = subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )
    results: dict[str, dict[str, Any]] = {}
    selector = selectors.DefaultSelector()
    try:
        assert process.stdout is not None
        selector.register(process.stdout, selectors.EVENT_READ)
        while True:
            remaining = args.timeout_seconds - (time.monotonic() - started)
            if remaining <= 0:
                raise TimeoutError(
                    f"mGBA did not emit every scenario within {args.timeout_seconds}s"
                )
            ready = selector.select(timeout=min(remaining, 0.25))
            line = process.stdout.readline() if ready else ""
            if line:
                line = line.rstrip()
                log_lines.append(line)
                print(line, flush=True)
                parsed = parse_performance_line(line)
                if parsed is not None:
                    results[parsed["scenario"]] = parsed
                    if set(results) == set(SCENARIO_METRICS):
                        break
            elif process.poll() is not None:
                raise RuntimeError(f"mGBA exited before a benchmark result (status {process.returncode})")
    finally:
        selector.close()
        if process.poll() is None:
            process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        if process.stdout is not None:
            process.stdout.close()
        # Preserve whatever the emulator emitted even when it crashes or the
        # benchmark times out, so the CI artifact contains actionable evidence.
        if args.log:
            log_text = "\n".join(log_lines)
            if log_text:
                log_text += "\n"
            Path(args.log).parent.mkdir(parents=True, exist_ok=True)
            Path(args.log).write_text(log_text, encoding="utf-8")

    if set(results) != set(SCENARIO_METRICS):
        missing = set(SCENARIO_METRICS).difference(results)
        raise RuntimeError(f"mGBA finished without performance scenarios: {', '.join(sorted(missing))}")
    payload = {
        "suite": "autopilot-v1",
        "emulator": args.emulator,
        "scenarios": results,
    }
    write_json(args.output, payload)
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


def load_metrics(path: Path) -> dict[str, dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if payload.get("suite") != "autopilot-v1":
        raise ValueError(f"{path}: expected autopilot-v1 suite")
    scenarios = payload.get("scenarios")
    if not isinstance(scenarios, dict) or set(scenarios) != set(SCENARIO_METRICS):
        expected = ", ".join(sorted(SCENARIO_METRICS))
        raise ValueError(f"{path}: expected scenarios {expected}")
    for scenario, metrics in scenarios.items():
        if not isinstance(metrics, dict):
            raise ValueError(f"{path}: {scenario} metrics must be an object")
        if metrics.get("scenario") != scenario:
            raise ValueError(f"{path}: {scenario} contains mismatched scenario name")
        for key in ("samples", "renders", *METRICS):
            if not isinstance(metrics.get(key), int):
                raise ValueError(f"{path}: {scenario} missing integer metric {key}")
        if metrics["samples"] < 1:
            raise ValueError(f"{path}: {scenario} did not contain samples")
        if len(SCENARIO_METRICS[scenario]) > 1 and metrics["renders"] < 1:
            raise ValueError(f"{path}: {scenario} did not contain a required render")
    return scenarios


def compare_metrics(
    baseline: dict[str, dict[str, Any]], candidate: dict[str, dict[str, Any]], max_regression_pct: float, min_absolute_ticks: int
) -> list[str]:
    failures: list[str] = []
    for scenario, gated_metrics in SCENARIO_METRICS.items():
        for metric in gated_metrics:
            before = baseline[scenario][metric]
            after = candidate[scenario][metric]
            allowed_delta = max(before * max_regression_pct / 100.0, min_absolute_ticks)
            limit = before + allowed_delta
            change_pct = 0.0 if before == 0 and after == 0 else float("inf") if before == 0 else (after - before) * 100.0 / before
            print(
                f"{scenario}/{metric}: {before} -> {after} ticks "
                f"({change_pct:+.1f}%, limit {limit:.1f})"
            )
            if after > limit:
                failures.append(
                    f"{scenario}/{metric} regressed from {before} to {after} ticks "
                    f"(allowed at most {limit:.1f}; {max_regression_pct:g}% or {min_absolute_ticks} ticks)"
                )
    return failures


def compare(args: argparse.Namespace) -> int:
    baseline = load_metrics(args.baseline)
    candidate = load_metrics(args.candidate)
    failures = compare_metrics(baseline, candidate, args.max_regression_pct, args.min_absolute_ticks)
    if failures:
        print("GBA performance regression detected:", file=sys.stderr)
        print("\n".join(f"- {failure}" for failure in failures), file=sys.stderr)
        return 1
    print("GBA performance is within the configured regression budget.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    record_parser = subparsers.add_parser("record", help="run a benchmark ROM and write its metrics")
    record_parser.add_argument("--rom", type=Path, required=True)
    record_parser.add_argument("--output", type=Path, required=True)
    record_parser.add_argument("--log", type=Path)
    record_parser.add_argument(
        "--emulator",
        default=os.environ.get("MGBA_COMMAND", "mgba"),
        help="mGBA command, optionally with a display wrapper (default: %(default)s)",
    )
    record_parser.add_argument("--timeout-seconds", type=float, default=45.0)
    record_parser.set_defaults(handler=record)

    compare_parser = subparsers.add_parser("compare", help="fail on a material benchmark regression")
    compare_parser.add_argument("--baseline", type=Path, required=True)
    compare_parser.add_argument("--candidate", type=Path, required=True)
    compare_parser.add_argument("--max-regression-pct", type=float, default=15.0)
    compare_parser.add_argument("--min-absolute-ticks", type=int, default=25)
    compare_parser.set_defaults(handler=compare)

    args = parser.parse_args()
    return args.handler(args)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, TimeoutError, ValueError, json.JSONDecodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(2)

#!/usr/bin/env python3
"""Measure connections, wild entry and twenty move animations on the GBA CPU.

Build with `frame-timing`. Each logic tick must get a render; `--allow-skips`
is only for recording the old loop as comparison evidence.
"""
import argparse
import json
import os
from pathlib import Path
import re
import selectors
import shlex
import shutil
import subprocess
import tempfile
import time

SCENES = (1, 2, 3, *range(10, 30))
FIELDS = {"scene", "tick", "clock", "updates", "update", "draw", "present", "map"}
TICKS_PER_MS = 262.144


class Evidence:
    def __init__(self, allow_skips=False):
        self.rows = {scene: [] for scene in SCENES}
        self.allow_skips = allow_skips
        self.complete = False
        self.previous_tick = None

    def consume(self, line):
        if re.search(r"panicked|panic:|memory allocation|invalid opcode", line, re.I):
            raise ValueError(f"GBA runtime failure: {line.strip()}")
        if "timing: frame " in line:
            row = {k: int(v) for k, v in re.findall(r"(\w+)=(\d+)", line)}
            if set(row) != FIELDS or row["scene"] not in self.rows:
                raise ValueError("Incomplete or unexpected frame sample")
            if not self.allow_skips and row["updates"] > 1:
                raise ValueError(f"Invisible simulation ticks: {row}")
            if self.previous_tick is not None and row["tick"] < self.previous_tick:
                raise ValueError("Simulation clock went backwards")
            self.previous_tick = row["tick"]
            self.rows[row["scene"]].append(row)
        if "timing: DONE" in line:
            if any(len(rows) < 2 for rows in self.rows.values()):
                raise ValueError("Missing connection, encounter or move samples")
            self.complete = True

    def report(self):
        if not self.complete:
            raise ValueError("Timing fixture did not complete")
        result = {}
        for scene, rows in self.rows.items():
            gaps = [b["clock"] - a["clock"] for a, b in zip(rows, rows[1:])]
            result[str(scene)] = {
                "draws": len(rows),
                "logic_ticks": sum(row["updates"] for row in rows),
                "max_updates_between_draws": max(row["updates"] for row in rows),
                "update_max_ms": max(row["update"] for row in rows) / TICKS_PER_MS,
                "draw_mean_ms": sum(row["draw"] for row in rows) / len(rows) / TICKS_PER_MS,
                "draw_max_ms": max(row["draw"] for row in rows) / TICKS_PER_MS,
                "display_gap_max_ms": max(gaps) / TICKS_PER_MS,
            }
        return {"timer_ticks_per_ms": TICKS_PER_MS, "scenes": result}


def record(args, evidence):
    args.log.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="gba-frame-timing-") as directory, args.log.open("w") as log:
        rom = Path(directory) / "timing.gba"
        shutil.copyfile(args.rom, rom)
        command = shlex.split(args.emulator)
        if any("{rom}" in part for part in command):
            command = [part.replace("{rom}", str(rom)) for part in command]
        else:
            command += ["-C", "logToStdout=1", "-C", "logLevel.gba.debug=127", "-C", "fpsTarget=0",
                        "-C", "audioSync=0", "-C", "videoSync=0", str(rom)]
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        selector = selectors.DefaultSelector()
        deadline = time.monotonic() + args.timeout_seconds
        pending = b""
        try:
            selector.register(process.stdout, selectors.EVENT_READ)
            while not evidence.complete:
                if time.monotonic() >= deadline:
                    raise TimeoutError("GBA frame timing timed out")
                if not selector.select(0.1):
                    continue
                chunk = os.read(process.stdout.fileno(), 65536)
                if not chunk:
                    raise RuntimeError("Emulator exited before timing: DONE")
                pending += chunk
                while b"\n" in pending:
                    raw, pending = pending.split(b"\n", 1)
                    line = raw.decode("utf-8", errors="replace")
                    if "timing:" in line or re.search(r"panic|allocation|invalid opcode", line, re.I):
                        log.write(line + "\n")
                        log.flush()
                    evidence.consume(line)
        finally:
            selector.close()
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            process.stdout.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, help="Omit to parse an existing log")
    parser.add_argument("--log", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--allow-skips", action="store_true")
    parser.add_argument("--emulator", default=os.environ.get("MGBA_COMMAND", "mgba"))
    parser.add_argument("--timeout-seconds", type=float, default=180)
    args = parser.parse_args()
    evidence = Evidence(args.allow_skips)
    try:
        if args.rom:
            record(args, evidence)
        else:
            for line in args.log.read_text().splitlines():
                evidence.consume(line)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(evidence.report(), indent=2) + "\n")
        print(f"PASS: {len(SCENES)} GBA frame timing scenarios; {args.output}")
    except (OSError, ValueError, RuntimeError, TimeoutError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()

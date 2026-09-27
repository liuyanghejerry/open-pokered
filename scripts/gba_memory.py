#!/usr/bin/env python3
"""Gate GBA encounter or broad scenario ROMs on completion and memory margins.

Runs an isolated copy of the ROM (never reads or overwrites a player's .sav).
--emulator may be a normal mGBA command, or a complete command containing {rom}.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import selectors
import shlex
import shutil
import subprocess
import tempfile
import time

CASES = ("before-parcel-lower", "before-parcel-upper", "after-parcel-lower",
         "final-upper", "final-lower")
ROUNDS = len(CASES) * 8
SCENARIOS = (("pokedex", 302), ("maps", 248), ("oak-parcel", 1), ("oak-dex", 16), ("fossil", 1),
             ("slots", 3), ("menus", 48), ("moves", 330), ("evolution", 3), ("trade", 3),
             ("ending", 1), ("pc-full", 300), ("save-full", 1))
FAILURE = re.compile(r"panicked|panic:|memory allocation|invalid opcode|invalid address", re.I)


class Evidence:
    def __init__(self, suite="route22"):
        self.suite = suite
        self.expected = ROUNDS if suite == "route22" else len(SCENARIOS)
        self.passed = 0
        self.min_heap = None
        self.min_stack = None
        self.complete = False

    def consume(self, line: str) -> None:
        if FAILURE.search(line):
            raise ValueError(f"GBA runtime failure: {line.strip()}")
        if match := re.search(r"repro: heap low (\d+)B", line):
            value = int(match[1])
            self.min_heap = value if self.min_heap is None else min(self.min_heap, value)
            if value < 4096:
                raise ValueError(f"GBA contiguous heap margin below 4096 B: {value}")
        if match := re.search(r"(?:route22|memory): .*stack=(\d+)B", line):
            value = int(match[1])
            self.min_stack = value if self.min_stack is None else min(self.min_stack, value)
            if value < 4096:
                raise ValueError(f"GBA untouched stack margin below 4096 B: {value}")
        if self.suite == "route22" and (match := re.search(r"route22: PASS round=(\d+) case=(\S+)", line)):
            if self.passed >= ROUNDS or int(match[1]) != self.passed + 1 or match[2] != CASES[self.passed % len(CASES)]:
                raise ValueError(f"Missing, repeated or incorrect encounter: {line.strip()}")
            self.passed += 1
        if self.suite == "route22" and (match := re.search(r"route22: ALL PASS rounds=(\d+) save=ok stack=(\d+)B", line)):
            if self.passed != ROUNDS or int(match[1]) != ROUNDS:
                raise ValueError(f"Incomplete encounter suite: {self.passed}/{ROUNDS}")
            if self.min_heap is None or self.min_stack is None:
                raise ValueError("Missing memory measurements")
            self.complete = True
        if self.suite == "scenarios" and (match := re.search(r"memory: PASS case=(\S+) count=(\d+)", line)):
            if self.passed >= len(SCENARIOS) or (match[1], int(match[2])) != SCENARIOS[self.passed]:
                raise ValueError(f"Missing, repeated or incorrect scenario: {line.strip()}")
            self.passed += 1
        if self.suite == "scenarios" and "memory: ALL PASS" in line:
            if self.passed != len(SCENARIOS):
                raise ValueError(f"Incomplete scenario suite: {self.passed}/{len(SCENARIOS)}")
            if self.min_heap is None or self.min_stack is None:
                raise ValueError("Missing memory measurements")
            self.complete = True


def run(args) -> None:
    evidence = Evidence(getattr(args, "suite", "route22"))
    args.log.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="gba-memory-") as directory, args.log.open("w") as log:
        rom = Path(directory) / "regression.gba"
        shutil.copyfile(args.rom, rom)
        command = shlex.split(args.emulator)
        if any("{rom}" in part for part in command):
            command = [part.replace("{rom}", str(rom)) for part in command]
        else:
            command += ["-C", "logToStdout=1", "-C", "logLevel.gba.debug=127",
                        "-C", "fpsTarget=0", "-C", "audioSync=0", "-C", "videoSync=0", str(rom)]
        print("Running:", shlex.join(command), flush=True)
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        selector = selectors.DefaultSelector()
        deadline = time.monotonic() + args.timeout_seconds
        pending = b""
        try:
            assert process.stdout is not None
            selector.register(process.stdout, selectors.EVENT_READ)
            while not evidence.complete:
                if time.monotonic() >= deadline:
                    raise TimeoutError(f"GBA memory regression timed out after {args.timeout_seconds}s")
                if not selector.select(0.1):
                    continue
                chunk = os.read(process.stdout.fileno(), 65536)
                if not chunk:
                    raise RuntimeError(f"Emulator exited before ALL PASS ({evidence.passed}/{evidence.expected})")
                pending += chunk
                while b"\n" in pending:
                    raw, pending = pending.split(b"\n", 1)
                    line = raw.decode("utf-8", errors="replace")
                    log.write(line + "\n")
                    if "route22:" in line or "memory:" in line or "heap low" in line or FAILURE.search(line):
                        print(line, flush=True)
                    evidence.consume(line)
            print(f"GBA memory PASS: {evidence.passed} {evidence.suite} cases; "
                  f"min contiguous heap={evidence.min_heap} B, untouched stack={evidence.min_stack} B")
        finally:
            if pending:
                log.write(pending.decode("utf-8", errors="replace"))
            selector.close()
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            if process.stdout:
                process.stdout.close()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", choices=("route22", "scenarios"), default="route22")
    parser.add_argument("--rom", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    parser.add_argument("--emulator", default=os.environ.get("MGBA_COMMAND", "mgba"))
    parser.add_argument("--timeout-seconds", type=float, default=120)
    args = parser.parse_args()
    try:
        run(args)
    except (OSError, ValueError, RuntimeError, TimeoutError) as error:
        parser.exit(1, f"{error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

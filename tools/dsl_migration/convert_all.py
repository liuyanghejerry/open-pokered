#!/usr/bin/env python3
"""Run convert_script_to_scene.py over every map directory.

Reports per-map status (OK / FAILED / SKIPPED) and a summary.
"""
import subprocess
import sys
from pathlib import Path

MAPS_DIR = Path(__file__).resolve().parents[2] / "examples/pokered/crates/pokered-data/maps"
TOOL = Path(__file__).resolve().parent / "convert_script_to_scene.py"


def main():
    if not MAPS_DIR.exists():
        sys.exit(f"maps dir not found: {MAPS_DIR}")
    if not TOOL.exists():
        sys.exit(f"tool not found: {TOOL}")

    maps = sorted(d for d in MAPS_DIR.iterdir() if d.is_dir())
    print(f"converting {len(maps)} maps…")

    ok, fail, skip = [], [], []
    for m in maps:
        if not (m / "script.js").exists():
            skip.append(m.name)
            continue
        try:
            r = subprocess.run(
                [sys.executable, str(TOOL), str(m)],
                capture_output=True, text=True, timeout=15,
            )
        except subprocess.TimeoutExpired:
            fail.append((m.name, "TIMEOUT"))
            continue
        if r.returncode == 0:
            ok.append(m.name)
        else:
            msg = (r.stderr or r.stdout).strip().splitlines()[-1] if r.stderr or r.stdout else "no output"
            fail.append((m.name, msg))

    print(f"\n=== summary ===")
    print(f"  ok    : {len(ok)}")
    print(f"  fail  : {len(fail)}")
    print(f"  skip  : {len(skip)}")
    if fail:
        print(f"\n=== failures (showing first 20) ===")
        for name, msg in fail[:20]:
            short = msg[:120]
            print(f"  {name}: {short}")


if __name__ == "__main__":
    main()

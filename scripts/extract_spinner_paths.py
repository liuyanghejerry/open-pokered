#!/usr/bin/env python3
"""Extract the spinner-tile RLE movement tables from a pret/pokered checkout.

Generates crates/pokered-core/src/overworld/spinner_paths.rs from the
<Map>ArrowTilePlayerMovement tables (scripts/RocketHideoutB2F.asm,
RocketHideoutB3F.asm, ViridianGym.asm) — the only maps whose scripts install
BIT_SPINNING paths. B1F/B4F arrow tiles are decorative (no table).

Usage: extract_spinner_paths.py <pokered-worktree> [-o out.rs]
"""
import argparse
import os
import re
import sys

MAPS = {
    "RocketHideoutB2F": "RocketHideoutB2F.asm",
    "RocketHideoutB3F": "RocketHideoutB3F.asm",
    "ViridianGym": "ViridianGym.asm",
}
PAD = {"PAD_DOWN": "Down", "PAD_UP": "Up", "PAD_LEFT": "Left", "PAD_RIGHT": "Right"}


def extract(scripts_dir: str) -> list[tuple[str, list[tuple[int, int, list[str]]]]]:
    result = []
    for map_name, fname in MAPS.items():
        src = open(os.path.join(scripts_dir, fname)).read()
        tbl = re.search(r"\w+ArrowTilePlayerMovement:\n(.*?)\n\tdb -1", src, re.S)
        if not tbl:
            raise SystemExit(f"no arrow table found in {fname}")
        entries = []
        for m in re.finditer(r"map_coord_movement\s+(\d+),\s+(\d+),\s+(\w+)", tbl.group(1)):
            x, y, list_name = int(m.group(1)), int(m.group(2)), m.group(3)
            mm = re.search(list_name + r":\n((?:\tdb [^\n]+\n)+)", src)
            if not mm:
                raise SystemExit(f"movement list {list_name} not found in {fname}")
            steps = []
            for s in re.finditer(r"db (PAD_\w+), (\d+)", mm.group(1)):
                steps.append(
                    f"SpinnerStep {{ dir: Direction::{PAD[s.group(1)]}, steps: {s.group(2)} }}"
                )
            entries.append((x, y, steps))
        result.append((map_name, entries))
    return result


def render(tables) -> str:
    out = [
        "//! Spinner-tile RLE movement tables, GENERATED from the original scripts",
        "//! (scripts/RocketHideoutB2F/B3F.asm, ViridianGym.asm — the",
        "//! `<Map>ArrowTilePlayerMovement` tables + their RLE movement lists;",
        "//! DecodeArrowMovementRLE + map_objects.asm semantics: standing on (x,y)",
        "//! feeds `dir × steps` straight-line simulated input while the sprite",
        "//! spins (LoadSpinnerArrowTiles) regardless of travel direction. B1F/B4F",
        "//! arrow tiles have no table (decorative).",
        "//! Do not edit by hand — regenerate via scripts/extract_spinner_paths.py.",
        "",
        "use crate::overworld::Direction;",
        "",
        "/// One spin-pad entry: the pad input to simulate, repeated `steps` times.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct SpinnerStep {",
        "    pub dir: Direction,",
        "    pub steps: u8,",
        "}",
        "",
        "/// (x, y) → movement list for the map. First match wins (asm order).",
        "pub fn spinner_paths(map: &str) -> &'static [(u8, u8, &'static [SpinnerStep])] {",
        "    match map {",
    ]
    total = 0
    for map_name, entries in tables:
        total += len(entries)
        out.append(f'        "{map_name}" => &[')
        for x, y, steps in entries:
            body = ", ".join(steps)
            out.append(f"            ({x}, {y}, &[{body}]),")
        out.append("        ],")
    out.append("        _ => &[],")
    out.append("    }")
    out.append("}")
    sys.stderr.write(f"extracted {total} entries across {len(tables)} maps\n")
    return "\n".join(out) + "\n"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("worktree", help="pret/pokered checkout (scripts/ dir)")
    ap.add_argument("-o", default="crates/pokered-core/src/overworld/spinner_paths.rs")
    args = ap.parse_args()
    tables = extract(os.path.join(args.worktree, "scripts"))
    with open(args.o, "w") as f:
        f.write(render(tables))


if __name__ == "__main__":
    main()

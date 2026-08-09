#!/usr/bin/env python3
"""Verify pokered-data's cry table against the pret/pokered disassembly.

Compares `CRY_DATA` in
  workspace/examples/pokered/crates/pokered-data/src/cries.rs
against `data/pokemon/cries.asm` (`mon_cry BASE, PITCH, LENGTH ; Name`) in a
local pret/pokered checkout. The Rust table stores the base cry as a raw u8
in `SfxId` id space (pokered-audio/src/sfx_data.rs: `Cry00 = N`, consecutive
cries step by 1).

Usage:
  verify_cry_data.py [asm_repo]            # diff mode (exit 1 on mismatch)
  verify_cry_data.py [asm_repo] --generate # print the Rust match arms
"""

import re
import sys
from pathlib import Path

WORKSPACE = Path(__file__).resolve().parent.parent
RUST_CRIES = WORKSPACE / "examples/pokered/crates/pokered-data/src/cries.rs"
RUST_SFX_ENUM = WORKSPACE / "examples/pokered/crates/pokered-audio/src/sfx_data.rs"
DEFAULT_ASM_REPO = "/Users/liuyanghe02/develop/pokered-worktree"

# asm display name -> Rust Species variant (only the irregular ones).
NAME_TO_VARIANT = {
    "Nidoran♀": "NidoranF",
    "Nidoran♂": "NidoranM",
    "Mr.Mime": "MrMime",
    "Farfetch'd": "Farfetchd",
}


def variant_for(name: str) -> str:
    if name in NAME_TO_VARIANT:
        return NAME_TO_VARIANT[name]
    return name.replace(" ", "")


def parse_asm(asm_repo: Path):
    rows = []
    text = (asm_repo / "data/pokemon/cries.asm").read_text()
    for m in re.finditer(
        r"mon_cry\s+SFX_CRY_([0-9A-F]{2}),\s*\$([0-9A-Fa-f]{2}),\s*\$([0-9A-Fa-f]{2})\s*;\s*(.+)",
        text,
    ):
        cry_idx, pitch, length, name = m.groups()
        rows.append((name.strip(), int(cry_idx, 16), int(pitch, 16), int(length, 16)))
    return rows


def cry00_base() -> int:
    """The `Cry00 = N` discriminant in the Rust SfxId enum."""
    m = re.search(r"Cry00\s*=\s*(\d+)", RUST_SFX_ENUM.read_text())
    if not m:
        print("ERROR: cannot find `Cry00 = N` in sfx_data.rs")
        sys.exit(2)
    return int(m.group(1))


def parse_rust():
    rows = {}
    text = RUST_CRIES.read_text()
    for m in re.finditer(
        r"Species::(\w+)\s*=>\s*CryData\s*\{\s*sfx:\s*(\d+),"
        r"\s*pitch:\s*0x([0-9A-Fa-f]{2}),\s*length:\s*0x([0-9A-Fa-f]{2})\s*\}",
        text,
    ):
        sp, sfx, pitch, length = m.groups()
        rows[sp] = (int(sfx), int(pitch, 16), int(length, 16))
    return rows


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    generate = "--generate" in sys.argv
    asm_repo = Path(args[0]) if args else Path(DEFAULT_ASM_REPO)
    base = cry00_base()

    asm_rows = [r for r in parse_asm(asm_repo) if r[0] != "MissingNo."]
    if len(asm_rows) != 151:
        print(f"ERROR: expected 151 non-MissingNo mon_cry rows, got {len(asm_rows)}")
        sys.exit(2)

    if generate:
        for name, idx, pitch, length in asm_rows:
            print(
                f"    Species::{variant_for(name)} => CryData {{ "
                f"sfx: {base + idx}, pitch: 0x{pitch:02X}, length: 0x{length:02X} }},"
                f" // {name} (SFX_CRY_{idx:02X})"
            )
        return

    rust_rows = parse_rust()
    diffs = 0
    for name, idx, pitch, length in asm_rows:
        sp = variant_for(name)
        want = (base + idx, pitch, length)
        got = rust_rows.get(sp)
        if got != want:
            diffs += 1
            print(f"DIFF {sp} ({name}): asm={want} rust={got}")
    extra = set(rust_rows) - {variant_for(r[0]) for r in asm_rows}
    for sp in sorted(extra):
        diffs += 1
        print(f"DIFF {sp}: present in rust, missing in asm")

    if diffs:
        print(f"\nRESULT: {diffs} diff(s)")
        sys.exit(1)
    print(f"checked {len(asm_rows)} species against data/pokemon/cries.asm")
    print(f"(SfxId id space: Cry00 = {base}, consecutive cries step by 1)")
    print("\nRESULT: 0 diffs — Rust table matches the disassembly")


if __name__ == "__main__":
    main()

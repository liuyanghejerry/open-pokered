#!/usr/bin/env python3
"""Verify the Rust MoveSoundTable transcription against the pokered disassembly.

Compares, row by row, the table transcribed into
  workspace/examples/pokered/crates/pokered-data/src/move_sfx.rs  (MOVE_SFX_TABLE)
against the original assembly source:
  data/moves/sfx.asm                            -> (sfx name, pitch mod, tempo mod)

The `sfx` column is compared in the `pokered_audio::sfx_data::SfxId` id space,
which this script derives independently from the asm repo exactly the way
tools/asm2sfx.py generated the enum: SFX header labels are collected in file
order from audio/headers/sfxheaders{1,2,3}.asm (skipping the SFX_Headers_N::
table labels and the `db $ff,$ff,$ff` padding rows), canonicalized by stripping
the bank suffix (_1/_2/_3), first occurrence wins; the id is the dedup index.
constants/music_constants.asm (`music_const SFX_FOO, SFX_Foo_2`) maps the
SFX_* constant names used by sfx.asm onto those canonical labels.

The pitch/tempo columns are ROM bytes and are compared byte-for-byte.

Usage:
  python3 verify_move_sfx_data.py [path-to-asm-repo]
  (default asm repo: /Users/liuyanghe02/develop/pokered-worktree)

Exit code 0 when everything matches, 1 when diffs were found.
"""

import re
import sys
from pathlib import Path

DEFAULT_ASM_REPO = "/Users/liuyanghe02/develop/pokered-worktree"
RUST_MOVE_SFX_RS = Path(__file__).resolve().parent.parent / \
    "examples/pokered/crates/pokered-data/src/move_sfx.rs"

SFX_HEADER_FILES = [
    "audio/headers/sfxheaders1.asm",
    "audio/headers/sfxheaders2.asm",
    "audio/headers/sfxheaders3.asm",
]


def canonical_label(label):
    """Strip the bank suffix (_1/_2/_3) like asm2sfx.py canonical_sfx_name."""
    return re.sub(r"_[123]$", "", label)


def asm_sfx_id_space(asm_repo):
    """-> {canonical label: SfxId-space id}, replicating asm2sfx.py's dedup."""
    canon_index = {}
    for rel in SFX_HEADER_FILES:
        for raw in (asm_repo / rel).read_text().splitlines():
            code = raw.split(";", 1)[0].strip()
            m = re.match(r"(\w+)::$", code)
            if not m:
                continue
            label = m.group(1)
            if re.fullmatch(r"SFX_Headers_\d", label):
                continue  # table label, not an SFX entry
            canon = canonical_label(label)
            if canon not in canon_index:
                canon_index[canon] = len(canon_index)
    return canon_index


def asm_sfx_const_labels(asm_repo):
    """-> {SFX_* constant name: canonical header label}."""
    name2label = {}
    for raw in (asm_repo / "constants/music_constants.asm").read_text().splitlines():
        m = re.match(r"\s*music_const\s+(\w+),\s*(\w+)",
                     raw.split(";", 1)[0])
        if m:
            name2label[m.group(1)] = canonical_label(m.group(2))
    return name2label


def asm_move_sound_table(asm_repo, canon_index, name2label):
    """-> [(sfx_id, pitch_mod, tempo_mod, comment)] from data/moves/sfx.asm."""
    rows = []
    for raw in (asm_repo / "data/moves/sfx.asm").read_text().splitlines():
        m = re.match(
            r"\s*db\s+(SFX_\w+),\s*\$([0-9A-Fa-f]{2}),\s*\$([0-9A-Fa-f]{2})"
            r"\s*(?:;\s*(\w+))?", raw)
        if m:
            name = m.group(1)
            label = name2label.get(name)
            if label is None or label not in canon_index:
                raise ValueError(f"unresolvable SFX constant: {name}")
            rows.append((canon_index[label], int(m.group(2), 16),
                         int(m.group(3), 16), m.group(4) or "(table tail)"))
    return rows


RUST_ROW_RE = re.compile(
    r"MoveSfx\s*\{\s*sfx:\s*(\d+),\s*pitch_mod:\s*0x([0-9A-Fa-f]{2}),"
    r"\s*tempo_mod:\s*0x([0-9A-Fa-f]{2})\s*\}")


def rust_move_sound_table(path):
    text = path.read_text()
    sec_start = text.index("pub static MOVE_SFX_TABLE")
    sec_end = text.index("\n];", sec_start)
    return [(int(sfx), int(p, 16), int(t, 16))
            for sfx, p, t in RUST_ROW_RE.findall(text[sec_start:sec_end])]


def main():
    asm_repo = Path(sys.argv[1] if len(sys.argv) > 1 else DEFAULT_ASM_REPO)
    for rel in SFX_HEADER_FILES + ["constants/music_constants.asm",
                                   "data/moves/sfx.asm"]:
        if not (asm_repo / rel).is_file():
            sys.exit(f"error: missing {asm_repo / rel}")
    if not RUST_MOVE_SFX_RS.is_file():
        sys.exit(f"error: missing {RUST_MOVE_SFX_RS}")

    canon_index = asm_sfx_id_space(asm_repo)
    name2label = asm_sfx_const_labels(asm_repo)
    asm_rows = asm_move_sound_table(asm_repo, canon_index, name2label)
    rust_rows = rust_move_sound_table(RUST_MOVE_SFX_RS)

    diffs = 0
    if len(asm_rows) != len(rust_rows):
        print(f"DIFF table length: asm {len(asm_rows)} vs rust {len(rust_rows)}")
        diffs += 1
    for i, (a, r) in enumerate(zip(asm_rows, rust_rows)):
        if a[:3] != r:
            print(f"DIFF row {i} ({a[3]}):")
            print(f"  asm : sfx={a[0]}, pitch=0x{a[1]:02X}, tempo=0x{a[2]:02X}")
            print(f"  rust: sfx={r[0]}, pitch=0x{r[1]:02X}, tempo=0x{r[2]:02X}")
            diffs += 1

    print()
    print(f"asm repo : {asm_repo}")
    print(f"rust data: {RUST_MOVE_SFX_RS}")
    print(f"rows compared: {len(asm_rows)} "
          f"(NUM_ATTACKS={len(asm_rows) - 1} + 1 table-tail row)")
    print(f"sfx id space: {len(canon_index)} deduplicated SFX headers "
          f"(asm2sfx.py convention)")
    print()
    if diffs:
        print(f"RESULT: {diffs} diff(s)")
        return 1
    print("RESULT: 0 diffs — Rust table matches the disassembly")
    return 0


if __name__ == "__main__":
    sys.exit(main())

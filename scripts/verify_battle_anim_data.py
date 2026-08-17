#!/usr/bin/env python3
"""Verify Rust battle-animation data against the original pokered disassembly.

Compares, entry by entry, the four data tables transcribed into
  workspace/crates/dotzuki-renderer/src/battle_anim/data.rs
against the original assembly sources in a pret/pokered-style repo:
  data/battle_anims/base_coords.asm    -> BASE_COORDS
  data/battle_anims/frame_blocks.asm   -> FRAME_BLOCK_DATA
  data/battle_anims/subanimations.asm  -> SUBANIM_DATA
  data/moves/animations.asm            -> MOVE_ANIM_DATA

Usage:
  python3 verify_battle_anim_data.py [path-to-asm-repo]
  (default asm repo: /Users/liuyanghe02/develop/pokered-worktree)

Exit code 0 when everything matches, 1 when diffs were found.

Encoding notes (see macros/gfx.asm `dbsprite`, data/moves/animations.asm
`battle_anim`, and data.rs / player.rs decode_command):

- dbsprite x_tile, y_tile, x_px, y_px, tile, attrs expands to 4 bytes:
    Y = y_tile*8 + y_px, X = x_tile*8 + x_px, tile, attrs.
  Rust stores (x_px, y_px, tile, flags) with x_px = X, y_px = Y — the full
  pixel bytes, so sub-tile pixel offsets are preserved exactly.
- battle_anim sound, subanim, tileset, delay -> bytes (tileset<<6)|delay,
  sound-1, subanim.  Rust tuple: (0, sound, subanim, (tileset<<6)|delay).
- battle_anim sound, SE_ID -> bytes se_id, sound-1.
  Rust tuple: (1, sound, se_id, 0).
- subanim TYPE, count -> header byte (type<<5)|count, then count triples
  db FRAMEBLOCK_x, BASECOORD_x, FRAMEBLOCKMODE_x.
  Rust: (type, [(frameblock, basecoord, mode), ...]).
"""

import re
import sys
from pathlib import Path

DEFAULT_ASM_REPO = "/Users/liuyanghe02/develop/pokered-worktree"
# The Rust table lives in the dotzuki engine checkout. Default: the historical
# in-repo path (or a symlink); override with the DOTZUKI_RENDERER env var or a
# second CLI argument pointing at the engine workspace/crate root.
RUST_DATA_RS = Path(__file__).resolve().parent.parent / \
    "crates/dotzuki-renderer/src/battle_anim/data.rs"


# ─── asm constant parsing ─────────────────────────────────────────────

def parse_constants(path):
    """Parse RGBDS const_def/const/const_skip blocks into {name: value}."""
    consts = {}
    value = 0
    for raw in path.read_text().splitlines():
        line = raw.split(";", 1)[0].strip()
        if not line:
            continue
        m = re.match(r"const_def(?:\s+\$([0-9A-Fa-f]+))?", line)
        if m:
            value = int(m.group(1), 16) if m.group(1) else 0
            continue
        m = re.match(r"const_skip\s+(\S+)", line)
        if m:
            arg = m.group(1)
            value += int(arg[1:], 16) if arg.startswith("$") else int(arg, 0)
            continue
        m = re.match(r"const\s+(\w+)", line)
        if m:
            consts[m.group(1)] = value
            value += 1
    return consts


def read_lines(path):
    return path.read_text().splitlines()


def strip_comment(line):
    return line.split(";", 1)[0]


def parse_pointer_table(lines, start_marker):
    """Return the dw label list following `start_marker:` until a non-dw line."""
    labels = []
    started = False
    for line in lines:
        if not started:
            if line.rstrip() == start_marker + ":":
                started = True
            continue
        m = re.match(r"\s+dw\s+(\w+)", strip_comment(line))
        if m:
            labels.append(m.group(1))
        elif re.match(r"\s+assert_table_length", line):
            continue
        elif labels:
            break
    return labels


def find_label_bodies(lines):
    """Map every `Label:` (col 0) to the list of lines that follow it,
    up to the next label that introduces content (aliases share a body)."""
    positions = [(i, m.group(1)) for i, ln in enumerate(lines)
                 if (m := re.match(r"^(\w+):", ln))]
    bodies = {}
    for idx, (start, name) in enumerate(positions):
        end = positions[idx + 1][0] if idx + 1 < len(positions) else len(lines)
        bodies[name] = lines[start + 1:end]
    return bodies, positions


# ─── asm data extraction ──────────────────────────────────────────────

def asm_base_coords(lines):
    coords = []
    for line in lines:
        m = re.match(r"\s+db\s+\$([0-9A-Fa-f]{2}),\s*\$([0-9A-Fa-f]{2})",
                     strip_comment(line))
        if m:
            coords.append((int(m.group(1), 16), int(m.group(2), 16)))
    return coords


OAM_FLAGS = {"OAM_XFLIP": 0x20, "OAM_YFLIP": 0x40, "OAM_PAL1": 0x10,
             "OAM_PRIO": 0x80}


def parse_attrs(text):
    text = text.strip()
    if text in ("", "0"):
        return 0
    value = 0
    for part in text.split("|"):
        part = part.strip()
        if part in OAM_FLAGS:
            value |= OAM_FLAGS[part]
        elif re.fullmatch(r"\$[0-9A-Fa-f]+", part):
            value |= int(part[1:], 16)
        elif re.fullmatch(r"\d+", part):
            value |= int(part)
        else:
            raise ValueError(f"unknown OAM attr: {part!r}")
    return value


def asm_frame_blocks(lines):
    """-> ordered [(label, [(x_px, y_px, tile, flags), ...])]"""
    order = parse_pointer_table(lines, "FrameBlockPointers")
    bodies, _ = find_label_bodies(lines)
    blocks = []
    subtile = 0
    for label in order:
        body = bodies[label]
        count = None
        tiles = []
        for line in body:
            code = strip_comment(line)
            if count is None:
                m = re.match(r"\s+db\s+(\d+)\s*$", code)
                if m:
                    count = int(m.group(1))
                continue
            m = re.match(r"\s+dbsprite\s+(.+)$", code)
            if m:
                args = re.split(r"[,\s]+", m.group(1).strip())
                xt, yt, xpx, ypx = (int(a) for a in args[0:4])
                tile = int(args[4].lstrip("$"), 16)
                flags = parse_attrs(" ".join(args[5:]))
                x_byte, y_byte = xt * 8 + xpx, yt * 8 + ypx
                if xpx or ypx:
                    subtile += 1
                tiles.append((x_byte, y_byte, tile, flags))
            if count is not None and len(tiles) >= count:
                break
        if count is None:
            raise ValueError(f"{label}: missing sprite-count db line")
        if len(tiles) != count:
            raise ValueError(f"{label}: expected {count} sprites, got {len(tiles)}")
        blocks.append((label, tiles))
    return blocks, subtile


def asm_subanimations(lines, consts):
    order = parse_pointer_table(lines, "SubanimationPointers")
    bodies, _ = find_label_bodies(lines)
    subanims = []
    for label in order:
        body = bodies[label]
        stype = count = None
        frames = []
        for line in body:
            code = strip_comment(line)
            m = re.match(r"\s+subanim\s+(\w+),\s*(\d+)", code)
            if m:
                stype, count = consts[m.group(1)], int(m.group(2))
                continue
            m = re.match(r"\s+db\s+(\w+),\s*(\w+),\s*(\w+)", code)
            if m and stype is not None:
                frames.append((consts[m.group(1)], consts[m.group(2)],
                               consts[m.group(3)]))
        if stype is None:
            raise ValueError(f"{label}: missing subanim header")
        if len(frames) != count:
            raise ValueError(f"{label}: expected {count} frames, got {len(frames)}")
        subanims.append((label, stype, frames))
    return subanims


def asm_move_anims(lines, consts):
    # The dw list includes ZigZagScreenAnim after the final
    # `assert_table_length NUM_ATTACK_ANIMS` (index 202, 0-based).
    order = parse_pointer_table(lines, "AttackAnimationPointers")
    _, positions = find_label_bodies(lines)
    anims = []
    for label in order:
        start = next(i for i, name in positions if name == label)
        cmds = []
        terminated = False
        for line in lines[start + 1:]:
            if re.match(r"^\w+:", line):
                continue  # stacked alias labels share the same stream
            code = strip_comment(line)
            m = re.match(r"\s+battle_anim\s+(.+)$", code)
            if m:
                args = [a.strip() for a in m.group(1).split(",")]
                sound = consts[args[0]]
                if len(args) == 4:
                    subanim = consts[args[1]]
                    tileset, delay = int(args[2]), int(args[3])
                    cmds.append((0, sound, subanim, (tileset << 6) | delay))
                elif len(args) == 2:
                    cmds.append((1, sound, consts[args[1]], 0))
                else:
                    raise ValueError(f"{label}: bad battle_anim arity: {line}")
                continue
            if re.match(r"\s+db\s+-1", code):
                terminated = True
                break
        if not terminated:
            raise ValueError(f"{label}: missing `db -1` terminator")
        anims.append((label, cmds))
    return anims


# ─── Rust data.rs extraction ──────────────────────────────────────────

INT = r"(?:0x[0-9A-Fa-f]+|\d+)"


def parse_int(text):
    return int(text, 0)


def rust_section(text, marker):
    start = text.index("= [", text.index(marker))
    end = text.index("\n];", start)
    return text[start:end]


def rust_base_coords(text):
    sec = rust_section(text, "pub static BASE_COORDS")
    return [(parse_int(a), parse_int(b)) for a, b in
            re.findall(rf"\(\s*({INT}),\s*({INT})\s*\)", sec)]


def rust_tuple_lists(text, marker, arity):
    """Parse `&[ (..), .. ]` entries of a static array of slices."""
    sec = rust_section(text, marker)
    entries = []
    for m in re.finditer(r"&\[(.*?)\]", sec, re.DOTALL):
        body = m.group(1)
        tup_re = r"\(\s*" + r",\s*".join(rf"({INT})" for _ in range(arity)) + r"\s*\)"
        entries.append([tuple(parse_int(g) for g in t)
                        for t in re.findall(tup_re, body)])
    return entries


def rust_subanims(text):
    sec = rust_section(text, "pub static SUBANIM_DATA")
    out = []
    for m in re.finditer(rf"\(\s*({INT}),\s*&\[(.*?)\]\s*,?\s*\)", sec, re.DOTALL):
        stype = parse_int(m.group(1))
        frames = [tuple(parse_int(g) for g in t) for t in
                  re.findall(rf"\(\s*({INT}),\s*({INT}),\s*({INT})\s*\)",
                             m.group(2))]
        out.append((stype, frames))
    return out


# ─── diff reporting ───────────────────────────────────────────────────

class Reporter:
    def __init__(self):
        self.diffs = 0
        self.by_table = {}

    def diff(self, table, where, asm_val, rust_val):
        self.diffs += 1
        self.by_table[table] = self.by_table.get(table, 0) + 1
        print(f"DIFF [{table}] {where}:")
        print(f"  asm : {asm_val}")
        print(f"  rust: {rust_val}")


def main():
    asm_repo = Path(sys.argv[1] if len(sys.argv) > 1 else DEFAULT_ASM_REPO)
    global RUST_DATA_RS
    import os
    override = os.environ.get("DOTZUKI_RENDERER") or (sys.argv[2] if len(sys.argv) > 2 else None)
    if override:
        RUST_DATA_RS = Path(override) / "src/battle_anim/data.rs"
    for rel in ("data/battle_anims/base_coords.asm",
                "data/battle_anims/frame_blocks.asm",
                "data/battle_anims/subanimations.asm",
                "data/moves/animations.asm",
                "constants/move_animation_constants.asm",
                "constants/move_constants.asm"):
        if not (asm_repo / rel).is_file():
            sys.exit(f"error: missing {asm_repo / rel}")
    if not RUST_DATA_RS.is_file():
        sys.exit(f"error: missing {RUST_DATA_RS}")

    consts = {}
    consts.update(parse_constants(asm_repo / "constants/move_constants.asm"))
    consts.update(parse_constants(
        asm_repo / "constants/move_animation_constants.asm"))

    asm_bc = asm_base_coords(read_lines(asm_repo / "data/battle_anims/base_coords.asm"))
    asm_fb, subtile_count = asm_frame_blocks(
        read_lines(asm_repo / "data/battle_anims/frame_blocks.asm"))
    asm_sub = asm_subanimations(
        read_lines(asm_repo / "data/battle_anims/subanimations.asm"), consts)
    asm_anim = asm_move_anims(
        read_lines(asm_repo / "data/moves/animations.asm"), consts)

    rust_text = RUST_DATA_RS.read_text()
    rust_bc = rust_base_coords(rust_text)
    rust_fb = rust_tuple_lists(rust_text, "pub static FRAME_BLOCK_DATA", 4)
    rust_sub = rust_subanims(rust_text)
    rust_anim = rust_tuple_lists(rust_text, "pub static MOVE_ANIM_DATA", 4)

    rep = Reporter()

    # ── BASE_COORDS ──
    if len(asm_bc) != len(rust_bc):
        rep.diff("BASE_COORDS", "table length", len(asm_bc), len(rust_bc))
    for i, (a, r) in enumerate(zip(asm_bc, rust_bc)):
        if a != r:
            rep.diff("BASE_COORDS", f"BASECOORD_{i:02X}", a, r)

    # ── FRAME_BLOCK_DATA ──
    if len(asm_fb) != len(rust_fb):
        rep.diff("FRAME_BLOCK_DATA", "table length", len(asm_fb), len(rust_fb))
    for i, ((label, a_tiles), r_tiles) in enumerate(zip(asm_fb, rust_fb)):
        if len(a_tiles) != len(r_tiles):
            rep.diff("FRAME_BLOCK_DATA",
                     f"{i:02X} {label} tile count",
                     len(a_tiles), len(r_tiles))
        for j, (a, r) in enumerate(zip(a_tiles, r_tiles)):
            if a != r:
                rep.diff("FRAME_BLOCK_DATA", f"{i:02X} {label} tile {j}", a, r)

    # ── SUBANIM_DATA ──
    if len(asm_sub) != len(rust_sub):
        rep.diff("SUBANIM_DATA", "table length", len(asm_sub), len(rust_sub))
    for i, ((label, a_type, a_frames), (r_type, r_frames)) in \
            enumerate(zip(asm_sub, rust_sub)):
        if a_type != r_type:
            rep.diff("SUBANIM_DATA", f"{i:02X} {label} transform type",
                     a_type, r_type)
        if len(a_frames) != len(r_frames):
            rep.diff("SUBANIM_DATA", f"{i:02X} {label} frame count",
                     len(a_frames), len(r_frames))
        for j, (a, r) in enumerate(zip(a_frames, r_frames)):
            if a != r:
                rep.diff("SUBANIM_DATA", f"{i:02X} {label} frame {j}", a, r)

    # ── MOVE_ANIM_DATA ──
    if len(asm_anim) != len(rust_anim):
        rep.diff("MOVE_ANIM_DATA", "table length", len(asm_anim), len(rust_anim))
    for i, ((label, a_cmds), r_cmds) in enumerate(zip(asm_anim, rust_anim)):
        if len(a_cmds) != len(r_cmds):
            rep.diff("MOVE_ANIM_DATA",
                     f"{i:02X} {label} command count",
                     len(a_cmds), len(r_cmds))
        for j, (a, r) in enumerate(zip(a_cmds, r_cmds)):
            if a != r:
                rep.diff("MOVE_ANIM_DATA", f"{i:02X} {label} command {j}", a, r)

    # ── summary ──
    print()
    print(f"asm repo : {asm_repo}")
    print(f"rust data: {RUST_DATA_RS}")
    print(f"entries compared: BASE_COORDS {len(asm_bc)}, "
          f"FRAME_BLOCK_DATA {len(asm_fb)}, SUBANIM_DATA {len(asm_sub)}, "
          f"MOVE_ANIM_DATA {len(asm_anim)}")
    print(f"sub-tile pixel offsets preserved: {subtile_count} tile(s) "
          f"(compared pixel-exact, 0 expected warnings)")
    print()
    if rep.diffs:
        print(f"RESULT: {rep.diffs} diff(s): "
              + ", ".join(f"{k}={v}" for k, v in sorted(rep.by_table.items())))
        return 1
    print("RESULT: 0 diffs — Rust data matches the disassembly")
    return 0


if __name__ == "__main__":
    sys.exit(main())

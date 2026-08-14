#!/usr/bin/env python3
"""Fill missing trainer end-battle defeat lines into map.json npcs.

Reference: scripts/{Map}.asm def_trainers (battle/end/after text labels) +
text/{Map}.asm (text_far targets). The remake stores the defeat line on the
npc's `endBattleText` field (line breaks as \\n).

Usage:
    python3 tools/fill_trainer_texts.py --ref /Users/liuyanghejerry/develop/pokered [--apply]
"""
import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def read(p: Path) -> str:
    return p.read_text(encoding="utf-8", errors="replace")


def strip_comment(line: str) -> str:
    return re.sub(r";.*$", "", line)


def pascal(name: str) -> str:
    if name != name.upper():
        return name
    parts = name.split("_")
    out = ""
    for p in parts:
        if not p:
            continue
        if re.fullmatch(r"[0-9A-F]{1,4}", p):
            out += p.upper()
        else:
            out += p[0].upper() + p[1:].lower()
    return out


def decode(s: str) -> str:
    return s.replace("#MON", "POKeMON").replace("$c4", "")


def extract_text_lines(asm: str, label: str) -> list[str]:
    """Extract a text_far/text_asm block as logical lines."""
    m = re.search(rf"^{re.escape(label)}::\s*$([\s\S]*?)(?=^\w+::|\Z)", asm, re.M)
    if not m:
        return []
    body = m.group(1)
    lines = []
    for raw in body.splitlines():
        s = strip_comment(raw).strip()
        if s.startswith(("text_end", "done", "prompt", "waitbutton")):
            break
        mm = re.match(r'^(?:text|line|cont|para)\s+"((?:[^"\\]|\\.)*)"', s)
        if mm:
            lines.append(decode(mm.group(1)))
    return lines


def resolve_far(asm: str, label: str) -> str:
    m = re.search(rf"^{re.escape(label)}::\s*$([\s\S]*?)(?=^\w+::|\Z)", asm, re.M)
    if not m:
        return ""
    mm = re.search(r"text_far\s+(_?\w+)", m.group(1))
    return mm.group(1) if mm else ""


def text_for_label(ref: Path, map_name: str, label: str) -> str:
    """Follow label → text_far → body, across the map text file and globals."""
    candidates = []
    p = ref / "text" / f"{map_name}.asm"
    if p.exists():
        candidates.append(read(p))
    for tp in sorted((ref / "data" / "text").glob("text_*.asm")):
        candidates.append(read(tp))
    for asm in candidates:
        lines = extract_text_lines(asm, label)
        if lines:
            return "\n".join(lines)
        target = resolve_far(asm, label)
        if target:
            for asm2 in candidates:
                lines = extract_text_lines(asm2, target)
                if lines:
                    return "\n".join(lines)
    return ""


def parse_trainer_lines(ref: Path, map_const: str) -> list[str]:
    """[(end_label, after_label)] in def_trainers order."""
    p = ref / "scripts" / f"{pascal(map_const)}.asm"
    if not p.exists():
        return []
    out, in_block = [], False
    for raw in read(p).splitlines():
        line = strip_comment(raw).strip()
        if re.match(r"^def_trainers\b", line):
            in_block = True
            continue
        if not in_block:
            continue
        if re.match(r"^db\s+-1", line):
            break
        m = re.match(r"^trainer\s+(EVENT_\w+)\s*,\s*\d+\s*,\s*(.*)$", line)
        if m:
            labels = re.findall(r"\w+", m.group(2))
            # battle, end-battle, after-battle (after-battle is optional)
            if len(labels) >= 2:
                out.append((labels[1], labels[2] if len(labels) > 2 else ""))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref", required=True)
    ap.add_argument("--apply", action="store_true")
    args = ap.parse_args()
    ref = Path(args.ref)
    filled = 0
    for map_path in sorted((ROOT / "crates" / "pokered-data" / "maps").glob("*/map.json")):
        mname = map_path.parent.name
        # map const name: derive from MapId? use the map.json name directly and
        # try both the PascalCase name and its UPPER_SNAKE form.
        tlines = parse_trainer_lines(ref, mname)
        if not tlines:
            continue
        map_json = json.loads(read(map_path))
        trainers = [n for n in map_json.get("npcs", []) if n.get("isTrainer")]
        if len(trainers) != len(tlines):
            print(f"[{mname}] trainer count mismatch npcs={len(trainers)} ref={len(tlines)} — skipped")
            continue
        changed = 0
        for npc, (end_label, _after) in zip(trainers, tlines):
            if npc.get("endBattleText"):
                continue
            text = text_for_label(ref, mname, end_label)
            if not text:
                # maybe the label is already the far target
                text = text_for_label(ref, mname, end_label)
            if not text:
                print(f"[{mname}] npc#{npc.get('textId')} end text NOT FOUND for {end_label}")
                continue
            npc["endBattleText"] = text
            changed += 1
        if changed:
            filled += changed
            print(f"[{mname}] +{changed} end-battle lines")
            if args.apply:
                with open(map_path, "w", encoding="utf-8") as f:
                    json.dump(map_json, f, indent=2, ensure_ascii=False)
    print(f"== {filled} end-battle lines filled")
    if not args.apply:
        print("(dry run — pass --apply to write)")


if __name__ == "__main__":
    main()

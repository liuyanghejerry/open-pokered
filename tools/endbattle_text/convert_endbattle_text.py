#!/usr/bin/env python3
"""Convert per-trainer EndBattleText from the pret/pokered baseline disassembly
into each map.json trainer NPC's `endBattleText` field.

Resolution: scripts/<Map>.asm `trainer` macro (4th arg = TextEndBattle label)
-> `<label>: text_far _X` -> `_X::` text block in text/<Map>*.asm. The Nth
`trainer` header (in file order) maps to the Nth isTrainer NPC in map.json
(ordinal join; verified against VermilionGym).
"""
import json, os, re, sys, glob

BASE = "/Users/liuyanghe02/develop/pokered-worktree"
MAPS = "/Users/liuyanghe02/develop/pokered/crates/pokered-data/maps"
WRITE = "--write" in sys.argv

# ── 1. Build a global text-label -> string table from all text/*.asm ────────
TEXT_LINE = re.compile(r'^\s*(text|line|cont|para|next|page)\s+"(.*)"\s*$')
LABEL = re.compile(r'^(_\w+)::')
END = re.compile(r'^\s*(done|prompt|text_end|text_waitbutton)\b')

def build_text_table():
    table = {}
    for path in glob.glob(f"{BASE}/text/**/*.asm", recursive=True) + glob.glob(f"{BASE}/text/*.asm"):
        with open(path, encoding="utf-8") as f:
            lines = f.readlines()
        i = 0
        while i < len(lines):
            m = LABEL.match(lines[i])
            if not m:
                i += 1
                continue
            label = m.group(1)
            i += 1
            parts = []
            while i < len(lines):
                ln = lines[i]
                if LABEL.match(ln) or END.match(ln):
                    if END.match(ln):
                        i += 1
                    break
                tm = TEXT_LINE.match(ln)
                if tm:
                    kind, txt = tm.group(1), tm.group(2)
                    if kind == "text":
                        parts.append(("text", txt))
                    elif kind in ("line", "cont", "next"):
                        parts.append(("line", txt))
                    elif kind in ("para", "page"):
                        parts.append(("para", txt))
                i += 1
            table[label] = assemble(parts)
    return table

def assemble(parts):
    out = ""
    for j, (kind, txt) in enumerate(parts):
        if j == 0:
            out = txt
        elif kind == "para":
            out += "\n\n" + txt
        else:
            out += "\n" + txt
    return detoken(out)

def detoken(s):
    # Baseline control tokens -> the display forms used elsewhere in game data.
    s = s.replace("#MON", "POKeMON").replace("#mon", "POKeMON")
    s = s.replace("#", "POKe")
    s = s.replace("¥", "¥")
    return s

# ── 2. Per-map: trainer macro order -> EndBattleText label -> _X ────────────
TRAINER = re.compile(r'^\s*trainer\s+(.+)$')
TEXTFAR = re.compile(r'text_far\s+(_\w+)')

def script_labels(map_name):
    """Return ordered list of resolved `_X` text labels for the map's trainers."""
    path = f"{BASE}/scripts/{map_name}.asm"
    if not os.path.exists(path):
        return None
    with open(path, encoding="utf-8") as f:
        src = f.read()
    lines = src.splitlines()
    # Map wrapper-label -> text_far target (scan `<Label>:` then following text_far).
    wrapper = {}
    for idx, ln in enumerate(lines):
        m = re.match(r'^(\w+):\s*$', ln)
        if m:
            # look at the next few non-empty lines for a text_far
            for j in range(idx + 1, min(idx + 3, len(lines))):
                tf = TEXTFAR.search(lines[j])
                if tf:
                    wrapper[m.group(1)] = tf.group(1)
                    break
    # Collect trainer macro 4th args in file order.
    result = []
    for ln in lines:
        m = TRAINER.match(ln)
        if not m:
            continue
        args = [a.strip() for a in m.group(1).split(",")]
        if len(args) < 4:
            result.append(None)
            continue
        end_label = args[3]
        target = wrapper.get(end_label) or ("_" + end_label)
        result.append(target)
    return result

# ── 3. Join + write ─────────────────────────────────────────────────────────
def main():
    table = build_text_table()
    total_filled = 0
    problems = []
    for mj in sorted(glob.glob(f"{MAPS}/*/map.json")):
        map_name = os.path.basename(os.path.dirname(mj))
        with open(mj, encoding="utf-8") as f:
            raw = f.read()
        data = json.loads(raw)
        trailing_nl = raw.endswith("\n")
        trainer_npcs = [n for n in data.get("npcs", []) if n.get("isTrainer")]
        if not trainer_npcs:
            continue
        labels = script_labels(map_name)
        if labels is None:
            problems.append(f"{map_name}: no baseline scripts/{map_name}.asm")
            continue
        if len(labels) != len(trainer_npcs):
            problems.append(
                f"{map_name}: {len(labels)} trainer headers vs {len(trainer_npcs)} isTrainer NPCs")
        filled = 0
        for npc, label in zip(trainer_npcs, labels):
            if label and label in table:
                npc["endBattleText"] = table[label]
                filled += 1
            else:
                problems.append(f"{map_name}: unresolved label {label}")
        total_filled += filled
        if WRITE:
            with open(mj, "w", encoding="utf-8") as f:
                f.write(json.dumps(data, indent=2, ensure_ascii=False))
                if trailing_nl:
                    f.write("\n")
    print(f"Filled {total_filled} / 308 trainer endBattleText fields "
          f"({'WROTE' if WRITE else 'DRY-RUN'})")
    if problems:
        print(f"\n{len(problems)} problems:")
        for p in problems[:40]:
            print("  " + p)
    # Verify VermilionGym known values.
    vg = json.load(open(f"{MAPS}/VermilionGym/map.json"))
    tr = [n for n in vg["npcs"] if n.get("isTrainer")]
    if not WRITE:
        labels = script_labels("VermilionGym")
        print("\nVermilionGym check:")
        for npc, lab in zip(tr, labels):
            print(f"  {npc.get('trainerClass')}: {table.get(lab)!r}")

if __name__ == "__main__":
    main()

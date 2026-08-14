#!/usr/bin/env python3
"""Fill missing hidden-event triggers into open-pokered map data.

Reads the reference pret/pokered hidden_events.asm + per-map/text asm files and
adds the missing interactables to the remake maps as signs (bg_event semantics:
triggered by facing the tile), plus scene wiring for gym statues / PCs / slot
machines. Chinese translations are left empty — the English source is canonical.

Usage:
    python3 tools/fill_hidden_events.py --ref /Users/liuyanghejerry/develop/pokered [--apply] [--dry-run]
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


# --------------------------------------------------------------------------
# Reference hidden events
# --------------------------------------------------------------------------

def parse_hidden_events(ref: Path) -> dict:
    text = read(ref / "data" / "events" / "hidden_events.asm")
    out, cur = {}, None
    for raw in text.splitlines():
        line = strip_comment(raw).strip()
        m = re.match(r"^hidden_events_for\s+(\w+)", line)
        if m:
            cur = m.group(1)
            out.setdefault(cur, [])
            continue
        if re.match(r"^db\s+-1", line):
            cur = None
            continue
        m = re.match(
            r"^hidden_(?:event|text_predef|coins_event|item)\s+(\d+),\s*(\d+),\s*(\w+)(?:\s*,\s*(.*))?$",
            line)
        if m and cur:
            out[cur].append((int(m.group(1)), int(m.group(2)), m.group(3),
                             (m.group(4) or "").strip()))
    return out


# --------------------------------------------------------------------------
# Reference text extraction
# --------------------------------------------------------------------------

def extract_text_blocks(asm_text: str, label: str) -> list[dict]:
    """Extract a text_far/text_asm block's pages as [{'line1','line2'}]."""
    m = re.search(rf"^{re.escape(label)}::\s*$([\s\S]*?)(?=^\w+::|\Z)", asm_text, re.M)
    if not m:
        return []
    body = m.group(1)
    pages, cur_line = [], None
    lines = []

    def flush():
        nonlocal cur_line, lines
        if cur_line is not None:
            while len(lines) < 2:
                lines.append("")
            pages.append({"line1": lines[0], "line2": lines[1]})
        lines = []
        cur_line = None

    for raw in body.splitlines():
        s = strip_comment(raw).strip()
        if not s:
            continue
        if s.startswith("text_end") or s.startswith("done") or s.startswith("prompt") \
                or s.startswith("waitbutton"):
            flush()
            break
        m = re.match(r'^text\s+"((?:[^"\\]|\\.)*)"', s)
        if m:
            if cur_line is not None:
                flush()
            cur_line = decode(m.group(1))
            continue
        m = re.match(r'^line\s+"((?:[^"\\]|\\.)*)"', s)
        if m:
            flush() if cur_line is None else None
            lines.append(cur_line) if cur_line is not None else None
            cur_line = decode(m.group(1))
            continue
        m = re.match(r'^cont\s+"((?:[^"\\]|\\.)*)"', s)
        if m:
            lines.append(cur_line) if cur_line is not None else None
            cur_line = decode(m.group(1))
            continue
        m = re.match(r'^para\s+"((?:[^"\\]|\\.)*)"', s)
        if m:
            lines.append(cur_line) if cur_line is not None else None
            flush()
            cur_line = decode(m.group(1))
            continue
        m = re.match(r'^text_start\s*$', s)
        if m:
            continue
        m = re.match(r'^text_far\s+(\w+)', s)
        if m:
            continue  # resolved by the caller
        # text_ram / other directives — stop
        if s.startswith("tx_") or s.startswith("text_") or s.startswith("sound_"):
            break
    flush()
    return pages


def decode(s: str) -> str:
    # GB control tokens: #MON, <PLAYER>, <RIVAL>, POKé, $E1 etc. — keep as-is
    # except the tokens the remake spells out.
    out = s.replace("#MON", "POKeMON").replace("#mon", "POKeMON")
    out = out.replace("$c4", "").replace("$ef", "♂").replace("$f5", "♀")
    return out


def find_far_target(asm_text: str, pointer_label: str) -> str:
    """Follow a `Pointer:: text_far _Target` chain to the target label."""
    m = re.search(rf"^{re.escape(pointer_label)}::\s*$([\s\S]*?)(?=^\w+::|\Z)", asm_text, re.M)
    if not m:
        return ""
    mm = re.search(r"text_far\s+(_?\w+)", m.group(1))
    return mm.group(1) if mm else ""


def pages_for_ref_text(ref: Path, map_name: str, label: str) -> list[dict]:
    """Resolve label → its text pages, following text_far pointers across files."""
    # look in text/{Map}.asm first, then data/text/text_N.asm globals
    candidates = []
    p = ref / "text" / f"{map_name}.asm"
    if p.exists():
        candidates.append(read(p))
    for tp in sorted((ref / "data" / "text").glob("text_*.asm")):
        candidates.append(read(tp))
    candidates.append(read(ref / "text.asm"))
    for asm in candidates:
        pages = extract_text_blocks(asm, label)
        if pages:
            return pages
        target = find_far_target(asm, label)
        if target:
            for asm2 in candidates:
                pages = extract_text_blocks(asm2, target)
                if pages:
                    return pages
    return []


# --------------------------------------------------------------------------
# Global text content (hardcoded from data/text/*.asm — extracted above)
# --------------------------------------------------------------------------

GLOBAL_TEXTS = {
    "PrintBookcaseText": "Crammed full of\nPOKeMON books!",
    "PrintMagazinesText": "POKeMON magazines!\n\nPOKeMON notebooks!\n\nPOKeMON graphs!",
    "PrintFightingDojoText": "FIGHTING DOJO",
    "PrintIndigoPlateauHQText": "INDIGO PLATEAU\nPOKeMON LEAGUE HQ",
    "AerodactylFossil": "AERODACTYL Fossil\nA primitive and\nrare POKeMON.",
    "KabutopsFossil": "KABUTOPS Fossil\nA primitive and\nrare POKeMON.",
    "PrintRedSNESText": "<PLAYER> is\nplaying the SNES!\n...Okay!\nIt's time to go!",
    "Route15GateLeftBinoculars": "Looked into the\nbinoculars...\n\nA large, shining\nbird is flying\ntoward the sea.",
    "PrintNewBikeText": "A shiny new\nBICYCLE!",
    "SLOTS_OUTOFORDER": "OUT OF ORDER\nThis is broken.",
    "SLOTS_OUTTOLUNCH": "OUT TO LUNCH\nThis is reserved.",
    "SLOTS_SOMEONESKEYS": "Someone's keys!\nThey'll be back.",
    "PrintTrashText": "Nope, there's\nonly trash here.",
    "DisplayOakLabLeftPoster": "Push START to\nopen the MENU!",
    "DisplayOakLabRightPoster": "The SAVE option is\non the MENU screen.",
    "DisplayOakLabEmailText": ("There's an e-mail message here!\n\n...\n\nCalling all POKeMON trainers!\n"
        "The elite trainers of POKeMON LEAGUE are ready to take on all comers!\n\n"
        "Bring your best POKeMON and see how you rate as a trainer!\n\n"
        "POKeMON LEAGUE HQ INDIGO PLATEAU\n\n"
        "PS: PROF.OAK, please visit us! ..."),
    "PrintNotebookText": ("It's a pamphlet on TMs.\n\n...\n\nThere are 50 TMs in all.\n\n"
        "There are also 5 HMs that can be used repeatedly.\n\nSILPH CO."),
}


def pages_from_flat(text: str) -> list[dict]:
    """Flat text with \n separators (double \n = page break) → page list."""
    pages = []
    for chunk in text.split("\n\n"):
        lines = chunk.split("\n")
        while lines:
            pair = lines[:2]
            while len(pair) < 2:
                pair.append("")
            pages.append({"line1": pair[0], "line2": pair[1]})
            lines = lines[2:]
    return pages


# _ViridianSchoolNotebookText1-5 (data/text/text_2.asm:497-560) — the school
# notebook flips through these pages (TurnPageSchoolNotebook).
SCHOOL_NOTEBOOK_PAGES = pages_from_flat(
    "Looked at the notebook!\n\n"
    "First page...\n\n"
    "# BALLs are used to catch POKeMON.\n"
    "Up to 6 POKeMON can be carried.\n"
    "People who raise and make POKeMON fight are called POKeMON trainers.\n\n"
    "Second page...\n\n"
    "A healthy POKeMON may be hard to catch, so weaken it first!\n"
    "Poison, burns and other damage are effective!\n\n"
    "Third page...\n\n"
    "POKeMON trainers seek others to engage in POKeMON fights.\n"
    "Battles are constantly fought at POKeMON GYMs.\n\n"
    "Fourth page...\n\n"
    "The goal for POKeMON trainers is to beat the top 8 POKeMON GYM LEADERs.\n"
    "Do so to earn the right to face...\n\n"
    "The ELITE FOUR of POKeMON LEAGUE!\n\n"
    "GIRL: Hey! Don't look at my notes!"
)


# --------------------------------------------------------------------------
# Per-map plans
# --------------------------------------------------------------------------

GYM_STATUE_INFO = {
    "VIRIDIAN_GYM": ("VIRIDIAN CITY", "GIOVANNI", "EVENT_BEAT_VIRIDIAN_GYM_GIOVANNI"),
    "PEWTER_GYM": ("PEWTER CITY", "BROCK", "EVENT_BEAT_BROCK"),
    "CERULEAN_GYM": ("CERULEAN CITY", "MISTY", "EVENT_BEAT_MISTY"),
    "VERMILION_GYM": ("VERMILION CITY", "LT.SURGE", "EVENT_BEAT_LT_SURGE"),
    "CELADON_GYM": ("CELADON CITY", "ERIKA", "EVENT_BEAT_ERIKA"),
    "FUCHSIA_GYM": ("FUCHSIA CITY", "KOGA", "EVENT_BEAT_KOGA"),
    "SAFFRON_GYM": ("SAFFRON CITY", "SABRINA", "EVENT_BEAT_SABRINA"),
    "CINNABAR_GYM": ("CINNABAR ISLAND", "BLAINE", "EVENT_BEAT_BLAINE"),
}

PC_ROUTINE_MAPS = {
    "OpenPokemonCenterPC", "OpenRedsPC", "BillsHousePC",
}

TEXT_ONLY_ROUTINES = {
    "PrintBookcaseText", "PrintMagazinesText", "PrintFightingDojoText",
    "PrintIndigoPlateauHQText", "AerodactylFossil", "KabutopsFossil",
    "PrintRedSNESText", "Route15GateLeftBinoculars", "PrintNewBikeText",
    "PrintTrashText", "PrintBenchGuyText",
    "DisplayOakLabLeftPoster", "DisplayOakLabRightPoster", "DisplayOakLabEmailText",
    "PrintNotebookText", "PrintBlackboardLinkCableText",
}


def build_plans(ref: Path) -> dict:
    """{map_const: [(x, y, kind, data)]} for every missing hidden event."""
    hidden = parse_hidden_events(ref)
    plans = {}
    for map_const, events in hidden.items():
        for x, y, routine, arg in events:
            if routine in ("HiddenItems", "HiddenCoins", "CardKeyDoor"):
                continue
            kind = None
            data = None
            if routine == "GymStatues":
                info = GYM_STATUE_INFO.get(map_const)
                if info:
                    kind, data = "gym_statue", info
            elif routine in PC_ROUTINE_MAPS:
                kind, data = "pc", None
            elif routine == "StartSlotMachine":
                if arg.startswith("SLOTS_"):
                    kind, data = "slot_special", GLOBAL_TEXTS.get(arg, arg)
                else:
                    kind, data = "slot_machine", None
            elif routine in TEXT_ONLY_ROUTINES:
                if routine == "PrintBenchGuyText":
                    kind, data = "bench_text", None
                elif routine == "PrintNotebookText":
                    kind, data = "notebook", arg
                elif routine == "PrintBlackboardLinkCableText":
                    kind, data = "defer_blackboard", None
                elif routine in GLOBAL_TEXTS:
                    kind, data = "text", GLOBAL_TEXTS[routine]
                else:
                    kind, data = "text", None  # per-map extraction
            elif routine in ("PrintCinnabarQuiz", "PrintSafariZoneStuff", "OpenElevator"):
                continue  # already adapted / documented
            else:
                continue  # unhandled (scripted) — reported separately
            plans.setdefault(map_const, []).append((x, y, kind, data))
    return plans


def bench_text_pages(ref: Path, map_const: str) -> list[dict]:
    # data/events/bench_guys.asm: map id → predef text label. The label's
    # `text_far` target (in engine/events/hidden_events/bench_guys.asm) holds
    # the actual string in text/*.asm or data/text/text_*.asm.
    tbl = read(ref / "data" / "events" / "bench_guys.asm")
    m = re.search(rf"bench_guy_text\s+{map_const}\s*,\s*\w+,\s*(\w+)", tbl)
    if not m:
        # Maps not in BenchGuyTextPointers (e.g. the three Safari rest houses)
        # show NOTHING in the original — the routine misses the terminator and
        # returns. Faithful: no sign.
        return None
    label = m.group(1)
    if label == "SaffronCityPokecenterBenchGuyText":
        # text_asm with CheckEvent EVENT_BEAT_SILPH_CO_GIOVANNI — the scene
        # wiring swaps the two texts; the sign stores the pre-Giovanni one.
        return pages_from_flat(
            "It would be great if the ELITE FOUR came and stomped TEAM ROCKET!")
    bench_asm = read(ref / "engine" / "events" / "hidden_events" / "bench_guys.asm")
    m = re.search(rf"^{label}::\s*$\s*text_far\s+(\w+)", bench_asm, re.M)
    if not m:
        return []
    target = m.group(1)
    cands = [read(p) for p in sorted((ref / "text").glob("*.asm"))]
    cands += [read(p) for p in sorted((ref / "data" / "text").glob("text_*.asm"))]
    for asm in cands:
        pages = extract_text_blocks(asm, target)
        if pages:
            return pages
    return []




def revert(plans: dict):
    total = 0
    for map_const, items in sorted(plans.items()):
        mname = pascal(map_const)
        map_path = ROOT / "crates" / "pokered-data" / "maps" / mname / "map.json"
        if not map_path.exists():
            continue
        map_json = json.loads(read(map_path))
        targets = {(x, y) for x, y, k, d in items}
        before = len(map_json.get("signs", []))
        map_json["signs"] = [s for s in map_json.get("signs", [])
                             if (s["x"], s["y"]) not in targets]
        removed = before - len(map_json["signs"])
        # drop orphaned text.sign keys
        used = {str(s["textId"]) for s in map_json["signs"]}
        sign_text = map_json.get("text", {}).get("sign", {})
        orphans = [k for k in sign_text if k not in used]
        for k in orphans:
            del sign_text[k]
        if removed:
            total += removed
            print(f"[{mname}] reverted {removed} signs")
            with open(map_path, "w", encoding="utf-8") as f:
                json.dump(map_json, f, indent=2, ensure_ascii=False)
    print(f"== reverted {total} signs")

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref", required=True)
    ap.add_argument("--apply", action="store_true")
    ap.add_argument("--revert", action="store_true",
                    help="remove the signs previously added by --apply")
    args = ap.parse_args()
    ref = Path(args.ref)
    plans = build_plans(ref)
    if args.revert:
        revert(plans)
        return
    total_added = 0
    report = []
    for map_const, items in sorted(plans.items()):
        mname = pascal(map_const)
        map_path = ROOT / "crates" / "pokered-data" / "maps" / mname / "map.json"
        if not map_path.exists():
            report.append(f"[{mname}] MISSING map.json — skipped {len(items)} triggers")
            continue
        map_json = json.loads(read(map_path))
        existing = {(s["x"], s["y"]) for s in map_json.get("signs", [])}
        sign_text = map_json.setdefault("text", {}).setdefault("sign", {})
        used_ids = [s.get("textId", 0) for s in map_json.get("signs", [])]
        next_id = max(used_ids or [0]) + 1
        added = []
        for x, y, kind, data in items:
            if (x, y) in existing:
                continue
            pages = []
            scene_note = None
            if kind == "text":
                if data is None:
                    report.append(f"[{mname}] ({x},{y}) text: no content")
                    continue
                pages = pages_from_flat(data)
            elif kind == "bench_text":
                pages = bench_text_pages(ref, map_const)
                if pages is None:
                    report.append(f"[{mname}] ({x},{y}) bench: no BenchGuyTextPointers entry (faithful no-op)")
                    continue
            elif kind == "notebook":
                # ViridianSchoolHouse flips through the 5 school pages; the
                # CeladonMansionRoofHouse one is the TM pamphlet.
                if data == "ViridianSchoolNotebook":
                    pages = SCHOOL_NOTEBOOK_PAGES
                else:
                    pages = pages_from_flat(GLOBAL_TEXTS["PrintNotebookText"])
            elif kind == "gym_statue":
                # scene-wired; sign gets a placeholder text id
                pages = [{"line1": "", "line2": ""}]
                scene_note = "gym_statue"
            elif kind == "pc":
                pages = [{"line1": "", "line2": ""}]
                scene_note = "pc"
            elif kind == "slot_special":
                pages = pages_from_flat(data)
            elif kind == "slot_machine":
                pages = [{"line1": "", "line2": ""}]
                scene_note = "slot_machine"
            else:
                continue
            if not pages:
                report.append(f"[{mname}] ({x},{y}) {kind}: NO TEXT extracted")
                continue
            map_json["signs"].append({"x": x, "y": y, "textId": next_id})
            sign_text[str(next_id)] = pages
            added.append((x, y, kind, scene_note))
            next_id += 1
        if added:
            total_added += len(added)
            report.append(f"[{mname}] +{len(added)} signs: "
                          + ", ".join(f"({x},{y}) {k}" for x, y, k, _ in added))
            if args.apply:
                with open(map_path, "w", encoding="utf-8") as f:
                    json.dump(map_json, f, indent=2, ensure_ascii=False)
    print(f"== {total_added} signs planned")
    for r in report:
        print(r)
    if not args.apply:
        print("(dry run — pass --apply to write)")


if __name__ == "__main__":
    main()

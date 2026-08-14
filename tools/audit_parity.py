#!/usr/bin/env python3
"""Parity audit tool: compare open-pokered content data vs the pret/pokered reference.

Usage:
    python3 tools/audit_parity.py --ref /Users/liuyanghejerry/develop/pokered \
        [--domain maps|rods|trainers|shops] [--only-map PalletTown] [--json out.json]

Domains:
  maps     - per-map header (tileset/music/connections/dimensions/border),
             warps, signs, NPCs (sprite/pos/facing/movement/text), trainer events,
             grass/water wild encounters (both versions)
  rods     - good rod / super rod tables
  trainers - trainer class parties, names, base money, AI move-choice lists
  shops    - mart inventories
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
    """UPPER_SNAKE -> PascalCase (remake convention).

    Already-mixed-case symbols (e.g. header targets like 'ViridianCity') pass
    through unchanged. All-hex parts (B1F, 0B, 6F) keep their case.
    """
    if name != name.upper():
        return name
    parts = name.split("_")
    out = ""
    for i, p in enumerate(parts):
        if not p:
            continue
        if i == 0 and p == "SS":
            out += "SS"
        elif re.fullmatch(r"[0-9A-F]{1,4}", p):  # hex-ish suffix like 0B / B1F
            out += p.upper()
        else:
            out += p[0].upper() + p[1:].lower()
    return out


# --------------------------------------------------------------------------
# Reference (pret/pokered) symbol tables
# --------------------------------------------------------------------------

def load_constants(ref: Path, filename: str) -> dict:
    """{SYMBOL: index} from a constants/*.asm `const` sequence."""
    text = read(ref / "constants" / filename)
    out = {}
    idx = 0
    for line in text.splitlines():
        s = strip_comment(line).strip()
        m = re.match(r"^const\s+(\w+)$", s)
        if m:
            out[m.group(1)] = idx
            idx += 1
            continue
        if s.startswith("const_def"):
            idx = 0
    return out


def load_maps(ref: Path):
    """[(const_name, width, height)] in map-id order."""
    text = read(ref / "constants" / "map_constants.asm")
    out = []
    for line in text.splitlines():
        m = re.match(r"^map_const\s+(\w+),\s*(\d+),\s*(\d+)", strip_comment(line).strip())
        if m:
            out.append((m.group(1), int(m.group(2)), int(m.group(3))))
    return out


def load_music(ref: Path) -> dict:
    return load_constants(ref, "music_constants.asm")


def load_sprites(ref: Path) -> dict:
    return load_constants(ref, "sprite_constants.asm")


def load_tilesets(ref: Path) -> dict:
    return load_constants(ref, "tileset_constants.asm")


def load_species(ref: Path) -> dict:
    return load_constants(ref, "pokemon_constants.asm")


def load_items(ref: Path) -> dict:
    return load_constants(ref, "item_constants.asm")


def load_trainers(ref: Path) -> dict:
    return load_constants(ref, "trainer_constants.asm")


def load_moves(ref: Path) -> dict:
    return load_constants(ref, "move_constants.asm")


def load_events(ref: Path) -> dict:
    return load_constants(ref, "event_constants.asm")


# --------------------------------------------------------------------------
# Reference object events parser (data/maps/objects/{Map}.asm)
# --------------------------------------------------------------------------

FACING_MAP = {
    "SPRITE_FACING_DOWN": "Down", "SPRITE_FACING_UP": "Up",
    "SPRITE_FACING_LEFT": "Left", "SPRITE_FACING_RIGHT": "Right",
}

# Movement-pattern arg -> (remake movement, remake facing, remake range)
# Same mapping the map generator used (scripts/parse_npcs.py DIRECTION_MAP).
# Numbered 1xN_STEP_* patterns fall through to the (Wander, Down, 0) default,
# which the generator also used; those are flagged separately as pattern loss.
DIRECTION_MAP = {
    "ANY_DIR": ("Wander", "Down", 0),
    "UP_DOWN": ("Wander", "Down", 1),
    "LEFT_RIGHT": ("Wander", "Left", 2),
    "DOWN": ("Stationary", "Down", 0),
    "UP": ("Stationary", "Up", 0),
    "LEFT": ("Stationary", "Left", 0),
    "RIGHT": ("Stationary", "Right", 0),
    "NONE": ("Stationary", "Down", 0),
}


def parse_objects(ref: Path, map_const: str) -> dict | None:
    """Parse objects asm -> dict(warps, signs, npcs)."""
    path = ref / "data" / "maps" / "objects" / f"{pascal(map_const)}.asm"
    if not path.exists():
        return None
    text = read(path)
    warps, signs, npcs = [], [], []
    section = None
    n_objects = 0
    for raw in text.splitlines():
        line = strip_comment(raw).strip()
        m = re.match(r"^def_warp_events\b", line)
        if m:
            section = "warps"; continue
        m = re.match(r"^def_bg_events\b", line)
        if m:
            section = "signs"; continue
        m = re.match(r"^def_object_events\b", line)
        if m:
            section = "npcs"; continue
        m = re.match(r"^def_warps_to\b", line)
        if m:
            section = None; continue
        if not line or line.startswith("db ") and "border block" not in line:
            continue
        if section == "warps":
            m = re.match(r"^warp_event\s+(\d+),\s*(\d+),\s*(\w+),\s*(\d+)", line)
            if m:
                warps.append({"x": int(m.group(1)), "y": int(m.group(2)),
                              "dest": m.group(3), "dest_warp_id": int(m.group(4))})
        elif section == "signs":
            m = re.match(r"^bg_event\s+(\d+),\s*(\d+),\s*(?:\d+,\s*)?(\w+)", line)
            if m:
                signs.append({"x": int(m.group(1)), "y": int(m.group(2)),
                              "text": m.group(3)})
        elif section == "npcs":
            m = re.match(r"^object_event\s+(\d+),\s*(\d+),\s*(\w+),\s*(\w+),\s*(\S+),\s*(\w+)", line)
            if m:
                n_objects += 1
                npcs.append({
                    "x": int(m.group(1)), "y": int(m.group(2)),
                    "sprite": m.group(3),
                    "movement": m.group(4),
                    "range": m.group(5),
                    "text": m.group(6),
                })
    return {"warps": warps, "signs": signs, "npcs": npcs, "n_objects": n_objects}


def parse_trainer_events(ref: Path, map_const: str) -> list:
    """[(event_flag, sight_range, [text ids])] from scripts/{Map}.asm def_trainers."""
    path = ref / "scripts" / f"{pascal(map_const)}.asm"
    if not path.exists():
        return []
    text = read(path)
    out, in_block = [], False
    for raw in text.splitlines():
        line = strip_comment(raw).strip()
        if re.match(r"^def_trainers\b", line):
            in_block = True
            continue
        if not in_block:
            continue
        if re.match(r"^db\s+-1", line):
            in_block = False
            continue
        m = re.match(r"^trainer\s+(EVENT_\w+)\s*,\s*(\d+)\s*,\s*0?\s*,?\s*(.*)$", line)
        if m:
            texts = re.findall(r"\w+", m.group(3))
            out.append((m.group(1), int(m.group(2)), texts))
        elif line and not line.startswith(";") and not line.endswith(":"):
            in_block = False  # left the def_trainers block
    return out


# --------------------------------------------------------------------------
# Reference header parser (data/maps/headers/{Map}.asm + songs.asm + map_constants)
# --------------------------------------------------------------------------

def parse_header(ref: Path, map_const: str) -> dict | None:
    path = ref / "data" / "maps" / "headers" / f"{pascal(map_const)}.asm"
    if not path.exists():
        return None
    text = read(path)
    hdr = {"tileset": None, "connections": []}
    for raw in text.splitlines():
        line = strip_comment(raw).strip()
        m = re.match(r"^map_header\s+\w+,\s*\w+,\s*(\w+)(?:,\s*(.*))?$", line)
        if m:
            hdr["tileset"] = m.group(1)
        m = re.match(r"^connection\s+(north|south|west|east),\s*(\w+),\s*\w+,\s*(-?\d+)", line)
        if m:
            hdr["connections"].append((m.group(1), m.group(2), int(m.group(3))))
    return hdr


def load_map_songs(ref: Path) -> dict:
    """{map_const: MUSIC_x} in map-id order."""
    text = read(ref / "data" / "maps" / "songs.asm")
    out = {}
    idx = 0
    for raw in text.splitlines():
        line = strip_comment(raw).strip()
        m = re.match(r"^db\s+(MUSIC_\w+)", line)
        if m:
            out[idx] = m.group(1)
            idx += 1
    return out


# --------------------------------------------------------------------------
# Reference wild encounter parser
# --------------------------------------------------------------------------

def parse_wild_file(ref: Path, label: str) -> dict | None:
    """Find data/wild/maps/*.asm defining '{label}:', parse red/blue grass/water."""
    for path in (ref / "data" / "wild" / "maps").glob("*.asm"):
        text = read(path)
        if re.search(rf"^{re.escape(label)}:\s*$", text, re.M):
            return parse_wild_text(text)
    return None


def parse_wild_text(text: str) -> dict:
    """Return {'red': {'grass': (rate, slots), 'water': ...}, 'blue': ...}"""
    version = "both"
    cur = None
    out = {"red": {"grass": (0, []), "water": (0, [])},
           "blue": {"grass": (0, []), "water": (0, [])}}
    for raw in text.splitlines():
        line = strip_comment(raw).strip()
        if re.match(r"^IF DEF\(_RED\)", line):
            version = "red"; continue
        if re.match(r"^IF DEF\(_BLUE\)", line):
            version = "blue"; continue
        if re.match(r"^ENDC", line):
            version = "both"; continue
        if re.match(r"^IF ", line):  # e.g. IF DEF(_DEBUG) — not in release ROMs
            version = "skip"; continue
        if version == "skip":
            continue
        m = re.match(r"^def_grass_wildmons\s+(\d+)", line)
        if m:
            cur = ("grass", int(m.group(1))); continue
        m = re.match(r"^def_water_wildmons\s+(\d+)", line)
        if m:
            cur = ("water", int(m.group(1))); continue
        m = re.match(r"^end_grass_wildmons", line)
        if m:
            cur = None; continue
        m = re.match(r"^end_water_wildmons", line)
        if m:
            cur = None; continue
        m = re.match(r"^db\s+(\d+),\s*(\w+)", line)
        if m and cur:
            level, species = int(m.group(1)), m.group(2)
            for v in (["red", "blue"] if version == "both" else [version]):
                kind, rate = cur
                out[v][kind][1].append((level, species))
    for v in out:
        out[v]["grass"] = (out[v]["grass"][0], out[v]["grass"][1])
        out[v]["water"] = (out[v]["water"][0], out[v]["water"][1])
    return out


def load_grass_water(ref: Path) -> list:
    """[(map_const, wild_label)] in map-id order."""
    out = []
    for raw in read(ref / "data" / "wild" / "grass_water.asm").splitlines():
        line = strip_comment(raw).strip()
        m = re.match(r"^dw\s+(\w+)\s*;\s*(\w+)", line)
        if m:
            out.append((m.group(2), m.group(1)))
    return out


def parse_rods(ref: Path) -> dict:
    """good rod + super rod tables from the reference."""
    def _lines(p: Path):
        return [strip_comment(l).strip() for l in read(p).splitlines()]

    good = [(int(m.group(1)), m.group(2)) for m in
            (re.match(r"^db\s+(\d+),\s*(\w+)", l) for l in _lines(ref / "data" / "wild" / "good_rod.asm")) if m]
    lines = _lines(ref / "data" / "wild" / "super_rod.asm")
    entries, groups, cur = [], [], None
    for line in lines:
        m = re.match(r"^dbw\s+(\w+),\s*\.?(\w+)", line)
        if m:
            entries.append((m.group(1), m.group(2)))
            continue
        m = re.match(r"^\.Group(\d+):$", line)
        if m:
            cur = []
            groups.append(cur)
            continue
        m = re.match(r"^db\s+(\d+),\s*(\w+)", line)
        if m and cur is not None:
            cur.append((int(m.group(1)), m.group(2)))
    return {"good": good, "super_entries": entries, "super_groups": groups}


def remake_rods() -> dict:
    """Parse good/super rod data from src/wild_data.rs."""
    text = read(ROOT / "crates" / "pokered-data" / "src" / "wild_data.rs")
    # good rod
    m = re.search(r"pub fn good_rod_data\(\)[\s\S]*?vec!\[([\s\S]*?)\]", text)
    good = [(int(l), s.removeprefix("Species::")) for l, s in
            re.findall(r"level:\s*(\d+),\s*species:\s*Species::(\w+)", m.group(1))] if m else []
    # super rod groups
    m = re.search(r"pub fn super_rod_groups\(\)[\s\S]*?vec!\[([\s\S]*?)\n\s*\]\s*\n\}", text)
    groups = []
    if m:
        for gm in re.finditer(r"FishingGroup\s*\{\s*mons:\s*vec!\[([\s\S]*?)\],\s*\}", m.group(1)):
            groups.append([(int(l), s.removeprefix("Species::")) for l, s in
                           re.findall(r"level:\s*(\d+),\s*species:\s*Species::(\w+)", gm.group(1))])
    # map entries
    m = re.search(r"pub fn super_rod_map_entries\(\)[\s\S]*?vec!\[([\s\S]*?)\n\s*\]\s*\n\}", text)
    entries = [(name, int(idx)) for name, idx in
               re.findall(r'map_name:\s*"(\w+)",\s*group_index:\s*(\d+)', m.group(1))] if m else []
    return {"good": good, "super_entries": entries, "super_groups": groups}


# --------------------------------------------------------------------------
# Reference trainer parser
# --------------------------------------------------------------------------

def parse_trainers(ref: Path) -> dict:
    """{class: {'parties': [[(level,species)...]], 'name': str, 'money': int,
                'move_choices': [int]}}"""
    def _lines(p: Path):
        return [strip_comment(l).strip() for l in read(p).splitlines()]

    parties_text = _lines(ref / "data" / "trainers" / "parties.asm")
    names_text = _lines(ref / "data" / "trainers" / "names.asm")
    money_text = _lines(ref / "data" / "trainers" / "pic_pointers_money.asm")
    mc_text = _lines(ref / "data" / "trainers" / "move_choices.asm")
    sp_text = _lines(ref / "data" / "trainers" / "special_moves.asm")

    # class order from TrainerDataPointers (group(1) already excludes "Data")
    order = [m.group(1) for m in (re.match(r"^dw\s+(\w+)Data$", l) for l in parties_text) if m]
    classes = order

    # parties: blocks by label
    blocks = {}
    cur = None
    for line in parties_text:
        m = re.match(r"^(\w+)Data:$", line)
        if m:
            cur = m.group(1)  # group already excludes the "Data" suffix
            blocks[cur] = []
            continue
        if cur is None:
            continue
        m = re.match(r"^db\s+(.+)$", line)
        if m:
            body = m.group(1).strip()
            if body.upper().startswith("$FF"):
                toks = re.split(r",\s*", body)
                party = []
                i = 1
                while i + 1 < len(toks):
                    try:
                        lvl = int(toks[i], 0)
                    except ValueError:
                        break
                    spec = toks[i + 1]
                    if spec == "0":
                        break
                    party.append((lvl, spec))
                    i += 2
                blocks[cur].append(party)
            else:
                toks = re.split(r",\s*", body)
                if toks and toks[0].isdigit():
                    lvl = int(toks[0])
                    party = [(lvl, s) for s in toks[1:] if s != "0"]
                    blocks[cur].append(party)
    names = [m.group(1) for m in (re.match(r'^li\s+"([^"]*)"', l) for l in names_text) if m]
    moneys = [m.group(1) for m in (re.match(r"^pic_money\s+\w+,\s*(\d+)", l) for l in money_text) if m]
    mcs = [[int(x) for x in re.findall(r"\d+", m.group(1))]
           for m in (re.match(r"^move_choices\s*(.*)$", l) for l in mc_text) if m]

    out = {}
    for i, cname in enumerate(classes):
        out[cname] = {
            "parties": blocks.get(cname, []),
            "name": names[i] if i < len(names) else None,
            "money": int(moneys[i]) if i < len(moneys) else None,
            "move_choices": mcs[i] if i < len(mcs) else None,
        }
    lone = [m.groups() for m in (re.match(r"^db\s+(\d+),\s*(\w+)", l) for l in sp_text) if m]
    team = [m.groups() for m in (re.match(r"^db\s+(\w+),\s*(\w+)", l) for l in sp_text) if m]
    out["_special"] = {
        "lone": [(int(a), b) for a, b in lone],
        "team": [(a, b) for a, b in team if a not in ("LONE_MOVES", "TEAM_MOVES")],
    }
    return out


# --------------------------------------------------------------------------
# Reference shop parser
# --------------------------------------------------------------------------

def parse_hidden_events(ref: Path) -> dict:
    """{map_const: [(x, y, routine, arg)]} from data/events/hidden_events.asm"""
    text = read(ref / "data" / "events" / "hidden_events.asm")
    out = {}
    cur = None
    for raw in text.splitlines():
        line = strip_comment(raw).strip()
        m = re.match(r"^hidden_events_for\s+(\w+)", line)
        if m:
            cur = m.group(1)
            out.setdefault(cur, [])
            continue
        m = re.match(r"^db\s+-1", line)
        if m:
            cur = None
            continue
        m = re.match(r"^hidden_(?:event|text_predef|coins_event|item)\s+(\d+),\s*(\d+),\s*(\w+)(?:\s*,\s*(.*))?$", line)
        if m and cur:
            out[cur].append((int(m.group(1)), int(m.group(2)), m.group(3),
                             (m.group(4) or "").strip()))
    return out


# Hidden-event routines that the remake models as signs (text/PC triggers).
SIGN_LIKE_HIDDEN_ROUTINES = {
    "OpenPokemonCenterPC", "OpenRedsPC", "BillsHousePC", "PrintTrashText",
    "GymTrashScript", "GymStatues", "PrintBookcaseText", "PrintBenchGuyText",
    "PrintNotebookText", "PrintBlackboardLinkCableText", "DisplayOakLabLeftPoster",
    "DisplayOakLabRightPoster", "DisplayOakLabEmailText", "PrintMagazinesText",
    "PrintShelfBooksText", "PrintPrePokedexText", "PrintTVText", "PrintGameText",
    "PrintPCStoryText", "PrintBinocularsText", "Route15GateLeftBinoculars",
    "PrintQuestionnaireText", "AerodactylFossil", "KabutopsFossil",
}


def parse_marts(ref: Path) -> dict:
    """{(mart_name, clerk_no): [ITEM...]} from data/items/marts.asm (clerk texts)."""
    out = {}
    cur = None
    for raw in read(ref / "data" / "items" / "marts.asm").splitlines():
        line = strip_comment(raw).strip()
        m = re.match(r"^(\w+)Clerk(\d*)Text::$", line)
        if m:
            base = m.group(1)
            no = int(m.group(2)) if m.group(2) else 1
            cur = (base, no)
            continue
        m = re.match(r"^script_mart\s+(.+)$", line)
        if m and cur:
            items = [tok for tok in re.split(r",\s*", m.group(1)) if tok]
            out[cur] = items
            cur = None
    return out


def remake_shop_inventories() -> dict:
    """{map_name: [inventory lists...]} from data/shops/*.json + scene openShop calls."""
    out = {}
    for p in sorted((ROOT / "crates" / "pokered-data" / "data" / "shops").glob("*.json")):
        rj = json.loads(read(p))
        out.setdefault(p.stem, []).append(rj.get("items", []))
    for p in sorted((ROOT / "crates" / "pokered-data" / "maps").glob("*/script.scene")):
        text = read(p)
        for m in re.finditer(r"openShop\(\[(.*?)\]\)", text, re.S):
            items = [tok.strip().strip('"') for tok in m.group(1).split(",") if tok.strip()]
            out.setdefault(p.parent.name, []).append(items)
    return out


# --------------------------------------------------------------------------
# Remake readers
# --------------------------------------------------------------------------

def remake_map_json(name: str) -> dict | None:
    p = ROOT / "crates" / "pokered-data" / "maps" / name / "map.json"
    if p.exists():
        return json.loads(read(p))
    return None


def remake_trainer_headers() -> dict:
    """{map_name_lower: [(event_flag, sight_range)]} from src/trainer_headers.rs"""
    p = ROOT / "crates" / "pokered-data" / "src" / "trainer_headers.rs"
    text = read(p)
    out = {}
    for m in re.finditer(
            r"pub static TRAINERS_(\w+):\s*\[TrainerHeaderData;\s*\d+\]\s*=\s*\[(.*?)\];",
            text, re.S):
        entries = re.findall(r"event_flag:\s*EventFlag::(\w+),\s*sight_range:\s*(\d+)",
                             m.group(2))
        out[m.group(1).lower().replace("_", "")] = [(e, int(r)) for e, r in entries]
    return out


def remake_money() -> dict:
    """{class: base money} from src/trainer_data.rs get_base_money."""
    p = ROOT / "crates" / "pokered-data" / "src" / "trainer_data.rs"
    text = read(p)
    out = {}
    for m in re.finditer(r"TrainerClass::(\w+)\s*=>\s*(\d+)", text):
        out[m.group(1)] = int(m.group(2))
    return out


def remake_move_choices() -> dict:
    """{class: [int]} — parses move_choice_layers() in trainer_ai/mod.rs."""
    p = ROOT / "crates" / "pokered-core" / "src" / "battle" / "trainer_ai" / "mod.rs"
    text = read(p)
    m = re.search(r"pub fn move_choice_layers[\s\S]*?match class \{([\s\S]*?)\n    \}", text)
    out = {}
    if not m:
        return out
    for arm in re.finditer(r"([\w\s|]+?)\s*=>\s*(?:\{\s*)?&\[([^\]]*)\](?:\s*\})?", m.group(1)):
        classes = [c for c in re.split(r"[|\s]+", arm.group(1)) if c]
        layers = [int(x) for x in re.findall(r"Layer(\d)", arm.group(2))]
        for c in classes:
            out[c] = layers
    return out


# --------------------------------------------------------------------------
# Comparators
# --------------------------------------------------------------------------

def species_name(sym: str) -> str:
    return pascal(sym)


def map_name(sym: str) -> str:
    return pascal(sym)


def music_name(sym: str) -> str:
    if sym == "MUSIC_SS_ANNE":
        return "SSAnne"
    return pascal(sym.removeprefix("MUSIC_"))


def tileset_name(sym: str) -> str:
    return pascal(sym.removeprefix("TILESET_"))


def sprite_name(sym: str) -> str:
    return pascal(sym.removeprefix("SPRITE_"))


# TM/HM item order (from constants/item_constants.asm add_tm/add_hm sequences)
def _load_tm_hm_lists(ref: Path):
    tms, hms = [], []
    for raw in read(ref / "constants" / "item_constants.asm").splitlines():
        line = strip_comment(raw).strip()
        m = re.match(r"^add_tm\s+(\w+)", line)
        if m:
            tms.append(m.group(1))
            continue
        m = re.match(r"^add_hm\s+(\w+)", line)
        if m:
            hms.append(m.group(1))
    return tms, hms


_TM_LIST, _HM_LIST = None, None


def item_name(sym: str, ref: Path | None = None) -> str:
    global _TM_LIST, _HM_LIST
    if sym.startswith("TM_"):
        if _TM_LIST is None and ref is not None:
            _TM_LIST, _HM_LIST = _load_tm_hm_lists(ref)
        if _TM_LIST is not None and sym[3:] in _TM_LIST:
            return f"Tm{_TM_LIST.index(sym[3:]) + 1:02d}"
        return pascal(sym)
    if sym.startswith("HM_"):
        if _HM_LIST is None and ref is not None:
            _TM_LIST, _HM_LIST = _load_tm_hm_lists(ref)
        if _HM_LIST is not None and sym[3:] in _HM_LIST:
            return f"Hm{_HM_LIST.index(sym[3:]) + 1:02d}"
        return pascal(sym)
    return pascal(sym)


def trainer_class_name(sym: str) -> str:
    return pascal(sym.removeprefix("OPP_"))


def compare_maps(ref: Path, only_map=None) -> list:
    """Returns list of human-readable diff lines."""
    maps = load_maps(ref)
    songs = load_map_songs(ref)
    remake_th = remake_trainer_headers()
    hidden_events = parse_hidden_events(ref)
    diffs = []
    for map_const, w, h in maps:
        mname = map_name(map_const)
        if only_map and mname != only_map:
            continue
        rj = remake_map_json(mname)
        robj = parse_objects(ref, map_const)
        rhdr = parse_header(ref, map_const)
        if rj is None:
            diffs.append(f"[{mname}] MISSING map.json")
            continue
        hdr = rj.get("header", {})
        # dimensions
        if (hdr.get("width"), hdr.get("height")) != (w, h):
            diffs.append(f"[{mname}] dims remake {hdr.get('width')}x{hdr.get('height')} != ref {w}x{h}")
        # music
        song = songs.get(rj.get("id"))
        if song and hdr.get("music") != music_name(song):
            diffs.append(f"[{mname}] music remake {hdr.get('music')!r} != ref {music_name(song)} ({song})")
        # tileset / connections / border / objects
        if robj is not None:
            if hdr.get("borderBlock") is not None:
                bm = re.search(r"db\s+\$?([0-9a-fA-F]+)\s*;\s*border block",
                               read(ref / "data" / "maps" / "objects" / f"{mname}.asm"))
                if bm and int(bm.group(1), 16) != hdr["borderBlock"]:
                    diffs.append(f"[{mname}] borderBlock remake {hdr['borderBlock']} != ref {int(bm.group(1),16)}")
            rwarps = rj.get("warps", [])
            if len(rwarps) != len(robj["warps"]):
                diffs.append(f"[{mname}] warps count remake {len(rwarps)} != ref {len(robj['warps'])}")
            for i, (rw, wref) in enumerate(zip(rwarps, robj["warps"])):
                ok = (rw.get("x"), rw.get("y")) == (wref["x"], wref["y"]) and \
                     ((wref["dest"] == "LAST_MAP" and not rw.get("destMap")) or
                      (wref["dest"] != "LAST_MAP" and
                       rw.get("destMap") == map_name(wref["dest"]))) and \
                     rw.get("destWarpId") == wref["dest_warp_id"] - 1
                if not ok:
                    diffs.append(f"[{mname}] warp#{i} remake {rw} != ref {wref}")
            rsigns = rj.get("signs", [])
            hidden = hidden_events.get(map_const, [])
            if len(rsigns) < len(robj["signs"]):
                diffs.append(f"[{mname}] signs count remake {len(rsigns)} < ref {len(robj['signs'])}")
            for i, (rs, sref) in enumerate(zip(rsigns, robj["signs"])):
                if (rs.get("x"), rs.get("y")) != (sref["x"], sref["y"]):
                    diffs.append(f"[{mname}] sign#{i} remake ({rs.get('x')},{rs.get('y')}) != ref ({sref['x']},{sref['y']})")
            # Remake signs beyond the reference bg_events may be adaptations of
            # reference hidden events (PCs, trash cans, statues, bookcases...).
            ref_bg = {(s["x"], s["y"]) for s in robj["signs"]}
            extra = [(rs.get("x"), rs.get("y")) for rs in rsigns
                     if (rs.get("x"), rs.get("y")) not in ref_bg]
            if extra:
                hset = {(x, y) for x, y, r, a in hidden}
                matched = [p for p in extra if p in hset]
                unmatched = [p for p in extra if p not in hset]
                if matched:
                    diffs.append(f"[{mname}] [adaptation] remake signs {matched} model ref hidden events")
                if unmatched:
                    diffs.append(f"[{mname}] [extra-sign] remake has signs with no ref counterpart: {unmatched}")
            # Reference hidden events with no remake sign — candidates for the
            # scene-script / coord-event audit.
            rem_signs = {(rs.get("x"), rs.get("y")) for rs in rsigns}
            for x, y, r, a in hidden:
                if r == "HiddenItems" or r == "HiddenCoins" or r == "CardKeyDoor":
                    continue  # handled by item/coin/door audits
                if (x, y) not in rem_signs:
                    diffs.append(f"[{mname}] [no-sign] ref hidden event ({x},{y}) {r} {a} has no remake sign")
            rnpcs = rj.get("npcs", [])
            if len(rnpcs) != len(robj["npcs"]):
                diffs.append(f"[{mname}] npcs count remake {len(rnpcs)} != ref {len(robj['npcs'])}")
            for i, (rn, nref) in enumerate(zip(rnpcs, robj["npcs"])):
                parts = []
                if rn.get("x") != nref["x"] or rn.get("y") != nref["y"]:
                    parts.append(f"pos remake ({rn.get('x')},{rn.get('y')}) != ref ({nref['x']},{nref['y']})")
                if rn.get("spriteName") != sprite_name(nref["sprite"]):
                    parts.append(f"sprite remake {rn.get('spriteName')} != ref {sprite_name(nref['sprite'])}")
                # movement/facing: same mapping the generator used
                ref_mv, ref_facing, ref_range = DIRECTION_MAP.get(
                    nref["range"], ("Wander", "Down", 0))
                if nref["movement"] == "STAY":
                    ref_mv, ref_range = "Stationary", 0
                if rn.get("movement") != ref_mv:
                    parts.append(f"movement remake {rn.get('movement')} != ref {ref_mv} ({nref['movement']},{nref['range']})")
                if rn.get("facing") != ref_facing:
                    parts.append(f"facing remake {rn.get('facing')} != ref {ref_facing} ({nref['movement']},{nref['range']})")
                if parts:
                    diffs.append(f"[{mname}] npc#{i} {sprite_name(nref['sprite'])}: {'; '.join(parts)}")
        else:
            if rj.get("warps") or rj.get("signs") or rj.get("npcs"):
                diffs.append(f"[{mname}] remake has objects but ref has no objects file")
        if rhdr is not None:
            if hdr.get("tileset") != tileset_name(rhdr["tileset"]):
                diffs.append(f"[{mname}] tileset remake {hdr.get('tileset')!r} != ref {tileset_name(rhdr['tileset'])} ({rhdr['tileset']})")
            rconns = rj.get("connections", {})
            for d, tgt, off in rhdr["connections"]:
                r = rconns.get(d)
                if r is None:
                    diffs.append(f"[{mname}] connection {d} missing in remake (ref {map_name(tgt)} off {off})")
                elif r.get("targetMap") != map_name(tgt) or r.get("offset") != off:
                    diffs.append(f"[{mname}] connection {d} remake {r} != ref {map_name(tgt)} off {off}")
            extra = set(rconns) - {d for d, _, _ in rhdr["connections"]}
            if extra:
                diffs.append(f"[{mname}] extra remake connections {sorted(extra)}")
        # trainer events
        tref = parse_trainer_events(ref, map_const)
        trmk = remake_th.get(mname.lower().replace("_", ""), [])
        if len(tref) != len(trmk):
            diffs.append(f"[{mname}] trainer events count remake {len(trmk)} != ref {len(tref)}")
        for i, ((ev, rng, _), (ev2, rng2)) in enumerate(zip(tref, trmk)):
            if ev.removeprefix("EVENT_") != ev2.removeprefix("EVENT_") or rng != rng2:
                diffs.append(f"[{mname}] trainer#{i} remake ({ev2},rng {rng2}) != ref ({ev},rng {rng})")
        # wild
        if "wild" in rj and rj["wild"] is not None:
            gw = load_grass_water(ref)
            lbl = next((l for c, l in gw if c == map_const), None)
            wref = parse_wild_file(ref, lbl) if lbl else None
            for v in ("red", "blue"):
                if wref is None:
                    continue
                rv = rj["wild"].get(v, {})
                for kind in ("grass", "water"):
                    rr = rv.get(kind, {})
                    rate, slots = wref[v][kind]
                    rslots = [(m["level"], m["species"]) for m in rr.get("mons", [])]
                    if rr.get("encounterRate") != rate:
                        diffs.append(f"[{mname}] {v}/{kind} rate remake {rr.get('encounterRate')} != ref {rate}")
                    if rslots != [(l, species_name(s)) for l, s in slots]:
                        diffs.append(f"[{mname}] {v}/{kind} slots differ\n    remake {rslots}\n    ref    {[(l, species_name(s)) for l, s in slots]}")
    return diffs


def compare_rods(ref: Path) -> list:
    diffs = []
    r = parse_rods(ref)
    rmk = remake_rods()
    if rmk["good"] != [(l, species_name(s)) for l, s in r["good"]]:
        diffs.append(f"[rods] good rod:\n    remake {rmk['good']}\n    ref    {r['good']}")
    if rmk["super_groups"] != [[(l, species_name(s)) for l, s in g] for g in r["super_groups"]]:
        diffs.append(f"[rods] super rod groups:\n    remake {rmk['super_groups']}\n    ref    {r['super_groups']}")
    ref_entries = [(map_name(m), i) for m, i in
                   [(m, int(g.removeprefix("Group")) - 1) for m, g in r["super_entries"]]]
    rmk_entries = [(map_name(m), i) for m, i in rmk["super_entries"]]
    if rmk_entries != ref_entries:
        diffs.append(f"[rods] super rod entries:\n    remake {rmk_entries}\n    ref    {ref_entries}")
    return diffs


def remake_names() -> dict:
    """{class: display name} from trainer_data.rs display_name()."""
    p = ROOT / "crates" / "pokered-data" / "src" / "trainer_data.rs"
    text = read(p)
    out = {}
    m = re.search(r"pub fn display_name\(&self\)[\s\S]*?match self \{([\s\S]*?)\n    \}", text)
    if m:
        for cm in re.finditer(r"TrainerClass::(\w+)\s*=>\s*\"([^\"]*)\"", m.group(1)):
            out[cm.group(1)] = cm.group(2)
    return out


def normalize_shop_item(s: str) -> str:
    """Normalize the various item spellings used in remake scene openShop lists."""
    s = s.strip().strip('"')
    if re.fullmatch(r"[A-Z][A-Z_]*", s):  # UPPER_SNAKE
        return pascal(s)
    if " " in s:  # "Ultra Ball" / "Full Restore"
        return pascal(s.replace(" ", "_").upper())
    return s


def compare_trainers(ref: Path) -> list:
    diffs = []
    r = parse_trainers(ref)
    rmon = remake_money()
    rname = remake_names()
    for cname, data in r.items():
        if cname.startswith("_"):
            continue
        rust_name = {"Psychic": "PsychicTr"}.get(cname, cname)
        p = ROOT / "crates" / "pokered-data" / "trainers" / f"{rust_name}.json"
        if not p.exists():
            diffs.append(f"[trainers] {cname}: missing {p.name}")
            continue
        rj = json.loads(read(p))
        rparties = [[(m["level"], m["species"]) for m in party["pokemon"]]
                    for party in rj.get("parties", [])]
        refp = [[(l, species_name(s)) for l, s in party] for party in data["parties"]]
        if rparties != refp:
            diffs.append(f"[trainers] {cname} parties differ:\n    remake {rparties}\n    ref    {refp}")
        if data["money"] is not None:
            ref_money = data["money"] // 100  # original stores yen×100 (display drops 2 digits)
            if rmon.get(rust_name) != ref_money:
                diffs.append(f"[trainers] {cname} money remake {rmon.get(rust_name)} != ref {ref_money} ({data['money']} stored)")
        if data["name"] is not None:
            rn = rname.get(rust_name)
            ref_name = data["name"].replace("é", "e")
            if rn is not None and rn != ref_name:
                diffs.append(f"[trainers] {cname} name remake {rn!r} != ref {ref_name!r} (raw {data['name']!r})")
        if data["move_choices"] is not None:
            rmc = remake_move_choices().get(rust_name)
            if rmc is not None and rmc != data["move_choices"]:
                diffs.append(f"[trainers] {cname} move_choices remake {rmc} != ref {data['move_choices']}")
    return diffs


def compare_shops(ref: Path) -> list:
    diffs = []
    marts = parse_marts(ref)
    rmk = remake_shop_inventories()
    # group ref clerks per map (clerk label base == map name)
    by_map = {}
    for (base, no), items in marts.items():
        if base.startswith("Unused"):
            continue  # unreferenced in the original
        by_map.setdefault(base, []).append([item_name(i, ref) for i in items])
    for map_name, ref_lists in by_map.items():
        rlists = rmk.get(map_name, [])
        rlists_norm = sorted(set(
            tuple(normalize_shop_item(i) for i in lst) for lst in rlists))
        if sorted(tuple(l) for l in ref_lists) != rlists_norm:
            diffs.append(f"[shops] {map_name}:\n    remake {[list(l) for l in rlists_norm]}\n    ref    {ref_lists}")
    for map_name in rmk:
        if map_name not in by_map:
            diffs.append(f"[shops] {map_name}: remake has shop(s) but ref has no clerk mart: {rmk[map_name]}")
    return diffs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref", required=True, help="path to pret/pokered checkout")
    ap.add_argument("--domain", default="all",
                    choices=["maps", "rods", "trainers", "shops", "all"])
    ap.add_argument("--only-map")
    ap.add_argument("--json")
    args = ap.parse_args()
    ref = Path(args.ref)
    diffs = []
    if args.domain in ("maps", "all"):
        diffs += compare_maps(ref, args.only_map)
    if args.domain in ("rods", "all"):
        diffs += compare_rods(ref)
    if args.domain in ("trainers", "all"):
        diffs += compare_trainers(ref)
    if args.domain in ("shops", "all"):
        diffs += compare_shops(ref)
    print(f"== {len(diffs)} diffs")
    for d in diffs:
        print(d)
    if args.json:
        Path(args.json).write_text(json.dumps(diffs, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()

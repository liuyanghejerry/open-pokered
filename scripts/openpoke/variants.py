#!/usr/bin/env python3
"""openpoke world variant generator (M7).

A variant is PURE DATA: a copy of the `crates/pokered-data/maps/` tree with
seeded mutations applied to `map.json` files. No engine changes. The desktop
build loads map data from the filesystem, so a spawned game picks the variant
up via `POKERED_MAPS_DIR` (map.json/map.blk) and `--scripts-dir`
(script.scene/script_config.json) — see `OpenPokeEnv(maps_dir=...)`.

Mutation classes (all recorded in the variant's `variant.json` manifest):

- `shuffle_encounters` — permute whole grass encounter tables (rate + mons)
  between maps, version sections (red/blue) kept paired. Legal by
  construction: species/levels are only exchanged, never invented.
- `shuffle_trainers` — re-roll `trainerClass`/`trainerSet` of sight trainers
  (`isTrainer: true`) on the given maps. Legality: class must exist in
  `crates/pokered-data/trainers/*.json`, set is 1-based into that class's
  party list, and the chosen party's max level is capped (`max_level`) so
  early-route trainers stay early-route strength.
- `shuffle_npcs` — relocate WANDERING NPCs to other walkable tiles of the
  same map. Only `movement == "Wander"` NPCs move: scripts cannot depend on
  a wanderer's tile, so scripted NPCs (old-man tutorial, Oak escort, gym
  NPCs) are structurally out of reach. Tile guards: walkable (engine
  collision data, dumped by `dump_world_data`), not a warp/sign/NPC tile,
  not adjacent to a warp (door entries stay clear), 2-cell margin from the
  connection edge.
- `shuffle_items` — relocate item balls (NPC entries with `itemId`) with
  the same tile guards plus one walkable 4-neighbour (so the ball can be
  faced and picked up).
- `shuffle_warps` — permute the DESTINATIONS of "simple" building doors
  within one outdoor map. A door is simple when its interior has only
  LAST_MAP exit mats and exactly one inbound door worldwide. After
  permuting, every touched interior's exit mats are re-pointed at the
  door's new warp index, preserving exact bidirectional pair integrity
  (LAST_MAP mats return to the map+warp index you entered from).
- `shuffle_connections` — EXPERIMENTAL. Swap two cardinal connections
  only when direction AND offset match and both source maps share the
  connection-axis dimension. Not used by the shipped profiles.

Determinism: one `random.Random(seed)` stream, mutations applied in a fixed
class order over sorted maps, no timestamps in the manifest — same
(seed, knobs) regenerates a byte-identical variant.

    python3 scripts/openpoke/variants.py npc-item-shuffle --seed 7001
    python3 scripts/openpoke/variants.py warp-shuffle --seed 7002 --name v2-warp-shuffle
"""
import argparse
import json
import random
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
BASE_MAPS = ROOT / "crates" / "pokered-data" / "maps"
TRAINERS_DIR = ROOT / "crates" / "pokered-data" / "trainers"
DUMP_BIN = ROOT / "target" / "debug" / "dump_world_data"
OUT_ROOT = ROOT / "target" / "agent" / "variants"

GENERATOR_VERSION = "openpoke-variants/1"

TOWN_MAPS = [
    "PalletTown", "ViridianCity", "PewterCity", "CeruleanCity",
    "VermilionCity", "LavenderTown", "CeladonCity", "FuchsiaCity",
    "SaffronCity", "CinnabarIsland",
]
TRAINER_MAPS = ["Route3", "Route22", "ViridianForest", "PewterGym"]
WARP_SHUFFLE_MAPS = ["ViridianCity", "PewterCity"]

# Shipped validation profiles (RQ3): distinct mutation classes per variant.
PROFILES = {
    "npc-item-shuffle": {"shuffle_npcs": {}, "shuffle_items": {}},
    "warp-shuffle": {"shuffle_warps": {}},
    "encounter-trainer-shuffle": {"shuffle_encounters": {}, "shuffle_trainers": {}},
}

# Fixed application order — part of the determinism contract.
MUTATION_ORDER = [
    "shuffle_encounters", "shuffle_trainers", "shuffle_npcs",
    "shuffle_items", "shuffle_warps", "shuffle_connections",
]


class VariantError(Exception):
    pass


# ── world data dump (Rust bin; engine collision + M3 world graph) ────
def dump_world_data(maps_dir):
    """Run the dump_world_data bin against a maps tree → {maps, edges}.

    The bin links pokered-data with default features (no embedded-map-data),
    so POKERED_MAPS_DIR selects the tree it reads.
    """
    if not DUMP_BIN.exists():
        subprocess.run(
            ["cargo", "build", "-p", "pokered-agent", "--bin", "dump_world_data"],
            cwd=str(ROOT), check=True)
    import os
    env = dict(os.environ, POKERED_MAPS_DIR=str(maps_dir))
    out = subprocess.run([str(DUMP_BIN)], cwd=str(ROOT), env=env,
                         check=True, capture_output=True, text=True)
    return json.loads(out.stdout)


# ── map.json helpers ──────────────────────────────────────────────────
def _load_map(maps_dir, name):
    return json.loads((maps_dir / name / "map.json").read_text())


def _save_map(maps_dir, name, data):
    (maps_dir / name / "map.json").write_text(
        json.dumps(data, indent=1) + "\n")


def _map_names(maps_dir):
    return sorted(p.name for p in maps_dir.iterdir()
                  if p.is_dir() and (p / "map.json").exists())


# ── tile eligibility (npcs / items) ───────────────────────────────────
def _eligible_tiles(grid, npcs, warps, signs, need_walkable_neighbor):
    """Walkable cells of one map that may receive a relocated entity."""
    w, h = grid["width_tiles"], grid["height_tiles"]
    rows = grid["walkable"]

    def walk(x, y):
        return 0 <= x < w and 0 <= y < h and rows[y][x] == "1"

    occupied = {(n["x"], n["y"]) for n in npcs}
    warp_tiles = {(t["x"], t["y"]) for t in warps}
    sign_tiles = {(s["x"], s["y"]) for s in signs}
    blocked = occupied | warp_tiles | sign_tiles
    for (wx, wy) in warp_tiles:  # keep door approaches clear
        blocked |= {(wx + 1, wy), (wx - 1, wy), (wx, wy + 1), (wx, wy - 1)}

    out = []
    for y in range(2, h - 2):
        for x in range(2, w - 2):
            if not walk(x, y) or (x, y) in blocked:
                continue
            if need_walkable_neighbor and not any(
                    (walk(x + 1, y), walk(x - 1, y), walk(x, y + 1), walk(x, y - 1))):
                continue
            out.append((x, y))
    return out


# ── mutation classes ──────────────────────────────────────────────────
def shuffle_encounters(rng, maps_dir, params, _walkable):
    """Permute whole grass tables across maps that have wild grass data."""
    candidates = []
    for name in _map_names(maps_dir):
        data = _load_map(maps_dir, name)
        wild = data.get("wild") or {}
        red = (wild.get("red") or {}).get("grass") or {}
        if red.get("mons"):
            candidates.append((name, data))
    tables = [json.loads(json.dumps(
        {v: (d["wild"].get(v) or {}).get("grass")
         for v in ("red", "blue") if (d["wild"].get(v) or {}).get("grass")}))
        for _, d in candidates]
    order = list(range(len(candidates)))
    rng.shuffle(order)
    if all(order[i] == i for i in range(len(order))):
        order = order[1:] + order[:1]  # guarantee a visible change
    records = []
    for i, (name, data) in enumerate(candidates):
        new = tables[order[i]]
        for version, table in new.items():
            data["wild"][version]["grass"] = json.loads(json.dumps(table))
        _save_map(maps_dir, name, data)
        old_grass = tables[i].get("red") or {}
        new_grass = new.get("red") or {}
        records.append({
            "class": "shuffle_encounters", "map": name,
            "from": _grass_sig(old_grass), "to": _grass_sig(new_grass)})
    return records


def _grass_sig(grass):
    mons = grass.get("mons") or []
    first = f"{mons[0]['species']}@{mons[0]['level']}" if mons else "-"
    return f"rate{grass.get('encounterRate')}/{first}..."


def shuffle_trainers(rng, maps_dir, params, _walkable, trainers_dir=None):
    """Re-roll trainer class/set on sight trainers, level-capped."""
    trainers_dir = Path(trainers_dir) if trainers_dir else TRAINERS_DIR
    max_level = params.get("max_level", 15)
    pool = []  # (class_name, 1-based set)
    for path in sorted(trainers_dir.glob("*.json")):
        data = json.loads(path.read_text())
        for idx, party in enumerate(data.get("parties", [])):
            mons = party.get("pokemon", [])
            if mons and max(m["level"] for m in mons) <= max_level:
                pool.append((data["class"], idx + 1))
    if not pool:
        raise VariantError(f"no trainer parties at max_level={max_level}")
    records = []
    for name in params.get("maps", TRAINER_MAPS):
        data = _load_map(maps_dir, name)
        changed = False
        for npc in data.get("npcs", []):
            if not npc.get("isTrainer"):
                continue
            old = (npc.get("trainerClass"), npc.get("trainerSet"))
            new = pool[rng.randrange(len(pool))]
            npc["trainerClass"], npc["trainerSet"] = new
            records.append({
                "class": "shuffle_trainers", "map": name,
                "textId": npc.get("textId"),
                "from": f"{old[0]}#{old[1]}", "to": f"{new[0]}#{new[1]}"})
            changed = True
        if changed:
            _save_map(maps_dir, name, data)
    return records


def _relocate(rng, maps_dir, params, walkable, *, want, need_neighbor, cls):
    records = []
    names = params.get("maps")
    for name in sorted(walkable if names is None else names):
        grid = walkable.get(name)
        if grid is None:
            continue
        path = maps_dir / name / "map.json"
        if not path.exists():
            continue
        data = json.loads(path.read_text())
        npcs = data.get("npcs", [])
        targets = [n for n in npcs if want(n)]
        if not targets:
            continue
        tiles = _eligible_tiles(grid, npcs, data.get("warps", []),
                                data.get("signs", []), need_neighbor)
        changed = False
        for npc in targets:
            if not tiles:
                records.append({"class": cls, "map": name,
                                "note": "no eligible tile left; skipped"})
                break
            old = (npc["x"], npc["y"])
            pos = tiles.pop(rng.randrange(len(tiles)))
            npc["x"], npc["y"] = pos
            records.append({"class": cls, "map": name,
                            "textId": npc.get("textId"),
                            "sprite": npc.get("spriteName"),
                            "from": list(old), "to": list(pos)})
            changed = True
        if changed:
            _save_map(maps_dir, name, data)
    return records


def shuffle_npcs(rng, maps_dir, params, walkable):
    """Relocate wandering NPCs (scripts can't depend on a wanderer's tile)."""
    params = dict(params)
    params.setdefault("maps", TOWN_MAPS)
    return _relocate(
        rng, maps_dir, params, walkable, need_neighbor=False,
        cls="shuffle_npcs",
        want=lambda n: n.get("itemId") is None and not n.get("isTrainer")
        and n.get("movement") == "Wander")


def shuffle_items(rng, maps_dir, params, walkable):
    """Relocate item balls; one walkable neighbour required for pickup."""
    return _relocate(
        rng, maps_dir, params, walkable, need_neighbor=True,
        cls="shuffle_items",
        want=lambda n: n.get("itemId") is not None)


def shuffle_warps(rng, maps_dir, params, _walkable):
    """Permute simple building-door destinations within each outdoor map,
    then re-point the touched interiors' exit mats at the door's new index."""
    # Worldwide inbound-door counts per interior (multi-entrance interiors
    # like museums/gates are not "simple" and stay put).
    inbound = {}
    all_maps = {n: _load_map(maps_dir, n) for n in _map_names(maps_dir)}
    for name, data in all_maps.items():
        for w in data.get("warps", []):
            if w.get("destMap"):
                inbound[w["destMap"]] = inbound.get(w["destMap"], 0) + 1

    def simple_interior(y):
        data = all_maps.get(y)
        warps = (data or {}).get("warps", [])
        return (data is not None and warps
                and all(not w.get("destMap") for w in warps)
                and inbound.get(y, 0) == 1)

    records = []
    for name in params.get("maps", WARP_SHUFFLE_MAPS):
        data = all_maps.get(name)
        if data is None:
            continue
        warps = data.get("warps", [])
        doors = [i for i, w in enumerate(warps)
                 if w.get("destMap") and simple_interior(w["destMap"])
                 and w["destWarpId"] < len(all_maps[w["destMap"]]["warps"])]
        if len(doors) < 2:
            records.append({"class": "shuffle_warps", "map": name,
                            "note": f"only {len(doors)} simple doors; skipped"})
            continue
        values = [(warps[i]["destMap"], warps[i]["destWarpId"]) for i in doors]
        order = list(range(len(values)))
        rng.shuffle(order)
        if all(order[i] == i for i in range(len(order))):
            order = order[1:] + order[:1]
        for slot, door_idx in enumerate(doors):
            old = (warps[door_idx]["destMap"], warps[door_idx]["destWarpId"])
            new = values[order[slot]]
            warps[door_idx]["destMap"], warps[door_idx]["destWarpId"] = new
            # Bidirectional integrity: the interior's LAST_MAP exit mats
            # deposit at the outdoor map's warp[door_idx] — the very door
            # the player walked in through.
            for w2 in all_maps[new[0]]["warps"]:
                w2["destWarpId"] = door_idx
            records.append({
                "class": "shuffle_warps", "map": name, "warpIndex": door_idx,
                "tile": [warps[door_idx]["x"], warps[door_idx]["y"]],
                "from": f"{old[0]}#{old[1]}", "to": f"{new[0]}#{new[1]}"})
        _save_map(maps_dir, name, data)
    touched = {r["to"].split("#")[0] for r in records if r["class"] == "shuffle_warps"}
    for interior in touched:
        _save_map(maps_dir, interior, all_maps[interior])
    return records


def shuffle_connections(rng, maps_dir, params, _walkable):
    """EXPERIMENTAL: swap same-direction/same-offset connection targets."""
    records = []
    entries = []  # (map, direction, target, offset, width, height)
    for name in params.get("maps", []):
        data = _load_map(maps_dir, name)
        for direction in ("north", "south", "west", "east"):
            conn = (data.get("connections") or {}).get(direction)
            if conn:
                entries.append((name, direction, data, conn))
    by_key = {}
    for name, direction, data, conn in entries:
        axis = (data["header"]["width"] if direction in ("north", "south")
                else data["header"]["height"])
        by_key.setdefault((direction, conn["offset"], axis), []).append(
            (name, data, conn))
    for (direction, offset, _axis), group in sorted(by_key.items()):
        if len(group) < 2:
            continue
        targets = [g[2]["targetMap"] for g in group]
        order = list(range(len(targets)))
        rng.shuffle(order)
        if all(order[i] == i for i in range(len(order))):
            continue  # identity here is fine; other pairs may still swap
        for slot, (name, data, conn) in enumerate(group):
            old = conn["targetMap"]
            conn["targetMap"] = targets[order[slot]]
            records.append({
                "class": "shuffle_connections", "map": name,
                "experimental": True,
                "direction": direction, "offset": offset,
                "from": old, "to": conn["targetMap"]})
            _save_map(maps_dir, name, data)  # conn mutates data in place
    return records


MUTATORS = {
    "shuffle_encounters": shuffle_encounters,
    "shuffle_trainers": shuffle_trainers,
    "shuffle_npcs": shuffle_npcs,
    "shuffle_items": shuffle_items,
    "shuffle_warps": shuffle_warps,
    "shuffle_connections": shuffle_connections,
}


# ── generation ────────────────────────────────────────────────────────
def generate_variant(name, seed, knobs, out_root=None, base_dir=None,
                     walkable=None, trainers_dir=None):
    """Copy the maps tree and apply seeded mutations → variant dir path."""
    base_dir = Path(base_dir) if base_dir else BASE_MAPS
    out_root = Path(out_root) if out_root else OUT_ROOT
    variant_dir = out_root / name
    if variant_dir.exists():
        raise VariantError(f"{variant_dir} already exists; delete or pick another name")
    shutil.copytree(base_dir, variant_dir)

    rng = random.Random(seed)
    if walkable is None:
        # Walkability never changes under these mutation classes (map.blk
        # and tilesets are untouched), so one pre-mutation dump is exact.
        walkable = dump_world_data(variant_dir)["maps"]

    mutations = []
    for cls in MUTATION_ORDER:
        if cls not in knobs:
            continue
        params = knobs[cls] or {}
        mutator = MUTATORS[cls]
        if cls == "shuffle_trainers":
            records = mutator(rng, variant_dir, params, walkable,
                              trainers_dir=trainers_dir)
        else:
            records = mutator(rng, variant_dir, params, walkable)
        mutations.extend(records)

    summary = {}
    for rec in mutations:
        summary[rec["class"]] = summary.get(rec["class"], 0) + 1
    manifest = {
        "name": name,
        "seed": seed,
        "generator": GENERATOR_VERSION,
        "base": str(base_dir.relative_to(ROOT) if base_dir.is_relative_to(ROOT)
                     else base_dir),
        "knobs": knobs,
        "summary": summary,
        "mutations": mutations,
    }
    (variant_dir / "variant.json").write_text(
        json.dumps(manifest, indent=1, sort_keys=True) + "\n")
    return variant_dir


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("profile", choices=sorted(PROFILES), help="mutation profile")
    p.add_argument("--seed", type=int, required=True)
    p.add_argument("--name", default=None,
                   help="variant dir name (default: <profile>-s<seed>)")
    p.add_argument("--out-root", default=None)
    args = p.parse_args(argv)
    name = args.name or f"{args.profile}-s{args.seed}"
    out = generate_variant(name, args.seed, PROFILES[args.profile],
                           out_root=args.out_root)
    manifest = json.loads((out / "variant.json").read_text())
    print(f"variant → {out}")
    print(f"  mutations: {json.dumps(manifest['summary'], sort_keys=True)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

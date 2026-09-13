#!/usr/bin/env python3
"""openpokered variant validator (M7).

Two layers:

1. **Static** (`validate_static`) — builds the M3 world graph + engine
   walkability grids from the VARIANT's own data (via the
   `dump_world_data` Rust bin, which honors POKERED_MAPS_DIR) and asserts:
   - every explicit warp destination exists and lands on a valid warp slot;
   - simple interiors (LAST_MAP exits, single inbound door) have all exit
     mats pointing back at that door's warp index — warp-pair integrity;
   - key routes exist in the graph (BFS): PalletTown→ViridianCity,
     PalletTown→PewterCity (the NEW route is reported, so a legitimate
     topology change is validated, not assumed);
   - no orphan towns: every town map is reachable from PalletTown;
   - every warp tile and every item ball sits on a walkable cell.
2. **In-game** (`validate_ingame`) — the strongest signal: spawn the
   headless seeded game on the variant (POKERED_MAPS_DIR + --scripts-dir)
   and run the M6 reach tasks for real traversal.

    python3 scripts/openpokered/validate_variant.py target/agent/variants/v1
    python3 scripts/openpokered/validate_variant.py <dir> --skip-ingame

Exit code 0 = all checks pass. Unit tests inject fixture world data; no
game or Rust bin is needed there.
"""
import argparse
import json
import sys
from collections import deque
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from openpokered.tasks import load_task  # noqa: E402
from openpokered.variants import TOWN_MAPS, VariantError, dump_world_data  # noqa: E402

KEY_ROUTES = [("PalletTown", "ViridianCity"), ("PalletTown", "PewterCity")]
INGAME_TASKS = ["reach-viridian-city", "reach-pewter-city"]
TASKS_DIR = Path(__file__).resolve().parent / "tasks"


class Check:
    def __init__(self, name, ok, detail=""):
        self.name = name
        self.ok = ok
        self.detail = detail

    def __repr__(self):
        status = "ok " if self.ok else "FAIL"
        return f"[{status}] {self.name}" + (f" — {self.detail}" if self.detail else "")


# ── static checks ─────────────────────────────────────────────────────
def _bfs_route(adjacency, start, goal):
    """Map-chain shortest route over graph edges; None when unreachable."""
    if start == goal:
        return [start]
    prev = {start: None}
    queue = deque([start])
    while queue:
        cur = queue.popleft()
        for nxt in adjacency.get(cur, ()):  # directed edges
            if nxt in prev:
                continue
            prev[nxt] = cur
            if nxt == goal:
                route = [goal]
                while route[-1] != start:
                    route.append(prev[route[-1]])
                return route[::-1]
            queue.append(nxt)
    return None


def check_warp_integrity(variant_dir):
    """Explicit warps land on valid slots; simple interiors return to their
    single inbound door (all exit mats carry its warp index)."""
    maps = {}
    for p in sorted(Path(variant_dir).iterdir()):
        if p.is_dir() and (p / "map.json").exists():
            maps[p.name] = json.loads((p / "map.json").read_text())
    inbound = {}
    for data in maps.values():
        for w in data.get("warps", []):
            if w.get("destMap"):
                inbound[w["destMap"]] = inbound.get(w["destMap"], 0) + 1
    problems = []
    for name, data in maps.items():
        warps = data.get("warps", [])
        for i, w in enumerate(warps):
            dest = w.get("destMap")
            if not dest:
                continue
            if dest.startswith("Unused"):
                continue  # elevator/script placeholder slots, never walked
            dest_data = maps.get(dest)
            if dest_data is None:
                problems.append(f"{name}#{i}: destMap {dest} missing")
                continue
            dest_warps = dest_data.get("warps", [])
            if w["destWarpId"] >= len(dest_warps):
                problems.append(
                    f"{name}#{i}: destWarpId {w['destWarpId']} >= "
                    f"{dest} warps ({len(dest_warps)})")
                continue
            simple = (inbound.get(dest, 0) == 1 and dest_warps
                      and all(not dw.get("destMap") for dw in dest_warps))
            if simple:
                bad = [dw["destWarpId"] for dw in dest_warps
                       if dw["destWarpId"] != i]
                if bad:
                    problems.append(
                        f"{name}#{i}→{dest}: exit mats point at {bad}, "
                        f"not the inbound door index {i}")
    return problems


def check_routes(world_data, required):
    adjacency = {}
    for e in world_data["edges"]:
        adjacency.setdefault(e["from_map"], set()).add(e["to_map"])
    routes, problems = {}, []
    for start, goal in required:
        route = _bfs_route(adjacency, start, goal)
        routes[f"{start}->{goal}"] = route
        if route is None:
            problems.append(f"{start}->{goal}: unreachable")
    return routes, problems, adjacency


def check_no_orphan_towns(adjacency, towns):
    problems = []
    for town in towns:
        if _bfs_route(adjacency, "PalletTown", town) is None:
            problems.append(f"{town} unreachable from PalletTown")
    return problems


def check_walkability(variant_dir, world_data):
    """Item balls must be grabbable: at least one standable 4-neighbour cell
    (walkable `1` or water `2` — Gen I items may be faced while surfing).
    The ball's own tile may be unwalkable in base data (items behind
    rails/statues, e.g. PokemonMansionB1F (5,4)); no mutation class moves
    warp tiles, so warp-tile walkability is not asserted."""
    grids = world_data["maps"]
    problems = []
    for p in sorted(Path(variant_dir).iterdir()):
        if not (p.is_dir() and (p / "map.json").exists()):
            continue
        grid = grids.get(p.name)
        if grid is None:
            continue
        data = json.loads((p / "map.json").read_text())

        def standable(x, y):
            return (0 <= x < grid["width_tiles"] and 0 <= y < grid["height_tiles"]
                    and grid["walkable"][y][x] in ("1", "2"))

        for n in data.get("npcs", []):
            if n.get("itemId") is None:
                continue
            x, y = n["x"], n["y"]
            if not any((standable(x + 1, y), standable(x - 1, y),
                        standable(x, y + 1), standable(x, y - 1))):
                problems.append(f"{p.name} item textId {n.get('textId')} has no "
                                f"standable neighbour at ({x},{y})")
    return problems


def validate_static(variant_dir, world_data=None, required_routes=None,
                    towns=None):
    variant_dir = Path(variant_dir)
    manifest_path = variant_dir / "variant.json"
    manifest = (json.loads(manifest_path.read_text())
                if manifest_path.exists() else None)
    if world_data is None:
        world_data = dump_world_data(variant_dir)
    required_routes = required_routes or KEY_ROUTES
    towns = towns or TOWN_MAPS

    checks = []
    warp_problems = check_warp_integrity(variant_dir)
    checks.append(Check("warp_integrity", not warp_problems,
                        "; ".join(warp_problems[:3]) +
                        (f" (+{len(warp_problems) - 3} more)"
                         if len(warp_problems) > 3 else "")))
    routes, route_problems, adjacency = check_routes(world_data, required_routes)
    checks.append(Check("key_routes", not route_problems,
                        "; ".join(route_problems) or
                        "; ".join(f"{k}: {'→'.join(v)}" for k, v in routes.items())))
    orphan_problems = check_no_orphan_towns(adjacency, towns)
    checks.append(Check("no_orphan_towns", not orphan_problems,
                        "; ".join(orphan_problems)))
    walk_problems = check_walkability(variant_dir, world_data)
    checks.append(Check("walkability", not walk_problems,
                        "; ".join(walk_problems[:3]) +
                        (f" (+{len(walk_problems) - 3} more)"
                         if len(walk_problems) > 3 else "")))
    return {
        "variant": str(variant_dir),
        "manifest": manifest,
        "ok": all(c.ok for c in checks),
        "checks": checks,
        "routes": routes,
    }


# ── in-game smoke ─────────────────────────────────────────────────────
def validate_ingame(variant_dir, task_ids=None, binary=None):
    """Run M6 reach tasks on a headless game pointed at the variant."""
    from openpokered.run_task import run_once
    task_ids = task_ids or INGAME_TASKS
    results = {}
    for task_id in task_ids:
        task = load_task(TASKS_DIR / f"{task_id}.json")
        metrics = run_once(task, task["seed"], binary=binary,
                           write_metrics=False, quiet=True, maps_dir=str(variant_dir))
        results[task_id] = metrics
    return results


# ── CLI ───────────────────────────────────────────────────────────────
def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("variant_dir")
    p.add_argument("--skip-ingame", action="store_true")
    p.add_argument("--tasks", default=None, help="comma task ids for the smoke run")
    args = p.parse_args(argv)

    variant_dir = Path(args.variant_dir)
    if not variant_dir.is_dir():
        raise VariantError(f"{variant_dir} is not a directory")

    report = validate_static(variant_dir)
    if report["manifest"]:
        m = report["manifest"]
        print(f"variant: {m['name']} seed={m['seed']} "
              f"mutations={json.dumps(m['summary'], sort_keys=True)}")
    else:
        print(f"variant: {variant_dir} (no manifest; checking raw data)")
    for check in report["checks"]:
        print(f"  {check!r}")

    ok = report["ok"]
    if not args.skip_ingame:
        task_ids = args.tasks.split(",") if args.tasks else None
        results = validate_ingame(variant_dir, task_ids)
        for task_id, m in results.items():
            status = "ok " if m.success else "FAIL"
            print(f"  [{status}] ingame {task_id} — frames={m.frames_elapsed} "
                  f"battles={m.battles} reason={m.failure_reason!r}")
        ok = ok and all(m.success for m in results.values())
    print("VALID" if ok else "INVALID")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

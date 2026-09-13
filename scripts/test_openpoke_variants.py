"""Unit checks for openpoke M7 variant tooling (stdlib unittest, fixtures
only — no game binary, no Rust dump bin, no repo maps tree).

Fixtures: a tiny synthetic maps tree (PalletTown/Route1/ViridianCity/
Route2 + three one-door interiors), synthetic walkable grids, synthetic
trainer data. Generation and static validation run entirely against these.
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from openpoke import validate_variant as vv
from openpoke import variants


# ── fixture construction ──────────────────────────────────────────────
def _grid(w, h, solid=()):
    rows = []
    for y in range(h):
        row = "".join("0" if (x, y) in solid else "1" for x in range(w))
        rows.append(row)
    return {"width_tiles": w, "height_tiles": h, "walkable": rows}


def _map(name, *, tileset="Overworld", width=6, height=5, connections=None,
         warps=None, npcs=None, signs=None, wild=None):
    return {
        "$schema": "../../schemas/map.schema.json",
        "id": 0, "name": name,
        "header": {"tileset": tileset, "music": "Routes1", "connectionFlags": 0,
                   "width": width, "height": height, "borderBlock": 0},
        "connections": connections or {},
        "warps": warps or [],
        "npcs": npcs or [],
        "signs": signs or [],
        "text": {"npc": {}, "sign": {}},
        "wild": wild,
    }


def _npc(sprite, x, y, movement="Wander", **kw):
    base = {"spriteId": 1, "spriteName": sprite, "x": x, "y": y,
            "movement": movement, "facing": "Down", "range": 0,
            "textId": kw.pop("text_id", 1), "isTrainer": False}
    base.update(kw)
    return base


FIXTURE_SPECS = {
    "PalletTown": _map(
        "PalletTown",
        connections={"north": {"targetMap": "Route1", "offset": 0}},
        warps=[{"x": 2, "y": 2, "destMap": "RedsHouse1F", "destWarpId": 0}],
        npcs=[_npc("Girl", 3, 3)]),
    "Route1": _map(
        "Route1",
        connections={"south": {"targetMap": "PalletTown", "offset": 0},
                     "north": {"targetMap": "ViridianCity", "offset": 0}},
        wild={"red": {"grass": {"encounterRate": 25, "mons": [
            {"level": 3, "species": "Pidgey"}, {"level": 3, "species": "Rattata"}]}}},
        npcs=[]),
    "ViridianCity": _map(
        "ViridianCity",
        connections={"south": {"targetMap": "Route1", "offset": 0},
                     "north": {"targetMap": "Route2", "offset": 0}},
        warps=[{"x": 2, "y": 2, "destMap": "MartA", "destWarpId": 0},
               {"x": 4, "y": 2, "destMap": "HouseB", "destWarpId": 0}],
        npcs=[_npc("Youngster", 3, 4, text_id=1),
              _npc("BugCatcher", 5, 4, movement="Stationary", text_id=2,
                   isTrainer=True, trainerClass="Youngster", trainerSet=1),
              _npc("Pokeball", 6, 4, movement="Stationary", text_id=3,
                   itemId=13)]),
    "Route2": _map(
        "Route2",
        connections={"south": {"targetMap": "ViridianCity", "offset": 0}},
        wild={"red": {"grass": {"encounterRate": 25, "mons": [
            {"level": 4, "species": "Caterpie"}, {"level": 4, "species": "Weedle"}]}}}),
    "RedsHouse1F": _map(
        "RedsHouse1F", tileset="RedsHouse1",
        warps=[{"x": 2, "y": 7, "destWarpId": 0}]),
    "MartA": _map(
        "MartA", tileset="Pokecenter",
        warps=[{"x": 3, "y": 7, "destWarpId": 0}]),
    "HouseB": _map(
        "HouseB", tileset="House",
        warps=[{"x": 2, "y": 7, "destWarpId": 1}]),
}

# Grids: 12x10 tiles, border ring solid, one solid pillar at (8,5).
SOLID = ({(x, y) for x in range(12) for y in (0, 9)}
         | {(x, y) for y in range(10) for x in (0, 11)} | {(8, 5)})
FIXTURE_WALKABLE = {name: _grid(12, 10, SOLID) for name in FIXTURE_SPECS}

FIXTURE_EDGES = [
    {"kind": "connection", "from_map": "PalletTown", "to_map": "Route1",
     "direction": "north", "offset": 0, "dynamic_destination": False},
    {"kind": "connection", "from_map": "Route1", "to_map": "PalletTown",
     "direction": "south", "offset": 0, "dynamic_destination": False},
    {"kind": "connection", "from_map": "Route1", "to_map": "ViridianCity",
     "direction": "north", "offset": 0, "dynamic_destination": False},
    {"kind": "connection", "from_map": "ViridianCity", "to_map": "Route1",
     "direction": "south", "offset": 0, "dynamic_destination": False},
    {"kind": "connection", "from_map": "ViridianCity", "to_map": "Route2",
     "direction": "north", "offset": 0, "dynamic_destination": False},
    {"kind": "connection", "from_map": "Route2", "to_map": "ViridianCity",
     "direction": "south", "offset": 0, "dynamic_destination": False},
    {"kind": "warp", "from_map": "PalletTown", "to_map": "RedsHouse1F",
     "warp_index": 0, "from_pos": {"x": 2, "y": 2},
     "to_pos": {"x": 2, "y": 7}, "dynamic_destination": False},
    {"kind": "warp", "from_map": "ViridianCity", "to_map": "MartA",
     "warp_index": 0, "from_pos": {"x": 2, "y": 2},
     "to_pos": {"x": 3, "y": 7}, "dynamic_destination": False},
    {"kind": "warp", "from_map": "ViridianCity", "to_map": "HouseB",
     "warp_index": 1, "from_pos": {"x": 4, "y": 2},
     "to_pos": {"x": 2, "y": 7}, "dynamic_destination": False},
]

FIXTURE_TRAINERS = {
    "Youngster": {"class": "Youngster", "constName": "YOUNGSTER", "parties": [
        {"pokemon": [{"level": 11, "species": "Rattata"}]},
        {"pokemon": [{"level": 14, "species": "Spearow"}]}]},
    "BugCatcher": {"class": "BugCatcher", "constName": "BUG_CATCHER", "parties": [
        {"pokemon": [{"level": 9, "species": "Caterpie"}]}]},
    "Brock": {"class": "Brock", "constName": "BROCK", "parties": [
        {"pokemon": [{"level": 99, "species": "Onix"}]}]},
}


def _write_fixture(root):
    maps_dir = Path(root) / "maps"
    for name, spec in FIXTURE_SPECS.items():
        d = maps_dir / name
        d.mkdir(parents=True)
        (d / "map.json").write_text(json.dumps(spec, indent=1) + "\n")
        (d / "map.blk").write_bytes(bytes(30))  # content unused by fixtures
    trainers_dir = Path(root) / "trainers"
    trainers_dir.mkdir()
    for cls, data in FIXTURE_TRAINERS.items():
        (trainers_dir / f"{cls}.json").write_text(json.dumps(data))
    return maps_dir, trainers_dir


def _generate(root, name, seed, knobs, maps_dir, trainers_dir):
    return variants.generate_variant(
        name, seed, knobs, out_root=Path(root) / "variants",
        base_dir=maps_dir, walkable=FIXTURE_WALKABLE,
        trainers_dir=trainers_dir)


def _manifest(variant_dir):
    return json.loads((Path(variant_dir) / "variant.json").read_text())


class GeneratorTests(unittest.TestCase):
    def test_determinism_same_seed_byte_identical(self):
        with tempfile.TemporaryDirectory() as root:
            maps_dir, trainers_dir = _write_fixture(root)
            knobs = {"shuffle_npcs": {"maps": ["PalletTown", "ViridianCity"]},
                     "shuffle_items": {},
                     "shuffle_warps": {"maps": ["ViridianCity"]},
                     "shuffle_encounters": {},
                     "shuffle_trainers": {"maps": ["ViridianCity"]}}
            a = _generate(root, "va", 1234, knobs, maps_dir, trainers_dir)
            # A second variant from the same (pristine) base + seed/knobs.
            maps_dir2 = Path(root) / "maps2"
            import shutil
            shutil.copytree(maps_dir, maps_dir2)
            b = variants.generate_variant(
                "va", 1234, knobs, out_root=Path(root) / "variants2",
                base_dir=maps_dir2, walkable=FIXTURE_WALKABLE,
                trainers_dir=trainers_dir)
            ma = (a / "variant.json").read_bytes()
            mb = (b / "variant.json").read_bytes()
            # Manifests differ only in the recorded base path.
            ma_obj, mb_obj = json.loads(ma), json.loads(mb)
            self.assertNotEqual(ma_obj["base"], mb_obj["base"])
            ma_obj.pop("base")
            mb_obj.pop("base")
            self.assertEqual(ma_obj, mb_obj)
            for spec in FIXTURE_SPECS:
                self.assertEqual(
                    (a / spec / "map.json").read_bytes(),
                    (b / spec / "map.json").read_bytes(), spec)

    def test_existing_dir_rejected(self):
        with tempfile.TemporaryDirectory() as root:
            maps_dir, trainers_dir = _write_fixture(root)
            _generate(root, "vx", 1, {}, maps_dir, trainers_dir)
            with self.assertRaises(variants.VariantError):
                _generate(root, "vx", 1, {}, maps_dir, trainers_dir)

    def test_warp_shuffle_preserves_pair_integrity(self):
        with tempfile.TemporaryDirectory() as root:
            maps_dir, trainers_dir = _write_fixture(root)
            out = _generate(root, "vw", 7, {"shuffle_warps": {"maps": ["ViridianCity"]}},
                            maps_dir, trainers_dir)
            recs = [r for r in _manifest(out)["mutations"]
                    if r["class"] == "shuffle_warps"]
            self.assertEqual(len(recs), 2)  # MartA and HouseB doors swapped
            self.assertTrue(all(r["from"] != r["to"] for r in recs))
            # ViridianCity's two doors now point at each other's interior,
            # and each interior's exit mat carries its door's new index.
            data = json.loads((out / "ViridianCity" / "map.json").read_text())
            doors = {i: w["destMap"] for i, w in enumerate(data["warps"])}
            self.assertEqual(doors, {0: "HouseB", 1: "MartA"})
            for interior, door_idx in (("HouseB", 0), ("MartA", 1)):
                iw = json.loads((out / interior / "map.json").read_text())["warps"]
                self.assertTrue(all(w["destWarpId"] == door_idx for w in iw))
            # The validator's pair-integrity check must see zero problems.
            self.assertEqual(vv.check_warp_integrity(out), [])

    def test_item_relocation_lands_legal(self):
        with tempfile.TemporaryDirectory() as root:
            maps_dir, trainers_dir = _write_fixture(root)
            out = _generate(root, "vi", 42, {"shuffle_items": {}},
                            maps_dir, trainers_dir)
            recs = [r for r in _manifest(out)["mutations"]
                    if r["class"] == "shuffle_items"]
            self.assertEqual(len(recs), 1)
            x, y = recs[0]["to"]
            grid = FIXTURE_WALKABLE["ViridianCity"]
            self.assertEqual(grid["walkable"][y][x], "1")
            self.assertNotIn((x, y), SOLID)
            self.assertGreaterEqual(x, 2)  # connection-edge margin
            self.assertGreaterEqual(y, 2)
            # Not on / adjacent to a warp tile.
            warps = FIXTURE_SPECS["ViridianCity"]["warps"]
            for w in warps:
                self.assertGreater(abs(x - w["x"]) + abs(y - w["y"]), 1)
            # On-disk data agrees, and the walkability check passes.
            data = json.loads((out / "ViridianCity" / "map.json").read_text())
            ball = next(n for n in data["npcs"] if n.get("itemId") is not None)
            self.assertEqual([ball["x"], ball["y"]], [x, y])
            world = {"maps": FIXTURE_WALKABLE, "edges": FIXTURE_EDGES}
            self.assertEqual(vv.check_walkability(out, world), [])

    def test_npc_relocation_only_wanderers(self):
        with tempfile.TemporaryDirectory() as root:
            maps_dir, trainers_dir = _write_fixture(root)
            out = _generate(root, "vn", 9, {"shuffle_npcs": {"maps": ["ViridianCity"]}},
                            maps_dir, trainers_dir)
            recs = [r for r in _manifest(out)["mutations"]
                    if r["class"] == "shuffle_npcs"]
            # Only the Youngster wanders; trainer + item ball stay put.
            self.assertEqual(len(recs), 1)
            self.assertEqual(recs[0]["textId"], 1)
            data = json.loads((out / "ViridianCity" / "map.json").read_text())
            trainer = next(n for n in data["npcs"] if n.get("isTrainer"))
            self.assertEqual((trainer["x"], trainer["y"]), (5, 4))

    def test_trainer_shuffle_legal_and_level_capped(self):
        with tempfile.TemporaryDirectory() as root:
            maps_dir, trainers_dir = _write_fixture(root)
            out = _generate(root, "vt", 5, {"shuffle_trainers": {"maps": ["ViridianCity"]}},
                            maps_dir, trainers_dir)
            recs = [r for r in _manifest(out)["mutations"]
                    if r["class"] == "shuffle_trainers"]
            self.assertEqual(len(recs), 1)
            new_class, new_set = recs[0]["to"].split("#")
            self.assertIn(new_class, FIXTURE_TRAINERS)
            parties = FIXTURE_TRAINERS[new_class]["parties"]
            self.assertTrue(1 <= int(new_set) <= len(parties))
            party_max = max(m["level"] for m in parties[int(new_set) - 1]["pokemon"])
            self.assertLessEqual(party_max, 15)  # Brock@99 pool-excluded

    def test_encounter_shuffle_preserves_species_pool(self):
        with tempfile.TemporaryDirectory() as root:
            maps_dir, trainers_dir = _write_fixture(root)
            out = _generate(root, "ve", 3, {"shuffle_encounters": {}},
                            maps_dir, trainers_dir)
            recs = [r for r in _manifest(out)["mutations"]
                    if r["class"] == "shuffle_encounters"]
            self.assertEqual(len(recs), 2)  # Route1 and Route2

            def pool(base, name):
                data = json.loads((base / name / "map.json").read_text())
                return sorted(f"{m['species']}@{m['level']}"
                              for m in data["wild"]["red"]["grass"]["mons"])

            before = pool(maps_dir, "Route1") + pool(maps_dir, "Route2")
            after = pool(out, "Route1") + pool(out, "Route2")
            self.assertEqual(sorted(before), sorted(after))
            # …but at least one map's table changed.
            self.assertNotEqual(pool(maps_dir, "Route1"), pool(out, "Route1"))


class ValidatorTests(unittest.TestCase):
    def test_static_validation_ok_on_fixture_variant(self):
        with tempfile.TemporaryDirectory() as root:
            maps_dir, trainers_dir = _write_fixture(root)
            out = _generate(root, "vv", 77, {"shuffle_warps": {"maps": ["ViridianCity"]},
                                             "shuffle_items": {}},
                            maps_dir, trainers_dir)
            world = {"maps": FIXTURE_WALKABLE, "edges": FIXTURE_EDGES}
            report = vv.validate_static(
                out, world_data=world,
                required_routes=[("PalletTown", "ViridianCity")],
                towns=["ViridianCity"])
            self.assertTrue(report["ok"], [c for c in report["checks"] if not c.ok])
            self.assertEqual(report["routes"]["PalletTown->ViridianCity"],
                             ["PalletTown", "Route1", "ViridianCity"])

    def test_bfs_route_and_unreachable(self):
        adjacency = {"A": {"B"}, "B": {"C"}, "C": set()}
        self.assertEqual(vv._bfs_route(adjacency, "A", "C"), ["A", "B", "C"])
        self.assertIsNone(vv._bfs_route(adjacency, "C", "A"))
        self.assertEqual(vv._bfs_route(adjacency, "A", "A"), ["A"])

    def test_warp_integrity_flags_broken_exit_mats(self):
        with tempfile.TemporaryDirectory() as root:
            maps_dir, _ = _write_fixture(root)
            # Break HouseB's exit mat: it must point at ViridianCity warp 1.
            house = maps_dir / "HouseB" / "map.json"
            data = json.loads(house.read_text())
            data["warps"][0]["destWarpId"] = 0
            house.write_text(json.dumps(data))
            problems = vv.check_warp_integrity(maps_dir)
            self.assertEqual(len(problems), 1)
            self.assertIn("HouseB", problems[0])

    def test_warp_integrity_flags_dangling_destination(self):
        with tempfile.TemporaryDirectory() as root:
            maps_dir, _ = _write_fixture(root)
            vc = maps_dir / "ViridianCity" / "map.json"
            data = json.loads(vc.read_text())
            data["warps"][0]["destWarpId"] = 9
            vc.write_text(json.dumps(data))
            problems = vv.check_warp_integrity(maps_dir)
            self.assertTrue(any("destWarpId" in p for p in problems))


if __name__ == "__main__":
    unittest.main()

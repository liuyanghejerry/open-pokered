"""Unit checks for openpoke M6 (stdlib unittest, no game binary needed)."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from openpoke import tasks as tasks_mod
from openpoke.metrics import RunMetrics
from openpoke.oracle import Oracle, OracleError


MINIMAL_SPEC = {
    "id": "t",
    "name": "T",
    "initial_state": {"warp": "PalletTown,10,6"},
    "seed": 1,
    "goal": {"type": "map", "id": "ViridianCity"},
    "max_steps": 10,
}


class TaskSpecTests(unittest.TestCase):
    def test_loads_all_packaged_tasks(self):
        specs = tasks_mod.load_tasks_dir()
        ids = [t["id"] for t in specs]
        # Sorted by (tier, id).
        self.assertEqual(
            ids,
            ["acquire-potion", "get-starter", "reach-pewter-city",
             "reach-viridian-city", "talk-to-oak", "win-wild-battle",
             "beat-brock", "beat-one-trainer", "get-pokedex"])
        for spec in specs:
            self.assertIn("tier", spec)

    def test_validation_accepts_minimal(self):
        self.assertTrue(tasks_mod.validate_task(dict(MINIMAL_SPEC)))

    def test_validation_rejects_bad_specs(self):
        def bad(**kw):
            spec = json.loads(json.dumps(MINIMAL_SPEC))
            spec.update(kw)
            return spec
        with self.assertRaises(tasks_mod.TaskSpecError):
            tasks_mod.validate_task({"id": "x"})
        with self.assertRaises(tasks_mod.TaskSpecError):
            tasks_mod.validate_task(bad(goal={"type": "moon", "id": "X"}))
        with self.assertRaises(tasks_mod.TaskSpecError):
            tasks_mod.validate_task(bad(goal={"type": "flag"}))
        with self.assertRaises(tasks_mod.TaskSpecError):
            tasks_mod.validate_task(bad(seed="abc"))
        with self.assertRaises(tasks_mod.TaskSpecError):
            tasks_mod.validate_task(bad(max_steps=0))
        with self.assertRaises(tasks_mod.TaskSpecError):
            tasks_mod.validate_task(bad(initial_state={"warp": "A", "save": "B"}))
        with self.assertRaises(tasks_mod.TaskSpecError):
            tasks_mod.validate_task(bad(setup={"flags": {"NOT_A_FLAG": True}}))

    def test_goal_flag_names_match_curated_objectives(self):
        # objectives.json is the verified flag registry from M4.
        objectives_path = (Path(__file__).resolve().parent.parent
                           / "crates/pokered-data/story/objectives.json")
        objectives = json.loads(objectives_path.read_text())
        known = {o["satisfied_when"]["flag"] for o in objectives["objectives"]}
        extra = {"EVENT_OAK_ASKED_TO_CHOOSE_MON", "EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_0"}
        for spec in tasks_mod.load_tasks_dir():
            goal = spec["goal"]
            if goal["type"] == "flag":
                self.assertIn(goal["id"], known | extra, spec["id"])


class MetricsTests(unittest.TestCase):
    def test_round_trip(self):
        with tempfile.TemporaryDirectory() as tmp:
            m = RunMetrics(task_id="demo", seed=7)
            m.start(100)
            m.env_steps = 12
            m.battles = 2
            m.finish(460, True)
            path = m.write(tmp)
            (runs,) = RunMetrics.read_runs(path)
            self.assertEqual(runs["task_id"], "demo")
            self.assertEqual(runs["seed"], 7)
            self.assertTrue(runs["success"])
            self.assertEqual(runs["frames_elapsed"], 360)
            self.assertEqual(runs["battles"], 2)
            self.assertEqual(runs["model_calls"], 0)


class FakeClient:
    """Minimal AgentClient stand-in for oracle decomposition tests."""

    def __init__(self):
        self.calls = []
        self.semantics = {
            "map": "PewterGym",
            "storylines": [
                {"id": "PewterGym:talkGuide", "map": "PewterGym",
                 "storyline": "talkGuide", "triggers": ["npc:3"],
                 "reads": [], "effects": []},
                {"id": "PewterGym:talkBrock", "map": "PewterGym",
                 "storyline": "talkBrock", "triggers": ["npc:1"],
                 "reads": [],
                 "effects": [
                     {"kind": "battle_started",
                      "battle": {"kind": "trainer", "trainer_id": "OPP_BROCK1",
                                 "class": "Brock", "set": 0}},
                     {"kind": "badge_given", "badge": "BOULDERBADGE"},
                     {"kind": "flag_set", "flag": "EVENT_BEAT_BROCK"}]},
            ],
        }
        self.npcs = [
            {"npc_index": 0, "text_id": 1, "visible": True},
            {"npc_index": 1, "text_id": 2, "visible": True},
        ]

    def cmd(self, **kw):
        self.calls.append(kw)
        if kw["cmd"] == "get_script_semantics":
            return self.semantics
        if kw["cmd"] == "get_npcs":
            return self.npcs
        raise AssertionError(f"unexpected call {kw}")

    def world_graph(self, maps=None):
        self.calls.append({"cmd": "get_world_graph", "maps": maps})
        return {"edges": [
            {"kind": "warp", "from_map": "PewterCity", "to_map": "PewterGym",
             "from_pos": {"x": 16, "y": 39}, "warp_index": 0},
        ]}

    def script_semantics(self, map_name=None):
        self.calls.append({"cmd": "get_script_semantics", "map": map_name})
        return self.semantics

    def move_to(self, x, y):
        self.calls.append({"cmd": "move_to", "x": x, "y": y})
        return {"result": "map_changed"}

    def interact_with(self, entity_id):
        self.calls.append({"cmd": "interact_with", "id": entity_id})
        return {"result": "dialogue"}

    def state(self):
        self.calls.append({"cmd": "get_state"})
        return {"dialogue_state": None}

    def flags(self):
        self.calls.append({"cmd": "get_flags"})
        return {"EVENT_BEAT_BROCK": True}

    def skip_dialogue(self):
        self.calls.append({"cmd": "skip_dialogue"})
        return {}


class FakeEnv:
    def __init__(self, client):
        self.client = client
        self.battles = 0
        self.battles_won = 0
        self.steps = []

    def step(self, action):
        self.steps.append(action)
        return None, {"done": False, "success": False}, {}


class OracleDecompositionTests(unittest.TestCase):
    def test_beat_brock_flow_uses_semantics_not_hardcode(self):
        client = FakeClient()
        env = FakeEnv(client)
        oracle = Oracle(env)
        # Patch the battle skill: no live battle in the unit test.
        import openpoke.skills as skills
        orig = skills.fight_current_battle
        skills.fight_current_battle = lambda c, prefer="fight": True
        try:
            _, outcome, info = oracle._beat_brock()
        finally:
            skills.fight_current_battle = orig
        self.assertTrue(outcome["success"])
        self.assertEqual(env.steps, ["travel_to:PewterCity"])
        # The gym warp came from the world graph, not a constant.
        self.assertIn({"cmd": "move_to", "x": 16, "y": 39}, client.calls)
        # Brock's entity was resolved from the storyline's npc trigger.
        self.assertIn({"cmd": "interact_with", "id": "npc:0"}, client.calls)
        self.assertEqual(info["storyline"], "PewterGym:talkBrock")

    def test_oracle_rejects_oracle_false_tasks(self):
        env = FakeEnv(FakeClient())
        oracle = Oracle(env)
        with self.assertRaises(OracleError):
            oracle.run({"id": "get-pokedex", "oracle": False,
                        "goal": {"type": "flag", "id": "EVENT_GOT_POKEDEX"}})


if __name__ == "__main__":
    unittest.main()

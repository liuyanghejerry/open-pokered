"""Unit checks for the RQ2 hierarchical planner (mock env, no game).

Covers: event-graph indexing/producer search, self-loop requires drop,
candidate ordering by world-graph route length, ablation determinism and
fraction, plan building per goal type, no-path failures, and run()
end-to-end over a scripted FakeEnv.
"""
import random
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from openpokered import planner as pl


def edge(kind, frm, to, detail=None):
    e = {"kind": kind, "from": frm, "to": to}
    if detail:
        e["detail"] = detail
    return e


FIXTURE_EDGES = [
    # talkBrock sets the goal flag, self-reads it (loop), fights OPP_BROCK1.
    edge("sets", "script:PewterGym:talkBrock", "flag:EVENT_BEAT_BROCK"),
    edge("requires", "script:PewterGym:talkBrock", "flag:EVENT_BEAT_BROCK"),
    edge("requires", "script:PewterGym:talkBrock", "flag:EVENT_GOT_TM34"),
    edge("sets", "script:PewterGym:talkBrock", "flag:EVENT_GOT_TM34"),
    edge("triggered_at", "script:PewterGym:talkBrock", "map:PewterGym"),
    edge("starts_battle", "script:PewterGym:talkBrock", "trainer:OPP_BROCK1"),
    # Two potion producers: far one with many reads, near one with none.
    edge("gives", "script:MtMoon1F:itemPotion1", "item:POTION", "x1"),
    edge("triggered_at", "script:MtMoon1F:itemPotion1", "map:MtMoon1F"),
    edge("requires", "script:MtMoon1F:itemPotion1", "flag:EVENT_GOT_POTION_MT_MOON_1F_A"),
    edge("gives", "script:Route1:talkYoungster1", "item:POTION", "x1"),
    edge("triggered_at", "script:Route1:talkYoungster1", "map:Route1"),
    # OaksLab @load speech.
    edge("sets", "script:OaksLab:@load", "flag:EVENT_OAK_ASKED_TO_CHOOSE_MON"),
    edge("triggered_at", "script:OaksLab:@load", "map:OaksLab"),
    edge("requires", "script:OaksLab:@load", "flag:EVENT_OAK_APPEARED_IN_PALLET"),
]

BROCK_TASK = {"id": "beat-brock", "name": "Beat Brock",
              "goal": {"type": "flag", "id": "EVENT_BEAT_BROCK"}}
POTION_TASK = {"id": "acquire-potion", "name": "Acquire a Potion",
               "goal": {"type": "item", "id": "Potion"}}
MAP_TASK = {"id": "reach-viridian-city", "name": "Reach Viridian City",
            "goal": {"type": "map", "id": "ViridianCity"}}
WILD_TASK = {"id": "win-wild-battle", "name": "Win a Wild Battle",
             "goal": {"type": "battle_won"}}


class EventGraphTests(unittest.TestCase):
    def setUp(self):
        self.g = pl.EventGraph(FIXTURE_EDGES)

    def test_producers(self):
        self.assertEqual(self.g.producers("sets", "flag:EVENT_BEAT_BROCK"),
                         ["script:PewterGym:talkBrock"])
        self.assertEqual(set(self.g.producers("gives", "item:POTION")),
                         {"script:MtMoon1F:itemPotion1",
                          "script:Route1:talkYoungster1"})
        self.assertEqual(self.g.producers("sets", "flag:DOES_NOT_EXIST"), [])

    def test_storyline_info_drops_self_loop_requires(self):
        info = self.g.storyline_info("script:PewterGym:talkBrock")
        # EVENT_BEAT_BROCK and EVENT_GOT_TM34 are set by the same
        # storyline → dropped as self-loops.
        self.assertEqual(info["requires"], [])
        self.assertEqual(info["triggered_at"], "PewterGym")
        self.assertEqual(info["battles"], ["trainer:OPP_BROCK1"])

    def test_ablation_determinism_and_fraction(self):
        g = pl.EventGraph(FIXTURE_EDGES + [
            edge("shows", f"script:M:m{i}", f"flag:F{i}") for i in range(90)])
        a1 = pl.ablate(g, 0.3, 42)
        a2 = pl.ablate(g, 0.3, 42)
        a3 = pl.ablate(g, 0.3, 777)
        self.assertEqual([e["from"] for e in a1.edges],
                         [e["from"] for e in a2.edges])  # deterministic
        self.assertNotEqual([e["from"] for e in a1.edges],
                            [e["from"] for e in a3.edges])  # seed-varying
        kept = len(a1.edges) / len(g.edges)
        self.assertTrue(0.55 < kept < 0.85)  # ~70% kept, binomial slack

    def test_ablation_can_remove_producers(self):
        # Deleting all 'sets' edges into the goal must surface no_producer.
        only_sets = [e for e in FIXTURE_EDGES
                     if not (e["kind"] == "sets"
                             and e["to"] == "flag:EVENT_BEAT_BROCK")]
        g = pl.EventGraph(only_sets)
        self.assertEqual(g.producers("sets", "flag:EVENT_BEAT_BROCK"), [])


def obs(mode="overworld", x=10, y=10, map_name="PalletTown"):
    return {"mode": mode, "position": {"x": x, "y": y},
            "map": {"name": map_name}, "facing": "Up"}


class FakeClient:
    def __init__(self, route_legs=1, semantics=None, npcs=None, flags=None):
        self._route_legs = route_legs
        self._semantics = semantics or {"storylines": []}
        self._npcs = npcs or []
        self._flags = flags or {}
        self.obs_map = "PalletTown"

    def observe(self, level=None, profile=None):
        return obs(map_name=self.obs_map)

    def route(self, a, b):
        return {"legs": [{"to_map": b}] * self._route_legs}

    def cmd(self, **kw):
        if kw.get("cmd") == "get_npcs":
            return self._npcs
        raise AssertionError(f"unexpected cmd {kw}")

    def flags(self):
        return self._flags

    def script_semantics(self, map_name=None):
        return self._semantics

    def travel_to(self, map_name):
        self.obs_map = map_name
        return {"result": "reached"}

    def skip_dialogue(self):
        return {}

    def wait_until(self, condition, n):
        return {"reached": True}


class FakeEnv:
    def __init__(self, client, steps_done_after=1, success=True, frames_per_step=10):
        self.client = client
        self.actions = []
        self.env_steps = 0
        self.frames = 0
        self.battles = 0
        self.battles_won = 0
        self.invalid_actions = 0
        self.task = None
        self._done_after = steps_done_after
        self._success = success
        self._fps = frames_per_step

    def frame_count(self):
        return self.frames

    def step(self, action):
        self.actions.append(action)
        self.env_steps += 1
        self.frames += self._fps
        done = self.env_steps >= self._done_after
        return (self.client.observe(),
                {"done": done, "success": done and self._success},
                {"invalid": False, "result": {"result": "reached"}})


def _goal_flag(flag):
    return {"type": "flag", "id": flag}


class BuildPlanTests(unittest.TestCase):
    def test_map_goal_is_travel_plan(self):
        env = FakeEnv(FakeClient())
        p = pl.HierarchicalPlanner(pl.EventGraph(FIXTURE_EDGES))
        self.assertEqual(p.build_plan(env, MAP_TASK),
                         [("travel", "ViridianCity")])

    def test_battle_won_goal_is_wild_grass_plan(self):
        env = FakeEnv(FakeClient())
        p = pl.HierarchicalPlanner(pl.EventGraph(FIXTURE_EDGES))
        self.assertEqual(p.build_plan(env, WILD_TASK),
                         [("wild_grass", "Route1")])

    def test_no_producer_raises(self):
        env = FakeEnv(FakeClient())
        p = pl.HierarchicalPlanner(pl.EventGraph(FIXTURE_EDGES))
        task = {"id": "x", "goal": _goal_flag("EVENT_NOT_THERE")}
        with self.assertRaises(pl.PlanError) as ctx:
            p.build_plan(env, task)
        self.assertIn("no_path:no_producer", str(ctx.exception))

    def test_candidates_ordered_by_route_length(self):
        client = FakeClient(route_legs=5)
        env = FakeEnv(client)
        p = pl.HierarchicalPlanner(pl.EventGraph(FIXTURE_EDGES))
        steps = p.build_plan(env, POTION_TASK)
        # Route1 (1 leg away? client returns 5 for all → tie broken by
        # requires count: Route1 has 0 requires < MtMoon's 1)
        self.assertEqual(steps[0][1]["node"], "script:Route1:talkYoungster1")
        self.assertEqual(steps[1][1]["node"], "script:MtMoon1F:itemPotion1")


class RunTests(unittest.TestCase):
    def test_map_goal_runs_travel(self):
        env = FakeEnv(FakeClient(), steps_done_after=1)
        env.task = MAP_TASK
        p = pl.HierarchicalPlanner(pl.EventGraph(FIXTURE_EDGES))
        ok, reason = p.run(env, MAP_TASK, frame_budget=100)
        self.assertTrue(ok)
        self.assertEqual(env.actions, ["travel_to:ViridianCity"])

    def test_no_producer_is_graceful(self):
        env = FakeEnv(FakeClient())
        env.task = {"id": "x", "goal": _goal_flag("EVENT_NOT_THERE")}
        p = pl.HierarchicalPlanner(pl.EventGraph(FIXTURE_EDGES))
        ok, reason = p.run(env, env.task, frame_budget=100)
        self.assertFalse(ok)
        self.assertEqual(reason, "no_path:no_producer:flag:EVENT_NOT_THERE")
        self.assertEqual(env.actions, [])  # nothing executed

    def test_frame_budget_failure(self):
        # 40 frames per step vs a 30-frame budget: the post-step budget
        # check must end the run with frame_budget reason.
        env = FakeEnv(FakeClient(), steps_done_after=10**9, frames_per_step=40)
        env.task = MAP_TASK
        p = pl.HierarchicalPlanner(pl.EventGraph(FIXTURE_EDGES))
        ok, reason = p.run(env, MAP_TASK, frame_budget=30)
        self.assertFalse(ok)
        self.assertEqual(reason, "frame_budget")

    def test_storyline_happy_path_brock(self):
        sem = {"storylines": [{
            "id": "PewterGym:talkBrock",
            "triggers": ["npc:1"],
            "effects": [{"kind": "battle_started"}]}]}
        client = FakeClient(semantics=sem,
                            npcs=[{"text_id": 1, "npc_index": 0, "visible": True}],
                            flags={"EVENT_BEAT_BROCK": True})
        client.obs_map = "PewterGym"
        # goal_satisfied via flags: patch env task flag check through
        # goal_satisfied's env.flags() path — FakeEnv needs flags().
        env = FakeEnv(client, steps_done_after=1)
        env.task = BROCK_TASK
        env.flags = lambda: client._flags
        p = pl.HierarchicalPlanner(pl.EventGraph(FIXTURE_EDGES))
        ok, reason = p.run(env, BROCK_TASK, frame_budget=200)
        self.assertTrue(ok, f"reason={reason}")
        self.assertIn("interact_with:npc:0", env.actions)


if __name__ == "__main__":
    unittest.main()

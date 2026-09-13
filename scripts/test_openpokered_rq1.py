"""Unit checks for openpokered RQ1 tier policies (stdlib unittest, no game).

FakeClient/FakeEnv mirror scripts/test_openpokered.py: observation feeds are
scripted, actions are recorded, no process is spawned.
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from openpokered import skills
from openpokered.policies import ButtonRandomWalk, LocalExplorer


def obs(mode="overworld", x=10, y=10, map_name="PalletTown"):
    return {"mode": mode, "position": {"x": x, "y": y},
            "map": {"name": map_name}}


class FakeClient:
    def __init__(self, obss=None, entities=None):
        self.obss = list(obss) if obss else None
        self.entities = entities if entities is not None else []
        self.skips = 0

    def observe(self):
        if self.obss:
            return self.obss[0]
        return obs()

    def advance(self):
        if self.obss and len(self.obss) > 1:
            self.obss.pop(0)

    def nearby(self, radius=None):
        return {"entities": self.entities}

    def skip_dialogue(self):
        self.skips += 1
        return {}


class FakeEnv:
    """Records actions, advances the scripted observation per step."""

    def __init__(self, client, step_result="reached", move_delta=None):
        self.client = client
        self.actions = []
        self.env_steps = 0
        self.frames = 0
        self.step_result = step_result
        # Optional (dx, dy) applied to the scripted obs position on move_to
        # (None = position frozen → reads as "bumped into a wall").
        self.move_delta = move_delta

    def frame_count(self):
        return self.frames

    def step(self, action):
        self.actions.append(action)
        self.env_steps += 1
        self.frames += 10
        cur = self.client.observe()
        if action.startswith("move_to") and self.move_delta is not None:
            cur["position"]["x"] += self.move_delta[0]
            cur["position"]["y"] += self.move_delta[1]
        self.client.advance()
        return (self.client.observe(), {"done": False, "success": False},
                {"invalid": False, "result": {"result": self.step_result}})


REACH_TASK = {"id": "reach-viridian-city", "goal": {"type": "map", "id": "ViridianCity"}}
POTION_TASK = {"id": "acquire-potion", "goal": {"type": "item", "id": "Potion"}}
OAK_TASK = {"id": "talk-to-oak", "goal": {"type": "flag", "id": "EVENT_OAK_ASKED_TO_CHOOSE_MON"}}


class ButtonRandomWalkTests(unittest.TestCase):
    def test_seeded_determinism_and_north_bias(self):
        def run():
            env = FakeEnv(FakeClient())
            policy = ButtonRandomWalk(42)
            policy.run(env, REACH_TASK, frame_budget=600)
            return env.actions

        first, second = run(), run()
        self.assertEqual(first, second)  # same seed → identical action stream
        ups = sum(1 for a in first if a.startswith("drive:up"))
        downs = sum(1 for a in first if a.startswith("drive:down"))
        self.assertGreater(ups, downs)  # the bias actually biases
        self.assertTrue(all(a.startswith(("drive:", "press:", "step_frames:"))
                            for a in first))  # buttons only, never move_to

    def test_battle_mashes_a_and_counts_once(self):
        feed = [obs("overworld"), obs("battle"), obs("battle"),
                obs("overworld"), obs("overworld")]
        env = FakeEnv(FakeClient(obss=feed))
        policy = ButtonRandomWalk(42)
        policy.run(env, REACH_TASK, frame_budget=200)
        self.assertEqual(policy.battles, 1)
        self.assertEqual(policy.battles_won, 1)
        self.assertIn("press:a", env.actions)


class LocalExplorerTests(unittest.TestCase):
    def test_seeded_determinism(self):
        def run():
            env = FakeEnv(FakeClient(), move_delta=(0, -2))
            policy = LocalExplorer(42)
            policy.run(env, REACH_TASK, frame_budget=400)
            return env.actions

        self.assertEqual(run(), run())

    def test_visible_item_target_acquired_directly(self):
        client = FakeClient(entities=[
            {"id": "hidden:3", "kind": "hidden_item", "name": "Potion",
             "x": 5, "y": 5, "interactable": True}])
        env = FakeEnv(client)
        policy = LocalExplorer(42)
        policy.run(env, POTION_TASK, frame_budget=100)
        self.assertEqual(env.actions[0], "interact_with:hidden:3")

    def test_npc_hint_target_from_task_id(self):
        client = FakeClient(entities=[
            {"id": "npc:1", "kind": "npc", "name": "Oak",
             "x": 6, "y": 4, "interactable": True}])
        env = FakeEnv(client)
        policy = LocalExplorer(42)
        policy.run(env, OAK_TASK, frame_budget=100)
        self.assertEqual(env.actions[0], "interact_with:npc:1")

    def test_compass_walk_heads_north(self):
        env = FakeEnv(FakeClient(), move_delta=(0, -2))
        policy = LocalExplorer(42)
        policy.run(env, REACH_TASK, frame_budget=50)
        move = next(a for a in env.actions if a.startswith("move_to"))
        _, arg = move.split(":")
        _, cy = (int(v) for v in arg.split(","))
        self.assertLess(cy, 10)  # compass target decreases y (north)

    def test_edge_burst_near_north_edge(self):
        env = FakeEnv(FakeClient(obss=[obs(y=3)]))
        policy = LocalExplorer(42)
        policy.run(env, REACH_TASK, frame_budget=30)
        self.assertIn("drive:up,12", env.actions)

    def test_bump_marks_blocked_and_repicks(self):
        # Frozen position + non-progress result reads as a wall bump.
        env = FakeEnv(FakeClient(), step_result="blocked", move_delta=None)
        policy = LocalExplorer(42)
        policy.run(env, REACH_TASK, frame_budget=120)
        moves = [a for a in env.actions if a.startswith("move_to")]
        self.assertGreaterEqual(len(moves), 2)
        self.assertNotEqual(moves[0], moves[1])  # never retried the bump
        self.assertTrue(policy.blocked)

    def test_item_goal_tries_building_warps_nearest_first(self):
        client = FakeClient(entities=[
            {"id": "warp:0", "kind": "warp", "name": "FarHouse",
             "position": {"x": 2, "y": 2}},
            {"id": "warp:1", "kind": "warp", "name": "NearHouse",
             "position": {"x": 9, "y": 9}}])
        env = FakeEnv(client)
        policy = LocalExplorer(42)
        policy.run(env, POTION_TASK, frame_budget=100)
        self.assertEqual(env.actions[0], "move_to:9,9")  # nearest, not named

    def test_battle_skill_used_and_counted_once(self):
        feed = [obs("overworld"), obs("battle"), obs("overworld")]
        client = FakeClient(obss=feed)
        env = FakeEnv(client)
        calls = []
        orig = skills.fight_current_battle

        def fight(c, prefer="fight"):
            calls.append(prefer)
            env.frames += 50  # the real skill steps frames while fighting
            client.obss = [o for o in client.obss if o["mode"] != "battle"]
            return True

        skills.fight_current_battle = fight
        try:
            policy = LocalExplorer(42)
            policy.run(env, REACH_TASK, frame_budget=100)
        finally:
            skills.fight_current_battle = orig
        self.assertEqual(len(calls), 1)
        self.assertEqual(policy.battles, 1)
        self.assertEqual(policy.battles_won, 1)


if __name__ == "__main__":
    unittest.main()

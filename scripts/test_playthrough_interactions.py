"""Regressions for real-input NPC dialogue / battle handoff ordering."""
import unittest
from types import SimpleNamespace

from playthrough_late import talk_npc, talk_object


class DeferredTrainer:
    """A trainer whose closed text is promoted by the next game update."""
    def __init__(self):
        self.flags = {}
        self.state = {"screen": "overworld", "dialogue_state": None,
                      "script_running": False, "active_script_effect": None}
        self.npc = {"text_id": 4, "x": 11, "y": 2, "visible": True}
        self.awaiting_promotion = False
        self.battles = 0
        self.taps = []
        self.approaches = []
        self.d = SimpleNamespace(cmd=self.command)

    def command(self, **kwargs):
        if kwargs["cmd"] == "get_npcs":
            return {"data": [dict(self.npc)]}
        if kwargs["cmd"] == "get_flags":
            return {"data": dict(self.flags)}
        raise AssertionError(kwargs)

    def approach_object(self, x, y, name):
        self.approaches.append((x, y, name))

    def tap(self, button, gap):
        self.taps.append(button)
        self.state["dialogue_state"] = {"page": 0}

    def cutscene(self):
        if self.state["dialogue_state"] is not None:
            self.state["dialogue_state"] = None
            self.awaiting_promotion = True
        # This is the actual debug control_ready boundary: text has closed,
        # but the engine has not yet run its pending trainer promotion.
        return True

    def step(self, frames):
        if self.awaiting_promotion:
            self.awaiting_promotion = False
            self.state["screen"] = "battle"

    def st(self):
        return dict(self.state)

    def battle_loop(self):
        self.battles += 1
        self.state["screen"] = "overworld"
        self.flags["BEAT_ROCKET"] = True


class InteractionRegression(unittest.TestCase):
    def test_trainer_text_closed_before_battle_is_not_completion(self):
        g = DeferredTrainer()
        talk_npc(g, "RocketHideoutB4F", 4, completion_flag="BEAT_ROCKET")
        self.assertEqual(g.battles, 1)
        self.assertTrue(g.flags["BEAT_ROCKET"])
        self.assertEqual(g.taps, ["a"])

    def test_fixed_coordinate_trainer_also_drains_deferred_handoff(self):
        g = DeferredTrainer()
        talk_object(g, "GameCorner", 11, 2)
        self.assertEqual(g.battles, 1)
        self.assertEqual(g.state["screen"], "overworld")

    def test_on_step_victory_can_hide_npc_before_second_observation(self):
        g = DeferredTrainer()
        def approach(x, y, name):
            g.flags["BEAT_ROCKET"] = True
            g.npc["visible"] = False
        g.approach_object = approach
        talk_npc(g, "RocketHideoutB4F", 4, completion_flag="BEAT_ROCKET")
        self.assertEqual(g.taps, [])

    def test_trainer_moved_by_sight_is_reapproached_at_actual_position(self):
        g = DeferredTrainer()
        original_approach = g.approach_object
        def approach(x, y, name):
            original_approach(x, y, name)
            if len(g.approaches) == 1:
                g.npc["y"] = 3
        g.approach_object = approach
        talk_npc(g, "RocketHideoutB4F", 4, completion_flag="BEAT_ROCKET")
        self.assertEqual(g.approaches, [(11, 2, "RocketHideoutB4F"),
                                        (11, 3, "RocketHideoutB4F")])
        self.assertEqual(g.taps, ["a"])

    def test_no_response_never_passes_a_required_story_event(self):
        g = DeferredTrainer()
        g.tap = lambda button, gap: g.taps.append(button)
        with self.assertRaisesRegex(RuntimeError, "NPC 4"):
            talk_npc(g, "RocketHideoutB4F", 4, completion_flag="BEAT_ROCKET")
        self.assertEqual(g.taps, ["a"] * 4)
        self.assertEqual(g.battles, 0)
        self.assertNotIn("BEAT_ROCKET", g.flags)


if __name__ == "__main__":
    unittest.main()

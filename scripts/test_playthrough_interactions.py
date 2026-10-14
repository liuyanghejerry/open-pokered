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

class ApproachInterruptionRegression(unittest.TestCase):
    def test_wild_encounter_during_facing_requires_another_approach(self):
        import playthrough as pt
        from unittest.mock import patch
        class Game(pt.Game):
            def __init__(self):
                self.state = {'screen':'overworld','map_name':'PokemonTower6F',
                              'player_x':6,'player_y':7,'player_facing':'Down',
                              'dialogue_state':None,'script_awaiting_battle':False}
                self.faces = 0
                self.battles = []
            def st(self): return dict(self.state)
            def live_npcs(self, name): return {(6,8)}
            def nav_to(self, x, y, name):
                self.state.update(player_x=x,player_y=y)
            def face(self, direction):
                self.faces += 1
                self.state.update(player_facing=direction.title())
                if self.faces == 1:self.state['screen']='battle'
            def battle_loop(self, prefer):
                self.battles.append(prefer);self.state['screen']='overworld'
            def cutscene(self): return True
            def step(self, frames): pass
        g = Game()
        # Only the object-adjacent tile is reachable in this corridor.
        def path(name,start,goal,*args,**kwargs):
            return [(start,None)] if goal==(6,7) else None
        with patch.object(pt,'bfs',side_effect=path),patch.object(pt,'warp_tiles',return_value=set()):
            g.approach_object(6,8,'PokemonTower6F')
        self.assertEqual(g.battles,['run'])
        self.assertEqual(g.faces,2)
        self.assertEqual(g.state['screen'],'overworld')


class DestinationTrainerRegression(unittest.TestCase):
    def trainer(self, *, script_only=False, leave_map=False, choice=False, silent_intro=False):
        import playthrough as pt
        class Game(pt.Game):
            def __init__(self):
                self.state = {"screen": "overworld", "map_name": "RocketHideoutB4F",
                              "player_x": 12, "player_y": 3, "player_facing": "Left",
                              "dialogue_state": None, "active_script_effect": None,
                              "script_running": False, "script_awaiting_battle": False}
                self.events = []
                self.arrivals = 0
                self.promote = False
                self.intro = False
                self.flags = {}
            def st(self): return dict(self.state)
            def live_npcs(self, name): return {(11, 2)}
            def nav_to(self, x, y, name):
                self.arrivals += 1
                self.state.update(player_x=x, player_y=y)
                if self.arrivals == 1:
                    self.state.update(dialogue_state=None if script_only else {"text": "The elevator"},
                                      active_script_effect="ShowDialogue", script_running=True)
                    if silent_intro:
                        self.state.update(dialogue_state=None, active_script_effect=None,
                                          script_running=False)
                        self.intro = True
                    self.events.append("trainer sight")
            def cutscene(self):
                self.events.append("cutscene")
                if choice:
                    raise pt.NavError("cutscene blocked on choice ['YES', 'NO']")
                if self.state["script_running"]:
                    self.state.update(dialogue_state=None, active_script_effect=None, script_running=False)
                    self.promote = True
                return True
            def step(self, frames):
                self.events.append("update")
                if self.intro:
                    if frames >= 64:
                        self.intro = False
                        self.state.update(dialogue_state={"text": "The elevator"},
                                          active_script_effect="ShowDialogue", script_running=True)
                elif self.promote:
                    self.promote = False
                    self.state["screen"] = "battle"
            def battle_loop(self, prefer):
                self.events.append("battle")
                self.flags["BEAT_ROCKET"] = True
                self.state["screen"] = "overworld"
                if leave_map: self.state["map_name"] = "PalletTown"
            def face(self, direction):
                assert self.state["screen"] == "overworld"
                assert not self.state["script_running"]
                self.events.append("face")
                self.state["player_facing"] = direction.title()
        return Game()

    def approach(self, game):
        import playthrough as pt
        from unittest.mock import patch
        def path(name, start, goal, *args, **kwargs):
            return [(start, None), (goal, "left")] if goal == (11, 3) else None
        with patch.object(pt, "bfs", side_effect=path), patch.object(pt, "warp_tiles", return_value=set()):
            game.approach_object(11, 2, "RocketHideoutB4F")

    def test_destination_sight_dialogue_finishes_battle_before_facing(self):
        g = self.trainer()
        self.approach(g)
        self.assertTrue(g.flags["BEAT_ROCKET"])
        self.assertLess(g.events.index("battle"), g.events.index("face"))
        self.assertEqual(g.events.count("face"), 1)
        self.assertEqual((g.state["map_name"], g.state["player_x"], g.state["player_y"],
                          g.state["player_facing"]), ("RocketHideoutB4F", 11, 3, "Up"))

    def test_silent_sight_intro_exposes_dialogue_before_any_facing_input(self):
        g = self.trainer(silent_intro=True)
        self.approach(g)
        self.assertTrue(g.flags["BEAT_ROCKET"])
        self.assertLess(g.events.index("battle"), g.events.index("face"))
        self.assertEqual(g.events.count("face"), 1)

    def test_pending_script_without_text_is_settled_before_facing(self):
        g = self.trainer(script_only=True)
        self.approach(g)
        self.assertTrue(g.flags["BEAT_ROCKET"])
        self.assertLess(g.events.index("update"), g.events.index("battle"))
        self.assertLess(g.events.index("battle"), g.events.index("face"))

    def test_blackout_during_destination_battle_keeps_map_assertion(self):
        import playthrough as pt
        g = self.trainer(leave_map=True)
        with self.assertRaisesRegex(pt.NavError, "left RocketHideoutB4F -> PalletTown"):
            self.approach(g)
        self.assertNotIn("face", g.events)

    def test_destination_choice_is_not_accepted_automatically(self):
        import playthrough as pt
        g = self.trainer(choice=True)
        with self.assertRaisesRegex(pt.NavError, "blocked on choice"):
            self.approach(g)
        self.assertNotIn("battle", g.events)
        self.assertNotIn("face", g.events)

    def settled_game(self):
        g = self.trainer()
        g.nav_to = lambda x, y, name: g.state.update(player_x=x, player_y=y)
        return g

    def test_neutral_settlement_map_change_keeps_map_assertion(self):
        import playthrough as pt
        g = self.settled_game()
        g.step = lambda frames: g.state.update(map_name="PalletTown")
        with self.assertRaisesRegex(pt.NavError, "left RocketHideoutB4F -> PalletTown"):
            self.approach(g)
        self.assertNotIn("face", g.events)

    def test_npc_disappearing_during_settlement_returns_to_completion_owner(self):
        g = self.settled_game()
        def update(frames):
            g.flags["BEAT_ROCKET"] = True
            g.live_npcs = lambda name: set()
        g.step = update
        self.approach(g)
        self.assertTrue(g.flags["BEAT_ROCKET"])
        self.assertNotIn("face", g.events)

    def test_npc_moving_during_settlement_returns_for_live_reobservation(self):
        g = self.settled_game()
        g.step = lambda frames: setattr(g, "live_npcs", lambda name: {(12, 2)})
        self.approach(g)
        self.assertNotIn("face", g.events)
        self.assertEqual(g.live_npcs("RocketHideoutB4F"), {(12, 2)})

    def test_player_displacement_replans_before_facing_the_object(self):
        g = self.settled_game()
        arrivals = []
        def navigate(x, y, name):
            arrivals.append((x, y))
            g.state.update(player_x=x, player_y=y)
        g.nav_to = navigate
        updates = []
        def update(frames):
            updates.append(frames)
            if len(updates) == 1:
                g.state.update(player_x=12, player_y=3)
        g.step = update
        self.approach(g)
        self.assertEqual(arrivals, [(11, 3), (11, 3)])
        self.assertEqual(g.events.count("face"), 1)
        self.assertEqual((g.state["player_x"], g.state["player_y"],
                          g.state["player_facing"]), (11, 3, "Up"))

    def test_choice_opening_during_neutral_settlement_stops_without_facing(self):
        import playthrough as pt
        g = self.settled_game()
        g.step = lambda frames: g.state.update(choice={"options": ["YES", "NO"]})
        def blocked():
            raise pt.NavError("cutscene blocked on choice ['YES', 'NO']")
        g.cutscene = blocked
        with self.assertRaisesRegex(pt.NavError, "blocked on choice"):
            self.approach(g)
        self.assertNotIn("face", g.events)

    def test_destination_cutscene_timeout_is_a_failure(self):
        import playthrough as pt
        g = self.trainer()
        g.cutscene = lambda: False
        with self.assertRaisesRegex(pt.NavError, "cutscene did not finish"):
            self.approach(g)
        self.assertNotIn("face", g.events)


if __name__ == "__main__":
    unittest.main()

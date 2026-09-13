"""Regression cases exposed by real post-Brock playthroughs (stdlib unittest)."""
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import subprocess
import sys
from pathlib import Path

import playthrough as nav
from playthrough_late import damage_slot


class NavigationRegression(unittest.TestCase):
    def test_short_live_map_has_no_walkable_missing_row(self):
        name = "UndergroundPathNorthSouth"
        data = dict(nav.MAPS[name], blocks=nav.MAPS[name]["blocks"][:92])
        with patch.dict(nav.MAPS, {name: data}):
            self.assertIsNone(nav.tile_at(name, 0, 46))
            self.assertFalse(nav.walkable(name, 0, 46))
            nav.grass_tiles(name)  # Full declared extent must be safe.

    def test_partial_walk_replans_before_turning(self):
        game = nav.Game.__new__(nav.Game)
        state = dict(screen="overworld", map_name="Route1", player_x=10, player_y=10)
        game.st = lambda: state.copy()
        game.pos = lambda: ("Route1", state["player_x"], state["player_y"])
        game.track_last_map = lambda _: None
        game.last_map = "Route1"
        game.npc_blocked = game.live_npcs = lambda _: set()
        directions = []

        def drive(buttons, frames):
            direction = buttons[0]
            directions.append(direction)
            tiles = len(buttons) // nav.FRAMES_PER_TILE
            if len(directions) == 1:
                tiles -= 1  # One tile was lost to an input lock or NPC.
            dx, dy = nav.DELTA[direction]
            state["player_x"] += dx * tiles
            state["player_y"] += dy * tiles

        def path(map_name, start, goal_map, goal, **kwargs):
            x, y = start
            result = [(map_name, x, y)]
            while y > goal[1]:
                y -= 1
                result.append(((map_name, x, y), "up"))
            while x < goal[0]:
                x += 1
                result.append(((map_name, x, y), "right"))
            return result

        game.d = SimpleNamespace(drive=drive, step=lambda _: None)
        with patch.object(nav, "bfs_cross", side_effect=path):
            game.nav_to_map(11, 7, "Route1", avoid_grass=False)
        self.assertEqual(directions, ["up", "up", "right"])

    def test_cross_map_no_path_rechecks_for_battle_transition(self):
        game = nav.Game.__new__(nav.Game)
        overworld = dict(screen="overworld", map_name="Route9",
                         player_x=53, player_y=9, dialogue_state=None)
        battle = dict(overworld, screen="battle",
                      script_awaiting_battle=False)
        destination = dict(screen="overworld", map_name="LavenderTown",
                           player_x=3, player_y=6, dialogue_state=None)
        states = iter((overworld, battle, battle, destination))
        game.st = lambda: next(states)
        game.track_last_map = lambda _: None
        game.last_map = "Route9"
        game.npc_blocked = game.live_npcs = lambda _: set()
        game.battle_loop = lambda prefer: None
        game.cutscene = lambda: True
        game.step = lambda _: None
        game.d = SimpleNamespace()

        with patch.object(nav, "bfs_cross", return_value=None):
            game.nav_to_map(3, 6, "LavenderTown", avoid_grass=False)

    def test_destination_occupied_by_live_npc_is_temporary(self):
        game = nav.Game.__new__(nav.Game)
        game.live_npcs = lambda _: {(11, 6)}

        self.assertTrue(game.destination_blocked_by_live_npc(
            "Route4", "Route4", 11, 6))
        self.assertFalse(game.destination_blocked_by_live_npc(
            "Route4", "LavenderTown", 11, 6))

    def test_directional_warp_retries_after_interrupted_hold(self):
        game = nav.Game.__new__(nav.Game)
        approaches = []
        game.nav_to = lambda *args, **kwargs: approaches.append(args)
        game.d = SimpleNamespace(drive=lambda *args, **kwargs: None)
        game._wait_for_warp = unittest.mock.Mock(
            side_effect=(None, "Route10"))

        result = game.nav_warp(15, 33, "RockTunnel1F", "Route10",
                               approach="down")

        self.assertEqual(result, "Route10")
        self.assertEqual(approaches, [(15, 32), (15, 32)])

    def test_warp_wait_handles_battle_before_its_placeholder_map(self):
        game = nav.Game.__new__(nav.Game)
        game.st = lambda: {
            "screen": "battle", "map_name": "PalletTown",
            "script_awaiting_battle": False,
        }
        calls = []
        game.battle_loop = lambda prefer: calls.append(prefer)
        game.cutscene = lambda: True

        self.assertIsNone(game._wait_for_warp("RockTunnel1F", "Route10"))
        self.assertEqual(calls, ["run"])

    def test_warp_does_not_replan_against_a_map_left_by_blackout(self):
        game = nav.Game.__new__(nav.Game)
        game.nav_to = unittest.mock.Mock(side_effect=nav.NavError("blackout"))
        game.pos = lambda: ("LavenderTown", 3, 6)

        with self.assertRaisesRegex(nav.NavError, "blackout"):
            game.nav_warp(3, 9, "PokemonTower4F", "PokemonTower5F")
        game.nav_to.assert_called_once()

    def test_locked_saffron_routes_recovery_through_underground(self):
        path = nav.bfs_cross("CeruleanCity", (19, 18), "VermilionCity", (11, 4),
                             allow_ledges=True, excluded_maps=("SaffronCity",))
        self.assertIsNotNone(path)
        maps = {node[0] for node, _ in path[1:]}
        self.assertNotIn("SaffronCity", maps)
        self.assertIn("UndergroundPathNorthSouth", maps)

    def test_script_and_late_helpers_share_the_live_map_cache(self):
        program = """
import contextlib, importlib, io, runpy, sys
sys.argv = ['playthrough', '--list']
with contextlib.redirect_stdout(io.StringIO()):
    driver = runpy.run_module('playthrough', run_name='__main__', alter_sys=True)
assert importlib.import_module('playthrough').MAPS is driver['MAPS']
"""
        subprocess.run([sys.executable, "-c", program],
                       cwd=Path(__file__).resolve().parent, check=True)

    def test_moon_floor_transition_is_not_just_a_passable_tile(self):
        self.assertTrue(nav.walkable("MtMoon1F", 9, 22))
        self.assertFalse(nav.walkable_edge("MtMoon1F", (10, 22), (9, 22)))

    def test_route_nine_ledge_is_one_way(self):
        self.assertEqual(nav.ledge_step("Route9", 10, 10, "down"), ("Route9", 10, 12))
        self.assertIsNone(nav.ledge_step("Route9", 10, 12, "up"))

    def test_spinner_lands_on_stop_tile(self):
        path = nav.bfs("RocketHideoutB3F", (9, 13), (14, 13), allow_spinners=True)
        self.assertEqual(path, [((9, 13), None), ((14, 13), "spin_right_4")])

    def test_high_critical_leaf_is_preferred_against_alakazam(self):
        moves = [{"move": name, "pp": 10, "disabled": False}
                 for name in ["Cut", "RazorLeaf"]]
        state = {"battle_live": {
            "player": {"species": "Venusaur", "hp": 155, "max_hp": 155},
            "enemy": {"species": "Alakazam"},
        }}
        self.assertEqual(damage_slot(moves, state), 1)


if __name__ == "__main__":
    unittest.main()

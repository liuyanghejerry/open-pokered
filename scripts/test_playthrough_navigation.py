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
    def test_grass_detour_returning_to_current_map_uses_stable_fallback(self):
        game = nav.Game.__new__(nav.Game)
        state = {'screen': 'overworld', 'map_name': 'Route16', 'player_x': 15, 'player_y': 13}
        game.st = lambda: state.copy()
        game.last_map = 'Route16'
        game.npc_blocked = game.live_npcs = lambda _: set()
        moved = []
        def drive(buttons, frames):
            moved.append(buttons[0])
            state['player_y'] = 12
        game.d = SimpleNamespace(drive=drive, step=lambda _: None)
        preferred = [('Route16', 15, 13), (('Route16Gate1F', 1, 1), 'left'),
                     (('Route16', 15, 12), 'right')]
        fallback = [('Route16', 15, 13), (('Route16', 15, 12), 'up')]
        with patch.object(nav, 'bfs_cross', side_effect=[preferred, fallback]) as search:
            game.nav_to_map(15, 12, 'Route16', tries=2)
        self.assertEqual(search.call_count, 2)
        self.assertEqual(moved, ['up'])

    def test_border_overshoot_recovers_inside_before_crossing_connection(self):
        self.assertIsNone(nav.cross_step('Route17', 10, 144, 'down'))
        path = nav.bfs_cross('Route17', (10, 144), 'Route18', (10, 0))
        self.assertEqual(path[1], (('Route17', 10, 143), 'up'))
        self.assertEqual(path[2], (('Route18', 10, 0), 'down'))

    def test_slope_brakes_idle_frames_and_respects_uphill_bike_speed(self):
        state = {'map_name': 'Route17', 'player_transport': 'Biking'}
        self.assertEqual(nav.movement_frames(state, 'down'), 4)
        self.assertEqual(nav.movement_frames(state, 'left'), 8)
        self.assertEqual(nav.movement_buttons(state, 'left', 8, 12), ['left']*8 + ['b']*4)
        game = nav.Game.__new__(nav.Game)
        game.smart_moves = True
        game.st = lambda: {**state, 'screen': 'overworld', 'script_running': False}
        calls = []
        game.d = SimpleNamespace(drive=lambda buttons, frames: calls.append((buttons, frames)))
        game.step(6)
        self.assertEqual(calls, [(['b']*6, 6)])

    def test_bike_navigation_does_not_overshoot_a_one_tile_turn(self):
        game = nav.Game.__new__(nav.Game)
        state = {'screen': 'overworld', 'map_name': 'Route16', 'player_x': 15, 'player_y': 13,
                 'player_transport': 'Biking'}
        game.st = lambda: state.copy()
        game.last_map = 'Route16'
        game.npc_blocked = game.live_npcs = lambda _: set()
        holds = []
        def drive(buttons, frames):
            holds.append(len(buttons))
            dx, dy = nav.DELTA[buttons[0]]
            state['player_x'] += dx * (len(buttons)//4)
            state['player_y'] += dy * (len(buttons)//4)
        game.d = SimpleNamespace(drive=drive, step=lambda _: None)
        game.nav_to_map(15, 12, 'Route16', tries=2, avoid_grass=False)
        self.assertEqual(holds, [4])

    def test_cross_map_planner_searches_alternative_goals_in_one_pass(self):
        goals = {('PalletTown', 10, 10), ('PalletTown', 10, 12)}
        path = nav.bfs_cross('PalletTown', (10, 11), 'PalletTown', (10, 10),
                             blocked_maps={'PalletTown': {(10, 10)}}, goal_nodes=goals)
        self.assertEqual(path[-1][0], ('PalletTown', 10, 12))
        self.assertEqual(len(path), 2)
        self.assertIsNone(nav.bfs_cross('PalletTown', (10, 11), 'PalletTown', (10, 10), goal_nodes=set()))

    def test_cross_map_planner_respects_forced_spinner_endpoint(self):
        name = 'RocketHideoutB3F'
        point = (10, 13)
        endpoint, count = nav.SPINNERS[name][point]
        path = nav.bfs_cross(name, (10, 12), name, endpoint, allow_spinners=True)
        self.assertEqual(path, [(name, 10, 12), ((name, *endpoint), f'spin_down_{count}')])
        blocked = nav.bfs_cross(name, (10, 12), name, endpoint, allow_spinners=True,
                                blocked_maps={name: {endpoint}})
        self.assertIsNone(blocked)

    def test_ship_port_name_normalization_preserves_gangplank_warp(self):
        self.assertTrue(nav.warp_triggers('VermilionDock', 14, 2, 'down'))
        target = nav.warp_edges_from('VermilionDock', 14, 2, 'VermilionCity')[0]
        path = nav.bfs_cross('VermilionDock', (14, 1), target[0], target[1:], last_map='VermilionCity')
        self.assertEqual(path, [('VermilionDock', 14, 1), (target, 'down')])

    def test_static_elevator_placeholder_exit_has_no_warp_destination(self):
        for x in (1, 2):
            self.assertEqual(nav.warp_edges_from('SilphCoElevator', x, 3, 'SilphCo1F'), [])
        self.assertIsNone(nav.bfs_cross('SilphCoElevator', (1, 2),
                                       'UnusedMapED', (0, 0), last_map='SilphCo1F'))

    def test_carpet_crossing_keeps_direction_held_after_the_turn_frame(self):
        game = nav.Game.__new__(nav.Game)
        state = {'screen': 'overworld', 'map_name': 'Route7Gate', 'player_x': 4, 'player_y': 3}
        game.st = lambda: state.copy()
        game.last_map = 'Route7'
        game.npc_blocked = game.live_npcs = lambda _: set()
        holds = []
        def drive(buttons, frames):
            holds.append(len(buttons))
            if len(buttons) > nav.FRAMES_PER_TILE:
                state.update(map_name='Route7', player_x=18, player_y=9)
            else:
                state.update(player_x=5, player_y=3)
        game.d = SimpleNamespace(drive=drive, step=lambda _: None)
        game.nav_to_map(18, 9, 'Route7', tries=2, avoid_grass=False)
        self.assertEqual(len(holds), 1)

    def test_side_gate_uses_the_engine_directional_carpet_rule(self):
        self.assertFalse(nav.warp_triggers('Route8', 8, 10, 'down'))
        self.assertTrue(nav.warp_triggers('Route8', 8, 10, 'left'))
        self.assertTrue(nav.warp_triggers('Route12', 10, 15, 'down'))
        self.assertFalse(nav.warp_triggers('Route12', 10, 15, 'right'))

    def test_explicit_interior_gate_entrance_still_connects_route_sections(self):
        path = nav.bfs_cross('Route12', (9, 4), 'Route12', (10, 61),
                             last_map='Route12', allow_ledges=True)
        self.assertIsNotNone(path)
        self.assertIn('Route12Gate1F', {step[0][0] for step in path[1:]})

    def test_guard_route_is_retryable_when_the_requested_drink_is_carried(self):
        game = nav.Game.__new__(nav.Game)
        game.smart_moves = True
        bag = []
        game.d = SimpleNamespace(cmd=lambda cmd: {'data': {} if cmd == 'get_flags' else bag})
        self.assertEqual(game.navigation_excluded_maps(), ('SaffronCity',))
        bag.append({'item': 'FreshWater', 'qty': 1})
        self.assertEqual(game.navigation_excluded_maps(), ())

    def test_explicit_exit_carpet_does_not_warp_on_a_sideways_step(self):
        path = nav.bfs_cross('CeladonMartElevator', (1, 3), 'CeladonMart1F', (5, 5),
                             last_map='CeladonCity')
        self.assertIsNotNone(path)
        self.assertEqual(path[1][0][0], 'CeladonMartElevator')
        first_exit = next(step for step in path[1:] if step[0][0] != 'CeladonMartElevator')
        self.assertEqual(first_exit[1], 'down')

    def test_floor_route_does_not_assume_an_unselected_elevator_destination(self):
        path = nav.bfs_cross('CeladonMartRoof', (10, 2), 'CeladonMart1F', (5, 5),
                             last_map='CeladonCity')
        self.assertIsNotNone(path)
        self.assertNotIn('CeladonMartElevator', {step[0][0] for step in path[1:]})

    def test_cave_stair_exit_is_not_restricted_to_the_map_border(self):
        destination = nav.warp_edges_from('RockTunnel1F', 15, 33, 'Route10')[0]
        path = nav.bfs_cross('RockTunnel1F', (15, 32), destination[0], destination[1:],
                             last_map='Route10')
        self.assertEqual(path, [('RockTunnel1F', 15, 32), (destination, 'down')])

    def test_no_through_building_can_still_be_the_destination(self):
        path = nav.bfs_cross('Route3', (15, 9), 'PewterPokecenter', (3, 3),
                             last_map='Route3', allow_ledges=True)
        self.assertIsNotNone(path)
        self.assertEqual(path[-1][0], ('PewterPokecenter', 3, 3))

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

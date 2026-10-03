"""Regression cases exposed by real post-Brock playthroughs (stdlib unittest)."""
import unittest
from unittest.mock import Mock, patch
from types import SimpleNamespace
import subprocess
import sys
from pathlib import Path

import playthrough as nav
import playthrough_late as late
from playthrough_late import damage_slot
from debug_drive import DebugClient


class AtomicDebugDriveRegression(unittest.TestCase):
    def client(self, frames=7):
        client = object.__new__(DebugClient)
        reply = {'ok': True, 'data': {'advanced': True, 'queue_start_frame': 100,
                                      'frame_count': 100 + frames}}
        client.cmd = Mock(return_value=reply)
        return client, reply

    def test_tap_and_neutral_tail_advance_in_one_acknowledged_request(self):
        client, reply = self.client()
        self.assertIs(client.drive(['a'], frames=7), reply)
        client.cmd.assert_called_once_with(cmd='press_timeline', advance=True,
                                           buttons=['a'] + [None] * 6)

    def test_default_length_preserves_explicit_neutral_frames(self):
        client, reply = self.client(3)
        self.assertIs(client.drive([None, 'a', None]), reply)
        client.cmd.assert_called_once_with(cmd='press_timeline', advance=True,
                                           buttons=[None, 'a', None])

    def test_invalid_frame_budget_sends_no_input(self):
        for frames in (0, -1, 1.5, True):
            with self.subTest(frames=frames):
                client, _ = self.client()
                with self.assertRaises(ValueError):
                    client.drive(['a'], frames=frames)
                client.cmd.assert_not_called()

    def test_empty_timeline_can_advance_only_neutral_frames(self):
        client, reply = self.client(2)
        self.assertIs(client.drive([], frames=2), reply)
        client.cmd.assert_called_once_with(cmd='press_timeline', advance=True,
                                           buttons=[None, None])

    def test_error_or_missing_exact_advancement_acknowledgement_fails_closed(self):
        for reply in ({'ok': False, 'error': 'unknown button'},
                      {'ok': True}, {'ok': True, 'data': None},
                      {'ok': True, 'data': {'advanced': False}},
                      {'ok': True, 'data': {'advanced': True}},
                      {'ok': True, 'data': {'advanced': True, 'queue_start_frame': 100,
                                           'frame_count': 108}},
                      {'ok': True, 'data': {'advanced': True, 'queue_start_frame': False,
                                           'frame_count': 7}}):
            with self.subTest(reply=reply):
                client, _ = self.client()
                client.cmd.return_value = reply
                with self.assertRaises(RuntimeError):
                    client.drive(['a'], frames=7)
                self.assertEqual(client.cmd.call_count, 1)


class GrassTrainingRegression(unittest.TestCase):
    def test_standing_grass_with_a_dry_rate_anchor_is_not_a_training_run(self):
        # Native wild_encounters.rs rolls against the right-hand tile, not
        # the grass under the player. The Route6 column is visibly grassy
        # but its vertical shuttle cannot produce outdoor encounters.
        self.assertTrue(nav.is_grass('Route6', 3, 21))
        self.assertFalse(nav.is_grass('Route6', 4, 21))
        self.assertEqual(nav.grass_training_run('Route6', (3, 21), 'down'), ('left', 3))

    def test_every_selected_training_step_has_both_native_grass_anchors(self):
        for start in ((3, 21), (2, 21), (10, 23), (14, 23)):
            run = nav.grass_training_run('Route6', start, 'down')
            self.assertIsNotNone(run)
            direction, steps = run
            dx, dy = nav.DELTA[direction]
            for offset in range(1, steps + 1):
                x, y = start[0] + dx * offset, start[1] + dy * offset
                self.assertTrue(nav.is_grass('Route6', x, y), (start, direction, x, y))
                self.assertTrue(nav.is_grass('Route6', x + 1, y), (start, direction, x, y))

    def test_actual_grass_run_avoids_live_npcs_and_coordinate_warps(self):
        self.assertEqual(nav.grass_training_run('Route6', (10, 23), 'down', {(10, 21)}),
                         ('left', 4))
        self.assertEqual(nav.grass_training_run('Route6', (10, 23), 'down', {(11, 23)}),
                         ('left', 4))
        self.assertEqual(nav.grass_training_run('Route6', (10, 23), 'down', {(9, 23)}),
                         ('right', 4))
        with patch.dict(nav.COORDINATE_WARPS, {'Route6': {(9, 23): ('OtherMap', 0, 0)}}):
            self.assertEqual(nav.grass_training_run('Route6', (10, 23), 'down'), ('right', 4))

    def test_reobserved_endpoint_preserves_ties_and_reroutes_around_occupancy(self):
        self.assertEqual(nav.grass_training_run('Route6', (14, 23), 'left'), ('left', 4))
        self.assertEqual(nav.grass_training_run('Route6', (14, 23), 'left', {(13, 23)}),
                         ('down', 4))
        self.assertIsNone(nav.grass_training_run('Route6', (10, 23), 'down',
                                               {(10, 22), (9, 23), (11, 23)}))

    def test_route_six_training_uses_actual_grass_in_the_same_input_budget(self):
        game = nav.Game.__new__(nav.Game)
        state = dict(screen='overworld', map_name='Route6', player_x=10, player_y=23,
                     party=[dict(level=28, hp=76, max_hp=76)])
        game.st = lambda: state.copy()
        game.pos = lambda: ('Route6', state['player_x'], state['player_y'])
        game.npc_blocked = lambda _: {(10, 21)}
        game.d = Mock()

        def drive(buttons, frames=None):
            dx, dy = nav.DELTA[buttons[0]]
            for _ in range(len(buttons) // 8):
                start = state['player_x'], state['player_y']
                target = start[0] + dx, start[1] + dy
                if target == (10, 21) or not nav.walkable_edge('Route6', start, target):
                    break
                state['player_x'], state['player_y'] = target

        game.d.drive.side_effect = drive
        self.assertFalse(game.train_until(30, 'Route6', (12, 25), None, max_cycles=1))
        bursts = game.d.drive.call_args_list
        self.assertEqual([call.args[0][0] for call in bursts], ['left', 'right'])
        self.assertEqual([len(call.args[0]) for call in bursts], [32, 32])
        self.assertEqual([call.kwargs['frames'] for call in bursts], [36, 36])
        self.assertEqual((state['player_x'], state['player_y']), (10, 23))
        self.assertEqual(state['party'][0]['level'], 28)

    def test_second_burst_is_not_sent_after_battle_or_map_change(self):
        for interrupted in ({'screen': 'battle'}, {'map_name': 'OtherMap'}):
            with self.subTest(interrupted=interrupted):
                game = nav.Game.__new__(nav.Game)
                state = dict(screen='overworld', map_name='Route6', player_x=10, player_y=23,
                             party=[dict(level=28, hp=76, max_hp=76)])
                game.st = lambda: state.copy()
                game.pos = lambda: (state['map_name'], state['player_x'], state['player_y'])
                game.npc_blocked = lambda _: set()
                game.d = Mock()
                game.d.drive.side_effect = lambda *_args, **_options: state.update(interrupted)
                self.assertFalse(game.train_until(30, 'Route6', (12, 25), None, max_cycles=1))
                game.d.drive.assert_called_once()
                self.assertEqual(game.d.drive.call_args.kwargs['frames'], 36)


class LegacyPartyReadinessRegression(unittest.TestCase):
    def test_observed_agatha_party_switches_to_remaining_effective_attack(self):
        state = {'battle_live': {'enemy': {'species': 'Gengar'}, 'player_party': [
            {'species': 'Zapdos', 'level': 56, 'hp': 0,
             'moves': ['Thunder', 'DrillPeck', 'Fly', 'Thunderbolt'], 'pp': [10, 18, 15, 11]},
            {'species': 'Lapras', 'level': 21, 'hp': 89,
             'moves': ['WaterGun', 'Growl', 'Surf', 'Strength'], 'pp': [25, 40, 15, 15]},
            {'species': 'Venusaur', 'level': 56, 'hp': 92,
             'moves': ['MegaDrain', 'RazorLeaf', 'Cut', 'VineWhip'], 'pp': [0, 0, 24, 0]},
        ]}}
        self.assertEqual(late.battle_party_target(state), 1)

    def test_conscious_preferred_member_with_empty_attack_pp_does_not_seal_the_battle(self):
        state = {'battle_live': {'enemy': {'species': 'Gengar'}, 'player_party': [
            {'species': 'Zapdos', 'level': 60, 'hp': 150,
             'moves': ['Thunderbolt'], 'pp': [0]},
            {'species': 'Lapras', 'level': 21, 'hp': 89,
             'moves': ['Surf'], 'pp': [1]},
        ]}}
        self.assertEqual(late.battle_party_target(state), 1)

    def test_weak_but_nonzero_damage_still_counts_as_a_legal_attack(self):
        state = {'battle_live': {'enemy': {'species': 'Gengar'}, 'player_party': [
            {'species': 'Zapdos', 'level': 60, 'hp': 150,
             'moves': ['Growl'], 'pp': [40]},
            {'species': 'Paras', 'level': 5, 'hp': 19,
             'moves': ['Absorb'], 'pp': [1]},
        ]}}
        self.assertEqual(late.battle_party_target(state), 1)

    def test_existing_ground_preference_is_retained_when_it_can_damage(self):
        state = {'battle_live': {'enemy': {'species': 'Rhydon'}, 'player_party': [
            {'species': 'Zapdos', 'level': 60, 'hp': 150,
             'moves': ['DrillPeck'], 'pp': [10]},
            {'species': 'Venusaur', 'level': 55, 'hp': 170,
             'moves': ['RazorLeaf'], 'pp': [10]},
        ]}}
        self.assertEqual(late.battle_party_target(state), 1)

    def test_no_usable_attacks_keeps_a_conscious_fallback_without_faking_pp(self):
        state = {'battle_live': {'enemy': {'species': 'Gengar'}, 'player_party': [
            {'species': 'Zapdos', 'level': 60, 'hp': 0, 'moves': ['Thunderbolt'], 'pp': [0]},
            {'species': 'Lapras', 'level': 21, 'hp': 89, 'moves': ['Surf'], 'pp': [0]},
            {'species': 'Venusaur', 'level': 56, 'hp': 92, 'moves': ['Cut'], 'pp': [24]},
        ]}}
        self.assertEqual(late.battle_party_target(state), 2)


class CutFieldMoveCompletionRegression(unittest.TestCase):
    def game(self, name='Cut', unchanged=False, departed=False, target_tile=0x3D,
             preserved_tree=False):
        game = Mock()
        progress = {'phase': 'initial', 'selections': 0, 'stepped': 0}
        menu = {'kind': 'party', 'cursor': 0, 'phase': 'FieldMoves { cursor: 0 }',
                'field_moves': [name]}

        def state():
            pending = progress['phase'] == 'pending'
            cleared = pending and not unchanged and progress['stepped'] >= 2
            return {'map_name': 'OtherMap' if pending and departed else 'VermilionCity',
                    'player_x': 15, 'player_y': 17, 'player_facing': 'Down',
                    'map_blocks': [1, 2 if cleared else 1],
                    'field_menu': menu if progress['phase'] == 'party' else None}

        def open_start(*_):
            progress['phase'] = 'party'

        def tap(button, _):
            if button == 'a':
                progress['selections'] += 1
                if progress['selections'] == 2:
                    progress['phase'] = 'pending'

        game.st.side_effect = state
        game.tap.side_effect = tap
        game.step.side_effect = lambda frames: progress.update(stepped=progress['stepped'] + frames)
        game.cutscene.return_value = True
        # Explicit native tile observation for this isolated menu/effect test.
        target = patch.object(nav, 'tile_at', side_effect=lambda *_args:
            0x2C if (target_tile == 0x3D and not unchanged and not preserved_tree
                     and progress['stepped'] >= 2) else target_tile)
        target.start()
        self.addCleanup(target.stop)
        return game, open_start, progress

    def test_cut_waits_for_actual_native_map_change_without_replaying_the_menu(self):
        game, open_start, progress = self.game()
        with patch.object(late, 'open_start', side_effect=open_start):
            late.field_move(game, 'Cut')
        game.step.assert_called_once_with(2)
        self.assertEqual(progress['selections'], 2)
        self.assertEqual(game.st()['map_blocks'], [1, 2])

    def test_closed_text_without_native_cut_change_is_not_success(self):
        game, open_start, progress = self.game(unchanged=True)
        with patch.object(late, 'open_start', side_effect=open_start):
            with self.assertRaisesRegex(RuntimeError, 'CUT did not change'):
                late.field_move(game, 'Cut')
        self.assertEqual(progress['selections'], 2)
        self.assertLessEqual(progress['stepped'], 120)

    def test_another_map_does_not_prove_cut_completed(self):
        game, open_start, _ = self.game(departed=True)
        with patch.object(late, 'open_start', side_effect=open_start):
            with self.assertRaisesRegex(RuntimeError, 'CUT left'):
                late.field_move(game, 'Cut')
        game.step.assert_not_called()

    def test_an_unrelated_block_edit_does_not_prove_the_tree_was_cut(self):
        game, open_start, progress = self.game(preserved_tree=True)
        with patch.object(late, 'open_start', side_effect=open_start):
            with self.assertRaisesRegex(RuntimeError, 'CUT did not change'):
                late.field_move(game, 'Cut')
        self.assertEqual(progress['selections'], 2)
        self.assertLessEqual(progress['stepped'], 120)

    def test_already_cleared_tree_and_grass_cut_do_not_require_a_block_edit(self):
        for tile in (0x2C, 0x52):
            with self.subTest(tile=tile):
                game, open_start, progress = self.game(target_tile=tile, unchanged=True)
                with patch.object(late, 'open_start', side_effect=open_start):
                    late.field_move(game, 'Cut')
                game.step.assert_not_called()
                self.assertEqual(progress['selections'], 2)

    def test_other_field_move_handoffs_are_unchanged(self):
        for name in ('Surf', 'Fly'):
            with self.subTest(name=name):
                game, open_start, _ = self.game(name=name, unchanged=True)
                with patch.object(late, 'open_start', side_effect=open_start):
                    late.field_move(game, name)
                game.step.assert_not_called()
                self.assertEqual(game.cutscene.call_count, 0 if name == 'Fly' else 1)


class DebugTransportRetryRegression(unittest.TestCase):
    def client(self):
        client = object.__new__(DebugClient)
        client.f = Mock()
        client.f.readline.return_value = '{"ok": true, "data": {}}\n'
        client._reconnect = Mock()
        return client

    def test_unacknowledged_mutations_are_never_replayed(self):
        commands = ('press', 'press_sequence', 'press_timeline', 'step_frames',
                    'run_frames', 'wait_until', 'skip_dialogue', 'move_to',
                    'travel_to', 'interact', 'interact_with', 'save',
                    'save_state', 'restore_state', 'shutdown', 'capture_frame',
                    'get_future_mutating_command')
        for command in commands:
            for failure in ('write', 'flush', 'readline', 'eof'):
                with self.subTest(command=command, failure=failure):
                    client = self.client()
                    if failure == 'eof':
                        client.f.readline.side_effect = ['', '{"ok": true}\n']
                    else:
                        getattr(client.f, failure).side_effect = [TimeoutError('lost reply'), None]
                    with self.assertRaisesRegex(OSError, 'not replayed'):
                        client.cmd(cmd=command)
                    self.assertEqual(client.f.write.call_count, 1)
                    client._reconnect.assert_not_called()

    def test_allowlisted_observations_can_retry_once(self):
        commands = ('get_state', 'get_position', 'get_party', 'get_bag',
                    'get_flags', 'get_npcs', 'get_map', 'get_agent_state',
                    'get_nearby', 'get_world_graph', 'find_world_route',
                    'get_script_semantics')
        for command in commands:
            with self.subTest(command=command):
                client = self.client()
                client.f.readline.side_effect = [TimeoutError('lost reply'),
                                                 '{"ok": true, "data": {}}\n']
                self.assertEqual(client.cmd(cmd=command), {'ok': True, 'data': {}})
                self.assertEqual(client.f.write.call_count, 2)
                client._reconnect.assert_called_once_with()

    def test_observation_retry_is_bounded_after_two_transport_failures(self):
        client = self.client()
        client.f.readline.side_effect = TimeoutError('still disconnected')
        with self.assertRaises(TimeoutError):
            client.cmd(cmd='get_state')
        self.assertEqual(client.f.write.call_count, 2)
        client._reconnect.assert_called_once_with()

    def test_acknowledged_native_rejection_is_not_retried(self):
        client = self.client()
        client.f.readline.return_value = '{"ok": false, "error": "blocked"}\n'
        self.assertEqual(client.cmd(cmd='move_to'), {'ok': False, 'error': 'blocked'})
        client.f.write.assert_called_once()
        client._reconnect.assert_not_called()

    def test_reconnect_closes_makefile_and_socket_even_if_file_close_fails(self):
        client = object.__new__(DebugClient)
        old_file, old_socket, new_socket = Mock(), Mock(), Mock()
        client.f, client.sock = old_file, old_socket
        old_file.close.side_effect = OSError('timed out while flushing')
        client._connect = Mock(side_effect=lambda timeout: setattr(client, 'sock', new_socket))
        client._reconnect()
        old_file.close.assert_called_once_with()
        old_socket.close.assert_called_once_with()
        client._connect.assert_called_once_with(60)
        new_socket.settimeout.assert_called_once_with(120.0)
        new_socket.makefile.assert_called_once_with('rw')
        self.assertIs(client.f, new_socket.makefile.return_value)


class SearchGeometryCacheRegression(unittest.TestCase):
    def setUp(self):
        self.name = 'SearchCacheFixture'
        self.data = {'width': 2, 'height': 1, 'tileset_id': 999,
                     'tileset_name': 'Cavern', 'blocks': [0, 0],
                     'passable_tiles': [0], 'warps': []}
        for table, entries in ((nav.MAPS, {self.name: self.data}),
                               (nav.CONNS, {self.name: {}}),
                               (nav.BLOCKSETS, {999: [[0] * 16, [1] * 16]})):
            context = patch.dict(table, entries)
            context.start()
            self.addCleanup(context.stop)

    def search(self, **kwargs):
        return nav.bfs_cross(self.name, (0, 0), self.name, (3, 0), **kwargs)

    def test_geometry_is_sampled_once_per_key_per_search_not_across_searches(self):
        with patch.object(nav, 'tile_at', wraps=nav.tile_at) as tiles, \
                patch.object(nav, 'warp_tiles', wraps=nav.warp_tiles) as warps:
            route = self.search()
            self.assertIsNotNone(route)
            tile_calls = [call.args for call in tiles.call_args_list]
            self.assertEqual(len(tile_calls), len(set(tile_calls)))
            warps.assert_called_once_with(self.name)
            self.assertIs(nav.tile_at, tiles)
            self.assertIs(nav.warp_tiles, warps)
            count = tiles.call_count
            self.assertEqual(self.search(), route)
            self.assertEqual(tiles.call_count, count * 2)
            self.assertEqual(warps.call_count, 2)

    def test_next_search_observes_in_place_block_warp_and_npc_changes(self):
        route = self.search()
        self.data['blocks'][1] = 1
        self.assertIsNone(self.search())
        self.data['blocks'][1] = 0
        self.assertEqual(self.search(), route)
        # A scripted fall cannot be treated as floor by an old warp cache.
        with patch.dict(nav.COORDINATE_WARPS, {self.name: {(3, 0): (self.name, 0, 0)}}):
            self.assertIsNone(self.search())
        self.assertEqual(self.search(), route)
        self.data['warps'].append({'x': 3, 'y': 0, 'dest_map_name': 'MissingMap',
                                   'dest_warp_id': 0})
        with patch.object(nav, 'warp_triggers', return_value=True):
            self.assertIsNone(self.search())
        self.data['warps'].clear()
        self.assertEqual(self.search(), route)
        self.assertIsNone(self.search(blocked_maps={self.name: {(3, 0)}}))
        self.assertEqual(self.search(), route)

    def test_geometry_functions_are_restored_after_nested_search_failure(self):
        original = nav.tile_at, nav.warp_tiles
        cross = nav.cross_step
        nested = False
        def fail_inside_outer(*args):
            nonlocal nested
            if not nested:
                nested = True
                outer = nav.tile_at, nav.warp_tiles
                with patch.object(nav, 'cross_step', side_effect=ValueError('inner search')):
                    with self.assertRaisesRegex(ValueError, 'inner search'):
                        self.search()
                self.assertEqual((nav.tile_at, nav.warp_tiles), outer)
            return cross(*args)
        with patch.object(nav, 'cross_step', side_effect=fail_inside_outer):
            self.assertIsNotNone(self.search())
        self.assertEqual((nav.tile_at, nav.warp_tiles), original)
        with patch.object(nav, 'cross_step', side_effect=ValueError('outer search')):
            with self.assertRaisesRegex(ValueError, 'outer search'):
                self.search()
        self.assertEqual((nav.tile_at, nav.warp_tiles), original)

    def test_geometry_cache_does_not_leak_surf_passability_into_land_search(self):
        from openpokered.navigation_skills import water_planning
        name = 'PalletTown'
        # Bound the search to this map, keeping this a small real-data test.
        excluded = set(nav.MAPS) - {name}
        def search():
            return nav.bfs_cross(name, (5, 13), name, (5, 14), excluded_maps=excluded)
        original = nav.tile_at, nav.warp_tiles
        self.assertIsNone(search())
        with water_planning():
            self.assertEqual(search(), [(name, 5, 13), ((name, 5, 14), 'down')])
        self.assertIsNone(search())
        self.assertEqual((nav.tile_at, nav.warp_tiles), original)


class SharedRouteSearchRegression(unittest.TestCase):
    def setUp(self):
        self.name, self.closed = 'SharedRouteFixture', 'SharedRouteClosed'
        self.data = {'width': 2, 'height': 1, 'tileset_id': 998,
                     'tileset_name': 'Cavern', 'blocks': [0, 0],
                     'passable_tiles': [0], 'warps': []}
        for table, entries in ((nav.MAPS, {self.name: self.data}),
                               (nav.CONNS, {self.name: {}}),
                               (nav.BLOCKSETS, {998: [[0] * 16, [1] * 16]})):
            context = patch.dict(table, entries)
            context.start()
            self.addCleanup(context.stop)

    def compare(self, name, start, regions, **options):
        expected = {}
        for key, region in regions.items():
            if region:
                first = next(iter(region))
                expected[key] = nav.bfs_cross(name, start, first[0], first[1:],
                                              goal_nodes=region, **options)
            else:
                expected[key] = None
        with patch.object(nav, 'bfs_cross', wraps=nav.bfs_cross) as search:
            actual = nav.bfs_cross_routes(name, start, regions, **options)
        self.assertEqual(actual, expected)
        return actual, search.call_count

    def test_regions_keep_exact_ties_root_overlap_empty_and_unreachable_paths(self):
        n = self.name
        regions = {'far': {(n, 3, 0)}, 'ties': {(n, 1, 0), (n, 0, 1)},
                   'overlap': {(n, 3, 0)}, 'root': {(n, 0, 0)},
                   'empty': set(), 'blocked': {(n, 3, 1)}}
        routes, calls = self.compare(n, (0, 0), regions, blocked_maps={n: {(3, 1)}})
        self.assertEqual(calls, 1)
        self.assertEqual(routes['root'], [(n, 0, 0)])
        self.assertIsNone(routes['blocked'])
        routes['far'].append('caller mutation')
        self.assertNotIn('caller mutation', routes['overlap'])

    def test_no_through_destination_cannot_unlock_transit_for_other_queries(self):
        n, closed = self.name, self.closed
        self.data['warps'] = [dict(x=1, y=0, dest_map_name=closed, dest_warp_id=0),
                              dict(x=3, y=0, dest_map_name=closed, dest_warp_id=1)]
        building = {**self.data, 'blocks': [0, 0], 'warps': [
            dict(x=0, y=0, dest_map_name=n, dest_warp_id=0),
            dict(x=3, y=0, dest_map_name=n, dest_warp_id=1)]}
        regions = {'east': {(n, 3, 0)}, 'building': {(closed, 2, 0)}}
        with patch.dict(nav.MAPS, {closed: building}), patch.dict(nav.CONNS, {closed: {}}), \
                patch.object(nav, 'NO_THROUGH', nav.NO_THROUGH | {closed}), \
                patch.object(nav, 'warp_triggers', return_value=True):
            routes, calls = self.compare(n, (0, 0), regions,
                blocked_maps={n: {(2, 0), (2, 1)}})
            self.assertEqual(calls, 2)
            self.assertIsNone(routes['east'])
            self.assertIsNotNone(routes['building'])
            with self.assertRaisesRegex(ValueError, 'NO_THROUGH'):
                nav.bfs_cross(n, (0, 0), n, (3, 0), goal_regions=regions)

    def test_next_batch_observes_blocks_coordinate_warps_and_live_occupancy(self):
        n = self.name
        regions = {'near': {(n, 0, 1)}, 'far': {(n, 3, 0)}}
        before, _ = self.compare(n, (0, 0), regions)
        self.data['blocks'][1] = 1
        closed, _ = self.compare(n, (0, 0), regions)
        self.assertIsNone(closed['far'])
        self.assertEqual(closed['near'], before['near'])
        self.data['blocks'][1] = 0
        with patch.dict(nav.COORDINATE_WARPS, {n: {(3, 0): (n, 0, 0)}}):
            fallen, _ = self.compare(n, (0, 0), regions)
            self.assertIsNone(fallen['far'])
        occupied, _ = self.compare(n, (0, 0), regions, blocked_maps={n: {(3, 0)}})
        self.assertIsNone(occupied['far'])
        self.assertEqual(self.compare(n, (0, 0), regions)[0], before)

    def test_empty_batches_and_conflicting_goal_modes_fail_without_search(self):
        with patch.object(nav, 'bfs_cross', wraps=nav.bfs_cross) as search:
            self.assertEqual(nav.bfs_cross_routes(self.name, (0, 0), {}), {})
            self.assertEqual(nav.bfs_cross_routes(self.name, (0, 0), {'empty': set()}),
                             {'empty': None})
            search.assert_not_called()
        for options in ({'goal_nodes': set()}, {'reachable_goals': True}, {'goal_regions': {}}):
            with self.subTest(options=options), self.assertRaises(ValueError):
                nav.bfs_cross_routes(self.name, (0, 0), {}, **options)

    def test_water_relaxation_is_per_batch_and_restores_dry_geometry(self):
        from openpokered.navigation_skills import water_planning
        name = 'PalletTown'
        regions = {'water': {(name, 5, 14)}, 'shore': {(name, 5, 13)}}
        options = {'excluded_maps': set(nav.MAPS) - {name}}
        original = nav.tile_at, nav.warp_tiles, nav.cross_step, nav.walkable_edge
        dry, calls = self.compare(name, (5, 13), regions, **options)
        self.assertEqual(calls, 1)
        self.assertIsNone(dry['water'])
        with water_planning():
            wet, _ = self.compare(name, (5, 13), regions, **options)
            self.assertEqual(wet['water'], [(name, 5, 13), ((name, 5, 14), 'down')])
        self.assertEqual(self.compare(name, (5, 13), regions, **options)[0], dry)
        self.assertEqual((nav.tile_at, nav.warp_tiles, nav.cross_step, nav.walkable_edge), original)

    def test_forced_spinner_and_jump_paths_match_independent_searches(self):
        name = 'RocketHideoutB3F'
        endpoint, _ = nav.SPINNERS[name][(10, 13)]
        self.compare(name, (10, 12), {'spinner': {(name, *endpoint)}, 'root': {(name, 10, 12)}},
                     allow_spinners=True, excluded_maps=set(nav.MAPS) - {name})
        name = 'Route3'
        jumps = [(x, y, direction, node)
                 for x in range(nav.MAPS[name]['width'] * 2)
                 for y in range(nav.MAPS[name]['height'] * 2)
                 for direction in nav.DELTA
                 if (node := nav.ledge_step(name, x, y, direction))
                 and nav.cross_step(name, x, y, direction) is None]
        self.assertTrue(jumps)
        x, y, direction, node = jumps[0]
        routes, _ = self.compare(name, (x, y), {'ledge': {node}, 'root': {(name, x, y)}},
                                 allow_ledges=True, excluded_maps=set(nav.MAPS) - {name})
        self.assertEqual(routes['ledge'], [(name, x, y), (node, 'jump_' + direction)])

    def test_entry_context_and_directional_warp_paths_match_independent_searches(self):
        target = nav.warp_edges_from('VermilionDock', 14, 2, 'VermilionCity')[0]
        self.compare('VermilionDock', (14, 1), {'ship': {target},
                     'dock': {('VermilionDock', 14, 1)}}, last_map='VermilionCity',
                     excluded_maps=set(nav.MAPS) - {'VermilionDock', target[0]})
        self.compare('CeladonMartElevator', (1, 3), {
                     'floor': {('CeladonMart1F', 5, 5)}, 'room': {('CeladonMartElevator', 1, 3)}},
                     last_map='CeladonCity', excluded_maps=set(nav.MAPS) - {
                         'CeladonMartElevator', 'CeladonMart1F'})

    def test_search_exception_restores_geometry_and_leaves_no_batch_cache(self):
        original = nav.tile_at, nav.warp_tiles
        regions = {'near': {(self.name, 0, 1)}, 'far': {(self.name, 3, 0)}}
        with patch.object(nav, 'cross_step', side_effect=ValueError('search failed')):
            with self.assertRaisesRegex(ValueError, 'search failed'):
                nav.bfs_cross_routes(self.name, (0, 0), regions)
        self.assertEqual((nav.tile_at, nav.warp_tiles), original)
        self.assertIsNotNone(self.compare(self.name, (0, 0), regions)[0]['far'])


class NavigationRegression(unittest.TestCase):
    def pushback_game(self, changes=False):
        from copy import deepcopy
        changing = 'flags' if changes is True else changes
        game = nav.Game.__new__(nav.Game)
        state = {'screen': 'overworld', 'map_name': 'Route16Gate1F',
                 'player_x': 4, 'player_y': 7, 'dialogue_state': {'text': 'Wait!'},
                 'player_movement_state': 'Idle', 'warp_fade': 'Idle',
                 'party': [], 'money': 0, 'map_blocks': [], 'script_running': True}
        progress = {'rounds': 0}
        game.st = lambda: deepcopy(state)
        game.navigation_state = game.st
        game.navigation_excluded_maps = lambda: ()
        game.navigation_barriers = lambda: {}
        game.npc_blocked = lambda name: set()
        game.live_npcs = lambda name: set()
        game.track_last_map = lambda name: None
        game.last_map = 'Route16'
        game.d = Mock()
        game.d.cmd.side_effect = lambda **kw: {'data': (
            {'progress': progress['rounds'] if changing == 'flags' else 0}
            if kw['cmd'] == 'get_flags' else
            [{'item': 'PokeBall', 'qty': progress['rounds']}] if changing == 'bag' else [])}
        def settle():
            progress['rounds'] += 1
            state.update(player_x=3 if changes and progress['rounds'] == 5 else 5,
                         dialogue_state=None, script_running=False)
            if changing not in (False, 'flags', 'bag'):
                state[changing] = {'observed_progress': progress['rounds']}
            return True
        game.cutscene = Mock(side_effect=settle)
        game.d.drive.side_effect = lambda *a, **kw: state.update(
            player_x=4, dialogue_state={'text': 'Wait!'}, script_running=True)
        path = [('Route16Gate1F', 5, 7), (('Route16Gate1F', 4, 7), 'left'),
                (('Route16Gate1F', 3, 7), 'left')]
        return game, path

    def test_repeated_unchanged_script_pushback_reports_before_full_walk_budget(self):
        game, path = self.pushback_game()
        with patch.object(nav, 'bfs_cross', return_value=path), \
                self.assertRaisesRegex(nav.NavError, 'repeated unchanged script displacement'):
            game.nav_to_map(3, 7, 'Route16Gate1F', tries=20, avoid_grass=False)
        self.assertEqual(game.cutscene.call_count, 3)
        self.assertEqual(game.d.drive.call_count, 2)

    def test_same_script_displacement_with_changed_flags_can_still_make_progress(self):
        game, path = self.pushback_game(changes=True)
        with patch.object(nav, 'bfs_cross', return_value=path):
            self.assertEqual(game.nav_to_map(3, 7, 'Route16Gate1F', tries=20,
                                            avoid_grass=False), (3, 7))
        self.assertEqual(game.cutscene.call_count, 5)

    def test_script_pushback_does_not_hide_inventory_party_or_terrain_progress(self):
        for field in ('bag', 'money', 'party', 'evaluation', 'pokedex', 'map_blocks', 'safari_game'):
            with self.subTest(field=field):
                game, path = self.pushback_game(changes=field)
                with patch.object(nav, 'bfs_cross', return_value=path):
                    self.assertEqual(game.nav_to_map(3, 7, 'Route16Gate1F', tries=20,
                                                    avoid_grass=False), (3, 7))
                self.assertEqual(game.cutscene.call_count, 5)

    def test_milestone_can_push_a_boulder_into_its_scripted_hole(self):
        import playthrough_late as late
        flag = 'EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2'
        flags = {}
        game = Mock()
        game.st.return_value = {'screen': 'overworld', 'player_x': 21, 'player_y': 15,
                                'party': [{'moves': ['Strength']}]}
        game.d.cmd.side_effect = lambda **kw: {'data': flags if kw['cmd'] == 'get_flags' else [
            {'text_id': 10, 'sprite_id': 63, 'x': 22, 'y': 15, 'visible': True}]}
        game.d.drive.side_effect = lambda *args, **kw: flags.update({flag: True})
        game.observed_npcs = {}
        game.live_npcs.return_value = {(22, 15)}
        with patch.object(nav, 'COORDINATE_WARPS', {
                'VictoryRoad3F': {(23, 15): ('VictoryRoad2F', 22, 16)}}), \
                patch.object(late, 'field_move'):
            late.push_boulder(game, 'VictoryRoad3F', 10, (23, 15), flag)
        game.nav_to.assert_called_once_with(21, 15, 'VictoryRoad3F')
        game.d.drive.assert_called_once_with(['right'] * 8, frames=48)

    def test_coordinate_warp_reads_native_program_and_named_config_binding(self):
        warp = lambda name, x, y: {'Command': {'name': 'warpTo', 'args': [
            {'StringLit': name}, {'NumberLit': x}, {'NumberLit': y}]}}
        program = [warp('SeafoamIslandsB1F', 23, 7)]
        client = Mock()
        client.cmd.return_value = {'ok': True, 'data': {'storylines': [{
            'id': 'SeafoamIslands1F:coordHole2', 'program': program}]}}
        configs = {'SeafoamIslands1F': {'coordEvents': [
            {'position': [24, 6], 'trigger': 'coordHole2'}]}}
        self.assertEqual(nav.load_coordinate_warps(client, configs), {
            'SeafoamIslands1F': {(24, 6): ('SeafoamIslandsB1F', 23, 7)}})
        client.cmd.assert_called_once_with(cmd='get_script_semantics', map='SeafoamIslands1F')

        conditional = [{'If': {'condition': {'BinaryOp': {'op': 'Eq',
            'left': {'Call': {'callee': 'getPlayerX', 'args': []}},
            'right': {'NumberLit': 19}}}, 'then_branch': [warp('PokemonMansion2F', 18, 14)],
            'else_branch': [warp('PokemonMansion1F', 16, 14)]}}]
        self.assertEqual(nav.coordinate_warp_destination(conditional, 19, 14), ('PokemonMansion2F', 18, 14))
        self.assertEqual(nav.coordinate_warp_destination(conditional, 16, 14), ('PokemonMansion1F', 16, 14))
        conditional[0]['If']['condition'] = {'Call': {'callee': 'getFlag', 'args': [{'StringLit': 'PAID'}]}}
        self.assertIsNone(nav.coordinate_warp_destination(conditional, 16, 14))
        self.assertIsNone(nav.coordinate_warp_destination([{'Choice': {}}, *program], 24, 6))
        self.assertIsNone(nav.coordinate_warp_destination([
            {'Command': {'name': 'heal', 'args': []}}, *program], 24, 6))

    def test_cross_map_planning_models_one_way_scripted_fall(self):
        holes = {'SeafoamIslands1F': {(17, 6): ('SeafoamIslandsB1F', 18, 7),
                                       (24, 6): ('SeafoamIslandsB1F', 23, 7)}}
        with patch.object(nav, 'COORDINATE_WARPS', holes):
            path = nav.bfs_cross('SeafoamIslands1F', (25, 6), 'SeafoamIslandsB1F', (23, 7))
            self.assertEqual(path, [('SeafoamIslands1F', 25, 6),
                                   (('SeafoamIslandsB1F', 23, 7), 'fall_left')])
            self.assertIsNone(nav.bfs_cross('SeafoamIslands1F', (25, 6), 'SeafoamIslands1F', (24, 6)))
            self.assertIsNone(nav.bfs_cross('SeafoamIslands1F', (25, 6), 'SeafoamIslandsB1F', (23, 7),
                blocked_maps={'SeafoamIslandsB1F': {(23, 7)}}))
            local = nav.bfs('SeafoamIslands1F', (25, 6), (21, 6))
            self.assertTrue(local)
            self.assertNotIn((24, 6), [point for point, _ in local])
            # Explicit local hole targets remain usable by the milestone driver.
            self.assertEqual(nav.bfs('SeafoamIslands1F', (25, 6), (24, 6)),
                             [((25, 6), None), ((24, 6), 'left')])

    def test_scripted_fall_input_stops_before_driving_the_destination(self):
        game = nav.Game.__new__(nav.Game)
        state = {'screen': 'overworld', 'map_name': 'SeafoamIslands1F', 'player_x': 25, 'player_y': 6}
        game.st = lambda: state.copy()
        game.last_map = 'Route20'
        game.npc_blocked = game.live_npcs = lambda _: set()
        def drive(buttons, frames):
            self.assertEqual(buttons, ['left'] * nav.FRAMES_PER_TILE)
            state.update(map_name='SeafoamIslandsB1F', player_x=23, player_y=7)
        game.d = SimpleNamespace(drive=Mock(side_effect=drive), step=lambda _: None)
        with patch.object(nav, 'COORDINATE_WARPS', {
                'SeafoamIslands1F': {(24, 6): ('SeafoamIslandsB1F', 23, 7)}}):
            game.nav_to_map(23, 7, 'SeafoamIslandsB1F', tries=2, avoid_grass=False)
        game.d.drive.assert_called_once()

    def test_battle_presentation_waits_without_spending_input_iterations(self):
        game = nav.Game.__new__(nav.Game)
        state = {'screen': 'battle', 'map_name': 'Arena', 'battle_phase': 'ShowingText',
                 'battle_message': 'Move animation', 'battle_presentation': {'waiting': True}}
        steps = []
        game.st = lambda: state.copy()
        def step(frames):
            steps.append(frames)
            if sum(steps) >= 90:
                state['battle_presentation'] = {'waiting': False}
        def tap(*args):
            self.assertFalse(state['battle_presentation']['waiting'])
            state['screen'] = 'overworld'
        game.step = step
        game.tap = Mock(side_effect=tap)
        game.wait = lambda *args: self.assertEqual(state['screen'], 'overworld')
        game.battle_loop(max_iters=1)
        self.assertEqual(sum(steps), 90)
        game.tap.assert_called_once_with('a', 10)

    def test_stuck_battle_presentation_has_a_separate_bounded_frame_guard(self):
        game = nav.Game.__new__(nav.Game)
        game.st = lambda: {'screen': 'battle', 'battle_phase': 'ShowingText', 'battle_message': '',
                          'battle_presentation': {'waiting': True, 'vfx_blockers': ['wave']}}
        game.step, game.tap, game.wait = Mock(), Mock(), Mock()
        with self.assertRaisesRegex(AssertionError, 'battle presentation.*1800'):
            game.battle_loop(max_iters=1)
        self.assertEqual(sum(call.args[0] for call in game.step.call_args_list), 1800)
        game.tap.assert_not_called()
        game.wait.assert_not_called()

    def test_cross_map_trip_retargets_after_discovering_destination_npcs(self):
        game = nav.Game.__new__(nav.Game)
        state = {'screen': 'overworld', 'map_name': 'SilphCo4F', 'player_x': 26, 'player_y': 1}
        game.st = lambda: state.copy()
        game.last_map = 'SaffronCity'
        game.smart_moves = True
        game.navigation_excluded_maps = lambda: ()
        occupied = {(28, 4), (21, 16), (13, 9), (8, 16), (8, 3),
                    (18, 10), (2, 13), (4, 6), (22, 12), (25, 10), (24, 6)}
        game.npc_blocked = game.live_npcs = lambda name: occupied if name == 'SilphCo5F' else set()
        search = nav.bfs_cross
        def plan(name, start, target, preferred, **kwargs):
            self.assertEqual(kwargs['goal_nodes'], {('SilphCo5F', 22, 16), ('SilphCo5F', 20, 16)})
            if name == 'SilphCo4F':
                return [(name, *start), (('SilphCo5F', 26, 0), 'up')]
            path = search(name, start, target, preferred, **kwargs)
            self.assertEqual(path[-1][0], ('SilphCo5F', 20, 16))
            self.assertTrue(any(node[0] == 'SilphCo9F' for node, _ in path[1:]))
            return path
        def drive(buttons, frames):
            if state['map_name'] == 'SilphCo4F':
                state.update(map_name='SilphCo5F', player_y=1)
            else:
                state.update(player_x=20, player_y=16)
        game.d = SimpleNamespace(drive=drive, step=lambda _: None)
        with patch.object(nav, 'bfs_cross', side_effect=plan) as planned:
            self.assertEqual(game.nav_to_map(22, 16, 'SilphCo5F', tries=3,
                avoid_grass=False, goal_points=[(22, 16), (20, 16)]), (20, 16))
        self.assertEqual(planned.call_count, 2)

    def test_cross_map_trip_rejects_empty_approach_region(self):
        game = nav.Game.__new__(nav.Game)
        with self.assertRaisesRegex(nav.NavError, 'no destination points'):
            game.nav_to_map(22, 16, 'SilphCo5F', goal_points=[])

    def test_navigation_observes_final_arrival_before_accepting_target(self):
        game = nav.Game.__new__(nav.Game)
        state = {'screen': 'overworld', 'map_name': 'CeladonMansion1F',
                 'player_x': 2, 'player_y': 1, 'warp_fade': 'FadingOut { frames_remaining: 2 }'}
        game.st = lambda: state.copy()
        game.last_map = 'CeladonCity'
        game.d = Mock()
        def settle(*args, **kwargs):
            state.update(map_name='CeladonMansion2F', player_y=2, warp_fade='Idle')
        game.wait = Mock(side_effect=settle)
        game.nav_to_map(2, 2, 'CeladonMansion2F', tries=1)
        game.wait.assert_called_once_with('control_ready', max_frames=240, must=False)
        game.d.drive.assert_not_called()

    def test_navigation_wait_is_scoped_to_observed_transition_not_dialogue(self):
        game = nav.Game.__new__(nav.Game)
        state = {'screen': 'overworld', 'warp_fade': 'Idle', 'player_movement_state': 'Idle'}
        game.st = lambda: state.copy()
        game.wait = Mock()
        self.assertEqual(game.navigation_state(), state)
        game.wait.assert_not_called()
        state['dialogue_state'] = {'waiting_for_input': True}
        game.navigation_state()
        game.wait.assert_not_called()
        for changes in ({'door_exit_pending': True}, {'player_movement_state': 'Walking'}):
            previous = dict(state)
            state.update(changes)
            game.navigation_state()
            game.wait.assert_called_once_with('control_ready', max_frames=240, must=False)
            game.wait.reset_mock()
            state.clear()
            state.update(previous)

    def test_navigation_settles_warp_before_evaluating_segment_result(self):
        game = nav.Game.__new__(nav.Game)
        state = {'screen': 'overworld', 'map_name': 'Route16', 'player_x': 15, 'player_y': 13}
        game.st = lambda: state.copy()
        game.last_map = 'Route16'
        game.npc_blocked = game.live_npcs = lambda _: set()
        def drive(buttons, frames):
            state.update(player_y=12, warp_fade='FadingIn { frames_remaining: 3 }')
        game.d = SimpleNamespace(drive=drive, step=lambda _: None)
        game.wait = Mock(side_effect=lambda *args, **kwargs: state.update(warp_fade='Idle'))
        with patch.object(nav, 'bfs_cross', return_value=[
                ('Route16', 15, 13), (('Route16', 15, 12), 'up')]):
            game.nav_to_map(15, 12, 'Route16', tries=2, avoid_grass=False)
        game.wait.assert_called_once_with('control_ready', max_frames=240, must=False)

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

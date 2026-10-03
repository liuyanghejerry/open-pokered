"""Autonomous skill contracts: real-input boundary and grounded preparation."""
import json
import sys
import time
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from openpokered.playthrough_judgments import ObservedProtocol, move_question, JevGame, replacement_options, medicine_options
from openpokered.autonomous_story import (AutonomousStoryAgent, counter_approaches, reachable_grass,
                                          training_tile, battle_readiness, encounter_value,
                                          training_battler, storage_deposit_indices,
                                          level_experience, evolution_training_cost, training_yield)
from openpokered.autonomous_story import compact_strategy_candidates, evolution_training_effort, factor_strategy_evidence, capture_inventory_risk
from openpokered.story_agent import DualStoryAgent
from openpokered.typesafe import TypeSafeError
from openpokered.story_agent import StoryStopped
from openpokered.navigation_skills import cut_requirement, surf_requirement, water_tile, hm_compatible, water_planning
from openpokered.story_rules import Rule
from openpokered.run_autonomous import observations_valid, checkpoint_field_requirements, checkpoint_first_clear_verification


class AutonomousTests(unittest.TestCase):
    def test_cut_execution_waits_for_observed_deferred_map_change(self):
        from openpokered.autonomous_story import CUT_TILES
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game, agent.client = Mock(), Mock()
        agent.max_actions, agent.actions = 3, 0
        agent.record, agent.check_budget, agent.remember_travel_result = Mock(), Mock(), Mock()
        agent.travel = Mock(return_value={'result': 'reached'})
        agent.game.st.return_value = {'map_name': 'Route9', 'frame_count': 3239}
        agent.cleared_terrain = set()
        target = ('terrain', 'Route9,5,8', True)
        rule = Rule('tree', 'Route9', 'skill:field', [], [], [], target, [])
        agent.active = {'target': target, 'rules': [rule], 'context': {
            'move': 'Cut', 'map': 'Route9', 'tree': [5, 8], 'stance': [4, 8], 'direction': 'right'}}
        with patch('openpokered.autonomous_story.data.field_move') as menu, \
                patch('openpokered.autonomous_story.pt.tile_at',
                      side_effect=[CUT_TILES['Overworld'], CUT_TILES['Overworld'], 0]):
            result = agent.execute('cut:0', rule)
        self.assertEqual(result['result'], 'tree_cleared')
        self.assertEqual(agent.cleared_terrain, {'Route9,5,8'})
        self.assertEqual(agent.client.step.call_args_list, [unittest.mock.call(2), unittest.mock.call(2)])
        menu.assert_called_once_with(agent.game, 'Cut', 0)

    def test_cut_execution_reports_failed_or_interrupted_effect_without_replaying_menu(self):
        from openpokered.autonomous_story import CUT_TILES
        for map_name in ('Route9', 'CeruleanPokecenter'):
            with self.subTest(map=map_name):
                agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
                agent.game, agent.client = Mock(), Mock()
                agent.max_actions, agent.actions = 3, 0
                agent.record, agent.check_budget, agent.remember_travel_result = Mock(), Mock(), Mock()
                agent.travel = Mock(return_value={'result': 'reached'})
                agent.game.st.return_value = {'map_name': map_name}
                agent.cleared_terrain = set()
                target = ('terrain', 'Route9,5,8', True)
                rule = Rule('tree', 'Route9', 'skill:field', [], [], [], target, [])
                agent.active = {'target': target, 'rules': [rule], 'context': {
                    'move': 'Cut', 'map': 'Route9', 'tree': [5, 8], 'stance': [4, 8], 'direction': 'right'}}
                with patch('openpokered.autonomous_story.data.field_move') as menu, \
                        patch('openpokered.autonomous_story.pt.tile_at', return_value=CUT_TILES['Overworld']):
                    result = agent.execute('cut:0', rule)
                self.assertEqual(result['result'], 'blocked')
                self.assertFalse(agent.cleared_terrain)
                menu.assert_called_once()
                if map_name == 'Route9':
                    self.assertEqual(agent.client.step.call_count, 60)
                else:
                    agent.client.step.assert_not_called()

    def test_cut_access_checks_approach_facing_tree_and_native_prerequisites(self):
        from openpokered.autonomous_story import CUT_TILES
        scenarios = [
            ('ready', True, True, True, True, True),
            ('no_path', False, True, True, True, False),
            ('no_badge', True, False, True, True, False),
            ('no_move', True, True, False, True, False),
            ('wrong_facing', True, True, True, False, False),
        ]
        for label, found, badge, knows, aligned, expected in scenarios:
            with self.subTest(label=label):
                agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
                agent.game = Mock(last_map='Route6')
                agent.game.navigation_barriers.return_value = {'Route9': {(9, 9)}}
                agent.game.live_npcs.return_value = {(3, 3)}
                agent.game.navigation_excluded_maps.return_value = ('SaffronCity',)
                agent.observed_navigation_barriers = Mock(return_value={})
                current = {'map': 'CeruleanCity', 'x': 19, 'y': 27,
                    'flags': {'EVENT_BEAT_MISTY': True} if badge else {},
                    'party': [{'moves': ['Cut'] if knows else ['Scratch'], 'hp': 0}]}
                target = ('terrain', 'Route9,5,8', True)
                group = {'target': target,
                    'rules': [Rule('tree', 'Route9', 'skill:field', [], [], [], target, [])],
                    'context': {'move': 'Cut', 'map': 'Route9', 'tree': [5, 8],
                                'stance': [5, 9], 'direction': 'up' if aligned else 'down'}}
                path = [('CeruleanCity', 19, 27), (('Route9', 5, 9), 'up')] if found else None
                with patch('openpokered.autonomous_story.pt.bfs_cross', return_value=path) as bfs, \
                        patch('openpokered.autonomous_story.pt.tile_at',
                              return_value=CUT_TILES['Overworld']):
                    groups = {'tree': group}
                    agent.annotate_navigation(groups, current, prune=False)
                route = group['context']['trigger_navigation'][0]
                self.assertEqual(route['tile_route_found'], expected)
                self.assertEqual(route['steps'], 1 if expected else None)
                self.assertEqual(route['stance'], [5, 9])
                self.assertEqual(route['field_action'], 'Cut')
                self.assertEqual(route['knows_required_move'], knows)
                self.assertEqual(route['faces_observed_tree'], aligned)
                self.assertEqual(bool(route['unmet_native_field_prerequisites']), not badge)
                self.assertIn('not prove', route['scope'])
                bfs.assert_called_once()
                self.assertEqual(bfs.call_args.args[2:], ('Route9', (5, 9)))
                self.assertEqual(set(groups), {'tree'})
                agent.game.nav_to_map.assert_not_called()

    def test_cut_access_does_not_certify_an_already_missing_tree(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock(last_map=None)
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = set()
        agent.observed_navigation_barriers = Mock(return_value={})
        current = {'map': 'Route9', 'x': 5, 'y': 9,
            'flags': {'EVENT_BEAT_MISTY': True}, 'party': [{'moves': ['Cut'], 'hp': 10}]}
        target = ('terrain', 'Route9,5,8', True)
        group = {'target': target,
            'rules': [Rule('tree', 'Route9', 'skill:field', [], [], [], target, [])],
            'context': {'move': 'Cut', 'map': 'Route9', 'tree': [5, 8],
                        'stance': [5, 9], 'direction': 'up'}}
        with patch('openpokered.autonomous_story.pt.bfs_cross', return_value=[('Route9', 5, 9)]), \
                patch('openpokered.autonomous_story.pt.tile_at', return_value=0):
            agent.annotate_navigation({'tree': group}, current, prune=False)
        route = group['context']['trigger_navigation'][0]
        self.assertFalse(route['observed_tree_present'])
        self.assertFalse(route['tile_route_found'])

    def test_level_evolution_gets_fresh_access_to_actual_training_terrain(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock(last_map=None)
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = set()
        agent.observed_navigation_barriers = Mock(return_value={})
        agent.training_navigation = {'OldMap': {'tile_route_found': True}}
        current = {'map': 'VermilionPokecenter', 'x': 13, 'y': 1, 'flags': {}}
        fresh = {
            'Route6': {'map': 'Route6', 'tile_route_found': True, 'steps': 35,
                       'scope': 'actual encounter terrain'},
            'Route9': {'map': 'Route9', 'tile_route_found': False, 'steps': None,
                       'scope': 'actual encounter terrain'},
        }

        def training(facts, *, shared_experience=False):
            self.assertIs(facts, current)
            self.assertTrue(shared_experience)
            agent.training_navigation = fresh
            return {'Route6': (4, 18), 'Route9': (8, 10)}

        agent.find_training_sites = Mock(side_effect=training)
        groups = {}
        for source, species in [('Geodude', 'Graveler'), ('Drowzee', 'Hypno')]:
            target = ('register', species, True)
            groups[species] = {'target': target,
                'rules': [Rule(species, current['map'], 'skill:evolve', [], [], [], target, [])],
                'context': {'acquisition_method': 'evolution', 'trigger': 'level',
                            'from_species': source, 'training_effort_examples': [
                                {'map': 'Route6', 'navigation': None},
                                {'map': 'OldMap', 'navigation': {'tile_route_found': True}}]}}
        agent.annotate_navigation(groups, current, prune=False)
        agent.find_training_sites.assert_called_once()
        for group in groups.values():
            routes = group['context']['trigger_navigation']
            self.assertEqual([(r['map'], r['tile_route_found'], r['steps']) for r in routes],
                             [('Route6', True, 35), ('Route9', False, None)])
            self.assertNotIn('VermilionPokecenter', [r['map'] for r in routes])
            self.assertIn('experience', routes[0]['scope'])
            examples = group['context']['training_effort_examples']
            self.assertEqual(examples[0]['navigation'], fresh['Route6'])
            self.assertIsNone(examples[1]['navigation'])
        self.assertEqual(set(groups), {'Graveler', 'Hypno'})

    def test_item_evolution_access_requires_carried_item_and_party_source(self):
        for held_item, source_in_party, expected in [(1, True, True), (0, True, False),
                                                     (1, False, False)]:
            with self.subTest(held_item=held_item, source_in_party=source_in_party):
                agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
                agent.game = Mock()
                agent.game.navigation_barriers.return_value = {}
                agent.game.live_npcs.return_value = set()
                agent.game.navigation_excluded_maps.return_value = set()
                agent.observed_navigation_barriers = Mock(return_value={})
                agent.find_training_sites = Mock()
                current = {'map': 'Route8', 'x': 1, 'y': 1, 'flags': {},
                    'bag': {'FIRESTONE': held_item},
                    'party': [{'species': 'Growlithe'}] if source_in_party else []}
                target = ('register', 'Arcanine', True)
                group = {'target': target,
                    'rules': [Rule('evolve', 'Route8', 'skill:evolve', [], [], [], target, [])],
                    'context': {'acquisition_method': 'evolution', 'trigger': 'item',
                                'item': 'FireStone', 'from_species': 'Growlithe'}}
                agent.annotate_navigation({'evolve': group}, current, prune=False)
                route = group['context']['trigger_navigation'][0]
                self.assertEqual(route['tile_route_found'], expected)
                self.assertEqual(route['steps'], 0 if expected else None)
                agent.find_training_sites.assert_not_called()

    def test_native_inputs_and_agent_judgments_share_the_boot_trace_clock(self):
        import io
        from types import SimpleNamespace
        client = Mock()
        client.state.return_value = {'map_name': 'PalletTown', 'hall_of_fame_count': 0}
        raw, old_record = Mock(), Mock()
        raw.cmd.return_value = {'ok': True, 'data': {'frame_count': 100}}
        protocol = ObservedProtocol(raw, old_record, time.monotonic()+60)
        started = time.monotonic()-10
        game = Mock(d=protocol, judgments=SimpleNamespace(start_time=started))
        trace = io.StringIO()
        agent = AutonomousStoryAgent(client, Mock(), [{'id': 'collect-dex', 'agent_verified': True}],
                                     game=game, trace=trace)
        self.assertEqual(agent.start_time, started)
        protocol.step(1)
        row = json.loads(trace.getvalue())
        self.assertEqual(row['kind'], 'native_input')
        self.assertGreaterEqual(row['elapsed_s'], 10)
        old_record.assert_not_called()

    def test_native_game_private_save_and_log_can_use_a_durable_root(self):
        import tempfile
        import playthrough as pt
        with tempfile.TemporaryDirectory() as root:
            path = Path(root)
            proc = Mock()
            proc.poll.return_value = 0
            with patch.object(pt.subprocess, 'Popen', return_value=proc), \
                    patch.object(pt, 'DebugClient'), \
                    patch.object(pt, 'load_coordinate_warps', return_value={}), \
                    patch.dict(pt.COORDINATE_WARPS, clear=True):
                game = pt.Game(port=9876, binary=path / 'pokered-app', runtime_root=path)
                try:
                    self.assertTrue(game.run_dir.is_relative_to(path))
                    self.assertTrue(game.save_path.is_relative_to(path))
                    self.assertTrue((game.run_dir / 'game.log').is_file())
                finally:
                    game.close()
                self.assertTrue(path.is_dir())  # Cleanup removes only the private child.
                self.assertFalse(game.run_dir.exists())

    def test_forced_replacement_cannot_abstain_with_a_conscious_status_only_member(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'target': ('heal', 'party', True)}
        party = [{'species': 'Charizard', 'hp': 0},
                 {'species': 'Metapod', 'hp': 25, 'moves': ['Harden'], 'pp': [40]}]
        state = {'battle_phase': 'PlayerFaintSwitch { cursor: 1 }',
                 'battle': {'is_wild': True, 'player_party': party,
                            'enemy': {'species': 'Mewtwo'}}}
        with patch.object(DualStoryAgent, 'choose', return_value='1') as choose:
            self.assertEqual(agent.choose('action', state, {'1': '{}'}, 'Choose a member.'), '1')
            self.assertFalse(choose.call_args.kwargs['allow_abstain'])
            self.assertIn('fainted', choose.call_args.args[1]['immediate_goal'])
            self.assertIn('does not guarantee', choose.call_args.args[3])
            self.assertNotIn('immediate_goal', state)
            for change in ({'battle_phase': 'PlayerParty'},
                           {'battle': {'player_party': [{'hp': 0}, {'hp': 0}]}},
                           {'battle': {'player_party': []}}):
                agent.choose('action', {**state, **change}, {'1': '{}'}, 'Choose a member.')
                self.assertTrue(choose.call_args.kwargs['allow_abstain'])
            agent.choose('action', state, {'0': '{}'}, 'Choose a member.')
            self.assertTrue(choose.call_args.kwargs['allow_abstain'])

    def test_party_target_exposes_native_phase_and_live_hp_not_stale_sram(self):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.choose.return_value = '1'
        party = [{'species': 'Charizard', 'hp': 200, 'level': 99, 'moves': ['Slash'], 'pp': [10]},
                 {'species': 'Metapod', 'hp': 25, 'level': 7, 'moves': ['Harden'], 'pp': [40]}]
        live = {'enemy': {'species': 'Mewtwo'}, 'player_party': [
            {**party[0], 'hp': 0}, party[1]]}
        state = {'party': party, 'battle_live': live, 'battle_phase': 'PlayerFaintSwitch'}
        self.assertEqual(game.battle_party_target(state), 1)
        args = game.judgments.choose.call_args.args
        self.assertEqual(args[1]['battle_phase'], 'PlayerFaintSwitch')
        self.assertEqual(args[1]['battle'], live)
        self.assertEqual(set(args[2]), {'1'})
        self.assertEqual(json.loads(args[2]['1'])['usable_effective_attacks'], [])
        self.assertEqual(party[0]['hp'], 200)
        game.battle_party_target(state)
        self.assertEqual(game.judgments.choose.call_count, 1)
        game.battle_party_target({**state, 'battle_phase': 'PlayerParty'})
        self.assertEqual(game.judgments.choose.call_count, 2)

    def test_settled_checkpoint_publication_also_requires_an_unshifted_save_ack(self):
        import io
        import tempfile
        from openpokered import run_autonomous
        state = {'screen': 'overworld', 'warp_fade': 'Idle', 'player_movement_state': 'Idle',
                 'script_running': False, 'script_awaiting_battle': False,
                 'door_exit_pending': False, 'active_script_effect': None,
                 'dialogue': None, 'choice': None, 'field_menu': None, 'fishing_active': False}
        for save_reply, published in (({'ok': True, 'data': None}, True),
                                      ({'ok': True, 'data': {'screen': 'overworld'}}, False),
                                      ({'ok': False, 'error': 'save failed'}, False)):
            with self.subTest(reply=save_reply), tempfile.TemporaryDirectory() as root:
                path = Path(root)
                (path / 'pokered-app').write_bytes(b'binary')
                (path / 'game.sav').write_bytes(b'native SRAM')
                (path / 'game.log').write_text('native log')
                game = Mock()
                game.run_dir, game.save_path, game.d.counts = path, path / 'game.sav', {}
                game.battles_driven, game.move_cache_hits = 0, 0
                data = {'get_state': state, 'get_flags': {}, 'get_party': [], 'get_bag': [], 'get_npcs': []}
                def reply(**request):
                    return save_reply if request['cmd'] == 'save' else {'ok': True, 'data': data.get(request['cmd'])}
                game.d.raw.cmd.side_effect = reply
                with patch.object(run_autonomous, 'TypeSafeClient'), \
                        patch.object(run_autonomous, 'JevGame', return_value=game), \
                        patch.object(run_autonomous, 'boot_new_game', side_effect=RuntimeError('interrupted')), \
                        patch('sys.stdout', new_callable=io.StringIO):
                    run_autonomous.main(['--checkpoint', '--binary', str(path / 'pokered-app'),
                                         '--output', str(path / 'out')])
                folder = next((path / 'out').iterdir())
                summary = json.loads((folder / 'summary.json').read_text())
                self.assertTrue(summary['native_checkpoint_safe'])
                self.assertEqual(summary['development_checkpoint'], published)
                self.assertEqual((folder / 'game.sav').exists(), published)
                self.assertEqual([call.kwargs['cmd'] for call in game.d.raw.cmd.call_args_list].count('save'), 1)
                if not published:
                    self.assertEqual(summary['reason'], 'invalid_checkpoint_acknowledgement')
                    self.assertEqual((folder / 'nonresumable-native-save.sav').read_bytes(), b'native SRAM')

    def test_malformed_final_state_keeps_failure_evidence_without_a_checkpoint(self):
        import io
        import tempfile
        from openpokered import run_autonomous
        with tempfile.TemporaryDirectory() as root:
            path = Path(root)
            (path / 'pokered-app').write_bytes(b'binary')
            (path / 'game.sav').write_bytes(b'unverified SRAM')
            (path / 'game.log').write_text('native log')
            game = Mock(run_dir=path, save_path=path / 'game.sav', battles_driven=0, move_cache_hits=0)
            game.d.counts = {}
            data = {'get_state': None, 'get_flags': {}, 'get_party': [], 'get_bag': [], 'get_npcs': []}
            game.d.raw.cmd.side_effect = lambda **r: {'ok': True, 'data': data.get(r['cmd'])}
            with patch.object(run_autonomous, 'TypeSafeClient'), \
                    patch.object(run_autonomous, 'JevGame', return_value=game), \
                    patch.object(run_autonomous, 'boot_new_game', side_effect=RuntimeError('interrupted')), \
                    patch('sys.stdout', new_callable=io.StringIO):
                code = run_autonomous.main(['--checkpoint', '--binary', str(path / 'pokered-app'),
                                            '--output', str(path / 'out')])
            folder = next((path / 'out').iterdir())
            summary = json.loads((folder / 'summary.json').read_text())
            self.assertEqual(code, 1)
            self.assertEqual(summary['reason'], 'invalid_final_protocol_observations')
            self.assertFalse(summary['development_checkpoint'])
            self.assertFalse((folder / 'game.sav').exists())
            self.assertEqual((folder / 'nonresumable-native-save.sav').read_bytes(), b'unverified SRAM')
            self.assertTrue((folder / 'final-observations.json').exists())
            self.assertTrue((folder / 'game.log').exists())
            game.close.assert_called_once()

    def test_collection_completion_cannot_publish_or_verify_a_battle_checkpoint(self):
        import io
        import tempfile
        from openpokered import run_autonomous
        with tempfile.TemporaryDirectory() as root:
            path = Path(root)
            (path / 'pokered-app').write_bytes(b'binary')
            (path / 'game.sav').write_bytes(b'prebattle SRAM')
            (path / 'game.log').write_text('native log')
            game = Mock(run_dir=path, save_path=path / 'game.sav', battles_driven=0, move_cache_hits=0)
            game.d.counts = {}
            data = {'get_state': {'screen': 'battle'}, 'get_flags': {},
                    'get_party': [], 'get_bag': [], 'get_npcs': []}
            game.d.raw.cmd.side_effect = lambda **r: {'ok': True, 'data': data.get(r['cmd'])}
            agent = Mock(visited=set(), observed_barrier_maps=set(), navigation_memory={},
                navigation_history={}, mechanism_goal=None, field_requirements={}, battle_requirements={},
                capture_retreats={}, capture_retreat_totals={}, collection_audit_pending={},
                battle_defeats=[], defeat_preparation=0, first_clear_verification=None,
                calls={}, tokens={}, completed=[], actions=0, models=set(), resolved_battles=0)
            agent.run.return_value = {'success': True}
            game.stationary_npcs = {}
            with patch.object(run_autonomous, 'TypeSafeClient'), \
                    patch.object(run_autonomous, 'JevGame', return_value=game), \
                    patch.object(run_autonomous, 'AutonomousStoryAgent', return_value=agent), \
                    patch.object(run_autonomous, 'boot_new_game', return_value=data['get_state']), \
                    patch.object(run_autonomous, 'require_collection_completion') as require, \
                    patch.object(run_autonomous, 'verify_collection_continue') as verify, \
                    patch.object(run_autonomous.signal, 'signal'), \
                    patch('sys.stdout', new_callable=io.StringIO):
                code = run_autonomous.main(['--goal', 'collect-dex', '--binary', str(path / 'pokered-app'),
                                            '--output', str(path / 'out')])
            folder = next((path / 'out').iterdir())
            summary = json.loads((folder / 'summary.json').read_text())
            self.assertEqual(code, 1)
            self.assertFalse(summary['success'])
            self.assertEqual(summary['reason'], 'unsafe_collection_checkpoint')
            self.assertFalse(summary['development_checkpoint'])
            self.assertFalse((folder / 'game.sav').exists())
            require.assert_not_called()
            verify.assert_not_called()

    def test_native_checkpoint_requires_settled_overworld_not_just_valid_json(self):
        from openpokered.run_autonomous import native_checkpoint_safe
        state = {'screen': 'overworld', 'warp_fade': 'Idle', 'player_movement_state': 'Idle',
                 'script_running': False, 'script_awaiting_battle': False,
                 'door_exit_pending': False, 'active_script_effect': None,
                 'dialogue': None, 'choice': None, 'field_menu': None, 'fishing_active': False}
        data = {'get_state': state, 'get_flags': {}, 'get_party': [], 'get_bag': [], 'get_npcs': []}
        observations = {cmd: {'ok': True, 'data': value} for cmd, value in data.items()}
        self.assertTrue(native_checkpoint_safe(observations))
        for change in ({'screen': 'battle'}, {'screen': 'start_menu'},
                       {'warp_fade': 'FadingOut'}, {'player_movement_state': 'Walking'},
                       {'script_running': True}, {'script_awaiting_battle': True},
                       {'door_exit_pending': True}, {'active_script_effect': 'Delay'},
                       {'dialogue': 'still open'}, {'choice': {'cursor': 0}},
                       {'field_menu': {'kind': 'bag'}}, {'fishing_active': True}):
            with self.subTest(change=change):
                unsafe = {**observations, 'get_state': {'ok': True, 'data': {**state, **change}}}
                self.assertTrue(observations_valid(unsafe))
                self.assertFalse(native_checkpoint_safe(unsafe))
        for malformed in (None, [], {}, {'get_state': {'ok': True, 'data': None}}):
            self.assertFalse(native_checkpoint_safe(malformed))
        for missing in ('warp_fade', 'player_movement_state', 'script_running', 'dialogue'):
            with self.subTest(missing=missing):
                partial = {key: value for key, value in state.items() if key != missing}
                self.assertFalse(native_checkpoint_safe({**observations,
                    'get_state': {'ok': True, 'data': partial}}))

    def test_unsafe_final_state_keeps_diagnostic_sram_but_never_publishes_checkpoint(self):
        import io
        import tempfile
        from openpokered import run_autonomous
        with tempfile.TemporaryDirectory() as root:
            path = Path(root)
            (path / 'pokered-app').write_bytes(b'binary')
            (path / 'game.sav').write_bytes(b'old uncompleted SRAM')
            (path / 'game.log').write_text('native log')
            game = Mock()
            game.run_dir, game.save_path, game.d.counts = path, path / 'game.sav', {}
            game.battles_driven, game.move_cache_hits = 0, 0
            data = {'get_state': {'screen': 'battle'}, 'get_flags': {},
                    'get_party': [], 'get_bag': [], 'get_npcs': []}
            def reply(**request):
                return {'ok': True, 'data': data.get(request['cmd'])}
            game.d.raw.cmd.side_effect = reply
            with patch.object(run_autonomous, 'TypeSafeClient'), \
                    patch.object(run_autonomous, 'JevGame', return_value=game), \
                    patch.object(run_autonomous, 'boot_new_game', side_effect=RuntimeError('interrupted')), \
                    patch.object(run_autonomous.signal, 'signal'), \
                    patch('sys.stdout', new_callable=io.StringIO):
                code = run_autonomous.main(['--checkpoint', '--binary', str(path / 'pokered-app'),
                                            '--output', str(path / 'out')])
            folder = next((path / 'out').iterdir())
            summary = json.loads((folder / 'summary.json').read_text())
            self.assertEqual(code, 1)
            self.assertTrue(summary['final_observations_valid'])
            self.assertFalse(summary['native_checkpoint_safe'])
            self.assertFalse(summary['development_checkpoint'])
            self.assertFalse((folder / 'game.sav').exists())
            self.assertEqual((folder / 'nonresumable-native-save.sav').read_bytes(), b'old uncompleted SRAM')
            self.assertNotIn('save', [call.kwargs['cmd'] for call in game.d.raw.cmd.call_args_list])
            game.close.assert_called_once()

    def test_resume_rejects_historical_battle_checkpoint_before_spawning_game(self):
        import io
        import tempfile
        from openpokered import run_autonomous
        with tempfile.TemporaryDirectory() as root:
            path = Path(root)
            (path / 'pokered-app').write_bytes(b'binary')
            parent = path / 'parent'
            parent.mkdir()
            (parent / 'summary.json').write_text(json.dumps({
                'mode': 'autonomous-new-game', 'development_checkpoint': True}))
            (parent / 'game.sav').write_bytes(b'not a runtime snapshot')
            data = {'get_state': {'screen': 'battle'}, 'get_flags': {},
                    'get_party': [], 'get_bag': [], 'get_npcs': []}
            (parent / 'final-observations.json').write_text(json.dumps({
                cmd: {'ok': True, 'data': value} for cmd, value in data.items()}))
            with patch.object(run_autonomous, 'TypeSafeClient'), \
                    patch.object(run_autonomous, 'JevGame') as factory, \
                    patch('sys.stderr', new_callable=io.StringIO), \
                    self.assertRaises(SystemExit) as stopped:
                run_autonomous.main(['--resume', str(parent), '--binary', str(path / 'pokered-app'),
                                     '--output', str(path / 'out')])
            self.assertEqual(stopped.exception.code, 2)
            factory.assert_not_called()

    def test_native_input_log_records_acknowledged_atomic_requests_not_reads_or_timeouts(self):
        raw, record = Mock(), Mock()
        raw.cmd.return_value = {'ok': True, 'data': {'advanced': True,
            'queue_start_frame': 100, 'frame_count': 103}}
        protocol = ObservedProtocol(raw, record, time.monotonic()+60)
        protocol.drive([None, 'a', None])
        record.assert_called_once_with('native_input', request={
            'cmd': 'press_timeline', 'buttons': [None, 'a', None], 'advance': True}, ok=True, frame=103)
        record.reset_mock()
        protocol.cmd(cmd='get_state')
        record.assert_not_called()
        raw.cmd.side_effect = TimeoutError('transport was not acknowledged')
        with self.assertRaises(TimeoutError):
            protocol.cmd(cmd='step_frames', count=1)
        record.assert_not_called()
        raw.cmd.side_effect = None
        raw.cmd.return_value = {'ok': False, 'error': 'blocked'}
        protocol.cmd(cmd='move_to', x=1, y=2)
        record.assert_called_once_with('native_input', request={'cmd': 'move_to', 'x': 1, 'y': 2},
                                       ok=False, frame=None)

    def test_native_input_log_retains_nested_wait_and_dialogue_frame_acknowledgements(self):
        raw, record = Mock(), Mock()
        protocol = ObservedProtocol(raw, record, time.monotonic()+60)
        for command in ('wait_until', 'skip_dialogue'):
            with self.subTest(command=command):
                record.reset_mock()
                raw.cmd.return_value = {'ok': True, 'data': {'stepped': 10,
                                        'state': {'frame_count': 1234}}}
                protocol.cmd(cmd=command)
                record.assert_called_once_with('native_input', request={'cmd': command},
                                               ok=True, frame=1234)

    def test_native_input_frame_acknowledgement_does_not_invent_malformed_telemetry(self):
        raw, record = Mock(), Mock()
        protocol = ObservedProtocol(raw, record, time.monotonic()+60)
        for data, expected in ((None, None), ({'state': None}, None),
                               ({'frame_count': False}, None), ({'frame_count': -1}, None),
                               ({'frame_count': '12'}, None),
                               ({'frame_count': 10, 'state': {'frame_count': 9}}, 10),
                               ({'frame_count': None, 'state': {'frame_count': 12}}, 12)):
            with self.subTest(data=data):
                record.reset_mock()
                raw.cmd.return_value = {'ok': True, 'data': data}
                protocol.cmd(cmd='step_frames', count=1)
                record.assert_called_once_with('native_input', request={'cmd': 'step_frames', 'count': 1},
                                               ok=True, frame=expected)

    def test_safari_ball_sequence_counts_capture_before_flee_and_budget(self):
        from openpokered.playthrough_judgments import safari_ball_sequence
        self.assertEqual(safari_ball_sequence(.2, .5, 0), (0, 0))
        self.assertEqual(safari_ball_sequence(.2, 1, 30), (.2, 1))
        self.assertEqual(safari_ball_sequence(1, .5, 30), (1, 1))
        self.assertEqual(safari_ball_sequence(0, 0, 5), (0, 5))
        success, spent = safari_ball_sequence(.2, .5, 2)
        self.assertAlmostEqual(success, .28)
        self.assertAlmostEqual(spent, 1.4)
        self.assertAlmostEqual(safari_ball_sequence(.2, 0, 5)[0], 1 - .8 ** 5)

    def test_safari_reference_uses_public_stats_and_bounds_actual_encounter(self):
        from openpokered.autonomous_story import safari_capture_reference
        from openpokered.playthrough_judgments import capture_probability, safari_ball_sequence
        table = {'encounterRate': 30, 'mons': [{'species': 'Kangaskhan', 'level': 25}] * 10}
        reference = safari_capture_reference(table)
        # The observed resume69 encounter: HP 91, Speed 56. No unseen DV is supplied.
        p = capture_probability('SafariBall', {'hp': 91, 'max_hp': 91, 'catch_rate': 45})
        actual, _ = safari_ball_sequence(p, 112 / 256, 30)
        row = reference['targets'][0]
        low, high = row['capture_before_flee_probability_range']
        self.assertLess(low, actual)
        self.assertGreater(high, actual)
        self.assertLess(high, .3)  # Seeing this species is far from registering it.
        self.assertEqual(reference['ball_budget_per_encounter'], 30)
        self.assertIn('not a forecast', reference['scope'])
        self.assertIn('shared', reference['scope'])
        per_step = reference['new_registration_per_eligible_step_pct_range']
        self.assertLess(per_step[1], 30 / 256 * 100)

    def test_safari_reference_respects_slots_levels_owned_and_zero_balls(self):
        from openpokered.autonomous_story import safari_capture_reference
        table = {'encounterRate': 30, 'mons': [
            {'species': 'Kangaskhan', 'level': 25}, {'species': 'Kangaskhan', 'level': 28},
            *[{'species': 'Paras', 'level': 20}] * 8]}
        reference = safari_capture_reference(table, {'Paras'})
        self.assertEqual([(r['species'], r['level']) for r in reference['targets']],
                         [('Kangaskhan', 25), ('Kangaskhan', 28)])
        self.assertEqual([r['slot_weight_per_256'] for r in reference['targets']], [51, 51])
        empty = safari_capture_reference(table, {'Paras', 'Kangaskhan'})
        exhausted = safari_capture_reference(table, balls=0)
        for result in (empty, exhausted):
            self.assertEqual(result['new_registration_per_eligible_step_pct_range'], [0, 0])
            self.assertEqual(result['expected_eligible_steps_to_registration_range'], [None, None])

    def test_safari_strategy_value_adds_reference_without_changing_encounter_candidates(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        table = {'encounterRate': 30, 'mons': [{'species': 'Kangaskhan', 'level': 25}] * 10}
        agent.maps = {'SafariZoneEast': {'wild': {'red': {'grass': table}}}}
        grass = agent.method_value('grass', 'SafariZoneEast', set())
        safari = agent.method_value('safari', 'SafariZoneEast', set())
        self.assertNotIn('safari_registration_reference', grass)
        self.assertIn('safari_registration_reference', safari)
        self.assertEqual(grass['targets'], safari['targets'])
        self.assertEqual(grass['expected_attempts_to_any_new_species'],
                         safari['expected_attempts_to_any_new_species'])

    def test_surf_probe_finds_a_shore_when_shortest_route_first_falls_into_water(self):
        import playthrough as pt
        falls = {
            'SeafoamIslands1F': {(17, 6): ('SeafoamIslandsB1F', 18, 7),
                                 (24, 6): ('SeafoamIslandsB1F', 23, 7)},
            'SeafoamIslandsB1F': {(18, 6): ('SeafoamIslandsB2F', 19, 7),
                                  (23, 6): ('SeafoamIslandsB2F', 22, 7)},
            'SeafoamIslandsB2F': {(19, 6): ('SeafoamIslandsB3F', 18, 7),
                                  (22, 6): ('SeafoamIslandsB3F', 19, 7)},
        }
        state = {'map_name': 'Route20', 'player_x': 48, 'player_y': 6, 'player_transport': 'Walking'}
        with patch.object(pt, 'COORDINATE_WARPS', falls):
            obstacle = surf_requirement(state, 'CinnabarPokecenter', [(3, 3)], 'Route20')
            self.assertIsNotNone(obstacle)
            self.assertIn(obstacle['direction'], pt.DELTA)
            self.assertTrue(pt.bfs_cross('Route20', (48, 6), obstacle['map'], obstacle['stance']))
            self.assertFalse(pt.bfs_cross('Route20', (48, 6), obstacle['landing'][0], obstacle['landing'][1:]))
            # General route planning still knows the legal automatic fall.
            with water_planning():
                plan = pt.bfs_cross('SeafoamIslandsB2F', (19, 7), 'SeafoamIslandsB3F', (18, 7))
            self.assertEqual(plan, [('SeafoamIslandsB2F', 19, 7),
                                   (('SeafoamIslandsB3F', 18, 7), 'fall_up')])

    def test_scripted_fall_retains_approach_and_explicit_step_action(self):
        import playthrough as pt
        from types import SimpleNamespace
        from openpokered.story_rules import MAPS_DIR
        name = 'SeafoamIslands1F'
        rule = Rule('hole', name, name + ':coordHole2', ['coord:(24,6)'], [], [],
                    ('transport', ('SeafoamIslandsB1F', 23, 7), True), [])
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        agent.index = SimpleNamespace(coordinates=lambda _: [(24, 6)])
        agent.active = {'target': rule.effect, 'rules': [rule]}
        agent.client = Mock()
        agent.client.cmd.return_value = []
        agent.client.route.return_value = {'legs': []}
        agent.visited = {name}
        with patch.object(pt, 'COORDINATE_WARPS', {name: {(24, 6): rule.effect[1]}}):
            points = agent.destination_points(name, rule, allow_entry_fallback=False)
            self.assertIn((25, 6), points)
            self.assertNotIn((24, 6), points)
            with patch.object(DualStoryAgent, 'action_candidates', return_value=(
                    {'step': '{}'}, {'step': ('move_to:24,6', rule)})):
                _, bindings = agent.action_candidates({'map': name, 'x': 25, 'y': 6})
            self.assertEqual(bindings['step'], ('move_to:24,6', rule))

    def test_access_panel_distinguishes_missing_paths_from_unknown_without_pruning(self):
        from openpokered.autonomous_story import strategy_access_evidence
        from copy import deepcopy
        def candidate(target, routes=(), **context):
            return json.dumps({'establish': target, 'context': {**context, 'trigger_navigation': [
                {'map': f'Room{index}', **route} for index, route in enumerate(routes)]}})
        candidates = {
            'reachable': candidate(['heal', 'party', True], [{'tile_route_found': True}, {'tile_route_found': False}]),
            'blocked': candidate(['level', 'Drowzee', 19], [{'tile_route_found': False}]),
            'compact': candidate(['level', 'Parasect', 31], unreachable_trigger_maps=['Grass']),
            'unknown': candidate(['terrain', 'Tree', True]),
            'partial': candidate(['item', 'Key', True], [{'tile_route_found': False}, {}]),
            'water': candidate(['flag', 'Boulder', True], [{'tile_route_found': True, 'requires_surf': True}]),
            'none': 'No suitable action',
        }
        original = deepcopy(candidates)
        panel = strategy_access_evidence(candidates)
        self.assertEqual(set(panel['path_found']), {'reachable'})
        self.assertEqual(set(panel['no_path_found']), {'blocked', 'compact'})
        self.assertEqual(set(panel['not_evaluated']), {'unknown', 'partial'})
        self.assertEqual(set(panel['field_action_needed']), {'water'})
        self.assertEqual(candidates, original)
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = None
        agent.choose_bounded_strategy = Mock(return_value='blocked')
        self.assertEqual(agent.choose('strategy', {}, candidates, 'Compare'), 'blocked')
        state, retained, instruction = agent.choose_bounded_strategy.call_args.args
        self.assertIn('immediate_access_comparison', state)
        self.assertEqual(set(retained), set(candidates))
        self.assertIn('concrete new access', instruction)

    def test_surf_frontier_is_grounded_at_its_dry_stance_not_remote_landing(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock(last_map='Town')
        agent.game.navigation_barriers.return_value = {}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.game.live_npcs.return_value = set()
        agent.observed_navigation_barriers = Mock(return_value={})
        target = ('location', ['FarTown', 4, 13], True)
        rule = Rule('surf', 'Island', 'skill:surf', [], [], [], target, [])
        group = {'target': target, 'rules': [rule], 'context': {
            'move': 'Surf', 'map': 'Island', 'stance': [4, 10], 'landing': target[1]}}
        facts = {'map': 'Island', 'x': 8, 'y': 8, 'flags': {'EVENT_BEAT_KOGA': True},
                 'party': [{'moves': ['Surf']}]}
        for known, badge, path, expected in [(True, True, ['start', 'step'], True),
                (False, True, ['start'], False), (True, False, ['start'], False), (True, True, None, False)]:
            current = {**facts, 'party': [{'moves': ['Surf'] if known else []}],
                       'flags': {'EVENT_BEAT_KOGA': badge}}
            with patch('playthrough.bfs_cross', return_value=path) as bfs:
                agent.annotate_navigation({'surf': group}, current, prune=False)
            route = group['context']['trigger_navigation'][0]
            self.assertEqual(route['tile_route_found'], expected)
            self.assertEqual(route['stance'], [4, 10])
            self.assertEqual(bfs.call_args.args[2:4], ('Island', (4, 10)))
            self.assertNotIn('requires_surf', route)  # This is the offered move, not another prerequisite.

    def test_recording_rejects_missing_assets_and_fingerprints_png_changes(self):
        import tempfile
        from openpokered.run_autonomous import recording_assets
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            with self.assertRaisesRegex(ValueError, 'fetch-gfx'):
                recording_assets(root)
            for name in ('sprites/red.png', 'tilesets/overworld.png', 'font/font.png',
                         'pokemon/front/charizard.png', 'pokemon/back/charizardb.png'):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'\x89PNG\r\n\x1a\nfixture')
            first = recording_assets(root)
            self.assertEqual(first['png_count'], 5)
            (root / 'sprites/red.png').write_bytes(b'\x89PNG\r\n\x1a\nchanged')
            self.assertNotEqual(first['sha256'], recording_assets(root)['sha256'])
            (root / 'sprites/red.png').write_text('version https://git-lfs.github.com/spec/v1')
            with self.assertRaisesRegex(ValueError, 'PNG asset'):
                recording_assets(root)

    def test_cross_search_reports_all_reachable_goals_not_the_first_only(self):
        import playthrough as pt
        name = 'CinnabarIsland'
        targets = {(name, 0, 0), (name, 1, 0), (name, 2, 0), (name, 9, 0)}
        def step(cm, x, y, direction):
            if direction == 'right' and x < 2:
                return cm, x + 1, y
            if direction == 'left' and x > 0:
                return cm, x - 1, y
            return None
        with patch.object(pt, 'cross_step', side_effect=step), patch.object(pt, 'warp_tiles', return_value=set()):
            reached = pt.bfs_cross(name, (0, 0), name, (1, 0), goal_nodes=targets,
                                   reachable_goals=True)
            self.assertEqual(reached, targets - {(name, 9, 0)})
            path = pt.bfs_cross(name, (0, 0), name, (1, 0))
            self.assertEqual(path[-1][0], (name, 1, 0))
            self.assertEqual(pt.bfs_cross(name, (0, 0), name, (1, 0), goal_nodes=set(),
                                          reachable_goals=True), set())

    def test_transport_reports_only_goals_reachable_from_its_landing(self):
        from types import SimpleNamespace
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        arrival = ('SeafoamIslandsB1F', 18, 7)
        entrance = Rule('hole', 'SeafoamIslands1F', 'Seafoam:hole', ['coord:hole'], [], [],
                        ('transport', arrival, True), [])
        agent.index = SimpleNamespace(rules=[entrance], frontier=Mock(return_value=[entrance]))
        agent.game = Mock(last_map='Route20')
        agent.game.navigation_barriers.return_value = {}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.destination_points = Mock(side_effect=lambda name, rule: [(3, 4)])
        groups = {name: {'rules': [Rule(name, name, name + ':goal', [], [], [], ('flag', name, True), [])],
                         'context': {'trigger_navigation': [{'map': name, 'tile_route_found': False}]}}
                  for name in ('SeafoamIslandsB4F', 'PokemonMansionB1F', 'SilphCo11F')}
        with patch('playthrough.bfs_cross', return_value={('SeafoamIslandsB4F', 3, 4)}) as search:
            self.assertTrue(agent.transport_frontiers(groups, {}))
        context = groups[json.dumps(entrance.effect)]['context']
        self.assertEqual(context['blocked_destinations'], ['SeafoamIslandsB4F'])
        self.assertTrue(search.call_args.kwargs['reachable_goals'])
        self.assertIn(('PokemonMansionB1F', 3, 4), search.call_args.kwargs['goal_nodes'])

    def test_inaccessible_transport_can_offer_its_reachable_causal_door(self):
        from types import SimpleNamespace
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        goal = Rule('goal', 'PalletTown', 'Town:goal', ['npc:1'], [], [], ('heal', 'party', True), [])
        transport = Rule('exit', 'HallOfFame', 'HallOfFame:@load', ['load'], [], [],
            ('transport', ('PalletTown', 5, 6), True), [('ending', 'hall_of_fame_and_credits', True)])
        door = Rule('door', 'BrunosRoom', 'BrunosRoom:battle', ['npc:1'], [], [],
            ('flag', 'DOOR_OPEN', True), [('battle', 'TRAINER', True)])
        agent.index = SimpleNamespace(rules=[transport], frontier=Mock(return_value=[door]))
        agent.game = Mock(last_map='IndigoPlateau')
        agent.game.navigation_barriers.return_value = {}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.destination_points = Mock(return_value=[(4, 2)])
        agent.discover_route_prerequisites = Mock(return_value=[door.effect])
        facts = {'map': 'LoreleisRoom', 'x': 4, 'y': 2, 'party': []}
        def search(source, position, destination, point, **kwargs):
            if source == 'PalletTown':
                return {('PalletTown', 4, 2)}
            return ['real path'] if destination == 'BrunosRoom' else None
        groups = {'heal': {'target': goal.effect, 'rules': [goal], 'context': {
            'trigger_navigation': [{'map': 'PalletTown', 'tile_route_found': False}]}}}
        with patch('playthrough.bfs_cross', side_effect=search):
            self.assertTrue(agent.transport_frontiers(groups, facts))
        self.assertEqual(groups[json.dumps(door.effect)]['rules'], [door])
        self.assertNotIn(json.dumps(transport.effect), groups)  # No fictitious instant teleport.
        self.assertEqual(groups[json.dumps(door.effect)]['context']['transport_script']['produces'], transport.effect)
        agent.discover_route_prerequisites.assert_called_once_with(
            {'map_name': 'LoreleisRoom', 'player_x': 4, 'player_y': 2}, 'HallOfFame', [(4, 2)])
        # An inaccessible producer is not an executable first step, even if
        # the hypothetical transport would reach the desired region.
        groups = {'heal': groups['heal']}
        with patch('playthrough.bfs_cross', side_effect=lambda source, *a, **k:
                   {('PalletTown', 4, 2)} if source == 'PalletTown' else None):
            self.assertFalse(agent.transport_frontiers(groups, facts))
        self.assertEqual(list(groups), ['heal'])

    def test_surf_completion_retains_the_blocked_parent_not_the_shore_goal(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        obstacle = {'map': 'Route20', 'stance': [58, 11], 'direction': 'down',
                    'landing': ['CinnabarIsland', 19, 5], 'destination': 'PokemonMansion1F'}
        parent = ['flag', 'EVENT_MANSION_SWITCH_ON', False]
        agent.active = {'context': obstacle}
        agent.navigation_memory = {'PokemonMansion1F': {'goal': parent}}
        agent.actions, agent.max_actions = 0, 10
        agent.client, agent.game, agent.record = Mock(), Mock(), Mock()
        agent.client.flags.return_value = {}
        agent.game.st.return_value = {'player_transport': 'Surfing'}
        agent.navigate_point = Mock()
        agent.field_requirements, agent.crossed_passages = {'Surf': obstacle}, set()
        rule = Rule('water', 'Route20', 'skill:surf', [], [], [], (), [])
        self.assertEqual(agent.execute('surf:1', rule)['result'], 'crossed_water')
        self.assertEqual(agent.route_continuation['goal'], parent)
        self.assertEqual(agent.route_continuation['destination'], 'PokemonMansion1F')
        self.assertEqual(agent.route_continuation['landing'], obstacle['landing'])

    def test_completed_route_context_expires_on_goal_completion_or_departure(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = False
        agent.index = Mock()
        continuation = {'goal': ['flag', 'WON', True], 'destination': 'City',
                        'landing': ['City', 4, 5]}
        agent.route_continuation = continuation
        agent.index.satisfied.return_value = False
        state = {}
        agent.augment_strategy_state(state, {'map': 'City'})
        self.assertEqual(state['completed_route_prerequisite'], continuation)
        agent.augment_strategy_state({}, {'map': 'Other'})
        self.assertIsNone(agent.route_continuation)
        agent.route_continuation = continuation
        agent.index.satisfied.return_value = True
        agent.augment_strategy_state({}, {'map': 'City'})
        self.assertIsNone(agent.route_continuation)

    def test_route_continuation_guidance_keeps_all_candidate_choices(self):
        from openpokered.story_agent import DualStoryAgent
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        options = {'a': 'Continue parent', 'b': 'Heal', 'c': 'Investigate new blocker'}
        state = {'completed_route_prerequisite': {'destination': 'City'}}
        with patch.object(DualStoryAgent, 'choose', return_value='a') as choose:
            agent.choose('strategy', state, options, 'pick')
        self.assertEqual(choose.call_args.args[2], options)
        self.assertIn('recorded parent goal', choose.call_args.args[3])
        self.assertIn('Urgent healing', choose.call_args.args[3])

    def test_route_continuation_reaches_action_layer_for_the_same_parent_goal(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = Mock()
        agent.index.satisfied.return_value = False
        continuation = {'goal': ['heal', 'party', True], 'destination': 'CinnabarPokecenter',
                        'landing': ['SeafoamIslandsB3F', 23, 9],
                        'evidence': 'Real Surf crossing completed'}
        agent.route_continuation = continuation
        state = {'subgoal': ('heal', 'party', True),
                 'local_state': {'map': 'SeafoamIslandsB3F'}}
        options = {key: json.dumps({'operation': operation}) for key, operation in (
            ('a', 'travel_to:CinnabarPokecenter'), ('b', 'travel_to:FuchsiaPokecenter'),
            ('c', 'lead_with:Charizard'))}
        with patch.object(DualStoryAgent, 'choose', return_value='b') as choose:
            self.assertEqual(agent.choose('action', state, options, 'pick'), 'b')
        forwarded_state, forwarded_options, instruction = choose.call_args.args[1:4]
        self.assertEqual(forwarded_state['completed_route_prerequisite'], continuation)
        self.assertNotIn('completed_route_prerequisite', state)
        self.assertEqual(forwarded_options, options)  # Evidence, not a forced route.
        self.assertIn('recorded parent goal', instruction)
        self.assertIn('different destination', instruction)
        self.assertIn('newly observed blocker', instruction)

    def test_route_continuation_does_not_leak_into_unrelated_or_stale_action(self):
        for subgoal, facts, satisfied in (
                (['catch', 'PokemonMansion1F', True], {'map': 'SeafoamIslandsB3F'}, False),
                (['heal', 'party', True], {'map': 'Other'}, False),
                (['heal', 'party', True], {'map': 'SeafoamIslandsB3F'}, True)):
            with self.subTest(subgoal=subgoal, facts=facts, satisfied=satisfied):
                agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
                agent.index = Mock()
                agent.index.satisfied.return_value = satisfied
                agent.route_continuation = {'goal': ['heal', 'party', True],
                    'destination': 'CinnabarPokecenter', 'landing': ['SeafoamIslandsB3F', 23, 9]}
                with patch.object(DualStoryAgent, 'choose', return_value='a') as choose:
                    agent.choose('action', {'subgoal': subgoal, 'local_state': facts}, {'a': '{}'}, 'pick')
                self.assertNotIn('completed_route_prerequisite', choose.call_args.args[1])

    def test_travel_action_includes_current_trigger_route_not_only_map_hops(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        target = ('heal', 'party', True)
        rules = [Rule(name, name, 'talkNurse', [], [], [], target, [])
                 for name in ('CinnabarPokecenter', 'FuchsiaPokecenter')]
        routes = [{'map': rules[0].map, 'tile_route_found': True, 'steps': 135,
                   'requires_surf': True, 'unmet_native_field_prerequisites': []},
                  {'map': rules[1].map, 'tile_route_found': False, 'steps': None,
                   'requires_surf': False}]
        origin = ['SeafoamIslandsB3F', 23, 9]
        agent.active = {'target': target, 'rules': rules, 'context': {
            'trigger_navigation': routes, 'trigger_navigation_origin': origin}}
        agent.visited = set()
        agent.client = Mock()
        agent.client.cmd.return_value = []
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'Route20'}]}
        options = {str(i): json.dumps({'operation': f'travel_to:{rule.map}'})
                   for i, rule in enumerate(rules)}
        bindings = {str(i): (f'travel_to:{rule.map}', rule) for i, rule in enumerate(rules)}
        with patch.object(DualStoryAgent, 'action_candidates', return_value=(options, bindings)):
            candidates, actual_bindings = agent._action_candidates({'map': 'SeafoamIslandsB3F'})
        self.assertEqual(actual_bindings, bindings)
        for i, route in enumerate(routes):
            value = json.loads(candidates[str(i)])
            self.assertEqual(value['trigger_navigation'], route)
            self.assertEqual(value['trigger_navigation_origin'], origin)
            self.assertEqual(value['navigation']['map_hops'], 1)
            self.assertIn('conditional', value['trigger_navigation_scope'])
            self.assertIn('not a fresh route after movement', value['trigger_navigation_scope'])

    def test_one_depleted_coverage_move_does_not_abort_ready_hunts(self):
        mon = {'species': 'Charizard', 'level': 57, 'hp': 193, 'max_hp': 193,
               'status': 'None', 'moves': ['Slash', 'Cut', 'Flamethrower', 'Dig'],
               'pp': [14, 30, 10, 0]}
        facts = {'party': [mon]}
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'target': ('catch', 'PokemonTower3F', True)}
        agent.replan_after_defeat = False
        self.assertTrue(agent.needs_healing(facts))  # Coverage recovery remains recommended.
        self.assertFalse(agent.needs_capture_recovery(facts))
        self.assertFalse(agent.needs_skill_recovery(facts))
        self.assertFalse(agent.should_replan(facts))
        mon['pp'] = [4, 7, 3, 0]
        self.assertTrue(agent.needs_healing(facts))
        self.assertTrue(agent.needs_capture_recovery(facts))
        mon['pp'] = [5, 0, 0, 0]
        self.assertTrue(agent.needs_healing(facts))
        self.assertTrue(agent.needs_capture_recovery(facts))
        mon['pp'] = [14, 30, 10, 0]
        mon['hp'] = 100
        self.assertTrue(agent.needs_healing(facts))
        self.assertTrue(agent.needs_capture_recovery(facts))
        mon['hp'], mon['status'] = 193, 'Paralysis'
        self.assertTrue(agent.needs_healing(facts))
        self.assertTrue(agent.needs_capture_recovery(facts))

    def test_healing_context_separates_coverage_from_urgent_survival(self):
        from copy import deepcopy
        mon = {'species': 'Charizard', 'level': 62, 'hp': 219, 'max_hp': 219,
               'status': 'None', 'moves': ['Slash', 'Cut', 'Flamethrower', 'Dig'],
               'pp': [14, 30, 14, 0]}
        facts = {'party': [mon]}
        before = deepcopy(facts)
        context = AutonomousStoryAgent.healing_context(facts)
        self.assertFalse(context['urgently_needed'])
        self.assertTrue(context['coverage_recovery_recommended'])
        evidence = context['lead_recovery_evidence']
        self.assertEqual(evidence['remaining_attack_pp'], 58)
        self.assertEqual([row['move'] for row in evidence['usable_attacks']],
                         ['Slash', 'Cut', 'Flamethrower'])
        self.assertEqual([row['move'] for row in evidence['low_pp_coverage_attacks']], ['Dig'])
        self.assertFalse(evidence['health_or_status_warning'])
        self.assertIn('matchups', context['scope'])
        self.assertEqual(facts, before)

    def test_healing_context_keeps_actual_health_status_and_exhaustion_warnings(self):
        base = {'species': 'Charizard', 'level': 62, 'hp': 219, 'max_hp': 219,
                'status': 'None', 'moves': ['Slash', 'Cut', 'Flamethrower', 'Dig'],
                'pp': [20, 30, 15, 10]}
        for changes in ({'hp': 0}, {'hp': 100}, {'status': 'Sleep(1)'},
                        {'pp': [0, 0, 0, 0]}, {'pp': [1, 2, 1, 0]}, {'pp': [4, 7, 3, 0]}):
            with self.subTest(changes=changes):
                context = AutonomousStoryAgent.healing_context({'party': [{**base, **changes}]})
                self.assertTrue(context['urgently_needed'])
        context = AutonomousStoryAgent.healing_context({'party': [{**base,
            'species': 'Abra', 'moves': ['Teleport'], 'pp': [20]}]})
        self.assertTrue(context['urgently_needed'])
        self.assertEqual(context['lead_recovery_evidence']['usable_attacks'], [])
        self.assertFalse(AutonomousStoryAgent.healing_context({'party': []})['urgently_needed'])

    def test_healing_group_uses_survival_warning_without_removing_recovery(self):
        agent = self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True}])
        agent.find_catch_areas.return_value = {}
        healer = Rule('heal', 'Center', 'nurse', ['npc:1'], [], [], ('heal', 'party', True), [])
        agent.nearby_healers = Mock(return_value=[healer])
        mon = {'species': 'Charizard', 'level': 62, 'hp': 219, 'max_hp': 219,
               'status': 'None', 'moves': ['Slash', 'Cut', 'Flamethrower', 'Dig'],
               'pp': [14, 30, 14, 0]}
        facts = {'party': [mon], 'bag': {}, 'flags': {}, 'fully_recovered': False,
                 'map': 'Room', 'x': 10, 'y': 6}
        with patch.object(DualStoryAgent, 'strategy_groups', return_value={}):
            groups = agent.strategy_groups(facts)
        context = groups['prepare:heal']['context']
        self.assertFalse(context['urgently_needed'])
        self.assertTrue(context['coverage_recovery_recommended'])
        self.assertEqual(groups['prepare:heal']['rules'], [healer])

    def test_unfinished_mechanism_parent_is_visible_without_forcing_a_strategy(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = False
        agent.index = Mock()
        agent.index.satisfied.return_value = False
        agent.mechanism_goal = ('item', 'KEY', True)
        state = {}
        agent.augment_strategy_state(state, {'map': 'Room'})
        self.assertEqual(state['unfinished_mechanism_parent']['target'], agent.mechanism_goal)
        options = {'continue': '{}', 'heal': json.dumps({'context': {
            'coverage_recovery_recommended': True, 'urgently_needed': False}}), 'other': '{}'}
        with patch.object(DualStoryAgent, 'choose', return_value='other') as choose:
            self.assertEqual(agent.choose('strategy', state, options, 'pick'), 'other')
        self.assertEqual({key: json.loads(value) for key, value in choose.call_args.args[2].items()},
                         {key: json.loads(value) for key, value in options.items()})
        self.assertIn('coverage', choose.call_args.args[3])
        self.assertIn('unfinished_mechanism_parent', choose.call_args.args[3])
        agent.index.satisfied.return_value = True
        completed = {}
        agent.augment_strategy_state(completed, {'map': 'Room'})
        self.assertNotIn('unfinished_mechanism_parent', completed)

    def test_selected_goal_is_not_cancelled_by_the_same_known_fatigue(self):
        from openpokered.story_agent import DualStoryAgent
        mon = {'species': 'Charizard', 'level': 84, 'hp': 213, 'max_hp': 293,
               'status': 'None', 'moves': ['Slash', 'Cut', 'Flamethrower', 'Dig'],
               'pp': [18, 30, 3, 0]}
        facts = {'party': [mon], 'bag': {}, 'map': 'AgathasRoom'}
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'target': ('supply', 'UltraBall', 12)}
        agent.replan_after_defeat = False
        self.assertTrue(agent.should_replan(facts))
        with patch.object(DualStoryAgent, 'select_strategy'):
            agent.select_strategy(facts)
        self.assertFalse(agent.should_replan(facts))
        self.assertFalse(agent.should_replan({**facts, 'map': 'IndigoPlateauLobby'}))
        # A new HP change within the same band also warrants reconsideration.
        for change in ({'hp': 212}, {'pp': [17, 30, 3, 0]}, {'status': 'Poison'},
                       {'level': 85}, {'hp': 0}):
            with self.subTest(change=change):
                self.assertTrue(agent.should_replan({**facts, 'party': [{**mon, **change}]}))
        self.assertTrue(agent.should_replan({**facts, 'bag': {'HYPERPOTION': 1}}))
        agent.active = {'target': ('flag', 'NEXT_BATTLE', True)}
        self.assertTrue(agent.should_replan(facts))
        agent.active = {'target': ('supply', 'UltraBall', 12)}
        agent.replan_after_defeat = True
        self.assertTrue(agent.should_replan(facts))

    def test_accepted_fatigue_never_bypasses_critical_or_capture_resource_guards(self):
        from openpokered.story_agent import DualStoryAgent
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.replan_after_defeat = False
        mon = {'species': 'Charizard', 'level': 84, 'hp': 1, 'max_hp': 293,
               'status': 'None', 'moves': ['Slash'], 'pp': [20]}
        facts = {'party': [mon], 'bag': {}}
        for target in [('supply', 'UltraBall', 12), ('catch', 'VictoryRoad3F', True)]:
            agent.active = {'target': target}
            with patch.object(DualStoryAgent, 'select_strategy'):
                agent.select_strategy(facts)
            self.assertTrue(agent.should_replan(facts))
        # A fresh judgment replaces the baseline; mutating observations later
        # must not mutate the stored comparison.
        mon['hp'] = 200
        agent.active = {'target': ('supply', 'UltraBall', 12)}
        with patch.object(DualStoryAgent, 'select_strategy'):
            agent.select_strategy(facts)
        self.assertFalse(agent.should_replan(facts))
        mon['pp'][0] = 0
        self.assertTrue(agent.should_replan(facts))

    def test_late_navigation_cannot_reintroduce_pushes_without_strength_and_badge(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.field_requirements = {}
        agent.index = Mock()
        teeth = Rule('teeth', 'SafariZoneWest', 'pickup', [], [], [], ('item', 'GOLD_TEETH', True), [])
        erika = Rule('erika', 'CeladonGym', 'trainer', [], [], [], ('flag', 'EVENT_BEAT_ERIKA', True), [])
        agent.index.frontier.side_effect = lambda target, facts: [teeth] if target[0] == 'item' else [erika]
        push = Rule('boulder:FLAG', 'SeafoamIslandsB3F', 'engine_boulder', [], [], [], ('flag', 'FLAG', True), [])
        other = Rule('other', 'PalletTown', 'pickup', [], [], [], ('item', 'POTION', True), [])
        facts = {'map': 'SeafoamIslandsB4F', 'bag': {}, 'flags': {},
                 'party': [{'species': 'Charizard', 'moves': ['Slash']}]}
        for moves, flags in ((['Slash'], {}), (['Strength'], {})):
            facts['party'][0]['moves'], facts['flags'] = moves, flags
            groups = {'late': {'target': push.effect, 'rules': [push]},
                      'mixed': {'target': other.effect, 'rules': [other, push]}}
            agent.defer_unusable_boulders(groups, facts)
            self.assertNotIn('late', groups)
            self.assertEqual(groups['mixed']['rules'], [other])
            self.assertIn(teeth.effect, [group['target'] for group in groups.values()])
            self.assertIn(erika.effect, [group['target'] for group in groups.values()])
        facts['party'][0]['moves'] = ['Slash']
        facts['bag']['HM04'] = 1
        groups = {'late': {'target': push.effect, 'rules': [push]}}
        agent.defer_unusable_boulders(groups, facts)
        self.assertIn('learn:Strength', groups)
        facts['party'][0]['moves'], facts['flags'] = ['Strength'], {'EVENT_BEAT_ERIKA': True}
        groups = {'late': {'target': push.effect, 'rules': [push]}}
        agent.defer_unusable_boulders(groups, facts)
        self.assertEqual(groups['late']['rules'], [push])

    def test_native_current_and_badge_prerequisites_are_position_and_flag_specific(self):
        from openpokered.navigation_skills import surf_current_prerequisites, field_badge_prerequisites
        obstacle = {'map': 'SeafoamIslandsB4F', 'stance': [7, 11]}
        one, two = 'EVENT_SEAFOAM4_BOULDER1_DOWN_HOLE', 'EVENT_SEAFOAM4_BOULDER2_DOWN_HOLE'
        self.assertEqual(surf_current_prerequisites(obstacle, {}), [('flag', one, True), ('flag', two, True)])
        self.assertEqual(surf_current_prerequisites(obstacle, {one: True}), [('flag', two, True)])
        self.assertEqual(surf_current_prerequisites(obstacle, {one: True, two: True}), [])
        self.assertEqual(surf_current_prerequisites({**obstacle, 'stance': [8, 11]}, {}), [])
        self.assertEqual(surf_current_prerequisites({**obstacle, 'map': 'Route19'}, {}), [])
        self.assertEqual(field_badge_prerequisites('Strength', {}), [('flag', 'EVENT_BEAT_ERIKA', True)])
        self.assertEqual(field_badge_prerequisites('Strength', {'EVENT_BEAT_ERIKA': True}), [])
        self.assertEqual(field_badge_prerequisites('Surf', {}), [('flag', 'EVENT_BEAT_KOGA', True)])

    def test_native_current_refusal_returns_real_prerequisites_without_menu_retry(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.actions, agent.max_actions = 0, 10
        obstacle = {'map': 'SeafoamIslandsB4F', 'stance': [7, 11], 'direction': 'down',
                    'landing': ['SeafoamIslandsB4F', 7, 3]}
        agent.active = {'context': obstacle}
        agent.client, agent.game, agent.record, agent.travel = Mock(), Mock(), Mock(), Mock()
        agent.client.flags.return_value = {}
        agent.game.st.return_value = {'player_transport': 'Walking'}
        rule = Rule('water', obstacle['map'], 'water', [], [], [], (), [])
        result = agent.execute('surf:1', rule)
        self.assertEqual(result['result'], 'blocked')
        self.assertEqual(len(result['prerequisites']), 2)
        agent.travel.assert_not_called()
        agent.game.face.assert_not_called()

    def test_water_geometry_does_not_certify_current_blocked_embarkation(self):
        from openpokered.navigation_skills import surf_path_prerequisites
        path = [('SeafoamIslandsB4F', 7, 11), (('SeafoamIslandsB4F', 7, 12), 'down')]
        flags = {'EVENT_BEAT_KOGA': True}
        self.assertEqual(len(surf_path_prerequisites(path, flags)), 2)
        flags.update(EVENT_SEAFOAM4_BOULDER1_DOWN_HOLE=True, EVENT_SEAFOAM4_BOULDER2_DOWN_HOLE=True)
        self.assertEqual(surf_path_prerequisites(path, flags), [])
        # Already on water is not a fresh embarkation and should not invent
        # a badge/current requirement for an existing legal transport mode.
        water = [('SeafoamIslandsB4F', 7, 12), (('SeafoamIslandsB4F', 7, 13), 'down')]
        self.assertEqual(surf_path_prerequisites(water, {}), [])

    def test_trainer_switch_is_grounded_in_live_battle_not_parent_heal_goal(self):
        from openpokered.story_agent import DualStoryAgent
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'target': ('heal', 'party', True)}
        party = [
            {'species': 'Marowak', 'hp': 81, 'moves': ['BoneClub'], 'pp': [20]},
            {'species': 'Charizard', 'hp': 181, 'moves': ['Slash'], 'pp': [18]}]
        state = {'battle': {'is_wild': False, 'player': party[0],
                           'player_party': party, 'enemy': {'species': 'Spearow'}}}
        candidates = {'switch:1': '{}'}
        with patch.object(DualStoryAgent, 'choose', return_value='switch:1') as choose:
            self.assertEqual(agent.choose('action', state, candidates, 'Choose a turn.'), 'switch:1')
            self.assertFalse(choose.call_args.kwargs['allow_abstain'])
            self.assertIn('forced trainer battle', choose.call_args.args[1]['immediate_goal'])
            self.assertIn('nurse', choose.call_args.args[3])
            self.assertNotIn('immediate_goal', state)
            for change in ({'hp': 0}, {'pp': [0]}, {'moves': ['Dig'], 'pp': [10]}):
                party[1] = {**party[1], **change}
                agent.choose('action', state, candidates, 'Choose a turn.')
                self.assertTrue(choose.call_args.kwargs['allow_abstain'])
                party[1] = {'species': 'Charizard', 'hp': 181, 'moves': ['Slash'], 'pp': [18]}
            for is_wild in (True, None):
                state['battle']['is_wild'] = is_wild
                agent.choose('action', state, candidates, 'Choose a turn.')
                self.assertTrue(choose.call_args.kwargs['allow_abstain'])
            state['battle']['is_wild'] = False
            agent.choose('action', state, {'fight': 'Attack'}, 'Choose a turn.')
            self.assertTrue(choose.call_args.kwargs['allow_abstain'])

    def test_full_party_retrieval_deposits_in_another_box_before_withdrawing(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'target': ('pokemon', 'Cubone', None)}
        agent.client, agent.tap, agent.check_budget = Mock(), Mock(), Mock()
        party = [{'species': species} for species in
                 ['Charizard', 'Gloom', 'Nidoqueen', 'Fearow', 'Pidgeot', 'Raticate']]
        def state(phase, box, team=party, **pc):
            return {'screen': 'pc', 'party': team, 'current_box_index': box,
                    'box_counts': [20, 20, 1] + [0] * 9,
                    'pc_state': {'phase': phase, **pc}}
        agent.client.state.side_effect = [
            state('BillsMenu', 1, bills_cursor=3),
            state('BoxList', 1, box_cursor=2),
            state('BillsMenu', 2, bills_cursor=1),
            state('MonList', 2, mon_mode='Deposit', mon_cursor=5),
            state('MonAction', 2, mon_action_cursor=0),
            state('MonList', 2, party[:5], mon_mode='Deposit', mon_cursor=4),
            state('BillsMenu', 2, party[:5], bills_cursor=3),
            state('BoxList', 2, party[:5], box_cursor=1),
            state('BillsMenu', 1, party[:5], bills_cursor=0),
            state('MonList', 1, party[:5], mon_mode='Withdraw', mon_cursor=17),
            state('MonAction', 1, party[:5], mon_action_cursor=0),
            state('MonList', 1, party[:5] + [{'species': 'Cubone'}],
                  mon_mode='Withdraw', mon_cursor=17),
            {'screen': 'overworld', 'party': party[:5] + [{'species': 'Cubone'}]},
        ]
        result = agent.retrieve_from_pc(1, 17, 5, 0)
        self.assertEqual(result['result'], 'withdrew_pokemon')
        self.assertEqual([call.args[0] for call in agent.tap.call_args_list],
                         ['a'] * 5 + ['b'] + ['a'] * 5 + ['b'])

    def test_storage_retrieval_reports_all_boxes_full_without_looping(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'target': ('pokemon', 'Cubone', None)}
        agent.client, agent.tap, agent.check_budget = Mock(), Mock(), Mock()
        agent.client.state.return_value = {'party': [{'species': 'Rattata'}] * 6,
            'box_counts': [20] * 12, 'current_box_index': 1,
            'pc_state': {'phase': 'BillsMenu', 'bills_cursor': 0}}
        with self.assertRaisesRegex(StoryStopped, 'no_deposit_capacity'):
            agent.retrieve_from_pc(1, 17, 5, 0)
        agent.tap.assert_not_called()

    def test_invalid_source_requires_new_native_evolution_without_changing_dex(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collection_audit_pending = {'Marowak': {'reason': 'uncatchable_restless_soul'}}
        agent._complete_collection_graph = {'Marowak': [{
            'method': 'evolution', 'trigger': 'level', 'from_species': 'Cubone', 'level': 28}]}
        agent.record = Mock()
        facts = {'dex': {'owned_species': ['Cubone', 'Marowak'], 'owned': 2}}
        self.assertEqual(agent.validated_owned(facts), {'Cubone'})
        agent.observe_audit_evolution([{'species': 'Cubone', 'level': 27}])
        agent.observe_audit_evolution([{'species': 'Marowak', 'level': 30},
                                      {'species': 'Cubone', 'level': 27}])
        self.assertIn('Marowak', agent.collection_audit_pending)
        agent.active = {'context': {'acquisition_method': 'evolution',
                                   'from_species': 'Cubone', 'species': 'Marowak'}}
        agent.observe_audit_evolution([{'species': 'Marowak', 'level': 30},
                                      {'species': 'Marowak', 'level': 28}])
        self.assertFalse(agent.collection_audit_pending)
        self.assertEqual(facts['dex']['owned'], 2)
        agent.record.assert_called_once()

    def test_registration_target_waits_for_source_audit_without_changing_native_gates(self):
        from openpokered.story_rules import StoryIndex
        index = StoryIndex.__new__(StoryIndex)
        facts = {'dex': {'owned': 50, 'owned_species': ['Marowak']},
                 'collection_audit_pending': ['Marowak']}
        self.assertFalse(index.satisfied(('register', 'Marowak', True), facts))
        self.assertTrue(index.satisfied(('dex', 'owned', 50), facts))
        facts['collection_audit_pending'] = []
        self.assertTrue(index.satisfied(('register', 'Marowak', True), facts))

    def test_source_audit_accepts_evolution_after_native_battle_party_reordering(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collection_audit_pending = {'Marowak': {'reason': 'uncatchable_restless_soul'}}
        agent._complete_collection_graph = {'Marowak': [{
            'method': 'evolution', 'trigger': 'level', 'from_species': 'Cubone', 'level': 28}]}
        agent.active = {'context': {'acquisition_method': 'evolution',
                                   'from_species': 'Cubone', 'species': 'Marowak'}}
        agent.record = Mock()
        agent.observe_audit_evolution([{'species': 'Cubone', 'level': 27},
            {'species': 'Charizard', 'level': 58}, {'species': 'Marowak', 'level': 30}])
        agent.observe_audit_evolution([{'species': 'Charizard', 'level': 59},
            {'species': 'Marowak', 'level': 28}, {'species': 'Marowak', 'level': 30}], 1000)
        self.assertFalse(agent.collection_audit_pending)
        self.assertEqual(agent.record.call_args.kwargs['after']['level'], 28)
        self.assertEqual(agent.record.call_args.kwargs['frame'], 1000)

    def test_checkpoint_audit_recovers_legacy_spirit_capture(self):
        from openpokered.run_autonomous import checkpoint_collection_audit
        import tempfile
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            (folder / 'summary.json').write_text('{}')
            (folder / 'trace.jsonl').write_text(json.dumps({'kind': 'dex_progress',
                'map': 'PokemonTower6F', 'owned_species': ['Cubone'], 'acquired': ['Cubone']})
                + '\n' + json.dumps({'kind': 'dex_progress',
                'map': 'PokemonTower6F', 'owned_species': ['Cubone', 'Marowak'],
                'acquired': ['Marowak'], 'elapsed_s': 10}) + '\n')
            pending = checkpoint_collection_audit(folder)
            self.assertEqual(pending['Marowak']['reason'], 'uncatchable_restless_soul')
            (folder / 'summary.json').write_text(json.dumps({
                'collection_audit_schema': 1, 'collection_audit_pending': {}}))
            self.assertEqual(checkpoint_collection_audit(folder), {})

    def test_tower_checkpoint_initial_snapshot_is_not_an_illegal_capture(self):
        from openpokered.run_autonomous import checkpoint_collection_audit
        import tempfile
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            (folder / 'summary.json').write_text('{}')
            (folder / 'trace.jsonl').write_text(json.dumps({'kind': 'dex_progress',
                'map': 'PokemonTower6F', 'owned_species': ['Marowak'],
                'acquired': ['Marowak'], 'elapsed_s': 1}) + '\n')
            self.assertEqual(checkpoint_collection_audit(folder), {})

    def test_capture_retreat_retries_require_actual_preparation_improvement(self):
        from openpokered.autonomous_story import capture_preparation
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        agent.capture_retreats, agent.battle_defeats = {}, []
        agent.record = Mock()
        mon = {'species': 'Gloom', 'level': 21, 'hp': 0, 'status': 'None',
               'moves': ['SleepPowder'], 'pp': [15]}
        before = {'script_awaiting_battle': True, 'map_name': 'PowerPlant', 'party': [mon],
                  'battle_inventory': [{'item': 'GreatBall', 'qty': 2}],
                  'battle_live': {'is_wild': True, 'enemy': {'species': 'Zapdos'},
                                  'player_party': [mon]}}
        after = {'battle_phase': 'BattleOver { won: false, escaped: true }',
                 'pokedex': {'owned_species': []}, 'party': [mon]}
        agent.observe_battle_result(before, after)
        self.assertIn('PowerPlant:Zapdos', agent.capture_retreats)
        facts = {'party': [dict(mon)], 'bag': {'GREATBALL': 2}, 'map': 'Route10'}
        self.assertTrue(agent.static_capture_deferred('ZAPDOS', 'PowerPlant', facts))
        self.assertFalse(agent.static_capture_deferred('Zapdos', 'OtherMap', facts))
        facts['bag']['GREATBALL'] = 1
        self.assertTrue(agent.static_capture_deferred('Zapdos', 'PowerPlant', facts))
        facts['party'][0]['hp'] = 50
        self.assertFalse(agent.static_capture_deferred('Zapdos', 'PowerPlant', facts))
        facts['party'][0]['hp'] = 0
        facts['bag']['GREATBALL'] = 12
        self.assertFalse(agent.static_capture_deferred('Zapdos', 'PowerPlant', facts))
        restored = json.loads(json.dumps(agent.capture_retreats))
        self.assertEqual(restored, agent.capture_retreats)
        self.assertEqual(capture_preparation([mon], {'GreatBall': 2}),
                         restored['PowerPlant:Zapdos']['preparation'])

    def test_retreat_outcome_stays_visible_when_more_balls_reopen_retry(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        agent.capture_retreats, agent.battle_defeats = {}, []
        agent.record, agent.dex_progress = Mock(), Mock(return_value={})
        mon = {'species': 'Gloom', 'level': 24, 'hp': 71, 'status': 'None',
               'moves': ['SleepPowder'], 'pp': [15]}
        enemy = {'species': 'Zapdos', 'capture_species': 'Zapdos', 'hp': 150,
                 'max_hp': 150, 'status': 'None'}
        before = {'script_awaiting_battle': True, 'map_name': 'PowerPlant', 'party': [mon],
                  'battle_inventory': [{'item': 'UltraBall', 'qty': 8}],
                  'battle_live': {'is_wild': True, 'enemy': enemy, 'player_party': [mon]}}
        fainted = {**mon, 'hp': 0}
        after = {'battle_phase': 'BattleOver { won: false, escaped: true }',
                 'pokedex': {'owned_species': []}, 'party': [fainted],
                 'battle_live': {'enemy': enemy, 'player_party': [fainted]},
                 'battle_inventory': [{'item': 'UltraBall', 'qty': 8}]}
        agent.observe_battle_result(before, after)
        facts = {'party': [mon], 'bag': {'ULTRABALL': 10}}
        state = {}
        agent.augment_strategy_state(state, facts)
        self.assertEqual(state['capture_retreats_requiring_preparation'], [])
        evidence = state['capture_retry_evidence'][0]
        self.assertEqual(evidence['retreat_observation']['party'][0]['hp'], 0)
        self.assertEqual(evidence['retreat_observation']['enemy']['hp'], 150)
        self.assertEqual(evidence['retreat_observation']['enemy']['status'], 'None')
        self.assertEqual(evidence['preparation_changes_since_attempt'], ['more_ball_stock:ULTRABALL'])
        self.assertEqual(evidence['preparation']['party'][0]['hp'], 71)
        self.assertEqual(evidence['recorded_history']['recorded_retreats'], 1)
        self.assertEqual(evidence['recorded_history']['inventory_observed_retreats'], 1)
        self.assertEqual(evidence['recorded_history']['balls_spent'], {})
        self.assertNotIn('preparation_changes_since_attempt', agent.capture_retreats['PowerPlant:Zapdos'])
        # Legacy checkpoints have no post-retreat observations. Do not invent
        # the result, and do not lose the rest of the preparation evidence.
        del agent.capture_retreats['PowerPlant:Zapdos']['retreat_observation']
        state = {}
        agent.augment_strategy_state(state, facts)
        self.assertNotIn('retreat_observation', state['capture_retry_evidence'][0])

    def test_capture_history_distinguishes_unobserved_from_zero_ball_cost(self):
        from openpokered.autonomous_story import accumulate_capture_retreat
        totals = {}
        event = {'map': 'PowerPlant', 'species': 'Zapdos',
                 'preparation': {'balls': {'ULTRA_BALL': 8}}}
        accumulate_capture_retreat(totals, event)
        event['retreat_observation'] = {'inventory': [{'item': 'UltraBall', 'qty': 3}]}
        accumulate_capture_retreat(totals, event)
        event['preparation']['balls'] = {'ULTRABALL': 3}
        accumulate_capture_retreat(totals, event)
        event['start_inventory_observed'] = False
        accumulate_capture_retreat(totals, event)
        self.assertEqual(totals['PowerPlant:Zapdos'], {'recorded_retreats': 4,
            'inventory_observed_retreats': 2, 'balls_spent': {'ULTRABALL': 5}})
        self.assertEqual(json.loads(json.dumps(totals)), totals)

    def test_capture_history_resume_counts_only_lineage_and_does_not_double_count(self):
        import tempfile
        from openpokered.run_autonomous import checkpoint_capture_retreat_totals
        with tempfile.TemporaryDirectory() as temporary:
            folders = [Path(temporary) / name for name in ('parent', 'child', 'discarded')]
            parent, child, discarded = folders
            event = {'kind': 'capture_retreat', 'map': 'PowerPlant', 'species': 'Zapdos',
                     'preparation': {'balls': {'GREATBALL': 4}},
                     'retreat_observation': {'inventory': []}}
            for folder in folders:
                folder.mkdir()
                folder.joinpath('summary.json').write_text(json.dumps(
                    {'resumed_from': str(parent)} if folder != parent else {}))
                folder.joinpath('trace.jsonl').write_text(json.dumps(event) + '\n')
            totals = checkpoint_capture_retreat_totals(child)
            self.assertEqual(totals['PowerPlant:Zapdos']['recorded_retreats'], 2)
            self.assertEqual(totals['PowerPlant:Zapdos']['balls_spent'], {'GREATBALL': 8})
            child.joinpath('summary.json').write_text(json.dumps({
                'resumed_from': str(parent), 'capture_retreat_totals_schema': 1,
                'capture_retreat_totals': totals}))
            self.assertEqual(checkpoint_capture_retreat_totals(child), totals)
            # The schema is authoritative, including an empty history.
            child.joinpath('summary.json').write_text(json.dumps({
                'resumed_from': str(parent), 'capture_retreat_totals_schema': 1,
                'capture_retreat_totals': {}}))
            self.assertEqual(checkpoint_capture_retreat_totals(child), {})
            parent.joinpath('summary.json').write_text(json.dumps({'resumed_from': str(parent)}))
            with self.assertRaisesRegex(ValueError, 'checkpoint cycle'):
                checkpoint_capture_retreat_totals(parent)

    def test_route_requirements_refresh_after_consumed_drink_unlocks_guard(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        goal = ('flag', 'EVENT_BEAT_SABRINA', True)
        agent.route_requirements = {'SaffronGym': {'goal': goal,
            'trigger_points': [(5, 3)], 'prerequisites': [('item', 'FRESH_WATER', True)]}}
        agent.index, agent.game = Mock(), Mock()
        agent.index.satisfied.return_value = False  # Gym still unfinished.
        agent.discover_route_prerequisites = Mock(return_value=[])
        facts = {'flags': {'EVENT_GAVE_SAFFRON_GUARDS_DRINK': True}, 'bag': {}}
        agent.refresh_route_requirements(facts)
        self.assertEqual(agent.route_requirements, {})
        agent.discover_route_prerequisites.assert_called_once_with(
            agent.game.st.return_value, 'SaffronGym', [(5, 3)])

    def test_checkpoint_recovers_capture_capacity_from_matching_attempt_only(self):
        import tempfile
        from openpokered.run_autonomous import checkpoint_capture_retreats
        event = {'kind': 'capture_retreat', 'elapsed_s': 2, 'map': 'VictoryRoad2F',
                 'species': 'Moltres', 'preparation': {'balls': {'POKEBALL': 12}, 'party': []}}
        saved = {key: value for key, value in event.items() if key not in ('kind', 'elapsed_s')}
        battle = {'kind': 'battle_started', 'state': {'map_name': 'VictoryRoad2F',
                  'party': [{'species': 'Charizard'}] * 6, 'box_counts': [0, 20], 'current_box_index': 1,
                  'battle_live': {'enemy': {'species': 'Moltres'}}}}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            parent, child = root / 'parent', root / 'child'
            parent.mkdir()
            child.mkdir()
            parent.joinpath('summary.json').write_text(json.dumps({'capture_retreats': {'VictoryRoad2F:Moltres': saved}}))
            child.joinpath('summary.json').write_text(json.dumps({'resumed_from': str(parent),
                'capture_retreats': {'VictoryRoad2F:Moltres': saved}}))
            trace = parent / 'trace.jsonl'
            trace.write_text('\n'.join(map(json.dumps, [battle, event])))
            original = trace.read_bytes()
            restored = checkpoint_capture_retreats(child)['VictoryRoad2F:Moltres']
            self.assertIs(restored['preparation']['storage_full'], True)
            self.assertEqual(restored['capacity_evidence_source']['elapsed_s'], 2)
            self.assertEqual(trace.read_bytes(), original)
            # A newer attempt missing its battle cannot borrow the old full box.
            child.joinpath('trace.jsonl').write_text(json.dumps(event) + '\n')
            self.assertNotIn('storage_full', checkpoint_capture_retreats(child)['VictoryRoad2F:Moltres']['preparation'])
            for events in ([{'kind': 'battle_started', 'state': {**battle['state'], 'map_name': 'Other'}}, event],
                           [{'kind': 'battle_started', 'state': {**battle['state'], 'battle_live': {}}}, event],
                           [{'kind': 'battle_started', 'state': {key: value for key, value in battle['state'].items()
                                                               if key != 'party'}}, event],
                           [battle, {'kind': 'battle_resolved'}, event],
                           [battle, {**event, 'preparation': {'balls': {'POKEBALL': 1}, 'party': []}}]):
                with self.subTest(events=events):
                    child.joinpath('trace.jsonl').write_text('\n'.join(map(json.dumps, events)))
                    self.assertNotIn('storage_full', checkpoint_capture_retreats(child)['VictoryRoad2F:Moltres']['preparation'])

    def test_script_unlocks_are_guard_evidence_not_automatic_rewards(self):
        from openpokered.story_rules import literal
        from types import SimpleNamespace
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        key_guard = {'Call': {'callee': 'hasItem', 'args': [literal('CARD_KEY')]}}
        other_guard = {'Call': {'callee': 'hasItem', 'args': [literal('OTHER_KEY')]}}
        door = Rule('door', 'Office', 'door', [], [(key_guard, True)], [],
                    ('block', 'Office,1,1', 14), [])
        gift = Rule('gift', 'Office', 'gift', [], [(key_guard, True)], [],
                    ('pokemon', 'LAPRAS', 15), [('battle', 'RIVAL', True)])
        blocked = Rule('blocked', 'Other', 'blocked', [],
                       [(key_guard, True), (other_guard, True)], [], ('item', 'PRIZE', True), [])
        agent.index = SimpleNamespace(rules=[door, door, gift, blocked])
        facts = {'bag': {}, 'flags': {}}
        groups = {'key': {'target': ('item', 'CARD_KEY', True)}}
        agent.annotate_script_unlocks(groups, facts)
        result = groups['key']['context']['script_unlocks']
        self.assertEqual(result['scripts'], 2)
        self.assertEqual(result['maps'], ['Office'])
        self.assertEqual(result['effect_counts'], {'block': 1, 'pokemon': 1})
        self.assertEqual(result['potential_gift_species'], ['LAPRAS'])
        self.assertIn('battles still require execution', result['scope'])
        self.assertEqual(facts, {'bag': {}, 'flags': {}})
        already_owned = {'key': {'target': ('item', 'CARD_KEY', True)}}
        agent.annotate_script_unlocks(already_owned, {'bag': {'CARDKEY': 1}, 'flags': {}})
        self.assertNotIn('context', already_owned['key'])

    def test_route_requirements_refresh_keeps_new_blocker_and_removes_finished_goal(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        goal = ('flag', 'EVENT_BEAT_SILPH_CO_GIOVANNI', True)
        agent.route_requirements = {'SilphCo11F': {'goal': goal,
            'trigger_points': [(5, 3)], 'prerequisites': [('item', 'FRESH_WATER', True)]}}
        agent.index, agent.game = Mock(), Mock()
        agent.index.satisfied.return_value = False
        agent.discover_route_prerequisites = Mock(return_value=[('item', 'CARD_KEY', True)])
        agent.refresh_route_requirements({})
        self.assertEqual(agent.route_requirements['SilphCo11F']['prerequisites'],
                         [('item', 'CARD_KEY', True)])
        agent.index.satisfied.return_value = True
        agent.refresh_route_requirements({})
        self.assertEqual(agent.route_requirements, {})
        self.assertEqual(agent.discover_route_prerequisites.call_count, 1)

    def test_late_source_backchains_exact_region_and_keeps_all_parent_goals(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.navigation_memory = {'Office': {}}
        agent.game, agent.index = Mock(), Mock()
        door = Rule('door', 'Lobby', 'unlock', ['coord:door'], [], [],
                    ('flag', 'DOOR_OPEN', True), [])
        agent.index.frontier.return_value = [door]
        agent.discover_route_prerequisites = Mock(return_value=[('block', 'Office,1,1', 3)])
        agent.destination_points = Mock(side_effect=lambda name, rule: [(rule.triggers[0], 5)])
        groups = {}
        for name, x in [('MASTER_BALL', 6), ('GIFT', 6), ('OTHER_REWARD', 8)]:
            target = ('item', name, True)
            groups[name] = {'target': target, 'objectives': ['Obtain ' + name],
                'rules': [Rule(name, 'Office', 'reward', [x], [], [], target, [])],
                'context': {'trigger_navigation': [{'map': 'Office', 'tile_route_found': False}],
                            'remaining_cost': {'coins': 25}, 'acquisition_method': 'gift'}}
        ready = ('item', 'READY_REWARD', True)
        groups['reachable'] = {'target': ready, 'rules': [
            Rule('blocked-source', 'Office', 'gift', [6], [], [], ready, []),
            Rule('open-source', 'Other', 'gift', [4], [], [], ready, [])],
            'context': {'trigger_navigation': [
                {'map': 'Office', 'tile_route_found': False},
                {'map': 'Other', 'tile_route_found': True}]}}
        previews = {('Office', ((x, 5),)): {'tile_route_found': False} for x in (6, 8)}
        facts = {'map': 'Road', 'x': 2, 'y': 3}
        agent.add_deferred_route_frontiers(groups, facts, previews)
        added = groups[json.dumps(door.effect)]
        self.assertEqual(added['rules'], [door])
        self.assertEqual(added['context']['prerequisite_for_goals'],
                         [('item', name, True) for name in ('MASTER_BALL', 'GIFT', 'OTHER_REWARD')])
        self.assertEqual(agent.discover_route_prerequisites.call_count, 2)
        self.assertEqual([call.args[2] for call in agent.discover_route_prerequisites.call_args_list],
                         [[(6, 5)], [(8, 5)]])
        self.assertEqual(len(added['context']['route_unlocks']), 3)
        downstream = added['context']['route_unlocks'][0]['downstream_context']
        self.assertEqual(downstream, {'remaining_cost': {'coins': 25}, 'acquisition_method': 'gift'})
        self.assertNotIn('trigger_navigation', downstream)
        groups['MASTER_BALL']['context']['remaining_cost']['coins'] = 50
        self.assertEqual(downstream['remaining_cost']['coins'], 25)
        agent.game.nav_to_map.assert_not_called()
        agent.game.st.assert_not_called()
        # Unknown failures, successful previews, and disproved causal paths
        # must never manufacture an unlock.
        for memory, preview, prerequisites in [({}, previews, [('block', 'Office,1,1', 3)]),
                ({'Office': {}}, {}, [('block', 'Office,1,1', 3)]),
                ({'Office': {}}, previews, [])]:
            agent.navigation_memory = memory
            agent.discover_route_prerequisites.return_value = prerequisites
            offered = {'reward': groups['MASTER_BALL']}
            agent.add_deferred_route_frontiers(offered, facts, preview)
            self.assertEqual(list(offered), ['reward'])

    def test_route_unlock_guidance_retains_all_choices_and_downstream_costs(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.choose_bounded_strategy = Mock(return_value='unlock')
        candidates = {'unlock': json.dumps({'context': {'route_unlocks': [{
            'goal': ['register', 'Moltres', True],
            'downstream_context': {'acquisition_method': 'static', 'remaining_cost': 25}}]}}),
            'gift': json.dumps({'context': {'acquisition_method': 'gift'}})}
        before = dict(candidates)
        self.assertEqual(agent.choose('strategy', {}, candidates, 'Compare options.'), 'unlock')
        _state, offered, instructions = agent.choose_bounded_strategy.call_args.args
        self.assertEqual(set(offered), set(candidates))
        self.assertIn('intermediate step, not a Pokédex registration', instructions)
        self.assertIn('remaining acquisition effort', instructions)
        self.assertEqual(candidates, before)

    def test_collection_source_unlock_is_added_before_final_navigation_pruning(self):
        agent = self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True}])
        agent.find_catch_areas.return_value = {}
        agent.navigation_memory = {'Office': {}}
        agent.observed_navigation_barriers = Mock(return_value={})
        agent.add_navigation_groups = Mock()
        agent.add_cut_route_frontiers = Mock()
        agent.add_mechanism_groups = Mock()
        target = ('item', 'MASTER_BALL', True)
        reward = Rule('reward', 'Office', 'reward', [], [], [], target, [])
        door = Rule('door', 'Lobby', 'unlock', [], [], [], ('flag', 'DOOR_OPEN', True), [])
        agent.index.frontier.return_value = [door]
        agent.discover_route_prerequisites = Mock(return_value=[('block', 'Office,1,1', 3)])
        agent.destination_points = Mock(return_value=[(6, 5)])
        def insert_late(groups, facts):
            groups['ball-source'] = {'target': target, 'rules': [reward], 'objectives': ['Ball supply']}
        agent.add_recovery_groups = insert_late
        def preview(groups, facts, previews=None, *, prune=True):
            if prune:
                self.assertIn(json.dumps(door.effect), groups)
                groups.pop('ball-source')
            else:
                groups['ball-source']['context'] = {'trigger_navigation': [
                    {'map': 'Office', 'tile_route_found': False}]}
            return {('Office', ((6, 5),)): {'tile_route_found': False}}
        agent.annotate_navigation = preview
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [35, 25]}
        facts = {'map': 'PewterCity', 'x': 12, 'y': 18, 'party': [mon],
                 'bag': {}, 'flags': {}, 'fully_recovered': True}
        with patch.object(DualStoryAgent, 'strategy_groups', return_value={}):
            groups = agent.strategy_groups(facts)
        self.assertNotIn('ball-source', groups)
        self.assertEqual(groups[json.dumps(door.effect)]['rules'], [door])
        self.assertEqual(groups[json.dumps(door.effect)]['context']['prerequisite_for_goals'], [target])

    def test_fainted_status_support_is_not_a_collection_resource(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        facts = {'bag': {}, 'party': [{'species': 'Gloom', 'hp': 0,
                'moves': ['SleepPowder'], 'pp': [15]}]}
        self.assertFalse(agent.collection_resources(facts)['can_apply_safe_capture_status'])
        facts['party'][0]['hp'] = 1
        self.assertTrue(agent.collection_resources(facts)['can_apply_safe_capture_status'])

    def test_static_retreat_requires_proof_for_every_post_battle_write(self):
        from openpokered.story_rules import static_retreat_contract
        result = {'Result': {'Call': {'callee': 'startWildBattle',
            'args': [{'StringLit': 'ZAPDOS'}, {'NumberLit': 50}]}}}
        guard = {'BinaryOp': {'op': 'Or', 'left': {'BinaryOp': {
            'op': 'Eq', 'left': result, 'right': {'StringLit': 'win'}}},
            'right': {'BinaryOp': {'op': 'Eq', 'left': result,
                                 'right': {'StringLit': 'caught'}}}}}
        battle = Rule('b', 'PowerPlant', 'PowerPlant:bird', [], [], [], ('battle', 'ZAPDOS', True), [])
        hidden = Rule('h', battle.map, battle.storyline, [], [(guard, True)], [],
                      ('visibility', 'BIRD', False), [battle.effect])
        self.assertTrue(static_retreat_contract(battle, [battle, hidden])['menu_run_preserves_source'])
        unconditional = Rule('u', battle.map, battle.storyline, [], [], [],
                             ('flag', 'SPENT', True), [battle.effect])
        self.assertFalse(static_retreat_contract(battle, [battle, hidden, unconditional])['menu_run_preserves_source'])
        self.assertFalse(static_retreat_contract(battle, [battle])['menu_run_preserves_source'])
        battle.preceding = [('flag', 'SOURCE_SPENT_BEFORE_FIGHT', True)]
        self.assertFalse(static_retreat_contract(battle, [hidden])['menu_run_preserves_source'])
        battle.preceding = []
        hidden.guards = [(guard, False)]
        self.assertFalse(static_retreat_contract(battle, [hidden])['menu_run_preserves_source'])

    def test_capture_threat_uses_native_move_shift_and_self_knockout(self):
        from openpokered.playthrough_judgments import capture_threat
        threat = capture_threat({'species': 'Electrode', 'level': 43})
        self.assertEqual([row['move'] for row in threat['inferred_natural_moves']],
                         ['Sonicboom', 'Selfdestruct', 'LightScreen', 'Swift'])
        self.assertEqual(threat['self_knockout_moves'], ['Selfdestruct'])
        self.assertNotIn('Explosion', threat['self_knockout_moves'])
        self.assertIn('Explosion', capture_threat({'species': 'Electrode', 'level': 50})['self_knockout_moves'])

    def test_capture_self_ko_reference_is_conditional_not_live_survival(self):
        from openpokered.playthrough_judgments import capture_threat
        enemy = {'species': 'Graveler', 'level': 43, 'status': 'None'}
        reference = capture_threat(enemy)['self_ko_selection_reference']
        self.assertEqual(reference['self_ko_slots'], 2)
        self.assertEqual(reference['natural_move_slots'], 4)
        self.assertEqual(reference['no_self_ko_selection'], [
            {'enemy_selections': 1, 'probability': .5},
            {'enemy_selections': 2, 'probability': .25}])
        for phrase in ('uniform', 'PP', 'not survival', 'status'):
            self.assertIn(phrase, reference['assumptions'])
        # Absence/changed status and a transformed combat form are not evidence
        # that the inferred natural slots are currently usable.
        for changed in ({'status': 'Sleep(2)'}, {'status': 'Paralysis'},
                        {'status': None}, {'capture_species': 'Ditto'},
                        {'species': 'Machoke'}):
            with self.subTest(changed=changed):
                self.assertNotIn('self_ko_selection_reference', capture_threat({**enemy, **changed}))
        self.assertNotIn('self_ko_selection_reference',
                         capture_threat({'species': 'Graveler', 'level': 43}))

    def test_consumed_static_source_requires_observed_monotone_entry_blocker(self):
        from openpokered.story_rules import spent_static_source
        guard = {'Call': {'callee': 'getFlag', 'args': [{'StringLit': 'SPENT'}]}}
        battle = Rule('b', 'PowerPlant', 'PowerPlant:bird', [], [(guard, False)], [],
                      ('battle', 'ZAPDOS', True), [])
        completed = Rule('c', battle.map, battle.storyline, [], [], [],
                         ('flag', 'SPENT', True), [battle.effect])
        rules = [battle, completed]
        self.assertEqual(spent_static_source(battle, rules, {'flags': {}}), [])
        self.assertEqual(spent_static_source(battle, rules, {'flags': {'SPENT': True}}), ['SPENT'])
        reset = Rule('r', battle.map, 'reset', [], [], [], ('flag', 'SPENT', False), [])
        self.assertEqual(spent_static_source(battle, rules + [reset], {'flags': {'SPENT': True}}), [])
        battle.guards = [({'Visible': ['BIRD', False]}, True)]
        self.assertEqual(spent_static_source(battle, rules,
                         {'flags': {'SPENT': True, '__OBJ_HIDDEN_BIRD': True}}), [])

    def test_static_source_loss_gate_keeps_owned_alternatives_and_unknown_paths(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        guard = {'Call': {'callee': 'getFlag', 'args': [{'StringLit': 'SPENT'}]}}
        battle = Rule('b', 'PowerPlant', 'PowerPlant:bird', [], [(guard, False)], [],
                      ('battle', 'ZAPDOS', True), [])
        completed = Rule('c', battle.map, battle.storyline, [], [], [],
                         ('flag', 'SPENT', True), [battle.effect])
        agent.index = Mock(rules=[battle, completed])
        agent.record = Mock()
        method = {'method': 'static', 'map': 'PowerPlant', 'storyline': 'bird'}
        graph = {'Zapdos': [method]}
        agent.complete_collection_graph = lambda: graph
        facts = {'flags': {'SPENT': True}, 'dex': {'owned_species': []}}
        with self.assertRaisesRegex(StoryStopped, 'finite_collection_source_lost:Zapdos'):
            agent.require_static_sources(facts)
        agent.record.assert_called_once()
        facts['dex']['owned_species'] = ['Zapdos']
        agent.require_static_sources(facts)
        facts['dex']['owned_species'] = []
        graph['Zapdos'].append({'method': 'grass', 'map': 'Elsewhere'})
        agent.require_static_sources(facts)
        graph['Zapdos'] = [method, {**method, 'map': 'Unknown'}]
        agent.require_static_sources(facts)
        graph['Zapdos'] = [method]
        facts['flags'] = {}
        agent.require_static_sources(facts)

    def test_capture_inventory_risk_keeps_reference_assumptions_and_finite_source_cost(self):
        from openpokered.collection_planner import acquisition_contract
        risk = capture_inventory_risk('Snorlax', [{'ball': 'PokeBall', 'quantity': 9}])
        full, asleep, prepared = risk['scenarios']
        self.assertGreater(full['inventory_failure_probability'], .7)
        self.assertLess(asleep['inventory_failure_probability'], full['inventory_failure_probability'])
        self.assertLess(prepared['inventory_failure_probability'], asleep['inventory_failure_probability'])
        self.assertIn('Not actual battle odds', risk['assumptions'])
        more = capture_inventory_risk('Snorlax', [{'ball': 'PokeBall', 'quantity': 50}])
        self.assertLess(more['scenarios'][0]['inventory_failure_probability'], full['inventory_failure_probability'])
        master = capture_inventory_risk('Snorlax', [{'ball': 'MasterBall', 'quantity': 1}])
        self.assertTrue(all(row['inventory_failure_probability'] == 0 for row in master['scenarios']))
        contract = acquisition_contract('Snorlax', {'method': 'static', 'map': 'Route16'})
        self.assertTrue(contract['direct_cost']['finite_encounter_opportunity'])

    def test_observed_ghost_hunts_reopen_after_identification_item(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.battle_requirements = {'SILPH_SCOPE': {'capture_blocked_maps': ['Tower3', 'Tower4']}}
        agent.client = Mock()
        self.assertTrue(agent.capture_area_blocked('Tower3', {'bag': {}}))
        self.assertFalse(agent.capture_area_blocked('Other', {'bag': {}}))
        self.assertFalse(agent.capture_area_blocked('Tower3', {'bag': {'SILPHSCOPE': 1}}))
        self.assertEqual(agent.rank_catch_areas(['Tower3'], {'bag': {}}, set(), {}), [])
        agent.client.route.assert_not_called()
        agent.replan_after_defeat = False
        agent.needs_skill_recovery = Mock(return_value=False)
        agent.active = {'target': ('catch', 'Tower3', True), 'rules': [
            Rule('hunt', 'Tower3', 'skill:catch', [], [], [], ('catch', 'Tower3', True), [])]}
        self.assertTrue(agent.should_replan({'bag': {}, 'party': []}))
        self.assertFalse(agent.should_replan({'bag': {'SILPHSCOPE': 1, 'POKEBALL': 1}, 'party': []}))

    def test_wild_ghost_escape_retains_observed_maps_and_story_requirement(self):
        import playthrough as pt
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        story_goal = ['flag', 'STORY_GATE', True]
        game.judgments.active = {'target': ['catch', 'Tower3', True]}
        game.judgments.battle_requirements = {'SILPH_SCOPE': {
            'capture_blocked_maps': ['Tower4'], 'blocked_goal': story_goal}}
        game.battles_driven = 0
        before = {'screen': 'battle', 'map_name': 'Tower3', 'battle_live': {'is_ghost': True},
                  'script_awaiting_battle': False, 'party': []}
        after = {'screen': 'overworld', 'map_name': 'Tower3', 'battle_phase': 'Over',
                 'party': [], 'frame_count': 100}
        game.st = Mock(side_effect=[before, after])
        with patch.object(pt.Game, 'battle_loop') as drive:
            game.battle_loop(prefer='catch')
        self.assertEqual(drive.call_args.kwargs['prefer'], 'run')
        requirement = game.judgments.battle_requirements['SILPH_SCOPE']
        self.assertEqual(requirement['capture_blocked_maps'], ['Tower3', 'Tower4'])
        self.assertEqual(requirement['blocked_goal'], story_goal)

    def test_shared_strategy_evidence_is_lossless_and_preserves_all_options(self):
        routes = [{'map': f'Center{i}', 'steps': i + 1, 'tile_route_found': i % 2 == 0,
                   'requires_surf': i % 3 == 0} for i in range(12)]
        evidence = {'routes': routes, 'assumption': 'known geometry; obstacles remain uncertain'}
        state = {'world': {'map': 'City'}, 'route_evidence': evidence}
        candidates = {f'subgoal:{i}': json.dumps({
            'establish': ['pokemon', f'Species{i}', None],
            'context': {'navigation': evidence, 'required_for': f'Evolution{i}',
                        'cost': i * 100}}) for i in range(40)}
        original_state = json.loads(json.dumps(state))
        factored, options = factor_strategy_evidence(state, candidates)
        library = factored['shared_strategy_evidence']

        def expand(value):
            if isinstance(value, dict):
                if set(value) == {'shared_strategy_evidence_ref'}:
                    return expand(library[value['shared_strategy_evidence_ref']])
                if set(value) == {'strategy_table'}:
                    table = value['strategy_table']
                    return [{key: expand(child) for key, child in zip(table['columns'], row)}
                            for row in table['rows']]
                return {key: expand(child) for key, child in value.items()}
            if isinstance(value, list):
                return [expand(child) for child in value]
            return value

        self.assertEqual(set(options), set(candidates))
        for key in candidates:
            self.assertEqual(expand(json.loads(options[key])), json.loads(candidates[key]))
        self.assertEqual(expand({key: value for key, value in factored.items()
                                 if key != 'shared_strategy_evidence'}), original_state)
        self.assertEqual(state, original_state)  # No mutation of executor data.
        original_size = len(json.dumps(state)) + len(json.dumps(candidates))
        compact_size = len(json.dumps(factored)) + len(json.dumps(options))
        self.assertLess(compact_size, original_size / 3)

    def test_shared_evidence_leaves_small_and_plain_candidates_alone(self):
        state = {'map': 'City'}
        candidates = {'a': 'Continue', 'b': json.dumps({'cost': 2})}
        self.assertEqual(factor_strategy_evidence(state, candidates), (state, candidates))

    def test_strategy_overflow_partition_keeps_state_and_considers_every_option(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.record = Mock()
        state = {'world': {'map': 'City'}, 'shared_strategy_evidence': {'e0': ['facts']}}
        options = {str(i): f'Candidate {i}' for i in range(8)}
        evaluated = set()

        def decide(_layer, actual_state, candidates, instruction, *, allow_abstain):
            self.assertIs(actual_state, state)
            if len(candidates) > 3:
                raise StoryStopped('strategy:service_unavailable') from TypeSafeError(
                    'HTTP 400 max_tokens_exceeded')
            evaluated.update(candidates)
            return max(candidates, key=int)

        with patch.object(DualStoryAgent, 'choose', side_effect=decide) as calls:
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), '7')
        self.assertEqual(evaluated, set(options))
        self.assertEqual(calls.call_args.kwargs['allow_abstain'], True)
        self.assertTrue(any(not call.kwargs['allow_abstain'] for call in calls.call_args_list))
        self.assertEqual(options, {str(i): f'Candidate {i}' for i in range(8)})
        self.assertTrue(any(call.args[0] == 'strategy_partition_finalists'
                            for call in agent.record.call_args_list))

    def test_strategy_partition_does_not_mask_other_errors_or_recurse_forever(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.record = Mock()
        for detail, count in [('HTTP 401 unauthorized', 8), ('max_tokens_exceeded', 2)]:
            with self.subTest(detail=detail, count=count):
                def fail(*args, **kwargs):
                    raise StoryStopped('strategy:service_unavailable') from TypeSafeError(detail)
                with patch.object(DualStoryAgent, 'choose', side_effect=fail) as calls:
                    with self.assertRaises(StoryStopped):
                        agent.choose_bounded_strategy({}, {str(i): 'Choice' for i in range(count)}, 'Pick')
                self.assertEqual(calls.call_count, 1)
        agent.record.assert_not_called()

    def test_action_choice_shares_evidence_without_dropping_candidates(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'context': {'goal': 'Make room for a quest item'}}
        evidence = {'description': 'Exact item effects and loss consequences. ' * 20}
        state = {'subgoal': ['bag_space', 'inventory', 20],
                 'local_state': {'item_reference': evidence}}
        candidates = {f'action:{i}': json.dumps({'operation': f'sell:{i}',
                      'reference': evidence}) for i in range(39)}
        original = json.loads(json.dumps([state, candidates]))
        with patch.object(DualStoryAgent, 'choose', return_value='action:38') as decide:
            self.assertEqual(agent.choose('action', state, candidates, 'Pick'), 'action:38')
        layer, actual, options, instruction = decide.call_args.args
        self.assertEqual(layer, 'action')
        self.assertEqual(set(options), set(candidates))
        self.assertIn('shared_strategy_evidence', actual)
        self.assertIn('shared_strategy_evidence_ref', instruction)
        library = actual['shared_strategy_evidence']
        def expand(value):
            if isinstance(value, dict):
                if set(value) == {'shared_strategy_evidence_ref'}:
                    return expand(library[value['shared_strategy_evidence_ref']])
                return {key: expand(child) for key, child in value.items()}
            return value
        for key, value in options.items():
            row = expand(json.loads(value))
            self.assertEqual(row['reference'], evidence)
            self.assertEqual(row['operation'], json.loads(candidates[key])['operation'])
        self.assertEqual([state, candidates], original)

    def test_action_overflow_compares_every_option_and_fails_closed_at_two(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.record = Mock()
        state = {'subgoal': ['bag_space', 'inventory', 20], 'local_state': {}}
        candidates = {str(i): f'Action {i}' for i in range(9)}
        evaluated = set()
        def decide(layer, actual, options, instruction, *, allow_abstain):
            self.assertEqual(layer, 'action')
            self.assertEqual(actual, state)
            if len(options) > 3:
                raise StoryStopped('action:service_unavailable') from TypeSafeError(
                    'HTTP 400 max_tokens_exceeded')
            evaluated.update(options)
            return max(options, key=int)
        with patch.object(DualStoryAgent, 'choose', side_effect=decide):
            self.assertEqual(agent.choose('action', state, candidates, 'Pick'), '8')
        self.assertEqual(evaluated, set(candidates))
        self.assertTrue(any(call.args[0] == 'action_partition_finalists'
                            for call in agent.record.call_args_list))
        for detail in ('HTTP 401 unauthorized', 'max_tokens_exceeded'):
            def fail(*args, **kwargs):
                raise StoryStopped('action:service_unavailable') from TypeSafeError(detail)
            with patch.object(DualStoryAgent, 'choose', side_effect=fail) as calls:
                with self.assertRaises(StoryStopped):
                    agent.choose('action', state, {'a': 'A', 'b': 'B'}, 'Pick')
                self.assertEqual(calls.call_count, 1)

    def test_shared_strategy_text_and_tables_have_no_unreferenced_library_entries(self):
        text = 'Detailed acquisition evidence and native prerequisites. ' * 10
        rows = [{'species': f'Species{i}', 'description': text, 'extra': 'value ' * 40}
                for i in range(4)]
        state = {'records': rows}
        candidates = {'a': json.dumps({'records': rows}), 'b': json.dumps({'description': text})}
        compact, options = factor_strategy_evidence(state, candidates)
        library = compact['shared_strategy_evidence']
        used = set()

        def expand(value):
            if isinstance(value, dict):
                if set(value) == {'shared_strategy_evidence_ref'}:
                    key = value['shared_strategy_evidence_ref']
                    used.add(key)
                    return expand(library[key])
                if set(value) == {'strategy_table'}:
                    table = value['strategy_table']
                    return [dict(zip(table['columns'], map(expand, row))) for row in table['rows']]
                return {key: expand(child) for key, child in value.items()}
            if isinstance(value, list):
                return list(map(expand, value))
            return value

        self.assertEqual(expand(compact['records']), rows)
        for key, value in options.items():
            self.assertEqual(expand(json.loads(value)), json.loads(candidates[key]))
        self.assertEqual(used, set(library))
        self.assertIn(text, library.values())

    def test_strategy_tables_keep_different_species_and_every_field(self):
        rows = [{'species': f'Species{i}', 'expected_attempts': i + 0.5,
                 'encounter_share_pct': i, 'levels': [i, i + 2],
                 'catch_rate': 190, 'band': 'medium'} for i in range(30)]
        state = {'encounters': rows}
        candidates = {'a': json.dumps({'targets': rows}), 'b': 'Heal'}
        factored, options = factor_strategy_evidence(state, candidates)
        library = factored['shared_strategy_evidence']

        def expand(value):
            if isinstance(value, dict):
                if set(value) == {'shared_strategy_evidence_ref'}:
                    return expand(library[value['shared_strategy_evidence_ref']])
                if set(value) == {'strategy_table'}:
                    table = value['strategy_table']
                    return [dict(zip(table['columns'], [expand(v) for v in row]))
                            for row in table['rows']]
                return {key: expand(child) for key, child in value.items()}
            if isinstance(value, list):
                return [expand(child) for child in value]
            return value

        self.assertEqual(expand(factored['encounters']), rows)
        self.assertEqual(expand(json.loads(options['a'])), {'targets': rows})
        self.assertEqual(options['b'], 'Heal')
        self.assertIn('strategy_table', json.dumps(library))
        self.assertLess(len(json.dumps(library)), len(json.dumps(rows)) * 0.55)

    def trainer_preview_agent(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        root = Path(__file__).resolve().parents[1] / 'crates/pokered-data'
        agent.trainers = {path.stem: json.loads(path.read_text())
                          for path in (root / 'trainers').glob('*.json')}
        agent.maps = {'SSAnne2F': json.loads((root / 'maps/SSAnne2F/map.json').read_text())}
        return agent

    def test_coordinate_rival_uses_script_class_not_initial_npc_roster(self):
        agent = self.trainer_preview_agent()
        rule = Rule('rival', 'SSAnne2F', 'rival', ['coord:battle'], [], [],
                    ('flag', 'WON', True), [('battle', 'OPP_RIVAL2', True)])
        expected = [{'species': 'Pidgeotto', 'level': 19}, {'species': 'Raticate', 'level': 16},
                    {'species': 'Kadabra', 'level': 18}, {'species': 'Wartortle', 'level': 20}]
        self.assertEqual(agent.opponent_parties([rule], {'dex': {'owned_species': ['Charmander']}}), expected)
        # The current lead is unrelated to the persistent original starter.
        self.assertEqual(agent.opponent_parties([rule], {'dex': {'owned_species': ['Charizard']},
            'party': [{'species': 'Pidgeotto'}]}), expected)
        self.assertEqual(agent.opponent_parties([rule]), [])

    def test_rival_triplet_base_and_starter_offsets_match_native_contract(self):
        agent = self.trainer_preview_agent()
        for starter, offset in [('Charmander', 0), ('Squirtle', 1), ('Bulbasaur', 2)]:
            for base in (3.0, 6.0, 9.0):
                rule = Rule('rival', 'Arena', 'rival', ['coord:battle'], [], [],
                            ('flag', 'WON', True), [('battle', ('OPP_RIVAL2', base), True)])
                with self.subTest(starter=starter, base=base):
                    expected = agent.trainers['Rival2']['parties'][int(base) + offset]['pokemon']
                    self.assertEqual(agent.opponent_parties([rule],
                        {'dex': {'owned_species': [starter]}}), expected)

    def test_scripted_numbered_opponent_does_not_collect_every_map_npc(self):
        agent = self.trainer_preview_agent()
        rule = Rule('rocket', 'SSAnne2F', 'rocket', ['npc:2'], [], [],
                    ('flag', 'WON', True), [('battle', 'OPP_ROCKET7', True)])
        self.assertEqual(agent.opponent_parties([rule, rule]),
                         agent.trainers['Rocket']['parties'][6]['pokemon'])
        unknown = Rule('unknown', 'SSAnne2F', 'unknown', ['coord:battle'], [], [],
                       ('flag', 'WON', True), [('battle', 'OPP_NOT_A_TRAINER', True)])
        self.assertEqual(agent.opponent_parties([unknown]), [])

    def test_stationary_actor_on_destination_warp_offers_ready_clearance_battle(self):
        from types import SimpleNamespace
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        target = ('item', 'HM01', True)
        hidden = ('visibility', 'RIVAL', False)
        clear = Rule('clear', 'GateRoom', 'GateRoom:coordBattle', ['coord:battle'],
                     [], [], hidden, [('battle', 'RIVAL', True)])
        agent.index = SimpleNamespace(rules=[clear], npc_toggles={('GateRoom', 2): ('RIVAL', False)},
            satisfied=lambda goal, facts: False,
            frontier=lambda goal, facts: [clear] if goal == hidden else [])
        agent.game = SimpleNamespace(stationary_npcs={'GateRoom': {2: [36, 4]}})
        agent.maps, agent.field_requirements, agent.navigation_memory = {}, {}, {}
        agent.navigation_blockage = None
        agent.navigation_history = {'failed': {'map': 'GateRoom', 'destination': 'End',
            'goal': target, 'blocking_npcs': []}}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'GateRoom'}, {'to_map': 'End'}]}
        groups = {'main': {'target': target, 'rules': [], 'objectives': []}}
        with patch.object(pt, 'MAPS', {'GateRoom': {'warps': [
                {'x': 36, 'y': 4, 'dest_map_name': 'End'}]}}):
            agent.add_navigation_groups(groups, {'map': 'City'})
        added = next(group for group in groups.values() if group['target'] == hidden)
        self.assertEqual(added['rules'], [clear])
        self.assertEqual(added['context']['observed_navigation_blockage']['blocking_npcs'], [2])

    def test_training_effort_converts_observed_bounds_to_battles_and_steps(self):
        mon = {'species': 'Pidgeotto', 'level': 18}
        table = {'encounterRate': 32, 'mons': [{'species': 'Pidgey', 'level': 10}] * 10}
        solo = evolution_training_effort(mon, 36, table)
        shared = evolution_training_effort(mon, 36, table, 2)
        self.assertGreater(shared['estimated_victories_max'], solo['estimated_victories_max'])
        self.assertGreater(shared['estimated_victories_min'], 100)
        self.assertEqual(shared['estimated_encounter_steps_max'], shared['estimated_victories_max'] * 8)
        self.assertIsNone(evolution_training_effort(mon, 36, None))
        complete = evolution_training_effort({'species': 'Pidgeotto', 'level': 36}, 36, table)
        self.assertEqual(complete['estimated_victories_max'], 0)
    def test_checkpoint_recovers_observed_hm_blockers_across_legacy_resumes(self):
        import tempfile
        with tempfile.TemporaryDirectory() as private:
            old, latest = Path(private) / 'old', Path(private) / 'latest'
            old.mkdir()
            latest.mkdir()
            old.joinpath('summary.json').write_text(json.dumps({'preparation_requirements': {}}))
            cut = {'move': 'Cut', 'map': 'VermilionCity', 'tree': [15, 10]}
            old.joinpath('trace.jsonl').write_text(json.dumps({
                'kind': 'operation', 'result': {'field_obstruction': cut}}) + '\n')
            latest.joinpath('summary.json').write_text(json.dumps({'resumed_from': str(old)}))
            surf = {'move': 'Surf', 'map': 'Route24', 'landing': ['CeruleanCity', 5, 13]}
            latest.joinpath('trace.jsonl').write_text(json.dumps({
                'kind': 'operation', 'result': {'field_obstruction': surf}}) + '\n')
            self.assertEqual(checkpoint_field_requirements(latest), {'Cut': cut, 'Surf': surf})
            # A new complete checkpoint is authoritative: do not resurrect
            # blockers explicitly cleared since the legacy observation.
            latest.joinpath('summary.json').write_text(json.dumps({
                'resumed_from': str(old), 'field_requirements_schema': 1,
                'preparation_requirements': {'Surf': surf}}))
            self.assertEqual(checkpoint_field_requirements(latest), {'Surf': surf})
    def test_strategy_compaction_preserves_goals_costs_and_blockers(self):
        routes = [{'map': 'Center', 'tile_route_found': True, 'steps': 11,
                   'requires_surf': False, 'scope': 'real trigger route'}]
        routes += [{'map': f'Blocked{i}', 'tile_route_found': False, 'steps': None,
                    'requires_surf': False, 'scope': 'real trigger route'} for i in range(20)]
        context = {'trigger_navigation': routes, 'required_for': ['Kadabra', 'MrMime'],
                   'training_cost': {'remaining_experience_max': 1200}}
        original = {'x': json.dumps({'establish': ['pokemon', 'Abra', None], 'context': context})}
        compact = compact_strategy_candidates(original)
        decoded = json.loads(compact['x'])
        self.assertEqual(decoded['establish'], ['pokemon', 'Abra', None])
        self.assertEqual(decoded['context']['required_for'], ['Kadabra', 'MrMime'])
        self.assertEqual(decoded['context']['trigger_navigation'][0]['steps'], 11)
        self.assertEqual(decoded['context']['unreachable_trigger_maps'],
                         [f'Blocked{i}' for i in range(20)])
        self.assertEqual(decoded['context']['navigation_scope'], 'real trigger route')
        self.assertLess(len(compact['x']), len(original['x']) / 2)
        self.assertEqual(len(context['trigger_navigation']), 21)
        self.assertEqual(compact_strategy_candidates({'plain': 'Continue'}), {'plain': 'Continue'})
    def test_source_capture_keeps_normal_hunts_and_requires_possession(self):
        from openpokered.story_rules import StoryIndex
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.catch_areas = {'Route2': {'method': 'grass'}}
        agent.catch_navigation = {'Route2': {'map': 'Route2'}}
        area = {'map': 'Route22', 'method': 'grass', 'species': ['Spearow'],
                'spots': [(3, 4)], 'navigation': {'map': 'Route22'}}
        def find(facts, requested_species=None):
            self.assertEqual(requested_species, ('Spearow',))
            agent.catch_areas = {'Route22': area}
            agent.catch_navigation = {'Route22': area['navigation']}
            return agent.catch_areas
        agent.find_catch_areas = find
        facts = {'party': [{'species': 'Fearow'}], 'bag': {'POKEBALL': 5},
                 'dex': {'owned_species': ['Spearow', 'Fearow']}}
        groups = {}
        agent.add_source_reacquisition(groups, facts, {'Spearow': {'Farfetchd'}})
        key = 'source:Spearow:Route22'
        self.assertEqual(set(agent.catch_areas), {'Route2', key})
        self.assertEqual(groups[key]['context']['required_capture_species'], 'Spearow')
        index = StoryIndex.__new__(StoryIndex)
        target = groups[key]['target']
        self.assertFalse(index.satisfied(target, facts))
        facts['stored_pokemon'] = [{'species': 'SPEAROW', 'box_index': 3}]
        self.assertTrue(index.satisfied(target, facts))
        facts['stored_pokemon'] = []
        facts['party'].append({'species': 'Spearow'})
        self.assertTrue(index.satisfied(target, facts))

    def test_registered_required_source_is_captured_not_skipped(self):
        state = self.battle_state()
        state['pokedex']['owned_species'].append('Caterpie')
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.active = {'context': {'required_capture_species': 'Caterpie'}}
        game.judgments.choose.return_value = 'ball:PokeBall'
        self.assertEqual(game.battle_recovery_plan(state), ('PokeBall', None))
        offered = json.loads(game.judgments.choose.call_args.args[2]['ball:PokeBall'])
        self.assertTrue(offered['already_owned'])
        self.assertTrue(offered['required_as_trade_or_evolution_source'])
        self.assertIn('Registration and possession', game.judgments.choose.call_args.args[3])
        state['battle_live'].update(is_safari=True, safari={
            'base_catch_rate': 45, 'catch_rate': 45, 'bait_factor': 0,
            'escape_factor': 0, 'balls': 9, 'enemy_speed': 60})
        game.judgments.choose.return_value = 'ball'
        self.assertEqual(game.safari_battle_action(state), 'ball')
        game.judgments.active = {'context': {}}
        self.assertEqual(game.safari_battle_action(state), 'run')

    def test_collection_balls_expose_registration_gain_without_banning_duplicates(self):
        state = self.battle_state()
        state['pokedex']['owned_species'].append('Caterpie')
        game = JevGame.__new__(JevGame)
        game.judgments = Mock(collects_dex=True)
        game.judgments.active = {'target': ['catch', 'Forest', True], 'context': {}}
        game.judgments.choose.return_value = 'ball:PokeBall'
        self.assertEqual(game.battle_recovery_plan(state), ('PokeBall', None))
        _layer, evidence, candidates, instruction = game.judgments.choose.call_args.args
        value = evidence['collection_capture_value']
        self.assertEqual(value['registration_increment_if_caught'], 0)
        self.assertFalse(value['requested_held_copy'])
        self.assertEqual(value['active_subgoal'], ['catch', 'Forest', True])
        self.assertEqual(value['balls_remaining'], 5)
        self.assertIn('zero new species', instruction)
        self.assertIn('ball:PokeBall', candidates)
        game.judgments.active['context']['required_capture_species'] = 'Caterpie'
        game.battle_recovery_plan(state)
        value = game.judgments.choose.call_args.args[1]['collection_capture_value']
        self.assertEqual(value['registration_increment_if_caught'], 0)
        self.assertTrue(value['requested_held_copy'])
        self.assertNotIn('zero new species', game.judgments.choose.call_args.args[3])
        state['pokedex']['owned_species'].remove('Caterpie')
        game.battle_recovery_plan(state)
        self.assertEqual(game.judgments.choose.call_args.args[1]['collection_capture_value'][
            'registration_increment_if_caught'], 1)

    def test_collection_capture_value_tracks_transform_identity_and_unknown_dex(self):
        from openpokered.playthrough_judgments import collection_capture_value
        state = self.battle_state()
        state['battle_live']['enemy'].update(species='Pidgey', capture_species='Ditto')
        self.assertEqual(collection_capture_value(state, {})['registration_increment_if_caught'], 1)
        state['pokedex']['owned_species'].append('Ditto')
        self.assertEqual(collection_capture_value(state, {})['registration_increment_if_caught'], 0)
        state.pop('pokedex')
        self.assertIsNone(collection_capture_value(state, {})['registration_increment_if_caught'])

    def test_consumed_trade_source_is_reacquired_unless_boxed(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        agent.index = Mock(rules=[], by_effect={})
        agent._complete_collection_graph = {
            'Spearow': [{'method': 'grass', 'map': 'Route22'}],
            'Fearow': [{'method': 'evolution', 'from_species': 'Spearow',
                        'trigger': 'level', 'level': 20}],
            'Farfetchd': [{'method': 'npc_trade', 'from_species': 'Spearow',
                          'map': 'VermilionTradeHouse', 'completion_flag': 'TRADED'}]}
        agent.add_source_reacquisition = Mock()
        agent.add_storage_retrieval = Mock()
        facts = {'party': [{'species': 'Fearow'}], 'stored_pokemon': [],
                 'bag': {}, 'flags': {}, 'map': 'Route22',
                 'dex': {'owned_species': ['Spearow', 'Fearow']}}
        agent.add_nonwild_collection_groups({}, facts)
        self.assertEqual(agent.add_source_reacquisition.call_args.args[2],
                         {'Spearow': {'Farfetchd'}})
        facts['stored_pokemon'] = [{'species': 'Spearow'}]
        facts['party'].append({'species': 'Charizard'})
        agent.add_nonwild_collection_groups({}, facts)
        self.assertEqual(agent.add_source_reacquisition.call_args.args[2], {})
        agent.add_storage_retrieval.assert_called_once()

    def test_full_capture_storage_does_not_offer_rejected_balls(self):
        state = self.battle_state()
        state['party'] *= 6
        state['box_counts'] = [0, 20]
        state['current_box_index'] = 1
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        self.assertIsNone(game.battle_recovery_plan(state))
        game.judgments.choose.assert_not_called()
        state['battle_live']['is_safari'] = True
        self.assertEqual(game.safari_battle_action(state), 'run')
        state['box_counts'][1] = 19
        state['battle_live']['is_safari'] = False
        game.judgments.choose.return_value = 'ball:PokeBall'
        self.assertEqual(game.battle_recovery_plan(state), ('PokeBall', None))

    def test_capture_resource_replanning_includes_static_and_storage_capacity(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.replan_after_defeat = False
        mon = {'species': 'Charizard', 'level': 77, 'hp': 270, 'max_hp': 270,
               'status': 'None', 'moves': ['Slash'], 'pp': [20]}
        facts = {'party': [mon] * 6, 'bag': {'POKEBALL': 12},
                 'box_counts': [0, 20], 'current_box_index': 1}
        for method, target in [('static', ('register', 'Moltres', True)),
                               ('grass', ('catch', 'Route1', True)),
                               ('safari', ('catch', 'SafariZoneCenter', True)),
                               ('fishing', ('held_species', 'Poliwag', True))]:
            with self.subTest(method=method):
                agent.active = {'target': target, 'rules': [],
                                'context': {'acquisition_method': method, 'species': 'Moltres'}}
                self.assertTrue(agent.should_replan(facts))
                facts['current_box_index'] = 0
                self.assertFalse(agent.should_replan(facts))
                facts['current_box_index'] = 1
        facts['party'] = [mon] * 5
        self.assertFalse(agent.capture_resources_missing(facts, 'static'))
        facts['bag'] = {}
        self.assertTrue(agent.capture_resources_missing(facts, 'static'))
        self.assertFalse(agent.capture_resources_missing(facts, 'safari'))
        agent.active = {'target': ('box_space', 'storage', True)}
        facts['party'] = [mon] * 6
        self.assertFalse(agent.capture_resources_missing(facts))

    def test_static_capture_capacity_keeps_pc_and_non_capture_evolution_choices(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        capture = Rule('bird', 'VictoryRoad2F', 'VictoryRoad2F:talkMoltres', [], [], [],
                       ('battle', 'MOLTRES', True), [])
        pc = Rule('pc', 'Center', 'Center:pcStorage', ['sign:1'], [], [],
                  ('pc', 'storage', True), [])
        agent.index = Mock(rules=[capture, pc], by_effect={('pc', 'storage', True): [pc]})
        agent._complete_collection_graph = {
            'Moltres': [{'method': 'static', 'map': 'VictoryRoad2F', 'storyline': 'talkMoltres'}],
            'Growlithe': [{'method': 'grass', 'map': 'Route8'}],
            'Arcanine': [{'method': 'evolution', 'from_species': 'Growlithe',
                          'trigger': 'item', 'item': 'FireStone'}]}
        facts = {'party': [{'species': 'Growlithe'}] * 6, 'stored_pokemon': [],
                 'bag': {'POKEBALL': 12, 'FIRESTONE': 1}, 'flags': {}, 'map': 'VictoryRoad2F',
                 'box_counts': [0, 20], 'current_box_index': 1,
                 'dex': {'owned_species': ['Growlithe']}}
        groups = {}
        agent.add_nonwild_collection_groups(groups, facts)
        self.assertNotIn('register:Moltres:static:VictoryRoad2F', groups)
        self.assertIn('storage:change_box', groups)
        self.assertIn('register:Arcanine:evolution:Growlithe', groups)
        self.assertEqual(groups['storage:change_box']['rules'], [pc])
        for party_size, box_count in [(5, 20), (6, 19)]:
            facts['party'] = [{'species': 'Growlithe'}] * party_size
            facts['box_counts'][1] = box_count
            groups = {}
            agent.add_nonwild_collection_groups(groups, facts)
            self.assertIn('register:Moltres:static:VictoryRoad2F', groups)

    def test_capture_retry_recognizes_observed_storage_recovery_only(self):
        from openpokered.autonomous_story import capture_preparation, capture_preparation_improvements
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        agent.capture_retreats, agent.battle_defeats = {}, []
        agent.record, agent.dex_progress = Mock(), Mock(return_value={})
        mon = {'species': 'Charizard', 'level': 77, 'hp': 270, 'status': 'None',
               'moves': ['Slash'], 'pp': [20]}
        before = {'script_awaiting_battle': True, 'map_name': 'VictoryRoad2F',
                  'party': [mon] * 6, 'box_counts': [20, 20, 13], 'current_box_index': 1,
                  'battle_inventory': [{'item': 'PokeBall', 'qty': 12}],
                  'battle_live': {'is_wild': True, 'enemy': {'species': 'Moltres'},
                                  'player_party': [mon] * 6, 'capture_blocked_reason': 'storage_full'}}
        after = {'battle_phase': 'BattleOver { won: false, escaped: true }',
                 'pokedex': {'owned_species': []}, 'party': [mon] * 6}
        agent.observe_battle_result(before, after)
        previous = agent.capture_retreats['VictoryRoad2F:Moltres']['preparation']
        self.assertIs(previous['storage_full'], True)
        facts = {'party': [mon] * 6, 'bag': {'POKEBALL': 12},
                 'box_counts': [20, 20, 13], 'current_box_index': 0}
        self.assertTrue(agent.static_capture_deferred('Moltres', 'VictoryRoad2F', facts))
        facts['current_box_index'] = 2
        self.assertFalse(agent.static_capture_deferred('Moltres', 'VictoryRoad2F', facts))
        state = {}
        agent.augment_strategy_state(state, facts)
        self.assertEqual(state['capture_retry_evidence'][0]['preparation_changes_since_attempt'],
                         ['capture_storage_available'])
        # Absence of old capacity observations is not evidence it was full.
        legacy = capture_preparation(facts['party'], facts['bag'])
        current = capture_preparation(facts['party'], facts['bag'], facts)
        self.assertEqual(capture_preparation_improvements(current, legacy), [])
        facts['current_box_index'] = 0
        facts['party'].pop()
        self.assertFalse(agent.static_capture_deferred('Moltres', 'VictoryRoad2F', facts))

    def test_status_only_evolution_trainee_can_use_ready_finisher(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.replan_after_defeat = False
        finisher = {'species': 'Charizard', 'level': 40, 'hp': 130, 'max_hp': 130,
                    'status': 'None', 'moves': ['Ember'], 'pp': [25]}
        for species, move in [('Abra', 'Teleport'), ('Metapod', 'Harden')]:
            trainee = {'species': species, 'level': 10, 'hp': 30, 'max_hp': 30,
                       'status': 'None', 'moves': [move], 'pp': [20]}
            facts = {'party': [trainee, finisher]}
            agent.active = {'target': ('register', 'evolved', True),
                            'context': {'acquisition_method': 'evolution', 'trigger': 'level',
                                        'from_species': species}}
            self.assertTrue(agent.needs_healing(facts))
            self.assertFalse(agent.needs_skill_recovery(facts))
            self.assertFalse(agent.should_replan(facts))
            finisher['pp'] = [0]
            self.assertTrue(agent.needs_skill_recovery(facts))
            finisher['pp'] = [25]
            trainee['hp'] = 0
            self.assertTrue(agent.needs_skill_recovery(facts))
            trainee['hp'] = 30
            agent.active['context'] = {'acquisition_method': 'grass'}
            self.assertTrue(agent.needs_skill_recovery(facts))

    def test_evolution_cost_uses_native_growth_and_observed_level_bounds(self):
        self.assertEqual(level_experience('Rattata', 1), 0)
        self.assertEqual(level_experience('Rattata', 20), 8000)
        self.assertEqual(level_experience('Oddish', 21), 6458)
        cost = evolution_training_cost({'species': 'Rattata', 'level': 15}, 20)
        self.assertEqual(cost['remaining_experience_min'], 3905)
        self.assertEqual(cost['remaining_experience_max'], 4625)
        self.assertEqual(cost['levels_remaining'], 5)
        self.assertEqual(evolution_training_cost({'species': 'Rattata', 'level': 20}, 20)
                         ['remaining_experience_max'], 0)

    def test_training_yield_weights_slots_and_switch_participants(self):
        table = {'encounterRate': 32, 'mons': [
            {'species': 'Rattata', 'level': 10}] * 9 + [{'species': 'Chansey', 'level': 30}]}
        solo = training_yield(table)
        shared = training_yield(table, 2)
        self.assertEqual(solo['expected_encounter_attempts'], 8)
        expected = (253 * (57 * 10 // 7) + 3 * (255 * 30 // 7)) / 256
        self.assertEqual(solo['expected_experience_per_victory'], round(expected, 2))
        self.assertLess(shared['expected_experience_per_victory'], solo['expected_experience_per_victory'])
        self.assertEqual(shared['participants'], 2)
        self.assertEqual(training_yield(None)['expected_experience_per_victory'], 0)

    def test_storage_retains_main_and_sole_required_field_move_carriers(self):
        party = [{'species': 'Charizard', 'level': 36, 'moves': ['Slash']},
                 {'species': 'Pidgey', 'level': 9, 'moves': ['Gust']},
                 {'species': 'Oddish', 'level': 13, 'moves': ['Cut']},
                 {'species': 'Lapras', 'level': 15, 'moves': ['Surf']},
                 {'species': 'Machop', 'level': 20, 'moves': ['Strength']},
                 {'species': 'Rattata', 'level': 3, 'moves': ['Tackle']}]
        self.assertEqual(storage_deposit_indices(party), [1, 5])

    def test_stored_main_is_retrieved_instead_of_training_low_level_replacement(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        rule = Mock()
        rule.missing.return_value = False
        agent.index = Mock(by_effect={('pc', 'storage', True): [rule]})
        facts = {'party': [{'species': 'Oddish', 'level': 13}],
                 'stored_pokemon': [{'species': 'Charizard', 'level': 36, 'box': 0, 'index': 0}]}
        groups = {}
        agent.add_stored_battler_retrieval(groups, facts)
        self.assertTrue(groups['retrieve:Charizard']['context']['restore_main_battler'])
        self.assertEqual(groups['retrieve:Charizard']['target'], ('pokemon', 'Charizard', None))

    def test_surf_finds_an_executable_shore_when_shortest_route_enters_water_across_map_border(self):
        import playthrough as pt
        from openpokered.navigation_skills import surf_requirement, water_tile
        state = {'map_name': 'CinnabarIsland', 'player_x': 19, 'player_y': 4,
                 'player_transport': 'Walking'}
        original = pt.cross_step
        crossing = surf_requirement(state, 'PalletTown', [(5, 6)], 'Route20')
        self.assertIsNotNone(crossing)
        self.assertIs(pt.cross_step, original)
        name, stance = crossing['map'], crossing['stance']
        self.assertTrue(pt.bfs_cross('CinnabarIsland', (19, 4), name, tuple(stance),
                                     last_map='Route20', allow_ledges=True))
        dx, dy = pt.DELTA[crossing['direction']]
        self.assertTrue(water_tile(name, stance[0]+dx, stance[1]+dy))

    def test_branched_fall_uses_only_positions_matching_its_landing(self):
        from openpokered.story_rules import literal
        from openpokered.autonomous_story import trigger_position_matches
        condition = {'BinaryOp': {'op': 'Eq', 'left': {'Call': {'callee': 'getPlayerX', 'args': []}},
                                  'right': literal(19)}}
        branch = Rule('fall', 'Room', 'Room:fall', [], [(condition, True)], [],
                      ('transport', ('LowerRoom', 18, 14), True), [])
        self.assertFalse(trigger_position_matches(branch, (16, 14)))
        self.assertFalse(trigger_position_matches(branch, (17, 14)))
        self.assertTrue(trigger_position_matches(branch, (19, 14)))

    def test_mechanism_parent_survives_a_frontier_change_after_switching(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        target = ('item', 'KEY', True)
        pickup = Rule('key', 'Room', 'Room:key', [], [], [], target, [])
        jump = Rule('jump', 'Room', 'Room:jump', [], [], [], ('transport', ('Room', 8, 8), True), [])
        agent.index = Mock()
        agent.index.satisfied.return_value = False
        agent.index.frontier.return_value = [pickup]
        agent.destination_points = Mock(return_value=[(9, 8)])
        agent.mechanism_plan = Mock(return_value={'steps': [jump], 'flags': ['SWITCH'], 'maps': {'Room'}})
        groups = {'key': {'target': target, 'rules': [pickup]}}
        agent.add_mechanism_groups(groups, {})
        self.assertEqual(agent.mechanism_goal, target)
        groups = {'switch': {'target': ('flag', 'SWITCH', False), 'rules': []}}
        agent.add_mechanism_groups(groups, {})
        self.assertEqual([g['target'] for g in groups.values()], [jump.effect])
        agent.mechanism_plan.return_value = {'steps': [], 'flags': ['SWITCH'], 'maps': {'Room'}}
        groups = {'switch': {'target': ('flag', 'SWITCH', False), 'rules': []}}
        agent.add_mechanism_groups(groups, {})
        self.assertEqual([g['target'] for g in groups.values()], [target])
        agent.index.satisfied.return_value = True
        agent.add_mechanism_groups({}, {})
        self.assertIsNone(agent.mechanism_goal)

    def test_switch_approach_respects_its_script_facing_guard(self):
        from openpokered.story_rules import literal
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        pose = {'BinaryOp': {'op': 'Eq', 'left': {'Call': {'callee': 'getPlayerFacing', 'args': []}},
                             'right': literal('up')}}
        rule = Rule('switch', 'Room', 'Room:switch', [], [(pose, True)], [], ('flag', 'ON', True), [])
        self.assertEqual(list(agent.interaction_approaches(rule, 10, 5)), [((10, 6), 'up')])

    def test_local_mechanism_prerequisite_precedes_exit_for_outer_goal(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        outer = Rule('battle', 'Gym', 'Gym:battle', [], [], [], ('flag', 'BADGE', True), [])
        pickup = Rule('key', 'Room', 'Room:key', [], [], [], ('item', 'KEY', True), [])
        switch = Rule('switch', 'Room', 'Room:switch', [], [], [], ('flag', 'SWITCH', True), [])
        agent.index = Mock()
        agent.index.satisfied.return_value = False
        agent.mechanism_goal = outer.effect
        agent.destination_points = Mock(return_value=[(2, 3)])
        agent.mechanism_plan = lambda name, points, facts: {
            'steps': [switch] if name == 'Room' else [], 'flags': ['SWITCH'], 'maps': {'Room'},
            'exit': None if name == 'Room' else ('Outside', 1, 1)}
        groups = {'outer': {'target': outer.effect, 'rules': [outer]},
                  'key': {'target': pickup.effect, 'rules': [pickup]}}
        agent.add_mechanism_groups(groups, {})
        self.assertEqual(agent.mechanism_goal, pickup.effect)
        self.assertIn(switch.effect, [g['target'] for g in groups.values()])
        self.assertFalse(any(g['rules'][0].storyline == 'skill:leave_mechanism' for g in groups.values()))

        # Once the local prerequisite is actually acquired, leaving becomes
        # useful again; the remembered item goal must not trap the agent.
        agent.index.satisfied.side_effect = lambda target, facts: target == pickup.effect
        groups = {'outer': {'target': outer.effect, 'rules': [outer]}}
        agent.add_mechanism_groups(groups, {})
        self.assertEqual(agent.mechanism_goal, outer.effect)
        self.assertTrue(any(g['rules'][0].storyline == 'skill:leave_mechanism' for g in groups.values()))

    def test_action_receives_selected_mechanism_context_and_live_reachability(self):
        from openpokered.story_agent import DualStoryAgent
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        context = {'towards': ('item', 'KEY', True),
                   'planned_effects': [('flag', 'SWITCH', False), ('flag', 'SWITCH', True)],
                   'trigger_navigation': [{'tile_route_found': True}]}
        agent.active = {'context': context}
        state = {'subgoal': ('flag', 'SWITCH', False), 'local_state': {}}
        with patch.object(DualStoryAgent, 'choose', return_value='step') as choose:
            agent.choose('action', state, {'step': '{}'}, 'Choose the next operation.')
            self.assertEqual(choose.call_args.args[1]['strategy_context'], context)
            self.assertFalse(choose.call_args.kwargs['allow_abstain'])
            self.assertNotIn('strategy_context', state)
            # An unverified route still allows rejection. The instruction
            # is not a blanket requirement to execute every proposed plan.
            context['trigger_navigation'][0]['tile_route_found'] = False
            agent.choose('action', state, {'step': '{}'}, 'Choose the next operation.')
            self.assertTrue(choose.call_args.kwargs['allow_abstain'])

    def test_healing_route_reports_only_live_entry_resets_of_won_battles(self):
        from types import SimpleNamespace
        from openpokered.story_rules import literal
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        guard = {'Call': {'callee': 'getFlag', 'args': [literal('STARTED')]}}
        win = Rule('win', 'Arena', 'Arena:trainer', [], [], [], ('flag', 'WON', True), [('battle', 'TRAINER', True)])
        reset = Rule('reset', 'Lobby', 'Lobby:@load', ['load'], [(guard, True)], [], ('flag', 'WON', False), [])
        lever = Rule('lever', 'Arena', 'Arena:lever', [], [], [], ('flag', 'LEVER', True), [])
        reset_lever = Rule('lever_reset', 'Lobby', 'Lobby:@load', ['load'], [], [], ('flag', 'LEVER', False), [])
        heal = Rule('heal', 'Lobby', 'Lobby:nurse', ['npc:1'], [], [], ('heal', 'party', True), [])
        agent.index = SimpleNamespace(rules=[win, reset, lever, reset_lever])
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'Hall'}, {'to_map': 'Lobby'}]}
        facts = {'map': 'Arena', 'flags': {'STARTED': True, 'WON': True, 'LEVER': True}}
        self.assertEqual(agent.healing_route_costs([heal], facts), {'Lobby': ['WON']})
        self.assertEqual(agent.healing_route_costs([heal], {**facts, 'flags': {'WON': True}}), {})
        self.assertTrue(facts['flags']['WON'])
        agent.client.route.return_value = {'found': True, 'legs': []}
        self.assertEqual(agent.healing_route_costs([heal], {**facts, 'map': 'Lobby'}), {})

    def test_all_candidate_routes_share_reset_costs_without_pruning_or_stale_cache(self):
        from copy import deepcopy
        from types import SimpleNamespace
        from openpokered.story_rules import literal
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        guard = {'Call': {'callee': 'getFlag', 'args': [literal('STARTED')]}}
        win = Rule('win', 'Arena', 'Arena:trainer', [], [], [],
                   ('flag', 'WON', True), [('battle', 'TRAINER', True)])
        reset = Rule('reset', 'Lobby', 'Lobby:@load', ['load'], [(guard, True)], [],
                     ('flag', 'WON', False), [])
        agent.index = SimpleNamespace(rules=[win, reset])
        agent.client = Mock()
        agent.client.route.side_effect = lambda origin, destination: {
            'found': destination != 'Unreachable',
            'legs': [{'to_map': 'Lobby'}, {'to_map': destination}]}
        groups = {}
        for key, kind, destination in [('train', 'level', 'Grass'), ('pc', 'pokemon', 'Town'),
                ('shop', 'supply', 'Town'), ('hunt', 'catch', 'Grass'), ('heal', 'heal', 'Lobby'),
                ('local', 'flag', 'Arena'), ('unknown', 'pokemon', 'Unreachable')]:
            target = (kind, key, True)
            groups[key] = {'target': target, 'rules': [Rule(key, destination, 'skill:' + key,
                [], [], [], target, [])], 'context': {'keep': key}}
        facts = {'map': 'Arena', 'flags': {'STARTED': True, 'WON': True}}
        original = deepcopy(facts)
        agent.annotate_route_reset_costs(groups, facts)
        self.assertEqual(set(groups), {'train', 'pc', 'shop', 'hunt', 'heal', 'local', 'unknown'})
        for key in ('train', 'pc', 'shop', 'hunt', 'heal'):
            context = groups[key]['context']
            self.assertEqual(context['route_resets_won_battles'], {groups[key]['rules'][0].map: ['WON']})
            self.assertIn('map-level', context['route_reset_scope'])
            self.assertIn('No flag is changed', context['route_reset_scope'])
            self.assertEqual(context['keep'], key)
        for key in ('local', 'unknown'):
            self.assertNotIn('route_resets_won_battles', groups[key]['context'])
        self.assertEqual(agent.client.route.call_count, 4)  # one per nonlocal destination
        self.assertEqual(facts, original)
        agent.annotate_route_reset_costs(groups, {**facts, 'flags': {'WON': True}})
        for group in groups.values():
            self.assertNotIn('route_resets_won_battles', group['context'])
            self.assertNotIn('route_reset_scope', group['context'])

    def test_completion_resets_are_not_replay_costs_but_en_route_resets_still_are(self):
        from types import SimpleNamespace
        from copy import deepcopy
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        win = Rule('win', 'Arena', 'Arena:trainer', [], [], [],
            ('flag', 'WON', True), [('battle', 'TRAINER', True)])
        reset = Rule('reset', 'Exit', 'Exit:@load', ['load'], [], [], ('flag', 'WON', False), [])
        transport = Rule('exit', 'Exit', 'Exit:@load', ['load'], [], [],
            ('transport', ('Town', 5, 6), True),
            [('flag', 'WON', False), ('ending', 'hall_of_fame_and_credits', True)])
        heal = Rule('heal', 'Lobby', 'Lobby:nurse', ['npc:1'], [], [], ('heal', 'party', True), [])
        retreat_reset = Rule('retreat', 'Lobby', 'Lobby:@load', ['load'], [], [], ('flag', 'WON', False), [])
        agent.index = SimpleNamespace(rules=[win, reset, transport, retreat_reset])
        agent.client = Mock()
        agent.client.route.side_effect = lambda origin, destination: {'found': True,
            'legs': [{'to_map': destination}]}
        groups = {key: {'target': rule.effect, 'rules': [rule], 'context': {}}
                  for key, rule in [('complete', transport), ('retreat', heal)]}
        facts = {'map': 'Arena', 'flags': {'WON': True}}
        original = deepcopy(facts)
        agent.annotate_route_reset_costs(groups, facts)
        completed = groups['complete']['context']
        self.assertNotIn('route_resets_won_battles', completed)
        self.assertEqual(completed['completion_resets_won_battles'], {'Exit': ['WON']})
        self.assertIn('not a replay cost', completed['completion_reset_scope'])
        self.assertEqual(groups['retreat']['context']['route_resets_won_battles'], {'Lobby': ['WON']})
        self.assertEqual(facts, original)
        # The same flag can also reset BEFORE reaching the ceremony. Do not
        # subtract flags globally and accidentally erase that genuine cost.
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'Lobby'}, {'to_map': 'Exit'}]}
        agent.client.route.side_effect = None
        agent.annotate_route_reset_costs(groups, facts)
        self.assertEqual(completed['completion_resets_won_battles'], {'Exit': ['WON']})
        self.assertEqual(groups['complete']['context']['route_resets_won_battles'], {'Exit': ['WON']})

    def test_coupled_doors_require_transport_before_toggling_back(self):
        from types import SimpleNamespace
        from openpokered.story_rules import literal
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        guard = {'Call': {'callee': 'getFlag', 'args': [literal('SWITCH')]}}
        on = Rule('on', 'A', 'A:lever', ['sign:1'], [(guard, False)], [], ('flag', 'SWITCH', True), [])
        off = Rule('off', 'B', 'B:lever', ['sign:1'], [(guard, True)], [], ('flag', 'SWITCH', False), [])
        jump = Rule('jump', 'A', 'A:jump', ['coord:(3,0)'], [], [], ('transport', ('B', 1, 0), True), [])
        rules = [on, off, jump]
        for name in ['A', 'B']:
            for enabled in [False, True]:
                rules.append(Rule(name + str(enabled), name, name + ':@load', ['load'],
                    [(guard, enabled)], [], ('block', name + ',0,0', int(enabled)), []))
        agent.index = SimpleNamespace(rules=rules, by_effect={r.effect: [r] for r in rules})
        agent.game = Mock(last_map='A')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        agent.destination_points = lambda name, rule: [(3 if rule.id == 'jump' else 2, 0)]
        maps = {name: {'blocks': [0], 'width': 1} for name in ['A', 'B']}
        original = maps['A']['blocks']
        def route(name, start, dest, end, **kwargs):
            enabled = maps[name]['blocks'][0]
            if name != dest or end[0] == 3 and ((name == 'A' and not enabled) or (name == 'B' and enabled)):
                return None
            return [(name, *start)] if tuple(start) == tuple(end) else [(name, *start), ((dest, *end), 'right')]
        facts = {'map': 'A', 'x': 1, 'y': 0, 'flags': {}}
        with patch.object(pt, 'MAPS', maps), patch.object(pt, 'bfs_cross', side_effect=route):
            result = agent.mechanism_plan('B', [(3, 0)], facts)
        self.assertEqual([r.id for r in result['steps']], ['on', 'jump', 'off'])
        self.assertIs(maps['A']['blocks'], original)
        self.assertEqual(maps['B']['blocks'], [0])
        with patch.object(pt, 'MAPS', maps), patch.object(pt, 'bfs_cross', side_effect=RuntimeError('cancelled')):
            with self.assertRaises(RuntimeError):
                agent.mechanism_plan('B', [(3, 0)], facts)
        self.assertIs(maps['A']['blocks'], original)
        maps['A']['warps'] = []
        maps['B']['warps'] = [{'x': 4, 'y': 0}]
        def exit_route(name, start, dest, end, **kwargs):
            if dest == 'C':
                return [(name, *start), (('C', *end), 'right')] if name == 'B' and not maps['B']['blocks'][0] else None
            return route(name, start, dest, end, **kwargs)
        with patch.object(pt, 'MAPS', maps), patch.object(pt, 'bfs_cross', side_effect=exit_route), \
                patch.object(pt, 'warp_edges_from', return_value=[('C', 4, 0)]), \
                patch.object(pt, 'walkable', side_effect=lambda name, x, y: (x, y) == (5, 0)), \
                patch.object(pt, 'warp_tiles', return_value=set()):
            result = agent.mechanism_plan('C', [(8, 0)], {**facts, 'map': 'B', 'flags': {'SWITCH': True}})
        self.assertEqual([r.id for r in result['steps']], ['off'])
        self.assertEqual(result['exit'], ('C', 5, 0))
        self.assertIs(maps['A']['blocks'], original)

    def test_same_map_boulder_in_another_region_requires_travel(self):
        from openpokered.story_rules import MAPS_DIR
        from openpokered.story_agent import DualStoryAgent
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'SeafoamIslandsB2F'
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        agent.index = Mock(maps_dir=MAPS_DIR)
        agent.index.coordinates.return_value = []
        flag = 'EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE'
        rule = Rule('boulder:' + flag, name, name + ':engine_boulder', [], [], [], ('flag', flag, True), [])
        agent.active = {'target': rule.effect, 'rules': [rule]}
        agent.client = Mock()
        agent.client.cmd.return_value = [{'npc_index': 1, 'text_id': 2, 'x': 23, 'y': 6, 'visible': True}]
        agent.client.route.return_value = {'legs': []}
        agent.visited = {name}
        facts = {'map': name, 'x': 4, 'y': 3, 'party': [{'moves': ['Strength']}], 'flags': {}}
        with patch.object(DualStoryAgent, 'action_candidates', return_value=({}, {})):
            _, bindings = agent.action_candidates(facts)
        self.assertEqual([v[0] for v in bindings.values()], ['travel_to:' + name])
        with patch.object(DualStoryAgent, 'action_candidates', return_value=({}, {})):
            _, bindings = agent.action_candidates({**facts, 'x': 24, 'y': 6})
        self.assertEqual([v[0] for v in bindings.values()], ['push_puzzle:0,' + flag])

    def test_observed_pushback_backchains_key_without_removing_real_barriers(self):
        from types import SimpleNamespace
        from openpokered.story_rules import literal
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        condition = {'Call': {'callee': 'hasItem', 'args': [literal('KEY')]}}
        target = ('item', 'KEY', True)
        rule = Rule('guard', 'Gate', 'Gate:guard', [], [(condition, False)], [], ('movement', 'down', True), [])
        agent.index = SimpleNamespace(rules=[rule], coordinates=lambda r: [(2, 3)],
            frontier=lambda target, facts: [rule] if target == ('item', 'KEY', True) else [])
        original = {'Gate': {(2, 3), (8, 8)}}
        agent.game = SimpleNamespace(last_map='Road', script_navigation_barriers=original,
            navigation_excluded_maps=lambda: ())
        agent.navigation_facts = {'flags': {}, 'bag': {}}
        def relaxed_path(*args, **kwargs):
            if kwargs['blocked_maps'] == original:
                return None
            self.assertEqual(kwargs['blocked_maps'], {'Gate': {(8, 8)}})
            self.assertEqual(original, {'Gate': {(2, 3), (8, 8)}})
            return [(('Road', 0, 0), ''), (('Gate', 2, 3), 'up')]
        with patch.object(pt, 'bfs_cross', side_effect=relaxed_path):
            result = agent.discover_route_prerequisites(
                {'map_name': 'Road', 'player_x': 0, 'player_y': 0}, 'Room', [(1, 1)])
        self.assertEqual(result, [target])
        self.assertIs(agent.game.script_navigation_barriers, original)
        # If a route preserving the gate exists, its incidental shortcut
        # through the guard must not introduce a spurious key dependency.
        agent.maps = {'PalletTown': {'npcs': []}}
        with patch.object(pt, 'bfs_cross', return_value=[(('Road', 0, 0), ''),
                (('PalletTown', 5, 5), 'up')]) as search:
            self.assertEqual(agent.discover_route_prerequisites(
                {'map_name': 'Road', 'player_x': 0, 'player_y': 0}, 'Room', [(1, 1)]), [])
            self.assertEqual(search.call_count, 1)
            self.assertEqual(search.call_args.kwargs['blocked_maps'], original)

    def test_excluded_region_unlock_requires_observed_guard_before_entry(self):
        from types import SimpleNamespace
        from openpokered.story_rules import literal
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        condition = {'Call': {'callee': 'hasItem', 'args': [literal('DRINK')]}}
        target = ('item', 'DRINK', True)
        guard = Rule('guard', 'Gate', 'Gate:guard', [], [(condition, False)], [],
                     ('movement', 'down', True), [])
        agent.index = SimpleNamespace(rules=[guard], coordinates=lambda _: [(2, 3)],
            frontier=lambda goal, facts: [guard] if goal == target else [])
        barriers = {'Gate': {(2, 3), (8, 8)}}
        agent.game = SimpleNamespace(last_map='Road', script_navigation_barriers=barriers,
            navigation_excluded_maps=lambda: ('ClosedCity',))
        agent.navigation_facts = {'flags': {}, 'bag': {}}
        state = {'map_name': 'Road', 'player_x': 0, 'player_y': 0}
        for nodes, expected in [
            ([('Gate', 2, 3), ('ClosedCity', 1, 1)], [target]),
            ([('ClosedCity', 1, 1), ('Gate', 2, 3)], []),
            ([('ClosedCity', 1, 1)], [])]:
            with self.subTest(nodes=nodes):
                def search(*args, **kwargs):
                    if kwargs['excluded_maps'] or kwargs['blocked_maps'] == barriers:
                        return None
                    self.assertEqual(kwargs['blocked_maps'], {'Gate': {(8, 8)}})
                    return [('Road', 0, 0), *[(node, 'up') for node in nodes]]
                with patch.object(pt, 'bfs_cross', side_effect=search):
                    self.assertEqual(agent.discover_route_prerequisites(state, 'Room', [(1, 1)]), expected)
                self.assertEqual(barriers, {'Gate': {(2, 3), (8, 8)}})
                self.assertEqual(agent.game.navigation_excluded_maps(), ('ClosedCity',))

    def test_puzzle_walk_avoids_fall_holes_and_restores_navigation_barriers(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock(script_navigation_barriers={'OtherRoom': {(1, 1)}})
        original = agent.game.script_navigation_barriers
        holes = {(3, 16), (6, 16)}
        rocks = {(3, 15), (4, 14), (9, 12), (6, 15)}
        def walk(x, y, name, **kwargs):
            blocked = {name: rocks | agent.game.script_navigation_barriers.get(name, set())}
            path = pt.bfs_cross(name, (7, 15), name, (x, y), blocked_maps=blocked, last_map='Route20')
            self.assertTrue(path)
            self.assertFalse(any(node[0] == name and tuple(node[1:]) in holes for node, _ in path[1:]))
        agent.game.nav_to_map.side_effect = walk
        agent.navigate_point('SeafoamIslandsB3F', (6, 14), avoid_tiles=holes)
        self.assertIs(agent.game.script_navigation_barriers, original)
        agent.game.nav_to_map.side_effect = pt.NavError('interrupted')
        with self.assertRaises(pt.NavError):
            agent.navigate_point('SeafoamIslandsB3F', (6, 14), avoid_tiles=holes)
        self.assertIs(agent.game.script_navigation_barriers, original)
        self.assertFalse(agent.game.navigation_active)

    def test_puzzle_approach_uses_the_stone_that_can_reach_its_hole(self):
        from openpokered.story_rules import MAPS_DIR
        from openpokered.boulder_skills import plan_pushes
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'SeafoamIslandsB2F'
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        agent.index = Mock(maps_dir=MAPS_DIR)
        agent.index.coordinates.return_value = []
        flag = 'EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE'
        rule = Rule('boulder:' + flag, name, name + ':engine_boulder', [], [], [], ('flag', flag, True), [])
        points = agent.destination_points(name, rule)
        self.assertIn((17, 6), points)
        self.assertNotIn((24, 6), points)
        self.assertNotIn((19, 6), points)  # A hole is not a safe approach tile.
        self.assertIsNone(plan_pushes(name, (24, 6), [(18, 6)], [], (19, 6)))
        self.assertTrue(plan_pushes(name, (17, 6), [(18, 6)], [], (19, 6)))

    def test_travel_chooses_reachable_side_of_key_with_live_trainer_collision(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock(last_map='SaffronCity')
        agent.game.st.return_value = {'map_name': 'SilphCo5F', 'player_x': 26, 'player_y': 1}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = {(28, 4), (21, 16), (13, 9), (8, 16), (8, 3),
            (18, 10), (2, 13), (4, 6), (22, 12), (25, 10), (24, 6)}
        agent.index = object()
        agent.facts = Mock(return_value={})
        agent.observed_navigation_barriers = Mock(return_value={})
        agent.navigate_point = Mock(return_value=(20, 16))
        rule = Rule('key', 'SilphCo5F', 'SilphCo5F:itemCardKey', [], [], [], ('item', 'CARD_KEY', True), [])
        result = agent.travel('SilphCo5F', rule, [(22, 16), (20, 16)])
        self.assertEqual(result['position'], (20, 16))
        agent.navigate_point.assert_called_once_with('SilphCo5F', (20, 16), goal_points=[(22, 16), (20, 16)])

    def test_travel_reports_the_live_arrival_not_the_preselected_approach(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock(last_map='SaffronCity')
        agent.game.st.return_value = {'map_name': 'SilphCo4F', 'player_x': 26, 'player_y': 1}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.navigate_point = Mock(return_value=(20, 16))
        rule = Rule('key', 'SilphCo5F', 'SilphCo5F:itemCardKey', [], [], [], ('item', 'CARD_KEY', True), [])
        with patch('playthrough.bfs_cross', side_effect=[['short'], ['long', 'path']]):
            result = agent.travel('SilphCo5F', rule, [(22, 16), (20, 16)])
        agent.navigate_point.assert_called_once_with('SilphCo5F', (22, 16), goal_points=[(22, 16), (20, 16)])
        self.assertEqual(result['position'], (20, 16))

    def test_navigation_dependencies_expand_recursively_but_ignore_unrelated_memories(self):
        from types import SimpleNamespace
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        main = ('flag', 'BOSS', True)
        prerequisite = ('flag', 'RESCUE', True)
        pickup = ('visibility', 'BALL', False)
        rescue = Rule('rescue', 'Tower', 'Tower:rescue', [], [], [], prerequisite, [])
        clear = Rule('clear', 'Tower', 'Tower:pickup', [], [], [], pickup, [])
        agent.index = SimpleNamespace(rules=[], npc_toggles={('Gate', 1): ('SLEEPER', False),
            ('Tower', 2): ('BALL', True), ('OldRoom', 3): ('OLD', True)},
            satisfied=lambda target, facts: target[1] in facts.get('completed', []),
            frontier=lambda target, facts: {('visibility', 'SLEEPER', False): [rescue],
                pickup: [clear]}.get(target, []))
        agent.maps, agent.field_requirements, agent.navigation_memory = {}, {}, {}
        agent.navigation_blockage = None
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'Road'}, {'to_map': 'End'}]}
        # The dependent memory comes first, before its parent is discovered.
        agent.navigation_history = {
            'child': {'map': 'Tower', 'destination': 'Tower', 'goal': prerequisite, 'blocking_npcs': [2]},
            'parent': {'map': 'Gate', 'destination': 'End', 'goal': main, 'blocking_npcs': [1]},
            'stale': {'map': 'OldRoom', 'destination': 'OldRoom', 'goal': ('flag', 'OLD_DETOUR', True), 'blocking_npcs': [3]}}
        groups = {'main': {'target': main, 'rules': [], 'objectives': []}}
        # Topology omits the gatehouse; the actual map warp includes it.
        with patch.object(pt, 'MAPS', {'Road': {'warps': [{'dest_map_name': 'Gate'}]}}):
            agent.add_navigation_groups(groups, {'map': 'City'})
        self.assertEqual({tuple(g['target']) for g in groups.values()}, {main, prerequisite, pickup})
        completed = {'main': {'target': main, 'rules': [], 'objectives': []}}
        agent.add_navigation_groups(completed, {'map': 'City', 'completed': ['BOSS']})
        self.assertEqual(list(completed), ['main'])

    def test_reachable_exact_goal_does_not_reactivate_historical_detour(self):
        from types import SimpleNamespace
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        target = ('location', ['Road', 10, 104], True)
        unlock = ('flag', 'OLD_PUZZLE', True)
        rule = Rule('open', 'SideRoom', 'open', [], [], [], unlock, [])
        agent.index = SimpleNamespace(rules=[], npc_toggles={('SideRoom', 1): ('BLOCKER', False)},
            satisfied=lambda goal, facts: False, frontier=lambda goal, facts: [rule])
        agent.maps, agent.field_requirements, agent.navigation_memory = {}, {}, {}
        blockage = {'map': 'SideRoom', 'destination': 'Road', 'goal': target, 'blocking_npcs': [1]}
        agent.navigation_history, agent.navigation_blockage = {'old': blockage}, None
        agent.game = SimpleNamespace(last_map='Town', stationary_npcs={},
            navigation_barriers=lambda: {'SideRoom': {(2, 3)}},
            navigation_excluded_maps=lambda: {'LockedHouse'})
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'Road'}]}
        geometry = {'Town': {}, 'Road': {'warps': [{'dest_map_name': 'SideRoom'}]}}
        facts = {'map': 'Town', 'x': 5, 'y': 6}
        for path, expected in [([('Town', 5, 6), (('Road', 10, 104), 'up')], {str(target)}),
                               (None, {str(target), str(unlock)})]:
            groups = {'main': {'target': target, 'rules': [], 'objectives': []}}
            with self.subTest(route_found=bool(path)), patch.object(pt, 'MAPS', geometry), \
                    patch.object(pt, 'bfs_cross', return_value=path) as bfs:
                agent.add_navigation_groups(groups, facts)
                self.assertEqual({str(g['target']) for g in groups.values()}, expected)
                bfs.assert_called_once_with('Town', (5, 6), 'Road', (10, 104),
                    last_map='Town', allow_ledges=True, allow_spinners=True,
                    blocked_maps={'SideRoom': {(2, 3)}}, excluded_maps={'LockedHouse'},
                    goal_nodes={('Road', 10, 104)})
            self.assertEqual(agent.navigation_history, {'old': blockage})  # Preserve history for a return.

    def nested_npc_corridor_fixture(self):
        from types import SimpleNamespace
        from openpokered.story_rules import literal
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        target = ('flag', 'BOSS', True)
        flag = 'EVENT_BEAT_DEEP_GATE_TRAINER'
        blockage = {'map': 'DeepGate', 'destination': 'Gym', 'goal': target,
                    'blocking_trainers': [2], 'blocking_npcs': [2]}
        program = [{'Call': {'callee': 'getFlag', 'args': [literal(flag)]}}]
        agent.index = SimpleNamespace(rules=[], npc_toggles={},
            configs={'DeepGate': {'npcs': [{'id': 2, 'talk': 'talkTrainer'}]}},
            stories={'DeepGate:talkTrainer': {'program': program}},
            satisfied=lambda goal, facts: False)
        agent.maps, agent.field_requirements, agent.navigation_memory = {}, {}, {}
        agent.navigation_history, agent.navigation_blockage = {'old': blockage}, None
        agent.game = SimpleNamespace(stationary_npcs={})
        agent.remembered_goal_reachable = Mock(return_value=False)
        agent.client = Mock()
        agent.client.route.return_value = {'found': True,
                                           'legs': [{'to_map': 'Road'}, {'to_map': 'Gym'}]}
        geometry = {'Road': {'warps': [{'dest_map_name': 'Entrance'}]},
                    'Entrance': {'warps': [{'dest_map_name': 'DeepGate'}]}}
        groups = {'boss': {'target': target, 'rules': [], 'objectives': []}}
        return agent, groups, geometry, flag

    def test_map_only_corridor_cannot_discard_an_observed_nested_trainer_blocker(self):
        import playthrough as pt
        agent, groups, geometry, flag = self.nested_npc_corridor_fixture()
        with patch.object(pt, 'MAPS', geometry):
            agent.add_navigation_groups(groups, {'map': 'Town', 'flags': {}})
        added = groups['trainer:' + flag]
        self.assertEqual(added['target'], ('flag', flag, True))
        self.assertEqual(added['rules'][0].triggers, ['npc:2'])
        self.assertEqual(added['context']['observed_navigation_blockage']['map'], 'DeepGate')

    def test_nested_npc_memory_does_not_override_fresh_access_or_a_won_trainer(self):
        import playthrough as pt
        for reachable, defeated in ((True, False), (False, True)):
            with self.subTest(reachable=reachable, defeated=defeated):
                agent, groups, geometry, flag = self.nested_npc_corridor_fixture()
                agent.remembered_goal_reachable.return_value = reachable
                with patch.object(pt, 'MAPS', geometry):
                    agent.add_navigation_groups(groups, {'map': 'Town', 'flags': {flag: defeated}})
                self.assertEqual(list(groups), ['boss'])

    def test_map_only_corridor_still_defers_unrelated_old_puzzles_without_actor_evidence(self):
        import playthrough as pt
        agent, groups, geometry, _ = self.nested_npc_corridor_fixture()
        agent.navigation_history['old'].pop('blocking_trainers')
        agent.navigation_history['old'].pop('blocking_npcs')
        with patch.object(pt, 'MAPS', geometry):
            agent.add_navigation_groups(groups, {'map': 'Town', 'flags': {}})
        self.assertEqual(list(groups), ['boss'])

    def test_remembered_route_requires_trigger_not_generic_entry_evidence(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = Mock()
        agent.index.coordinates.return_value = []
        agent.maps = {'Room': {}}
        target = ('item', 'KEY', True)
        rule = Rule('unknown', 'Room', 'Room:unknown', [], [], [], target, [])
        blockage = {'destination': 'Room', 'goal': target}
        groups = {'main': {'target': target, 'rules': [rule]}}
        with patch.object(pt, 'MAPS', {'Town': {}, 'Room': {
                'warps': [{'x': 3, 'y': 7}]}}), \
                patch.object(pt, 'walkable', return_value=True), \
                patch.object(pt, 'warp_tiles', return_value=set()), \
                patch.object(pt, 'bfs_cross', return_value=['entry']) as bfs:
            self.assertTrue(agent.destination_points('Room', rule))
            self.assertEqual(agent.destination_points('Room', rule, allow_entry_fallback=False), [])
            self.assertFalse(agent.remembered_goal_reachable(blockage, groups,
                {'map': 'Town', 'x': 5, 'y': 6}, {}))
            bfs.assert_not_called()

    def test_reachable_alternative_producer_supersedes_old_pc_route_obstruction(self):
        from types import SimpleNamespace
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        target = ('pokemon', 'Pikachu', None)
        unlock = ('flag', 'OLD_DOOR', True)
        door = Rule('door', 'SideRoom', 'SideRoom:door', [], [], [], unlock, [])
        far = Rule('far-pc', 'FarPC', 'FarPC:pc', ['sign:1'], [], [], ('pc', 'storage', True), [])
        near = Rule('local-pc', 'Town', 'Town:pc', ['sign:1'], [], [], ('pc', 'storage', True), [])
        blockage = {'map': 'SideRoom', 'destination': 'FarPC', 'goal': target, 'blocking_npcs': [1]}
        agent.index = SimpleNamespace(rules=[], npc_toggles={('SideRoom', 1): ('BLOCKER', False)},
            satisfied=lambda goal, facts: False, frontier=lambda goal, facts: [door])
        agent.maps, agent.field_requirements, agent.navigation_memory = {}, {}, {}
        agent.navigation_history, agent.navigation_blockage = {'old': blockage}, None
        agent.game = SimpleNamespace(last_map='Town', stationary_npcs={},
            navigation_barriers=lambda: {}, navigation_excluded_maps=lambda: ())
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'FarPC'}]}
        agent.destination_points = Mock(return_value=[(2, 3)])
        geometry = {'Town': {}, 'FarPC': {'warps': [{'dest_map_name': 'SideRoom'}]}}
        facts = {'map': 'Town', 'x': 5, 'y': 6}
        for local_reachable in (True, False):
            groups = {'retrieve': {'target': target, 'rules': [far, near], 'objectives': []}}
            def search(start_map, start, destination, point, **kwargs):
                return ['real PC trigger'] if local_reachable and destination == 'Town' else None
            with self.subTest(local_reachable=local_reachable), patch.object(pt, 'MAPS', geometry), \
                    patch.object(pt, 'bfs_cross', side_effect=search) as bfs:
                agent.add_navigation_groups(groups, facts)
            self.assertEqual(unlock in [g['target'] for g in groups.values()], not local_reachable)
            self.assertEqual(bfs.call_args_list[0].args[2], 'Town')
            self.assertTrue(all(call.kwargs == {'allow_entry_fallback': False}
                                for call in agent.destination_points.call_args_list))
            self.assertEqual(agent.navigation_history, {'old': blockage})

    def test_an_unrelated_reachable_producer_cannot_clear_another_goal_obstruction(self):
        from types import SimpleNamespace
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        target = ('pokemon', 'Pikachu', None)
        far = Rule('far', 'FarPC', 'FarPC:pc', ['sign:1'], [], [], ('pc', 'storage', True), [])
        near = Rule('near', 'Town', 'Town:heal', ['npc:1'], [], [], ('heal', 'party', True), [])
        groups = {'retrieve': {'target': target, 'rules': [far]},
                  'heal': {'target': ('heal', 'party', True), 'rules': [near]}}
        agent.destination_points = Mock(return_value=[(2, 3)])
        agent.game = SimpleNamespace(last_map='Town', navigation_barriers=lambda: {},
                                     navigation_excluded_maps=lambda: ())
        with patch.object(pt, 'MAPS', {'Town': {}, 'FarPC': {}}), \
                patch.object(pt, 'bfs_cross', return_value=None) as bfs:
            self.assertFalse(agent.remembered_goal_reachable(
                {'destination': 'FarPC', 'goal': target}, groups, {'map': 'Town', 'x': 5, 'y': 6}, {}))
        self.assertEqual([call.args[2] for call in bfs.call_args_list], ['FarPC'])

    def test_training_history_needs_a_live_target_and_unreachable_training_sites(self):
        from types import SimpleNamespace
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        target = ('level', 'Drowzee', 19)
        unlock = ('flag', 'OLD_DOOR', True)
        door = Rule('door', 'Room', 'Room:door', [], [], [], unlock, [])
        agent.index = SimpleNamespace(rules=[], npc_toggles={('Room', 1): ('BLOCKER', False)},
            satisfied=lambda goal, facts: False, frontier=lambda goal, facts: [door])
        agent.maps, agent.field_requirements, agent.navigation_memory = {}, {}, {}
        agent.navigation_blockage = None
        agent.navigation_history = {'old': {'map': 'Room', 'destination': 'OldGrass',
            'goal': target, 'blocking_npcs': [1]}}
        agent.game = SimpleNamespace(stationary_npcs={})
        agent.client = Mock()
        agent.client.route.return_value = {'found': False}
        groups = {}
        agent.add_navigation_groups(groups, {'map': 'Town'})
        self.assertEqual(groups, {})  # Historical training is not a standing goal.
        training = Rule('train', 'NearbyGrass', 'skill:train', [], [], [], target, [])
        agent.training_navigation = {'NearbyGrass': {'tile_route_found': True}}
        groups = {'train': {'target': target, 'rules': [training]}}
        agent.add_navigation_groups(groups, {'map': 'Town'})
        self.assertEqual(list(groups), ['train'])  # Reachable alternative needs no old door.
        agent.training_navigation['NearbyGrass']['tile_route_found'] = False
        agent.add_navigation_groups(groups, {'map': 'Town'})
        self.assertIn(unlock, [group['target'] for group in groups.values()])

    def test_fresh_exact_capture_terrain_supersedes_old_entrance_detour(self):
        from types import SimpleNamespace
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        target = ('catch', 'safari:ParkEast', True)
        unlock = ('flag', 'IN_PARK', False)
        exit_rule = Rule('exit', 'Gate', 'Gate:exit', [], [], [], unlock, [])
        agent.index = SimpleNamespace(rules=[], npc_toggles={('Gate', 1): ('GATE', False)},
            satisfied=lambda goal, facts: False, frontier=lambda goal, facts: [exit_rule])
        agent.maps, agent.field_requirements, agent.navigation_memory = {}, {}, {}
        agent.navigation_blockage = None
        blockage = {'map': 'Gate', 'destination': 'ParkEast', 'goal': target, 'blocking_npcs': [1]}
        agent.navigation_history = {'old': blockage}
        agent.game = SimpleNamespace(stationary_npcs={})
        agent.client = Mock()
        agent.client.route.return_value = {'found': False}
        capture = Rule('catch', 'ParkEast', 'skill:catch_encounter', [], [], [], target, [])
        for key, destination, found, cleared in [
                ('safari:ParkEast', 'ParkEast', True, True),
                ('safari:ParkEast', 'ParkEast', False, False),
                ('safari:ParkEast', 'ParkEast', None, False),
                ('safari:ParkEast', 'OtherArea', True, False),
                ('fishing:SuperRod:ParkEast', 'ParkEast', True, False)]:
            with self.subTest(key=key, destination=destination, found=found):
                agent.catch_navigation = {key: {'map': destination, 'tile_route_found': found}}
                groups = {'catch': {'target': target, 'rules': [capture]}}
                agent.add_navigation_groups(groups, {'map': 'ParkCenter'})
                self.assertEqual(unlock not in [g['target'] for g in groups.values()], cleared)
                self.assertEqual(agent.navigation_history, {'old': blockage})

    def test_load_autowalk_is_not_a_coordinate_pushback_prerequisite(self):
        from types import SimpleNamespace
        from openpokered.story_rules import literal
        target = ('level', 'Drowzee', 19)
        entered = ('flag', 'WALKED_IN', True)
        guard = ({'Call': {'callee': 'getFlag', 'args': [literal('WALKED_IN')]}}, False)
        autowalk = Rule('walk', 'Room', 'Room:@load', ['load'], [guard], [],
                        ('movement', 'movePlayerRelative', True), [])
        producer = Rule('enter', 'Room', 'Room:@load', ['load'], [], [], entered, [])
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = SimpleNamespace(rules=[autowalk], npc_toggles={},
            satisfied=lambda goal, facts: False, frontier=lambda goal, facts: [producer],
            coordinates=lambda rule: [] if 'load' in rule.triggers else [(4, 10)])
        agent.maps, agent.field_requirements, agent.navigation_memory = {}, {}, {}
        agent.navigation_blockage = None
        agent.navigation_history = {'old': {'map': 'Room', 'destination': 'Grass', 'goal': target}}
        agent.game = SimpleNamespace(stationary_npcs={})
        agent.client = Mock()
        agent.client.route.return_value = {'found': False}
        groups = {'train': {'target': target, 'rules': []}}
        agent.add_navigation_groups(groups, {'map': 'Town', 'flags': {}})
        self.assertEqual(list(groups), ['train'])
        autowalk.triggers = ['coord:exit']
        agent.add_navigation_groups(groups, {'map': 'Town', 'flags': {}})
        self.assertIn(entered, [group['target'] for group in groups.values()])

    def test_reachable_frontier_precedes_a_remote_npc_detour(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock(last_map='City')
        agent.game.st.return_value = {'map_name': 'City', 'player_x': 3, 'player_y': 3}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.index = None
        agent.client = Mock()
        agent.client.route.return_value = {'legs': [{'to_map': 'Frontier'}, {'to_map': 'Tower'}]}
        agent.destination_points = Mock(return_value=[(3, 4)])
        agent.remembered_route_blocker = Mock(return_value={'result': 'blocked',
            'blockage_map': 'Detour', 'blocking_npcs': [1]})
        agent.discover_route_prerequisites = Mock(return_value=[])
        agent.navigate_point = Mock()
        rule = Rule('goal', 'Tower', 'Tower:goal', [], [], [], ('flag', 'DONE', True), [])
        def path(source, position, destination, target, **kwargs):
            return [(('Frontier', 3, 4), 'up')] if destination == 'Frontier' else None
        with patch.object(pt, 'bfs_cross', side_effect=path), patch(
                'openpokered.autonomous_story.cut_requirement', return_value=None), patch(
                'openpokered.autonomous_story.surf_requirement', return_value=None):
            result = agent.travel('Tower', rule, [(5, 5)])
        self.assertEqual(result['stage'], 'Frontier')
        agent.navigate_point.assert_called_once_with('Frontier', (3, 4), tries=50)

    def test_remote_npc_does_not_hide_an_alternative_cut_route(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock(last_map='City')
        agent.game.st.return_value = {'map_name': 'City', 'player_x': 3, 'player_y': 3}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.index = None
        agent.field_requirements = {}
        agent.remembered_route_blocker = Mock(return_value={'result': 'blocked',
            'blockage_map': 'Road', 'blocking_npcs': [1]})
        tree = {'move': 'Cut', 'map': 'Detour', 'tree': [3, 4], 'stance': [3, 3]}
        rule = Rule('goal', 'Tower', 'Tower:goal', [], [], [], ('flag', 'DONE', True), [])
        with patch.object(pt, 'bfs_cross', return_value=None), patch(
                'openpokered.autonomous_story.cut_requirement', return_value=tree):
            result = agent.travel('Tower', rule, [(5, 5)])
        self.assertEqual(result['field_obstruction'], tree)
        self.assertEqual(result['blocking_npcs'], [1])
        self.assertEqual(agent.field_requirements['Cut'], tree)
        # A later NPC can also prevent the complete relaxed-tree route.
        # The same tree must still be discoverable toward an earlier region.
        agent.client = Mock()
        agent.client.route.return_value = {'legs': [{'to_map': 'Frontier'}, {'to_map': 'Tower'}]}
        agent.destination_points = Mock(return_value=[(3, 4)])
        with patch.object(pt, 'bfs_cross', return_value=None), patch(
                'openpokered.autonomous_story.cut_requirement', side_effect=[None, tree]):
            result = agent.travel('Tower', rule, [(5, 5)])
        self.assertEqual(result['field_obstruction']['frontier'], 'Frontier')
        self.assertEqual(result['field_obstruction']['tree'], [3, 4])

    def test_remote_trainer_positions_expire_while_story_guards_remain(self):
        from types import SimpleNamespace
        game = JevGame.__new__(JevGame)
        game.script_navigation_barriers = {}
        game._prev_map = 'Room'
        game.stationary_npcs = {'Hall': {1: (2, 3), 2: (4, 5)}}
        game.judgments = SimpleNamespace(
            index=SimpleNamespace(npc_toggles={('Hall', 2): ('GUARD', False)}),
            navigation_facts={'flags': {}},
            maps={'Hall': {'npcs': [{'textId': 1, 'isTrainer': True}, {'textId': 2}]}})
        self.assertEqual(game.navigation_barriers(), {'Hall': {(4, 5)}})

    def test_field_move_approach_reports_its_travel_blockage_before_using_move(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.actions, agent.max_actions = 0, 10
        agent.active = {'context': {'map': 'Route14', 'stance': [4, 42]}}
        agent.travel = Mock(return_value={'result': 'blocked', 'detail': 'A guard blocks the approach'})
        agent.remember_travel_result, agent.record, agent.game = Mock(), Mock(), Mock()
        rule = Rule('cut', 'Route14', 'skill:field', [], [], [], (), [])
        result = agent.execute('cut:0', rule)
        self.assertEqual(result['result'], 'blocked')
        agent.remember_travel_result.assert_called_once_with('Route14', result)
        agent.game.face.assert_not_called()

    def test_remote_observed_npc_blockage_keeps_its_map_in_recovery_memory(self):
        from types import SimpleNamespace
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = SimpleNamespace(last_map='CeruleanCity', script_navigation_barriers={},
            stationary_npcs={'CeruleanCity': {11: (27, 12)}})
        agent.maps = {'CeruleanCity': {'npcs': [{'textId': 11, 'isTrainer': False}]}}
        agent.client = Mock()
        agent.client.cmd.return_value = []
        agent.client.route.return_value = {'found': True, 'legs': []}
        state = {'map_name': 'CeruleanPokecenter', 'player_x': 3, 'player_y': 3}
        agent.client.state.return_value = state
        def path(*args, **kwargs):
            if (27, 12) in kwargs['blocked_maps']['CeruleanCity']:
                return None
            return [(('CeruleanPokecenter', 3, 3), None),
                    (('CeruleanCity', 27, 12), 'up'), (('Route5', 10, 13), 'down')]
        with patch.object(pt, 'bfs_cross', side_effect=path):
            result = agent.remembered_route_blocker(state, 'Route5', [(10, 13)],
                {'CeruleanCity': {(27, 12)}}, ())
        self.assertEqual(result['blocking_npcs'], [11])
        self.assertEqual(result['blockage_map'], 'CeruleanCity')
        agent.active = {'target': ('item', 'HM01', True)}
        agent.navigation_memory, agent.navigation_history = {}, {}
        agent.observed_barrier_maps = set()
        agent.remember_travel_result('Route5', result)
        self.assertEqual(agent.navigation_memory['Route5']['map'], 'CeruleanCity')
        self.assertEqual(agent.navigation_memory['Route5']['position'], [27, 12])
        with patch.object(pt, 'bfs_cross', return_value=[(('Route5', 10, 13), None)]):
            self.assertIsNone(agent.remembered_route_blocker(state, 'Route5', [(10, 13)],
                {'CeruleanCity': {(27, 12)}}, ()))

    def test_training_action_ranks_grounded_sites_without_redeciding_strategy(self):
        from openpokered.story_agent import DualStoryAgent
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        state = {'subgoal': ['level', 'leader', 26], 'local_state': {'party': [
            {'hp': 70, 'max_hp': 70, 'status': 'None', 'moves': ['Scratch'], 'pp': [35]}]}}
        candidates = {'a': json.dumps({'navigation': {'tile_route_found': True}})}
        with patch.object(DualStoryAgent, 'choose', return_value='a') as choose:
            self.assertEqual(agent.choose('action', state, candidates, 'choose'), 'a')
            self.assertFalse(choose.call_args.kwargs['allow_abstain'])
            agent.choose('strategy', state, candidates, 'choose')
            self.assertTrue(choose.call_args.kwargs['allow_abstain'])
            state['local_state']['party'][0]['pp'] = [0]
            agent.choose('action', state, candidates, 'choose')
            self.assertTrue(choose.call_args.kwargs['allow_abstain'])

    def test_training_uses_capable_member_instead_of_low_level_capture_lead(self):
        from openpokered.story_agent import DualStoryAgent
        weedle = {'species': 'Weedle', 'level': 3, 'hp': 12, 'max_hp': 12,
                  'status': 'None', 'moves': ['PoisonSting'], 'pp': [35]}
        charmeleon = {'species': 'Charmeleon', 'level': 21, 'hp': 52, 'max_hp': 52,
                      'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [35, 25]}
        self.assertIs(training_battler([weedle, charmeleon]), charmeleon)
        agent, facts, groups = self.preparation_agent('none')
        facts['party'] = [weedle, charmeleon]
        with patch.object(DualStoryAgent, 'strategy_groups', return_value=groups):
            self.assertNotIn('prepare:train', agent.strategy_groups(facts))

    def test_training_reorders_the_capable_member_before_seeking_an_encounter(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        rule = Rule('train', 'Route2', 'skill:train_encounter', [], [], [],
                    ('level', 'leader', 14), [])
        agent.active = {'target': ('level', 'leader', 14), 'rules': [rule]}
        facts = {'party': [
            {'species': 'Weedle', 'level': 3, 'hp': 12, 'moves': ['PoisonSting'], 'pp': [35]},
            {'species': 'Charmander', 'level': 12, 'hp': 35, 'moves': ['Scratch'], 'pp': [35]},
        ]}
        candidates, bindings = agent.action_candidates(facts)
        self.assertEqual(json.loads(candidates['action:0'])['operation'], 'lead_with:Charmander')
        self.assertEqual(bindings['action:0'][0], 'lead_with:Charmander')

    def test_capture_reorders_weak_lead_but_evolution_retains_its_source(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        rule = Rule('hunt', 'Route24', 'skill:catch_encounter', [], [], [],
                    ('catch', 'Route24', True), [])
        agent.active = {'target': rule.effect, 'rules': [rule],
                        'context': {'acquisition_method': 'grass'}}
        facts = {'party': [
            {'species': 'Pidgey', 'level': 9, 'hp': 27, 'moves': ['Gust'], 'pp': [35]},
            {'species': 'Charmeleon', 'level': 27, 'hp': 79, 'moves': ['Ember'], 'pp': [25]},
        ]}
        _, bindings = agent.action_candidates(facts)
        self.assertEqual(bindings['action:0'][0], 'lead_with:Charmeleon')
        agent.active = {'target': ('register', 'Pidgeotto', True), 'rules': [rule],
                        'context': {'acquisition_method': 'evolution', 'from_species': 'Pidgey',
                                    'trigger': 'level', 'species': 'Pidgeotto', 'level': 18}}
        agent.find_training_sites = Mock(return_value={'Route24': (5, 18)})
        agent.training_sites = {'Route24': (5, 18)}
        facts.update(map='Route24', x=5, y=18)
        _, bindings = agent.action_candidates(facts)
        self.assertEqual(bindings['action:0'][0], 'train_encounter:Route24,5,18')

    def test_training_is_not_offered_when_the_skill_would_stop_for_recovery(self):
        from openpokered.story_agent import DualStoryAgent
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        agent = AutonomousStoryAgent(client, Mock(), [{'id': 'brock',
            'satisfied_when': {'flag': 'EVENT_BEAT_BROCK'}}], game=Mock())
        agent.index = Mock(rules=[], by_effect={})
        agent.defeat_preparation = 17
        healer = Rule('heal', 'PewterPokecenter', 'nurse', ['npc:1'], [], [], ('heal', 'party', True), [])
        agent.nearby_healers = Mock(return_value=[healer])
        agent.annotate_navigation = Mock(return_value={})
        agent.transport_frontiers = Mock()
        agent.find_training_sites = Mock(return_value={'Route2': (7, 7)})
        agent.find_catch_areas = Mock(return_value={})
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [8, 25]}
        facts = {'party': [mon], 'bag': {}, 'flags': {}, 'fully_recovered': False,
                 'map': 'PewterCity', 'x': 12, 'y': 18}
        with patch.object(DualStoryAgent, 'strategy_groups', return_value={}):
            groups = agent.strategy_groups(facts)
            self.assertIn('prepare:heal', groups)
            self.assertNotIn('prepare:train', groups)
            mon['pp'][0] = 35
            self.assertIn('prepare:train', agent.strategy_groups(facts))

    def test_real_object_approach_propagates_battle_pause_without_retrying(self):
        import playthrough as pt
        from openpokered.playthrough_judgments import NavigationPause
        game = JevGame.__new__(JevGame)
        game.pos = Mock(return_value=('CeruleanGym', 7, 10))
        game.live_npcs = Mock(return_value=set())
        game.nav_to = Mock(side_effect=NavigationPause('blackout moved the player'))
        game.face = Mock()
        with patch.object(pt, 'bfs', return_value=[(7, 10), (7, 9)]):
            with self.assertRaises(NavigationPause):
                game.approach_object(4, 2, 'CeruleanGym')
        self.assertEqual(game.nav_to.call_count, 1)
        game.face.assert_not_called()

    def test_pp_recovery_goal_matches_useful_owned_medicine(self):
        from openpokered.story_rules import StoryIndex
        index = StoryIndex.__new__(StoryIndex)
        mon = {'hp': 70, 'max_hp': 70, 'moves': ['Dig', 'Growl'], 'pp': [5, 0]}
        facts = {'party': [mon]}
        target = ('pp_reserve', 'party', True)
        self.assertFalse(index.satisfied(target, facts))
        self.assertEqual([item for item, _, _ in medicine_options([mon], {'Elixer': 1, 'Ether': 1})], ['Elixer'])
        mon['pp'][0] = 10
        self.assertTrue(index.satisfied(target, facts))
        self.assertEqual(list(medicine_options([mon], {'Elixer': 1})), [])

    def observed_pickup_route_fixture(self):
        from types import SimpleNamespace
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        target = ('visibility', 'PICKUP', False)
        pickup = Rule('pickup', 'Hall', 'Hall:pickup', ['npc:3'], [], ['YES'],
                      target, [('item', 'FOSSIL', True)])
        agent.index = SimpleNamespace(rules=[], npc_toggles={('Hall', 3): ('PICKUP', False)},
            frontier=lambda goal, facts: [pickup] if goal == target
                and not facts.get('flags', {}).get('__OBJ_HIDDEN_PICKUP') else [])
        agent.game = SimpleNamespace(last_map='Town', script_navigation_barriers={},
            stationary_npcs={'Hall': {2: (2, 2), 3: (3, 3)}},
            navigation_excluded_maps=lambda: ())
        agent.maps = {'Hall': {'npcs': []}, 'Town': {'npcs': []}}
        agent.navigation_facts = {'flags': {}}
        path = [(('Town', 1, 1), None), (('Hall', 2, 2), 'up'),
                (('Hall', 3, 3), 'up'), (('Hall', 4, 4), 'up')]
        return agent, target, path

    def test_route_relaxation_backchains_a_removable_observed_npc_on_the_exact_path(self):
        import playthrough as pt
        agent, target, path = self.observed_pickup_route_fixture()
        with patch.dict(pt.MAPS, {'Hall': {'width': 10}}), \
                patch.object(pt, 'bfs_cross', return_value=path):
            found = agent.discover_route_prerequisites(
                {'map_name': 'Town', 'player_x': 1, 'player_y': 1}, 'Hall', [(4, 4)])
        self.assertEqual(found, [target])
        self.assertEqual(agent.game.stationary_npcs['Hall'][3], (3, 3))
        self.assertEqual(agent.navigation_facts, {'flags': {}})

    def test_route_relaxation_does_not_backchain_off_path_or_already_hidden_npcs(self):
        import playthrough as pt
        for hidden in (False, True):
            with self.subTest(hidden=hidden):
                agent, _, path = self.observed_pickup_route_fixture()
                if hidden:
                    agent.navigation_facts['flags']['__OBJ_HIDDEN_PICKUP'] = True
                else:
                    agent.game.stationary_npcs['Hall'][3] = (99, 99)
                with patch.dict(pt.MAPS, {'Hall': {'width': 10}}), \
                        patch.object(pt, 'bfs_cross', return_value=path):
                    self.assertEqual(agent.discover_route_prerequisites(
                        {'map_name': 'Town', 'player_x': 1, 'player_y': 1}, 'Hall', [(4, 4)]), [])

    def test_route_relaxation_does_not_invent_npc_removal_without_a_real_producer_or_path(self):
        import playthrough as pt
        for no_path in (False, True):
            with self.subTest(no_path=no_path):
                agent, _, path = self.observed_pickup_route_fixture()
                if not no_path:
                    agent.index.frontier = lambda *_: []
                with patch.dict(pt.MAPS, {'Hall': {'width': 10}}), \
                        patch.object(pt, 'bfs_cross', return_value=None if no_path else path):
                    self.assertEqual(agent.discover_route_prerequisites(
                        {'map_name': 'Town', 'player_x': 1, 'player_y': 1}, 'Hall', [(4, 4)]), [])

    def test_route_relaxation_discovers_boulders_without_leaving_doors_open(self):
        from types import SimpleNamespace
        from openpokered.story_rules import MAPS_DIR
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'VictoryRoad2F'
        agent.maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        rule = Rule('door', name, name + ':@load', ['load'], [], [], ('block', name + ',3,4', 21), [])
        agent.index = SimpleNamespace(rules=[rule], frontier=Mock(return_value=[rule]))
        agent.game = SimpleNamespace(last_map='Route23', script_navigation_barriers={}, navigation_excluded_maps=lambda: ())
        agent.navigation_facts = {'flags': {}}
        state = {'map_name': name, 'player_x': 0, 'player_y': 8}
        original = list(pt.MAPS[name]['blocks'])
        targets = agent.discover_route_prerequisites(state, name, [(10, 5)])
        self.assertIn(('flag', 'EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1', True), targets)
        self.assertEqual(pt.MAPS[name]['blocks'], original)
        with patch.object(pt, 'bfs_cross', side_effect=RuntimeError('interrupted search')):
            with self.assertRaises(RuntimeError):
                agent.discover_route_prerequisites(state, name, [(10, 5)])
        self.assertEqual(pt.MAPS[name]['blocks'], original)

    def test_training_can_consider_unvisited_grass_on_the_explored_frontier(self):
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        agent.visited = {'PewterCity', 'Route1', 'Route2', 'ViridianCity', 'ViridianForest'}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [0]}
        agent.game = Mock(last_map='PewterCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        sites = agent.find_training_sites({'map': 'PewterCity', 'x': 18, 'y': 18, 'party': [{'level': 16}]})
        self.assertIn('Route3', sites)
        self.assertNotIn('Route3', agent.visited)
        self.assertNotIn('VictoryRoad1F', sites)

    def test_training_excludes_capture_only_safari_but_retains_normal_grass(self):
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        agent.visited = {'SafariZoneCenter', 'SafariZoneWest', 'Route15'}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': []}
        agent.game = Mock(last_map='FuchsiaCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        with patch('openpokered.autonomous_story.pt.bfs_cross', return_value=[]):
            sites = agent.find_training_sites({'map': 'FuchsiaCity', 'x': 19, 'y': 18,
                                               'party': [{'level': 58}]})
        self.assertIn('Route15', sites)
        self.assertFalse(any(name.startswith('SafariZone') for name in sites))
        requested = {call.args[1] for call in agent.client.route.call_args_list}
        self.assertFalse(any(name.startswith('SafariZone') for name in requested))

    def test_safari_training_execution_is_rejected_before_travel_or_input(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.actions, agent.max_actions = 0, 10
        agent.record, agent.client, agent.game = Mock(), Mock(), Mock()
        result = agent.execute('train_encounter:SafariZoneCenter,12,22',
                               Mock(storyline='skill:evolve'))
        self.assertEqual(result['required_capability'], 'experience_awarding_battle')
        agent.client.state.assert_not_called()
        agent.game.st.assert_not_called()

    def test_native_storage_guard_overrides_a_stale_public_box_count(self):
        from openpokered.playthrough_judgments import capture_storage_full
        state = {'party': [{'species': 'Cubone'}] * 6, 'box_counts': [19],
                 'current_box_index': 0,
                 'battle_live': {'capture_blocked_reason': 'storage_full'}}
        self.assertTrue(capture_storage_full(state))
        game = JevGame.__new__(JevGame)
        self.assertEqual(game.safari_battle_action(state), 'run')

    def test_training_route_checks_current_npcs_and_keeps_reachable_alternative(self):
        from openpokered.story_rules import MAPS_DIR
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        agent.visited = {'CeruleanCity'}
        agent.navigation_memory = {'Route5': {}, 'Route9': {}}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [0]}
        agent.game = Mock(last_map='CeruleanCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = {(27, 12)}
        agent.game.navigation_excluded_maps.return_value = ()
        def path(start, position, destination, target, **kwargs):
            self.assertIn((27, 12), kwargs['blocked_maps']['CeruleanCity'])
            if destination == 'Route24':
                return [(('CeruleanCity', *position), None), (('Route24', 5, 18), 'up')]
            return None
        with patch.object(pt, 'bfs_cross', side_effect=path):
            sites = agent.find_training_sites({'map': 'CeruleanCity', 'x': 19, 'y': 18, 'party': [{'level': 23}]})
        self.assertNotIn('Route5', sites)
        self.assertNotIn('Route9', sites)
        self.assertEqual(sites['Route24'], (5, 18))
        agent.active = {'context': {'acquisition_method': 'evolution', 'trigger': 'level'}}
        trainee_party = [{'species': 'NidoranF', 'level': 4, 'hp': 19,
                          'moves': ['Tackle'], 'pp': [35]},
                         {'species': 'Charizard', 'level': 36, 'hp': 120,
                          'moves': ['Ember'], 'pp': [25]}]
        with patch.object(pt, 'bfs_cross', side_effect=path):
            switch_sites = agent.find_training_sites({'map': 'CeruleanCity', 'x': 19,
                'y': 18, 'party': trainee_party})
        self.assertIn('Route24', switch_sites)
        agent.active = {'target': ('level', 'leader', 25), 'rules': [
            Rule('train', 'Route24', 'skill:train_encounter', [], [], [], ('level', 'leader', 25), [])]}
        candidates, _ = agent.action_candidates({})
        self.assertTrue(json.loads(candidates['action:0'])['navigation']['tile_route_found'])

    def test_training_site_can_be_the_current_cave_floor(self):
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'VictoryRoad2F'
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        agent.visited = {name}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': []}
        agent.game = Mock(last_map='Route23')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        sites = agent.find_training_sites({'map': name, 'x': 0, 'y': 7, 'party': [{'level': 61}]})
        self.assertEqual(sites[name], (0, 7))
        self.assertEqual(agent.training_navigation[name]['steps'], 0)

    def test_catch_areas_skip_registered_tables_and_record_their_navigation(self):
        from openpokered.story_rules import MAPS_DIR
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())
                      for name in ('ViridianCity', 'Route1', 'Route2')}
        agent.visited = {'ViridianCity'}
        agent.navigation_memory = {}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [0]}
        agent.game = Mock(last_map='ViridianCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        owned = {mon['species'] for mon in agent.maps['Route1']['wild']['red']['grass']['mons']}
        self.assertEqual(owned, {'Pidgey', 'Rattata'})
        path = [(('ViridianCity', 19, 18), None), (('Route2', 5, 17), 'up'), (('Route2', 5, 18), 'up')]
        with patch.object(pt, 'bfs_cross', return_value=path) as search:
            areas = agent.find_catch_areas({'map': 'ViridianCity', 'x': 19, 'y': 18,
                                            'dex': {'owned_species': sorted(owned)}})
        # Route1 holds no unregistered species, so it is never even searched.
        self.assertEqual(search.call_args.args[2], 'Route2')
        self.assertEqual(list(areas), ['Route2'])
        self.assertEqual(areas['Route2']['species'], ['Weedle'])
        self.assertEqual(areas['Route2']['spots'], [(5, 18)])
        self.assertTrue(areas['Route2']['reachable'])
        self.assertEqual(list(agent.catch_navigation), ['Route2'])
        self.assertTrue(agent.catch_navigation['Route2']['tile_route_found'])
        self.assertEqual(agent.catch_navigation['Route2']['steps'], 2)

    def ranked_catch_agent(self):
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())
                      for name in ('PewterCity', 'Route2', 'Route3')}
        agent.visited = {'PewterCity'}
        agent.navigation_memory = {}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [0]}
        agent.game = Mock(last_map='PewterCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        return agent

    def test_catch_areas_expand_past_an_exhausted_neighbourhood(self):
        import playthrough as pt
        from openpokered.story_rules import MAPS_DIR
        # Everything one hop from PewterCity is Route2/Route3 grass; owning
        # those species leaves nothing there, and only a wider ring helps.
        exhausted = ['Pidgey', 'Rattata', 'Weedle', 'Jigglypuff', 'Spearow']
        facts = {'map': 'PewterCity', 'x': 14, 'y': 7, 'dex': {'owned_species': exhausted}}
        with patch.object(pt, 'bfs_cross', return_value=[(('PewterCity', 14, 7), None)]):
            near_only = self.ranked_catch_agent().find_catch_areas(dict(facts))
        self.assertEqual(near_only, {})  # Control: the old one-hop search found nothing.
        agent = self.ranked_catch_agent()
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())
                      for name in ('PewterCity', 'Route2', 'Route3', 'Route4')}
        with patch.object(pt, 'bfs_cross', return_value=[(('PewterCity', 14, 7), None)]):
            expanded = agent.find_catch_areas(dict(facts))
        self.assertIn('Route4', expanded)
        self.assertEqual(expanded['Route4']['species'], ['Ekans'])

    def test_forest_and_safari_interiors_are_both_huntable(self):
        import playthrough as pt
        from openpokered.story_rules import MAPS_DIR
        agent = self.ranked_catch_agent()
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())
                      for name in ('PewterCity', 'Route2', 'Route3', 'ViridianForest',
                                   'SafariZoneCenter')}
        agent.visited = {'ViridianForest', 'SafariZoneCenter'}
        facts = {'map': 'ViridianForest', 'x': 16, 'y': 43, 'dex': {'owned_species': []}}
        with patch.object(pt, 'bfs_cross', return_value=[(('ViridianForest', 16, 43), None)]):
            areas = agent.find_catch_areas(dict(facts))
        self.assertIn('ViridianForest', areas)
        self.assertEqual(areas['ViridianForest']['species'],
                         ['Caterpie', 'Kakuna', 'Metapod', 'Pikachu', 'Weedle'])
        safari = areas['safari:SafariZoneCenter']
        self.assertEqual(safari['method'], 'safari')
        self.assertIn('Scyther', safari['species'])

    def test_catch_ranking_prefers_more_unregistered_species_at_equal_distance(self):
        import playthrough as pt
        agent = self.ranked_catch_agent()
        equal_paths = [(('PewterCity', 14, 7), None), (('Route2', 5, 18), 'up')]
        with patch.object(pt, 'bfs_cross', return_value=equal_paths):
            areas = agent.find_catch_areas({'map': 'PewterCity', 'x': 14, 'y': 7,
                                            'dex': {'owned_species': ['Pidgey', 'Rattata']}})
        # Route3 still holds two unregistered species where Route2 holds one,
        # so the richer table outranks the closer name at the same distance.
        self.assertEqual(list(areas), ['Route3', 'Route2'])
        self.assertEqual(areas['Route2']['species'], ['Weedle'])

    def test_catch_ranking_demotes_an_area_with_unproductive_recent_hunts(self):
        import playthrough as pt
        agent = self.ranked_catch_agent()
        agent.catch_attempts = [{'map': 'Route2', 'registered': False},
                                {'map': 'Route2', 'registered': False},
                                {'map': 'Route3', 'registered': True}]
        equal_paths = [(('PewterCity', 14, 7), None), (('Route2', 5, 18), 'up')]
        facts = {'map': 'PewterCity', 'x': 14, 'y': 7, 'dex': {'owned_species': []}}
        with patch.object(pt, 'bfs_cross', return_value=equal_paths):
            self.assertEqual(list(agent.find_catch_areas(facts)), ['Route3', 'Route2'])
        # With no history at all the tie falls back to the map name, matching
        # the old distance-only order.
        with patch.object(pt, 'bfs_cross', return_value=equal_paths):
            self.assertEqual(list(self.ranked_catch_agent().find_catch_areas(facts)),
                             ['Route2', 'Route3'])

    def test_encounter_value_uses_real_slot_weights_and_step_rate(self):
        table = {'wild': {'red': {'grass': {'encounterRate': 25, 'mons': [
            {'level': 3, 'species': 'Common'}, {'level': 4, 'species': 'Common'},
            {'level': 5, 'species': 'Common'}, {'level': 6, 'species': 'Common'},
            {'level': 7, 'species': 'Common'}, {'level': 8, 'species': 'Common'},
            {'level': 9, 'species': 'Common'}, {'level': 10, 'species': 'Common'},
            {'level': 11, 'species': 'Common'}, {'level': 12, 'species': 'Pidgey'},
        ]}}}}
        value = encounter_value(table, {'Common'})
        target = value['targets'][0]
        self.assertEqual(target['species'], 'Pidgey')
        self.assertEqual(target['encounter_share_pct'], 1.2)  # threshold 253..255 = 3/256
        self.assertEqual(target['levels'], [12, 12])
        self.assertEqual(value['new_species_per_step_pct'], 0.11)
        self.assertEqual(value['expected_steps_to_any_new_species'], 873.8)

    def test_unified_red_acquisition_graph_contains_86_wild_species(self):
        from openpokered.collection_planner import acquisition_graph, SUPER_ROD_MAP_GROUP
        from openpokered.story_rules import MAPS_DIR
        maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        graph = acquisition_graph(maps, SUPER_ROD_MAP_GROUP)
        self.assertEqual(len(graph), 86)
        self.assertEqual({row['method'] for row in graph['Scyther']}, {'safari'})
        self.assertTrue(any(row['method'] == 'water' for row in graph['Tentacool']))
        self.assertTrue(any(row.get('rod') == 'SuperRod' for row in graph['Dratini']))
        self.assertEqual({row['method'] for row in graph['Magikarp']}, {'fishing'})

    def test_complete_red_solo_graph_proves_the_124_species_ceiling(self):
        from openpokered.collection_planner import (complete_acquisition_graph,
            solo_plan, SUPER_ROD_MAP_GROUP)
        from openpokered.story_rules import MAPS_DIR
        maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        graph = complete_acquisition_graph(maps, SUPER_ROD_MAP_GROUP)
        plan = solo_plan(graph)
        self.assertEqual(len(graph), 151)
        self.assertEqual(plan['ceiling'], 124)
        self.assertEqual(len(plan['unreachable_species']), 27)
        self.assertEqual({'Alakazam', 'Gengar', 'Golem', 'Machamp'} &
                         set(plan['unreachable_species']),
                         {'Alakazam', 'Gengar', 'Golem', 'Machamp'})
        self.assertIn('Mew', plan['unreachable_species'])
        self.assertEqual(len({'Bulbasaur', 'Charmander', 'Squirtle'} &
                             set(plan['reachable_species'])), 1)
        self.assertEqual(plan['optimal_assignment_count'], 36)
        self.assertEqual(plan['optimal_choices']['starter'],
                         ['Bulbasaur', 'Charmander', 'Squirtle'])
        self.assertEqual(plan['optimal_choices']['fossil'], ['Kabuto', 'Omanyte'])
        self.assertEqual(plan['optimal_choices']['dojo'], ['Hitmonchan', 'Hitmonlee'])
        self.assertEqual(plan['optimal_choices']['eevee_evolution'],
                         ['Flareon', 'Jolteon', 'Vaporeon'])
        self.assertEqual(len(plan['choice_reachable_species']), 135)
        self.assertEqual(len(plan['always_unreachable_species']), 16)

    def test_complete_graph_keeps_exact_solo_choice_and_external_trade_reasons(self):
        from openpokered.collection_planner import (complete_acquisition_graph,
            solo_plan, SUPER_ROD_MAP_GROUP)
        from openpokered.story_rules import MAPS_DIR
        maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        graph = complete_acquisition_graph(maps, SUPER_ROD_MAP_GROUP)
        forced = {'starter': 'Bulbasaur', 'fossil': 'Kabuto',
                  'dojo': 'Hitmonchan', 'eevee_evolution': 'Jolteon'}
        plan = solo_plan(graph, forced_choices=forced)
        self.assertEqual(plan['ceiling'], 124)
        self.assertEqual(plan['choices'], forced)
        self.assertEqual(plan['optimal_assignment_count'], 1)
        self.assertEqual(plan['optimal_choices'], {
            'dojo': ['Hitmonchan'], 'eevee_evolution': ['Jolteon'],
            'fossil': ['Kabuto'], 'starter': ['Bulbasaur'],
        })
        self.assertTrue(any(row.get('external_trade') for row in graph['Alakazam']))
        self.assertTrue(any(row.get('source_version') == 'blue' for row in graph['Vulpix']))
        self.assertEqual(next(row['coins'] for row in graph['Porygon']
                              if row['method'] == 'prize'), 9999)
        self.assertFalse(any(row.get('storyline') == 'coordGhostMarowak'
                             for row in graph['Marowak']))

    def test_acquisition_contract_exposes_code_owned_conditions_and_costs(self):
        from openpokered.collection_planner import acquisition_contract
        prize = acquisition_contract('Porygon', {
            'method': 'prize', 'map': 'GameCornerPrizeRoom', 'coins': 9999})
        self.assertIn({'kind': 'visit_map', 'value': 'GameCornerPrizeRoom'},
                      prize['requirements'])
        self.assertIn({'kind': 'coin_case', 'value': True}, prize['requirements'])
        self.assertEqual(prize['direct_cost']['coins'], 9999)

        evolution = acquisition_contract('Arcanine', {
            'method': 'evolution', 'from_species': 'Growlithe',
            'trigger': 'item', 'item': 'FireStone'})
        self.assertIn({'kind': 'party_species', 'value': 'Growlithe'},
                      evolution['requirements'])
        self.assertEqual(evolution['direct_cost']['consumed_items'], {'FireStone': 1})

        trade = acquisition_contract('Farfetchd', {
            'method': 'npc_trade', 'map': 'VermilionTradeHouse',
            'from_species': 'Spearow'})
        self.assertIn({'kind': 'party_species', 'value': 'Spearow'}, trade['requirements'])
        self.assertIn({'kind': 'party_members_at_least', 'value': 2}, trade['requirements'])
        self.assertEqual(trade['direct_cost']['relinquished_species'], ['Spearow'])

        choice = acquisition_contract('Bulbasaur', {
            'method': 'gift', 'map': 'OaksLab', 'exclusive_group': 'starter',
            'choice': 'Bulbasaur'})
        self.assertEqual(choice['direct_cost']['irreversible_choice'],
                         {'group': 'starter', 'choice': 'Bulbasaur'})

    def test_nonwild_collection_groups_offer_gift_and_item_evolution(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        gift = Rule('gift', 'House', 'House:talkGift', [], [], [],
                    ('pokemon', 'LAPRAS', 15), [])
        agent.index = Mock(rules=[gift], by_effect={})
        agent._complete_collection_graph = {
            'Growlithe': [{'method': 'grass', 'map': 'Route8'}],
            'Arcanine': [{'method': 'evolution', 'from_species': 'Growlithe',
                          'trigger': 'item', 'item': 'FireStone'}],
            'Lapras': [{'method': 'gift', 'map': 'House', 'storyline': 'talkGift',
                        'level': 15}],
        }
        facts = {'party': [{'species': 'Growlithe', 'level': 20}], 'stored_pokemon': [],
                 'bag': {'FIRESTONE': 1}, 'flags': {}, 'coins': 0, 'money': 0,
                 'map': 'Route8', 'dex': {'owned_species': ['Growlithe']}}
        groups = {}
        agent.add_nonwild_collection_groups(groups, facts)
        self.assertEqual(groups['register:Arcanine:evolution:Growlithe']['target'],
                         ('register', 'Arcanine', True))
        self.assertEqual(groups['register:Lapras:gift:House']['rules'], [gift])

    def test_overlevel_evolution_requires_a_real_new_level_not_zero_experience(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        agent.index = Mock(rules=[], by_effect={})
        agent._complete_collection_graph = {
            'Tentacool': [{'method': 'water', 'map': 'Route19'}],
            'Tentacruel': [{'method': 'evolution', 'from_species': 'Tentacool',
                           'trigger': 'level', 'level': 30}]}
        agent.visited = {'Route19'}
        agent.maps = {'Route19': {'wild': {'red': {'grass': {
            'encounterRate': 25, 'mons': [{'species': 'Pidgey', 'level': 12}] * 10}}}}}
        facts = {'party': [{'species': 'Tentacool', 'level': 40, 'hp': 89, 'max_hp': 89,
                           'moves': ['Bubblebeam'], 'pp': [20], 'status': 'None'}],
                 'stored_pokemon': [], 'bag': {}, 'flags': {}, 'coins': 0, 'money': 0,
                 'map': 'FuchsiaCity', 'dex': {'owned_species': ['Tentacool']}}
        for current, trigger in ((29, 30), (30, 31), (40, 41)):
            facts['party'][0]['level'] = current
            groups = {}
            agent.add_nonwild_collection_groups(groups, facts)
            group = groups['register:Tentacruel:evolution:Tentacool']
            context = group['context']
            self.assertEqual(context['level'], 30)
            self.assertEqual(context['experience_trigger_level'], trigger)
            self.assertEqual(context['training_cost']['levels_remaining'], 1)
            self.assertGreater(context['training_cost']['remaining_experience_max'], 0)
            agent.active = group
            agent.find_training_sites = Mock(return_value={'Route19': (5, 18)})
            agent.training_sites = {'Route19': (5, 18)}
            facts.update(map='FuchsiaCity', x=10, y=10)
            _, bindings = agent.action_candidates(facts)
            self.assertEqual(bindings['action:0'][0], 'reach_training:Route19,5,18')
            facts.update(map='Route19', x=5, y=18)
            candidates, _ = agent.action_candidates(facts)
            action = json.loads(candidates['action:0'])
            self.assertEqual(action['required_level'], trigger)
            self.assertEqual(action['natural_evolution_level'], 30)
            self.assertEqual(action['training_cost']['levels_remaining'], 1)
        facts['party'][0]['level'] = 100
        groups = {}
        agent.add_nonwild_collection_groups(groups, facts)
        self.assertNotIn('register:Tentacruel:evolution:Tentacool', groups)

    def test_party_capacity_preparation_requires_explored_ready_acquisition(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.visited = {'CeruleanCity'}
        rule = Mock()
        rule.missing.return_value = []
        agent.acquisition_story_rules = Mock(return_value=[rule])
        facts = {'map': 'CeruleanPokecenter'}
        method = {'map': 'CeladonMansionRoof', 'method': 'gift'}
        self.assertFalse(agent.acquisition_capacity_ready(facts, 'Eevee', method))
        agent.visited.add('CeladonMansionRoof')
        self.assertTrue(agent.acquisition_capacity_ready(facts, 'Eevee', method))
        rule.missing.return_value = [('flag', 'REQUIRED', True)]
        self.assertFalse(agent.acquisition_capacity_ready(facts, 'Eevee', method))

    def test_uncommitted_solo_choices_offer_every_ceiling_preserving_branch(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        gifts = [
            Rule(name, 'Lab', f'Lab:{name}', [], [], [], ('pokemon', name, 5), [])
            for name in ('Bulbasaur', 'Charmander', 'Squirtle')
        ]
        agent.index = Mock(rules=gifts, by_effect={})
        agent._complete_collection_graph = {
            name: [{'method': 'gift', 'map': 'Lab', 'storyline': name, 'level': 5,
                    'exclusive_group': 'starter', 'choice': name}]
            for name in ('Bulbasaur', 'Charmander', 'Squirtle')
        }
        facts = {'party': [], 'stored_pokemon': [], 'bag': {}, 'flags': {},
                 'coins': 0, 'money': 0, 'map': 'Lab', 'dex': {'owned_species': []}}
        groups = {}
        agent.add_nonwild_collection_groups(groups, facts)
        self.assertEqual({group['context']['species'] for group in groups.values()},
                         {'Bulbasaur', 'Charmander', 'Squirtle'})

    def test_boxed_evolution_source_backchains_through_real_pc_rule(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        pc = Rule('pc', 'Center', 'Center:pcStorage', ['sign:1'], [], [],
                  ('pc', 'storage', True), [])
        agent.index = Mock(rules=[pc], by_effect={('pc', 'storage', True): [pc]})
        agent._complete_collection_graph = {
            'Charmander': [{'method': 'gift', 'map': 'Lab', 'storyline': 'starter'}],
            'Charmeleon': [{'method': 'evolution', 'from_species': 'Charmander',
                            'trigger': 'level', 'level': 16}],
        }
        facts = {'party': [{'species': 'Pidgey', 'level': 8}],
                 'stored_pokemon': [{'box': 2, 'index': 4, 'species': 'Charmander', 'level': 15}],
                 'bag': {}, 'flags': {}, 'coins': 0, 'money': 0, 'map': 'Route1',
                 'dex': {'owned_species': ['Charmander']}}
        groups = {}
        agent.add_nonwild_collection_groups(groups, facts)
        self.assertEqual(groups['retrieve:Charmander']['target'], ('pokemon', 'Charmander', None))
        self.assertEqual(groups['retrieve:Charmander']['context']['stored_pokemon']['box'], 2)

    def field_carrier_agent(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        pc = Rule('pc', 'FuchsiaPokecenter', 'FuchsiaPokecenter:pc',
                  ['sign:1'], [], [], ('pc', 'storage', True), [])
        agent.index = Mock(by_effect={('pc', 'storage', True): [pc]})
        return agent, pc

    def field_carrier_facts(self):
        return {
            'party': [{'species': species, 'moves': [], 'level': 20}
                      for species in ('Charizard', 'Wigglytuff', 'Exeggcute',
                                      'Parasect', 'Gloom', 'Hypno')],
            'stored_pokemon': [{'box': 0, 'index': 14, 'species': 'Snorlax',
                'level': 30, 'hp': 139, 'max_hp': 139, 'status': 'Sleep(1)',
                'moves': ['Headbutt', 'Amnesia', 'Rest', 'None'], 'pp': [15, 20, 10, 0]}],
            'bag': {'HM03': 1}, 'flags': {'EVENT_BEAT_KOGA': True}}

    def test_full_party_backchains_surf_through_owned_stored_carrier(self):
        agent, pc = self.field_carrier_agent()
        facts = self.field_carrier_facts()
        obstacle = {'move': 'Surf', 'map': 'Route10', 'stance': [15, 4],
                    'direction': 'right', 'landing': ['Route10', 15, 44],
                    'destination': 'PowerPlant'}
        groups = {}
        agent.add_stored_field_carrier_retrieval(groups, facts, 'Surf', obstacle)
        group = groups['retrieve:Snorlax']
        self.assertEqual(group['rules'], [pc])
        self.assertEqual(group['target'], ('pokemon', 'Snorlax', None))
        context = group['context']
        self.assertTrue(context['storage_retrieval'])
        self.assertTrue(context['requires_party_deposit'])
        self.assertTrue(context['stored_pokemon_not_fully_healthy'])
        self.assertEqual(context['stored_pokemon']['index'], 14)
        requirement = context['field_move_requirements'][0]
        self.assertEqual(requirement['required_move'], 'Surf')
        self.assertTrue(requirement['compatible'])
        self.assertFalse(requirement['already_knows_move'])
        self.assertTrue(requirement['machine_held'])
        self.assertEqual(requirement['unmet_native_field_prerequisites'], [])
        self.assertEqual(requirement['terrain_obstruction'], obstacle)
        self.assertIn('not a completed crossing', requirement['scope'])
        self.assertEqual(len(facts['party']), 6)  # Planning is not PC execution.

    def test_field_carrier_preview_keeps_missing_machine_and_badge_explicit(self):
        agent, _ = self.field_carrier_agent()
        facts = self.field_carrier_facts()
        facts['bag'], facts['flags'] = {}, {}
        groups = {}
        agent.add_stored_field_carrier_retrieval(groups, facts, 'Surf', {'move': 'Surf'})
        requirement = groups['retrieve:Snorlax']['context']['field_move_requirements'][0]
        self.assertFalse(requirement['machine_held'])
        self.assertEqual(requirement['unmet_native_field_prerequisites'],
                         [('flag', 'EVENT_BEAT_KOGA', True)])

    def test_field_carrier_can_already_know_move_without_requiring_machine(self):
        agent, _ = self.field_carrier_agent()
        facts = self.field_carrier_facts()
        facts['bag'] = {}
        facts['stored_pokemon'][0]['moves'].append('Surf')
        groups = {}
        agent.add_stored_field_carrier_retrieval(groups, facts, 'Surf', {'move': 'Surf'})
        requirement = groups['retrieve:Snorlax']['context']['field_move_requirements'][0]
        self.assertTrue(requirement['already_knows_move'])
        self.assertFalse(requirement['machine_held'])

    def test_stored_field_carrier_merges_existing_followups_and_deduplicates_moves(self):
        agent, _ = self.field_carrier_agent()
        facts = self.field_carrier_facts()
        stored = facts['stored_pokemon'][0]
        stored['species'] = 'Tentacool'
        method = {'method': 'evolution', 'from_species': 'Tentacool',
                  'trigger': 'level', 'level': 30}
        groups = {}
        agent.add_storage_retrieval(groups, facts, 'Tentacool', 'Tentacruel', method)
        preview = groups['retrieve:Tentacool']['context']['post_withdrawal_acquisitions'][0]
        for _ in range(2):
            agent.add_stored_field_carrier_retrieval(groups, facts, 'Surf', {'move': 'Surf'})
        context = groups['retrieve:Tentacool']['context']
        self.assertEqual(len(groups), 1)
        self.assertEqual(len(context['field_move_requirements']), 1)
        self.assertEqual(context['post_withdrawal_acquisitions'], [preview])
        self.assertIn('Tentacruel', context['required_for'])

    def test_stored_field_carrier_is_not_needed_when_party_can_learn_or_use_move(self):
        agent, _ = self.field_carrier_agent()
        for learned in (False, True):
            with self.subTest(learned=learned):
                facts = self.field_carrier_facts()
                facts['party'][1] = {'species': 'Snorlax', 'moves': ['Surf'] if learned else []}
                groups = {}
                agent.add_stored_field_carrier_retrieval(groups, facts, 'Surf', {'move': 'Surf'})
                self.assertEqual(groups, {})

    def test_stored_field_carrier_requires_actual_compatible_slot_and_available_pc_rule(self):
        agent, _ = self.field_carrier_agent()
        for missing in ('carrier', 'pc'):
            with self.subTest(missing=missing):
                facts = self.field_carrier_facts()
                if missing == 'carrier':
                    facts['stored_pokemon'][0]['species'] = 'Pidgey'
                else:
                    agent.index.by_effect = {}
                groups = {}
                agent.add_stored_field_carrier_retrieval(groups, facts, 'Surf', {'move': 'Surf'})
                self.assertEqual(groups, {})

    def test_storage_preview_accounts_for_real_level_trigger_and_level_cap(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        pc = Rule('pc', 'Center', 'Center:pc', ['sign:1'], [], [], ('pc', 'storage', True), [])
        agent.index = Mock(by_effect={('pc', 'storage', True): [pc]})
        method = {'method': 'evolution', 'from_species': 'Tentacool', 'trigger': 'level', 'level': 30}
        for current, trigger in ((29, 30), (40, 41), (100, None)):
            stored = {'box': 2, 'index': 4, 'species': 'Tentacool', 'level': current}
            facts = {'party': [], 'stored_pokemon': [stored], 'bag': {}}
            groups = {}
            agent.add_storage_retrieval(groups, facts, 'Tentacool', 'Tentacruel', method)
            context = groups['retrieve:Tentacool']['context']
            preview = context['post_withdrawal_acquisitions'][0]
            self.assertFalse(preview['withdrawal_registers_target'])
            self.assertEqual(preview['experience_trigger_level'], trigger)
            self.assertEqual(preview['level_up_possible'], current < 100)
            if trigger:
                self.assertEqual(preview['training_cost']['levels_remaining'], 1)
                self.assertGreater(preview['training_cost']['remaining_experience_min'], 0)
            else:
                self.assertNotIn('training_cost', preview)
            self.assertEqual(context['stored_pokemon'], stored)
            self.assertEqual(facts['party'], [])

    def test_storage_preview_distinguishes_held_stone_from_reference_purchase_cost(self):
        from openpokered.autonomous_story import ITEM_CATALOG
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        pc = Rule('pc', 'Center', 'Center:pc', ['sign:1'], [], [], ('pc', 'storage', True), [])
        agent.index = Mock(by_effect={('pc', 'storage', True): [pc]})
        method = {'method': 'evolution', 'from_species': 'Growlithe', 'trigger': 'item', 'item': 'FireStone'}
        stored = {'box': 1, 'index': 3, 'species': 'Growlithe', 'level': 32,
                  'hp': 10, 'max_hp': 80, 'status': 'Sleep(1)'}
        for quantity in (0, 1):
            facts = {'party': [{}] * 6, 'stored_pokemon': [stored], 'bag': {'FIRESTONE': quantity}}
            groups = {}
            agent.add_storage_retrieval(groups, facts, 'Growlithe', 'Arcanine', method)
            context = groups['retrieve:Growlithe']['context']
            preview = context['post_withdrawal_acquisitions'][0]
            self.assertEqual(preview['required_item'], 'FireStone')
            self.assertEqual(preview['item_quantity_held'], quantity)
            self.assertEqual(preview['item_missing'], not quantity)
            self.assertEqual(preview['item_unit_price_reference'], ITEM_CATALOG['FireStone']['price'])
            self.assertEqual(preview['acquisition_contract']['direct_cost']['consumed_items'], {'FireStone': 1})
            self.assertTrue(context['requires_party_deposit'])
            self.assertTrue(context['stored_pokemon_not_fully_healthy'])
            self.assertEqual(len(groups), 1)  # Evidence does not force a shop or route.

    def test_storage_preview_keeps_trade_and_evolution_as_alternatives_for_one_individual(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        pc = Rule('pc', 'Center', 'Center:pc', ['sign:1'], [], [], ('pc', 'storage', True), [])
        agent.index = Mock(by_effect={('pc', 'storage', True): [pc]})
        evolution = {'method': 'evolution', 'from_species': 'Abra', 'trigger': 'level', 'level': 16}
        trade = {'method': 'npc_trade', 'from_species': 'Abra', 'map': 'Route2TradeHouse',
                 'completion_flag': 'TRADE_COMPLETE'}
        stored = {'box': 0, 'index': 10, 'species': 'Abra', 'level': 10}
        facts = {'party': [{'species': 'Charizard'}], 'stored_pokemon': [stored],
                 'bag': {}, 'flags': {'TRADE_COMPLETE': False}}
        groups = {}
        for target, method in [('Kadabra', evolution), ('MrMime', trade), ('MrMime', trade)]:
            agent.add_storage_retrieval(groups, facts, 'Abra', target, method)
        context = groups['retrieve:Abra']['context']
        previews = context['post_withdrawal_acquisitions']
        self.assertEqual(len(previews), 2)
        self.assertEqual(context['required_for'], ['Kadabra', 'MrMime'])
        self.assertTrue(context['post_withdrawal_options_share_one_individual'])
        self.assertEqual(previews[0]['training_cost']['levels_remaining'], 6)
        self.assertEqual(previews[1]['acquisition_contract']['direct_cost']['relinquished_species'], ['Abra'])
        self.assertFalse(previews[1]['trade_already_completed'])
        self.assertTrue(previews[1]['party_count_requirement_after_withdrawal_met'])
        self.assertEqual(len(groups), 1)

    def test_story_semantics_compile_pc_and_coin_sources(self):
        from openpokered.story_rules import compile_story
        story = {'id': 'Room:test', 'map': 'Room', 'triggers': ['sign:1'], 'program': [
            {'Command': {'name': 'openPC', 'args': []}},
            {'Command': {'name': 'giveCoins', 'args': [{'NumberLit': 50}]}}
        ]}
        effects = {rule.effect for rule in compile_story(story)}
        self.assertIn(('pc', 'storage', True), effects)
        self.assertIn(('coins', 50, True), effects)

    def test_fishing_profiles_include_no_bite_and_uniform_group_odds(self):
        from openpokered.collection_planner import fishing_profile
        old = fishing_profile('OldRod', 'PalletTown')
        self.assertEqual((old['bite_probability_pct'], old['targets'][0]['per_attempt_pct']),
                         (100.0, 100.0))
        super_rod = fishing_profile('SuperRod', 'SafariZoneCenter')
        self.assertEqual(super_rod['bite_probability_pct'], 50.0)
        self.assertEqual({row['per_attempt_pct'] for row in super_rod['targets']}, {12.5})

    def test_collection_resources_expose_ball_quality_and_safe_status_support(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        resources = agent.collection_resources({
            'bag': {'POKEBALL': 3, 'ULTRABALL': 2, 'POTION': 4},
            'party': [{'species': 'Butterfree', 'moves': ['SleepPowder', 'Poisonpowder'],
                       'pp': [7, 12]}],
        })
        self.assertEqual(resources['total_balls'], 5)
        self.assertEqual(resources['ball_inventory'], [
            {'ball': 'PokeBall', 'quantity': 3, 'quality': 'basic'},
            {'ball': 'UltraBall', 'quantity': 2, 'quality': 'strong'},
        ])
        self.assertTrue(resources['can_apply_safe_capture_status'])
        self.assertEqual([(row['move'], row['capture_bonus'], row['residual_damage_risk'])
                          for row in resources['capture_status_moves']],
                         [('SleepPowder', 'strong', False),
                          ('Poisonpowder', 'moderate', True)])

    def test_catch_target_offers_encounter_terrain_without_training_sites(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        rule = Rule('catch:Route24', 'Route24', 'skill:catch_encounter', [], [], [],
                    ('catch', 'Route24', True), [])
        agent.active = {'target': rule.effect, 'rules': [rule]}
        agent.catch_areas = {'Route24': {'species': ['Abra'], 'spots': [(5, 18)], 'reachable': True}}
        agent.catch_navigation = {'Route24': {'map': 'Route24', 'tile_route_found': True, 'steps': 2,
                                             'scope': 'path to actual encounter terrain'}}
        agent.maps = {'Route24': {'wild': {'red': {'grass': {'mons': [{'level': 8, 'species': 'Abra'}]}}}}}
        candidates, bindings = agent.action_candidates({})
        self.assertEqual(json.loads(candidates['action:0'])['operation'], 'catch_encounter:Route24,5,18')
        self.assertEqual(json.loads(candidates['action:0'])['unregistered_species_here'], ['Abra'])
        self.assertEqual(bindings['action:0'], ('catch_encounter:Route24,5,18', rule))
        # The level fallthrough would raise here; a catch target has no training site.
        self.assertFalse(hasattr(agent, 'training_sites'))

    def test_stochastic_hunts_are_observed_bounded_attempts_not_registration(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        target = ('catch', 'safari:Park', True)
        rule = Rule('hunt', 'Park', 'skill:catch_encounter', [], [], [], target, [])
        self.assertFalse(agent.completed_stochastic_attempt('catch_encounter:safari,Park,5,8', rule,
            {'result': 'hunted'}, 0))
        self.assertFalse(agent.completed_stochastic_attempt('travel_to:Park', rule,
            {'result': 'hunted'}, 1))
        self.assertFalse(agent.completed_stochastic_attempt('catch_encounter:safari,Park,5,8', rule,
            {'result': 'blocked'}, 1))
        agent.active = {'target': target, 'rules': [rule]}
        agent.replan_after_defeat = False
        agent.capture_resources_missing = Mock(return_value=False)
        agent.capture_area_blocked = Mock(return_value=False)
        agent.needs_skill_recovery = Mock(return_value=False)
        for _ in range(agent.CATCH_WINDOW - 1):
            self.assertTrue(agent.completed_stochastic_attempt('catch_encounter:safari,Park,5,8', rule,
                {'result': 'hunted', 'owned_before': 76, 'owned_after': 76}, 1))
        self.assertFalse(agent.should_replan({'party': []}))
        agent.completed_stochastic_attempt('catch_encounter:safari,Park,5,8', rule,
            {'result': 'hunted', 'owned_before': 76, 'owned_after': 76}, 1)
        self.assertTrue(agent.should_replan({'party': []}))
        with patch('openpokered.story_agent.DualStoryAgent.select_strategy'):
            agent.select_strategy({'party': [], 'bag': {}})
        self.assertEqual(agent._completed_hunts_since_strategy, 0)

    def test_catch_encounter_reports_the_dex_delta_of_whatever_battle_started(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        rule = Rule('catch:Route2', 'Route2', 'skill:catch_encounter', [], [], [],
                    ('catch', 'Route2', True), [])
        agent.active = {'target': rule.effect, 'rules': [rule]}
        agent.facts = Mock(return_value={'bag': {'POKEBALL': 1}})
        agent.actions, agent.max_actions = 0, 100
        before = {'map_name': 'Route2', 'player_x': 5, 'player_y': 18, 'screen': 'overworld',
                  'pokedex': {'owned': 2}, 'party': [{'level': 12}]}
        battle = {**before, 'screen': 'battle', 'pokedex': {'owned': 3}}
        agent.client = Mock()
        agent.client.state.side_effect = [before, before, before] + [battle] * 8
        agent.client.cmd.return_value = []
        agent.client.move_to.return_value = {'result': 'reached'}
        agent.game = Mock()
        agent.game.st.return_value = {'player_x': 5, 'player_y': 18}
        agent.navigate_point = Mock(return_value=(5, 18))
        agent.check_budget, agent.settle, agent.record = Mock(), Mock(), Mock()
        agent.record_travel = Mock()
        with patch('openpokered.autonomous_story.reachable_grass', return_value=(5, 18)):
            result = agent.execute('catch_encounter:Route2,5,18', rule)
        self.assertEqual(result, {'result': 'hunted', 'map': 'Route2', 'owned_before': 2, 'owned_after': 3})
        self.assertEqual(agent.record.call_args.kwargs['operation'], 'catch_encounter:Route2,5,18')
        self.assertEqual(agent.record.call_args.kwargs['result'], result)
        self.assertEqual(agent.catch_attempts, [{'map': 'Route2', 'registered': True}])

    def test_encounter_approach_uses_normal_navigation_and_preserves_battle_or_blocker_interruptions(self):
        import playthrough as pt
        from openpokered.playthrough_judgments import NavigationPause
        for error, expected in ((NavigationPause('trainer interrupted'), 'paused_after_battle'),
                                (pt.NavError('observed obstruction'), 'blocked')):
            with self.subTest(interruption=expected):
                agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
                rule = Rule('catch', 'Route24', 'skill:catch_encounter', [], [], [],
                            ('catch', 'Route24', True), [])
                agent.active = {'target': rule.effect, 'rules': [rule]}
                agent.actions, agent.max_actions = 0, 30
                state = {'map_name': 'Route24', 'player_x': 10, 'player_y': 15,
                         'screen': 'overworld', 'party': [{'level': 40}],
                         'pokedex': {'owned': 17}}
                agent.client, agent.game = Mock(), Mock()
                agent.client.state.return_value = state
                agent.client.cmd.return_value = []
                agent.client.move_to.return_value = {'result': 'interrupted'}
                agent.game.st.return_value = state
                agent.facts = Mock(return_value={'bag': {'POKEBALL': 10}})
                agent.check_budget = Mock()
                agent.needs_capture_recovery = Mock(return_value=False)
                agent.navigate_point = Mock(side_effect=error)
                agent.settle, agent.record = Mock(), Mock()
                agent.remember_travel_result = Mock()
                with patch('openpokered.autonomous_story.reachable_grass', return_value=(4, 18)):
                    result = agent.execute('catch_encounter:Route24,4,18', rule)
                self.assertEqual(result['result'], expected)
                self.assertEqual(result['destination'], 'Route24')
                agent.navigate_point.assert_called_once_with('Route24', (4, 18), tries=50)
                agent.client.move_to.assert_not_called()
                agent.game.d.drive.assert_not_called()
                self.assertFalse(hasattr(agent, 'catch_attempts'))
                if expected == 'blocked':
                    agent.remember_travel_result.assert_called_once_with('Route24', result)

    def travel_goal_agent(self, legs, origin='ViridianCity'):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = None
        agent.game = Mock(last_map=origin)
        agent.game.st.return_value = {'map_name': origin, 'player_x': 1, 'player_y': 1}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': legs}
        agent.navigate_point = Mock()
        return agent

    def test_catch_trip_blocks_grass_on_the_maps_it_only_crosses(self):
        import playthrough as pt
        agent = self.travel_goal_agent([{'to_map': 'Route2'}, {'to_map': 'Route22'}])
        rule = Rule('catch:Route22', 'Route22', 'skill:catch_encounter', [], [], [],
                    ('catch', 'Route22', True), [])
        searches = []
        def search(source, start, goal_map, goal, **kwargs):
            searches.append(kwargs['blocked_maps'])
            return [('ViridianCity', 1, 1), ('Route22', *goal)] if len(searches) > 1 else None
        grass = {'ViridianCity': {(0, 0)}, 'Route2': {(5, 5)}, 'Route22': {(9, 9)}}
        with patch.object(pt, 'bfs_cross', side_effect=search), \
                patch.object(pt, 'grass_tiles', side_effect=lambda name: grass[name]) as tiles:
            result = agent.travel('Route22', rule, [(3, 3)], avoid_encounters=True)
        self.assertEqual(result['result'], 'reached')
        self.assertEqual(searches[0], {'ViridianCity': {(0, 0)}, 'Route2': {(5, 5)}})
        # Extra blocked tiles can make a route infeasible: the trip still plans.
        self.assertEqual(searches[1], {})
        # The destination's own grass is the point of the trip; never queried.
        self.assertEqual([row.args[0] for row in tiles.call_args_list], ['Route2', 'ViridianCity'])
        # The walk re-plans from every observation, so it is handed the transit
        # maps as navigation barriers — without the map it starts on.
        self.assertEqual(agent.navigate_point.call_args.args, ('Route22', (3, 3)))
        self.assertEqual(agent.navigate_point.call_args.kwargs['avoid_maps'], {'Route2': {(5, 5)}})

    def test_ordinary_travel_never_blocks_encounter_terrain(self):
        import playthrough as pt
        agent = self.travel_goal_agent([{'to_map': 'Route22'}])
        rule = Rule('stop', 'Route22', 'Route22:trigger', [], [], [], ('flag', 'DONE', True), [])
        with patch.object(pt, 'bfs_cross', return_value=[('ViridianCity', 1, 1), ('Route22', 2, 2)]) as search, \
                patch.object(pt, 'grass_tiles') as tiles:
            self.assertEqual(agent.travel('Route22', rule, [(2, 2)])['result'], 'reached')
        self.assertEqual(search.call_count, 1)
        self.assertEqual(search.call_args.kwargs['blocked_maps'], {})
        tiles.assert_not_called()
        agent.navigate_point.assert_called_once_with('Route22', (2, 2))

    def test_only_a_catch_trip_avoids_encounters_on_the_way(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.actions, agent.max_actions = 0, 100
        agent.facts = Mock(return_value={'bag': {'POKEBALL': 1}})
        agent.game = Mock()
        agent.game.st.return_value = {'map_name': 'Route2', 'player_x': 5, 'player_y': 18}
        agent.client = Mock()
        agent.client.state.return_value = {'map_name': 'PewterCity', 'party': [{'level': 12}],
                                           'pokedex': {'owned': 2}}
        agent.travel = Mock(return_value={'result': 'blocked'})
        agent.record, agent.record_travel, agent.settle, agent.remember_travel_result = (
            Mock(), Mock(), Mock(), Mock())
        rule = Rule('trip:Route2', 'Route2', 'skill:catch_encounter', [], [], [],
                    ('catch', 'Route2', True), [])
        agent.active = {'target': ('level', 'leader', 14), 'rules': [rule]}
        agent.execute('train_encounter:Route2,5,18', rule)
        self.assertFalse(agent.travel.call_args.kwargs['avoid_encounters'])
        agent.active = {'target': ('catch', 'Route2', True), 'rules': [rule]}
        agent.execute('catch_encounter:Route2,5,18', rule)
        self.assertTrue(agent.travel.call_args.kwargs['avoid_encounters'])

    def test_catch_area_context_states_quality_registration_and_attempts(self):
        from openpokered.story_agent import DualStoryAgent
        agent = self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True}])
        agent.find_catch_areas = Mock(return_value={'Route2': {'species': ['Chansey', 'Zubat'],
                                                              'map': 'Route2', 'method': 'grass',
                                                              'spots': [(5, 18)], 'reachable': True}})
        agent.catch_attempts = [{'map': 'Route2', 'registered': False},
                                {'map': 'Route2', 'registered': True},
                                {'map': 'Route3', 'registered': True}]
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [8, 25]}
        facts = {'party': [mon], 'bag': {'POKEBALL': 5}, 'flags': {}, 'fully_recovered': False,
                 'map': 'PewterCity', 'x': 12, 'y': 18,
                 'dex': {'owned_species': ['Pidgey', 'Rattata']}}
        with patch.object(DualStoryAgent, 'strategy_groups', side_effect=lambda facts: {}):
            context = agent.strategy_groups(facts)['collect:Route2']['context']
        self.assertEqual(context['unregistered_species'],
                         [{'species': 'Chansey', 'catch_rate': 30, 'band': 'hard'},
                          {'species': 'Zubat', 'catch_rate': 255, 'band': 'easy'}])
        # A table that is mostly duplicates is visible rather than inferred.
        self.assertEqual(context['already_registered_here'], ['Pidgey', 'Rattata'])
        self.assertEqual(context['recent_attempts'], {'hunts': 2, 'registered': 1})
        self.assertEqual(context['balls_held'], 5)
        self.assertEqual(context['prerequisite'], 'Catching spends balls; 5 carried')
        self.assertIn('expected_steps_to_any_new_species', context['encounter_value'])
        self.assertEqual(context['collection_resources']['ball_inventory'],
                         [{'ball': 'PokeBall', 'quantity': 5, 'quality': 'basic'}])
        self.assertIn('Chansey', context['species_scarcity'])
        self.assertEqual(context['encounters'],
                         ((agent.maps['Route2'].get('wild') or {}).get('red') or {}).get('grass'))

    def test_dex_panel_reports_progress_rungs_and_remaining_areas(self):
        agent = self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True}])
        def panel(owned, seen=()):
            return agent.dex_progress({'bag': {'POKEBALL': 3, 'POTION': 1},
                                       'dex': {'owned': len(owned), 'seen': len(seen),
                                               'owned_species': list(owned), 'seen_species': list(seen)}})
        start = panel(set())
        self.assertEqual((start['owned'], start['seen'], start['total']), (0, 0, 151))
        self.assertEqual(start['next_rung'], {'rung': 2, 'needs': 2, 'remaining': 2})
        self.assertEqual(panel({'Pidgey', 'Rattata'})['next_rung'],
                         {'rung': 10, 'needs': 10, 'remaining': 8})
        from openpokered.collection_planner import solo_plan
        reachable = set(solo_plan(agent.complete_collection_graph())['reachable_species'])
        self.assertIsNone(panel(reachable)['next_rung'])
        # Species already met are known-reachable: the strongest collection lead.
        self.assertEqual(panel({'Pidgey'}, {'Zubat', 'Pidgey', 'Rattata'})['seen_not_owned'],
                         ['Rattata', 'Zubat'])
        self.assertEqual(panel({'Pidgey'})['unregistered_by_area'], {'Route2': 2, 'Route3': 2})
        self.assertEqual(panel({'Pidgey'})['expected_yield_by_area']['Route2']
                     ['unregistered_encounter_share_pct'], 55.1)
        self.assertEqual(panel({'Pidgey'})['balls_held'], 3)

    def test_dex_progress_rung_stops_at_verified_solo_ceiling(self):
        from openpokered.collection_planner import solo_plan
        agent = self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True}])
        reachable = solo_plan(agent.complete_collection_graph())['reachable_species']
        self.assertEqual(len(reachable), 124)
        for count in (50, 74, 123, 124):
            facts = {'bag': {}, 'dex': {'owned_species': reachable[:count]}}
            panel = agent.dex_progress(facts)
            self.assertEqual(panel['next_rung'], None if count == 124 else
                             {'rung': 124, 'needs': 124, 'remaining': 124 - count})
        # A native registration still awaiting source proof cannot finish it.
        agent.collection_audit_pending = {reachable[-1]: {'reason': 'test'}}
        panel = agent.dex_progress({'bag': {}, 'dex': {'owned_species': reachable}})
        self.assertEqual(panel['next_rung'], {'rung': 124, 'needs': 124, 'remaining': 1})

    def test_map_hops_counts_map_crossings_on_real_map_data(self):
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        self.assertEqual(agent.map_hops('MtMoon1F', 'ViridianMart'), 6)
        self.assertEqual(agent.map_hops('Route22', 'ViridianMart'), 2)
        # The diagnosis quoted two hops here, but the map graph cannot be
        # shorter: CeruleanMart opens only off CeruleanCity, and MtMoon1F's
        # street neighbours are Route4 and MtMoonB1F, so three is the minimum.
        self.assertEqual(agent.map_hops('MtMoon1F', 'CeruleanMart'), 3)
        # The cable-club rooms sit outside the walking world entirely.
        self.assertIsNone(agent.map_hops('PalletTown', 'Colosseum'))

    def test_dex_panel_points_at_the_nearest_reachable_ball_shop(self):
        agent = self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True}])
        viridian = Rule('shop:ViridianMart', 'ViridianMart', 'ViridianMart:shop', [], [], [],
                        ('shop', ('POTION', 'POKE_BALL'), True), [])
        cerulean = Rule('shop:CeruleanMart', 'CeruleanMart', 'CeruleanMart:shop', [], [], [],
                        ('shop', ('POKE_BALL', 'GREAT_BALL'), True), [])
        agent.index = Mock(rules=[viridian, cerulean])
        def panel(map_name):
            return agent.dex_progress({'map': map_name, 'bag': {},
                                       'dex': {'owned': 0, 'seen': 0,
                                               'owned_species': [], 'seen_species': []}})
        self.assertEqual(panel('MtMoon1F')['nearest_ball_source'],
                         {'map': 'CeruleanMart', 'hops': 3, 'stock': ['GreatBall', 'PokeBall']})
        self.assertEqual(panel('Route22')['nearest_ball_source'],
                         {'map': 'ViridianMart', 'hops': 2, 'stock': ['PokeBall']})
        agent.index = Mock(rules=[])
        self.assertIsNone(panel('MtMoon1F')['nearest_ball_source'])

    def test_dex_panel_is_only_assembled_for_the_collecting_goal(self):
        facts = {'bag': {}, 'dex': {'owned': 0, 'seen': 0, 'owned_species': [], 'seen_species': []}}
        collected = {}
        self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True}]).augment_strategy_state(
            collected, facts)
        self.assertIn('dex_progress', collected)
        story = {}
        self.catch_goal_agent([{'id': 'beat-brock', 'satisfied_when': {'flag': 'EVENT_BEAT_BROCK'}}]
                              ).augment_strategy_state(story, facts)
        self.assertEqual(story, {})

    def test_strategy_assembly_asks_for_the_panel_before_judging(self):
        from openpokered.story_agent import DualStoryAgent
        agent = self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True,
                                        'name': 'Register every wild species'}])
        agent.client.route.return_value = {'found': True, 'legs': []}
        agent.index.wild_species.return_value = {'Zubat'}  # collection still in progress
        rule = Rule('catch:Route2', 'Route2', 'skill:catch_encounter', [], [], [],
                    ('catch', 'Route2', True), [])
        group = {'target': ('catch', 'Route2', True), 'rules': [rule],
                 'objectives': ['Register wild species that are not in the Pokédex yet'], 'context': {}}
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [8, 25]}
        facts = {'party': [mon], 'bag': {}, 'flags': {}, 'badges': 0, 'fully_recovered': False,
                 'map': 'PewterCity', 'x': 12, 'y': 18,
                 'dex': {'owned': 1, 'seen': 3, 'owned_species': ['Pidgey'],
                         'seen_species': ['Pidgey', 'Rattata', 'Zubat']}}
        agent.augment_strategy_state = Mock(wraps=agent.augment_strategy_state)
        agent.choose = Mock(return_value='subgoal:0')
        with patch.object(DualStoryAgent, 'strategy_groups', return_value={'collect:Route2': group}):
            agent.select_strategy(facts)
        state, passed = agent.augment_strategy_state.call_args.args
        self.assertIs(state, agent.choose.call_args.args[1])
        self.assertEqual(passed, facts)
        self.assertEqual(state['dex_progress']['seen_not_owned'], ['Rattata', 'Zubat'])

    def ball_supply_agent(self, collecting=True):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = collecting
        rule = Rule('shop:ViridianMart', 'ViridianMart', 'ViridianMart:shop', [], [], [],
                    ('shop', ('POTION', 'POKE_BALL'), True), [])
        agent.index = Mock(rules=[rule])
        agent._complete_collection_graph = {}
        agent.visited = {'ViridianMart'}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'ViridianMart'}]}
        return agent

    def test_ball_supply_is_bought_through_the_shop_path_only_when_collecting(self):
        facts = {'bag': {}, 'money': 3000, 'map': 'ViridianCity'}
        groups = {}
        self.ball_supply_agent(collecting=False).add_ball_supply(groups, facts)
        self.assertEqual(groups, {})
        self.ball_supply_agent().add_ball_supply(groups, facts)
        self.assertEqual(list(groups), ['ball:shop:ViridianMart:PokeBall'])
        group = groups['ball:shop:ViridianMart:PokeBall']
        self.assertEqual(group['target'], ('supply', 'PokeBall', 9))
        self.assertEqual(group['context']['stock_index'], 1)

    def test_ball_supply_stops_once_the_reserve_is_held(self):
        groups = {}
        self.ball_supply_agent().add_ball_supply(
            groups, {'bag': {'POKEBALL': 12}, 'money': 3000, 'map': 'ViridianCity'})
        self.assertEqual(groups, {})

    def bulk_ball_agent(self):
        agent = self.ball_supply_agent()
        method = {'method': 'static', 'map': 'VictoryRoad2F', 'storyline': 'talkMoltres'}
        agent._complete_collection_graph = {'Moltres': [method]}
        agent.index.rules.append(Rule('moltres', method['map'], 'VictoryRoad2F:talkMoltres',
            [], [], [], ('battle', 'MOLTRES', True), []))
        return agent

    def test_low_catch_rate_target_can_offer_more_than_the_ordinary_reserve(self):
        agent = self.bulk_ball_agent()
        facts = {'bag': {'POKEBALL': 12}, 'money': 100000, 'map': 'ViridianCity'}
        groups = {}
        agent.add_ball_supply(groups, facts)
        self.assertEqual({g['target'] for g in groups.values()},
                         {('supply', 'PokeBall', 36), ('supply', 'PokeBall', 99)})
        for group in groups.values():
            context = group['context']
            quantity = group['target'][2] - 12
            self.assertEqual(context['purchase_quantity'], quantity)
            self.assertEqual(context['total_cost'], quantity * 200)
            self.assertEqual(context['money_after_purchase'], 100000 - quantity * 200)
            target = context['capture_supply_reference']['targets'][0]
            self.assertEqual(target['species'], 'Moltres')
            for scenario in target['scenarios']:
                self.assertLess(scenario['failure_after_purchase'], scenario['failure_before_purchase'])
            self.assertIn('not a guarantee', context['capture_supply_reference']['scope'])
        self.assertEqual(facts['bag'], {'POKEBALL': 12})

    def test_bulk_ball_budget_deduplicates_affordable_limits_and_respects_bag_space(self):
        agent = self.bulk_ball_agent()
        groups = {}
        agent.add_ball_supply(groups, {'bag': {}, 'money': 3000, 'map': 'ViridianCity'})
        self.assertEqual([g['target'] for g in groups.values()], [('supply', 'PokeBall', 9)])
        bag = {f'ITEM{i}': 1 for i in range(20)}
        facts = {'bag': bag, 'money': 100000, 'map': 'ViridianCity'}
        groups = {}
        agent.add_ball_supply(groups, facts)
        self.assertEqual(groups, {})
        del bag['ITEM0']
        bag['POKEBALL'] = 98
        agent.add_ball_supply(groups, facts)
        self.assertEqual([g['target'] for g in groups.values()], [('supply', 'PokeBall', 99)])
        bag['POKEBALL'] = 99
        groups = {}
        agent.add_ball_supply(groups, facts)
        self.assertEqual(groups, {})

    def stone_spending_agent(self):
        agent = self.bulk_ball_agent()
        agent._complete_collection_graph.update({
            'Growlithe': [{'method': 'grass', 'map': 'Route7'}],
            'Arcanine': [{'method': 'evolution', 'trigger': 'item',
                         'from_species': 'Growlithe', 'item': 'FireStone'}],
            'Pikachu': [{'method': 'grass', 'map': 'ViridianForest'}],
            'Raichu': [{'method': 'evolution', 'trigger': 'item',
                       'from_species': 'Pikachu', 'item': 'ThunderStone'}],
        })
        return agent

    def test_ball_spending_exposes_lost_stone_affordability_without_removing_batches(self):
        agent = self.stone_spending_agent()
        facts = {'bag': {}, 'money': 5000, 'map': 'ViridianCity',
                 'party': [{'species': 'Growlithe'}], 'stored_pokemon': [],
                 'dex': {'owned_species': ['Growlithe']}}
        original = json.loads(json.dumps(facts))
        groups = {}
        agent.add_ball_supply(groups, facts)
        self.assertEqual({g['target'] for g in groups.values()},
                         {('supply', 'PokeBall', 12), ('supply', 'PokeBall', 15)})
        for group in groups.values():
            context = group['context']
            reference = context['item_evolution_spending_reference']
            self.assertEqual(reference['money_before_purchase'], 5000)
            self.assertEqual(reference['money_after_purchase'], context['money_after_purchase'])
            option, = reference['held_source_options']
            self.assertEqual(option['species'], 'Arcanine')
            self.assertEqual(option['source_party_count'], 1)
            self.assertEqual(option['source_stored_count'], 0)
            self.assertTrue(option['affordable_before_purchase'])
            self.assertEqual(option['affordable_after_purchase'], group['target'][2] == 12)
            self.assertEqual(option['purchase_removes_affordability'], group['target'][2] == 15)
            self.assertIn('not a joint registration yield', reference['scope'])
            self.assertIn('not proof of shop access', reference['scope'])
        self.assertEqual(facts, original)

    def test_stone_spending_does_not_count_registered_but_unheld_sources(self):
        agent = self.stone_spending_agent()
        facts = {'money': 5000, 'bag': {}, 'party': [], 'stored_pokemon': [],
                 'dex': {'owned_species': ['Growlithe', 'Pikachu']}}
        reference = agent.item_evolution_spending_reference(facts, 0)
        self.assertEqual(reference['held_source_options'], [])

    def test_stone_spending_includes_pc_sources_but_excludes_registered_targets(self):
        agent = self.stone_spending_agent()
        facts = {'money': 2100, 'bag': {}, 'party': [{'species': 'Growlithe'}],
                 'stored_pokemon': [{'species': 'Pikachu', 'box': 2, 'index': 0}],
                 'dex': {'owned_species': ['Growlithe', 'Pikachu', 'Arcanine']}}
        option, = agent.item_evolution_spending_reference(facts, 2099)['held_source_options']
        self.assertEqual(option['species'], 'Raichu')
        self.assertEqual(option['source_party_count'], 0)
        self.assertEqual(option['source_stored_count'], 1)
        self.assertEqual(option['cash_needed_for_one_evolution'], 2100)
        self.assertTrue(option['affordable_before_purchase'])
        self.assertFalse(option['affordable_after_purchase'])

    def test_stone_spending_uses_carried_stone_without_repurchase_or_cash_reserve(self):
        facts = {'money': 2100, 'bag': {'FIRE_STONE': 1},
                 'party': [{'species': 'Growlithe'}], 'stored_pokemon': []}
        option, = self.stone_spending_agent().item_evolution_spending_reference(
            facts, 0)['held_source_options']
        self.assertEqual(option['item_quantity_held'], 1)
        self.assertEqual(option['cash_needed_for_one_evolution'], 0)
        self.assertTrue(option['affordable_before_purchase'])
        self.assertTrue(option['affordable_after_purchase'])
        self.assertFalse(option['purchase_removes_affordability'])

    def test_stone_spending_unknown_purchase_price_is_not_free_or_unaffordable(self):
        facts = {'money': 2100, 'bag': {}, 'party': [{'species': 'Growlithe'}]}
        from openpokered.autonomous_story import ITEM_CATALOG
        for catalog_entry in ({}, {'price': None}, {'price': 0}, {'price': -1}):
            with self.subTest(catalog_entry=catalog_entry):
                with patch.dict(ITEM_CATALOG, {'FireStone': catalog_entry}):
                    option, = self.stone_spending_agent().item_evolution_spending_reference(
                        facts, 0)['held_source_options']
                self.assertIsNone(option['item_unit_price_reference'])
                self.assertIsNone(option['cash_needed_for_one_evolution'])
                self.assertIsNone(option['affordable_before_purchase'])
                self.assertIsNone(option['affordable_after_purchase'])
                self.assertFalse(option['purchase_removes_affordability'])

    def test_reserve_ball_batch_also_carries_stone_spending_evidence(self):
        agent = self.stone_spending_agent()
        agent._complete_collection_graph.pop('Moltres')
        facts = {'bag': {}, 'money': 2200, 'map': 'ViridianCity',
                 'party': [{'species': 'Growlithe'}]}
        groups = {}
        agent.add_ball_supply(groups, facts)
        group, = groups.values()
        self.assertEqual(group['context']['batch'], 'reserve')
        self.assertEqual(group['target'], ('supply', 'PokeBall', 6))
        option, = group['context']['item_evolution_spending_reference']['held_source_options']
        self.assertTrue(option['purchase_removes_affordability'])

    def test_stone_spending_alternatives_do_not_duplicate_a_shared_stone(self):
        agent = self.stone_spending_agent()
        agent._complete_collection_graph.update({
            'Vulpix': [{'method': 'grass', 'map': 'Route7'}],
            'Ninetales': [{'method': 'evolution', 'trigger': 'item',
                          'from_species': 'Vulpix', 'item': 'FireStone'}],
        })
        facts = {'money': 0, 'bag': {'FIRESTONE': 1},
                 'party': [{'species': 'Growlithe'}, {'species': 'Vulpix'}]}
        reference = agent.item_evolution_spending_reference(facts, 0)
        self.assertEqual({row['species'] for row in reference['held_source_options']},
                         {'Arcanine', 'Ninetales'})
        self.assertTrue(all(row['item_quantity_held'] == 1 for row in reference['held_source_options']))
        self.assertIn('carried stones and source individuals are shared', reference['scope'])
        self.assertNotIn('guaranteed_registrations', reference)

    def test_stone_spending_respects_an_observed_exclusive_evolution_choice(self):
        agent = self.stone_spending_agent()
        agent._complete_collection_graph.update({
            'Eevee': [{'method': 'gift', 'map': 'CeladonMansionRoofHouse'}],
            'Flareon': [{'method': 'evolution', 'trigger': 'item', 'from_species': 'Eevee',
                        'item': 'FireStone', 'exclusive_group': 'eevee_evolution', 'choice': 'Flareon'}],
            'Jolteon': [{'method': 'evolution', 'trigger': 'item', 'from_species': 'Eevee',
                        'item': 'ThunderStone', 'exclusive_group': 'eevee_evolution', 'choice': 'Jolteon'}],
        })
        facts = {'money': 5000, 'bag': {}, 'party': [{'species': 'Eevee'}],
                 'dex': {'owned_species': ['Eevee', 'Jolteon']}}
        self.assertEqual(agent.item_evolution_spending_reference(facts, 0)['held_source_options'], [])

    def test_bulk_ball_references_require_an_unregistered_ready_static_source(self):
        agent = self.bulk_ball_agent()
        facts = {'bag': {'POKEBALL': 12}, 'money': 100000, 'map': 'ViridianCity',
                 'dex': {'owned_species': ['Moltres']}}
        groups = {}
        agent.add_ball_supply(groups, facts)
        self.assertEqual(groups, {})
        facts['dex']['owned_species'] = []
        from openpokered.story_rules import literal
        agent.index.rules[-1].guards.append(({'Call': {'callee': 'getFlag',
            'args': [literal('CAN_MEET_TARGET')]}}, True))
        facts['flags'] = {}
        agent.add_ball_supply(groups, facts)
        self.assertEqual(groups, {})
        facts['flags']['CAN_MEET_TARGET'] = True
        agent.add_ball_supply(groups, facts)
        self.assertTrue(groups)

    def scripted_ball_agent(self):
        from openpokered.story_rules import StoryIndex, compile_story, literal
        call = lambda name, *args: {'Call': {'callee': name, 'args': [literal(a) for a in args]}}
        gift = {'id': 'SilphCo11F:talkSilphPresident', 'map': 'SilphCo11F',
                'triggers': ['npc:1'], 'program': [{'If': {
                    'condition': call('getFlag', 'EVENT_GOT_MASTER_BALL'), 'then_branch': [],
                    'else_branch': [{'Command': {'name': 'giveItem',
                                                 'args': [literal('MASTER_BALL'), literal(1)]}}]}}]}
        agent = self.ball_supply_agent()
        index = StoryIndex.__new__(StoryIndex)
        index.rules = compile_story(gift)
        index.by_effect = {}
        for rule in index.rules:
            index.by_effect.setdefault(rule.effect, []).append(rule)
        agent.index = index
        return agent

    def test_scripted_ball_source_survives_full_ordinary_reserve_without_money(self):
        agent = self.scripted_ball_agent()
        groups = {}
        facts = {'bag': {'ULTRABALL': 12}, 'flags': {}, 'money': 0, 'map': 'FuchsiaCity'}
        agent.add_ball_supply(groups, facts)
        gift = groups['ball-source:MASTER_BALL']
        self.assertEqual(gift['target'], ('item', 'MASTER_BALL', True))
        self.assertEqual(gift['context']['ball'], 'MasterBall')
        self.assertEqual(gift['context']['source_maps'], ['SilphCo11F'])
        self.assertIn('Guaranteed capture', gift['context']['capture_behavior'])
        self.assertEqual(gift['rules'], agent.index.rules)
        self.assertEqual(facts['bag'], {'ULTRABALL': 12})

    def test_scripted_ball_source_is_not_replenishable_or_already_owned(self):
        agent = self.scripted_ball_agent()
        for bag, flags in [({'ULTRABALL': 12}, {'EVENT_GOT_MASTER_BALL': True}),
                           ({'MASTERBALL': 1, 'ULTRABALL': 12}, {})]:
            groups = {}
            agent.add_ball_supply(groups, {'bag': bag, 'flags': flags, 'money': 0, 'map': 'FuchsiaCity'})
            self.assertEqual(groups, {})
        agent.collects_dex = False
        groups = {}
        agent.add_ball_supply(groups, {'bag': {}, 'flags': {}, 'money': 0, 'map': 'FuchsiaCity'})
        self.assertEqual(groups, {})

    def test_scripted_ball_source_preserves_backchained_prerequisites_and_bag_capacity(self):
        agent = self.scripted_ball_agent()
        gift = agent.index.rules[0]
        from openpokered.story_rules import literal
        gift.guards.append(({'Call': {'callee': 'getFlag', 'args': [literal('ACCESS')]}}, True))
        access = Rule('access', 'SilphCo5F', 'SilphCo5F:access', [], [], [], ('flag', 'ACCESS', True), [])
        agent.index.rules.append(access)
        agent.index.by_effect[access.effect] = [access]
        bag = {f'ITEM{i}': 1 for i in range(20)}
        groups = {}
        agent.add_ball_supply(groups, {'bag': bag, 'flags': {}, 'money': 0, 'map': 'FuchsiaCity'})
        gift_group = groups['ball-source:MASTER_BALL']
        self.assertEqual(gift_group['rules'], [access])
        self.assertEqual(gift_group['target'], gift.effect)
        self.assertEqual(gift_group['context']['occupied_bag_slots'], 20)
        self.assertEqual(bag, {f'ITEM{i}': 1 for i in range(20)})

    def test_scripted_ball_sources_are_not_a_master_ball_shortlist(self):
        agent = self.scripted_ball_agent()
        pickup = Rule('pickup', 'Route10', 'Route10:ball', [], [], [], ('item', 'GREAT_BALL', True), [])
        agent.index.rules.append(pickup)
        agent.index.by_effect[pickup.effect] = [pickup]
        groups = {}
        agent.add_ball_supply(groups, {'bag': {'ULTRABALL': 12}, 'flags': {}, 'money': 0, 'map': 'FuchsiaCity'})
        self.assertEqual(set(groups), {'ball-source:MASTER_BALL', 'ball-source:GREAT_BALL'})
        self.assertIn('not a guaranteed capture', groups['ball-source:GREAT_BALL']['context']['capture_behavior'])

    def support_training_agent(self):
        from openpokered.story_rules import StoryIndex
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        agent.index = StoryIndex.__new__(StoryIndex)
        agent.validated_owned = Mock(return_value=set())
        agent.capture_retreats = {'PowerPlant:Zapdos': {
            'map': 'PowerPlant', 'species': 'Zapdos', 'retreat_observation': {
                'enemy': {'species': 'Zapdos', 'level': 50, 'hp': 150, 'max_hp': 150, 'status': 'None'},
                'party': [{'species': 'Gloom', 'hp': 0}]}}}
        agent.find_training_sites = Mock(return_value={'Route24': (5, 18)})
        agent.training_sites = {'Route24': (5, 18)}
        agent.maps = {'Route24': {'wild': {'red': {'grass': {
            'encounterRate': 25, 'mons': [{'species': 'Pidgey', 'level': 12}] * 10}}}}}
        party = [{'species': 'Charizard', 'level': 70, 'hp': 240, 'max_hp': 240,
                  'moves': ['Slash'], 'pp': [20], 'status': 'None'},
                 {'species': 'Gloom', 'level': 24, 'hp': 71, 'max_hp': 71,
                  'moves': ['Absorb', 'Poisonpowder', 'StunSpore', 'SleepPowder'],
                  'pp': [20, 35, 30, 15], 'status': 'None'}]
        return agent, {'party': party, 'bag': {}, 'map': 'CeruleanCity'}

    def test_capture_support_training_uses_named_trainee_not_strong_leader(self):
        agent, facts = self.support_training_agent()
        groups = {}
        agent.add_capture_support_training(groups, facts)
        group = groups['prepare:capture-support:Gloom']
        self.assertEqual(group['target'], ('level', 'Gloom', 25))
        self.assertFalse(agent.index.satisfied(group['target'], facts))
        self.assertEqual(group['context']['safe_status_moves'], ['StunSpore', 'SleepPowder'])
        self.assertIn('not a survival guarantee', group['context']['scope'])
        self.assertEqual(group['context']['training_cost']['levels_remaining'], 1)
        self.assertEqual(group['context']['level_gap_to_highest_observed_target'], 26)
        self.assertEqual(group['context']['training_cost_to_observed_target_level']['levels_remaining'], 26)
        agent.find_training_sites.assert_called_once_with(facts, shared_experience=True)
        agent.active = group
        facts.update(map='Route24', x=5, y=18)
        candidates, bindings = agent.action_candidates(facts)
        self.assertEqual(bindings['action:0'][0], 'lead_with:Gloom')
        facts['party'].reverse()
        candidates, bindings = agent.action_candidates(facts)
        self.assertEqual(bindings['action:0'][0], 'train_encounter:Route24,5,18')
        facts['party'][0]['level'] = 25
        self.assertTrue(agent.index.satisfied(group['target'], facts))

    def test_support_training_reaches_terrain_before_exposing_trainee(self):
        agent, facts = self.support_training_agent()
        groups = {}
        agent.add_capture_support_training(groups, facts)
        agent.active = next(iter(groups.values()))
        facts.update(map='CeruleanCity', x=10, y=10)
        choices, bindings = agent.action_candidates(facts)
        operations = {value[0] for value in bindings.values()}
        self.assertEqual(operations, {'reach_training:Route24,5,18'})
        self.assertIn('training point', json.loads(choices['action:0'])['purpose'])
        # A trainee already leading (e.g. a resumed training trip) does not
        # force a specific replacement; Jev may select the other living lead.
        facts['party'].reverse()
        _, bindings = agent.action_candidates(facts)
        self.assertEqual({value[0] for value in bindings.values()},
                         {'reach_training:Route24,5,18', 'lead_with:Charizard'})
        facts.update(map='Route24', x=5, y=17)
        _, bindings = agent.action_candidates(facts)
        self.assertIn('reach_training:Route24,5,18', {value[0] for value in bindings.values()})
        facts.update(x=5, y=18)
        _, bindings = agent.action_candidates(facts)
        self.assertEqual({value[0] for value in bindings.values()}, {'train_encounter:Route24,5,18'})

    def test_training_transit_preserves_arrival_and_interruption_results(self):
        rule = Rule('train', 'Route24', 'skill:train_encounter', [], [], [],
                    ('level', 'Gloom', 25), [])
        for outcome in ('reached', 'blocked', 'paused_after_battle'):
            with self.subTest(outcome=outcome):
                agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
                agent.actions, agent.max_actions = 0, 10
                agent.active = {'target': rule.effect}
                result = {'result': outcome, 'destination': 'Route24'}
                agent.travel = Mock(return_value=result)
                agent.settle, agent.remember_travel_result, agent.record = Mock(), Mock(), Mock()
                agent.game, agent.client = Mock(), Mock()
                self.assertIs(agent.execute('reach_training:Route24,5,18', rule), result)
                agent.travel.assert_called_once_with('Route24', rule, [(5, 18)], avoid_encounters=True)
                agent.settle.assert_called_once_with(rule.effect, rule)
                agent.remember_travel_result.assert_called_once_with('Route24', result)
                agent.record.assert_called_once_with('operation', operation='reach_training:Route24,5,18',
                                                      result=result, script=rule.storyline)
                self.assertEqual(agent.actions, 1)
                # Transit does not reorder the party or initiate a training hunt.
                self.assertEqual(agent.game.mock_calls, [])
                self.assertEqual(agent.client.mock_calls, [])
                agent.actions = agent.max_actions
                agent.travel.reset_mock()
                with self.assertRaisesRegex(StoryStopped, 'action_budget'):
                    agent.execute('reach_training:Route24,5,18', rule)
                agent.travel.assert_not_called()

    def test_training_transit_offers_all_other_living_leads(self):
        agent, facts = self.support_training_agent()
        groups = {}
        agent.add_capture_support_training(groups, facts)
        agent.active = next(iter(groups.values()))
        facts['party'].reverse()
        facts['party'].extend([
            {**facts['party'][0], 'species': 'Pidgey', 'level': 3},
            {**facts['party'][0], 'species': 'Pikachu', 'hp': 0},
        ])
        facts.update(x=10, y=10)
        _, bindings = agent.action_candidates(facts)
        self.assertEqual({value[0] for value in bindings.values()},
                         {'reach_training:Route24,5,18', 'lead_with:Charizard', 'lead_with:Pidgey'})

    def test_collection_travel_exposes_leader_choices_without_forcing_one(self):
        agent, facts = self.support_training_agent()
        facts['party'].reverse()
        agent.maps['PowerPlant'] = {'wild': {'red': {'grass': {'encounterRate': 10,
            'mons': [{'species': 'Voltorb', 'level': 21}, {'species': 'Magnemite', 'level': 24}]}}}}
        # Static capture, healing and Surf must also expose preparation,
        # not only evolution/shared-experience training.
        for target, operation in ((('register', 'Zapdos', True), 'travel_to:PowerPlant'),
                                  (('heal', 'party', True), 'travel_to:CeruleanPokecenter'),
                                  (('location', ('Route10', 15, 44), True), 'surf:1')):
            with self.subTest(operation=operation):
                rule = Rule('trip', 'Route10', 'trip', [], [], [], target, [])
                agent.active = {'target': target}
                agent._action_candidates = Mock(return_value=(
                    {'action:0': json.dumps({'operation': operation})},
                    {'action:0': (operation, rule)}))
                choices, bindings = agent.action_candidates(facts)
                self.assertEqual({op for op, _ in bindings.values()}, {operation, 'lead_with:Charizard'})
                self.assertEqual(bindings['action:0'], (operation, rule))
                self.assertEqual(json.loads(choices['action:1'])['current_leader']['species'], 'Gloom')
                self.assertIs(bindings['action:1'][1], rule)
                if operation == 'travel_to:PowerPlant':
                    route = json.loads(choices['action:0'])['route_encounters']
                    self.assertEqual(route, [{'map': 'PowerPlant', 'terrain': 'grass',
                        'level_range': [21, 24], 'species': ['Magnemite', 'Voltorb']}])
                    self.assertEqual(json.loads(choices['action:1'])['route_encounters'], route)

    def test_transit_leader_choice_explains_preparation_without_forcing_it(self):
        agent, facts = self.support_training_agent()
        agent.active = {'context': {}}
        candidates = {'action:0': json.dumps({'operation': 'lead_with:Charizard',
                                             'transit_leader': facts['party'][0]})}
        with patch.object(DualStoryAgent, 'choose', return_value='action:0') as choose:
            agent.choose('action', {'subgoal': ['register', 'Zapdos', True], 'local_state': facts},
                         candidates, 'Choose the next operation.')
        instruction = choose.call_args.args[3]
        self.assertIn('valid preparation step', instruction)
        self.assertIn('already suitable', instruction)
        self.assertIn('not a forecast or a survival guarantee', instruction)
        self.assertTrue(choose.call_args.kwargs['allow_abstain'])

    def test_transit_leader_options_do_not_mask_duplicates_or_change_local_work(self):
        agent, facts = self.support_training_agent()
        rule = Rule('trip', 'Route24', 'trip', [], [], [], (), [])
        facts['party'].extend([{**facts['party'][1], 'species': 'Pikachu', 'hp': 0},
                               {**facts['party'][1], 'species': 'Pikachu', 'hp': 30}])
        candidates = {'action:0': '{}', 'action:2': '{}'}
        bindings = {'action:0': ('travel_to:Route24', rule), 'action:2': ('lead_with:Gloom', rule)}
        agent.add_transit_lead_candidates(candidates, bindings, facts)
        self.assertEqual(len(bindings), 2)  # Already offered Gloom; first Pikachu is fainted.
        for operation in ('train_encounter:Route24,5,18', 'lead_with:Gloom', 'interact_counter:3,3,up,0'):
            candidates, bindings = {'action:0': '{}'}, {'action:0': (operation, rule)}
            agent.add_transit_lead_candidates(candidates, bindings, facts)
            self.assertEqual(bindings, {'action:0': (operation, rule)})

    def test_selected_journey_asks_leader_separately_and_keeps_field_move_user(self):
        agent, facts = self.support_training_agent()
        rule = Rule('surf', 'Route10', 'surf', [], [], [], (), [])
        facts['party'][1]['moves'].append('Surf')
        agent.active = {'target': ('location', ('Route10', 15, 44), True),
                        'context': {'map': 'Route10', 'destination': 'PowerPlant'}}
        agent.facts = Mock(return_value=facts)
        agent.execute = Mock(side_effect=lambda *args: facts['party'].reverse())
        with patch.object(DualStoryAgent, 'choose', return_value='lead_with:Gloom') as choose:
            self.assertEqual(agent.prepare_transit_lead('surf:1', rule), 'surf:0')
        agent.execute.assert_called_once_with('lead_with:Gloom', rule)
        self.assertEqual(choose.call_args.args[1]['stage'], 'transit_preparation')
        self.assertEqual(set(choose.call_args.args[2]), {'keep', 'lead_with:Gloom'})
        self.assertFalse(choose.call_args.kwargs['allow_abstain'])
        # Keeping the already suitable leader is a complete valid outcome.
        agent.execute.reset_mock()
        with patch.object(DualStoryAgent, 'choose', return_value='keep'):
            self.assertEqual(agent.prepare_transit_lead('surf:0', rule), 'surf:0')
        agent.execute.assert_not_called()

    def test_transit_preparation_is_scoped_and_respects_action_budget(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        agent.actions = agent.max_actions = 1
        agent.prepare_transit_lead = Mock(return_value='travel_to:Route24')
        rule = Rule('trip', 'Route24', 'trip', [], [], [], (), [])
        with self.assertRaisesRegex(StoryStopped, 'action_budget'):
            agent.execute('travel_to:Route24', rule)
        agent.prepare_transit_lead.assert_not_called()
        agent.actions = 0
        agent.active = {'target': ('location', 'Route24', True)}
        agent.travel, agent.settle, agent.remember_travel_result, agent.record = Mock(), Mock(), Mock(), Mock()
        agent.execute('travel_to:Route24', rule)
        agent.prepare_transit_lead.assert_called_once_with('travel_to:Route24', rule)
        agent.travel.assert_called_once_with('Route24', rule)

    def capture_retrieval_fixture(self):
        agent, facts = self.support_training_agent()
        pc = Rule('pc', 'CeruleanPokecenter', 'pc', ['sign:1'], [], [],
                  ('pc', 'storage', True), [])
        agent.index.by_effect = {('pc', 'storage', True): [pc]}
        facts['stored_pokemon'] = [
            {'box': 3, 'index': 11, 'species': 'Pikachu', 'level': 44, 'hp': 101, 'max_hp': 101,
             'status': 'None', 'moves': ['ThunderWave', 'Thunderbolt'], 'pp': [20, 15]},
            {'box': 0, 'index': 8, 'species': 'Jigglypuff', 'level': 3, 'hp': 20, 'max_hp': 20,
             'status': 'None', 'moves': ['Sing'], 'pp': [15]}]
        return agent, facts

    def test_stored_capture_support_is_offered_even_without_an_evolution_goal(self):
        agent, facts = self.capture_retrieval_fixture()
        facts['party'] = facts['party'][:1] * 6
        groups = {}
        agent.add_capture_support_retrieval(groups, facts)
        self.assertEqual(len(groups), 2)
        pika = groups['prepare:retrieve-capture-support:3:11']
        self.assertEqual(pika['target'], ('pokemon', 'Pikachu', None))
        self.assertEqual(pika['context']['required_for'], ['Zapdos'])
        self.assertTrue(pika['context']['storage_retrieval'])
        self.assertTrue(pika['context']['requires_party_deposit'])
        self.assertFalse(pika['context']['requires_healing'])
        moves = pika['context']['capture_support_matchups'][0]['non_damaging_status_moves']
        self.assertEqual([m['move'] for m in moves], ['ThunderWave'])
        self.assertEqual(moves[0]['pp'], 20)
        self.assertIn('No survival guarantee', pika['context']['scope'])

    def test_stored_capture_support_keeps_distinct_slots_and_healing_cost(self):
        agent, facts = self.capture_retrieval_fixture()
        facts['stored_pokemon'].append({**facts['stored_pokemon'][0], 'index': 12, 'level': 10,
                                       'hp': 0, 'max_hp': 30})
        groups = {}
        agent.add_capture_support_retrieval(groups, facts)
        self.assertEqual(len(groups), 3)
        weak = groups['prepare:retrieve-capture-support:3:12']['context']
        self.assertTrue(weak['requires_healing'])
        self.assertEqual(weak['stored_pokemon']['level'], 10)

    def test_stored_capture_support_binds_the_real_pc_slot_and_menu(self):
        agent, facts = self.capture_retrieval_fixture()
        groups = {}
        agent.add_capture_support_retrieval(groups, facts)
        agent.active = groups['prepare:retrieve-capture-support:3:11']
        choices, bindings = agent.action_candidates(facts)
        self.assertEqual(bindings['action:0'][0], 'travel_to:CeruleanPokecenter')
        agent.index.maps_dir = Path(__file__).resolve().parents[1] / 'crates/pokered-data/maps'
        facts['map'] = 'CeruleanPokecenter'
        choices, bindings = agent.action_candidates(facts)
        self.assertEqual(bindings['action:0'][0], 'retrieve_pc:3,11,-1,0')
        self.assertEqual(json.loads(choices['action:0'])['withdraw']['level'], 44)

    def test_stored_capture_support_rejects_unsupported_or_satisfied_candidates(self):
        for change in ('no_retreat', 'registered', 'no_pc', 'already_present', 'no_pp',
                       'damaging_move', 'poison', 'immune', 'noncollector'):
            agent, facts = self.capture_retrieval_fixture()
            facts['stored_pokemon'] = facts['stored_pokemon'][:1]
            mon = facts['stored_pokemon'][0]
            if change == 'no_retreat':
                agent.capture_retreats.clear()
            elif change == 'registered':
                agent.validated_owned.return_value = {'Zapdos'}
            elif change == 'no_pc':
                agent.index.by_effect.clear()
            elif change == 'already_present':
                facts['party'].append(mon)
            elif change == 'no_pp':
                mon['pp'][0] = 0
            elif change == 'damaging_move':
                mon['moves'][0] = 'Thunderbolt'
            elif change == 'poison':
                mon['moves'][0] = 'Poisonpowder'
            elif change == 'immune':
                agent.capture_retreats['PowerPlant:Zapdos']['species'] = 'Onix'
            else:
                agent.collects_dex = False
            groups = {}
            agent.add_capture_support_retrieval(groups, facts)
            self.assertEqual(groups, {}, change)

    def test_capture_support_training_requires_observed_failure_and_healthy_trainee(self):
        for change in ('unknown', 'registered', 'fainted', 'injured', 'status', 'noncollector', 'no_sites'):
            agent, facts = self.support_training_agent()
            if change == 'unknown':
                agent.capture_retreats['PowerPlant:Zapdos'].pop('retreat_observation')
            elif change == 'registered':
                agent.validated_owned.return_value = {'Zapdos'}
            elif change == 'fainted':
                facts['party'][1]['hp'] = 0
            elif change == 'injured':
                facts['party'][1]['hp'] = 20
            elif change == 'status':
                facts['party'][1]['status'] = 'Paralysis'
            elif change == 'noncollector':
                agent.collects_dex = False
            else:
                agent.find_training_sites.return_value = {}
            groups = {}
            agent.add_capture_support_training(groups, facts)
            self.assertEqual(groups, {}, change)

    def test_capture_support_training_offers_all_eligible_supports(self):
        agent, facts = self.support_training_agent()
        facts['party'].append({'species': 'Paras', 'level': 23, 'hp': 60, 'max_hp': 60,
                               'moves': ['StunSpore'], 'pp': [30], 'status': 'None'})
        groups = {}
        agent.add_capture_support_training(groups, facts)
        self.assertEqual(set(groups), {'prepare:capture-support:Gloom', 'prepare:capture-support:Paras'})
        target = groups['prepare:capture-support:Paras']['target']
        self.assertFalse(agent.index.satisfied(target, facts))
        facts['party'][-1].update(species='Parasect', level=24)
        self.assertTrue(agent.index.satisfied(target, facts))

    def test_capture_support_training_switches_once_to_finisher(self):
        agent, facts = self.support_training_agent()
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.collects_dex = False
        game.judgments.active = {'context': {'capture_support_training': True,
            'trigger': 'level', 'from_species': 'Gloom'}}
        game.judgments.choose.return_value = 'switch:1'
        party = list(reversed(facts['party']))
        state = {'party': party, 'battle_inventory': [], 'battle_live': {
            'player': party[0], 'enemy': {'species': 'Kakuna'}, 'player_party': party}}
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 1))
        self.assertIn('share experience', game.judgments.choose.call_args.args[2]['switch:1'])
        self.assertIn('train the active trainee', game.judgments.choose.call_args.args[3])
        state['battle_live']['player'] = party[1]
        self.assertIsNone(game.battle_recovery_plan(state))

    def test_ball_supply_offers_an_unvisited_mart_with_its_distance(self):
        agent = self.ball_supply_agent()
        agent.visited = set()  # A mart is a restock target before it is entered.
        agent.client.route.return_value = {'found': True, 'legs': [{}] * 6}
        groups = {}
        agent.add_ball_supply(groups, {'bag': {}, 'money': 3000, 'map': 'MtMoon1F'})
        self.assertEqual(list(groups), ['ball:shop:ViridianMart:PokeBall'])
        context = groups['ball:shop:ViridianMart:PokeBall']['context']
        self.assertEqual((context['map'], context['map_hops']), ('ViridianMart', 6))

    def test_ball_supply_keeps_the_two_nearest_shops_per_ball_kind(self):
        agent = self.ball_supply_agent()
        agent.visited = set()
        cerulean = Rule('shop:CeruleanMart', 'CeruleanMart', 'CeruleanMart:shop', [], [], [],
                        ('shop', ('POKE_BALL',), True), [])
        saffron = Rule('shop:SaffronMart', 'SaffronMart', 'SaffronMart:shop', [], [], [],
                       ('shop', ('POKE_BALL',), True), [])
        agent.index = Mock(rules=[*agent.index.rules, cerulean, saffron])
        def route(origin, destination):
            legs = {'CeruleanMart': 2, 'SaffronMart': 4}.get(destination, 6)
            return {'found': True, 'legs': [{}] * legs}
        agent.client.route.side_effect = route
        groups = {}
        agent.add_ball_supply(groups, {'bag': {}, 'money': 3000, 'map': 'MtMoon1F'})
        # The two nearest survive — a blocked route to the nearest must not
        # strike the whole supply line out — and a farther one is dropped.
        self.assertEqual(sorted(groups), ['ball:shop:CeruleanMart:PokeBall',
                                          'ball:shop:SaffronMart:PokeBall'])
        self.assertEqual(groups['ball:shop:CeruleanMart:PokeBall']['context']['map_hops'], 2)
        self.assertEqual(groups['ball:shop:SaffronMart:PokeBall']['context']['map_hops'], 4)

    def test_ball_supply_offers_nothing_when_no_shop_is_reachable(self):
        agent = self.ball_supply_agent()
        agent.client.route.return_value = {'found': False}
        groups = {}
        agent.add_ball_supply(groups, {'bag': {}, 'money': 3000, 'map': 'MtMoon1F'})
        self.assertEqual(groups, {})

    def catch_goal_agent(self, objectives):
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        agent = AutonomousStoryAgent(client, Mock(), objectives, game=Mock())
        agent.index = Mock(rules=[], by_effect={})
        agent.defeat_preparation = 0
        agent.nearby_healers = Mock(return_value=[])
        agent.annotate_navigation = Mock(return_value={})
        agent.transport_frontiers = Mock()
        agent.find_catch_areas = Mock(return_value={'Route2': {'species': ['Weedle'],
                                                              'spots': [(5, 18)], 'reachable': True}})
        return agent

    def test_catch_targets_are_offered_only_for_the_collect_dex_goal(self):
        from openpokered.story_agent import DualStoryAgent
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [8, 25]}
        facts = {'party': [mon], 'bag': {'POKEBALL': 1}, 'flags': {}, 'fully_recovered': False,
                 'map': 'PewterCity', 'x': 12, 'y': 18}
        with patch.object(DualStoryAgent, 'strategy_groups', side_effect=lambda facts: {}):
            collecting = self.catch_goal_agent([{
                'id': 'collect-dex', 'agent_verified': True,
                'name': 'Register every wild species; clear the first playthrough to open the areas '
                            'that hold the rest'}]).strategy_groups(facts)
            story = self.catch_goal_agent([{
                'id': 'beat-brock', 'satisfied_when': {'flag': 'EVENT_BEAT_BROCK'}}]).strategy_groups(facts)
        self.assertEqual(list(collecting), ['collect:Route2'])
        self.assertEqual(collecting['collect:Route2']['target'], ('catch', 'Route2', True))
        self.assertEqual(collecting['collect:Route2']['rules'][0].storyline, 'skill:catch_encounter')
        self.assertEqual(story, {})

    def test_catch_targets_require_balls_and_state_the_stock(self):
        from openpokered.story_agent import DualStoryAgent
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [8, 25]}
        def group(bag):
            facts = {'party': [mon], 'bag': bag, 'flags': {}, 'fully_recovered': False,
                     'map': 'PewterCity', 'x': 12, 'y': 18}
            with patch.object(DualStoryAgent, 'strategy_groups', side_effect=lambda facts: {}):
                return self.catch_goal_agent([{'id': 'collect-dex',
                                               'agent_verified': True}]).strategy_groups(facts)
        empty = group({})
        self.assertNotIn('collect:Route2', empty)
        stocked = group({'POKEBALL': 7})['collect:Route2']
        self.assertEqual(stocked['context']['balls_held'], 7)
        self.assertIn('7 carried', stocked['context']['prerequisite'])

    def test_exhausted_capture_stock_replans_except_safari_or_training(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.replan_after_defeat = False
        agent.needs_skill_recovery = Mock(return_value=False)
        agent.active = {'target': ('catch', 'Route2', True), 'rules': []}
        for method in ('grass', 'water', 'fishing'):
            agent.active['context'] = {'acquisition_method': method}
            self.assertTrue(agent.should_replan({'bag': {}}))
            self.assertFalse(agent.should_replan({'bag': {'GREATBALL': 1}}))
        agent.active['context'] = {'acquisition_method': 'safari'}
        self.assertFalse(agent.should_replan({'bag': {}}))
        agent.active = {'target': ('level', 'leader', 40)}
        self.assertFalse(agent.should_replan({'bag': {}}))

    def test_empty_ball_hunt_never_walks_into_an_encounter(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        rule = Rule('hunt', 'Route2', 'skill:catch_encounter', [], [], [],
                    ('catch', 'Route2', True), [])
        agent.active = {'target': rule.effect}
        agent.actions, agent.max_actions = 0, 100
        agent.facts = Mock(return_value={'bag': {}})
        agent.game, agent.client, agent.record = Mock(), Mock(), Mock()
        result = agent.execute('catch_encounter:Route2,5,18', rule)
        self.assertEqual(result['required_capability'], 'capture_balls')
        self.assertIsNone(agent.active)
        agent.game.d.drive.assert_not_called()
        agent.client.state.assert_not_called()

    def test_fishing_hunt_stops_immediately_after_spending_last_ball(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        rule = Rule('hunt', 'Route2', 'skill:catch_encounter', [], [], [],
                    ('catch', 'fishing:OldRod:Route2', True), [])
        agent.active = {'target': rule.effect}
        agent.actions, agent.max_actions = 0, 100
        agent.facts = Mock(side_effect=[{'bag': {'POKEBALL': 1}}, {'bag': {}}])
        agent.needs_capture_recovery = Mock(return_value=False)
        agent.game, agent.client, agent.record = Mock(), Mock(), Mock()
        agent.client.state.return_value = {'map_name': 'Route2', 'party': [{'level': 20}],
                                           'pokedex': {'owned': 58}}
        agent.check_budget, agent.settle = Mock(), Mock()
        result = agent.execute('catch_encounter:fishing,Route2,5,18,down,OldRod', rule)
        self.assertEqual(result['owned_after'], 58)
        self.assertIsNone(agent.active)
        agent.game.use_field_item.assert_called_once_with('OldRod')

    def test_dex_completion_requires_every_supported_acquisition_and_is_never_vacuous(self):
        from openpokered.story_rules import StoryIndex
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = StoryIndex.__new__(StoryIndex)
        agent.index._wild_cache = {'Route1': {'Pidgey', 'Rattata'}, 'ViridianCity': set()}
        agent.maps = {'Route1': {}, 'ViridianCity': {}}
        agent._collection_graph = {
            'Pidgey': [{'method': 'grass', 'map': 'Route1'}],
            'Rattata': [{'method': 'grass', 'map': 'Route1'}],
            'Magikarp': [{'method': 'fishing', 'map': 'ViridianCity', 'rod': 'OldRod'}],
        }
        agent._complete_collection_graph = agent._collection_graph
        self.assertFalse(agent.dex_complete({'dex': {'owned_species': ['Pidgey']}}))
        self.assertFalse(agent.dex_complete({'dex': {'owned_species': ['Rattata', 'Pidgey']}}))
        self.assertTrue(agent.dex_complete({'dex': {'owned_species': ['Rattata', 'Pidgey', 'Magikarp']}}))
        # The spawn room has no encounter table. "Nothing unregistered here"
        # must not read as a finished collection before the run has moved.
        agent.index._wild_cache = {'RedsHouse2F': set(), 'Route1': {'Pidgey'}}
        agent.maps = {'RedsHouse2F': {}, 'Route1': {}}
        agent._collection_graph = {'Pidgey': [{'method': 'grass', 'map': 'Route1'}]}
        agent._complete_collection_graph = agent._collection_graph
        self.assertFalse(agent.dex_complete({'dex': {'owned_species': []}}))
        agent.index = None  # No planning index yet: nothing is proven registered.
        self.assertFalse(agent.dex_complete({'dex': {'owned_species': ['Pidgey']}}))

    def test_collect_dex_objective_is_decided_by_registered_species(self):
        from openpokered.story_rules import StoryIndex
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = StoryIndex.__new__(StoryIndex)
        agent.index._wild_cache = {'Route1': {'Pidgey'}}
        agent.maps = {'Route1': {}}
        agent._complete_collection_graph = {
            'Pidgey': [{'method': 'grass', 'map': 'Route1'}],
        }
        objective = {'id': 'collect-dex', 'agent_verified': True,
                     'name': 'Register every wild species; clear the first playthrough to open the areas '
                            'that hold the rest'}
        self.assertFalse(agent.objective_satisfied(objective, {'dex': {'owned_species': []}}))
        self.assertTrue(agent.objective_satisfied(objective, {'dex': {'owned_species': ['Pidgey']}}))

    def test_runner_collect_dex_goal_enables_the_catch_targets(self):
        from openpokered.run_autonomous import DEX_OBJECTIVE
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        agent = AutonomousStoryAgent(client, Mock(), [DEX_OBJECTIVE], game=Mock())
        self.assertNotIn('satisfied_when', DEX_OBJECTIVE)  # The goal waits for no game flag.
        self.assertTrue(agent.collects_dex)

    def test_runner_goal_registry_reuses_the_collect_dex_entry_and_the_champion_flag(self):
        from openpokered import run_autonomous
        from openpokered.judgment_agent import load_objectives
        self.assertEqual(list(run_autonomous.GOAL_OBJECTIVES),
                         ['collect-dex', 'max-coverage', 'fast-clear'])
        self.assertIs(run_autonomous.DEX_OBJECTIVE, run_autonomous.GOAL_OBJECTIVES['collect-dex'])
        self.assertTrue(all(key == goal['id'] for key, goal in run_autonomous.GOAL_OBJECTIVES.items()))
        flagged = {key: goal for key, goal in run_autonomous.GOAL_OBJECTIVES.items()
                   if not goal.get('agent_verified')}
        self.assertEqual(list(flagged), ['fast-clear'])
        champion = next(o for o in load_objectives() if o['id'] == 'become-champion')
        self.assertEqual(flagged['fast-clear']['satisfied_when'], champion['satisfied_when'])

    def test_runner_records_the_preference_and_threads_it_into_the_agent(self):
        import io
        import tempfile
        from openpokered import run_autonomous
        options = {}
        add_argument = run_autonomous.argparse.ArgumentParser.add_argument
        def record(parser, *names, **kwargs):
            for name in names:
                options[name] = kwargs
            return add_argument(parser, *names, **kwargs)
        with tempfile.TemporaryDirectory() as root:
            path = Path(root)
            (path / 'pokered-app').write_bytes(b'binary')
            logs = path / 'logs'
            logs.mkdir()
            (logs / 'game.log').write_text('')
            game, agent = Mock(), Mock()
            game.run_dir, game.d.counts = logs, {}
            game.save_path = path / 'absent-native-save.sav'
            game.battles_driven, game.move_cache_hits, game.stationary_npcs = 0, 0, {}
            game.d.raw.cmd.return_value = {'ok': True, 'data': {}}
            agent.run.return_value = {}
            agent.calls, agent.tokens, agent.models = {}, {}, set()
            agent.completed, agent.actions, agent.resolved_battles = [], 0, 0
            agent.visited, agent.observed_barrier_maps = set(), set()
            agent.navigation_memory, agent.navigation_history = {}, {}
            agent.field_requirements, agent.battle_requirements = {}, {}
            agent.capture_retreats = {}
            agent.capture_retreat_totals = {}
            agent.collection_audit_pending = {}
            agent.battle_defeats, agent.defeat_preparation = [], 0
            agent.first_clear_verification, agent.mechanism_goal = None, None
            def verify_start_manifest():
                manifest = json.loads(next((path / 'out').glob('*/run-manifest.json')).read_text())
                self.assertEqual(manifest['status'], 'starting')
                self.assertEqual(manifest['preference'], 'level')
                self.assertFalse(manifest['success'])
                self.assertNotIn('development_checkpoint', manifest)
                self.assertTrue(manifest['policy_sha256'])
                self.assertTrue(manifest['binary_sha256'])
                self.assertEqual(manifest['budgets']['wall_seconds'], 7200)
                root = Path(run_autonomous.JevGame.call_args.kwargs['runtime_root'])
                self.assertTrue(root.is_relative_to((path / 'out').resolve()))
                self.assertEqual(run_autonomous.JevGame.call_args.kwargs['binary'].parent, root)
                return {}
            agent.run.side_effect = verify_start_manifest
            with patch.object(run_autonomous.argparse.ArgumentParser, 'add_argument', record), \
                    patch.object(run_autonomous, 'TypeSafeClient'), \
                    patch.object(run_autonomous, 'JevGame', return_value=game), \
                    patch.object(run_autonomous, 'boot_new_game', return_value={'screen': 'overworld'}), \
                    patch.object(run_autonomous, 'AutonomousStoryAgent', return_value=agent) as factory, \
                    patch.object(run_autonomous.signal, 'signal') as signals, \
                    patch('sys.stdout', new_callable=io.StringIO):
                code = run_autonomous.main(['--preference', 'level',
                                            '--binary', str(path / 'pokered-app'),
                                            '--output', str(path / 'out')])
            self.assertEqual(options['--preference']['choices'], ['none', 'level', 'type', 'tactic'])
            self.assertEqual(options['--preference']['default'], 'none')
            self.assertEqual(options['--jev-provider']['choices'],
                             ['auto', 'openrouter', 'typesafe'])
            self.assertEqual(factory.call_args.kwargs['preference'], 'level')
            handlers = dict(call.args for call in signals.call_args_list)
            for signal in (run_autonomous.signal.SIGINT, run_autonomous.signal.SIGTERM):
                game.d.stop_requested = False
                handlers[signal]()
                self.assertTrue(game.d.stop_requested)
            self.assertEqual(code, 1)
            folder = next((path / 'out').iterdir())
            self.assertFalse((folder / 'failure.txt').exists())  # The stubbed run reached the end.
            self.assertEqual(json.loads((folder / 'summary.json').read_text())['preference'], 'level')

    def test_every_goal_entry_appends_a_terminal_objective_to_the_story_prefix(self):
        from openpokered import run_autonomous
        from openpokered.judgment_agent import load_objectives
        for key, goal in run_autonomous.GOAL_OBJECTIVES.items():
            # The constructor rejects an unverified objective without a flag,
            # so building the agent proves the appended entry is complete.
            agent = self.goal_agent(load_objectives() + [goal])
            self.assertEqual(agent.objectives[-1]['id'], key)

    def goal_agent(self, objectives):
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        agent = AutonomousStoryAgent(client, Mock(), objectives, game=Mock())
        agent.index = Mock(rules=[], by_effect={})
        agent.defeat_preparation = 0
        agent.nearby_healers = Mock(return_value=[])
        agent.annotate_navigation = Mock(return_value={})
        agent.transport_frontiers = Mock()
        return agent

    def goal_flags(self, ids):
        agent = self.goal_agent([{'id': name, 'agent_verified': True} for name in ids])
        return agent.collects_dex, agent.maximizes_coverage, agent.avoids_optional_preparation

    def test_goal_flags_are_derived_from_the_objective_ids(self):
        self.assertEqual(self.goal_flags(['get-starter']), (False, False, False))
        self.assertEqual(self.goal_flags(['collect-dex']), (True, False, False))
        self.assertEqual(self.goal_flags(['max-coverage']), (False, True, False))
        self.assertEqual(self.goal_flags(['fast-clear']), (False, False, True))

    def test_goal_flags_add_their_instruction_to_the_strategy_call(self):
        from openpokered.story_agent import DualStoryAgent
        def instruction(objectives):
            agent = self.goal_agent(objectives)
            with patch.object(DualStoryAgent, 'choose', return_value='a') as choose:
                agent.choose('strategy', {}, {'a': 'x'}, 'pick')
            return choose.call_args.args[3]
        story = instruction([{'id': 'beat-brock', 'satisfied_when': {'flag': 'EVENT_BEAT_BROCK'}}])
        coverage = instruction([{'id': 'max-coverage', 'agent_verified': True}])
        speed = instruction([{'id': 'fast-clear', 'satisfied_when': {'flag': 'EVENT_BEAT_CHAMPION_RIVAL'}}])
        dex = instruction([{'id': 'collect-dex', 'agent_verified': True}])
        self.assertEqual(story, 'pick')  # A story run keeps its instruction untouched.
        self.assertIn('terminal goal is coverage', coverage)
        self.assertIn('progress in itself', coverage)
        self.assertIn('terminal goal is speed', speed)
        self.assertIn('shortest route', speed)
        # Collection must be told that clearing is the channel to more species,
        # or it would treat the story objectives as the finish line.
        self.assertIn('terminal goal is the Pokédex', dex)
        self.assertIn('channel to more species', dex)
        self.assertIn('never stop at the champion', dex.lower())
        self.assertIn('even if local species remain', dex)

    def preference_agent(self, preference):
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        return AutonomousStoryAgent(client, Mock(), [{'id': 'beat-brock',
            'satisfied_when': {'flag': 'EVENT_BEAT_BROCK'}}], game=Mock(), preference=preference)

    def test_preference_bias_reaches_the_strategy_and_action_questions(self):
        from openpokered.story_agent import DualStoryAgent
        from openpokered.autonomous_story import PREFERENCE_INSTRUCTIONS
        self.assertEqual(sorted(PREFERENCE_INSTRUCTIONS), ['level', 'tactic', 'type'])
        for preference, bias in PREFERENCE_INSTRUCTIONS.items():
            agent = self.preference_agent(preference)
            for layer in ('strategy', 'action'):
                with patch.object(DualStoryAgent, 'choose', return_value='a') as choose:
                    self.assertEqual(agent.choose(layer, {}, {'a': 'x'}, 'pick'), 'a')
                self.assertEqual(choose.call_args.args[3], f'pick {bias}')
        with patch.object(DualStoryAgent, 'choose', return_value='a') as choose:
            self.preference_agent('none').choose('strategy', {}, {'a': 'x'}, 'pick')
        self.assertEqual(choose.call_args.args[3], 'pick')

    def preparation_agent(self, preference, moves=('Scratch', 'Ember', 'Growl')):
        """An agent whose only offered battle is one stubbed level 21 trainer."""
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        agent = AutonomousStoryAgent(client, Mock(), [{'id': 'beat-brock',
            'satisfied_when': {'flag': 'EVENT_BEAT_BROCK'}}], game=Mock(), preference=preference)
        agent.index = Mock(rules=[], by_effect={})
        agent.destination_points = Mock(return_value=[])
        agent.nearby_healers = Mock(return_value=[])
        agent.annotate_navigation = Mock(return_value={})
        agent.transport_frontiers = Mock()
        agent.find_training_sites = Mock(return_value={'Route2': (7, 7)})
        agent.find_catch_areas = Mock(return_value={})
        agent.maps = {'Route2': {}, 'Route3': {'npcs': [
            {'textId': 2, 'trainerClass': 'Youngster', 'trainerSet': 1}]}}
        agent.trainers = {'Youngster': {'parties': [
            {'pokemon': [{'species': 'Bellsprout', 'level': 21}]}]}}
        rule = Rule('youngster', 'Route3', 'Route3:youngster', ['npc:2'], [], [],
                    ('flag', 'EVENT_BEAT_YOUNGSTER', True), [('battle', 'Youngster:1', True)])
        facts = {'party': [{'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
                            'status': 'None', 'moves': list(moves), 'pp': [35] * len(moves)}],
                 'bag': {}, 'flags': {}, 'fully_recovered': True,
                 'map': 'PewterCity', 'x': 12, 'y': 18}
        return agent, facts, {'youngster': {'target': rule.effect, 'rules': [rule],
                                           'objectives': ['Defeat the route trainer']}}

    def preparation_groups(self, preference, moves=('Scratch', 'Ember', 'Growl')):
        from openpokered.story_agent import DualStoryAgent
        agent, facts, groups = self.preparation_agent(preference, moves)
        with patch.object(DualStoryAgent, 'strategy_groups', return_value=groups):
            return agent.strategy_groups(facts)

    def test_level_preference_trains_a_margin_past_the_pending_threat(self):
        from openpokered.autonomous_story import LEVEL_PREFERENCE_MARGIN
        self.assertEqual(LEVEL_PREFERENCE_MARGIN, 2)
        self.assertEqual(self.preparation_groups('none')['prepare:train']['target'],
                         ('level', 'leader', 21))
        biased = self.preparation_groups('level')['prepare:train']
        self.assertEqual(biased['target'], ('level', 'leader', 21 + LEVEL_PREFERENCE_MARGIN))
        self.assertEqual(biased['context']['target_level'], 21 + LEVEL_PREFERENCE_MARGIN)

    def test_type_preference_adds_super_effective_options_to_the_battle_context(self):
        context = self.preparation_groups('type')['youngster']['context']
        self.assertEqual(context['type_options']['Bellsprout']['types'], ['Grass', 'Poison'])
        self.assertEqual(context['type_options']['Bellsprout']['super_effective_moves'],
                         {'Ember': {'pokemon': 'Charmeleon', 'type': 'Fire', 'power': 40,
                                    'effectiveness': 2}})
        self.assertNotIn('type_options', self.preparation_groups('none')['youngster']['context'])

    def test_tactical_preference_adds_status_moves_and_none_adds_no_options(self):
        context = self.preparation_groups('tactic')['youngster']['context']
        self.assertEqual(context['tactical_options']['opponent_species'], ['Bellsprout'])
        self.assertEqual(context['tactical_options']['status_moves'], {'Growl': {
            'pokemon': 'Charmeleon', 'type': 'Normal', 'effect': 'AttackDown1Effect'}})
        story = self.preparation_groups('none')['youngster']['context']
        self.assertNotIn('tactical_options', story)
        self.assertNotIn('type_options', story)

    def test_coverage_completion_requires_every_bordering_map(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {'PalletTown': {'connections': {'north': {'targetMap': 'Route1'}},
                                     'warps': [{'x': 5, 'y': 5, 'destMap': 'RedsHouse1F'}]},
                      'Route1': {'connections': {'south': {'targetMap': 'PalletTown'},
                                                 'north': {'targetMap': 'ViridianCity'}}},
                      'RedsHouse1F': {}, 'ViridianCity': {}}
        agent.visited = set()
        self.assertFalse(agent.coverage_complete({}))
        agent.visited = {'PalletTown', 'RedsHouse1F'}
        self.assertFalse(agent.coverage_complete({}))
        agent.visited = {'PalletTown', 'RedsHouse1F', 'Route1'}
        self.assertFalse(agent.coverage_complete({}))
        agent.visited = {'PalletTown', 'RedsHouse1F', 'Route1', 'ViridianCity'}
        self.assertTrue(agent.coverage_complete({}))

    def test_max_coverage_objective_is_decided_by_bordering_maps(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {'PalletTown': {'connections': {'north': {'targetMap': 'Route1'}}}, 'Route1': {}}
        objective = {'id': 'max-coverage', 'agent_verified': True,
                     'name': 'Leave no bordering area unexplored and finish the first playthrough'}
        agent.visited = {'PalletTown'}
        self.assertFalse(agent.objective_satisfied(objective, {}))
        agent.visited = {'PalletTown', 'Route1'}
        self.assertTrue(agent.objective_satisfied(objective, {}))

    def test_speed_goal_drops_optional_preparation_groups(self):
        from openpokered.story_agent import DualStoryAgent
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 20, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [8, 25]}
        facts = {'party': [mon], 'bag': {'POTION': 1}, 'flags': {}, 'fully_recovered': False,
                 'map': 'PewterCity', 'x': 12, 'y': 18}
        with patch.object(DualStoryAgent, 'strategy_groups', side_effect=lambda facts: {}):
            speed = self.goal_agent([{'id': 'fast-clear', 'satisfied_when': {
                'flag': 'EVENT_BEAT_CHAMPION_RIVAL'}}]).strategy_groups(facts)
            story = self.goal_agent([{'id': 'beat-brock', 'satisfied_when': {
                'flag': 'EVENT_BEAT_BROCK'}}]).strategy_groups(facts)
        self.assertEqual(list(story), ['prepare:medicine'])
        self.assertTrue(story['prepare:medicine']['context']['optional_preparation'])
        self.assertEqual(speed, {})

    def test_code_layers_pick_candidates_without_calling_the_model(self):
        from openpokered.run_autonomous import DEX_OBJECTIVE
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        model = Mock()
        agent = AutonomousStoryAgent(client, model, [DEX_OBJECTIVE], game=Mock(),
                                     strategy_jev=False, action_jev=False)
        self.assertEqual(agent.layer_jev, {'strategy': False, 'action': False})
        self.assertEqual(agent.choose('strategy', {}, {'a': 'x', 'b': 'y'}, 'pick'), 'a')
        self.assertFalse(model.system_one.called)

    def test_coverage_groups_offer_unvisited_bordering_areas(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {'A': {'connections': {'north': {'targetMap': 'B'}}, 'warps': []},
                      'B': {'connections': {}, 'warps': []},
                      'C': {'connections': {}, 'warps': [{'x': 4, 'y': 6, 'destMap': 'D'}]},
                      'D': {'connections': {}, 'warps': []}}
        agent.visited = {'A', 'C'}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'B'}]}
        agent.entry_tile = lambda name: (3, 5)
        groups = {}
        agent.add_coverage_groups(groups, {'map': 'A'})
        self.assertEqual(sorted(groups), ['explore:B', 'explore:D'])
        self.assertEqual(groups['explore:B']['target'], ('explore', 'B', True))
        self.assertEqual(groups['explore:B']['rules'][0].map, 'B')
        self.assertEqual(groups['explore:B']['rules'][0].triggers, ['coord:(3,5)'])

    def test_coverage_groups_skip_areas_with_no_confirmed_route(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {'A': {'connections': {'north': {'targetMap': 'B'}}, 'warps': []},
                      'B': {'connections': {}, 'warps': []}}
        agent.visited = {'A'}
        agent.client = Mock()
        agent.client.route.return_value = {'found': False, 'legs': []}
        agent.entry_tile = lambda name: (3, 5)
        groups = {}
        agent.add_coverage_groups(groups, {'map': 'A'})
        self.assertEqual(groups, {})

    def test_receiving_travel_supplies_replans_before_a_full_bag_pickup(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'target': ('item', 'GOLD_TEETH', True)}
        agent.replan_after_defeat, agent.needs_healing = False, Mock(return_value=False)
        bag = {f'item{i}': 1 for i in range(19)}
        self.assertFalse(agent.should_replan({'bag': bag}))
        self.assertTrue(agent.should_replan({'bag': {**bag, 'SAFARIBALL': 30}}))

    def test_inventory_preparation_teaches_only_compatible_tms_and_preserves_hms(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        options = agent.inventory_use_options({'bag': {'TM24': 1}, 'party': [
            {'species': 'Charizard', 'level': 60, 'hp': 100, 'moves': ['Flamethrower', 'Cut', 'Slash', 'FireSpin']},
            {'species': 'Lapras', 'level': 16, 'hp': 60, 'moves': ['WaterGun', 'Growl', 'Surf', 'Sing']}]})
        self.assertTrue(options)
        self.assertTrue(all(op.startswith('teach_tm:Tm24,Thunderbolt,1,') for op, _ in options))
        self.assertFalse(any(op.endswith(',Surf') for op, _ in options))

    def test_canonical_gate_warps_override_an_outdated_map_export(self):
        import playthrough as pt
        self.assertEqual(pt.warp_edges_from('Route22Gate', 4, 0, 'Route22'), [('Route23', 7, 139)])
        self.assertEqual(pt.warp_edges_from('Route22Gate', 4, 7, 'Route23'), [('Route22', 8, 5)])
        path = pt.bfs_cross('Route22', (8, 6), 'Route23', (7, 138), last_map='Route22')
        self.assertTrue(path)
        self.assertIn('Route22Gate', {node[0] for node, _ in path[1:]})

    def test_consumable_skill_closes_result_menus_after_exactly_one_use(self):
        game = JevGame.__new__(JevGame)
        game.d, game.judgments, game.tap = Mock(), Mock(), Mock()
        game.d.cmd.side_effect = [
            {'data': [{'item': 'RareCandy', 'qty': 1}]},
            {'data': [{'item': 'RareCandy', 'qty': 1}]},
            {'data': []}, {'data': []}, {'data': []}]
        game.st = Mock(side_effect=[
            {'field_menu': {'kind': 'party', 'phase': 'Browsing', 'cursor': 1}},
            {'field_menu': {'kind': 'party', 'phase': 'ItemUseNotice { text: "Level up" }'}},
            {'field_menu': {'kind': 'bag', 'phase': 'Browsing', 'cursor': 0}},
            {'field_menu': None}])
        game.use_consumable('RareCandy', 1)
        self.assertEqual([c.args[0] for c in game.tap.call_args_list], ['a', 'a', 'b'])

    def test_start_menu_finishes_post_battle_dialogue_before_start_input(self):
        from playthrough_late import open_start
        game = Mock()
        pending = {'screen': 'overworld', 'field_menu': None,
                   'dialogue_state': {'waiting_for_input': True}, 'script_running': True}
        menu = {'screen': 'overworld', 'field_menu': {
            'kind': 'start', 'items': ['Pokedex', 'Pokemon', 'Item'], 'cursor': 2}}
        game.st.side_effect = [pending, menu]
        open_start(game, 'Item')
        calls = [call[0] for call in game.mock_calls]
        self.assertLess(calls.index('cutscene'), calls.index('tap'))
        self.assertEqual([call.args[0] for call in game.tap.call_args_list], ['start', 'a'])

    def test_critical_main_requires_a_reachable_nurse_before_story(self):
        nurse = Rule('heal', 'VermilionPokecenter', 'nurse', [], [], [], ('heal', 'party', True), [])
        blocked = Rule('blocked', 'SaffronPokecenter', 'nurse', [], [], [], nurse.effect, [])
        def groups(reachable):
            return {'story': {'target': ('item', 'HM01', True), 'rules': []},
                    'heal': {'target': nurse.effect, 'rules': [nurse, blocked], 'context': {
                        'trigger_navigation': [{'map': nurse.map, 'tile_route_found': reachable},
                                               {'map': blocked.map, 'tile_route_found': False}]}}}
        facts = {'party': [{'species': 'Charmeleon', 'level': 30, 'hp': 14, 'max_hp': 88},
                           {'species': 'Pidgey', 'level': 9, 'hp': 27, 'max_hp': 27}]}
        offered = groups(True)
        AutonomousStoryAgent.prioritize_critical_recovery(offered, facts)
        self.assertEqual(set(offered), {'heal'})
        self.assertEqual(offered['heal']['rules'], [nurse])
        offered = groups(False)
        AutonomousStoryAgent.prioritize_critical_recovery(offered, facts)
        self.assertIn('story', offered)
        offered = groups(True)
        offered['heal']['context']['trigger_navigation'][0]['requires_surf'] = True
        offered['surf'] = {'target': ('location', ['Route10', 15, 4], True), 'rules': []}
        AutonomousStoryAgent.prioritize_critical_recovery(offered, facts)
        self.assertIn('surf', offered)
        self.assertIn('story', offered)
        facts['party'][0]['hp'] = 70
        offered = groups(True)
        AutonomousStoryAgent.prioritize_critical_recovery(offered, facts)
        self.assertIn('story', offered)

    def test_switch_training_replans_when_nonlead_main_faints(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'target': ('register', 'Fearow', True)}
        agent.replan_after_defeat = False
        agent.needs_healing = Mock(return_value=False)
        facts = {'party': [{'species': 'Spearow', 'level': 5, 'hp': 20, 'max_hp': 20},
                           {'species': 'Charizard', 'level': 36, 'hp': 0, 'max_hp': 120}]}
        self.assertTrue(agent.should_replan(facts))
        facts['party'][1]['hp'] = 120
        self.assertFalse(agent.should_replan(facts))
        agent.active['target'] = ('heal', 'party', True)
        facts['party'][1]['hp'] = 0
        self.assertFalse(agent.should_replan(facts))

    def test_forced_bike_road_is_not_a_surf_shortcut(self):
        import playthrough as pt
        from openpokered.navigation_skills import forced_bike_region
        region = forced_bike_region()
        self.assertIn(('Route18', 23, 6), region)
        self.assertNotIn(('FuchsiaCity', 5, 14), region)
        with water_planning():
            self.assertFalse(pt.walkable_edge('Route18', (23, 6), (23, 5)))
            self.assertTrue(pt.walkable_edge('PalletTown', (5, 13), (5, 14)))

    def test_push_search_solves_geometry_without_changing_the_map(self):
        import playthrough as pt
        from openpokered.boulder_skills import BOULDER_TARGETS, plan_pushes
        from openpokered.story_rules import MAPS_DIR
        name = 'VictoryRoad1F'
        flag = 'EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH'
        npcs = json.loads((MAPS_DIR / name / 'map.json').read_text())['npcs']
        rocks = {(n['x'], n['y']) for n in npcs if n['spriteName'] == 'Boulder'}
        fixed = {(n['x'], n['y']) for n in npcs if n['spriteName'] != 'Boulder'}
        blocks = list(pt.MAPS[name]['blocks'])
        plan = plan_pushes(name, (8, 16), rocks, fixed, BOULDER_TARGETS[flag]['target'])
        self.assertTrue(plan)
        player = (8, 16)
        for step in plan:
            self.assertIn(step['boulder'], rocks)
            self.assertIsNotNone(pt.bfs_cross(name, player, name, step['stance'],
                                             blocked_maps={name: rocks | fixed}, last_map='Route23'))
            self.assertNotIn(step['landing'], rocks | fixed)
            self.assertTrue(pt.walkable(name, *step['landing']))
            rocks.remove(step['boulder'])
            rocks.add(step['landing'])
            player = step['boulder']
        self.assertIn(BOULDER_TARGETS[flag]['target'], rocks)
        self.assertEqual(pt.MAPS[name]['blocks'], blocks)
        self.assertEqual(BOULDER_TARGETS['EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2']['target'], (23, 15))

    def test_current_displacement_returns_observation_and_restores_land_planning(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.actions, agent.max_actions = 0, 10
        obstacle = {'map': 'UpperCave', 'landing': ['UpperCave', 3, 9]}
        agent.active = {'context': obstacle}
        agent.field_requirements = {'Surf': obstacle}
        agent.observed_barrier_maps = set()
        agent.game, agent.record, agent.client = Mock(), Mock(), Mock()
        agent.game.st.return_value = {'player_transport': 'Surfing', 'map_name': 'LowerCave',
                                     'player_x': 20, 'player_y': 17}
        agent.game.nav_to_map.side_effect = pt.NavError('current displaced the player')
        rule = Rule('water', 'UpperCave', 'water', [], [], [], (), [])
        original = pt.walkable, pt.walkable_edge
        result = agent.execute('surf:1', rule)
        self.assertEqual(result['result'], 'blocked')
        self.assertEqual(result['position'], ['LowerCave', 20, 17])
        self.assertEqual(agent.observed_barrier_maps, {'UpperCave', 'LowerCave'})
        self.assertNotIn('Surf', agent.field_requirements)
        self.assertEqual((pt.walkable, pt.walkable_edge), original)
        self.assertFalse(agent.game.navigation_active)

    def test_intermediate_menu_knows_the_destination_not_only_the_final_effect(self):
        from openpokered.story_agent import DualStoryAgent
        agent = DualStoryAgent.__new__(DualStoryAgent)
        agent.client, agent.game = Mock(), Mock(navigation_active=True)
        agent.check_budget, agent.tap, agent.record = Mock(), Mock(), Mock()
        agent.settle_special = Mock(return_value=False)
        agent.choose = Mock(return_value='0')
        agent.navigation_intent = {'destination': 'HealingCenter'}
        agent.client.state.side_effect = [
            {'screen': 'overworld', 'map_name': 'Gate', 'choice': {'options': ['YES', 'NO'], 'selected': 0}},
            {'screen': 'overworld', 'map_name': 'Gate'}]
        agent.client.observe.return_value = {'mode': 'overworld'}
        agent.settle(('heal', 'party', True))
        offered = agent.choose.call_args.args[1]
        self.assertEqual(offered['travel_in_progress']['destination'], 'HealingCenter')
        self.assertEqual(offered['current_map'], 'Gate')

    def test_settle_dismisses_the_post_catch_pokedex_screen(self):
        from openpokered.story_agent import DualStoryAgent
        agent = DualStoryAgent.__new__(DualStoryAgent)
        agent.client = Mock()
        agent.check_budget, agent.tap, agent.record = Mock(), Mock(), Mock()
        agent.settle_special = Mock(return_value=False)
        settled = {'screen': 'overworld'}
        agent.client.state.side_effect = [
            {'screen': 'pokedex', 'active_script_effect': None},
            {'screen': 'pokedex', 'active_script_effect': None},
            settled, settled, settled]
        agent.client.observe.return_value = {'mode': 'overworld'}
        agent.settle(('dex', 'count', 10))
        self.assertEqual([c.args[0] for c in agent.tap.call_args_list], ['b', 'b'])

    def battle_state(self, **live_overrides):
        live = {'is_wild': True, 'is_safari': False,
                'enemy': {'species': 'Caterpie', 'level': 3, 'hp': 16, 'max_hp': 16, 'status': 'None'},
                'player': {'species': 'Charmander'},
                'player_party': [{'species': 'Charmander', 'level': 12, 'hp': 30, 'max_hp': 30,
                                  'status': 'None', 'moves': ['Scratch', 'None'], 'pp': [35, 0]}]}
        live.update(live_overrides)
        return {'battle_live': live,
                'party': [{'species': 'Charmander', 'level': 12, 'hp': 30, 'max_hp': 30, 'status': 'None'}],
                'battle_inventory': [{'item': 'PokeBall', 'qty': 5}],
                'pokedex': {'owned_species': ['Pidgey']}}

    def test_wild_encounter_offers_balls_and_honours_the_chosen_one(self):
        from openpokered.playthrough_judgments import ball_options, JevGame
        state = self.battle_state()
        offered = list(ball_options(state['battle_live'], {'PokeBall': 5, 'Potion': 2}, ['Pidgey']))
        self.assertEqual([name for name, _t, _d in offered], ['PokeBall'])
        details = offered[0][2]
        self.assertEqual((details['catch_rate'], details['already_owned'], details['quantity']),
                         (255, False, 5))
        self.assertEqual((details['capture_probability_now'], details['hp_percent']),
                         (0.3359, 100.0))
        agent = JevGame.__new__(JevGame)
        agent.judgments = Mock()
        agent.judgments.choose.return_value = 'ball:PokeBall'
        self.assertEqual(JevGame.battle_recovery_plan(agent, state), ('PokeBall', None))
        self.assertIn('ball:PokeBall', agent.judgments.choose.call_args.args[2])

    def test_capture_status_uses_primary_rules_not_damage_type_chart(self):
        from openpokered.playthrough_judgments import capture_status_options
        for move, species in [('Sing', 'Gastly'), ('Glare', 'Gastly'),
                              ('Glare', 'Snorlax'), ('StunSpore', 'Bulbasaur'),
                              ('Hypnosis', 'Abra'), ('ThunderWave', 'Pikachu'),
                              ('ThunderWave', 'Zapdos'), ('StunSpore', 'Onix')]:
            with self.subTest(move=move, species=species):
                options = capture_status_options({'moves': [move], 'pp': [1]},
                                                 {'species': species, 'status': 'None'}, {})
                self.assertEqual(len(options), 1)
                self.assertEqual(options[0]['effectiveness'], 1)
        for species in ('Onix', 'Nidoking'):
            self.assertEqual(capture_status_options({'moves': ['ThunderWave'], 'pp': [20]},
                                                    {'species': species}, {}), [])

    def test_transform_keeps_capture_identity_and_original_catch_rate(self):
        from types import SimpleNamespace
        from openpokered.playthrough_judgments import ball_options, capture_intent, capture_status_options, JevGame
        state = self.battle_state()
        state['battle_live']['enemy'].update(species='Gloom', capture_species='Ditto',
                                           capture_catch_rate=35)
        state['pokedex'] = {'owned_species': ['Gloom']}
        judgments = SimpleNamespace(collects_dex=True, active=None)
        self.assertTrue(capture_intent(state, judgments))
        offered = list(ball_options(state['battle_live'], {'PokeBall': 5}, ['Gloom']))
        details = offered[0][2]
        self.assertEqual((details['capture_species'], details['catch_rate'], details['already_owned']),
                         ('Ditto', 35, False))
        mon = {'moves': ['SleepPowder'], 'pp': [15]}
        status = capture_status_options(mon, state['battle_live']['enemy'], {'PokeBall': 5})[0]
        from openpokered.playthrough_judgments import capture_probability
        expected = capture_probability('PokeBall', {**state['battle_live']['enemy'],
                                                   'status': 'Sleep(2)', 'catch_rate': 35})
        self.assertEqual(status['capture_probability_if_status_lands']['PokeBall'], expected)
        game = JevGame.__new__(JevGame)
        game.judgments = Mock(collects_dex=True, active=None)
        game.judgments.choose.return_value = 'ball:PokeBall'
        self.assertEqual(game.battle_recovery_plan(state), ('PokeBall', None))
        options = game.judgments.choose.call_args.args[2]
        self.assertIn('Prepare capture', options['fight'])
        self.assertFalse(json.loads(options['ball:PokeBall'])['already_owned'])
        state['pokedex']['owned_species'].append('Ditto')
        self.assertFalse(capture_intent(state, judgments))
        judgments.active = {'context': {'required_capture_species': 'Ditto'}}
        self.assertTrue(capture_intent(state, judgments))

    def test_capture_probability_reflects_hp_status_and_gen1_ball_formula(self):
        from openpokered.playthrough_judgments import capture_probability
        enemy = {'hp': 16, 'max_hp': 16, 'catch_rate': 45, 'status': 'None'}
        full = capture_probability('PokeBall', enemy)
        sleeping = capture_probability('PokeBall', {**enemy, 'status': 'Sleep'})
        weakened = capture_probability('PokeBall', {**enemy, 'hp': 1})
        ultra = capture_probability('UltraBall', enemy)
        safari = capture_probability('SafariBall', enemy)
        self.assertLess(full, sleeping)
        self.assertLess(full, weakened)
        self.assertGreater(ultra, full)
        self.assertEqual(safari, ultra)
        self.assertEqual(capture_probability('MasterBall', enemy), 1.0)

    def test_collecting_frames_the_ball_choice_around_the_goal(self):
        from openpokered.playthrough_judgments import JevGame
        state = self.battle_state()
        def instruction(collects):
            agent = JevGame.__new__(JevGame)
            agent.judgments = Mock()
            agent.judgments.collects_dex = collects
            agent.judgments.choose.return_value = 'fight'
            JevGame.battle_recovery_plan(agent, state)
            return agent.judgments.choose.call_args.args[3]
        phrase = 'register species that are not in the Pokédex yet'
        self.assertIn(phrase, instruction(True))
        self.assertNotIn(phrase, instruction(False))

    def capture_support_state(self):
        state = self.battle_state()
        state['battle_live']['enemy'] = {'species': 'Snorlax', 'level': 30,
            'hp': 139, 'max_hp': 139, 'status': 'None'}
        support = {'species': 'Gloom', 'level': 21, 'hp': 63, 'max_hp': 63,
                   'status': 'None', 'moves': ['SleepPowder', 'Poisonpowder'], 'pp': [15, 35]}
        state['party'].append(support)
        state['battle_live']['player_party'].append(support)
        return state

    def test_capture_offers_switch_to_status_only_support(self):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.collects_dex = True
        game.judgments.active = {'context': {'acquisition_method': 'static'}}
        game.judgments.choose.return_value = 'switch:1'
        self.assertEqual(game.battle_recovery_plan(self.capture_support_state()), ('switch', 1))
        details = json.loads(game.judgments.choose.call_args.args[2]['switch:1'])
        self.assertEqual(details['capture_status_options'][0]['move'], 'SleepPowder')
        self.assertEqual(details['usable_effective_attacks'], [])
        self.assertIn('Switching and setup cost turns', game.judgments.choose.call_args.args[3])

    def test_capture_fight_exposes_current_support_as_clearly_as_switches(self):
        state = self.capture_support_state()
        state['battle_live']['player'] = {'species': 'Gloom'}
        game = JevGame.__new__(JevGame)
        game.judgments = Mock(collects_dex=True, active=None, preference='none')
        game.judgments.choose.return_value = 'fight'
        self.assertIsNone(game.battle_recovery_plan(state))
        candidates = game.judgments.choose.call_args.args[2]
        fight = json.loads(candidates['fight'])
        self.assertEqual(fight['active_party_index'], 1)
        self.assertEqual(fight['active_pokemon']['species'], 'Gloom')
        self.assertEqual(fight['capture_status_options'][0]['move'], 'SleepPowder')
        self.assertEqual(fight['capture_status_options'][0]['pp'], 15)
        self.assertIn('PokeBall', fight['capture_status_options'][0]['capture_probability_if_status_lands'])
        self.assertEqual(fight['usable_effective_attacks'], [])
        self.assertEqual(set(candidates), {'fight', 'switch:0', 'ball:PokeBall'})
        self.assertIn('without switching', fight['reason'])
        self.assertIn('does not apply', game.judgments.choose.call_args.args[3])

    def test_capture_turn_economy_preserves_choices_and_separates_first_throw(self):
        state = self.capture_support_state()
        game = JevGame.__new__(JevGame)
        game.judgments = Mock(collects_dex=True, active=None, preference='none')
        game.judgments.choose.return_value = 'switch:1'
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 1))
        _, evidence, candidates, instruction = game.judgments.choose.call_args.args
        timing = evidence['capture_turn_economy']
        self.assertEqual(timing['throw_now']['enemy_response_opportunities_before_throw'], 0)
        self.assertEqual(timing['active_move_then_throw']['enemy_response_opportunities_before_throw'], 1)
        self.assertEqual(timing['switch_then_move_then_throw']['enemy_response_opportunities_before_throw'], 2)
        self.assertEqual(timing['switch_then_move_then_throw']['earliest_ball_turn'], 3)
        self.assertIn('speed', timing['scope'])
        self.assertIn('status', timing['scope'])
        self.assertIn('conditional', instruction)
        self.assertEqual(set(candidates), {'fight', 'switch:1', 'ball:PokeBall'})
        game.judgments.collects_dex = False
        game.battle_recovery_plan(state)
        self.assertNotIn('capture_turn_economy', game.judgments.choose.call_args.args[1])

    def test_capture_fight_capabilities_refresh_with_pp_status_and_active_member(self):
        state = self.capture_support_state()
        state['battle_live']['player'] = {'species': 'Gloom'}
        state['battle_live']['player_party'][1].update(moves=['SleepPowder', 'Absorb'], pp=[0, 25])
        game = JevGame.__new__(JevGame)
        game.judgments = Mock(collects_dex=True, active=None, preference='none')
        game.judgments.choose.return_value = 'fight'
        for status, pp, expected in [('None', 0, []), ('Paralysis', 15, []), ('None', 15, ['SleepPowder'])]:
            with self.subTest(status=status, pp=pp):
                state['battle_live']['enemy']['status'] = status
                state['battle_live']['player_party'][1]['pp'][0] = pp
                game.battle_recovery_plan(state)
                fight = json.loads(game.judgments.choose.call_args.args[2]['fight'])
                self.assertEqual([row['move'] for row in fight['capture_status_options']], expected)
                self.assertEqual(fight['usable_effective_attacks'], ['Absorb'])
        state['battle_live']['player'] = {'species': 'Charmander'}
        game.battle_recovery_plan(state)
        fight = json.loads(game.judgments.choose.call_args.args[2]['fight'])
        self.assertEqual(fight['active_party_index'], 0)
        self.assertEqual(fight['capture_status_options'], [])
        self.assertEqual(fight['usable_effective_attacks'], ['Scratch'])
        # Ordinary non-collection combat retains the old attack contract.
        game.judgments.collects_dex = False
        game.battle_recovery_plan(state)
        self.assertEqual(game.judgments.choose.call_args.args[2]['fight'],
                         'Attack this turn; preserve recovery supplies')

    def test_capture_retreat_binds_run_only_for_verified_scripted_source(self):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.collects_dex = True
        game.judgments.choose.return_value = 'run'
        state = self.capture_support_state()
        with patch('openpokered.playthrough_judgments.capture_retreat',
                   return_value={'purpose': 'preserve verified source'}):
            self.assertEqual(game.battle_recovery_plan(state), ('run', None))
            state['battle_inventory'] = []
            self.assertEqual(game.battle_recovery_plan(state), ('run', None))
        question = game.judgments.choose.call_args
        self.assertIn('run', question.args[2])
        self.assertIn('inventory_failure_at_current_state', question.args[1])
        self.assertIn('capture_threat', question.args[1])
        from openpokered.playthrough_judgments import capture_retreat
        self.assertIsNone(capture_retreat(state, game.judgments))

    def test_capture_menu_offers_only_usable_safe_status_and_updates_after_landing(self):
        from openpokered.playthrough_judgments import capture_move_question
        state = self.capture_support_state()
        support = state['battle_live']['player_party'][1]
        state['battle_live']['player'] = support
        menu = {'cursor': 0, 'moves': [
            {'move': 'SleepPowder', 'pp': 15, 'disabled': False},
            {'move': 'Poisonpowder', 'pp': 35, 'disabled': False}]}
        compact, choices = capture_move_question(state, menu)
        self.assertEqual(choices, {'0': 'SleepPowder'})
        projected = compact['moves']['0']['capture_probability_if_status_lands']['PokeBall']
        self.assertGreater(projected, compact['available_balls'][0][2]['capture_probability_now'])
        menu['moves'][0]['disabled'] = True
        self.assertEqual(capture_move_question(state, menu)[1], {})
        menu['moves'][0].update(disabled=False, pp=0)
        self.assertEqual(capture_move_question(state, menu)[1], {})
        menu['moves'][0]['pp'] = 15
        state['battle_live']['enemy']['status'] = 'Sleep(1)'
        self.assertEqual(capture_move_question(state, menu)[1], {})

    def test_capture_menu_preserves_native_hit_ranges_and_unknown_is_not_zero(self):
        from openpokered.playthrough_judgments import capture_move_question
        state = self.capture_support_state()
        state['battle_live']['player'] = state['party'][0]
        preview = {'normal_damage': [25, 30], 'critical_damage': [50, 59],
                   'critical_threshold': 50, 'target_hp': 139, 'direct_hit_can_ko': False}
        menu = {'moves': [
            {'move': 'Cut', 'pp': 20, 'disabled': False, 'direct_hit_preview': preview},
            {'move': 'Dig', 'pp': 10, 'disabled': False},
            {'move': 'Tackle', 'pp': 0, 'disabled': False, 'direct_hit_preview': preview}]}
        compact, choices = capture_move_question(state, menu)
        self.assertEqual(choices, {'0': 'Cut', '1': 'Dig'})
        self.assertEqual(compact['moves']['0']['direct_hit_preview'], preview)
        self.assertIsNone(compact['moves']['1']['direct_hit_preview'])
        self.assertIn('Not a whole-turn safety guarantee', compact['direct_hit_preview_scope'])
        self.assertIn('NOT zero damage', compact['direct_hit_preview_scope'])
        old_key = json.dumps(compact, sort_keys=True)
        preview['normal_damage'] = [40, 48]
        new_compact, _ = capture_move_question(state, menu)
        self.assertNotEqual(old_key, json.dumps(new_compact, sort_keys=True))

    def test_real_move_selector_drives_the_judged_capture_status_slot(self):
        game = JevGame.__new__(JevGame)
        state = self.capture_support_state()
        support = state['battle_live']['player_party'][1]
        state['battle_live']['player'] = support
        state.update(screen='battle', battle_phase='MoveSelect', battle_moves={
            'cursor': 0, 'moves': [{'move': 'SleepPowder', 'pp': 15, 'disabled': False}]})
        game.st = Mock(return_value=state)
        game.tap, game.step = Mock(), Mock()
        game.judgments = Mock()
        game.judgments.collects_dex = True
        game.judgments.active = {'context': {}}
        game.judgments.choose.return_value = '0'
        game.move_cache, game.move_cache_hits, game.active_milestone = {}, 0, None
        game._select_move()
        game.tap.assert_called_once_with('a', 4)
        self.assertEqual(game.judgments.choose.call_args.args[2]['0'], 'SleepPowder')
        self.assertIn('back', game.judgments.choose.call_args.args[2])
        self.assertIn('without knocking out', game.judgments.choose.call_args.args[3])
        state['battle_live']['enemy']['status'] = 'Sleep(1)'
        game.tap.reset_mock()
        game.judgments.choose.reset_mock()
        game._select_move()
        game.tap.assert_called_once_with('b', 4)
        game.judgments.choose.assert_not_called()

    def test_capture_move_abstention_returns_to_menu_without_attacking(self):
        game = JevGame.__new__(JevGame)
        state = self.capture_support_state()
        state['battle_live']['player'].update(hp=50, max_hp=50, level=20)
        state.update(screen='battle', battle_phase='MoveSelect', battle_moves={
            'cursor': 0, 'moves': [{'move': 'Cut', 'pp': 30, 'disabled': False}]})
        game.st = Mock(return_value=state)
        game.tap, game.step = Mock(), Mock()
        game.judgments = Mock()
        game.judgments.collects_dex = True
        game.judgments.active = {'context': {}}
        game.judgments.choose.side_effect = StoryStopped('action:no_selection')
        game.move_cache, game.move_cache_hits, game.active_milestone = {}, 0, None
        game._select_move()
        game.tap.assert_called_once_with('b', 4)
        game.judgments.choose.side_effect = None
        game.judgments.choose.return_value = 'ball:PokeBall'
        self.assertEqual(game.battle_recovery_plan(state), ('PokeBall', None))
        self.assertNotIn('fight', game.judgments.choose.call_args.args[2])

    def test_retryable_source_without_capacity_never_becomes_attack_goal(self):
        from openpokered.playthrough_judgments import capture_intent
        for full_storage in (False, True):
            with self.subTest(full_storage=full_storage):
                game = JevGame.__new__(JevGame)
                state = self.capture_support_state()
                if full_storage:
                    state['battle_live']['capture_blocked_reason'] = 'storage_full'
                else:
                    state['battle_inventory'] = []
                state.update(screen='battle', battle_phase='MoveSelect', battle_moves={
                    'cursor': 0, 'moves': [{'move': 'Flamethrower', 'pp': 15, 'disabled': False}]})
                game.st = Mock(return_value=state)
                game.tap, game.step = Mock(), Mock()
                game.judgments = Mock(collects_dex=True, active={'context': {}})
                game.judgments.choose.return_value = 'run'
                with patch('openpokered.playthrough_judgments.capture_retreat',
                           return_value={'contracts': [], 'purpose': 'preserve source'}):
                    self.assertTrue(capture_intent(state, game.judgments))
                    game._select_move()
                    game.tap.assert_called_once_with('b', 4)
                    game.judgments.choose.assert_not_called()
                    self.assertEqual(game.battle_recovery_plan(state), ('run', None))
                    self.assertNotIn('fight', game.judgments.choose.call_args.args[2])
                    self.assertIn('capture is impossible', game.judgments.choose.call_args.args[3])

    def test_balls_are_never_offered_against_a_trainer_or_in_the_safari_zone(self):
        from openpokered.playthrough_judgments import ball_options, JevGame
        self.assertEqual(list(ball_options({'is_wild': False, 'enemy': {'species': 'Caterpie'}}, {'PokeBall': 5})), [])
        self.assertEqual(list(ball_options({'is_wild': True, 'is_safari': True,
                                           'enemy': {'species': 'Caterpie'}}, {'PokeBall': 5})), [])
        for live in ({'is_wild': False}, {'is_wild': True, 'is_safari': True}):
            state = self.battle_state(**live)
            agent = JevGame.__new__(JevGame)
            agent.judgments = Mock()
            # No ball and no useful medicine leaves nothing to spend the turn
            # on, so the plan stays None and the judge is not even consulted.
            self.assertIsNone(JevGame.battle_recovery_plan(agent, state))
            self.assertFalse(agent.judgments.choose.called)

    def test_identified_restless_soul_is_not_a_capture_target(self):
        from openpokered.playthrough_judgments import ball_options, capture_intent
        state = self.capture_support_state()
        state['battle_live'].update(is_ghost=False, capture_blocked_reason='restless_soul')
        judge = Mock(collects_dex=True, active={'context': {}})
        self.assertEqual(list(ball_options(state['battle_live'], {'MasterBall': 1})), [])
        self.assertFalse(capture_intent(state, judge))

    def test_safari_action_exposes_capture_flee_and_ball_factors(self):
        from openpokered.playthrough_judgments import safari_action_options, JevGame
        state = self.battle_state(is_safari=True)
        state['battle_live']['safari'] = {
            'base_catch_rate': 45, 'catch_rate': 45, 'bait_factor': 0,
            'escape_factor': 0, 'balls': 9, 'enemy_speed': 60,
        }
        options = safari_action_options(state['battle_live'], state['pokedex']['owned_species'])
        self.assertEqual(set(options), {'ball', 'bait', 'rock', 'run'})
        self.assertGreater(options['rock']['projected_catch_probability'],
                           options['ball']['capture_probability_now'])
        self.assertEqual(options['ball']['capture_probability_now'], 0.1023)
        self.assertEqual(options['rock']['duration_turns_uniform'], [1, 5])
        self.assertGreater(options['rock']['projected_flee_probability_next_turn'],
                           options['ball']['flee_probability_if_not_caught'])
        self.assertLess(options['bait']['projected_flee_probability_next_turn'],
                        options['ball']['flee_probability_if_not_caught'])
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.choose.return_value = 'ball'
        self.assertEqual(game.safari_battle_action(state), 'ball')
        self.assertIn('capture probability', game.judgments.choose.call_args.args[3])

    def test_native_door_interaction_reaches_the_corridor_side(self):
        import playthrough as pt
        from types import SimpleNamespace
        from openpokered.story_agent import DualStoryAgent
        from openpokered.story_rules import MAPS_DIR
        from openpokered.autonomous_story import NATIVE_INTERACTIONS
        name = 'SilphCo11F'
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        rule = Rule('door', name, name + ':cardKeyDoor', ['coord:(6,12)'], [], [],
                    ('flag', 'UNLOCKED', True), [])
        agent.index = SimpleNamespace(coordinates=lambda _: [(6, 12)])
        agent.active = {'target': rule.effect, 'rules': [rule]}
        agent.client = Mock()
        agent.client.cmd.return_value = []
        blocks = list(pt.MAPS[name]['blocks'])
        blocks[6*pt.MAPS[name]['width']+3] = 32
        facts = {'map': name, 'x': 3, 'y': 12}
        with patch.dict(pt.MAPS[name], {'blocks': blocks}):
            self.assertIn((6, 14), agent.destination_points(name, rule))
            with patch.object(DualStoryAgent, 'action_candidates', return_value=({}, {})):
                _, bindings = agent.action_candidates(facts)
            self.assertIn('interact_tile:6,13', [op for op, _ in bindings.values()])
        # Other floors only declare OnStep: do not invent A-button bindings.
        self.assertNotIn('SilphCo7F:cardKeyDoor1', NATIVE_INTERACTIONS)

    def test_door_approach_does_not_offer_a_zero_step_trip_instead_of_interaction(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'SilphCo3F'
        rule = Rule('door', name, name + ':cardKeyDoor2', [], [], [],
                    ('flag', 'UNLOCKED', True), [])
        agent.active = {'target': rule.effect, 'rules': [rule]}
        agent.client, agent.index = Mock(), Mock()
        agent.client.cmd.return_value = []
        agent.client.route.return_value = {'legs': []}
        agent.visited = {name}
        agent.destination_points = Mock(return_value=[(18, 8)])
        facts = {'map': name, 'x': 18, 'y': 8}
        def initial(_facts):
            return {'a': json.dumps({'operation': 'move_to:16,8'})}, {'a': ('move_to:16,8', rule)}
        def path(name, start, end, **kwargs):
            return [start, end] if end == (18, 8) else None
        with patch.object(DualStoryAgent, 'action_candidates', side_effect=initial), \
                patch.object(pt, 'bfs', side_effect=path), patch.object(pt, 'walkable', return_value=True):
            _, bindings = agent.action_candidates(facts)
            self.assertEqual([op for op, _ in bindings.values()], ['interact_tile:17,8'])
            # Before arriving at the approach tile the trip is not a no-op.
            _, bindings = agent.action_candidates({**facts, 'x': 19})
            self.assertIn('travel_to:SilphCo3F', [op for op, _ in bindings.values()])
        with patch.object(DualStoryAgent, 'action_candidates', side_effect=initial), \
                patch.object(pt, 'bfs', return_value=None), patch.object(pt, 'walkable', return_value=True):
            _, bindings = agent.action_candidates(facts)
            self.assertEqual([op for op, _ in bindings.values()], ['travel_to:SilphCo3F'])

    def test_recovery_does_not_forget_a_healer_behind_a_locked_door(self):
        from types import SimpleNamespace
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        near = Rule('near', 'LockedRoom', 'Room:heal', ['npc:1'], [], [], ('heal', 'party', True), [])
        far = Rule('far', 'Center', 'Center:heal', ['npc:1'], [], [], ('heal', 'party', True), [])
        agent.index = SimpleNamespace(by_effect={near.effect: [near, far]})
        agent.client = Mock()
        agent.client.route.side_effect = lambda start, dest: {'found': True, 'legs': [0] * (1 if dest == 'LockedRoom' else 3)}
        agent.navigation_memory = {'LockedRoom': {'detail': 'locked'}}
        agent.game = Mock()
        agent.destination_points = Mock(return_value=[(3, 3)])
        facts = {'map': 'Hallway', 'x': 1, 'y': 1}
        with patch('openpokered.autonomous_story.pt.bfs_cross', return_value=None):
            self.assertEqual(agent.nearby_healers(facts), [far])
        with patch('openpokered.autonomous_story.pt.bfs_cross', return_value=[('Hallway', 1, 1)]):
            self.assertEqual(agent.nearby_healers(facts), [near, far])

    def test_possible_battle_loss_does_not_seal_off_the_challenge_trigger(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.observed_barrier_maps = {'Tower'}
        pushback = Rule('loss', 'Tower', 'Tower:ghost', ['coord:(10,16)'], [], [],
                        ('movement', 'movePlayerRelative', True), [('battle', 'Marowak', True)])
        agent.index = Mock(rules=[pushback])
        agent.index.coordinates.return_value = [(10, 16)]
        self.assertEqual(agent.observed_navigation_barriers({'flags': {}}), {})

    def test_declining_an_affordable_entrance_does_not_make_its_trigger_a_wall(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.observed_barrier_maps = {'Gate'}
        refusal = Rule('refuse', 'Gate', 'Gate:entry', ['coord:(3,2)'], [], ['NO'],
                       ('movement', 'movePlayerRelative', True), [])
        payment = {'Call': {'callee': 'hasMoney', 'args': [{'NumberLit': 500}]}}
        entrance = Rule('pay', 'Gate', 'Gate:entry', ['coord:(3,2)'], [(payment, True)], ['YES'],
                        ('transport', ('Park', 14, 25), True), [])
        agent.index = Mock(rules=[refusal, entrance])
        agent.index.coordinates.return_value = [(3, 2)]
        self.assertEqual(agent.observed_navigation_barriers({'money': 500}), {})
        self.assertEqual(agent.observed_navigation_barriers({'money': 499}), {'Gate': {(3, 2)}})
        # Another transport in the building cannot open this guarded trigger.
        entrance.storyline = 'Gate:unrelated'
        self.assertEqual(agent.observed_navigation_barriers({'money': 500}), {'Gate': {(3, 2)}})

    def test_exit_confirmation_can_disable_its_own_movement_guard(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.observed_barrier_maps = {'Gate'}
        guard = {'Call': {'callee': 'getFlag', 'args': [{'StringLit': 'INSIDE'}]}}
        movements = [Rule(choice, 'Gate', 'Gate:exit', ['coord:(3,2)'], [(guard, True)], [choice],
                          ('movement', 'movePlayerRelative', True), []) for choice in ('YES', 'NO')]
        leave = Rule('leave', 'Gate', 'Gate:exit', ['coord:(3,2)'], [(guard, True)], ['YES'],
                     ('flag', 'INSIDE', False), [])
        agent.index = Mock(rules=[*movements, leave])
        agent.index.coordinates.return_value = [(3, 2)]
        self.assertEqual(agent.observed_navigation_barriers({'flags': {'INSIDE': True}}), {})
        # An unrelated flag write does not remove the active guard.
        leave.effect = ('flag', 'UNRELATED', False)
        self.assertEqual(agent.observed_navigation_barriers({'flags': {'INSIDE': True}}),
                         {'Gate': {(3, 2)}})

    def test_npc_approach_reassesses_after_an_interrupting_battle_before_talking(self):
        from openpokered.playthrough_judgments import NavigationPause
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.actions, agent.max_actions = 0, 10
        agent.game = Mock()
        agent.game.approach_object.side_effect = NavigationPause('trainer interrupted')
        agent.client = Mock()
        agent.client.cmd.return_value = [{'npc_index': 0, 'x': 25, 'y': 3, 'visible': True}]
        agent.tap, agent.settle, agent.record = Mock(), Mock(), Mock()
        rule = Rule('boss', 'Base', 'Base:boss', ['npc:1'], [], [], ('flag', 'WON', True), [])
        agent.active = {'target': rule.effect}
        result = agent.execute('interact_with:npc:0', rule)
        self.assertEqual(result['result'], 'paused_after_battle')
        agent.tap.assert_not_called()
        self.assertFalse(agent.game.navigation_active)
        agent.settle.assert_called_once_with(rule.effect, rule)
        agent.game.approach_object.side_effect = None
        self.assertEqual(agent.execute('interact_with:npc:0', rule)['result'], 'interacted_with_npc')
        agent.tap.assert_called_once_with('a')

    def test_unreachable_region_offers_transport_without_an_old_failed_trip(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock()
        agent.game.navigation_barriers.return_value = {}
        agent.game.navigation_excluded_maps.return_value = ()
        elevator = Rule('panel', 'RocketHideoutElevator', 'Elevator:panel', ['sign:1'], [], ['B4F'],
                        ('transport', ('RocketHideoutB4F', 25, 15), True), [])
        agent.index = Mock(rules=[elevator])
        agent.index.frontier.return_value = [elevator]
        groups = {'boss': {'rules': [], 'context': {'trigger_navigation': [
            {'map': 'RocketHideoutB4F', 'tile_route_found': False}]}}}
        self.assertTrue(agent.transport_frontiers(groups, {}))
        option = next(g for k, g in groups.items() if k != 'boss')
        self.assertEqual(option['target'], elevator.effect)
        agent.index.frontier.assert_called_once_with(elevator.effect, {})

    def test_scripted_entrance_can_lead_to_a_target_on_another_map(self):
        from types import SimpleNamespace
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        destination = 'SafariZoneSecretHouse'
        agent.maps = {destination: json.loads((MAPS_DIR / destination / 'map.json').read_text())}
        reward = Rule('reward', destination, destination + ':reward', ['npc:1'], [], [],
                      ('item', 'HM03', True), [])
        entrance = Rule('entrance', 'SafariZoneGate', 'Gate:pay', ['npc:1'], [], ['YES'],
                        ('transport', ('SafariZoneCenter', 14, 25), True), [])
        agent.index = SimpleNamespace(rules=[entrance], coordinates=lambda _: [],
                                      frontier=Mock(return_value=[entrance]))
        agent.game = Mock(last_map='FuchsiaCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.navigation_excluded_maps.return_value = ()
        groups = {'reward': {'rules': [reward], 'context': {'trigger_navigation': [
            {'map': destination, 'tile_route_found': False}]}}}
        self.assertTrue(agent.transport_frontiers(groups, {}))
        agent.index.frontier.assert_called_once_with(entrance.effect, {})
        self.assertIn(json.dumps(entrance.effect), groups)
        agent.game.nav_to_map.assert_not_called()

    def test_transport_to_same_floor_must_reach_the_requested_region(self):
        from types import SimpleNamespace
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'RocketHideoutB4F'
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        # The stairs/key area (19,9) is disconnected from the boss wing.
        transport = Rule('panel', 'RocketHideoutElevator', 'Elevator:panel', ['sign:1'], [], [],
                         ('transport', (name, 19, 9), True), [])
        boss = Rule('boss', name, name + ':boss', ['npc:1'], [], [], ('flag', 'BOSS', True), [])
        agent.index = SimpleNamespace(rules=[transport], coordinates=lambda _: [],
                                      frontier=Mock(return_value=[transport]))
        agent.game = Mock(last_map='CeladonCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.navigation_excluded_maps.return_value = ()
        groups = {'boss': {'rules': [boss], 'context': {'trigger_navigation': [
            {'map': name, 'tile_route_found': False}]}}}
        self.assertFalse(agent.transport_frontiers(groups, {}))
        agent.index.frontier.assert_not_called()

    def test_navigation_preview_distinguishes_regions_on_the_same_map(self):
        from types import SimpleNamespace
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'RocketHideoutB4F'
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        agent.index = SimpleNamespace(rules=[], coordinates=lambda _: [])
        agent.observed_barrier_maps = set()
        agent.navigation_memory = {name: {'goal': ['flag', 'BOSS', True]}}
        agent.game = Mock(last_map='CeladonCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        groups = {label: {'rules': [Rule(label, name, name + ':' + label, [f'npc:{npc}'],
                                        [], [], ('flag', label, True), [])]}
                  for label, npc in [('key', 4), ('boss', 1)]}
        groups['mixed'] = {'rules': [*groups['key']['rules'], *groups['boss']['rules']]}
        agent.annotate_navigation(groups, {'map': name, 'x': 19, 'y': 9, 'flags': {}})
        self.assertTrue(groups['key']['context']['trigger_navigation'][0]['tile_route_found'])
        self.assertNotIn('boss', groups)  # Its known failure must not suppress the reachable key region.
        self.assertEqual([rule.id for rule in groups['mixed']['rules']], ['key'])
        self.assertEqual(groups['mixed']['context']['deferred_trigger_maps'], [name])
        agent.game.nav_to_map.assert_not_called()

    def test_deferred_goal_keeps_reachable_cut_prerequisite_without_execution(self):
        from types import SimpleNamespace
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.navigation_memory = {'Gym': {}}
        agent.game = Mock(last_map='Road')
        barriers = {'Road': {(9, 9)}}
        agent.game.navigation_barriers.return_value = barriers
        agent.game.live_npcs.return_value = {(3, 3)}
        agent.game.navigation_excluded_maps.return_value = ('ClosedCity',)
        agent.destination_points = Mock(return_value=[(1, 1)])
        goal = ('flag', 'BADGE', True)
        rule = Rule('leader', 'Gym', 'Gym:leader', [], [], [], goal, [])
        def groups():
            return {'boss': {'target': goal, 'rules': [rule], 'context': {
                'trigger_navigation': [{'map': 'Gym', 'tile_route_found': False}]}},
                'other': {'target': ('item', 'TM', True), 'rules': [rule], 'context': {
                'trigger_navigation': [{'map': 'Gym', 'tile_route_found': False}]}}}
        facts = {'map': 'Road', 'x': 2, 'y': 3, 'flags': {'EVENT_BEAT_MISTY': True},
                 'party': [{'moves': ['Cut'], 'hp': 10}]}
        obstacle = {'move': 'Cut', 'map': 'City', 'tree': [4, 5],
                    'stance': [4, 6], 'direction': 'up', 'destination': 'Gym'}
        with patch('openpokered.autonomous_story.cut_requirement', return_value=obstacle) as probe:
            offered = groups()
            agent.add_cut_route_frontiers(offered, facts)
            added = offered['field:City,4,5']
            self.assertEqual(added['target'], ('terrain', 'City,4,5', True))
            self.assertEqual(added['context']['prerequisite_for_goals'], [goal, ('item', 'TM', True)])
            self.assertEqual(probe.call_count, 1)
            self.assertEqual(probe.call_args.args[-2], {'Road': {(9, 9), (3, 3)}})
            self.assertEqual(probe.call_args.args[-1], ('ClosedCity',))
            self.assertEqual(barriers, {'Road': {(9, 9)}})
            agent.active = added
            candidates, bindings = agent.action_candidates(facts)
            self.assertEqual([value[0] for value in bindings.values()], ['cut:0'])
            agent.game.nav_to_map.assert_not_called()
            for missing in [dict(facts, flags={}), dict(facts, party=[{'moves': ['Cut'], 'hp': 0}]),
                            dict(facts, party=[{'moves': ['Tackle'], 'hp': 10}])]:
                offered = groups()
                agent.add_cut_route_frontiers(offered, missing)
                self.assertNotIn('field:City,4,5', offered)
            probe.assert_called_once()
        with patch('openpokered.autonomous_story.cut_requirement', return_value=None):
            offered = groups()
            agent.add_cut_route_frontiers(offered, facts)
            self.assertEqual(set(offered), {'boss', 'other'})

    def test_cut_frontier_uses_real_regrown_celadon_gym_tree(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.navigation_memory = {'CeladonGym': {}}
        agent.game = Mock(last_map='CeladonCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ('SaffronCity',)
        agent.destination_points = Mock(return_value=[(4, 5)])
        goal = ('flag', 'EVENT_BEAT_ERIKA', True)
        rule = Rule('leader', 'CeladonGym', 'CeladonGym:leader', [], [], [], goal, [])
        groups = {'boss': {'target': goal, 'rules': [rule], 'context': {
            'trigger_navigation': [{'map': 'CeladonGym', 'tile_route_found': False}]}}}
        original = {name: row['blocks'] for name, row in pt.MAPS.items()}
        facts = {'map': 'CeladonCity', 'x': 20, 'y': 17,
                 'flags': {'EVENT_BEAT_MISTY': True}, 'party': [{'moves': ['Cut'], 'hp': 10}]}
        agent.add_cut_route_frontiers(groups, facts)
        added = [value for key, value in groups.items() if key.startswith('field:')]
        self.assertEqual(len(added), 1)
        self.assertEqual(added[0]['context']['map'], 'CeladonCity')
        self.assertEqual(added[0]['context']['destination'], 'CeladonGym')
        self.assertEqual(added[0]['context']['prerequisite_for_goals'], [goal])
        for name, blocks in original.items():
            self.assertIs(pt.MAPS[name]['blocks'], blocks)

    def test_battle_preparation_uses_selected_trainer_without_overwriting_causal_context(self):
        from types import SimpleNamespace
        from openpokered.story_agent import DualStoryAgent
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        rule = Rule('key-grunt', 'Base', 'Base:grunt', ['npc:2'], [], [],
                    ('flag', 'GRUNT_BEATEN', True), [('battle', 'Rocket:1', True)])
        agent.maps = {'Base': {'npcs': [
            {'textId': 1, 'trainerClass': 'Boss', 'trainerSet': 1},
            {'textId': 2, 'trainerClass': 'Rocket', 'trainerSet': 1}]}}
        agent.trainers = {'Boss': {'parties': [{'pokemon': [{'species': 'Rhydon', 'level': 50}]}]},
                          'Rocket': {'parties': [{'pokemon': [{'species': 'Zubat', 'level': 21}]}]}}
        self.assertEqual(agent.opponent_parties([rule, rule]), [{'species': 'Zubat', 'level': 21}])
        agent.battle_requirements, agent.field_requirements = {}, {}
        agent.navigation_blockage, agent.navigation_memory = None, {}
        agent.index = SimpleNamespace(rules=[])
        group = {'target': rule.effect, 'rules': [rule], 'objectives': ['Obtain key'],
                 'context': {'required_for': 'elevator'}}
        with patch.object(DualStoryAgent, 'strategy_groups', return_value={'key': group}):
            result = agent.strategy_groups({'party': []})
        self.assertEqual(result['key']['context']['required_for'], 'elevator')
        self.assertEqual(result['key']['context']['opponent_parties'], [{'species': 'Zubat', 'level': 21}])

    def test_stationary_obstruction_survives_map_exit_and_expires_when_hidden(self):
        from types import SimpleNamespace
        game = JevGame.__new__(JevGame)
        game.judgments = SimpleNamespace(
            maps={'Route12': {'npcs': [{'textId': 1, 'movement': 'Stationary'},
                                      {'textId': 2, 'movement': 'Walk'}]}},
            index=SimpleNamespace(npc_toggles={('Route12', 1): ('SLEEPER', False)}),
            navigation_facts={'flags': {}})
        game.remember_npcs('Route12', [{'text_id': 1, 'x': 10, 'y': 62, 'visible': True},
                                       {'text_id': 2, 'x': 9, 'y': 10, 'visible': True}])
        game._prev_map = 'LavenderTown'
        self.assertEqual(game.navigation_barriers(), {'Route12': {(10, 62)}})
        game._prev_map = 'Route12'
        self.assertEqual(game.navigation_barriers(), {})
        game._prev_map = 'LavenderTown'
        game.judgments.navigation_facts['flags']['__OBJ_HIDDEN_SLEEPER'] = True
        self.assertEqual(game.navigation_barriers(), {})

    def test_scripted_npc_move_and_hide_clear_its_old_patrol_obstacle(self):
        from types import SimpleNamespace
        game = JevGame.__new__(JevGame)
        game.observed_npcs = {}
        game.judgments = SimpleNamespace(maps={'Room': {'npcs': [
            {'textId': 1, 'movement': 'Stationary'}, {'textId': 2, 'movement': 'Walk'}]}})
        game.d = Mock()
        game.d.cmd.return_value = {'data': [{'text_id': 1, 'x': 10, 'y': 62, 'visible': True},
                                          {'text_id': 2, 'x': 9, 'y': 10, 'visible': True}]}
        self.assertIn((10, 62), game.npc_blocked('Room'))
        game.d.cmd.return_value = {'data': [{'text_id': 1, 'x': 10, 'y': 63, 'visible': True},
                                          {'text_id': 2, 'x': 9, 'y': 11, 'visible': True}]}
        blocked = game.npc_blocked('Room')
        self.assertNotIn((10, 62), blocked)
        self.assertTrue({(10, 63), (9, 10), (9, 11)} <= blocked)
        game.d.cmd.return_value['data'][0]['visible'] = False
        blocked = game.npc_blocked('Room')
        self.assertNotIn((10, 63), blocked)
        self.assertTrue({(9, 10), (9, 11)} <= blocked)

    def test_actual_defeat_offers_further_training_but_escape_does_not(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.battle_defeats, agent.defeat_preparation = [], 0
        agent.record = Mock()
        before = {'map_name': 'PewterGym', 'battle_live': {'enemy_party': [{'species': 'Onix'}]}}
        after = {'party': [{'level': 14}], 'battle_phase': 'BattleOver { won: false, escaped: true }'}
        agent.observe_battle_result(before, after)
        self.assertEqual(agent.defeat_preparation, 0)
        after['battle_phase'] = 'TrainerVictory { player_won: false }'
        agent.observe_battle_result(before, after)
        self.assertEqual(agent.defeat_preparation, 16)
        self.assertTrue(agent.should_replan({}))
        self.assertEqual(len(agent.battle_defeats), 1)
        self.assertEqual(after['party'][0]['level'], 14)
        after['battle_phase'] = 'TrainerVictory { player_won: true }'
        agent.observe_battle_result(before, after)
        self.assertEqual(agent.defeat_preparation, 0)
        self.assertTrue(agent.battle_defeats[0]['resolved_by_victory'])

    def test_water_requirement_finds_real_shores_and_restores_land_collision(self):
        import playthrough as pt
        original = pt.walkable, pt.walkable_edge
        state = {'map_name': 'FuchsiaCity', 'player_x': 5, 'player_y': 14}
        obstacle = surf_requirement(state, 'CinnabarGym', [(3, 2), (4, 3)], 'FuchsiaCity')
        self.assertEqual(obstacle['move'], 'Surf')
        self.assertTrue(pt.walkable(obstacle['map'], *obstacle['stance']))
        dx, dy = pt.DELTA[obstacle['direction']]
        x, y = obstacle['stance']
        self.assertTrue(water_tile(obstacle['map'], x+dx, y+dy))
        self.assertFalse(water_tile(*obstacle['landing']))
        self.assertEqual((pt.walkable, pt.walkable_edge), original)
        with self.assertRaises(ValueError), water_planning():
            raise ValueError('search failed')
        self.assertEqual((pt.walkable, pt.walkable_edge), original)
        self.assertTrue(hm_compatible('LAPRAS', 'Surf'))
        self.assertFalse(hm_compatible('VENUSAUR', 'Surf'))

    def test_water_requirement_skips_shortcuts_with_existing_land_routes(self):
        import playthrough as pt
        state = {'map_name': 'Route12', 'player_x': 10, 'player_y': 10}
        obstacle = surf_requirement(state, 'CinnabarGym', [(3, 2), (4, 3)], 'Route12')
        self.assertIsNotNone(obstacle)
        self.assertIsNotNone(pt.bfs_cross('Route12', (10, 10), obstacle['map'], tuple(obstacle['stance']),
            last_map='Route12', allow_ledges=True, allow_spinners=True))
        landing = obstacle['landing']
        self.assertIsNone(pt.bfs_cross('Route12', (10, 10), landing[0], tuple(landing[1:]),
            last_map='Route12', allow_ledges=True, allow_spinners=True))

    def test_observed_gate_barrier_allows_detour_and_expires_with_prerequisite(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.observed_barrier_maps = {'Route18Gate1F'}
        guard = {'Call': {'callee': 'game.hasItem', 'args': [{'StringLit': 'BICYCLE'}]}}
        rule = Rule('gate', 'Route18Gate1F', 'gate:check', [], [(guard, False)], [],
                    ('movement', 'push', True), [])
        agent.index = Mock(rules=[rule])
        agent.index.coordinates.return_value = [(4, y) for y in range(3, 7)]
        barriers = agent.observed_navigation_barriers({'bag': {}})
        self.assertEqual(barriers['Route18Gate1F'], {(4, y) for y in range(3, 7)})
        path = pt.bfs_cross('Route18Gate1F', (5, 3), 'PokemonFanClub', (2, 1),
                            last_map='Route18', allow_ledges=True, blocked_maps=barriers)
        self.assertIsNotNone(path)
        self.assertFalse(any(node[0] == 'Route18Gate1F' and tuple(node[1:]) in barriers['Route18Gate1F']
                             for node, _ in path[1:]))
        self.assertEqual(agent.observed_navigation_barriers({'bag': {'BICYCLE': 1}}), {})
        agent.observed_barrier_maps.clear()
        self.assertEqual(agent.observed_navigation_barriers({'bag': {}}), {})

    def test_shifted_final_protocol_responses_are_not_resumable_evidence(self):
        data = {'get_state': {'screen': 'overworld'}, 'get_flags': {'EVENT_A': True},
                'get_party': [{'species': 'Venusaur'}], 'get_bag': [{'item': 'PokeFlute'}],
                'get_npcs': [{'text_id': 1}]}
        observations = {cmd: {'ok': True, 'data': value} for cmd, value in data.items()}
        self.assertTrue(observations_valid(observations))
        observations['get_bag'] = observations['get_party']
        self.assertFalse(observations_valid(observations))

    def test_soft_stop_waits_for_the_current_protocol_round_trip(self):
        raw = Mock()
        protocol = ObservedProtocol(raw, Mock(), time.monotonic()+60)
        def finish(**kwargs):
            protocol.stop_requested = True
            return {'ok': True, 'data': {'screen': 'overworld'}}
        raw.cmd.side_effect = finish
        self.assertEqual(protocol.cmd(cmd='get_state')['data']['screen'], 'overworld')
        with self.assertRaisesRegex(StoryStopped, 'command_boundary'):
            protocol.cmd(cmd='get_flags')
        raw.cmd.assert_called_once()

    def test_observed_unidentified_ghost_escapes_and_records_story_requirement(self):
        import playthrough as pt
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.battle_requirements = {}
        game.battles_driven = 0
        before = {'screen': 'battle', 'map_name': 'Tower', 'battle_live': {'is_ghost': True},
                  'script_awaiting_battle': True, 'party': []}
        after = {'screen': 'overworld', 'map_name': 'Tower', 'battle_phase': 'Over',
                 'party': [], 'frame_count': 100}
        game.st = Mock(side_effect=[before, after])
        with patch.object(pt.Game, 'battle_loop') as drive:
            game.battle_loop(prefer='fight')
        self.assertEqual(drive.call_args.kwargs['prefer'], 'run')
        self.assertIn('SILPH_SCOPE', game.judgments.battle_requirements)
        game.judgments.choose.assert_called_once()

    def test_scripted_battle_escape_interrupts_navigation_without_party_or_map_change(self):
        import playthrough as pt
        from openpokered.playthrough_judgments import NavigationPause
        for ghost in (True, False):
            with self.subTest(ghost=ghost):
                game = JevGame.__new__(JevGame)
                game.judgments = Mock()
                game.judgments.active = {'target': ['transport', ['Destination', 1, 1], True]}
                game.judgments.battle_requirements = {}
                game.battles_driven = 0
                game.navigation_active = True
                before = {'screen': 'battle', 'map_name': 'Tower',
                          'battle_live': {'is_ghost': ghost},
                          'script_awaiting_battle': True, 'party': []}
                after = {'screen': 'overworld', 'map_name': 'Tower', 'battle_phase': 'Over',
                         'party': [], 'frame_count': 100}
                game.st = Mock(side_effect=[before, after])
                with patch.object(pt.Game, 'battle_loop') as drive, \
                        self.assertRaises(NavigationPause):
                    game.battle_loop(prefer='run')
                self.assertEqual(drive.call_args.kwargs['prefer'], 'run')
                self.assertEqual(game.battles_driven, 1)
                if ghost:
                    self.assertEqual(game.judgments.battle_requirements['SILPH_SCOPE']['blocked_goal'],
                                     game.judgments.active['target'])

    def test_ordinary_wild_escape_keeps_unchanged_navigation(self):
        import playthrough as pt
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.battles_driven = 0
        game.navigation_active = True
        before = {'screen': 'battle', 'map_name': 'Route', 'battle_live': {'is_ghost': False},
                  'script_awaiting_battle': False, 'party': []}
        after = {'screen': 'overworld', 'map_name': 'Route', 'battle_phase': 'Over',
                 'party': [], 'frame_count': 100}
        game.st = Mock(side_effect=[before, after])
        with patch.object(pt.Game, 'battle_loop') as drive:
            game.battle_loop(prefer='run')
        self.assertEqual(drive.call_args.kwargs['prefer'], 'run')

    def test_failed_tile_route_reports_the_object_occupying_its_passage(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock()
        agent.game.st.return_value = {'map_name': 'PokemonTower6F', 'player_x': 18, 'player_y': 9}
        agent.game.last_map = 'LavenderTown'
        agent.game.navigation_excluded_maps.return_value = ()
        agent.game.nav_to_map.side_effect = pt.NavError('blocked passage')
        agent.game.live_npcs.return_value = {(16, 5), (6, 8), (14, 14)}
        agent.maps = {'PokemonTower6F': {'npcs': [{'textId': 3, 'isTrainer': True}]}}
        agent.client = Mock()
        agent.client.cmd.return_value = [{'text_id': 3, 'x': 16, 'y': 5, 'visible': True},
                                         {'text_id': 4, 'x': 6, 'y': 8, 'visible': True},
                                         {'text_id': 5, 'x': 14, 'y': 14, 'visible': True}]
        result = agent.travel('PokemonTower7F', Rule('', '', '', [], [], [], (), []), [(10, 4)])
        self.assertEqual(result['blocking_npcs'], [4])
        self.assertEqual(result['blocking_trainers'], [])
        self.assertEqual(result['result'], 'blocked')

    def test_npc_barrier_still_offers_a_reachable_alternative_cut_passage(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock()
        agent.game.st.return_value = {'map_name': 'Route12', 'player_x': 0, 'player_y': 62}
        agent.game.last_map = 'Route12'
        agent.game.navigation_excluded_maps.return_value = ('SaffronCity',)
        agent.game.nav_to_map.side_effect = pt.NavError('blocked by sleeping Pokemon')
        agent.game.live_npcs.return_value = {(10, 62), (9, 52)}
        agent.maps = {'Route12': {'npcs': [{'textId': 3, 'isTrainer': True}]}}
        agent.client = Mock()
        agent.client.cmd.return_value = [{'text_id': 3, 'x': 9, 'y': 52, 'visible': True},
                                         {'text_id': 1, 'x': 10, 'y': 62, 'visible': True}]
        agent.field_requirements = {}
        result = agent.travel('Route8', Rule('', '', '', [], [], [], (), []), [(41, 11)])
        self.assertEqual(result['blocking_npcs'], [1])
        self.assertEqual(result['blocking_trainers'], [])
        self.assertEqual(result['field_obstruction']['map'], 'Route9')
        self.assertEqual(agent.field_requirements['Cut']['tree'], [5, 8])

    def test_regrown_tree_invalidates_clearance_during_an_intermediate_map_visit(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.cleared_terrain = {'Route9,5,8', 'CeladonCity,35,32'}
        agent.invalidate_terrain({'map_name': 'Route9'})
        self.assertEqual(agent.cleared_terrain, {'CeladonCity,35,32'})

    def test_machine_boot_and_teach_confirmation_precede_party_selection(self):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        menus = [
            {'kind': 'bag', 'phase': 'Browsing', 'cursor': 0, 'items': [{'item': 'Hm01'}]},
            {'kind': 'bag', 'phase': 'ActionMenu { cursor: 0 }'},
            {'kind': 'bag', 'phase': 'MachineBoot { item: Hm01 }'},
            {'kind': 'bag', 'phase': 'MachineTeach { item: Hm01, cursor: 0 }'},
            {'kind': 'party', 'phase': 'Browsing', 'cursor': 0},
            {'kind': 'party', 'phase': 'ChooseMove { cursor: 0 }', 'known_moves': ['Tackle', 'Growl']},
            {'kind': 'party', 'phase': 'ChooseMove { cursor: 1 }', 'known_moves': ['Tackle', 'Growl']},
            {'kind': 'party', 'phase': 'ItemUseNotice { wait_frames: 0 }'},
            None,
        ]
        snapshots = iter({'field_menu': menu, 'party': [{'moves': ['Tackle', 'Cut' if i >= 7 else 'Growl']}]}
                         for i, menu in enumerate(menus))
        game.st = lambda: next(snapshots)
        game.tap = Mock()
        game.learn_machine('Hm01', 'Cut', 0, 'Growl')
        self.assertEqual([call.args[0] for call in game.tap.call_args_list],
                         ['a', 'a', 'a', 'a', 'a', 'down', 'a', 'b'])

    def test_cut_obstruction_is_derived_without_changing_planning_maps(self):
        import playthrough as pt
        before = {name: tuple(data['blocks']) for name, data in pt.MAPS.items()}
        obstacle = cut_requirement({'map_name': 'CeruleanCity', 'player_x': 19, 'player_y': 18},
                                   'VermilionGym', [(5, 2)], 'CeruleanCity')
        self.assertEqual(obstacle['move'], 'Cut')
        self.assertEqual(obstacle['destination'], 'VermilionGym')
        self.assertEqual(before, {name: tuple(data['blocks']) for name, data in pt.MAPS.items()})
        self.assertTrue(hm_compatible('Ivysaur', 'Cut'))
        self.assertFalse(hm_compatible('Venusaur', 'Strength'))

    def test_checkpoint_inherits_verified_ending_through_legacy_gap(self):
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            ancestor, checkpoint = root / 'ancestor', root / 'checkpoint'
            ancestor.mkdir()
            checkpoint.mkdir()
            proof = {'autosave_sha256': 'a' * 64,
                'phases': [{'phase': ['overworld', 'MonInfo', None]},
                           {'phase': ['overworld', None, 'TheEnd']},
                           {'phase': ['title', None, None]}],
                'separate_process_continue': {'map_name': 'PalletTown', 'badges': 255, 'hall_of_fame_count': 1}}
            original = json.dumps({'first_clear_verification': proof})
            (ancestor / 'summary.json').write_text(original)
            (checkpoint / 'summary.json').write_text(json.dumps({'resumed_from': str(ancestor)}))
            inherited = checkpoint_first_clear_verification(checkpoint,
                {'map_name': 'VictoryRoad1F', 'badges': 255, 'hall_of_fame_count': 1})
            self.assertEqual(inherited, {**proof, 'inherited_from': str(ancestor.resolve())})
            self.assertEqual((ancestor / 'summary.json').read_text(), original)
            agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
            agent.first_clear_verification = inherited
            agent.objectives = [{'id': 'become-champion', 'name': 'First clear',
                                 'satisfied_when': {'flag': 'EVENT_BEAT_CHAMPION_RIVAL'}}]
            agent.index = Mock()
            self.assertEqual(DualStoryAgent.strategy_groups(agent, {'flags': {}}), {})
            agent.index.frontier.assert_not_called()  # Reset Elite Four flags do not re-open the objective.
            (checkpoint / 'summary.json').write_text(json.dumps({'first_clear_verification': inherited}))
            self.assertEqual(checkpoint_first_clear_verification(checkpoint,
                {'badges': 255, 'hall_of_fame_count': 2})['inherited_from'], str(ancestor.resolve()))

    def test_each_completed_ending_advances_the_next_ceremony_baseline(self):
        import tempfile
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.hof_baseline = 1
        agent.check_budget = Mock()
        agent.tap = Mock()
        agent.record = Mock()
        agent.client = Mock()
        with tempfile.TemporaryDirectory() as temporary:
            saved = Path(temporary) / 'game.sav'
            saved.write_bytes(bytes(32768))
            agent.game = Mock(save_path=saved, seed=42)
            check = Mock()
            for count in (2, 3):
                states = [
                    {'screen': 'overworld', 'hof_phase': 'MonInfo'},
                    {'screen': 'overworld', 'credits_phase': 'TheEnd', 'credits_final_button': True},
                    {'screen': 'title'},
                ]
                states = [{**row, 'frame_count': i, 'hall_of_fame_count': count}
                          for i, row in enumerate(states)]
                agent.client.state.side_effect = states
                check.st.return_value = {'map_name': 'PalletTown', 'badges': 255,
                                         'hall_of_fame_count': count}
                with patch('playthrough.Game', return_value=check), patch('playthrough.resume_reentry'):
                    self.assertTrue(agent.settle_special(states[0]))
                self.assertEqual(agent.hof_baseline, count)
                self.assertEqual(agent.first_clear_verification['separate_process_continue']['hall_of_fame_count'], count)

    def test_checkpoint_first_clear_requires_proof_and_matching_live_record(self):
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'summary.json').write_text(json.dumps({'completed': ['become-champion']}))
            restored = {'badges': 255, 'hall_of_fame_count': 1}
            self.assertIsNone(checkpoint_first_clear_verification(root, restored))
            proof = {'autosave_sha256': 'a' * 64,
                'phases': [{'phase': ['overworld', 'MonInfo', None]},
                           {'phase': ['overworld', None, 'TheEnd']},
                           {'phase': ['title', None, None]}],
                'separate_process_continue': {'map_name': 'PalletTown', **restored}}
            for invalid in [{**proof, 'phases': proof['phases'][:1]},
                            {**proof, 'autosave_sha256': ''},
                            {**proof, 'separate_process_continue': {}}]:
                (root / 'summary.json').write_text(json.dumps({'first_clear_verification': invalid}))
                with self.assertRaisesRegex(ValueError, 'incomplete'):
                    checkpoint_first_clear_verification(root, restored)
            (root / 'summary.json').write_text(json.dumps({'first_clear_verification': proof}))
            for stale in [{'badges': 255, 'hall_of_fame_count': 0},
                          {'badges': 127, 'hall_of_fame_count': 1}]:
                with self.assertRaisesRegex(ValueError, 'contradicts'):
                    checkpoint_first_clear_verification(root, stale)
            (root / 'summary.json').write_text(json.dumps({'resumed_from': str(root)}))
            with self.assertRaisesRegex(ValueError, 'cycle'):
                checkpoint_first_clear_verification(root, restored)

    def test_champion_flag_is_not_durable_first_clear_proof(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.first_clear_verification = None
        objective = {'id': 'become-champion',
                     'satisfied_when': {'flag': 'EVENT_BEAT_CHAMPION_RIVAL'}}
        facts = {'flags': {'EVENT_BEAT_CHAMPION_RIVAL': True}}
        self.assertFalse(agent.objective_satisfied(objective, facts))
        agent.first_clear_verification = {'separate_process_continue': {}}
        facts['flags'].clear()  # The transient flag resets in Hall of Fame.
        self.assertTrue(agent.objective_satisfied(objective, facts))

    def test_interrupted_route_does_not_make_unreached_maps_visited(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.visited = {'PewterCity'}
        agent.client = Mock()
        agent.client.state.return_value = {'map_name': 'Route3'}
        agent.record_travel({'result': 'entered_battle', 'legs_completed': 1,
                             'legs': [{'from_map': 'PewterCity', 'to_map': 'Route3'},
                                      {'from_map': 'Route3', 'to_map': 'Route4'}]})
        self.assertEqual(agent.visited, {'PewterCity', 'Route3'})

    def test_training_grass_is_reachable_from_current_route_two_section(self):
        import playthrough as pt
        spot = reachable_grass('Route2', (8, 71))
        self.assertIsNotNone(spot)
        self.assertGreater(spot[1], 40)
        self.assertTrue(pt.bfs('Route2', (8, 71), spot, pt.warp_tiles('Route2')))

    def test_nurse_is_approached_across_the_canonical_counter_tile(self):
        self.assertIn(((3, 3), 'up'), list(counter_approaches('ViridianPokecenter', {'x': 3, 'y': 1})))
        self.assertEqual(list(counter_approaches('OaksLab', {'x': 5, 'y': 2})), [])

    def test_rejects_state_shortcuts_before_they_reach_game(self):
        raw = Mock()
        protocol = ObservedProtocol(raw, Mock(), time.monotonic()+60)
        for command in ('warp', 'give_pokemon', 'give_item', 'set_flag', 'restore_state', 'save'):
            with self.assertRaisesRegex(StoryStopped, 'forbidden_debug_command'):
                protocol.cmd(cmd=command)
        raw.cmd.assert_not_called()

    def test_neutral_input_and_frame_count_are_forwarded(self):
        raw = Mock()
        raw.cmd.return_value = {'ok': True, 'data': {'advanced': True, 'queue_start_frame': 100, 'frame_count': 113}}
        protocol = ObservedProtocol(raw, Mock(), time.monotonic()+60)
        protocol.drive([None, 'a', None], frames=13)
        raw.cmd.assert_called_once_with(cmd='press_timeline', buttons=[None, 'a', None]+[None]*10, advance=True)
        raw.cmd.return_value['data']['frame_count'] = 114
        with self.assertRaisesRegex(StoryStopped, 'input_timeline_not_advanced_atomically'):
            protocol.drive(['b']*13)

    def state(self):
        return {'battle_live': {
            'player': {'species': 'Bulbasaur', 'hp': 20, 'max_hp': 30},
            'enemy': {'species': 'Onix', 'hp': 30, 'max_hp': 30}}}

    def test_move_cache_changes_when_pp_or_disable_changes(self):
        menu = {'moves': [{'move': 'Tackle', 'pp': 10, 'disabled': False},
                          {'move': 'VineWhip', 'pp': 10, 'disabled': False}]}
        before, options = move_question(self.state(), menu)
        self.assertEqual(before['moves']['1']['effectiveness'], 4)
        menu['moves'][1]['pp'] = 2
        low, _ = move_question(self.state(), menu)
        self.assertNotEqual(json.dumps(before, sort_keys=True), json.dumps(low, sort_keys=True))
        menu['moves'][1]['disabled'] = True
        after, options = move_question(self.state(), menu)
        self.assertNotIn('1', options)
        self.assertNotIn('1', after['moves'])

    def test_high_critical_attack_exposes_its_expected_advantage(self):
        state = {'battle_live': {
            'player': {'species': 'Charizard', 'level': 60, 'hp': 186, 'max_hp': 186},
            'enemy': {'species': 'Lapras', 'level': 40, 'hp': 150, 'max_hp': 150}}}
        menu = {'moves': [{'move': name, 'pp': 10, 'disabled': False}
                          for name in ['Strength', 'Slash']]}
        compact, _ = move_question(state, menu)
        self.assertAlmostEqual(compact['moves']['1']['critical_probability_without_focus_energy'], 255/256, places=4)
        self.assertGreater(compact['moves']['1']['effective_expected_power'],
                           1.4 * compact['moves']['0']['effective_expected_power'])

    def test_machine_replacement_preserves_stronger_same_type_attack(self):
        mon = {'species': 'Charizard', 'level': 60,
               'moves': ['Flamethrower', 'Cut', 'FireSpin', 'Slash']}
        self.assertNotIn('Flamethrower', replacement_options(mon, 'Toxic'))
        self.assertIn('FireSpin', replacement_options(mon, 'Toxic'))
        self.assertNotIn('Cut', replacement_options(mon, 'Toxic'))
        self.assertIn('Flamethrower', replacement_options(mon, 'FireBlast'))

    def test_medicine_options_match_actual_injury_and_status(self):
        party = [{'hp': 40, 'max_hp': 40, 'status': 'Sleep(2)'},
                 {'hp': 0, 'max_hp': 70, 'status': 'None'}]
        bag = {'FullRestore': 2, 'HyperPotion': 1, 'Antidote': 1, 'Revive': 1}
        offered = {(item, index) for item, index, _ in medicine_options(party, bag)}
        self.assertEqual(offered, {('FullRestore', 0), ('Revive', 1)})
        party[0]['hp'] = 20
        self.assertIn(('HyperPotion', 0), {(item, index) for item, index, _ in medicine_options(party, bag)})

    def test_cave_training_uses_ordinary_floor_but_avoids_exit_warps(self):
        self.assertTrue(training_tile('VictoryRoad2F', 2, 8))
        self.assertFalse(training_tile('VictoryRoad2F', 0, 8))
        self.assertIsNotNone(reachable_grass('VictoryRoad2F', (0, 8)))

    def test_safari_paths_are_not_encounter_grass(self):
        import playthrough as pt
        self.assertFalse(training_tile('SafariZoneCenter', 14, 24))
        spot = reachable_grass('SafariZoneCenter', (14, 24))
        self.assertIsNotNone(spot)
        self.assertEqual(pt.tile_at('SafariZoneCenter', *spot), 0x20)
        self.assertEqual(pt.tile_at('SafariZoneCenter', spot[0]+1, spot[1]), 0x20)

    def test_outdoor_grass_requires_native_right_rate_anchor(self):
        import playthrough as pt
        edge = next((x, y) for x in range(pt.MAPS['Route1']['width']*2)
            for y in range(pt.MAPS['Route1']['height']*2)
            if pt.is_grass('Route1', x, y) and not pt.is_grass('Route1', x+1, y))
        self.assertFalse(training_tile('Route1', *edge))

    def test_level_up_replacement_respects_one_judgment_across_confirmation(self):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.choose.return_value = 'skip'
        game.tap = Mock()
        state = {'party': [{'species': 'Charizard', 'level': 55,
                            'moves': ['Flamethrower', 'Cut', 'Slash', 'Strength']}],
                 'battle_phase': 'LearnMoveAsk { party_index: 0, move_id: FireSpin, resume: PlayerMenu }'}
        game.learn_move(state)
        game.tap.assert_called_with('b', 8)
        state['battle_phase'] = state['battle_phase'].replace('LearnMoveAsk', 'LearnMoveGiveUpConfirm')
        game.learn_move(state)
        self.assertEqual(game.judgments.choose.call_count, 1)
        self.assertEqual([call.args[0] for call in game.tap.call_args_list], ['b', 'up', 'a'])

    def test_immune_active_battler_offers_a_conscious_teammate(self):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.choose.return_value = 'switch:1'
        party = [{'species': 'Charizard', 'level': 67, 'hp': 80, 'max_hp': 210,
                  'moves': ['Slash', 'FireBlast'], 'pp': [10, 0], 'status': 'None'},
                 {'species': 'Lapras', 'level': 16, 'hp': 67, 'max_hp': 67,
                  'moves': ['Surf'], 'pp': [15], 'status': 'None'}]
        state = {'party': party, 'battle_inventory': [], 'battle_live': {
            'player': party[0], 'enemy': {'species': 'Gengar'}, 'player_party': party}}
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 1))
        self.assertIn('switch:1', game.judgments.choose.call_args.args[2])

    def test_live_pp_overrides_stale_persistent_party_for_switching(self):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.active = {'context': {}}
        game.judgments.choose.return_value = 'switch:1'
        party = [{'species': 'Gloom', 'level': 21, 'hp': 45, 'max_hp': 63,
                  'moves': ['Absorb', 'Poisonpowder'], 'pp': [10, 20], 'status': 'None'},
                 {'species': 'Charizard', 'level': 38, 'hp': 90, 'max_hp': 127,
                  'moves': ['Ember'], 'pp': [20], 'status': 'None'}]
        live_party = [{**party[0], 'pp': [0, 20]}, party[1]]
        state = {'party': party, 'battle_inventory': [], 'battle_live': {
            'player': live_party[0], 'enemy': {'species': 'Oddish'},
            'player_party': live_party}}
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 1))
        self.assertIn('switch:1', game.judgments.choose.call_args.args[2])
        self.assertNotIn('fight', game.judgments.choose.call_args.args[2])

    def test_evolution_trainee_can_switch_to_stronger_finisher(self):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.active = {'context': {'acquisition_method': 'evolution',
            'trigger': 'level', 'from_species': 'Oddish'}}
        game.judgments.choose.return_value = 'switch:1'
        party = [{'species': 'Oddish', 'level': 14, 'hp': 40, 'max_hp': 40,
                  'moves': ['Absorb'], 'pp': [20], 'status': 'None'},
                 {'species': 'Charizard', 'level': 36, 'hp': 120, 'max_hp': 120,
                  'moves': ['Ember'], 'pp': [25], 'status': 'None'}]
        state = {'party': party, 'battle_inventory': [], 'battle_live': {
            'player': party[0], 'enemy': {'species': 'Kakuna'}, 'player_party': party}}
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 1))
        self.assertIn('share experience', game.judgments.choose.call_args.args[2]['switch:1'])
        self.assertIn('remains conscious', game.judgments.choose.call_args.args[3])
        # After switching, do not send the trainee back into harm's way.
        state['battle_live']['player'] = party[1]
        self.assertIsNone(game.battle_recovery_plan(state))

    def test_normal_battle_does_not_offer_switch_training(self):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.active = {'context': {}}
        party = [{'species': 'Oddish', 'level': 14, 'hp': 40, 'max_hp': 40,
                  'moves': ['Absorb'], 'pp': [20], 'status': 'None'},
                 {'species': 'Charizard', 'level': 36, 'hp': 120, 'max_hp': 120,
                  'moves': ['Ember'], 'pp': [25], 'status': 'None'}]
        state = {'party': party, 'battle_inventory': [], 'battle_live': {
            'player': party[0], 'enemy': {'species': 'Kakuna'}, 'player_party': party}}
        self.assertIsNone(game.battle_recovery_plan(state))

    def test_failed_battle_preparation_changes_with_pp_or_recovery_stock(self):
        party = [{'species': 'Charizard', 'level': 65, 'hp': 203, 'max_hp': 203,
                  'moves': ['FireBlast', 'Slash'], 'pp': [1, 15], 'status': 'None'}]
        before = battle_readiness(party, {'NUGGET': 1})
        self.assertEqual(before, battle_readiness(party, {}))
        self.assertNotEqual(before, battle_readiness(party, {'FULLRESTORE': 1}))
        party[0]['pp'] = [5, 20]
        self.assertNotEqual(before, battle_readiness(party, {}))

    def test_training_navigation_remembers_the_same_barrier_as_story_travel(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.client = Mock()
        agent.client.state.return_value = {'map_name': 'PewterCity', 'player_x': 35, 'player_y': 17}
        agent.active = {'target': ('level', 'leader', 14)}
        agent.navigation_memory, agent.navigation_history = {}, {}
        agent.observed_barrier_maps = set()
        agent.remember_travel_result('Route3', {'result': 'blocked', 'detail': 'A guide returns the player'})
        self.assertEqual(agent.navigation_memory['Route3']['map'], 'PewterCity')
        self.assertIn('PewterCity', agent.observed_barrier_maps)
        agent.remember_travel_result('Route3', {'result': 'reached'})
        self.assertNotIn('Route3', agent.navigation_memory)

    def test_entry_autowalk_does_not_make_one_way_exit_guard_block_entry(self):
        from types import SimpleNamespace
        entry = Rule('entry', 'Room', 'Room:@load', ['load'],
                     [({'Call': {'callee': 'getFlag', 'args': [{'StringLit': 'ENTERED'}]}}, False)],
                     [], ('movement', 'movePlayerRelative', True), [])
        gate = Rule('gate', 'Room', 'Room:exit', ['coord:exit'], [], [],
                    ('movement', 'movePlayerRelative', True), [])
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = SimpleNamespace(rules=[entry, gate], coordinates=lambda r: [(4, 10)] if r.id == 'gate' else [])
        agent.observed_barrier_maps = {'Room'}
        self.assertEqual(agent.observed_navigation_barriers({'map': 'Lobby', 'flags': {}}), {})
        self.assertEqual(agent.observed_navigation_barriers({'map': 'Room', 'flags': {'ENTERED': True}}),
                         {'Room': {(4, 10)}})
        self.assertEqual(agent.observed_navigation_barriers({'map': 'Lobby', 'flags': {'ENTERED': True}}),
                         {'Room': {(4, 10)}})

    def test_depleted_attacks_require_recovery_even_at_full_hp(self):
        facts = {'party': [{'hp': 30, 'max_hp': 30, 'status': 'None',
                            'moves': ['Tackle', 'Growl'], 'pp': [0, 40]}]}
        self.assertTrue(AutonomousStoryAgent.needs_healing(facts))
        facts['party'][0]['pp'][0] = 35
        self.assertFalse(AutonomousStoryAgent.needs_healing(facts))
        # Total PP can hide the depletion of the only suitable attack.
        facts['party'][0]['moves'] = ['Cut', 'RazorLeaf', 'VineWhip']
        facts['party'][0]['pp'] = [4, 8, 10]
        self.assertTrue(AutonomousStoryAgent.needs_healing(facts))


if __name__ == '__main__':
    unittest.main()

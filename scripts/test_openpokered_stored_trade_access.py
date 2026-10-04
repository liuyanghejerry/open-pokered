"""PC preparation must retain the downstream trade's actual access evidence."""
from copy import deepcopy
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from openpokered.autonomous_story import AutonomousStoryAgent
from openpokered.decision_wire import compact_decision_json_text_state, expand_decision_evidence
from openpokered.story_rules import Rule, literal


class StoredTradeAccessTests(unittest.TestCase):
    def fixture(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        agent.client, agent.game = Mock(), Mock()
        pc = Rule('pc', 'Center', 'Center:pc', ['sign:1'], [], [], ('pc', 'storage', True), [])
        trade = Rule('trade', 'Route2TradeHouse', 'Route2TradeHouse:talk', ['npc:1'],
            [], [], ('flag', 'TRADE_DONE', True), [])
        unrelated = Rule('unrelated', 'OtherHouse', 'OtherHouse:talk', ['npc:1'],
            [], [], ('flag', 'OTHER_TRADE', True), [])
        agent.index = Mock(by_effect={('pc', 'storage', True): [pc],
            ('flag', 'TRADE_DONE', True): [trade], ('flag', 'OTHER_TRADE', True): [unrelated]})
        method = {'method': 'npc_trade', 'from_species': 'Abra',
            'map': 'Route2TradeHouse', 'completion_flag': 'TRADE_DONE'}
        stored = {'box': 1, 'index': 3, 'species': 'Abra', 'level': 8,
            'hp': 23, 'max_hp': 23, 'status': 'None'}
        facts = {'map': 'Center', 'x': 12, 'y': 3, 'flags': {'TRADE_DONE': False},
            'party': [{'species': 'Charizard', 'level': 66}], 'bag': {},
            'stored_pokemon': [stored], 'dex': {'owned_species': ['Abra', 'Charizard']}}
        groups = {}
        agent.add_storage_retrieval(groups, facts, 'Abra', 'MrMime', method)
        preview = groups['retrieve:Abra']['context']['post_withdrawal_acquisitions'][0]

        def annotate(shadows, actual, previews=None, *, prune=True):
            self.assertFalse(prune)
            self.assertIs(actual, facts)
            self.assertEqual([rule for group in shadows.values() for rule in group['rules']], [trade])
            for group in shadows.values():
                group['context']['trigger_navigation'] = [{
                    'map': trade.map, 'tile_route_found': False, 'steps': None,
                    'requires_surf': False, 'scope': 'observed blocked trigger region'}]
            return previews or {}

        agent.annotate_navigation = Mock(side_effect=annotate)
        return agent, facts, groups, preview, trade

    def test_npc_trade_preview_retains_the_exact_source_scene_not_only_pc_cost(self):
        _, _, _, preview, _ = self.fixture()
        self.assertEqual(preview['trade_source_scene'], {
            'map': 'Route2TradeHouse', 'completion_flag': 'TRADE_DONE'})
        self.assertFalse(preview['withdrawal_registers_target'])

    def test_blocked_trade_is_annotated_without_hiding_withdrawal_or_changing_binding(self):
        agent, facts, groups, preview, trade = self.fixture()
        before, original = deepcopy(facts), list(groups.items())
        pc_rule = groups['retrieve:Abra']['rules'][0]
        cached = {('Center', ((12, 3),)): {'tile_route_found': True, 'steps': 0}}
        agent.annotate_stored_trade_access(groups, facts, cached)
        reference = preview['downstream_trade_access_reference']
        self.assertEqual(reference['origin'], ['Center', 12, 3])
        self.assertFalse(reference['trigger_navigation'][0]['tile_route_found'])
        self.assertIsNone(reference['trigger_navigation'][0]['steps'])
        self.assertEqual(reference['source_rule_ids'], [trade.id])
        self.assertEqual(list(groups), [key for key, _ in original])
        self.assertIs(groups['retrieve:Abra']['rules'][0], pc_rule)
        self.assertEqual(facts, before)
        self.assertIs(agent.annotate_navigation.call_args.args[2], cached)
        self.assertIn('before withdrawal', reference['scope'])
        self.assertIn('not the chosen PC', reference['scope'])
        self.assertEqual(agent.client.mock_calls + agent.game.mock_calls, [])

    def test_access_can_refresh_to_reachable_without_retaining_stale_blockage(self):
        agent, facts, groups, preview, _ = self.fixture()
        agent.annotate_stored_trade_access(groups, facts)
        def reachable(shadows, actual, previews=None, *, prune=True):
            for group in shadows.values():
                group['context']['trigger_navigation'] = [{
                    'map': 'Route2TradeHouse', 'tile_route_found': True,
                    'steps': 321, 'requires_surf': True, 'unmet_native_field_prerequisites': []}]
        agent.annotate_navigation.side_effect = reachable
        facts['x'] = 4
        agent.annotate_stored_trade_access(groups, facts)
        reference = preview['downstream_trade_access_reference']
        self.assertEqual(reference['origin'], ['Center', 4, 3])
        self.assertTrue(reference['trigger_navigation'][0]['tile_route_found'])
        self.assertEqual(reference['trigger_navigation'][0]['steps'], 321)
        self.assertIn('Surf', reference['scope'])

    def test_missing_or_wrong_map_producer_stays_unknown_not_reachable_or_blocked(self):
        for missing in (True, False):
            agent, facts, groups, preview, _ = self.fixture()
            if missing:
                agent.index.by_effect.pop(('flag', 'TRADE_DONE', True))
            else:
                agent.index.by_effect[('flag', 'TRADE_DONE', True)][0].map = 'OtherHouse'
            preview['downstream_trade_access_reference'] = {'stale': True}
            agent.annotate_stored_trade_access(groups, facts)
            reference = preview['downstream_trade_access_reference']
            self.assertIsNone(reference['trigger_navigation'])
            self.assertEqual(reference['source_rule_ids'], [])
            self.assertNotIn('stale', reference)
            agent.annotate_navigation.assert_not_called()

    def test_level_evolution_and_unrelated_goals_remain_unchanged(self):
        agent, facts, groups, preview, _ = self.fixture()
        preview['acquisition_method'] = 'evolution'
        preview.pop('trade_source_scene', None)
        before = deepcopy(groups)
        agent.annotate_stored_trade_access(groups, facts)
        self.assertEqual(groups, before)
        agent.annotate_navigation.assert_not_called()

    def test_noncollection_and_absent_source_scene_do_not_invent_trade_paths(self):
        for collection in (False, True):
            agent, facts, groups, preview, _ = self.fixture()
            agent.collects_dex = collection
            if collection:
                preview.pop('trade_source_scene')
            before = deepcopy(groups)
            agent.annotate_stored_trade_access(groups, facts)
            self.assertEqual(groups, before)
            agent.annotate_navigation.assert_not_called()

    def test_shared_trade_regions_compare_once_and_each_option_keeps_its_own_evidence(self):
        agent, facts, groups, preview, _ = self.fixture()
        second = deepcopy(preview)
        groups['retrieve:Abra']['context']['post_withdrawal_acquisitions'].append(second)
        agent.annotate_stored_trade_access(groups, facts)
        agent.annotate_navigation.assert_called_once()
        self.assertEqual(len(agent.annotate_navigation.call_args.args[0]), 1)
        self.assertEqual(preview['downstream_trade_access_reference'],
                         second['downstream_trade_access_reference'])
        self.assertIsNot(preview['downstream_trade_access_reference'],
                         second['downstream_trade_access_reference'])

    def test_complete_json_text_roundtrip_preserves_new_costs_and_all_world_facts(self):
        agent, facts, groups, _, _ = self.fixture()
        agent.annotate_stored_trade_access(groups, facts)
        state = {'world': facts, 'history': ['kept']}
        candidates = {'retrieve': json.dumps(groups['retrieve:Abra']['context']),
            'other': 'Every competing goal remains available'}
        encoded, offered = compact_decision_json_text_state(state, candidates)
        restored, choices = expand_decision_evidence(encoded, offered)
        self.assertEqual(restored, state)
        self.assertEqual(choices, candidates)

    def test_source_guards_are_evaluated_on_real_flags_not_future_withdrawal(self):
        agent, facts, groups, preview, trade = self.fixture()
        trade.guards = [({'Call': {'callee': 'getFlag', 'args': [literal('TRADE_DONE')]}}, False)]
        facts['flags']['TRADE_DONE'] = True
        original = deepcopy(facts)
        agent.annotate_stored_trade_access(groups, facts)
        guards = preview['downstream_trade_access_reference']['source_script_preconditions']
        self.assertEqual(guards, [{'rule_id': trade.id,
            'missing_alternatives': [[('flag', 'TRADE_DONE', False)]]}])
        self.assertEqual(facts, original)

    def test_real_navigation_annotation_keeps_the_trade_region_and_native_field_guards(self):
        agent, facts, groups, preview, trade = self.fixture()
        agent.annotate_navigation = AutonomousStoryAgent.annotate_navigation.__get__(agent)
        facts['party'][0]['moves'] = ['Surf']
        facts['flags']['EVENT_BEAT_KOGA'] = False
        agent.game.last_map = 'FuchsiaCity'
        agent.game.navigation_barriers.return_value = {}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.game.live_npcs.return_value = set()
        agent.observed_navigation_barriers = Mock(return_value={})
        agent.destination_points = Mock(return_value=[(3, 5)])
        region = trade.map, ((3, 5),)
        path = [('Center', 12, 3), (('Route2TradeHouse', 3, 5), 'up')]
        with patch('openpokered.autonomous_story.pt.bfs_cross_routes',
                   side_effect=[{region: None}, {region: path}]) as traversal, \
                patch('openpokered.navigation_skills.water_tile',
                      side_effect=lambda name, x, y: name == trade.map):
            agent.annotate_stored_trade_access(groups, facts)
        route, = preview['downstream_trade_access_reference']['trigger_navigation']
        self.assertEqual(route['map'], trade.map)
        self.assertFalse(route['tile_route_found'])
        self.assertTrue(route['requires_surf'])
        self.assertEqual(route['unmet_native_field_prerequisites'],
                         [('flag', 'EVENT_BEAT_KOGA', True)])
        self.assertEqual(traversal.call_count, 2)
        self.assertEqual(list(groups), ['retrieve:Abra'])

    def test_model_guidance_keeps_all_competing_options_and_abstention(self):
        agent, facts, groups, _, _ = self.fixture()
        agent.annotate_stored_trade_access(groups, facts)
        agent.choose_bounded_strategy = Mock(return_value='retrieve')
        state = {'world': facts}
        candidates = {'retrieve': json.dumps({'establish': ['pokemon', 'Abra', None],
            'context': groups['retrieve:Abra']['context']}, separators=(',', ':')),
            'other': 'A different collecting goal', 'none': 'No suitable choice'}
        original = deepcopy(state), deepcopy(candidates)
        self.assertEqual(agent.choose('strategy', state, candidates, 'Compare'), 'retrieve')
        passed, offered, instruction = agent.choose_bounded_strategy.call_args.args
        restored, options = expand_decision_evidence(passed, offered)
        self.assertEqual(restored['world'], original[0]['world'])
        self.assertEqual(list(options), list(original[1]))
        self.assertEqual(json.loads(options['retrieve']), json.loads(original[1]['retrieve']))
        self.assertEqual(options['other'], candidates['other'])
        self.assertIn('downstream_trade_access_reference', instruction)
        self.assertIn('neither bans retrieval', instruction)
        self.assertTrue(agent.choose_bounded_strategy.call_args.kwargs['allow_abstain'])


if __name__ == '__main__':
    unittest.main()

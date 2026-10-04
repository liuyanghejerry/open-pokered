"""ID-indexed comparison metadata belongs to current options, not every partition."""
from copy import deepcopy
import json
import unittest
from unittest.mock import Mock, patch
from openpokered.autonomous_story import AutonomousStoryAgent, scope_strategy_access_evidence
from openpokered.story_agent import DualStoryAgent, StoryStopped
from openpokered.typesafe import TypeSafeError
from openpokered.decision_wire import expand_decision_evidence


class AccessPartitionTests(unittest.TestCase):
    def state(self):
        return {'world': {'money': 93, 'party': [{'species': 'Hypno', 'level': 38}], 'flags': {'Surf': True}},
            'navigation': {'other_regions': ['SafariZoneGate', 'PowerPlant']},
            'immediate_access_comparison': {
                'path_found': {'sale': ['sale', 'Tm06', False], 'train': ['level', 'Hypno', 39]},
                'field_action_needed': {'zap': ['register', 'Zapdos', True]},
                'no_path_found': {'story': ['flag', 'EVENT_BEAT_CHAMPION_RIVAL', True]},
                'not_evaluated': {'unknown': ['catch', 'Unknown', True]},
                'scope': 'Fresh current trigger access, not a guarantee or permanent impossibility'}}

    def test_current_metadata_and_all_world_facts_survive_without_mutating_full_frontier(self):
        state = self.state()
        original = deepcopy(state)
        options = {'zap': 'Zapdos capture', 'sale': 'Finite sale with teaching cost'}
        actual = scope_strategy_access_evidence(state, options)
        self.assertEqual(actual['immediate_access_comparison'], {
            'path_found': {'sale': ['sale', 'Tm06', False]},
            'field_action_needed': {'zap': ['register', 'Zapdos', True]},
            'no_path_found': {}, 'not_evaluated': {}, 'scope': state['immediate_access_comparison']['scope']})
        self.assertIs(actual['world'], state['world'])
        self.assertIs(actual['navigation'], state['navigation'])
        self.assertEqual(state, original)

    def test_full_frontier_missing_or_unknown_metadata_keep_original_object(self):
        state = self.state()
        full = {key: key for value in state['immediate_access_comparison'].values()
                if isinstance(value, dict) for key in value}
        self.assertIs(scope_strategy_access_evidence(state, full), state)
        self.assertIs(scope_strategy_access_evidence(state, {}), state)
        self.assertIs(scope_strategy_access_evidence(state, {'not-in-this-comparison': 'Continue'}), state)
        variants = [None, {'scope': 'Unknown shape'},
                    {**state['immediate_access_comparison'], 'unrecognised_world_facts': {'money': 93}},
                    {**state['immediate_access_comparison'], 'scope': None},
                    {**state['immediate_access_comparison'], 'path_found': []},
                    {**state['immediate_access_comparison'], 'not_evaluated': {'sale': ['unknown', 'value', True]}},
                    {**state['immediate_access_comparison'], 'not_evaluated': {1: ['unknown', 'value', True]}}]
        for comparison in variants:
            with self.subTest(comparison=comparison):
                actual = {**state, 'immediate_access_comparison': comparison}
                self.assertIs(scope_strategy_access_evidence(actual, {'sale': 'Sell'}), actual)
        without = {'world': state['world']}
        self.assertIs(scope_strategy_access_evidence(without, {'sale': 'Sell'}), without)

    def test_all_disjoint_options_keep_their_own_access_and_original_values(self):
        state = self.state()
        full = {key: key for value in state['immediate_access_comparison'].values()
                if isinstance(value, dict) for key in value}
        covered = {}
        for keys in (['sale', 'zap'], ['train', 'story', 'unknown']):
            actual = scope_strategy_access_evidence(state, {key: full[key] for key in keys})
            comparison = actual['immediate_access_comparison']
            for category, value in comparison.items():
                if not isinstance(value, dict):
                    continue
                for key, target in value.items():
                    self.assertEqual(target, state['immediate_access_comparison'][category][key])
                    self.assertNotIn(key, covered)
                    covered[key] = target
        self.assertEqual(set(covered), set(full))

    def test_binary_comparison_scopes_only_foreign_ids_before_call_and_keeps_abstention(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.record = Mock()
        state, options = self.state(), {'sale': 'Sell', 'zap': 'Capture'}
        with patch.object(DualStoryAgent, 'choose', return_value='sale') as choose:
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), 'sale')
        choose.assert_called_once()
        layer, actual, sent, instruction = choose.call_args.args
        self.assertEqual(layer, 'strategy')
        self.assertEqual(sent, options)
        self.assertIs(actual['world'], state['world'])
        self.assertEqual(choose.call_args.kwargs, {'allow_abstain': True})
        self.assertIn('other disjoint groups are compared separately', instruction)
        event = next(call.kwargs for call in agent.record.call_args_list if call.args[0] == 'strategy_access_scope')
        self.assertEqual((event['access_entries_before'], event['access_entries_sent']), (5, 2))
        self.assertTrue(event['world_facts_preserved'] and event['current_candidate_access_preserved'])

    def test_action_choice_does_not_scope_its_parent_strategy_metadata(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.record = Mock()
        state = self.state()
        with patch.object(DualStoryAgent, 'choose', return_value='sale') as choose:
            agent.choose_bounded_choice('action', state, {'sale': 'Sell'}, 'Pick')
        self.assertIs(choose.call_args.args[1], state)
        agent.record.assert_not_called()

    def test_overflow_tournament_still_compares_every_option_and_each_native_access_status(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.record = Mock()
        state = self.state()
        options = {key: json.dumps({'establish': target}) for value in state['immediate_access_comparison'].values()
                   if isinstance(value, dict) for key, target in value.items()}
        original = deepcopy((state, options))
        evaluated = set()

        def decide(layer, actual, candidates, instruction, *, allow_abstain):
            self.assertEqual(layer, 'strategy')
            actual, restored_options = expand_decision_evidence(actual, candidates)
            self.assertEqual(restored_options, candidates)
            self.assertEqual(actual['world'], state['world'])
            comparison = actual['immediate_access_comparison']
            sent_ids = {key for value in comparison.values() if isinstance(value, dict) for key in value}
            self.assertEqual(sent_ids, set(candidates))
            self.assertLessEqual(instruction.count('other disjoint groups are compared separately'), 1)
            for key, value in candidates.items():
                self.assertEqual(value, options[key])
            if len(candidates) > 2:
                raise StoryStopped('strategy:service_unavailable') from TypeSafeError('max_tokens_exceeded')
            evaluated.update(candidates)
            return 'sale' if 'sale' in candidates else next(iter(candidates))

        with patch.object(DualStoryAgent, 'choose', side_effect=decide) as choose:
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), 'sale')
        self.assertEqual(evaluated, set(options))
        self.assertTrue(choose.call_args.kwargs['allow_abstain'])
        self.assertEqual((state, options), original)


if __name__ == '__main__':
    unittest.main()

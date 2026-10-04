"""Observed selected goals do not overwrite compiled script-condition receipts."""
from copy import deepcopy
import io
import json
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
from openpokered.story_agent import DualStoryAgent, post_operation_effects
from openpokered.story_rules import StoryIndex


class PostOperationEffectTests(unittest.TestCase):
    def setUp(self):
        self.index = StoryIndex.__new__(StoryIndex)
        self.rule = SimpleNamespace(effect=('party_space', 'party', True))
        self.facts = {'flags': {}, 'party': [{'species': 'Seel', 'level': 30},
            *[{'species': 'Other', 'level': 1} for _ in range(5)]]}

    def test_full_party_withdrawal_keeps_script_false_and_selected_goal_true(self):
        target = ['pokemon', 'Seel', None]
        original = deepcopy((self.facts, target, self.rule.effect))
        actual = post_operation_effects(self.index, self.rule, target, self.facts)
        self.assertFalse(actual['intended_effect_observed'])
        self.assertEqual(actual['script_effect'], ['party_space', 'party', True])
        self.assertEqual(actual['selected_subgoal'], target)
        self.assertTrue(actual['selected_subgoal_satisfied_after'])
        self.assertEqual((self.facts, target, self.rule.effect), original)

    def test_withdrawal_receipt_without_the_requested_species_is_not_success(self):
        actual = post_operation_effects(self.index, self.rule,
            ['pokemon', 'Kadabra', None], self.facts)
        self.assertFalse(actual['selected_subgoal_satisfied_after'])
        self.facts['party'] = self.facts['party'][:5]
        actual = post_operation_effects(self.index, self.rule,
            ['pokemon', 'Kadabra', None], self.facts)
        self.assertTrue(actual['intended_effect_observed'])
        self.assertFalse(actual['selected_subgoal_satisfied_after'])

    def test_nonfull_current_box_is_independent_of_full_party(self):
        actual = post_operation_effects(self.index, self.rule,
            ['box_space', 'current', True],
            {**self.facts, 'current_box_index': 2, 'box_counts': [20, 20, 2]})
        self.assertTrue(actual['selected_subgoal_satisfied_after'])
        self.assertFalse(actual['intended_effect_observed'])

    def test_normal_healing_condition_remains_identical(self):
        rule = SimpleNamespace(effect=('heal', 'party', True))
        for recovered in (True, False):
            with self.subTest(recovered=recovered):
                actual = post_operation_effects(self.index, rule, rule.effect,
                    {**self.facts, 'fully_recovered': recovered})
                self.assertEqual(actual['intended_effect_observed'], recovered)
                self.assertEqual(actual['selected_subgoal_satisfied_after'], recovered)

    def test_pending_source_stays_unsatisfied_and_absent_target_stays_unknown(self):
        actual = post_operation_effects(self.index, self.rule,
            ['register', 'Seel', True],
            {**self.facts, 'dex': {'owned_species': ['Seel']}, 'collection_audit_pending': ['Seel']})
        self.assertFalse(actual['selected_subgoal_satisfied_after'])
        for target in (None, [], ['pokemon', 'Seel'], 'Seel'):
            actual = post_operation_effects(self.index, self.rule, target, self.facts)
            self.assertIsNone(actual['selected_subgoal_satisfied_after'])
            self.assertEqual(actual['selected_subgoal'], target)

    def test_reference_is_a_snapshot_not_shared_mutable_goal_or_script_storage(self):
        target = ['location', ['Route15', 18, 9], True]
        rule = SimpleNamespace(effect=('location', ['Route15', 18, 9], True))
        actual = post_operation_effects(self.index, rule, target,
            {**self.facts, 'map': 'Route15', 'x': 18, 'y': 9})
        target[1][1] = 99
        rule.effect[1][1] = 88
        self.assertEqual(actual['selected_subgoal'][1], ['Route15', 18, 9])
        self.assertEqual(actual['script_effect'][1], ['Route15', 18, 9])
        self.assertTrue(actual['selected_subgoal_satisfied_after'])

    def test_run_latches_goal_before_execution_and_retains_observations_in_recent_context(self):
        trace = io.StringIO()
        client = Mock()
        client.state.return_value = {'frame_count': 1}
        agent = DualStoryAgent(client, Mock(), [{'id': 'done', 'satisfied_when': {'flag': 'DONE'}}], trace=trace)
        before = {**self.facts, 'party': [{'species': 'Other', 'level': 1} for _ in range(6)],
                  'bag': {}, 'badges': 0, 'map': 'Center', 'x': 0, 'y': 0}
        after = {**before, 'party': deepcopy(self.facts['party']), 'flags': {'DONE': True}}
        agent.facts = Mock(side_effect=[before, before, after, after, after])
        agent.active = {'target': ['pokemon', 'Seel', None], 'objectives': ['Retrieve Seel']}
        agent.should_replan = Mock(return_value=False)
        agent.settle = Mock()
        agent.action_candidates = Mock(return_value=({'action:0': 'PC'}, {'action:0': ('retrieve_pc:1,1,1,0', self.rule)}))
        agent.choose = Mock(return_value='action:0')
        def execute(*args):
            # A nested operation can change the live active target. The receipt
            # must still describe the goal that authorized this operation.
            agent.active['target'][1] = 'Kadabra'
            return {'result': 'withdrew_pokemon'}
        agent.execute = Mock(side_effect=execute)
        agent.operation_outcome_observations = Mock(return_value={'observed_progress': 'sentinel'})
        self.index.rules, self.index.errors, self.index.sha256 = [self.rule], [], 'mock'
        with patch('openpokered.story_agent.StoryIndex', return_value=self.index):
            result = agent.run()
        self.assertTrue(result['success'])
        outcome = next(json.loads(line) for line in trace.getvalue().splitlines()
                       if json.loads(line)['kind'] == 'outcome')
        self.assertEqual(outcome['selected_subgoal'], ['pokemon', 'Seel', None])
        self.assertTrue(outcome['selected_subgoal_satisfied_after'])
        self.assertFalse(outcome['intended_effect_observed'])
        self.assertEqual(agent.recent[-1]['selected_subgoal'], ['pokemon', 'Seel', None])
        agent.operation_outcome_observations.assert_called_once_with(
            'retrieve_pc:1,1,1,0', {'result': 'withdrew_pokemon'}, before, after)
        self.assertEqual(outcome['observed_progress'], 'sentinel')
        self.assertEqual(agent.recent[-1]['observed_progress'], 'sentinel')
        self.assertIn('not causation or final collection proof', agent.choose.call_args.args[3])

    def test_default_outcome_observer_preserves_noncollection_behavior(self):
        agent = DualStoryAgent.__new__(DualStoryAgent)
        self.assertEqual(agent.operation_outcome_observations('deposit_pc:1,0',
            {'result': 'deposited_pokemon'}, self.facts, self.facts), {})


if __name__ == '__main__':
    unittest.main()

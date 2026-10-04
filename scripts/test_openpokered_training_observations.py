"""Observed collection training cost, without invented battle or route forecasts."""
from copy import deepcopy
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
from openpokered.autonomous_story import AutonomousStoryAgent
from openpokered.decision_wire import compact_decision_field_wire, restore_decision_field_wire


class TrainingObservationTests(unittest.TestCase):
    def fixture(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        agent.recent = []
        facts = {'dex': {'owned_species': ['Geodude', 'Charizard']}, 'party': [
            {'species': 'Geodude', 'level': 18, 'experience': 4058,
             'hp': 48, 'max_hp': 48, 'status': 'None'},
            {'species': 'Charizard', 'level': 66, 'experience': 294747,
             'hp': 227, 'max_hp': 227, 'status': 'None'}]}
        return agent, facts

    def outcome(self, agent, before, after, area='Route7', result='trained'):
        operation = f'train_encounter:{area},8,2'
        return {'operation': operation, 'result': result,
                'selected_subgoal': ['register', 'Graveler', True],
                **agent.operation_outcome_observations(operation, {'result': result}, before, after)}

    def test_training_boundaries_record_real_xp_and_condition_without_io_or_pc_change(self):
        agent, before = self.fixture()
        after = deepcopy(before)
        after['party'][0].update(experience=4163, hp=11)
        agent.client, agent.game = Mock(), Mock()
        agent._last_completed_collection_pc = {'untouched': True}
        original = deepcopy((before, after))
        value = self.outcome(agent, before, after)['collection_training_observation']
        geodude = next(row for row in value['during_operation']['party_species_observations']
                       if row['species'] == 'Geodude')
        self.assertEqual(geodude['experience_change_if_single_sample_each'], 105)
        self.assertEqual(value['area'], 'Route7')
        self.assertEqual(value['party_condition_after'][0], {
            'party_index': 0, 'species': 'Geodude', 'hp': 11, 'max_hp': 48, 'status': 'None'})
        self.assertEqual((before, after), original)
        self.assertEqual(agent._last_completed_collection_pc, {'untouched': True})
        self.assertEqual(agent.client.mock_calls + agent.game.mock_calls, [])

    def test_training_history_aggregates_samples_not_battles_or_reward_forecasts(self):
        agent, before = self.fixture()
        after = deepcopy(before)
        after['party'][0].update(experience=4163, hp=11)
        second = deepcopy(after)
        second['party'][0].update(experience=4417, hp=48, status='Sleep(4)')
        third = deepcopy(second)
        third['party'][0].update(experience=4417, hp=0, status='None')
        agent.recent = [self.outcome(agent, before, after), self.outcome(agent, after, second),
                        self.outcome(agent, second, third),
                        {'operation': 'travel_to:Center', 'result': 'reached',
                         'selected_subgoal': ['heal', 'party', True],
                         'selected_subgoal_satisfied_after': False},
                        {'operation': 'interact_with:npc:0', 'result': 'interacted',
                         'selected_subgoal': ['heal', 'party', True],
                         'selected_subgoal_satisfied_after': True}]
        original = deepcopy(agent.recent)
        value = agent.collection_training_execution_reference()
        self.assertEqual(value['completed_training_operation_count'], 3)
        self.assertEqual(value['training_operations_with_boundary_observations'], 3)
        self.assertEqual(value['satisfied_recovery_goal_outcome_count'], 1)
        area = value['areas'][0]
        self.assertEqual(area['area'], 'Route7')
        geodude = next(row for row in area['species_boundary_samples'] if row['species'] == 'Geodude')
        self.assertEqual(geodude['known_experience_difference_sum'], 359)
        self.assertEqual(geodude['known_experience_difference_count'], 3)
        self.assertEqual(geodude['unknown_experience_difference_count'], 0)
        self.assertEqual(geodude['health_status_warning_boundary_count'], 3)
        self.assertEqual(len(value['latest_training_operations']), 1)
        self.assertEqual(agent.recent, original)
        self.assertIn('not battle counts', value['scope'])
        self.assertIn('not per-site recovery causation', value['scope'])
        self.assertIn('not individual identity', value['scope'])

    def test_unknown_duplicate_xp_and_missing_condition_are_not_zero_cost(self):
        agent, before = self.fixture()
        before['party'].append({**before['party'][0], 'experience': 4500})
        after = deepcopy(before)
        after['party'].reverse()
        after['party'][0]['experience'] += 50
        after['party'][0].pop('hp')
        agent.recent = [self.outcome(agent, before, after)]
        area = agent.collection_training_execution_reference()['areas'][0]
        geodude = next(row for row in area['species_boundary_samples'] if row['species'] == 'Geodude')
        self.assertIsNone(geodude['known_experience_difference_sum'])
        self.assertEqual(geodude['known_experience_difference_count'], 0)
        self.assertEqual(geodude['unknown_experience_difference_count'], 1)
        self.assertEqual(geodude['unknown_condition_boundary_count'], 1)
        self.assertEqual(geodude['known_condition_boundary_count'], 0)
        self.assertEqual(geodude['health_status_warning_boundary_count'], 0)

    def test_negative_xp_and_zero_are_recorded_as_observed(self):
        agent, before = self.fixture()
        after = deepcopy(before)
        after['party'][0]['experience'] -= 100
        agent.recent = [self.outcome(agent, before, after), self.outcome(agent, after, after)]
        geodude = next(row for row in agent.collection_training_execution_reference()['areas'][0][
            'species_boundary_samples'] if row['species'] == 'Geodude')
        self.assertEqual(geodude['known_experience_difference_sum'], -100)
        self.assertEqual(geodude['known_experience_difference_count'], 2)

    def test_health_warning_requires_complete_native_sample_each_side_is_not_invented(self):
        agent, facts = self.fixture()
        for changes in ({'hp': True}, {'hp': -1}, {'max_hp': 0}, {'status': None}, {'status': ''}):
            with self.subTest(changes=changes):
                after = deepcopy(facts)
                after['party'][0].update(changes)
                agent.recent = [self.outcome(agent, facts, after)]
                row = next(row for row in agent.collection_training_execution_reference()['areas'][0][
                    'species_boundary_samples'] if row['species'] == 'Geodude')
                self.assertEqual(row['known_condition_boundary_count'], 0)
                self.assertEqual(row['unknown_condition_boundary_count'], 1)
                self.assertEqual(row['health_status_warning_boundary_count'], 0)

    def test_failed_training_and_noncollection_do_not_create_observation(self):
        agent, facts = self.fixture()
        for result in ('blocked', 'paused_after_battle', 'no_reachable_training_grass'):
            self.assertNotIn('collection_training_observation', self.outcome(agent, facts, facts, result=result))
        agent.collects_dex = False
        self.assertNotIn('collection_training_observation', self.outcome(agent, facts, facts))
        self.assertFalse(hasattr(agent, '_last_completed_collection_pc'))

    def test_legacy_history_missing_boundaries_stays_unknown_and_empty_history_is_local(self):
        agent, facts = self.fixture()
        empty = agent.collection_training_execution_reference()
        self.assertEqual(empty['completed_training_operation_count'], 0)
        self.assertEqual(empty['areas'], [])
        self.assertIn('current controller', empty['scope'])
        self.assertIn('not before CONTINUE', empty['scope'])
        agent.recent = [self.outcome(agent, facts, facts, result='blocked'),
                        {'operation': 'train_encounter:Route7,8,2', 'result': 'trained'},
                        self.outcome(agent, facts, facts, area='Route24')]
        value = agent.collection_training_execution_reference()
        self.assertEqual(value['completed_training_operation_count'], 2)
        self.assertEqual(value['training_operations_with_boundary_observations'], 1)
        legacy = next(row for row in value['areas'] if row['area'] == 'Route7')
        self.assertEqual(legacy['operations_with_boundary_observations'], 0)
        self.assertEqual(legacy['species_boundary_samples'], [])

    def test_training_snapshot_is_unaliased_and_lossless_wire_preserves_all_options(self):
        agent, before = self.fixture()
        after = deepcopy(before)
        after['party'][0].update(hp=0, experience=4163)
        agent.recent = [self.outcome(agent, before, after)]
        after['party'][0]['hp'] = 48
        state = {'world': after, 'collection_training_execution_reference':
                 agent.collection_training_execution_reference()}
        observed = state['collection_training_execution_reference']['latest_training_operations'][0]
        self.assertEqual(observed['collection_training_observation']['party_condition_after'][0]['hp'], 0)
        criteria = {'train': json.dumps({'cost': 1}), 'other': 'Different acquisition', 'none': 'Abstain'}
        packed, offered = compact_decision_field_wire(state, criteria)
        restored, choices = restore_decision_field_wire(packed, offered)
        self.assertEqual(restored, state)
        self.assertEqual(choices, criteria)
        self.assertEqual(list(choices), list(criteria))

    def test_augment_and_guidance_include_reference_without_changing_strategy_choices(self):
        agent, facts = self.fixture()
        agent.completed_route_context = Mock(return_value=None)
        agent.dex_progress = Mock(return_value={})
        agent.collection_preparation_continuity_reference = Mock(return_value={})
        agent.script_resource_guard_reference = Mock(return_value=None)
        agent.navigation_goal_resource_guard_reference = Mock(return_value=None)
        agent.route_execution_reference = Mock(return_value={})
        agent.maps = {}
        state = {'world': facts}
        agent.augment_strategy_state(state, facts)
        self.assertEqual(state['collection_training_execution_reference']['completed_training_operation_count'], 0)
        criteria = {'train': '{}', 'other': '{}', 'none': 'Abstain'}
        agent.choose_bounded_choice = Mock(return_value='other')
        self.assertEqual(agent.choose('strategy', state, criteria, 'Select.'), 'other')
        _, restored = restore_decision_field_wire(*agent.choose_bounded_choice.call_args.args[1:3])
        self.assertEqual(restored, criteria)
        self.assertIn('collection_training_execution_reference', agent.choose_bounded_choice.call_args.args[3])

    def test_missing_party_and_owned_changes_do_not_become_zero_cost_or_legal_proof(self):
        agent, before = self.fixture()
        agent.collection_audit_pending = {'Graveler': {'reason': 'unverified'}}
        after = {'dex': {'owned_species': ['Geodude', 'Charizard', 'Graveler']}}
        agent.recent = [self.outcome(agent, before, after)]
        observed = agent.recent[0]['collection_training_observation']
        self.assertEqual(observed['during_operation']['native_owned_species_added'], ['Graveler'])
        self.assertIsNone(observed['during_operation']['party_species_observations'])
        self.assertIsNone(observed['party_condition_after'])
        reference = agent.collection_training_execution_reference()
        self.assertEqual(reference['areas'][0]['operations_with_unknown_party_progress'], 1)
        self.assertEqual(reference['areas'][0]['species_boundary_samples'], [])
        self.assertEqual(agent.collection_audit_pending, {'Graveler': {'reason': 'unverified'}})
        self.assertNotIn('Graveler', agent.validated_owned(after))


if __name__ == '__main__':
    unittest.main()

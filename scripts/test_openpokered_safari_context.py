"""Observed Safari allowances and deterministic training comparison evidence."""
from copy import deepcopy
import unittest
from unittest.mock import Mock, patch

from openpokered.autonomous_story import AutonomousStoryAgent, safari_session_reference
from openpokered.story_agent import DualStoryAgent


def expanded(value, library):
    if isinstance(value, list):
        return [expanded(item, library) for item in value]
    if not isinstance(value, dict):
        return value
    if set(value) == {'shared_strategy_evidence_ref'}:
        return expanded(library[value['shared_strategy_evidence_ref']], library)
    if set(value) == {'strategy_table'}:
        table = value['strategy_table']
        return [{key: expanded(item, library) for key, item in zip(table['columns'], row)}
                for row in table['rows']]
    return {key: expanded(item, library) for key, item in value.items()}


class SafariContextTests(unittest.TestCase):
    def facts(self, balls=28, steps=447, active=True):
        return {'map': 'SafariZoneCenter', 'safari_game': {'active': active,
            'balls_remaining': balls, 'steps_remaining': steps}, 'safari_observation_frame': 44739}

    def value_agent(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {'SafariZoneCenter': {'wild': {'red': {'grass': {
            'encounterRate': 30, 'mons': [{'species': 'Chansey', 'level': 23}] * 10}}}}}
        return agent

    def test_reference_copies_current_allowance_not_full_admission(self):
        facts = self.facts()
        reference = safari_session_reference(facts)
        self.assertEqual(reference['observed_session'], facts['safari_game'])
        self.assertEqual(reference['native_observation_frame'], 44739)
        self.assertEqual(reference['fresh_admission_reference']['balls'], 30)
        facts['safari_game']['balls_remaining'] = 0
        self.assertEqual(reference['observed_session']['balls_remaining'], 28)

    def test_unknown_is_not_inactive_or_full_and_map_flags_do_not_infer_it(self):
        reference = safari_session_reference({'map': 'SafariZoneCenter',
            'flags': {'EVENT_IN_SAFARI_ZONE': True}, 'bag': {'POKEBALL': 99}})
        self.assertIsNone(reference['observed_session'])
        self.assertIsNone(reference['native_observation_frame'])

    def test_inactive_zero_allowance_stays_explicit(self):
        reference = safari_session_reference(self.facts(0, 0, False))
        self.assertEqual(reference['observed_session'], {'active': False,
            'balls_remaining': 0, 'steps_remaining': 0})

    def test_invalid_counter_types_ranges_and_missing_fields_stay_unknown(self):
        for change in ({'active': 1}, {'balls_remaining': True}, {'balls_remaining': -1},
                       {'balls_remaining': 31}, {'steps_remaining': False},
                       {'steps_remaining': 501}, {'steps_remaining': -1},
                       {'balls_remaining': 2.0}, {'active': None}):
            with self.subTest(change=change):
                facts = self.facts()
                facts['safari_game'].update(change)
                self.assertIsNone(safari_session_reference(facts)['observed_session'])
        for field in ('active', 'balls_remaining', 'steps_remaining'):
            facts = self.facts()
            facts['safari_game'].pop(field)
            self.assertIsNone(safari_session_reference(facts)['observed_session'])

    def test_observation_frame_rejects_bool_negative_and_text(self):
        for frame in (False, True, -1, '3', 1.0, None):
            facts = self.facts()
            facts['safari_observation_frame'] = frame
            self.assertIsNone(safari_session_reference(facts)['native_observation_frame'])

    def test_reference_separates_bag_and_support_from_safari_tools(self):
        reference = safari_session_reference(self.facts())
        self.assertFalse(reference['bag_balls_usable_in_safari'])
        self.assertFalse(reference['party_status_moves_usable_in_safari'])
        self.assertTrue(reference['early_exit_forfeits_remaining_allowance'])
        self.assertIn('sunk costs', reference['scope'])
        self.assertIn('no choice is prescribed', reference['scope'])

    def test_safari_reference_uses_active_current_shared_budget(self):
        value = self.value_agent().method_value('safari', 'SafariZoneCenter', set(), facts=self.facts(2))
        reference = value['safari_registration_reference']
        self.assertEqual(reference['ball_budget_per_encounter'], 2)
        self.assertEqual(reference['ball_budget_source'], 'observed_current_shared_allowance')
        self.assertEqual(reference['native_observation_frame'], 44739)
        self.assertIn('Travel', reference['budget_scope'])

    def test_zero_active_balls_does_not_silently_refresh_to_thirty(self):
        value = self.value_agent().method_value('safari', 'SafariZoneCenter', set(), facts=self.facts(0))
        reference = value['safari_registration_reference']
        self.assertEqual(reference['ball_budget_per_encounter'], 0)
        self.assertEqual(reference['new_registration_per_eligible_step_pct_range'], [0, 0])
        self.assertEqual(reference['expected_eligible_steps_to_registration_range'], [None, None])

    def test_unknown_and_inactive_budgets_label_hypothetical_admission(self):
        for facts in ({}, self.facts(0, 0, False), {'safari_game': {'active': True}}):
            reference = self.value_agent().method_value('safari', 'SafariZoneCenter', set(),
                facts=facts)['safari_registration_reference']
            self.assertEqual(reference['ball_budget_per_encounter'], 30)
            self.assertEqual(reference['ball_budget_source'],
                'fresh_admission_reference_not_current_observation')

    def test_safari_budget_does_not_change_grass_encounter_values(self):
        agent = self.value_agent()
        self.assertEqual(agent.method_value('grass', 'SafariZoneCenter', set()),
                         agent.method_value('grass', 'SafariZoneCenter', set(), facts=self.facts(0)))

    def test_fact_reader_copies_only_normal_snapshot_no_map_or_evaluation_fallback(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        state = {'screen': 'overworld', 'map_name': 'SafariZoneCenter', 'frame_count': 31,
            'party': [], 'safari_game': self.facts()['safari_game'],
            'evaluation': {'safari_game': self.facts(30, 500)['safari_game']}}
        agent.client, agent.game = Mock(), Mock()
        agent.client.state.return_value = state
        agent.client.party.return_value = []
        agent.game.st.return_value = state
        agent.observe_audit_evolution = Mock()
        agent.collection_audit_pending, agent.cleared_terrain = {}, set()
        agent.battle_defeats, agent.visited, agent.crossed_passages = [], set(), set()
        agent.collects_dex, agent.index = False, None
        base = {'map': 'SafariZoneCenter', 'bag': {}, 'flags': {}, 'dex': {}}
        with patch.object(DualStoryAgent, 'facts', side_effect=lambda: deepcopy(base)):
            facts = agent.facts()
            self.assertEqual(facts['safari_game'], self.facts()['safari_game'])
            self.assertEqual(facts['safari_observation_frame'], 31)
            state.pop('safari_game')
            self.assertIsNone(agent.facts()['safari_game'])
        self.assertNotIn('evaluation', facts)

    def test_strategy_state_receives_reference_without_new_goal_or_native_call(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        agent.dex_progress = Mock(return_value={'owned': 60})
        agent.script_resource_guard_reference = Mock(return_value=[])
        agent.navigation_goal_resource_guard_reference = Mock(return_value=[])
        facts, state = self.facts(), {}
        agent.augment_strategy_state(state, facts)
        self.assertEqual(state['safari_session_reference'], safari_session_reference(facts))
        self.assertEqual(state['dex_progress'], {'owned': 60})
        self.assertFalse(hasattr(agent, 'client'))

    def test_action_local_and_menu_requests_preserve_all_choices_and_abstention(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        agent.choose_bounded_choice = Mock(return_value='1')
        for state in ({'local_state': self.facts()}, self.facts()):
            options = {'0': 'YES', '1': 'NO'}
            self.assertEqual(agent.choose('action', state, options, 'Choose', allow_abstain=True), '1')
            _, offered, actual, instruction = agent.choose_bounded_choice.call_args.args
            offered = expanded(offered, offered.get('shared_strategy_evidence') or {})
            self.assertEqual(offered['safari_session_reference']['observed_session'], self.facts()['safari_game'])
            self.assertEqual(actual, options)
            self.assertTrue(agent.choose_bounded_choice.call_args.kwargs['allow_abstain'])
            self.assertIn('does not require', instruction)
            self.assertNotIn('safari_session_reference', state)

    def test_noncollector_action_does_not_add_collection_reference(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = False
        agent.choose_bounded_choice = Mock(return_value='0')
        agent.choose('action', self.facts(), {'0': 'YES', '1': 'NO'}, 'Choose')
        self.assertNotIn('safari_session_reference', agent.choose_bounded_choice.call_args.args[1])

    def test_native_yes_no_menu_passes_its_own_fresh_counters_and_money(self):
        agent = DualStoryAgent.__new__(DualStoryAgent)
        agent.collects_dex = True
        agent.client, agent.game = Mock(), Mock(navigation_active=False)
        agent.check_budget, agent.record, agent.tap = Mock(), Mock(), Mock()
        agent.settle_special = Mock(return_value=False)
        raw = {'screen': 'overworld', 'map_name': 'SafariZoneGate', 'frame_count': 91,
            'safari_game': self.facts(23, 320)['safari_game'], 'money': 353}
        agent.client.state.side_effect = [
            {**raw, 'dialogue_state': 'AwaitingConfirm', 'dialogue': 'Leaving early?'},
            {**raw, 'choice': {'options': ['YES', 'NO'], 'selected': 1}}, raw]
        agent.client.observe.return_value = {'mode': 'overworld'}
        agent.choose = Mock(return_value='1')
        agent.settle(('shop', 'UltraBall', True))
        _, state, options, _ = agent.choose.call_args.args
        self.assertEqual(state['safari_game'], raw['safari_game'])
        self.assertEqual(state['safari_observation_frame'], 91)
        self.assertEqual(state['money'], 353)
        self.assertEqual(state['dialogue'], 'Leaving early?')
        self.assertEqual(options, {'0': 'YES', '1': 'NO'})
        agent.tap.assert_called_once_with('a')

    def test_unrelated_native_menus_keep_the_existing_request_shape(self):
        for collector, name in ((False, 'SafariZoneGate'), (True, 'FuchsiaPokecenter')):
            agent = DualStoryAgent.__new__(DualStoryAgent)
            agent.collects_dex = collector
            agent.client, agent.game = Mock(), Mock(navigation_active=False)
            agent.check_budget, agent.record, agent.tap = Mock(), Mock(), Mock()
            agent.settle_special = Mock(return_value=False)
            raw = {'screen': 'overworld', 'map_name': name, 'frame_count': 91,
                'safari_game': self.facts(23, 320)['safari_game'], 'money': 353}
            agent.client.state.side_effect = [
                {**raw, 'choice': {'options': ['YES', 'NO'], 'selected': 1}}, raw]
            agent.client.observe.return_value = {'mode': 'overworld'}
            agent.choose = Mock(return_value='1')
            agent.settle(('heal', 'party', True))
            state = agent.choose.call_args.args[1]
            self.assertNotIn('safari_game', state)
            self.assertNotIn('safari_observation_frame', state)
            self.assertNotIn('money', state)

    def test_training_examples_break_ties_stably_without_changing_goal_or_cost(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex, agent.index = True, Mock(rules=[], by_effect={})
        agent._complete_collection_graph = {'Paras': [], 'Parasect': [{'method': 'evolution',
            'from_species': 'Paras', 'trigger': 'level', 'level': 24}]}
        names = ['Delta', 'Beta', 'Alpha', 'Gamma']
        table = {'encounterRate': 30, 'mons': [{'species': 'Rattata', 'level': 30}] * 10}
        agent.maps = {name: {'wild': {'red': {'grass': table}}} for name in names}
        facts = {'map': 'Route17', 'party': [{'species': 'Paras', 'level': 22,
            'experience': 11657, 'hp': 47}], 'stored_pokemon': [], 'bag': {}, 'flags': {},
            'dex': {'owned_species': ['Paras']}}
        results = []
        for visited in (names, list(reversed(names)), names[1:] + names[:1]):
            agent.visited = visited
            groups = {}
            agent.add_nonwild_collection_groups(groups, facts)
            group = groups['register:Parasect:evolution:Paras']
            results.append({'target': group['target'], 'rules': [rule.id for rule in group['rules']],
                'objectives': group['objectives'], 'context': group['context']})
        self.assertTrue(all(result == results[0] for result in results))
        examples = results[0]['context']['training_effort_examples']
        self.assertEqual([example['map'] for example in examples], ['Alpha', 'Beta', 'Delta'])
        self.assertEqual(len({example['estimated_victories_max'] for example in examples}), 1)
        self.assertEqual(results[0]['target'], ('register', 'Parasect', True))

    def test_training_example_priority_remains_victory_count_before_map_name(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex, agent.index = True, Mock(rules=[], by_effect={})
        agent._complete_collection_graph = {'Paras': [], 'Parasect': [{'method': 'evolution',
            'from_species': 'Paras', 'trigger': 'level', 'level': 24}]}
        agent.visited = ['Alpha', 'Zulu', 'SafariZoneEast']
        agent.maps = {name: {'wild': {'red': {'grass': {'encounterRate': 30,
            'mons': [{'species': 'Rattata', 'level': level}] * 10}}}}
            for name, level in (('Alpha', 5), ('Zulu', 30), ('SafariZoneEast', 50))}
        facts = {'map': 'Route17', 'party': [{'species': 'Paras', 'level': 22,
            'experience': 11657, 'hp': 47}], 'stored_pokemon': [], 'bag': {}, 'flags': {},
            'dex': {'owned_species': ['Paras']}}
        groups = {}
        agent.add_nonwild_collection_groups(groups, facts)
        examples = groups['register:Parasect:evolution:Paras']['context']['training_effort_examples']
        self.assertEqual([example['map'] for example in examples], ['Zulu', 'Alpha'])
        self.assertLess(examples[0]['estimated_victories_max'], examples[1]['estimated_victories_max'])


if __name__ == '__main__':
    unittest.main()

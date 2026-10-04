"""Field-name compression preserves types, missingness and all choices."""
from copy import deepcopy
from collections import Counter
import json
import unittest
from unittest.mock import Mock, patch

from openpokered.decision_wire import (FIELD_DICTIONARY, compact_decision_field_wire,
                                      restore_decision_field_wire, expand_decision_evidence)
from openpokered.autonomous_story import (AutonomousStoryAgent, compact_refactored_decision_wire,
    REFACTORED_DECISION_EVIDENCE_INSTRUCTION)
from openpokered.story_agent import DualStoryAgent, StoryStopped
from openpokered.typesafe import TypeSafeClient, TypeSafeError, ChoiceAnswer, SystemOneResult


class DecisionFieldWireTests(unittest.TestCase):
    def fixture(self):
        rows = [{'already_knows_move': i % 2 == 0, 'withdrawal_required': i % 3 == 0,
                 'topological_route_found': None if i % 4 == 0 else True,
                 'expected_steps_to_any_new_species': i,
                 'unregistered_encounter_share_pct': 3.2,
                 'literal': '{"$f0": "literal text"}'} for i in range(60)]
        del rows[1]['topological_route_found']
        return {'world': {'rows': rows}, 'shared_strategy_evidence': {'e0': {'cost': 3}}}, {
            'candidate:50': json.dumps({'context': rows[:20], 'ref': {'shared_strategy_evidence_ref': 'e0'}}),
            'candidate:52': json.dumps({'context': rows[20:40]}), 'none': 'No matching action',
            'string': '"literal JSON string"', 'number': '0'}

    def test_nested_roundtrip_all_options_types_and_missing_fields(self):
        state, options = self.fixture()
        untouched = deepcopy((state, options))
        wire, offered = compact_decision_field_wire(state, options)
        self.assertIn(FIELD_DICTIONARY, wire)
        restored, original = restore_decision_field_wire(wire, offered)
        self.assertEqual(restored, state)
        self.assertEqual(list(offered), list(options))
        for key in options:
            if key.startswith('candidate:'):
                self.assertEqual(json.loads(original[key]), json.loads(options[key]))
            else:
                self.assertEqual(original[key], options[key])
        self.assertEqual((state, options), untouched)
        self.assertEqual(wire['shared_strategy_evidence'], state['shared_strategy_evidence'])

    def test_small_unprofitable_and_no_repeated_names_keep_identity(self):
        for state in ({'world': {'cost': 3}}, {'world': [{'long_field_name': 1}] * 2}):
            options = {'a': '{ "cost": 3 }', 'none': 'No matching action'}
            wire, offered = compact_decision_field_wire(state, options)
            self.assertIs(wire, state)
            self.assertIs(offered, options)

    def test_reserved_collision_in_world_library_or_candidate_refuses(self):
        for key in ('$f0', '$future', FIELD_DICTIONARY):
            for placement in ('world', 'library', 'candidate'):
                state, options = self.fixture()
                if placement == 'world':
                    state['world'][key] = 'ordinary'
                elif placement == 'library':
                    state['shared_strategy_evidence']['e0'][key] = 'ordinary'
                else:
                    options['candidate:50'] = json.dumps({key: 'ordinary'})
                with self.subTest(key=key, placement=placement), self.assertRaisesRegex(ValueError, 'collision'):
                    compact_decision_field_wire(state, options)

    def test_decoder_rejects_missing_duplicate_or_ambiguous_names(self):
        for dictionary, row in (({'$f0': 'cost'}, {'$f1': 1}),
                                ({'$f0': 'cost'}, {'$f0': 1, 'cost': 2}),
                                ({'$f0': 'cost', '$f1': 'cost'}, {}),
                                ({'$f0': '$f1'}, {})):
            with self.subTest(dictionary=dictionary), self.assertRaises(ValueError):
                restore_decision_field_wire({'world': row, FIELD_DICTIONARY: dictionary}, {})

    def test_expansion_rejects_missing_cyclic_and_malformed_table_evidence(self):
        for state in (
            {'world': {'$e': 'missing'}},
            {'world': {'$e': 'e0'}, 'shared_strategy_evidence': {'e0': {'$e': 'e0'}}},
            {'world': {'strategy_table': {'columns': ['cost'], 'rows': [[1, 2]]}}},
            {'world': {'strategy_table': {'columns': ['cost', 'cost'], 'rows': [[1, 2]]}}}):
            with self.subTest(state=state), self.assertRaises(ValueError):
                expand_decision_evidence(state, {})

    def test_refactoring_roundtrip_retains_every_world_value_and_candidate(self):
        state, options = self.fixture()
        untouched = deepcopy((state, options))
        wire, offered = compact_refactored_decision_wire(state, options)
        self.assertIsNot(wire, state)
        raw_state, raw_options = expand_decision_evidence(state, options)
        rebuilt, rebuilt_options = expand_decision_evidence(wire, offered)
        self.assertEqual(rebuilt, raw_state)
        self.assertEqual(list(offered), list(options))
        for key in options:
            if key.startswith('candidate:'):
                self.assertEqual(json.loads(rebuilt_options[key]), json.loads(raw_options[key]))
            else:
                self.assertEqual(rebuilt_options[key], raw_options[key])
        self.assertEqual((state, options), untouched)

    def agent(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.record = Mock()
        agent.model = 'jev-1.13.0'
        agent.model_client = TypeSafeClient('https://offline.invalid', 'offline-test',
                                          agent.model, provider='openrouter')
        return agent

    def production_fixture(self):
        state, all_options = self.fixture()
        return state, {key: all_options[key] for key in ('candidate:50', 'candidate:52')}

    def overflow(self, layer='strategy', message='max_tokens_exceeded'):
        failure = StoryStopped(f'{layer}:service_unavailable')
        failure.__cause__ = TypeSafeError(message)
        return failure

    def test_production_third_format_uses_same_complete_evidence_and_abstention(self):
        for layer in ('strategy', 'action'):
            for abstain in (False, True):
                agent = self.agent()
                state, options = self.production_fixture()
                expected = expand_decision_evidence(state, options)
                calls = []
                def choose(actual_layer, actual, offered, instruction, *, allow_abstain):
                    self.assertEqual(actual_layer, layer)
                    self.assertEqual(allow_abstain, abstain)
                    actual_state, actual_options = expand_decision_evidence(actual, offered)
                    self.assertEqual(actual_state, expected[0])
                    self.assertEqual({key: json.loads(value) for key, value in actual_options.items()},
                                     {key: json.loads(value) for key, value in expected[1].items()})
                    self.assertEqual(list(offered), list(options))
                    calls.append(actual)
                    if FIELD_DICTIONARY not in actual:
                        raise self.overflow(layer)
                    self.assertEqual(instruction.count(REFACTORED_DECISION_EVIDENCE_INSTRUCTION), 1)
                    return 'candidate:52'
                with self.subTest(layer=layer, abstain=abstain), patch.object(DualStoryAgent, 'choose', side_effect=choose):
                    self.assertEqual(agent.choose_bounded_choice(layer, state, options, 'Pick',
                        allow_abstain=abstain), 'candidate:52')
                self.assertEqual(len(calls), 3)
                self.assertEqual(len(agent._choice_context_sizes), 3)
                self.assertFalse(any(call.args[0] == f'{layer}_partition' for call in agent.record.call_args_list))

    def test_production_success_keeps_original_format_without_learning(self):
        agent = self.agent()
        state, options = self.production_fixture()
        with patch.object(DualStoryAgent, 'choose', return_value='candidate:50') as choose:
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), 'candidate:50')
        choose.assert_called_once_with('strategy', state, options, 'Pick', allow_abstain=True)
        self.assertFalse(hasattr(agent, '_refactored_field_dictionary_scopes'))

    def test_all_three_leaf_overflows_fail_closed_without_partition(self):
        agent = self.agent()
        state, options = self.production_fixture()
        failure = self.overflow()
        with patch.object(DualStoryAgent, 'choose', side_effect=failure) as choose:
            with self.assertRaises(StoryStopped) as observed:
                agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertIs(observed.exception, failure)
        self.assertEqual(choose.call_count, 3)
        self.assertFalse(any(call.args[0] == 'strategy_partition' for call in agent.record.call_args_list))

    def test_non_context_error_never_enables_any_fallback(self):
        for message in ('HTTP 401 unauthorized', 'HTTP 402 payment_required', 'HTTP 429 rate_limit'):
            agent = self.agent()
            state, options = self.production_fixture()
            with self.subTest(message=message), patch.object(DualStoryAgent, 'choose', side_effect=self.overflow(message=message)) as choose:
                with self.assertRaises(StoryStopped):
                    agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertEqual(choose.call_count, 1)
            self.assertFalse(hasattr(agent, '_refactored_field_dictionary_scopes'))

    def test_learning_is_endpoint_model_provider_path_layer_and_runtime_scoped(self):
        agent = self.agent()
        state, options = self.production_fixture()
        def choose(layer, actual, offered, instruction, *, allow_abstain):
            if FIELD_DICTIONARY not in actual:
                raise self.overflow(layer)
            return 'candidate:50'
        with patch.object(DualStoryAgent, 'choose', side_effect=choose) as calls:
            agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertEqual(calls.call_count, 3)
            agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertEqual(calls.call_count, 4)
            agent.choose_bounded_choice('action', state, options, 'Pick')
            agent.model_client.base_url = 'https://other-offline.invalid'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model = 'different-model'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model_client.system_one_path = '/other'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model_client.provider = 'typesafe'
            agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertEqual(calls.call_count, 19)
        self.assertEqual(len(agent._refactored_field_dictionary_scopes), 6)
        successor = self.agent()
        with patch.object(DualStoryAgent, 'choose', return_value='candidate:50') as calls:
            successor.choose_bounded_strategy(state, options, 'Pick')
        calls.assert_called_once()

    def test_tournament_still_compares_all_options_and_final_abstention(self):
        agent = self.agent()
        state, original = self.production_fixture()
        options = {str(i): original['candidate:50'] for i in range(16)}
        evaluated, abstentions = set(), []
        raw_state = expand_decision_evidence(state, original)[0]
        def choose(layer, actual, offered, instruction, *, allow_abstain):
            self.assertEqual(expand_decision_evidence(actual, offered)[0], raw_state)
            self.assertLessEqual(instruction.count(REFACTORED_DECISION_EVIDENCE_INSTRUCTION), 1)
            if len(offered) > 2:
                raise self.overflow()
            evaluated.update(offered)
            abstentions.append(allow_abstain)
            return max(offered, key=int)
        with patch.object(DualStoryAgent, 'choose', side_effect=choose):
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), '15')
        self.assertEqual(evaluated, set(options))
        self.assertTrue(abstentions[-1])
        self.assertTrue(all(not value for value in abstentions[:-1]))

    def test_real_adapter_retains_none_and_conditional_choice_probabilities(self):
        for probabilities, selected in (({'candidate:50': 0.35, 'candidate:52': 0.25, 'none': 0.4}, 'candidate:50'),
                                        ({'candidate:50': 0.2, 'candidate:52': 0.1, 'none': 0.7}, None)):
            agent = self.agent()
            agent.layer_jev = {'strategy': True, 'action': True}
            agent.calls, agent.tokens, agent.models = Counter(), Counter(), set()
            agent.max_calls = 6
            agent.check_budget = Mock()
            agent.client = Mock()
            agent.client.state.return_value = {'frame_count': 0}
            state, options = self.production_fixture()
            response = SystemOneResult('offline', {'strategy': ChoiceAnswer('none', probabilities, 0.3)}, 1, 1)
            with self.subTest(probabilities=probabilities), patch.object(agent.model_client, 'system_one',
                    side_effect=[TypeSafeError('max_tokens_exceeded'), TypeSafeError('max_tokens_exceeded'), response]) as transport:
                if selected is None:
                    with self.assertRaisesRegex(StoryStopped, 'strategy:no_selection'):
                        agent.choose_bounded_strategy(state, options, 'Pick')
                else:
                    self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), selected)
            self.assertEqual(transport.call_count, 3)
            for call in transport.call_args_list:
                criteria = call.args[1]['strategy'].criteria
                self.assertEqual(list(criteria), ['candidate:50', 'candidate:52', 'none'])
                self.assertEqual(criteria['none'], 'None of these candidates can advance the current goal.')
            self.assertEqual(agent.calls['strategy'], 3)


if __name__ == '__main__':
    unittest.main()

"""Field-name compression preserves types, missingness and all choices."""
from copy import deepcopy
from collections import Counter
import json
import unittest
from unittest.mock import Mock, patch

from openpokered.decision_wire import (FIELD_DICTIONARY, compact_decision_field_wire,
    restore_decision_field_wire, expand_decision_evidence, STRING_REFERENCE_PREFIX,
    STRING_REFERENCE_INSTRUCTION, compact_decision_string_references,
    restore_decision_string_references)
from openpokered.autonomous_story import (AutonomousStoryAgent, compact_refactored_decision_wire,
    REFACTORED_DECISION_EVIDENCE_INSTRUCTION, compact_string_decision_wire)
from openpokered.story_agent import DualStoryAgent, StoryStopped
from openpokered.typesafe import TypeSafeClient, TypeSafeError, ChoiceAnswer, SystemOneResult


class DecisionMappingTableTests(unittest.TestCase):
    def fixture(self):
        records = {f'area:{i}': {'unregistered_species_count': i,
            'unregistered_encounter_share_pct': None if i % 3 == 0 else i / 10,
            'topological_route_found': i % 2 == 0,
            'nested': {'values': [0, False, None, f'路線{i}']}} for i in range(40)}
        return {'world': {'areas': records}, 'recent': []}, {
            'a': json.dumps({'areas': records}), 'b': '{ "goal": "advance" }',
            'none': 'No matching action', 'scalar': '"literal JSON string"', 'zero': '0'}

    def codec(self):
        from openpokered.decision_wire import (MAPPING_TABLE_SCHEMA, MAPPING_TABLE_INSTRUCTION,
            compact_decision_mapping_tables)
        return MAPPING_TABLE_SCHEMA, MAPPING_TABLE_INSTRUCTION, compact_decision_mapping_tables

    def assert_semantics(self, actual, expected):
        self.assertEqual(actual[0], expected[0])
        self.assertEqual(list(actual[1]), list(expected[1]))
        for key in expected[1]:
            try:
                original = json.loads(expected[1][key])
            except ValueError:
                self.assertEqual(actual[1][key], expected[1][key])
            else:
                self.assertEqual(json.loads(actual[1][key]), original)

    def test_roundtrip_keeps_all_mapping_keys_columns_types_and_order(self):
        schema, _, encode = self.codec()
        state, options = self.fixture()
        untouched = deepcopy((state, options))
        wire, offered = encode(state, options)
        self.assertEqual(wire[schema], 1)
        keys, columns, rows = wire['world']['areas']['$m']
        self.assertEqual(keys, list(state['world']['areas']))
        self.assertEqual(columns, list(state['world']['areas']['area:0']))
        self.assertEqual(len(rows), 40)
        self.assert_semantics(expand_decision_evidence(wire, offered), (state, options))
        self.assertEqual((state, options), untouched)
        for key in ('b', 'none', 'scalar', 'zero'):
            self.assertEqual(offered[key], options[key])

    def test_small_heterogeneous_or_differently_ordered_records_are_not_packed(self):
        _, _, encode = self.codec()
        for records in ({'a': {'v': 1}, 'b': {'v': 2}},
                        {'a': {'v': 1}, 'b': {'w': 2}, 'c': {'v': 3}},
                        {'a': {'v': 1, 'w': 2}, 'b': {'w': 2, 'v': 1}, 'c': {'v': 3, 'w': 4}}):
            state, options = {'world': records}, {'a': 'Literal'}
            wire, offered = encode(state, options)
            self.assertIs(wire, state)
            self.assertIs(offered, options)

    def test_reserved_collisions_in_state_or_candidates_fail_closed(self):
        schema, _, encode = self.codec()
        for reserved in ('$m', schema):
            for location in ('state', 'candidate'):
                state, options = self.fixture()
                if location == 'state':
                    state['world'][reserved] = 'ordinary data'
                else:
                    options['collision'] = json.dumps({reserved: 'ordinary data'})
                with self.subTest(reserved=reserved, location=location), self.assertRaisesRegex(ValueError, 'collision'):
                    encode(state, options)

    def test_malformed_schema_keys_columns_rows_and_mixed_tags_fail_closed(self):
        schema, _, _ = self.codec()
        good = [['a'], ['cost'], [[0]]]
        for version in (True, '1', 2, None):
            with self.subTest(version=version), self.assertRaises(ValueError):
                expand_decision_evidence({schema: version, 'world': {'$m': good}}, {})
        for table in ([], [['a'], ['cost']], [['a', 'a'], ['cost'], [[0], [1]]],
                      [['a'], ['cost', 'cost'], [[0, 1]]], [[0], ['cost'], [[0]]],
                      [['a'], [0], [[0]]], [['a'], ['cost'], []],
                      [['a'], ['cost'], [[0, 1]]], [['a'], ['cost'], [False]],
                      ['a', ['cost'], [[0]]], [['a'], [], [[]]]):
            with self.subTest(table=table), self.assertRaises(ValueError):
                expand_decision_evidence({schema: 1, 'world': {'$m': table}}, {})
        with self.assertRaises(ValueError):
            expand_decision_evidence({schema: 1, 'world': {'$m': good, 'extra': 1}}, {})

    def test_untagged_literals_keep_their_meaning(self):
        state = {'world': {'$m': [['a'], ['cost'], [[0]]]}}
        options = {'a': json.dumps({'$m': 'literal'})}
        self.assertEqual(expand_decision_evidence(state, options), (state, options))

    def test_transitive_references_inside_table_axes_and_cells_expand_before_mapping(self):
        schema, _, _ = self.codec()
        state = {schema: 1, 'world': {'$e': 'e0'}, 'shared_strategy_evidence': {
            'e0': {'$m': [{'$e': 'e1'}, {'$e': 'e2'}, [[{'$e': 'e3'}]]]},
            'e1': ['area'], 'e2': ['cost'], 'e3': {'null': None, 'false': False, 'zero': 0}}}
        restored, options = expand_decision_evidence(state, {'a': '{"record":{"$e":"e0"}}'})
        self.assertEqual(restored, {'world': {'area': {'cost': {'null': None, 'false': False, 'zero': 0}}}})
        self.assertEqual(json.loads(options['a'])['record'], restored['world'])

    def test_full_refactoring_roundtrip_and_profit_includes_all_guidance(self):
        from openpokered.autonomous_story import compact_mapped_decision_wire
        schema, guidance, _ = self.codec()
        state, options = self.fixture()
        wire, offered = compact_mapped_decision_wire(state, options)
        self.assertIn(schema, wire)
        self.assert_semantics(expand_decision_evidence(wire, offered), (state, options))
        before = len(json.dumps({'state': state, 'criteria': options}).encode())
        after = len(json.dumps({'state': wire, 'criteria': offered}).encode())
        from openpokered.decision_wire import FIELD_DICTIONARY_INSTRUCTION
        all_guidance = (REFACTORED_DECISION_EVIDENCE_INSTRUCTION + FIELD_DICTIONARY_INSTRUCTION
                        + STRING_REFERENCE_INSTRUCTION + guidance)
        self.assertLess(after + len(all_guidance.encode()), before)

    def test_coarse_reference_density_preserves_full_semantics_and_plain_criteria(self):
        from openpokered.autonomous_story import compact_mapped_decision_wire
        state, options = self.fixture()
        dense, _ = compact_mapped_decision_wire(state, options)
        coarse, offered = compact_mapped_decision_wire(state, options, min_chars=160)
        self.assert_semantics(expand_decision_evidence(coarse, offered), (state, options))
        self.assertLess(len(coarse['shared_strategy_evidence']), len(dense['shared_strategy_evidence']))
        for key in ('none', 'scalar', 'zero'):
            self.assertEqual(offered[key], options[key])

    def test_coarse_density_is_lazy_and_learned_only_after_explicit_mapped_overflow(self):
        from openpokered.autonomous_story import compact_mapped_decision_wire
        helper = DecisionFieldWireTests()
        state, all_options = self.fixture()
        options = {key: all_options[key] for key in ('a', 'b')}
        coarse, _ = compact_mapped_decision_wire(state, options, min_chars=160)
        expected_entries = len(coarse['shared_strategy_evidence'])
        for layer in ('strategy', 'action'):
            agent = helper.agent()
            def choose(actual_layer, actual, offered, instruction, *, allow_abstain):
                self.assertEqual(actual_layer, layer)
                self.assert_semantics(expand_decision_evidence(actual, offered), (state, options))
                if len(actual.get('shared_strategy_evidence', {})) != expected_entries:
                    raise helper.overflow(layer)
                return 'b'
            with patch.object(DualStoryAgent, 'choose', side_effect=choose) as calls:
                self.assertEqual(agent.choose_bounded_choice(layer, state, options, 'Pick'), 'b')
                first = calls.call_count
                self.assertGreater(first, 1)
                self.assertEqual(agent.choose_bounded_choice(layer, state, options, 'Pick'), 'b')
                self.assertEqual(calls.call_count, first + 1)
            enabled = [call.kwargs['encoding'] for call in agent.record.call_args_list
                       if call.args[0] == layer + '_wire_encoding_enabled']
            self.assertIn('mapped_record_coarse_references', enabled)
            self.assertEqual(len(agent._coarse_mapped_reference_scopes), 1)
            self.assertFalse(any(call.args[0] == layer + '_partition' for call in agent.record.call_args_list))

    def test_coarse_density_non_context_errors_and_new_runtime_do_not_learn(self):
        helper = DecisionFieldWireTests()
        state, all_options = self.fixture()
        options = {key: all_options[key] for key in ('a', 'b')}
        for message in ('HTTP 401 unauthorized', 'HTTP 402 payment_required', 'HTTP 429 rate_limit'):
            agent = helper.agent()
            with patch.object(DualStoryAgent, 'choose', side_effect=helper.overflow(message=message)) as calls:
                with self.assertRaises(StoryStopped):
                    agent.choose_bounded_strategy(state, options, 'Pick')
            calls.assert_called_once()
            self.assertFalse(hasattr(agent, '_coarse_mapped_reference_scopes'))
        agent = helper.agent()
        with patch.object(DualStoryAgent, 'choose', return_value='a') as calls:
            agent.choose_bounded_strategy(state, options, 'Pick')
        calls.assert_called_once()
        self.assertFalse(hasattr(agent, '_coarse_mapped_reference_scopes'))

    def test_coarse_density_learning_is_endpoint_model_provider_path_and_layer_scoped(self):
        from openpokered.autonomous_story import compact_mapped_decision_wire
        helper, agent = DecisionFieldWireTests(), DecisionFieldWireTests().agent()
        state, all_options = self.fixture()
        options = {key: all_options[key] for key in ('a', 'b')}
        coarse, _ = compact_mapped_decision_wire(state, options, min_chars=160)
        def choose(layer, actual, offered, instruction, *, allow_abstain):
            if len(actual.get('shared_strategy_evidence', {})) != len(coarse['shared_strategy_evidence']):
                raise helper.overflow(layer)
            return 'a'
        with patch.object(DualStoryAgent, 'choose', side_effect=choose) as calls:
            agent.choose_bounded_strategy(state, options, 'Pick')
            first = calls.call_count
            agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertEqual(calls.call_count, first + 1)
            agent.choose_bounded_choice('action', state, options, 'Pick')
            agent.model_client.base_url = 'https://other-offline.invalid'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model = 'different-model'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model_client.system_one_path = '/other'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model_client.provider = 'typesafe'
            agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertEqual(calls.call_count, first * 6 + 1)
        self.assertEqual(len(agent._coarse_mapped_reference_scopes), 6)

    def test_identical_coarse_format_does_not_repeat_a_failed_mapped_leaf(self):
        from openpokered.autonomous_story import compact_mapped_decision_wire
        helper = DecisionFieldWireTests()
        state, all_options = self.fixture()
        options = {key: all_options[key] for key in ('a', 'b')}
        def same_density(actual, offered, **kwargs):
            return compact_mapped_decision_wire(actual, offered)
        agent = helper.agent()
        with patch('openpokered.autonomous_story.compact_mapped_decision_wire', side_effect=same_density), \
                patch.object(DualStoryAgent, 'choose', side_effect=helper.overflow()):
            with self.assertRaises(StoryStopped):
                agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertFalse(hasattr(agent, '_coarse_mapped_reference_scopes'))
        self.assertFalse(any(call.args[0] == 'strategy_partition' for call in agent.record.call_args_list))

    def test_production_lazy_fallback_and_same_scope_learning_preserve_all_options(self):
        schema, guidance, _ = self.codec()
        helper = DecisionFieldWireTests()
        state, all_options = self.fixture()
        options = {key: all_options[key] for key in ('a', 'b')}
        for layer in ('strategy', 'action'):
            for abstain in (False, True):
                agent, calls = helper.agent(), []
                expected = expand_decision_evidence(state, options)
                def choose(actual_layer, actual, offered, instruction, *, allow_abstain):
                    self.assertEqual(actual_layer, layer)
                    self.assertEqual(allow_abstain, abstain)
                    self.assertEqual(list(offered), list(options))
                    self.assert_semantics(expand_decision_evidence(actual, offered), expected)
                    calls.append(actual)
                    if schema not in actual:
                        raise helper.overflow(layer)
                    self.assertEqual(instruction.count(guidance), 1)
                    return 'b'
                with patch.object(DualStoryAgent, 'choose', side_effect=choose):
                    self.assertEqual(agent.choose_bounded_choice(layer, state, options, 'Pick',
                        allow_abstain=abstain), 'b')
                    first_calls = len(calls)
                    self.assertGreater(first_calls, 1)
                    self.assertEqual(agent.choose_bounded_choice(layer, state, options, 'Pick',
                        allow_abstain=abstain), 'b')
                    self.assertEqual(len(calls), first_calls + 1)
                self.assertFalse(any(call.args[0] == f'{layer}_partition' for call in agent.record.call_args_list))
                self.assertEqual(len(agent._mapped_record_table_scopes), 1)
        agent = helper.agent()
        with patch('openpokered.autonomous_story.compact_mapped_decision_wire', side_effect=AssertionError('not lazy')):
            with patch.object(DualStoryAgent, 'choose', return_value='a'):
                self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), 'a')

    def test_exhausted_leaf_and_non_context_errors_do_not_choose_or_partition(self):
        self.codec()
        helper = DecisionFieldWireTests()
        state, all_options = self.fixture()
        options = {key: all_options[key] for key in ('a', 'b')}
        agent = helper.agent()
        with patch.object(DualStoryAgent, 'choose', side_effect=helper.overflow()) as calls:
            with self.assertRaises(StoryStopped):
                agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertGreater(calls.call_count, 1)
        self.assertFalse(any(call.args[0] == 'strategy_partition' for call in agent.record.call_args_list))
        for message in ('HTTP 401 unauthorized', 'HTTP 402 payment_required', 'HTTP 429 rate_limit'):
            agent = helper.agent()
            with patch.object(DualStoryAgent, 'choose', side_effect=helper.overflow(message=message)) as calls:
                with self.assertRaises(StoryStopped):
                    agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertEqual(calls.call_count, 1)
            self.assertFalse(hasattr(agent, '_mapped_record_table_scopes'))

    def test_mapping_learning_is_endpoint_model_provider_path_layer_and_runtime_scoped(self):
        schema, _, _ = self.codec()
        helper, agent = DecisionFieldWireTests(), DecisionFieldWireTests().agent()
        state, all_options = self.fixture()
        options = {key: all_options[key] for key in ('a', 'b')}
        def choose(layer, actual, offered, instruction, *, allow_abstain):
            if schema not in actual:
                raise helper.overflow(layer)
            return 'a'
        with patch.object(DualStoryAgent, 'choose', side_effect=choose) as calls:
            agent.choose_bounded_strategy(state, options, 'Pick')
            first_calls = calls.call_count
            agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertEqual(calls.call_count, first_calls + 1)
            agent.choose_bounded_choice('action', state, options, 'Pick')
            agent.model_client.base_url = 'https://other-offline.invalid'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model = 'different-model'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model_client.system_one_path = '/other'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model_client.provider = 'typesafe'
            agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertEqual(calls.call_count, first_calls * 6 + 1)
        self.assertEqual(len(agent._mapped_record_table_scopes), 6)
        successor = helper.agent()
        with patch.object(DualStoryAgent, 'choose', return_value='a') as calls:
            successor.choose_bounded_strategy(state, options, 'Pick')
        calls.assert_called_once()

    def test_real_adapter_mapped_format_preserves_none_and_conditional_probabilities(self):
        schema, _, _ = self.codec()
        helper = DecisionFieldWireTests()
        for probabilities, selected in (({'a': .35, 'b': .25, 'none': .4}, 'a'),
                                        ({'a': .2, 'b': .1, 'none': .7}, None)):
            agent = helper.agent()
            agent.layer_jev = {'strategy': True, 'action': True}
            agent.calls, agent.tokens, agent.models = Counter(), Counter(), set()
            agent.max_calls, agent.check_budget, agent.client = 8, Mock(), Mock()
            agent.client.state.return_value = {'frame_count': 0}
            state, all_options = self.fixture()
            options = {key: all_options[key] for key in ('a', 'b')}
            response = SystemOneResult('offline', {'strategy': ChoiceAnswer('none', probabilities, .3)}, 1, 1)
            def respond(actual, questions, **kwargs):
                if schema not in actual:
                    raise TypeSafeError('max_tokens_exceeded')
                return response
            with patch.object(agent.model_client, 'system_one', side_effect=respond) as transport:
                if selected is None:
                    with self.assertRaisesRegex(StoryStopped, 'strategy:no_selection'):
                        agent.choose_bounded_strategy(state, options, 'Pick')
                else:
                    self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), selected)
            self.assertGreater(transport.call_count, 1)
            for call in transport.call_args_list:
                criteria = call.args[1]['strategy'].criteria
                self.assertEqual(list(criteria), ['a', 'b', 'none'])
                self.assertEqual(criteria['none'], 'None of these candidates can advance the current goal.')
            self.assertEqual(agent.calls['strategy'], transport.call_count)

    def test_mapping_fallback_tournament_keeps_every_option_and_final_none(self):
        self.codec()
        helper, agent = DecisionFieldWireTests(), DecisionFieldWireTests().agent()
        state, offered = self.fixture()
        options = {str(i): offered['a'] for i in range(8)}
        evaluated, abstentions = set(), []
        expected_state = expand_decision_evidence(state, options)[0]
        def choose(layer, actual, candidates, instruction, *, allow_abstain):
            self.assertEqual(expand_decision_evidence(actual, candidates)[0], expected_state)
            if len(candidates) > 2:
                raise helper.overflow()
            evaluated.update(candidates)
            abstentions.append(allow_abstain)
            return max(candidates, key=int)
        with patch.object(DualStoryAgent, 'choose', side_effect=choose):
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), '7')
        self.assertEqual(evaluated, set(options))
        self.assertTrue(abstentions[-1])
        self.assertTrue(all(not value for value in abstentions[:-1]))


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

    def test_all_four_leaf_overflows_fail_closed_without_partition(self):
        agent = self.agent()
        state, options = self.production_fixture()
        failure = self.overflow()
        with patch.object(DualStoryAgent, 'choose', side_effect=failure) as choose:
            with self.assertRaises(StoryStopped) as observed:
                agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertIs(observed.exception, failure)
        self.assertEqual(choose.call_count, 4)
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


class DecisionStringReferenceTests(unittest.TestCase):
    def fixture(self):
        state = {'world': [{'item': {'$e': 'e1'}, 'literal': '@email'} for _ in range(100)],
                 'shared_strategy_evidence': {'e0': {'absent': None, 'false': False, 'zero': 0,
                    'nested': [{'species': 'Parasect', 'level': 24}]},
                    'e1': [{'value': {'$e': 'e0'}}]}}
        options = {'a': json.dumps({'context': [{'ref': {'$e': 'e1'}} for _ in range(50)]}),
                   'none': 'No matching action', 'scalar': '"literal JSON string"', 'zero': '0'}
        return state, options

    def test_roundtrip_transitive_typed_values_options_and_unmodified_inputs(self):
        state, options = self.fixture()
        untouched = deepcopy((state, options))
        expected = expand_decision_evidence(state, options)
        wire, offered = compact_decision_string_references(state, options)
        self.assertEqual(wire[STRING_REFERENCE_PREFIX], '@')
        self.assertEqual(wire['world'][0]['item'], '@e1')
        restored, original = restore_decision_string_references(wire, offered)
        self.assertEqual(restored, state)
        self.assertEqual(json.loads(original['a']), json.loads(options['a']))
        self.assertEqual(list(offered), list(options))
        self.assertEqual(expand_decision_evidence(wire, offered)[0], expected[0])
        self.assertEqual((state, options), untouched)
        for key in ('none', 'scalar', 'zero'):
            self.assertEqual(original[key], options[key])

    def test_small_or_reference_free_requests_retain_identity(self):
        for state in ({'world': '@email'}, {'world': {'$e': 'e0'}, 'shared_strategy_evidence': {'e0': 1}}):
            options = {'a': 'literal'}
            wire, offered = compact_decision_string_references(state, options)
            self.assertIs(wire, state)
            self.assertIs(offered, options)

    def test_reserved_literal_and_metadata_collisions_fail_closed(self):
        for location in ('world', 'library', 'candidate'):
            for value in ('@e0', {STRING_REFERENCE_PREFIX: '@'}):
                state, options = self.fixture()
                if location == 'world':
                    state['world'].append(value)
                elif location == 'library':
                    state['shared_strategy_evidence']['e2'] = value
                else:
                    options['collision'] = json.dumps(value)
                with self.subTest(location=location, value=value), self.assertRaises(ValueError):
                    compact_decision_string_references(state, options)

    def test_missing_invalid_targets_and_metadata_are_rejected(self):
        for target in ('missing', 'e999', 0):
            state, options = self.fixture()
            state['world'][0]['item'] = {'$e': target}
            with self.subTest(target=target), self.assertRaises(ValueError):
                compact_decision_string_references(state, options)
        for state in ({STRING_REFERENCE_PREFIX: '!', 'shared_strategy_evidence': {}},
                      {STRING_REFERENCE_PREFIX: '@', 'world': '@e9', 'shared_strategy_evidence': {}},
                      {STRING_REFERENCE_PREFIX: '@', 'world': '@e0'}):
            with self.subTest(state=state), self.assertRaises(ValueError):
                restore_decision_string_references(state, {})

    def test_untagged_literals_and_mixed_objects_are_not_reference_objects(self):
        literal = {'world': '@e0', 'shared_strategy_evidence': {'e0': 1}}
        self.assertIs(restore_decision_string_references(literal, {})[0], literal)
        state, options = self.fixture()
        state['world'].append({'$e': 'e0', 'meaning': 'literal mixed object'})
        wire, offered = compact_decision_string_references(state, options)
        self.assertEqual(wire['world'][-1], state['world'][-1])
        self.assertEqual(restore_decision_string_references(wire, offered)[0], state)

    def test_cyclic_tagged_references_do_not_expand(self):
        state = {STRING_REFERENCE_PREFIX: '@', 'world': '@e0',
                 'shared_strategy_evidence': {'e0': '@e1', 'e1': '@e0'}}
        with self.assertRaisesRegex(ValueError, 'cyclic'):
            expand_decision_evidence(state, {})

    def test_refactored_field_and_string_wire_roundtrip(self):
        state, options = DecisionFieldWireTests().production_fixture()
        expected = expand_decision_evidence(state, options)
        wire, offered = compact_string_decision_wire(state, options)
        self.assertIn(STRING_REFERENCE_PREFIX, wire)
        actual = expand_decision_evidence(wire, offered)
        self.assertEqual(actual[0], expected[0])
        self.assertEqual({key: json.loads(value) for key, value in actual[1].items()},
                         {key: json.loads(value) for key, value in expected[1].items()})

    def test_full_pipeline_can_profit_without_a_profitable_field_dictionary(self):
        state = {'world': [{'value': 'repeat payload!'} for _ in range(100)]}
        options = {'a': '{"goal":"advance"}', 'none': 'No matching action'}
        wire, offered = compact_string_decision_wire(state, options)
        self.assertIn(STRING_REFERENCE_PREFIX, wire)
        self.assertNotIn(FIELD_DICTIONARY, wire)
        self.assertEqual(expand_decision_evidence(wire, offered), (state, options))

    def test_production_fourth_format_keeps_complete_evidence_and_abstention(self):
        helper = DecisionFieldWireTests()
        for layer in ('strategy', 'action'):
            for abstain in (False, True):
                agent = helper.agent()
                state, options = helper.production_fixture()
                expected = expand_decision_evidence(state, options)
                calls = []
                def choose(actual_layer, actual, offered, instruction, *, allow_abstain):
                    self.assertEqual(actual_layer, layer)
                    self.assertEqual(allow_abstain, abstain)
                    self.assertEqual(list(offered), list(options))
                    expanded = expand_decision_evidence(actual, offered)
                    self.assertEqual(expanded[0], expected[0])
                    self.assertEqual({key: json.loads(value) for key, value in expanded[1].items()},
                                     {key: json.loads(value) for key, value in expected[1].items()})
                    calls.append(actual)
                    if STRING_REFERENCE_PREFIX not in actual:
                        raise helper.overflow(layer)
                    self.assertEqual(instruction.count(STRING_REFERENCE_INSTRUCTION), 1)
                    return 'candidate:52'
                with self.subTest(layer=layer, abstain=abstain), patch.object(DualStoryAgent, 'choose', side_effect=choose):
                    self.assertEqual(agent.choose_bounded_choice(layer, state, options, 'Pick',
                        allow_abstain=abstain), 'candidate:52')
                self.assertEqual(len(calls), 4)
                self.assertEqual(len(agent._choice_context_sizes), 4)
                self.assertFalse(any(call.args[0] == f'{layer}_partition' for call in agent.record.call_args_list))

    def test_new_format_is_lazy_and_non_context_error_does_not_retry(self):
        helper = DecisionFieldWireTests()
        agent = helper.agent()
        state, options = helper.production_fixture()
        with patch('openpokered.autonomous_story.compact_string_decision_wire', side_effect=AssertionError('not lazy')):
            with patch.object(DualStoryAgent, 'choose', return_value='candidate:50'):
                self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), 'candidate:50')
        for message in ('HTTP 401 unauthorized', 'HTTP 402 payment_required', 'HTTP 429 rate_limit'):
            agent = helper.agent()
            with self.subTest(message=message), patch.object(DualStoryAgent, 'choose',
                    side_effect=[helper.overflow(), helper.overflow(), helper.overflow(message=message)]) as choose:
                with self.assertRaises(StoryStopped):
                    agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertEqual(choose.call_count, 3)
            self.assertFalse(hasattr(agent, '_string_evidence_reference_scopes'))

    def test_string_learning_is_endpoint_model_provider_path_layer_and_runtime_scoped(self):
        helper = DecisionFieldWireTests()
        agent = helper.agent()
        state, options = helper.production_fixture()
        def choose(layer, actual, offered, instruction, *, allow_abstain):
            if STRING_REFERENCE_PREFIX not in actual:
                raise helper.overflow(layer)
            return 'candidate:50'
        with patch.object(DualStoryAgent, 'choose', side_effect=choose) as calls:
            agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertEqual(calls.call_count, 4)
            agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertEqual(calls.call_count, 5)
            agent.choose_bounded_choice('action', state, options, 'Pick')
            agent.model_client.base_url = 'https://other-offline.invalid'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model = 'different-model'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model_client.system_one_path = '/other'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model_client.provider = 'typesafe'
            agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertEqual(calls.call_count, 25)
        self.assertEqual(len(agent._string_evidence_reference_scopes), 6)
        successor = helper.agent()
        with patch.object(DualStoryAgent, 'choose', return_value='candidate:50') as calls:
            successor.choose_bounded_strategy(state, options, 'Pick')
        calls.assert_called_once()

    def test_real_adapter_fourth_format_preserves_none_conditional_probabilities(self):
        helper = DecisionFieldWireTests()
        for probabilities, selected in (({'candidate:50': 0.35, 'candidate:52': 0.25, 'none': 0.4}, 'candidate:50'),
                                        ({'candidate:50': 0.2, 'candidate:52': 0.1, 'none': 0.7}, None)):
            agent = helper.agent()
            agent.layer_jev = {'strategy': True, 'action': True}
            agent.calls, agent.tokens, agent.models = Counter(), Counter(), set()
            agent.max_calls, agent.check_budget, agent.client = 8, Mock(), Mock()
            agent.client.state.return_value = {'frame_count': 0}
            state, options = helper.production_fixture()
            response = SystemOneResult('offline', {'strategy': ChoiceAnswer('none', probabilities, 0.3)}, 1, 1)
            with self.subTest(probabilities=probabilities), patch.object(agent.model_client, 'system_one',
                    side_effect=[TypeSafeError('max_tokens_exceeded')] * 3 + [response]) as transport:
                if selected is None:
                    with self.assertRaisesRegex(StoryStopped, 'strategy:no_selection'):
                        agent.choose_bounded_strategy(state, options, 'Pick')
                else:
                    self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), selected)
            self.assertEqual(transport.call_count, 4)
            for call in transport.call_args_list:
                criteria = call.args[1]['strategy'].criteria
                self.assertEqual(list(criteria), ['candidate:50', 'candidate:52', 'none'])
            self.assertEqual(agent.calls['strategy'], 4)


if __name__ == '__main__':
    unittest.main()

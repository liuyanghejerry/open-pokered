"""Mixed party/PC record lists remain complete through overflow-only packing."""
from copy import deepcopy
from collections import Counter
import json
import unittest
from unittest.mock import Mock, patch

from openpokered.decision_wire import (SEQUENCE_TABLE_SCHEMA, SEQUENCE_TABLE_INSTRUCTION,
    compact_decision_sequence_tables, expand_decision_evidence)
from openpokered.autonomous_story import compact_mapped_decision_wire
from openpokered.story_agent import DualStoryAgent, StoryStopped
from openpokered.typesafe import TypeSafeError, ChoiceAnswer, SystemOneResult
import test_openpokered_decision_wire as fixtures


class SequenceWireTests(unittest.TestCase):
    def fixture(self):
        party = [{'origin': 'party', 'index': i, 'species': f'Species{i}',
            'compatible_with_owned_tm': True, 'observed_moves': ['Tackle', 'None'],
            'known_experience_boundary': None if i % 2 else 0} for i in range(6)]
        pc = [{'origin': 'pc', 'index': i % 20, 'box': i // 20, 'species': f'Species{i+6}',
            'compatible_with_owned_tm': i % 2 == 0, 'observed_moves': ['Tackle', 'None'],
            'known_experience_boundary': 200 + i} for i in range(53)]
        return {'world': {'recipients': party + pc}, 'recent': []}, {
            'a': json.dumps({'establish': ['sale', 'Tm34', False], 'recipients': party + pc}),
            'b': json.dumps({'establish': ['sale', 'Tm38', False], 'recipients': party + pc}),
            'none': 'Nothing fits', 'scalar': '"literal JSON string"', 'zero': '0'}

    def assert_semantics(self, wire, expected):
        state, options = expand_decision_evidence(*wire)
        self.assertEqual(state, expected[0])
        self.assertEqual(list(options), list(expected[1]))
        for key, value in expected[1].items():
            try:
                original = json.loads(value)
            except ValueError:
                self.assertEqual(options[key], value)
            else:
                self.assertEqual(json.loads(options[key]), original)

    def test_mixed_contiguous_shapes_roundtrip_without_null_filling_or_sorting(self):
        state, options = self.fixture()
        untouched = deepcopy((state, options))
        wire, offered = compact_decision_sequence_tables(state, options)
        self.assertEqual(wire[SEQUENCE_TABLE_SCHEMA], 1)
        segments = wire['world']['recipients']['$s']
        self.assertEqual(len(segments), 2)
        self.assertEqual([len(row['strategy_table']['rows']) for row in segments], [6, 53])
        self.assertNotIn('box', segments[0]['strategy_table']['columns'])
        self.assertIn('box', segments[1]['strategy_table']['columns'])
        self.assert_semantics((wire, offered), (state, options))
        self.assertEqual((state, options), untouched)
        for key in ('none', 'scalar', 'zero'):
            self.assertEqual(offered[key], options[key])

    def test_plain_segments_nested_lists_order_and_distinct_missingness_survive(self):
        state, options = self.fixture()
        records = state['world']['recipients']
        records[3:3] = [None, False, 0, ['literal'], {'box': None}, {}]
        state['nested'] = [deepcopy(records), deepcopy(records)]
        wire, offered = compact_decision_sequence_tables(state, options)
        self.assertIn(SEQUENCE_TABLE_SCHEMA, wire)
        self.assert_semantics((wire, offered), (state, options))

    def test_small_or_single_shape_lists_do_not_add_a_segment_protocol(self):
        for value in ([{'x': 1}], [{'x': 1}, {'y': 2}],
                      [{'long_field_name': i} for i in range(40)]):
            state, options = {'world': value}, {'none': 'Literal'}
            wire, offered = compact_decision_sequence_tables(state, options)
            self.assertIs(wire, state)
            self.assertIs(offered, options)

    def test_reserved_tags_in_world_or_candidates_fail_closed(self):
        for reserved in ('$s', SEQUENCE_TABLE_SCHEMA):
            for where in ('world', 'candidate'):
                state, options = self.fixture()
                if where == 'world':
                    state['world'][reserved] = 'literal'
                else:
                    options['collision'] = json.dumps({reserved: 'literal'})
                with self.subTest(reserved=reserved, where=where), self.assertRaises(ValueError):
                    compact_decision_sequence_tables(state, options)

    def test_bad_schema_mixed_tags_and_nonlist_segments_are_rejected(self):
        for version in (True, '1', 2, None):
            with self.subTest(version=version), self.assertRaises(ValueError):
                expand_decision_evidence({SEQUENCE_TABLE_SCHEMA: version, 'world': {'$s': [[]]}}, {})
        for value in ({'$s': []}, {'$s': None}, {'$s': ['literal']},
                      {'$s': [[1]], 'other': True}, {'$s': [{'cost': 1}]}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                expand_decision_evidence({SEQUENCE_TABLE_SCHEMA: 1, 'world': value}, {})

    def test_untagged_s_literals_are_not_spliced(self):
        state, options = {'world': {'$s': [[0], [None]]}}, {'a': '{"$s":"literal"}'}
        self.assertEqual(expand_decision_evidence(state, options), (state, options))

    def test_transitive_references_and_nested_table_cells_expand_in_order(self):
        state = {SEQUENCE_TABLE_SCHEMA: 1, 'world': {'$e': 'e0'}, 'shared_strategy_evidence': {
            'e0': {'$s': [{'$e': 'e1'}, [{'literal': [0, False, None]}]]},
            'e1': {'strategy_table': {'columns': ['species', 'box'],
                'rows': [['Abra', {'$e': 'e2'}]]}}, 'e2': None}}
        original, options = expand_decision_evidence(state, {'a': '{"recipients":{"$e":"e0"}}'})
        self.assertEqual(original['world'], [{'species': 'Abra', 'box': None}, {'literal': [0, False, None]}])
        self.assertEqual(json.loads(options['a'])['recipients'], original['world'])

    def test_combined_codec_preserves_complete_state_options_and_pays_for_guidance(self):
        state, options = self.fixture()
        wire, offered = compact_mapped_decision_wire(state, options,
            min_chars=160, alias_fields=False, sequence_tables=True)
        self.assertIn(SEQUENCE_TABLE_SCHEMA, wire)
        self.assertNotIn('decision_field_dictionary', wire)
        self.assert_semantics((wire, offered), (state, options))
        self.assertLess(len(json.dumps([wire, offered]).encode()) + len(SEQUENCE_TABLE_INSTRUCTION),
            len(json.dumps([state, options]).encode()))

    def test_runtime_lazy_leaf_fallback_is_learned_without_pruning_or_reused_answer(self):
        helper = fixtures.DecisionFieldWireTests()
        state, all_options = self.fixture()
        options = {key: all_options[key] for key in ('a', 'b')}
        for layer in ('strategy', 'action'):
            agent = helper.agent()
            def choose(actual_layer, actual, offered, instruction, *, allow_abstain):
                self.assertEqual(actual_layer, layer)
                self.assertTrue(allow_abstain)
                self.assert_semantics((actual, offered), (state, options))
                if SEQUENCE_TABLE_SCHEMA not in actual:
                    raise helper.overflow(layer)
                self.assertIn(SEQUENCE_TABLE_INSTRUCTION, instruction)
                return 'b'
            with patch.object(DualStoryAgent, 'choose', side_effect=choose) as calls:
                self.assertEqual(agent.choose_bounded_choice(layer, state, options, 'Pick'), 'b')
                first = calls.call_count
                self.assertGreater(first, 1)
                self.assertEqual(agent.choose_bounded_choice(layer, state, options, 'Pick'), 'b')
                self.assertEqual(calls.call_count, first + 1)
            self.assertEqual(len(agent._sequence_mapped_field_scopes), 1)
            self.assertFalse(any(call.args[0] == layer + '_partition' for call in agent.record.call_args_list))

    def test_normal_success_and_non_context_errors_never_enable_sequence_encoding(self):
        helper = fixtures.DecisionFieldWireTests()
        state, options = self.fixture()
        for failure in (None, 'HTTP 401 unauthorized', 'HTTP 402 payment_required', 'HTTP 429 rate_limit'):
            agent = helper.agent()
            with patch('openpokered.autonomous_story.compact_mapped_decision_wire',
                    wraps=compact_mapped_decision_wire) as encode, \
                    patch.object(DualStoryAgent, 'choose', return_value='a',
                        side_effect=helper.overflow(message=failure) if failure else None) as calls:
                if failure:
                    with self.assertRaises(StoryStopped):
                        agent.choose_bounded_strategy(state, options, 'Pick')
                else:
                    self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), 'a')
                calls.assert_called_once()
                encode.assert_not_called()
            self.assertFalse(hasattr(agent, '_sequence_mapped_field_scopes'))

    def test_learning_is_separate_per_layer_model_endpoint_provider_and_path(self):
        helper, agent = fixtures.DecisionFieldWireTests(), fixtures.DecisionFieldWireTests().agent()
        state, all_options = self.fixture()
        options = {key: all_options[key] for key in ('a', 'b')}
        def choose(layer, actual, offered, instruction, *, allow_abstain):
            self.assert_semantics((actual, offered), (state, options))
            if SEQUENCE_TABLE_SCHEMA not in actual:
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
            agent.model = 'another-model'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model_client.system_one_path = '/other'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model_client.provider = 'typesafe'
            agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertEqual(calls.call_count, first * 6 + 1)
        self.assertEqual(len(agent._sequence_mapped_field_scopes), 6)

    def test_exhausted_sequence_leaf_fails_closed_preserving_every_option(self):
        helper, agent = fixtures.DecisionFieldWireTests(), fixtures.DecisionFieldWireTests().agent()
        state, all_options = self.fixture()
        options = {key: all_options[key] for key in ('a', 'b')}
        requests = []
        def choose(layer, actual, offered, instruction, *, allow_abstain):
            self.assert_semantics((actual, offered), (state, options))
            requests.append(json.dumps([actual, offered]))
            raise helper.overflow(layer)
        with patch.object(DualStoryAgent, 'choose', side_effect=choose):
            with self.assertRaises(StoryStopped):
                agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertEqual(len(requests), len(set(requests)))
        self.assertFalse(any(call.args[0] == 'strategy_partition' for call in agent.record.call_args_list))

    def test_real_transport_keeps_none_and_whole_distribution_in_new_format(self):
        helper, agent = fixtures.DecisionFieldWireTests(), fixtures.DecisionFieldWireTests().agent()
        state, all_options = self.fixture()
        options = {key: all_options[key] for key in ('a', 'b')}
        agent.layer_jev = {'strategy': True, 'action': True}
        agent.max_calls, agent.calls, agent.tokens, agent.models = 100, Counter(), Counter(), set()
        agent.check_budget, agent.client = Mock(), Mock()
        seen = []
        def transport(actual, questions, *, model):
            question = questions['strategy']
            self.assertEqual(list(question.criteria), ['a', 'b', 'none'])
            self.assertEqual(question.criteria['none'], 'None of these candidates can advance the current goal.')
            self.assert_semantics((actual, {k: question.criteria[k] for k in ('a', 'b')}), (state, options))
            seen.append(actual)
            if SEQUENCE_TABLE_SCHEMA not in actual:
                raise TypeSafeError('max_tokens_exceeded')
            return SystemOneResult('mock-jev', {'strategy': ChoiceAnswer('a', .8,
                {'a': .8, 'b': .1, 'none': .1})}, 200, 20)
        with patch.object(agent.model_client, 'system_one', side_effect=transport) as calls:
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), 'a')
            self.assertGreater(calls.call_count, 1)
        self.assertIn(SEQUENCE_TABLE_SCHEMA, seen[-1])
        self.assertEqual(agent.calls['strategy'], len(seen))


if __name__ == '__main__':
    unittest.main()

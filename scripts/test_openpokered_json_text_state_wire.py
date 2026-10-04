"""Overflow-only compact JSON state text preserves every decision fact/option."""
from collections import Counter
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import openpokered.decision_wire as wire
from openpokered.autonomous_story import AutonomousStoryAgent, compact_mapped_decision_wire
from openpokered.story_agent import DualStoryAgent, StoryStopped
from openpokered.story_rules import Rule
from openpokered.typesafe import ChoiceAnswer, SystemOneResult, TypeSafeError
import test_openpokered_decision_wire as fixtures
import test_openpokered_sequence_wire as sequence_fixtures
import test_jev_dex_dashboard as dashboard_fixtures
from openpokered.run_autonomous import checkpoint_legacy_navigation_history


class JsonTextStateWireTests(unittest.TestCase):
    def codec(self):
        encode = getattr(wire, 'compact_decision_json_text_state', None)
        self.assertTrue(callable(encode), 'A lossless JSON-text root-state codec is required')
        return encode, wire.JSON_TEXT_STATE_PREFIX, wire.JSON_TEXT_STATE_INSTRUCTION

    def fixture(self):
        state, offered = sequence_fixtures.SequenceWireTests().fixture()
        state['world']['literal'] = 'decision_json_text_state_v1:\n{"do_not_decode":true}'
        state['world']['missingness'] = [{'missing': {}}, {'present': None}, [], False, 0, '', 0.0]
        state['world']['unicode'] = '可达鸭：Pokémon 🦆\nQuoted "fact" and \\ path'
        return state, {key: offered[key] for key in ('a', 'b')}

    def assert_semantics(self, actual, expected):
        state, options = wire.expand_decision_evidence(*actual)
        self.assertEqual(state, expected[0])
        self.assertEqual(list(options), list(expected[1]))
        for key, value in expected[1].items():
            try:
                original = json.loads(value)
            except (TypeError, ValueError):
                self.assertEqual(options[key], value)
            else:
                self.assertEqual(json.loads(options[key]), original)

    def test_roundtrip_complete_state_unicode_missingness_order_and_literal_prefix(self):
        encode, prefix, guidance = self.codec()
        state, options = self.fixture()
        original = deepcopy((state, options))
        text, offered = encode(state, options)
        self.assertIsInstance(text, str)
        self.assertTrue(text.startswith(prefix))
        self.assertEqual(json.loads(text[len(prefix):]), state)
        self.assertIn('可达鸭', text)
        self.assertIs(offered, options)
        self.assert_semantics((text, offered), (state, options))
        self.assertEqual((state, options), original)
        restored, _ = wire.expand_decision_evidence(text, offered)
        self.assertEqual(list(restored), list(state))
        self.assertEqual(list(restored['world']), list(state['world']))

    def test_criteria_are_not_reencoded_even_null_or_scalar_json_or_plain_text(self):
        encode, _, _ = self.codec()
        state, _ = self.fixture()
        options = {'a': '{ "payload": [0, null, false] }', 'plain': 'Literal criterion',
            'null': None, 'scalar': '"quoted JSON string"', 'zero': '0'}
        text, offered = encode(state, options)
        self.assertIs(offered, options)
        self.assertEqual(list(offered), list(options))
        self.assert_semantics((text, offered), (state, options))

    def test_text_profitability_guard_includes_prefix_and_guidance_not_a_token_limit(self):
        encode, prefix, guidance = self.codec()
        state, options = self.fixture()
        text, offered = encode(state, options)
        self.assertLess(len(text.encode()) + len(guidance.encode()),
            len(json.dumps(state, indent=2, ensure_ascii=False).encode()))
        self.assertEqual(text, prefix + json.dumps(state, separators=(',', ':'), ensure_ascii=False))
        for small in ({'x': 1}, {'world': {'single_fact': 'x' * 12000}}):
            unchanged, same = encode(small, options)
            self.assertIs(unchanged, small)
            self.assertIs(same, options)

    def test_preexisting_reference_and_record_protocols_expand_after_text_root(self):
        encode, _, _ = self.codec()
        state, options = self.fixture()
        for kwargs in ({}, {'min_chars': 160}, {'min_chars': 160, 'alias_fields': False},
            {'min_chars': 160, 'alias_fields': False, 'sequence_tables': True}):
            packed, offered = compact_mapped_decision_wire(state, options, **kwargs)
            text, same = encode(packed, offered)
            self.assertIsInstance(text, str)
            self.assertIs(same, offered)
            self.assert_semantics((text, same), (state, options))

    def test_nonstructured_root_is_not_wrapped_and_ordinary_text_stays_literal(self):
        encode, _, _ = self.codec()
        options = {'a': 'Literal'}
        for ordinary in ('Ordinary state text', ' {"already":"literal JSON text"} ', ['ordinary']):
            unchanged, same = encode(ordinary, options)
            self.assertIs(unchanged, ordinary)
            self.assertIs(same, options)
        self.assertEqual(wire.expand_decision_evidence('Ordinary state text', options),
            ('Ordinary state text', options))

    def test_malformed_reserved_version_invalid_json_or_nonobject_root_fails_closed(self):
        _, prefix, _ = self.codec()
        for text in ('decision_json_text_state_v2:\n{}', prefix + 'not JSON',
            prefix + '{} trailing', prefix + '[]', prefix + 'null', prefix + '"literal"'):
            with self.subTest(text=text), self.assertRaises(ValueError):
                wire.expand_decision_evidence(text, {'a': 'Literal'})

    def test_duplicate_keys_and_nonfinite_json_constants_cannot_silently_change_facts(self):
        _, prefix, _ = self.codec()
        for payload in ('{"x":1,"x":2}', '{"world":{"hp":0,"hp":100}}',
            '{"unknown":NaN}', '{"unknown":Infinity}', '{"unknown":-Infinity}'):
            with self.subTest(payload=payload), self.assertRaises(ValueError):
                wire.expand_decision_evidence(prefix + payload, {})

    def test_non_json_source_types_fail_closed_instead_of_coercing_keys_or_bytes(self):
        encode, _, _ = self.codec()
        for unsafe in ({1: 'integer key'}, {'nested': {False: 0}},
            {'not_finite': float('nan')}, {'bytes': b'not JSON'}):
            with self.subTest(unsafe=unsafe), self.assertRaises(ValueError):
                encode(unsafe, {'a': 'Literal'})

    def native_rule_state(self):
        state, options = self.fixture()
        query = {'Call': {'callee': 'hasMoney', 'args': [{'NumberLit': 500.0}]}}
        rule = Rule('617072a37ab0794d', 'SafariZoneGate',
            'SafariZoneGate:talkSafariZoneWorker1', ['npc:0'], [(query, True)], ['YES'],
            ('transport', ('SafariZoneCenter', 14, 25), True),
            [('flag', 'EVENT_IN_SAFARI_ZONE', True), ('flag', 'EVENT_SAFARI_GAME_OVER', False)])
        agent = SimpleNamespace(index=SimpleNamespace(rules=[rule]), observed_barrier_maps=set())
        state['script_resource_guard_reference'] = AutonomousStoryAgent.script_resource_guard_reference(
            agent, {'money': 171, 'badges': 6, 'flags': {}})
        return state, options

    def test_production_rule_reference_uses_existing_json_tuple_array_semantics(self):
        encode, prefix, _ = self.codec()
        state, options = self.native_rule_state()
        original = deepcopy(state)
        effect = state['script_resource_guard_reference']['resource_guarded_transports'][0]['script']['produces']
        self.assertIsInstance(effect, tuple)
        self.assertIsInstance(effect[1], tuple)
        text, offered = encode(state, options)
        self.assertIsInstance(text, str)
        expected = json.loads(json.dumps(state, allow_nan=False))
        self.assertEqual(json.loads(text[len(prefix):]), expected)
        self.assert_semantics((text, offered), (expected, options))
        self.assertEqual(state, original)
        self.assertIsInstance(effect, tuple)
        self.assertIs(offered, options)

    def test_nested_tuple_library_fields_order_missingness_and_literals_survive(self):
        encode, _, _ = self.codec()
        state, options = self.fixture()
        state['shared_strategy_evidence'] = {'e0': {
            'position_and_guard': ('SafariZoneCenter', 14, 25, None, False),
            'nested': ({'missing': {}, 'present': None}, ['decision_json_text_state_v1:\n{}'])}}
        state['native_reference'] = {'shared_strategy_evidence_ref': 'e0'}
        untouched = deepcopy(state)
        text, offered = encode(state, options)
        expected = wire.expand_decision_evidence(json.loads(json.dumps(state)), options)
        self.assert_semantics((text, offered), expected)
        self.assertEqual(state, untouched)

    def test_production_tuple_reference_survives_all_older_record_formats(self):
        encode, _, _ = self.codec()
        state, options = self.native_rule_state()
        expected = json.loads(json.dumps(state))
        for kwargs in ({}, {'min_chars': 160}, {'min_chars': 160, 'alias_fields': False},
            {'min_chars': 160, 'alias_fields': False, 'sequence_tables': True}):
            with self.subTest(kwargs=kwargs):
                packed, offered = compact_mapped_decision_wire(state, options, **kwargs)
                text, same = encode(packed, offered)
                self.assertIsInstance(text, str)
                self.assert_semantics((text, same), (expected, options))

    def test_real_http_client_serialization_seam_preserves_production_tuples_and_options(self):
        import io
        import urllib.error
        from openpokered.typesafe import TypeSafeClient
        self.codec()
        state, options = self.native_rule_state()
        expected = json.loads(json.dumps(state))
        agent = fixtures.DecisionFieldWireTests().agent()
        agent.layer_jev = {'strategy': True, 'action': True}
        agent.max_calls, agent.calls, agent.tokens, agent.models = 100, Counter(), Counter(), set()
        agent.check_budget, agent.client = Mock(), Mock()
        requests = []
        def opener(request, *, timeout):
            payload = json.loads(request.data)
            offered = payload['questions']['strategy']['criteria']
            self.assertEqual(list(offered), ['a', 'b', 'none'])
            self.assert_semantics((payload['state'], {key: offered[key] for key in ('a', 'b')}),
                (expected, options))
            requests.append(payload)
            if not isinstance(payload['state'], str):
                raise urllib.error.HTTPError(request.full_url, 400, 'context limit', {},
                    io.BytesIO(b'{"error_type":"max_tokens_exceeded"}'))
            return io.BytesIO(json.dumps({'model': 'offline-test-jev', 'answers': {'strategy': {
                'type': 'choice', 'choice': 'b', 'confidence': .8,
                'probabilities': {'a': .1, 'b': .8, 'none': .1}}},
                'usage': {'input_tokens': 200, 'output_tokens': 20}}).encode())
        agent.model_client = TypeSafeClient(api_key='offline-no-network', provider='openrouter',
            model='jev-1.13.0', base_url='https://offline.invalid', opener=opener, max_retries=0)
        untouched = deepcopy(state)
        self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), 'b')
        self.assertIsInstance(requests[-1]['state'], str)
        self.assertEqual(agent.calls['strategy'], len(requests))
        self.assertEqual(agent.tokens['strategy'], 200)
        self.assertEqual(state, untouched)

    def test_tuple_hidden_malformed_reference_still_fails_complete_decoder_validation(self):
        encode, _, _ = self.codec()
        state, options = self.fixture()
        state['native_effect'] = ('transport', {'$e': 'missing'})
        with self.assertRaisesRegex(ValueError, 'Missing or cyclic shared evidence reference'):
            encode(state, options)

    def test_invalid_text_alternative_records_reason_without_a_request_or_state_change(self):
        self.codec()
        helper, agent = fixtures.DecisionFieldWireTests(), fixtures.DecisionFieldWireTests().agent()
        state, options = self.fixture()
        original = deepcopy((state, options))
        with patch.object(DualStoryAgent, 'choose', side_effect=helper.overflow()), \
            patch('openpokered.autonomous_story.compact_decision_json_text_state',
                side_effect=ValueError('Non-JSON value in JSON-text state: bytes')):
            with self.assertRaises(StoryStopped):
                agent.choose_bounded_strategy(state, options, 'Pick')
        records = [call.kwargs for call in agent.record.call_args_list
            if call.args[0] == 'strategy_wire_encoding_ineligible']
        self.assertEqual(len(records), 1)
        self.assertEqual(records[0]['encoding'], 'compact_json_text_state')
        self.assertIn('bytes', records[0]['reason'])
        self.assertEqual(records[0]['candidate_ids'], list(options))
        self.assertFalse(records[0]['request_attempted'])
        self.assertEqual((state, options), original)

    def test_unprofitable_text_alternative_records_heuristic_reason_without_retry(self):
        self.codec()
        helper, agent = fixtures.DecisionFieldWireTests(), fixtures.DecisionFieldWireTests().agent()
        state, options = self.fixture()
        with patch.object(DualStoryAgent, 'choose', side_effect=helper.overflow()), \
            patch('openpokered.autonomous_story.compact_decision_json_text_state',
                side_effect=lambda state, candidates: (state, candidates)):
            with self.assertRaises(StoryStopped):
                agent.choose_bounded_strategy(state, options, 'Pick')
        records = [call.kwargs for call in agent.record.call_args_list
            if call.args[0] == 'strategy_wire_encoding_ineligible']
        self.assertEqual(len(records), 1)
        self.assertEqual(records[0]['reason'], 'compact_text_plus_guidance_not_smaller_than_possible_pretty_json')
        self.assertFalse(records[0]['request_attempted'])
        self.assertFalse(records[0]['byte_reference_is_token_limit'])

    def test_cyclic_native_container_fails_closed_without_mutating_source(self):
        encode, _, _ = self.codec()
        state = {'native': []}
        state['native'].append(state)
        with self.assertRaisesRegex(ValueError, 'Circular JSON-text state'):
            encode(state, {'a': 'Literal'})
        self.assertIs(state['native'][0], state)

    def test_malformed_existing_protocol_cannot_be_bypassed_by_text_serialization(self):
        encode, _, _ = self.codec()
        state, options = self.fixture()
        state['world']['ordinary'] = {'$e': 'Literal non-protocol evidence'}
        with self.assertRaises(ValueError):
            encode(state, options)
        state['world'].pop('ordinary')
        options['bad'] = '{"context":{"$e":"missing"}}'
        with self.assertRaises(ValueError):
            encode(state, options)

    def test_new_text_format_is_lazy_on_normal_success_and_noncontext_errors(self):
        encode, _, _ = self.codec()
        helper = fixtures.DecisionFieldWireTests()
        state, options = self.fixture()
        for failure in (None, 'HTTP 401 unauthorized', 'HTTP 402 payment_required', 'HTTP 429 rate_limit'):
            agent = helper.agent()
            with patch('openpokered.autonomous_story.compact_decision_json_text_state', wraps=encode) as codec, \
                patch.object(DualStoryAgent, 'choose', return_value='a',
                    side_effect=helper.overflow(message=failure) if failure else None) as calls:
                if failure:
                    with self.assertRaises(StoryStopped):
                        agent.choose_bounded_strategy(state, options, 'Pick')
                else:
                    self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), 'a')
                calls.assert_called_once()
                codec.assert_not_called()
            self.assertFalse(hasattr(agent, '_json_text_state_scopes'))

    def test_one_and_two_option_leaf_can_use_text_without_partition_or_chosen_override(self):
        _, prefix, guidance = self.codec()
        helper = fixtures.DecisionFieldWireTests()
        state, all_options = self.fixture()
        for layer in ('strategy', 'action'):
            for count in (1, 2):
                for abstain in (False, True):
                    agent = helper.agent()
                    options = dict(list(all_options.items())[:count])
                    def choose(actual_layer, actual, offered, instruction, *, allow_abstain):
                        self.assertEqual(actual_layer, layer)
                        self.assertEqual(allow_abstain, abstain)
                        self.assert_semantics((actual, offered), (state, options))
                        if not isinstance(actual, str):
                            raise helper.overflow(layer)
                        self.assertTrue(actual.startswith(prefix))
                        self.assertEqual(instruction.count(guidance), 1)
                        return list(options)[-1]
                    with patch.object(DualStoryAgent, 'choose', side_effect=choose) as calls:
                        selected = agent.choose_bounded_choice(layer, state, options, 'Pick', allow_abstain=abstain)
                        self.assertEqual(selected, list(options)[-1])
                        first = calls.call_count
                        self.assertGreater(first, 1)
                        self.assertEqual(agent.choose_bounded_choice(layer, state, options, 'Pick',
                            allow_abstain=abstain), selected)
                        self.assertEqual(calls.call_count, first + 1)
                    self.assertEqual(len(agent._json_text_state_scopes), 1)
                    self.assertFalse(any(call.args[0] == layer + '_partition' for call in agent.record.call_args_list))

    def test_runtime_learning_is_endpoint_model_provider_path_and_layer_scoped(self):
        self.codec()
        helper, agent = fixtures.DecisionFieldWireTests(), fixtures.DecisionFieldWireTests().agent()
        state, options = self.fixture()
        def choose(layer, actual, offered, instruction, *, allow_abstain):
            self.assert_semantics((actual, offered), (state, options))
            if not isinstance(actual, str):
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
        self.assertEqual(len(agent._json_text_state_scopes), 6)

    def test_exhausted_text_leaf_fails_closed_with_every_option_and_distinct_requests(self):
        self.codec()
        helper, agent = fixtures.DecisionFieldWireTests(), fixtures.DecisionFieldWireTests().agent()
        state, options = self.fixture()
        requests = []
        def choose(layer, actual, offered, instruction, *, allow_abstain):
            self.assert_semantics((actual, offered), (state, options))
            requests.append(json.dumps([actual, offered]))
            raise helper.overflow(layer)
        with patch.object(DualStoryAgent, 'choose', side_effect=choose):
            with self.assertRaises(StoryStopped):
                agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertIsInstance(json.loads(requests[-1])[0], str)
        self.assertEqual(len(requests), len(set(requests)))
        self.assertFalse(any(call.args[0] == 'strategy_partition' for call in agent.record.call_args_list))

    def test_ineligible_or_invalid_text_format_never_repeats_the_structured_request(self):
        self.codec()
        helper = fixtures.DecisionFieldWireTests()
        state, options = self.fixture()
        for invalid in (False, True):
            agent = helper.agent()
            with patch('openpokered.autonomous_story.compact_decision_json_text_state',
                side_effect=ValueError('invalid root') if invalid else lambda state, options: (state, options)), \
                patch.object(DualStoryAgent, 'choose', side_effect=helper.overflow()) as calls:
                with self.assertRaises(StoryStopped):
                    agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertTrue(all(isinstance(call.args[1], dict) for call in calls.call_args_list))
            self.assertFalse(hasattr(agent, '_json_text_state_scopes'))

    def test_actual_client_seam_keeps_abstention_distribution_and_full_request(self):
        self.codec()
        helper, agent = fixtures.DecisionFieldWireTests(), fixtures.DecisionFieldWireTests().agent()
        state, options = self.fixture()
        agent.layer_jev = {'strategy': True, 'action': True}
        agent.max_calls, agent.calls, agent.tokens, agent.models = 100, Counter(), Counter(), set()
        agent.check_budget, agent.client = Mock(), Mock()
        seen = []
        def transport(actual, questions, *, model):
            question = questions['strategy']
            self.assertEqual(list(question.criteria), ['a', 'b', 'none'])
            self.assertEqual(question.criteria['none'], 'None of these candidates can advance the current goal.')
            self.assert_semantics((actual, {key: question.criteria[key] for key in ('a', 'b')}), (state, options))
            seen.append(actual)
            if not isinstance(actual, str):
                raise TypeSafeError('max_tokens_exceeded')
            return SystemOneResult('offline-test-jev', {'strategy': ChoiceAnswer('b', .8,
                {'a': .1, 'b': .8, 'none': .1})}, 200, 20)
        with patch.object(agent.model_client, 'system_one', side_effect=transport):
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), 'b')
        self.assertIsInstance(seen[-1], str)
        self.assertEqual(agent.calls['strategy'], len(seen))
        self.assertEqual(agent.tokens['strategy'], 200)

    def test_dashboard_projection_keeps_identical_display_facts_and_original_request_hash(self):
        encode, _, _ = self.codec()
        state, options = self.fixture()
        state['dex_progress'] = {'owned': 75, 'validated_owned': 75}
        packed, offered = compact_mapped_decision_wire(state, options,
            min_chars=160, alias_fields=False, sequence_tables=True)
        text, same = encode(packed, offered)
        before = dashboard_fixtures.decision(offered, packed)
        after = dashboard_fixtures.decision(same, text)
        untouched = deepcopy((before, after))
        old_descriptions, new_descriptions = {}, {}
        old = dashboard_fixtures.dashboard.export_decision(before, old_descriptions)
        new = dashboard_fixtures.dashboard.export_decision(after, new_descriptions)
        self.assertEqual({key: value for key, value in old.items() if key != 'request_sha256'},
            {key: value for key, value in new.items() if key != 'request_sha256'})
        self.assertEqual(old_descriptions, new_descriptions)
        expected_hash = hashlib.sha256(json.dumps({'state': text, 'question': after['question']},
            ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
        self.assertEqual(new['request_sha256'], expected_hash)
        self.assertNotEqual(new['request_sha256'], old['request_sha256'])
        self.assertEqual((before, after), untouched)

    def test_dashboard_malformed_reserved_root_fails_before_exporting_any_description(self):
        _, prefix, _ = self.codec()
        event = dashboard_fixtures.decision({'a': 'Literal'}, prefix + '{} trailing')
        descriptions, original = {}, deepcopy(event)
        with self.assertRaises(ValueError):
            dashboard_fixtures.dashboard.export_decision(event, descriptions)
        self.assertEqual(descriptions, {})
        self.assertEqual(event, original)

    def test_legacy_checkpoint_reader_restores_actual_obstacles_across_structured_and_text_wires(self):
        encode, _, _ = self.codec()
        state, options = self.fixture()
        blockage = {'destination': 'PowerPlant', 'map': 'Route10', 'x': 8, 'y': 5,
            'reason': 'observed obstacle', 'unknown': None}
        state['known_navigation_failures'] = {'observed': blockage}
        packed, offered = compact_mapped_decision_wire(state, options,
            min_chars=160, alias_fields=False, sequence_tables=True)
        text, _ = encode(packed, offered)
        with tempfile.TemporaryDirectory() as directory:
            run = Path(directory)
            trace = run / 'trace.jsonl'
            for representation in (state, packed, text):
                original = json.dumps({'kind': 'judgment', 'state': representation}) + '\n'
                trace.write_text(original)
                self.assertEqual(checkpoint_legacy_navigation_history(run),
                    {json.dumps(['PowerPlant', 'Route10']): blockage})
                self.assertEqual(trace.read_text(), original)

    def test_legacy_checkpoint_reader_does_not_invent_history_from_unknown_or_missing_state(self):
        self.codec()
        with tempfile.TemporaryDirectory() as directory:
            run = Path(directory)
            self.assertEqual(checkpoint_legacy_navigation_history(run), {})
            (run / 'trace.jsonl').write_text('not JSON\n' + '\n'.join(json.dumps(event) for event in (
                {'kind': 'operation'}, {'state': None}, {'state': []}, {'state': 'Ordinary text'},
                {'state': {'world': {'route_offer': 'not an observed obstacle'}}})))
            self.assertEqual(checkpoint_legacy_navigation_history(run), {})

    def test_legacy_checkpoint_reader_fails_closed_on_malformed_reserved_root(self):
        _, prefix, _ = self.codec()
        with tempfile.TemporaryDirectory() as directory:
            run = Path(directory)
            (run / 'trace.jsonl').write_text(json.dumps({'state': prefix + '{"hp":1,"hp":2}'}))
            with self.assertRaises(ValueError):
                checkpoint_legacy_navigation_history(run)


if __name__ == '__main__':
    unittest.main()

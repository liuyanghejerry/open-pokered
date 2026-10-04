"""Reversible evidence keys rescue binary overflow without pruning facts/options."""
from collections import Counter
from copy import deepcopy
import json
import unittest
from unittest.mock import Mock, patch

from openpokered.autonomous_story import (AutonomousStoryAgent,
    SHORT_EVIDENCE_REFERENCE_INSTRUCTION, compact_evidence_reference_wire,
    restore_evidence_reference_wire)
from openpokered.story_agent import DualStoryAgent, StoryStopped
from openpokered.decision_wire import (restore_decision_json_text_state,
    expand_decision_evidence, JSON_TEXT_STATE_INSTRUCTION)
from openpokered.typesafe import ChoiceAnswer, SystemOneResult, TypeSafeClient, TypeSafeError


class ShortEvidenceWireTests(unittest.TestCase):
    def agent(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.record = Mock()
        agent.model = 'jev-1.13.0'
        agent.model_client = TypeSafeClient('https://offline.invalid', 'offline-test',
                                          agent.model, provider='openrouter')
        return agent

    def fixture(self):
        ref = lambda key: {'shared_strategy_evidence_ref': key}
        state = {'world': ref('world'), 'other': [False, 0, None, True],
            'shared_strategy_evidence': {
                'world': {'party': ref('party'), 'money': 93, 'flags': {'bicycle': True}},
                'party': {'strategy_table': {'columns': ['species', 'hp', 'moves'],
                    'rows': [['Paras', 47, ref('moves')], ['Hypno', 129, ref('moves')]]}},
                'moves': ['Scratch', 'StunSpore', 'LeechLife', 'None']}}
        candidates = {'a': json.dumps({'context': ref('world'), 'goal': 'Parasect'}),
                      'b': json.dumps({'context': ref('party'), 'goal': 'Seadra'})}
        return state, candidates

    def test_roundtrip_retains_all_nested_library_tables_types_and_options(self):
        state, options = self.fixture()
        options.update({'plain': 'No matching action', 'literal': '"A JSON string"',
                        'number': '3', 'mixed': json.dumps({
                            'shared_strategy_evidence_ref': 'ordinary', 'other': 2}),
                        'text': json.dumps({'text': '{"$e":"literal text"}'})})
        original = deepcopy((state, options))
        wire, candidates = compact_evidence_reference_wire(state, options)
        self.assertEqual(restore_evidence_reference_wire(wire), state)
        self.assertEqual(list(candidates), list(options))
        self.assertEqual(set(wire['shared_strategy_evidence']), set(state['shared_strategy_evidence']))
        for key, value in options.items():
            if key in ('a', 'b'):
                self.assertEqual(restore_evidence_reference_wire(json.loads(candidates[key])),
                                 json.loads(value))
            else:
                self.assertEqual(candidates[key], value)
        self.assertLess(len(json.dumps(wire)), len(json.dumps(state)))
        self.assertEqual((state, options), original)

    def test_no_library_or_reference_preserves_identity_and_candidate_bytes(self):
        for state in ({'money': 93}, {'world': {}, 'shared_strategy_evidence': {}}):
            options = {'a': '{ "cost": 3 }', 'b': 'Continue'}
            wire, candidates = compact_evidence_reference_wire(state, options)
            self.assertIs(wire, state)
            self.assertIs(candidates, options)

    def test_reserved_alias_collision_in_state_library_or_candidate_refuses_encoding(self):
        for placement in ('state', 'library', 'candidate'):
            state, options = self.fixture()
            if placement == 'state':
                state['ordinary'] = {'$e': 'Unrelated value', 'cost': 5}
            elif placement == 'library':
                state['shared_strategy_evidence']['moves'] = {'$e': 'Unrelated value'}
            else:
                options['b'] = json.dumps({'$e': 'Unrelated value'})
            original = deepcopy((state, options))
            with self.subTest(placement=placement), self.assertRaisesRegex(ValueError, 'collision'):
                compact_evidence_reference_wire(state, options)
            self.assertEqual((state, options), original)

    def test_missing_reference_is_not_transmitted_as_an_alias(self):
        state, options = self.fixture()
        options['b'] = json.dumps({'shared_strategy_evidence_ref': 'missing'})
        with self.assertRaisesRegex(ValueError, 'Missing shared evidence'):
            compact_evidence_reference_wire(state, options)

    def test_successful_original_request_does_not_change_wire_or_enable_alias(self):
        agent = self.agent()
        state, options = self.fixture()
        with patch.object(DualStoryAgent, 'choose', return_value='b') as decide:
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), 'b')
        decide.assert_called_once_with('strategy', state, options, 'Pick', allow_abstain=True)
        agent.record.assert_not_called()
        self.assertFalse(hasattr(agent, '_short_evidence_reference_scopes'))

    def test_one_and_two_option_overflow_try_once_with_same_facts_and_abstention(self):
        for layer in ('strategy', 'action'):
            for count in (1, 2):
                for abstain in (False, True):
                    agent = self.agent()
                    state, options = self.fixture()
                    options = dict(list(options.items())[:count])
                    original = deepcopy((state, options))
                    def decide(actual_layer, actual, offered, instruction, *, allow_abstain):
                        self.assertEqual(actual_layer, layer)
                        self.assertEqual(allow_abstain, abstain)
                        self.assertEqual(restore_evidence_reference_wire(actual), state)
                        self.assertEqual(list(offered), list(options))
                        for key in offered:
                            self.assertEqual(restore_evidence_reference_wire(json.loads(offered[key])),
                                             json.loads(options[key]))
                        if actual['world'] == state['world']:
                            raise StoryStopped(f'{layer}:service_unavailable') from TypeSafeError('max_tokens_exceeded')
                        self.assertEqual(instruction.count(SHORT_EVIDENCE_REFERENCE_INSTRUCTION), 1)
                        return list(offered)[-1]
                    with self.subTest(layer=layer, count=count, abstain=abstain), \
                            patch.object(DualStoryAgent, 'choose', side_effect=decide) as calls:
                        self.assertEqual(agent.choose_bounded_choice(layer, state, options,
                            'Pick', allow_abstain=abstain), list(options)[-1])
                    self.assertEqual(calls.call_count, 2)
                    self.assertEqual((state, options), original)
                    self.assertEqual(len(agent._choice_context_sizes), 2)
                    self.assertEqual({key[-1] for key in agent._choice_context_sizes
                        if len(key) == 6}, {'short_evidence_reference'})

    def test_leaf_overflow_after_all_formats_is_terminal_not_recursive(self):
        agent = self.agent()
        state, options = self.fixture()
        failure = StoryStopped('strategy:service_unavailable')
        failure.__cause__ = TypeSafeError('max_tokens_exceeded')
        with patch.object(DualStoryAgent, 'choose', side_effect=failure) as calls:
            with self.assertRaises(StoryStopped) as observed:
                agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertIs(observed.exception, failure)
        self.assertEqual(calls.call_count, 3)
        self.assertIsInstance(calls.call_args.args[1], str)
        for call in calls.call_args_list:
            self.assertEqual(expand_decision_evidence(call.args[1], call.args[2]),
                expand_decision_evidence(state, options))
        self.assertFalse(any(call.args[0] == 'strategy_partition' for call in agent.record.call_args_list))

    def test_actual_transport_keeps_none_and_conditional_probability_semantics(self):
        for probabilities, selected in (({'a': 0.35, 'b': 0.25, 'none': 0.4}, 'a'),
                                        ({'a': 0.2, 'b': 0.1, 'none': 0.7}, None)):
            agent = self.agent()
            agent.layer_jev = {'strategy': True, 'action': True}
            agent.calls, agent.tokens, agent.models = Counter(), Counter(), set()
            agent.max_calls = 4
            agent.check_budget = Mock()
            agent.client = Mock()
            agent.client.state.return_value = {'frame_count': 0}
            state, options = self.fixture()
            response = SystemOneResult('offline', {'strategy': ChoiceAnswer('none', probabilities, 0.3)}, 1, 1)
            with self.subTest(probabilities=probabilities), patch.object(agent.model_client, 'system_one',
                    side_effect=[TypeSafeError('max_tokens_exceeded'), response]) as transport:
                if selected is None:
                    with self.assertRaisesRegex(StoryStopped, 'strategy:no_selection'):
                        agent.choose_bounded_strategy(state, options, 'Pick')
                else:
                    self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), selected)
            self.assertEqual(transport.call_count, 2)
            for call in transport.call_args_list:
                question = call.args[1]['strategy']
                self.assertEqual(list(question.criteria), ['a', 'b', 'none'])
                self.assertEqual(question.criteria['none'], 'None of these candidates can advance the current goal.')
            self.assertEqual(agent.calls['strategy'], 2)

    def test_non_context_errors_do_not_enable_encoding_or_retry(self):
        for detail in ('HTTP 401 unauthorized', 'HTTP 402 payment_required', 'HTTP 429 rate_limit'):
            agent = self.agent()
            state, options = self.fixture()
            failure = StoryStopped('strategy:service_unavailable')
            failure.__cause__ = TypeSafeError(detail)
            with self.subTest(detail=detail), patch.object(DualStoryAgent, 'choose', side_effect=failure) as calls:
                with self.assertRaises(StoryStopped) as observed:
                    agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertIs(observed.exception, failure)
            calls.assert_called_once()
            agent.record.assert_not_called()

    def test_collision_keeps_canonical_failure_closed_and_does_not_retry(self):
        agent = self.agent()
        state, options = self.fixture()
        state['ordinary'] = {'$e': 'Literal non-protocol evidence'}
        failure = StoryStopped('strategy:service_unavailable')
        failure.__cause__ = TypeSafeError('max_tokens_exceeded')
        with patch.object(DualStoryAgent, 'choose', side_effect=failure) as calls:
            with self.assertRaises(StoryStopped):
                agent.choose_bounded_strategy(state, options, 'Pick')
        calls.assert_called_once_with('strategy', state, options, 'Pick', allow_abstain=True)
        self.assertFalse(hasattr(agent, '_short_evidence_reference_scopes'))

    def test_encoding_learning_is_separate_by_endpoint_model_provider_and_layer(self):
        agent = self.agent()
        state, options = self.fixture()
        starts = []
        def decide(layer, actual, offered, instruction, *, allow_abstain):
            starts.append((layer, actual['world']))
            if actual['world'] == state['world']:
                raise StoryStopped(f'{layer}:service_unavailable') from TypeSafeError('max_tokens_exceeded')
            return 'b'
        with patch.object(DualStoryAgent, 'choose', side_effect=decide):
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertEqual(len(starts), 3)
            agent.choose_bounded_choice('action', state, options, 'Pick')
            agent.model_client.base_url = 'https://other-offline.invalid'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model = 'different-model'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model_client.system_one_path = '/other-path'
            agent.choose_bounded_strategy(state, options, 'Pick')
            agent.model_client.provider = 'typesafe'
            agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertEqual(len(starts), 13)
        self.assertEqual(len(agent._short_evidence_reference_scopes), 6)
        self.assertEqual(len(agent._choice_context_sizes), 12)
        successor = self.agent()
        with patch.object(DualStoryAgent, 'choose', return_value='b') as calls:
            successor.choose_bounded_strategy(state, options, 'Pick')
        calls.assert_called_once_with('strategy', state, options, 'Pick', allow_abstain=True)

    def test_full_set_retries_same_format_then_partitions_every_option_without_note_growth(self):
        agent = self.agent()
        state, small = self.fixture()
        options = {str(i): small['a'] for i in range(16)}
        evaluated = set()
        def decide(layer, actual, offered, instruction, *, allow_abstain):
            self.assertEqual(restore_evidence_reference_wire(restore_decision_json_text_state(actual)), state)
            self.assertLessEqual(instruction.count(SHORT_EVIDENCE_REFERENCE_INSTRUCTION), 1)
            self.assertLessEqual(instruction.count(JSON_TEXT_STATE_INSTRUCTION), 1)
            for value in offered.values():
                self.assertEqual(restore_evidence_reference_wire(json.loads(value)), json.loads(small['a']))
            if len(offered) > 2:
                raise StoryStopped('strategy:service_unavailable') from TypeSafeError('max_tokens_exceeded')
            evaluated.update(offered)
            return max(offered, key=int)
        with patch.object(DualStoryAgent, 'choose', side_effect=decide) as calls:
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), '15')
        self.assertEqual(calls.call_args_list[0].args[2], options)
        self.assertEqual(list(calls.call_args_list[1].args[2]), list(options))
        self.assertEqual(evaluated, set(options))
        self.assertTrue(calls.call_args.kwargs['allow_abstain'])
        for call in calls.call_args_list[:-1]:
            self.assertEqual(call.kwargs['allow_abstain'], list(call.args[2]) == list(options))


if __name__ == '__main__':
    unittest.main()

"""Runtime overflow references preserve choices, evidence and failure boundaries."""
from copy import deepcopy
import json
import unittest
from unittest.mock import Mock, patch

from openpokered.autonomous_story import AutonomousStoryAgent
from openpokered.story_agent import DualStoryAgent, StoryStopped
from openpokered.typesafe import Choice, TypeSafeClient, TypeSafeError


class ContextPartitionTests(unittest.TestCase):
    def agent(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.record = Mock()
        agent.model = 'jev-1.13.0'
        agent.model_client = TypeSafeClient('https://offline.invalid', 'offline-test',
                                          agent.model, provider='openrouter')
        return agent

    def wire(self, agent, layer, state, options, instruction, allow_abstain):
        criteria = dict(options)
        if allow_abstain:
            criteria['none'] = 'None of these candidates can advance the current goal.'
        return json.dumps({'state': state, 'model': agent.model_client.request_model(agent.model),
            'questions': {layer: Choice(instruction, criteria).to_json()}}).encode()

    def bounded_service(self, agent, limit=3500):
        failures, successes = [], []

        def decide(layer, state, options, instruction, *, allow_abstain):
            body = self.wire(agent, layer, state, options, instruction, allow_abstain)
            if len(body) > limit:
                failures.append((layer, state, dict(options), instruction, allow_abstain, body))
                raise StoryStopped(f'{layer}:service_unavailable') from TypeSafeError('HTTP 400 max_tokens_exceeded')
            successes.append((layer, state, dict(options), instruction, allow_abstain, body))
            return max(options, key=int)

        return decide, failures, successes

    def test_repeated_large_round_skips_overflow_but_considers_every_candidate(self):
        agent = self.agent()
        state = {'world': {'money': 93, 'species': 'Hypno', 'level': 38}}
        options = {str(i): 'Evidence ' + 'X' * 800 for i in range(8)}
        original = deepcopy((state, options))
        decide, failures, successes = self.bounded_service(agent)
        with patch.object(DualStoryAgent, 'choose', side_effect=decide):
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), '7')
            first_errors = len(failures)
            first_successes = len(successes)
            self.assertGreater(first_errors, 0)
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), '7')
        self.assertEqual(len(failures), first_errors)
        for rounds in (successes[:first_successes], successes[first_successes:]):
            self.assertEqual({key for row in rounds for key in row[2]}, set(options))
            self.assertTrue(rounds[-1][4])  # Only final comparison may abstain.
            self.assertTrue(all(not row[4] for row in rounds[:-1]))
            self.assertTrue(all(row[1] is state for row in rounds))
        self.assertEqual((state, options), original)
        proactive = [call.kwargs for call in agent.record.call_args_list
                     if call.args[0] == 'strategy_partition'
                     and call.kwargs['reason'] == 'observed_context_size_reference']
        self.assertTrue(proactive)
        self.assertTrue(all(row['byte_reference_is_token_limit'] is False for row in proactive))
        self.assertTrue(all(row['state_preserved'] for row in proactive))

    def test_reference_is_exact_transport_size_including_unicode_none_and_model_alias(self):
        agent = self.agent()
        state = {'世界': {'money': 93, 'map': 'FuchsiaPokecenter'}}
        options = {str(i): '状态证据' * 200 for i in range(4)}
        decide, failures, _ = self.bounded_service(agent, limit=15000)
        with patch.object(DualStoryAgent, 'choose', side_effect=decide):
            agent.choose_bounded_strategy(state, options, '选择下一步')
        first = next(call.kwargs for call in agent.record.call_args_list
                     if call.args[0] == 'strategy_partition')
        self.assertEqual(first['request_bytes'], len(failures[0][-1]))
        self.assertEqual(first['reference_scope'][1], 'typesafe/jev-1.13')
        self.assertIn(b'\\u', failures[0][-1])
        self.assertIn('none', json.loads(failures[0][-1])['questions']['strategy']['criteria'])
        actual_bodies = []

        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *args):
                pass

            def read(self):
                return b'{"answers":{},"usage":{}}'

        def offline_opener(request, *, timeout):
            actual_bodies.append(request.data)
            return Response()

        agent.model_client.opener = offline_opener
        criteria = {**options, 'none': 'None of these candidates can advance the current goal.'}
        agent.model_client.system_one(state, {'strategy': Choice('选择下一步', criteria)}, model=agent.model)
        self.assertEqual(actual_bodies, [failures[0][-1]])

    def test_cached_preflight_does_not_hide_non_context_error_in_a_leaf(self):
        agent = self.agent()
        options = {str(i): 'X' * 800 for i in range(8)}
        decide, failures, _ = self.bounded_service(agent)
        with patch.object(DualStoryAgent, 'choose', side_effect=decide):
            agent.choose_bounded_strategy({}, options, 'Pick')
        self.assertTrue(failures)
        failure = StoryStopped('strategy:service_unavailable')
        failure.__cause__ = TypeSafeError('HTTP 402 payment_required')
        with patch.object(DualStoryAgent, 'choose', side_effect=failure) as choose:
            with self.assertRaises(StoryStopped) as observed:
                agent.choose_bounded_strategy({}, options, 'Pick')
        self.assertIs(observed.exception, failure)
        self.assertEqual(len(choose.call_args.args[2]), 2)
        choose.assert_called_once()

    def test_small_requests_still_use_one_unchanged_full_set_call(self):
        agent = self.agent()
        state, options = {'money': 93}, {str(i): f'Choice {i}' for i in range(8)}
        with patch.object(DualStoryAgent, 'choose', return_value='7') as choose:
            self.assertEqual(agent.choose_bounded_strategy(state, options, 'Pick'), '7')
            choose.assert_called_once_with('strategy', state, options, 'Pick', allow_abstain=True)
        agent.record.assert_not_called()

    def test_reference_is_per_endpoint_model_and_layer(self):
        agent = self.agent()
        state, options = {}, {str(i): 'X' * 800 for i in range(8)}
        decide, failures, _ = self.bounded_service(agent)
        with patch.object(DualStoryAgent, 'choose', side_effect=decide):
            agent.choose_bounded_strategy(state, options, 'Pick')
            old_count = len(failures)
            agent.choose_bounded_choice('action', state, options, 'Pick')
            self.assertGreater(len(failures), old_count)
            old_count = len(failures)
            agent.model_client.base_url = 'https://other-offline.invalid'
            agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertGreater(len(failures), old_count)
            old_count = len(failures)
            agent.model = 'different-model'
            agent.choose_bounded_strategy(state, options, 'Pick')
            self.assertGreater(len(failures), old_count)
        self.assertEqual(len(agent._choice_context_sizes), 4)

    def test_larger_observed_success_disables_inconsistent_byte_reference(self):
        agent = self.agent()
        state, options = {}, {str(i): 'X' * 800 for i in range(8)}
        decide, failures, _ = self.bounded_service(agent)
        with patch.object(DualStoryAgent, 'choose', side_effect=decide):
            agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertTrue(failures)
        # Two options always reach the service; bytes cannot certify token use.
        with patch.object(DualStoryAgent, 'choose', return_value='1') as choose:
            agent.choose_bounded_strategy({}, {'0': 'Y' * 8000, '1': 'Z' * 8000}, 'Different texture')
            agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertEqual(choose.call_count, 2)
        self.assertEqual(choose.call_args.args[2], options)
        reference = next(iter(agent._choice_context_sizes.values()))
        self.assertGreater(reference['largest_success_bytes'], reference['smallest_overflow_bytes'])

    def test_context_reference_does_not_hide_other_errors_or_binary_overflow(self):
        for detail, count in [('HTTP 401 unauthorized', 8), ('HTTP 402 payment_required', 8),
                              ('HTTP 429 rate_limit', 8), ('HTTP 400 max_tokens_exceeded', 2),
                              ('HTTP 400 max_tokens_exceeded', 1)]:
            with self.subTest(detail=detail, count=count):
                agent = self.agent()
                failure = StoryStopped('strategy:service_unavailable')
                failure.__cause__ = TypeSafeError(detail)
                with patch.object(DualStoryAgent, 'choose', side_effect=failure) as choose:
                    with self.assertRaises(StoryStopped) as observed:
                        agent.choose_bounded_strategy({}, {str(i): 'X' * 800 for i in range(count)}, 'Pick')
                self.assertIs(observed.exception, failure)
                choose.assert_called_once()
                agent.record.assert_not_called()

    def test_empty_candidates_keep_original_no_candidates_exception(self):
        agent = self.agent()
        with self.assertRaisesRegex(StoryStopped, 'strategy:no_candidates'):
            agent.choose_bounded_strategy({}, {}, 'Pick')
        self.assertFalse(hasattr(agent, '_choice_context_sizes'))

    def test_second_agent_does_not_inherit_a_runtime_calibration(self):
        agent = self.agent()
        options = {str(i): 'X' * 800 for i in range(8)}
        decide, failures, _ = self.bounded_service(agent)
        with patch.object(DualStoryAgent, 'choose', side_effect=decide):
            agent.choose_bounded_strategy({}, options, 'Pick')
        self.assertTrue(failures)
        successor = self.agent()
        with patch.object(DualStoryAgent, 'choose', return_value='7') as choose:
            successor.choose_bounded_strategy({}, options, 'Pick')
        choose.assert_called_once()
        self.assertIsNot(successor._choice_context_sizes, agent._choice_context_sizes)

    def test_proactive_partitions_keep_transitive_world_and_option_evidence(self):
        agent = self.agent()
        ref = lambda key: {'shared_strategy_evidence_ref': key}
        library = {'world': {'money': 93, 'registered_species': ['Tentacool']},
                   'nested': {'status': 'Sleep', 'known_not_automatic_registration': True},
                   'unused': 'Not evidence for any offered option'}
        library.update({f'e{i}': {'detail': 'X' * 800, 'nested': ref('nested')} for i in range(8)})
        state = {'world': ref('world'), 'shared_strategy_evidence': library}
        options = {str(i): json.dumps({'evidence': ref(f'e{i}')}) for i in range(8)}
        original = deepcopy((state, options))
        decide, failures, successes = self.bounded_service(agent, limit=4200)
        with patch.object(DualStoryAgent, 'choose', side_effect=decide):
            agent.choose_bounded_strategy(state, options, 'Pick')
            first_errors, first_successes = len(failures), len(successes)
            agent.choose_bounded_strategy(state, options, 'Pick')
        self.assertEqual(len(failures), first_errors)
        for _, actual, selected, _, _, _ in successes[first_successes:]:
            self.assertEqual(actual['world'], state['world'])
            self.assertEqual(actual['shared_strategy_evidence']['world'], library['world'])
            self.assertEqual(actual['shared_strategy_evidence']['nested'], library['nested'])
            self.assertNotIn('unused', actual['shared_strategy_evidence'])
            for key in selected:
                self.assertEqual(selected[key], options[key])
                self.assertEqual(actual['shared_strategy_evidence'][f'e{key}'], library[f'e{key}'])
        self.assertEqual({key for row in successes[first_successes:] for key in row[2]}, set(options))
        self.assertEqual((state, options), original)


if __name__ == '__main__':
    unittest.main()

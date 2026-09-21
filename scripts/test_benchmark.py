"""Benchmark fairness, provider boundaries, accounting and isolation contracts."""
import copy
import io
import json
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from openpokered.benchmark import (ROOT, case_digest, decision_trials, digest, load_cases, load_manifest,
                                 plan, score_answer)
from openpokered.benchmark_models import (CommandAdapter, TypeSafeAdapter, validate_answers,
                                        validate_model, usage_from_json)
from openpokered.benchmark_reports import decision_metrics, flatten, report, rows
from openpokered.evaluation import EvaluationBudget
from openpokered.evaluation_models import MeasuredModel
from openpokered.typesafe import Choice, ChoiceAnswer, TypeSafeError

EXAMPLE = ROOT/'benchmarks/autonomy/manifest.example.json'
ADAPTER = ROOT/'benchmarks/autonomy/adapters/example.py'


class ConfigTests(unittest.TestCase):
    def test_example_defines_multiple_models_and_independent_cases(self):
        config = load_manifest(EXAMPLE)
        suite = load_cases(config['decisions']['cases'])
        self.assertEqual(len(config['models']), 2)
        self.assertEqual(len(suite['cases']), 16)
        self.assertEqual({c['split'] for c in suite['cases']}, {'development'})
        self.assertEqual({c['layer'] for c in suite['cases']}, {'strategy', 'action'})

    def test_rejects_embedded_credentials_and_unknown_provider(self):
        for config in ({'id': 'bad', 'provider': 'unknown', 'model': 'x'},
                       {'id': 'bad', 'provider': 'typesafe', 'model': 'x', 'api_key': 'must-not-be-stored'}):
            with self.assertRaises(ValueError):
                validate_model(config)

    def test_laya_requires_pinned_revision(self):
        with self.assertRaisesRegex(ValueError, 'revision'):
            validate_model({'id': 'local', 'provider': 'laya', 'model': 'my/model'})

    def test_shell_command_strings_are_not_accepted(self):
        with self.assertRaisesRegex(ValueError, 'argv'):
            validate_model({'id': 'x', 'provider': 'command', 'model': 'x', 'command': 'echo unsafe'})

    def test_nonfinite_timeout_is_not_accepted(self):
        for value in (float('inf'), float('nan'), -1, True):
            with self.assertRaises(ValueError):
                validate_model({'id': 'x', 'provider': 'typesafe', 'model': 'x', 'timeout_s': value})

    @patch('openpokered.benchmark.environment', return_value={'source_sha256': 'code', 'data_sha256': 'data', 'host': {}, 'binary_sha256': None})
    def test_new_models_share_conditions_but_smoke_and_new_budgets_do_not(self, _):
        config = load_manifest(EXAMPLE)
        config['tracks'] = ['decisions']
        a = plan(config)
        config['models'] = [{'id': 'third', 'provider': 'typesafe', 'model': 'third-v1'}]
        b = plan(config)
        self.assertEqual(a['comparison_key'], b['comparison_key'])
        self.assertNotEqual(a['comparison_key'], plan(config, smoke_seconds=5)['comparison_key'])
        config['story']['seconds'] = 900
        self.assertNotEqual(b['comparison_key'], plan(config)['comparison_key'])

    @patch('openpokered.benchmark.environment', return_value={'source_sha256': 'code', 'data_sha256': 'data', 'host': {}, 'binary_sha256': None})
    def test_schedule_is_paired_and_rotates_order(self, _):
        config = load_manifest(EXAMPLE)
        config['tracks'] = ['decisions']
        jobs = plan(config)['jobs']
        self.assertEqual([j['model_id'] for j in jobs[:2]], list(reversed([j['model_id'] for j in jobs[2:4]])))
        self.assertEqual(jobs[0]['seed'], jobs[1]['seed'])

    def test_option_reversal_does_not_mutate_suite_or_leak_labels_into_question(self):
        suite = load_cases(ROOT/'benchmarks/autonomy/cases-v1.json')
        before = copy.deepcopy(suite)
        trials = list(decision_trials(suite, 42, ['original', 'reversed']))
        self.assertEqual(len(trials), 32)
        self.assertEqual(suite, before)
        self.assertEqual(list(trials[0][2]['criteria']), list(reversed(trials[1][2]['criteria'])))
        self.assertNotIn('acceptable', trials[0][2])
        self.assertNotIn('rationale', trials[0][2])

    def test_candidate_order_is_part_of_the_dataset_fingerprint(self):
        suite = load_cases(ROOT/'benchmarks/autonomy/cases-v1.json')
        changed = copy.deepcopy(suite)
        q = changed['cases'][0]['question']
        q['criteria'] = dict(reversed(list(q['criteria'].items())))
        self.assertNotEqual(case_digest(suite), case_digest(changed))


class ProtocolTests(unittest.TestCase):
    def config(self, code=None, timeout=2):
        return {'id': 'stub', 'provider': 'command', 'model': 'test', 'timeout_s': timeout,
                'command': [sys.executable, '-c', code] if code else [sys.executable, str(ADAPTER)]}

    def test_real_persistent_command_adapter_preserves_missing_usage(self):
        client = CommandAdapter(self.config())
        try:
            for expected in ('a', 'b'):
                result = client.system_one({'goal': 'test'}, {'action': Choice('Choose', {expected: 'first', 'none': 'none'})})
                self.assertEqual(result.answers['action'].choice, expected)
                self.assertEqual(result.answers['action'].probabilities, {})
                self.assertEqual(client.last_usage, {'input_tokens': None, 'output_tokens': None})
            self.assertIsNone(client.proc.poll())
        finally:
            client.close()
        self.assertIsNotNone(client.proc.poll())

    def test_ready_handshake_timeout_is_bounded(self):
        started = time.monotonic()
        with self.assertRaisesRegex(TypeSafeError, 'timeout'):
            CommandAdapter(self.config('import time; time.sleep(10)', timeout=.1))
        self.assertLess(time.monotonic()-started, 3)

    def test_partial_line_cannot_bypass_response_timeout(self):
        code = '''import json,sys,time
print(json.dumps({'protocol':'open-pokered-judge-v1','ready':True}),flush=True)
sys.stdin.readline()
sys.stdout.write('{');sys.stdout.flush();time.sleep(10)
'''
        client = CommandAdapter(self.config(code, timeout=.3))
        try:
            with self.assertRaisesRegex(TypeSafeError, 'timeout'):
                client.system_one({}, {'x': Choice('Pick', {'a': 'A', 'b': 'B'})})
        finally:
            client.close()

    def test_mismatched_response_id_is_not_used_as_a_decision(self):
        code = '''import json,sys
print(json.dumps({'protocol':'open-pokered-judge-v1','ready':True}),flush=True)
sys.stdin.readline();print(json.dumps({'id':999}),flush=True)
'''
        client = CommandAdapter(self.config(code))
        try:
            with self.assertRaisesRegex(TypeSafeError, 'ID mismatch'):
                client.system_one({}, {'x': Choice('Pick', {'a': 'A', 'b': 'B'})})
        finally:
            client.close()

    def test_invalid_choices_and_probabilities_are_rejected(self):
        questions = {'q': Choice('Pick', {'a': 'A', 'b': 'B'})}
        for answer in [
            {'type': 'choice', 'choice': 'outside'},
            {'type': 'choice', 'choice': 'a', 'probabilities': {'a': .6, 'b': .6}},
            {'type': 'choice', 'choice': 'a', 'probabilities': {'a': float('nan'), 'b': .4}},
            {'type': 'choice', 'choice': 'a', 'probabilities': {'a': 1}},
        ]:
            with self.assertRaises(TypeSafeError):
                validate_answers({'q': answer}, questions)

    def test_null_usage_is_not_reported_as_known_zero(self):
        self.assertEqual(usage_from_json(None), {'input_tokens': None, 'output_tokens': None})
        self.assertEqual(usage_from_json({'input_tokens': 0})['input_tokens'], 0)
        with self.assertRaises(TypeSafeError):
            usage_from_json({'input_tokens': -1})

    def test_meter_keeps_null_tokens_in_journal_and_counts_missing_usage(self):
        journal = io.StringIO()
        budget = EvaluationBudget(20, 0)
        model = MeasuredModel('stub', budget, journal, config=self.config())
        try:
            budget.start()
            model.system_one({}, {'q': Choice('Pick', {'a': 'A', 'b': 'B'})})
            row = json.loads(journal.getvalue())
            self.assertIsNone(row['input_tokens'])
            self.assertEqual(model.summary()['usage_missing_calls'], 1)
            self.assertEqual(model.summary()['input_tokens'], 0)
        finally:
            model.close()

    def test_typesafe_missing_usage_survives_legacy_client_defaults(self):
        body = {'model': 'test', 'answers': {'q': {'type': 'choice', 'choice': 'a', 'probabilities': {'a': 1, 'b': 0}}}}
        with patch.dict('os.environ', {'BENCH_TEST_KEY': 'test-only'}):
            adapter = TypeSafeAdapter({'model': 'test', 'api_key_env': 'BENCH_TEST_KEY'},
                                      lambda *a, **k: io.BytesIO(json.dumps(body).encode()))
            adapter.system_one({}, {'q': Choice('Pick', {'a': 'A', 'b': 'B'})})
            self.assertEqual(adapter.last_usage, {'input_tokens': None, 'output_tokens': None})


class ScoringTests(unittest.TestCase):
    def test_label_only_models_do_not_get_fabricated_probability_scores(self):
        score = score_answer({'acceptable': ['a']}, ChoiceAnswer('a', {}, 0))
        self.assertTrue(score['correct'])
        self.assertIsNone(score['brier'])
        self.assertIsNone(score['negative_log_likelihood'])

    def test_multiple_acceptable_actions_use_total_correct_mass(self):
        score = score_answer({'acceptable': ['a', 'b']}, ChoiceAnswer('a', {'a': .3, 'b': .3, 'none': .4}, .1))
        self.assertAlmostEqual(score['acceptable_probability'], .6)
        self.assertIsNone(score['brier'])

    def test_accuracy_requires_coverage_and_order_pairs_are_not_independent_cases(self):
        row = {'case_id': 'c', 'layer': 'action', 'category': 'navigation', 'split': 'development',
               'order': 'original', 'valid': True, 'correct': True, 'choice': 'a', 'brier': None, 'negative_log_likelihood': None}
        metric = decision_metrics({'expected_trials': 4, 'trials': [row, {**row, 'order': 'reversed', 'choice': 'b', 'correct': False}]})
        self.assertEqual(metric['coverage'], .5)
        self.assertEqual(metric['accuracy'], .5)
        self.assertEqual(metric['order_consistency'], 0)
        self.assertEqual(metric['case_count'], 1)

    def test_missing_game_observations_do_not_become_zero_scores(self):
        row = flatten({'job': {'model_id': 'x', 'track': 'story', 'seed': 42, 'repetition': 0},
                       'summary': {'reason': 'worker_failed'}, 'requests': []})
        self.assertIsNone(row['story_objectives'])
        self.assertIsNone(row['money_gained'])
        self.assertFalse(row['scored_observation'])

    def test_shutdown_time_is_distinct_from_last_scored_sample(self):
        summary = {'reason': 'effective_time_budget', 'clock': {'effective_s': 21.2, 'limit_s': 20},
                   'metrics': {'final': {'effective_s': 19.9}, 'milestones': {}}}
        row = flatten({'job': {'model_id': 'x', 'track': 'story', 'seed': 42, 'repetition': 0},
                       'summary': summary, 'requests': []})
        self.assertEqual(row['scored_until_s'], 19.9)
        self.assertAlmostEqual(row['shutdown_overrun_s'], 1.2)


class ReportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def campaign(self, name, ids, seeds=(42,), change=None):
        folder = self.root/name
        folder.mkdir()
        conditions = {'purpose': 'smoke', 'profile': 'native-controller-v1', 'source_sha256': 'same-code',
                      'story': {'seconds': 20}, **(change or {})}
        models = [{'id': mid, 'provider': 'command', 'model': mid, 'command': ['stub']} for mid in ids]
        jobs = [{'id': f'decisions-{mid}-{seed}', 'track': 'decisions', 'model_id': mid, 'seed': seed, 'repetition': 0}
                for mid in ids for seed in seeds]
        frozen = {'benchmark': 'test-only', 'config': {'models': models}, 'jobs': jobs,
                  'conditions': conditions, 'comparison_key': digest(conditions), 'model_artifacts': {mid: {} for mid in ids}}
        (folder/'plan.json').write_text(json.dumps(frozen))
        for job in jobs:
            model = next(m for m in models if m['id'] == job['model_id'])
            metadata = {**job, 'conditions': conditions, 'comparison_key': frozen['comparison_key'],
                        'model_config_sha256': digest({'config': model, 'artifacts': {}})}
            path = folder/job['id']
            path.mkdir()
            (path/'summary.json').write_text(json.dumps({'benchmark': metadata, 'reason': 'worker_failed',
                                                        'trials': [], 'expected_trials': 32}))
        return folder

    @patch('openpokered.benchmark_reports.plot', return_value=False)
    def test_later_third_model_merges_without_hardcoded_names_or_columns(self, _):
        a = self.campaign('first', ['one', 'two'])
        b = self.campaign('later', ['third'])
        output = self.root/'report'
        result = report([a, b], output)
        self.assertEqual({r['model_id'] for r in result}, {'one', 'two', 'third'})
        self.assertTrue((output/'metrics.csv').exists())
        self.assertEqual(len(json.loads((output/'results.json').read_text())['aggregates']), 3)

    def test_cannot_mix_different_controller_versions(self):
        a = self.campaign('a', ['one'])
        b = self.campaign('b', ['two'], change={'source_sha256': 'different-code'})
        with self.assertRaisesRegex(ValueError, 'Non-comparable'):
            report([a, b], self.root/'out')

    def test_cannot_double_count_the_same_episode(self):
        a = self.campaign('a', ['one'])
        with self.assertRaisesRegex(ValueError, 'Duplicate'):
            report([a, a], self.root/'out')

    def test_model_coverage_must_use_the_same_seeds(self):
        a = self.campaign('a', ['one'], seeds=(42,))
        b = self.campaign('b', ['two'], seeds=(43,))
        with self.assertRaisesRegex(ValueError, 'Unequal seed'):
            report([a, b], self.root/'out')

    def test_actual_model_alias_drift_is_not_silently_averaged(self):
        folder = self.campaign('a', ['one'], seeds=(42, 43))
        for seed, version in [(42, 'version-a'), (43, 'version-b')]:
            path = folder/f'decisions-one-{seed}'/'summary.json'
            data = json.loads(path.read_text())
            data['model'] = {'actual_models': [version]}
            path.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, 'Actual model version changed'):
            report([folder], self.root/'out')

    def test_tokens_must_reconcile_with_journal(self):
        folder = self.campaign('a', ['one'])
        path = folder/'decisions-one-42'/'summary.json'
        data = json.loads(path.read_text())
        data.update(reason='completed', model={'calls': 0, 'input_tokens': 123, 'output_tokens': 0})
        path.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, 'input_tokens'):
            report([folder], self.root/'out')

    def test_killed_worker_journal_only_salvages_unfinished_final_line(self):
        path = self.root/'journal.jsonl'
        path.write_bytes(b'{"a":1}\n{"partial":')
        with self.assertRaises(ValueError):
            rows(path)
        self.assertEqual(rows(path, allow_truncated=True), [{'a': 1}])
        path.write_bytes(b'{"a":1}\ncorrupt\n')
        with self.assertRaises(ValueError):
            rows(path, allow_truncated=True)


if __name__ == '__main__':
    unittest.main()

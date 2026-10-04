"""Unit checks for the preference probe — stub client only, no network and no
key, so the decision-level pipeline is exercisable while API credits are spent.

The stub answers through the bias text it finds in the instructions, which is
also how the tests tell which preference asked a question: a question that lost
its bias reads as `none` here.

Run: PYTHONPATH=scripts python3 -m unittest test_openpokered_preference_probe
"""
import copy
import contextlib
import hashlib
import io
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from openpokered import preference_probe as probe
from openpokered import typesafe as ts
from openpokered.playthrough_judgments import PREFERENCE_INSTRUCTIONS


# ── stub client ───────────────────────────────────────────────────────
def bias_of(instructions):
    """Which preference asked this question, read off the appended bias."""
    return next((name for name, bias in PREFERENCE_INSTRUCTIONS.items()
                 if instructions.endswith(f' {bias}')), 'none')


def answers(by_preference, default=None):
    """A per-preference answer table; a list is cycled over the repeats.

    An entry is used only where the case offers it, so one table can drive a
    case set whose candidates differ from case to case.
    """
    def pick(preference, question, index):
        criteria = list(question.criteria)
        choice = by_preference.get(preference, default)
        if isinstance(choice, list):
            choice = choice[index % len(choice)]
        return criteria[0] if choice not in criteria else choice
    return pick


class StubClient:
    """A scripted System One client: records requests, replays one choice.

    `pick(preference, question, call index)` returns the choice; probabilities
    put 0.7 on it, so a test can tell which options carried mass.
    """
    model = 'jev-stub'

    def __init__(self, pick=None, error=None, input_tokens=11, output_tokens=5):
        self.pick = pick or answers({})
        self.error = error
        self.input_tokens, self.output_tokens = input_tokens, output_tokens
        self.requests = []

    def system_one(self, state, questions, model=None):
        (layer, question), = questions.items()
        preference = bias_of(question.instructions)
        index = len(self.requests)
        self.requests.append({'state': state, 'layer': layer, 'question': question,
                              'model': model or self.model, 'preference': preference})
        if self.error is not None:
            raise self.error
        chosen = self.pick(preference, question, index)
        others = [key for key in question.criteria if key != chosen]
        probabilities = {chosen: 0.7, **{key: 0.3/len(others) for key in others}}
        return ts.SystemOneResult(model or self.model,
                                  {layer: ts.ChoiceAnswer(chosen, probabilities, 0.7)},
                                  self.input_tokens, self.output_tokens)


class FakeResponse:
    def __init__(self, payload):
        self.payload = payload

    def read(self):
        return json.dumps(self.payload).encode()

    def __enter__(self):
        return self

    def __exit__(self, *rest):
        return False


class ScriptedOpener:
    """The injectable seam of the real transport: bodies are built, never sent."""

    def __init__(self, payload):
        self.payload, self.calls = payload, []

    def __call__(self, request, timeout=None):
        self.calls.append(request)
        return FakeResponse(self.payload)


# ── fixtures ──────────────────────────────────────────────────────────
CASES_PATH = Path(__file__).resolve().parent/'openpokered'/'preference_cases.json'
CASE = probe.load_cases()['cases'][0]
TRAIN, CHALLENGE, HEAL = list(CASE['question']['criteria'])


def case(identifier, criteria, expect, layer='strategy'):
    return {'id': identifier, 'layer': layer, 'category': 'test',
            'state': {'situation': identifier},
            'question': {'type': 'choice', 'instructions': f'Pick one for {identifier}.',
                         'criteria': criteria},
            'expect': expect, 'rationale': 'Test fixture.'}


TWO_OPTIONS = {'yes': 'Take it', 'no': 'Decline it'}


def two_option_suite():
    return {'schema': probe.SCHEMA,
            'cases': [case('yes_case', TWO_OPTIONS, {'none': ['yes'], 'level': ['no']}),
                      case('no_case', TWO_OPTIONS, {'none': ['no']})]}


def call_records(choices, repeats=3):
    """Hand-built call records: `choices` maps (case id, preference) to a list."""
    out = []
    for selected, picks in choices.items():
        for repeat in range(repeats):
            out.append({'case_id': selected[0], 'layer': 'strategy', 'category': 'test',
                        'preference': selected[1], 'repeat': repeat, 'requested_model': None,
                        'model': 'jev-stub', 'choice': picks[repeat % len(picks)],
                        'probabilities': {}, 'confidence': None, 'input_tokens': 1,
                        'output_tokens': 1, 'request_sha256': '0'*64})
    return out


class CaseSetTests(unittest.TestCase):
    def test_checked_in_case_set_is_valid(self):
        suite = probe.load_cases()
        self.assertEqual(suite['schema'], probe.SCHEMA)
        self.assertEqual(len(suite['cases']), 6)
        self.assertEqual({c['layer'] for c in suite['cases']}, {'strategy', 'action'})
        for entry in suite['cases']:
            self.assertTrue(3 <= len(entry['question']['criteria']) <= 5, entry['id'])
            self.assertTrue(entry['rationale'] and entry['state'], entry['id'])
            for preference, acceptable in (entry['expect'] or {}).items():
                self.assertIn(preference, probe.PREFERENCES, entry['id'])
                if acceptable is not None:
                    self.assertLessEqual(set(acceptable), set(entry['question']['criteria']), entry['id'])

    def test_two_strategy_cases_offer_train_and_challenge(self):
        strategies = [c for c in probe.load_cases()['cases'] if c['layer'] == 'strategy']
        self.assertEqual(len(strategies), 2)
        for entry in strategies:
            criteria = entry['question']['criteria']
            self.assertIn('train', criteria)
            self.assertIn('challenge', criteria)
            self.assertEqual(entry['expect']['level'], ['train'], entry['id'])

    def test_move_cases_share_one_opponent(self):
        moves = [c for c in probe.load_cases()['cases'] if c['id'].startswith('move_')]
        self.assertEqual(len(moves), 2)
        self.assertEqual({c['state']['enemy']['species'] for c in moves}, {'Starmie'})

    def test_turn_cases_offer_attack_item_and_switch(self):
        turns = [c for c in probe.load_cases()['cases'] if c['id'].startswith('turn_')]
        self.assertEqual(len(turns), 2)
        for entry in turns:
            criteria = list(entry['question']['criteria'])
            self.assertEqual(criteria[0], 'attack', entry['id'])
            self.assertTrue(any(key.startswith('item:') for key in criteria), entry['id'])
            self.assertTrue(any(key.startswith('switch:') for key in criteria), entry['id'])
            self.assertEqual(entry['expect']['tactic'], [key for key in criteria
                                                         if key.startswith('item:')], entry['id'])

    def test_wrong_schema_is_refused(self):
        path = self.write({'schema': 'something-else', 'cases': [CASE]})
        with self.assertRaises(probe.ProbeError):
            probe.load_cases(path)

    def test_duplicate_ids_are_refused(self):
        path = self.write({'schema': probe.SCHEMA, 'cases': [CASE, copy.deepcopy(CASE)]})
        with self.assertRaises(probe.ProbeError):
            probe.load_cases(path)

    def test_expect_must_name_offered_candidates(self):
        broken = copy.deepcopy(CASE)
        broken['expect'] = {'level': ['teleport']}
        path = self.write({'schema': probe.SCHEMA, 'cases': [broken]})
        with self.assertRaises(probe.ProbeError):
            probe.load_cases(path)

    def test_expect_must_name_a_known_preference(self):
        broken = copy.deepcopy(CASE)
        broken['expect'] = {'aggressive': ['train']}
        path = self.write({'schema': probe.SCHEMA, 'cases': [broken]})
        with self.assertRaises(probe.ProbeError):
            probe.load_cases(path)

    def write(self, suite):
        folder = self.tmp()
        path = Path(folder)/'cases.json'
        path.write_text(json.dumps(suite))
        return path

    def tmp(self):
        folder = tempfile.mkdtemp(prefix='preference-probe-test-')
        self.addCleanup(shutil.rmtree, folder, ignore_errors=True)
        return folder


class CaseDataTests(unittest.TestCase):
    """A frozen state may only name Pokemon, moves and items the engine has."""

    DATA = Path(__file__).resolve().parents[1]/'crates'/'pokered-data'

    def test_referenced_species_moves_and_items_exist(self):
        species, moves, items = set(), set(), set()
        for entry in probe.load_cases()['cases']:
            referenced(entry['state'], ('species', 'wild_species'), species)
            referenced(entry['state'], ('move', 'moves', 'usable_attacks'), moves)
            referenced(entry['state'], ('item',), items)
        self.assertTrue(species and moves and items)
        for folder, names in (('pokemon', species), ('moves', moves),
                              ('data/items', items)):
            for name in names:
                self.assertTrue((self.DATA/folder/f'{name}.json').exists(),
                                f'{folder}/{name}.json is missing')

    def test_offered_candidates_are_described_not_bare(self):
        for entry in probe.load_cases()['cases']:
            for key, description in entry['question']['criteria'].items():
                self.assertTrue(description.strip().endswith('.'), key)
                self.assertGreater(len(description.split()), 8, key)


def names(value):
    """The candidate names under one matched key.

    The states use three shapes: a name, a list of names, and a mapping of
    name to list of names (`usable_attacks`). Anything deeper is a different
    kind of field — a move's own description, for instance — and is skipped.
    """
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        return [entry for entry in value if isinstance(entry, str)]
    if isinstance(value, dict):
        return [entry for nested in value.values() if isinstance(nested, list)
                for entry in nested if isinstance(entry, str)]
    return []


def referenced(state, keys, found):
    """Collect the names under `keys`, wherever the state nests them."""
    if isinstance(state, dict):
        for key, value in state.items():
            if key in keys:
                found.update(names(value))
            referenced(value, keys, found)
    elif isinstance(state, list):
        for entry in state:
            referenced(entry, keys, found)


class InstructionTests(unittest.TestCase):
    def test_bias_appended_for_each_preference(self):
        for preference, bias in PREFERENCE_INSTRUCTIONS.items():
            self.assertEqual(probe.instruction_for(CASE, preference),
                             CASE['question']['instructions'] + ' ' + bias, preference)

    def test_none_appends_nothing(self):
        self.assertEqual(probe.instruction_for(CASE, 'none'), CASE['question']['instructions'])

    def test_only_the_bias_differs(self):
        instructions = [probe.instruction_for(CASE, preference) for preference in probe.PREFERENCES]
        self.assertEqual(len(set(instructions)), len(probe.PREFERENCES))
        for text in instructions:
            self.assertTrue(text.startswith(CASE['question']['instructions']))

    def test_candidate_order_is_preserved(self):
        criteria = list(CASE['question']['criteria'])
        for preference in probe.PREFERENCES:
            self.assertEqual(list(probe.question_for(CASE, preference).criteria), criteria, preference)

    def test_the_stub_sees_the_same_bias_the_game_appends(self):
        client = StubClient(answers({}))
        probe.run_call(client, CASE, 'tactic')
        self.assertEqual(client.requests[0]['preference'], 'tactic')
        probe.run_call(client, CASE, 'none')
        self.assertEqual(client.requests[1]['preference'], 'none')


class RequestHashTests(unittest.TestCase):
    def test_hash_covers_the_body_the_transport_posts(self):
        opener = ScriptedOpener({'model': 'jev-1.13.0', 'answers': {
            'strategy': {'type': 'choice', 'choice': TRAIN, 'probabilities': {TRAIN: 0.6},
                         'confidence': 0.5}}})
        client = ts.TypeSafeClient('https://api.example', 'k', 'jev-latest', opener=opener)
        client.system_one(CASE['state'], {CASE['layer']: probe.question_for(CASE, 'type')},
                          model='jev-1.13.0')
        self.assertEqual(opener.calls[0].data.decode(),
                         probe.request_body(CASE, 'type', 'jev-1.13.0'))

    def test_hash_follows_the_criteria(self):
        changed = copy.deepcopy(CASE)
        changed['question']['criteria'][HEAL] = 'Heal somewhere else entirely'
        self.assertNotEqual(probe.request_body(CASE, 'none', 'm'),
                            probe.request_body(changed, 'none', 'm'))

    def test_hash_follows_the_candidate_order(self):
        reordered = copy.deepcopy(CASE)
        reordered['question']['criteria'] = dict(reversed(list(reordered['question']['criteria'].items())))
        self.assertNotEqual(probe.request_body(CASE, 'none', 'm'),
                            probe.request_body(reordered, 'none', 'm'))

    def test_hash_follows_the_preference_bias_and_model(self):
        self.assertNotEqual(probe.request_body(CASE, 'none', 'm'),
                            probe.request_body(CASE, 'level', 'm'))
        self.assertNotEqual(probe.request_body(CASE, 'level', 'm'),
                            probe.request_body(CASE, 'level', 'other'))

    def test_recorded_hash_is_the_hash_of_the_recorded_body(self):
        record = probe.run_call(StubClient(answers({})), CASE, 'level')
        body = probe.request_body(CASE, 'level', StubClient.model)
        self.assertEqual(record['request_sha256'], hashlib.sha256(body.encode()).hexdigest())
        self.assertEqual(record['request_sha256'],
                         probe.run_call(StubClient(answers({})), CASE, 'level')['request_sha256'])


class CallRecordTests(unittest.TestCase):
    def test_record_carries_choice_distribution_and_usage(self):
        record = probe.run_call(StubClient(answers({'none': CHALLENGE})), CASE, 'none', repeat=2)
        self.assertEqual(record['case_id'], CASE['id'])
        self.assertEqual(record['choice'], CHALLENGE)
        self.assertEqual(record['probabilities'][CHALLENGE], 0.7)
        self.assertEqual(record['confidence'], 0.7)
        self.assertEqual((record['input_tokens'], record['output_tokens']), (11, 5))
        self.assertEqual(record['model'], 'jev-stub')
        self.assertEqual(record['repeat'], 2)

    def test_choice_outside_criteria_is_an_error(self):
        client = StubClient(lambda preference, question, index: 'teleport')
        with self.assertRaises(probe.ProbeError) as raised:
            probe.run_call(client, CASE, 'none')
        self.assertIn('teleport', str(raised.exception))
        self.assertIn('offered criteria', str(raised.exception))

    def test_service_error_is_an_error(self):
        client = StubClient(answers({}), error=ts.TypeSafeError('HTTP 500: boom'))
        with self.assertRaises(probe.ProbeError):
            probe.run_call(client, CASE, 'none')

    def test_missing_answer_is_an_error(self):
        class Silent(StubClient):
            def system_one(self, state, questions, model=None):
                return ts.SystemOneResult(self.model, {}, 1, 1)
        with self.assertRaises(probe.ProbeError):
            probe.run_call(Silent(answers({})), CASE, 'none')


class AggregationTests(unittest.TestCase):
    def test_modal_choice_over_repeats(self):
        records = self.records([CHALLENGE, CHALLENGE, TRAIN])
        self.assertEqual(probe.modal(records, [TRAIN, CHALLENGE]), CHALLENGE)

    def test_tie_breaks_on_mean_probability(self):
        records = self.records([CHALLENGE, TRAIN])
        records[0]['probabilities'] = {CHALLENGE: 0.6, TRAIN: 0.4}
        records[1]['probabilities'] = {CHALLENGE: 0.2, TRAIN: 0.3}
        self.assertEqual(probe.modal(records, [TRAIN, CHALLENGE]), CHALLENGE)

    def test_tie_breaks_on_the_order_offered(self):
        records = self.records([CHALLENGE, TRAIN])
        self.assertEqual(records[0]['probabilities'], {})
        self.assertEqual(probe.modal(records, [CHALLENGE, TRAIN]), CHALLENGE)
        self.assertEqual(probe.modal(records, [TRAIN, CHALLENGE]), TRAIN)

    def test_empty_aggregation_is_none(self):
        self.assertIsNone(probe.modal([], [TRAIN, CHALLENGE]))

    def test_summary_reports_agreement_and_mean_probabilities(self):
        summary = probe.summarize(probe.load_cases()['cases'],
                                  self.records([TRAIN, TRAIN, CHALLENGE], 'level'))
        cell = summary[0]['preferences']['level']
        self.assertEqual(cell['modal_choice'], TRAIN)
        self.assertEqual(cell['choices'], [TRAIN, TRAIN, CHALLENGE])
        self.assertEqual(cell['agreement'], 0.6667)
        self.assertEqual(cell['calls'], 3)

    def records(self, picks, preference='none'):
        return call_records({('strategy_pewter_level_margin', preference): picks}, len(picks))


class VerdictTests(unittest.TestCase):
    def summary(self, suite, choices, repeats=3):
        return probe.summarize(suite['cases'], call_records(choices, repeats))

    def test_hits_are_counted_per_preference(self):
        suite = two_option_suite()
        summary = self.summary(suite, {('yes_case', 'none'): ['yes'],
                                       ('yes_case', 'level'): ['yes'],
                                       ('no_case', 'none'): ['no']})
        counts = {(row['case_id'], name): row['preferences'][name]['hit']
                  for row in summary for name in row['preferences']}
        self.assertTrue(counts[('yes_case', 'none')])
        self.assertFalse(counts[('yes_case', 'level')])
        self.assertTrue(counts[('no_case', 'none')])
        table = probe.predictions(summary)
        self.assertEqual((table['none']['hits'], table['none']['predicted']), (2, 2))
        self.assertEqual((table['level']['hits'], table['level']['predicted']), (0, 1))
        self.assertEqual(table['tactic'], {'hits': 0, 'predicted': 0, 'misses': []})

    def test_misses_name_the_case_and_choice(self):
        suite = two_option_suite()
        summary = self.summary(suite, {('yes_case', 'level'): ['yes']})
        self.assertEqual(probe.predictions(summary)['level']['misses'],
                         [{'case_id': 'yes_case', 'choice': 'yes', 'expected': ['no']}])

    def test_a_null_prediction_is_not_scored(self):
        suite = {'schema': probe.SCHEMA,
                 'cases': [case('open_case', TWO_OPTIONS, {'level': None}, layer='action')]}
        summary = self.summary(suite, {('open_case', 'level'): ['yes']})
        row = summary[0]['preferences']['level']
        self.assertIsNone(row['hit'])
        self.assertEqual((row['expected'], summary[0]['predicted']), (None, 0))
        self.assertEqual(probe.predictions(summary)['level']['predicted'], 0)

    def test_the_checked_in_predictions_are_directional(self):
        expect = {entry['id']: entry['expect'] for entry in probe.load_cases()['cases']}
        self.assertEqual(expect['strategy_pewter_level_margin']['level'], ['train'])
        self.assertEqual(expect['move_vs_starmie_super_effective']['type'], ['thundershock'])
        self.assertIsNone(expect['move_vs_starmie_no_super_effective']['type'])
        for entry in ('turn_vs_starmie_cure_paralysis', 'turn_vs_starmie_potion_hold_the_line'):
            self.assertTrue(expect[entry]['tactic'][0].startswith('item:'), entry)


class DifferentiationTests(unittest.TestCase):
    def suite(self):
        return {'schema': probe.SCHEMA,
                'cases': [case('shared_case', TWO_OPTIONS, {'none': ['yes'], 'level': ['no']})]}

    def test_false_when_every_preference_agrees(self):
        suite = self.suite()
        summary = probe.summarize(suite['cases'],
                                  call_records({('shared_case', 'none'): ['yes'],
                                                ('shared_case', 'level'): ['yes']}))
        self.assertFalse(summary[0]['differs'])
        self.assertFalse(probe.differentiates(summary))

    def test_true_when_a_modal_choice_differs(self):
        suite = self.suite()
        summary = probe.summarize(suite['cases'],
                                  call_records({('shared_case', 'none'): ['yes'],
                                                ('shared_case', 'level'): ['no']}))
        self.assertTrue(summary[0]['differs'])
        self.assertTrue(probe.differentiates(summary))

    def test_a_minority_of_repeats_does_not_differentiate(self):
        suite = self.suite()
        summary = probe.summarize(suite['cases'],
                                  call_records({('shared_case', 'none'): ['yes', 'yes', 'no'],
                                                ('shared_case', 'level'): ['yes', 'yes', 'no']}))
        self.assertEqual(summary[0]['preferences']['level']['choices'], ['yes', 'yes', 'no'])
        self.assertFalse(probe.differentiates(summary))


class RunTests(unittest.TestCase):
    def setUp(self):
        self.output = Path(self.tmp())/'probe'
        self.suite = probe.load_cases()

    def test_run_writes_both_artifacts_and_counts_every_call(self):
        client = StubClient(answers({}, default=TRAIN))
        result, code = probe.run_probe(client, self.suite, self.output, repeats=2,
                                       echo=lambda *a, **k: None)
        self.assertEqual(code, 0)
        self.assertTrue(result['complete'])
        self.assertEqual(len(result['calls']), len(self.suite['cases'])*len(probe.PREFERENCES)*2)
        self.assertTrue((self.output/'probe.json').exists())
        self.assertTrue((self.output/'README.md').exists())
        stored = json.loads((self.output/'probe.json').read_text())
        self.assertEqual(stored['run']['cases_sha256'],
                         hashlib.sha256(CASES_PATH.read_bytes()).hexdigest())
        self.assertEqual(len(stored['calls']), len(result['calls']))
        self.assertEqual([cell['modal_choice'] for cell in stored['cases'][0]['preferences'].values()],
                         [TRAIN]*len(probe.PREFERENCES))

    def test_readme_reports_rows_cases_and_columns_preferences(self):
        probe.run_probe(StubClient(answers({}, default=TRAIN)), self.suite, self.output,
                        repeats=1, echo=lambda *a, **k: None)
        report = (self.output/'README.md').read_text()
        self.assertIn('| case | layer | ' + ' | '.join(probe.PREFERENCES) + ' | differs |', report)
        for entry in self.suite['cases']:
            self.assertIn(f'| {entry["id"]} |', report)

    def test_readme_marks_a_miss(self):
        client = StubClient(answers({'level': CHALLENGE}, default=TRAIN))
        probe.run_probe(client, self.suite, self.output, repeats=1, echo=lambda *a, **k: None)
        report = (self.output/'README.md').read_text()
        self.assertIn(f'{CHALLENGE} MISS', report)
        self.assertIn('strategy_pewter_level_margin → ' + CHALLENGE, report)

    def test_summary_lines_carry_hits_and_differentiation(self):
        client = StubClient(answers({'level': CHALLENGE}, default=TRAIN))
        result, _code = probe.run_probe(client, self.suite, self.output, repeats=1,
                                        echo=lambda *a, **k: None)
        lines = '\n'.join(probe.summary_lines(result))
        self.assertIn('differentiates: True', lines)
        self.assertIn('predictions:', lines)
        self.assertIn('level 0/', lines)

    def test_existing_output_is_refused(self):
        self.output.mkdir()
        with self.assertRaises(probe.ProbeError):
            probe.run_probe(StubClient(answers({})), self.suite, self.output,
                            echo=lambda *a, **k: None)

    def test_a_service_error_stops_the_run_and_stays_visible(self):
        client = StubClient(answers({}, default=TRAIN), error=ts.TypeSafeError('HTTP 529: overloaded'))
        result, code = probe.run_probe(client, self.suite, self.output, repeats=2,
                                       echo=lambda *a, **k: None)
        self.assertEqual(code, 1)
        self.assertFalse(result['complete'])
        self.assertIn('HTTP 529', result['error'])
        self.assertLess(len(result['calls']), result['run']['expected_calls'])
        stored = json.loads((self.output/'probe.json').read_text())
        self.assertFalse(stored['complete'])

    def test_an_unusable_answer_stops_the_run(self):
        client = StubClient(lambda preference, question, index: 'not-offered')
        result, code = probe.run_probe(client, self.suite, self.output, repeats=1,
                                       echo=lambda *a, **k: None)
        self.assertEqual(code, 1)
        self.assertEqual(result['calls'], [])
        self.assertIn('not-offered', result['error'])

    def test_differentiation_is_reported_per_case(self):
        """The bias only moves the strategy cases, so only they may differ."""
        def pick(preference, question, index):
            criteria = list(question.criteria)
            if TRAIN not in criteria:
                return criteria[0]
            return TRAIN if preference == 'level' else CHALLENGE
        result, _code = probe.run_probe(StubClient(pick), self.suite, self.output, repeats=1,
                                        echo=lambda *a, **k: None)
        self.assertTrue(result['differentiates'])
        self.assertEqual(sorted(result['different_cases']),
                         ['strategy_pewter_level_margin', 'strategy_vermilion_level_margin'])
        self.assertEqual(result['predictions']['level']['hits'], 2)

    def test_option_subsets_are_carried_into_the_metadata(self):
        result, _code = probe.run_probe(StubClient(answers({}, default=TRAIN)), self.suite,
                                        self.output, preferences=['none', 'level'], models=['jev-1.13.0'],
                                        repeats=1, echo=lambda *a, **k: None)
        self.assertEqual(result['run']['preferences'], ['none', 'level'])
        self.assertEqual(result['run']['models'], ['jev-1.13.0'])
        self.assertEqual(result['run']['expected_calls'], len(self.suite['cases'])*2)
        self.assertEqual({record['requested_model'] for record in result['calls']}, {'jev-1.13.0'})

    def test_invalid_options_are_refused(self):
        client = StubClient(answers({}))
        for preferences, repeats in ((['aggressive'], 1), ([], 1), (['none', 'none'], 1), (['none'], 0)):
            with self.assertRaises(probe.ProbeError):
                probe.run_probe(client, self.suite, self.output, preferences=preferences,
                                repeats=repeats, echo=lambda *a, **k: None)

    def tmp(self):
        folder = tempfile.mkdtemp(prefix='preference-probe-test-')
        self.addCleanup(shutil.rmtree, folder, ignore_errors=True)
        return folder


class CommandLineTests(unittest.TestCase):
    def setUp(self):
        self.output = Path(self.tmp())/'probe'

    def main(self, argv, client):
        """The CLI path, with the summary it prints kept out of the report."""
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return probe.main(argv, client=client)

    def test_main_runs_the_whole_probe_against_an_injected_client(self):
        code = self.main(['--cases', str(CASES_PATH), '--repeats', '1', '--preferences', 'none,level',
                          '--output', str(self.output)], StubClient(answers({}, default=TRAIN)))
        self.assertEqual(code, 0)
        self.assertTrue((self.output/'probe.json').exists())

    def test_main_refuses_an_existing_output(self):
        self.output.mkdir()
        self.assertEqual(self.main(['--cases', str(CASES_PATH), '--output', str(self.output)],
                                   StubClient(answers({}))), 2)

    def test_main_refuses_a_broken_case_set(self):
        path = Path(self.tmp())/'cases.json'
        path.write_text(json.dumps({'schema': 'nope', 'cases': [CASE]}))
        self.assertEqual(self.main(['--cases', str(path), '--output', str(self.output)],
                                   StubClient(answers({}))), 2)

    def test_main_reports_a_failed_run_as_nonzero(self):
        client = StubClient(answers({}), error=ts.TypeSafeError('HTTP 500: boom'))
        self.assertEqual(self.main(['--cases', str(CASES_PATH), '--output', str(self.output)], client), 1)

    def test_defaults_are_the_checked_in_case_set_and_every_preference(self):
        args = probe.parse_args(['--output', 'x'])
        self.assertEqual(args.cases, probe.CASES_PATH)
        self.assertEqual(args.repeats, 3)
        self.assertEqual(args.preferences, ','.join(probe.PREFERENCES))
        self.assertEqual(args.models, '')

    def tmp(self):
        folder = tempfile.mkdtemp(prefix='preference-probe-test-')
        self.addCleanup(shutil.rmtree, folder, ignore_errors=True)
        return folder


if __name__ == '__main__':
    unittest.main()

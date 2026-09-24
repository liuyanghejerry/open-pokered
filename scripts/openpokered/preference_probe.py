#!/usr/bin/env python3
"""Fixed decision-level probe for the playstyle preference dimension.

A preference that never changes a decision is a cosmetic flag. This replays a
frozen case set once per preference — same state, same question, only the bias
text appended to the instructions — and reports which option each preference
actually picked.

The judgment is stochastic (about three probability points of run-to-run
noise), so every (case, preference) pair is asked `--repeats` times and the
report compares modal choices rather than a single draw.

Stdlib only, and the client is injected: the whole pipeline — case loading,
instruction building, per-call recording, modal aggregation and expect
checking — is pure code that tests and inline harnesses exercise against a
stub, with no network and no key.

    python3 scripts/openpokered/preference_probe.py \
        --cases scripts/openpokered/preference_cases.json \
        --repeats 3 --output .artifacts/preference-probe
"""
import argparse
import hashlib
import json
import platform
import sys
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from openpokered.evaluation import atomic_json
from openpokered.playthrough_judgments import (PREFERENCE_INSTRUCTIONS,
                                               preference_suffix)
from openpokered.typesafe import Choice, TypeSafeClient, TypeSafeError, load_env_file

SCHEMA = 'open-pokered-preference-probe-v1'
CASES_PATH = Path(__file__).resolve().parent / 'preference_cases.json'
# 'none' first: the unbiased default is the baseline the others are read against.
PREFERENCES = ('none', *PREFERENCE_INSTRUCTIONS)
LAYERS = ('strategy', 'action')


class ProbeError(RuntimeError):
    """A run never continues past a bad case, a bad answer or a failed call."""


class Playstyle:
    """The one attribute `preference_suffix` reads.

    The probe asks for the bias through the production helper, so the text it
    appends is the same text the game appends and cannot drift from it.
    """

    def __init__(self, preference):
        self.preference = preference


# ── cases and questions ───────────────────────────────────────────────
def load_cases(path=CASES_PATH):
    """The frozen case set, validated before a single request is sent."""
    suite = json.loads(Path(path).read_text())
    if suite.get('schema') != SCHEMA:
        raise ProbeError(f'cases must declare schema {SCHEMA!r}')
    cases = suite.get('cases')
    if not isinstance(cases, list) or not cases:
        raise ProbeError('cases must be a nonempty array')
    seen = set()
    for case in cases:
        identifier = case.get('id')
        if not isinstance(identifier, str) or not identifier or identifier in seen:
            raise ProbeError('case IDs must be unique nonempty strings')
        seen.add(identifier)
        if case.get('layer') not in LAYERS or not case.get('category'):
            raise ProbeError(f'{identifier}: layer must be one of {LAYERS} and category is required')
        if 'state' not in case or not case.get('rationale'):
            raise ProbeError(f'{identifier}: state and a human-reviewable rationale are required')
        question = case.get('question') or {}
        criteria = question.get('criteria')
        if not isinstance(criteria, dict) or len(criteria) < 2:
            raise ProbeError(f'{identifier}: at least two named candidates are required')
        if question.get('type') != 'choice' or not isinstance(question.get('instructions'), str) or not question['instructions']:
            raise ProbeError(f'{identifier}: a self-contained choice question is required')
        expect = case.get('expect', {})
        if not isinstance(expect, dict):
            raise ProbeError(f'{identifier}: expect must be an object keyed by preference')
        for preference, acceptable in expect.items():
            if preference not in PREFERENCES:
                raise ProbeError(f'{identifier}: expect names unknown preference {preference!r}')
            if acceptable is None:
                continue
            if not isinstance(acceptable, list) or not acceptable or not set(acceptable) <= set(criteria):
                raise ProbeError(f'{identifier}/{preference}: expect must be null or a nonempty subset of the candidates')
    return suite


def instruction_for(case, preference):
    """The question as this playstyle reads it; 'none' appends nothing."""
    return case['question']['instructions'] + preference_suffix(Playstyle(preference))


def question_for(case, preference):
    return Choice(instruction_for(case, preference), case['question']['criteria'])


def request_body(case, preference, model):
    """Exactly the bytes the transport posts for this call.

    Mirrors `TypeSafeClient.system_one`; a unit test pins the two together, so
    the recorded hash covers the real request rather than a description of it.
    """
    return json.dumps({'state': case['state'], 'model': model,
                       'questions': {case['layer']: question_for(case, preference).to_json()}})


# ── recording ─────────────────────────────────────────────────────────
def run_call(client, case, preference, model=None, repeat=0):
    """One judgment, with everything the report needs to attribute it."""
    question = question_for(case, preference)
    try:
        result = client.system_one(case['state'], {case['layer']: question}, model=model)
    except TypeSafeError as failure:
        raise ProbeError(f'{case["id"]}/{preference}: service error: {failure}') from failure
    answer = result.answers.get(case['layer'])
    if answer is None:
        raise ProbeError(f'{case["id"]}/{preference}: no answer for layer {case["layer"]!r}')
    offered = list(case['question']['criteria'])
    if answer.choice not in offered:
        # A choice outside the offer is not a wrong answer to score: it is an
        # answer no caller could act on, so it stops the run.
        raise ProbeError(f'{case["id"]}/{preference}: returned {answer.choice!r}, '
                         f'not one of the offered criteria {offered}')
    requested = model or getattr(client, 'model', None)
    return {'case_id': case['id'], 'layer': case['layer'], 'category': case['category'],
            'preference': preference, 'repeat': repeat,
            'requested_model': model or None, 'model': result.model or requested,
            'choice': answer.choice, 'probabilities': dict(answer.probabilities),
            'confidence': getattr(answer, 'confidence', None),
            'input_tokens': result.input_tokens, 'output_tokens': result.output_tokens,
            'request_sha256': hashlib.sha256(request_body(case, preference, requested).encode()).hexdigest()}


# ── aggregation ───────────────────────────────────────────────────────
def mean(values):
    values = list(values)
    return sum(values)/len(values) if values else 0.0


def modal(records, criteria):
    """The most repeated choice, or None when there is nothing to aggregate.

    Ties break on the mean probability of the choice, then on the order the
    candidates were offered, so a report never depends on dict ordering.
    """
    if not records:
        return None
    counts = Counter(record['choice'] for record in records)
    means = {key: mean(record['probabilities'].get(key, 0.0) for record in records)
             for key in criteria}
    return max(criteria, key=lambda key: (counts[key], means[key]))


def aggregate(records):
    """{(case id, preference): [records]} — what each report cell aggregates."""
    grouped = defaultdict(list)
    for record in records:
        grouped[(record['case_id'], record['preference'])].append(record)
    return grouped


def verdict(choice, acceptable):
    """True/False against a prediction, or None where none was made."""
    if not acceptable:
        return None
    return choice in acceptable


def summarize(cases, records):
    """One row per case: modal choice per preference, and the expect verdict."""
    grouped = aggregate(records)
    summary = []
    for case in cases:
        criteria = list(case['question']['criteria'])
        expect = case.get('expect') or {}
        preferences, predicted, hits = {}, 0, 0
        for preference in PREFERENCES:
            group = grouped.get((case['id'], preference))
            if not group:
                continue
            choice = modal(group, criteria)
            hit = verdict(choice, expect.get(preference))
            predicted += 1 if hit is not None else 0
            hits += 1 if hit else 0
            preferences[preference] = {
                'calls': len(group), 'choices': [record['choice'] for record in group],
                'modal_choice': choice,
                'agreement': round(Counter(record['choice'] for record in group)[choice]/len(group), 4),
                'mean_probabilities': {key: round(mean(record['probabilities'].get(key, 0.0) for record in group), 4)
                                       for key in criteria},
                'expected': expect.get(preference), 'hit': hit}
        summary.append({
            'case_id': case['id'], 'layer': case['layer'], 'category': case['category'],
            'criteria': criteria, 'rationale': case['rationale'], 'preferences': preferences,
            'differs': len({cell['modal_choice'] for cell in preferences.values()}) > 1,
            'predicted': predicted, 'hits': hits})
    return summary


def predictions(summary):
    """Per preference: hits, predictions made, and the misses by name."""
    table = {preference: {'hits': 0, 'predicted': 0, 'misses': []} for preference in PREFERENCES}
    for case in summary:
        for preference, cell in case['preferences'].items():
            if cell['hit'] is None:
                continue
            table[preference]['predicted'] += 1
            if cell['hit']:
                table[preference]['hits'] += 1
            else:
                table[preference]['misses'].append({'case_id': case['case_id'],
                                                    'choice': cell['modal_choice'],
                                                    'expected': cell['expected']})
    return table


def differentiates(summary):
    """True when at least one case's modal choice differs across preferences."""
    return any(case['differs'] for case in summary)


# ── run ───────────────────────────────────────────────────────────────
def validate_options(preferences, models, repeats):
    if not preferences or len(set(preferences)) != len(preferences) or set(preferences) - set(PREFERENCES):
        raise ProbeError(f'preferences must be a unique subset of {list(PREFERENCES)}')
    for model in models:
        if not model:
            raise ProbeError('model IDs must be nonempty strings')
    if isinstance(repeats, bool) or not isinstance(repeats, int) or repeats < 1:
        raise ProbeError('repeats must be a positive integer')


def sha_file(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def run_probe(client, suite, output, *, cases_path=CASES_PATH, preferences=PREFERENCES,
              models=(), repeats=3, echo=print):
    """Ask every (case, preference, model, repeat); write both artifacts.

    Returns (result, exit code). A transport failure or an unusable answer
    stops the run, keeps the records already collected and exits non-zero —
    a partial probe must never read as a complete one.
    """
    validate_options(preferences, models, repeats)
    output = Path(output)
    if output.exists():
        raise ProbeError(f'{output} already exists; a probe run is never overwritten')
    cases = suite['cases']
    requested = list(models) or [None]
    records, error = [], None
    try:
        for model in requested:
            for case in cases:
                for preference in preferences:
                    for repeat in range(repeats):
                        record = run_call(client, case, preference, model=model, repeat=repeat)
                        records.append(record)
                        echo(f'  {case["id"]:<42} {preference:<7} {record["choice"]}')
    except ProbeError as failure:
        error = str(failure)
        echo(f'error: {error}', file=sys.stderr)
    summary = summarize(cases, records)
    result = {
        'schema': SCHEMA, 'complete': error is None, 'error': error,
        'run': {'started': datetime.now(timezone.utc).isoformat(timespec='seconds'),
                'python': platform.python_version(),
                'cases': str(Path(cases_path).resolve()), 'cases_sha256': sha_file(cases_path),
                'runner_sha256': sha_file(Path(__file__).resolve()),
                'case_count': len(cases), 'preferences': list(preferences),
                'models': [model or '<client default>' for model in requested],
                'repeats': repeats, 'calls': len(records),
                'expected_calls': len(cases)*len(preferences)*len(requested)*repeats},
        'calls': records, 'cases': summary, 'predictions': predictions(summary),
        'differentiates': differentiates(summary),
        'different_cases': [case['case_id'] for case in summary if case['differs']],
    }
    output.mkdir(parents=True)
    atomic_json(output/'probe.json', result)
    (output/'README.md').write_text('\n'.join(readme(result)))
    echo('\n'.join(summary_lines(result)))
    return result, (0 if error is None else 1)


# ── reporting ─────────────────────────────────────────────────────────
def fmt(value):
    if value is None:
        return '—'
    if isinstance(value, float):
        return f'{value:.3f}'
    return str(value).replace('|', '\\|').replace('\n', ' ')


def table(headers, values):
    return ['| ' + ' | '.join(headers) + ' |', '| ' + ' | '.join('---' for _ in headers) + ' |',
            *['| ' + ' | '.join(fmt(v) for v in row) + ' |' for row in values]]


def cell_text(cell):
    """A modal choice, marked when it contradicts the case's prediction."""
    if cell is None:
        return None
    return cell['modal_choice'] + (' MISS' if cell['hit'] is False else '')


def summary_lines(result):
    """The compact stdout table: modal choices, hit counts, differentiation."""
    preferences = result['run']['preferences']
    rows = [[case['case_id'], *[cell_text(case['preferences'].get(p)) for p in preferences],
             'yes' if case['differs'] else None] for case in result['cases']]
    headers = ['case', *preferences, 'differs']
    cells = [[fmt(value) for value in row] for row in [headers, *rows]]
    widths = [max(len(row[index]) for row in cells) for index in range(len(headers))]
    lines = ['  '.join(header.ljust(width) for header, width in zip(cells[0], widths)).rstrip(),
             '  '.join('-'*width for width in widths)]
    lines += ['  '.join(value.ljust(width) for value, width in zip(row, widths)).rstrip()
              for row in cells[1:]]
    hits = result['predictions']
    lines.append('')
    lines.append('predictions: ' + '  '.join(
        f'{preference} {hits[preference]["hits"]}/{hits[preference]["predicted"]}'
        if hits[preference]['predicted'] else f'{preference} n/a' for preference in preferences))
    lines.append(f'calls: {result["run"]["calls"]}/{result["run"]["expected_calls"]}'
                 + (f'  error: {result["error"]}' if result['error'] else ''))
    lines.append(f'differentiates: {result["differentiates"]}'
                 + (f' — {len(result["different_cases"])}/{len(result["cases"])} cases differ across preferences: '
                    + ', '.join(result['different_cases']) if result['different_cases'] else ''))
    return lines


def readme(result):
    """The human-readable report: one row per case, one column per preference."""
    run = result['run']
    preferences = run['preferences']
    text = [f'# Preference probe — {result["schema"]}', '',
            'Every case is asked under the same state and the same question, with only the playstyle bias '
            'appended to the instructions. Judgments move by about three probability points between runs, so each '
            'case is repeated and the columns below compare modal choices, not single draws.', '',
            *table(['field', 'value'], [
                ['cases', f'{run["case_count"]} (`{run["cases_sha256"][:12]}` sha256 of the case file)'],
                ['preferences', ', '.join(preferences)],
                ['model(s)', ', '.join(run['models'])],
                ['repeats per (case, preference)', run['repeats']],
                ['calls recorded', f'{run["calls"]} of {run["expected_calls"]}'],
                ['complete', 'yes' if result['complete'] else f'NO — {result["error"]}'],
                ['started (UTC)', run['started']],
                ['runner sha256', f'`{run["runner_sha256"][:12]}`'],
            ]), '',
            '## Modal choice per case', '',
            *table(['case', 'layer', *preferences, 'differs'],
                   [[case['case_id'], case['layer'],
                     *[cell_text(case['preferences'].get(p)) for p in preferences],
                     'yes' if case['differs'] else '—'] for case in result['cases']]), '',
            'A cell is the modal choice of the repeats; `MISS` marks a choice outside the case\'s `expect` '
            'prediction, and `—` marks a preference the case makes no directional prediction for. '
            '`differs` means the case picked more than one option across the preferences.', '',
            '## Prediction hits', '',
            *table(['preference', 'matching cases', 'predicted cases', 'misses'],
                   [[preference, result['predictions'][preference]['hits'],
                     result['predictions'][preference]['predicted'],
                     ', '.join(f'{m["case_id"]} → {m["choice"]}' for m in result['predictions'][preference]['misses']) or '—']
                    for preference in preferences]), '',
            '`predicted cases` counts only the cells carrying an `expect` prediction; a case that predicts '
            'nothing for a preference cannot be scored for it.', '',
            '## Predictions and modal choices', '',
            *table(['case', 'preference', 'expect', 'modal choice', 'verdict', 'choices per repeat'],
                   [[case['case_id'], preference, ', '.join(cell['expected']), cell['modal_choice'],
                     'miss' if cell['hit'] is False else 'hit', ', '.join(cell['choices'])]
                    for case in result['cases'] for preference in preferences
                    for cell in [case['preferences'].get(preference)]
                    if cell is not None and cell['hit'] is not None]), '',
            '## Calls', '',
            *table(['case', 'preference', 'choices per repeat', 'mean probabilities',
                    'p(modal mean)', 'model', 'input tokens', 'output tokens', 'request sha256'],
                   [[case['case_id'], preference, ', '.join(cell['choices']),
                     ', '.join(f'{key} {value:.2f}' for key, value in cell['mean_probabilities'].items()),
                     cell['mean_probabilities'].get(cell['modal_choice']),
                     next((record['model'] for record in result['calls']
                           if record['case_id'] == case['case_id'] and record['preference'] == preference), None),
                     sum(record['input_tokens'] for record in result['calls']
                         if record['case_id'] == case['case_id'] and record['preference'] == preference),
                     sum(record['output_tokens'] for record in result['calls']
                         if record['case_id'] == case['case_id'] and record['preference'] == preference),
                     next((f'`{record["request_sha256"][:12]}`' for record in result['calls']
                           if record['case_id'] == case['case_id'] and record['preference'] == preference), None)]
                    for case in result['cases'] for preference in preferences
                    for cell in [case['preferences'].get(preference)] if cell is not None]), '',
            '## Cases', '',
            *table(['case', 'layer', 'category', 'candidates', 'rationale'],
                   [[case['case_id'], case['layer'], case['category'], ', '.join(case['criteria']),
                     case['rationale']] for case in result['cases']]), '',
            f'**differentiates: {result["differentiates"]}** — '
            + (f'{len(result["different_cases"])} of {len(result["cases"])} cases pick a different option '
               f'across the preferences ({", ".join(result["different_cases"])}).'
               if result['different_cases'] else
               'no case picks a different option across the preferences.'),
            '',
            'Per-call records — full probability distributions, token counts and the request hash of every '
            'call — are in `probe.json`. The case set is an authored fixture, not a held-out sample: it shows '
            'whether the bias changes the decision, not how well any of these choices would play out.', '']
    return text


# ── entry point ───────────────────────────────────────────────────────
def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--cases', type=Path, default=CASES_PATH,
                        help='frozen case set; defaults to the file next to this script')
    parser.add_argument('--repeats', type=int, default=3,
                        help='judgments per (case, preference); judgments are stochastic')
    parser.add_argument('--output', type=Path, required=True,
                        help='directory for probe.json and README.md; must not exist yet')
    parser.add_argument('--models', default='',
                        help='comma-separated System One model IDs; empty uses the client default')
    parser.add_argument('--preferences', default=','.join(PREFERENCES),
                        help=f'comma-separated subset of {",".join(PREFERENCES)}')
    parser.add_argument('--env-file', type=Path,
                        help='file of KEY=VALUE lines to fill missing environment variables')
    return parser.parse_args(argv)


def main(argv=None, client=None):
    args = parse_args(argv)
    preferences = [name.strip() for name in args.preferences.split(',') if name.strip()]
    models = [name.strip() for name in args.models.split(',') if name.strip()]
    try:
        suite = load_cases(args.cases)
        validate_options(preferences, models, args.repeats)
        if client is None:
            if args.env_file:
                load_env_file(args.env_file)
            client = TypeSafeClient.from_env()
        _result, code = run_probe(client, suite, args.output, cases_path=args.cases,
                                  preferences=preferences, models=models, repeats=args.repeats)
    except ProbeError as failure:
        print(f'probe refused: {failure}', file=sys.stderr)
        return 2
    return code


if __name__ == '__main__':
    raise SystemExit(main())

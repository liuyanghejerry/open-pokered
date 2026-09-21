#!/usr/bin/env python3
"""Plan, run and report reproducible model benchmarks without changing gameplay."""
import argparse
import copy
import hashlib
import json
import math
import os
import platform
import random
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from openpokered.benchmark_models import validate_model, question_from_json, validate_answers
from openpokered.evaluation import EvaluationBudget, EvaluationStopped, atomic_json
from openpokered.evaluation_models import MeasuredModel
from openpokered.typesafe import Choice, TypeSafeError, load_env_file

ROOT = Path(__file__).resolve().parents[2]
VERSION = 'open-pokered-autonomy-v1'
PROFILE = 'native-controller-v1'


def sha_file(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False).encode()).hexdigest()


def case_digest(value):
    # Candidate/state field order is part of the actual model input, unlike
    # unordered bookkeeping fields. Sorting here would hide order changes.
    return hashlib.sha256(json.dumps(value, ensure_ascii=False).encode()).hexdigest()


def positive(value, label, integer=False):
    if isinstance(value, bool) or not isinstance(value, int if integer else (int, float)) or not math.isfinite(value) or value <= 0:
        raise ValueError(f'{label} must be a positive {"integer" if integer else "finite number"}')


def load_cases(path):
    suite = json.loads(Path(path).read_text())
    if suite.get('schema_version') != 1 or not suite.get('id') or not suite.get('cases'):
        raise ValueError('Cases require schema_version=1, id and a nonempty cases array')
    identifiers = set()
    for case in suite['cases']:
        if not isinstance(case.get('id'), str) or case['id'] in identifiers:
            raise ValueError('Case IDs must be unique strings')
        identifiers.add(case['id'])
        if case.get('split') not in ('development', 'evaluation'):
            raise ValueError('Every case must declare development/evaluation provenance')
        if case.get('layer') not in ('strategy', 'action') or not case.get('category'):
            raise ValueError('Every case needs layer and category')
        question = case['question']
        if question.get('type') != 'choice':
            raise ValueError('Decision suite v1 uses choice questions')
        criteria = question.get('criteria')
        if not isinstance(criteria, dict) or len(criteria) < 2:
            raise ValueError('Cases need at least two named candidates')
        if not isinstance(question.get('instructions'), str) or not question['instructions']:
            raise ValueError('Cases require self-contained instructions')
        expected = case.get('acceptable')
        if not isinstance(expected, list) or not expected or len(set(expected)) != len(expected) or not set(expected) <= set(criteria):
            raise ValueError('Acceptable answers must be a nonempty subset of candidates')
        if not case.get('rationale') or 'state' not in case:
            raise ValueError('Cases need state and a human-reviewable labeling rationale')
    return suite


def load_manifest(path):
    path = Path(path).resolve()
    raw = json.loads(path.read_text())
    allowed = {'schema_version', 'benchmark', 'profile', 'models', 'seeds', 'repeats', 'tracks', 'story', 'decisions'}
    if set(raw)-allowed:
        raise ValueError(f'Unknown manifest fields: {sorted(set(raw)-allowed)}')
    if raw.get('schema_version') != 1 or raw.get('benchmark') != VERSION or raw.get('profile') != PROFILE:
        raise ValueError(f'Expected schema_version=1, benchmark={VERSION}, profile={PROFILE}')
    models = [validate_model(c) for c in raw.get('models', [])]
    ids = [c['id'] for c in models]
    if not models or len(set(ids)) != len(ids):
        raise ValueError('Provide at least one model with unique IDs')
    import re
    if not all(re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,79}', name) for name in ids):
        raise ValueError('Model IDs must be path-safe names of at most 80 characters')
    seeds = raw.get('seeds', [42, 43, 44])
    if not isinstance(seeds, list) or not seeds or any(isinstance(s, bool) or not isinstance(s, int) or not 0 <= s < 2**64 for s in seeds) or len(set(seeds)) != len(seeds):
        raise ValueError('seeds must be unique unsigned 64-bit integers')
    repeats = raw.get('repeats', 1)
    positive(repeats, 'repeats', integer=True)
    tracks = raw.get('tracks', ['decisions', 'story'])
    if not isinstance(tracks, list) or not tracks or len(set(tracks)) != len(tracks) or set(tracks)-{'decisions', 'story'}:
        raise ValueError('tracks must contain decisions and/or story, without duplicates')
    story = {'seconds': 1200, 'max_rtt_credit_s': 300, **raw.get('story', {})}
    if set(story)-{'seconds', 'max_rtt_credit_s'}:
        raise ValueError('Unknown story settings')
    positive(story['seconds'], 'story.seconds')
    if isinstance(story['max_rtt_credit_s'], bool):
        raise ValueError('RTT cap must be a number, not a boolean')
    EvaluationBudget(story['seconds'], story['max_rtt_credit_s'])
    decisions = {'seconds': 300, 'option_orders': ['original', 'reversed'], **raw.get('decisions', {})}
    if set(decisions)-{'seconds', 'option_orders', 'cases'}:
        raise ValueError('Unknown decision settings')
    positive(decisions['seconds'], 'decisions.seconds')
    if decisions['option_orders'] not in (['original'], ['original', 'reversed']):
        raise ValueError('option_orders must be [original] or [original, reversed]')
    if 'decisions' in tracks:
        cases_path = path.parent / decisions.get('cases', 'cases-v1.json')
        load_cases(cases_path)
        decisions['cases'] = str(cases_path.resolve())
    for model in models:
        if model['provider'] == 'command':
            # Resolve relative file arguments against the manifest, not a shell.
            model['command'] = [str((path.parent/a).resolve()) if not Path(a).is_absolute() and (path.parent/a).is_file() else a
                                for a in model['command']]
    return {**raw, 'models': models, 'seeds': seeds, 'repeats': repeats,
            'tracks': tracks, 'story': story, 'decisions': decisions}


def environment(binary=None):
    sources = sorted([*Path(__file__).parent.glob('*.py'), ROOT/'scripts/playthrough.py',
                      ROOT/'scripts/playthrough_late.py', ROOT/'scripts/debug_drive.py'])
    data = sorted(p for p in (ROOT/'crates/pokered-data').rglob('*')
                  if p.is_file() and p.suffix in ('.json', '.scene', '.rs', '.blk', '.gui'))
    # Navigation helpers also parse engine-owned warp/spinner/field-move tables.
    data += [ROOT/'tools/map_data.json', *sorted((ROOT/'crates/pokered-core/src/overworld').rglob('*.rs'))]
    def hashes(paths):
        return {str(p.relative_to(ROOT)): sha_file(p) for p in paths if p.is_file()}
    source_hashes, data_hashes = hashes(sources), hashes(data)
    host = {'system': platform.system(), 'release': platform.release(), 'machine': platform.machine(),
            'processor': platform.processor(), 'python': sys.version}
    if sys.platform == 'darwin':
        for key in ('machdep.cpu.brand_string', 'hw.memsize'):
            host[key] = subprocess.check_output(['sysctl', '-n', key], text=True).strip()
    return {'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
            'source_files': source_hashes, 'source_sha256': digest(source_hashes),
            'data_files': data_hashes, 'data_sha256': digest(data_hashes), 'host': host,
            'binary_sha256': sha_file(binary) if binary else None}


def plan(manifest, binary=None, smoke_seconds=None):
    config = copy.deepcopy(manifest)
    if 'story' in config['tracks'] and (not binary or not Path(binary).is_file()):
        raise ValueError('Story track requires --binary with read-only evaluation telemetry')
    if smoke_seconds is not None:
        positive(smoke_seconds, 'smoke seconds')
        config['story']['seconds'] = smoke_seconds
        config['story']['max_rtt_credit_s'] = 0
        config['seeds'] = config['seeds'][:1]
        config['repeats'] = 1
    env = environment(binary)
    suite = load_cases(config['decisions']['cases']) if 'decisions' in config['tracks'] else None
    conditions = {'benchmark': VERSION, 'profile': PROFILE, 'purpose': 'smoke' if smoke_seconds is not None else 'evaluation',
                  'source_sha256': env['source_sha256'], 'data_sha256': env['data_sha256'],
                  'host': env['host'], 'binary_sha256': env['binary_sha256'], 'story': config['story'],
                  'decisions': {k: v for k, v in config['decisions'].items() if k != 'cases'},
                  'cases_sha256': case_digest(suite) if suite else None}
    jobs = []
    # Rotate across seed/repetition blocks, preventing a fixed model order.
    block = 0
    for repetition in range(config['repeats']):
        for seed in config['seeds']:
            models = config['models'][block % len(config['models']):] + config['models'][:block % len(config['models'])]
            for track in config['tracks']:
                for model in models:
                    jobs.append({'id': f'{track}-{model["id"]}-s{seed}-r{repetition}', 'track': track,
                                 'model_id': model['id'], 'seed': seed, 'repetition': repetition})
            block += 1
    artifacts = {}
    for model in config['models']:
        paths = list(model.get('command', []))
        if paths and shutil.which(paths[0]):
            paths[0] = shutil.which(paths[0])
        artifacts[model['id']] = {str(Path(p).resolve()): sha_file(p) for p in paths if Path(p).is_file()}
    return {'schema_version': 1, 'benchmark': VERSION, 'config': config, 'environment': env,
            'conditions': conditions, 'comparison_key': digest(conditions), 'cases': suite,
            'model_artifacts': artifacts,
            'jobs': jobs, 'schedule_policy': 'Serial; rotate model order for each seed/repetition block.',
            'binary': str(Path(binary).resolve()) if binary else None}


def decision_trials(suite, seed, orders):
    cases = copy.deepcopy(suite['cases'])
    random.Random(seed).shuffle(cases)
    for case in cases:
        for order in orders:
            question = copy.deepcopy(case['question'])
            if order == 'reversed':
                question['criteria'] = dict(reversed(list(question['criteria'].items())))
            yield case, order, question


def score_answer(case, answer):
    correct = answer.choice in case['acceptable']
    probabilities = answer.probabilities
    # Probability metrics are absent for label-only adapters, never fabricated.
    mass = sum(probabilities.get(k, 0) for k in case['acceptable']) if probabilities else None
    brier = None
    if probabilities and len(case['acceptable']) == 1:
        brier = sum((p-(k == case['acceptable'][0]))**2 for k, p in probabilities.items())
    return {'choice': answer.choice, 'correct': correct, 'acceptable_probability': mass,
            'negative_log_likelihood': -math.log(max(mass, 1e-12)) if mass is not None else None,
            'brier': brier, 'abstained': answer.choice == 'none'}


def decision_worker(args):
    spec = json.loads(args.job.read_text())
    out = args.output
    out.mkdir(parents=True, exist_ok=True)
    budget = EvaluationBudget(spec['seconds'], 0)
    rows, model, reason = [], None, 'completed'
    def snapshot():
        return {'track': 'decisions', 'backend': spec['model']['id'], 'benchmark': spec['metadata'],
                'clock': budget.snapshot(), 'reason': reason, 'trials': rows,
                'expected_trials': len(spec['cases']['cases'])*len(spec['orders']),
                'model': model.summary() if model else None}
    def status():
        atomic_json(out/'checkpoint.json', snapshot())
    started = time.monotonic()
    with (out/'requests.jsonl').open('w') as journal:
        try:
            model = MeasuredModel(spec['model']['id'], budget, journal, status, config=spec['model'])
            model.system_one({'purpose': 'Warm up the decision adapter before the timed benchmark.'},
                             {'warmup': Choice('Which label describes this preparation?', {'warmup': 'Prepare for evaluation', 'gameplay': 'Already playing'})})
            setup = time.monotonic()-started
            budget.start()
            status()
            for case, order, definition in decision_trials(spec['cases'], spec['seed'], spec['orders']):
                budget.check()
                row = {'case_id': case['id'], 'layer': case['layer'], 'category': case['category'],
                       'split': case['split'], 'order': order, 'acceptable': case['acceptable']}
                question = question_from_json(definition)
                try:
                    result = model.system_one(case['state'], {'decision': question})
                    raw = {k: {'type': definition['type'], **vars(a)} for k, a in result.answers.items()}
                    validate_answers(raw, {'decision': question})
                    row.update(score_answer(case, result.answers['decision']), valid=True)
                except TypeSafeError as error:
                    row.update(valid=False, error=str(error))
                    # A transport/protocol failure stops the run; a valid wrong
                    # answer is scored and does not stop later cases.
                    reason = 'model_error'
                    rows.append(row)
                    break
                rows.append(row)
                status()
        except EvaluationStopped as error:
            reason = str(error)
        except Exception as error:
            reason = f'{type(error).__name__}: {error}'
        finally:
            budget.finish()
            result = snapshot()
            result['setup_s'] = locals().get('setup', time.monotonic()-started)
            atomic_json(out/'summary.json', result)
            if model:
                model.close()
    return 0 if reason == 'completed' else 1


def wait_process(command, out, seconds, effective=False):
    """Own one process group; do not let hung adapters survive the campaign."""
    launched = time.monotonic()
    forced = False
    with (out/'launcher.log').open('w') as log:
        process = subprocess.Popen(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            while process.poll() is None:
                now = time.monotonic()
                if effective:
                    path = out/'checkpoint.json'
                    checkpoint = json.loads(path.read_text()) if path.exists() else {}
                    clock = checkpoint.get('clock', {})
                    start = clock.get('start_monotonic')
                    expired = now-start >= seconds if start is not None else now-launched >= 180
                else:
                    # The story runner owns its effective timer and inner process
                    # group. This is only an outer catastrophic-failure bound.
                    expired = now-launched >= seconds
                if expired:
                    forced = True
                    break
                time.sleep(.1)
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
            # Includes an adapter that survived after its Python parent exited.
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
    atomic_json(out/'launcher.json', {'returncode': process.returncode, 'forced_termination': forced,
                                      'wall_s': time.monotonic()-launched})
    return process.returncode


def execute(plan_data, output):
    output.mkdir(parents=True, exist_ok=False)
    atomic_json(output/'plan.json', plan_data)
    (output/'specs').mkdir()
    configs = {m['id']: m for m in plan_data['config']['models']}
    for model in configs.values():
        atomic_json(output/'specs'/(model['id']+'.json'), model)
    atomic_json(output/'campaign.json', {'finished': False, 'jobs_finished': 0})
    def verify_frozen_files():
        for section in ('source_files', 'data_files'):
            for path, expected in plan_data['environment'][section].items():
                if sha_file(ROOT/path) != expected:
                    raise RuntimeError(f'Benchmark input changed during campaign: {path}')
        if plan_data['binary'] and sha_file(plan_data['binary']) != plan_data['environment']['binary_sha256']:
            raise RuntimeError('Game binary changed during campaign')
        for files in plan_data['model_artifacts'].values():
            for path, expected in files.items():
                if sha_file(path) != expected:
                    raise RuntimeError(f'Model adapter artifact changed during campaign: {path}')
    for number, job in enumerate(plan_data['jobs']):
        verify_frozen_files()
        out = output/job['id']
        metadata = {**job, 'comparison_key': plan_data['comparison_key'], 'conditions': plan_data['conditions'],
                    'model_config_sha256': digest({'config': configs[job['model_id']],
                                                  'artifacts': plan_data['model_artifacts'][job['model_id']]})}
        job_path = output/'specs'/(job['id']+'.json')
        if job['track'] == 'story':
            atomic_json(job_path, metadata)
            settings = plan_data['config']['story']
            command = [sys.executable, str(ROOT/'scripts/openpokered/run_model_evaluation.py'), job['model_id'],
                       '--binary', plan_data['binary'], '--output', str(out), '--seconds', str(settings['seconds']),
                       '--max-rtt-credit', str(settings['max_rtt_credit_s']), '--seed', str(job['seed']),
                       '--model-config', str(output/'specs'/(job['model_id']+'.json')), '--run-metadata', str(job_path)]
            # The existing story supervisor creates and owns this directory.
            # Capture its one-line result in the campaign log; it enforces its
            # own setup/deadline/process-group watchdog.
            with (output/'story-launches.log').open('a') as log:
                code = subprocess.call(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
        else:
            spec = {'metadata': metadata, 'model': configs[job['model_id']], 'cases': plan_data['cases'],
                    'seed': job['seed'], 'orders': plan_data['config']['decisions']['option_orders'],
                    'seconds': plan_data['config']['decisions']['seconds']}
            atomic_json(job_path, spec)
            out.mkdir()
            code = wait_process([sys.executable, str(Path(__file__).resolve()), '_decisions', '--job', str(job_path),
                                 '--output', str(out)], out, spec['seconds'], effective=True)
        if not (out/'summary.json').exists():
            checkpoint = out/'checkpoint.json'
            fallback = json.loads(checkpoint.read_text()) if checkpoint.exists() else {}
            fallback.update(backend=job['model_id'], track=job['track'], benchmark=metadata, reason='worker_failed',
                            warning='Incomplete checkpoint: pending request usage may be missing.')
            out.mkdir(exist_ok=True)
            atomic_json(out/'summary.json', fallback)
        verify_frozen_files()
        print(f'[{number+1}/{len(plan_data["jobs"])}] {job["id"]}: exit {code}', flush=True)
        atomic_json(output/'campaign.json', {'finished': number+1 == len(plan_data['jobs']), 'jobs_finished': number+1})
    from openpokered.benchmark_reports import report
    report([output], output/'report')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    for name in ('validate', 'plan', 'run'):
        p = sub.add_parser(name)
        p.add_argument('manifest', type=Path)
        if name != 'validate':
            p.add_argument('--binary', type=Path)
            p.add_argument('--smoke-seconds', type=float, help='Mark as smoke, use one seed/repetition and a short story limit')
        if name == 'run':
            p.add_argument('--output', type=Path, required=True)
            p.add_argument('--env-file', type=Path)
    p = sub.add_parser('report')
    p.add_argument('runs', type=Path, nargs='+')
    p.add_argument('--output', type=Path, required=True)
    p = sub.add_parser('_decisions', help=argparse.SUPPRESS)
    p.add_argument('--job', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.command == '_decisions':
        return decision_worker(args)
    if args.command == 'report':
        from openpokered.benchmark_reports import report
        report([p.resolve() for p in args.runs], args.output.resolve())
        return 0
    manifest = load_manifest(args.manifest)
    if args.command == 'validate':
        print(json.dumps({'valid': True, 'models': [m['id'] for m in manifest['models']], 'tracks': manifest['tracks'],
                          'seeds': manifest['seeds'], 'repeats': manifest['repeats']}, indent=2))
        return 0
    frozen = plan(manifest, args.binary, args.smoke_seconds)
    if args.command == 'plan':
        print(json.dumps(frozen, ensure_ascii=False, indent=2))
    else:
        if args.env_file:
            load_env_file(args.env_file)
        execute(frozen, args.output.resolve())
        print(args.output.resolve()/'report/README.md')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

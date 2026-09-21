#!/usr/bin/env python3
"""Offline, non-gameplay ablations of the frozen Laya failure requests.

Never rewrites the formal evaluation or installed laya_mlx package. Expanded
encoding is a diagnostic intervention, not a supported checkpoint setting.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import time
import types

os.environ.setdefault('HF_HUB_OFFLINE', '1')

from openpokered.evaluation_models import LAYA_MODEL, LAYA_REVISION


def encoding(agent, state, questions, *, option_limit=48, head_limit=192, max_len=512):
    from laya_mlx.common import QTYPES, render_options, serialize_state
    tok = agent.tok
    encode = lambda s: tok(s.replace(tok.mask_token, ' '))['input_ids']
    state_ids = encode(serialize_state(state))
    items, internals, audits = [], [], []
    for qid, definition in questions.items():
        q = agent._to_internal(definition)
        head = encode('%s question: %s' % (q['t'], q['ins']))
        full_options = [encode(' ' + s) for s in render_options(q)]
        options = [[tok.mask_token_id] + s[:option_limit] for s in full_options]
        room = head_limit - sum(map(len, options))
        if room < 16:
            per = max(4, (head_limit - 16) // len(options))
            options = [s[:per] for s in options]
            room = head_limit - sum(map(len, options))
        used_head = head[:max(8, room)]
        ids = [tok.cls_token_id] + used_head + [tok.sep_token_id]
        markers = []
        for option in options:
            markers.append(len(ids))
            ids.extend(option)
        ids.append(tok.sep_token_id)
        used_state = state_ids[:max(0, max_len - len(ids) - 1)]
        ids += used_state + [tok.sep_token_id]
        assert len(ids) <= max_len
        items.append({'ids': ids, 'markers': markers, 'qtype': QTYPES[q['t']]})
        internals.append(q)
        audits.append({'question': qid, 'encoded_tokens': len(ids),
                       'instruction_tokens': [len(head), len(used_head)],
                       'state_tokens': [len(state_ids), len(used_state)],
                       'option_tokens': [[len(a), len(b) - 1] for a, b in zip(full_options, options)],
                       'input_ids_sha256': hashlib.sha256(json.dumps(ids).encode()).hexdigest(),
                       'effective_input_text': tok.backend.decode(ids, skip_special_tokens=False)})
    return items, internals, audits


def compact(record):
    """Hand-written diagnostic equivalents of the initial three requests only."""
    questions = copy.deepcopy(record['questions'])
    qid, question = next(iter(questions.items()))
    if qid == 'strategy':
        descriptions = {
            'subgoal:0': 'Make Prof. Oak appear at the north exit of Pallet Town; advances choosing a starter Pokemon and receiving the Pokedex.',
            'subgoal:1': "Get Oak's Parcel at Viridian Mart; advances receiving the Pokedex.",
            'none': 'None of these candidates can advance the current goal.',
        }
        state = ('Location: upstairs in the player house (RedsHouse2F). Party: empty. '
                 'No story flags, items or badges. Goals: choose a starter, receive the Pokedex, '
                 'earn eight badges and become champion. Map routes exist to PalletTown via '
                 'RedsHouse1F, and to ViridianMart via RedsHouse1F, PalletTown, Route1 and ViridianCity. '
                 'No known navigation failures.')
        if record['number'] == 4:
            state += ' Last judgment rejected travel to PalletTown; no game action was executed.'
        instruction = ('Which attainable subgoal advances the story? Prefer necessary early '
                       'prerequisites. Travel can precede a script interaction. Consider supplied routes and failures.')
    else:
        descriptions = {
            'action:0': 'Travel to PalletTown via RedsHouse1F (2 map hops); the north-exit script there makes Prof. Oak appear.',
            'none': 'None of these candidates can advance the current goal.',
        }
        state = ('Location: RedsHouse2F (3,6). Party: empty. No story flags, items or badges. '
                 'Money: 3000. Goal: make Prof. Oak appear in PalletTown to advance choosing '
                 'a starter Pokemon and receiving the Pokedex. No recent execution outcomes.')
        instruction = ('Which next operation progresses toward the goal? Travel can be an intermediate '
                       'step before the script interaction completes it.')
    question['criteria'] = {k: descriptions[k] for k in question['criteria']}
    question['instructions'] = instruction
    return state, questions


def variants(record):
    state, original = record['state'], record['questions']
    short_state, short_questions = compact(record)
    qid = next(iter(original))
    yield 'native', state, original, {}
    yield 'repeat_native', state, original, {}
    yield 'empty_state', '', original, {}
    yield 'max_len_1024_only', state, original, {'max_len': 1024}
    # On the first action the entire original text fits in 512 tokens, including
    # all 149 option tokens and the unchanged instruction/state.
    yield 'full_options_512', state, original, {'option_limit': 1024, 'head_limit': 448}
    yield 'full_input_1024', state, original, {'option_limit': 1024, 'head_limit': 640, 'max_len': 1024}
    for part in ('criteria', 'instructions'):
        q = copy.deepcopy(original)
        q[qid][part] = short_questions[qid][part]
        yield 'compact_' + part + '_only', state, q, {}
    yield 'compact_state_only', short_state, original, {}
    yield 'compact_all', short_state, short_questions, {}
    q = copy.deepcopy(short_questions)
    q[qid]['criteria'] = dict(reversed(list(q[qid]['criteria'].items())))
    yield 'compact_reversed_options', short_state, q, {}
    q = copy.deepcopy(short_questions)
    q[qid]['criteria']['none'] = 'Wait without taking any of the listed operations.'
    yield 'compact_wait_label', short_state, q, {}
    q = copy.deepcopy(original)
    q[qid]['criteria'] = {k: json.loads(v) if v.startswith('{') else v
                          for k, v in q[qid]['criteria'].items()}
    yield 'structured_criteria_only', state, q, {}


def controls():
    # Explicitly contradictory states check responsiveness, not Pokemon knowledge.
    definitions = [
        ('route_open', 'Goal: reach the garden. Location: bedroom. The door to the garden is open.',
         'Go through the door to the garden.', 'move'),
        ('route_locked', 'Goal: reach the garden. Location: bedroom. The only door is locked. No key or alternative route is available.',
         'Go through the door to the garden.', 'none'),
        ('healing_needed', 'Goal: restore HP. Party HP: 1/20. A nurse is here and offers free healing.',
         'Talk to the nurse to heal the party.', 'move'),
        ('healing_unavailable', 'Goal: restore HP. Party HP: 1/20. No nurse or healing items are present.',
         'Talk to the nurse to heal the party.', 'none'),
    ]
    for name, state, move, expected in definitions:
        for reverse in (False, True):
            criteria = {'move': move, 'none': 'None of these candidates can advance the current goal.'}
            if reverse:
                criteria = dict(reversed(list(criteria.items())))
            yield name + ('_reversed' if reverse else ''), state, {
                'action': {'type': 'choice', 'instructions': 'Which available operation advances the stated goal?',
                           'criteria': criteria}}, expected


def probe(agent, name, state, questions, overrides=None):
    overrides = overrides or {}
    items, internals, audits = encoding(agent, state, questions, **overrides)
    if not overrides:
        native_items, _ = agent.prepare(state, questions)
        assert native_items == items, 'Diagnostic encoder differs from native encoder'
    saved_prepare = agent.prepare
    saved_cfg = dict(agent.cfg)
    try:
        agent.prepare = types.MethodType(lambda self, s, q: (items, internals), agent)
        agent.cfg['max_len'] = overrides.get('max_len', 512)
        started = time.monotonic()
        result = agent.predict(state, questions)
        latency = time.monotonic() - started
    finally:
        agent.prepare = saved_prepare
        agent.cfg = saved_cfg
    return {'name': name, 'state': state, 'questions': questions, 'encoding_overrides': overrides,
            'answers': result['answers'], 'usage': result['usage'], 'latency_s': latency, 'encoding': audits}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence', type=Path, default=Path('docs/laya-jev-evaluation-results/decision-evidence.json'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('Refusing to overwrite an existing diagnostic run')
    import importlib.metadata
    import laya_mlx
    import mlx.core as mx
    evidence = json.loads(args.evidence.read_text())
    records = evidence['laya']['first_requests']
    report = {'checkpoint': LAYA_MODEL, 'revision': LAYA_REVISION,
              'package_version': importlib.metadata.version('laya-mlx'),
              'evidence_sha256': hashlib.sha256(args.evidence.read_bytes()).hexdigest(),
              'scope': 'Offline development probes, not a new formal gameplay comparison.', 'probes': []}
    for mode, config in [
        ('fp16_compiled_cached', {'dtype': 'float16', 'compile': True, 'cache_prompts': True}),
        ('fp16_eager_uncached', {'dtype': 'float16', 'compile': False, 'cache_prompts': False}),
        ('fp32_eager_uncached', {'dtype': 'float32', 'compile': False, 'cache_prompts': False}),
    ]:
        agent = laya_mlx.load(LAYA_MODEL, revision=LAYA_REVISION, pad_to_multiple=16, **config)
        for record in records:
            for name, state, questions, overrides in variants(record):
                if mode != 'fp16_compiled_cached' and name != 'native':
                    continue
                result = probe(agent, name, state, questions, overrides)
                result.update(runtime=mode, request_number=record['number'])
                report['probes'].append(result)
                print(mode, record['number'], name, json.dumps(result['answers']), flush=True)
        if mode == 'fp16_compiled_cached':
            for name, state, questions, expected in controls():
                result = probe(agent, name, state, questions)
                result.update(runtime=mode, expected=expected)
                report['probes'].append(result)
                print('CONTROL', name, json.dumps(result['answers']), flush=True)
        del agent
        mx.clear_cache()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    main()

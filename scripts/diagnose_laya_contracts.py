#!/usr/bin/env python3
"""Follow-up diagnostic controls, declared independently of model predictions."""
import argparse
import copy
import json
from pathlib import Path

from diagnose_laya import compact, probe
from openpokered.evaluation_models import LAYA_MODEL, LAYA_REVISION


def fixtures():
    # Facts and expected labels are explicit; these are development controls,
    # not a held-out accuracy benchmark or a substitute for a gameplay run.
    cases = [
        ('door_open', 'The door to the garden is open.', 'Walk through the door to the garden.', True),
        ('door_locked', 'The only door to the garden is locked. No key or other route is available.', 'Walk through the door to the garden.', False),
        ('nurse_present', 'The party has 1/20 HP. A nurse is present and offers free healing.', 'Talk to the nurse to heal the party.', True),
        ('nurse_absent', 'The party has 1/20 HP. No nurse or healing items are present.', 'Talk to the nurse to heal the party.', False),
        ('money_enough', 'The player has 500 coins. The shop is open and sells a potion for 300 coins.', 'Buy one potion for 300 coins.', True),
        ('money_short', 'The player has 100 coins. The shop is open and sells a potion for 300 coins. Credit is unavailable.', 'Buy one potion for 300 coins.', False),
        ('route_found', 'Location: RedsHouse2F. A walking route to PalletTown is available via RedsHouse1F.', 'Travel to PalletTown via RedsHouse1F.', True),
        ('route_blocked', 'Location: RedsHouse2F. Every exit is blocked. No walking route to PalletTown is available.', 'Travel to PalletTown via RedsHouse1F.', False),
    ]
    for name, facts, operation, available in cases:
        for schema in ('choice_none', 'choice_available', 'noul'):
            for reversed_options in (False, True):
                if schema == 'noul' and reversed_options:
                    continue
                if schema == 'choice_none':
                    state = facts
                    question = {'type': 'choice', 'instructions': 'Which listed operation can be executed in the current state?',
                                'criteria': {'move': operation, 'none': 'None of the listed operations can be executed now.'}}
                    expected = 'move' if available else 'none'
                else:
                    state = facts + ' Proposed operation: ' + operation
                    if schema == 'choice_available':
                        question = {'type': 'choice', 'instructions': 'Is the proposed operation executable in the current state?',
                                    'criteria': {'yes': 'The operation is executable now.', 'no': 'The operation is not executable now.'}}
                    else:
                        question = {'type': 'noul', 'instructions': 'The proposed operation is executable in the current state.'}
                    expected = 'yes' if available else 'no'
                if reversed_options:
                    question['criteria'] = dict(reversed(list(question['criteria'].items())))
                yield name, schema + ('_reversed' if reversed_options else ''), state, {'check': question}, expected


def goal_probes():
    facts = ('Location: RedsHouse2F. Party: empty. There is a walking route to PalletTown '
             'via RedsHouse1F. At the north exit of PalletTown, an interaction makes Prof. Oak appear. '
             'Making Prof. Oak appear advances choosing a starter Pokemon.')
    question = {'action': {'type': 'choice',
                          'instructions': 'Which next operation advances the goal? Intermediate steps are allowed.',
                          'criteria': {'move': 'Travel to PalletTown via RedsHouse1F.',
                                       'none': 'None of these candidates can advance the current goal.'}}}
    for name, goal in [('local', 'Reach PalletTown.'), ('oak', 'Make Prof. Oak appear.'), ('starter', 'Choose a starter Pokemon.')]:
        for order in ('original', 'reversed'):
            q = copy.deepcopy(question)
            if order == 'reversed':
                q['action']['criteria'] = dict(reversed(list(q['action']['criteria'].items())))
            yield name, order, facts + ' Goal: ' + goal, q


def representation_probes(record):
    """Separate option labels from a short, explicit state-to-effect relation."""
    compact_state, compact_questions = compact(record)
    contexts = [
        ('native', record['state'], record['questions']),
        ('compact', compact_state, compact_questions),
    ]
    _, _, relational_state, relational_questions = next(goal_probes())
    relational_state = relational_state.replace('Goal: Reach PalletTown.', 'Goal: Make Prof. Oak appear.')
    contexts.append(('relational', relational_state, relational_questions))
    for name, state, questions in contexts:
        for label in ('action:0', 'move', 'travel'):
            q = copy.deepcopy(questions)
            qid = next(iter(q))
            q[qid]['criteria'] = {label if k != 'none' else k: v for k, v in q[qid]['criteria'].items()}
            yield name, label, state, q
    # Same compact text, add only the map connection already present in the
    # original candidate. Its placement in state is the intervention.
    for connection in (
        ' A walking route goes from RedsHouse2F through RedsHouse1F to PalletTown.',
        ' At the north exit of PalletTown, an interaction makes Prof. Oak appear.',
        ' A walking route goes from RedsHouse2F through RedsHouse1F to PalletTown. '
        'At the north exit of PalletTown, an interaction makes Prof. Oak appear.',
    ):
        yield 'compact_relation_in_state', connection, compact_state + connection, compact_questions


def controller_replay(evidence):
    from unittest.mock import Mock
    from openpokered.story_agent import DualStoryAgent, StoryStopped, attempt_key
    from openpokered.autonomous_story import AutonomousStoryAgent
    from openpokered.story_rules import Rule
    from openpokered.typesafe import ChoiceAnswer, SystemOneResult
    records = evidence['laya']['first_requests']
    def answer(record):
        return SystemOneResult('frozen-replay', {k: ChoiceAnswer(**a) for k, a in record['answers'].items()},
                               record['input_tokens'], record['output_tokens'])
    model = Mock()
    model.system_one.side_effect = [answer(r) for r in records]
    client = Mock()
    client.state.return_value = {'frame_count': 0}
    agent = DualStoryAgent(client, model, [{'name': 'Starter', 'satisfied_when': {'flag': 'GOT_STARTER'}}])
    events = []
    agent.record = lambda kind, **payload: events.append({'kind': kind, **payload})
    facts = records[1]['state']['local_state']
    target = records[1]['state']['subgoal']
    rule = Rule('oak', 'PalletTown', 'PalletTown:coordNorthExit', ['coordNorthExit'], [], [], tuple(target), [])
    def choose(record):
        layer, question = next(iter(record['questions'].items()))
        candidates = {k: v for k, v in question['criteria'].items() if k != 'none'}
        return agent.choose(layer, record['state'], candidates, question['instructions'])
    selected = choose(records[0])
    agent.active = {'rules': [rule], 'target': target}
    before = agent.failures[attempt_key(rule, facts)]
    try:
        choose(records[1])
        raise AssertionError('Expected action refusal')
    except StoryStopped as error:
        assert AutonomousStoryAgent.action_rejected(agent, facts, str(error))
    after = agent.failures[attempt_key(rule, facts)]
    try:
        choose(records[2])
        raise AssertionError('Expected strategy refusal')
    except StoryStopped as error:
        stopped = str(error)
    return {'first_controller_selection': selected, 'physical_operations_executed': agent.actions,
            'execution_failure_counter_before': before, 'execution_failure_counter_after': after,
            'rule_excluded_by_strategy_threshold': after >= 2, 'stop_reason': stopped,
            'events': events}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--suite', choices=('contracts', 'representation'), default='contracts')
    args = parser.parse_args()
    if args.output.exists():
        parser.error('Refusing to overwrite a diagnostic run')
    import laya_mlx
    agent = laya_mlx.load(LAYA_MODEL, revision=LAYA_REVISION, dtype='float16', compile=True,
                          pad_to_multiple=16, cache_prompts=True)
    report = {'checkpoint': LAYA_MODEL, 'revision': LAYA_REVISION, 'probes': [],
              'scope': 'Development fixtures; not a held-out benchmark. No gameplay or network inference.'}
    evidence = json.loads(Path('docs/laya-jev-evaluation-results/decision-evidence.json').read_text())
    if args.suite == 'representation':
        for name, label, state, questions in representation_probes(evidence['laya']['first_requests'][1]):
            result = probe(agent, name, state, questions)
            result['intervention'] = label
            report['probes'].append(result)
            print(name, label, json.dumps(result['answers']), flush=True)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
        return
    for name, schema, state, questions, expected in fixtures():
        result = probe(agent, name, state, questions)
        answer = result['answers']['check']
        actual = answer.get('choice', 'yes' if answer.get('noul', 0) > .5 else 'no')
        result.update(schema=schema, expected=expected, actual=actual, correct=actual == expected)
        report['probes'].append(result)
        print(name, schema, expected, json.dumps(answer), flush=True)
    for name, order, state, questions in goal_probes():
        result = probe(agent, name, state, questions)
        result.update(schema='goal_' + order, expected='move')
        report['probes'].append(result)
        print('GOAL', name, order, json.dumps(result['answers']), flush=True)
    report['controller_replay'] = controller_replay(evidence)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    main()

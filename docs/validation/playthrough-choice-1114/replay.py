"""Replay the m39 choice timing or m44 nurse/supply regression using real inputs."""
import argparse
import ast
import hashlib
import inspect
import json
import os
from pathlib import Path
import shutil
import signal
import sys
import tempfile
import traceback

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
ALLOWED = {'get_state', 'get_npcs', 'get_flags', 'get_agent_state', 'get_party',
           'get_bag', 'get_position', 'press', 'press_sequence', 'press_timeline',
           'step_frames', 'wait_until', 'skip_dialogue'}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def interrupted(*_):
    raise KeyboardInterrupt('diagnostic owner interrupted the replay')


def quizzes(game):
    results = []
    questions = [(15, 7, 'YES'), (10, 1, 'NO'), (9, 7, 'NO'),
                 (9, 13, 'NO'), (1, 13, 'YES'), (1, 7, 'NO')]
    for number, (x, y, answer) in enumerate(questions, 1):
        game.nav_to(x, y + 1, 'CinnabarGym')
        game.face('up')
        game.tap('a', 16)
        game.dialogue_then_choice()
        before = game.st()
        game.choose(answer)
        assert game.cutscene()
        flags = game.d.cmd(cmd='get_flags')['data']
        assert flags.get(f'EVENT_CINNABAR_GYM_GATE{number}_UNLOCKED')
        assert not flags.get(f'EVENT_BEAT_CINNABAR_GYM_TRAINER_{number}')
        results.append({'number': number, 'answer': answer,
                        'initial_choice': before['choice'],
                        'gate_unlocked': True, 'trainer_unbeaten': True})
    return {'quizzes': results}


def indigo(game, late):
    node = ast.parse(inspect.getsource(late.m44_indigo)).body[0]
    start = next(i for i, statement in enumerate(node.body)
                 if isinstance(statement, ast.Expr)
                 and isinstance(statement.value, ast.Call)
                 and isinstance(statement.value.func, ast.Attribute)
                 and statement.value.func.attr == 'nav_to'
                 and [getattr(v, 'value', None) for v in statement.value.args[:3]]
                 == [7, 7, 'IndigoPlateauLobby'])
    node.body = node.body[start:]
    node.name = 'm44_from_lobby'
    namespace = dict(vars(late))
    exec(compile(ast.Module(body=[node], type_ignores=[]),
                 str(ROOT / 'scripts/playthrough_late.py'), 'exec'), namespace)
    assert game.st()['map_name'] == 'IndigoPlateauLobby'
    namespace['m44_from_lobby'](game)
    after = game.st()
    assert all(mon['hp'] == mon['max_hp'] for mon in after['party'])
    return {'tail_ast_sha256': hashlib.sha256(
        ast.dump(node, include_attributes=False).encode()).hexdigest(),
        'after': after, 'bag': game.d.cmd(cmd='get_bag')['data']}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--artifacts', type=Path, required=True)
    parser.add_argument('--case', choices=['quiz', 'indigo'], default='quiz')
    args = parser.parse_args()
    output = args.artifacts.resolve()
    output.mkdir(parents=True, exist_ok=False)
    (output / 'tmp').mkdir()
    tempfile.tempdir = str(output / 'tmp')
    for key in list(os.environ):
        if key.startswith('POKERED_'):
            del os.environ[key]
    sys.path.insert(0, str(ROOT / 'scripts'))
    import playthrough as driver
    import playthrough_late as late
    evidence = json.loads((HERE / 'evidence.json').read_text())
    fixture = HERE / ('fixture.json' if args.case == 'quiz' else 'indigo-fixture.json')
    expected = (evidence['fixture_sha256'] if args.case == 'quiz'
                else evidence['indigo']['fixture_sha256'])
    assert digest(fixture) == expected
    binary = args.binary.resolve()
    source_hashes = {name: digest(ROOT / 'scripts' / name)
                     for name in ['playthrough.py', 'playthrough_late.py']}
    result = {'complete': False, 'all_pass': False, 'case': args.case,
              'binary_sha256': digest(binary), 'driver_hashes': source_hashes,
              'scope': 'persistent fixture, ordinary Continue/buttons; not fresh route or ordinary SAVE',
              'commands': {}}
    game = None
    requests = []
    try:
        game = driver.Game(snapshot=fixture, binary=binary, seed=42, speed=0)
        result.update(Game_pid=game.proc.pid, argv=game.proc.args)
        original = game.d.cmd
        def command(*values, **kwargs):
            name = kwargs.get('cmd', values[0] if values else None)
            assert name in ALLOWED, ('unexpected runtime command', name)
            requests.append(kwargs)
            result['commands'][name] = result['commands'].get(name, 0) + 1
            return original(*values, **kwargs)
        game.d.cmd = command
        driver.resume_reentry(game)
        assert game.cutscene()
        result.update(quizzes(game) if args.case == 'quiz' else indigo(game, late))
        assert digest(binary) == result['binary_sha256']
        assert all(digest(ROOT / 'scripts' / name) == value
                   for name, value in source_hashes.items())
        result['all_pass'] = True
    except BaseException:
        result['failure'] = traceback.format_exc()
        print(result['failure'], flush=True)
    finally:
        if game is not None:
            shutil.copy2(game.run_dir / 'game.log', output / 'game.log')
            game.close()
            result['Game_exit'] = game.proc.poll()
        result['complete'] = True
        (output / 'requests.json').write_text(json.dumps(requests, indent=2) + '\n')
        (output / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'all_pass': result['all_pass'], 'result': str(output / 'result.json')}))
    return 0 if result['all_pass'] else 1


if __name__ == '__main__':
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    raise SystemExit(main())

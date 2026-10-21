#!/usr/bin/env python3
"""Normal-boot physical-input check for remembered START-menu cursor readiness."""
import argparse
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import traceback
import types

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
import playthrough as pt
import playthrough_late as late


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--expect', choices=['base', 'fixed'], required=True)
    parser.add_argument('--helper-ref', help='Load the exact historical helper from git')
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir()
    (output / 'tmp').mkdir()
    tempfile.tempdir = str(output / 'tmp')
    spec = importlib.util.spec_from_file_location('wire', ROOT / 'docs/audits/npc-motion-selectors/validation-transport.py')
    wire = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(wire)
    pt.DebugClient = wire.accelerated_client(pt.DebugClient)
    helper = late
    if args.helper_ref:
        source = subprocess.check_output(['git', 'show', args.helper_ref + ':scripts/playthrough_late.py'], cwd=ROOT)
        helper = types.ModuleType('historical_late')
        helper.__file__ = str(ROOT / 'scripts/playthrough_late.py')
        sys.modules[helper.__name__] = helper
        exec(compile(source, 'git:' + args.helper_ref + ':scripts/playthrough_late.py', 'exec'), helper.__dict__)
    fixture = ROOT / 'docs/audits/npc-motion-selectors/fixture.sav'
    raw = fixture.read_bytes()
    summary = []
    for phase in range(4):
        case = output / f'phase-{phase}'
        case.mkdir()
        save = case / 'play.sav'
        save.write_bytes(raw)
        record = {'phase': phase, 'expect': args.expect, 'helper_ref': args.helper_ref,
                  'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                  'fixture_sha256': hashlib.sha256(raw).hexdigest(),
                  'commands': [], 'states': {}, 'completed': False,
                  'scope': 'Offline SRAM, normal boot/CONTINUE, seed42, only physical timeline input plus readonly observations. Exact helper called; no setters/warp/skip/restore. Tool readiness regression, not original animation parity.'}
        game = None
        try:
            for key in list(os.environ):
                if key.startswith('POKERED_'):
                    del os.environ[key]
            game = pt.Game(binary=args.binary.resolve(), save_path=save, seed=42, speed=0)
            original = game.d.cmd

            def command(**kwargs):
                assert kwargs['cmd'] in {'get_state', 'capture_frame', 'press_timeline'}
                response = original(**kwargs)
                record['commands'].append({'command': kwargs, 'response': response})
                return response

            game.d.cmd = command

            def drive(buttons, frames=None):
                buttons = list(buttons)
                if frames is not None:
                    assert frames >= len(buttons)
                    buttons += [None] * (frames - len(buttons))
                response = game.d.cmd(cmd='press_timeline', buttons=buttons, advance=True)
                assert response['ok'], response
                return response

            # Keep Game.tap and the helper unchanged, adapting their transport
            # conveniences to explicit physical timelines only.
            game.d.drive = drive
            game.d.step = lambda count: drive([None] * count)
            for _ in range(30):
                if game.st()['screen'] == 'overworld':
                    break
                drive(['a'] * 3 + [None] * 60)
            drive([None] * (2000 - game.st()['frame_count']))
            # Seed only the remembered cursor through normal menu operation.
            late.open_start(game, 'Pokemon')
            assert game.st()['field_menu']['kind'] == 'party'
            game.tap('b', 60)
            assert game.st()['field_menu']['kind'] == 'start'
            game.tap('b', 60)
            assert game.st()['screen'] == 'overworld'
            frame = game.st()['frame_count']
            assert frame <= 3000 + phase
            drive([None] * (3000 + phase - frame))
            record['states']['before_helper'] = game.st()
            helper.open_start(game, 'Pokemon')
            state = game.st()
            record['states']['after_helper'] = state
            menu = state['field_menu']
            if args.expect == 'fixed':
                assert state['screen'] == 'party' and menu['kind'] == 'party', state
            else:
                assert state['screen'] == 'start-menu' and menu['kind'] == 'start', state
                assert menu['ready_for_input'], state
                game.tap('a', 12)
                assert game.st()['field_menu']['kind'] == 'party'
                record['states']['control_after_ready_a'] = game.st()
            assert save.read_bytes() == raw
            record['completed'] = True
        except BaseException:
            record['failure'] = traceback.format_exc()
            print(record['failure'], flush=True)
        finally:
            if game:
                game.close()
                record['game_exit'] = game.proc.returncode
            (case / 'result.json.gz').write_bytes(gzip.compress(json.dumps(record, indent=2).encode(), mtime=0))
        summary.append({'phase': phase, 'completed': record['completed'],
                        'after_helper': record['states'].get('after_helper', {}).get('field_menu')})
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary), flush=True)
    return 0 if all(case['completed'] for case in summary) else 1


if __name__ == '__main__':
    raise SystemExit(main())

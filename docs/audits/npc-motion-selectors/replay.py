#!/usr/bin/env python3
"""Verify idle facing through ordinary boot/CONTINUE and physical input only."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import traceback

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'scripts'))
import playthrough  # noqa: E402


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--expect', choices=['base', 'fixed'], required=True)
    parser.add_argument('--case', choices=['route10', 'museum'], default='route10')
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir()
    (output / 'tmp').mkdir()
    tempfile.tempdir = str(output / 'tmp')
    museum = args.case == 'museum'
    map_name, x, y = ('Museum2F', 11, 7) if museum else ('Route10', 3, 59)
    fixture = HERE / ('museum-fixture.sav' if museum else 'fixture.sav')
    raw = fixture.read_bytes()
    save = output / 'play.sav'
    save.write_bytes(raw)
    binary = args.binary.resolve()
    record = {
        'case': args.case, 'expect': args.expect,
        'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'map_sha256': hashlib.sha256((ROOT / f'crates/pokered-data/maps/{map_name}/map.json').read_bytes()).hexdigest(),
        'fixture_sha256': hashlib.sha256(raw).hexdigest(),
        'commands': [], 'samples': [], 'completed': False,
        'scope': 'Offline SRAM; ordinary boot/CONTINUE, seed42, physical input and readonly observation. Matched absolute frames. Functional facing test, not original animation parity or fresh travel.',
    }
    game = None
    try:
        for key in list(os.environ):
            if key.startswith('POKERED_'):
                del os.environ[key]
        game = playthrough.Game(binary=binary, save_path=save, seed=42, speed=0)
        original = game.d.cmd

        def command(**kwargs):
            assert kwargs['cmd'] in {'get_state', 'get_npcs', 'capture_frame', 'press_timeline'}
            record['commands'].append(kwargs)
            return original(**kwargs)

        game.d.cmd = command

        def drive(buttons):
            response = game.d.cmd(cmd='press_timeline', buttons=buttons, advance=True)
            assert response['ok'], response

        for _ in range(30):
            if game.st()['screen'] == 'overworld':
                break
            drive(['a'] * 3 + [None] * 60)
        state = game.st()
        assert (state['screen'], state['map_name'], state['player_x'], state['player_y']) == ('overworld', map_name, x, y), state
        record['continued_at'] = state
        for frame in range(2000, 2513, 32):
            state = game.st()
            assert state['frame_count'] <= frame
            drive([None] * (frame - state['frame_count']))
            state = game.st()
            assert state['frame_count'] == frame
            assert (state['screen'], state['map_name'], state['player_x'], state['player_y']) == ('overworld', map_name, x, y), state
            assert not state['script_running'] and not state['dialogue_state'], state
            npcs = game.d.cmd(cmd='get_npcs')
            assert npcs['ok'], npcs
            record['samples'].append({'frame': frame, 'state': state, 'npcs': npcs})
            if not museum:
                for index, position, facing in [(1, (3, 57), 'Up'), (4, (3, 61), 'Down')]:
                    npc = npcs['data'][index]
                    assert npc['visible'] and npc['walk_counter'] == 0
                    assert (npc['x'], npc['y']) == position
                    assert npc['facing'] == facing if args.expect == 'fixed' else npc['facing'] in ('Left', 'Right'), npc
            response = game.d.cmd(cmd='capture_frame', path=str(output / f'frame-{frame}.png'))
            assert response['ok'], response
        if museum:
            facings = {sample['npcs']['data'][3]['facing'] for sample in record['samples']}
            assert len(facings) > 1, facings
        assert save.read_bytes() == raw
        record['completed'] = True
    except BaseException:
        record['failure'] = traceback.format_exc()
        print(record['failure'], flush=True)
    finally:
        if game:
            game.log.flush()
            shutil.copyfile(game.run_dir / 'game.log', output / 'game.log')
            game.close()
            record['game_exit'] = game.proc.returncode
        (output / 'result.json.gz').write_bytes(gzip.compress(json.dumps(record, indent=2).encode(), mtime=0))
    print(json.dumps({'completed': record['completed'], 'case': args.case}), flush=True)
    return 0 if record['completed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())

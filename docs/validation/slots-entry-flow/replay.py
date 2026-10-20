"""Replay slot entry from normal Continue using real button inputs."""
from pathlib import Path
import gzip
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import traceback

ROOT = Path.cwd()
HERE = Path(__file__).resolve().parent
assert (ROOT / 'scripts/playthrough.py').is_file(), 'Run from the workspace root'
sys.path.insert(0, str(ROOT / 'scripts'))
import playthrough as p

role, executable, output = sys.argv[1:]
assert role in ('before', 'after')
binary = Path(executable).resolve()
out = Path(output).resolve()
out.mkdir(parents=True)
for key in list(os.environ):
    if key.startswith('POKERED_'):
        del os.environ[key]
fixture_bytes = gzip.decompress((HERE / 'fixture.json.gz').read_bytes())
base = json.loads(fixture_bytes)
result = {
    'role': role,
    'workspace_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
    'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
    'fixture_sha256': hashlib.sha256(fixture_bytes).hexdigest(),
    'scope': 'Caller-provided executable; workspace head does not prove executable origin. Offline native fixture, normal Continue and buttons only; no original-ROM animation/audio proof.',
    'cases': [], 'all_pass': False,
}
cases = [('missing', False, 99), ('empty', True, 0), ('neither', False, 0),
         ('no', True, 99), ('b', True, 99), ('yes', True, 99),
         ('one', True, 1), ('max', True, 9999)]
if role == 'before':
    cases = [cases[0], cases[1], ('offer', True, 99)]
normalize = lambda text: ' '.join((text or '').split())
try:
    for case, has_case, coins in cases:
        row = {'case': case, 'commands': [], 'states': []}
        result['cases'].append(row)
        with tempfile.TemporaryDirectory(prefix='slot-entry-') as tmp:
            fixture = json.loads(json.dumps(base))
            fixture['game_data']['bag']['items'] = [['CoinCase', 1]] if has_case else []
            fixture['game_data']['player_coins'] = coins
            path = Path(tmp) / 'fixture.json'
            path.write_text(json.dumps(fixture))
            g = p.Game(binary=binary, snapshot=path, seed=42, speed=0)
            original = g.d.cmd
            def command(*args, **kwargs):
                name = kwargs.get('cmd', args[0] if args else None)
                assert name in {'get_state', 'get_bag', 'press_sequence', 'press_timeline',
                                'press', 'step_frames', 'wait_until', 'capture_frame'}, name
                row['commands'].append(kwargs)
                response = original(*args, **kwargs)
                if name == 'get_state':
                    row['states'].append({key: response['data'][key] for key in
                        ['frame_count', 'screen', 'coins', 'dialogue', 'dialogue_state', 'choice',
                         'player_x', 'player_y', 'script_running', 'active_script_effect']})
                return response
            g.d.cmd = command
            try:
                p.resume_reentry(g)
                g.step(60)
                before_bag = g.d.cmd(cmd='get_bag')['data']
                g.face('up')
                g.tap('a', 120)
                state = g.st()
                row['comparison'] = state
                assert state['coins'] == coins and state['frame_count'] == 803
                g.d.cmd(cmd='capture_frame', path=str(out / f'{case}-entry.png'))
                if role == 'before':
                    assert state['screen'] == 'slots'
                else:
                    assert state['screen'] == 'overworld'
                    if not has_case or not coins:
                        assert state['dialogue_state']['waiting_for_input']
                        expected = 'A COIN CASE is required!' if not has_case else "You don't have any coins!"
                        assert normalize(state['dialogue']) == expected
                        assert g.st()['choice'] is None
                        g.tap('right', 16)
                        assert g.pos() == ('GameCorner', 18, 16) and g.st()['coins'] == coins
                        g.tap('a', 120)
                        g.wait('control_ready', 600)
                    else:
                        # DONE returns straight to YesNoChoice: no A/B to ACK the question.
                        g.wait('choice_open', 600)
                        choice = g._ready_choice()
                        assert choice['options'] == ['YES', 'NO']
                        assert normalize(g.st()['dialogue']) == 'A slot machine! Want to play?'
                        g.tap('right', 16)
                        assert g.pos() == ('GameCorner', 18, 16) and g.st()['coins'] == coins
                        row['offer_choice'] = choice
                        if case == 'b':
                            g.tap('b', 120)
                        elif case == 'yes':
                            # A held across the screen handoff must not also place a bet.
                            assert g._ready_choice()['selected'] == 0
                            g.d.cmd(cmd='press_timeline', buttons=['a'] * 30, advance=True)
                            assert g.st()['screen'] == 'slots' and g.st()['coins'] == coins
                            g.step(1)
                        else:
                            g.choose('NO' if case == 'no' else 'YES')
                        if case in ('no', 'b'):
                            g.wait('control_ready', 600)
                        else:
                            # YesNoChoice restores its parent after fifteen VBlanks.
                            # Observe the real handoff instead of assuming immediate completion.
                            g.wait('screen=slots', 600)
                            assert g.st()['screen'] == 'slots' and g.st()['coins'] == coins
                            if coins >= 3:
                                g.tap('a', 2)
                                assert g.st()['coins'] == coins - 3
                            else:
                                g.tap('b', 120)
                                g.wait('control_ready', 600)
                                assert g.st()['coins'] == 1
                    if case in ('missing', 'empty', 'neither', 'no', 'b'):
                        assert g.st()['screen'] == 'overworld' and g.st()['coins'] == coins
                assert g.d.cmd(cmd='get_bag')['data'] == before_bag
                if role == 'after' and g.st()['screen'] == 'overworld':
                    # Cancellation/error return must restore actual movement, not only a screen label.
                    g.d.drive(['left'] * 32, frames=32)
                    moved = g.st()
                    assert moved['map_name'] == 'GameCorner' and moved['player_x'] < 18 and moved['player_y'] == 16
                    row['control_position'] = [moved['player_x'], moved['player_y']]
                row.update(passed=True, final_screen=g.st()['screen'], final_coins=g.st()['coins'])
            finally:
                g.log.flush()
                shutil.copy2(g.run_dir / 'game.log', out / f'{case}.log')
                g.close()
                row['exit'] = g.proc.returncode
    result['all_pass'] = True
except BaseException:
    result['failure'] = traceback.format_exc()
    print(result['failure'], flush=True)
finally:
    (out / 'verification.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'all_pass': result['all_pass'], 'cases': len(result['cases'])}), flush=True)
raise SystemExit(0 if result['all_pass'] else 1)

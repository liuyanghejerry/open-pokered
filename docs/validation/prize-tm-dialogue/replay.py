"""Normal Continue and button-only TM counter checks from offline fixtures."""
from pathlib import Path
import hashlib
import json
import shutil
import sys
import tempfile
import traceback

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'scripts'))
import playthrough as p

role, executable, output = sys.argv[1:]
binary = Path(executable).resolve()
out = Path(output).resolve()
out.mkdir(parents=True)
results = {'role': role, 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
           'scope': 'Offline persistent fixtures, normal boot/Continue, real buttons; not fresh travel or original ROM animation proof.',
           'all_pass': False, 'cases': []}
base = json.loads((HERE / 'fixture.json').read_text())
cases = ['entry', 'cancel', 'decline', 'poor', 'full', 'tm23', 'tm15', 'tm50', 'no-case'] if role == 'after' else ['entry']
try:
    for case in cases:
        with tempfile.TemporaryDirectory(prefix='prize-tm-') as tmp:
            fixture = json.loads(json.dumps(base))
            data = fixture['game_data']
            if case == 'poor':
                data['player_coins'] = 3299
            if case == 'no-case':
                data['bag']['items'] = []
            if case == 'full':
                original = json.loads((ROOT / 'docs/validation/tower-rocket-exits/fixture.json').read_text())
                data['bag']['items'] += original['game_data']['bag']['items']
                assert len(data['bag']['items']) == 20
            path = Path(tmp) / 'fixture.json'
            path.write_text(json.dumps(fixture))
            record = {'case': case, 'commands': [], 'observations': []}
            results['cases'].append(record)
            g = p.Game(binary=binary, snapshot=path, seed=42, speed=0)
            original_cmd = g.d.cmd
            allowed = {'get_state', 'get_npcs', 'get_flags', 'get_party', 'get_bag', 'get_agent_state',
                       'press_sequence', 'press_timeline', 'press', 'step_frames', 'wait_until', 'capture_frame'}
            def command(*args, **kwargs):
                name = kwargs.get('cmd', args[0] if args else None)
                assert name in allowed, name
                record['commands'].append(kwargs)
                response = original_cmd(*args, **kwargs)
                if name == 'get_state':
                    s = response['data']
                    record['observations'].append({k: s[k] for k in
                        ['frame_count', 'screen', 'dialogue', 'dialogue_state', 'choice', 'coins', 'script_running']})
                return response
            g.d.cmd = command
            def choice():
                for _ in range(20):
                    s = g.st()
                    if s['choice'] is not None:
                        return g._ready_choice()
                    if s['dialogue_state'] and s['dialogue_state']['waiting_for_input']:
                        g.tap('a', 120)
                    else:
                        g.step(120)
                raise AssertionError('no choice')
            def settle():
                for _ in range(20):
                    s = g.st()
                    if not s['script_running'] and s['dialogue_state'] is None and s['choice'] is None:
                        return s
                    assert s['choice'] is None, s['choice']
                    if s['dialogue_state'] and s['dialogue_state']['waiting_for_input']:
                        g.tap('a', 120)
                    else:
                        g.step(120)
                raise AssertionError('script did not finish')
            try:
                p.resume_reentry(g)
                g.step(60)
                assert g.pos() == ('GameCornerPrizeRoom', 6, 3)
                initial = g.d.cmd(cmd='get_bag')['data']
                g.face('up')
                g.tap('a', 120)
                if case == 'no-case':
                    assert g.st()['dialogue'] == 'A COIN CASE is required!'
                    settle()
                else:
                    assert g.st()['dialogue'] == 'We exchange your coins for prizes.'
                    g.tap('a', 120)
                    record['comparison'] = g.st()
                    if case == 'entry':
                        g.d.cmd(cmd='capture_frame', path=str(out / 'comparison.png'))
                    expected = 'Which prize do you want?' if role == 'after' else 'TM prizes: TM23 3300 TM15 5500 TM50 7700'
                    assert g.st()['dialogue'] == expected
                    if case != 'entry':
                        options = choice()['options']
                        index = {'cancel': 3, 'tm15': 1, 'tm50': 2}.get(case, 0)
                        g.choose(options[index])
                        if case != 'cancel':
                            yes_no = choice()['options']
                            g.choose(yes_no[1 if case == 'decline' else 0])
                        settle()
                final_bag = g.d.cmd(cmd='get_bag')['data']
                expected_cost = {'tm23': 3300, 'tm15': 5500, 'tm50': 7700}.get(case, 0)
                assert g.st()['coins'] == data['player_coins'] - expected_cost
                if expected_cost:
                    assert final_bag != initial
                    # Exact delivered item/quantity is checked below in the real bag payload.
                    expected_item = {'tm23': 'Tm23', 'tm15': 'Tm15', 'tm50': 'Tm50'}[case]
                    assert any(item['item'] == expected_item and item['qty'] == 1
                               for item in final_bag), final_bag
                else:
                    assert final_bag == initial
                seen = [s['dialogue'] for s in record['observations'] if s['dialogue']]
                if case == 'poor':
                    assert 'Sorry, you need more coins.' in seen
                if case == 'full':
                    assert "Oops! You don't have enough room." in seen
                if case == 'decline':
                    assert 'Oh, fine then.' in seen
                record.update(passed=True, final_bag=final_bag, final_coins=g.st()['coins'])
            finally:
                g.log.flush()
                shutil.copy2(g.run_dir / 'game.log', out / f'{case}.log')
                g.close()
                record['process_exit'] = g.proc.returncode
    results['all_pass'] = True
except BaseException:
    results['failure'] = traceback.format_exc()
    print(results['failure'])
finally:
    (out / 'verification.json').write_text(json.dumps(results, indent=2) + '\n')
print(json.dumps({'all_pass': results['all_pass'], 'cases': len(results['cases'])}))
raise SystemExit(0 if results['all_pass'] else 1)

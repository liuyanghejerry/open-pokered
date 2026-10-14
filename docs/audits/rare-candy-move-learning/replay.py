"""Replay constructed Rare Candy cases through actual menus and ordinary SAVE.

Fixtures are prepared before boot. The input window contains no state edits.
Each scenario repeats twice with a private copied executable and clean sidecars.
"""

from pathlib import Path
import sys, os, socket, subprocess, json, hashlib, tempfile, shutil
import argparse
ap = argparse.ArgumentParser(description='Replay real Rare Candy menus and ordinary SAVE, twice per scenario')
ap.add_argument('--binary', required=True, type=Path)
ap.add_argument('--kind', required=True, choices=['before', 'after'])
ap.add_argument('--output', required=True, type=Path)
args = ap.parse_args()
root = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(root / 'scripts'))
from debug_drive import DebugClient
label = args.kind
original_binary = args.binary.resolve()
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
template = json.loads(Path(__file__).with_name('fixture.json').read_text())
template['party'][0]['hp'] = 35
env = os.environ.copy()
env['POKERED_MAPS_DIR'] = str(root / 'crates/pokered-data/maps')
records = []
for flow in ['free-slot', 'replace-growl', 'cancel-learning', 'hm-refusal']:
    initial = json.loads(json.dumps(template))
    initial['party'][0]['moves'] = ['Tackle', 'Growl', 'None', 'None'] if flow == 'free-slot' else ['Tackle', 'Growl', 'LeechSeed', 'Cut']
    initial['party'][0]['pp'] = [35, 40, 0, 0] if flow == 'free-slot' else [35, 40, 10, 30]
    fixture = out / (flow + '-fixture.json')
    if fixture.exists():
        assert json.loads(fixture.read_text()) == initial
    else:
        fixture.write_text(json.dumps(initial, indent=2))
    for repeat in [1, 2]:
        trial = out / f'{flow}-{label}-{repeat}'
        trial.mkdir()
        binary = trial / 'pokered-app'
        shutil.copyfile(original_binary, binary)
        binary.chmod(493)
        assert not (trial / 'pokered.script_flags.json').exists() and (not (trial / 'isolated.script_flags.json').exists())
        actions = []
        states = []
        captures = {}
        sock = socket.socket()
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        sock.close()
        with (trial / 'game.log').open('w') as log:
            p = subprocess.Popen([str(binary), 'run', '--headless', '--speed', '0', '--seed', '42', '--debug-port', str(port), '--skip-intro', '--snapshot', str(fixture), '--lang', 'en', '--no-audio', '--save', str(trial / 'isolated.sav')], cwd=trial, env=env, stdout=log, stderr=subprocess.STDOUT)
            try:
                d = DebugClient(port)
                d.step(120)

                def state(tag):
                    value = d.state()
                    states.append(dict(tag=tag, state=value))
                    return value

                def tap(button, frames=32):
                    st = state('pre-' + button)
                    held = 4 if button == 'start' else 1
                    actions.append(dict(frame=st['frame_count'], button=button, held=held, frames=frames))
                    d.press_timeline([button] * held + [None] * (frames - held))
                    d.step(frames)

                def capture(tag):
                    st = state(tag)
                    path = trial / (tag + '.png')
                    assert d.cmd(cmd='capture_frame', path=str(path))['ok']
                    captures[tag] = dict(frame=st['frame_count'], sha256=hashlib.sha256(path.read_bytes()).hexdigest(), path=str(path))
                    return st

                def phase():
                    return (state('phase')['field_menu'] or {}).get('rare_candy_phase')
                flags_before = d.cmd(cmd='get_flags')
                tap('start', 60)
                for button in ['down', 'down', 'a', 'a', 'a', 'a']:
                    tap(button)
                ready = state('effect-ready')
                assert ready['screen'] == 'party' and ready['field_menu']['phase'].startswith('ItemUseNotice')
                d.step(240)
                first = capture('level-and-stats')
                assert first['frame_count'] == 640
                tap('a')
                st = capture('first-learning')
                assert st['frame_count'] == 676
                prefix_actions = json.loads(json.dumps(actions))
                if label == 'after':
                    if flow == 'free-slot':
                        assert st['field_menu']['rare_candy_phase'] == 'Learned(VineWhip)'
                        d.step(240)
                        tap('a')
                    else:
                        assert st['field_menu']['rare_candy_phase'] == 'TryingToLearn(0)'
                        tap('a')
                        tap('a')
                        st = capture('learn-choice')
                        assert st['field_menu']['rare_candy_phase'] == 'ReplaceConfirm { yes: true }'
                        if flow == 'cancel-learning':
                            tap('down')
                            tap('a')
                            st = capture('abandon-choice')
                            assert st['field_menu']['rare_candy_phase'] == 'AbandonConfirm { yes: true }'
                            tap('a')
                            st = capture('did-not-learn')
                            assert st['field_menu']['rare_candy_phase'] == 'DidNotLearn'
                            tap('a')
                        else:
                            tap('a')
                            st = capture('forget-menu')
                            assert st['field_menu']['rare_candy_phase'] == 'ChooseMove'
                            if flow == 'hm-refusal':
                                for _ in range(3):
                                    tap('down')
                                tap('a')
                                st = capture('hm-refusal')
                                assert st['field_menu']['phase'] == 'MoveChoiceNotice' and st['party'][0]['moves'] == initial['party'][0]['moves']
                                tap('a')
                                st = capture('hm-retry')
                                assert st['field_menu']['phase'] == 'ChooseMove { cursor: 0 }'
                            tap('down')
                            tap('a')
                            st = capture('forgot')
                            assert st['field_menu']['rare_candy_phase'] == 'Forgot { slot: 1 }'
                            assert st['party'][0]['moves'] == initial['party'][0]['moves']
                            tap('a')
                            st = capture('learned')
                            assert st['field_menu']['rare_candy_phase'] == 'Learned(VineWhip)'
                            d.step(240)
                            tap('a')
                else:
                    assert st['screen'] == 'bag' and (st['field_menu'] or {}).get('rare_candy_phase') is None
                st = capture('bag-after-use')
                assert st['screen'] == 'bag'
                for button in ['b', 'down', 'down', 'a']:
                    tap(button)
                assert state('save-open')['screen'] == 'save'
                d.step(200)
                tap('a', 400)
                d.step(300)
                capture('saved')
                assert (trial / 'isolated.sav').exists()
                flags_after = d.cmd(cmd='get_flags')
                d.close()
            finally:
                (trial / 'states.json').write_text(json.dumps(states, indent=2))
                (trial / 'actions.json').write_text(json.dumps(actions, indent=2))
                p.terminate()
                p.wait(timeout=10)
        cp = subprocess.run([str(binary), 'import-snapshot', '--input', str(trial / 'isolated.sav'), '-o', str(trial / 'readback.json')], env=env, cwd=trial, capture_output=True)
        assert cp.returncode == 0, cp.stderr
        rb = json.loads((trial / 'readback.json').read_text())
        m = rb['party'][0]
        expected_moves = ['Tackle', 'Growl', 'VineWhip', 'None'] if flow == 'free-slot' else ['Tackle', 'VineWhip', 'LeechSeed', 'Cut'] if label == 'after' and flow in ['replace-growl', 'hm-refusal'] else ['Tackle', 'Growl', 'LeechSeed', 'Cut']
        expected_pp = [35, 40, 10, 0] if flow == 'free-slot' else [35, 10, 10, 30] if label == 'after' and flow in ['replace-growl', 'hm-refusal'] else [35, 40, 10, 30]
        assert (m['level'], m['hp'], m['max_hp'], m['attack'], m['defense'], m['speed'], m['special'], m['moves'], m['pp'], m['pp_ups']) == (13, 37, 37, 20, 20, 18, 23, expected_moves, expected_pp, [0, 0, 0, 0]), (flow, label, m)
        assert rb['game_data']['bag']['items'][0][1] == 1 and rb['game_data']['player_money'] == 3000
        for key, value in initial['party'][1].items():
            assert rb['party'][1][key] == value
        ram = (trial / 'isolated.sav').read_bytes()
        checksum = ~sum(ram[9624:13603]) & 255
        assert checksum == ram[13603]
        record = dict(flow=flow, label=label, repeat=repeat, binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(), initial_party=initial['party'], after_party=rb['party'], after_items=rb['game_data']['bag']['items'], money=rb['game_data']['player_money'], actual_flags_before=flags_before, actual_flags_after=flags_after, sidecars_initially_absent=True, private_binary=True, captures=captures, prefix_actions=prefix_actions, actions=actions, normal_save=True, sram_sha256=hashlib.sha256(ram).hexdigest(), sram_checksum=checksum, first_party_raw_hex=ram[12084:12128].hex(), healthy_second_raw_hex=ram[12128:12172].hex())
        (trial / 'record.json').write_text(json.dumps(record, indent=2))
        records.append(record)
        print(flow, label, repeat, 'complete', 'moves', m['moves'], 'checksum', checksum, 'frames', {k: v['frame'] for k, v in captures.items()}, flush=True)
for flow in ['free-slot', 'replace-growl', 'cancel-learning', 'hm-refusal']:
    a, b = [v for v in records if v['flow'] == flow]
    assert a['actions'] == b['actions'] and a['after_party'] == b['after_party'] and (a['captures'] == {k: dict(v, path=str(out / f'{flow}-{label}-1' / (k + '.png'))) for k, v in b['captures'].items()})
(out / (label + '-verification.json')).write_text(json.dumps(dict(scope='Native actual item use and blocking learning/replace/abandon/HM flow through ordinary SAVE/YES. Same initial fixtures and fixed prefix inputs with paired level640/first-ack676 images; subsequent inputs necessarily diverge because master lacks the learning continuation. Each build/case repeats twice, private frozen executable, sidecars absent before boot, live flags recorded. No input-window state edits or debug SAVE.', script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), trials=records), indent=2))
print('output', out, flush=True)

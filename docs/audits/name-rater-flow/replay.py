from pathlib import Path
import os, sys, json, tempfile, shutil, hashlib, gzip, traceback, subprocess, argparse
R = Path(__file__).resolve().parents[3]
E = Path(__file__).resolve().parent
W = R
sys.path.insert(0, str(W / 'scripts'))
import playthrough as p
parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--case', choices=['empty', 'rename', 'party-cancel'], required=True)
parser.add_argument('--version', choices=['base', 'fixed'], default='fixed')
parser.add_argument('--binary', type=Path, default=R / 'target/debug/pokered-app')
parser.add_argument('--fixture', type=Path, default=E / 'fixture.sav')
parser.add_argument('--original-name', default='SPROUT')
args = parser.parse_args()
out = args.output.resolve()
case = args.case
version = args.version
assert case in ['empty', 'rename', 'party-cancel']
assert version in ['base', 'fixed']
out.mkdir()
(out / 'tmp').mkdir()
tempfile.tempdir = str(out / 'tmp')
raw = args.fixture.resolve().read_bytes()
save = out / 'play.sav'
save.write_bytes(raw)
binary = args.binary.resolve()
g = None
rec = {'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=W, text=True).strip(), 'scene_sha256': hashlib.sha256((W / 'crates/pokered-data/maps/NameRatersHouse/script.scene').read_bytes()).hexdigest(), 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'fixture_sha256': hashlib.sha256(raw).hexdigest(), 'case': case, 'version': version, 'commands': [], 'pages': [], 'snapshots': {}, 'anchors': [], 'completed': False, 'scope': 'Native ordinaryboot/CONTINUE/physical menu+naming+SAVE from offlineSRAM. Explicit neutral setup waits align physical triggers at2000/5000/5200; screenshots share exact absoluteframes. No runtime setters/skip/warp/restore. Not originalemulator/freshtravel or an animationPASS.'}
seen = set()
try:
    for k in list(os.environ):
        if k.startswith('POKERED_'):
            del os.environ[k]
    g = p.Game(binary=binary, save_path=save, seed=42, speed=0)
    original = g.d.cmd

    def cmd(**kw):
        assert kw['cmd'] in {'get_state', 'get_party', 'get_npcs', 'get_flags', 'capture_frame', 'press_timeline'}
        rec['commands'].append(kw)
        return original(**kw)
    g.d.cmd = cmd

    def drive(bs):
        assert g.d.cmd(cmd='press_timeline', buttons=bs, advance=True)['ok']

    def tap(k='a', gap=32):
        drive([k] * 3 + [None] * gap)

    def anchor(frame):
        start = g.st()['frame_count']
        assert start <= frame, (case, frame, start)
        drive([None] * (frame - start))
        assert g.st()['frame_count'] == frame
        rec['anchors'].append({'from': start, 'to': frame, 'input': 'neutral'})

    def observe(s):
        d = s['dialogue_state']
        if d and d['waiting_for_input']:
            key = (d['text'], d['page'], d['total_pages'])
            if key not in seen:
                seen.add(key)
                rec['pages'].append({'state': s})
                print('TEXT', d['text'], flush=True)

    def capture(label):
        s = g.st()
        observe(s)
        rec['snapshots'][label] = s
        g.d.cmd(cmd='capture_frame', path=str(out / (label + '.png')))
        return s

    def wait(pred, label):
        for _ in range(300):
            s = g.st()
            observe(s)
            if pred(s):
                return capture(label)
            d = s['dialogue_state']
            if d and d['waiting_for_input']:
                tap()
            else:
                drive([None] * 16)
        raise AssertionError((label, g.st()))

    def choice(s):
        return s['choice'] is not None and (not s['dialogue_state'] or s['dialogue_state']['waiting_for_input'])
    for _ in range(30):
        if g.st()['screen'] == 'overworld':
            break
        tap(gap=60)
    drive([None] * 120 + ['up'] * 4 + [None] * 60)
    s = g.st()
    assert (s['map_name'], s['player_x'], s['player_y'], s['player_facing']) == ('NameRatersHouse', 5, 4, 'Up'), s
    rec['pretrigger'] = s
    rec['npcs_before'] = g.d.cmd(cmd='get_npcs')
    tap()
    s = wait(choice, 'rate-choice')
    assert s['choice']['selected'] == 0
    drive([None] * 60)
    tap()
    wait(lambda s: s['active_script_effect'] == 'ChoosePartyPokemon' and s['script_effect']['started'], 'party')
    drive([None] * 60)
    capture('party-ready')
    anchor(2000)
    if case == 'party-cancel':
        tap('b', 180)
        s = capture('after-party-cancel')
        assert s['frame_count'] == 2183
        if version == 'base':
            assert not s['dialogue_state'] and (not s['script_running']) and (not s['active_script_effect'])
        else:
            assert s['dialogue_state'] and s['dialogue_state']['text'].startswith('Fine!'), s
    else:
        tap(gap=180)
        s = capture('offer-first')
        assert s['frame_count'] == 2183 and s['dialogue_state']
        if version == 'base':
            assert s['dialogue_state']['text'].startswith('That is a decent nickname!')
        else:
            assert (args.original_name + ', is it?') in s['dialogue_state']['text'], s
        s = wait(choice, 'rename-offer')
        assert s['choice']['selected'] == 0
        drive([None] * 60)
        tap()
        wait(lambda s: s['active_script_effect'] == 'NamingScreen' and s['script_effect']['started'], 'naming')
        drive([None] * 60)
        anchor(5000)
        capture('naming-ready')
        if case == 'rename':
            tap('a', 60)
            capture('naming-entered')
            anchor(5200)
        tap('start', 180)
        s = capture('after-naming')
        assert s['frame_count'] == (5383 if case == 'rename' else 5183)
        text = s['dialogue_state']['text']
        rec['result_text'] = text
        if version == 'base':
            assert 'has been renamed!' in text, text
        elif case == 'empty':
            assert text.startswith('Fine!'), text
        else:
            assert 'renamed A!' in text, text
    wait(lambda s: s['screen'] == 'overworld' and (not s['dialogue_state']) and (not s['script_running']) and (not s['active_script_effect']), 'final')
    assert save.read_bytes() == raw
    tap('start', 60)
    s = g.st()
    assert s['screen'] == 'start-menu'
    target = s['field_menu']['items'].index('Save')
    for _ in range(12):
        if g.st()['field_menu']['cursor'] == target:
            break
        tap('down')
    assert g.st()['field_menu']['cursor'] == target
    tap(gap=80)
    for _ in range(20):
        s = g.st()
        if s['screen'] in ['overworld', 'start-menu'] and (not s['dialogue_state']) and (save.read_bytes() != raw):
            break
        tap(gap=120)
    else:
        raise AssertionError(('ordinary SAVE did not complete', g.st()))
    if g.st()['screen'] == 'start-menu':
        tap('b', 60)
    assert g.st()['screen'] == 'overworld' and (not g.st()['dialogue_state'])
    saved = save.read_bytes()
    assert len(saved) == 32768
    expected = bytes([128] + [80] * 10) if case == 'rename' else raw[12414:12414 + 11]
    assert saved[12414:12414 + 11] == expected
    assert ~sum(saved[9624:13603]) & 255 == saved[13603]
    (out / 'after-save.sav').write_bytes(saved)
    rec['nickname_after_actual_save'] = list(expected)
    rec['saved_sha256'] = hashlib.sha256(saved).hexdigest()
    rec['completed'] = True
except BaseException:
    rec['failure'] = traceback.format_exc()
    print(rec['failure'], flush=True)
finally:
    if g:
        g.log.flush()
        shutil.copyfile(g.run_dir / 'game.log', out / 'game.log')
        g.close()
        rec['game_exit'] = g.proc.returncode
    (out / 'result.json.gz').write_bytes(gzip.compress(json.dumps(rec, indent=2).encode(), mtime=0))
print(json.dumps({'completed': rec['completed'], 'case': case, 'version': version, 'result_text': rec.get('result_text')}), flush=True)
raise SystemExit(0 if rec['completed'] else 1)

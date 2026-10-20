"""Run from workspace root; ordinary Continue and buttons, no state setters."""
from pathlib import Path
import gzip
import hashlib
import json
import shutil
import sys
import tempfile
import traceback
from PIL import Image

ROOT = Path.cwd()
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'scripts'))
import playthrough as p

role, binary_arg, output_arg = sys.argv[1:]
assert role in ('before', 'after')
binary = Path(binary_arg).resolve()
out = Path(output_arg).resolve()
out.mkdir(parents=True)
result = {'role': role, 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
          'scope': 'Offline native fixture; normal boot/Continue and buttons. Original assembly inspected; no original-ROM runtime or animation claim.',
          'cases': [], 'all_pass': False}
fixture_bytes = gzip.decompress((HERE / 'fixture.json.gz').read_bytes())
result['fixture_sha256'] = hashlib.sha256(fixture_bytes).hexdigest()
expected = {'before': {'entry': 1, 'horizontal': 3, 'repeat': 2},
            'after': {'entry': 3, 'horizontal': 2, 'repeat': 3}}[role]
try:
    for case in expected:
        record = {'case': case, 'commands': [], 'observations': []}
        result['cases'].append(record)
        with tempfile.TemporaryDirectory(prefix='slots-bet-') as tmp:
            fixture = Path(tmp) / 'fixture.json'
            fixture.write_bytes(fixture_bytes)
            g = p.Game(binary=binary, snapshot=fixture, seed=42, speed=0)
            original = g.d.cmd
            allowed = {'get_state', 'press_sequence', 'press_timeline', 'press',
                       'step_frames', 'wait_until', 'capture_frame'}
            def command(*args, **kwargs):
                name = kwargs.get('cmd', args[0] if args else None)
                assert name in allowed, name
                record['commands'].append(kwargs)
                response = original(*args, **kwargs)
                if name == 'get_state':
                    s = response['data']
                    record['observations'].append({k: s[k] for k in
                        ['frame_count', 'screen', 'map_name', 'player_x', 'player_y', 'coins']})
                return response
            g.d.cmd = command
            def capture(name):
                path = out / f'{case}-{name}.png'
                assert g.d.cmd(cmd='capture_frame', path=str(path))['ok']
                return path
            def cursor(path):
                with Image.open(path) as image:
                    image = image.convert('RGB')
                    ink = {bet: sum(pixel != (255, 255, 255) for pixel in
                        image.crop((120, y, 127, y + 10)).getdata())
                        for bet, y in [(3, 88), (2, 104), (1, 120)]}
                # Only the five-pixel Fusion Pixel '>' is in these three strips.
                active = [bet for bet, pixels in ink.items() if pixels == 5]
                assert len(active) == 1 and sum(ink.values()) == 5, ink
                return active[0]
            try:
                p.resume_reentry(g)
                g.step(60)
                assert g.pos() == ('GameCorner', 18, 16)
                g.face('up')
                g.tap('a', 120)
                assert g.st()['screen'] == 'slots'
                assert g.st()['coins'] == 99
                if case in ('horizontal', 'repeat'):
                    # Regardless of initial row, reach x1, then x2.
                    for key in ['down', 'down', 'up']:
                        g.tap(key, 16)
                    assert cursor(capture('selected-two')) == 2
                if case == 'horizontal':
                    g.tap('right', 16)
                if case == 'repeat':
                    g.tap('a', 80)
                    for _ in range(3):
                        g.tap('a', 40)
                    g.step(2000)
                    capture('one-more-go')
                    assert g.st()['coins'] == 97
                    g.tap('a', 120)
                image = capture('comparison')
                record['comparison_state'] = g.st()
                record['cursor'] = cursor(image)
                assert record['cursor'] == expected[case]
                if case != 'repeat':
                    g.tap('a', 2)
                    assert g.st()['coins'] == 99 - expected[case]
                else:
                    assert g.st()['coins'] == 97
                record.update(passed=True, final_coins=g.st()['coins'])
            finally:
                g.log.flush()
                shutil.copy2(g.run_dir / 'game.log', out / f'{case}.log')
                g.close()
                record['process_exit'] = g.proc.returncode
    result['all_pass'] = True
except BaseException:
    result['failure'] = traceback.format_exc()
    print(result['failure'])
finally:
    (out / 'verification.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'all_pass': result['all_pass'], 'cases': len(result['cases'])}))
raise SystemExit(0 if result['all_pass'] else 1)

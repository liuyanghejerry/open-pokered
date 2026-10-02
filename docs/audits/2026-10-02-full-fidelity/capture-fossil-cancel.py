#!/usr/bin/env python3
"""Capture the real fossil menu B-cancel using an isolated save/snapshot."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

repo = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(repo / 'scripts'))
from debug_drive import DebugClient

parser = argparse.ArgumentParser()
parser.add_argument('binary')
parser.add_argument('--source', required=True)
parser.add_argument('--label', choices=['before', 'after'], required=True)
parser.add_argument('--port', type=int, default=9530)
args = parser.parse_args()
binary = Path(args.binary).resolve()
audit = Path(__file__).resolve().parent
shots = repo / 'docs/screenshots/fidelity-systems'
shots.mkdir(parents=True, exist_ok=True)

with tempfile.TemporaryDirectory(prefix='pokered-fossil-cancel-') as directory:
    temporary = Path(directory)
    fixture = json.loads((audit / 'systems-runtime-input.json').read_text())
    fixture['game_data']['bag']['items'] = [['OldAmber', 1]]
    snapshot = temporary / 'input.json'
    snapshot.write_text(json.dumps(fixture))
    log_path = temporary / 'app.log'
    with log_path.open('w') as log:
        process = subprocess.Popen([
            str(binary), 'run', '--snapshot', str(snapshot), '--save', str(temporary / 'slot.sav'),
            '--skip-intro', '--headless', '--no-audio', '--speed', '0', '--debug-port', str(args.port),
        ], cwd=repo, stdout=log, stderr=log)
        try:
            client = DebugClient(args.port)
            def command(**request):
                result = client.cmd(**request)
                assert result.get('ok'), result
                return result.get('data')
            command(cmd='warp', map='CinnabarLabFossilRoom', x=5, y=3)
            command(cmd='step_frames', count=90)
            command(cmd='interact_with', id='npc:0')
            command(cmd='skip_dialogue')
            command(cmd='step_frames', count=32)
            menu = command(cmd='get_state')
            assert menu['screen'] == 'filter-bag', menu
            bag_before = command(cmd='get_bag')
            command(cmd='press_timeline', buttons=[None, None, 'b', None, None, None, None], advance=True)
            command(cmd='step_frames', count=120)
            canceled = command(cmd='get_state')
            bag_after = command(cmd='get_bag')
            flags = command(cmd='get_flags')
            assert bag_after == bag_before
            assert not flags.get('EVENT_GAVE_FOSSIL_TO_LAB', False)
            assert not flags.get('EVENT_LAB_STILL_REVIVING_FOSSIL', False)
            if args.label == 'after':
                # get_state flattens the displayed page for its summary;
                # the active script effect retains the authored line break.
                assert canceled['script_effect']['text'] == 'Aiyah! You come\nagain!', canceled
            shot = shots / f'fossil-cancel-{args.label}.png'
            command(cmd='capture_frame', path=str(shot))
            evidence = {
                'source': args.source,
                'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                'input': 'OldAmber x1; warp 5,3; scientist1; finish intro; real filtered menu B; 120 frames',
                'menu_before_cancel': menu, 'after_cancel': canceled,
                'bag_before': bag_before, 'bag_after': bag_after, 'flags': flags,
                'screenshot': str(shot.relative_to(repo)),
                'screenshot_sha256': hashlib.sha256(shot.read_bytes()).hexdigest(),
            }
            client.close()
        finally:
            process.terminate()
            process.wait(timeout=10)
    evidence['runtime_log'] = log_path.read_text()
    (audit / f'fossil-cancel-{args.label}.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + '\n')
    print(f'{args.label}: actual fossil menu B-cancel, item and flags preserved; capture recorded')

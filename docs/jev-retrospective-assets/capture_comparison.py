#!/usr/bin/env python3
"""Supplementary, real-input opening recordings; never a full-clear replay.

Run from the repository root: python3 docs/jev-retrospective-assets/capture_comparison.py script|jev
Only adds recording/isolated process setup to the existing controllers.
"""
import argparse
import hashlib
import json
import shutil
import sys
import tempfile
import time
import traceback
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import playthrough as pt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('controller', choices=['script', 'jev'])
    parser.add_argument('--output', type=Path,
                        default=ROOT / '.artifacts/jev-retrospective-20260920/captures')
    args = parser.parse_args()
    folder = args.output.resolve() / args.controller
    folder.mkdir(parents=True, exist_ok=False)
    private = args.output.resolve() / 'temporary'
    private.mkdir(exist_ok=True)
    tempfile.tempdir = str(private)
    video = folder / 'opening.mp4'
    metadata = {'controller': args.controller, 'purpose': 'supplementary opening capture',
                'seed': 42, 'binary_sha256': hashlib.sha256(pt.BIN.read_bytes()).hexdigest(),
                'recording': 'every simulated update; engine default record-video-fps=240',
                'full_clear_evidence': False}
    started = time.monotonic()
    if args.controller == 'jev':
        from openpokered import run_autonomous as runner
        original = runner.JevGame

        class RecordedGame(original):
            def __init__(self, *a, **kw):
                super().__init__(*a, record_video=video, **kw)

        runner.JevGame = RecordedGame
        code = runner.main(['--until', 'get-pokedex', '--seed', '42', '--max-calls', '150',
                            '--max-actions', '100', '--wall-budget', '300',
                            '--output', str(folder / 'runs')])
        metadata['success'] = code == 0
    else:
        with tempfile.TemporaryDirectory(prefix='script-', dir=private) as temp:
            binary = Path(temp) / 'pokered-app'
            shutil.copy2(pt.BIN, binary)
            # Preserve the historical driver's ordinary real-time loop.
            # Jev's runner owns its driven-only timing separately.
            game = pt.Game(binary=binary, seed=42, record_video=video)
            records = []
            try:
                for mid, description, fn in pt.MILESTONES:
                    game.smart_moves = False
                    fn(game)
                    state = game.st()
                    records.append({'milestone': mid, 'description': description,
                                    'elapsed_s': round(time.monotonic()-started, 3),
                                    'state': state})
                    game.d.cmd(cmd='capture_frame', path=str(folder / f'{mid}.png'))
                    if mid == 'm08':
                        break
                flags = game.d.cmd(cmd='get_flags')['data']
                metadata['success'] = bool(flags.get('EVENT_GOT_POKEDEX'))
                (folder / 'milestones.json').write_text(json.dumps(records, indent=2))
                (folder / 'flags.json').write_text(json.dumps(flags, indent=2))
            except Exception:
                metadata['success'] = False
                metadata['failure'] = traceback.format_exc()
                (folder / 'milestones.json').write_text(json.dumps(records, indent=2))
                (folder / 'failure-state.json').write_text(json.dumps(game.st(), indent=2))
                game.d.cmd(cmd='capture_frame', path=str(folder / 'failure.png'))
            finally:
                game.log.flush()
                shutil.copy2(game.run_dir / 'game.log', folder / 'game.log')
                game.close()
        code = 0 if metadata.get('success') else 1
    metadata['wall_s_including_recording'] = round(time.monotonic()-started, 3)
    (folder / 'capture.json').write_text(json.dumps(metadata, indent=2)+'\n')
    print(json.dumps(metadata), flush=True)
    return code


if __name__ == '__main__':
    raise SystemExit(main())

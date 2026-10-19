"""Capture paired native frames; use master first, then the PR checkout.

Requires a desktop,debug-server CLI. Each run uses isolated temporary saves.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workspace', type=Path, required=True)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--identity', type=Path, required=True)
    parser.add_argument('--mode', choices=['before', 'after'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    workspace = args.workspace.resolve()
    binary = args.binary.resolve()
    here = Path(__file__).resolve().parent
    plan_path = here / 'capture-plan.json'
    plan = json.loads(plan_path.read_text())
    identity = json.loads(args.identity.read_text())
    assert binary == Path(identity['binary']).resolve()
    assert digest(binary) == identity['sha256']
    output = args.output.resolve()
    output.mkdir(parents=True)
    temporary = output / 'tmp'
    temporary.mkdir()
    os.environ['TMPDIR'] = str(temporary)
    sys.path.insert(0, str(workspace / 'scripts'))
    import playthrough as driver

    def capture(game, destination):
        frame = game.st()['frame_count']
        assert game.d.cmd(cmd='capture_frame', path=str(destination))['ok']
        assert game.st()['frame_count'] == frame, 'capture must not advance frames'
        return frame

    rows = []
    for case in plan['cases']:
        directory = output / case['name']
        directory.mkdir()
        fixture = here / 'fixtures' / case['fixture']
        assert digest(binary) == identity['sha256'], 'binary changed during run'
        game = driver.Game(binary=binary, snapshot=fixture, seed=plan['seed'], speed=0)
        try:
            driver.resume_reentry(game)
            capture(game, directory / 'initial-world.png')
            assert game.d.cmd(cmd='start_wild_battle', species=case['target'],
                              level=case['target_level'])['ok']
            initial = game.st()
            keys = ('screen', 'frame_count', 'map_name', 'player_x', 'player_y',
                    'party', 'battle_live', 'battle_phase', 'battle_inventory')
            initial = {key: initial[key] for key in keys}
            (directory / 'initial.json').write_text(json.dumps(initial, sort_keys=True, indent=2) + '\n')
            for press in case['inputs']:
                assert game.st()['frame_count'] == press['frame'], (case['name'], press)
                game.tap(press['button'], press['gap'])
                game.step(10)
            assert game.st()['battle_message'] == case[args.mode]
            game.step(plan['settle_frames'])
            state = game.st()
            assert state['screen'] == 'battle'
            assert state['battle_message'] == case[args.mode]
            victim = 'player' if case['name'].startswith('enemy-') else 'enemy'
            mon = state['battle_live'][victim]
            assert mon['hp'] == mon['max_hp']
            frame = capture(game, directory / 'frame.png')
            (directory / 'state.json').write_text(json.dumps(state, indent=2) + '\n')
            rows.append({'case': case['name'], 'frame': frame,
                         'initial_world_sha256': digest(directory / 'initial-world.png'),
                         'initial_state_sha256': digest(directory / 'initial.json'),
                         'frame_sha256': digest(directory / 'frame.png'),
                         'fixture_sha256': digest(fixture),
                         'message': state['battle_message'], 'victim_hp_unchanged': True})
            print(rows[-1], flush=True)
        finally:
            game.log.flush()
            shutil.copyfile(game.run_dir / 'game.log', directory / 'game.log')
            game.close()
    receipt = {'mode': args.mode, 'binary_identity': identity,
               'workspace_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=workspace, text=True).strip(),
               'controller_sha256': digest(Path(__file__).resolve()),
               'plan_sha256': digest(plan_path), 'cases': rows,
               'scope': 'Native before/after, same saved fixtures, seed, input timeline and frame; not original raster parity.'}
    (output / 'verification.json').write_text(json.dumps(receipt, indent=2) + '\n')
    temporary.rmdir()


if __name__ == '__main__':
    main()

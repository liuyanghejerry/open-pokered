"""Capture actual master/PR frames using identical fixture and raw input tape."""
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
    for name in ('workspace', 'binary', 'identity', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--mode', choices=('before', 'after'), required=True)
    args = parser.parse_args()
    workspace = args.workspace.resolve()
    binary = args.binary.resolve()
    here = Path(__file__).resolve().parent
    plan_path = here / 'capture-plan.json'
    plan = json.loads(plan_path.read_text())
    identity = json.loads(args.identity.read_text())
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=workspace, text=True).strip()
    assert identity['actual_source'] == head
    assert Path(identity['binary']).resolve() == binary
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
        assert game.st()['frame_count'] == frame
        return frame

    rows = []
    for case in plan['cases']:
        directory = output / case['name']
        directory.mkdir()
        fixture = here / 'fixtures' / case['fixture']
        assert digest(binary) == identity['sha256']
        game = driver.Game(binary=binary, snapshot=fixture, seed=plan['seed'], speed=0)
        try:
            driver.resume_reentry(game)
            capture(game, directory / 'initial-world.png')
            assert game.d.cmd(cmd='start_wild_battle', species=case['target'],
                              level=case['target_level'])['ok']
            initial = game.st()
            keys = ('screen', 'frame_count', 'map_name', 'player_x', 'player_y',
                    'party', 'battle_live', 'battle_phase', 'battle_inventory')
            (directory / 'initial.json').write_text(json.dumps(
                {key: initial[key] for key in keys}, sort_keys=True, indent=2) + '\n')
            for entry in case['inputs']:
                assert game.st()['frame_count'] == entry['frame'], (case['name'], entry)
                assert game.d.cmd(**entry['command'])['ok']
            state = game.st()
            assert state['frame_count'] == case['final_frame']
            assert state['battle_message'] == case[args.mode], state['battle_phase']
            assert state['battle_live']['enemy']['hp'] == case[args.mode + '_hp']
            if case['use_x_accuracy']:
                assert state['battle_inventory'] == [], state['battle_inventory']
            frame = capture(game, directory / 'frame.png')
            (directory / 'state.json').write_text(json.dumps(state, indent=2) + '\n')
            rows.append({'case': case['name'], 'frame': frame,
                         'initial_world_sha256': digest(directory / 'initial-world.png'),
                         'initial_state_sha256': digest(directory / 'initial.json'),
                         'fixture_sha256': digest(fixture), 'frame_sha256': digest(directory / 'frame.png'),
                         'message': state['battle_message'], 'victim_hp': state['battle_live']['enemy']['hp'],
                         'x_accuracy_consumed': case['use_x_accuracy']})
            print(rows[-1], flush=True)
        finally:
            game.log.flush()
            shutil.copyfile(game.run_dir / 'game.log', directory / 'game.log')
            game.close()
    assert digest(binary) == identity['sha256']
    assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=workspace, text=True).strip() == head
    receipt = {'mode': args.mode, 'binary_identity': identity, 'workspace_head': head,
               'controller_sha256': digest(Path(__file__).resolve()), 'plan_sha256': digest(plan_path),
               'cases': rows, 'scope': 'Native matched inputs and frames; not original raster parity.'}
    (output / 'verification.json').write_text(json.dumps(receipt, indent=2) + '\n')
    temporary.rmdir()


if __name__ == '__main__':
    main()

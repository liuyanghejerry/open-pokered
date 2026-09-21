#!/usr/bin/env python3
"""Record real full playthroughs, including independent-process CONTINUE.

Instrumentation only: frozen controller sources keep their original decisions.
Every engine update is recorded at 60 fps; wall/request time is logged separately.
"""
import argparse
import datetime
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import traceback
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('controller', choices=['script', 'jev'])
    ap.add_argument('--source', type=Path, required=True)
    ap.add_argument('--output', type=Path, required=True)
    ap.add_argument('--seed', type=int, default=42)
    ap.add_argument('--wall-budget', type=float, default=10800)
    ap.add_argument('--until', help='Preflight only; omit for a full clear')
    args = ap.parse_args()
    source, out = args.source.resolve(), args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    (out / 'temporary').mkdir()
    tempfile.tempdir = str(out / 'temporary')
    # Use the existing account without copying credentials into the frozen tree.
    env_file = REPO / '.env'
    if env_file.exists():
        for line in env_file.read_text().splitlines():
            key, sep, value = line.partition('=')
            if sep and key.strip() in ('TYPESAFE_API_KEY', 'TYPESAFE_BASE_URL'):
                os.environ.setdefault(key.strip(), value.strip().strip('"\''))
    sys.path.insert(0, str(source / 'scripts'))
    import playthrough as pt
    from openpokered import run_autonomous as autonomous

    started = time.monotonic()
    metadata = {
        'controller': args.controller, 'seed': args.seed,
        'controller_pid': os.getpid(),
        'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'source_commit': subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip(),
        'binary_sha256': hashlib.sha256(pt.BIN.read_bytes()).hexdigest(),
        'video_fps': 60, 'game_frames_per_video_second': 60,
        'wall_waits_recorded_in': 'commands.jsonl',
        'controller_clock': 'default real-time loop' if args.controller == 'script' else 'driven-only',
        'scope': args.until or 'full clear and independent CONTINUE',
        'processes': [], 'success': False,
    }
    files = [*source.joinpath('scripts/openpokered').glob('*.py'),
             source/'scripts/playthrough.py', source/'scripts/playthrough_late.py', source/'scripts/debug_drive.py']
    metadata['policy_files'] = {str(p.relative_to(source)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(files)}
    source_patch = subprocess.check_output(['git', '-C', str(source), 'diff', '--binary'])
    if source_patch:
        (out / 'source.patch').write_bytes(source_patch)
        metadata['source_patch_sha256'] = hashlib.sha256(source_patch).hexdigest()
    original_init, original_close = pt.Game.__init__, pt.Game.close
    original_popen = subprocess.Popen
    commands = (out/'commands.jsonl').open('w', buffering=1)
    observations = (out/'observations.jsonl').open('w', buffering=1)
    active, last_progress = {}, [0.0]
    script_binary = out/'script-runtime/pokered-app'
    if args.controller == 'script':
        script_binary.parent.mkdir()
        shutil.copy2(pt.BIN, script_binary)

    def emit(stream, value):
        stream.write(json.dumps(value, ensure_ascii=False, separators=(',', ':'))+'\n')

    def progress(**extra):
        data = {'elapsed_s': round(time.monotonic()-started, 3),
                'controller': args.controller, 'processes': list(active.values()), **extra}
        temp = out/'progress.next.json'
        temp.write_text(json.dumps(data, ensure_ascii=False, indent=2))
        temp.replace(out/'progress.json')

    def launch(command, *a, **kw):
        if isinstance(command, list) and '--record-video' in command:
            command = [*command, '--record-video-fps', '60']
        return original_popen(command, *a, **kw)

    def init(game, *a, **kw):
        index = len(metadata['processes']) + 1
        folder = out/f'process-{index:02}'
        folder.mkdir()
        kw['record_video'] = folder/'game.mp4'
        kw.setdefault('seed', args.seed)
        if args.controller == 'script':
            kw.setdefault('binary', script_binary)
        original_init(game, *a, **kw)
        game._capture_index, game._capture_folder = index, folder
        item = {'index': index, 'pid': game.proc.pid, 'started_wall_s': round(time.monotonic()-started, 3),
                'video': str((folder/'game.mp4').relative_to(out)), 'initial_save_exists': game.save_path.exists()}
        if game.save_path.exists():
            shutil.copy2(game.save_path, folder/'input.sav')
            item['input_save_sha256'] = hashlib.sha256(game.save_path.read_bytes()).hexdigest()
        metadata['processes'].append(item)
        active[index] = {'index': index, 'pid': game.proc.pid}
        raw_cmd = game.d.cmd
        last = {'frame': None}

        def command(**request):
            t0 = time.monotonic()
            if t0-started > args.wall_budget + 180:
                raise TimeoutError('full recording wall budget exceeded')
            response = raw_cmd(**request)
            t1 = time.monotonic()
            payload = response.get('data') if isinstance(response, dict) else None
            state = payload if isinstance(payload, dict) and 'screen' in payload else None
            if state is None and isinstance(payload, dict) and isinstance(payload.get('state'), dict):
                state = payload['state']
            before = last['frame']
            if state and 'frame_count' in state:
                last['frame'] = state['frame_count']
                keys = ('frame_count', 'screen', 'map_name', 'player_x', 'player_y', 'badges', 'money',
                        'party', 'battle_phase', 'battle_live', 'hall_of_fame_count', 'hof_phase',
                        'credits_phase', 'credits_final_button', 'active_script_effect', 'shop_phase')
                compact = {k: state.get(k) for k in keys}
                emit(observations, {'process': index, 'wall_s': round(t1-started, 4), **compact})
                active[index].update({k: compact.get(k) for k in ('frame_count','screen','map_name','badges','party','hall_of_fame_count','battle_phase')})
            emit(commands, {'process': index, 'start_s': round(t0-started, 4), 'end_s': round(t1-started, 4),
                            'request': request, 'ok': response.get('ok'),
                            'frame_before': before, 'frame_after': last['frame']})
            if t1-last_progress[0] > 5:
                progress()
                last_progress[0] = t1
            return response

        game.d.cmd = command
        progress()

    def close(game):
        folder = getattr(game, '_capture_folder', None)
        if folder is not None:
            game.log.flush()
            shutil.copy2(game.run_dir/'game.log', folder/'game.log')
            if game.save_path.exists():
                shutil.copy2(game.save_path, folder/'output.sav')
            flags = game.binary.parent/'pokered.script_flags.json'
            if flags.exists():
                shutil.copy2(flags, folder/'script_flags.json')
        original_close(game)
        if folder is not None:
            index = game._capture_index
            item = metadata['processes'][index-1]
            item['ended_wall_s'] = round(time.monotonic()-started, 3)
            item['last_observed'] = active.pop(index, {})
            progress()

    pt.Game.__init__, pt.Game.close = init, close
    subprocess.Popen = launch
    (out/'capture-start.json').write_text(json.dumps(metadata, indent=2))
    try:
        if args.controller == 'jev':
            code = autonomous.main(['--binary', str(pt.BIN), '--until', args.until or 'become-champion',
                                    '--seed', str(args.seed), '--max-calls', '4000', '--max-actions', '4000',
                                    '--wall-budget', str(args.wall_budget), '--output', str(out/'run')])
            metadata['success'] = code == 0
        else:
            sys.argv = ['playthrough.py', '--until', args.until or 'm49', '--artifacts', str(out/'milestones')]
            pt.main()
            final = json.loads((out/'milestones'/f'{args.until or "m49"}.json').read_text())
            metadata['success'] = bool(args.until or final.get('first_clear_verification'))
        metadata['result'] = 'passed' if metadata['success'] else 'controller_stopped'
    except BaseException:
        metadata['result'] = 'failed'
        metadata['failure'] = traceback.format_exc()
        (out/'failure.txt').write_text(metadata['failure'])
    finally:
        commands.close()
        observations.close()
        metadata['wall_s'] = round(time.monotonic()-started, 3)
        for item in metadata['processes']:
            video = out/item['video']
            for _ in range(100):
                probe = subprocess.run(['ffprobe','-v','error','-show_entries',
                                        'format=duration,size:stream=width,height,r_frame_rate,nb_frames',
                                        '-of','json',str(video)],capture_output=True,text=True)
                if probe.returncode == 0:
                    item['video_probe'] = json.loads(probe.stdout)
                    item['video_sha256'] = hashlib.sha256(video.read_bytes()).hexdigest()
                    break
                time.sleep(.2)
            else:
                metadata['success'] = False
                item['video_error'] = 'video did not finalize'
        (out/'capture.json').write_text(json.dumps(metadata,ensure_ascii=False,indent=2)+'\n')
        progress(finished=True, success=metadata['success'])
        print(json.dumps({'success':metadata['success'],'result':metadata.get('result'),
                          'wall_s':metadata['wall_s'],'output':str(out)}),flush=True)
    return 0 if metadata['success'] else 1


if __name__ == '__main__':
    raise SystemExit(main())

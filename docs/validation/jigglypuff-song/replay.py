#!/usr/bin/env python3
"""Replay the committed 2609-frame tape using a normal debug-server CLI.

Python standard library only. --check validates assets and tape arithmetic;
it does not start a Game. A replay uses a fresh physical save copy and ordinary
inputs, then stops only its owned diagnostic Game process group.
"""
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import struct
import subprocess
import time
import traceback

HERE = Path(__file__).resolve().parent


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def checked_assets():
    manifest = json.loads((HERE / 'validation.json').read_text())
    for name in ['fixture.sav', 'inputs.rle.json', 'replay.py', 'README.md']:
        assert sha(HERE / name) == manifest['assets'][name]['sha256'], name
    packed = json.loads((HERE / 'inputs.rle.json').read_text())
    buttons = []
    for button, count in packed['runs']:
        assert button in {None, 'a', 'up'}
        assert type(count) is int and count > 0
        buttons.extend([button] * count)
    assert len(buttons) == packed['frames'] == 2609
    assert packed['anchors'] == {
        'constructor': 0, 'boot_end': 503, 'talk_start': 535,
        'first_text_ready': 608, 'final': 2609,
    }
    assert buttons[535:541] == ['up', 'up', None, None, 'a', 'a']
    assert buttons[541:608] == [None] * 67
    assert buttons[608:] == ['a'] + [None] * 2000
    original_json = json.dumps(buttons, ensure_ascii=False, indent=2) + '\n'
    assert hashlib.sha256(original_json.encode()).hexdigest() == packed['expanded_json_sha256']
    assert packed['expanded_json_sha256'] == manifest['normal_cli']['full_tape_sha256']
    assert (HERE / 'fixture.sav').stat().st_size == 32768
    return manifest, buttons


def interrupted(signum, _frame):
    raise KeyboardInterrupt(f'received signal {signum}')


def stop(proc):
    record = {'pid': proc.pid, 'signals': [], 'scope': 'owned diagnostic Game shutdown'}
    for sig, timeout in [(signal.SIGTERM, 8), (signal.SIGKILL, 2)]:
        if proc.poll() is None:
            try:
                os.killpg(proc.pid, sig)
                record['signals'].append(sig.name)
            except ProcessLookupError:
                pass
        try:
            proc.wait(timeout=timeout)
            break
        except subprocess.TimeoutExpired:
            continue
    record['exit_code'] = proc.poll()
    try:
        os.killpg(proc.pid, 0)
        record['process_group_gone'] = False
    except ProcessLookupError:
        record['process_group_gone'] = True
    assert record['exit_code'] is not None and record['process_group_gone'], record
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--role', choices=['before', 'after'])
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    manifest, buttons = checked_assets()
    if args.check:
        assert not any([args.binary, args.role, args.output])
        print('PASS assets and literal 2609-frame tape; no Game started')
        return
    assert all([args.binary, args.role, args.output]), 'supply --binary, --role, and --output'
    binary = args.binary.resolve(strict=True)
    assert binary.is_file() and os.access(binary, os.X_OK)
    expected = manifest['normal_cli']['cases'][args.role]
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    save = out / 'ordinary.sav'
    shutil.copyfile(HERE / 'fixture.sav', save)
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        port = probe.getsockname()[1]
    argv = [str(binary), 'run', '--headless', '--speed', '0', '--no-audio',
            '--save', str(save), '--seed', '42', '--lang', 'en', '--debug-port', str(port)]
    environment = {key: value for key, value in os.environ.items() if not key.startswith('POKERED_')}
    result = {
        'role': args.role, 'passed': False, 'argv': argv, 'binary_sha256': sha(binary),
        'recorded_source_head': expected['source_head'],
        'matches_recorded_binary': sha(binary) == expected['binary_sha256'],
        'note': 'Source of a newly built supplied binary is the caller responsibility.',
        'runtime_setters': 0, 'input_frames': len(buttons), 'samples': {}, 'captures': [],
    }
    requests = collections.Counter()
    phases = set()
    entries = 0
    previous_effect = None
    proc = stream = sock = None
    failure = None
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    try:
        with (out / 'game.log').open('x') as log:
            proc = subprocess.Popen(argv, cwd=out, env=environment, stdout=log,
                                    stderr=subprocess.STDOUT, start_new_session=True)
            for _ in range(200):
                assert proc.poll() is None, ('early Game exit', proc.returncode)
                try:
                    sock = socket.create_connection(('127.0.0.1', port), timeout=.1)
                    break
                except (ConnectionRefusedError, TimeoutError):
                    time.sleep(.05)
            assert sock is not None, 'debug server did not start; no restart'
            sock.settimeout(30)
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            stream = sock.makefile('rwb')

            def rpc(command, **arguments):
                assert command in {'get_state', 'press_timeline', 'capture_frame'}
                requests[command] += 1
                stream.write((json.dumps({'cmd': command, **arguments}) + '\n').encode())
                stream.flush()
                raw = stream.readline(4 * 1024 * 1024)
                assert raw.endswith(b'\n'), 'truncated debug reply'
                reply = json.loads(raw)
                assert reply.get('ok') is True, reply
                return reply['data']

            state = rpc('get_state')
            assert state['frame_count'] == 0 and state['screen'] != 'overworld'
            for frame, button in enumerate(buttons, 1):
                ack = rpc('press_timeline', buttons=[button], start_at_frame=frame - 1, advance=True)
                assert ack['frame_count'] == frame and ack['padding_frames'] == 0 and ack['frames'] == 1
                state = rpc('get_state')
                assert state['frame_count'] == frame
                effect = state.get('active_script_effect')
                if effect == 'JigglypuffSong':
                    entries += previous_effect != effect
                    phases.add(state['script_effect']['state']['phase'])
                previous_effect = effect
                if frame in {503, 535, 608, 2609}:
                    assert (state['screen'], state['map_name'], state['player_x'], state['player_y']) == (
                        'overworld', 'PewterPokecenter', 1, 4)
                    result['samples'][str(frame)] = {key: state.get(key) for key in [
                        'screen', 'map_name', 'player_x', 'player_y', 'player_facing',
                        'script_running', 'active_script_effect', 'script_effect',
                        'dialogue', 'dialogue_state', 'audio_sfx']}
                if frame in {503, 535}:
                    assert state['player_facing'] == 'Down'
                    assert not state['script_running'] and effect is None and state['dialogue'] is None
                if frame == 608 and args.role == 'before':
                    assert effect == 'ShowDialogue'
                    assert state['script_effect']['text'] == 'JIGGLYPUFF: Puu pupuu!'
                    assert state['dialogue_state']['waiting_for_input']
                if frame in {608, 2609}:
                    image = out / f'frame-{frame:05}.png'
                    rpc('capture_frame', path=str(image))
                    png = image.read_bytes()
                    assert png[:8] == b'\x89PNG\r\n\x1a\n' and struct.unpack('>II', png[16:24]) == (160, 144)
                    result['captures'].append({'frame': frame, 'sha256': sha(image),
                        'matches_recorded_png': sha(image) == expected['captures'][str(frame)]['sha256']})
            assert state['player_facing'] == 'Up'
            if args.role == 'before':
                assert state['active_script_effect'] == 'ShowDialogue'
                assert 'JIGGLYPUFF sang its SONG.' in state['script_effect']['text']
                assert state['dialogue_state']['waiting_for_input']
            else:
                assert not state['script_running'] and state['active_script_effect'] is None
                assert state['dialogue'] is None and state['dialogue_state'] is None
                assert entries == 1 and {'SongSpin', 'PostSongWait'} <= phases
            assert sha(save) == manifest['assets']['fixture.sav']['sha256']
            assert not save.with_suffix('.script_flags.json').exists()
            result['passed'] = True
    except BaseException:
        failure = traceback.format_exc()
        result['failure'] = failure
    finally:
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        for resource in [stream, sock]:
            if resource is not None:
                try:
                    resource.close()
                except OSError as error:
                    result.setdefault('cleanup_errors', []).append(str(error))
        try:
            if proc is not None:
                result['owned_shutdown'] = stop(proc)
        except BaseException:
            failure = traceback.format_exc()
            result['cleanup_failure'] = failure
            result['passed'] = False
        result['request_counts'] = dict(requests)
        result['observed_typed_phases'] = sorted(phases)
        (out / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    assert failure is None, failure
    print(f'PASS {args.role}: identical 2609 inputs, captures at absolute frames 608 and 2609')


if __name__ == '__main__':
    main()

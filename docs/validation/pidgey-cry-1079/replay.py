#!/usr/bin/env python3
"""Replay one committed literal tape against a normal debug-server CLI binary.

Python stdlib only. No private imports, snapshots, warps, setters or seed search.
The output directory must be new. The child Game is stopped after the recorded
boundary; its diagnostic termination is not a natural successful Game exit.
"""
from pathlib import Path
import argparse
import collections
import hashlib
import json
import os
import shutil
import signal
import socket
import struct
import subprocess
import time
import traceback

HERE = Path(__file__).resolve().parent


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def expand(runs):
    result = []
    for key, count in runs:
        assert key in {None, 'a', 'b', 'up'} and isinstance(count, int) and count > 0
        result.extend([key] * count)
    return result


def interrupt(signum, _frame):
    raise KeyboardInterrupt(f'received signal {signum}; stop only the owned Game')


def stop(proc):
    record = {'pid': proc.pid, 'scope': 'owned diagnostic shutdown after boundary', 'signals': []}
    for sig, timeout in [(signal.SIGTERM, 8), (signal.SIGKILL, 2)]:
        if proc.poll() is None:
            try:
                os.killpg(proc.pid, sig)
                record['signals'].append(sig.name)
            except ProcessLookupError:
                pass
        try:
            proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            continue
        break
    record['exit_code'] = proc.poll()
    assert record['exit_code'] is not None
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--role', required=True, choices=['before', 'after'])
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    manifest = json.loads((HERE / 'validation.json').read_text())
    for name in ['pidgey.sav', 'inputs.json', 'replay.py']:
        assert sha(HERE / name) == manifest['assets'][name]['sha256'], name
    packed = json.loads((HERE / 'inputs.json').read_text())
    assert packed['format'] == 'gb-button-runs/v1'
    buttons = expand(packed['runs'])
    assert len(buttons) == packed['frames'] == 682
    assert buttons[543:612] == [None] * 69
    assert buttons[612:] == ['b'] * 2 + [None] * 64 + ['b'] * 2 + [None] * 2
    expected = manifest['normal_cli']['cases'][args.role]
    assert len(buttons) == expected['final_frame']
    original_json = json.dumps(buttons, ensure_ascii=False, indent=2) + '\n'
    assert hashlib.sha256(original_json.encode()).hexdigest() == expected['recorded_full_tape_sha256']
    binary = args.binary.resolve()
    assert binary.is_file() and os.access(binary, os.X_OK)
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    save = out / 'pokered.sav'
    shutil.copyfile(HERE / 'pidgey.sav', save)
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        port = probe.getsockname()[1]
    argv = [str(binary), 'run', '--headless', '--speed', '0', '--seed', '42',
            '--debug-port', str(port), '--lang', 'en', '--no-audio', '--save', str(save)]
    env = {key: value for key, value in os.environ.items() if not key.startswith('POKERED_')}
    result = {'role': args.role, 'binary_sha256': sha(binary), 'argv': argv,
              'recorded_source_head': expected['actual_source_head'], 'passed': False,
              'input_frames': len(buttons), 'runtime_setters': 0, 'samples': {}}
    proc = stream = sock = None
    requests = collections.Counter()
    signal.signal(signal.SIGTERM, interrupt)
    signal.signal(signal.SIGINT, interrupt)
    failure = None
    try:
        with (out / 'game.log').open('x') as log:
            proc = subprocess.Popen(argv, cwd=out, env=env, stdout=log,
                                    stderr=subprocess.STDOUT, start_new_session=True)
            for _ in range(200):
                assert proc.poll() is None, ('early Game exit', proc.returncode)
                try:
                    sock = socket.create_connection(('127.0.0.1', port), timeout=.1)
                    break
                except (ConnectionRefusedError, TimeoutError):
                    time.sleep(.05)
            assert sock is not None, 'debug server did not start'
            sock.settimeout(30)
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            stream = sock.makefile('rwb')

            def rpc(command, **kwargs):
                assert command in {'get_state', 'get_flags', 'get_bag', 'press_timeline', 'capture_frame'}
                requests[command] += 1
                stream.write((json.dumps({'cmd': command, **kwargs}) + '\n').encode())
                stream.flush()
                raw = stream.readline(4 * 1024 * 1024)
                assert raw.endswith(b'\n'), 'truncated debug reply'
                reply = json.loads(raw)
                assert reply.get('ok') is True, reply
                return reply.get('data')

            state = rpc('get_state')
            assert state['frame_count'] == 0
            initial = None
            for frame, button in enumerate(buttons, 1):
                receipt = rpc('press_timeline', buttons=[button], start_at_frame=frame - 1, advance=True)
                assert receipt['frame_count'] == frame and receipt['padding_frames'] == 0 and receipt['frames'] == 1
                state = rpc('get_state')
                assert state['frame_count'] == frame
                if frame == 503:
                    assert state['screen'] == 'overworld' and state['map_name'] == 'VermilionPidgeyHouse'
                    assert [state['player_x'], state['player_y']] == [4, 6]
                    assert state['money'] == 3000 and rpc('get_bag') == []
                    initial = (state['party'], rpc('get_flags'))
                if frame == expected['anchor_frame']:
                    dialogue = state['dialogue_state']
                    assert dialogue['text'] == 'PIDGEY: Kurukkoo!' and dialogue['waiting_for_input']
                    assert dialogue['char_index'] == dialogue['total_chars'] and not dialogue['holding_open']
                    if args.role == 'after':
                        assert state['script_effect']['state']['phase'] == 'SoundWait'
                        assert state['audio_sfx'] == {'id': 33, 'playing': True}
                    else:
                        assert state['active_script_effect'] == 'ShowDialogue'
                        assert state['audio_sfx'] != {'id': 33, 'playing': True}
                if frame == 614:
                    assert state['audio_sfx'] == {'id': 33, 'playing': True}
                    if args.role == 'after':
                        assert state['script_effect']['state']['phase'] == 'SoundWait'
                if args.role == 'after' and 614 <= frame <= 678:
                    dialogue = state['dialogue_state']
                    assert dialogue['text'] == 'PIDGEY: Kurukkoo!'
                    assert dialogue['char_index'] == dialogue['total_chars']
                    assert dialogue['waiting_for_input'] and not dialogue['holding_open']
                    assert state['script_effect']['state']['phase'] in {'SoundWait', 'OuterAck'}
                if frame in {503, 543, expected['anchor_frame'], expected['anchor_frame'] + 2,
                             expected['comparison_frame'], len(buttons)}:
                    result['samples'][str(frame)] = {key: state.get(key) for key in [
                        'screen', 'map_name', 'player_x', 'player_y', 'player_facing', 'money',
                        'active_script_effect', 'script_effect', 'dialogue', 'dialogue_state', 'audio_sfx']}
                if frame == expected['comparison_frame']:
                    assert frame == expected['anchor_frame'] + 66
                    if args.role == 'before':
                        assert state['dialogue'] is None and state['active_script_effect'] is None
                    else:
                        assert state['script_effect']['state']['phase'] == 'OuterAck'
                        assert state['dialogue_state']['waiting_for_input'] and not state['dialogue_state']['holding_open']
                    # Map music's normal drum can make the broad playing flag true.
                    assert state['audio_sfx'] != {'id': 33, 'playing': True}
                    rpc('capture_frame', path=str(out / 'comparison.png'))
                    assert rpc('get_state') == state, 'capture changed state'
                    png = (out / 'comparison.png').read_bytes()
                    assert png[:8] == b'\x89PNG\r\n\x1a\n' and struct.unpack('>II', png[16:24]) == (160, 144)
                    result['capture_sha256'] = sha(out / 'comparison.png')
                    result['matches_recorded_png_bytes'] = result['capture_sha256'] == expected['capture_sha256']
            assert state['active_script_effect'] is None and not state['script_running']
            assert state['dialogue'] is None and state['dialogue_state'] is None
            assert [state['player_x'], state['player_y']] == [4, 6] and state['money'] == 3000
            assert state['party'] == initial[0] and rpc('get_flags') == initial[1] and rpc('get_bag') == []
            assert sha(save) == manifest['assets']['pidgey.sav']['sha256']
            assert not save.with_suffix('.script_flags.json').exists()
            result['passed'] = True
    except BaseException:
        failure = traceback.format_exc()
        result['failure'] = failure
    finally:
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
        (out / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    assert failure is None, failure
    print(f"PASS {args.role}: same literal682, absolute comparison {expected['comparison_frame']}")


if __name__ == '__main__':
    main()

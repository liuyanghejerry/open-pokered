#!/usr/bin/env python3
"""Measure actual window-loop frames; never use synchronous frame stepping."""
import argparse
import json
from pathlib import Path
import socket
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--save', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        port = reservation.getsockname()[1]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = {'windowed': True, 'phases': []}
    command = [str(args.binary.resolve()), 'run',
               '--save' if args.save.suffix == '.sav' else '--snapshot',
               str(args.save.resolve()), '--skip-intro', '--seed', '42',
               '--debug-port', str(port)]
    with args.output.with_suffix('.log').open('w') as log:
        process = subprocess.Popen(command, cwd=root, stdout=log, stderr=subprocess.STDOUT)
        try:
            for _ in range(50):
                if process.poll() is not None:
                    raise RuntimeError(f'game exited: {process.returncode}')
                try:
                    sock = socket.create_connection(('127.0.0.1', port), timeout=2)
                    break
                except OSError:
                    time.sleep(0.2)
            else:
                raise TimeoutError('debug server did not start')
            sock.settimeout(5)
            with sock, sock.makefile('rwb') as wire:
                def rpc(**command):
                    wire.write((json.dumps(command) + '\n').encode())
                    wire.flush()
                    result = json.loads(wire.readline())
                    if not result['ok']:
                        raise RuntimeError(result)
                    return result.get('data')

                time.sleep(2)
                for phase in ['idle', 'held-down', 'battle-intro']:
                    if phase == 'held-down':
                        rpc(cmd='press_sequence', buttons=['down'] * 120)
                    if phase == 'battle-intro':
                        rpc(cmd='start_wild_battle', species='Pidgey', level=5)
                    samples = []
                    for sample in range(9):
                        state = rpc(cmd='get_state')
                        samples.append({'time': time.monotonic(), 'frame': state['frame_count'],
                                        'screen': state['screen'], 'x': state['player_x'],
                                        'y': state['player_y']})
                        if sample < 8:
                            time.sleep(1)
                    first, last = samples[0], samples[-1]
                    fps = (last['frame'] - first['frame']) / (last['time'] - first['time'])
                    report['phases'].append({'phase': phase, 'fps': fps, 'samples': samples})
                    print(f'{phase}: {fps:.2f} FPS', flush=True)
        except Exception as error:
            report['error'] = repr(error)
            raise
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            args.output.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()

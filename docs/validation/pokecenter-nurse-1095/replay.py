#!/usr/bin/env python3
"""Replay recorded ordinary inputs through a normal CLI; Python stdlib only."""
import argparse, hashlib, json, os, shutil, signal, socket, subprocess, time, traceback
from pathlib import Path
HERE = Path(__file__).resolve().parent

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--scenario', required=True, choices=['pair-before', 'pair-after', 'save', 'continue', 'heal'])
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--saved-sram', type=Path, help='SRAM produced by the save scenario; no sidecar is copied')
    parser.add_argument('--expected-binary-sha256')
    args = parser.parse_args()
    binary, out = args.binary.resolve(), args.output.resolve()
    assert binary.is_file() and not out.exists()
    if args.expected_binary_sha256:
        assert sha(binary) == args.expected_binary_sha256
    meta = json.loads((HERE / 'evidence.json').read_text())
    tape_name = 'pair-inputs.json' if args.scenario.startswith('pair-') else args.scenario + '-inputs.json'
    tape_file = HERE / tape_name
    assert sha(tape_file) == meta['inputs'][tape_name]['sha256']
    tape = json.loads(tape_file.read_text())
    assert len(tape) == meta['inputs'][tape_name]['frames']
    assert all(key in [None, 'a', 'b', 'up', 'down', 'start'] for key in tape)
    checkpoints = {int(k): v for k, v in meta.get('heal_checkpoints', {}).items()} if args.scenario == 'heal' else {}
    if args.scenario == 'heal':
        assert all(key is None for key in tape[meta['heal_confirmation_frame'] + 1:meta['first_healed_frame']])
    fixture = args.saved_sram.resolve() if args.scenario == 'continue' else HERE / 'fixture.sav'
    assert fixture and fixture.stat().st_size == 32768
    expected = meta['saved_sram_sha256'] if args.scenario == 'continue' else meta['fixture_sha256']
    assert sha(fixture) == expected
    out.mkdir()
    save = out / 'pokered.sav'
    shutil.copyfile(fixture, save)
    assert not save.with_suffix('.script_flags.json').exists()
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        port = probe.getsockname()[1]
    env = {k: v for k, v in os.environ.items() if not k.startswith('POKERED_')}
    argv = [str(binary), 'run', '--headless', '--speed', '0', '--seed', '42',
            '--debug-port', str(port), '--lang', 'en', '--no-audio', '--save', str(save)]
    result = dict(complete=False, all_pass=False, scenario=args.scenario, argv=argv,
                  binary_sha256=sha(binary), input_sha256=sha(tape_file), fixture_sha256=sha(fixture),
                  ordinary_input_only=True, runtime_setters=False, frames=0)
    def persist():
        (out / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    process, sock, stream = None, None, None
    frame, sequence = 0, 0
    log = (out / 'game.log').open('x')
    rpc_log = (out / 'rpc.jsonl').open('x')
    def rpc(command, **kwargs):
        nonlocal sequence
        assert command in {'get_state', 'press_timeline', 'capture_frame'}
        sequence += 1
        request = dict(cmd=command, **kwargs)
        stream.write((json.dumps(request) + '\n').encode()); stream.flush()
        reply = json.loads(stream.readline(4 * 1024 * 1024))
        rpc_log.write(json.dumps(dict(sequence=sequence, frame=frame, request=request, response=reply)) + '\n'); rpc_log.flush()
        assert reply.get('ok') is True, reply
        return reply['data']
    try:
        process = subprocess.Popen(argv, cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        result['Game_pid'] = process.pid; persist()
        for _ in range(200):
            assert process.poll() is None, process.returncode
            try:
                sock = socket.create_connection(('127.0.0.1', port), timeout=.1)
                break
            except (ConnectionRefusedError, TimeoutError):
                time.sleep(.05)
        assert sock is not None
        sock.settimeout(30); sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        if hasattr(socket, 'TCP_QUICKACK'): sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_QUICKACK, 1)
        stream = sock.makefile('rwb')
        assert rpc('get_state')['frame_count'] == 0
        while frame < len(tape):
            stop = min([frame + 60, len(tape)] + [n for n in checkpoints if n > frame])
            keys = tape[frame:stop]
            receipt = rpc('press_timeline', buttons=keys, start_at_frame=frame, advance=True)
            assert receipt == dict(queue_start_frame=frame, start_frame=frame, end_frame=frame + len(keys) - 1,
                                   frames=len(keys), padding_frames=0, advanced=True, frame_count=frame + len(keys))
            frame += len(keys)
            if frame in checkpoints:
                observed = rpc('get_state'); expected_state = checkpoints[frame]
                assert observed['active_script_effect'] == expected_state['active_script_effect']
                if 'hp' in expected_state: assert observed['party'][0]['hp'] == expected_state['hp']
                if expected_state.get('full_party'):
                    assert all(mon['hp'] == mon['max_hp'] and all(mon['pp']) for mon in observed['party'])
                if 'prompt' in expected_state: assert observed['script_effect']['prompt'] == expected_state['prompt']
                if expected_state.get('waiting_last_page'):
                    d = observed['dialogue_state']; assert d['waiting_for_input'] and d['page'] == d['total_pages']
                result.setdefault('checked_frames', []).append(frame)
        state = rpc('get_state')
        assert state['frame_count'] == len(tape) and state['screen'] == 'overworld'
        assert state['map_name'] == 'ViridianPokecenter' and [state['player_x'], state['player_y']] == [3, 3]
        if args.scenario == 'heal':
            assert state['party'][0]['hp'] == 342 and all(state['party'][0]['pp'])
        else:
            assert state['party'][0]['hp'] == 1 and state['party'][0]['pp'] == [0, 0, 0, 0]
        if args.scenario == 'pair-before':
            assert state['active_script_effect'] == 'ShowDialogue' and state['choice'] is None
            assert state['dialogue_state']['waiting_for_input']
        elif args.scenario in ['pair-after', 'continue']:
            assert state['choice']['options'] == ['HEAL', 'CANCEL'] and state['choice']['selected'] == 0
            assert state['script_effect']['state']['used'] == (args.scenario == 'continue')
            if args.scenario == 'continue':
                assert state['script_effect']['state']['question']['phase'] == 'Start'
                assert not save.with_suffix('.script_flags.json').exists()
        else:
            assert state['active_script_effect'] is None and not state['script_running']
            expected_saved = meta['healed_saved_sram_sha256'] if args.scenario == 'heal' else meta['saved_sram_sha256']
            assert sha(save) == expected_saved and sha(save) != sha(fixture)
        capture = out / 'final.png'
        rpc('capture_frame', path=str(capture))
        assert rpc('get_state') == state, 'capture must not change state'
        (out / 'final-state.json').write_text(json.dumps(state, ensure_ascii=False, indent=2) + '\n')
        result.update(all_pass=True, frames=frame, final_sram_sha256=sha(save), capture_sha256=sha(capture))
    except BaseException:
        result['failure'] = traceback.format_exc()
        print(result['failure'], flush=True)
    finally:
        if stream is not None: stream.close()
        if sock is not None: sock.close()
        if process is not None:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
                try: process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL); process.wait()
            try: os.killpg(process.pid, 0); gone = False
            except ProcessLookupError: gone = True
            result['owned_shutdown'] = dict(exit_code=process.returncode, group_gone=gone,
                scope='owned termination at diagnostic boundary; not natural game completion')
            result['all_pass'] &= gone
        result.update(complete=True, rpc_count=sequence)
        persist(); rpc_log.close(); log.close()
    print(json.dumps(dict(all_pass=result['all_pass'], scenario=args.scenario, frames=frame)), flush=True)
    return 0 if result['all_pass'] else 1

if __name__ == '__main__':
    raise SystemExit(main())

#!/usr/bin/env python3
"""Replay the published fixed physical-input tape with an existing normal CLI.

Prepared for reproduction; the recorded pair used the reviewed private recorder.
This independent public script has only been parsed and input expansion checked.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import signal
import socket
import subprocess
import time
import traceback

HERE = Path(__file__).resolve().parent


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def put(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + '\n')


def expand_inputs():
    meta = json.loads((HERE / 'validation.json').read_text())
    spec = meta['inputs']
    assert sha(HERE / spec['file']) == spec['sha256']
    encoded = json.loads((HERE / spec['file']).read_text())
    buttons = []
    for button, count in encoded['runs']:
        assert button in {None, 'a', 'b', 'start', 'down'}
        assert type(count) is int and count > 0
        buttons.extend([button] * count)
    assert len(buttons) == encoded['frames'] == spec['frames'] == 1905
    digest = hashlib.sha256((json.dumps(buttons, indent=2) + '\n').encode()).hexdigest()
    assert digest == encoded['expanded_json_sha256'] == spec['expanded_json_sha256']
    return meta, buttons


def gone(pgid):
    try:
        os.killpg(pgid, 0)
    except ProcessLookupError:
        return True
    return False


def stop(proc):
    record = {'pid': proc.pid, 'pgid': proc.pid, 'signals': [],
              'scope': 'owned diagnostic shutdown after assertions, not natural Game exit'}
    for sig, timeout in [(signal.SIGTERM, 8), (signal.SIGKILL, 2)]:
        if not gone(proc.pid):
            try:
                os.killpg(proc.pid, sig)
                record['signals'].append(sig.name)
            except ProcessLookupError:
                pass
        try:
            proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            pass
        if gone(proc.pid):
            break
    record.update(exit_code=proc.poll(), group_gone=gone(proc.pid))
    assert record['group_gone'] and record['exit_code'] is not None
    return record


class Run:
    def __init__(self, args):
        self.args = args
        self.meta, self.buttons = expand_inputs()
        self.case = self.meta['cases'][args.case]
        self.root = Path(args.source_root).resolve()
        self.binary = Path(args.binary).resolve()
        self.out = Path(args.output).resolve()
        self.fixture = HERE / self.meta['fixtures']['sram']['file']
        self.proc = self.sock = self.stream = None
        self.files = []
        self.frame = 0
        self.field_cry = []
        self.field_effect = []
        self.battle_cry = []
        self.flute = []
        assert self.binary.is_file() and not self.out.exists()
        assert sha(self.fixture) == self.meta['fixtures']['sram']['sha256']
        assert self.fixture.stat().st_size == 32768
        self.source_guard()
        self.result = {'case': args.case, 'declared_build_tree_head': self.case['recorded_source'],
                       'binary_sha256': sha(self.binary), 'seed': 42, 'Game_calls': 0,
                       'runtime_setters': 0, 'all_pass': False,
                       'scope': 'Literal replay; source checkout validation alone does not prove binary compiler provenance.'}

    def source_guard(self):
        actual = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=self.root, text=True).strip()
        assert actual == self.case['recorded_source'], ('check out recorded product source', actual)
        assert not subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=no'], cwd=self.root)

    def rpc(self, command, **kwargs):
        assert command in {'get_state', 'get_bag', 'get_flags', 'get_npcs', 'press_timeline', 'capture_frame'}
        self.stream.write((json.dumps({'cmd': command, **kwargs}) + '\n').encode())
        self.stream.flush()
        raw = self.stream.readline(4 * 1024 * 1024)
        assert raw.endswith(b'\n'), 'transport EOF/truncation'
        reply = json.loads(raw)
        assert reply.get('ok') is True, reply
        return reply.get('data')

    def read(self):
        state = self.rpc('get_state')
        assert state['frame_count'] == self.frame
        self.states.write(json.dumps(state) + '\n')
        self.states.flush()
        audio = state.get('audio_sfx') or {}
        if audio.get('playing') and audio.get('id') == 24:
            assert state['screen'] in {'overworld', 'battle'}
            if state['screen'] == 'overworld':
                self.field_cry.append(self.frame)
            else:
                assert 'WildReveal' in state['battle_phase']
                self.battle_cry.append(self.frame)
        if audio.get('playing') and audio.get('id') == 93:
            self.flute.append(self.frame)
        effect = state.get('script_effect') or {}
        if state['screen'] == 'overworld' and effect.get('effect') == 'PlayCry' and effect.get('species') == 'SNORLAX':
            self.field_effect.append(self.frame)
        return state

    def check(self, state):
        if self.frame == 615:
            assert state['screen'] == 'overworld' and state['map_name'] == 'Route12'
            assert (state['player_x'], state['player_y']) == (9, 62) and state['money'] == 1000
            assert state['party'][0]['species'] == 'Pidgey' and state['party'][0]['hp'] == state['party'][0]['max_hp'] == 50
            assert self.rpc('get_bag') == [{'item': 'PokeFlute', 'qty': 1}]
            assert self.rpc('get_flags') == self.meta['initial_flags']
            assert any(n['x'] == 10 and n['y'] == 62 and n['visible'] for n in self.rpc('get_npcs'))
        if self.frame in {657, 667, 677}:
            menu = state['field_menu']
            assert menu['kind'] == 'start' and menu['cursor'] == {657: 0, 667: 1, 677: 2}[self.frame]
            assert menu['items'][:3] == ['Pokedex', 'Pokemon', 'Item']
        if self.frame in {687, 697}:
            menu = state['field_menu']
            assert menu['kind'] == 'bag' and menu['cursor'] == 0
            assert menu['items'] == [{'item': 'PokeFlute', 'qty': 1}]
            assert menu['phase'] == ('Browsing' if self.frame == 687 else 'ActionMenu { cursor: 0 }')
        if self.frame == self.meta['capture_frame']:
            assert state['screen'] == 'overworld'
            assert state['dialogue_state']['text'] == self.case['capture_visible_text']
            image = self.out / (self.args.case + '.png')
            self.rpc('capture_frame', path=str(image))
            assert self.rpc('get_state') == state
            assert sha(image) == self.case['capture_sha256']
            self.result['capture'] = {'frame': self.frame, 'sha256': sha(image)}

    def play(self):
        self.out.mkdir()
        self.save = self.out / 'pokered.sav'
        shutil.copyfile(self.fixture, self.save)
        log = (self.out / 'game.log').open('x')
        self.states = (self.out / 'states.jsonl').open('x')
        self.inputs = (self.out / 'inputs.jsonl').open('x')
        self.files = [log, self.states, self.inputs]
        with socket.socket() as probe:
            probe.bind(('127.0.0.1', self.args.port))
        env = {key: value for key, value in os.environ.items() if not key.startswith('POKERED_')}
        argv = [str(self.binary), 'run', '--headless', '--speed', '0', '--seed', '42',
                '--debug-port', str(self.args.port), '--lang', 'en', '--no-audio', '--save', str(self.save)]
        self.proc = subprocess.Popen(argv, cwd=self.out, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        self.result.update(Game_calls=1, argv=argv, pid=self.proc.pid)
        put(self.out / 'result.json', self.result)
        for _ in range(200):
            assert self.proc.poll() is None, ('early Game exit', self.proc.returncode)
            try:
                self.sock = socket.create_connection(('127.0.0.1', self.args.port), timeout=.1)
                break
            except (ConnectionRefusedError, TimeoutError):
                time.sleep(.05)
        assert self.sock is not None
        self.sock.settimeout(30)
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.stream = self.sock.makefile('rwb')
        state = self.read()
        for button in self.buttons:
            # Fixed tape: these are ordinary fresh intro ACKs, never menu commands.
            if self.frame + 1 in self.meta['inputs']['ordinary_intro_ACK_input_frames']:
                phase = 'WildReveal' if self.frame + 1 == 1870 else 'PlayerSendOut'
                assert state['screen'] == 'battle' and state['battle_phase'] == f'Intro {{ phase: {phase}, wait_frames: 0 }}'
                assert button == 'a' and self.buttons[self.frame - 1] is None
            receipt = self.rpc('press_timeline', buttons=[button], start_at_frame=self.frame, advance=True)
            assert receipt == {'queue_start_frame': self.frame, 'start_frame': self.frame, 'end_frame': self.frame,
                               'frames': 1, 'padding_frames': 0, 'advanced': True, 'frame_count': self.frame + 1}
            self.inputs.write(json.dumps({'input_frame': self.frame, 'button': button, 'receipt': receipt}) + '\n')
            self.frame += 1
            state = self.read()
            self.check(state)
        assert self.frame == 1905 and state['screen'] == 'battle' and state['battle_phase'] == 'PlayerMenu'
        assert state['money'] == 1000 and self.rpc('get_bag') == [{'item': 'PokeFlute', 'qty': 1}]
        assert self.rpc('get_flags') == self.meta['final_flags']
        enemy, player = state['battle_live']['enemy'], state['battle_live']['player']
        assert (enemy['species'], enemy['level'], enemy['hp'], enemy['max_hp']) == ('Snorlax', 30, 137, 137)
        assert (player['species'], player['level'], player['hp'], player['max_hp']) == ('Pidgey', 20, 50, 50)
        assert self.field_cry == self.case['field_cry_frames']
        assert self.field_effect == self.case['field_PlayCry_effect_frames']
        assert self.battle_cry == self.case['battle_cry_frames'] == list(range(1735, 1751))
        assert self.flute == list(range(699, 1228))
        self.source_guard()
        assert sha(self.binary) == self.result['binary_sha256']
        self.result.update(all_pass=True, frames=self.frame, field_cry_frames=self.field_cry,
                           field_PlayCry_effect_frames=self.field_effect, battle_cry_frames=self.battle_cry)

    def finish(self):
        errors = []
        for resource in [self.stream, self.sock]:
            if resource is not None:
                try:
                    resource.close()
                except BaseException:
                    errors.append(traceback.format_exc())
        try:
            if self.proc is not None:
                self.result['owned_diagnostic_shutdown'] = stop(self.proc)
        except BaseException:
            errors.append(traceback.format_exc())
        for file in self.files:
            try:
                file.close()
            except BaseException:
                errors.append(traceback.format_exc())
        try:
            if hasattr(self, 'save'):
                assert sha(self.save) == self.meta['fixtures']['sram']['sha256']
                assert not self.save.with_suffix('.script_flags.json').exists()
        except BaseException:
            errors.append(traceback.format_exc())
        self.result.update(cleanup_errors=errors, all_pass=bool(self.result['all_pass'] and not errors))
        if self.out.exists():
            put(self.out / 'result.json', self.result)
        assert not errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', choices=['before', 'after'], required=True)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--source-root', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--port', type=int, default=19386)
    run = Run(parser.parse_args())

    def interrupt(*_):
        raise KeyboardInterrupt('received signal; clean up owned Game')

    signal.signal(signal.SIGTERM, interrupt)
    signal.signal(signal.SIGINT, interrupt)
    try:
        run.play()
    except BaseException:
        run.result.update(failure=traceback.format_exc(), all_pass=False)
        raise
    finally:
        run.finish()
    assert run.result['all_pass']


if __name__ == '__main__':
    main()

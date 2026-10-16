#!/usr/bin/env python3
"""Prepared only: one ordinary M9 cold-Continue discovery per approved case.

No Game starts without --execute-reviewed-before-only and an independently
frozen READY plan with actual codec/provider pins. Never run a candidate here.
"""
from pathlib import Path
import argparse, hashlib, json, os, shutil, signal, socket, struct, subprocess
import time, traceback

M9 = '25a9d8c6cac0aa4d662fcbac55aea718b637b038'
CASES = {
    'bike-sale': {'map': 'BikeShop', 'xy': [6, 4], 'question': "It's a cool BIKE!",
                  'preceding': ['Hi! Welcome to our BIKE'],
                  'options': ['BICYCLE ¥1000000', 'CANCEL']},
    'safari-admission': {'map': 'SafariZoneGate', 'xy': [4, 3],
                         'question': 'For just ¥500,',
                         'preceding': ['Welcome to the'], 'options': ['YES', 'NO']},
    'safari-return': {'map': 'SafariZoneCenter', 'xy': [14, 24],
                      'question': 'Leaving early?', 'preceding': [],
                      'options': ['YES', 'NO']},
}
MAX_BOOT = 2500
MAX_ROUTE = 180
MAX_QUESTION = 4000
FINAL_NEUTRAL = 64
PROBE_NEUTRAL = 32

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def write(p, value):
    Path(p).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')

def pinned(item):
    p = Path(item['path'])
    assert p.is_absolute() and p.is_file() and sha(p) == item['sha256'], item
    return p

def identity_field(value, path):
    assert isinstance(path, list) and path, 'actual identity field path must be declared'
    for component in path:
        value = value[component]
    return value

def fingerprint(p):
    return hashlib.sha256(os.fsencode(os.readlink(p))).hexdigest() if p.is_symlink() else sha(p)

def gone(pg):
    try:
        os.killpg(pg, 0)
    except ProcessLookupError:
        return True
    return False

def stop(proc):
    record = {'pid': proc.pid, 'pgid': proc.pid, 'signals': [],
              'scope': 'owned shutdown after diagnostic boundary, not natural success exit'}
    for sig, seconds in [(signal.SIGTERM, 8), (signal.SIGKILL, 2)]:
        if not gone(proc.pid):
            try:
                os.killpg(proc.pid, sig)
                record['signals'].append(sig.name)
            except ProcessLookupError:
                pass
        try:
            proc.wait(timeout=seconds)
        except subprocess.TimeoutExpired:
            pass
        if gone(proc.pid):
            break
    record.update(exit_code=proc.poll(), group_gone=gone(proc.pid))
    assert record['group_gone'] and record['exit_code'] is not None, record
    return record

def interrupt(signum, frame):
    raise KeyboardInterrupt(f'Received signal {signum}; enter owned cleanup')

class Discovery:
    def __init__(self, plan, case, output):
        self.plan, self.case, self.spec, self.out = plan, case, CASES[case], output
        self.proc = self.sock = self.stream = None
        self.files = []
        self.frame = self.seq = 0
        self.buttons = []
        self.state = None
        self.result = {'case': case, 'source': M9, 'candidate_executions': 0,
                       'seed': 42, 'seed_configurations': 'one normal CLI preboot --seed42',
                       'runtime_setters': 0, 'Game_calls': 0, 'all_pass': False,
                       'runner_pid': os.getpid(), 'runner_pgid': os.getpgrp(),
                       'events': [], 'captures': []}

    def guards(self):
        provider = self.plan['before_provider']
        assert provider['head'] == M9 and provider['kind'] == 'normal-CLI-debug-server'
        pinned(provider['binary'])
        identity = json.loads(pinned(provider['identity']).read_text())
        assert identity_field(identity, provider['identity_head_path']) == M9
        assert identity_field(identity, provider['identity_binary_sha_path']) == provider['binary']['sha256']
        freeze = json.loads(pinned(provider['source_guard']).read_text())
        root = Path(freeze['cwd'])
        assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip() == M9
        assert subprocess.check_output(['git', 'status', '--porcelain=v1'], cwd=root) == b''
        roster = subprocess.check_output(['git', 'ls-files', '-z'], cwd=root)
        assert hashlib.sha256(roster).hexdigest() == freeze['git_roster_sha256']
        for p, h in freeze['source'].items():
            assert fingerprint(root / p) == h, ('source changed', p)
        for key, base in [('engine981', freeze['engine_cwd']), ('gfx702', freeze['gfx_canonical'])]:
            for p, h in freeze[key].items():
                assert fingerprint(Path(base) / p) == h, (key, p)
        fixture = self.plan['fixtures'][self.case]
        assert pinned(fixture['sram']).stat().st_size == 32768
        codec = json.loads(pinned(fixture['codec_result']).read_text())
        assert codec['case'] == self.case and codec['import_checksum_valid']
        assert codec['export_import_reexport_byte_identical'] and codec['no_other_semantic_changes']

    def persist(self):
        write(self.out / 'result.json', self.result)

    def rpc(self, command, **kwargs):
        assert command in {'get_state', 'get_flags', 'get_bag', 'press_timeline', 'capture_frame'}
        request = {'cmd': command, **kwargs}
        self.seq += 1
        self.rpc_log.write(json.dumps({'seq': self.seq, 'frame': self.frame, 'request': request}) + '\n')
        self.rpc_log.flush()
        self.stream.write((json.dumps(request) + '\n').encode())
        self.stream.flush()
        raw = self.stream.readline(4 * 1024 * 1024)
        assert raw.endswith(b'\n'), ('transport EOF/truncation', raw[:200])
        reply = json.loads(raw)
        self.rpc_log.write(json.dumps({'seq': self.seq, 'frame': self.frame, 'response': reply}) + '\n')
        self.rpc_log.flush()
        assert reply.get('ok') is True, reply
        return reply.get('data')

    def read(self):
        self.state = self.rpc('get_state')
        assert self.state['frame_count'] == self.frame
        self.state_log.write(json.dumps(self.state, ensure_ascii=False) + '\n')
        self.state_log.flush()
        return self.state

    def step(self, button):
        assert button in {None, 'a', 'b', 'up', 'down', 'start'}
        receipt = self.rpc('press_timeline', buttons=[button], start_at_frame=self.frame, advance=True)
        assert receipt == {'queue_start_frame': self.frame, 'start_frame': self.frame,
                           'end_frame': self.frame, 'frames': 1, 'padding_frames': 0,
                           'advanced': True, 'frame_count': self.frame + 1}, receipt
        self.buttons.append(button)
        self.button_log.write(json.dumps({'input_frame': self.frame, 'button': button,
                                         'receipt': receipt}) + '\n')
        self.button_log.flush()
        self.frame += 1
        return self.read()

    def frames(self, button, count):
        for _ in range(count):
            self.step(button)

    def capture(self, reason):
        path = self.out / f'{self.frame:05}-{reason}.png'
        assert not path.exists()
        receipt = self.rpc('capture_frame', path=str(path))
        assert self.rpc('get_state') == self.state, 'capture mutated public state'
        raw = path.read_bytes()
        assert raw[:8] == b'\x89PNG\r\n\x1a\n' and struct.unpack('>II', raw[16:24]) == (160, 144)
        self.result['captures'].append({'frame': self.frame, 'reason': reason, 'path': str(path),
                                        'sha256': sha(path), 'receipt': receipt, 'state': self.state})

    def start(self):
        self.guards()
        fixture = self.plan['fixtures'][self.case]['sram']
        self.save = self.out / 'pokered.sav'
        shutil.copyfile(pinned(fixture), self.save)
        assert not self.save.with_suffix('.script_flags.json').exists()
        self.game_log = (self.out / 'game.log').open('x')
        self.rpc_log = (self.out / 'rpc.jsonl').open('x')
        self.state_log = (self.out / 'states.jsonl').open('x')
        self.button_log = (self.out / 'inputs.jsonl').open('x')
        self.files = [self.game_log, self.rpc_log, self.state_log, self.button_log]
        port = self.plan['ports'][self.case]
        with socket.socket() as probe:
            probe.bind(('127.0.0.1', port))
        env = os.environ.copy()
        for key in list(env):
            if key.startswith('POKERED_'):
                env.pop(key)
        runtime_environment = self.plan.get('runtime_environment', {})
        assert set(runtime_environment) <= {'LD_LIBRARY_PATH'}, 'no provider override environment'
        env.update(runtime_environment)
        argv = [self.plan['before_provider']['binary']['path'], 'run', '--headless', '--speed', '0',
                '--seed', '42', '--debug-port', str(port), '--lang', 'en', '--no-audio', '--save', str(self.save)]
        self.proc = subprocess.Popen(argv, cwd=self.out, env=env, stdout=self.game_log,
                                     stderr=subprocess.STDOUT, start_new_session=True)
        self.result.update(Game_calls=1, argv=argv, Game_pid=self.proc.pid, Game_pgid=self.proc.pid)
        self.persist()
        for _ in range(200):
            assert self.proc.poll() is None, ('early Game exit', self.proc.returncode)
            try:
                self.sock = socket.create_connection(('127.0.0.1', port), timeout=.1)
                break
            except (ConnectionRefusedError, TimeoutError):
                time.sleep(.05)
        assert self.sock is not None
        self.sock.settimeout(30)
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.stream = self.sock.makefile('rwb')
        self.read()
        self.capture('cold-boot')

    def boot_and_route(self):
        for _ in range(MAX_BOOT):
            screen = self.state['screen']
            if screen == 'overworld':
                break
            # Both are real ordinary boot screens. Splash ignores early
            # presses and accepts fresh Start only in interruptible phases;
            # IntroScene accepts a fresh ordinary button. No debug skip API.
            if screen in {'gamefreak-splash', 'copyright', 'intro', 'title'}:
                button = 'start' if self.frame % 60 < 2 else None
            elif screen in {'main-menu', 'main_menu', 'mainmenu', 'language-select'}:
                button = 'a' if self.frame % 8 == 0 else None
            else:
                raise AssertionError(('unexpected boot screen; failed physical Continue', screen))
            self.step(button)
        else:
            raise AssertionError('boot/Continue frame budget exhausted')
        s = self.state
        assert s['map_name'] == self.spec['map'] and [s['player_x'], s['player_y']] == self.spec['xy']
        assert s['player_name'] == 'RED' and s['money'] == 1000
        assert len(s['party']) == 1 and s['party'][0]['species'] == 'Pidgey'
        assert s['party'][0]['level'] == 20 and s['party'][0]['hp'] == s['party'][0]['max_hp']
        assert self.rpc('get_bag') == []
        initial_flags = self.rpc('get_flags')
        assert not initial_flags.get('EVENT_GOT_BICYCLE', False)
        assert not initial_flags.get('EVENT_SAFARI_GAME_OVER', False)
        assert bool(initial_flags.get('EVENT_IN_SAFARI_ZONE', False)) == (self.case == 'safari-return')
        self.result['initial_flags'] = initial_flags
        self.frames(None, 32)
        assert not self.state['script_running'] and self.state['dialogue_state'] is None
        if self.case == 'bike-sale':
            self.frames('up', 2)
            self.frames(None, 18)
            assert [self.state['player_x'], self.state['player_y']] == [6, 4]
            assert self.state['player_facing'] == 'Up'
            self.frames('a', 3)  # one ordinary field press across its sampling phase
            assert self.state['script_running'], 'one physical talk press was not accepted'
        else:
            direction = 'up' if self.case == 'safari-admission' else 'down'
            for _ in range(MAX_ROUTE):
                if self.case == 'safari-admission' and self.state['script_running']:
                    break
                if self.case == 'safari-return' and self.state['map_name'] == 'SafariZoneGate':
                    break
                assert self.state['screen'] == 'overworld' and self.state['choice'] is None
                self.step(direction)
            else:
                raise AssertionError('ordinary step/return-warp budget exhausted')
        self.capture('route-handoff')

    def target(self):
        return (self.state.get('script_effect') or {}).get('text', '').startswith(self.spec['question'])

    def find_final_question(self):
        earlier_acks = 0
        deadline = self.frame + MAX_QUESTION
        while self.frame < deadline:
            s = self.state
            assert s['screen'] == 'overworld' and s['choice'] is None, ('unexpected early choice/screen', s)
            effect = s.get('script_effect') or {}
            d = s['dialogue_state']
            if self.target():
                if d and d['page'] == d['total_pages']:
                    if d['waiting_for_input'] and not d['holding_open']:
                        self.result.update(final_question_frame=self.frame, earlier_page_acknowledgements=earlier_acks)
                        return
                    self.step(None)  # never ACK the target final page while finding it
                elif d and d['waiting_for_input']:
                    assert self.frame + 8 <= deadline, 'question frame budget before page pulse'
                    earlier_acks += 1
                    self.step('a')
                    self.frames(None, 7)
                else:
                    self.step(None)
            else:
                t = effect.get('text')
                if t:
                    assert any(t.startswith(prefix) for prefix in self.spec['preceding']), ('unknown preceding text', t)
                assert s.get('active_script_effect') in {None, 'ShowDialogue', 'PrintFieldText',
                    'MovePlayerRelative', 'FacePlayer', 'ShowMoneyBox', 'Immediate'}, effect
                if d and d['waiting_for_input']:
                    assert self.frame + 8 <= deadline, 'question frame budget before welcome pulse'
                    earlier_acks += 1
                    self.step('a')
                    self.frames(None, 7)
                else:
                    self.step(None)
        raise AssertionError('target final question frame budget exhausted')

    def observe_extra_ack(self):
        f = self.frame
        self.capture('final-question-ready')
        xy = [self.state['player_x'], self.state['player_y']]
        for i in range(1, FINAL_NEUTRAL + 1):
            self.step(None)
            d = self.state['dialogue_state']
            assert self.target() and d and d['waiting_for_input'] and d['page'] == d['total_pages']
            assert not d['holding_open'] and self.state['choice'] is None
            assert [self.state['player_x'], self.state['player_y']] == xy and self.state['money'] == 1000
            if i in {1, 2, 12, 32, 64}:
                self.capture(f'neutral-Fplus{i}')
        prefix = list(self.buttons)
        write(self.out / 'question-neutral-prefix.json', prefix)
        # Deliberately prove the extra key only after freezing the no-ACK prefix.
        # Candidate screenshot replay uses the prefix ending F+64; it must not
        # adapt to its earlier menu or inherit this baseline-only ACK probe.
        self.step('a')
        self.frames(None, PROBE_NEUTRAL)
        ch = self.state['choice']
        assert ch and ch['options'] == self.spec['options'] and ch['selected'] == 0
        assert self.state['dialogue_state'] is None
        assert self.state['money'] == 1000 and [self.state['player_x'], self.state['player_y']] == xy
        self.capture('one-extra-A-opens-choice')
        write(self.out / 'actual-discovery-buttons.json', self.buttons)
        assert self.buttons == prefix + ['a'] + [None] * PROBE_NEUTRAL
        flags = self.rpc('get_flags')
        assert flags == self.result['initial_flags'], 'discovery must not answer the menu or mutate story flags'
        self.result.update(all_pass=True, actual_frames=self.frame, final_question_frame=f,
                           final_state=self.state, final_flags=flags,
                           candidate_replay_prefix={'path': str(self.out / 'question-neutral-prefix.json'),
                              'sha256': sha(self.out / 'question-neutral-prefix.json'), 'frames': len(prefix)},
                           baseline_only_ack_probe={'A_frames': 1, 'neutral_frames': PROBE_NEUTRAL},
                           actual_buttons_sha256=sha(self.out / 'actual-discovery-buttons.json'))

    def finish(self):
        errors = []
        for label, resource in [('stream', self.stream), ('socket', self.sock)]:
            if resource is not None:
                try:
                    resource.close()
                except BaseException:
                    errors.append({'operation': label + '.close', 'error': traceback.format_exc()})
        try:
            if self.proc is not None:
                self.result['owned_shutdown'] = stop(self.proc)
        finally:
            try:
                self.guards()
                if hasattr(self, 'save'):
                    fixture = self.plan['fixtures'][self.case]['sram']
                    assert sha(self.save) == fixture['sha256']
                    assert not self.save.with_suffix('.script_flags.json').exists()
                self.result['source_and_fixture_guard_after'] = True
            except BaseException:
                errors.append({'operation': 'after-guards', 'error': traceback.format_exc()})
            for file in self.files:
                try:
                    file.close()
                except BaseException:
                    errors.append({'operation': 'log.close', 'error': traceback.format_exc()})
            write(self.out / 'all-accepted-buttons-including-failure.json', self.buttons)
            shutdown_ok = self.proc is None or self.result.get('owned_shutdown', {}).get('group_gone')
            self.result['seam_assertions_passed_before_cleanup'] = self.result['all_pass']
            self.result['all_pass'] = bool(self.result['all_pass'] and not errors and shutdown_ok)
            self.result.update(cleanup_errors=errors, last_frame=self.frame, last_state=self.state,
                               controller_sha256=sha(__file__), rpc_count=self.seq)
            self.persist()
        assert not errors and shutdown_ok

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--plan', required=True)
    parser.add_argument('--plan-sha256', required=True)
    parser.add_argument('--case', choices=CASES, required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--execute-reviewed-before-only', action='store_true', required=True)
    args = parser.parse_args()
    assert sha(args.plan) == args.plan_sha256
    plan = json.loads(Path(args.plan).read_text())
    assert plan['state'] == 'READY' and plan['root_authorized_before_only'] is True
    assert plan['before_head'] == M9 and plan['controller_sha256'] == sha(__file__)
    assert plan['policy'] == {'max_boot': MAX_BOOT, 'max_route': MAX_ROUTE,
                              'max_question': MAX_QUESTION, 'final_neutral': FINAL_NEUTRAL,
                              'probe_neutral': PROBE_NEUTRAL, 'seed': 42}
    output = Path(args.output)
    assert output.is_absolute() and not output.exists()
    output.mkdir()
    discovery = Discovery(plan, args.case, output)
    signal.signal(signal.SIGTERM, interrupt)
    signal.signal(signal.SIGINT, interrupt)
    error = None
    try:
        discovery.start()
        discovery.boot_and_route()
        discovery.find_final_question()
        discovery.observe_extra_ack()
    except BaseException:
        error = traceback.format_exc()
        discovery.result['first_failure'] = error
        discovery.result['all_pass'] = False
    finally:
        discovery.finish()
    assert error is None and discovery.result['all_pass'], error

if __name__ == '__main__':
    main()

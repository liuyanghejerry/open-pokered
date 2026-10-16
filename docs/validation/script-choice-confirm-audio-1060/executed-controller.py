#!/usr/bin/env python3
"""Fixed, ordinary-input choice confirmation audio pair. Import/check-plan never launches Game."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import time
import traceback

ROOT = Path('/home/jerry/develop/open-pokered')
R = ROOT / '.task-tmp/fidelity-39-plus'
PLAN = R / 'audit-1060-choice-confirm-fixed-native-plan-v2.json'
PLAN_SHA = '670a577f842b8549414a8f0926e696dee9dd8372ceaf2d7b4ca50b49e7c96bad'
GATE = 'EVENT_CINNABAR_GYM_GATE1_UNLOCKED'
BEATEN = 'EVENT_BEAT_CINNABAR_GYM_TRAINER_1'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def load(path):
    return json.loads(Path(path).read_text())

def write(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')

def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

def gone(pg):
    try:
        os.killpg(pg, 0)
    except ProcessLookupError:
        return True
    return False

def git(path, *args):
    return subprocess.check_output(['git', '-C', str(path), *args])

def check_plan():
    assert sha(PLAN) == PLAN_SHA
    plan = load(PLAN)
    f, v = plan['fixture'], plan['buttons']
    assert sha(f['path']) == f['sha256'] and Path(f['path']).stat().st_size == 32768
    assert sha(v['path']) == v['sha256']
    buttons = load(v['path'])
    literal = (['a'] + [None] * 7) * 70 + [None] * 32 + ['up'] * 2 + [None] * 4 + ['a'] * 2 + [None] * 4 + (['b'] + [None] * 31) * 44
    assert buttons == literal and len(buttons) == 2012
    assert sha(plan['prefix']['historical_raw_path']) == plan['prefix']['historical_raw_sha256']
    assert plan['prefix']['all_public_state_npcs_flags_equal_inclusive'] == [0, 1020]
    return plan, buttons

def verify_bytes(base, records):
    for name, record in records.items():
        p = Path(base) / name
        data = os.fsencode(os.readlink(p)) if p.is_symlink() else p.read_bytes()
        digest = record if isinstance(record, str) else record['sha256']
        assert hashlib.sha256(data).hexdigest() == digest, (base, name)
        if isinstance(record, dict):
            assert len(data) == record['bytes'], (base, name, 'size')

def normalized(records):
    return {x['path']: x for x in records} if isinstance(records, list) else records

def verify_roster(base, head, records):
    records = normalized(records)
    actual = {}
    for row in git(base, 'ls-tree', '-rz', '--full-tree', head).split(b'\0'):
        if row:
            meta, name = row.split(b'\t')
            mode, kind, blob = meta.decode().split()
            actual[os.fsdecode(name)] = dict(mode=mode, git_blob=blob)
    assert actual == {n:dict(mode=v['mode'], git_blob=v['git_blob']) for n,v in records.items()}, (base, 'Git roster drift')
    verify_bytes(base, records)
    return len(records)

def guards(plan, ready):
    records = {}
    for label, spec in plan['providers'].items():
        path = spec['identity']
        assert sha(path) == ready['identity_sha256'][label] == spec['identity_sha256'], (label, 'identity drift')
        identity = load(path)
        actual_head = identity.get('actual_game_source', identity.get('head'))
        binary = identity.get('cli', identity.get('binary'))
        binary_sha = identity.get('cli_sha256', identity.get('binary_sha256'))
        assert actual_head == spec['head']
        assert sha(binary) == binary_sha
        assert sha(identity['source_freeze']) == identity['source_freeze_sha256']
        frozen = load(identity['source_freeze'])
        assert frozen['head'] == spec['head']
        w = Path(frozen['worktree'])
        assert git(w, 'rev-parse', 'HEAD').decode().strip() == spec['head']
        assert git(w, 'status', '--porcelain') == b''
        count = verify_roster(w, spec['head'], frozen['all_tracked'])
        engine = Path(frozen['engine_path'])
        assert git(engine, 'rev-parse', 'HEAD').decode().strip() == frozen['engine_head'] == identity['engine_head']
        assert git(engine, 'status', '--porcelain').decode().strip() == '?? .cargo-ok'
        engine_count = verify_roster(engine, frozen['engine_head'], frozen.get('engine_tracked', frozen.get('engine_all_tracked')))
        assert engine_count == 981
        marker = engine / '.cargo-ok'
        assert marker.is_file() and marker.stat().st_size == 0 and sha(marker) == hashlib.sha256(b'').hexdigest()
        gfx = normalized(frozen.get('gfx', frozen.get('external_gfx')))
        verify_bytes(ROOT / 'gfx', gfx)
        assert len(gfx) == 702
        records[label] = dict(identity=path, identity_sha256=sha(path), binary=binary, binary_sha256=sha(binary), actual_source=spec['head'], guard=dict(head=spec['head'], source_freeze_sha256=sha(identity['source_freeze']), tracked_count=count, engine_count=engine_count, gfx_count=len(gfx), marker_sha256=sha(marker)))
    return records

class Actor:
    def __init__(self, label, port, directory, plan, identity, report, actors):
        self.label, self.frame, self.seq = label, 0, 0
        self.proc = self.sock = self.stream = None
        self.files = []
        self.events = {}
        self.plan, self.report = plan, report
        self.dir = directory / label
        self.dir.mkdir()
        self.save = self.dir / 'ordinary.sav'
        self.save.write_bytes(Path(plan['fixture']['path']).read_bytes())
        assert not self.save.with_suffix('.script_flags.json').exists()
        self.record = dict(identity=identity, events=self.events)
        report['actors'][label] = self.record
        actors.append(self)  # register ownership before process creation
        for name in ['game.log', 'rpc.jsonl', 'states.jsonl', 'npcs.jsonl', 'flags.jsonl']:
            f = (self.dir / name).open('x')
            self.files.append(f)
        self.game_log, self.rpc_log, self.state_log, self.npc_log, self.flag_log = self.files
        argv = [identity['binary'], 'run', '--headless', '--speed', '0', '--seed', '42', '--debug-port', str(port), '--lang', 'en', '--no-audio', '--save', str(self.save)]
        env = os.environ.copy()
        env.pop('POKERED_MAPS_DIR', None)
        env['LD_LIBRARY_PATH'] = '/tmp/pokered-build-lib'
        self.record.update(argv=argv, cwd=str(self.dir), env_overrides={'LD_LIBRARY_PATH': env['LD_LIBRARY_PATH'], 'POKERED_MAPS_DIR': 'removed'}, physical_preboot_sram_sha256=sha(self.save))
        self.proc = subprocess.Popen(argv, cwd=self.dir, env=env, stdout=self.game_log, stderr=subprocess.STDOUT, start_new_session=True)
        self.record.update(game_pid=self.proc.pid, game_pgid=self.proc.pid)
        write(directory / 'result.json', report)
        print(json.dumps(dict(event='game-started', provider=label, game_pid=self.proc.pid, runner_pid=os.getpid())), flush=True)
        for _ in range(200):
            assert self.proc.poll() is None, ('premature Game exit', self.proc.returncode)
            try:
                self.sock = socket.create_connection(('127.0.0.1', port), timeout=.1)
                break
            except (ConnectionRefusedError, TimeoutError):
                time.sleep(.05)
        assert self.sock is not None, 'No debug server; no process retry'
        self.sock.settimeout(30)
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.stream = self.sock.makefile('rwb')
        self.observe()

    def rpc(self, cmd, **kwargs):
        assert cmd in self.plan['allowed_rpc']
        self.seq += 1
        request = dict(cmd=cmd, **kwargs)
        self.rpc_log.write(json.dumps(dict(seq=self.seq, request=request)) + '\n')
        self.rpc_log.flush()
        self.stream.write((json.dumps(request) + '\n').encode())
        self.stream.flush()
        raw = self.stream.readline(4 * 1024 * 1024)
        assert raw.endswith(b'\n'), 'Truncated RPC; no retry'
        response = json.loads(raw)
        self.rpc_log.write(json.dumps(dict(seq=self.seq, response=response)) + '\n')
        self.rpc_log.flush()
        assert response.get('ok') is True, response
        return response.get('data')

    def observe(self):
        self.state = self.rpc('get_state')
        assert isinstance(self.state, dict) and sorted(self.state) == self.plan['every_frame']['state_top_level_keys'], 'Unknown get_state schema'
        assert self.state['frame_count'] == self.frame
        self.npcs, self.flags = self.rpc('get_npcs'), self.rpc('get_flags')
        assert isinstance(self.npcs, list) and isinstance(self.flags, dict)
        for npc in self.npcs:
            assert sorted(npc) == self.plan['every_frame']['npc_exact_keys'], 'Unknown get_npcs schema'
            assert type(npc['visible']) is bool and isinstance(npc['facing'], str)
            for k in ['npc_index', 'text_id', 'sprite_id', 'x', 'y', 'home_x', 'home_y', 'walk_counter', 'scripted_path_remaining']:
                assert type(npc[k]) is int and npc[k] >= 0, (k, npc)
        assert all(type(v) is bool for v in self.flags.values()), 'Unknown get_flags schema'
        for file, value in [(self.state_log, self.state), (self.npc_log, self.npcs), (self.flag_log, self.flags)]:
            file.write(json.dumps(dict(frame=self.frame, data=value), ensure_ascii=False) + '\n')
            file.flush()
        if self.frame < 604:
            return
        targets = [n for n in self.npcs if n['text_id'] == 3 and n['visible']]
        assert len(targets) == 1, ('NPC identity not unique', targets)
        target = targets[0]
        xy = [target['x'], target['y']]
        moving = target['walk_counter'] > 0 or target['scripted_path_remaining'] > 0
        effect = self.state.get('script_effect') or {}
        prebattle = effect.get('effect') == 'ShowDialogue' and effect.get('text') == self.plan['expected_sequence']['prebattle_text']
        battle = self.state['screen'] == 'battle'
        assert not self.flags.get(GATE, False) and not self.flags.get(BEATEN, False), 'No win inputs: gate/trainer flags must stay unset'
        if self.frame == 604:
            assert xy == [17, 8] and target['walk_counter'] == 0
            assert (self.state['player_x'], self.state['player_y'], self.state['player_facing']) == (15, 8, 'Up')
            assert 'POKéMON Quiz!' in effect.get('text', '')
        audio = self.state['audio_sfx']
        assert sorted(audio) == ['id', 'playing'] and type(audio['id']) is int and type(audio['playing']) is bool
        if self.state['choice'] is not None:
            self.events.setdefault('first_choice', self.frame)
        if self.frame == 1021:
            assert self.state['choice'] is None and self.state['active_script_effect'] == 'WaitSound'
            expected = self.plan['prefix']['expected_' + self.label + '_audio']
            assert audio == expected, (self.label, self.frame, audio, expected)
            self.events['choice_confirmation'] = self.frame
        if self.frame > 1021 and audio['id'] == 80 and audio['playing']:
            self.events.setdefault('first_denied_playing', self.frame)
        if target['walk_counter'] > 0:
            self.events.setdefault('first_nonzero_walk_counter', self.frame)
        if 'first_nonzero_walk_counter' in self.events and not moving and xy == [16, 8]:
            self.events.setdefault('route_complete', self.frame)
        if moving:
            assert not prebattle and not battle, 'Text/battle before real route completion'
        if prebattle:
            assert 'route_complete' in self.events and xy == [16, 8] and not moving
            self.events.setdefault('first_prebattle', self.frame)
            dialogue = self.state.get('dialogue_state') or {}
            if dialogue.get('waiting_for_input') and dialogue.get('page') == dialogue.get('total_pages'):
                self.events.setdefault('prebattle_final_page_ready', self.frame)
        if battle:
            self.events.setdefault('first_battle', self.frame)
            assert 'first_prebattle' in self.events and 'route_complete' in self.events

    def step(self, button):
        f = self.frame
        receipt = self.rpc('press_timeline', buttons=[button], start_at_frame=f, advance=True)
        assert receipt == dict(queue_start_frame=f, start_frame=f, end_frame=f, frames=1, padding_frames=0, advanced=True, frame_count=f + 1), receipt
        self.frame += 1
        self.observe()

    def capture(self):
        path = self.dir / f'frame-{self.frame:04d}.png'
        assert not path.exists()
        receipt = self.rpc('capture_frame', path=str(path))
        png = path.read_bytes()
        assert png[:8] == b'\x89PNG\r\n\x1a\n' and struct.unpack('>II', png[16:24]) == (160, 144)
        post = dict(state=self.rpc('get_state'), npcs=self.rpc('get_npcs'), flags=self.rpc('get_flags'))
        assert post['state'] == self.state, 'capture_frame changed public state'
        assert post['npcs'] == self.npcs, 'capture_frame changed public NPCs'
        assert post['flags'] == self.flags, 'capture_frame changed public flags'
        return dict(provider=self.label, frame=self.frame, path=str(path), sha256=sha(path), rpc_receipt=receipt,
                    post_capture_readonly=post, public_state_npcs_flags_unchanged=True)

    def finish(self):
        if self.stream:
            self.stream.close()
        if self.sock:
            self.sock.close()
        if self.proc and self.proc.poll() is None:
            os.killpg(self.proc.pid, signal.SIGTERM)
        if self.proc:
            self.record.update(game_exit_code=self.proc.wait(), game_group_gone=gone(self.proc.pid), shutdown='Owned SIGTERM only after full fixed vector or first error; no restart')
            assert self.record['game_group_gone']
        self.record.update(frames=self.frame, rpc_count=self.seq, fixture_unchanged=sha(self.save) == self.plan['fixture']['sha256'], no_companion=not self.save.with_suffix('.script_flags.json').exists())
        assert self.record['fixture_unchanged'] and self.record['no_companion']
        for f in self.files:
            f.close()

def run(args):
    plan, buttons = check_plan()
    assert args.ready and args.output
    ready = load(args.ready)
    assert ready['root_approved_for_execution'] is True
    assert ready['plan_sha256'] == sha(PLAN) and ready['controller_sha256'] == sha(__file__)
    initial_guards = guards(plan, ready)
    out = Path(args.output).resolve()
    assert not out.exists()
    out.mkdir()
    (out / 'buttons.json').write_bytes(Path(plan['buttons']['path']).read_bytes())
    report = dict(complete=False, all_pass=False, started_utc=now(), runner_pid=os.getpid(), runner_pgid=os.getpgrp(), plan_sha256=sha(PLAN), controller_sha256=sha(__file__), ready_sha256=sha(args.ready), allowed_rpc=plan['allowed_rpc'], actual_sources={k:v['head'] for k,v in plan['providers'].items()}, source_before=initial_guards, actors={}, captures=[])
    actors = []
    write(out / 'result.json', report)
    try:
        for port in plan['launch']['ports']:
            with socket.socket() as probe:
                probe.bind(('127.0.0.1', port))
        for (label, identity), port in zip(initial_guards.items(), plan['launch']['ports']):
            Actor(label, port, out, plan, identity, report, actors)
        before, after = actors
        report['initial_bags'] = {x.label:x.rpc('get_bag') for x in actors}
        assert report['initial_bags'] == {'before': [], 'after': []}
        conserved_at_604 = {}
        for frame in range(2013):
            if frame:
                for actor in actors:
                    actor.step(buttons[frame - 1])
            if frame <= 1020:
                assert before.state == after.state, ('full public state prefix mismatch', frame)
                assert before.npcs == after.npcs and before.flags == after.flags, ('NPC/flags prefix mismatch', frame)
            fields = plan['conserved_public_fields']
            assert {k:before.state[k] for k in fields} == {k:after.state[k] for k in fields}, ('cross-provider conserved fields', frame)
            if frame == 604:
                conserved_at_604 = {x.label:{k:x.state[k] for k in fields} for x in actors}
            if frame >= 604:
                for actor in actors:
                    assert {k:actor.state[k] for k in fields} == conserved_at_604[actor.label], ('HP/PP/item-related public fields changed', actor.label, frame)
            if frame in plan['fixed_capture_frames']:
                report['captures'].extend(x.capture() for x in actors)
            if frame % 128 == 0:
                write(out / 'result.json', report)
        report['fixed_b_ack_frames'] = {}
        for actor in actors:
            events = actor.events
            assert events['choice_confirmation'] == 1021
            assert 1021 < events['first_denied_playing'] < events['first_nonzero_walk_counter'] < events['route_complete'] <= events['first_prebattle'] < events['first_battle'] <= 2012
            assert 'prebattle_final_page_ready' in events
            b_frames = [f for f in range(events['first_prebattle'] + 1, events['first_battle'] + 1) if buttons[f - 1] == 'b']
            assert b_frames, 'Actual battle must follow an ordinary fixed B acknowledgement'
            report['fixed_b_ack_frames'][actor.label] = b_frames
        report['final_bags'] = {x.label:x.rpc('get_bag') for x in actors}
        assert report['final_bags'] == report['initial_bags']
        report.update(all_pass=True, frames=2012, full_public_prefix_equal_inclusive=[0, 1020], no_direct_rng_observation=True)
    except BaseException:
        report.update(error=traceback.format_exc(), first_failure_preserved=True)
        print(report['error'], flush=True)
    finally:
        for actor in actors:
            try:
                actor.finish()
            except BaseException:
                report.update(all_pass=False, shutdown_error=traceback.format_exc())
        try:
            report['source_after'] = guards(plan, ready)
            assert report['source_after'] == initial_guards
        except BaseException:
            report.update(all_pass=False, final_guard_error=traceback.format_exc())
        artifacts = {}
        for path in sorted(out.rglob('*')):
            if path.is_file() and path.name != 'result.json':
                artifacts[str(path.relative_to(out))] = dict(sha256=sha(path), bytes=path.stat().st_size)
        report.update(complete=True, artifacts=artifacts, finished_utc=now(), runner_group_requires_external_terminal_check=True)
        write(out / 'result.json', report)
    print(json.dumps(dict(result=str(out/'result.json'), sha256=sha(out/'result.json'), all_pass=report['all_pass'], game_groups_gone={x.label:x.record.get('game_group_gone') for x in actors})), flush=True)
    return 0 if report['all_pass'] and len(actors) == 2 else 1

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--check-plan', action='store_true')
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--ready')
    parser.add_argument('--output')
    args = parser.parse_args()
    assert args.check_plan != args.execute, 'Choose source-only --check-plan or explicitly authorized --execute'
    if args.check_plan:
        plan, buttons = check_plan()
        print(json.dumps(dict(source_only=True, Game=0, Cargo=0, frames=len(buttons), plan_sha256=sha(PLAN), controller_sha256=sha(__file__), after_identity_exists=Path(plan['providers']['after']['identity']).exists())))
        return 0
    return run(args)

if __name__ == '__main__':
    raise SystemExit(main())

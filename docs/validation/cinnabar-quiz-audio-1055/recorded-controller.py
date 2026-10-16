#!/usr/bin/env python3
"""One fixed-vector pair; physical cold SRAM, ordinary buttons, immutable observations."""
import argparse, datetime, hashlib, json, os, pathlib, signal, socket, struct, subprocess, sys, time, traceback
R=pathlib.Path('/home/jerry/develop/open-pokered');O=R/'.task-tmp/fidelity-39-plus';I=O/'audit-1055-quiz-master-pair-preparation-v1';GATE='EVENT_CINNABAR_GYM_GATE1_UNLOCKED'
def sha(p): return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def load(p): return json.loads(pathlib.Path(p).read_text())
def write(p,x): pathlib.Path(p).write_text(json.dumps(x,indent=2,ensure_ascii=False)+'\n')
def gone(p):
    try: os.killpg(p,0)
    except ProcessLookupError: return True
    return False
def effect_text(s): return json.dumps((s.get('script_effect') or {}).get('text'),ensure_ascii=False)
def brief(s): return {k:s.get(k) for k in ['frame_count','screen','map_name','player_x','player_y','player_facing','active_script_effect','script_effect','dialogue','dialogue_state','choice','audio_sfx','script_running','battle_phase']}
P=argparse.ArgumentParser();P.add_argument('--case',required=True,choices=['locked-correct','locked-wrong','already-open-correct']);P.add_argument('--output',required=True);a=P.parse_args();D=pathlib.Path(a.output).resolve();assert not D.exists();D.mkdir()
plan=load(I/'plan.json');assert sha(I/'plan.json')=='dd1a147c297be3832f2aaf6a7004a2ea44e7e08d4510f787cce1e9e32b7ec64a';case=next(x for x in plan['scenarios'] if x['name']==a.case);fixture=plan['fixtures'][case['fixture']];vector=plan['fixed_button_vectors'][case['buttons']];buttons=load(vector['path']);assert len(buttons)==2012 and sha(vector['path'])==vector['sha256'] and sha(fixture['path'])==fixture['sha256']
identities={'before':O/'audit-1055-quiz-master89ef-identity.json','after':O/'audit-1055-quiz-integrated-identity.json'};expected={'before':'89ef61e096387964897fed2c47f067b9b058097a','after':'476f064a7ae208c3f313bd4d4060a15851e5f58b'}
report=dict(complete=False,all_pass=False,case=a.case,runner_pid=os.getpid(),runner_pgid=os.getpgrp(),controller=str(pathlib.Path(__file__).resolve()),controller_sha256=sha(__file__),plan_sha256=sha(I/'plan.json'),fixture=fixture,vector=vector,actual_sources=expected,allowed_rpc=['get_state','get_flags','get_bag','press_timeline','capture_frame'],scope='Actual89ef versus ordinary-integrated476f, each ownembeddedmaps, samephysicalprebootSRAM andsamefixed2012ordinarybuttons. No snapshot/restore/setter/warp/skip/battleinjection, no seedfitting/restart. Allfirstcyclecues carry quizordinal; latercycles/battle are retained but not firstcue evidence.',started_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),actors={},captures=[],events=[])
write(D/'result.json',report)
class Actor:
    def __init__(self,label,port):
        self.label=label;self.frame=0;self.seq=0;self.proc=None;self.sock=None;self.stream=None;self.logfiles=[];self.quiz=0;self.prev_intro=False;self.prev_sound=None;self.prev_choice=False;self.first_choice=None;self.first_question=None;self.first_cycle_end=None;self.result_seen=False;self.gate_initial=fixture['gate1_unlocked'];self.gate_now=self.gate_initial;self.first_gate_rise=None;self.sound_starts=[];self.first_result_ready=None
        self.dir=D/label;self.dir.mkdir();self.save=self.dir/'ordinary.sav';self.save.write_bytes(pathlib.Path(fixture['path']).read_bytes());assert not self.save.with_suffix('.script_flags.json').exists()
        identity=load(identities[label]);assert identity['actual_game_source']==expected[label];assert sha(identity['cli'])==identity['cli_sha256'];source=load(identity['source_freeze']);assert sha(identity['source_freeze'])==identity['source_freeze_sha256'] and source['head']==expected[label]
        for name,digest in source['external_gfx'].items(): assert sha(R/'gfx'/name)==digest,name
        assert len(source['external_gfx'])==702 and source['engine_head']=='e533a3d23a0cdbed1524a1ac91184733da4a6f7e'
        self.argv=[identity['cli'],'run','--headless','--speed','0','--seed','42','--debug-port',str(port),'--lang','en','--no-audio','--save',str(self.save)]
        env=os.environ.copy();env.pop('POKERED_MAPS_DIR',None);env['LD_LIBRARY_PATH']='/tmp/pokered-build-lib'
        self.game_log=(self.dir/'game.log').open('x');self.rpc_log=(self.dir/'rpc.jsonl').open('x');self.state_log=(self.dir/'states.jsonl').open('x');self.logfiles=[self.game_log,self.rpc_log,self.state_log]
        self.record=dict(argv=self.argv,cwd=str(self.dir),env_overrides={'LD_LIBRARY_PATH':env['LD_LIBRARY_PATH'],'POKERED_MAPS_DIR':'removed'},identity=str(identities[label]),identity_sha256=sha(identities[label]),source_sha256=identity['source_freeze_sha256'],binary_sha256=sha(identity['cli']),physical_preboot_sram_sha256=sha(self.save),actual_source=expected[label]);report['actors'][label]=self.record;write(D/'result.json',report)
        actors.append(self)  # Register ownership before any process can start or fail.
        self.proc=subprocess.Popen(self.argv,cwd=self.dir,env=env,stdout=self.game_log,stderr=subprocess.STDOUT,start_new_session=True);self.record.update(game_pid=self.proc.pid,game_pgid=self.proc.pid);write(D/'result.json',report);print(json.dumps(dict(actor=label,game_pid=self.proc.pid,runner_pid=os.getpid(),source=expected[label])),flush=True)
        for _ in range(200):
            assert self.proc.poll() is None,('prematureGameexit',self.proc.returncode)
            try:self.sock=socket.create_connection(('127.0.0.1',port),timeout=.1);break
            except (ConnectionRefusedError,TimeoutError):time.sleep(.05)
        assert self.sock is not None,'No server; no Game retry';self.sock.settimeout(30);self.sock.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1);self.stream=self.sock.makefile('rwb')
        self.current=self.state();assert self.current['frame_count']==0
    def rpc(self,cmd,**kwargs):
        assert cmd in report['allowed_rpc'];self.seq+=1;request=dict(cmd=cmd,**kwargs);self.rpc_log.write(json.dumps(dict(seq=self.seq,request=request))+'\n');self.rpc_log.flush();self.stream.write((json.dumps(request)+'\n').encode());self.stream.flush();raw=self.stream.readline(4*1024*1024);assert raw.endswith(b'\n'),'TruncatedRPC; no retry';answer=json.loads(raw);self.rpc_log.write(json.dumps(dict(seq=self.seq,response=answer))+'\n');self.rpc_log.flush();assert answer.get('ok') is True,answer;return answer.get('data')
    def state(self):
        s=self.rpc('get_state');assert s['frame_count']==self.frame,(self.label,s['frame_count'],self.frame);self.state_log.write(json.dumps(s,ensure_ascii=False)+'\n');self.state_log.flush();return s
    def step(self,b):
        f=self.frame;v=self.rpc('press_timeline',buttons=[b],start_at_frame=f,advance=True);assert v==dict(queue_start_frame=f,start_frame=f,end_frame=f,frames=1,padding_frames=0,advanced=True,frame_count=f+1),v;self.frame+=1;self.current=self.state();self.observe();return self.current
    def event(self,kind,**data):
        row=dict(provider=self.label,frame=self.frame,quiz_ordinal=self.quiz,kind=kind,**data);report['events'].append(row);return row
    def flags(self):
        flags=self.rpc('get_flags');assert isinstance(flags,dict);value=bool(flags.get(GATE,False));self.event('flags_read',gate1=value)
        if value and not self.gate_now and self.quiz==1 and self.first_gate_rise is None:self.first_gate_rise=self.frame;self.event('first_gate_rise')
        self.gate_now=value;return flags
    def observe(self):
        s=self.current;t=effect_text(s);intro='POKéMON Quiz!' in t
        if intro and not self.prev_intro:self.quiz+=1;self.event('intro_enter',effect=s.get('script_effect'))
        self.prev_intro=intro
        if s.get('choice') is not None and not self.prev_choice:
            self.event('choice_open',choice=s['choice'],visible=s.get('dialogue'))
            if self.quiz==1 and self.first_choice is None:self.first_choice=self.frame;self.first_question=s.get('dialogue')
        self.prev_choice=s.get('choice') is not None
        audio=s.get('audio_sfx') or {};sound=audio.get('id') if audio.get('playing') else None
        if sound is not None and sound!=self.prev_sound:
            cue=self.event('sfx_start',sound_id=sound,state=brief(s));self.sound_starts.append(cue)
            if self.label=='after' and self.quiz==1 and self.first_choice is not None and self.first_cycle_end is None and s.get('screen')=='overworld':
                if sound==57:
                    assert "You're absolutely correct!" in t and (s.get('script_effect') or {}).get('leading_sound_started') is True
                    assert (s.get('dialogue_state') or {}).get('char_index')==0,('GET_ITEM_1mustprecedefirstglyph',brief(s))
                if sound==80:assert self.first_question is not None and s.get('dialogue')==self.first_question,('DENIEDmustretainquestion',self.first_question,brief(s))
        if sound is None and self.prev_sound is not None:self.event('sfx_end',sound_id=self.prev_sound)
        self.prev_sound=sound
        if self.quiz==1 and ("You're absolutely correct!" in t or 'Sorry! Bad call!' in t):self.result_seen=True
        d=s.get('dialogue_state') or {}
        if self.quiz==1 and self.result_seen and d.get('waiting_for_input') and d.get('page')==d.get('total_pages') and self.first_result_ready is None:self.first_result_ready=self.frame;self.event('first_result_final_page_ready',state=brief(s))
        if self.quiz==1 and self.first_cycle_end is None and self.first_choice is not None:
            if s.get('screen')=='battle':self.first_cycle_end=self.frame;self.event('first_cycle_handoff_battle')
            elif self.result_seen and not s.get('script_running') and s.get('active_script_effect') is None:self.first_cycle_end=self.frame;self.event('first_cycle_returned_to_field')
        if self.frame in [560,604,2012] or (self.quiz==1 and self.first_choice is not None and self.first_cycle_end is None and not self.gate_now):self.flags()
    def capture(self,reasons):
        p=self.dir/(f'frame-{self.frame:04d}.png');assert not p.exists();receipt=self.rpc('capture_frame',path=str(p));assert p.is_file();png=p.read_bytes();assert png[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',png[16:24])==(160,144);return dict(provider=self.label,path=str(p),sha256=sha(p),frame=self.frame,quiz_ordinal=self.quiz,reasons=reasons,rpc_receipt=receipt,state=brief(self.current))
    def finish(self):
        if self.stream:self.stream.close()
        if self.sock:self.sock.close()
        if self.proc and self.proc.poll() is None:os.killpg(self.proc.pid,signal.SIGTERM)
        if self.proc:self.record.update(game_exit_code=self.proc.wait(),game_group_gone=gone(self.proc.pid),shutdown='Owned Game SIGTERM after completedfixedvector or firsterror; no timeout/restart.')
        self.record.update(rpc_count=self.seq,frames=self.frame,quiz_count=self.quiz,first_choice_frame=self.first_choice,first_question=self.first_question,first_cycle_end=self.first_cycle_end,first_gate_rise=self.first_gate_rise,first_result_ready=self.first_result_ready,sound_starts=self.sound_starts)
        self.record['fixture_unchanged']=sha(self.save)==fixture['sha256'];assert self.record['fixture_unchanged'];assert not self.save.with_suffix('.script_flags.json').exists()
        for f in self.logfiles:f.close()
actors=[]
try:
    for port in case['ports']:
        with socket.socket() as probe:probe.bind(('127.0.0.1',port))
    before=Actor('before',case['ports'][0]);after=Actor('after',case['ports'][1])
    report['initial_bags']={x.label:x.rpc('get_bag') for x in actors};assert all(b==[] for b in report['initial_bags'].values())
    report['captures'].append(dict(frame=0,images=[x.capture(['fixed-anchor']) for x in actors]));write(D/'result.json',report)
    fixed={560,592,598,604,700,800,900,1000,1200,1400,1600,1800,2012};common=['screen','map_name','player_x','player_y','player_facing','player_transport','party','badges','money','coins','map_blocks']
    for frame,b in enumerate(buttons,1):
        states=[x.step(b) for x in actors]
        if frame<=598:
            for key in common:assert states[0].get(key)==states[1].get(key),('publicprefixdiff',frame,key)
        if frame==560:
            for x in actors:
                s=x.current;assert s.get('screen')=='overworld' and s.get('map_name')=='CinnabarGym' and (s.get('player_x'),s.get('player_y'))==(15,8) and s.get('player_facing')=='Down' and x.quiz==0,(x.label,brief(s))
                assert x.gate_now==fixture['gate1_unlocked']
        if frame==604:
            for x in actors:
                s=x.current;assert s.get('screen')=='overworld' and s.get('map_name')=='CinnabarGym' and (s.get('player_x'),s.get('player_y'))==(15,8) and s.get('player_facing')=='Up' and x.quiz==1 and 'POKéMON Quiz!' in effect_text(s),(x.label,brief(s),x.quiz)
            report['first_quiz_anchor604']={x.label:brief(x.current) for x in actors};print(json.dumps(dict(checkpoint604=True,case=a.case)),flush=True)
        reasons=[]
        if frame in fixed:reasons.append('fixed-anchor')
        if frame==after.first_choice:reasons.append('first-after-choice')
        for x in actors:
            audio=x.current.get('audio_sfx') or {}
            if frame>=604 and x.quiz==1 and (x.first_cycle_end is None or frame<=x.first_cycle_end) and audio.get('playing') and audio.get('id') in [57,80]:reasons.append(x.label+'-first-cycle-target-sfx')
        if reasons:report['captures'].append(dict(frame=frame,images=[x.capture(reasons) for x in actors]))
        if frame%128==0:write(D/'result.json',report)
    for x in actors:assert x.first_choice is not None and x.first_cycle_end is not None,(x.label,'No firstquizcompletion/handoff; preservefixedrun, no retry')
    def first_sounds(x):return [e['sound_id'] for e in x.sound_starts if e['quiz_ordinal']==1 and x.first_choice is not None and e['frame']>=x.first_choice and (x.first_cycle_end is None or e['frame']<=x.first_cycle_end)]
    af=first_sounds(after);bf=first_sounds(before);report['first_cycle_sound_ids']={'before':bf,'after':af}
    if a.case=='locked-wrong':assert 80 in af and 57 not in af and 80 in bf
    else:
        assert 57 in af and 57 not in bf
        if a.case=='already-open-correct':assert 86 not in af and 86 in bf
        else:assert 86 in af and after.first_gate_rise is not None and before.first_gate_rise is not None
    report['final_bags']={x.label:x.rpc('get_bag') for x in actors};assert report['final_bags']==report['initial_bags'];write(D/'buttons.json',buttons);report.update(all_pass=True,frames=2012,first_cycle_only_claims=True,later_cycles_and_battles_retained_without_first_cycle_claims=True)
except BaseException:report.update(error=traceback.format_exc(),first_failure_preserved=True);print(report['error'],flush=True)
finally:
    for x in actors:
        try:x.finish()
        except BaseException:report.update(all_pass=False,shutdown_error=traceback.format_exc())
    report.update(complete=True,finished_utc=datetime.datetime.now(datetime.timezone.utc).isoformat());write(D/'result.json',report)
print(json.dumps(dict(result=str(D/'result.json'),result_sha256=sha(D/'result.json'),all_pass=report['all_pass'],actual_sources=expected,frames={x.label:x.frame for x in actors},game_groups_gone={x.label:x.record.get('game_group_gone') for x in actors})),flush=True);sys.exit(0 if report['all_pass'] and all(x.record.get('game_group_gone') for x in actors) else 1)

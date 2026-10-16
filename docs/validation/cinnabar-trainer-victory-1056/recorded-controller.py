#!/usr/bin/env python3
"""Review-only preparation: replay one frozen before-derived vector on M1/912.
No discovery/adaptive input, no Cargo, no execution on import. Runtime requires
an explicitly supplied reviewed plan hash and --execute-reviewed-plan.
"""
from pathlib import Path
import argparse, datetime, hashlib, importlib.util, json, os, signal, socket, struct, subprocess, sys, time, traceback
R=Path('/home/jerry/develop/open-pokered');O=R/'.task-tmp/fidelity-39-plus'
M1='762fc5a32602d7dcee4da23a687ad42a7751384d';AFTER='912a593109c13495d04968e0b12436de6b910508'
HEADS={'before':M1,'after':AFTER}
BINARY_SHA={'before':'c13fd2f3cba4cbf4878a7a3497a35af39b210becf0e006f03554d1bf8549911c','after':'5318db30212aaea216aa05225cec043e4c11859ee5956a2d506bc14c473999fd'}
ALLOWED={'get_state','get_flags','get_bag','press_timeline','capture_frame'}
def sha(p):
    with Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def load(p):return json.loads(Path(p).read_text())
def write(p,v):Path(p).write_text(json.dumps(v,indent=2,ensure_ascii=False)+'\n')
def now():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def gone(p):
    try:os.killpg(p,0)
    except ProcessLookupError:return True
    return False
def artifact(p):p=Path(p);return {'path':str(p),'sha256':sha(p),'bytes':p.stat().st_size}
def interrupt_for_shutdown(signum,frame):
    raise KeyboardInterrupt(f'Receivedsignal{signum}; enterownedGamecleanup')
def stop_owned_game(proc):
    # Reap the direct child while observing its entire owned process group.
    # TERM and KILL each have one bounded ten-second grace period.
    record={'shutdown':'OwnedGameTERM10thenKILL10afterfixedvectororfirstfailure;notrestart','shutdown_signals':[]}
    errors=[]
    for sig in [signal.SIGTERM,signal.SIGKILL]:
        proc.poll()
        if gone(proc.pid):break
        try:
            os.killpg(proc.pid,sig)
            record['shutdown_signals'].append(signal.Signals(sig).name)
        except ProcessLookupError:pass
        except BaseException:errors.append(traceback.format_exc())
        deadline=time.monotonic()+10
        while True:
            proc.poll()
            if gone(proc.pid) or time.monotonic()>=deadline:break
            time.sleep(.05)
    record.update(exit_code=proc.poll(),group_gone=gone(proc.pid),shutdown_errors=errors)
    return record
def brief(s):return {k:s.get(k) for k in ['frame_count','screen','map_name','player_x','player_y','player_facing','script_running','active_script_effect','script_effect','dialogue','dialogue_state','battle_phase','battle_message','battle_live','battle_moves','audio_sfx','money','map_blocks']}
def guards():
    p=O/'audit-1057-run-victory-912-tail-v6.py';assert sha(p)=='eb14fbcf22defe04b6297ed0ccb7584949a253aaec530c9d3ef9b45b57965d92'
    spec=importlib.util.spec_from_file_location('approved_source_guards',p);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m

class Actor:
    def __init__(self,label,provider,case,fixture,directory,report):
        self.label=label;self.provider=provider;self.case=case;self.fixture=fixture;self.directory=directory;directory.mkdir()
        self.report=report;self.frame=0;self.seq=0;self.proc=None;self.sock=None;self.stream=None;self.files=[];self.current=None
        self.last_screen=None;self.battle_count=0;self.battle_seen=False;self.first_exit=None;self.first_victory=None
        self.messages=[];self.quote_episodes=0;self.first_quote=None;self.first_money=None;self.completed=None
        self.initial_blocks=None;self.flags={};self.last_flags=None;self.first_beaten_rise=None;self.first_gate_rise=None
        self.prev_sound=None;self.go_episodes=0;self.first_go=None;self.last_go_busy=None;self.go_busy_frames=[]
        self.beaten=f"EVENT_BEAT_CINNABAR_GYM_TRAINER_{case['trainer']}";self.gate=f"EVENT_CINNABAR_GYM_GATE{case['trainer']}_UNLOCKED"
        self.save=directory/'ordinary.sav';self.save.write_bytes(Path(fixture['path']).read_bytes())
        assert sha(self.save)==fixture['sha256'] and not self.save.with_suffix('.script_flags.json').exists()
        self.record={'source':HEADS[label],'binary':provider['binary'],'source_freeze':provider['source_freeze'],'physical_preboot_SRAM':fixture}
        report['actors'][label]=self.record
    def start(self,port):
        binary=self.provider['binary'];assert sha(binary['path'])==binary['sha256']==BINARY_SHA[self.label]
        self.game_log=(self.directory/'game.log').open('x');self.rpc_log=(self.directory/'rpc.jsonl').open('x');self.state_log=(self.directory/'states.jsonl').open('x');self.files=[self.game_log,self.rpc_log,self.state_log]
        argv=[binary['path'],'run','--headless','--speed','0','--seed','42','--debug-port',str(port),'--lang','en','--no-audio','--save',str(self.save)]
        env=os.environ.copy();env.pop('POKERED_MAPS_DIR',None);env['LD_LIBRARY_PATH']='/tmp/pokered-build-lib'
        self.record.update(argv=argv,cwd=str(self.directory),env_overrides={'LD_LIBRARY_PATH':env['LD_LIBRARY_PATH'],'POKERED_MAPS_DIR':'removed'})
        self.proc=subprocess.Popen(argv,cwd=self.directory,env=env,stdout=self.game_log,stderr=subprocess.STDOUT,start_new_session=True)
        self.record.update(pid=self.proc.pid,pgid=self.proc.pid);write(self.directory.parent/'result.json',self.report)
        print(json.dumps({'event':'Game-started','provider':self.label,'Game_pid':self.proc.pid,'source':HEADS[self.label]}),flush=True)
        for _ in range(200):
            assert self.proc.poll() is None,('GameexitedbeforeTCP',self.label,self.proc.returncode)
            try:self.sock=socket.create_connection(('127.0.0.1',port),timeout=.1);break
            except (ConnectionRefusedError,TimeoutError):time.sleep(.05)
        assert self.sock is not None,'NoTCP; noGame retry'
        self.sock.settimeout(30);self.sock.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1);self.stream=self.sock.makefile('rwb')
        self.current=self.state();assert self.current['frame_count']==0
    def rpc(self,cmd,**kwargs):
        assert cmd in ALLOWED;self.seq+=1;request={'cmd':cmd,**kwargs}
        self.rpc_log.write(json.dumps({'seq':self.seq,'request':request})+'\n');self.rpc_log.flush()
        self.stream.write((json.dumps(request)+'\n').encode());self.stream.flush();raw=self.stream.readline(4*1024*1024)
        assert raw.endswith(b'\n'),'TruncatedRPC; no retry'
        response=json.loads(raw);self.rpc_log.write(json.dumps({'seq':self.seq,'response':response})+'\n');self.rpc_log.flush()
        assert response.get('ok') is True,response
        return response.get('data')
    def state(self):
        s=self.rpc('get_state');assert s['frame_count']==self.frame
        self.state_log.write(json.dumps(s,ensure_ascii=False)+'\n');self.state_log.flush();return s
    def event(self,kind,**kwargs):self.report['events'].append({'provider':self.label,'frame':self.frame,'battle_ordinal':self.battle_count,'kind':kind,**kwargs})
    def read_flags(self):
        self.flags=self.rpc('get_flags');assert isinstance(self.flags,dict)
        brief_flags={'beaten':bool(self.flags.get(self.beaten,False)),'gate':bool(self.flags.get(self.gate,False))}
        if brief_flags!=self.last_flags:
            self.event('flags_change',**brief_flags)
            if self.last_flags is not None:
                if brief_flags['beaten'] and not self.last_flags['beaten'] and self.first_beaten_rise is None:self.first_beaten_rise=self.frame
                if brief_flags['gate'] and not self.last_flags['gate'] and self.first_gate_rise is None:self.first_gate_rise=self.frame
            self.last_flags=brief_flags
        return self.flags
    def step(self,b):
        assert b in [[],['A'],['B'],['UP']];wire=b[0] if b else None;f=self.frame
        receipt=self.rpc('press_timeline',buttons=[wire],start_at_frame=f,advance=True)
        assert receipt=={'queue_start_frame':f,'start_frame':f,'end_frame':f,'frames':1,'padding_frames':0,'advanced':True,'frame_count':f+1},receipt
        self.frame+=1;self.current=self.state();s=self.current
        if s['screen']=='battle':
            if self.last_screen!='battle':self.battle_count+=1;self.battle_seen=True;self.event('battle_enter')
            assert self.battle_count==1,'Secondbattleforbidden'
            message=s.get('battle_message');phase=s.get('battle_phase') or ''
            if message is not None and (not self.messages or self.messages[-1]['message']!=message):
                self.messages.append({'frame':self.frame,'message':message});self.event('battle_message',message=message)
                if message==self.case['quote']:self.quote_episodes+=1
            if message==self.case['quote'] and self.first_quote is None:self.first_quote=self.frame
            if phase.startswith('TrainerVictory') and self.first_victory is None:self.first_victory=self.frame;self.event('first_TrainerVictory')
            if message and 'for\nwinning!' in message and self.first_money is None:self.first_money=self.frame;self.event('first_money_text')
        if self.battle_seen and s['screen']=='overworld':
            if self.first_exit is None:self.first_exit=self.frame;self.event('first_battle_exit')
            self.read_flags()
        audio=s.get('audio_sfx') or {};sound=audio.get('id') if audio.get('playing') else None
        if sound!=self.prev_sound:
            if self.prev_sound is not None:self.event('sfx_end',sound_id=self.prev_sound)
            if sound is not None:self.event('sfx_start',sound_id=sound)
        if self.first_exit is not None and s['screen']=='overworld' and sound==86:
            if self.prev_sound!=86:self.go_episodes+=1
            if self.first_go is None:self.first_go=self.frame
            self.last_go_busy=self.frame;self.go_busy_frames.append(self.frame)
            if self.label=='after':
                assert s['active_script_effect']=='WaitSound',('GOrequiresactualWaitSound',brief(s))
                assert not self.flags.get(self.beaten,False) and bool(self.flags.get(self.gate,False))==self.case['initial_gate_flag']
                if self.case['trainer']==0:assert s['map_blocks']==self.initial_blocks
                else:assert s['map_blocks'][39]==self.case['initial_block']
        if self.first_exit is not None and self.completed is None and self.flags.get(self.beaten,False) and not s['script_running'] and s['active_script_effect'] is None:
            self.completed=self.frame;self.event('first_continuation_complete')
        self.prev_sound=sound;self.last_screen=s['screen'];return s
    def capture(self,reasons):
        p=self.directory/f'frame-{self.frame:05d}.png';assert not p.exists()
        receipt=self.rpc('capture_frame',path=str(p));observed=self.rpc('get_state');assert observed==self.current,('Capturechangedpublicstate',self.label,self.frame)
        raw=p.read_bytes();assert raw[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',raw[16:24])==(160,144)
        return {'provider':self.label,'frame':self.frame,'png':artifact(p),'reasons':reasons,'state':brief(self.current),'capture_receipt':receipt,'readonly_state_guard':True}
    def finish(self):
        errors=[]
        # A transport close failure must never bypass owned-process cleanup.
        for name,resource in [('stream',self.stream),('socket',self.sock)]:
            if resource is not None:
                try:resource.close()
                except BaseException:errors.append({'operation':name+'.close','error':traceback.format_exc()})
        if self.proc is not None:
            try:self.record.update(stop_owned_game(self.proc))
            except BaseException:
                errors.append({'operation':'stop_owned_game','error':traceback.format_exc()})
                self.record.update(exit_code=self.proc.poll(),group_gone=gone(self.proc.pid))
        try:
            self.record.update(frames=self.frame,rpc_count=self.seq,battle_count=self.battle_count,first_victory=self.first_victory,first_quote=self.first_quote,quote_episodes=self.quote_episodes,first_money=self.first_money,first_exit=self.first_exit,first_GO=self.first_go,last_GO_busy=self.last_go_busy,GO_busy_frames=self.go_busy_frames,GO_episodes=self.go_episodes,first_beaten_rise=self.first_beaten_rise,first_gate_rise=self.first_gate_rise,first_continuation_complete=self.completed,battle_messages=self.messages,fixture_unchanged=sha(self.save)==self.fixture['sha256'])
            assert self.record['fixture_unchanged'] and not self.save.with_suffix('.script_flags.json').exists()
        except BaseException:errors.append({'operation':'final_record_or_fixture_guard','error':traceback.format_exc()})
        finally:
            for f in self.files:
                try:f.close()
                except BaseException:errors.append({'operation':'log.close','error':traceback.format_exc()})
        self.record['cleanup_errors']=errors
        assert self.proc is None or self.record.get('group_gone'),('OwnedGamegroupstillpresent',self.record)
        assert not errors and not self.record.get('shutdown_errors'),('Cleanupfailurepreserved',self.record)

def main():
    p=argparse.ArgumentParser();p.add_argument('--plan',required=True);p.add_argument('--plan-sha256',required=True);p.add_argument('--case',required=True,choices=['trainer0-locked','trainer1-locked','trainer1-open']);p.add_argument('--output',required=True);p.add_argument('--execute-reviewed-plan',required=True,action='store_true');args=p.parse_args()
    pp=Path(args.plan).resolve();assert sha(pp)==args.plan_sha256;plan=load(pp);assert plan['before_source']==M1 and plan['after_source']==AFTER
    case=next(x for x in plan['cases'] if x['case']==args.case);vector=case['fixed_vector'];assert sha(vector['path'])==vector['sha256'];buttons=load(vector['path']);assert len(buttons)==vector['frames'] and all(b in [[],['A'],['B'],['UP']] for b in buttons)
    discovery=load(case['before_discovery_result']['path']);assert sha(case['before_discovery_result']['path'])==case['before_discovery_result']['sha256'] and discovery['all_pass'] and discovery['actual_source']==M1
    assert vector['sha256']==discovery['proposed_pair_vector_sha256'];assert buttons==load(Path(case['before_discovery_result']['path']).parent/'actual-discovery-buttons.json')+([['B']]*2+[[]]*30)*16+[[]]*64
    D=Path(args.output).resolve();assert not D.exists();D.mkdir();report={'complete':False,'all_pass':False,'runner_pid':os.getpid(),'runner_pgid':os.getpgrp(),'controller':artifact(__file__),'plan':artifact(pp),'actual_sources':HEADS,'case':case,'vector':vector,'started':now(),'actors':{},'events':[],'captures':[],'Cargo_calls':0,'adaptive_input':False,'RNG_stream_equality_claim':False,'first_phase_split':None};write(D/'result.json',report);g=guards();actors=[]
    def guard_sources(suffix):
        for label in ['before','after']:
            provider=plan['providers'][label];assert sha(provider['source_freeze']['path'])==provider['source_freeze']['sha256'];freeze=load(provider['source_freeze']['path']);assert freeze['head']==HEADS[label];g.HEAD=HEADS[label];write(D/f'{label}-source-{suffix}.json',g.source_guard(freeze,Path(freeze['worktree'])));assert sha(provider['binary']['path'])==BINARY_SHA[label]
    previous_term_handler=signal.signal(signal.SIGTERM,interrupt_for_shutdown)
    try:
        guard_sources('before');fixture=case['physical_preboot_SRAM'];assert sha(fixture['path'])==fixture['sha256']
        for port in case['ports']:
            with socket.socket() as probe:probe.bind(('127.0.0.1',port))
        for label,port in zip(['before','after'],case['ports']):
            actor=Actor(label,plan['providers'][label],case,fixture,D/label,report);actors.append(actor);actor.start(port)
        before,after=actors;assert all(a.rpc('get_bag')==[] for a in actors)
        report['captures'].append({'frame':0,'images':[a.capture(['frame-zero']) for a in actors]})
        common=['screen','map_id','map_name','player_x','player_y','player_facing','player_transport','party','badges','money','coins','map_blocks','battle_phase','battle_message','battle_live','battle_moves','audio_sfx','dialogue','dialogue_state','choice','script_running','active_script_effect','script_awaiting_battle']
        for frame,b in enumerate(buttons,1):
            states=[a.step(b) for a in actors];reasons=[]
            if report['first_phase_split'] is None:
                differing=[k for k in common if states[0].get(k)!=states[1].get(k)]
                if differing:
                    quote=case['quote'];a_phase=states[1].get('battle_phase') or '';b_phase=states[0].get('battle_phase') or ''
                    expected_quote_boundary=(states[0]['screen']==states[1]['screen']=='battle' and (states[1].get('battle_message')==quote or quote.replace('\n','\\n') in a_phase) and ('for\nwinning!' in (states[0].get('battle_message') or '') or 'for\\nwinning!' in b_phase))
                    assert expected_quote_boundary,('Unexpectedpublicprefixdifference',frame,differing,brief(states[0]),brief(states[1]))
                    report['first_phase_split']={'frame':frame,'fields':differing,'before':brief(states[0]),'after':brief(states[1]),'scope':'Expectedexplicitquoteversusmoneyboundary; laterphasesneednotmatch'};reasons.append('first-public-victory-quote-split')
            if frame==592:
                for a in actors:
                    s=a.current;assert s['screen']=='overworld' and s['map_name']=='CinnabarGym' and [s['player_x'],s['player_y']]==case['position'] and s['player_facing']=='Down' and not s['script_running'] and s['active_script_effect'] is None
                    a.initial_blocks=s['map_blocks'];flags=a.read_flags();assert not flags.get(a.beaten,False) and bool(flags.get(a.gate,False))==case['initial_gate_flag']
                    if case['trainer']==1:assert s['map_blocks'][39]==case['initial_block']
            if frame==600:
                for a in actors:
                    s=a.current;assert s['screen']=='overworld' and [s['player_x'],s['player_y']]==case['position'] and s['player_facing']=='Up' and s['script_running']
            if frame in {560,592,600,len(buttons)}:reasons.append('fixed-anchor')
            for a in actors:
                s=a.current;audio=s.get('audio_sfx') or {}
                if a.battle_count==1 and s['screen']=='battle' and ('TrainerVictory' in (s.get('battle_phase') or '') or s.get('battle_message')==case['quote'] or 'for\nwinning!' in (s.get('battle_message') or '')):reasons.append(a.label+'-first-battle-victory-text')
                if a.first_exit is not None and audio.get('playing') and audio.get('id')==86:reasons.append(a.label+'-first-victory-GO-busy')
                if frame in [a.first_beaten_rise,a.first_gate_rise,a.completed]:reasons.append(a.label+'-flag-or-completion-boundary')
            if reasons:report['captures'].append({'frame':frame,'images':[a.capture(reasons) for a in actors]})
            if frame%128==0:write(D/'result.json',report)
        for a in actors:
            s=a.current;assert a.battle_count==1 and a.completed is not None and a.first_money is not None
            assert s['screen']=='overworld' and not s['script_running'] and s['active_script_effect'] is None and s['dialogue'] is None
            flags=a.read_flags();assert flags.get(a.beaten,False) and flags.get(a.gate,False)
            if case['trainer']==0:assert s['map_blocks']==a.initial_blocks
            else:assert s['map_blocks'][39]==14
            assert a.rpc('get_bag')==[]
        assert before.quote_episodes==0 and before.first_quote is None and before.go_episodes==0
        assert after.quote_episodes==1 and after.first_quote<after.first_money and after.go_episodes==1 and after.go_busy_frames
        assert after.first_beaten_rise>after.last_go_busy
        if not case['initial_gate_flag']:assert after.first_gate_rise>after.last_go_busy
        for key in ['player_x','player_y','player_facing','money','party','badges','coins','map_blocks','evaluation']:assert before.current.get(key)==after.current.get(key),('Finalpersistentpublicdifference',key)
        assert before.flags==after.flags and report['first_phase_split'] is not None
        report.update(all_pass=True,completed_fixed_frames=len(buttons),scope='ActualM1/actual912,samephysicalSRAMandfrozenbefore-derivedordinarybuttons; firstbattleonly; realnoaudiosequencersignals, actualPCMnonzero separatelycoveredby912App1/3case; noRNGsnapshot/equalityclaim')
    except BaseException:report.update(all_pass=False,error=traceback.format_exc(),first_failure_preserved=True);print(report['error'],flush=True)
    finally:
        # A second TERM cannot interrupt the bounded cleanup already in progress.
        signal.signal(signal.SIGTERM,signal.SIG_IGN)
        try:
            for a in actors:
                try:a.finish()
                except BaseException:
                    report.update(all_pass=False)
                    report.setdefault('shutdown_errors',[]).append({'provider':a.label,'error':traceback.format_exc()})
            try:guard_sources('after');report['source_before_after_equal']=True
            except BaseException:report.update(all_pass=False,source_guard_error=traceback.format_exc())
            report.update(complete=True,ended=now(),all_Game_groups_gone=all(a.record.get('group_gone') for a in actors));write(D/'result.json',report)
        finally:signal.signal(signal.SIGTERM,previous_term_handler)
    print(json.dumps({'result':str(D/'result.json'),'sha256':sha(D/'result.json'),'all_pass':report['all_pass'],'all_Game_groups_gone':report['all_Game_groups_gone']}),flush=True)
    return 0 if report['all_pass'] and report['all_Game_groups_gone'] else 1
if __name__=='__main__':sys.exit(main())

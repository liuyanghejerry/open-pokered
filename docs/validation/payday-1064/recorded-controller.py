#!/usr/bin/env python3
"""Source-only prepared fixed Pay Day replay. No execution on import.
A ready, fully pinned plan and --execute-reviewed-plan are required. Readonly
state/capture events never choose or modify an input. No Cargo or debug setters.
"""
from pathlib import Path
import argparse,datetime,hashlib,json,os,signal,socket,struct,subprocess,sys,time,traceback

ALLOWED={'get_state','get_flags','press_timeline','capture_frame'}
TAIL=([['B']]*2+[[]]*30)*16+[[]]*64
SCHEMA='payday-fixed-pair-v1'

def sha(path):
    with Path(path).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def load(path):return json.loads(Path(path).read_text())
def write(path,value):Path(path).write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
def now():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def artifact(path):
    p=Path(path).resolve();return {'path':str(p),'sha256':sha(p),'bytes':p.stat().st_size}
def pinned(record):
    p=Path(record['path']).resolve();assert p.is_file() and sha(p)==record['sha256'],('artifact changed',record);return p
def fingerprint(p):return 'symlink:'+os.readlink(p) if p.is_symlink() else sha(p)
def gone(pg):
    try:os.killpg(pg,0)
    except ProcessLookupError:return True
    return False

def source_guard(provider):
    freeze=load(pinned(provider['source_freeze']));spec=provider['guard'];w=Path(spec['worktree']).resolve()
    assert freeze[spec['head_key']]==provider['head']
    assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=w).decode().strip()==provider['head']
    assert subprocess.check_output(['git','status','--porcelain'],cwd=w)==b''
    expected=freeze[spec['source_key']]
    roster={p for p in subprocess.check_output(['git','ls-files','-z','--',':!:docs/**'],cwd=w).decode().split('\0') if p}
    assert roster==set(expected) and all(fingerprint(w/p)==h for p,h in expected.items()),('source guard',provider['head'])
    gp=(w/'gfx').resolve();graphics={str(p.relative_to(gp)):sha(p) for p in gp.rglob('*') if p.is_file()}
    assert graphics==freeze[spec['gfx_key']] and len(graphics)==702
    engine=Path(spec['engine_root']).resolve();assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=engine).decode().strip()==spec['engine_head']
    assert subprocess.check_output(['git','status','--porcelain','--untracked-files=no'],cwd=engine)==b''
    expected_engine=freeze[spec['engine_key']]
    roster_engine={p for p in subprocess.check_output(['git','ls-files','-z'],cwd=engine).decode().split('\0') if p}
    assert roster_engine==set(expected_engine) and all(fingerprint(engine/p)==h for p,h in expected_engine.items())
    pinned(provider['binary']);identity=load(pinned(provider['identity']))
    # Paths into the actual build/identity receipt are specified by the plan,
    # rather than assuming a different task's receipt schema or source label.
    def getpath(value,path):
        for k in path:value=value[k]
        return value
    assert getpath(identity,provider['identity_fields']['head'])==provider['head']
    assert getpath(identity,provider['identity_fields']['binary_sha256'])==provider['binary']['sha256']
    for a in provider.get('extra_identity_artifacts',[]):pinned(a)
    return {'head':provider['head'],'worktree':str(w),'source_count':len(expected),'engine_head':spec['engine_head'],'engine_count':len(expected_engine),'gfx_count':len(graphics),'source_freeze':provider['source_freeze'],'binary':provider['binary'],'all_equal':True}


def stop_owned(proc):
    # Always reap the direct child, then separately prove the whole group gone.
    rec={'exit_before_cleanup':proc.poll(),'shutdown_signals':[],'cleanup_not_natural_success':True}
    for sig in [signal.SIGTERM,signal.SIGKILL]:
        proc.poll()
        if gone(proc.pid):break
        try:os.killpg(proc.pid,sig);rec['shutdown_signals'].append(signal.Signals(sig).name)
        except ProcessLookupError:pass
        deadline=time.monotonic()+10
        while time.monotonic()<deadline:
            proc.poll()
            if gone(proc.pid):break
            time.sleep(.05)
    try:proc.wait(timeout=2)
    except subprocess.TimeoutExpired:pass
    rec.update(actual_process_returncode=proc.poll(),group_gone=gone(proc.pid),direct_child_reaped=proc.returncode is not None)
    return rec

def interrupted(sig,frame):raise KeyboardInterrupt(f'signal {sig}: enter owned Game cleanup')

def normalized(text):return ' '.join((text or '').split())
def true_flags(flags):
    assert isinstance(flags,dict);return {k for k,v in flags.items() if v is True}

class Actor:
    def __init__(self,label,provider,fixture,plan,root,report):
        self.label=label;self.provider=provider;self.fixture=fixture;self.plan=plan;self.root=root;root.mkdir()
        self.report=report;self.frame=0;self.sequence=0;self.proc=None;self.sock=None;self.stream=None;self.files=[]
        self.current=None;self.flags=None;self.initial_flags=None;self.previous_screen=None;self.battle_count=0;self.first_return=None
        self.messages=[];self.markers={};self.previous_message=None;self.last_progress=time.monotonic()
        self.save=root/'ordinary.sav';self.save.write_bytes(pinned(fixture).read_bytes());assert sha(self.save)==fixture['sha256']
        assert not list(root.glob('*.script_flags.json'))
        self.record={'head':provider['head'],'binary':provider['binary'],'identity':provider['identity'],'source_freeze':provider['source_freeze'],'physical_SRAM':fixture,'complete':False}
        report['actors'][label]=self.record
    def start(self,port):
        self.game_log=(self.root/'game.log').open('xb');self.rpc_log=(self.root/'rpc.jsonl').open('x');self.states=(self.root/'states.jsonl').open('x');self.files=[self.game_log,self.rpc_log,self.states]
        env=os.environ.copy();env.pop('POKERED_MAPS_DIR',None);env['LD_LIBRARY_PATH']=self.plan['runtime']['LD_LIBRARY_PATH']
        argv=[str(pinned(self.provider['binary'])),'run','--headless','--speed','0','--seed',str(self.plan['runtime']['seed']),'--debug-port',str(port),'--lang','en','--no-audio','--save',str(self.save)]
        self.record.update(argv=argv,cwd=str(self.root),env_overrides={'LD_LIBRARY_PATH':env['LD_LIBRARY_PATH'],'POKERED_MAPS_DIR':'removed'})
        self.proc=subprocess.Popen(argv,cwd=self.root,env=env,stdout=self.game_log,stderr=subprocess.STDOUT,start_new_session=True)
        self.report['Game_calls']+=1
        self.record.update(pid=self.proc.pid,pgid=self.proc.pid);write(self.root.parent/'result.json',self.report)
        print(json.dumps({'event':'Game-started','label':self.label,'pid':self.proc.pid,'pgid':self.proc.pid,'head':self.provider['head']}),flush=True)
        for _ in range(200):
            assert self.proc.poll() is None,('early Game exit',self.label,self.proc.returncode)
            try:self.sock=socket.create_connection(('127.0.0.1',port),timeout=.1);break
            except (ConnectionRefusedError,TimeoutError):time.sleep(.05)
        assert self.sock is not None,'no TCP, no retry'
        self.sock.settimeout(30);self.sock.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1);self.stream=self.sock.makefile('rwb')
        self.current=self.read_state();self.flags=self.rpc('get_flags');self.initial_flags=true_flags(self.flags)
        assert self.current['frame_count']==0
        assert self.plan['expected']['trainer_flag'] not in self.initial_flags
    def rpc(self,cmd,**kw):
        assert cmd in ALLOWED;self.sequence+=1;request={'cmd':cmd,**kw}
        self.rpc_log.write(json.dumps({'sequence':self.sequence,'absolute_frame':self.frame,'direction':'request','payload':request})+'\n');self.rpc_log.flush()
        self.stream.write((json.dumps(request)+'\n').encode());self.stream.flush();raw=self.stream.readline(4*1024*1024)
        assert raw.endswith(b'\n'),'truncated RPC, no retry';reply=json.loads(raw)
        self.rpc_log.write(json.dumps({'sequence':self.sequence,'absolute_frame':self.frame,'direction':'reply','payload':reply})+'\n');self.rpc_log.flush()
        assert reply.get('ok') is True,reply;return reply.get('data')
    def read_state(self):
        s=self.rpc('get_state');assert s['frame_count']==self.frame
        self.states.write(json.dumps({'absolute_frame':self.frame,'state':s},ensure_ascii=False)+'\n');self.states.flush();return s
    def step(self,buttons):
        assert buttons in [[],['A'],['B'],['START'],['UP']],('unreviewed input',buttons)
        assert self.proc.poll() is None,('Game exited during fixed vector',self.label,self.frame,self.proc.returncode)
        f=self.frame;wire=buttons[0] if buttons else None
        result=self.rpc('press_timeline',buttons=[wire],start_at_frame=f,advance=True)
        assert result=={'queue_start_frame':f,'start_frame':f,'end_frame':f,'frames':1,'padding_frames':0,'advanced':True,'frame_count':f+1},result
        self.frame+=1;self.current=self.read_state();self.flags=self.rpc('get_flags')
        s=self.current
        assert s['screen'] in self.plan['expected']['allowed_screens'],('unexpected screen',self.label,self.frame,s['screen'])
        if s['screen']=='battle':
            phase=(s.get('battle_phase') or '').split(' ')[0].split('{')[0]
            assert phase in self.plan['expected']['allowed_battle_phase_tags'],('unexpected battle phase',self.label,self.frame,s.get('battle_phase'))
        if s['screen']=='battle' and self.previous_screen!='battle':
            self.battle_count+=1;assert self.battle_count==1,('second battle',self.label,self.frame)
        if self.battle_count and s['screen']=='overworld' and self.first_return is None:self.first_return=self.frame
        if s['screen']=='battle':
            message=s.get('battle_message');text=normalized(message)
            if message is not None and message!=self.previous_message:self.messages.append({'frame':self.frame,'message':message})
            self.previous_message=message
            for kind,expected in self.plan['expected']['messages'][self.label].items():
                if text==normalized(expected) and kind not in self.markers:self.markers[kind]=self.frame
        self.previous_screen=s['screen']
        if time.monotonic()-self.last_progress>=30:
            print(json.dumps({'event':'progress','label':self.label,'frame':self.frame,'screen':s['screen'],'battle_phase':s.get('battle_phase'),'markers':self.markers}),flush=True);self.last_progress=time.monotonic()
        return s
    def capture(self,reasons):
        p=self.root/f'frame-{self.frame:05d}.png';assert not p.exists()
        receipt=self.rpc('capture_frame',path=str(p));after=self.rpc('get_state');assert after==self.current,('capture mutated state',self.label,self.frame)
        data=p.read_bytes();assert data[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',data[16:24])==(160,144)
        return {'label':self.label,'frame':self.frame,'reasons':reasons,'png':artifact(p),'capture_receipt':receipt,'full_state':self.current,'flags':self.flags,'readonly_state_guard':True}
    def validate_final(self):
        s=self.current;e=self.plan['expected'];assert self.battle_count==1 and self.first_return is not None
        assert s['screen']=='overworld' and s['map_name']==e['map'] and [s['player_x'],s['player_y']]==e['position']
        assert s['money']==e['final_money']==e['initial_money']+e['prize']+e['pot']
        assert not s['script_running'] and s['active_script_effect'] is None and s['dialogue'] is None
        new=true_flags(self.flags)-self.initial_flags;lost=self.initial_flags-true_flags(self.flags)
        assert new=={e['trainer_flag']} and not lost,('unexpected flags',self.label,new,lost)
        for k in e['messages'][self.label]:assert k in self.markers,('missing text marker',self.label,k,self.messages)
        for forbidden in e['forbidden_messages'][self.label]:assert all(normalized(m['message'])!=normalized(forbidden) for m in self.messages)
        if self.label=='after':assert self.markers['scatter']<self.markers['prize']<self.markers['pickup']<self.first_return
        self.record.update(complete=True,first_return=self.first_return,markers=self.markers,battle_count=self.battle_count,battle_messages=self.messages,final_state=s,final_flags=self.flags,new_flags=sorted(new))
    def finish(self):
        errors=[]
        # Closing an RPC stream must never bypass cleanup/reaping the Game.
        try:
            for label,resource in [('stream',self.stream),('socket',self.sock)]:
                if resource is not None:
                    try:resource.close()
                    except BaseException:errors.append({'operation':label+'.close','error':traceback.format_exc()})
        finally:
            if self.proc is not None:
                try:self.record['shutdown']=stop_owned(self.proc)
                except BaseException:errors.append({'operation':'owned-process-cleanup','error':traceback.format_exc()})
            try:
                self.record.update(frames=self.frame,rpc_count=self.sequence,fixture_copy_unchanged=sha(self.save)==self.fixture['sha256'],no_sidecar=not list(self.root.glob('*.script_flags.json')))
                assert self.record['fixture_copy_unchanged'] and self.record['no_sidecar']
            except BaseException:errors.append({'operation':'fixture final guard','error':traceback.format_exc()})
            for f in self.files:
                try:f.close()
                except BaseException:errors.append({'operation':'log.close','error':traceback.format_exc()})
        self.record['cleanup_errors']=errors
        assert not errors and (self.proc is None or (self.record['shutdown']['group_gone'] and self.record['shutdown']['direct_child_reaped']))


def validate_plan(plan_path,expected_sha):
    assert sha(plan_path)==expected_sha;plan=load(plan_path)
    assert plan['schema']==SCHEMA and plan['ready'] is True
    assert sha(__file__)==plan['controller']['sha256'] and Path(plan['controller']['path']).resolve()==Path(__file__).resolve()
    assert plan['runtime']['seed']==42 and plan['runtime']['speed']==0
    assert set(plan['providers'])=={'before','after'} and plan['providers']['before']['head']!=plan['providers']['after']['head']
    assert len(plan['ports'])==2 and len(set(plan['ports']))==2 and all(isinstance(p,int) and 1024<=p<65536 for p in plan['ports'])
    discovery=load(pinned(plan['before_discovery']));assert discovery['complete'] and discovery['all_pass'] and discovery['game_group_gone']
    assert discovery['actual_source']==plan['providers']['before']['head']
    assert discovery['provider_sha256']==plan['base_provider_receipt']['sha256'];pinned(plan['base_provider_receipt'])
    assert discovery['actual_buttons']==plan['actual_discovery_buttons'] and discovery['proposed_fixed_vector']==plan['fixed_vector']
    actual=load(pinned(plan['actual_discovery_buttons']));fixed=load(pinned(plan['fixed_vector']))
    assert fixed==actual+TAIL and len(actual)==plan['actual_discovery_buttons']['frames'] and len(fixed)==plan['fixed_vector']['frames']
    assert all(b in [[],['A'],['B'],['START'],['UP']] for b in fixed)
    fixture=pinned(plan['physical_SRAM']);assert fixture.stat().st_size==32768 and not fixture.with_suffix('.script_flags.json').exists()
    before=load(pinned(plan['base_provider_receipt']));assert before['head']==plan['providers']['before']['head'] and before['binary']==plan['providers']['before']['binary']
    assert before['fixture']['path']==str(fixture) and before['fixture']['sha256']==plan['physical_SRAM']['sha256']
    assert plan['expected']['initial_money']==1000 and plan['expected']['prize']==90 and plan['expected']['pot']==100 and plan['expected']['final_money']==1190
    assert plan['expected']['position']==[2,19] and plan['expected']['map']=='ViridianForest'
    assert plan['expected']['trainer_flag']=='EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_2'
    assert isinstance(plan['expected']['allowed_screens'],list) and {'overworld','battle'}<=set(plan['expected']['allowed_screens'])
    assert isinstance(plan['expected']['allowed_battle_phase_tags'],list) and plan['expected']['allowed_battle_phase_tags']
    assert set(plan['expected']['messages']['before'])=={'prize'} and set(plan['expected']['messages']['after'])=={'scatter','prize','pickup'}
    assert all(isinstance(v,str) and v for d in plan['expected']['messages'].values() for v in d.values())
    assert '$190' in plan['expected']['messages']['before']['prize'] and '$90' in plan['expected']['messages']['after']['prize'] and '$100' in plan['expected']['messages']['after']['pickup']
    assert 'coins' in plan['expected']['messages']['after']['scatter'].lower()
    assert plan['capture']['scatter_followup_frames']==list(range(17))
    assert set(plan['expected']['messages']['after'].values())<=set(plan['expected']['forbidden_messages']['before'])
    assert plan['expected']['messages']['before']['prize'] in plan['expected']['forbidden_messages']['after']
    return plan,fixed


def main():
    ap=argparse.ArgumentParser();ap.add_argument('--plan',required=True);ap.add_argument('--plan-sha256',required=True);ap.add_argument('--output',required=True);ap.add_argument('--execute-reviewed-plan',action='store_true',required=True);args=ap.parse_args()
    plan_path=Path(args.plan).resolve();plan,buttons=validate_plan(plan_path,args.plan_sha256)
    D=Path(args.output).resolve();assert not D.exists();D.mkdir();report={'complete':False,'all_pass':False,'runner_pid':os.getpid(),'runner_pgid':os.getpgrp(),'started':now(),'controller':artifact(__file__),'plan':artifact(plan_path),'fixed_vector':plan['fixed_vector'],'actors':{},'captures':[],'first_public_text_split':None,'source_guards':{},'Game_calls':0,'Cargo_calls':0,'adaptive_inputs':False,'RNG_snapshot_or_stream_equality_claim':False,'capture_pixel_scope':'State-triggered17consecutive scatter frames need actual visual review to identify first visible Coins. A message marker alone is not a visible frame claim.'};write(D/'result.json',report)
    actors=[];old=signal.signal(signal.SIGTERM,interrupted);oldint=signal.getsignal(signal.SIGINT)
    try:
        report['source_guards']['before']={label:source_guard(provider) for label,provider in plan['providers'].items()}
        # Both providers must consume the same actual engine/GFX input bytes.
        bf=load(pinned(plan['providers']['before']['source_freeze']));af=load(pinned(plan['providers']['after']['source_freeze']));bg=plan['providers']['before']['guard'];ag=plan['providers']['after']['guard']
        assert bf[bg['engine_key']]==af[ag['engine_key']] and bf[bg['gfx_key']]==af[ag['gfx_key']]
        for port in plan['ports']:
            with socket.socket() as probe:probe.bind(('127.0.0.1',port))
        for label,port in zip(['before','after'],plan['ports']):
            a=Actor(label,plan['providers'][label],plan['physical_SRAM'],plan,D/label,report);actors.append(a);a.start(port)
        before,after=actors;assert before.initial_flags==after.initial_flags
        schedule={0:['cold-boot'],len(buttons):['final-fixed-frame']}
        schedule.setdefault(load(pinned(plan['before_discovery']))['first_prize_frame'],[]).append('base-discovery-prize-absolute-frame')
        report['captures'].append({'frame':0,'images':[a.capture(['cold-boot']) for a in actors]})
        seen_markers=set()
        for frame,b in enumerate(buttons,1):
            states=[a.step(b) for a in actors];assert states[0]['frame_count']==states[1]['frame_count']==frame
            # Full Debug phase strings include FUTURE narration pages; those
            # differ before any page is shown, so do not call that a visible split.
            if report['first_public_text_split'] is None:
                shared=['screen','map_name','player_x','player_y','player_facing','party','money','coins','badges','map_blocks','battle_live','battle_moves','battle_message','dialogue','choice','script_running','active_script_effect']
                difference=[k for k in shared if states[0].get(k)!=states[1].get(k)]
                if difference:
                    assert after.markers.get('scatter')==frame,('unexpected public prefix split',frame,difference)
                    report['first_public_text_split']={'frame':frame,'fields':difference,'scope':'first current-message/state split; pixel visibility still requires PNG inspection'}
            for a in actors:
                for kind,first in a.markers.items():
                    key=(a.label,kind)
                    if key not in seen_markers:
                        seen_markers.add(key);schedule.setdefault(first,[]).append(a.label+'-first-'+kind)
                        offsets=plan['capture']['scatter_followup_frames'] if kind=='scatter' else [1,2,8]
                        for off in offsets:
                            if first+off<=len(buttons):schedule.setdefault(first+off,[]).append(a.label+'-'+kind+'-followup')
                if a.first_return==frame:schedule.setdefault(frame,[]).append(a.label+'-first-return')
            if frame in schedule:report['captures'].append({'frame':frame,'images':[a.capture(schedule[frame]) for a in actors]})
            if frame%128==0:write(D/'result.json',report)
        for a in actors:a.validate_final()
        assert report['first_public_text_split'] is not None
        # Settlement differs only in narration timing, not final persistent data.
        for key in ['map_name','player_x','player_y','player_facing','player_name','rival_name','party','money','coins','badges','map_blocks']:
            assert before.current.get(key)==after.current.get(key),('unexpected final difference',key)
        assert before.flags==after.flags
        report['all_pass']=True
    except BaseException:report.update(all_pass=False,error=traceback.format_exc(),first_failure_preserved=True);print(report['error'],flush=True)
    finally:
        signal.signal(signal.SIGTERM,signal.SIG_IGN);signal.signal(signal.SIGINT,signal.SIG_IGN)
        try:
            for a in actors:
                try:a.finish()
                except BaseException:report.update(all_pass=False);report.setdefault('cleanup_errors',[]).append({'label':a.label,'error':traceback.format_exc()})
            try:
                report['source_guards']['after']={label:source_guard(provider) for label,provider in plan['providers'].items()};pinned(plan['physical_SRAM']);pinned(plan['fixed_vector']);pinned(plan['before_discovery']);report['all_frozen_inputs_unchanged']=True
            except BaseException:report.update(all_pass=False,source_guard_error=traceback.format_exc())
            report['all_Game_groups_gone']=all(a.record.get('shutdown',{}).get('group_gone',a.proc is None) for a in actors)
            if not report['all_Game_groups_gone']:report['all_pass']=False
            report.update(complete=True,ended=now());write(D/'result.json',report)
        finally:
            signal.signal(signal.SIGTERM,old);signal.signal(signal.SIGINT,oldint)
    print(json.dumps({'result':str(D/'result.json'),'sha256':sha(D/'result.json'),'all_pass':report['all_pass'],'all_Game_groups_gone':report['all_Game_groups_gone']}),flush=True)
    return 0 if report['all_pass'] else 1
if __name__=='__main__':sys.exit(main())

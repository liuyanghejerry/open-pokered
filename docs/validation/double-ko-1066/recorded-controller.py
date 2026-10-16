#!/usr/bin/env python3
"""Prepared Double-KO native controller. No Game or runtime action on import.
--dry-only validates an explicitly unready plan, without starting a process.
Discovery uses ONLY M4; pair accepts only its reviewed, pinned literal vector.
All real execution requires a new fully ready plan and explicit root allocation.
"""
from pathlib import Path
import argparse,datetime,hashlib,json,os,re,signal,socket,struct,subprocess,sys,time,traceback
ALLOWED={'get_state','get_flags','press_timeline','capture_frame'}
SCHEMA='double-ko-native-v1'
BASE='062e76d91fdbf5109fcc02793b8a8064788e8700'
CANDIDATE='8c3c6b02ff1baf9887ba8b21bad2d958fd3ac4e7'
TAIL=([['B']]*2+[[]]*30)*32+[[]]*64
CASES=['only-geodude','healthy-reserve']
MOVE_NAMES=['Tackle','DefenseCurl','RockThrow','Selfdestruct']
SHARED=['screen','map_name','player_x','player_y','player_facing','party','money','coins','badges','map_blocks','battle_live','battle_moves','battle_message','dialogue','choice','script_running','active_script_effect']

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

    def capture(self,reasons):
            p=self.root/f'frame-{self.frame:05d}.png';assert not p.exists()
            receipt=self.rpc('capture_frame',path=str(p));after=self.rpc('get_state');assert after==self.current,('capture mutated state',self.label,self.frame)
            data=p.read_bytes();assert data[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',data[16:24])==(160,144)
            return {'label':self.label,'frame':self.frame,'reasons':reasons,'png':artifact(p),'capture_receipt':receipt,'full_state':self.current,'flags':self.flags,'readonly_state_guard':True}

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

    def step(self,buttons):
        assert buttons in [[],['A'],['B'],['START'],['UP']],('unreviewed input',buttons)
        assert self.proc.poll() is None,('early Game exit',self.label,self.frame,self.proc.returncode)
        f=self.frame;wire=buttons[0] if buttons else None
        receipt=self.rpc('press_timeline',buttons=[wire],start_at_frame=f,advance=True)
        assert receipt=={'queue_start_frame':f,'start_frame':f,'end_frame':f,'frames':1,'padding_frames':0,'advanced':True,'frame_count':f+1},receipt
        self.frame+=1;self.current=self.read_state();self.flags=self.rpc('get_flags');s=self.current
        assert s['screen'] in self.plan['expected']['allowed_screens'],('unexpected screen',self.label,self.frame,s['screen'])
        tag=(s.get('battle_phase') or '').split(' ')[0].split('{')[0]
        if s['screen']=='battle':
            assert tag in self.plan['expected']['allowed_battle_phase_tags'],('unexpected battle phase',self.label,self.frame,s.get('battle_phase'))
            if self.previous_screen!='battle':self.battle_count+=1
            assert self.battle_count==1,('second battle',self.label,self.frame)
            message=s.get('battle_message');text=normalized(message)
            if message is not None and message!=self.previous_message:
                self.messages.append({'frame':self.frame,'message':message})
                if 'missed' in text.lower():raise AssertionError(('first actual miss: preserve, no seed retry',self.label,self.frame,message))
            self.previous_message=message
            for kind,literal in self.plan['expected']['messages'].items():
                if text==normalized(literal):self.markers.setdefault(kind,self.frame)
            if 'blacked out!' in text.lower():self.markers.setdefault('blackout-text',self.frame)
            live=s.get('battle_live') or {};player=live.get('player') or {};enemy=live.get('enemy') or {}
            if player.get('hp')==0 and enemy.get('hp')==0:self.markers.setdefault('both-active-zero',self.frame)
            # Read the current terminal phase only. ShowingText's future Debug
            # queue may already include a terminal outcome; that is not current.
            if tag=='TrainerVictory':
                win=re.search(r'player_won:\s*(true|false)',s['battle_phase'])
                assert win,('unknown terminal phase encoding',s['battle_phase'])
                self.markers.setdefault('terminal-win' if win.group(1)=='true' else 'terminal-loss',self.frame)
            if 'selfdestruct-used' in self.markers and tag in ('PlayerMenu','MoveSelect'):
                raise AssertionError(('Selfdestruct did not reach declared final double KO; do not try another move/seed',self.label,self.frame,s))
        if self.battle_count and s['screen']=='overworld':
            if self.first_return is None:self.first_return=self.frame
            if s['map_name']=='PalletTown':self.markers.setdefault('pallet-arrival',self.frame)
        if s['screen']!=self.previous_screen:self.record.setdefault('screen_transitions',[]).append({'frame':self.frame,'screen':s['screen']})
        self.previous_screen=s['screen']
        if time.monotonic()-self.last_progress>=30:
            print(json.dumps({'event':'progress','label':self.label,'frame':self.frame,'screen':s['screen'],'phase':s.get('battle_phase'),'markers':self.markers}),flush=True);self.last_progress=time.monotonic()
        return s

    def validate_final(self,case,mode):
        s=self.current;e=self.plan['expected'];loss=(mode=='pair' and self.label=='after' and case=='only-geodude')
        assert self.battle_count==1 and self.first_return is not None
        assert {'selfdestruct-used','both-active-zero'}<=set(self.markers),('declared actual double KO not observed',self.label,self.markers)
        assert self.markers['selfdestruct-used']<=self.markers['both-active-zero']<self.first_return
        assert s['screen']=='overworld' and not s['script_running'] and s['active_script_effect'] is None and s['dialogue'] is None
        assert s['player_movement_state']=='Idle' and s['warp_fade']=='Idle'
        assert s['coins']==0
        if loss:
            assert {'terminal-loss','blackout-text','pallet-arrival'}<=set(self.markers)
            assert 'terminal-win' not in self.markers and 'prize' not in self.markers
            assert s['map_name']=='PalletTown' and [s['player_x'],s['player_y']]==[5,6]
            assert s['money']==500 and not self.flags.get(e['trainer_flag'],False)
            assert len(s['party'])==1 and s['party'][0]['species']=='Geodude'
            assert s['party'][0]['hp']==s['party'][0]['max_hp']==54
            expected='Loss/blackout/half-money/no-defeated-flag'
        else:
            assert {'terminal-win','prize'}<=set(self.markers)
            assert 'terminal-loss' not in self.markers and 'blackout-text' not in self.markers and 'pallet-arrival' not in self.markers
            assert s['map_name']=='ViridianForest' and [s['player_x'],s['player_y']]==[2,19]
            assert s['money']==1090 and self.flags.get(e['trainer_flag']) is True
            assert true_flags(self.flags)-self.initial_flags=={e['trainer_flag']} and not self.initial_flags-true_flags(self.flags)
            assert len(s['party'])==(1 if case=='only-geodude' else 2)
            assert s['party'][0]['species']=='Geodude' and s['party'][0]['hp']==0
            assert s['party'][0]['pp'][3]==4,('Selfdestruct consumed exactly one PP',s['party'][0])
            if case=='healthy-reserve':
                reserve=s['party'][1];assert reserve['species']=='Pidgey' and reserve['hp']==reserve['max_hp']>0
            expected='Win/prize90/defeated-flag/no-blackout'
        assert all('scattered' not in normalized(m['message']).lower() and 'picked up' not in normalized(m['message']).lower() for m in self.messages)
        self.record.update(complete=True,case=case,mode=mode,expected_terminal_contract=expected,battle_count=self.battle_count,first_return=self.first_return,markers=self.markers,battle_messages=self.messages,final_state=s,final_flags=self.flags,new_flags=sorted(true_flags(self.flags)-self.initial_flags),lost_flags=sorted(self.initial_flags-true_flags(self.flags)))


def common_plan(plan_path,expected_sha):
    assert sha(plan_path)==expected_sha;plan=load(plan_path)
    assert plan['schema']==SCHEMA and plan['controller']['sha256']==sha(__file__) and Path(plan['controller']['path']).resolve()==Path(__file__).resolve()
    assert plan['runtime']['seed']==42 and plan['runtime']['speed']==0
    assert plan['case_order']==CASES and plan['discovery_budget']==6000 and plan['fixed_tail_frames']==1088
    assert plan['before']['head']==BASE and plan['candidate_expected_head']==CANDIDATE
    assert set(plan['cases'])==set(CASES)
    assert plan['expected']['trainer_flag']=='EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_2'
    assert plan['expected']['messages']['selfdestruct-used']=='GEODUDE used SELFDESTRUCT!'
    assert plan['expected']['messages']['prize']=='RED got $90 for\nwinning!'
    return plan

def check_fixture(plan,case):
    spec=plan['cases'][case];p=pinned(spec['physical_SRAM']);assert p.stat().st_size==32768 and not p.with_suffix('.script_flags.json').exists()
    codec=load(pinned(spec['codec_receipt']));assert codec['reserve']==(case=='healthy-reserve')
    assert codec['all_other_serialized_fields_exactly_equal'] and codec['sram_checksum_validated'] and codec['reexport_all_32768_bytes_equal']
    typed=load(pinned(spec['roundtrip_JSON']));party=typed['party']
    # Party serializes as a bounded vector; reject unknown schema before Game.
    assert isinstance(party,list) and len(party)==(1 if case=='only-geodude' else 2)
    geo=party[0];assert geo['species']=='Geodude' and geo['level']==21 and geo['hp']==1 and geo['max_hp']==54
    assert geo['moves']==MOVE_NAMES and geo['pp'][3]==5 and geo['dv_bytes']==[255,255]
    if case=='healthy-reserve':assert party[1]['species']=='Pidgey' and party[1]['level']==5 and party[1]['hp']==party[1]['max_hp']>0
    gd=typed['game_data'];assert gd['player_money']==1000 and gd['player_coins']==0 and gd['player_moving_direction']==0
    assert gd['position']['map_id']==51 and [gd['position']['x'],gd['position']['y']]==[2,19]
    assert not gd['event_flags'][0x564>>3]&(1<<(0x564&7))
    return spec['physical_SRAM']

def schedule_markers(actors,schedule,seen,frame):
    for a in actors:
        for key,first in a.markers.items():
            marker=(a.label,key)
            if marker not in seen:
                seen.add(marker)
                for offset in [0,1,2,8,16]:schedule.setdefault(first+offset,[]).append(a.label+'-'+key)
        if a.messages and a.messages[-1]['frame']==frame:
            for offset in [0,1,2,8]:schedule.setdefault(frame+offset,[]).append(a.label+'-current-message')
        if a.first_return==frame:
            for offset in [0,1,2,8,32]:schedule.setdefault(frame+offset,[]).append(a.label+'-first-return')
        # Capture transition phase boundaries, not every countdown value.
        tag=(a.current.get('warp_fade') or '').split(' ')[0].split('{')[0]
        old=getattr(a,'capture_warp_tag',None)
        if tag!=old:
            a.capture_warp_tag=tag
            for offset in [0,1,2,8]:schedule.setdefault(frame+offset,[]).append(a.label+'-warp-'+tag)

def run(plan,plan_path,mode,case,out):
    assert not out.exists();out.mkdir();report={'controller':artifact(__file__),'plan':artifact(plan_path),'started':now(),'runner_pid':os.getpid(),'runner_pgid':os.getpgrp(),'mode':mode,'case':case,'complete':False,'all_pass':False,'Game_calls':0,'Cargo_calls':0,'rustc_calls':0,'actors':{},'captures':[],'actual_buttons':[],'candidate_input_adaptive':False,'RNG_equality_claim':False}
    write(out/'result.json',report);actors=[];providers={};schedule={0:['cold-boot']};seen=set();old=signal.signal(signal.SIGTERM,interrupted);oldint=signal.getsignal(signal.SIGINT)
    try:
        assert plan['ready_for_'+mode] is True
        fixture=check_fixture(plan,case)
        providers={'before':plan['before']}
        vector=None
        if mode=='pair':
            assert plan['after'] is not None and plan['after']['head']==CANDIDATE
            providers['after']=plan['after'];spec=plan['cases'][case]
            discovery=load(pinned(spec['discovery_receipt']))
            assert discovery['mode']=='discover' and discovery['case']==case and discovery['complete'] and discovery['all_pass'] and discovery['all_Game_groups_gone']
            assert discovery['actors']['before']['head']==BASE and discovery['actors']['before']['physical_SRAM']==fixture
            vector=load(pinned(spec['fixed_vector']));actual=load(pinned(spec['actual_discovery_vector']))
            assert discovery['fixed_vector']==spec['fixed_vector'] and discovery['literal_actual_vector']==spec['actual_discovery_vector']
            assert vector==actual+TAIL and len(vector)==spec['fixed_vector']['frames']
            assert all(b in [[],['A'],['B'],['START'],['UP']] for b in vector)
        report['source_guards_before']={label:source_guard(provider) for label,provider in providers.items()}
        if mode=='pair':
            bf=load(pinned(providers['before']['source_freeze']));af=load(pinned(providers['after']['source_freeze']));bg=providers['before']['guard'];ag=providers['after']['guard']
            assert bf[bg['engine_key']]==af[ag['engine_key']] and bf[bg['gfx_key']]==af[ag['gfx_key']]
        ports=plan['ports'][mode];assert len(ports)==len(providers) and len(set(ports))==len(ports)
        for port in ports:
            with socket.socket() as probe:probe.bind(('127.0.0.1',port))
        for (label,provider),port in zip(providers.items(),ports):
            a=Actor(label,provider,fixture,plan,out/label,report);actors.append(a);a.start(port)
        report['captures'].append({'frame':0,'images':[a.capture(['cold-boot']) for a in actors]})
        def advance(button):
            frame=len(report['actual_buttons'])+1
            assert mode=='pair' or frame<=plan['discovery_budget'],('declared discovery budget exhausted',frame)
            states=[a.step(button) for a in actors];report['actual_buttons'].append(button)
            assert all(s['frame_count']==frame for s in states)
            schedule_markers(actors,schedule,seen,frame)
            if frame in schedule:report['captures'].append({'frame':frame,'images':[a.capture(schedule[frame]) for a in actors]})
            if mode=='pair':
                changes=[k for k in SHARED if states[0].get(k)!=states[1].get(k)]
                if changes and report.get('first_current_public_split') is None:
                    assert all('selfdestruct-used' in a.markers and 'both-active-zero' in a.markers for a in actors),('unexpected prefix split',frame,changes)
                    report['first_current_public_split']={'frame':frame,'fields':changes,'scope':'current public state only, not future Debug text or pixel claim'}
            if frame%128==0:write(out/'result.json',report)
        if mode=='discover':
            before=actors[0]
            def block(button,total=32):
                assert total in (32,60)
                for i in range(total):advance([button] if button is not None and i<2 else [])
            # Cold SRAM boots visibly through language and Continue. No skip.
            boot_screens=set()
            while before.current['screen']!='overworld':
                screen=before.current['screen'];boot_screens.add(screen)
                if screen in ('gamefreak-splash','intro'):block(None)
                elif screen=='language-select':block('A')
                elif screen=='title':block('START',60)
                elif screen=='main-menu':block('A')
                else:raise AssertionError(('unexpected cold Continue screen',screen))
            assert {'language-select','title','main-menu'}<=boot_screens
            s=before.current;assert s['map_name']=='ViridianForest' and [s['player_x'],s['player_y']]==[2,19] and s['player_facing']=='Down'
            assert s['money']==1000 and len(s['party'])==(1 if case=='only-geodude' else 2)
            report['cold_Continue_anchor']={'frame':before.frame,'state':s,'normal_language_and_continue':True}
            block(None)
            assert before.current['screen']=='overworld' and not before.current['script_running'] and before.current['active_script_effect'] is None and before.current['dialogue'] is None
            block('UP')
            s=before.current;assert s['screen']=='overworld' and [s['player_x'],s['player_y']]==[2,19] and s['player_facing']=='Up'
            report['ordinary_UP_anchor']={'frame':before.frame,'state':s}
            block('A')
            assert before.current['screen']=='battle' or before.current['dialogue'] is not None or before.current['script_running'] or before.current['active_script_effect'] is not None,('one ordinary A did not engage; preserve and stop',before.current)
            while before.current['screen']!='battle':block('B')
            while before.current.get('battle_phase')!='PlayerMenu':block('B')
            block('A')
            s=before.current;assert s['screen']=='battle' and s['battle_phase']=='MoveSelect' and s['battle_moves']['cursor']==0
            assert [m['move'] for m in s['battle_moves']['moves']]==MOVE_NAMES
            block('UP')
            s=before.current;assert s['battle_phase']=='MoveSelect' and s['battle_moves']['cursor']==3 and s['battle_moves']['moves'][3]['move']=='Selfdestruct'
            report['ordinary_FIGHT_UPwrap_anchor']={'frame':before.frame,'state':s}
            block('A')
            assert 'selfdestruct-used' in before.markers,('the one declared move was not used',before.messages)
            while before.first_return is None:block('B')
            block(None)
            before.validate_final(case,mode)
            assert 'both-active-zero' in before.markers,('seed42 did not exercise doubleKO, no retry',before.markers)
            actual=out/'actual-buttons.json';fixed=out/'fixed-buttons.json'
            write(actual,report['actual_buttons']);write(fixed,report['actual_buttons']+TAIL)
            report['literal_actual_vector']={**artifact(actual),'frames':len(report['actual_buttons'])}
            report['fixed_vector']={**artifact(fixed),'frames':len(report['actual_buttons'])+len(TAIL),'base_only_plus_predeclared_tail':True}
            report['candidate_executions']=0
        else:
            assert vector is not None
            for button in vector:advance(button)
            for a in actors:a.validate_final(case,mode)
            assert actors[0].markers['selfdestruct-used']==actors[1].markers['selfdestruct-used'] and actors[0].markers['both-active-zero']==actors[1].markers['both-active-zero']
            if case=='healthy-reserve':
                for key in ['map_name','player_x','player_y','player_facing','party','money','coins','badges','map_blocks']:assert actors[0].current.get(key)==actors[1].current.get(key),('reserve control final delta',key)
                assert actors[0].flags==actors[1].flags
            else:assert report.get('first_current_public_split') is not None
            report['fixed_vector']=plan['cases'][case]['fixed_vector']
        final=actors[0].frame
        if not any(row['frame']==final for row in report['captures']):report['captures'].append({'frame':final,'images':[a.capture(['final-fixed-frame' if mode=='pair' else 'base-discovery-settled']) for a in actors]})
        report['all_pass']=True
    except BaseException:
        report.update(all_pass=False,error=traceback.format_exc(),first_failure_preserved=True);print(report['error'],flush=True)
    finally:
        signal.signal(signal.SIGTERM,signal.SIG_IGN);signal.signal(signal.SIGINT,signal.SIG_IGN)
        try:
            for a in actors:
                try:a.finish()
                except BaseException:report.update(all_pass=False);report.setdefault('cleanup_errors',[]).append({'label':a.label,'error':traceback.format_exc()})
            try:
                report['source_guards_after']={label:source_guard(provider) for label,provider in providers.items()};check_fixture(plan,case)
                report['frozen_input_bytes_unchanged']=True
                if mode=='pair':pinned(plan['cases'][case]['fixed_vector']);pinned(plan['cases'][case]['discovery_receipt'])
            except BaseException:report.update(all_pass=False,source_guard_error=traceback.format_exc())
            report['all_Game_groups_gone']=all(a.record.get('shutdown',{}).get('group_gone',a.proc is None) for a in actors)
            if not report['all_Game_groups_gone']:report['all_pass']=False
            report.update(complete=True,ended=now());write(out/'result.json',report)
        finally:signal.signal(signal.SIGTERM,old);signal.signal(signal.SIGINT,oldint)
    print(json.dumps({'result':str(out/'result.json'),'sha256':sha(out/'result.json'),'all_pass':report['all_pass'],'all_Game_groups_gone':report['all_Game_groups_gone']}),flush=True)
    return 0 if report['all_pass'] else 1

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--plan',required=True);ap.add_argument('--plan-sha256',required=True);ap.add_argument('--output',required=True)
    ap.add_argument('--dry-only',action='store_true');ap.add_argument('--mode',choices=['discover','pair']);ap.add_argument('--case',choices=CASES);ap.add_argument('--execute-reviewed-plan',action='store_true');args=ap.parse_args()
    path=Path(args.plan).resolve();plan=common_plan(path,args.plan_sha256)
    if args.dry_only:
        assert not args.execute_reviewed_plan and args.mode is None and args.case is None
        out=Path(args.output).resolve();assert not out.exists();out.mkdir()
        # Frozen metadata only; this cannot authorize Game or invent pending IDs.
        for p in ['binary','identity','source_freeze']:pinned(plan['before'][p])
        pinned(plan['fixture_source']);pinned(plan['prior_source_plan'])
        assert not plan['ready_for_discover'] and not plan['ready_for_pair'] and plan['after'] is None
        assert all(plan['cases'][c][k] is None for c in CASES for k in ['physical_SRAM','codec_receipt','roundtrip_JSON','discovery_receipt','actual_discovery_vector','fixed_vector'])
        result={'dry_plan_valid':True,'plan':artifact(path),'controller':artifact(__file__),'ready_for_execution':False,'pending_actuals':plan['pending_actuals'],'actual_Cargo_calls':0,'actual_rustc_calls':0,'actual_Game_calls':0,'actual_RPC_calls':0}
        write(out/'result.json',result);print(json.dumps({'dry_plan_valid':True,'result':str(out/'result.json'),'sha256':sha(out/'result.json')}));return 0
    assert args.execute_reviewed_plan and args.mode is not None and args.case is not None
    return run(plan,path,args.mode,args.case,Path(args.output).resolve())
if __name__=='__main__':sys.exit(main())

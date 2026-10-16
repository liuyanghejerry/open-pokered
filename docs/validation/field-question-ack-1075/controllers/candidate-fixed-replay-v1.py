#!/usr/bin/env python3
"""Prepared fixed candidate replay. Importing this file starts no Game.

Requires a separately pinned, Root-authorized READY plan with the actual
candidate CLI identity and all three completed baseline prefixes. Only those
literal inputs are submitted; state never selects a button or extends a run.
"""
from pathlib import Path
import argparse,hashlib,json,os,shutil,signal,socket,struct,subprocess,time,traceback

M9='25a9d8c6cac0aa4d662fcbac55aea718b637b038'
H='2c9b876e7983ea34852cafa249b0747068db9bac'
ORDER=['bike-sale','safari-admission','safari-return']
OPTIONS={'bike-sale':['BICYCLE ¥1000000','CANCEL'],'safari-admission':['YES','NO'],'safari-return':['YES','NO']}
INVARIANTS=['screen','map_id','map_name','player_x','player_y','player_facing','player_transport','player_movement_state','money','coins','party','party_count','badges','player_name','evaluation','map_blocks','last_outside_map']

def sha(p):
 h=hashlib.sha256()
 with Path(p).open('rb') as f:
  for b in iter(lambda:f.read(4194304),b''):h.update(b)
 return h.hexdigest()
def obj(p):return json.loads(Path(p).read_text())
def artifact(p):return dict(path=str(p),sha256=sha(p),bytes=Path(p).stat().st_size)
def put(p,x):Path(p).write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n')
def pin(a):
 p=Path(a['path']);assert p.is_absolute() and sha(p)==a['sha256'],str(p)
 if 'bytes' in a:assert p.stat().st_size==a['bytes']
 return p
def field(x,path):
 assert isinstance(path,list) and path
 for k in path:x=x[k]
 return x
def fingerprint(p):return hashlib.sha256(os.fsencode(os.readlink(p))).hexdigest() if p.is_symlink() else sha(p)
def gone(pg):
 try:os.killpg(pg,0);return False
 except ProcessLookupError:return True
def stop(proc):
 r=dict(pid=proc.pid,pgid=proc.pid,signals=[],scope='Owned shutdown after the fixed diagnostic boundary; not a natural Game success exit.')
 for sig,wait in [(signal.SIGTERM,8),(signal.SIGKILL,2)]:
  if not gone(proc.pid):
   try:os.killpg(proc.pid,sig);r['signals'].append(sig.name)
   except ProcessLookupError:pass
  try:proc.wait(timeout=wait)
  except subprocess.TimeoutExpired:pass
  if gone(proc.pid):break
 r.update(exit_code=proc.poll(),group_gone=gone(proc.pid));assert r['group_gone'] and r['exit_code'] is not None,r
 return r

def source_guard(provider,head):
 assert provider['head']==head and provider['kind']=='normal-CLI-debug-server'
 pin(provider['binary']);identity=obj(pin(provider['identity']))
 assert field(identity,provider['identity_head_path'])==head
 assert field(identity,provider['identity_binary_sha_path'])==provider['binary']['sha256']
 frozen=obj(pin(provider['source_guard']));assert frozen['head']==head
 root=Path(frozen['cwd']);assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==head
 assert subprocess.check_output(['git','status','--porcelain=v1'],cwd=root)==b''
 roster=subprocess.check_output(['git','ls-files','-z'],cwd=root);assert hashlib.sha256(roster).hexdigest()==frozen['git_roster_sha256']
 assert {n for n in roster.decode().split('\0') if n}==set(frozen['source'])
 for key,base in [('source',root),('engine981',Path(frozen['engine_cwd'])),('gfx702',Path(frozen['gfx_canonical']))]:
  for name,expected in frozen[key].items():assert fingerprint(base/name)==expected,(key,name)
 return dict(head=head,tracked=len(frozen['source']),engine=len(frozen['engine981']),gfx=len(frozen['gfx702']),all_exact=True)

def baseline_case(plan,name):
 """Freeze the no-ACK boundary and its existing absolute-frame captures."""
 c=plan['cases'][name];assert c['status']=='PINNED_COMPLETED_BASELINE'
 for a in c['pins']:pin(a)
 b=obj(pin(c['baseline_result']));prefix=obj(pin(c['literal_prefix']))
 assert b['source']==M9 and b['case']==name and b['all_pass'] and b['candidate_executions']==0
 assert b['controller_sha256']==plan['baseline_controller']['sha256']
 assert b['source_and_fixture_guard_after'] and b['owned_shutdown']['group_gone'] and gone(b['Game_pgid'])
 assert b['candidate_replay_prefix']['sha256']==c['literal_prefix']['sha256'] and b['candidate_replay_prefix']['frames']==len(prefix)
 assert b['baseline_only_ack_probe']=={'A_frames':1,'neutral_frames':32}
 assert all(x in {None,'a','b','up','down','start'} for x in prefix)
 actual=obj(pin(c['baseline_actual_buttons']));assert actual==prefix+['a']+[None]*32
 assert len(prefix)==b['final_question_frame']+64 and prefix[-64:]==[None]*64
 states=[json.loads(row) for row in pin(c['baseline_states']).read_text().splitlines()]
 assert len(states)==len(actual)+1 and [r['frame_count'] for r in states]==list(range(len(actual)+1))
 before_end=states[len(prefix)];assert before_end['choice'] is None
 d=before_end['dialogue_state'];assert d and d['page']==d['total_pages'] and d['waiting_for_input'] and not d['holding_open']
 captures=[v for v in b['captures'] if v['frame']<=len(prefix)]
 assert c['capture_frames']==[v['frame'] for v in captures] and len(c['capture_frames'])==len(set(c['capture_frames']))
 for v in captures:pin(v)
 assert b['final_state']['choice']=={'options':OPTIONS[name],'selected':0} and b['final_flags']==b['initial_flags']
 for key in INVARIANTS:assert before_end[key]==b['final_state'][key],('baseline extra-A changed persistent field',name,key)
 fixture=pin(c['sram']);assert fixture.stat().st_size==32768
 assert c['sram']==plan['before_discovery_fixtures'][name]['sram']
 return b,prefix,captures,before_end

class Replay:
 def __init__(self,plan,name,out,baseline,prefix,captures,baseline_end):
  self.plan,self.name,self.out=plan,name,out
  self.baseline,self.prefix,self.baseline_end=baseline,prefix,baseline_end
  self.capture_at={v['frame']:v for v in captures}
  self.frame=self.seq=0;self.buttons=[];self.state=None;self.proc=self.sock=self.stream=None;self.files=[]
  self.result=dict(schema='field-question-candidate-fixed-replay-case-v1',case=name,source=H,baseline_source=M9,seed=42,seed_configurations='one normal CLI preboot --seed42',runtime_setters=0,Game_calls=0,all_pass=False,runner_pid=os.getpid(),runner_pgid=os.getpgrp(),captures=[],events=[],literal_prefix=plan['cases'][name]['literal_prefix'],baseline_result=plan['cases'][name]['baseline_result'],adaptive_input_decisions=0,first_choice_frame=None)
 def persist(self):put(self.out/'result.json',self.result)
 def guards(self):
  for a in self.plan['pins']:pin(a)
  source_guard(self.plan['before_provider'],M9);source_guard(self.plan['after_provider'],H)
  for name,c in self.plan['cases'].items():
   for a in c['pins']:pin(a)
  return True
 def rpc(self,command,**kwargs):
  assert command in {'get_state','get_flags','get_bag','press_timeline','capture_frame'}
  request=dict(cmd=command,**kwargs);self.seq+=1
  self.rpc_log.write(json.dumps(dict(seq=self.seq,frame=self.frame,request=request))+'\n');self.rpc_log.flush()
  self.stream.write((json.dumps(request)+'\n').encode());self.stream.flush();raw=self.stream.readline(4*1024*1024)
  assert raw.endswith(b'\n'),('transport EOF/truncation',raw[:200]);reply=json.loads(raw)
  self.rpc_log.write(json.dumps(dict(seq=self.seq,frame=self.frame,response=reply))+'\n');self.rpc_log.flush()
  assert reply.get('ok') is True,reply
  return reply.get('data')
 def read(self):
  self.state=self.rpc('get_state');assert self.state['frame_count']==self.frame
  self.state_log.write(json.dumps(self.state,ensure_ascii=False)+'\n');self.state_log.flush()
  if self.state['choice'] is not None and self.result['first_choice_frame'] is None:self.result['first_choice_frame']=self.frame
  return self.state
 def capture(self,baseline):
  reason=baseline['reason'];path=self.out/f'{self.frame:05}-{reason}.png';assert not path.exists()
  old_flags=self.rpc('get_flags');old_bag=self.rpc('get_bag')
  receipt=self.rpc('capture_frame',path=str(path))
  assert self.rpc('get_state')==self.state and self.rpc('get_flags')==old_flags and self.rpc('get_bag')==old_bag,'capture changed public observation'
  raw=path.read_bytes();assert raw[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',raw[16:24])==(160,144)
  self.result['captures'].append(dict(frame=self.frame,reason=reason,path=str(path),sha256=sha(path),receipt=receipt,state=self.state,baseline_capture=dict(path=baseline['path'],sha256=baseline['sha256']),post_capture_state_flags_bag_unchanged=True))
 def start(self):
  self.guards();self.save=self.out/'pokered.sav';shutil.copyfile(pin(self.plan['cases'][self.name]['sram']),self.save)
  assert not self.save.with_suffix('.script_flags.json').exists()
  self.game_log=(self.out/'game.log').open('x');self.rpc_log=(self.out/'rpc.jsonl').open('x');self.state_log=(self.out/'states.jsonl').open('x');self.button_log=(self.out/'inputs.jsonl').open('x');self.files=[self.game_log,self.rpc_log,self.state_log,self.button_log]
  port=self.plan['ports'][self.name]
  with socket.socket() as probe:probe.bind(('127.0.0.1',port))
  env=os.environ.copy()
  for key in list(env):
   if key.startswith('POKERED_'):env.pop(key)
  environment=self.plan['runtime_environment'];assert set(environment)<={'LD_LIBRARY_PATH'};env.update(environment)
  argv=[self.plan['after_provider']['binary']['path'],'run','--headless','--speed','0','--seed','42','--debug-port',str(port),'--lang','en','--no-audio','--save',str(self.save)]
  self.proc=subprocess.Popen(argv,cwd=self.out,env=env,stdout=self.game_log,stderr=subprocess.STDOUT,start_new_session=True)
  self.result.update(Game_calls=1,argv=argv,Game_pid=self.proc.pid,Game_pgid=self.proc.pid);self.persist()
  for _ in range(200):
   assert self.proc.poll() is None,('early Game exit',self.proc.returncode)
   try:self.sock=socket.create_connection(('127.0.0.1',port),timeout=.1);break
   except (ConnectionRefusedError,TimeoutError):time.sleep(.05)
  assert self.sock is not None;self.sock.settimeout(30);self.sock.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1);self.stream=self.sock.makefile('rwb')
  self.read()
  if 0 in self.capture_at:self.capture(self.capture_at[0])
 def replay(self):
  for expected_frame,button in enumerate(self.prefix):
   assert self.frame==expected_frame
   receipt=self.rpc('press_timeline',buttons=[button],start_at_frame=self.frame,advance=True)
   assert receipt==dict(queue_start_frame=self.frame,start_frame=self.frame,end_frame=self.frame,frames=1,padding_frames=0,advanced=True,frame_count=self.frame+1),receipt
   self.buttons.append(button);self.button_log.write(json.dumps(dict(input_frame=self.frame,button=button,receipt=receipt))+'\n');self.button_log.flush()
   self.frame+=1;self.read()
   if self.frame in self.capture_at:self.capture(self.capture_at[self.frame])
  assert self.buttons==self.prefix and self.frame==len(self.prefix)
  assert self.state['choice']=={'options':OPTIONS[self.name],'selected':0} and self.state['active_script_effect']=='ShowChoice'
  assert self.state['dialogue_state'] is None and self.state['dialogue']==self.baseline['final_state']['dialogue']
  assert self.state['money']==1000 and self.rpc('get_bag')==[]
  flags=self.rpc('get_flags');assert flags==self.baseline['final_flags']==self.baseline['initial_flags']
  for key in INVARIANTS:assert self.state[key]==self.baseline_end[key],('changed persistent/final field',self.name,key)
  assert [v['frame'] for v in self.result['captures']]==self.plan['cases'][self.name]['capture_frames']
  put(self.out/'actual-replayed-buttons.json',self.buttons)
  self.result.update(all_pass=True,actual_frames=self.frame,final_state=self.state,final_flags=flags,baseline_only_probe_replayed=False,baseline_persistent_fields_identical=INVARIANTS,actual_buttons_sha256=sha(self.out/'actual-replayed-buttons.json'))
 def finish(self):
  errors=[]
  for label,resource in [('stream',self.stream),('socket',self.sock)]:
   if resource is not None:
    try:resource.close()
    except BaseException:errors.append(dict(operation=label+'.close',error=traceback.format_exc()))
  try:
   if self.proc is not None:self.result['owned_shutdown']=stop(self.proc)
  finally:
   try:
    self.guards()
    if hasattr(self,'save'):
     assert sha(self.save)==self.plan['cases'][self.name]['sram']['sha256'] and not self.save.with_suffix('.script_flags.json').exists()
    self.result['source_and_fixture_guard_after']=True
   except BaseException:errors.append(dict(operation='after-guards',error=traceback.format_exc()))
   for f in self.files:
    try:f.close()
    except BaseException:errors.append(dict(operation='log.close',error=traceback.format_exc()))
   put(self.out/'all-accepted-buttons-including-failure.json',self.buttons)
   okay=self.proc is None or self.result.get('owned_shutdown',{}).get('group_gone')
   self.result['seam_assertions_passed_before_cleanup']=self.result['all_pass'];self.result['all_pass']=bool(self.result['all_pass'] and not errors and okay)
   self.result.update(cleanup_errors=errors,last_frame=self.frame,last_state=self.state,controller_sha256=sha(__file__),rpc_count=self.seq);self.persist()
  assert not errors and okay

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--plan',required=True);parser.add_argument('--plan-sha256',required=True);parser.add_argument('--execute-reviewed-candidate-fixed-replay',action='store_true',required=True);args=parser.parse_args()
 path=Path(args.plan);assert sha(path)==args.plan_sha256;plan=obj(path)
 assert plan['state']=='READY' and plan['root_authorized_candidate_execution'] is True and plan['controller_sha256']==sha(__file__)
 assert plan['before_head']==M9 and plan['candidate_head']==H and plan['case_order']==ORDER and isinstance(plan['after_provider'],dict)
 for item in plan['pins']:pin(item)
 assert obj(pin(plan['before_discovery_plan']))['controller_sha256']==plan['baseline_controller']['sha256']
 prepared={name:baseline_case(plan,name) for name in ORDER}
 source_guard(plan['before_provider'],M9);source_guard(plan['after_provider'],H)
 out=Path(plan['output']);assert out.is_absolute() and not out.exists();out.mkdir()
 result=dict(schema='field-question-three-fixed-candidate-replays-v1',source=H,baseline_source=M9,plan=artifact(path),controller=artifact(Path(__file__)),runner_pid=os.getpid(),runner_pgid=os.getpgrp(),complete=False,all_pass=False,Game_calls=0,Cargo_calls=0,rustc_calls=0,cases=[])
 def interrupt(signum,frame):raise KeyboardInterrupt(f'Signal{signum}; enter owned cleanup')
 signal.signal(signal.SIGTERM,interrupt)
 try:
  put(out/'result.json',result)
  for name in ORDER:
   folder=out/name;folder.mkdir();actor=Replay(plan,name,folder,*prepared[name]);failure=None
   try:actor.start();actor.replay()
   except BaseException:
    failure=traceback.format_exc();actor.result['first_failure']=failure
   finally:
    try:actor.finish()
    except BaseException:
     cleanup_failure=traceback.format_exc();failure=(failure or '')+'\n'+cleanup_failure
    result['Game_calls']+=actor.result['Game_calls'];result['cases'].append(dict(case=name,result=artifact(folder/'result.json'),all_pass=actor.result['all_pass'],Game_pgid=actor.result.get('Game_pgid')));put(out/'result.json',result)
   if failure:raise AssertionError(('first fixed replay failure; later cases not started',name,failure))
   assert actor.result['all_pass']
  result.update(complete=True,all_pass=True,source_guard_before_after=True);put(out/'result.json',result);print(json.dumps(dict(all_pass=True,result=artifact(out/'result.json'))))
 except BaseException:
  result.update(complete=True,all_pass=False,first_failure=traceback.format_exc());put(out/'result.json',result);raise
if __name__=='__main__':main()

#!/usr/bin/env python3
"""Replay one published literal tape through an already-built normal CLI."""
from pathlib import Path
import argparse, hashlib, json, os, shutil, signal, socket, subprocess, time, traceback
HERE=Path(__file__).resolve().parent
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def put(p,v):Path(p).write_text(json.dumps(v,indent=2)+'\n')
def gone(pg):
 try:os.killpg(pg,0)
 except ProcessLookupError:return True
 return False
def stop(proc):
 rec={'pid':proc.pid,'pgid':proc.pid,'signals':[]}
 for sig,timeout in [(signal.SIGTERM,8),(signal.SIGKILL,2)]:
  if not gone(proc.pid):
   try:os.killpg(proc.pid,sig);rec['signals'].append(sig.name)
   except ProcessLookupError:pass
  try:proc.wait(timeout=timeout)
  except subprocess.TimeoutExpired:pass
  if gone(proc.pid):break
 rec.update(exit_code=proc.poll(),group_gone=gone(proc.pid));assert rec['group_gone'] and rec['exit_code'] is not None
 return rec
class Run:
 def __init__(self,a):
  self.a=a;self.proc=self.sock=self.stream=None;self.files=[];self.frame=0
  self.meta=json.loads((HERE/'validation.json').read_text());self.case=self.meta['cases'][a.case]
  self.buttons=json.loads((HERE/self.case['inputs']).read_text())
  assert sha(HERE/self.case['inputs'])==self.case['input_sha256'] and len(self.buttons)==self.case['frames']
  assert self.buttons[self.case['welcome_suffix_origin']:]==['b']*2+['a']*64
  self.root=Path(a.source_root).resolve();self.binary=Path(a.binary).resolve();self.out=Path(a.output).resolve()
  assert self.binary.is_file() and not self.out.exists()
  actual=subprocess.check_output(['git','rev-parse','HEAD'],cwd=self.root).decode().strip()
  assert actual==self.case['source'],('check out exact recorded source',self.case['source'],actual)
  assert not subprocess.check_output(['git','status','--porcelain','--untracked-files=no'],cwd=self.root)
  self.fixture=self.root/self.meta['fixture']['path'];assert sha(self.fixture)==self.meta['fixture']['sha256']
  self.result={'case':a.case,'declared_build_tree_head':actual,'binary_sha256':sha(self.binary),'seed':42,'all_pass':False,'Game_calls':0,'runtime_setters':0,'scope':'literal physical-input reproduction; source checkout validation alone does not prove binary compiler provenance'}
 def rpc(self,cmd,**kw):
  assert cmd in {'get_state','get_bag','get_flags','press_timeline','capture_frame'}
  self.stream.write((json.dumps({'cmd':cmd,**kw})+'\n').encode());self.stream.flush();raw=self.stream.readline(4*1024*1024)
  assert raw.endswith(b'\n'),'transport EOF/truncation';reply=json.loads(raw);assert reply.get('ok') is True,reply
  return reply.get('data')
 def read(self):
  s=self.rpc('get_state');assert s['frame_count']==self.frame
  self.states.write(json.dumps(s)+'\n');self.states.flush();return s
 def play(self):
  self.out.mkdir();self.save=self.out/'pokered.sav';shutil.copyfile(self.fixture,self.save)
  log=(self.out/'game.log').open('x');self.states=(self.out/'states.jsonl').open('x');self.files=[log,self.states]
  with socket.socket() as probe:probe.bind(('127.0.0.1',self.a.port))
  env={k:v for k,v in os.environ.items() if not k.startswith('POKERED_')}
  argv=[str(self.binary),'run','--headless','--speed','0','--seed','42','--debug-port',str(self.a.port),'--lang','en','--no-audio','--save',str(self.save)]
  self.proc=subprocess.Popen(argv,cwd=self.out,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  self.result.update(Game_calls=1,argv=argv,pid=self.proc.pid);put(self.out/'result.json',self.result)
  for _ in range(200):
   assert self.proc.poll() is None,('early Game exit',self.proc.returncode)
   try:self.sock=socket.create_connection(('127.0.0.1',self.a.port),timeout=.1);break
   except (ConnectionRefusedError,TimeoutError):time.sleep(.05)
  assert self.sock is not None;self.sock.settimeout(30);self.sock.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1);self.stream=self.sock.makefile('rwb')
  state=self.read();flags=None
  for button in self.buttons:
   assert button in {None,'a','b','up','down','start'}
   receipt=self.rpc('press_timeline',buttons=[button],start_at_frame=self.frame,advance=True)
   assert receipt=={'queue_start_frame':self.frame,'start_frame':self.frame,'end_frame':self.frame,'frames':1,'padding_frames':0,'advanced':True,'frame_count':self.frame+1},receipt
   self.frame+=1;state=self.read()
   if self.frame==670:
    assert state['screen']=='overworld' and state['map_name']=='BikeShop' and (state['player_x'],state['player_y'])==(6,4) and state['money']==1000
    flags=self.rpc('get_flags');assert self.rpc('get_bag')==[] and not flags.get('EVENT_GOT_BICYCLE',False)
  if self.a.case=='before':assert state['active_script_effect']=='ShowChoice' and state['choice']['selected']==0
  else:
   offer=state['script_effect']['state'];assert state['active_script_effect']=='BikeShopOffer' and offer['accepted_buttons']==1 and offer['selected']==0 and offer['phase']=={'Prompt':'Goodbye'}
  assert state['money']==1000 and self.rpc('get_bag')==[] and self.rpc('get_flags')==flags
  image=self.out/(self.a.case+'.png');self.rpc('capture_frame',path=str(image));assert self.rpc('get_state')==state
  self.result.update(all_pass=True,frames=self.frame,relative_suffix_frames=66,image_sha256=sha(image))
 def finish(self):
  errors=[]
  for resource in [self.stream,self.sock]:
   if resource is not None:
    try:resource.close()
    except BaseException:errors.append(traceback.format_exc())
  try:
   if self.proc is not None:self.result['owned_diagnostic_shutdown']=stop(self.proc)
  finally:
   for f in self.files:f.close()
   if hasattr(self,'save'):assert sha(self.save)==self.meta['fixture']['sha256'] and not self.save.with_suffix('.script_flags.json').exists()
   self.result.update(cleanup_errors=errors,all_pass=bool(self.result['all_pass'] and not errors))
   if self.out.exists():put(self.out/'result.json',self.result)
  assert not errors

def main():
 p=argparse.ArgumentParser();p.add_argument('--case',choices=['before','after'],required=True);p.add_argument('--binary',required=True);p.add_argument('--source-root',required=True);p.add_argument('--output',required=True);p.add_argument('--port',type=int,default=19385);a=p.parse_args();r=Run(a)
 def interrupt(*_):raise KeyboardInterrupt('received signal; clean up owned Game')
 signal.signal(signal.SIGTERM,interrupt);signal.signal(signal.SIGINT,interrupt)
 try:r.play()
 except BaseException:r.result['failure']=traceback.format_exc();r.result['all_pass']=False;raise
 finally:r.finish()
 assert r.result['all_pass']
if __name__=='__main__':main()

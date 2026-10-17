#!/usr/bin/env python3
"""Replay ordinary button tapes against a normal desktop/debug-server CLI.
Only observation, timeline input, and screenshot RPCs are used. Each case
starts a new process with its own copy of a physical SRAM fixture.
"""
import argparse,hashlib,json,os,pathlib,shutil,signal,socket,subprocess,time
P=pathlib.Path(__file__).resolve().parent

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 a=argparse.ArgumentParser(description=__doc__);a.add_argument('--binary',type=pathlib.Path,required=True);a.add_argument('--case',required=True);a.add_argument('--output',type=pathlib.Path,required=True);a.add_argument('--port',type=int,default=19089);args=a.parse_args()
 manifest=json.loads((P/'manifest.json').read_text());case=manifest['cases'][args.case];args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
 for name,h in manifest['files'].items():assert sha(P/name)==h,name
 sav=args.output/'pokered.sav';shutil.copyfile(P/case['fixture'],sav);env={k:v for k,v in os.environ.items()if not k.startswith('POKERED_')};binary=args.binary.resolve()
 log=(args.output/'game.log').open('wb');rpc_log=(args.output/'rpc.jsonl').open('w');proc=None;sock=None;stream=None;result={'all_pass':False,'case':args.case,'binary_sha256':sha(binary),'fixture_sha256':sha(sav),'runtime_setters':0}
 def rpc(cmd,**kw):
  request={'cmd':cmd,**kw};rpc_log.write(json.dumps({'request':request})+'\n');rpc_log.flush();stream.write((json.dumps(request)+'\n').encode());stream.flush();reply=json.loads(stream.readline(4*1024*1024));assert reply['ok'],reply;return reply.get('data')
 try:
  proc=subprocess.Popen([str(binary),'run','--headless','--speed','0','--seed','42','--debug-port',str(args.port),'--lang','en','--no-audio','--save',str(sav)],cwd=args.output,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  for _ in range(200):
   assert proc.poll()is None
   try:sock=socket.create_connection(('127.0.0.1',args.port),timeout=.1);break
   except (OSError,TimeoutError):time.sleep(.05)
  assert sock is not None;sock.settimeout(30);sock.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1);stream=sock.makefile('rwb');state=rpc('get_state');assert state['frame_count']==0
  tape=json.loads((P/case['tape']).read_text());frame=0;captures=set(case.get('capture_frames',[]))
  while frame<len(tape):
   end=min(frame+60,len(tape),min((f for f in captures if f>frame),default=len(tape)))
   receipt=rpc('press_timeline',buttons=tape[frame:end],start_at_frame=frame,advance=True);assert receipt['frame_count']==end and receipt['advanced'];frame=end
   if frame in captures:rpc('capture_frame',path=str(args.output/(str(frame)+'.png')))
  state=rpc('get_state');flags=rpc('get_flags');npcs=rpc('get_npcs');bag=rpc('get_bag')
  for key,value in case['expected_state'].items():assert state[key]==value,(key,state[key],value)
  for key,value in case.get('expected_flags',{}).items():assert bool(flags.get(key,False))==value,(key,flags)
  if 'expected_bag'in case:assert bag==case['expected_bag'],bag
  if 'hidden_npc'in case:assert not next(n for n in npcs if n['text_id']==case['hidden_npc'])['visible']
  if case.get('save_output'):
   raw=sav.read_bytes();assert len(raw)==32768 and ((~sum(raw[0x2598:0x3523]))&255)==raw[0x3523];assert sha(sav)!=result['fixture_sha256']
  else:assert sha(sav)==result['fixture_sha256']
  result.update(all_pass=True,frames=frame,state=state,flags=flags,bag=bag,npcs=npcs,saved_sha256=sha(sav));rpc('capture_frame',path=str(args.output/'final.png'))
 finally:
  if stream:stream.close()
  if sock:sock.close()
  if proc and proc.poll()is None:
   os.killpg(proc.pid,signal.SIGTERM)
   try:proc.wait(timeout=10)
   except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);proc.wait()
  if proc:result['owned_exit_code']=proc.returncode
  log.close();rpc_log.close();(args.output/'result.json').write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps({'all_pass':result['all_pass'],'case':args.case,'frames':frame}))
if __name__=='__main__':main()

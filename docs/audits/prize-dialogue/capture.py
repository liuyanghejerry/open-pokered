from pathlib import Path
import sys,subprocess,os,socket,json
root=Path('/home/jerry/develop/open-pokered');sys.path.insert(0,str(root/'scripts'))
from debug_drive import DebugClient
r=root/'.task-tmp/fidelity-39-plus';label=sys.argv[1];binary=Path(sys.argv[2]);cwd=Path(sys.argv[3]);out=r/f'audit-775-{label}';out.mkdir(exist_ok=True)
s=socket.socket();s.bind(('127.0.0.1',0));port=s.getsockname()[1];s.close();env=os.environ.copy();env['LD_LIBRARY_PATH']='/tmp/pokered-build-lib';env['TMPDIR']=str(root/'.task-tmp/fidelity-18-27')
with (out/'game.log').open('w') as log:
 p=subprocess.Popen([str(binary),'run','--headless','--speed','0','--debug-port',str(port),'--skip-intro','--warp','GameCornerPrizeRoom,2,3','--lang','en','--record-frames',str(out/'frames'),'--save',str(out/'isolated.sav')],cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT)
 try:
  d=DebugClient(port);trace=[]
  def event(name):
   st=d.state();trace.append({'stage':name,'state':st});print(label,name,st['screen'],st.get('active_script_effect'),st.get('choice'),flush=True);return st
  d.step(120);d.cmd(cmd='give_item',item='COIN_CASE',qty=1);d.drive(['up']);d.step(40);d.drive(['a']);d.step(80);event('first-dialogue')
  for i in range(6):
   st=d.state()
   if st.get('choice'):break
   d.cmd(cmd='skip_dialogue');d.step(10)
  st=event('list');assert st.get('choice'),st
  for i in range(3):d.drive(['down']);d.step(2)
  event('no-thanks-selected');d.drive(['a']);d.step(100);event('after-list-exit')
  (out/'trace.json').write_text(json.dumps(trace,indent=2)+'\n');d.close()
 finally:
  p.terminate();p.wait(timeout=10)

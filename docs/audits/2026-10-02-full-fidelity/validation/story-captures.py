"""Fixed seed/input screenshot fixtures for original-fidelity story repairs."""
from pathlib import Path
import sys,json,subprocess,argparse
ROOT=Path(__file__).resolve().parents[4]
sys.path.insert(0,str(ROOT/'scripts'))
from debug_drive import DebugClient
p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--label',required=True);p.add_argument('--port',type=int,default=19012);a=p.parse_args()
OUT=ROOT/'docs/screenshots/fidelity-story';OUT.mkdir(parents=True,exist_ok=True)
log=[]
def cmd(**kw):
 r=c.cmd(**kw);log.append({'command':kw,'response':r})
 if not r.get('ok'):raise RuntimeError(r)
 return r.get('data')
def state():return cmd(cmd='get_state')
def warp(map,x,y):cmd(cmd='warp',map=map,x=x,y=y);cmd(cmd='step_frames',count=60)
def flag(name):cmd(cmd='set_flag',name=name,value=True)
def talk(direction='up'):cmd(cmd='press_timeline',buttons=[direction,None,'a',None],advance=True)
def drain_until_moving():
 for _ in range(150):
  s=state();eff=s.get('active_script_effect')
  if eff=='MoveNpc':return
  if s['dialogue'] is not None:cmd(cmd='skip_dialogue')
  elif s['choice'] is not None:cmd(cmd='press_timeline',buttons=['a',None],advance=True)
  else:cmd(cmd='step_frames',count=1)
 raise RuntimeError('No NPC movement')
def capture(name):
 path=OUT/f'{name}-{a.label}.png';cmd(cmd='capture_frame',path=str(path))
 (OUT/f'{name}-{a.label}.json').write_text(json.dumps({'seed':42,'state':state(),'npcs':cmd(cmd='get_npcs'),'transcript':log},indent=2)+'\n')
 print(path.name,flush=True)
def begin():
 global log
 log=[];cmd(cmd='restore_state',slot=0)
process=subprocess.Popen([a.binary,'run','--headless','--debug-port',str(a.port),'--skip-intro','--seed','42','--speed','0'],cwd=ROOT,stdout=(OUT/f'capture-{a.label}.log').open('w'),stderr=subprocess.STDOUT)
try:
 c=DebugClient(a.port);cmd(cmd='step_frames',count=2);cmd(cmd='save_state',slot=0)
 begin();flag('EVENT_FOLLOWED_OAK_INTO_LAB');flag('EVENT_GOT_TOWN_MAP');flag('EVENT_ENTERED_BLUES_HOUSE');warp('PalletTown',15,6);warp('BluesHouse',3,7);capture('daisy-reentry')
 begin();warp('BillsHouse',6,4);talk('down');drain_until_moving();cmd(cmd='step_frames',count=10);capture('bill-walk-around-player')
 begin();flag('EVENT_BILL_SAID_USE_CELL_SEPARATOR');warp('BillsHouse',1,5);talk();
 for _ in range(120):
  s=state()
  if s['dialogue'] is not None:cmd(cmd='skip_dialogue');break
  cmd(cmd='step_frames',count=1)
 cmd(cmd='step_frames',count=16);capture('bill-exits-machine')
 begin();warp('PalletTown',10,2);cmd(cmd='press_timeline',buttons=['up']*18+[None],advance=True)
 for _ in range(120):
  s=state()
  if s['dialogue'] is not None:cmd(cmd='skip_dialogue');break
  cmd(cmd='step_frames',count=1)
 cmd(cmd='step_frames',count=12);capture('oak-player-exclamation')
 begin();flag('EVENT_FOLLOWED_OAK_INTO_LAB');warp('CeruleanCity',21,7);cmd(cmd='press_timeline',buttons=['up']*18+[None],advance=True);cmd(cmd='step_frames',count=16);capture('cerulean-rival-right')
 c.close()
finally:process.terminate();process.wait(timeout=10)

#!/usr/bin/env python3
import argparse,sys,json,subprocess
from pathlib import Path
ap=argparse.ArgumentParser(description="Capture four native ground-item before/after regressions.")
ap.add_argument('--before',required=True);ap.add_argument('--after',required=True)
ap.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[3])
opt=ap.parse_args();root=opt.root.resolve();sys.path.insert(0,str(root/'scripts'));from debug_drive import DebugClient
out=root/'docs/screenshots/fidelity-world';out.mkdir(parents=True,exist_ok=True);trace={}
items=json.loads((root/'crates/pokered-data/data/items/item_list.json').read_text())['items']
cases=[('full-bag','Route2',13,55,0),('repeat-pickup','MtMoonB2F',25,22,7),('correct-tm','MtMoon1F',5,33,12),('missing-handler','PowerPlant',7,26,9)]
for label,binary in [('before',opt.before),('after',opt.after)]:
 for i,(case,mapname,x,y,npc) in enumerate(cases):
  port=9430+i
  log=open('/tmp/world-ground-'+case+'-'+label+'.log','w')
  proc=subprocess.Popen([binary,'run','--skip-intro','--warp',f'{mapname},{x},{y}','--seed','123','--no-audio','--headless','--debug-port',str(port),'--speed','0'],stdout=log,stderr=log,cwd=root)
  try:
   d=DebugClient(port);d.cmd(cmd='step_frames',count=30)
   if case=='full-bag':
    # Twenty actual distinct native Inventory entries, excluding Moon Stone.
    for item in items[:21]:
     if item!='MoonStone':d.cmd(cmd='give_item',item=item,qty=1)
    d.cmd(cmd='step_frames',count=2)
   beforebag=d.cmd(cmd='get_bag')
   interaction=d.cmd(cmd='interact_with',id=f'npc:{npc}')
   ready=d.cmd(cmd='wait_until',condition='dialogue_ready',max_frames=300)
   d.cmd(cmd='capture_frame',path=str(out/f'ground-{case}-{label}-dialogue.png'))
   d.cmd(cmd='skip_dialogue');d.cmd(cmd='wait_until',condition='control_ready',max_frames=300)
   first={'bag':d.cmd(cmd='get_bag'),'npcs':d.cmd(cmd='get_npcs'),'flags':d.cmd(cmd='get_flags'),'state':d.cmd(cmd='get_state')}
   d.cmd(cmd='capture_frame',path=str(out/f'ground-{case}-{label}-closed.png'))
   second=None
   if case=='repeat-pickup':
    d.cmd(cmd='interact_with',id=f'npc:{npc}');d.cmd(cmd='step_frames',count=30);d.cmd(cmd='skip_dialogue');d.cmd(cmd='wait_until',condition='control_ready',max_frames=300)
    second={'bag':d.cmd(cmd='get_bag'),'npcs':d.cmd(cmd='get_npcs')}
   trace[f'{case}-{label}']={'initial_bag':beforebag,'interaction':interaction,'ready':ready,'first_pickup':first,'second_pickup':second}
  finally:
   proc.terminate();proc.wait(timeout=8);log.close()
(root/'docs/audits/2026-10-02-full-fidelity/ground-native-trace.json').write_text(json.dumps(trace,indent=2)+'\n')
for key,v in trace.items():
 print(key, 'start',v['initial_bag'],'first',v['first_pickup']['bag'],'second',v['second_pickup'] and v['second_pickup']['bag'])

assert not trace['full-bag-before']['first_pickup']['npcs']['data'][0]['visible']
assert trace['full-bag-after']['first_pickup']['npcs']['data'][0]['visible']
assert trace['repeat-pickup-before']['second_pickup']['bag']['data']==[{'item':'HpUp','qty':2}]
assert trace['repeat-pickup-after']['second_pickup']['bag']['data']==[{'item':'HpUp','qty':1}]
assert trace['correct-tm-before']['first_pickup']['bag']['data']==[{'item':'Tm34','qty':1}]
assert trace['correct-tm-after']['first_pickup']['bag']['data']==[{'item':'Tm12','qty':1}]
assert trace['missing-handler-before']['first_pickup']['bag']['data']==[]
assert trace['missing-handler-after']['first_pickup']['bag']['data']==[{'item':'Carbos','qty':1}]
print('Native ground pickup regressions: all 8 before/after outcomes confirmed.')

import json,sys,time,traceback,shutil,os
from pathlib import Path
ROOT=Path('/Users/liuyanghe02/develop/open-pokered-3')
OUT=Path('/tmp/pokered-completion-20260907-zapdos-visible-repro')
sys.path[:0]=[str(ROOT/'scripts'),str(ROOT/'docs/audits/2026-09-06/playthrough')]
import playthrough
from playthrough import resume_reentry,NavError
from navigation import AuditGame
playthrough.BIN=OUT/'bin/pokered-app'
shutil.copy2(ROOT/'target/debug/pokered-app',playthrough.BIN)
shutil.copy2(ROOT/'docs/audits/2026-09-07/m65-zapdos-caught.sav',OUT/'active.sav')
shutil.copy2(ROOT/'docs/audits/2026-09-07/m65-zapdos-caught.script_flags.json',OUT/'bin/pokered.script_flags.json')
g=AuditGame(save_path=OUT/'active.sav')
orig=g.d.cmd
trace=(OUT/'protocol.jsonl').open('a')
seen=set();shot_count=0

cancel_requested=False
def cmd(**kw):
 global shot_count,cancel_requested
 if cancel_requested:
  cancel_requested=False
  raise NavError("audit cancelled at protocol boundary")
 r=orig(**kw)
 trace.write(json.dumps({'request':kw,'response':r},ensure_ascii=False)+'\n');trace.flush()
 if kw.get('cmd')=='get_state' and r.get('ok'):
  s=r['data'];key=(s['map_name'],s['screen'],s.get('battle_message'),s.get('dialogue'),s.get('active_script_effect'))
  if key not in seen:
   seen.add(key);shot_count+=1
   p=OUT/'shots'/f'{shot_count:05d}-{s["map_name"]}-{s["screen"]}.png'
   cr=orig(cmd='capture_frame',path=str(p));assert cr['ok'],cr
   (p.with_suffix('.json')).write_text(json.dumps(s,indent=2))
 return r
g.d.cmd=cmd

def snap(name):
 s=g.st();d={'state':s,'flags':g.d.cmd(cmd='get_flags'),'bag':g.d.cmd(cmd='get_bag'),'party':g.d.cmd(cmd='get_party'),'npcs':g.d.cmd(cmd='get_npcs')}
 (OUT/(name+'.json')).write_text(json.dumps(d,indent=2))
 r=g.d.cmd(cmd='capture_frame',path=str(OUT/'shots'/(name+'.png')));assert r['ok'],r
 return s

def checkpoint(name):
 s=snap(name)
 assert s['screen']=='overworld' and not s['script_running'],s
 r=g.d.cmd(cmd='save');assert r['ok'],r
 shutil.copy2(OUT/'active.sav',OUT/(name+'.sav'))
 p=OUT/'bin/pokered.script_flags.json'
 if p.exists():shutil.copy2(p,OUT/(name+'.script_flags.json'))
 print('CHECKPOINT',name,s['map_name'],s['player_x'],s['player_y'],s['party'],flush=True)

import types
def advance_cutscene(self,max_rounds=300):
 for _ in range(max_rounds):
  r=self.d.cmd(cmd='wait_until',condition='control_ready',max_frames=240)
  if r['data']['reached']:return True
  s=r['data']['state']
  if s['screen']!='overworld':return True
  if s['choice'] is not None:raise NavError(f"choice requires decision {s['choice']}")
  if s['dialogue_state'] is not None:self.skip()
  elif s['script_running'] and s['active_script_effect'] is None:
   print('SCRIPT_INPUT_WAIT',s['map_name'],s['player_x'],s['player_y'],flush=True)
   self.tap('a',12)
 return False
g.cutscene=types.MethodType(advance_cutscene,g)

g.PREFERRED_MOVES=['RazorLeaf','Tackle','VineWhip']
resume_reentry(g);checkpoint('tower-resumed')
print('GAME_LOG',g.run_dir/'game.log',flush=True)

try:
 g.cutscene();g.step(60)
 snap('zapdos-visible-after-continue')
 assert g.d.cmd(cmd='get_flags')['data'].get('EVENT_BEAT_ZAPDOS')
 assert g.d.cmd(cmd='get_npcs')['data'][8]['visible']
 assert any(p['species']=='Zapdos' for p in g.d.cmd(cmd='get_party')['data'])
 g.face('up');g.tap('a',20);s=snap('caught-zapdos-cries-after-continue')
 g.cutscene();g.step(40)
 assert g.st()['screen']=='overworld'
 print('CAUGHT_ZAPDOS_VISIBLE_AND_INTERACTIVE_REPRODUCED',flush=True)
finally:
 shutil.copy2(g.run_dir/'game.log',OUT/'game-final.log');g.close()

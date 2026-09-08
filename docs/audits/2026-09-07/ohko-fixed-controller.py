import json,sys,time,traceback,shutil,os
from pathlib import Path
ROOT=Path('/Users/liuyanghe02/develop/open-pokered-3')
OUT=Path('/tmp/pokered-completion-20260907-ohko-fixed')
sys.path[:0]=[str(ROOT/'scripts'),str(ROOT/'docs/audits/2026-09-06/playthrough')]
import playthrough
from playthrough import resume_reentry,NavError
from navigation import AuditGame
playthrough.BIN=OUT/'bin/pokered-app'
shutil.copy2(ROOT/'target/debug/pokered-app',playthrough.BIN)
shutil.copy2(ROOT/'docs/audits/2026-09-07/m140-champion-loss-whiteout.sav',OUT/'active.sav')
shutil.copy2(ROOT/'docs/audits/2026-09-07/m140-champion-loss-whiteout.script_flags.json',OUT/'bin/pokered.script_flags.json')
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
 s=g.st();d={'state':s,'flags':g.d.cmd(cmd='get_flags'),'bag':g.d.cmd(cmd='get_bag'),'party':g.d.cmd(cmd='get_party'),'npcs':g.d.cmd(cmd='get_npcs'),'map':g.d.cmd(cmd='get_map')}
 (OUT/(name+'.json')).write_text(json.dumps(d,indent=2))
 r=g.d.cmd(cmd='capture_frame',path=str(OUT/'shots'/(name+'.png')));assert r['ok'],r
 return s

def checkpoint(name):
 g.cutscene();g.step(60);g.cutscene()
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
resume_reentry(g);checkpoint('league-resumed')
print('GAME_LOG',g.run_dir/'game.log',flush=True)
import signal
def request_cancel(sig,frame):
 global cancel_requested
 cancel_requested=True
signal.signal(signal.SIGUSR1,request_cancel)
print('READY',flush=True)
try:
 while True:
  todo=sorted((OUT/'inbox').glob('*.py'))
  if not todo:time.sleep(.2);continue
  __task_path=todo[0];print('RUN',__task_path.name,flush=True)
  try:
   exec(compile(__task_path.read_text(),str(__task_path),'exec'),globals())
   snap(__task_path.stem+'-done');print('DONE',__task_path.name,flush=True)
  except Exception:
   traceback.print_exc();snap(__task_path.stem+'-failed');print('FAILED',__task_path.name,flush=True)
  __task_path.rename(__task_path.with_suffix('.done'))
finally:
 shutil.copy2(g.run_dir/'game.log',OUT/'game-final.log')
 g.close()

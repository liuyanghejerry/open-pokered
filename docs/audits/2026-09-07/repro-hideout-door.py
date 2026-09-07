import sys,json,shutil,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
OUT=Path(tempfile.mkdtemp(prefix='pokered-hideout-door-repro-'));(OUT/'bin').mkdir();print('Evidence:',OUT,flush=True)
sys.path[:0]=[str(ROOT/'scripts'),str(ROOT/'docs/audits/2026-09-06/playthrough')]
import playthrough
from navigation import AuditGame
playthrough.BIN=OUT/'bin/pokered-app';shutil.copy2(ROOT/'target/debug/pokered-app',playthrough.BIN)
for src,dst in [('rocket-guards-blocked.sav','active.sav'),('rocket-guards-blocked.script_flags.json','bin/pokered.script_flags.json')]:shutil.copy2(ROOT/'docs/audits/2026-09-07'/src,OUT/dst)
g=AuditGame(save_path=OUT/'active.sav')
def snap(name):
 d={'state':g.st(),'flags':g.d.cmd(cmd='get_flags')};(OUT/(name+'.json')).write_text(json.dumps(d,indent=2));assert g.d.cmd(cmd='capture_frame',path=str(OUT/(name+'.png')))['ok']
try:
 playthrough.resume_reentry(g);g.step(40)
 for attempt in range(2):
  flags=g.d.cmd(cmd='get_flags')['data'];assert not flags.get('EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_0') and not flags.get('EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_1')
  g.nav_to(25,12,'RocketHideoutB4F');snap('door-before-'+str(attempt));g.nav_to(25,7,'RocketHideoutB4F');snap('door-passed-'+str(attempt))
  assert g.st()['player_y']==7
 print('REPRODUCED TWICE: guards unbeaten, closed-door condition bypassed by normal walking',flush=True)
finally:g.close()

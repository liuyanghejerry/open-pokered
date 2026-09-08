import sys,json,shutil,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
OUT=Path(tempfile.mkdtemp(prefix='pokered-move-menu-repro-'))
(OUT/'bin').mkdir();print('Evidence:',OUT,flush=True)
sys.path[:0]=[str(ROOT/'scripts'),str(ROOT/'docs/audits/2026-09-06/playthrough')]
import playthrough
from navigation import AuditGame
playthrough.BIN=OUT/'bin/pokered-app'
shutil.copy2(ROOT/'target/debug/pokered-app',playthrough.BIN)
for src,dst in [('m22-vermilion-healed.sav','active.sav'),('m22-vermilion-healed.script_flags.json','bin/pokered.script_flags.json')]:
 shutil.copy2(ROOT/'docs/audits/2026-09-07'/src,OUT/dst)
g=AuditGame(save_path=OUT/'active.sav')
try:
 playthrough.resume_reentry(g);g.step(40);g.tap('start',12)
 g.tap('down',8);g.tap('down',8);g.tap('a',12)
 for _ in range(7):g.tap('down',8)
 g.tap('a',12);g.tap('a',12);g.tap('a',12)
 s=g.st();assert s['screen']=='party',s
 assert g.d.cmd(cmd='capture_frame',path=str(OUT/'move-forget-second.png'))['ok']
 (OUT/'move-forget-second.json').write_text(json.dumps(s,indent=2))
 print('Captured independent normal HM01 selection',flush=True)
finally:g.close()

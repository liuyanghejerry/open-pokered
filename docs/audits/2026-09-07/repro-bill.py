import sys,json,shutil,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
OUT=Path(tempfile.mkdtemp(prefix='pokered-bill-repro-'))
(OUT/'bin').mkdir()
print('Evidence:',OUT)
sys.path[:0]=[str(ROOT/'scripts'),str(ROOT/'docs/audits/2026-09-06/playthrough')]
import playthrough
from navigation import AuditGame
playthrough.BIN=OUT/'bin/pokered-app'
shutil.copy2(ROOT/'target/debug/pokered-app',playthrough.BIN)
shutil.copy2(ROOT/'docs/audits/2026-09-07/m17-bill-door.sav',OUT/'active.sav')
shutil.copy2(ROOT/'docs/audits/2026-09-07/m17-bill-door.script_flags.json',OUT/'bin/pokered.script_flags.json')
g=AuditGame(save_path=OUT/'active.sav')
def snap(name):
 d={'state':g.st(),'flags':g.d.cmd(cmd='get_flags'),'npcs':g.d.cmd(cmd='get_npcs')}
 (OUT/(name+'.json')).write_text(json.dumps(d,indent=2))
 assert g.d.cmd(cmd='capture_frame',path=str(OUT/(name+'.png')))['ok']
try:
 playthrough.resume_reentry(g)
 g.nav_warp(45,3,'Route25','BillsHouse')
 g.nav_to(6,6,'BillsHouse');g.face('up');g.tap('a',20);g.dialogue_then_choice();g.choose('YES');g.cutscene()
 snap('agreed')
 g.nav_to(1,5,'BillsHouse');g.face('up');g.tap('a',20)
 g.d.cmd(cmd='wait_until',condition='dialogue_ready',max_frames=1200);snap('visible-pc')
 g.cutscene()
 assert not g.d.cmd(cmd='get_flags')['data'].get('EVENT_MET_BILL_2')
 g.nav_to(5,6,'BillsHouse');g.nav_to(5,5,'BillsHouse');g.tap('a',20)
 g.d.cmd(cmd='wait_until',condition='dialogue_ready',max_frames=1200);snap('invisible-pc')
 g.cutscene();snap('converted')
 assert g.d.cmd(cmd='get_flags')['data'].get('EVENT_MET_BILL_2')
 print('REPRODUCED: visible PC fails to convert; empty floor succeeds')
finally:g.close()

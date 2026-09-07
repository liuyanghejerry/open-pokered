import sys,json,shutil,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];OUT=Path(tempfile.mkdtemp(prefix='pokered-item-repro-'))
(OUT/'bin').mkdir()
print('Evidence:',OUT)
sys.path.insert(0,str(ROOT/'scripts'))
import playthrough
playthrough.BIN=OUT/'bin/pokered-app'
shutil.copy2(ROOT/'target/debug/pokered-app',playthrough.BIN)
def capture(g,name):
 d=g.st();(OUT/(name+'.json')).write_text(json.dumps(d,indent=2))
 assert g.d.cmd(cmd='capture_frame',path=str(OUT/(name+'.png')))['ok']
 return d
def menu(g):
 for _ in range(200):
  s=g.st()
  if s['battle_phase']=='PlayerMenu':return s
  g.tap('a',10)
 raise AssertionError('menu timeout')
for attempt in range(2):
 shutil.copy2(ROOT/'docs/audits/2026-09-07/supplies-ready.sav',OUT/'active.sav')
 shutil.copy2(ROOT/'docs/audits/2026-09-07/supplies-ready.script_flags.json',OUT/'bin/pokered.script_flags.json')
 g=playthrough.Game(save_path=OUT/'active.sav')
 try:
  playthrough.resume_reentry(g)
  assert g.d.cmd(cmd='start_wild_battle',species='Rattata',level=5)['ok']
  g.PREFERRED_MOVES=['Growl']
  for _ in range(12):
   s=menu(g)
   if s['battle_live']['player']['hp']<s['battle_live']['player']['max_hp']:break
   g.d.drive(['up','left'],frames=10);g.tap('a',4);assert g._await_phase('MoveSelect',120)
   g._select_move();g.step(30)
  s=menu(g);assert s['battle_live']['player']['hp']<s['battle_live']['player']['max_hp']
  before=capture(g,f'{attempt}-before')
  g.d.drive(['down','left'],frames=10);g.tap('a',4);g.step(10)
  s=g.st();assert s['battle_phase']=='BagSelect',s
  assert s['battle_bag']['items'][0]['item']=='Potion',s
  g.tap('a',4);g.step(10);assert g.st()['battle_phase'].startswith('ItemTargetSelect')
  g.tap('a',10);menu(g)
  after=capture(g,f'{attempt}-after')
  items={i['item']:i['qty'] for i in after['battle_inventory']}
  assert 'HelixFossil' not in items and items['Potion']==12,items
  print('REPRODUCED',attempt,'Potion used; HelixFossil removed; Potion quantity unchanged',flush=True)
 finally:g.close()

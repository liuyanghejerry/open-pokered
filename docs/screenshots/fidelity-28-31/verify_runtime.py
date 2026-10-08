"""Seed legal saves, then verify actual button-driven interactions."""
import argparse,json,sys,tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[3];sys.path.insert(0,str(root/'scripts'))
import playthrough as pt,playthrough_late as late,save_builder as sb,debug_drive,fidelity_stdio
parser=argparse.ArgumentParser();parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
fidelity_stdio.install(pt,debug_drive,args.binary.resolve())
temp=tempfile.TemporaryDirectory();fixtures=Path(temp.name)
g=pt.Game()
try:
 pt.m01_boot(g);pt.m02_oak_speech(g);g.d.cmd(cmd='save');h=pt.Game(save_path=g.save_path)
 try:sb.SaveBuilder._tpl=h.d.cmd(cmd='export_fixture')['data']
 finally:h.close()
finally:g.close()
def snapshot(name,map,x,y,items=(),flags=()):
 assert pt.walkable(map,x,y),(map,x,y)
 b=sb.SaveBuilder().party_add('Charizard',50).position(map,x,y)
 for i in items:b.give_item(i,1)
 for f in flags:b.flag(f)
 return b.write(fixtures/f'{name}.json')
results={}
for route in ['12','16']:
 g=pt.Game(snapshot=snapshot('snorlax'+route,'Route'+route,9 if route=='12' else 25,62 if route=='12' else 10,['POKE_FLUTE']))
 try:
  pt.resume_reentry(g);g.step(10);g.face('right');g.tap('a',1);g.step(4)
  assert 'sleeping' in g.st()['script_effect']['text'];g.skip();g.step(4)
  assert g.st()['screen']=='overworld' and not g.st()['script_running']
  late.use_item(g,'PokeFlute')
  for _ in range(500):
   s=g.st()
   if s['screen']=='battle':break
   if s['dialogue']:g.skip()
   g.step(2)
  assert g.st()['screen']=='battle',g.st()
  assert g.st()['battle_live']['enemy']['species']=='Snorlax'
  results[f'route{route}']='Talk sleeps; BAG USE starts Snorlax battle'
 finally:g.close()
for x,y,button,face,npcface in [(6,14,'up','Up','Down'),(8,12,'left','Left','Right')]:
 path=snapshot('giovanni','SilphCo11F',x,y,flags=['EVENT_SILPH_CO_11_UNLOCKED_DOOR','EVENT_BEAT_SILPH_CO_11F_TRAINER_0','EVENT_BEAT_SILPH_CO_11F_TRAINER_1'])
 g=pt.Game(snapshot=path)
 try:
  pt.resume_reentry(g);g.step(10);g.tap(button,20)
  while g.st()['dialogue']:g.skip();g.step(2)
  seen=[]
  for _ in range(500):
   s=g.st()
   if s['screen']=='battle':break
   ns=g.d.cmd(cmd='get_npcs')['data'];n=ns[2];seen.append((n['x'],n['y']))
   if s['script_effect'] and s['script_effect']['effect']=='Delay':
    assert n['y']==12 and n['facing']==npcface,(n,s)
    assert s['player_facing']==face,s
   g.step(1)
  assert g.st()['screen']=='battle';assert (6,12) in seen,seen
  results[f'giovanni-{x}-{y}']=seen
 finally:g.close()
for drink,tm in [('FRESH_WATER','Tm13'),('SODA_POP','Tm48'),('LEMONADE','Tm49')]:
 path=snapshot('drink','CeladonMartRoof',5,6,[drink])
 g=pt.Game(snapshot=path)
 try:
  pt.resume_reentry(g);g.step(10)
  for _ in range(12):
   n=g.d.cmd(cmd='get_npcs')['data'][1];g.approach_object(n['x'],n['y'],'CeladonMartRoof');g.tap('a',1);g.step(2)
   if g.st()['dialogue']:break
  assert g.st()['dialogue'],g.st()
  seen=[]
  for _ in range(500):
   s=g.st();e=s['script_effect'];seen.append(e)
   if s['screen']=='filter-bag' or s['field_menu']:g.tap('a',8)
   elif s['choice']:
    opts=s['choice']['options'];g.choose('YES' if 'YES' in opts else opts[0])
   elif s['dialogue']:g.skip()
   else:g.step(2)
   if tm in str(g.d.cmd(cmd='get_bag')['data']) and not g.st()['script_running'] and not g.st()['dialogue'] and not g.st()['field_menu']:break
  bag=g.d.cmd(cmd='get_bag')['data'];assert tm in str(bag),bag
  assert any(e and e['effect']=='ShowItemDialogue' for e in seen),seen
  results[drink]=bag
 finally:g.close()
args.output.write_text(json.dumps(results,indent=2));print('All runtime scenarios passed')

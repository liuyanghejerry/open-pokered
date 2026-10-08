"""Replay identical input/frame sequences on master and the repair branch."""
import argparse,json,sys,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];sys.path.insert(0,str(ROOT/'scripts'))
import playthrough as pt,save_builder as sb,debug_drive,fidelity_stdio
p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,required=True);p.add_argument('--label',required=True);a=p.parse_args()
fidelity_stdio.install(pt,debug_drive,a.binary.resolve())
g=pt.Game()
try:
 pt.m01_boot(g);pt.m02_oak_speech(g);g.d.cmd(cmd='save');h=pt.Game(save_path=g.save_path)
 try:sb.SaveBuilder._tpl=h.d.cmd(cmd='export_fixture')['data']
 finally:h.close()
finally:g.close()
states={}
with tempfile.TemporaryDirectory() as temp:
 for name,map,x,y,direction,items,flags in [
  ('snorlax','Route12',9,62,'right',['POKE_FLUTE'],[]),
  ('warden','WardensHouse',2,4,'up',['GOLD_TEETH'],[]),
  ('giovanni','SilphCo11F',6,14,'up',[],['EVENT_SILPH_CO_11_UNLOCKED_DOOR','EVENT_BEAT_SILPH_CO_11F_TRAINER_0','EVENT_BEAT_SILPH_CO_11F_TRAINER_1'])]:
  b=sb.SaveBuilder().party_add('Charizard',50).position(map,x,y)
  for i in items:b.give_item(i,1)
  for f in flags:b.flag(f)
  assert pt.walkable(map,x,y)
  g=pt.Game(snapshot=b.write(Path(temp)/f'{name}.json'))
  try:
   pt.resume_reentry(g);g.step(10)
   if name=='giovanni':g.tap('up',20);g.step(10)
   else:g.face(direction);g.tap('a',1)
   if name in ['warden','giovanni']:g.skip();g.step(2)
   g.step(1300-g.st()['frame_count'] if name=='warden' else (24 if name=='giovanni' else 200))
   dest=Path(__file__).parent/f'{name}-{a.label}.png';g.d.cmd(cmd='capture_frame',path=str(dest.resolve()))
   states[name]=g.st();states[name]['npcs']=g.d.cmd(cmd='get_npcs')['data']
   print(name,states[name]['frame_count'],states[name]['screen'],states[name]['script_effect'])
  finally:g.close()
(Path(__file__).parent/f'states-{a.label}.json').write_text(json.dumps(states,indent=2))

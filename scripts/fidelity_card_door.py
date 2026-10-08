#!/usr/bin/env python3
"""Reproduce an opened card-door retry and verify closed/open door routing."""
import sys,json,argparse
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
import fidelity_stdio,playthrough as pt,debug_drive,fidelity_pc_audio as pc,playthrough_late as late,save_builder as sb
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('driver',type=Path);parser.add_argument('output',type=Path)
args=parser.parse_args();args.driver=args.driver.resolve();args.output=args.output.resolve()
args.output.mkdir(parents=True,exist_ok=True);fidelity_stdio.install(pt,debug_drive,args.driver)
fresh,_=pc.prepare(args);sb.SaveBuilder._tpl=fresh;records={}
for opened in [False,True]:
 fixture=sb.SaveBuilder().position('SilphCo3F',18,9).give_item('CardKey',1).flag('EVENT_SILPH_CO_3_UNLOCKED_DOOR2',opened).data
 g=pc.boot(args,fixture)
 try:
  g.face('left');assert g.pos()==('SilphCo3F',18,9)
  if opened:
   try:late.talk_object(g,'SilphCo3F',17,9)
   except pt.NavError as exc:records['before-open']={'error':str(exc),'state':g.st()}
   else:raise AssertionError('expected the pre-fix open-door approach to fail')
 finally:g.close()
 g=pc.boot(args,fixture)
 try:
  g.face('left');initial=g.st();late.unlock_card_door(g,'SilphCo3F',17,9,'EVENT_SILPH_CO_3_UNLOCKED_DOOR2')
  assert g.d.cmd(cmd='get_flags')['data']['EVENT_SILPH_CO_3_UNLOCKED_DOOR2']
  if opened:assert g.st()['frame_count']==initial['frame_count']
  g.nav_to(16,9,'SilphCo3F');assert g.pos()==('SilphCo3F',16,9)
  records['after-open' if opened else 'after-closed']=g.st()
 finally:g.close()
(args.output/'results.json').write_text(json.dumps(records,indent=2));print('Closed door opens; opened door retry skips interaction and remains traversable')

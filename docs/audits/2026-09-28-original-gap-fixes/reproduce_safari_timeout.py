import sys,json,shutil,argparse,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'scripts'))
from playthrough import Game,m01_boot,m02_oak_speech
from playthrough_late import m29_strength
parser=argparse.ArgumentParser(description='Seed a Safari timeout, then exit and obtain HM04 using normal inputs.')
parser.add_argument('--output',type=Path)
parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/pokered-app')
args=parser.parse_args()
out=args.output or Path(tempfile.mkdtemp(prefix='safari-timeout-'))
out.mkdir(parents=True,exist_ok=True)
for repeat in range(2):
 g=Game(binary=args.binary,speed=0,seed=42)
 def drive(buttons,frames=None):
  sequence=list(buttons)
  sequence += [None]*max(0,(frames if frames is not None else len(sequence))-len(sequence))
  return g.d.cmd(cmd='press_timeline',buttons=sequence,advance=True)
 g.d.drive=drive
 try:
  m01_boot(g);m02_oak_speech(g)
  assert g.d.cmd(cmd='give_pokemon',species='Bulbasaur',level=40)['ok']
  assert g.d.cmd(cmd='give_item',item='GOLD_TEETH',qty=1)['ok']
  assert g.d.cmd(cmd='warp',map='FuchsiaCity',x=18,y=4)['ok']
  g.step(60)
  g.nav_warp(18,3,'FuchsiaCity','SafariZoneGate')
  g.nav_to(3,3,'SafariZoneGate')
  g.d.drive(['up']*8,frames=16)
  g.dialogue_then_choice();g.step(2);g.choose('YES');assert g.cutscene()
  assert g.pos()[0]=='SafariZoneCenter',g.pos()
  assert g.d.cmd(cmd='warp',map='SafariZoneSecretHouse',x=2,y=3)['ok']
  g.step(60)
  before=g.st()['money']
  m29_strength(g)
  assert g.st()['money']==before,(before,g.st()['money'])
  (out/f'after-{repeat}.json').write_text(json.dumps({'state':g.st(),'flags':g.d.cmd(cmd='get_flags'),'bag':g.d.cmd(cmd='get_bag')},indent=2))
  print('PASS timeout exits without repurchase, Gold Teeth exchanged for HM04; money unchanged',flush=True)
 except Exception:
  (out/f'failed-{repeat}.json').write_text(json.dumps({'state':g.st(),'flags':g.d.cmd(cmd='get_flags')},indent=2));raise
 finally:
  g.log.flush();shutil.copy(g.run_dir/'game.log',out/f'game-{repeat}.log');g.close()

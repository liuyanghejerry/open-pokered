"""One offline pre-help fixture; normal CONTINUE and actual input through Bill's full subplot."""
from pathlib import Path
import sys,os,json,gzip,hashlib,tempfile,shutil,subprocess,traceback
from PIL import Image
root=Path.cwd();sys.path.insert(0,str(root/'scripts'));import playthrough as p
binary=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();out.mkdir(exist_ok=True)
for k in list(os.environ):
 if k.startswith('POKERED_'):del os.environ[k]
raw=gzip.decompress((root/'docs/validation/bill-help-offer/fixture.json.gz').read_bytes());prefix=json.loads(gzip.decompress((root/'docs/validation/bill-help-offer/prefix.json.gz').read_bytes()))
flags=['EVENT_BILL_SAID_USE_CELL_SEPARATOR','EVENT_USED_CELL_SEPARATOR_ON_BILL','EVENT_MET_BILL','EVENT_MET_BILL_2','EVENT_GOT_SS_TICKET','EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING']
r={'source':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256(raw).hexdigest(),'scope':'One offline pre-help fixture, normal boot/Continue, then actual buttons and read-only RPCs. No runtime seeding/warps/skip. Not fresh mainline or original-ROM runtime.','commands':[],'observations':[],'complete':False}
try:
 with tempfile.TemporaryDirectory(prefix='bill-full-subplot-')as tmp:
  path=Path(tmp)/'fixture.json';path.write_bytes(raw);g=p.Game(binary=binary,snapshot=path,seed=42,speed=0);original=g.d.cmd
  def cmd(*a,**kw):
   assert kw['cmd']in{'get_state','get_flags','get_npcs','get_bag','export_fixture','press_timeline','press_sequence','step_frames','wait_until','capture_frame'}
   r['commands'].append(kw);return original(*a,**kw)
  g.d.cmd=cmd
  def step(key=None,count=1):g.d.cmd(cmd='press_timeline',buttons=[key]*count,advance=True)
  def flag():
   f=g.d.cmd(cmd='get_flags')['data'];return {k:bool(f.get(k,False))for k in flags}
  def observe(label,capture=False):
   s=g.st();r['observations'].append({'label':label,'state':s,'flags':flag(),'npcs':g.d.cmd(cmd='get_npcs')['data'],'bag':g.d.cmd(cmd='get_bag')['data']})
   if capture:g.d.cmd(cmd='capture_frame',path=str(out/(label+'.png')))
   return s
  def read_until(target):
   for _ in range(1000):
    s=g.st()
    if target(s):return s
    d=s['dialogue_state'];kind=s['active_script_effect']
    if kind=='AskFieldYesNo'and d and d['page']==d['total_pages']:step(count=30)
    elif (kind in ['ShowDialogue','PrintFieldText','AskFieldYesNo'] or kind=='BillPokemonList' and s['script_effect']['state']['phase']=='Favorite') and d and d['waiting_for_input']:
     step(count=10);step('a');step(count=4)
    else:step(count=30)
   raise AssertionError(('dialogue or movie stalled',g.st()))
  def finish():
   read_until(lambda s:not s['script_running']and s['active_script_effect']is None and s['dialogue_state']is None and s['choice']is None)
   g.wait('control_ready',600)
  def tile(key):
   old=g.pos();step(key,16);step(count=16);new=g.pos()
   if old[0]==new[0]:assert abs(old[1]-new[1])+abs(old[2]-new[2])==1,(key,old,new)
  def go(x,y):
   for _ in range(32):
    m,px,py=g.pos();assert m=='BillsHouse'
    if(px,py)==(x,y):return
    tile('left'if px>x else'right'if px<x else'up'if py>y else'down')
   raise AssertionError('room walk did not finish')
  def interact():
   step('up',2);step(count=4);step('a',3)
  try:
   for request in prefix['commands']:
    if request['cmd']=='capture_frame':break
    g.d.cmd(**request)
   assert g.pos()==('BillsHouse',6,6)and not any(flag().values());observe('pre-help')
   s=read_until(lambda s:s['choice']is not None);assert s['choice']['options']==['YES','NO'];step(count=4);step('a');step(count=15);assert g.st()['choice']is None
   finish();observe('monster-in-machine');assert flag()[flags[0]]and not flag()[flags[1]]
   monster=next(n for n in g.d.cmd(cmd='get_npcs')['data']if n['text_id']==1);assert not monster['visible']and(monster['x'],monster['y'])==(6,2)
   go(1,5);interact();finish();observe('separated');assert all(flag()[k]for k in flags[:4])
   human=next(n for n in g.d.cmd(cmd='get_npcs')['data']if n['text_id']==2);assert human['visible']and(human['x'],human['y'])==(4,4)
   go(4,5);interact();finish();observe('ticket-received');assert flag()[flags[4]]and 'TICKET'in json.dumps(g.d.cmd(cmd='get_bag')['data']).upper()
   go(2,6);tile('down');step(count=240);g.wait('control_ready',600);assert g.pos()[0]=='Route25';observe('left-house');assert flag()[flags[5]]
   # Arrival occupies the door's warp tile. Step off it before re-entering.
   if g.pos()==('Route25',45,3):tile('down')
   for _ in range(4):
    if g.pos()[0]=='BillsHouse':break
    assert g.pos()[0]=='Route25'and g.pos()[1]==45,g.pos();tile('up');step(count=240);g.wait('control_ready',600)
   assert g.pos()[0]=='BillsHouse'
   if g.pos()[2]==7:tile('up')
   observe('returned-house',True)
   npcs=g.d.cmd(cmd='get_npcs')['data'];assert next(n for n in npcs if n['text_id']==3)['visible'];assert not next(n for n in npcs if n['text_id']==2)['visible']
   go(1,5);interact();s=read_until(lambda s:s['choice']is not None and s['script_effect']['state']['phase']=='Poll');assert s['choice']['options']==['EEVEE','FLAREON','JOLTEON','VAPOREON','CANCEL']
   step(count=4);step('a');step(count=300);s=observe('unowned-eevee',True);assert s['script_effect']['effect']=='BillPokemonList' and s['script_effect']['state']['phase']=='Data' and s['script_effect']['state']['selected']==0
   dex=g.d.cmd(cmd='export_fixture')['data']['game_data']['pokedex'];assert not dex['owned'][132//8]&(1<<(132%8))
   with Image.open(out/'unowned-eevee.png')as im:ink=sum(p!=(255,255,255)for p in im.convert('RGB').crop((8,96,145,132)).getdata())
   assert ink==0 and all(flag().values());step(count=4);step('b');step(count=20);assert g.st()['choice']is not None and g.st()['script_effect']['state']['phase']=='Poll' and 'Which POKeMON' in g.st()['dialogue']
   step('b');step(count=30);g.wait('control_ready',600);step('right',32);assert g.pos()[1]>1 and g.pos()[2]==5;assert path.read_bytes()==raw;r['complete']=True
  finally:g.log.flush();shutil.copyfile(g.run_dir/'game.log',out/'game.log');g.close()
except BaseException:r['failure']=traceback.format_exc();print(r['failure'],flush=True)
finally:(out/'verification.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'complete':r['complete'],'observations':[x['label']for x in r['observations']]}),flush=True)
raise SystemExit(0 if r['complete']else 1)

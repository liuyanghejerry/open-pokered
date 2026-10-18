from pathlib import Path
import sys,os,json,tempfile,shutil,traceback,hashlib,subprocess
import argparse
parser=argparse.ArgumentParser(description="Replay forced-bike SRAM loading, Bicycle use and ordinary UI SAVE/Continue")
parser.add_argument('--binary',type=Path,required=True)
parser.add_argument('--output-dir',type=Path,required=True)
parser.add_argument('--expected-transport',choices=['Walking','Biking'],required=True)
parser.add_argument('--fixture',type=Path,default=Path(__file__).with_name('fixture.sav'))
parser.add_argument('--repo-root',type=Path,default=Path(__file__).resolve().parents[3])
a=parser.parse_args()
R=a.repo_root.resolve();sys.path.insert(0,str(R/'scripts'));import playthrough as p
os.chdir(R)
binary=a.binary.resolve();out=a.output_dir.resolve();expected=a.expected_transport;out.mkdir();(out/'tmp').mkdir();tempfile.tempdir=str(out/'tmp')
for k in list(os.environ):
 if k.startswith('POKERED_'):del os.environ[k]
src=a.fixture.resolve();save=out/'play.sav';shutil.copy2(src,save);x={'all_pass':False,'expected_transport':expected,'repo_root':str(R),'checkout_head':subprocess.check_output(['git','-C',str(R),'rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256(src.read_bytes()).hexdigest(),'commands':{},'scope':'ordinary Continue and actual bag Bicycle USE, UI SAVE and independent Continue on fixed case; constructed SRAM, no runtime setters/debug save/skipintro'};g=None
try:
 g=p.Game(binary=binary,save_path=save,seed=42,speed=0);x['argv']=g.proc.args;orig=g.d.cmd
 def cmd(*a,**kw):
  n=kw.get('cmd',a[0]if a else None);assert n in {'get_state','get_npcs','get_flags','get_party','get_bag','press_sequence','press_timeline','step_frames','wait_until','skip_dialogue','capture_frame'},n;x['commands'][n]=x['commands'].get(n,0)+1;return orig(*a,**kw)
 g.d.cmd=cmd;p.resume_reentry(g);assert g.cutscene();g.step(64);x['initial']=g.st();assert(x['initial']['map_name'],x['initial']['player_x'],x['initial']['player_y'],x['initial']['player_transport'])==('Route18',30,8,'Biking'),x['initial']
 g.tap('start',32);s=g.st();assert s['screen']=='start-menu';target=s['field_menu']['items'].index('Item')
 for _ in range(10):
  if g.st()['field_menu']['cursor']==target:break
  g.tap('down',16)
 assert g.st()['field_menu']['cursor']==target;g.tap('a',32);s=g.st();x['bag']=s;assert s['field_menu']['kind']=='bag' and s['field_menu']['items'][0]['item']=='Bicycle',s
 g.tap('a',32);x['after_use']=g.st()
 g.step(48);s=g.st();x['capture']=s;assert s['player_transport']==expected,s
 assert g.d.cmd(cmd='capture_frame',path=str(out/'bicycle-use.png'))['ok']
 if expected=='Biking':assert s['screen']=='bag' and s['field_menu']['phase']=='FieldMessage { item: Bicycle, consumed: false }',s
 for _ in range(3):g.tap('b',32)
 x['field_capture']=g.st();assert x['field_capture']['screen']=='overworld' and x['field_capture']['player_transport']==expected
 assert g.d.cmd(cmd='capture_frame',path=str(out/'field.png'))['ok']
 if expected=='Biking':
  g.tap('start',32);s=g.st();assert s['screen']=='start-menu';target=s['field_menu']['items'].index('Save')
  for _ in range(12):
   if g.st()['field_menu']['cursor']==target:break
   g.tap('down',16)
  assert g.st()['field_menu']['cursor']==target;g.tap('a',32)
  for _ in range(40):
   g.tap('a',32);s=g.st()
   if save.read_bytes()!=src.read_bytes()and s['screen']in ['overworld','start-menu']:break
  x['save_end']=s;saved=save.read_bytes();assert saved!=src.read_bytes()and len(saved)==32768
  assert saved[0x29dc]&0x20==0 and saved[0x29de]&0x20 and saved[0x29ac]==1
  assert saved[0x3523]==(~sum(saved[0x2598:0x3523]))&255
  x['saved_sha256']=hashlib.sha256(saved).hexdigest();x['saved_status_flags']={'Flags5':saved[0x29dc],'Flags6':saved[0x29de]};g.close();x['first_cleanup_exit']=g.proc.returncode
  g=p.Game(binary=binary,save_path=save,seed=42,speed=0);orig=g.d.cmd;g.d.cmd=cmd;p.resume_reentry(g);assert g.cutscene();g.step(64);x['independent_continue']=g.st();assert x['independent_continue']['player_transport']=='Biking' and x['independent_continue']['map_name']=='Route18'
  assert hashlib.sha256(save.read_bytes()).hexdigest()==x['saved_sha256']
  g.tap('start',32);target=g.st()['field_menu']['items'].index('Item')
  for _ in range(10):
   if g.st()['field_menu']['cursor']==target:break
   g.tap('down',16)
  g.tap('a',32);g.tap('a',32);x['independent_bicycle_use']=g.st();assert x['independent_bicycle_use']['player_transport']=='Biking' and x['independent_bicycle_use']['field_menu']['phase'].startswith('FieldMessage')
 x['all_pass']=True
except BaseException:x['failure']=traceback.format_exc();print(x['failure'],flush=True)
finally:
 if g:g.close();x['owner_cleanup_exit']=g.proc.returncode
 (out/'result.json').write_text(json.dumps(x,indent=2)+'\n')
print(json.dumps({'all_pass':x['all_pass'],'result':str(out/'result.json')}),flush=True);raise SystemExit(0 if x['all_pass']else 1)

"""Normal native SRAM/CONTINUE, physical walking and SAVE menu, independent-process CONTINUE.
Readonly observations only; no debug SAVE, setters, warp, snapshot load/restore or dialogue skip.
"""
from pathlib import Path
import os,sys,json,gzip,tempfile,subprocess,hashlib,traceback,shutil,argparse
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--repo',type=Path,required=True)
parser.add_argument('--binary',type=Path,required=True)
parser.add_argument('--out',type=Path,required=True)
parser.add_argument('--fixture',type=Path,default=Path(__file__).with_name('fixture.sav'))
parser.add_argument('--boot-commands',type=Path,default=Path(__file__).parents[1]/'offscreen-npc-availability'/'boot-commands.json')
parser.add_argument('--expect-retained',action='store_true')
args=parser.parse_args()
R=args.repo.resolve();B=args.binary.resolve();D=args.out.resolve();D.mkdir();(D/'tmp').mkdir();tempfile.tempdir=str(D/'tmp')
for key in list(os.environ):
 if key.startswith('POKERED_'):del os.environ[key]
sys.path.insert(0,str(R/'scripts'));import playthrough as p
assert not subprocess.check_output(['git','status','--porcelain'],cwd=R,text=True).strip();head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip();names=subprocess.check_output(['git','ls-files','crates','scripts','Cargo.toml','Cargo.lock'],cwd=R,text=True).splitlines()
def manifest():return {n:hashlib.sha256((R/n).read_bytes()).hexdigest()for n in names if(R/n).is_file()}
frozen=manifest();bh=hashlib.sha256(B.read_bytes()).hexdigest();raw=args.fixture.read_bytes();save=D/'play.sav';save.write_bytes(raw);j={'scope':__doc__,'head':head,'binary_sha256':bh,'fixture_sha256':hashlib.sha256(raw).hexdigest(),'commands':[],'processes':[],'all_pass':False};g=None
prefix=json.loads(args.boot_commands.read_text())
def boot():
 global g
 g=p.Game(binary=B,save_path=save,seed=42,speed=0);j['processes'].append(g.proc.pid);orig=g.d.cmd
 def cmd(**kw):
  assert kw['cmd']in {'get_state','get_npcs','press_sequence','step_frames','press_timeline','capture_frame'};j['commands'].append({'pid':g.proc.pid,**kw});return orig(**kw)
 g.d.cmd=cmd
 for c in prefix:
  g.d.cmd(**c);s=g.st()
  if s['screen']=='overworld':return s
 raise AssertionError('CONTINUE did not reach field')
def tape(keys):g.d.cmd(cmd='press_timeline',buttons=keys,advance=True)
def capture(label):
 s=g.st();n=g.d.cmd(cmd='get_npcs')['data'];j[label]={'state':s,'npcs':n};g.d.cmd(cmd='capture_frame',path=str(D/f'{label}.png'));return s,n
try:
 s=boot();assert(s['map_name'],s['player_x'],s['player_y'])==('Route1',5,19)
 tape([None]*192+['down']*16+[None]*656+['down']*16+[None]*144)
 s,n=capture('before-menu');assert(s['player_x'],s['player_y'])==(5,21);assert(n[0]['x'],n[0]['y'],n[0]['walk_counter'])==(5,25,0),n
 tape(['start']*8+[None]*64);s,n=capture('start-menu');print('menu',s.get('field_menu'),flush=True);assert s['screen']=='start-menu',s['screen']
 for _ in range(4):tape(['down']*8+[None]*16)
 tape(['a']*8+[None]*64);s,n=capture('save-menu');print('save',s['screen'],s.get('field_menu'),flush=True);assert s['screen']in {'save','save-menu'},s['screen'];j['saved_npc_candidate']=n
 for _ in range(40):
  tape(['a']*8+[None]*32)
  if save.read_bytes()!=raw:break
 else:raise AssertionError('physical SAVE never wrote SRAM')
 j['saved_file_sha256']=hashlib.sha256(save.read_bytes()).hexdigest();(D/'ui-saved.sav').write_bytes(save.read_bytes());tape([None]*160)
 for _ in range(8):
  if g.st()['screen']=='overworld':break
  tape(['b']*8+[None]*32)
 capture('after-save');g.close();j['first_game_exit']=g.proc.returncode;g=None
 s=boot();s,n=capture('continued');assert(s['map_name'],s['player_x'],s['player_y'])==('Route1',5,21);assert len(set(j['processes']))==2
 j['observed_npc_reset']=(n[0]['x'],n[0]['y'])!=(j['saved_npc_candidate'][0]['x'],j['saved_npc_candidate'][0]['y']);assert j['observed_npc_reset'] != args.expect_retained,(j['saved_npc_candidate'],n)
 j['all_pass']=True
except BaseException:j['failure']=traceback.format_exc();print(j['failure'],flush=True)
finally:
 if g:g.close();j['last_game_exit']=g.proc.returncode
 j['source_unchanged']=manifest()==frozen;j['binary_unchanged']=hashlib.sha256(B.read_bytes()).hexdigest()==bh;j['head_unchanged']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==head;(D/'result.json.gz').write_bytes(gzip.compress(json.dumps(j,indent=2).encode(),mtime=0))
print(json.dumps({'all_pass':j['all_pass'],'observed_npc_reset':j.get('observed_npc_reset')}),flush=True)
raise SystemExit(0 if j['all_pass']and all(j[k]for k in ['source_unchanged','binary_unchanged','head_unchanged'])else 1)

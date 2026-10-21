"""Normal native boot/CONTINUE from offline counter fixtures; physical idle tape/readonly observations.
Fixtures differ only by the Y displacement byte and checksum. This is not a fresh playthrough.
"""
from pathlib import Path
import os,sys,json,gzip,tempfile,subprocess,hashlib,traceback,shutil,argparse
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--repo',type=Path,required=True)
parser.add_argument('--binary',type=Path,required=True)
parser.add_argument('--out',type=Path,required=True)
parser.add_argument('--zero-fixture',type=Path,default=Path(__file__).with_name('counter-zero.sav'))
parser.add_argument('--control-fixture',type=Path,default=Path(__file__).with_name('counter-eight.sav'))
parser.add_argument('--boot-commands',type=Path,default=Path(__file__).parents[1]/'offscreen-npc-availability'/'boot-commands.json')
args=parser.parse_args()
R=args.repo.resolve();B=args.binary.resolve();D=args.out.resolve();D.mkdir();(D/'tmp').mkdir();tempfile.tempdir=str(D/'tmp')
for name in list(os.environ):
 if name.startswith('POKERED_'):del os.environ[name]
sys.path.insert(0,str(R/'scripts'));import playthrough as p
assert not subprocess.check_output(['git','status','--porcelain'],cwd=R,text=True).strip();head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip();names=subprocess.check_output(['git','ls-files','crates','scripts','Cargo.toml','Cargo.lock'],cwd=R,text=True).splitlines()
def manifest():return {n:hashlib.sha256((R/n).read_bytes()).hexdigest()for n in names if(R/n).is_file()}
frozen=manifest();bh=hashlib.sha256(B.read_bytes()).hexdigest()
j={'scope':__doc__,'head':head,'binary_sha256':bh,'cases':[],'all_pass':False};g=None
try:
 for displacement in [0,8]:
  raw=(args.zero_fixture if displacement==0 else args.control_fixture).read_bytes()
  out=D/f'y{displacement}';out.mkdir();save=out/'fixture.sav';save.write_bytes(raw);case={'y_displacement':displacement,'fixture_sha256':hashlib.sha256(raw).hexdigest(),'commands':[],'observations':[]};j['cases'].append(case)
  g=p.Game(binary=B,save_path=save,seed=42,speed=0);original=g.d.cmd
  def cmd(**kw):
   assert kw['cmd']in {'get_state','get_npcs','press_sequence','step_frames','press_timeline','capture_frame'};case['commands'].append(kw);return original(**kw)
  g.d.cmd=cmd
  for request in json.loads(args.boot_commands.read_text()):
   g.d.cmd(**request);s=g.st()
   if s['screen']=='overworld':break
  assert(s['map_name'],s['player_x'],s['player_y'])==('Route1',5,21),s
  for tick in range(257):
   if tick:g.d.cmd(cmd='press_timeline',buttons=[None]*4,advance=True)
   s=g.st();ns=g.d.cmd(cmd='get_npcs')['data'];case['observations'].append({'frame':s['frame_count'],'player':[s['map_name'],s['player_x'],s['player_y']],'npcs':ns})
   assert(s['screen'],s['map_name'],s['player_x'],s['player_y'])==('overworld','Route1',5,21)
   if tick in [0,64,128,256]:g.d.cmd(cmd='capture_frame',path=str(out/f'frame-{s["frame_count"]}.png'))
  case['moved']=any((o['npcs'][0]['x'],o['npcs'][0]['y'])!=(5,25)or o['npcs'][0]['walk_counter'] for o in case['observations'])
  assert case['moved']==(displacement==8),case['moved'];assert save.read_bytes()==raw;g.close();case['game_exit']=g.proc.returncode;g=None
 assert [o['player']for o in j['cases'][0]['observations']]==[o['player']for o in j['cases'][1]['observations']]
 j['all_pass']=True
except BaseException:j['failure']=traceback.format_exc();print(j['failure'],flush=True)
finally:
 if g:g.close()
 j['source_unchanged']=manifest()==frozen;j['binary_unchanged']=hashlib.sha256(B.read_bytes()).hexdigest()==bh;j['head_unchanged']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==head;(D/'result.json.gz').write_bytes(gzip.compress(json.dumps(j,indent=2).encode(),mtime=0))
print(json.dumps({'all_pass':j['all_pass'],'cases':[(c['y_displacement'],c.get('moved'))for c in j['cases']]}),flush=True)
raise SystemExit(0 if j['all_pass']and all(j[k]for k in ['source_unchanged','binary_unchanged','head_unchanged'])else 1)

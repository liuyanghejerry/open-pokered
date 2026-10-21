"""Normal native boot/CONTINUE with an offline SRAM fixture, physical idle tape and readonly observations.
Not a fresh playthrough or proof of original DIV/seed/raster parity.
"""
from pathlib import Path
import os,sys,json,gzip,tempfile,subprocess,hashlib,traceback,argparse
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--repo',type=Path,required=True)
parser.add_argument('--binary',type=Path,required=True)
parser.add_argument('--out',type=Path,required=True)
parser.add_argument('--fixture',type=Path,required=True)
args=parser.parse_args();R=args.repo.resolve();B=args.binary.resolve();D=args.out.resolve();D.mkdir();(D/'tmp').mkdir();tempfile.tempdir=str(D/'tmp')
for key in list(os.environ):
 if key.startswith('POKERED_'):del os.environ[key]
sys.path.insert(0,str(R/'scripts'));import playthrough as p
assert not subprocess.check_output(['git','status','--porcelain'],cwd=R,text=True).strip()
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip();names=subprocess.check_output(['git','ls-files','crates','scripts','Cargo.toml','Cargo.lock'],cwd=R,text=True).splitlines()
def manifest():return {n:hashlib.sha256((R/n).read_bytes()).hexdigest()for n in names if(R/n).is_file()}
frozen=manifest();bh=hashlib.sha256(B.read_bytes()).hexdigest();raw=args.fixture.read_bytes();save=D/'fixture.sav';save.write_bytes(raw)
j={'scope':__doc__,'head':head,'binary_sha256':bh,'fixture_sha256':hashlib.sha256(raw).hexdigest(),'commands':[],'observations':[],'all_pass':False};g=None
try:
 g=p.Game(binary=B,save_path=save,seed=42,speed=0);j['pid']=g.proc.pid;original=g.d.cmd
 def cmd(**kw):
  assert kw['cmd']in {'get_state','get_npcs','press_sequence','step_frames','press_timeline','capture_frame'}
  j['commands'].append(kw);return original(**kw)
 g.d.cmd=cmd
 for request in json.loads((R/'docs/audits/offscreen-npc-availability/boot-commands.json').read_text()):
  g.d.cmd(**request);s=g.st()
  if s['screen']=='overworld':break
 assert(s['map_name'],s['player_x'],s['player_y'])==('Route1',5,21),s
 for tick in range(513):
  if tick:g.d.cmd(cmd='press_timeline',buttons=[None]*2,advance=True)
  s=g.st();ns=g.d.cmd(cmd='get_npcs')['data'];j['observations'].append({'offset':tick*2,'frame':s['frame_count'],'player':[s['map_name'],s['player_x'],s['player_y']],'npcs':ns})
  assert(s['screen'],s['map_name'],s['player_x'],s['player_y'])==('overworld','Route1',5,21)
  if tick*2 in [0,64,128,256,512,768,1024]:g.d.cmd(cmd='capture_frame',path=str(D/f'offset-{tick*2}.png'))
 assert any(o['npcs'][0]['walk_counter']for o in j['observations'])
 assert save.read_bytes()==raw;j['all_pass']=True
except BaseException:j['failure']=traceback.format_exc();print(j['failure'],flush=True)
finally:
 if g:g.close();j['game_exit']=g.proc.returncode
 j['source_unchanged']=manifest()==frozen;j['binary_unchanged']=hashlib.sha256(B.read_bytes()).hexdigest()==bh;j['head_unchanged']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==head
 (D/'result.json.gz').write_bytes(gzip.compress(json.dumps(j,indent=2).encode(),mtime=0))
print(json.dumps({'all_pass':j['all_pass'],'head':head,'observations':len(j['observations'])}),flush=True)
raise SystemExit(0 if j['all_pass']and all(j[k]for k in ['source_unchanged','binary_unchanged','head_unchanged'])else 1)

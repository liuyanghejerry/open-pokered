"""Normal startup/CONTINUE into Pewter PC, physical keys/readonly state and whole screenshots.
Offline disclosed HoF fixture, not fresh gameplay. No runtime setters/warp/snapshot/skip/debug save.
"""
from pathlib import Path
import sys,os,json,gzip,tempfile,hashlib,subprocess,traceback
import argparse
parser=argparse.ArgumentParser(description=__doc__)
for name in ['repo','binary','fixture','out']:parser.add_argument('--'+name,type=Path,required=True)
parser.add_argument('--role',choices=['before','after'],required=True)
args=parser.parse_args();R=args.repo.resolve();B=args.binary.resolve();D=args.out.resolve();D.mkdir();(D/'tmp').mkdir();tempfile.tempdir=str(D/'tmp')
for k in list(os.environ):
 if k.startswith('POKERED_'):del os.environ[k]
sys.path.insert(0,str(R/'scripts'));import playthrough as p
raw=args.fixture.read_bytes();save=D/'fixture.sav';save.write_bytes(raw);head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip();names=subprocess.check_output(['git','ls-files','crates','scripts','Cargo.toml','Cargo.lock'],cwd=R,text=True).splitlines()
def manifest():return {n:hashlib.sha256((R/n).read_bytes()).hexdigest()for n in names if(R/n).is_file()}
frozen=manifest();bh=hashlib.sha256(B.read_bytes()).hexdigest();j={'scope':__doc__,'role':args.role,'head':head,'binary_sha256':bh,'fixture_sha256':hashlib.sha256(raw).hexdigest(),'commands':[],'states':[],'all_pass':False};g=None
try:
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=R,text=True).strip();g=p.Game(binary=B,save_path=save,seed=42,speed=0);j['pid']=g.proc.pid;original=g.d.cmd
 def cmd(**kw):
  assert kw['cmd']in{'get_state','press_sequence','step_frames','press_timeline','capture_frame'};j['commands'].append(kw);return original(**kw)
 g.d.cmd=cmd
 def state():
  s=g.st();j['states'].append({k:s.get(k)for k in ['frame_count','screen','map_name','player_x','player_y','pc_phase','audio_sfx']});return s
 def tape(keys):g.d.cmd(cmd='press_timeline',buttons=keys,advance=True);return state()
 for request in json.loads((R/'docs/audits/offscreen-npc-availability/boot-commands.json').read_text()):
  g.d.cmd(**request);s=state()
  if s['screen']=='overworld':break
 assert(s['map_name'],s['player_x'],s['player_y'])==('PewterPokecenter',13,4),s
 tape(['up']*4+[None]*4);tape(['a']*8+[None]*120)
 for _ in range(5):
  if state()['pc_phase']=='MainMenu':break
  tape(['a']+[None]*8)
 assert state()['pc_phase']=='MainMenu'
 for _ in range(3):tape(['down']+[None]*5)
 tape(['a']+[None]*2)
 for _ in range(5):
  if state()['pc_phase']=='LeagueHoF':break
  tape(['a']+[None])
 before=state();assert before['pc_phase']=='LeagueHoF'and before['audio_sfx']['playing'],before;j['before']=before;g.d.cmd(cmd='capture_frame',path=str(D/'before-early-a.png'))
 after=tape(['a']);j['after_early_a']=after;g.d.cmd(cmd='capture_frame',path=str(D/'after-early-a.png'));assert after['pc_phase']=='LeagueHoF'and after['audio_sfx']['playing'];assert(after['audio_sfx']['id']!=before['audio_sfx']['id'])==(args.role=='before'),(before['audio_sfx'],after['audio_sfx'])
 rest=tape([None]*96);assert rest['pc_phase']=='LeagueHoF'and not rest['audio_sfx']['playing'];j['after_wait']=rest
 late=tape(['a']);j['after_late_a']=late;assert late['pc_phase']=='LeagueHoF'and late['audio_sfx']['playing'];assert late['audio_sfx']['id']!=after['audio_sfx']['id'];g.d.cmd(cmd='capture_frame',path=str(D/'after-late-a.png'));tape([None]);assert save.read_bytes()==raw;j['all_pass']=True
except BaseException:j['failure']=traceback.format_exc();print(j['failure'],flush=True)
finally:
 if g:g.close();j['exit_code']=g.proc.returncode
 j['source_unchanged']=manifest()==frozen;j['head_unchanged']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==head;j['binary_unchanged']=hashlib.sha256(B.read_bytes()).hexdigest()==bh;(D/'result.json.gz').write_bytes(gzip.compress(json.dumps(j,indent=2).encode(),mtime=0))
print(json.dumps({'all_pass':j['all_pass'],'before':j.get('before',{}).get('audio_sfx'),'after_early_a':j.get('after_early_a',{}).get('audio_sfx')}),flush=True)
raise SystemExit(0 if j['all_pass']and all(j[k]for k in ['source_unchanged','head_unchanged','binary_unchanged'])else 1)

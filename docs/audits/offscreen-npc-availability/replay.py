"""Normal SRAM boot/CONTINUE; physical key tape and readonly NPC observations.
No setters, warp, runtime save writes, snapshot load/restore or dialogue skip.
"""
from pathlib import Path
import sys,os,json,gzip,tempfile,subprocess,hashlib,traceback,shutil,argparse
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--repo',type=Path,required=True)
parser.add_argument('--binary',type=Path,required=True)
parser.add_argument('--fixture',type=Path,default=Path(__file__).with_name('fixture.sav'))
parser.add_argument('--boot-commands',type=Path,default=Path(__file__).with_name('boot-commands.json'))
parser.add_argument('--out',type=Path,required=True)
parser.add_argument('--expect-freeze',action='store_true')
parser.add_argument('--extended',action='store_true',help='observe640 nearby frames, then walk one further tile and observe128')
args=parser.parse_args()
R=args.repo.resolve();B=args.binary.resolve()
sys.path.insert(0,str(R/'scripts'));import playthrough as p
out=args.out.resolve();out.mkdir();(out/'tmp').mkdir();tempfile.tempdir=str(out/'tmp')
assert not subprocess.check_output(['git','status','--porcelain'],cwd=R,text=True).strip()
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip();names=subprocess.check_output(['git','ls-files','crates','scripts','Cargo.toml','Cargo.lock'],cwd=R,text=True).splitlines()
def sources():return {name:hashlib.sha256((R/name).read_bytes()).hexdigest() for name in names if (R/name).is_file()}
frozen=sources();binaryhash=hashlib.sha256(B.read_bytes()).hexdigest();raw=args.fixture.read_bytes();save=out/'play.sav';save.write_bytes(raw)
for name in list(os.environ):
 if name.startswith('POKERED_'):del os.environ[name]
record={'scope':'Actual native normal SRAM boot/CONTINUE; physical buttons only and readonly observations. Native before/after regression, not original timing/audio parity or fresh new-game proof.','head':head,'binary_sha256':binaryhash,'fixture_sha256':hashlib.sha256(raw).hexdigest(),'commands':[],'observations':[],'all_pass':False};game=None
(out/'source-manifest.json').write_text(json.dumps(frozen,indent=2)+'\n')
try:
 game=p.Game(binary=B,save_path=save,seed=42,speed=0);original=game.d.cmd
 def cmd(**kw):
  assert kw['cmd']in {'get_state','get_npcs','get_party','press_sequence','press_timeline','step_frames','capture_frame'}
  record['commands'].append(kw);return original(**kw)
 game.d.cmd=cmd
 prefix=json.loads(args.boot_commands.read_text())
 for request in prefix:
  assert request['cmd']in {'get_state','press_sequence','step_frames'}
  game.d.cmd(**request);s=game.st()
  if s['screen']=='overworld':break
 else:raise AssertionError('normalCONTINUE did not reach overworld')
 assert (s['map_name'],s['player_x'],s['player_y'])==('Route1',5,19),s
 record['boot']=s;record['party']=game.d.cmd(cmd='get_party')
 game.d.cmd(cmd='press_timeline',buttons=[None]*64,advance=True)
 def observe(stage):
  s=game.st();n=game.d.cmd(cmd='get_npcs')['data'];row={'stage':stage,'frame':s['frame_count'],'state':{k:s.get(k)for k in ['screen','map_name','player_x','player_y','player_facing','player_movement_state','dialogue','warp_fade']},'npcs':n};record['observations'].append(row);return row
 initial=observe('idle-start');record['idle_start']=initial
 for t in range(128):
  game.d.cmd(cmd='press_timeline',buttons=[None],advance=True);row=observe('idle')
  assert (row['state']['screen'],row['state']['map_name'],row['state']['player_x'],row['state']['player_y'])==('overworld','Route1',5,19)
 game.d.cmd(cmd='capture_frame',path=str(out/'before-approach.png'))
 record['before_approach']=record['observations'][-1]
 game.d.cmd(cmd='press_timeline',buttons=['down']*16,advance=True)
 observe('after-down16');game.d.cmd(cmd='press_timeline',buttons=[None]*16,advance=True)
 arrived=observe('after-release16');assert (arrived['state']['screen'],arrived['state']['map_name'],arrived['state']['player_x'],arrived['state']['player_y'])==('overworld','Route1',5,20),arrived
 record['arrived']=arrived
 for t in range(640 if args.extended else 96):
  game.d.cmd(cmd='press_timeline',buttons=[None],advance=True);row=observe('nearby')
  assert (row['state']['screen'],row['state']['map_name'],row['state']['player_x'],row['state']['player_y'])==('overworld','Route1',5,20)
  if t in [0,15,31,63,95,127,255,511,639]:game.d.cmd(cmd='capture_frame',path=str(out/f'nearby-{t:03}.png'))
 if args.extended:
  record['extended_nearby']=record['observations'][-1]
  game.d.cmd(cmd='press_timeline',buttons=['down']*16+[None]*16,advance=True)
  row=observe('second-step');assert (row['state']['screen'],row['state']['map_name'],row['state']['player_x'],row['state']['player_y'])==('overworld','Route1',5,21),row
  for t in range(128):
   game.d.cmd(cmd='press_timeline',buttons=[None],advance=True);row=observe('second-nearby')
   assert (row['state']['screen'],row['state']['map_name'],row['state']['player_x'],row['state']['player_y'])==('overworld','Route1',5,21)
  game.d.cmd(cmd='capture_frame',path=str(out/'second-step-end.png'))
  assert max(o['npcs'][0]['walk_counter'] for o in record['observations']if o['stage']in {'nearby','second-nearby'})>0
  assert any((o['npcs'][0]['x'],o['npcs'][0]['y'])!=(5,24)for o in record['observations']if o['stage']=='second-nearby')
 if args.expect_freeze:
  initial=record['idle_start']['npcs']
  assert all(o['npcs']==initial for o in record['observations']if o['stage']=='idle')
  assert any(o['npcs'][0]!=initial[0] for o in record['observations']if o['stage']=='nearby')
  assert all(o['npcs'][1]==initial[1] for o in record['observations'])
 assert save.read_bytes()==raw,'no in-session save mutation expected'
 record['all_pass']=True
except BaseException:
 record['failure']=traceback.format_exc();print(record['failure'],flush=True)
finally:
 if game:
  game.log.flush();shutil.copyfile(game.run_dir/'game.log',out/'game.log');game.close();record['game_exit']=game.proc.returncode
 record['source_unchanged']=sources()==frozen;record['binary_unchanged']=hashlib.sha256(B.read_bytes()).hexdigest()==binaryhash;record['head_unchanged']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==head
 (out/'result.json.gz').write_bytes(gzip.compress(json.dumps(record,indent=2).encode(),mtime=0))
print(json.dumps({'all_pass':record['all_pass'],'observations':len(record['observations']),'head':head}),flush=True)
raise SystemExit(0 if record['all_pass'] and record['source_unchanged'] and record['binary_unchanged'] and record['head_unchanged'] else 1)

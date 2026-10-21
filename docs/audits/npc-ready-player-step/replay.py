"""Native normal-SRAM/CONTINUE ready-walker regression, physical keys only."""
from pathlib import Path
import os,sys,json,gzip,subprocess,tempfile,shutil,hashlib,traceback,importlib.util,argparse
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--repo',type=Path,required=True)
parser.add_argument('--binary',type=Path,required=True)
parser.add_argument('--fixture',type=Path,default=Path(__file__).with_name('fixture.sav'))
parser.add_argument('--boot-commands',type=Path,default=Path(__file__).with_name('boot-commands.json'))
parser.add_argument('--out',type=Path,required=True)
parser.add_argument('--expect-pause',action='store_true')
args=parser.parse_args();R=args.repo.resolve();B=args.binary.resolve();out=args.out.resolve()
out.mkdir();(out/'tmp').mkdir();tempfile.tempdir=str(out/'tmp')
for name in list(os.environ):
 if name.startswith('POKERED_'):del os.environ[name]
sys.path.insert(0,str(R/'scripts'));import playthrough as p
assert not subprocess.check_output(['git','status','--porcelain'],cwd=R,text=True).strip();head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip();names=subprocess.check_output(['git','ls-files','crates','scripts','Cargo.toml','Cargo.lock'],cwd=R,text=True).splitlines()
def sources():return {name:hashlib.sha256((R/name).read_bytes()).hexdigest()for name in names if(R/name).is_file()}
frozen=sources();binaryhash=hashlib.sha256(B.read_bytes()).hexdigest();raw=args.fixture.read_bytes();save=out/'play.sav';save.write_bytes(raw)
j={'scope':__doc__+' No setters, warp, snapshot load/restore, runtime save writes or dialogue skip. Native functional before/after evidence, not original timing/audio/raster parity.','head':head,'binary_sha256':binaryhash,'fixture_sha256':hashlib.sha256(raw).hexdigest(),'commands':[],'observations':[],'violations':[],'all_pass':False};game=None
(out/'source-manifest.json').write_text(json.dumps(frozen,indent=2)+'\n')
try:
 game=p.Game(binary=B,save_path=save,seed=42,speed=0);orig=game.d.cmd
 def cmd(**kw):
  assert kw['cmd']in {'get_state','get_npcs','press_sequence','step_frames','press_timeline','capture_frame'}
  j['commands'].append(kw);return orig(**kw)
 game.d.cmd=cmd
 for c in json.loads(args.boot_commands.read_text()):
  game.d.cmd(**c);s=game.st()
  if s['screen']=='overworld':break
 assert(s['screen'],s['map_name'],s['player_x'],s['player_y'])==('overworld','PalletTown',5,10),s
 j['boot']=s;game.d.cmd(cmd='press_timeline',buttons=[None]*64,advance=True)
 def observe(stage):
  s=game.st();n=game.d.cmd(cmd='get_npcs')['data'];row={'stage':stage,'frame':s['frame_count'],'state':{k:s.get(k)for k in['screen','map_name','player_x','player_y','player_facing','player_movement_state','active_script_effect','dialogue','dialogue_state']},'npcs':n};j['observations'].append(row);return row
 prev=observe('start')
 for t in range(247):
  key=('left'if(t//32)%2==0 else'right')if t<119 else None
  game.d.cmd(cmd='press_timeline',buttons=[key],advance=True);row=observe('arrows'if t<119 else'idle')
  assert(row['state']['screen'],row['state']['map_name'])==('overworld','PalletTown'),row
  same_tile=(prev['state']['player_x'],prev['state']['player_y'])==(row['state']['player_x'],row['state']['player_y'])
  if same_tile and prev['state']['player_movement_state']==row['state']['player_movement_state']=='Walking' and row['state']['active_script_effect']is None and row['state']['dialogue']is None:
   for a,b in zip(prev['npcs'],row['npcs']):
    if a['npc_index']in [1,2] and a['walk_counter']==0 and b['walk_counter']==16 and a['scripted_path_remaining']==b['scripted_path_remaining']==0:j['violations'].append({'t':t,'slot':a['npc_index'],'before':prev,'after':row})
  if t in [118,134,150,182,246]:game.d.cmd(cmd='capture_frame',path=str(out/f'frame-{row["frame"]:04}.png'))
  prev=row
 arrow_end=next(o for o in j['observations']if o['frame']==791);idle=[o for o in j['observations']if o['stage']=='idle'];j['resumed_after_stop']=any(o['state']['player_movement_state']=='Idle'and o['npcs'][1]!=arrow_end['npcs'][1]for o in idle)
 assert j['resumed_after_stop'],'NPC must remain able to update after the player stops'
 if args.expect_pause:assert not j['violations'],j['violations']
 assert save.read_bytes()==raw
 j['all_pass']=True
except BaseException:j['failure']=traceback.format_exc();print(j['failure'],flush=True)
finally:
 if game:game.log.flush();shutil.copyfile(game.run_dir/'game.log',out/'game.log');game.close();j['game_exit']=game.proc.returncode
 j['source_unchanged']=sources()==frozen;j['binary_unchanged']=hashlib.sha256(B.read_bytes()).hexdigest()==binaryhash;j['head_unchanged']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==head;(out/'result.json.gz').write_bytes(gzip.compress(json.dumps(j,indent=2).encode(),mtime=0))
print(json.dumps({'all_pass':j['all_pass'],'violations':[(v['before']['frame'],v['after']['frame'],v['slot'])for v in j['violations']],'observations':len(j['observations']),'resumed_after_stop':j.get('resumed_after_stop')}),flush=True)
raise SystemExit(0 if j['all_pass']and all(j[k]for k in ['source_unchanged','binary_unchanged','head_unchanged'])else 1)

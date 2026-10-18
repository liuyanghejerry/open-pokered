from pathlib import Path
import sys,os,json,tempfile,shutil,traceback,hashlib,collections
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sys.path.insert(0,str(R/'scripts'))
import playthrough as p
role=sys.argv[1];binary=Path(sys.argv[2]).resolve();out=Path(sys.argv[3]).resolve();out.mkdir();(out/'tmp').mkdir();tempfile.tempdir=str(out/'tmp')
for k in list(os.environ):
 if k.startswith('POKERED_'):del os.environ[k]
# Keep the caller's dynamic-library environment; never override game state.
x={'all_pass':False,'role':role,'binary':str(binary),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'scope':'offline constructed persistent fixture; ordinary Continue, real Rocket battle, then frame-by-frame buttons and observations; not fresh route','commands':{},'trace':[]}
class VictoryReached(Exception):pass
class Game(p.Game):
 armed=False
 def st(self):
  s=super().st()
  if self.armed and s['screen']=='battle' and s['battle_phase'].startswith('TrainerVictory'):
   raise VictoryReached()
  return s
g=None
try:
 g=Game(binary=binary,snapshot=D/'fixture.json',seed=42,speed=0);x['Game_pid']=g.proc.pid;x['argv']=g.proc.args
 original=g.d.cmd
 allowed={'get_state','get_npcs','get_flags','get_party','get_bag','get_agent_state','press_sequence','press_timeline','press','step_frames','wait_until','skip_dialogue','capture_frame'}
 def cmd(*args,**kw):
  name=kw.get('cmd',args[0]if args else None);assert name in allowed,name;x['commands'][name]=x['commands'].get(name,0)+1
  return original(*args,**kw)
 g.d.cmd=cmd
 p.resume_reentry(g);assert g.cutscene();s=g.st();assert s['map_name']=='PokemonTower7F'
 x['npcs_before']=g.d.npcs();assert x['npcs_before'][0]['visible']
 g.face('up');g.tap('a',16)
 for _ in range(500):
  s=g.st()
  if s['screen']=='battle':break
  g.tap('a',4)
 assert s['screen']=='battle',s
 g.armed=True
 try:g.battle_loop()
 except VictoryReached:pass
 else:raise AssertionError('no trainer victory observed')
 g.armed=False;start=None;last=None
 for i in range(1500):
  s=g.st();n=g.d.npcs()[0];pos=[n['x'],n['y']]
  if s['screen']=='overworld' and start is None:start=s['frame_count']
  if last!=(pos,n['visible']):
   x['trace'].append({'frame':s['frame_count'],'position':pos,'visible':n['visible']});last=(pos,n['visible'])
  if start is not None and s['frame_count']-start==140:
   x['comparison_state']=s;assert g.d.cmd(cmd='capture_frame',path=str(out/'comparison.png'))['ok']
  if start is not None and s['frame_count']-start>=350 and not n['visible']:break
  g.d.cmd(cmd='press_timeline',buttons=['a' if i%8==0 else None]);g.step(1)
 else:raise AssertionError('departure did not finish')
 x['after']=g.st();x['flags']=g.d.cmd(cmd='get_flags')['data'];assert x['flags']['EVENT_BEAT_POKEMONTOWER_7_TRAINER_0'];assert not g.d.npcs()[0]['visible'];x['all_pass']=True
except BaseException:x['failure']=traceback.format_exc();print(x['failure'],flush=True)
finally:
 if g:
  g.log.flush();shutil.copy2(g.run_dir/'game.log',out/'game.log');g.close();x['Game_exit']=g.proc.returncode
 (out/'result.json').write_text(json.dumps(x,indent=2)+'\n')
print(json.dumps({'all_pass':x['all_pass'],'result':str(out/'result.json')}),flush=True)
raise SystemExit(0 if x['all_pass']else 1)

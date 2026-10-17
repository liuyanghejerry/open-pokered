from pathlib import Path
import sys,os,json,tempfile,traceback,subprocess,hashlib,shutil
import argparse
ap=argparse.ArgumentParser();ap.add_argument('--binary',type=Path,required=True);ap.add_argument('--artifacts',type=Path,required=True);args=ap.parse_args()
HERE=Path(__file__).resolve().parent;R=HERE.parents[2];W=R;D=args.artifacts.resolve();D.mkdir(parents=True,exist_ok=False);(D/'tmp').mkdir()
for k in list(os.environ):
 if k.startswith('POKERED_'):del os.environ[k]
os.environ['LD_LIBRARY_PATH']='/tmp/pokered-build-lib';tempfile.tempdir=str(D/'tmp');sys.path.insert(0,str(W/'scripts'));import playthrough as p
x={'complete':False,'all_pass':False,'scope':'six real quiz machines via fixed driver from persistent failure export, normal CLI ordinary Continue/buttons; product binary unchanged; not full fresh route or ordinary SAVE proof','driver_file_sha256':hashlib.sha256((W/'scripts/playthrough.py').read_bytes()).hexdigest(),'commands':{}};g=None
allowed={'get_state','get_npcs','get_flags','get_agent_state','get_party','get_bag','get_position','press','press_sequence','press_timeline','step_frames','wait_until','skip_dialogue'}
try:
 assert hashlib.sha256((HERE/'fixture.json').read_bytes()).hexdigest()==json.loads((HERE/'evidence.json').read_text())['fixture_sha256']
 g=p.Game(snapshot=HERE/'fixture.json',binary=args.binary.resolve(),seed=42,speed=0)
 x['argv']=g.proc.args;x['Game_pid']=g.proc.pid
 orig=g.d.cmd
 def cmd(*args,**kwargs):
  c=kwargs.get('cmd',args[0]if args else None);assert c in allowed,c;x['commands'][c]=x['commands'].get(c,0)+1;return orig(*args,**kwargs)
 g.d.cmd=cmd;p.resume_reentry(g);assert g.cutscene();x['quizzes']=[]
 for number,(qx,qy,answer) in enumerate([(15,7,'YES'),(10,1,'NO'),(9,7,'NO'),(9,13,'NO'),(1,13,'YES'),(1,7,'NO')],1):
  g.nav_to(qx,qy+1,'CinnabarGym');g.face('up');g.tap('a',16);g.dialogue_then_choice();before=g.st();g.choose(answer);assert g.cutscene();after=g.st();flags=g.d.cmd(cmd='get_flags')['data'];flag=f'EVENT_CINNABAR_GYM_GATE{number}_UNLOCKED';assert flags.get(flag);assert not flags.get(f'EVENT_BEAT_CINNABAR_GYM_TRAINER_{number}');x['quizzes'].append({'number':number,'answer':answer,'before_choice':before,'after':after,'gate_unlocked':True,'trainer_unbeaten':True})
 assert hashlib.sha256((HERE/'fixture.json').read_bytes()).hexdigest()==json.loads((HERE/'evidence.json').read_text())['fixture_sha256'];assert hashlib.sha256((W/'scripts/playthrough.py').read_bytes()).hexdigest()==x['driver_file_sha256'];x['all_pass']=True
except BaseException:x['failure']=traceback.format_exc();print(x['failure'],flush=True)
finally:
 if g is not None:
  shutil.copy2(g.run_dir/'game.log',D/'game.log');g.close();x['Game_exit']=g.proc.poll()
 x['complete']=True;(D/'result.json').write_text(json.dumps(x,indent=2)+'\n')
print(json.dumps({'all_pass':x['all_pass'],'result':str(D/'result.json')}),flush=True);raise SystemExit(0 if x['all_pass']else 1)

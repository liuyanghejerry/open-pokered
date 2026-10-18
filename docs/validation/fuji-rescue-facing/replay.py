from pathlib import Path
import os,sys,json,shutil,tempfile,traceback,hashlib
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sys.path.insert(0,str(R/'scripts'));import playthrough as p
role=sys.argv[1];assert role in ["before","after"];out=Path(sys.argv[3]).resolve();out.mkdir();(out/'tmp').mkdir();tempfile.tempdir=str(out/'tmp')
for k in list(os.environ):
 if k.startswith('POKERED_'):del os.environ[k]
binary=Path(sys.argv[2]).resolve()
x={'complete':False,'all_pass':False,'scope':'constructed persistent fixture, ordinary Continue and actual Fuji rescue from his right side (11,3), verified walkable; not fresh route','binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'commands':{}};g=None
try:
 g=p.Game(binary=binary,snapshot=D/'fixture.json',seed=42,speed=0);x['Game_pid']=g.proc.pid;x['argv']=g.proc.args
 original=g.d.cmd
 def command(*args,**kwargs):
  name=kwargs.get('cmd',args[0]if args else None);assert name in {'get_state','get_npcs','get_flags','press_sequence','press_timeline','step_frames','wait_until','skip_dialogue','capture_frame'},name;x['commands'][name]=x['commands'].get(name,0)+1;r=original(*args,**kwargs);assert r['ok'],r;return r
 g.d.cmd=command
 p.resume_reentry(g);assert g.cutscene();assert g.st()['map_name']=='PokemonTower7F';assert g.d.npcs()[3]['visible']
 g.face('left');x['before']=g.st();g.tap('a',16);assert g.cutscene();assert g.st()['map_name']=='MrFujisHouse';assert g.cutscene();assert g.st()['frame_count']<=1500;g.step(1500-g.st()['frame_count']);x['after']=g.st();x['flags']=g.d.cmd(cmd='get_flags')['data'];assert x['flags']['EVENT_RESCUED_MR_FUJI'];assert[x['after']['player_x'],x['after']['player_y']]==[3,7];assert g.d.cmd(cmd='capture_frame',path=str(out/'arrival.png'))['ok'];assert x['after']['player_facing']==('Up' if role=='after' else 'Left');x['all_pass']=True
except BaseException:x['failure']=traceback.format_exc();print(x['failure'],flush=True)
finally:
 if g:
  g.log.flush();shutil.copy2(g.run_dir/'game.log',out/'game.log');g.close();x['Game_exit']=g.proc.returncode
 x['complete']=True;(out/'result.json').write_text(json.dumps(x,indent=2)+'\n')
print(json.dumps({'all_pass':x['all_pass'],'result':str(out/'result.json')}),flush=True)
raise SystemExit(0 if x['all_pass']else 1)

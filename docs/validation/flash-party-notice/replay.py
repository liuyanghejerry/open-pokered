from pathlib import Path
import sys,os,json,tempfile,shutil,traceback,hashlib
D=Path(__file__).resolve().parent;R=D.parents[2];W=R;sys.path.insert(0,str(W/'scripts'));import playthrough as p
role=sys.argv[1];binary=Path(sys.argv[2]).resolve();out=Path(sys.argv[3]).resolve();out.mkdir();(out/'tmp').mkdir();tempfile.tempdir=str(out/'tmp')
for k in list(os.environ):
 if k.startswith('POKERED_'):del os.environ[k]
fixture=D/'fixture.sav';save=out/'play.sav';shutil.copy2(fixture,save);x={'all_pass':False,'role':role,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256(fixture.read_bytes()).hexdigest(),'scope':'ordinary Continue and START/Pokemon/FLASH; notice backing and input lock only; full restoration timing not aligned','commands':{}};g=None
try:
 g=p.Game(binary=binary,save_path=save,seed=42,speed=0);x.update(Game_pid=g.proc.pid,argv=g.proc.args);original=g.d.cmd
 def cmd(*a,**kw):
  n=kw.get('cmd',a[0]if a else None);assert n in {'get_state','get_npcs','get_flags','get_party','get_bag','press_sequence','press_timeline','step_frames','wait_until','skip_dialogue','capture_frame'},n;x['commands'][n]=x['commands'].get(n,0)+1;return original(*a,**kw)
 g.d.cmd=cmd;p.resume_reentry(g);assert g.cutscene();g.step(64);s=g.st();assert s['map_name']=='RockTunnel1F' and(s['player_x'],s['player_y'])==(15,3);x['initial']=s
 g.tap('start',12);s=g.st();assert s['screen']=='start-menu';target=s['field_menu']['items'].index('Pokemon')
 for _ in range(10):
  if g.st()['field_menu']['cursor']==target:break
  g.tap('down',8)
 assert g.st()['field_menu']['cursor']==target;g.tap('a',32);s=g.st();assert s['screen']=='party'
 for _ in range(8):
  if g.st()['field_menu']['cursor']==0:break
  g.tap('up',8)
 g.tap('a',32);s=g.st();assert s['field_menu']['field_moves'][0]=='Flash' and s['field_menu']['phase'].startswith('ActionMenu');x['pretrigger']=s;assert g.d.cmd(cmd='press_timeline',buttons=['a']+[None]*119,advance=True)['ok'];s=g.st();x['notice']=s;assert s['screen']==('overworld'if role=='before'else'party');assert(s['player_x'],s['player_y'])==(15,3);assert g.d.cmd(cmd='capture_frame',path=str(out/'notice.png'))['ok']
 x['notice_npcs']=g.d.npcs();g.d.drive(['down']*60,frames=60);s=g.st();assert(s['player_x'],s['player_y'])==(15,3);assert s['screen']==('overworld'if role=='before'else'party');x['held_direction']=s
 assert g.d.cmd(cmd='press_timeline',buttons=[None,'b']+[None]*18,advance=True)['ok'];s=g.st();x['after_ack']=s;assert s['screen']=='overworld' and(s['player_x'],s['player_y'])==(15,3) and s['dialogue_state']is None;assert g.d.cmd(cmd='capture_frame',path=str(out/'returned.png'))['ok'];x['all_pass']=True
except BaseException:x['failure']=traceback.format_exc();print(x['failure'],flush=True)
finally:
 if g:g.log.flush();shutil.copy2(g.run_dir/'game.log',out/'game.log');g.close();x['Game_exit']=g.proc.returncode
 (out/'result.json').write_text(json.dumps(x,indent=2)+'\n')
print(json.dumps({'all_pass':x['all_pass'],'result':str(out/'result.json')}),flush=True);raise SystemExit(0 if x['all_pass']else 1)

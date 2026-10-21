from pathlib import Path
import os,sys,json,tempfile,shutil,subprocess,hashlib,gzip,traceback,argparse
R=Path(__file__).resolve().parents[3];E=Path(__file__).resolve().parent;W=R;sys.path.insert(0,str(W/'scripts'));import playthrough as p
ap=argparse.ArgumentParser();ap.add_argument('--output',type=Path,required=True);ap.add_argument('--binary',type=Path,default=R/'target/debug/pokered-app');ap.add_argument('--expected-last-line',default='catch anything!');args=ap.parse_args()
out=args.output.resolve();out.mkdir();(out/'tmp').mkdir();tempfile.tempdir=str(out/'tmp');raw=(E/'fixture.sav').read_bytes();save=out/'play.sav';save.write_bytes(raw);binary=args.binary.resolve();g=None;rec={'source':subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256(raw).hexdigest(),'commands':[],'pages':[],'scope':'Offline SRAM player8,4 adjacent to canonical scientist8,3; beatentrainerflags. Normal boot/CONTINUE and physical A, no runtime mutation/skip/warp/restore; not freshtravel/battle proof.','passed':False}
try:
 for k in list(os.environ):
  if k.startswith('POKERED_'):del os.environ[k]
 g=p.Game(binary=binary,save_path=save,seed=42,speed=0);original=g.d.cmd
 def cmd(**kw):
  assert kw['cmd'] in {'get_state','get_npcs','get_flags','capture_frame','press_timeline'};rec['commands'].append(kw);return original(**kw)
 g.d.cmd=cmd
 def drive(buttons):assert g.d.cmd(cmd='press_timeline',buttons=buttons,advance=True)['ok']
 def tap():drive(['a']*3+[None]*180)
 for _ in range(30):
  if g.st()['screen']=='overworld':break
  drive(['a']*3+[None]*60)
 drive([None]*120);drive(['up']*4+[None]*60);s=g.st();assert(s['map_name'],s['player_x'],s['player_y'],s['player_facing'])==('SilphCo5F',8,4,'Up'),s;assert not s['dialogue_state'];rec['pretrigger']=s;rec['flags']=g.d.cmd(cmd='get_flags');rec['npcs']=g.d.cmd(cmd='get_npcs');n=next(x for x in rec['npcs']['data'] if x['npc_index']==2);assert(n['x'],n['y'],n['visible'])==(8,3,True);g.d.cmd(cmd='capture_frame',path=str(out/'pretrigger.png'))
 tap()
 for _ in range(8):
  s=g.st();d=s['dialogue_state'];assert d and d['waiting_for_input'],s;name=f"page-{d['page']}.png";g.d.cmd(cmd='capture_frame',path=str(out/name));rec['pages'].append({'state':s,'png':name,'sha256':hashlib.sha256((out/name).read_bytes()).hexdigest()})
  if d['page']==d['total_pages']:break
  tap()
 else:raise AssertionError('too many dialogue pages')
 assert rec['pages'][0]['state']['dialogue_state']['text'].startswith('We worked on'),rec['pages']
 assert rec['pages'][-1]['state']['dialogue_state']['text']==args.expected_last_line,rec['pages']
 tap();drive([None]*120);s=g.st();assert(s['screen'],s['player_x'],s['player_y'])==('overworld',8,4);assert not s['dialogue_state'];assert save.read_bytes()==raw;rec['final']=s;rec['passed']=True
except BaseException:rec['failure']=traceback.format_exc();print(rec['failure'],flush=True)
finally:
 if g:g.log.flush();shutil.copyfile(g.run_dir/'game.log',out/'game.log');g.close();rec['game_exit']=g.proc.returncode
 (out/'result.json.gz').write_bytes(gzip.compress(json.dumps(rec,indent=2).encode(),mtime=0))
print(json.dumps({'passed':rec['passed'],'texts':[x['state']['dialogue_state']['text'] for x in rec['pages']]}),flush=True)
raise SystemExit(0 if rec['passed'] else 1)

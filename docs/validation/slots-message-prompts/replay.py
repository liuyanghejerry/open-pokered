"""Run from workspace root; normal Continue, actual buttons, no runtime setters."""
from pathlib import Path
import sys,os,json,gzip,hashlib,tempfile,traceback,shutil,subprocess
root=Path.cwd();sys.path.insert(0,str(root/'scripts'));import playthrough as p
binary=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();out.mkdir(parents=True,exist_ok=True)
for key in list(os.environ):
 if key.startswith('POKERED_'):del os.environ[key]
fixture_path=root/'docs/validation/slots-entry-flow/fixture.json.gz'
base_bytes=gzip.decompress(fixture_path.read_bytes());base=json.loads(base_bytes)
r={'workspace_head':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256(base_bytes).hexdigest(),'scope':'Native offline fixture, normal boot/Continue, actual buttons; read-only observation. Not original-ROM runtime or timing equivalence.','cases':[],'complete':False}
try:
 for case in ['insufficient-a','insufficient-b','loss-idle','loss-zero']:
  row={'case':case,'commands':[],'states':[]};r['cases'].append(row)
  with tempfile.TemporaryDirectory(prefix='slot-prompts-') as tmp:
   fixture=json.loads(json.dumps(base));fixture['game_data']['bag']['items']=[['CoinCase',1]];coins=2 if case.startswith('insufficient') or case=='loss-zero' else 99;fixture['game_data']['player_coins']=coins
   path=Path(tmp)/'fixture.json';path.write_text(json.dumps(fixture));g=p.Game(binary=binary,snapshot=path,seed=42,speed=0)
   original=g.d.cmd
   def cmd(*args,**kw):
    assert kw['cmd'] in {'get_state','get_bag','press_sequence','press_timeline','press','step_frames','wait_until','capture_frame'}
    row['commands'].append(kw);v=original(*args,**kw)
    if kw['cmd']=='get_state':row['states'].append(v['data'])
    return v
   g.d.cmd=cmd
   def shot(label):
    file=out/f'{case}-{label}.png';g.d.cmd(cmd='capture_frame',path=str(file));row[label]=g.st();return hashlib.sha256(file.read_bytes()).hexdigest()
   try:
    p.resume_reentry(g);g.step(60);g.face('up');g.tap('a',120);g.wait('choice_open',600);g.choose('YES');g.wait('screen=slots',600)
    if case.startswith('insufficient'):
     g.tap('a',2);first=shot('after-a');g.step(120);assert shot('after-idle')==first
     g.tap('down',16);assert shot('ignored-direction')==first
     g.tap(case[-1],2);shot('acknowledged');assert g.st()['screen']=='slots' and g.st()['coins']==2
     g.tap('b',120);assert g.st()['screen']=='overworld' and g.st()['coins']==2
    else:
     for key in ['down','down','up']:g.tap(key,16)
     g.tap('a',80)
     for _ in range(3):g.tap('a',40)
     g.step(2000);first=shot('after-loss-idle');assert g.st()['coins']==coins-2 and g.st()['screen']=='slots'
     g.step(120);assert shot('still-waiting')==first
     g.tap('down',16);assert shot('ignored-direction')==first
     g.d.cmd(cmd='press_timeline',buttons=['b'],advance=True);shot('acknowledged')
     assert g.st()['screen']=='slots'
     if case=='loss-zero':
      g.step(59);assert g.st()['screen']=='slots';g.step(1);assert g.st()['screen']=='overworld'
     else:
      g.step(120);assert g.st()['screen']=='slots';g.tap('b',120);assert g.st()['screen']=='overworld'
    g.wait('control_ready',600);g.d.drive(['left']*32,frames=32);s=g.st();assert s['map_name']=='GameCorner' and s['player_x']<18 and s['player_y']==16
    row.update(complete=True,final_coins=s['coins'],control_position=[s['player_x'],s['player_y']])
   finally:
    g.log.flush();shutil.copy2(g.run_dir/'game.log',out/f'{case}.log');g.close()
 r['complete']=True
except BaseException:r['failure']=traceback.format_exc();print(r['failure'],flush=True)
finally:(out/'verification.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'complete':r['complete'],'cases':len(r['cases'])}),flush=True)
raise SystemExit(0 if r['complete'] else 1)

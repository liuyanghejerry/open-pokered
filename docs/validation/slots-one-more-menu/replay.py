"""Replay OneMoreGo through normal Continue and actual buttons only."""
from pathlib import Path
import sys,json,gzip,hashlib,tempfile,shutil,os,traceback,subprocess
from PIL import Image
root=Path.cwd();sys.path.insert(0,str(root/'scripts'));import playthrough as p
binary=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();out.mkdir(parents=True,exist_ok=True)
for k in list(os.environ):
 if k.startswith('POKERED_'):del os.environ[k]
fixture_bytes=gzip.decompress((root/'docs/validation/slots-entry-flow/fixture.json.gz').read_bytes());fixture=json.loads(fixture_bytes);fixture['game_data']['bag']['items']=[['CoinCase',1]];fixture['game_data']['player_coins']=99
r={'workspace_head':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256(fixture_bytes).hexdigest(),'scope':'Native offline fixture, normal boot/Continue; buttons/read-only observation only. Not originalROM or whole-slots polling/audio parity.','cases':[],'complete':False}
try:
 for case in ['down-a','b','yes','early-held-a','clamp']:
  row={'case':case,'commands':[],'states':[]};r['cases'].append(row)
  with tempfile.TemporaryDirectory(prefix='slots-one-more-') as tmp:
   path=Path(tmp)/'fixture.json';path.write_text(json.dumps(fixture));g=p.Game(binary=binary,snapshot=path,seed=42,speed=0);original=g.d.cmd
   def cmd(*a,**kw):
    assert kw['cmd'] in {'get_state','get_bag','press_sequence','press_timeline','press','step_frames','wait_until','capture_frame'}
    row['commands'].append(kw);v=original(*a,**kw)
    if kw['cmd']=='get_state':row['states'].append(v['data'])
    return v
   g.d.cmd=cmd
   def shot(label):
    path=out/f'{case}-{label}.png';g.d.cmd(cmd='capture_frame',path=str(path));row[label]=g.st();return path
   def choice(label):
    path=shot(label)
    with Image.open(path) as im:
     im=im.convert('RGB');ink=[sum(px!=(255,255,255) for px in im.crop((120,y,127,y+10)).getdata()) for y in [104,120]]
    assert sorted(ink)==[0,5],ink
    return ink.index(5)
   try:
    p.resume_reentry(g);g.step(60);g.face('up');g.tap('a',120);g.wait('choice_open',600);g.choose('YES');g.wait('screen=slots',600)
    for key in ['down','down','up']:g.tap(key,16)
    g.tap('a',80)
    for _ in range(3):g.tap('a',40)
    g.step(2000)
    if case=='early-held-a':
     before_ack=g.st()['frame_count']
     g.d.cmd(cmd='press_timeline',buttons=['b'],advance=True)
     assert g.st()['frame_count']==before_ack+1
     g.d.cmd(cmd='press_timeline',buttons=['a']*3,advance=True)
     assert g.st()['frame_count']==before_ack+4
     assert choice('accepted-during-third-frame')==0
     g.d.cmd(cmd='press_timeline',buttons=['a']*14,advance=True)
     assert choice('fourteenth-completion-frame')==0
     assert g.st()['frame_count']==before_ack+18
     row['timing_frames']={'parent_ack':before_ack+1,'menu_accept':before_ack+4,'still_menu':before_ack+18,'handoff':before_ack+19}
     g.d.cmd(cmd='press_timeline',buttons=['a']*2,advance=True)
     shot('held-through-handoff');assert g.st()['screen']=='slots' and g.st()['coins']==97
     g.step(1);g.tap('a',2);assert g.st()['coins']==94
    else:
     g.tap('b',120);assert choice('menu')==0;assert g.st()['coins']==97
     if case=='down-a':g.tap('down',16);assert choice('down')==1;g.tap('a',120);shot('confirmed');assert g.st()['screen']=='overworld'
     elif case=='b':g.tap('b',120);shot('confirmed');assert g.st()['screen']=='overworld'
     elif case=='yes':
      g.tap('a',120);shot('confirmed');assert g.st()['screen']=='slots' and g.st()['coins']==97
      g.tap('a',2);assert g.st()['coins']==94
     else:
      for index,(key,expected) in enumerate([('up',0),('down',1),('down',1),('right',1),('left',1),('up',0),('up',0)]):
       g.tap(key,16);assert choice(f'cursor-{index}')==expected
      g.tap('a',120);shot('confirmed');assert g.st()['screen']=='slots' and g.st()['coins']==97
      g.tap('b',120);assert g.st()['screen']=='overworld'
    if g.st()['screen']=='overworld':
     assert g.st()['coins']==97;g.wait('control_ready',600);g.d.drive(['left']*32,frames=32);s=g.st();assert s['map_name']=='GameCorner' and s['player_x']<18 and s['player_y']==16
     row['control_position']=[s['player_x'],s['player_y']]
    row.update(complete=True,final_coins=g.st()['coins'],final_screen=g.st()['screen'])
   finally:g.log.flush();shutil.copy2(g.run_dir/'game.log',out/f'{case}.log');g.close()
 r['complete']=True
except BaseException:r['failure']=traceback.format_exc();print(r['failure'],flush=True)
finally:(out/'verification.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'complete':r['complete'],'cases':len(r['cases'])}),flush=True)
raise SystemExit(0 if r['complete'] else 1)

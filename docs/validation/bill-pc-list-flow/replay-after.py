"""Normal boot/CONTINUE, physical input and read-only observations, from workspace root."""
from pathlib import Path
import sys,os,json,gzip,tempfile,hashlib,shutil,subprocess,traceback
from PIL import Image
root=Path.cwd();sys.path.insert(0,str(root/'scripts'));import playthrough as p
binary=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();fixtures=Path(sys.argv[3]).resolve();mode=sys.argv[4];assert mode in ['before','after'];out.mkdir(parents=True,exist_ok=True)
for key in list(os.environ):
 if key.startswith('POKERED_'):del os.environ[key]
prefix=json.loads(gzip.decompress((fixtures/'prefix.json.gz').read_bytes()));cases=json.loads((fixtures/'cases.json').read_text())
r={'source':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'mode':mode,'scope':'Offline JSON fixtures, normal boot/Continue, actual single-key input/read-only RPCs. export_fixture clones live save data without writing it. Native verification against original source, not original-ROM runtime or fresh mainline.','cases':[],'complete':False}
try:
 for spec in cases:
  name=spec['name'];raw=gzip.decompress((fixtures/(name+'.json.gz')).read_bytes());row={'case':name,'species':spec['species'],'owned':spec['owned'],'fixture_sha256':hashlib.sha256(raw).hexdigest(),'commands':[],'observations':[],'audio_trace':[],'complete':False};r['cases'].append(row)
  with tempfile.TemporaryDirectory(prefix='bill-dex-') as tmp:
   path=Path(tmp)/'fixture.json';path.write_bytes(raw);g=p.Game(binary=binary,snapshot=path,seed=42,speed=0);original=g.d.cmd
   def cmd(*a,**kw):
    assert kw['cmd'] in {'get_state','get_npcs','get_flags','get_bag','export_fixture','press_sequence','press_timeline','step_frames','wait_until','capture_frame'}
    row['commands'].append(kw);return original(*a,**kw)
   g.d.cmd=cmd
   def step(key=None,count=1):g.d.cmd(cmd='press_timeline',buttons=[key]*count,advance=True)
   def observe(label,capture=False):
    s=g.st();row['observations'].append({'label':label,'state':s})
    if capture:g.d.cmd(cmd='capture_frame',path=str(out/f'{name}-{label}.png'))
    return s
   def dex():return g.d.cmd(cmd='export_fixture')['data']['game_data']['pokedex']
   try:
    for request in prefix['commands']:
     if request['cmd']=='capture_frame':break
     g.d.cmd(**request)
    assert g.pos()==('BillsHouse',1,5) and g.st()['frame_count']==804
    initial_bag=g.d.cmd(cmd='get_bag')['data'];initial_dex=dex();index=spec['dex']-1;byte=index//8;mask=1<<(index%8)
    assert bool(initial_dex['owned'][byte]&mask)==spec['owned']
    assert bool(initial_dex['seen'][byte]&mask)==spec['seen']
    step('a');step(count=300);s=g.st();observe('question',True);assert s['choice'] is not None and 'Which POKeMON' in s['dialogue'];assert s['script_effect']['state']['phase']=='Poll'
    step(count=31);s=g.st();assert s['choice']['options']==['EEVEE','FLAREON','JOLTEON','VAPOREON','CANCEL']
    for _ in range(spec['index']):step('down');step(count=8)
    assert g.st()['choice']['selected']==spec['index'];step('a')
    if name=='eevee-unowned':
     for _ in range(300):
      s=g.st();row['audio_trace'].append({'frame':s['frame_count'],'sfx':s['audio_sfx'],'effect':s['script_effect']});step()
    else:step(count=300)
    s=observe('entry',True)
    assert s['frame_count']==1437+9*spec['index'] and g.pos()==('BillsHouse',1,5)
    assert s['script_effect']['effect']=='BillPokemonList' and s['script_effect']['state']['phase']=='Data'
    if name=='eevee-unowned':
     assert any(t['sfx']=={'id':45,'playing':True} and t['effect']['state']['phase']=='Cry' for t in row['audio_trace'])
     whites=[t for t in row['audio_trace'] if isinstance(t['effect']['state']['phase'],dict) and 'White' in t['effect']['state']['phase']]
     assert len(whites)==6,whites
    with Image.open(out/f'{name}-entry.png')as im:
     assert im.size==(160,144);body=im.convert('RGB').crop((8,96,145,132));ink=sum(px!=(255,255,255)for px in body.getdata())
    row['description_ink_pixels']=ink
    assert ink>0 if spec['owned'] else ink==0,(name,ink)
    for presses in range(1,9):
     step(count=4);step('b')
     if name=='eevee-unowned':
      row['return_trace']=[]
      for tick in range(10):
       current=g.st();row['return_trace'].append({'frame':current['frame_count'],'phase':current['script_effect']['state']['phase'],'seen':dex()['seen']})
       assert current['script_effect']['state']['phase']=={'ReturnMap':10-tick}
       assert dex()['seen']==initial_dex['seen']
       step()
      current=g.st();assert current['script_effect']['state']['phase']=='Question'
      expected=initial_dex['seen'].copy();expected[byte]|=mask;assert dex()['seen']==expected
      step(count=16)
     else:step(count=26)
     s=g.st()
     if s['choice'] and s['script_effect']['state']['phase']=='Poll':break
     if s['choice']:step(count=12);s=g.st();assert s['script_effect']['state']['phase']=='Poll';break
    else:raise AssertionError('entry did not return to list')
    if not spec['owned']:assert presses==1
    if spec['owned']:assert presses==2
    row['entry_exit_presses']=presses;assert s['choice']['selected']==spec['index'];assert 'Which POKeMON' in s['dialogue'];observe('returned',True)
    final_dex=dex();expected_seen=initial_dex['seen'].copy();expected_seen[byte]|=mask
    assert final_dex['seen']==expected_seen and final_dex['owned']==initial_dex['owned']
    assert g.d.cmd(cmd='get_bag')['data']==initial_bag;step('b');step(count=30);g.wait('control_ready',600)
    step('right',32);s=g.st();assert s['map_name']=='BillsHouse'and s['player_y']==5 and s['player_x']>1
    assert path.read_bytes()==raw
    row.update(complete=True,initial_dex=initial_dex,final_dex=final_dex,control_position=[s['player_x'],s['player_y']])
   finally:g.log.flush();shutil.copyfile(g.run_dir/'game.log',out/(name+'.log'));g.close()
 r['complete']=True
except BaseException:r['failure']=traceback.format_exc();print(r['failure'],flush=True)
finally:(out/'verification.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'complete':r['complete'],'cases':len(r['cases'])}),flush=True)
raise SystemExit(0 if r['complete'] else 1)

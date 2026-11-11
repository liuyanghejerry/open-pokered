import argparse, hashlib, io, json, gzip
from pathlib import Path
from pyboy import PyBoy
a=argparse.ArgumentParser()
a.add_argument('--rom',type=Path,required=True)
a.add_argument('--symbols',type=Path,required=True)
a.add_argument('--state',type=Path,default=Path(__file__).resolve().parents[4]/'docs/audits/2026-11-05/start-menu-close/fast-cancel/reference.state')
a.add_argument('--output',type=Path,required=True)
a.add_argument('--record-all-frames',action='store_true')
args=a.parse_args()
out=args.output;out.mkdir(exist_ok=False)
rom=args.rom;fixture=args.state;symbols_path=args.symbols
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(rom)=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
assert sha(fixture)=='b12777818a990843546b623615caa67a05a811941dfcf4d078b46a564d6e09a4'
symbols={}
for line in symbols_path.read_text().splitlines():
 parts=line.split()
 if len(parts)==2 and ':' in parts[0]:symbols[parts[1]]=tuple(int(p,16) for p in parts[0].split(':'))

summary=[]
for preamble,idx in [('Pokedex',0),('TrainerInfo',3),('Pokemon',1)]:
 records=[]
 for repeat in [1,2]:
  p=PyBoy(io.BytesIO(rom.read_bytes()),window='null',sound_emulated=True);p.set_emulation_speed(0);p.load_state(io.BytesIO(fixture.read_bytes()))
  for key in ['a','b','start','select','up','down','left','right']:p.button_release(key)
  def press(key,n=2,wait=24):
   p.button_press(key);p.tick(n,render=True,sound=True);p.button_release(key);p.tick(wait,render=True,sound=True)
  get=lambda n:int(p.memory[symbols[n][1]])
  def select(index):
   for _ in range(8):
    if get('wCurrentMenuItem')==index:return
    press('down')
   raise AssertionError(index)
  p.tick(8,render=True,sound=True);press('b',2,160)
  select(idx);press('a',2,240);press('b',2,160)
  select(4);press('a',2,240);press('down');assert get('wCurrentMenuItem')==1
  p.button_press('a');p.tick(80,render=True,sound=True)
  folder=out/f'{preamble}-{repeat}';folder.mkdir();p.screen.image.save(folder/'initial.png')
  def snap():return {'cycles':p._cycles(),'x':get('wXCoord'),'y':get('wYCoord'),'held':get('hJoyHeld'),'raw':get('hJoyInput'),'lcdc':int(p.memory[0xff40]),'ly':int(p.memory[0xff44])}
  record={'initial':snap(),'frames':[],'events':[],'images':{'initial':sha(folder/'initial.png')}};now=[-1]
  def hook(name):record['events'].append({'tick':now[0],'routine':name,**snap()})
  for name in ['ReadJoypad','HoldTextDisplayOpen','CloseTextDisplay','OverworldLoop']:p.hook_register(*symbols[name],hook,name)
  try:
   p.button_release('a')
   for tick in range(40):
    now[0]=tick;p.tick(1,render=True,sound=True)
    record['frames'].append({'tick':tick,**snap()})
    path=folder/f'frame-{tick:03}.png';p.screen.image.save(path);record['images'][str(tick)]=sha(path)
   assert any(e['routine']=='OverworldLoop' for e in record['events'])
   assert all((f['x'],f['y'])==(record['initial']['x'],record['initial']['y']) for f in record['frames'])
  finally:p.stop(save=False)
  (out/f'{preamble}-{repeat}.json.gz').write_bytes(gzip.compress(json.dumps(record,separators=(',',':')).encode(),mtime=0));records.append(record)
 assert records[0]==records[1],preamble
 summary.append({'preamble':preamble,'first_close':next(e['tick'] for e in record['events'] if e['routine']=='CloseTextDisplay'),'first_map':next(e['tick'] for e in record['events'] if e['routine']=='OverworldLoop')})
 print(json.dumps(summary[-1]),flush=True)
(out/'result.json').write_text(json.dumps({'status':'passed','cases':summary,'rom_sha256':sha(rom),'fixture_sha256':sha(fixture)},indent=2))

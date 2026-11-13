import argparse, hashlib, io, json, gzip
from pathlib import Path
from pyboy import PyBoy
a=argparse.ArgumentParser()
a.add_argument('--rom',type=Path,required=True)
a.add_argument('--symbols',type=Path,required=True)
a.add_argument('--state',type=Path,default=Path(__file__).resolve().parent.parent/'start-menu-close/fast-cancel/reference.state')
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
for case in ['cancel','short','no-held','save-held']:
 records=[]
 for repeat in [1,2]:
  folder=out/f'{case}-{repeat}';folder.mkdir()
  p=PyBoy(io.BytesIO(rom.read_bytes()),window='null',sound_emulated=True);p.set_emulation_speed(0)
  p.load_state(io.BytesIO(fixture.read_bytes()))
  for key in ['a','b','start','select','up','down','left','right']:p.button_release(key)
  def press(key,n=2,wait=24):
   p.button_press(key);p.tick(n,render=True,sound=True);p.button_release(key);p.tick(wait,render=True,sound=True)
  p.tick(8,render=True,sound=True);press('b',2,160)
  get=lambda n:int(p.memory[symbols[n][1]])
  for _ in range(7):
   if get('wCurrentMenuItem')==4:break
   press('down')
  assert get('wCurrentMenuItem')==4
  press('a',2,240)
  if case=='no-held':press('down')
  initial={'map':get('wCurMap'),'x':get('wXCoord'),'y':get('wYCoord'),'cursor':get('wCurrentMenuItem')}
  assert initial['cursor']==int(case=='no-held'),initial
  p.screen.image.save(folder/'initial.png')
  now=[-1];events=[];frames=[];held=None;images={'initial':sha(folder/'initial.png')}
  def snap():return {'cycles':p._cycles(),'x':get('wXCoord'),'y':get('wYCoord'),'held':get('hJoyHeld'),'pressed':get('hJoyPressed'),'font_loaded':get('wFontLoaded')}
  def hook(name):events.append({'tick':now[0],'routine':name,**snap()})
  names=['SaveGameData','HoldTextDisplayOpen','LoadScreenTilesFromBuffer2','CloseTextDisplay','UpdateSprites','OverworldLoop','Joypad']
  for name in names:p.hook_register(*symbols[name],hook,name)
  count=760 if case=='save-held' else 240
  try:
   for tick in range(count):
    now[0]=tick
    key=('b' if tick<2 else 'right' if case=='short' and tick in [3,4] else None) if case in ['cancel','short'] else ('a' if tick<(600 if case=='save-held' else 80) else None)
    if key!=held:
     if held:p.button_release(held)
     if key:p.button_press(key)
     held=key
    p.tick(1,render=True,sound=True)
    frames.append({'tick':tick,'input':key,**snap(),'rgba_sha256':hashlib.sha256(bytes(p.screen.raw_buffer)).hexdigest()})
    if args.record_all_frames or tick in [0,14,15,38,39,40,79,80,159,239,599,600,639,759]:
     file=folder/f'frame-{tick:03}.png';p.screen.image.save(file);images[str(tick)]=sha(file)
   returns=[x for x in events if x['routine']=='OverworldLoop'];writes=[x for x in events if x['routine']=='SaveGameData']
   assert returns,(case,events[-20:])
   if case in ['no-held','save-held']:assert returns[0]['tick']>=(600 if case=='save-held' else 80)
   assert len(writes)==int(case=='save-held'),(case,writes)
   assert all((f['x'],f['y'])==(initial['x'],initial['y']) for f in frames)
   record={'case':case,'fixture_sha256':sha(fixture),'rom_sha256':sha(rom),'initial':initial,'frames':frames,'events':events,'images':images,'scope':'Prepared original party state; actual B to START, actual SAVE selection and B/NO/YES input. No recording-time RAM or ROM writes, audio active.'}
   (folder/'record.json.gz').write_bytes(gzip.compress((json.dumps(record,separators=(',',':'))+'\n').encode(),mtime=0));records.append(record)
  finally:p.stop(save=False)
 assert records[0]==records[1],case
 row={'case':case,'repeats':2,'initial':initial,'first_overworld':returns[0]['tick'],'save_calls':len(writes),'first_hold':next(x['tick'] for x in events if x['routine']=='HoldTextDisplayOpen'),'first_close':next(x['tick'] for x in events if x['routine']=='CloseTextDisplay')};summary.append(row);print(json.dumps(row),flush=True)
(out/'result.json').write_text(json.dumps({'status':'passed','cases':summary,'source_sha256':sha(Path(__file__))},indent=2)+'\n')

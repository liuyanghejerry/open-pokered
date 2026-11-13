import argparse,hashlib,io,json,gzip
from pathlib import Path
from pyboy import PyBoy
a=argparse.ArgumentParser();a.add_argument('--rom',type=Path,required=True);a.add_argument('--symbols',type=Path,required=True);a.add_argument('--state',type=Path,default=Path(__file__).resolve().parent.parent/'start-menu-close/fast-cancel/reference.state');a.add_argument('--output',type=Path,required=True);args=a.parse_args()
out=args.output;out.mkdir(exist_ok=False)
rom=args.rom;symbols_path=args.symbols
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(rom)=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
assert sha(args.state)=='b12777818a990843546b623615caa67a05a811941dfcf4d078b46a564d6e09a4'
symbols={}
for line in symbols_path.read_text().splitlines():
 parts=line.split()
 if len(parts)==2 and ':' in parts[0]: symbols[parts[1]]=tuple(int(p,16) for p in parts[0].split(':'))
summary=[]
for case, menu_index in [('dex-neutral',0),('dex-short',0),('trainer-neutral',3),('trainer-short',3)]:
 records=[]
 for repeat in [1,2]:
  p=PyBoy(io.BytesIO(rom.read_bytes()),window='null',sound_emulated=True);p.set_emulation_speed(0)
  fixture=args.state;p.load_state(io.BytesIO(fixture.read_bytes()))
  for key in ['a','b','start','select','up','down','left','right']:p.button_release(key)
  p.tick(8,render=True,sound=True);p.button_press('b');p.tick(2,render=True,sound=True);p.button_release('b');p.tick(160,render=True,sound=True)
  get=lambda n:int(p.memory[symbols[n][1]])
  for _ in range(7):
   if get('wCurrentMenuItem')==menu_index:break
   p.button_press('down');p.tick(2,render=True,sound=True);p.button_release('down');p.tick(20,render=True,sound=True)
  assert get('wCurrentMenuItem')==menu_index,(case,get('wCurrentMenuItem'))
  p.button_press('a');p.tick(2,render=True,sound=True);p.button_release('a');p.tick(240,render=True,sound=True)
  folder=out/f'{case}-{repeat}';folder.mkdir();p.screen.image.save(folder/'initial.png')
  initial={'map':get('wCurMap'),'x':get('wXCoord'),'y':get('wYCoord'),'cursor':get('wCurrentMenuItem')}
  now=[-1];events=[];frames=[];held=None
  def snap():return {'cycles':p._cycles(),'x':get('wXCoord'),'y':get('wYCoord'),'held':get('hJoyHeld'),'pressed':get('hJoyPressed'),'font_loaded':get('wFontLoaded'),'bg_enabled':get('hAutoBGTransferEnabled'),'bg_portion':get('hAutoBGTransferPortion'),'lcdc':int(p.memory[0xff40]),'bgp':int(p.memory[0xff47]),'objp0':int(p.memory[0xff48]),'tile_animation':get('hTileAnimations'),'tile_map':bytes(p.memory[0x9800:0x9c00]).hex()}
  def hook(name):events.append({'tick':now[0],'routine':name,**snap()})
  for name in ['RedisplayStartMenu','HandleMenuInput','HoldTextDisplayOpen','LoadTextBoxTilePatterns','LoadFontTilePatterns','LoadScreenTilesFromBuffer2','CloseTextDisplay','UpdateSprites','OverworldLoop','Joypad','PlaySound','AutoBgMapTransfer','ReloadMapData','LoadTilesetTilePatternData','GBPalWhiteOutWithDelay3','GBPalWhiteOut','LoadGBPal','RunDefaultPaletteCommand']:
   assert name in symbols,name;p.hook_register(*symbols[name],hook,name)
  try:
   for tick in range(160):
    now[0]=tick;key='b' if tick<2 else 'down' if case.endswith('short') and tick in [2,3] else None
    if key!=held:
     if held:p.button_release(held)
     if key:p.button_press(key)
     held=key
    p.tick(1,render=True,sound=True)
    p.screen.image.save(folder/f'frame-{tick:03}.png')
    frames.append({'cursor':get('wCurrentMenuItem'),'tick':tick,'input':key,**snap(),'rgba_sha256':hashlib.sha256(bytes(p.screen.raw_buffer)).hexdigest()})
   assert any(x['routine']==('OverworldLoop' if case=='save' else 'RedisplayStartMenu') for x in events),(case,events[:12])
   assert (frames[-1]['x'],frames[-1]['y'])==(initial['x'],initial['y'])
   record={'case':case,'fixture_sha256':sha(fixture),'rom_sha256':sha(rom),'initial':initial,'frames':frames,'events':events,'scope':'Prepared original party state; real B returns to START, then actual Pokedex/Trainer selection/cancellation, short Down2..3 for short cases. No RAM/ROM writes after loading, audio active.'}
   (out/f'{case}-{repeat}.json.gz').write_bytes(gzip.compress((json.dumps(record,separators=(',',':'))+'\n').encode(),mtime=0));records.append(record)
  finally:p.stop(save=False)
 assert records[0]==records[1],case
 row={'case':case,'repeats':2,'initial':initial,'last':{k:frames[-1][k] for k in ['x','y']},'events':[{k:x[k] for k in ['tick','routine']} for x in events if x['routine'] in ['RedisplayStartMenu','HandleMenuInput','HoldTextDisplayOpen','LoadTextBoxTilePatterns','LoadFontTilePatterns','LoadScreenTilesFromBuffer2','CloseTextDisplay','OverworldLoop']][:12]};summary.append(row);print(json.dumps(row),flush=True)
(out/'result.json').write_text(json.dumps({'status':'passed','cases':summary,'source_sha256':sha(Path(__file__))},indent=2)+'\n')

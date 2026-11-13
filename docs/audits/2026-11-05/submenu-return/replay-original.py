import argparse,hashlib,io,json,gzip
from pathlib import Path
from pyboy import PyBoy
a=argparse.ArgumentParser();a.add_argument('--rom',type=Path,required=True);a.add_argument('--symbols',type=Path,required=True);a.add_argument('--output',type=Path,required=True);a.add_argument('--extra-idle',type=int,default=0);a.add_argument('--case',choices=['bag-neutral','bag-short','option-neutral','option-short']);a.add_argument('--fixture',type=Path,default=Path(__file__).resolve().parent.parent/'start-menu-close/fast-cancel/reference.state');args=a.parse_args()
assert args.extra_idle >= 0, 'extra idle must be nonnegative'
out=args.output;out.mkdir(exist_ok=False);rom=args.rom;symbols_path=args.symbols
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
play_ticks=lambda value:((value[0]*60+value[1])*60+value[2])*60+value[3]
assert sha(rom)=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
symbols={}
for line in symbols_path.read_text().splitlines():
 parts=line.split()
 if len(parts)==2 and ':' in parts[0]: symbols[parts[1]]=tuple(int(p,16) for p in parts[0].split(':'))
summary=[]
for case, menu_index in [('bag-neutral',2),('bag-short',2),('option-neutral',5),('option-short',5)]:
 if args.case is not None and case != args.case:continue
 records=[]
 for repeat in [1,2]:
  p=PyBoy(io.BytesIO(rom.read_bytes()),window='null',sound_emulated=True);p.set_emulation_speed(0)
  fixture=args.fixture;assert sha(fixture)=='b12777818a990843546b623615caa67a05a811941dfcf4d078b46a564d6e09a4';p.load_state(io.BytesIO(fixture.read_bytes()))
  for key in ['a','b','start','select','up','down','left','right']:p.button_release(key)
  p.tick(8,render=True,sound=True);p.button_press('b');p.tick(2,render=True,sound=True);p.button_release('b');p.tick(160,render=True,sound=True)
  get=lambda n:int(p.memory[symbols[n][1]])
  for _ in range(7):
   if get('wCurrentMenuItem')==menu_index:break
   p.button_press('down');p.tick(2,render=True,sound=True);p.button_release('down');p.tick(20,render=True,sound=True)
  assert get('wCurrentMenuItem')==menu_index,(case,get('wCurrentMenuItem'))
  p.button_press('a');p.tick(2,render=True,sound=True);p.button_release('a');p.tick(240+args.extra_idle,render=True,sound=True)
  folder=out/f'{case}-{repeat}';folder.mkdir();p.screen.image.save(folder/'initial.png')
  play_time=lambda:[get(name) for name in ['wPlayTimeHours','wPlayTimeMinutes','wPlayTimeSeconds','wPlayTimeFrames']]
  initial={'map':get('wCurMap'),'x':get('wXCoord'),'y':get('wYCoord'),'cursor':get('wCurrentMenuItem'),'play_time':play_time()}
  now=[-1];events=[];frames=[];held=None
  def snap():return {'cycles':p._cycles(),'x':get('wXCoord'),'y':get('wYCoord'),'held':get('hJoyHeld'),'pressed':get('hJoyPressed'),'font_loaded':get('wFontLoaded'),'bg_enabled':get('hAutoBGTransferEnabled'),'bg_portion':get('hAutoBGTransferPortion'),'tile_map':bytes(p.memory[0x9800:0x9c00]).hex(),'oam':bytes(p.memory[0xfe00:0xfea0]).hex(),'shadow_oam':bytes(p.memory[symbols['wShadowOAM'][1]:symbols['wShadowOAM'][1]+160]).hex(),'wram_tiles':bytes(p.memory[symbols['wTileMap'][1]:symbols['wTileMap'][1]+360]).hex(),'transfer_dest':int(p.memory[symbols['hAutoBGTransferDest'][1]])+256*int(p.memory[symbols['hAutoBGTransferDest'][1]+1]),'transfer_map':bytes(p.memory[0x9800:0xa000]).hex(),'lcdc':int(p.memory[0xff40]),'scroll':[int(p.memory[0xff42]),int(p.memory[0xff43])],'window':[int(p.memory[0xff4a]),int(p.memory[0xff4b])] }
  def hook(name):events.append({'tick':now[0],'routine':name,**snap()})
  for name in ['RedisplayStartMenu','HandleMenuInput','HoldTextDisplayOpen','LoadTextBoxTilePatterns','LoadFontTilePatterns','LoadScreenTilesFromBuffer2','CloseTextDisplay','UpdateSprites','OverworldLoop','Joypad','PlaySound','AutoBgMapTransfer']:
   assert name in symbols,name;p.hook_register(*symbols[name],hook,name)
  try:
   for tick in range(80):
    now[0]=tick;key='b' if tick<2 else 'down' if case.endswith('short') and tick in [2,3] else None
    if key!=held:
     if held:p.button_release(held)
     if key:p.button_press(key)
     held=key
    p.tick(1,render=True,sound=True)
    p.screen.image.save(folder/f'frame-{tick:03}.png')
    frames.append({'cursor':get('wCurrentMenuItem'),'tick':tick,'input':key,'play_time':play_time(),**snap(),'rgba_sha256':hashlib.sha256(bytes(p.screen.raw_buffer)).hexdigest()})
   assert any(x['routine']==('OverworldLoop' if case=='save' else 'RedisplayStartMenu') for x in events),(case,events[:12])
   assert (frames[-1]['x'],frames[-1]['y'])==(initial['x'],initial['y'])
   assert play_ticks(frames[-1]['play_time'])-play_ticks(initial['play_time'])==80
   final=frames[-1];base=final['transfer_dest']-0x9800;vram=bytes.fromhex(final['transfer_map']);logical=bytes.fromhex(final['wram_tiles']);assert 0 <= base <= 0x400,final['transfer_dest'];assert all(vram[base+y*32+x]==logical[y*20+x] for y in range(18) for x in range(20)), 'active transfer map must contain the final logical START buffer'
   record={'case':case,'extra_idle':args.extra_idle,'fixture_sha256':sha(fixture),'rom_sha256':sha(rom),'initial':initial,'frames':frames,'events':events,'scope':'Prepared original party state; real B returns to START, then actual Bag/Option selection/cancellation, short Down2..3 for short cases. No RAM/ROM writes after loading, audio active.'}
   (out/f'{case}-{repeat}.json.gz').write_bytes(gzip.compress((json.dumps(record,separators=(',',':'))+'\n').encode(),mtime=0));records.append(record)
  finally:p.stop(save=False)
 assert records[0]==records[1],case
 row={'case':case,'repeats':2,'initial':initial,'last':{k:frames[-1][k] for k in ['x','y']},'events':[{k:x[k] for k in ['tick','routine']} for x in events if x['routine'] in ['RedisplayStartMenu','HandleMenuInput','HoldTextDisplayOpen','LoadTextBoxTilePatterns','LoadFontTilePatterns','LoadScreenTilesFromBuffer2','CloseTextDisplay','OverworldLoop']][:12]};summary.append(row);print(json.dumps(row),flush=True)
(out/'result.json').write_text(json.dumps({'status':'passed','cases':summary,'source_sha256':sha(Path(__file__))},indent=2)+'\n')

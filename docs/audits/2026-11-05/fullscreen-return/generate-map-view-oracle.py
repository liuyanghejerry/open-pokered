import argparse,hashlib,io,json
from pathlib import Path
from pyboy import PyBoy
a=argparse.ArgumentParser();a.add_argument('--rom',type=Path,required=True);a.add_argument('--symbols',type=Path,required=True);a.add_argument('--state',type=Path,default=Path(__file__).resolve().parent.parent/'start-menu-close/fast-cancel/reference.state');a.add_argument('--output',type=Path,required=True);args=a.parse_args()
rom=args.rom;symbols_path=args.symbols;fixture=args.state
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
symbols={l.split()[1]:tuple(int(v,16) for v in l.split()[0].split(':')) for l in symbols_path.read_text().splitlines() if len(l.split())==2 and ':' in l.split()[0]}
records=[]
for repeat in range(2):
 p=PyBoy(io.BytesIO(rom.read_bytes()),window='null',sound_emulated=True);p.set_emulation_speed(0);p.load_state(io.BytesIO(fixture.read_bytes()))
 get=lambda n:int(p.memory[symbols[n][1]])
 for k in ['a','b','start','select','up','down','left','right']:p.button_release(k)
 p.tick(8);p.button_press('b');p.tick(2);p.button_release('b');p.tick(160)
 for _ in range(7):
  if get('wCurrentMenuItem')==0:break
  p.button_press('down');p.tick(2);p.button_release('down');p.tick(20)
 p.button_press('a');p.tick(2);p.button_release('a');p.tick(240)
 out={};data=rom.read_bytes()
 def entry(_):
  if out:return
  addr=symbols['wCurrentTileBlockMapViewPointer'][1];ptr=int(p.memory[addr])+(int(p.memory[addr+1])<<8)
  addr=symbols['wTilesetBlocksPtr'][1];bst=int(p.memory[addr])+(int(p.memory[addr+1])<<8)
  bank=get('wTilesetBank'); width=get('wCurMapWidth'); tiles=[0]*480
  for by in range(5):
   for bx in range(6):
    block=int(p.memory[ptr+by*(width+6)+bx]);start=bank*0x4000+bst-0x4000+block*16
    source=data[start:start+16]
    for y in range(4):tiles[(by*4+y)*24+bx*4:(by*4+y)*24+bx*4+4]=source[y*4:y*4+4]
  out.update(start=p._cycles(),width=width,bank=bank,saved_bank=get('hLoadedROMBank'),x=get('wXCoord'),y=get('wYCoord'),x_half=get('wXBlockCoord'),y_half=get('wYBlockCoord'),pointer=ptr,tiles=tiles,lcdc=int(p.memory[0xff40]))
 def done(_):
  if not out or 'cycles' in out:return
  out.update(cycles=p._cycles()-out['start'],visible=list(p.memory[0xc3a0:0xc3a0+360]),restored_bank=get('hLoadedROMBank'),surrounding=list(p.memory[0xc508:0xc508+480]))
 p.hook_register(*symbols['LoadCurrentMapView'],entry,None);p.hook_register(*symbols['LoadTilesetTilePatternData'],done,None)
 p.button_press('b');p.tick(2);p.button_release('b');p.tick(60)
 assert 'cycles' in out and out['lcdc']&0x80==0,out
 records.append(out);p.stop(save=False)
assert records[0]==records[1]
out={'rom_sha256':sha(rom),'fixture_sha256':sha(fixture),'source_sha256':sha(Path(__file__)),'case':records[0]}
args.output.write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({k:v for k,v in records[0].items() if not isinstance(v,list)}))

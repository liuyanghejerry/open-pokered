import argparse, hashlib, io, json, os, re
from pathlib import Path
os.environ['SDL_AUDIODRIVER']='dummy'
from pyboy import PyBoy
p=argparse.ArgumentParser()
p.add_argument('--rom',type=Path,required=True)
p.add_argument('--symbols',type=Path,required=True)
p.add_argument('--state',type=Path,default=Path(__file__).with_name('reference.state'))
p.add_argument('--output',type=Path,required=True)
a=p.parse_args()
assert hashlib.sha256(a.rom.read_bytes()).hexdigest()=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
assert hashlib.sha256(a.state.read_bytes()).hexdigest()=='4f0ba69a1cb103c6f2d1fb26164ce3c99883f21b5a69ab4dd57382776b8c78f7'
symbols={}
for line in a.symbols.read_text().splitlines():
 m=re.match(r'([0-9a-fA-F]+):([0-9a-fA-F]+) (\S+)',line)
 if m:symbols[m[3]]=(int(m[1],16),int(m[2],16))
VALUES={'stand': {'wSprite01StateData1MovementStatus': 2, 'wSprite01StateData1ImageIndex': 32, 'wSprite01StateData1YStepVector': 0, 'wSprite01StateData1YPixels': 60, 'wSprite01StateData1XStepVector': 0, 'wSprite01StateData1XPixels': 96, 'wSprite01StateData1IntraAnimFrameCounter': 0, 'wSprite01StateData1AnimFrameCounter': 0, 'wSprite01StateData1FacingDirection': 0, 'wSprite01StateData2WalkAnimationCounter': 0, 'wSprite01StateData2MovementDelay': 127, 'wSprite01StateData2MapY': 26, 'wSprite01StateData2MapX': 18, 'wSprite01StateData2GrassPriority': 0}, 'walk': {'wSprite02StateData1MovementStatus': 3, 'wSprite02StateData1ImageIndex': 45, 'wSprite02StateData1YStepVector': 0, 'wSprite02StateData1YPixels': 60, 'wSprite02StateData1XStepVector': 1, 'wSprite02StateData1XPixels': 116, 'wSprite02StateData1IntraAnimFrameCounter': 0, 'wSprite02StateData1AnimFrameCounter': 1, 'wSprite02StateData1FacingDirection': 12, 'wSprite02StateData2WalkAnimationCounter': 12, 'wSprite02StateData2MovementDelay': 0, 'wSprite02StateData2MapY': 26, 'wSprite02StateData2MapX': 20, 'wSprite02StateData2GrassPriority': 0}, 'player': {'wSprite02StateData1MovementStatus': 3, 'wSprite02StateData1ImageIndex': 45, 'wSprite02StateData1YStepVector': 0, 'wSprite02StateData1YPixels': 60, 'wSprite02StateData1XStepVector': 1, 'wSprite02StateData1XPixels': 116, 'wSprite02StateData1IntraAnimFrameCounter': 0, 'wSprite02StateData1AnimFrameCounter': 1, 'wSprite02StateData1FacingDirection': 12, 'wSprite02StateData2WalkAnimationCounter': 12, 'wSprite02StateData2MovementDelay': 0, 'wSprite02StateData2MapY': 26, 'wSprite02StateData2MapX': 20, 'wSprite02StateData2GrassPriority': 0}}
for case in ['stand','walk','player']:
 for repeat in [1,2]:
  out=a.output/f'{case}-{repeat}';out.mkdir(parents=True,exist_ok=False)
  game=PyBoy(str(a.rom),window='null',sound_emulated=True,color_palette=[0xffffff,0xaaaaaa,0x555555,0]);game.set_emulation_speed(0)
  game.load_state(io.BytesIO(a.state.read_bytes()))
  get=lambda n:int(game.memory[symbols[n][1]])
  assert (get('wCurMap'),get('wXCoord'),get('wYCoord'))==(12,12,22)
  for n,v in VALUES[case].items():game.memory[symbols[n][1]]=v
  game.tick(4)
  slot='01' if case=='stand' else '02'
  fields=list(VALUES[case])+['wCurMap','wXCoord','wYCoord','wWalkCounter','wFontLoaded','wGrassTile',f'wSprite{slot}StateData1PictureID']
  if case=='player':fields+=['wPlayerLastStopDirection','wPlayerMovingDirection','wCheckFor180DegreeTurn']
  rows=[]
  for t in range(-1,{'stand':20,'walk':80,'player':30}[case]):
   if case=='player' and t==0:game.button_press('down')
   if case=='player' and t==16:game.button_release('down')
   if t>=0:game.tick(1)
   game.screen.image.save(out/f'frame-{t+1:04}.png')
   rows.append({'t':t,**{n:get(n) for n in fields}})
  assert rows[0][f'wSprite{slot}StateData2GrassPriority']==128
  (out/'frames.json').write_text(json.dumps(rows,indent=2)+'\n')
  game.stop(save=False)
  print(case,repeat,len(rows),flush=True)

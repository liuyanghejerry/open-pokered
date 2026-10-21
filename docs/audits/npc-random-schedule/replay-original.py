"""Original normal power-on/CONTINUE with physical input and readonly call-site hooks.
Offline constructed SRAM fixture, not a fresh playthrough or native timing parity.
No runtime memory/register setters, snapshot restore, warp or forced RNG outputs.
Requires PyBoy 2.8.1 and the pinned original pokered ROM/symbols (see README).
"""
from pathlib import Path
import argparse,os,re,io,json,hashlib,gzip
os.environ['SDL_AUDIODRIVER']='dummy'
from pyboy import PyBoy
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rom',type=Path,required=True)
parser.add_argument('--sym',type=Path,required=True)
parser.add_argument('--fixture',type=Path,required=True)
parser.add_argument('--out',type=Path,required=True)
args=parser.parse_args();rom=args.rom.resolve();blob=rom.read_bytes()
assert hashlib.sha256(blob).hexdigest()=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
symbols={}
for line in args.sym.read_text().splitlines():
 m=re.match(r'([0-9a-fA-F]+):([0-9a-fA-F]+) (\S+)',line)
 if m:symbols[m[3]]=(int(m[1],16),int(m[2],16))
def segment(label,end):
 bank,a=symbols[label];assert bank==symbols[end][0];off=bank*16384+a-16384;return bank,a,blob[off:off+symbols[end][1]-a]
def random_site(label,end):
 bank,a,data=segment(label,end);target=symbols['Random'][1];needle=bytes([0xcd,target&255,target>>8]);assert data.count(needle)==1,(label,data.count(needle));return bank,a+data.index(needle)
ready=random_site('UpdateNPCSprite.randomMovement','ChangeFacingDirection')
finish=random_site('UpdateSpriteInWalkingAnimation.initNextMovementCounter','UpdateSpriteMovementDelay')
failed=random_site('CanWalkOntoTile.impassable','GetTileSpriteStandsOn')
bank,start,data=segment('TryWalking','UpdateSpriteInWalkingAnimation');target=symbols['CanWalkOntoTile'][1];after=(bank,start+data.index(bytes([0xcd,target&255,target>>8]))+3)
bank,stop=symbols['UpdateSpriteMovementDelay'];assert blob[bank*16384+stop-16384-1]==0xc9;walk_return=(bank,stop-1)
bank,init=symbols['UpdateSpriteInWalkingAnimation.initNextMovementCounter'];assert blob[bank*16384+init-16384-1]==0xc9;script_return=(bank,init-1)
scope='Normal original boot/CONTINUE and physical idle tape; readonly original call-site hooks. Offline constructed SRAM, not fresh gameplay or native timing parity.'
raw=args.fixture.read_bytes();trials=[]
for trial in [1,2]:
 p=PyBoy(str(rom),ram_file=io.BytesIO(raw),window='null',sound_emulated=False);p.set_emulation_speed(0);entered=[False];events=[];inputs=[];first=symbols['wSpriteStateData1'][1]+16;second=symbols['wSpriteStateData2'][1]+16
 get=lambda name:int(p.memory[symbols[name][1]])
 def state():return {'frame':p.frame_count,'status':int(p.memory[first+1]),'walk_counter':int(p.memory[second]),'delay':int(p.memory[second+8]),'map_yx':[int(p.memory[second+4]),int(p.memory[second+5])],'vectors_yx':[int(p.memory[first+3]),int(p.memory[first+5])],'random_add':get('hRandomAdd'),'player':[get('wCurMap'),get('wXCoord'),get('wYCoord')]}
 def hook(label):
  if label=='world':entered[0]=True;return
  if not entered[0]or get('hCurrentSpriteOffset')!=16:return
  s=state()
  if label=='image':
   if s['status']!=3 or s['walk_counter']!=16:return
   label='start_stride'
  events.append({'event':label,'state':s})
 sites={'world':symbols['OverworldLoop'],'can_walk_return':after,'walk_return':walk_return,'image':symbols['UpdateSpriteImage']}
 for name,address in [('ready_random',ready),('finish_random',finish),('failed_random',failed)]:sites[name]=address;sites[name+'_return']=(address[0],address[1]+3)
 for name,address in sites.items():p.hook_register(*address,hook,name)
 def wait(n,key=None,stop=False):
  frame=p.frame_count
  if key:p.button_press(key)
  for _ in range(n):
   p.tick(1,False)
   if stop and entered[0]:break
  if key:p.button_release(key)
  inputs.append({'frame':frame,'frames':p.frame_count-frame,'key':key})
 try:
  wait(1000);wait(8,'start',True);wait(90,stop=True)
  for _ in range(16):
   if entered[0]:break
   wait(8,'a',True)
   if entered[0]:break
   wait(90,stop=True)
  assert entered[0],state()
  initial=state();wait(4096);final=state();assert initial['player']==final['player']==[1,20,30]
  names=[e['event']for e in events];assert names.count('start_stride')>=1 and names.count('finish_random')>=1,(names,initial,final)
  cycles=[];active=None
  for e in events:
   if e['event']=='start_stride':active={'start':e['state']}
   elif e['event']=='finish_random'and active is not None:active['finish_call']=e['state']
   elif e['event']=='finish_random_return'and active is not None:active['draw']=e['state']['random_add']
   elif e['event']=='walk_return'and active is not None:
    active['return']=e['state'];assert active['start']['delay']==active['finish_call']['delay'];assert active['finish_call']['walk_counter']==0;assert e['state']['status']==2 and e['state']['delay']==(active['draw']&127);cycles.append(active);active=None
  assert cycles
  trials.append({'initial':initial,'final':final,'events':events,'inputs':inputs,'complete_successful_cycles':cycles})
 finally:p.stop(save=False)
assert trials[0]==trials[1]
result={'scope':scope,'source_ref':'fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c','rom_sha256':hashlib.sha256(blob).hexdigest(),'rom_unchanged':rom.read_bytes()==blob,'fixture_sha256':hashlib.sha256(raw).hexdigest(),'fixture_unchanged':args.fixture.read_bytes()==raw,'repeat_exact':True,'trial':trials[0]};args.out.write_bytes(__import__('gzip').compress(json.dumps(result,indent=2).encode(),mtime=0));print({'all_pass':True,'repeat_exact':True,'successful_cycles':len(trials[0]['complete_successful_cycles']),'events':len(events)},flush=True)

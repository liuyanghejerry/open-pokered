import os,io,re,json,hashlib,argparse
from pathlib import Path
os.environ['SDL_AUDIODRIVER']='dummy'
from pyboy import PyBoy
parser=argparse.ArgumentParser()
for name in ['rom','symbols','fixture','out']:parser.add_argument('--'+name,type=Path,required=True)
args=parser.parse_args();E=args.out;E.mkdir(parents=True,exist_ok=True)
rom=args.rom;fixture=args.fixture
symbols={}
for line in args.symbols.read_text().splitlines():
 m=re.match(r'([0-9a-f]+):([0-9a-f]+) (\S+)',line)
 if m:symbols[m[3]]=(int(m[1],16),int(m[2],16))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(rom)=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
frozen={str(p):sha(p) for p in [rom,fixture,Path(__file__)]}
trials=[]
for repeat in [1,2]:
 p=PyBoy(str(rom),ram_file=io.BytesIO(fixture.read_bytes()),window='null',sound_emulated=True,sound_volume=0);p.set_emulation_speed(0)
 held=None;inputs=[];field=[False];started=[False];active=[None];done=[];spins=[0]
 def read(name):return int(p.memory[symbols[name][1]])
 def state():return {'frame':p.frame_count,'tcycle':p._cycles(),'mute':read('wMuteAudioAndPauseMusic'),'pointers':list(p.memory[0xc006:0xc00e]),'delays':list(p.memory[0xc0b6:0xc0ba]),'sound_ids':list(p.memory[0xc026:0xc02e]),'nr51':int(p.memory[0xff25]),'coins':list(p.memory[symbols['wPlayerCoins'][1]:symbols['wPlayerCoins'][1]+2]),'payout':list(p.memory[0xcd4a:0xcd4c])}
 def field_hook(_):field[0]=True
 def slot_hook(_):started[0]=True
 def spin_hook(_):spins[0]+=1
 def begin(_):
  assert active[0]is None
  active[0]={'begin':state(),'frames':[]}
 def end(_):
  if active[0]is not None:
   active[0]['end']=state();done.append(active[0]);active[0]=None
 p.hook_register(*symbols['OverworldLoop'],field_hook,None)
 def tick(key=None):
  global held
  if key!=held:
   if held:p.button_release(held)
   if key:p.button_press(key)
   held=key
  p.tick(1,False);inputs.append(key)
  if active[0]is not None:active[0]['frames'].append(state())
 def wait(n):
  for _ in range(n):tick()
 def tap(key,n):
  for _ in range(16):tick(key)
  wait(n)
 try:
  for i in range(2400):
   if field[0] and (read('wCurMap'),read('wXCoord'),read('wYCoord'))==(135,18,16):break
   tick('start'if i%60<2 else'a'if i%60 in[20,21]else None)
  assert field[0] and (read('wCurMap'),read('wXCoord'),read('wYCoord'))==(135,18,16)
  p.hook_register(*symbols['MainSlotMachineLoop.loop'],slot_hook,None)
  p.hook_register(*symbols['SlotMachine_SpinWheels'],spin_hook,None)
  p.hook_register(13,0x7770,begin,None)
  p.hook_register(13,0x75f4,end,None)
  wait(64);tap('left',32);tap('up',32);tap('right',32)
  for n in range(1200):
   if started[0]:break
   tick('a'if n%24<2 else None)
  assert started[0],{'state':state(),'facing':int(p.memory[0xc109]),'map_xy':(read('wCurMap'),read('wXCoord'),read('wYCoord')),'ignore':read('wJoyIgnore'),'walk':read('wWalkCounter'),'front_tile':read('wTileInFrontOfPlayer'),'tileset_bytes':list(p.memory[0xd52b:0xd536]),'view':list(p.memory[0xd35e:0xd36a]),'standing':read('wTilePlayerStandingOn'),'bag':list(p.memory[0xd31d:0xd332]),'pc':p.register_file.PC}
  for index in range(15000):
   if done:break
   tick('a'if index%12<2 else None)
  assert done,(p.frame_count,spins[0],state())
  row=done[0];assert row['begin']['mute']==1 and row['end']['mute']==0
  assert len(row['frames'])>=8
  assert all(f['pointers']==row['begin']['pointers'] for f in row['frames'])
  assert all(f['delays']==row['begin']['delays'] for f in row['frames'])
  assert row['end']['pointers']==row['begin']['pointers'] and row['end']['delays']==row['begin']['delays']
  assert any(f['mute']==0x81 for f in row['frames'])
  assert any(any(f['sound_ids'][4:]) for f in row['frames']), 'reward SFX must remain active while music is paused'
  wait(60);post=state()
  assert post['mute']==0
  assert post['pointers']!=row['end']['pointers'], 'music must resume after the payout'
  trials.append({'payout':row,'post_pause_60_frames':post,'inputs':inputs,'spins_to_win':spins[0],'final_frame':p.frame_count})
  print({'repeat':repeat,'spins':spins[0],'pause_frames':len(row['frames']),'frame':p.frame_count},flush=True)
 finally:p.stop(save=False)
 assert frozen=={name:sha(Path(name))for name in frozen}
assert trials[0]==trials[1]
result={'scope':'Existing offline constructed SRAM; normal boot/CONTINUE, physical A inputs and read-only RAM/register hooks. No runtime writes/warp/state restore/font rendering/new ROM or environment. Not fresh travel or native Game proof.','source_ref':'pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c','rom_sha256':sha(rom),'fixture_sha256':sha(fixture),'repeat_exact':True,'sound_emulated':True,'trial':trials[0]}
(E/'original-runtime.json').write_text(json.dumps(result,separators=(',',':'))+'\n')
print('Original slot payout music pause PASS',flush=True)

import io,gzip,json,hashlib
from pathlib import Path
from pyboy import PyBoy
import argparse
args=argparse.ArgumentParser()
for n in ['rom','symbols','output']:args.add_argument('--'+n,type=Path,required=True)
args=args.parse_args()
out=args.output;out.mkdir(exist_ok=False)
rom=args.rom;fixture=Path(__file__).with_name('reference.state')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(rom)=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
symbols={}
for line in args.symbols.read_text().splitlines():
 q=line.split()
 if len(q)==2 and ':' in q[0]:symbols[q[1]]=tuple(int(x,16)for x in q[0].split(':'))
records=[]
for repeat in [1,2]:
 p=PyBoy(io.BytesIO(rom.read_bytes()),window='null',sound_emulated=True);p.set_emulation_speed(0);p.load_state(io.BytesIO(fixture.read_bytes()))
 for key in ['a','b','start','select','up','down','left','right']:p.button_release(key)
 p.tick(8,render=True,sound=True)
 for _ in range(2):p.button_press('b');p.tick(2,render=True,sound=True);p.button_release('b');p.tick(160,render=True,sound=True)
 folder=out/f'run-{repeat}';folder.mkdir();events=[];now=[-1]
 get=lambda n:int(p.memory[symbols[n][1]])
 def snap():return {'x':get('wXCoord'),'y':get('wYCoord'),'noise_id':int(p.memory[symbols['wChannelSoundIDs'][1]+7]),'cycle':p._cycles()}
 def hook(n):events.append({'routine':n,'tick':now[0],**snap()})
 for n in ['HandleMenuInput_.getJoypadState','CloseStartMenu','OverworldLoop']:
  p.hook_register(*symbols[n],hook,n)
 p.button_press('start');p.tick(2,render=True,sound=True);p.button_release('start')
 for tick in range(64):
  now[0]=tick
  if any(e['routine']=='HandleMenuInput_.getJoypadState' for e in events):break
  p.tick(1,render=True,sound=True)
 else:raise AssertionError('START never accepted input')
 p.screen.image.save(folder/'before-b.png');initial=snap();assert initial['noise_id']==143,initial
 frames=[];events.clear()
 for tick in range(80):
  now[0]=tick
  if tick==0:p.button_press('b')
  if tick==2:p.button_release('b')
  p.tick(1,render=True,sound=True)
  p.screen.image.save(folder/f'frame-{tick:03}.png')
  frames.append({'tick':tick,**snap(),'rgba_sha256':hashlib.sha256(bytes(p.screen.raw_buffer)).hexdigest()})
 assert any(e['routine']=='OverworldLoop'for e in events)
 assert (frames[-1]['x'],frames[-1]['y'])==(20,30)
 p.screen.image.save(folder/'after-b-80.png');p.stop(save=False)
 d={'rom_sha256':sha(rom),'fixture_sha256':sha(fixture),'initial':initial,'frames':frames,'events':events,'scope':'Original normal Red, actual buttons only after saved-state setup; B at first observed menu input poll while START CHAN8 noise remains active.'}
 (out/f'run-{repeat}.json.gz').write_bytes(gzip.compress(json.dumps(d).encode(),mtime=0));records.append(d)
 print(json.dumps({'repeat':repeat,'active_noise_at_b':initial['noise_id'],'first_field':next(e['tick']for e in events if e['routine']=='OverworldLoop')}),flush=True)
assert records[0]==records[1]

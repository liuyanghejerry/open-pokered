from pathlib import Path
import gzip,hashlib,io,json,tempfile
from pyboy import PyBoy
import argparse
a=argparse.ArgumentParser();a.add_argument('--rom',type=Path,required=True);a.add_argument('--state',type=Path,required=True);a.add_argument('--symbols',type=Path,required=True);a.add_argument('--output',type=Path,required=True);args=a.parse_args()
rom=args.rom;fixture=args.state;sym=args.symbols;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(rom)=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b';assert sha(fixture)=='b12777818a990843546b623615caa67a05a811941dfcf4d078b46a564d6e09a4'
symbols={}
for line in sym.read_text().splitlines():
 parts=line.split()
 if len(parts)==2 and ':'in parts[0]:symbols[parts[1]]=tuple(int(p,16)for p in parts[0].split(':'))
root=args.output;root.mkdir();summary=[]
for mode in ['start','save-no','save-yes']:
 pair=[]
 for repeat in [1,2]:
  p=PyBoy(io.BytesIO(rom.read_bytes()),window='null',sound_emulated=True);p.set_emulation_speed(0);p.load_state(io.BytesIO(fixture.read_bytes()));folder=root/f'{mode}-{repeat}';folder.mkdir();commands=[];events=[];frames=[];now=[-1]
  get=lambda n:int(p.memory[symbols[n][1]])
  def drive(keys,n):
   for k in ['a','b','select','start','up','down','left','right']:
    (p.button_press if k in keys else p.button_release)(k)
   commands.append({'keys':keys,'frames':n});p.tick(n,render=True,sound=True)
  def press(k,n=2,wait=24):drive([k],n);drive([],wait)
  try:
   drive([],8);press('b',2,160)
   if mode.startswith('save-'):
    for _ in range(8):
     if get('wCurrentMenuItem')==4:break
     press('down')
    assert get('wCurrentMenuItem')==4
    press('a',2,240)
    if mode=='save-no':press('down')
    assert get('wCurrentMenuItem')==int(mode=='save-no')
    drive(['a'],600 if mode=='save-yes' else 80)
   initial={'counter':get('hSoftReset'),'raw':get('hJoyInput'),'held':get('hJoyHeld')};assert initial['counter']==16
   p.screen.image.save(folder/'before-combo.png')
   def hook(name):events.append({'routine':name,'tick':now[0],'cycles':p._cycles(),'counter':get('hSoftReset')})
   for name in ['TrySoftReset','SoftReset']:p.hook_register(*symbols[name],hook,name)
   for t in range(64):
    now[0]=t;drive(['a','b','select','start'],1);frames.append({'tick':t,'cycles':p._cycles(),'counter':get('hSoftReset'),'raw':get('hJoyInput')});p.screen.image.save(folder/f'frame-{t:03}.png')
   assert any(x['routine']=='SoftReset'for x in events),'original must reset in START and both held-A SAVE branches'
   data={'mode':mode,'initial':initial,'events':events,'frames':frames,'commands':commands,'rom_sha256':sha(rom),'state_sha256':sha(fixture),'scope':'Original ordinary START or SAVE NO/YES A-held; actual combo for64frames, no state writes.'};pair.append(data);(folder/'record.json').write_text(json.dumps(data,indent=2))
  finally:p.stop(save=False)
 assert pair[0]==pair[1]
 summary.append({'mode':mode,'first_reset':next(x['tick']for x in pair[0]['events']if x['routine']=='SoftReset'),'repeats':2})
(root/'summary.json').write_text(json.dumps({'status':'passed','cases':summary},indent=2));print(root);print(json.dumps(summary))

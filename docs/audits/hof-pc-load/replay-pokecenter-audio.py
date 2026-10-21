"""Normal original boot/CONTINUE and physical PC-menu approach, readonly audio hooks.
No setters, snapshot restore, warp or font capture. Native binary is guarded, not run.
"""
from pathlib import Path
import io,os,json,re,hashlib,gzip,subprocess,argparse
os.environ['SDL_AUDIODRIVER']='dummy'
from pyboy import PyBoy
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rom',type=Path,required=True)
parser.add_argument('--sym',type=Path,required=True)
parser.add_argument('--native-binary',type=Path,required=True,help='Sole game binary, guarded only; not executed')
parser.add_argument('--out',type=Path,required=True)
args=parser.parse_args()
R=Path(__file__).resolve().parents[3];E=args.out.parent;E.mkdir(parents=True,exist_ok=True)
rom=args.rom;binary=args.native_binary
sy={}
for l in args.sym.read_text().splitlines():
 m=re.match(r'([0-9a-f]+):([0-9a-f]+) (\S+)',l)
 if m:sy[m[3]]=(int(m[1],16),int(m[2],16))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
paths=[rom,binary,*sorted((R/'crates/pokered-audio/src').rglob('*.rs')),Path(__file__)]
frozen={str(p):sha(p)for p in paths};head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()
assert sha(rom)=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
fixture=R/'docs/audits/hof-pc-cry/original-fixture.sav';raw=fixture.read_bytes()
c=json.loads(gzip.decompress((R/'docs/audits/hof-pc-cry/original-controls.json.gz').read_bytes()))
tape=[i for i in next(i for i in c['cases']if i['mode']=='short-a')['inputs']if i['frame']<2987]
assert tape[-1]['frame']+tape[-1]['frames']==2987
fields=['wChannelSoundIDs','wChannelFlags1','wChannelFlags2','wChannelDutyCycles','wChannelVibratoDelayCounters','wChannelVibratoExtents','wChannelVibratoRates','wChannelFrequencyLowBytes','wChannelVibratoDelayCounterReloadValues','wChannelNoteDelayCounters','wChannelLoopCounters','wChannelNoteSpeeds','wChannelNoteDelayCountersFractionalPart','wChannelOctaves','wChannelVolumes']
ret=sy['Audio1_ApplyMusicAffects'][1]-1
assert rom.read_bytes()[2*0x4000+ret-0x4000]==0xc9
trials=[]
for trial in range(2):
 p=PyBoy(str(rom),ram_file=io.BytesIO(raw),window='null',sound_emulated=False);p.set_emulation_speed(0)
 rows=[];pending=[None]
 def rd(n,size=8):return list(p.memory[sy[n][1]:sy[n][1]+size])
 def hook(which):
  if which=='enter':
   assert pending[0]is None
   if p.frame_count<2987:return
   if not (all(x==0 for x in rd('wChannelSoundIDs')[4:]) and rd('wMuteAudioAndPauseMusic',1)==[0]):return
   pointers=rd('wChannelCommandPointers',16)
   ptrs=[int.from_bytes(bytes(pointers[i*2:i*2+2]),'little')for i in range(8)]
   if not all(sy['Music_Pokecenter_Ch'+str(i+1)][1]<=ptrs[i]<0x8000 for i in range(3)):return
   pending[0]={'frame':p.frame_count,'enter':p._cycles(),'tempo':rd('wMusicTempo',2),'pointers':ptrs,'returns':rd('wChannelReturnAddresses',16),'state':{f:rd(f)for f in fields}}
  elif pending[0]is not None:
   row=pending[0];row['cycles']=p._cycles()+16-row.pop('enter');rows.append(row);pending[0]=None
 p.hook_register(*sy['Audio1_UpdateMusic'],hook,'enter');p.hook_register(2,ret,hook,'return')
 try:
  for i in tape:
   assert p.frame_count==i['frame']
   if i['key']:p.button_press(i['key'])
   p.tick(i['frames'],False)
   if i['key']:p.button_release(i['key'])
  p.tick(10000,False)
  trials.append(rows);print('trial',trial,'rows',len(rows),flush=True)
 finally:p.stop(save=False)
assert trials[0]==trials[1] and len(trials[0])>9500
assert {str(p):sha(p)for p in paths}==frozen
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==head
assert fixture.read_bytes()==raw
out={'scope':__doc__,'head':head,'frozen':frozen,'repeat_exact':True,'inputs':tape+[{'frame':2987,'frames':10000,'key':None}],'fields':fields,'rows':trials[0]}
args.out.write_bytes(gzip.compress(json.dumps(out,separators=(',',':')).encode(),mtime=0))
fields=[f for f in fields if f!='wChannelSoundIDs']
bases=[sy['Music_Pokecenter_Ch'+str(i+1)][1]for i in range(3)]
packed=bytearray(len(trials[0]).to_bytes(4,'little'))
for row in trials[0]:
 packed+=bytes(row['tempo']);packed+=row['cycles'].to_bytes(4,'little')
 for ch in range(3):
  packed+=(row['pointers'][ch]-bases[ch]).to_bytes(2,'little')
  ret=int.from_bytes(bytes(row['returns'][ch*2:ch*2+2]),'little')
  packed+=((ret-bases[ch])if ret else 0).to_bytes(2,'little')
  packed+=bytes(row['state'][f][ch]for f in fields)
assert packed==(R/'docs/audits/hof-pc-load/pokecenter-audio-cases.bin').read_bytes(),'original update inputs/cycles differ from published oracle'
print('All 10000 packed source inputs and cycle results match published fixture',flush=True)
print('PASS frozen normal-boot readonly original audio',flush=True)

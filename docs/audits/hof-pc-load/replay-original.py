"""Original 151-species HoF load coverage: offline SRAM fixture, normal boot/CONTINUE,
physical PC input and readonly hooks. No runtime state writes, snapshots, warp,
original-font screenshots or copied ROM. Not native/original timing parity.
"""
from pathlib import Path
import io,os,re,json,gzip,hashlib,subprocess,argparse
os.environ['SDL_AUDIODRIVER']='dummy'
from pyboy import PyBoy
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rom',type=Path,required=True)
parser.add_argument('--sym',type=Path,required=True)
parser.add_argument('--native-binary',type=Path,required=True,help='Sole binary, read only, for the recording freeze guard')
parser.add_argument('--phase-offset',type=int,choices=[0,1],default=0)
parser.add_argument('--out',type=Path,required=True)
args=parser.parse_args()
R=Path(__file__).resolve().parents[3];E=args.out.parent;E.mkdir(parents=True,exist_ok=True)
symbols={}
for line in args.sym.read_text().splitlines():
 m=re.match(r'([0-9a-fA-F]+):([0-9a-fA-F]+) (\S+)',line)
 if m:symbols[m[3]]=(int(m[1],16),int(m[2],16))
rom=args.rom; romhash=hashlib.sha256(rom.read_bytes()).hexdigest()
assert romhash=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()
binary=args.native_binary; binaryhash=hashlib.sha256(binary.read_bytes()).hexdigest()
guard_paths=[Path(__file__),R/'crates/pokered-core/src/front_pic_decode.rs',R/'crates/pokered-core/src/pc_screen.rs']
def source_hashes():return {str(p.relative_to(R)):hashlib.sha256(p.read_bytes()).hexdigest() for p in guard_paths}
frozen_sources=source_hashes()
base=(R/'docs/audits/hof-pc-cry/original-fixture.sav').read_bytes(); raw=bytearray(base)
cases=(R/'docs/audits/hof-pc-load/front-codec-cases.bin').read_bytes();ids=[];at=0
while at<len(cases):
 ids.append(cases[at]);length=int.from_bytes(cases[at+1:at+3],'little');at+=3+length+784+8
assert len(ids)==151 and len(set(ids))==151
hof=symbols['sHallOfFame'][1]-0xa000; raw[hof:hof+4800]=b'\xff'*4800
for i,species in enumerate(ids):
 off=hof+i*16;raw[off:off+16]=bytes([species,80])+bytes([0x80])*10+bytes([0x50,0,0,0])
main=0x2000+symbols['sMainData'][1]-0xa000
num=main+symbols['wNumHoFTeams'][1]-symbols['wMainDataStart'][1];raw[num]=26
checksum=0x2000+symbols['sMainDataCheckSum'][1]-0xa000;raw[checksum]=(~sum(raw[0x2598:checksum]))&255
raw=bytes(raw); fixture=E/f'original-151-phase-{args.phase_offset}-fixture.sav';fixture.write_bytes(raw)
controls=json.loads(gzip.decompress((R/'docs/audits/hof-pc-cry/original-controls.json.gz').read_bytes()))
case=next(c for c in controls['cases'] if c['mode']=='short-a')
trials=[]
assert rom.read_bytes()[0x40]==0xc3
assert int.from_bytes(rom.read_bytes()[0x41:0x43],"little")==symbols["VBlank"][1]
assert rom.read_bytes()[symbols["DelayFrame"][1]-1]==0xd9
assert rom.read_bytes()[symbols["_UncompressSpriteData"][1]-1]==0xc9
for trial in [1,2]:
 p=PyBoy(str(rom),ram_file=io.BytesIO(raw),window='null',sound_emulated=False);p.set_emulation_speed(0)
 events=[];frames=[];tape=[];active=[False];ack=[0];interrupts=[];irq_start=[None]
 def get(n):return int(p.memory[symbols[n][1]])
 def state():return {'tcycles':p._cycles(),'frame':p.frame_count,'mon':get('wHoFMonSpecies'),'bgp':int(p.memory[0xff47]),'dimensions':get('wMonHSpriteDim')}
 def hook(label):
  if label=='show':active[0]=True
  if not active[0]:return
  item={'event':label,**state()}
  if label=='align':
   start=symbols['sSpriteBuffer1'][1];size=symbols['sSpriteBuffer2'][1]-start
   item['uncompressed_planes_sha256']=hashlib.sha256(bytes(p.memory[start:start+size*2])).hexdigest()
  if label=='cry':
   start=symbols['vFrontPic'][1];item['front_2bpp']=bytes(p.memory[start:start+49*16]).hex()
  if label=='ack':ack[0]+=1
  events.append(item)
 symbols['_UNCOMPRESS_RETURN']=(0,symbols['_UncompressSpriteData'][1]-1)
 def irq(label):
  if not active[0]:return
  if label=='enter':
   assert irq_start[0] is None;irq_start[0]=p._cycles()
  else:
   assert irq_start[0] is not None
   interrupts.append({'entry':irq_start[0],'reti':p._cycles()});irq_start[0]=None
 p.hook_register(*symbols['VBlank'],irq,'enter')
 p.hook_register(0,symbols['DelayFrame'][1]-1,irq,'return')
 for label,symbol in [('show','LeaguePCShowMon'),('uncompress','UncompressSpriteData'),('uncompress_return','_UNCOMPRESS_RETURN'),('align','LoadUncompressedSpriteData'),('interlace','InterlaceMergeSpriteBuffers'),('vram','CopyVideoData'),('normal','GBPalNormal'),('cry','PlayCry'),('ack','WaitForTextScrollButtonPress')]:p.hook_register(*symbols[symbol],hook,label)
 def step(n,key=None,stop_on_viewer=False):
  frame=p.frame_count
  if key:p.button_press(key)
  done=0
  for _ in range(n):
   p.tick(1,False);done+=1
   if active[0]:frames.append(state())
   if stop_on_viewer and active[0]:break
  if key:p.button_release(key)
  tape.append({'frame':frame,'frames':done,'key':key})
 try:
  offset=0
  for c in case['inputs']:
   if c['frame']==3283 and args.phase_offset:
    step(args.phase_offset);offset=args.phase_offset
   assert p.frame_count==c['frame']+offset
   step(c['frames'],c['key'],True)
   if active[0]:break
  assert active[0]
  sent=0
  while p.frame_count<40000 and sent<151:
   if ack[0]>sent:
    assert ack[0]==sent+1
    step(4);step(8,'a');step(4);sent+=1
   else:step(1)
  assert sent==151,(sent,p.frame_count)
  cries=[x for x in events if x['event']=='cry']; assert [x['mon'] for x in cries]==ids
  starts=[i for i,x in enumerate(events) if x['event']=='show']; assert len(starts)==151
  records=[]
  for j,start in enumerate(starts):
   chunk=events[start:starts[j+1] if j+1<len(starts) else len(events)]
   by={x['event']:x for x in chunk}
   assert {'show','uncompress','align','interlace','vram','normal','cry','ack'}<=by.keys()
   assert by['normal']['bgp']==0 and by['cry']['bgp']==228
   white=[x['frame'] for x in frames if by['show']['frame']<x['frame']<=by['cry']['frame'] and x['bgp']==0]
   assert white
   records.append({'species':ids[j],'show_frame':by['show']['frame'],'dimensions':by['align']['dimensions'],'decompress_frames':by['align']['frame']-by['uncompress']['frame'],'load_to_cry_frames':by['cry']['frame']-by['show']['frame'],'white_frames':len(white),'uncompressed_planes_sha256':by['align']['uncompressed_planes_sha256'],'decompress_enter_tcycles':by['uncompress']['tcycles'],'decompress_return_tcycles':by['uncompress_return']['tcycles']})
  for record in records:
   start,end=record['decompress_enter_tcycles'],record['decompress_return_tcycles']+16
   inside=[q for q in interrupts if start<=q['entry']-36 and q['reti']+16<=end]
   record['vblank_interrupts_in_decompress']=len(inside)
   record['decompress_elapsed_tcycles']=end-start
   record['decompress_foreground_tcycles']=end-start-sum(q['reti']+16-q['entry']+36 for q in inside)
   assert record['decompress_foreground_tcycles']>0 and record['decompress_foreground_tcycles']%4==0
  trials.append({'events':events,'records':records,'inputs':tape,'interrupts':interrupts,'final_frame':p.frame_count})
  print({'trial':trial,'species':len(records),'final_frame':p.frame_count,'decompress_range':(min(x['decompress_frames'] for x in records),max(x['decompress_frames'] for x in records))},flush=True)
 finally:p.stop(save=False)
assert trials[0]==trials[1]
assert hashlib.sha256(rom.read_bytes()).hexdigest()==romhash
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==head
assert hashlib.sha256(binary.read_bytes()).hexdigest()==binaryhash
assert fixture.read_bytes()==raw
assert source_hashes()==frozen_sources
j={'scope':__doc__,'all_pass':True,'repeat_exact':True,'source_ref':'fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c','rom_sha256':romhash,'head':head,'native_binary_sha256':binaryhash,'rom_head_binary_fixture_frozen':True,'source_hashes':frozen_sources,'source_files_unchanged':True,'base_fixture_sha256':hashlib.sha256(base).hexdigest(),'fixture_sha256':hashlib.sha256(raw).hexdigest(),'offline_mutations':{'hof_window':[hof,4800],'species':ids,'teams':26,'level':80,'nickname':'10 original A glyph bytes plus terminator; not rendered/captured','num_teams_offset':num,'checksum_offset':checksum},'trial':trials[0]}
args.out.write_bytes(gzip.compress(json.dumps(j,indent=2).encode(),mtime=0))
print('151 original pictures, two normal boots, exact repeat PASS',flush=True)

expected=json.loads(gzip.decompress((R/f'docs/audits/hof-pc-load/original-phase-{args.phase_offset}.json.gz').read_bytes()))
assert j['trial']==expected['trial'],'portable original replay differs from published events/records/interrupts/inputs'
print('Portable replay matches all published original trial data exactly',flush=True)

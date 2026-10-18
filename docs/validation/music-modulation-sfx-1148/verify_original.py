from pathlib import Path
import json,hashlib
import argparse
parser=argparse.ArgumentParser(description='Verify paired music-effect gating using pinned original-ROM instructions')
parser.add_argument('--rom',type=Path,required=True)
parser.add_argument('--symbols',type=Path,default=Path(__file__).with_name('source_symbols.json'))
parser.add_argument('--output-file',type=Path,required=True)
a=parser.parse_args()
rom=a.rom.read_bytes();sym=json.loads(a.symbols.read_text())
assert hashlib.sha256(rom).hexdigest()=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b', 'Use the pinned original ROM'
def execute(ch,delay,paired):
 bank,pc=sym['Audio3_ApplyMusicAffects'];regs=[0,ch,0,0,0,0,0,0];mem={};sp=0xdff0;z=c=False;trace=[]
 def pair(i):return regs[i]*256+regs[i+1]
 def setpair(i,v):regs[i]=(v>>8)&255;regs[i+1]=v&255
 def read(a):return mem.get(a,rom[a if a<0x4000 else bank*0x4000+a-0x4000] if a<0x8000 else 0)
 def write(a,v):mem[a]=v&255
 def r(i):return read(pair(4))if i==6 else regs[i]
 def w(i,v):
  if i==6:write(pair(4),v)
  else:regs[i]=v&255
 def byte():
  nonlocal pc
  v=read(pc);pc+=1;return v
 def word():return byte()|byte()<<8
 write(sym['wChannelNoteDelayCounters'][1]+ch,delay);write(sym['wChannelVibratoDelayCounters'][1]+ch,5);write(sym['wChannelVibratoExtents'][1]+ch,0x22);write(sym['wChannelSoundIDs'][1]+ch+4,144 if paired else 0)
 while True:
  at=pc;op=byte();done=False
  if 0x40<=op<=0x7f and op!=0x76:
   dst=(op>>3)&7;src=op&7;w(dst,r(src));t=8 if 6 in[dst,src]else 4
  elif op in[0x06,0x0e,0x16,0x1e,0x26,0x2e,0x3e]:w((op>>3)&7,byte());t=8
  elif op in[0x01,0x11,0x21]:setpair((op>>4)*2,word());t=12
  elif op==0x09:v=pair(4)+pair(0);setpair(4,v);c=v>65535;t=8
  elif op==0xfe:rhs=byte();z=regs[7]==rhs;c=regs[7]<rhs;t=8
  elif op==0xa7:z=regs[7]==0;c=False;t=4
  elif op in[0x3d,0x35]:i=7 if op==0x3d else 6;v=(r(i)-1)&255;w(i,v);z=v==0;t=4 if i==7 else 12
  elif op==0xcb:q=byte();assert q>>6==1,(hex(at),hex(q));i=q&7;z=r(i)&(1<<((q>>3)&7))==0;t=12 if i==6 else 8
  elif op in[0x20,0x28,0x30,0x38]:off=byte();off=off if off<128 else off-256;take={0x20:not z,0x28:z,0x30:not c,0x38:c}[op];t=12 if take else 8;pc=pc+off if take else pc
  elif op==0xca:dest=word();take=z;t=16 if take else 12;pc=dest if take else pc
  elif op==0xc9:t=16;done=True
  else:raise AssertionError((hex(at),hex(op)))
  trace.append({'pc':at,'op':op,'T':t});assert len(trace)<100
  if done:break
 return {'channel':ch,'initial_note_delay':delay,'paired_sfx':paired,'initial_vibrato_delay':5,'note_delay':read(sym['wChannelNoteDelayCounters'][1]+ch),'vibrato_delay':read(sym['wChannelVibratoDelayCounters'][1]+ch),'T':sum(x['T']for x in trace),'trace':trace}
cases=[execute(ch,n,pair)for ch in range(4)for n in[2,13,255]for pair in[False,True]]
for x in cases:assert x['note_delay']==x['initial_note_delay']-1 and x['vibrato_delay']==(5 if x['paired_sfx']else 4)
out={'scope':'Restricted original-ROM ApplyMusicAffects instructions, constructed RAM controls; paired SFX gates music effects after decrementing note delay; no ordinary scene/APU waveform claim','rom_sha256':hashlib.sha256(rom).hexdigest(),'cases':cases};a.output_file.write_text(json.dumps(out,separators=(',',':'))+'\n');print('PASS',len(cases),'independent original controls')

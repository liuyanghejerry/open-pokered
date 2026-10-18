from pathlib import Path
import argparse,json,hashlib
p=argparse.ArgumentParser(description='Verify original rest register requests and next-note entry state')
p.add_argument('--rom',type=Path,required=True)
p.add_argument('--symbols',type=Path,default=Path(__file__).with_name('source_symbols.json'))
p.add_argument('--output',type=Path,required=True)
a=p.parse_args();a.output.parent.mkdir(parents=True,exist_ok=True)
rom=a.rom.read_bytes()
assert hashlib.sha256(rom).hexdigest()=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
s=json.loads(a.symbols.read_text())
def run(data,ptr,fraction,duty,channel=4, paired=True):
 regs=[0]*8 # B,C,D,E,H,L,(HL),A
 regs[1]=channel;mem={};writes=[];pc=s['Audio3_sound_ret'][1];entry=pc;bank=s['Audio3_sound_ret'][0];sp=0xdff0;cycles=0;trace=[]
 def pair(i):return regs[i]*256+regs[i+1]
 def setpair(i,v):regs[i]=(v>>8)&255;regs[i+1]=v&255
 def read(q):
  if q in mem:return mem[q]
  return rom[q if q<0x4000 else bank*0x4000+q-0x4000]if q<0x8000 else 0
 def write(q,v):
  mem[q]=v&255
  if 0xff10<=q<=0xff25:writes.append([q,v&255])
 def r(i):return read(pair(4))if i==6 else regs[i]
 def w(i,v):
  if i==6:write(pair(4),v)
  else:regs[i]=v&255
 def byte():
  nonlocal pc
  v=read(pc);pc+=1;return v
 def word():return byte()|byte()<<8
 pointer=s['wChannelCommandPointers'][1]+2*channel
 origin=0xd000;entry=0;pc=s['Audio3_PlayNextNote'][1]
 write(sp,0);write(sp+1,0)
 for i,v in enumerate(data):write(origin+i,v)
 write(pointer,(origin+ptr)&255);write(pointer+1,(origin+ptr)>>8)
 for name,value in {'wChannelFlags1':4 if channel==4 else 0x30,'wChannelFlags2':0,'wChannelNoteSpeeds':1 if channel==4 else 6,'wChannelNoteDelayCountersFractionalPart':fraction,'wChannelDutyCycles':duty,'wChannelSoundIDs':144}.items():write(s[name][1]+channel,value)
 write(s['wChannelSoundIDs'][1]+channel+4,144 if paired else 0);write(s['wChannelFrequencyLowBytes'][1]+channel,77);write(s['wChannelOctaves'][1]+channel,6);write(s['wChannelVolumes'][1]+channel,179);write(s['wMusicTempo'][1],1);write(s['wMusicTempo'][1]+1,0)
 write(s['wSfxTempo'][1],1);write(s['wSfxTempo'][1]+1,0)
 mem[0xff25]=255
 write(s['wChannelVibratoDelayCounterReloadValues'][1]+channel,7)
 write(s['wChannelVibratoDelayCounters'][1]+channel,2)
 zero=False;carry=False
 while not(trace and pc==entry):
  at=pc;op=byte()
  if 0x40<=op<=0x7f and op!=0x76:
   dst=(op>>3)&7;src=op&7;w(dst,r(src));t=8 if 6 in[dst,src]else 4
  elif op in[0x06,0x0e,0x16,0x1e,0x26,0x2e,0x3e]:w((op>>3)&7,byte());t=8
  elif op in[0x01,0x11,0x21]:setpair((op>>4)*2,word());t=12
  elif op==0xfa:regs[7]=read(word());t=16
  elif 0x80<=op<=0xbf:
   kind=(op>>3)&7;rhs=r(op&7);old=regs[7]
   if kind==0:value=old+rhs;carry=value>255
   elif kind==1:value=old+rhs+carry;carry=value>255
   elif kind==2:value=old-rhs;carry=value<0
   elif kind==3:value=old-rhs-carry;carry=value<0
   elif kind==4:value=old&rhs;carry=False
   elif kind==5:value=old^rhs;carry=False
   elif kind==6:value=old|rhs;carry=False
   else:value=old-rhs;carry=value<0
   zero=(value&255)==0
   if kind!=7:regs[7]=value&255
   t=8 if op&7==6 else 4
  elif op in[0xc6,0xce]:value=regs[7]+byte()+(carry if op==0xce else 0);regs[7]=value&255;carry=value>255;zero=regs[7]==0;t=8
  elif op==0x0f:old=regs[7];carry=bool(old&1);regs[7]=(old>>1)|((old&1)<<7);zero=False;t=4
  elif op==0x37:carry=True;t=4
  elif op==0x3f:carry=not carry;t=4
  elif op==0xf0:regs[7]=read(0xff00+byte());t=12
  elif op==0xe0:write(0xff00+byte(),regs[7]);t=12
  elif op==0xea:write(word(),regs[7]);t=16
  elif op==0xf6:regs[7]|=byte();zero=regs[7]==0;carry=False;t=8
  elif op==0xe6:regs[7]&=byte();zero=regs[7]==0;carry=False;t=8
  elif op==0xcb:
   q=byte();reg=q&7;bit=(q>>3)&7;group=q>>6;v=r(reg)
   if group==1:zero=v&(1<<bit)==0
   elif group==2:w(reg,v&~(1<<bit))
   elif group==3:w(reg,v|(1<<bit))
   else:
    if bit==7:carry=bool(v&1);v>>=1
    elif bit==4:carry=bool(v&128);v=(v<<1)&255
    elif bit==2:old=carry;carry=bool(v&128);v=((v<<1)|old)&255
    elif bit==5:carry=bool(v&1);v=(v>>1)|(v&128)
    elif bit==3:old=carry;carry=bool(v&1);v=(v>>1)|(128 if old else 0)
    elif bit==6:v=((v<<4)|(v>>4))&255;carry=False
    else:raise AssertionError(('CB',hex(q)))
    w(reg,v);zero=v==0
   t=(12 if group==1 else 16)if reg==6 else 8
  elif op in[0x09,0x19,0x29]:v=pair(4)+pair((op>>4)*2);setpair(4,v);carry=v>65535;t=8
  elif op==0x87:v=regs[7]*2;regs[7]=v&255;carry=v>255;zero=regs[7]==0;t=4
  elif op in[0x2a,0x3a]:regs[7]=read(pair(4));setpair(4,(pair(4)+(1 if op==0x2a else -1))&65535);t=8
  elif op==0x1a:regs[7]=read(pair(2));t=8
  elif op in[0x13,0x23]:i=2 if op==0x13 else 4;setpair(i,(pair(i)+1)&65535);t=8
  elif op==0x22:write(pair(4),regs[7]);setpair(4,(pair(4)+1)&65535);t=8
  elif op in[0xc5,0xd5,0xe5]:i=((op>>4)-12)*2;sp-=2;write(sp,regs[i+1]);write(sp+1,regs[i]);t=16
  elif op==0xf5:sp-=2;write(sp,0x80*zero+0x10*carry);write(sp+1,regs[7]);t=16
  elif op==0xf1:flags=read(sp);regs[7]=read(sp+1);zero=bool(flags&128);carry=bool(flags&16);sp+=2;t=12
  elif op in[0xc1,0xd1,0xe1]:setpair(((op>>4)-12)*2,read(sp)|(read(sp+1)<<8));sp+=2;t=12
  elif op==0xcd:target_pc=word();sp-=2;write(sp,pc&255);write(sp+1,pc>>8);pc=target_pc;t=24
  elif op==0xc9:pc=read(sp)|read(sp+1)<<8;sp+=2;t=16
  elif op==0xc3:pc=word();t=16
  elif op==0xc2:target_pc=word();t=16 if not zero else 12;pc=target_pc if not zero else pc
  elif op in[0x18,0x20,0x28,0x30,0x38]:off=byte();off=off if off<128 else off-256;take={0x18:True,0x20:not zero,0x28:zero,0x30:not carry,0x38:carry}[op];t=12 if take else 8;pc=pc+off if take else pc
  elif op in[0xfe,0xbb]:rhs=byte()if op==0xfe else regs[3];zero=regs[7]==rhs;carry=regs[7]<rhs;t=8 if op==0xfe else 4
  elif op==0xa7:zero=regs[7]==0;carry=False;t=4
  elif op&0xc7 in[0x04,0x05]:i=(op>>3)&7;v=(r(i)+(1 if op&7==4 else -1))&255;w(i,v);zero=v==0;t=12 if i==6 else 4
  else:raise AssertionError((hex(at),hex(op)))
  trace.append([at,op,t]);cycles+=t;assert len(trace)<500
 assert sp==0xdff2
 get=lambda name:read(s[name][1]+channel)
 return {'vibrato_delay':get('wChannelVibratoDelayCounters'),'write_intents':writes,'input':{'data':data,'ptr':ptr,'fraction':fraction,'duty':duty,'channel':channel},'T':cycles,'next_ptr':(read(pointer)|read(pointer+1)<<8)-origin,'delay':get('wChannelNoteDelayCounters'),'fraction':get('wChannelNoteDelayCountersFractionalPart'),'duty':get('wChannelDutyCycles'),'flags1':get('wChannelFlags1'),'sound_id':get('wChannelSoundIDs'),'frequency_low':get('wChannelFrequencyLowBytes'),'sfx_tempo':read(s['wSfxTempo'][1])*256+read(s['wSfxTempo'][1]+1),'registers':{hex(a):v for a,v in mem.items()if 0xff10<=a<=0xff25},'instructions':trace}
cases=[]
for channel in range(3):
 for paired in [False,True]:
  for fraction in [0,255]:
   c=run([0xc1],0,fraction,128,channel,paired);c['paired']=paired;c['entry_reload']=7;c['entry_vibrato_delay']=2;c['entry_flags1']=0x30;cases.append(c)
   assert c['vibrato_delay']==7 and c['flags1']==0
   expected=[] if paired else ([[0xff25,0xbb]]if channel==2 else [[0xff12 if channel==0 else 0xff17,8],[0xff14 if channel==0 else 0xff19,128]])
   assert c['write_intents']==expected,(channel,paired,c['write_intents'],expected)
a.output.write_text(json.dumps({'source':'fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c','ROM_SHA256':hashlib.sha256(rom).hexdigest(),'scope':'Original PlayNextNote music rest, artificial RAM, matching paired SFX active or not; paired rest holds, pulse writes envelope8/high128 only, wave masks NR51 0xff->0xbb; not physical APU waveform','cases':cases},separators=(',',':'),sort_keys=True)+'\n')
print('12 original rest controls passed')

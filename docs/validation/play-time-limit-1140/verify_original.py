from pathlib import Path
import json,hashlib,argparse
parser=argparse.ArgumentParser();parser.add_argument('--rom',type=Path,required=True);parser.add_argument('--symbols',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();rom=args.rom.read_bytes()
if args.symbols.suffix=='.json':s=json.loads(args.symbols.read_text())
else:
 s={}
 for line in args.symbols.read_text().splitlines():
  parts=line.split()
  if len(parts)<2 or ':'not in parts[0]or parts[0].startswith(';'):continue
  bank,addr=parts[0].split(':');s[parts[1]]=[int(bank,16),int(addr,16)]
names=['wPlayTimeHours','wPlayTimeMaxed','wPlayTimeMinutes','wPlayTimeSeconds','wPlayTimeFrames'];keys=['hours','maxed','minutes','seconds','frames']
def run(values):
 before=dict(zip(keys,values));mem={s[n][1]:v for n,v in zip(names,values)};mem.update({s['wStatusFlags6'][1]:1,s['wIgnoreInputCounter'][1]:189,0xdfed:0,0xdfee:0});pc=s['TrackPlayTime'][1];sp=0xdfed;a=0;zero=False;cycles=0;writes=[]
 def byte():
  nonlocal pc
  at=pc if pc<0x4000 else s['TrackPlayTime'][0]*0x4000+pc-0x4000;v=rom[at];pc+=1;return v
 def word():return byte()|byte()<<8
 while pc:
  at=pc;op=byte()
  if op==0xcd:target=word();sp-=2;mem[sp]=pc&255;mem[sp+1]=pc>>8;pc=target;t=24
  elif op in[0xc9,0xc0,0xc8]:
   take=True if op==0xc9 else not zero if op==0xc0 else zero;t=16 if op==0xc9 else 20 if take else 8
   if take:pc=mem[sp]|mem[sp+1]<<8;sp+=2
  elif op==0xfa:a=mem.get(word(),0);t=16
  elif op==0xea:q=word();mem[q]=a;writes.append([q,a,cycles+12]);t=16
  elif op==0xa7:zero=a==0;t=4
  elif op==0xcb:
   q=byte();assert q==0x47;zero=a&1==0;t=8
  elif op==0x20:
   off=byte();off=off if off<128 else off-256;t=12 if not zero else 8
   if not zero:pc+=off
  elif op==0x3d:a=(a-1)&255;zero=a==0;t=4
  elif op==0x3c:a=(a+1)&255;zero=a==0;t=4
  elif op==0xfe:zero=a==byte();t=8
  elif op==0xaf:a=0;zero=True;t=4
  elif op==0x3e:a=byte();t=8
  else:raise AssertionError((hex(at),hex(op)))
  cycles+=t;assert cycles<2000
 assert sp==0xdfef
 after={k:mem[s[n][1]]for k,n in zip(keys,names)}
 return {'before':before,'after':after,'T':cycles,'writes':writes}
cases=[run(v)for v in[(0,0,0,0,0),(254,0,59,59,58),(254,0,59,59,59),(255,255,0,0,0),(254,255,59,59,59),(0,0,0,0,59),(0,0,0,59,59),(0,0,59,59,59),(7,0,10,20,60),(7,0,10,60,59),(7,0,60,59,59),(255,0,59,59,59)]]
x={'ROM_SHA256':hashlib.sha256(rom).hexdigest(),'source':'engine/play_time.asm TrackPlayTime','scope':'Restricted ROM interpreter, game timer active, input-ignore countdown supplied189; compares time fields only. Invalid60+ counters and255unmaxedhour are constructed byte controls, not normal-game reachability.','cases':cases}
args.output.write_text(json.dumps(x,indent=2)+'\n');print(json.dumps(cases[2]));print('controls',len(cases))

#!/usr/bin/env python3
"""Controlled whole noise-music endings with real original prefix and RET."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('rom',type=Path);p.add_argument('symbols',type=Path);p.add_argument('output',type=Path);args=p.parse_args()
    spec=importlib.util.spec_from_file_location('oracle',Path(__file__).with_name('generate-timer-instructions.py'));o=importlib.util.module_from_spec(spec);spec.loader.exec_module(o)
    rom=args.rom.read_bytes();assert hashlib.sha256(rom).hexdigest()==o.ROM_SHA
    symbols={}
    for line in args.symbols.read_text().splitlines():
        bits=line.split()
        if len(bits)>1 and ':' in bits[0] and not bits[0].startswith(';'):
            bank,address=bits[0].split(':');symbols[bits[1]]=(int(bank,16),int(address,16))
    programs=[[0xff],[0xd4,0xff],[0xe6,0xff],[0xe8,0xff],[0xea,8,34,0xff],[0xed,0,124,0xff],[0xed,0,160,0xff],[0xec,3,0xff],[0xf0,0x77,0xff],[0xfd,6,0,0xff,0,0,0xff],[0xfe,2,8,0,0xff,0,0,0,0xff]]
    out=[];ch=3;base=0xd100
    for prefix in ('Audio1','Audio3'):
        bank,entry=symbols[prefix+'_PlayNextNote'];tail=symbols[prefix+'_sound_ret'][1]+19
        for sid in (0,19,134,144):
            for data in programs:
                state=dict(speed=6,volume=0,duty=0,octave=4,flags1=0x3c,vibrato_delay=0)
                stream=dict(channel=ch,ptr=0,return_ptr=0,loop_counter=0,state=state,vibrato_reload=4)
                i=dict(preparation=dict(stream=stream,flags2=0x80,fraction=64,tempo=124),bank=bank,paired_sfx_active=False,source_sfx_id=sid,frequency_modifier=0,previous_frequency_low=77)
                memory={base+n:v for n,v in enumerate(data)}
                if data[0] in (0xfd,0xfe):
                    at=1 if data[0]==0xfd else 2;target=base+(data[at]|data[at+1]<<8);memory[base+at]=target&255;memory[base+at+1]=target>>8
                def putword(name,value):
                    a=symbols[name][1]+2*ch;memory[a]=value&255;memory[a+1]=value>>8
                putword('wChannelCommandPointers',base);putword('wChannelReturnAddresses',base)
                fields={'speed':'wChannelNoteSpeeds','volume':'wChannelVolumes','duty':'wChannelDutyCycles','octave':'wChannelOctaves','flags1':'wChannelFlags1','vibrato_delay':'wChannelVibratoDelayCounters'}
                for key,name in fields.items():memory[symbols[name][1]+ch]=state[key]
                memory[symbols['wChannelVibratoDelayCounterReloadValues'][1]+ch]=4;memory[symbols['wChannelFlags2'][1]+ch]=0x80;memory[symbols['wChannelLoopCounters'][1]+ch]=1
                memory[symbols['wChannelSoundIDs'][1]+ch]=0xba;memory[symbols['wChannelSoundIDs'][1]+4]=sid
                memory[symbols['wMusicTempo'][1]]=0;memory[symbols['wMusicTempo'][1]+1]=124
                def run(stop=None):return o.run(rom,bank,dict(mute=0,sound_ids=[0]*8,effects=[None]*8),entry=entry,registers={'C':ch},memory=memory,stop_pc=stop,capture=True)
                before,r,m=run(tail)
                def byte(name):return m.get(symbols[name][1]+ch)
                def pointer(name):
                    a=symbols[name][1]+2*ch;return (m[a]|m[a+1]<<8)-base
                tempo= symbols['wMusicTempo'][1]
                written=any(rom[bank*0x4000+pc-0x4000]==0xea and int.from_bytes(rom[bank*0x4000+pc-0x4000+1:bank*0x4000+pc-0x4000+3],'little')==tempo for pc,_ in before)
                count=data[1] if data[0]==0xfe else 0;counter=byte('wChannelLoopCounters');remaining=(0 if counter==1 else count-counter+1) if count else 0
                source=dict(state={key:byte(name) for key,name in fields.items()},note=255,next_ptr=pointer('wChannelCommandPointers'),return_ptr=pointer('wChannelReturnAddresses'),loop_counter=remaining,tempo=(m[tempo]<<8|m[tempo+1]) if written else None,vibrato_reload=byte('wChannelVibratoDelayCounterReloadValues'),vibrato_extent=byte('wChannelVibratoExtents'),vibrato_rate=byte('wChannelVibratoRates'),wave_instrument=None,master_volume=m.get(0xff24))
                trace,_,final=run();assert trace[:len(before)]==before
                ending=dict(ended=final[symbols['wChannelSoundIDs'][1]+ch]==0,flags1=final[symbols['wChannelFlags1'][1]+ch],flags2=final[symbols['wChannelFlags2'][1]+ch],disable_output_write=None,clear_saved_volume=False,panning_write=None,command_ptr_write=None,wave_restarted=False,volume_restore=None)
                out.append(dict(input=i,data=data,result=dict(stream=source,ending=ending,cycles=sum(t for _,t in trace)),trace=trace))
    text='{\n  "scope":"Controlled whole noise-ending source calls; no fake note length, raw pointer defaults, Game or physical APU timing claim.",\n  "rom_sha256":'+json.dumps(o.ROM_SHA)+',\n  "cases":[\n'+',\n'.join('    '+json.dumps(c,separators=(',',':')) for c in out)+'\n  ]\n}\n';args.output.write_text(text);print(len(out),'whole music endings')

if __name__=='__main__':main()

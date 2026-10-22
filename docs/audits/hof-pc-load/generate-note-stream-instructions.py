#!/usr/bin/env python3
"""Controlled source command-stream instruction fixtures, no gameplay/rendering."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('rom',type=Path);p.add_argument('symbols',type=Path);p.add_argument('output',type=Path);p.add_argument('--through-length',action='store_true');p.add_argument('--whole-note',action='store_true');args=p.parse_args()
    if args.whole_note:args.through_length=True
    spec=importlib.util.spec_from_file_location('oracle',Path(__file__).with_name('generate-timer-instructions.py'));o=importlib.util.module_from_spec(spec);spec.loader.exec_module(o)
    rom=args.rom.read_bytes();assert hashlib.sha256(rom).hexdigest()==o.ROM_SHA
    symbols={}
    for line in args.symbols.read_text().splitlines():
        bits=line.split()
        if len(bits)>1 and ':' in bits[0] and not bits[0].startswith(';'):
            bank,address=bits[0].split(':');symbols[bits[1]]=(int(bank,16),int(address,16))
    root=Path(__file__).resolve().parents[3];directory=root/'crates/pokered-audio/src/source_clock'
    cases=[]
    for c in json.loads((directory/'audio_note_dispatch_work_golden.json').read_text())['cases']:
        s={k:c['initial'][k] for k in ('speed','volume','duty','octave','flags1')};s['vibrato_delay']=0
        cases.append(dict(label='linear',channel=c['channel'],data=c['commands'],state=s,vibrato_reload=c['initial']['vibrato_reload'],ptr=0,return_ptr=0,loop_counter=0))
    for c in json.loads((directory/'audio_stream_work_golden.json').read_text())['cases']:
        i=c['input'];cases.append(dict(label=i['label'],channel=i['channel'],data=i['data'],state=c['initial']['state'],vibrato_reload=c['initial']['vibrato_reload'],ptr=i.get('ptr',0),return_ptr=i.get('return_ptr',0),loop_counter=i.get('loop_counter',0)))
    state=dict(speed=1,volume=0,octave=4,duty=0,flags1=0,vibrato_delay=0)
    for ch in range(4):
        for count in (1,2,3,255):
            for remaining in (0,1,count):
                cases.append(dict(label='finite-loop',channel=ch,data=[0xfe,count,6,0,0xc0,0,0xc1],state=dict(state,flags1=4 if ch==3 else 0),vibrato_reload=0,ptr=0,return_ptr=0,loop_counter=remaining))
    if args.whole_note:
        for ch in range(3):
            for note in (0x00,0xa0,0xc0):
                for paired in (False,True):
                    cases.append(dict(label='paired-whole',channel=ch,data=[note],state=dict(state,flags1=1,speed=6,volume=0x40 if ch==2 else 0xb3,duty=0xc0,octave=6),vibrato_reload=4,ptr=0,return_ptr=0,loop_counter=0,paired=paired,source_sfx_id=144 if paired else 0,frequency_modifier=255))
        for ch in (1,2):
            for sound_id in (20,133,134):
                for note in (0x00,0xa0):
                    cases.append(dict(label='cry-whole',channel=ch,data=[note],state=dict(state,flags1=1,speed=6,volume=0x40 if ch==2 else 0xb3,duty=0xc0,octave=6),vibrato_reload=4,ptr=0,return_ptr=0,loop_counter=0,source_sfx_id=sound_id,frequency_modifier=255))
    output=[]
    for prefix in ('Audio1','Audio3'):
        bank,entry=symbols[prefix+'_PlayNextNote'];_,stop=symbols[prefix+'_note_length']
        for case in cases:
            ch=case['channel'];data=case['data'];base=0xd100;memory={};targets=set();visited=set()
            def scan(offset):
                while offset not in visited:
                    visited.add(offset);cmd=data[offset]
                    if cmd==0xfd:
                        targets.add(offset+1);target=data[offset+1]|data[offset+2]<<8;scan(target);offset+=3
                    elif cmd==0xfe:
                        targets.add(offset+2);target=data[offset+2]|data[offset+3]<<8;scan(target);offset+=4
                    elif cmd==0xff:return
                    elif cmd&0xf0==0xd0:offset+=1 if ch==3 else 2
                    elif cmd in (0xe8,) or 0xe0<=cmd<=0xe7:offset+=1
                    elif cmd in (0xea,0xed):offset+=3
                    elif cmd in (0xec,0xf0):offset+=2
                    else:return
            scan(case['ptr'])
            if case['state']['flags1']&2:scan(case['return_ptr'])
            for i,value in enumerate(data):memory[base+i]=value
            for offset in targets:
                target=base+(data[offset]|data[offset+1]<<8);memory[base+offset]=target&255;memory[base+offset+1]=target>>8
            def putword(name,index,value):
                a=symbols[name][1]+index*2;memory[a]=value&255;memory[a+1]=value>>8
            putword('wChannelCommandPointers',ch,base+case['ptr']);putword('wChannelReturnAddresses',ch,base+case['return_ptr'])
            fields={'speed':'wChannelNoteSpeeds','volume':'wChannelVolumes','duty':'wChannelDutyCycles','octave':'wChannelOctaves','flags1':'wChannelFlags1','vibrato_delay':'wChannelVibratoDelayCounters'}
            for key,name in fields.items():memory[symbols[name][1]+ch]=case['state'][key]
            memory[symbols['wChannelVibratoDelayCounterReloadValues'][1]+ch]=case['vibrato_reload']
            count=data[1] if data[0]==0xfe else 0
            initial_counter=1 if not case['loop_counter'] else count-case['loop_counter']+1
            memory[symbols['wChannelLoopCounters'][1]+ch]=initial_counter
            if args.whole_note:
                memory[symbols['wChannelFrequencyLowBytes'][1]+ch]=77
                memory[symbols['wFrequencyModifier'][1]]=case.get('frequency_modifier',0)
                memory[symbols['wChannelSoundIDs'][1]+4]=case.get('source_sfx_id',0)
                if case.get('paired',False):memory[symbols['wChannelSoundIDs'][1]+ch+4]=144
                memory[symbols['wMusicWaveInstrument'][1]]=0;memory[symbols['wStereoPanning'][1]]=255;memory[0xff25]=0
            if args.through_length:
                memory[symbols['wChannelFlags2'][1]+ch]=0
                memory[symbols['wChannelNoteDelayCountersFractionalPart'][1]+ch]=64
                memory[symbols['wMusicTempo'][1]]=0;memory[symbols['wMusicTempo'][1]+1]=124
            trace,r,m=o.run(rom,bank,dict(mute=0,sound_ids=[0]*8,effects=[None]*8),entry=entry,registers={'C':ch},memory=memory,stop_pc=stop,capture=True)
            def byte(name):return m.get(symbols[name][1]+ch)
            def pointer(name):
                a=symbols[name][1]+ch*2;return (m[a]|m[a+1]<<8)-base
            result_state={key:byte(name) for key,name in fields.items()}
            final_counter=byte('wChannelLoopCounters')
            loop_counter=(0 if final_counter==1 else count-final_counter+1) if count else case['loop_counter']
            tempo_address=symbols['wMusicTempo'][1]
            tempo_written=any(rom[bank*0x4000+pc-0x4000]==0xea and int.from_bytes(rom[bank*0x4000+pc-0x4000+1:bank*0x4000+pc-0x4000+3],'little')==tempo_address for pc,_ in trace)
            tempo=(m[tempo_address]<<8)|m[tempo_address+1] if tempo_written else None
            wave_write=symbols[prefix+'_note_type.channel3'][1]+3
            assert rom[bank*0x4000+wave_write-0x4000]==0x77
            wave_written=any(pc==wave_write for pc,_ in trace)
            result=dict(state=result_state,note=r['D'],next_ptr=pointer('wChannelCommandPointers'),return_ptr=pointer('wChannelReturnAddresses'),loop_counter=loop_counter,tempo=tempo,vibrato_reload=byte('wChannelVibratoDelayCounterReloadValues'),vibrato_extent=byte('wChannelVibratoExtents'),vibrato_rate=byte('wChannelVibratoRates'),wave_instrument=m.get(symbols['wMusicWaveInstrument'][1]) if wave_written else None,master_volume=m.get(0xff24))
            i={key:case[key] for key in ('channel','ptr','return_ptr','loop_counter','state','vibrato_reload')}
            if args.through_length:
                boundary='NoiseReturned' if ch==3 else 'NotePitch'
                end=symbols[prefix+'_sound_ret'][1]-1 if ch==3 else symbols[prefix+'_note_pitch'][1]
                whole,_,final=o.run(rom,bank,dict(mute=0,sound_ids=[0]*8,effects=[None]*8),entry=entry,registers={'C':ch},memory=memory,stop_pc=end,capture=True)
                assert whole[:len(trace)]==trace
                result=dict(stream=result,fraction=final[symbols['wChannelNoteDelayCountersFractionalPart'][1]+ch],delay=final[symbols['wChannelNoteDelayCounters'][1]+ch],boundary=boundary,cycles=sum(t for _,t in whole))
                i=dict(stream=i,flags2=0,fraction=64,tempo=124);trace=whole
            if args.whole_note:
                whole,r,final=o.run(rom,bank,dict(mute=0,sound_ids=[0]*8,effects=[None]*8),entry=entry,registers={'C':ch},memory=memory,capture=True)
                assert whole[:len(trace)]==trace
                wave_entry=symbols[prefix+'_ApplyWavePatternAndFrequency'][1]
                result=dict(preparation=result,saved_frequency_low=final[symbols['wChannelFrequencyLowBytes'][1]+ch],wave_return_frequency=(r['D']<<8|r['E']) if any(pc==wave_entry for pc,_ in whole) else None,cycles=sum(t for _,t in whole))
                i=dict(preparation=i,bank=bank,paired_sfx_active=case.get('paired',False),source_sfx_id=case.get('source_sfx_id',0),frequency_modifier=case.get('frequency_modifier',0),previous_frequency_low=77);trace=whole
            output.append(dict(bank=bank,label=case['label'],input=i,data=data,result=result,trace=trace))
    text='{\n  "scope":"Controlled scalar source command streams; no gameplay, APU timing or runtime hookup claim.",\n  "rom_sha256":'+json.dumps(o.ROM_SHA)+',\n  "cases":[\n'
    text+=',\n'.join('    '+json.dumps(c,separators=(',',':')) for c in output)+'\n  ]\n}\n';args.output.write_text(text);print(len(output),'command streams')

if __name__=='__main__':main()

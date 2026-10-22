#!/usr/bin/env python3
"""Controlled whole confirmation-tone source calls, independently crosschecked."""
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
    directory=Path(__file__).resolve().parents[3]/'crates/pokered-audio/src/source_clock';prior=json.loads((directory/'press_ab_work_golden.json').read_text())['cases'];out=[]
    for prefix in ('Audio1','Audio3'):
        suffix='1' if prefix=='Audio1' else '3'
        header_bank,table=symbols['SFX_Headers_'+suffix];_,header=symbols['SFX_Press_AB_'+suffix];_,voice=symbols['SFX_Press_AB_'+suffix+'_Ch5']
        assert (header-table)//3==144 and (header-table)%3==0
        header_offset=header_bank*0x4000+header-0x4000
        assert rom[header_offset]&15==4 and int.from_bytes(rom[header_offset+1:header_offset+3],'little')==voice
        voice_offset=header_bank*0x4000+voice-0x4000
        assert list(rom[voice_offset:voice_offset+19])==prior[0]['input']['data']
        bank,entry=symbols[prefix+'_PlayNextNote'];wave=symbols[prefix+'_ApplyWavePatternAndFrequency'][1]
        for c in prior:
            a=c['input'];data=a['data'];ch=4;base=0xd100
            i=dict(bank=bank,source_sfx_id=144,ptr=a['ptr'],fraction=a['fraction'],duty=a['duty'],flags1=4,flags2=0,note_speed=1,sfx_tempo=256,vibrato_reload=0)
            memory={base+n:value for n,value in enumerate(data)}
            for name,value in [('wChannelSoundIDs',144),('wChannelFlags1',4),('wChannelFlags2',0),('wChannelNoteSpeeds',1),('wChannelNoteDelayCountersFractionalPart',a['fraction']),('wChannelDutyCycles',a['duty']),('wChannelVibratoDelayCounterReloadValues',0),('wChannelNoteDelayCounters',0)]:memory[symbols[name][1]+ch]=value
            ptr=symbols['wChannelCommandPointers'][1]+ch*2;memory[ptr]=(base+a['ptr'])&255;memory[ptr+1]=(base+a['ptr'])>>8
            tempo=symbols['wSfxTempo'][1];memory[tempo]=1;memory[tempo+1]=0;memory[0xff25]=0
            def run(stop=None):return o.run(rom,bank,dict(mute=0,sound_ids=[0]*8,effects=[None]*8),entry=entry,registers={'C':ch},memory=memory,stop_pc=stop,capture=True)
            trace,r,m=run();assert sum(t for _,t in trace)==c['T']
            if bank==31:
                actual=[[pc,rom[bank*0x4000+pc-0x4000],t] for pc,t in trace]
                assert actual==c['instructions'],'independent interpreter differs from earlier recorded original trace'
            next_ptr=(m[ptr]|m[ptr+1]<<8)-base;assert next_ptr==c['next_ptr']
            assert m[symbols['wChannelNoteDelayCountersFractionalPart'][1]+ch]==c['fraction']
            assert m[symbols['wChannelDutyCycles'][1]+ch]==c['duty']
            assert m[symbols['wChannelFlags1'][1]+ch]==c['flags1'] and m[symbols['wChannelSoundIDs'][1]+ch]==c['sound_id']
            for address,value in c['registers'].items():assert m[int(address,16)]==value
            if a['ptr']==18:
                outcome={'End':dict(next_ptr=next_ptr,fraction=c['fraction'],duty=c['duty'],vibrato_delay=0,ending=dict(ended=True,flags1=0,flags2=0,disable_output_write=None,clear_saved_volume=False,panning_write=None,command_ptr_write=None,wave_restarted=False,volume_restore=None))}
            else:
                before,raw,_=run(wave);assert trace[:len(before)]==before
                outcome={'Note':dict(next_ptr=next_ptr,fraction=c['fraction'],delay=c['delay'],duty=c['duty'],envelope=m[0xff12],frequency=raw['D']<<8|raw['E'],wave_return_frequency=r['D']<<8|r['E'],vibrato_delay=0)}
            out.append(dict(input=i,data=data,result=outcome,cycles=c['T'],trace=trace))
    text='{\n  "scope":"Controlled finite PressAB calls; Audio3 traces independently match every earlier recorded PC/opcode/T. No Game/native tick/APU timestamp claim.",\n  "rom_sha256":'+json.dumps(o.ROM_SHA)+',\n  "cases":[\n'+',\n'.join('    '+json.dumps(c,separators=(',',':')) for c in out)+'\n  ]\n}\n';args.output.write_text(text);print(len(out),'PressAB calls; all prior Audio3 instruction and register records matched')

if __name__=='__main__':main()

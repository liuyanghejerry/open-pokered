#!/usr/bin/env python3
"""Controlled source note-pitch continuation fixtures, not gameplay recordings."""
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
    root=Path(__file__).resolve().parents[3];directory=root/'crates/pokered-audio/src/source_clock';cases=[]
    prior=json.loads((directory/'audio_note_dispatch_work_golden.json').read_text())['cases']
    for c in prior:
        if c['channel']>=3:continue
        state=c['result'];cases.append(dict(channel=c['channel'],note=state['note'],octave=state['octave'],flags1=state['flags1'],paired_sfx_active=False,source_sfx_id=0,frequency_modifier=0,previous_frequency_low=77))
    for channel in range(3):
        for note in (0x00,0x21,0xa0,0xbf,0xc0,0xcf):
            for flags in (0,1):
                for octave in (0,7):
                    for paired in (False,True):
                        cases.append(dict(channel=channel,note=note,octave=octave,flags1=flags,paired_sfx_active=paired,source_sfx_id=144 if paired else 0,frequency_modifier=255,previous_frequency_low=77))
    for channel in (1,2):
        for sound_id in (19,20,133,134,144,255):
            for note in (0x00,0xb0):
                for modifier in (0,255):
                    cases.append(dict(channel=channel,note=note,octave=6,flags1=1,paired_sfx_active=False,source_sfx_id=sound_id,frequency_modifier=modifier,previous_frequency_low=77))
    output=[]
    for prefix in ('Audio1','Audio3'):
        bank,entry=symbols[prefix+'_note_pitch'];_,pitches=symbols[prefix+'_Pitches']
        for original in cases:
            i=dict(bank=bank,**original);ch=i['channel'];note=i['note'];offset=bank*0x4000+pitches-0x4000+2*(note>>4)
            i['pitch_base']=int.from_bytes(rom[offset:offset+2],'little') if note<0xc0 else None
            memory={symbols['wChannelFlags1'][1]+ch:i['flags1'],symbols['wChannelOctaves'][1]+ch:i['octave'],symbols['wChannelFrequencyLowBytes'][1]+ch:i['previous_frequency_low'],symbols['wFrequencyModifier'][1]:i['frequency_modifier'],symbols['wChannelSoundIDs'][1]+4:i['source_sfx_id'],symbols['wChannelVolumes'][1]+ch:0x40 if ch==2 else 0xb3,symbols['wChannelDutyCycles'][1]+ch:0xc0,symbols['wChannelNoteDelayCounters'][1]+ch:8,symbols['wMusicWaveInstrument'][1]:0,symbols['wStereoPanning'][1]:255,0xff25:0}
            if i['paired_sfx_active']:
                if ch==0:assert i['source_sfx_id']!=0
                else:memory[symbols['wChannelSoundIDs'][1]+ch+4]=1
            elif ch==0:assert i['source_sfx_id']==0
            # The source continuation consumes the caller's pushed AF. Its
            # flags are immediately overwritten by AND $f0 before any branch.
            trace,r,m=o.run(rom,bank,dict(mute=0,sound_ids=[0]*8,effects=[None]*8),entry=entry,registers={'C':ch},memory=memory,capture=True,initial_stack=[note<<8])
            wave_entry=symbols[prefix+'_ApplyWavePatternAndFrequency'][1]
            wave_return=(r['D']<<8|r['E']) if any(pc==wave_entry for pc,_ in trace) else None
            result=dict(saved_frequency_low=m[symbols['wChannelFrequencyLowBytes'][1]+ch],wave_return_frequency=wave_return)
            output.append(dict(input=i,result=result,trace=trace))
    text='{\n  "scope":"Controlled original note-pitch CPU continuations; no gameplay or physical APU timing claim.",\n  "rom_sha256":'+json.dumps(o.ROM_SHA)+',\n  "cases":[\n'+',\n'.join('    '+json.dumps(c,separators=(',',':')) for c in output)+'\n  ]\n}\n'
    args.output.write_text(text);print(len(output),'note-pitch traces')

if __name__=='__main__':main()

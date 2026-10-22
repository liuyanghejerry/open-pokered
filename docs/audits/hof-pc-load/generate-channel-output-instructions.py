#!/usr/bin/env python3
"""Controlled original channel-setup instruction traces. No gameplay/rendering."""
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
    root=Path(__file__).resolve().parents[3];directory=root/'crates/pokered-audio/src/source_clock'
    prior=json.loads((directory/'audio_output_work_golden.json').read_text())['cases']
    prior+= [c for c in json.loads((directory/'audio_wave_cry_work_golden.json').read_text())['cases'] if c['name']=='Audio3_ApplyWavePatternAndFrequency']
    output=[]
    names={'GetRegisterPointer':'RegisterPointer','ApplyDutyCycleAndSoundLength':'DutyAndLength','EnableChannelOutput':'EnableChannel','ApplyWavePatternAndFrequency':'WaveFrequency'}
    for prefix in ('Audio1','Audio3'):
        for c in prior:
            func=c['name'].split('_',1)[1];bank,entry=symbols[prefix+'_'+func];channel=c['channel'];frequency=c.get('frequency',0x550)
            memory={symbols['wChannelSoundIDs'][1]+4:c.get('sound_id',0),symbols['wFrequencyModifier'][1]:c.get('modifier',0),symbols['wMusicWaveInstrument'][1]:0,symbols['wSfxWaveInstrument'][1]:0,symbols['wStereoPanning'][1]:255,0xff25:0}
            if channel<4 and c.get('paired',False):memory[symbols['wChannelSoundIDs'][1]+channel+4]=1
            trace,r,_=o.run(rom,bank,dict(mute=0,sound_ids=[0]*8,effects=[None]*8),entry=entry,registers={'C':channel,'B':3,'D':frequency>>8,'E':frequency&255},memory=memory,capture=True)
            assert sum(t for _,t in trace)==c['cycles'],c
            result=(r['D']<<8|r['E']) if func=='ApplyWavePatternAndFrequency' else None
            if result is not None:assert result==c['DE'],c
            i=dict(bank=bank,channel=channel,paired_sfx_active=c.get('paired',False),source_sfx_id=c.get('sound_id',0),frequency_modifier=c.get('modifier',0),frequency=frequency)
            output.append(dict(routine=names[func],input=i,frequency=result,trace=trace))
    text='{\n  "scope":"Controlled channel-setup CPU fixtures; no gameplay or physical APU write timing claim.",\n  "rom_sha256":'+json.dumps(o.ROM_SHA)+',\n  "cases":[\n'
    text+=',\n'.join('    '+json.dumps(c,separators=(',',':')) for c in output)+'\n  ]\n}\n';args.output.write_text(text);print(len(output),'channel output traces')

if __name__=='__main__':main()

#!/usr/bin/env python3
"""Controlled original-ROM arithmetic fixtures. No gameplay or rendering."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('rom',type=Path)
    parser.add_argument('symbols',type=Path)
    parser.add_argument('output',type=Path)
    args=parser.parse_args()
    spec=importlib.util.spec_from_file_location('timer_oracle',Path(__file__).with_name('generate-timer-instructions.py'))
    oracle=importlib.util.module_from_spec(spec);spec.loader.exec_module(oracle)
    rom=args.rom.read_bytes();assert hashlib.sha256(rom).hexdigest()==oracle.ROM_SHA
    symbols={}
    for line in args.symbols.read_text().splitlines():
        if line.strip() and not line.lstrip().startswith(';') and ':' in line.split()[0]:
            address,name=line.split()[:2];bank,offset=address.split(':');symbols[name]=(int(bank,16),int(offset,16))
    root=Path(__file__).resolve().parents[3]
    prior=json.loads((root/'crates/pokered-audio/src/source_clock/audio_math_work_golden.json').read_text())
    prior_length=json.loads((root/'crates/pokered-audio/src/source_clock/audio_note_length_work_golden.json').read_text())
    output={'scope':'Controlled scalar fixtures. Instruction traces interpreted from pinned original ROM; no gameplay or APU timing claim.',
            'rom_sha256':oracle.ROM_SHA,'multiply':[],'frequency':[],'length':[]}
    quiet={'mute':0,'sound_ids':[0]*8,'effects':[None]*8}
    def call(bank,entry,registers,memory=None,stop=None):
        return oracle.run(rom,bank,quiet,entry=entry,registers=registers,memory=memory,stop_pc=stop,capture=True)
    for prefix in ('Audio1','Audio3'):
        bank,multiply=symbols[prefix+'_MultiplyAdd']
        for c in prior['multiply']:
            trace,r,_=call(bank,multiply,{'A':c['a'],'D':c['de']>>8,'E':c['de']&255,'L':c['low']})
            result=r['H']<<8|r['L'];assert result==c['result'] and sum(t for _,t in trace)==c['cycles']
            output['multiply'].append(dict(bank=bank,factor=c['a'],value=c['de'],low=c['low'],result=result,trace=trace))
        _,frequency=symbols[prefix+'_CalculateFrequency'];_,pitches=symbols[prefix+'_Pitches']
        for note in range(12):
            offset=bank*0x4000+pitches-0x4000+2*note
            pitch=int.from_bytes(rom[offset:offset+2],'little')
            for octave in (*range(8),8,254,255):
                trace,r,_=call(bank,frequency,{'A':note,'B':octave})
                result=r['D']<<8|r['E']
                output['frequency'].append(dict(bank=bank,pitch=pitch,octave=octave,result=result,carry=bool(r['F']&16),trace=trace))
        _,length=symbols[prefix+'_note_length'];_,pitch_entry=symbols[prefix+'_note_pitch']
        for c in prior_length['cases']:
            memory={}
            for field,key in [('wChannelNoteSpeeds','speed'),('wChannelNoteDelayCountersFractionalPart','fraction'),('wChannelFlags1','flags1'),('wChannelFlags2','flags2')]:
                memory[symbols[field][1]+c['channel']]=c[key]
            tempo=symbols['wMusicTempo'][1];memory[tempo]=c['tempo']>>8;memory[tempo+1]=c['tempo']&255
            trace,r,m=call(bank,length,{'C':c['channel'],'D':c['note']},memory,pitch_entry)
            frac=m[symbols['wChannelNoteDelayCountersFractionalPart'][1]+c['channel']]
            delay=m[symbols['wChannelNoteDelayCounters'][1]+c['channel']]
            assert (frac,delay,sum(t for _,t in trace))==(c['after_fraction'],c['after_delay'],c['cycles'])
            i={key:c[key] for key in ('channel','note','speed','fraction','tempo','flags1','flags2')}
            output['length'].append(dict(bank=bank,input=i,result=[frac,delay],returned=c['channel']==3,trace=trace))
    text='{\n  "scope":'+json.dumps(output['scope'])+',\n  "rom_sha256":'+json.dumps(oracle.ROM_SHA)+',\n'
    for key in ('multiply','frequency','length'):
        text+='  '+json.dumps(key)+':[\n'+',\n'.join('    '+json.dumps(c,separators=(',',':')) for c in output[key])+'\n  ]'+('' if key=='length' else ',')+'\n'
    args.output.write_text(text+'}\n')
    print({key:len(output[key]) for key in ('multiply','frequency','length')})

if __name__=='__main__':main()

#!/usr/bin/env python3
"""Original pitch-slide initialization, continuation and note-onset accesses.

Controlled scalar source calls; no Game inputs or physical timing claim.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
from source_apu_access_oracle import run_with_accesses


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('rom', 'symbols', 'output'):
        p.add_argument(name, type=Path)
    p.add_argument('--engine-cases', type=Path)
    args = p.parse_args()
    spec = importlib.util.spec_from_file_location('oracle', Path(__file__).with_name('generate-timer-instructions.py'))
    o = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(o)
    rom = args.rom.read_bytes()
    assert hashlib.sha256(rom).hexdigest() == o.ROM_SHA
    syms = {}
    for line in args.symbols.read_text().splitlines():
        words = line.split()
        if len(words) > 1 and ':' in words[0] and not words[0].startswith(';'):
            bank, address = words[0].split(':')
            syms[words[1]] = int(bank, 16), int(address, 16)
    fields = {'length_modifier':'wChannelPitchSlideLengthModifiers',
              'step':'wChannelPitchSlideFrequencySteps',
              'step_fraction':'wChannelPitchSlideFrequencyStepsFractionalPart',
              'current_fraction':'wChannelPitchSlideCurrentFrequencyFractionalPart'}
    def put(memory, channel, name, value): memory[syms[name][1]+channel] = value
    def state(memory, channel):
        result = {key:memory[syms[name][1]+channel] for key,name in fields.items()}
        for key,stem in [('current','Current'),('target','Target')]:
            result[key] = memory[syms['wChannelPitchSlide'+stem+'FrequencyLowBytes'][1]+channel] | memory[syms['wChannelPitchSlide'+stem+'FrequencyHighBytes'][1]+channel] << 8
        return result
    def populate(memory, channel, value):
        for key,name in fields.items(): put(memory,channel,name,value[key])
        for key,stem in [('current','Current'),('target','Target')]:
            put(memory,channel,'wChannelPitchSlide'+stem+'FrequencyLowBytes',value[key]&255)
            put(memory,channel,'wChannelPitchSlide'+stem+'FrequencyHighBytes',value[key]>>8)
    output = {'scope':__doc__, 'rom_sha256':o.ROM_SHA, 'init':[], 'apply':[], 'onset':[]}
    for bank, prefix in [(2,'Audio1'),(31,'Audio3')]:
        def byte(address): return rom[bank*0x4000+address-0x4000]
        def frequency(note, octave):
            base = syms[prefix+'_Pitches'][1] + note*2
            pitch = byte(base) | byte(base+1)<<8
            _,regs,_ = o.run(rom,bank,dict(mute=0,sound_ids=[0]*8,effects=[None]*8),
                entry=syms[prefix+'_CalculateFrequency'][1], registers={'A':note,'B':octave}, capture=True, strict_memory=True)
            return pitch, regs['D']<<8|regs['E']
        for ch in range(3):
            for current,target,delay,modifier in [
                (0x100,0x150,11,1),(0x150,0x100,11,1),(0x1f0,0x220,11,1),
                (0x200,0x200,6,0),(0x700,0x710,1,2),(0x710,0x700,6,1),
                (frequency(0,4)[1],frequency(9,3)[1],6,0),
                (frequency(7,4)[1],frequency(9,3)[1],6,0),
            ]:
                memory = {}
                put(memory,ch,'wChannelFlags1',0x11)
                put(memory,ch,'wChannelNoteDelayCounters',delay)
                populate(memory,ch,dict(current=0,target=target,step=0,length_modifier=modifier,step_fraction=0,current_fraction=0))
                trace,regs,final,accesses = run_with_accesses(o,rom,bank,dict(mute=0,sound_ids=[0]*8,effects=[None]*8),
                    entry=syms[prefix+'_InitPitchSlideVars'][1],registers={'C':ch,'D':current>>8,'E':current&255},memory=memory,instruction_limit=100000)
                assert not accesses
                output['init'].append(dict(bank=bank,input=dict(channel=ch,current_frequency=current,target_frequency=target,
                    note_delay=delay,length_modifier=modifier,flags1=0x11),trace=trace,result=dict(state=state(final,ch),
                    flags1=final[syms['wChannelFlags1'][1]+ch],returned_frequency=regs['D']<<8|regs['E'])))
            for decreasing,current,target,step,fraction,remainder in [
                (False,0x100,0x120,8,0,0),(False,0x118,0x120,8,250,6),
                (False,0x118,0x120,8,0,0),(False,0x119,0x120,8,0,0),
                (False,0x1ff,0x220,1,255,1),(False,0x1ff,0x200,1,0,0),
                (True,0x120,0x100,8,0,0),(True,0x108,0x100,8,0,128),
                (True,0x108,0x100,8,0,0),(True,0x107,0x100,8,0,0),
                (True,0x200,0x1df,1,0,128),(True,0x200,0x1ff,1,0,0),
            ]:
                before=dict(current=current,target=target,step=step,current_fraction=fraction,step_fraction=remainder,length_modifier=6)
                flags=0x11 | (0x20 if decreasing else 0)
                memory = {};populate(memory,ch,before);put(memory,ch,'wChannelFlags1',flags)
                trace,regs,final,accesses = run_with_accesses(o,rom,bank,dict(mute=0,sound_ids=[0]*8,effects=[None]*8),
                    entry=syms[prefix+'_ApplyPitchSlide'][1],registers={'C':ch},memory=memory,instruction_limit=100000)
                output['apply'].append(dict(bank=bank,channel=ch,state=before,flags1=flags,trace=trace,
                    result=dict(state=state(final,ch),flags1=final[syms['wChannelFlags1'][1]+ch]),accesses=accesses))
            for note,octave,target_note,target_octave,delay,modifier in [
                    (0,4,9,3,6,0),(7,4,9,3,6,0),
                    (0,5,9,4,6,0),(7,5,9,4,6,0),
                    (0,5,9,4,7,0),(7,5,9,4,7,0),
                    (0,4,7,4,1,0),(7,4,0,4,6,0),
            ]:
                for perfect in (False,True):
                    for paired in (False,True):
                        pitch,current=frequency(note,octave);_,target=frequency(target_note,target_octave)
                        memory={0xff25:0}
                        populate(memory,ch,dict(current=0,target=target,step=0,length_modifier=modifier,step_fraction=0,current_fraction=0))
                        for name,value in [('wChannelFlags1',16|int(perfect)),('wChannelOctaves',octave),
                            ('wChannelFrequencyLowBytes',77),('wChannelVolumes',0x40 if ch==2 else 0x92),
                            ('wChannelDutyCycles',0x80),('wChannelNoteDelayCounters',delay)]:put(memory,ch,name,value)
                        for name,value in [('wFrequencyModifier',0),('wMusicWaveInstrument',0),('wStereoPanning',255)]:put(memory,0,name,value)
                        ids=[0]*8
                        if paired: ids[ch+4]=144
                        trace,regs,final,accesses=run_with_accesses(o,rom,bank,dict(mute=0,sound_ids=ids,effects=[None]*8),
                            entry=syms[prefix+'_note_pitch'][1],registers={'C':ch},memory=memory,
                            initial_stack=[note<<12],instruction_limit=100000)
                        output['onset'].append(dict(bank=bank,channel=ch,note=note,perfect=perfect,paired=paired,
                            octave=octave,target_note=target_note,target_octave=target_octave,note_delay=delay,length_modifier=modifier,
                            current=current,target=target,pitch_base=pitch,trace=trace,state=state(final,ch),
                            flags1=final[syms['wChannelFlags1'][1]+ch],saved_low=final[syms['wChannelFrequencyLowBytes'][1]+ch],
                            accesses=accesses,returned_frequency=regs['D']<<8|regs['E']))
    args.output.write_text(json.dumps(output,separators=(',',':'))+'\n')
    print({name:len(output[name]) for name in ['init','apply','onset']})
    if args.engine_cases:
        rows = [item for item in output['onset'] if item['bank']==2 and item['channel']==0 and not item['paired']]
        lines = ['// Controlled byte-stream note-onset observations, not Rust-generated expectations.',
                 '// Captured from pinned source ROM SHA-256: '+o.ROM_SHA,
                 '// Reproduce with open-pokered docs/audits/hof-pc-load/generate-pitch-slide.py --engine-cases.',
                 '// Each row was checked on both source music banks and all three tonal channels.',
                 '// Fields: note, octave, target command, delay, modifier, perfect, low, high,',
                 '//         slide current, slide target, step, fractional step, slide flags.',
                 '&[']
        for row in rows:
            keys = ('note','octave','target_note','target_octave','note_delay','length_modifier','perfect')
            for other in output['onset']:
                if not all(row[key]==other[key] for key in keys): continue
                assert other['state']==row['state']
                low_port = [0xff13,0xff18,0xff1d][other['channel']]
                writes = [(a[3]-low_port,a[4]) for a in other['accesses'] if a[2]=='Write' and a[3] in (low_port,low_port+1)]
                assert writes == ([] if other['paired'] else [(0,row['saved_low']),(1,row['returned_frequency']>>8)])
                assert other['saved_low'] == (77 if other['paired'] else row['saved_low'])
            state=row['state']
            vals=[row['note'],row['octave'],(row['target_octave']<<4)|row['target_note'],row['note_delay'],row['length_modifier'],int(row['perfect']),row['saved_low'],row['returned_frequency']>>8,state['current'],state['target'],state['step'],state['step_fraction'],row['flags1']]
            lines.append('    ['+', '.join(str(v) for v in vals)+'],')
        lines.append(']')
        args.engine_cases.write_text('\n'.join(lines)+'\n')
        print('engine onset rows:',len(rows))


if __name__=='__main__': main()

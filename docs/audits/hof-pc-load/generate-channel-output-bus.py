#!/usr/bin/env python3
"""Controlled original helper APU stores and their actual instruction indices.

Not a gameplay recording or proof of subinstruction physical bus timing.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('rom', 'symbols', 'output'): p.add_argument(name, type=Path)
    args = p.parse_args()
    spec = importlib.util.spec_from_file_location('oracle', Path(__file__).with_name('generate-timer-instructions.py'))
    o = importlib.util.module_from_spec(spec); spec.loader.exec_module(o)
    rom = args.rom.read_bytes(); assert hashlib.sha256(rom).hexdigest() == o.ROM_SHA
    symbols = {}
    for line in args.symbols.read_text().splitlines():
        bits = line.split()
        if len(bits) > 1 and ':' in bits[0] and not bits[0].startswith(';'):
            bank, address = bits[0].split(':'); symbols[bits[1]] = (int(bank, 16), int(address, 16))
    directory = Path(__file__).resolve().parents[3]/'crates/pokered-audio/src/source_clock'
    prior = json.loads((directory/'channel_output_instructions_golden.json').read_text())['cases']
    names = dict(RegisterPointer='GetRegisterPointer', DutyAndLength='ApplyDutyCycleAndSoundLength',
                 EnableChannel='EnableChannelOutput', WaveFrequency='ApplyWavePatternAndFrequency')
    cases = []
    for case in prior:
        i = case['input']; ch = i['channel']; bank = i['bank']; prefix = 'Audio1' if bank == 2 else 'Audio3'
        def byte(address): return rom[bank*0x4000+address-0x4000]
        bases = symbols[prefix+'_HWChannelBaseAddresses'][1]
        masks = symbols[prefix+'_HWChannelEnableMasks'][1]
        assert [byte(bases+n) for n in range(8)] == [16,21,26,31]*2
        assert [byte(masks+n) for n in range(8)] == [17,34,68,136]*2
        for nr51, panning, delay, duty, instrument in [(0,255,1,0,0),(165,90,63,192,4),(255,0,255,128,5)]:
            wave_pointer = symbols[prefix+'_WavePointers'][1]+instrument*2
            wave_address = byte(wave_pointer) | byte(wave_pointer+1) << 8
            wave = [byte(wave_address+n) for n in range(16)]
            memory = {0xff25:nr51}
            for name, value in [('wChannelNoteDelayCounters',delay),('wChannelDutyCycles',duty)]:
                memory[symbols[name][1]+ch] = value
            for name, value in [('wFrequencyModifier',i['frequency_modifier']),('wMusicWaveInstrument',instrument),
                                ('wSfxWaveInstrument',instrument),('wStereoPanning',panning)]:
                memory[symbols[name][1]] = value
            ids = [0]*8; ids[4] = i['source_sfx_id']
            if ch < 4 and i['paired_sfx_active']: ids[ch+4] = 1
            routine = case['routine']
            if routine == 'RegisterPointer': bus = 'RegisterPointer'
            elif routine == 'DutyAndLength': bus = {routine:dict(note_delay=delay,duty=None if ch%4==2 else duty)}
            elif routine == 'EnableChannel': bus = {routine:dict(nr51=nr51,panning=panning if ch==7 or (ch<4 and not i['paired_sfx_active']) else None)}
            else: bus = {routine:dict(wave=wave if ch%4==2 else None)}
            entry = symbols[prefix+'_'+names[routine]][1]; frequency = i['frequency']; writes = []
            trace, r, final = o.run(rom,bank,dict(mute=0,sound_ids=ids,effects=[None]*8),entry=entry,
                registers={'C':ch,'B':3,'D':frequency>>8,'E':frequency&255},memory=memory,capture=True,
                strict_memory=True,write_events=writes)
            assert trace == case['trace'], (i, routine, len(trace), len(case['trace']), next(((n,a,b) for n,(a,b) in enumerate(zip(trace,case['trace'])) if tuple(a)!=tuple(b)), None))
            result = r['D']<<8 | r['E'] if routine == 'WaveFrequency' else None
            assert result == case['frequency']
            for index, pc, address, value in writes:
                assert trace[index][0] == pc
            cases.append(dict(input=i,bus=bus,trace=trace,writes=writes,frequency=result))
    args.output.write_text(json.dumps(dict(scope=__doc__,rom_sha256=o.ROM_SHA,cases=cases),separators=(',',':'))+'\n')
    print(len(cases),'helper calls;',sum(len(c['writes']) for c in cases),'APU stores')


if __name__ == '__main__': main()

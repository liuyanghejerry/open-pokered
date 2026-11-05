#!/usr/bin/env python3
"""Original noise-instrument next-note calls and actual APU accesses.

Controlled scalar inputs, not a gameplay or physical-clock comparison.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path

from source_apu_access_oracle import run_with_accesses


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('rom', 'symbols', 'output'):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location('oracle', Path(__file__).with_name('generate-timer-instructions.py'))
    oracle = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(oracle)
    rom = args.rom.read_bytes()
    assert hashlib.sha256(rom).hexdigest() == oracle.ROM_SHA
    symbols = {}
    for line in args.symbols.read_text().splitlines():
        parts = line.split()
        if len(parts) > 1 and ':' in parts[0] and not parts[0].startswith(';'):
            bank, address = parts[0].split(':')
            symbols[parts[1]] = int(bank, 16), int(address, 16)
    cases = []
    for bank, suffix in [(2, '1'), (31, '3')]:
        prefix = 'Audio' + suffix
        def rom_byte(address):
            return rom[bank * 0x4000 + address - 0x4000]
        for instrument in range(1, 20):
            header_bank, header = symbols[f'SFX_Noise_Instrument{instrument:02}_{suffix}']
            assert header_bank == bank
            assert header - symbols['SFX_Headers_' + suffix][1] == 3 * instrument
            assert rom_byte(header) == 7
            voice = rom_byte(header + 1) | rom_byte(header + 2) << 8
            data = []
            while True:
                opcode = rom_byte(voice + len(data))
                if opcode == 255:
                    data.append(opcode)
                    break
                assert opcode & 0xf0 == 0x20
                offset = len(data)
                data.extend(rom_byte(voice + offset + i) for i in range(3))
            for ptr in range(0, len(data), 3):
                for pulse, panning, nr51, fraction in [(0, 0x0f, 0xa5, 0), (144, 0xa5, 0x5a, 173)]:
                    item = dict(bank=bank, source_sfx_id=pulse, ptr=ptr, fraction=fraction,
                                duty=0, flags1=4, flags2=0, note_speed=1, sfx_tempo=0x157,
                                vibrato_reload=0)
                    memory = {0xff25: nr51, symbols['wStereoPanning'][1]: panning,
                              symbols['wChannelSoundIDs'][1] + 4: pulse}
                    for name, value in [('wChannelSoundIDs', instrument), ('wChannelFlags1', 4),
                                        ('wChannelFlags2', 0), ('wChannelNoteSpeeds', 1),
                                        ('wChannelNoteDelayCountersFractionalPart', fraction),
                                        ('wChannelDutyCycles', 0), ('wChannelVibratoDelayCounterReloadValues', 0),
                                        ('wChannelNoteDelayCounters', 1)]:
                        memory[symbols[name][1] + 7] = value
                    pointer = symbols['wChannelCommandPointers'][1] + 14
                    memory[pointer], memory[pointer + 1] = (voice + ptr) & 255, (voice + ptr) >> 8
                    trace, registers, final, events = run_with_accesses(
                        oracle, rom, bank, dict(mute=0, sound_ids=[0] * 8, effects=[None] * 8),
                        entry=symbols[prefix + '_PlayNextNote'][1], registers={'C': 7}, memory=memory)
                    next_ptr = (final[pointer] | final[pointer + 1] << 8) - voice
                    if data[ptr] == 255:
                        assert not events
                        outcome = {'End': dict(next_ptr=next_ptr, fraction=fraction, duty=0, vibrato_delay=0,
                            ending=dict(ended=True, flags1=0, flags2=0, disable_output_write=None,
                            clear_saved_volume=False, panning_write=None, command_ptr_write=None,
                            wave_restarted=False, volume_restore=None))}
                        assert final[symbols['wChannelSoundIDs'][1] + 7] == 0
                    else:
                        outcome = {'Note': dict(next_ptr=next_ptr,
                            fraction=final[symbols['wChannelNoteDelayCountersFractionalPart'][1] + 7],
                            delay=final[symbols['wChannelNoteDelayCounters'][1] + 7], duty=0,
                            envelope=final[0xff21], frequency=final[0xff22],
                            wave_return_frequency=registers['D'] << 8 | registers['E'], vibrato_delay=0)}
                        assert sum(e[2] == 'Read' for e in events) == 2
                        assert sum(e[2] == 'Write' for e in events) == 6
                    cases.append(dict(instrument=instrument, panning=panning, common=dict(input=item, data=data,
                        nr51=nr51, trace=trace, result=outcome,
                        accesses=[dict(instruction=i, pc=pc, kind=kind.lower(), address=address,
                                       value=value, at_tcycle=t) for i,pc,kind,address,value,t in events],
                        final_apu={str(a):v for a,v in final.items() if 0xff10 <= a <= 0xff3f})))
    args.output.write_text(json.dumps(dict(scope=__doc__, rom_sha256=oracle.ROM_SHA, cases=cases), separators=(',', ':')) + '\n')
    print(len(cases), 'noise-instrument note/end cases')


if __name__ == '__main__':
    main()

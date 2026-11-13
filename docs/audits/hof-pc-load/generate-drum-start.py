#!/usr/bin/env python3
"""Strict original-ROM PlaySound noise starts and busy drops, both audio banks.

Controlled source calls, not Game boot, full audio-frame or physical timing proof.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path

from source_apu_access_oracle import run_with_accesses


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for key in ('rom', 'symbols', 'output'):
        p.add_argument(key, type=Path)
    args = p.parse_args()
    spec = importlib.util.spec_from_file_location('oracle', Path(__file__).with_name('generate-timer-instructions.py'))
    oracle = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(oracle)
    rom = args.rom.read_bytes()
    assert hashlib.sha256(rom).hexdigest() == oracle.ROM_SHA
    symbols = {}
    for line in args.symbols.read_text().splitlines():
        bits = line.split()
        if len(bits) > 1 and ':' in bits[0] and not bits[0].startswith(';'):
            bank, address = bits[0].split(':')
            symbols[bits[1]] = int(bank, 16), int(address, 16)
    cases = []
    for bank, suffix in [(2, '1'), (31, '3')]:
        def byte(address): return rom[bank * 0x4000 + address - 0x4000]
        for instrument in range(1, 20):
            header_bank, header = symbols[f'SFX_Noise_Instrument{instrument:02}_{suffix}']
            assert header_bank == bank and byte(header) == 7
            assert header - symbols['SFX_Headers_' + suffix][1] == instrument * 3
            voice = byte(header + 1) | byte(header + 2) << 8
            for busy in (False, True):
                # Only the existing CHAN8 ID may be read before the source
                # initializes other state. Absent memory is an oracle error.
                memory = {symbols['wChannelSoundIDs'][1] + 7: 19 if busy else 0}
                trace, regs, final, accesses = run_with_accesses(
                    oracle, rom, bank, dict(mute=0, sound_ids=[0]*8, effects=[None]*8),
                    entry=symbols['Audio'+suffix+'_PlaySound'][1], registers={'A': instrument},
                    memory=memory, instruction_limit=20000)
                assert not accesses
                assert final[symbols['wSoundID'][1]] == instrument
                if busy:
                    assert final[symbols['wChannelSoundIDs'][1] + 7] == 19
                    assert symbols['wChannelFlags1'][1] + 7 not in final
                else:
                    for name, expected in [('wChannelSoundIDs', instrument), ('wChannelFlags1', 4),
                                           ('wChannelFlags2', 0), ('wChannelDutyCycles', 0),
                                           ('wChannelNoteSpeeds', 1), ('wChannelLoopCounters', 1),
                                           ('wChannelNoteDelayCounters', 1), ('wChannelFrequencyLowBytes', 0)]:
                        assert final[symbols[name][1] + 7] == expected
                    ptr = symbols['wChannelCommandPointers'][1] + 14
                    assert final[ptr] | final[ptr+1] << 8 == voice
                    ret = symbols['wChannelReturnAddresses'][1] + 14
                    assert final[ret] == final[ret+1] == 0
                cases.append(dict(bank=bank, instrument=instrument, busy=busy, trace=trace,
                                  result=dict(instrument=instrument, started=not busy)))
    args.output.write_text(json.dumps(dict(scope=__doc__, rom_sha256=oracle.ROM_SHA, cases=cases), separators=(',', ':'))+'\n')
    print(len(cases), 'original drum start/drop cases')


if __name__ == '__main__':
    main()

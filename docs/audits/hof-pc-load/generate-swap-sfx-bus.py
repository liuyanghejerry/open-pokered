#!/usr/bin/env python3
"""Original party SWAP pulse calls and actual APU accesses in both audio banks.

Controlled scalar inputs, including CHAN6 surviving a CHAN5 PressAB override.
This oracle runs original ROM instructions and does not import the Rust engine.
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
    spec = importlib.util.spec_from_file_location(
        'oracle', Path(__file__).with_name('generate-timer-instructions.py'))
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
        header = symbols['SFX_Swap_' + suffix][1]
        assert header - symbols['SFX_Headers_' + suffix][1] == 3 * 174
        for channel, pointers, length in [(4, [0, 6], 7), (5, [0, 6, 10], 11)]:
            voice_bank, voice = symbols[f'SFX_Swap_{suffix}_Ch{channel + 1}']
            assert voice_bank == bank
            offset = bank * 0x4000 + voice - 0x4000
            data = list(rom[offset:offset + length])
            assert data[-1] == 255
            for ptr in pointers:
                for pulse, nr51, fraction, duty in [(0, 0, 0, 0), (144, 0xa5, 173, 0x40), (174, 255, 255, 0xc0)]:
                    pulse = 174 if channel == 4 else pulse
                    item = dict(bank=bank, source_sfx_id=pulse, ptr=ptr, fraction=fraction,
                                duty=duty, flags1=4, flags2=0, note_speed=1, sfx_tempo=256,
                                vibrato_reload=0)
                    memory = {0xff25: nr51, symbols['wChannelSoundIDs'][1] + 4: pulse}
                    for name, value in [('wChannelSoundIDs', 174), ('wChannelFlags1', 4),
                                        ('wChannelFlags2', 0), ('wChannelNoteSpeeds', 1),
                                        ('wChannelNoteDelayCountersFractionalPart', fraction),
                                        ('wChannelDutyCycles', duty), ('wChannelVibratoDelayCounterReloadValues', 0),
                                        ('wChannelNoteDelayCounters', 1)]:
                        memory[symbols[name][1] + channel] = value
                    pointer = symbols['wChannelCommandPointers'][1] + 2 * channel
                    memory[pointer], memory[pointer + 1] = (voice + ptr) & 255, (voice + ptr) >> 8
                    tempo = symbols['wSfxTempo'][1]
                    memory[tempo], memory[tempo + 1] = 1, 0
                    context = dict(mute=0, sound_ids=[0] * 8, effects=[None] * 8)
                    entry = symbols[prefix + '_PlayNextNote'][1]
                    trace, registers, final, events = run_with_accesses(
                        oracle, rom, bank, context, entry=entry, registers={'C': channel}, memory=memory)
                    next_ptr = (final[pointer] | final[pointer + 1] << 8) - voice
                    if data[ptr] == 255:
                        assert not events
                        outcome = {'End': dict(next_ptr=next_ptr, fraction=fraction, duty=duty, vibrato_delay=0,
                            ending=dict(ended=True, flags1=0, flags2=0, disable_output_write=None,
                            clear_saved_volume=False, panning_write=None, command_ptr_write=None,
                            wave_restarted=False, volume_restore=None))}
                        assert final[symbols['wChannelSoundIDs'][1] + channel] == 0
                    else:
                        before, raw, _ = oracle.run(
                            rom, bank, context, entry=entry, registers={'C': channel}, memory=memory,
                            stop_pc=symbols[prefix + '_ApplyWavePatternAndFrequency'][1], capture=True)
                        assert trace[:len(before)] == before
                        outcome = {'Note': dict(next_ptr=next_ptr,
                            fraction=final[symbols['wChannelNoteDelayCountersFractionalPart'][1] + channel],
                            delay=final[symbols['wChannelNoteDelayCounters'][1] + channel],
                            duty=final[symbols['wChannelDutyCycles'][1] + channel],
                            envelope=final[0xff12 + (channel - 4) * 5], frequency=raw['D'] << 8 | raw['E'],
                            wave_return_frequency=registers['D'] << 8 | registers['E'], vibrato_delay=0)}
                        assert sum(e[2] == 'Read' for e in events) == 1
                        assert sum(e[2] == 'Write' for e in events) == 6
                    cases.append(dict(channel=channel, common=dict(input=item, data=data, nr51=nr51,
                        trace=trace, result=outcome,
                        accesses=[dict(instruction=i, pc=pc, kind=kind.lower(), address=address,
                                       value=value, at_tcycle=t) for i, pc, kind, address, value, t in events],
                        final_apu={str(a): v for a, v in final.items() if 0xff10 <= a <= 0xff3f})))
    args.output.write_text(json.dumps(dict(scope=__doc__, rom_sha256=oracle.ROM_SHA,
        pret_revision='fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c', cases=cases), separators=(',', ':')) + '\n')
    print(len(cases), 'SWAP note/end cases;', sum(len(c['common']['accesses']) for c in cases), 'APU accesses')


if __name__ == '__main__':
    main()

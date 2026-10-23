#!/usr/bin/env python3
"""Source-ROM helper/pitch APU access fixtures, without graphics or font data."""
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
            symbols[parts[1]] = (int(bank, 16), int(address, 16))
    directory = Path(__file__).resolve().parents[3] / 'crates/pokered-audio/src/source_clock'
    helpers = json.loads((directory / 'channel_output_instructions_golden.json').read_text())['cases']
    pitches = json.loads((directory / 'note_pitch_instructions_golden.json').read_text())['cases']
    helper_cases, pitch_cases = [], []
    names = dict(RegisterPointer='GetRegisterPointer', DutyAndLength='ApplyDutyCycleAndSoundLength',
                 EnableChannel='EnableChannelOutput', WaveFrequency='ApplyWavePatternAndFrequency')
    variants = [(0, 255, 1, 0, 0, 255), (165, 90, 63, 192, 4, 90), (255, 0, 255, 128, 5, 0)]

    def source_inputs(i, variant):
        bank, channel = i['bank'], i['channel']
        first, second, delay, duty, instrument, panning = variant
        prefix = 'Audio1' if bank == 2 else 'Audio3'
        def byte(address):
            return rom[bank * 0x4000 + address - 0x4000]
        pointer = symbols[prefix + '_WavePointers'][1] + instrument * 2
        address = byte(pointer) | byte(pointer + 1) << 8
        wave = [byte(address + n) for n in range(16)]
        volume = (0x40, 0x20, 0x60)[instrument % 3] if channel % 4 == 2 else (first ^ 0xb3)
        bus = dict(note_delay=delay, volume=volume, duty=duty, panning=panning,
                   wave=wave if channel % 4 == 2 else None)
        memory = {0xff25: first}
        for name, value in [('wChannelNoteDelayCounters', delay), ('wChannelDutyCycles', duty),
                            ('wChannelVolumes', volume)]:
            memory[symbols[name][1] + channel] = value
        for name, value in [('wFrequencyModifier', i['frequency_modifier']),
                            ('wMusicWaveInstrument', instrument), ('wSfxWaveInstrument', instrument),
                            ('wStereoPanning', panning)]:
            memory[symbols[name][1]] = value
        ids = [0] * 8
        ids[4] = i['source_sfx_id']
        if channel < 4 and i['paired_sfx_active']:
            if channel != 0 or ids[4] == 0:
                ids[channel + 4] = 1
        return bus, memory, dict(mute=0, sound_ids=ids, effects=[None] * 8), prefix, (first, second)

    for index, prior in enumerate(helpers):
        i = prior['input']
        for variant in variants:
            bus, memory, context, prefix, samples = source_inputs(i, variant)
            # Some historical isolated-helper fixtures only carry a boolean for
            # channel 0's paired SFX; its sound ID is otherwise irrelevant here.
            if i['channel'] < 4 and i['paired_sfx_active']:
                context['sound_ids'][i['channel'] + 4] = 1
            frequency = i['frequency']
            trace, r, _, accesses = run_with_accesses(
                oracle, rom, i['bank'], context, nr51_samples=samples,
                entry=symbols[prefix + '_' + names[prior['routine']]][1],
                registers={'C': i['channel'], 'B': 3, 'D': frequency >> 8, 'E': frequency & 255},
                memory=memory)
            assert trace == prior['trace']
            result = r['D'] << 8 | r['E'] if prior['routine'] == 'WaveFrequency' else None
            assert result == prior['frequency']
            helper_cases.append(dict(prior=index, bus=bus, accesses=accesses))

    for index, prior in enumerate(pitches):
        i = prior['input']
        for variant in variants:
            bus, memory, context, prefix, samples = source_inputs(i, variant)
            channel = i['channel']
            for name, value in [('wChannelFlags1', i['flags1']), ('wChannelOctaves', i['octave']),
                                ('wChannelFrequencyLowBytes', i['previous_frequency_low'])]:
                memory[symbols[name][1] + channel] = value
            trace, r, final, accesses = run_with_accesses(
                oracle, rom, i['bank'], context, nr51_samples=samples,
                entry=symbols[prefix + '_note_pitch'][1], registers={'C': channel},
                memory=memory, initial_stack=[i['note'] << 8])
            assert trace == prior['trace']
            wave_entry = symbols[prefix + '_ApplyWavePatternAndFrequency'][1]
            result = dict(saved_frequency_low=final[symbols['wChannelFrequencyLowBytes'][1] + channel],
                          wave_return_frequency=r['D'] << 8 | r['E'] if any(pc == wave_entry for pc, _ in trace) else None)
            assert result == prior['result']
            if i['note'] >= 0xc0 or i['paired_sfx_active']:
                bus['wave'] = None  # These paths never read a wave pattern.
            pitch_cases.append(dict(prior=index, bus=bus, accesses=accesses))
    result = dict(scope=__doc__, rom_sha256=oracle.ROM_SHA, helpers=helper_cases, pitches=pitch_cases)
    args.output.write_text(json.dumps(result, separators=(',', ':')) + '\n')
    all_cases = helper_cases + pitch_cases
    print(f'{len(helper_cases)} helpers; {len(pitch_cases)} pitches; '
          f'{sum(len(case["accesses"]) for case in all_cases)} APU accesses')


if __name__ == '__main__':
    main()

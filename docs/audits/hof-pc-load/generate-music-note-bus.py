#!/usr/bin/env python3
"""Controlled original PlayNextNote APU reads/writes; no gameplay or rendering."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path


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
        fields = line.split()
        if len(fields) > 1 and ':' in fields[0] and not fields[0].startswith(';'):
            bank, address = fields[0].split(':')
            symbols[fields[1]] = (int(bank, 16), int(address, 16))
    directory = Path(__file__).resolve().parents[3] / 'crates/pokered-audio/src/source_clock'
    prior_path = directory / 'music_note_instructions_golden.json'
    prior = json.loads(prior_path.read_text())['cases']
    jobs = [(index, case['input'], case['data'], case['trace']) for index, case in enumerate(prior)]
    for bank in (2, 31):
        for channel in range(4):
            source = next(c for c in prior if c['input']['bank'] == bank and c['input']['preparation']['stream']['channel'] == channel)
            input_ = json.loads(json.dumps(source['input']))
            stream = input_['preparation']['stream']
            stream.update(ptr=0, return_ptr=0, loop_counter=0)
            stream['state']['flags1'] = 4 if channel == 3 else 0
            input_.update(paired_sfx_active=False, source_sfx_id=0)
            # Two distinct writes must survive even though the retained result
            # contains only the final volume. Wave note_type replaces instrument.
            data = [0xf0, 0x12, 0xf0, 0x65, 0xd1]
            if channel != 3:
                data += [0x23 if channel == 2 else 0xa3]
            data += [0xc1 if channel == 3 else 0x07]
            jobs.append((None, input_, data, None))
            if channel == 3:
                jobs.append((None, input_, [0xf0, 0x21, 0xf0, 0x43, 0xff], None))
    waves = {}
    for bank, prefix in ((2, 'Audio1'), (31, 'Audio3')):
        def byte(address):
            return rom[bank * 0x4000 + address - 0x4000]
        table = symbols[prefix + '_WavePointers'][1]
        count = (symbols[prefix + '_WavePointers.wave0'][1] - table) // 2
        waves[str(bank)] = []
        for instrument in range(count):
            address = byte(table + instrument * 2) | byte(table + instrument * 2 + 1) << 8
            waves[str(bank)].append([byte(address + n) for n in range(16)])
    cases = []
    for ordinal, (source_index, input_, data, expected_trace) in enumerate(jobs):
        bank = input_['bank']
        prefix = 'Audio1' if bank == 2 else 'Audio3'
        preparation = input_['preparation']
        stream = preparation['stream']
        channel = stream['channel']
        base, memory, targets, visited = 0xd100, {}, set(), set()

        def scan(offset):
            while offset not in visited:
                visited.add(offset)
                command = data[offset]
                if command == 0xfd:
                    targets.add(offset + 1)
                    scan(data[offset + 1] | data[offset + 2] << 8)
                    offset += 3
                elif command == 0xfe:
                    targets.add(offset + 2)
                    scan(data[offset + 2] | data[offset + 3] << 8)
                    offset += 4
                elif command == 0xff:
                    return
                elif command & 0xf0 == 0xd0:
                    offset += 1 if channel == 3 else 2
                elif command == 0xe8 or 0xe0 <= command <= 0xe7:
                    offset += 1
                elif command in (0xea, 0xed):
                    offset += 3
                elif command in (0xec, 0xf0):
                    offset += 2
                else:
                    return

        scan(stream['ptr'])
        if stream['state']['flags1'] & 2:
            scan(stream['return_ptr'])
        for index, value in enumerate(data):
            memory[base + index] = value
        for offset in targets:
            pointer = base + (data[offset] | data[offset + 1] << 8)
            memory[base + offset], memory[base + offset + 1] = pointer & 255, pointer >> 8
        for name, offset in [('wChannelCommandPointers', stream['ptr']), ('wChannelReturnAddresses', stream['return_ptr'])]:
            address = symbols[name][1] + channel * 2
            memory[address], memory[address + 1] = (base + offset) & 255, (base + offset) >> 8
        fields = dict(speed='wChannelNoteSpeeds', volume='wChannelVolumes', duty='wChannelDutyCycles', octave='wChannelOctaves', flags1='wChannelFlags1', vibrato_delay='wChannelVibratoDelayCounters')
        for field, name in fields.items():
            memory[symbols[name][1] + channel] = stream['state'][field]
        count = data[1] if data[0] == 0xfe else 0
        counter = 1 if not stream['loop_counter'] else count - stream['loop_counter'] + 1
        for name, value in [('wChannelLoopCounters', counter), ('wChannelFlags2', preparation['flags2']), ('wChannelVibratoDelayCounterReloadValues', stream['vibrato_reload']), ('wChannelNoteDelayCountersFractionalPart', preparation['fraction']), ('wChannelFrequencyLowBytes', input_['previous_frequency_low'])]:
            memory[symbols[name][1] + channel] = value
        tempo_address = symbols['wMusicTempo'][1]
        memory[tempo_address], memory[tempo_address + 1] = preparation['tempo'] >> 8, preparation['tempo'] & 255
        instrument = ordinal % len(waves[str(bank)])
        panning = [255, 0x5a, 0][ordinal % 3]
        nr51 = [0, 0xa5, 255][ordinal % 3]
        for name, value in [('wMusicWaveInstrument', instrument), ('wStereoPanning', panning), ('wFrequencyModifier', input_['frequency_modifier'])]:
            memory[symbols[name][1]] = value
        memory[0xff25] = nr51
        ids = [0] * 8
        ids[4] = input_['source_sfx_id']
        if input_['paired_sfx_active']:
            ids[channel + 4] = 144
        writes, reads = [], []
        trace = oracle.run(rom, bank, dict(mute=0, sound_ids=ids, effects=[None] * 8), entry=symbols[prefix + '_PlayNextNote'][1], registers={'C': channel}, memory=memory, strict_memory=True, write_events=writes, read_events=reads)
        if expected_trace is not None:
            assert trace == expected_trace, (ordinal, input_, len(trace), len(expected_trace))
        # Observe the original memory accesses themselves, including read values
        # after earlier stores; do not derive a second output implementation.
        events = []
        for kind, accesses in [('read', reads), ('write', writes)]:
            for index, original_pc, address, value in accesses:
                assert trace[index][0] == original_pc
                events.append([index, kind, address, value])
        events.sort(key=lambda event: event[0])
        case = dict(source_index=source_index, panning=panning, initial_wave_instrument=instrument, nr51=nr51, events=events)
        if source_index is None:
            case.update(input=input_, data=data, trace=trace)
        cases.append(case)
    args.output.write_text(json.dumps(dict(scope=__doc__, rom_sha256=oracle.ROM_SHA, source_fixture_sha256=hashlib.sha256(prior_path.read_bytes()).hexdigest(), waves=waves, cases=cases), separators=(',', ':')) + '\n')
    print(len(cases), 'calls;', sum(len(c['events']) for c in cases), 'original APU accesses')


if __name__ == '__main__':
    main()

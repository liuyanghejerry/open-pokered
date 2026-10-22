#!/usr/bin/env python3
"""Original ROM whole-update CPU traces from controlled native scalar fixtures.

No Game inputs, display, physical APU emulation or IRQ epilogue are claimed.
Every non-ROM memory read must have an explicit fixture value or prior store.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('rom', 'symbols', 'native_inputs', 'output'):
        p.add_argument(name, type=Path)
    args = p.parse_args()
    spec = importlib.util.spec_from_file_location('oracle', Path(__file__).with_name('generate-timer-instructions.py'))
    o = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(o)
    rom = args.rom.read_bytes()
    assert hashlib.sha256(rom).hexdigest() == o.ROM_SHA
    symbols = {}
    for line in args.symbols.read_text().splitlines():
        bits = line.split()
        if len(bits) > 1 and ':' in bits[0] and not bits[0].startswith(';'):
            bank, address = bits[0].split(':')
            symbols[bits[1]] = (int(bank, 16), int(address, 16))
    cases = []
    for row in json.loads(args.native_inputs.read_text())['frames']:
        work = row['work']; start = work['start']; bank = work['bank']
        prefix = 'Audio1' if bank == 2 else 'Audio3'
        suffix = '1' if bank == 2 else '3'
        header_bank, header = symbols['Music_'+work['song']]
        table_bank, table = symbols['SFX_Headers_'+suffix]
        assert bank == header_bank == table_bank and (header-table) % 3 == 0
        raw_id = (header-table)//3
        def rom_byte(address): return rom[bank*0x4000+address-0x4000]
        channels = {rom_byte(header+i*3) & 15 for i in range((rom_byte(header) >> 6)+1)}
        press_bank, press = symbols['SFX_Press_AB_'+suffix]
        assert bank == press_bank and (press-table)//3 == 144 and (press-table)%3 == 0
        for ch, v in enumerate(start['voices']):
            expected = raw_id if ch < 4 else 144
            assert work['ids'][ch] == (expected if v['active'] else 0)
            if v['active'] and ch < 4: assert ch in channels
        memory = {0xff24: 0x77, 0xff25: 0}  # Explicit controlled ports, not a native APU snapshot.
        def put(name, index, value):
            memory[symbols[name][1] + index] = value
        def word(name, index, value):
            put(name, index*2, value & 255); put(name, index*2+1, value >> 8)
        put('wMuteAudioAndPauseMusic', 0, 0)
        put('wStereoPanning', 0, start['panning'])
        put('wMusicWaveInstrument', 0, 0)  # Controlled instrument; never affects the CPU branch path.
        put('wFrequencyModifier', 0, start['frequency_modifier'] & 255)
        for name, value in [('wMusicTempo', start['music_tempo']), ('wSfxTempo', start['sfx_tempo'])]:
            put(name, 0, value >> 8); put(name, 1, value & 255)
        base = 0xd000
        for ch, (v, data) in enumerate(zip(start['voices'], row['data'])):
            put('wChannelSoundIDs', ch, work['ids'][ch])
            if not v['active']:
                continue
            fields = {'flags1':'wChannelFlags1', 'flags2':'wChannelFlags2', 'speed':'wChannelNoteSpeeds',
                      'fraction':'wChannelNoteDelayCountersFractionalPart', 'duty':'wChannelDutyCycles',
                      'octave':'wChannelOctaves', 'vib_delay':'wChannelVibratoDelayCounters',
                      'vib_reload':'wChannelVibratoDelayCounterReloadValues', 'vib_extent':'wChannelVibratoExtents',
                      'vib_rate':'wChannelVibratoRates', 'saved_low':'wChannelFrequencyLowBytes'}
            for key, name in fields.items(): put(name, ch, v[key])
            put('wChannelVolumes', ch, (v['volume'] & 0x30) << 1 if ch == 2 else v['volume'])
            delay = v['delay']
            if delay == 0:
                assert ch == 4 and v['ptr'] == 0 and v['duty'] == 0 and v['volume'] == 240 and v['fraction'] == 0
                delay = 1
            put('wChannelNoteDelayCounters', ch, delay)
            if delay > 1:
                continue  # Timer path must never read an unprovided stream or loop counter.
            assert base + len(data) < 0xe000
            for i, byte in enumerate(data): memory[base+i] = byte
            targets = set(); visited = set()
            def scan(offset):
                while offset not in visited:
                    visited.add(offset); cmd = data[offset]
                    if ch == 4:
                        if cmd == 255: return
                        offset += 2 if cmd == 236 else 4
                        return  # A tone ends command processing for this update.
                    if cmd == 253:
                        targets.add(offset+1); scan(data[offset+1] | data[offset+2] << 8); offset += 3
                    elif cmd == 254:
                        targets.add(offset+2); scan(data[offset+2] | data[offset+3] << 8); offset += 4
                    elif cmd == 255: return
                    elif cmd & 240 == 208: offset += 1 if ch == 3 else 2
                    elif cmd == 232 or 224 <= cmd <= 231: offset += 1
                    elif cmd in (234, 237): offset += 3
                    elif cmd in (236, 240): offset += 2
                    else: return
            scan(v['ptr'])
            if v['flags1'] & 2: scan(v['return_ptr'])
            for offset in targets:
                target = base + (data[offset] | data[offset+1] << 8)
                memory[base+offset] = target & 255; memory[base+offset+1] = target >> 8
            word('wChannelCommandPointers', ch, base + v['ptr'])
            word('wChannelReturnAddresses', ch, base + v['return_ptr'])
            if not v['loop_counter']:
                put('wChannelLoopCounters', ch, 1)
            else:
                # Native remaining count and source ascending count differ. Find the
                # first finite loop actually reachable before the next note, without
                # inferring a value when this update never reads that counter.
                ptr = v['ptr']; ret = v['return_ptr']; seen = set()
                while ptr not in seen:
                    seen.add(ptr); cmd = data[ptr]
                    if cmd == 253: ret = ptr+3; ptr = data[ptr+1] | data[ptr+2] << 8
                    elif cmd == 255 and v['flags1'] & 2: ptr = ret
                    elif cmd == 254:
                        count = data[ptr+1]
                        if count:
                            assert 0 < v['loop_counter'] < count
                            put('wChannelLoopCounters', ch, count-v['loop_counter']+1)
                            break
                        ptr = data[ptr+2] | data[ptr+3] << 8
                    elif cmd & 240 == 208: ptr += 1 if ch == 3 else 2
                    elif cmd == 232 or 224 <= cmd <= 231: ptr += 1
                    elif cmd in (234, 237): ptr += 3
                    elif cmd in (236, 240): ptr += 2
                    else: break
            base += len(data) + 16
        entry = symbols[prefix+'_UpdateMusic'][1]
        try:
            trace, regs, final = o.run(rom, bank, dict(mute=0, sound_ids=work['ids'], effects=[None]*8),
                entry=entry, memory=memory, capture=True, strict_memory=True, instruction_limit=20000)
        except Exception as exc:
            raise AssertionError(f"{work['song']} confirm{row['confirmation']} frame{row['frame']}: {exc}") from exc
        cases.append(dict(song=work['song'], confirmation=row['confirmation'], frame=row['frame'],
                          cycles=sum(t for _, t in trace), trace=trace))
    args.output.write_text(json.dumps(dict(scope=__doc__, rom_sha256=o.ROM_SHA,
        input_sha256=hashlib.sha256(args.native_inputs.read_bytes()).hexdigest(), cases=cases), separators=(',', ':'))+'\n')
    print(f'{len(cases)} strict-memory original whole updates, {sum(len(c["trace"]) for c in cases)} instructions')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Independent original Audio1/3 timer/effect APU store fixtures.

Controlled scalar WRAM inputs interpreted from pinned ROM instructions. No
gameplay recording, APU timing simulation, graphics or font rendering occurs.
"""
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
    spec = importlib.util.spec_from_file_location(
        'oracle', Path(__file__).with_name('generate-timer-instructions.py'))
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
    previous = json.loads((directory / 'timer_instructions_golden.json').read_text())
    effects, frames = [], []

    def record(context, channel=None):
        bank = context['bank']
        prefix = 'Audio1' if bank == 2 else 'Audio3'
        name = '_ApplyMusicAffects' if channel is not None else '_UpdateMusic'
        assert symbols[prefix + name][0] == bank
        writes = []
        trace = oracle.run(rom, bank, context, channel,
                           entry=symbols[prefix + name][1],
                           strict_memory=True, write_events=writes)
        for index, pc, address, value in writes:
            assert trace[index][0] == pc
            assert 0xff10 <= address <= 0xff3f and 0 <= value <= 255
        return dict(trace=trace, writes=writes)

    def effect(bank, item, old_trace=None):
        channel = item['channel']
        context = dict(bank=bank, mute=0, sound_ids=[0] * 8, effects=[None] * 8)
        context['sound_ids'][channel] = 1
        context['effects'][channel] = item
        if channel < 4 and item['paired_sfx_active']:
            context['sound_ids'][channel + 4] = 2
        result = record(context, channel)
        if old_trace is not None:
            assert result['trace'] == old_trace
        effects.append(dict(bank=bank, input=item, **result))

    # Preserve every existing CPU-only branch fixture and independently add its
    # writes. No expected APU value/address is synthesized by the generator.
    for old in previous['effects']:
        effect(old['bank'], old['input'], old['trace'])
    for old in previous['frames']:
        result = record(old['context'])
        assert result['trace'] == old['trace']
        frames.append(dict(context=old['context'], **result))

    base = dict(channel=0, paired_sfx_active=False, note_delay=5, flags1=0,
                flags2=0, vibrato_delay=0, vibrato_extent=0x21,
                vibrato_rate=0, frequency_low=90)
    # Exercise all nibble magnitudes with deliberately unequal upper/lower
    # extents, both directions and the exact saturating arithmetic boundaries.
    for bank in (2, 31):
        for amount in range(16):
            extent = amount << 4 | (15 - amount)
            for direction in (0, 8):
                threshold = 15 - amount if direction else 255 - amount
                frequencies = sorted({0, 255, max(0, threshold - 1), threshold,
                                      min(255, threshold + 1)})
                for frequency in frequencies:
                    item = dict(base, channel=amount % 8, flags1=direction,
                                note_delay=(0, 2, 255)[frequency % 3],
                                vibrato_extent=extent, vibrato_rate=amount << 4,
                                frequency_low=frequency)
                    effect(bank, item)
        # Unsupported flags are harmless only when the actual branch returns
        # before reading their associated payload. No duty/slide data is filled.
        for channel in range(8):
            effect(bank, dict(base, channel=channel, flags1=4 | 16))
            if channel < 4:
                effect(bank, dict(base, channel=channel, flags1=64 | 16,
                                  flags2=1, paired_sfx_active=True))
        for mask in (0, 1, 8, 16, 17, 0x55, 0xaa, 0xff):
            for variant in range(4):
                context = dict(bank=bank, mute=0, sound_ids=[0] * 8,
                               effects=[None] * 8)
                for channel in range(8):
                    if not mask & 1 << channel:
                        continue
                    context['sound_ids'][channel] = channel + 1
                    item = dict(base, channel=channel,
                                paired_sfx_active=channel < 4 and bool(mask & 1 << (channel + 4)),
                                note_delay=(2, 255)[channel % 2],
                                flags1=8 if channel % 2 else 0,
                                frequency_low=(0, 255)[channel % 2])
                    if variant == 1:
                        item['vibrato_delay'] = channel % 3
                    elif variant == 2:
                        item['vibrato_rate'] = 0xa0 | channel % 3
                        item['vibrato_extent'] = 0 if channel % 3 == 0 else 0xf1
                    elif variant == 3:
                        item['flags1'] |= 4
                        item['flags2'] = channel % 2
                    context['effects'][channel] = item
                frames.append(dict(context=context, **record(context)))

    result = dict(scope=__doc__, rom_sha256=oracle.ROM_SHA,
                  pret_revision='fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c',
                  effects=effects, frames=frames)
    args.output.write_text(json.dumps(result, separators=(',', ':')) + '\n')
    print(f'{len(effects)} effect calls; {len(frames)} complete timer updates; '
          f'{sum(len(case["writes"]) for case in effects + frames)} APU stores')


if __name__ == '__main__':
    main()

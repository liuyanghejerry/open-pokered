#!/usr/bin/env python3
"""Controlled CPU fixture oracle, not a gameplay recording or APU emulator.

Walk pinned original ROM bytes with explicit scalar WRAM fixtures. This bounded
interpreter covers only the Audio1/3 timer/effect subset, fails on any unknown
opcode/path, and records each actual instruction's PC and T-cycles. No graphics
or font patterns are decoded or rendered.
"""
import argparse
import hashlib
import json
from pathlib import Path

ROM_SHA = '5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
FIELDS = {'note_delay':0xc0b6, 'flags1':0xc02e, 'flags2':0xc036,
          'vibrato_delay':0xc04e, 'vibrato_extent':0xc056,
          'vibrato_rate':0xc05e, 'frequency_low':0xc066}


def run(rom, bank, context, channel=None):
    r = {x:0 for x in 'ABCDEHL'}
    r['C'] = channel or 0
    m = {0xc002:context['mute']}
    for i in range(8):
        m[0xc026+i] = context['sound_ids'][i]
        if context['effects'][i]:
            for field, address in FIELDS.items():
                m[address+i] = context['effects'][i][field]
    pc = (0x5138 if bank == 2 else 0x51ac) if channel is not None else (0x5103 if bank == 2 else 0x5177)
    stack, trace = [], []
    z = carry = False

    def read(address):
        if 0x4000 <= address < 0x8000:
            return rom[bank*0x4000+address-0x4000]
        return m.get(address, 0)

    def fetch():
        nonlocal pc
        v = read(pc)
        pc += 1
        return v

    def word():
        lo = fetch()
        return lo | fetch()<<8

    def hl():
        return r['H']<<8 | r['L']

    def sethl(value):
        r['H'], r['L'] = value>>8 & 255, value & 255

    for _ in range(2000):
        start = pc
        op = fetch()
        done = False
        if op in (0x06,0x0e,0x26,0x3e):
            r[{0x06:'B',0x0e:'C',0x26:'H',0x3e:'A'}[op]] = fetch(); t = 8
        elif op == 0x21:
            sethl(word()); t = 12
        elif op == 0x09:
            value = hl()+(r['B']<<8 | r['C']); carry = value>65535
            sethl(value); t = 8
        elif op in (0x7e,0x5e):
            r['A' if op == 0x7e else 'E'] = read(hl()); t = 8
        elif op == 0xfa:
            r['A'] = read(word()); t = 16
        elif op in (0x77,0x72):
            m[hl()] = r['A' if op == 0x77 else 'D']; t = 8
        elif op in (0x79,0x7a,0x7b,0x57,0x6f):
            dst, src = {0x79:('A','C'),0x7a:('A','D'),0x7b:('A','E'),0x57:('D','A'),0x6f:('L','A')}[op]
            r[dst] = r[src]; t = 4
        elif op in (0xa7,0xe6,0xb6):
            if op == 0xe6: r['A'] &= fetch(); t = 8
            elif op == 0xb6: r['A'] |= read(hl()); t = 8
            else: t = 4
            z, carry = r['A'] == 0, False
        elif op == 0xfe:
            value = fetch(); z, carry = r['A'] == value, r['A'] < value; t = 8
        elif op in (0x80,0x83,0x85,0x92):
            v = r[{0x80:'B',0x83:'E',0x85:'L',0x92:'D'}[op]]
            n = r['A']-v if op == 0x92 else r['A']+v
            carry = n<0 or n>255; r['A'] = n&255; z = r['A']==0; t = 4
        elif op in (0x3d,0x35,0x0c):
            key = 'A' if op == 0x3d else 'C'
            v = read(hl()) if op == 0x35 else r[key]
            v = (v+(1 if op == 0x0c else -1))&255
            if op == 0x35: m[hl()] = v
            else: r[key] = v
            z = v == 0; t = 12 if op == 0x35 else 4
        elif op in (0x18,0x20,0x28,0x30):
            off = fetch(); off = off-256 if off&128 else off
            take = {0x18:True,0x20:not z,0x28:z,0x30:not carry}[op]
            if take: pc += off
            t = 12 if take else 8
        elif op == 0xca:
            target = word()
            if z: raise AssertionError('next-note fixture is outside timer oracle')
            t = 12
        elif op == 0xcd:
            target = word(); stack.append(pc); pc = target; t = 24
        elif op == 0xc9:
            if stack: pc = stack.pop()
            else: done = True
            t = 16
        elif op == 0xcb:
            cb = fetch(); target = hl() if cb&7 == 6 else None
            value = read(target) if target is not None else r['A']
            if 0x40 <= cb <= 0x7f:
                z = not bool(value & (1<<((cb-0x40)//8))); t = 12 if target is not None else 8
            elif cb in (0x36,0x37):
                value = (value>>4 | value<<4)&255; z,carry = value==0,False
                if target is not None: m[target] = value
                else:r['A'] = value
                t = 16 if target is not None else 8
            elif cb in (0x9e,0xde):
                m[hl()] = value & ~8 if cb == 0x9e else value | 8; t = 16
            else: raise AssertionError(('CB',hex(cb),hex(start)))
        else:
            raise AssertionError((hex(op),hex(start),bank))
        trace.append([start,t])
        if done:return trace
    raise AssertionError('oracle exceeded bound')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('rom',type=Path)
    parser.add_argument('output',type=Path)
    args = parser.parse_args()
    rom = args.rom.read_bytes()
    assert hashlib.sha256(rom).hexdigest() == ROM_SHA
    cases = []
    # All branch edges: delay/extent/rate early returns, both vibrato directions,
    # carry/clamps, SFX noise/execute flags, and paired music suppression.
    variants = [dict(vibrato_delay=2),dict(vibrato_extent=0),dict(vibrato_rate=3),
                dict(flags1=8,frequency_low=0),dict(flags1=8,frequency_low=90),
                dict(flags1=0,frequency_low=255),dict(flags1=0,frequency_low=90),
                dict(flags1=4),dict(flags1=4,flags2=1),dict(paired_sfx_active=True)]
    for bank in (2,31):
        for ch in range(8):
            for variant in variants:
                i = dict(channel=ch,paired_sfx_active=False,note_delay=5,flags1=0,flags2=0,
                         vibrato_delay=0,vibrato_extent=0x21,vibrato_rate=0,frequency_low=90)
                i.update(variant)
                context = dict(bank=bank,mute=0,sound_ids=[0]*8,effects=[None]*8)
                context['sound_ids'][ch] = 1; context['effects'][ch] = i
                if ch < 4 and i['paired_sfx_active']:context['sound_ids'][ch+4] = 2
                cases.append(dict(input=i,bank=bank,trace=run(rom,bank,context,ch)))
    frames = []
    for bank in (2,31):
        for pattern in (0,1,0x55,0xff):
            context = dict(bank=bank,mute=0,sound_ids=[0]*8,effects=[None]*8)
            for ch in range(8):
                if pattern & 1<<ch:
                    context['sound_ids'][ch] = ch+1
                    i = dict(channel=ch,paired_sfx_active=ch<4 and bool(pattern & 1<<(ch+4)),
                             note_delay=5,flags1=8 if ch%2 else 0,flags2=0,
                             vibrato_delay=ch%2,vibrato_extent=0x21,vibrato_rate=0,frequency_low=90)
                    context['effects'][ch] = i
            frames.append(dict(context=context,trace=run(rom,bank,context)))
    metadata = dict(scope='Controlled scalar fixtures interpreted from pinned ROM bytes; not gameplay, APU timing or Game integration.',
                    rom_sha256=ROM_SHA,pret_revision='fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c')
    text = json.dumps(metadata,indent=2)[:-2]+',\n'
    for key, values in [('effects',cases),('frames',frames)]:
        text += '  '+json.dumps(key)+': [\n'
        text += ',\n'.join('    '+json.dumps(v,separators=(',',':')) for v in values)
        text += '\n  ]'+(',' if key == 'effects' else '')+'\n'
    args.output.write_text(text+'}\n')
    print(f'{len(cases)} effect cases and {len(frames)} full timer bodies')

if __name__ == '__main__':main()

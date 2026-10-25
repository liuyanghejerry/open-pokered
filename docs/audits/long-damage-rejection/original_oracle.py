#!/usr/bin/env python3
"""Observe original RandomizeDamage with prepared RAM, stack and link seeds.

Requires PyBoy and matching ROM/symbol/state files. No ROM is included here.
"""
import argparse
import hashlib
import importlib.metadata
import io
import json
import os
from pathlib import Path

os.environ.setdefault('SDL_AUDIODRIVER', 'dummy')
from pyboy import PyBoy

parser = argparse.ArgumentParser(description=__doc__)
for name in ['rom', 'symbols', 'state', 'output']:
    parser.add_argument('--' + name, type=Path, required=True)
args = parser.parse_args()
symbols = {}
for line in args.symbols.read_text().splitlines():
    words = line.split()
    if len(words) == 2 and ':' in words[0]:
        bank, address = words[0].split(':')
        symbols[words[1]] = (int(bank, 16), int(address, 16))
rom, state = args.rom.read_bytes(), args.state.read_bytes()
rows = []
cases = [([raw] * 10, 2) for raw in range(256)]
for seeds, damage in cases:
    raw = seeds[0]
    p = PyBoy(io.BytesIO(rom), window='null', sound_emulated=False)
    p.set_emulation_speed(0)
    seen, calls = [], []
    try:
        p.load_state(io.BytesIO(state))
        for name, value in [('wLinkState', 4), ('wLinkBattleRandomNumberListIndex', 2), ('hWhoseTurn', 0)]:
            p.memory[symbols[name][1]] = value
        for index, value in enumerate(seeds):
            p.memory[symbols['wLinkBattleRandomNumberList'][1] + index] = value
        damage_address = symbols['wDamage'][1]
        p.memory[damage_address], p.memory[damage_address + 1] = damage >> 8, damage & 255

        def observe_rng(_):
            calls.append(p.memory[symbols['wLinkBattleRandomNumberListIndex'][1]])

        def observe_return(_):
            if not seen:
                seen.append({'damage': p.memory[damage_address] * 256 + p.memory[damage_address + 1],
                             'draws': len(calls), 'final_index': p.memory[symbols['wLinkBattleRandomNumberListIndex'][1]]})

        p.hook_register(*symbols['BattleRandom'], observe_rng, None)
        p.hook_register(*symbols['SoftReset'], observe_return, None)
        bank, pc = symbols['RandomizeDamage']
        p.memory[0x2000] = bank
        p.memory[symbols['hLoadedROMBank'][1]] = bank
        sp, ret = p.register_file.SP - 2, symbols['SoftReset'][1]
        p.memory[sp], p.memory[sp + 1] = ret & 255, ret >> 8
        p.register_file.SP, p.register_file.PC = sp, pc
        for _ in range(3):
            p.tick(1, render=False, sound=False)
            if seen: break
        assert len(seen) == 1, (raw, damage)
        assert len(calls) == seen[0]['draws']
        rows.append({'raw': raw, 'seeds': seeds, 'initial_damage': damage, 'initial_index': 2, 'result': seen[0], 'BattleRandom_calls': calls})
    finally:
        p.stop(save=False)

proof = {'status': 'passed', 'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
         'pyboy': importlib.metadata.version('pyboy'),
         'scope': 'Original RandomizeDamage subroutine execution with prepared RAM/registers/stack and actual BattleRandom link seeds. Read-only hooks, no ROM patch, no formula reimplementation or full-battle/UI/timing claim.',
         'rom_sha256': hashlib.sha256(rom).hexdigest(), 'state_sha256': hashlib.sha256(state).hexdigest(),
         'symbols_sha256': hashlib.sha256(args.symbols.read_bytes()).hexdigest(),
         'source_reference': 'pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c engine/battle/core.asm RandomizeDamage',
         'cases': rows}
args.output.write_text(json.dumps(proof, indent=2) + '\n')
print(json.dumps({'status': 'passed', 'cases': len(rows), 'output': str(args.output)}))

#!/usr/bin/env python3
"""Observe CriticalHitTest in an original ROM using prepared RAM and registers.

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
rom = args.rom.read_bytes()
state = args.state.read_bytes()
rows = []
for side in [0, 1]:
    for raw, power in [(raw, 35) for raw in range(256)] + [(16, 0)]:
        p = PyBoy(io.BytesIO(rom), window='null', sound_emulated=False)
        p.set_emulation_speed(0)
        seen, calls = [], []
        try:
            p.load_state(io.BytesIO(state))
            values = {'wLinkState': 4, 'wLinkBattleRandomNumberListIndex': 0,
                      'hWhoseTurn': side, 'wBattleMonSpecies': 0x83,
                      'wEnemyMonSpecies': 0x83, 'wCriticalHitOrOHKO': 0}
            for prefix in ['wPlayer', 'wEnemy']:
                values.update({prefix + 'MoveNum': 0x21, prefix + 'MovePower': power,
                               prefix + 'BattleStatus2': 0})
            for name, value in values.items():
                p.memory[symbols[name][1]] = value
            for index, value in enumerate([raw] + [255] * 8):
                p.memory[symbols['wLinkBattleRandomNumberList'][1] + index] = value

            def observe_rng(_):
                calls.append(p.memory[symbols['wLinkBattleRandomNumberListIndex'][1]])

            def observe_return(_):
                if not seen:
                    seen.append({name: p.memory[symbols[symbol][1]] for name, symbol in [
                        ('critical', 'wCriticalHitOrOHKO'),
                        ('draws', 'wLinkBattleRandomNumberListIndex'),
                        ('base_speed', 'wMonHBaseSpeed')]})

            p.hook_register(*symbols['BattleRandom'], observe_rng, None)
            p.hook_register(*symbols['SoftReset'], observe_return, None)
            bank, pc = symbols['CriticalHitTest']
            p.memory[0x2000] = bank
            p.memory[symbols['hLoadedROMBank'][1]] = bank
            sp, ret = p.register_file.SP - 2, symbols['SoftReset'][1]
            p.memory[sp], p.memory[sp + 1] = ret & 255, ret >> 8
            p.register_file.SP, p.register_file.PC = sp, pc
            for _ in range(3):
                p.tick(1, render=False, sound=False)
                if seen:
                    break
            assert len(seen) == 1 and seen[0]['base_speed'] == 130
            assert seen[0]['critical'] in [0, 1]
            assert seen[0]['draws'] == len(calls) == int(power > 0)
            rows.append({'side': side, 'raw': raw, 'power': power, **seen[0]})
        finally:
            p.stop(save=False)

sha = lambda data: hashlib.sha256(data).hexdigest()
proof = {'status': 'passed', 'pyboy_version': importlib.metadata.version('pyboy'),
         'scope': 'Prepared original-ROM subroutine execution with real BattleRandom '
                  'and the link seed list. RAM/register operands and stack return '
                  'are prepared; hooks only observe. No full-battle/UI/clock claim.',
         'script_sha256': sha(Path(__file__).read_bytes()), 'rom_sha256': sha(rom),
         'symbols_sha256': sha(args.symbols.read_bytes()), 'state_sha256': sha(state),
         'cases': rows}
args.output.write_text(json.dumps(proof, indent=2) + '\n')
print(json.dumps({'status': 'passed', 'cases': len(rows), 'output': str(args.output)}))

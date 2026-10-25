#!/usr/bin/env python3
"""Observe original HealEffect guards, HP results and Rest's first text branch."""
import argparse, hashlib, importlib.metadata, io, json, os
from pathlib import Path
os.environ.setdefault('SDL_AUDIODRIVER', 'dummy')
from pyboy import PyBoy

parser = argparse.ArgumentParser(description=__doc__)
for name in ['rom', 'symbols', 'state', 'output']: parser.add_argument('--' + name, type=Path, required=True)
a = parser.parse_args(); symbols = {}
for line in a.symbols.read_text().splitlines():
    words = line.split()
    if len(words) == 2 and ':' in words[0]:
        bank, pc = words[0].split(':'); symbols[words[1]] = (int(bank, 16), int(pc, 16))
rom = a.rom.read_bytes(); saved = a.state.read_bytes(); rows = []
for name, move_id in [('Recover', 105), ('Softboiled', 135), ('Rest', 156)]:
    for side in [0, 1]:
        conditions = [(f'deficit{deficit}', deficit, 0) for deficit in [0, 30, 255, 511]]
        if name == 'Rest': conditions += [('poisoned', 30, 8), ('burned', 30, 16), ('paralyzed', 30, 64)]
        for condition, deficit, status in conditions:
            p = PyBoy(io.BytesIO(rom), window='null', sound_emulated=False)
            p.set_emulation_speed(0); seen = []; draws = []
            try:
                p.load_state(io.BytesIO(saved))
                def put(key, value): p.memory[symbols[key][1]] = value
                def word(key, value):
                    addr = symbols[key][1]; p.memory[addr] = value >> 8; p.memory[addr + 1] = value & 255
                def get_word(key):
                    addr = symbols[key][1]; return (p.memory[addr] << 8) | p.memory[addr + 1]
                put('wLinkState', 4); put('wLinkBattleRandomNumberListIndex', 0); put('hWhoseTurn', side)
                owner = 'Enemy' if side else 'Player'; mon = 'wEnemyMon' if side else 'wBattleMon'
                put('w' + owner + 'MoveNum', move_id); put('w' + owner + 'MoveEffect', 56)
                for actor in ['Player', 'Enemy']:
                    for n in [1, 2, 3]: put('w' + actor + 'BattleStatus' + str(n), 0)
                hp = 630 - deficit; attack = 20 if status == 16 else 40; speed = 25 if status == 64 else 100
                word(mon + 'HP', hp); word(mon + 'MaxHP', 630); put(mon + 'Status', status)
                word(mon + 'Attack', attack); word(mon + 'Speed', speed)
                p.hook_register(*symbols['BattleRandom'], lambda _: draws.append(1), None)
                def finish(branch):
                    def observe(_):
                        if not seen:
                            seen.append({'branch': branch, 'hp': get_word(mon + 'HP'),
                                'status': p.memory[symbols[mon + 'Status'][1]],
                                'attack': get_word(mon + 'Attack'), 'speed': get_word(mon + 'Speed'),
                                'text': p.register_file.HL if branch == 'rest_before_sleep_text' else None,
                                'draws': len(draws)})
                    return observe
                p.hook_register(*symbols['PlayCurrentMoveAnimation'], finish('healed_before_animation'), None)
                p.hook_register(*symbols['HealEffect_.failed'], finish('recovery_failed'), None)
                p.hook_register(*symbols['HealEffect_.printRestText'], finish('rest_before_sleep_text'), None)
                bank, pc = symbols['HealEffect']; p.memory[0x2000] = bank; put('hLoadedROMBank', bank)
                # The saved frame can be inside VBlank with IME cleared.
                # Execute the original VBlank RETI (no patched instructions),
                # returning into HealEffect with normal interrupt handling.
                reti = symbols['DelayFrame'][1] - 1
                assert rom[reti] == 0xd9
                sp = p.register_file.SP - 4; ret = symbols['SoftReset'][1]
                p.memory[sp:sp + 4] = [pc & 255, pc >> 8, ret & 255, ret >> 8]
                p.register_file.SP = sp; p.register_file.PC = reti
                for _ in range(80):
                    p.tick(1, render=False, sound=False)
                    if seen: break
                assert len(seen) == 1, (name, side, condition, p.register_file.PC)
                failed = deficit in [0, 255, 511]
                expected = {'branch': 'recovery_failed' if failed else 'rest_before_sleep_text' if name == 'Rest' else 'healed_before_animation',
                    'hp': hp if failed or name == 'Rest' else min(630, hp + 315),
                    'status': 2 if name == 'Rest' and not failed else status,
                    'attack': attack, 'speed': speed,
                    'text': symbols['FellAsleepBecameHealthyText' if status else 'StartedSleepingEffect'][1] if name == 'Rest' and not failed else None,
                    'draws': 0}
                assert seen[0] == expected, (name, side, condition, seen[0], expected)
                rows.append({'move': name, 'side': side, 'condition': condition,
                             'initial_hp': hp, 'initial_status': status, 'result': seen[0]})
            finally: p.stop(save=False)
record = {'status': 'passed', 'scope': '30 prepared original HealEffect calls. Read-only hooks stop observation at failure, healed-before-animation, or Rest first sleep-text branch before HP healing. No complete original turn, animation, text playback or timing claim.',
          'entry_setup': 'Original VBlank RETI returns to HealEffect with IME enabled; no ROM patch or injected instructions.',
          'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
          'rom_sha256': hashlib.sha256(rom).hexdigest(), 'state_sha256': hashlib.sha256(saved).hexdigest(),
          'symbols_sha256': hashlib.sha256(a.symbols.read_bytes()).hexdigest(),
          'pyboy': importlib.metadata.version('pyboy'), 'cases': rows}
a.output.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps({'status': record['status'], 'cases': len(rows), 'output': str(a.output)}))

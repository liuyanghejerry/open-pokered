"""Observe original trapping status-helper text dispatch and return operands."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path

os.environ['SDL_AUDIODRIVER'] = 'dummy'
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
for side in [0, 1]:
    for active in [False, True]:
        for remaining in [1, 4]:
            p = PyBoy(io.BytesIO(rom), window='null', sound_emulated=False)
            p.set_emulation_speed(0)
            calls, returned = [], []
            try:
                p.load_state(io.BytesIO(state))
                def set_byte(name, value):
                    p.memory[symbols[name][1]] = value
                set_byte('hWhoseTurn', side)
                prefix = 'wPlayer' if side == 0 else 'wEnemy'
                status = prefix + 'BattleStatus1'
                counter = prefix + 'NumAttacksLeft'
                set_byte(status, 0x20 if active else 0)
                set_byte(prefix + 'BattleStatus2', 0)
                set_byte(counter, remaining)
                for name in ['wBattleMonNick', 'wEnemyMonNick']:
                    address = symbols[name][1]
                    p.memory[address], p.memory[address + 1] = 0x80, 0x50

                def observe_text(_):
                    if not returned:
                        calls.append({'hook': 'PrintText', 'hl': p.register_file.HL})

                def observe_return(_):
                    if not returned:
                        returned.append({'hl': p.register_file.HL,
                                         'zero_flag': bool(p.register_file.F & 0x80),
                                         'remaining': p.memory[symbols[counter][1]]})

                p.hook_register(*symbols['PrintText'], observe_text, None)
                p.hook_register(*symbols['SoftReset'], observe_return, None)
                entry = ('CheckPlayerStatusConditions.MultiturnMoveCheck' if side == 0
                         else 'CheckEnemyStatusConditions.checkIfUsingMultiturnMove')
                bank, pc = symbols[entry]
                p.memory[0x2000] = bank
                set_byte('hLoadedROMBank', bank)
                bootstrap = symbols['DelayFrame'][1] - 1
                assert p.memory[bootstrap] == 0xd9
                sp, ret = p.register_file.SP - 4, symbols['SoftReset'][1]
                p.memory[sp], p.memory[sp + 1] = pc & 255, pc >> 8
                p.memory[sp + 2], p.memory[sp + 3] = ret & 255, ret >> 8
                p.register_file.SP, p.register_file.PC = sp, bootstrap
                p.register_file.HL = symbols[status][1]
                for frame in range(360):
                    if frame % 4 == 0: p.button_press('a')
                    if frame % 4 == 2: p.button_release('a')
                    p.tick(1, render=False, sound=False)
                    if returned: break
                assert len(returned) == 1, (side, active, remaining)
                assert calls == ([{'hook': 'PrintText', 'hl': symbols['AttackContinuesText'][1]}]
                                 if active else []), calls
                assert returned[0]['remaining'] == remaining - int(active)
                assert returned[0]['zero_flag'] == active
                if active:
                    animation = 'GetPlayerAnimationType' if side == 0 else 'GetEnemyAnimationType'
                    assert returned[0]['hl'] == symbols[animation][1]
                rows.append({'side': side, 'active': active, 'remaining': remaining,
                             'entry': entry, 'text_calls': calls, 'return': returned[0]})
            finally:
                p.stop(save=False)
proof = {'status': 'passed',
         'scope': 'Isolated original trapping status-helper branches with prepared flag, counter, registers, stack and short names. Read-only hooks observe PrintText receiving AttackContinuesText, counter decrement and animation-address return. Not full turns, rendered dialogue or animation timing.',
         'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
         'rom_sha256': hashlib.sha256(rom).hexdigest(),
         'state_sha256': hashlib.sha256(state).hexdigest(),
         'symbols_sha256': hashlib.sha256(args.symbols.read_bytes()).hexdigest(),
         'cases': rows}
args.output.write_text(json.dumps(proof, indent=2) + '\n')
print(json.dumps({'status': 'passed', 'cases': len(rows)}))

from pathlib import Path
import argparse, hashlib, io, json, os
os.environ['SDL_AUDIODRIVER'] = 'dummy'
from pyboy import PyBoy

parser = argparse.ArgumentParser(description='Observe the original zero-damage trapping post-hit dispatcher.')
for name in ['rom', 'symbols', 'state', 'output']: parser.add_argument('--' + name, type=Path, required=True)
args = parser.parse_args()
symbols_file = args.symbols
symbols = {}
for line in symbols_file.read_text().splitlines():
    fields = line.split()
    if len(fields) == 2 and ':' in fields[0]:
        bank, address = fields[0].split(':')
        symbols[fields[1]] = (int(bank, 16), int(address, 16))
rom = args.rom.read_bytes()
state = args.state.read_bytes()
rows = []
for side in [0, 1]:
    for hp in [450, 0]:
        p = PyBoy(io.BytesIO(rom), window='null', sound_emulated=False)
        p.set_emulation_speed(0)
        calls = []
        try:
            p.load_state(io.BytesIO(state))
            def set_byte(name, value): p.memory[symbols[name][1]] = value
            def set_word(name, value):
                address = symbols[name][1]
                p.memory[address], p.memory[address + 1] = value >> 8, value & 255
            set_byte('hWhoseTurn', side)
            set_byte('wMoveMissed', 0)
            set_byte('wCriticalHitOrOHKO', 0)
            set_byte('wDamageMultipliers', 10)
            set_word('wDamage', 0)
            for name in ['wBattleMonHP', 'wEnemyMonHP', 'wBattleMonMaxHP', 'wEnemyMonMaxHP']: set_word(name, 450)
            target_hp = 'wEnemyMonHP' if side == 0 else 'wBattleMonHP'
            set_word(target_hp, hp)
            for who, prefix in enumerate(['wPlayer', 'wEnemy']):
                set_byte(prefix + 'MoveNum', 0x23)  # WRAP
                set_byte(prefix + 'MovePower', 15)
                set_byte(prefix + 'MoveEffect', 0x2a)  # TRAPPING_EFFECT
                set_byte(prefix + 'MoveType', 0)
                set_byte(prefix + 'BattleStatus1', 0x20 if who == side else 0)
                set_byte(prefix + 'BattleStatus2', 0 if who == side else 0x40)
                set_byte(prefix + 'MonStatMods', 7)
            def observe_rage(_):
                if not calls:
                    calls.append({'hook': 'HandleBuildingRage', 'damage': p.memory[symbols['wDamage'][1]] * 256 + p.memory[symbols['wDamage'][1] + 1], 'move_missed': p.memory[symbols['wMoveMissed'][1]]})
            def observe_boost(_):
                calls.append({'hook': 'StatModifierUpEffect', 'turn': p.memory[symbols['hWhoseTurn'][1]]})
            def observe_return(_):
                if not calls: calls.append({'hook': 'SoftReset'})
            p.hook_register(*symbols['HandleBuildingRage'], observe_rage, None)
            p.hook_register(*symbols['StatModifierUpEffect'], observe_boost, None)
            p.hook_register(*symbols['SoftReset'], observe_return, None)
            entry = 'MirrorMoveCheck.next' if side == 0 else 'EnemyCheckIfMirrorMoveEffect.notMetronomeEffect'
            bank, pc = symbols[entry]
            p.memory[0x2000] = bank
            set_byte('hLoadedROMBank', bank)
            # Snapshot ends inside VBlank with interrupts disabled. Enter via the
            # ROM's existing RETI to enable interrupts for HP-bar DelayFrame.
            bootstrap = symbols['DelayFrame'][1] - 1
            assert p.memory[bootstrap] == 0xd9
            sp, ret = p.register_file.SP - 4, symbols['SoftReset'][1]
            p.memory[sp], p.memory[sp + 1] = pc & 255, pc >> 8
            p.memory[sp + 2], p.memory[sp + 3] = ret & 255, ret >> 8
            p.register_file.SP, p.register_file.PC = sp, bootstrap
            for frame in range(360):
                if frame % 4 == 0: p.button_press("a")
                if frame % 4 == 2: p.button_release("a")
                p.tick(1, render=False, sound=False)
                if calls and calls[-1]["hook"] in ["StatModifierUpEffect", "SoftReset"]: break
            assert calls, (side, hp, hex(p.register_file.PC), p.memory[symbols["hLoadedROMBank"][1]], [(n,p.memory[symbols[n][1]]) for n in ["wMoveMissed","wCriticalHitOrOHKO","wDamageMultipliers"]])
            rows.append({'side': side, 'target_hp': hp, 'entry': entry, 'observations': calls})
            assert (calls[0]['hook'] == 'HandleBuildingRage') == (hp > 0)
            assert any(c['hook'] == 'StatModifierUpEffect' for c in calls) == (hp > 0)
        finally:
            p.stop(save=False)
proof = {'status': 'passed', 'scope': 'Original post-hit dispatcher entered with prepared Wrap continuation operands, zero damage, clear miss flag and raging defender. Entry uses an existing RETI opcode with prepared stack to enable VBlank interrupts for HP animation. Read-only hooks observe HandleBuildingRage and StatModifierUpEffect entry or early return. Does not execute two full original turns or claim the subsequent stat animation was observed.', 'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'rom_sha256': hashlib.sha256(rom).hexdigest(), 'state_sha256': hashlib.sha256(state).hexdigest(), 'symbols_sha256': hashlib.sha256(symbols_file.read_bytes()).hexdigest(), 'cases': rows}
args.output.write_text(json.dumps(proof, indent=2) + '\n')
print(json.dumps(proof))

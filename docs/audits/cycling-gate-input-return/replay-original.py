import argparse, hashlib, io, json, os, re
from pathlib import Path
os.environ['SDL_AUDIODRIVER'] = 'dummy'
from pyboy import PyBoy

ap = argparse.ArgumentParser()
for key in ['rom', 'symbols', 'base-save', 'out']:
    ap.add_argument('--' + key, type=Path, required=True)
ap.add_argument('--gate', type=int, choices=[16, 18], required=True)
ap.add_argument('--approach', type=int, choices=range(4), default=0)
ap.add_argument('--button', choices=['start', 'up', 'down', 'left', 'right'], default='start')
ap.add_argument('--press-from', type=int, default=16)
ap.add_argument('--release-at', type=int, default=17)
a = ap.parse_args()
a.out.mkdir(parents=True, exist_ok=False)
sha = lambda b: hashlib.sha256(b).hexdigest()
rom = a.rom.read_bytes()
assert sha(rom) == '5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
syms = {}
for line in a.symbols.read_text().splitlines():
    m = re.match(r'([0-9a-f]+):([0-9a-f]+) (\S+)', line)
    if m: syms[m[3]] = (int(m[1], 16), int(m[2], 16))
def offset(name):
    bank, address = syms[name]
    return bank * 0x4000 + address % 0x4000
if a.gate == 16:
    outer, gate, start, counter_y = 27, 186, (25, 10), 7
    warps = [(10,17,0,gate),(11,17,1,gate),(10,24,2,gate),(11,24,3,gate),
             (4,17,4,gate),(5,17,5,gate),(4,24,6,gate),(5,24,7,gate),(5,7,0,188)]
else:
    outer, gate, start, counter_y = 29, 190, (41, 8), 3
    warps = [(8,33,0,gate),(9,33,1,gate),(8,40,2,gate),(9,40,3,gate)]
name = f'Route{a.gate}Gate1F'
script = f'w{name}CurScript'
data = bytearray(a.base_save.read_bytes())
def put(name, value):
    if isinstance(value, int): value = bytes([value])
    address = 0x25a3 + syms[name][1] - 0xd2f7
    assert 0x25a3 <= address and address + len(value) <= 0x3523, name
    data[address:address + len(value)] = value
header = rom[offset(f'Route{a.gate}_h'):offset(f'Route{a.gate}_h') + 10]
tileset = rom[offset('Tilesets') + header[0] * 12:offset('Tilesets') + (header[0] + 1) * 12]
x, y = start
put('wCurMap', outer); put('wXCoord', x); put('wYCoord', y)
put('wXBlockCoord', x % 2); put('wYBlockCoord', y % 2)
put('wCurrentTileBlockMapViewPointer', (syms['wOverworldMap'][1] + 7 + header[2] + (y // 2) * (header[2] + 6) + x // 2).to_bytes(2, 'little'))
put('wCurMapHeader', header); put('wTilesetBank', tileset[:11]); data[0x3522] = tileset[11]
put('wNumberOfWarps', len(warps)); put('wWarpEntries', bytes(v for w in warps for v in w))
put('wNumSprites', 0)
put('wMapMusicSoundID', rom[offset('MapSongBanks') + outer * 2:offset('MapSongBanks') + (outer + 1) * 2])
put('wMapPalOffset', 0); put('wLastMap', outer); put('wWalkBikeSurfState', 0)
put('wMovementFlags', 0); put(script, 0)
put('wNumBagItems', 0); put('wBagItems', 255)
put('wSavedSpriteImageIndex', 4); put('wPlayerMovingDirection', 0); put('wPlayerLastStopDirection', 4); put('wPlayerDirection', 4)
data[0x3523] = (~sum(data[0x2598:0x3523])) & 255
(a.out / 'fixture.sav').write_bytes(data)
p = PyBoy(str(a.rom), ram_file=io.BytesIO(data), window='null', sound_emulated=True, sound_volume=0)
p.set_emulation_speed(0)
held = None
control_start = None
recording = False
inputs, rows, hooks, menus = [], [], [], []
field = [False]
def read(name): return int(p.memory[syms[name][1]])
def state():
    return dict(frame=p.frame_count, map=read('wCurMap'), x=read('wXCoord'), y=read('wYCoord'),
                walk=read('wWalkCounter'), script=read(script), sim_index=read('wSimulatedJoypadStatesIndex'),
                joy_ignore=read('wJoyIgnore'), raw=read('hJoyInput'), last=read('hJoyLast'),
                held=read('hJoyHeld'), pressed=read('hJoyPressed'), moving=read('wPlayerMovingDirection'),
                last_stop=read('wPlayerLastStopDirection'), flags5=read('wStatusFlags5'))
def field_hook(_): field[0] = True
p.hook_register(*syms['OverworldLoop'], field_hook, None)
def observe(label):
    def hook(_):
        if recording:
            hooks.append(dict(label=label, offset=None if control_start is None else p.frame_count-control_start, **state()))
    return hook
for label in ['JoypadOverworld', '_Joypad', 'JoypadOverworld.doneSimulating', 'UpdateSprites',
              f'{name}PlayerMovingRightScript', f'{name}PlayerMovingUpScript', f'{name}GuardScript',
              'OverworldLoopLessDelay.noDirectionButtonsPressed', 'OverworldLoopLessDelay.noCollision']:
    p.hook_register(*syms[label], observe(label), None)
def menu_hook(_):
    if control_start is not None and read('hTextID') == 0: menus.append(state())
p.hook_register(*syms['DisplayTextID'], menu_hook, None)
def tick(key=None):
    global held, control_start
    if control_start is None and read('wCurMap') == gate and read(script) == 3 and read('wWalkCounter') == 7:
        control_start = p.frame_count
    if control_start is not None:
        key = a.button if a.press_from <= p.frame_count-control_start < a.release_at else None
    if key != held:
        if held: p.button_release(held)
        if key: p.button_press(key)
        held = key
    p.tick(1, False); inputs.append(key)
    if recording: rows.append(state())
def wait(n):
    for _ in range(n): tick()
def step(key):
    before = (read('wXCoord'), read('wYCoord'))
    for _ in range(80):
        tick(key)
        if (read('wXCoord'), read('wYCoord')) != before: break
    else: raise AssertionError(('blocked',key,state()))
    wait(40)
try:
    for frame in range(2400):
        if field[0] and (read('wCurMap'), read('wXCoord'), read('wYCoord')) == (outer, x, y): break
        tick('start' if frame % 60 < 2 else 'a' if frame % 60 in [20,21] else None)
    assert field[0] and (read('wCurMap'), read('wXCoord'), read('wYCoord')) == (outer, x, y), state()
    wait(64)
    for _ in range(300):
        if read('wCurMap') == gate: break
        tick('left')
    wait(100)
    assert read('wCurMap') == gate and read('wNumSprites') == (2 if a.gate == 16 else 1), state()
    entry = state()
    while read('wXCoord') > 5: step('left')
    target_y = counter_y + a.approach
    while read('wYCoord') != target_y: step('up' if read('wYCoord') > target_y else 'down')
    assert (read('wXCoord'), read('wYCoord')) == (5, target_y), state()
    recording = True
    step('left')
    wait(160)
    for frame in range(2000):
        if control_start is not None and p.frame_count - control_start >= 64: break
        tick('a' if frame % 16 < 2 else None)
    assert control_start is not None, state()
    end = state()
finally:
    p.stop(save=False)
result = dict(gate=a.gate, approach=a.approach, button=a.button, press_from=a.press_from, release_at=a.release_at,
              control_start=control_start, entry=entry, end=end, rows=rows, hooks=hooks, menus=menus, inputs=inputs,
              rom_sha256=sha(rom), fixture_sha256=sha(data), replay_sha256=sha(Path(__file__).read_bytes()),
              scope='Normal boot/CONTINUE from offline SRAM, physical gate entry and movement, read-only CPU hooks; no live RAM writes, savestates or original-font rendering.')
(a.out/'results.json').write_text(json.dumps(result,indent=2)+'\n')
later = [r for r in rows if r['frame']-control_start >= 17 and r['walk']]
print(json.dumps(dict(gate=a.gate, approach=a.approach, menus=[r['frame']-control_start for r in menus],
                     next_stride=later[0]['frame']-control_start if later else None, end=end)))

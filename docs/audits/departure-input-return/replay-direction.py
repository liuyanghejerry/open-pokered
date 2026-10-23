import argparse, hashlib, io, json, os, re
from pathlib import Path
os.environ['SDL_AUDIODRIVER'] = 'dummy'
from pyboy import PyBoy

ap = argparse.ArgumentParser()
for name in ['rom', 'symbols', 'base-save', 'out']:
    ap.add_argument('--' + name, type=Path, required=True)
ap.add_argument("--control-button", choices=["up","down","left","right"], required=True)
ap.add_argument("--press-from", type=int, default=0)
ap.add_argument("--release-at", type=int, default=46)
ap.add_argument("--fresh-start-at", type=int)
a = ap.parse_args()
a.out.mkdir(parents=True, exist_ok=True)
sha = lambda b: hashlib.sha256(b).hexdigest()
rom = a.rom.read_bytes()
assert sha(rom) == '5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
syms = {}
for line in a.symbols.read_text().splitlines():
    m = re.match(r'([0-9a-f]+):([0-9a-f]+) (\S+)', line)
    if m: syms[m[3]] = (int(m[1], 16), int(m[2], 16))
def offset(name):
    b, p = syms[name]
    return b * 0x4000 + p % 0x4000
data = bytearray(a.base_save.read_bytes())
def put(name, value):
    if isinstance(value, int): value = bytes([value])
    off = 0x25a3 + syms[name][1] - 0xd2f7
    assert 0x25a3 <= off and off + len(value) <= 0x3523, name
    data[off:off + len(value)] = value
header = rom[offset('VermilionDock_h'):offset('VermilionDock_h') + 10]
tileset = rom[offset('Tilesets') + header[0] * 12:offset('Tilesets') + (header[0] + 1) * 12]
put('wCurMap', 0x5e); put('wXCoord', 14); put('wYCoord', 1)
put('wXBlockCoord', 0); put('wYBlockCoord', 1)
put('wCurrentTileBlockMapViewPointer', (syms['wOverworldMap'][1] + 7 + header[2] + 7).to_bytes(2, 'little'))
put('wCurMapHeader', header); put('wTilesetBank', tileset[:11]); data[0x3522] = tileset[11]
put('wNumberOfWarps', 2); put('wWarpEntries', bytes([0, 14, 5, 0xff, 2, 14, 1, 0x5f]))
put('wNumSprites', 0)
put('wMapMusicSoundID', rom[offset('MapSongBanks') + 0x5e * 2:offset('MapSongBanks') + 0x5f * 2])
put('wMapPalOffset', 0); put('wLastMap', 5); put('wWalkBikeSurfState', 0)
put('wMovementFlags', 0); put('wVermilionCityCurScript', 0)
put('wSavedSpriteImageIndex', 4); put('wPlayerMovingDirection', 0); put('wPlayerLastStopDirection', 4); put('wPlayerDirection', 4)
events_off = 0x25a3 + syms['wEventFlags'][1] - 0xd2f7
for flag, enabled in [(0x5e2, True), (0x5e3, False)]:
    off = events_off + flag // 8
    if enabled: data[off] |= 1 << (flag % 8)
    else: data[off] &= ~(1 << (flag % 8))
data[0x3523] = (~sum(data[0x2598:0x3523])) & 255
(a.out / 'fixture.sav').write_bytes(data)
p = PyBoy(str(a.rom), ram_file=io.BytesIO(data), window='null', sound_emulated=True, sound_volume=0)
p.set_emulation_speed(0)
held = None
control_start = None
inputs, rows, events = [], [], []
menu_events, menu_polls, source_hooks = [], [], []
field = [False]
def read(name): return int(p.memory[syms[name][1]])
def state():
    return {'frame': p.frame_count, 'map': read('wCurMap'), 'x': read('wXCoord'), 'y': read('wYCoord'),
            'walk': read('wWalkCounter'), 'script': read('wVermilionCityCurScript'),
            'sim_index': read('wSimulatedJoypadStatesIndex'), 'joy_ignore': read('wJoyIgnore'),
            'last':read('hJoyLast'),'held':read('hJoyHeld'),'pressed':read('hJoyPressed')}
def field_hook(_): field[0] = True
p.hook_register(*syms['OverworldLoop'], field_hook, None)
def exit_hook(_): events.append(state())
p.hook_register(*syms['VermilionCityPlayerExitShipScript'], exit_hook, None)
def menu_hook(_):
    if control_start is not None and read('hTextID')==0: menu_events.append(state())
def menu_poll(_):
    if menu_events: menu_polls.append(state())
p.hook_register(*syms['DisplayTextID'],menu_hook,None)
p.hook_register(*syms['HandleMenuInput'],menu_poll,None)

def observe(label):
    def hook(_):
        if control_start is not None:
            source_hooks.append(dict(label=label, offset=p.frame_count-control_start,
                raw=read('hJoyInput'), moving=read('wPlayerMovingDirection'),
                last_stop=read('wPlayerLastStopDirection'), **state()))
    return hook
for label in ['JoypadOverworld', '_Joypad', 'VermilionCityPlayerMovingUp2Script',
              'JoypadOverworld.doneSimulating',
              'OverworldLoopLessDelay.noDirectionButtonsPressed',
              'OverworldLoopLessDelay.noCollision']:
    p.hook_register(*syms[label], observe(label), None)


def tick(key=None):
    global held, control_start
    if control_start is None and read("wCurMap")==5 and read("wWalkCounter")==7: control_start=p.frame_count
    if control_start is not None:
        elapsed=p.frame_count-control_start
        key=a.control_button if a.press_from<=elapsed<a.release_at or (a.fresh_start_at is not None and a.fresh_start_at<=elapsed<a.fresh_start_at+2) else None
    if key != held:
        if held: p.button_release(held)
        if key: p.button_press(key)
        held = key
    p.tick(1, False); inputs.append(key)
    if read('wCurMap') == 5 and read('wXCoord') in [18, 19] and 29 <= read('wYCoord') <= 31:
        rows.append(state())
try:
    for t in range(2400):
        if field[0] and read('wCurMap') == 0x5e: break
        tick('start' if t % 60 < 2 else 'a' if t % 60 in [20, 21] else None)
    assert field[0] and (read('wCurMap'), read('wXCoord'), read('wYCoord')) == (0x5e, 14, 1), state()
    assert read('wNumSprites') == 0
    for _ in range(64): tick()
    for _ in range(500):
        if read('wCurMap') == 5: break
        tick('up')
    for _ in range(120): tick()
    end = state()
    assert end['map'] == 5, end
    assert len(events) == 1, events
finally: p.stop(save=False)
result = {'source_hooks':source_hooks, 'button':a.control_button, 'press_from':a.press_from, 'release_at':a.release_at, 'fresh_start_at':a.fresh_start_at, 'rom_sha256': sha(rom), 'fixture_sha256': sha(data), 'normal_continue_and_physical_exit': True,
          'runtime_memory_writes': False, 'render_original_font': False, 'inputs': inputs, 'events': events, 'rows': rows, 'end': end, 'menu_events':menu_events, 'menu_polls':menu_polls}
(a.out / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
first = next(i for i, row in enumerate(rows) if row['walk'])
print(json.dumps({'end': end, 'trace': [[row['frame'] - rows[first]['frame'], row['walk'], row['y'], row['script'], row['sim_index'], row['joy_ignore']] for row in rows[first:first+45]]}))

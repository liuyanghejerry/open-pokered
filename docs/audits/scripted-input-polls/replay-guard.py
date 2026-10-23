import io, os, re, json, hashlib, argparse
from pathlib import Path
os.environ['SDL_AUDIODRIVER'] = 'dummy'
from pyboy import PyBoy
parser = argparse.ArgumentParser()
for name in ["rom", "symbols", "base-save", "out"]:
    parser.add_argument("--" + name, type=Path, required=True)
parser.add_argument("--press-from", type=int, default=0)
parser.add_argument("--release-at", type=int, default=30)
args = parser.parse_args()
OUT = args.out
OUT.mkdir(parents=True, exist_ok=True)
rom = args.rom
base = args.base_save
sha = lambda b: hashlib.sha256(b).hexdigest()
rom_bytes = rom.read_bytes()
assert sha(rom_bytes) == '5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b'
symbols = {}
for line in args.symbols.read_text().splitlines():
    m = re.match(r'([0-9a-f]+):([0-9a-f]+) (\S+)', line)
    if m: symbols[m[3]] = (int(m[1],16), int(m[2],16))
def offset(name):
    b,a = symbols[name]
    return b*0x4000 + a%0x4000
def fixture(x,badge):
    data = bytearray(base.read_bytes())
    def put(name, values):
        if isinstance(values,int): values = bytes([values])
        n = 0x25a3 + symbols[name][1] - 0xd2f7
        data[n:n+len(values)] = values
    header = rom_bytes[offset('Route22_h'):offset('Route22_h')+10]
    tileset = rom_bytes[offset('Tilesets')+header[0]*12:offset('Tilesets')+(header[0]+1)*12]
    put('wCurMap',33); put('wYCoord',6); put('wXCoord',8)
    put('wYBlockCoord',0); put('wXBlockCoord',0)
    put('wCurrentTileBlockMapViewPointer',(symbols['wOverworldMap'][1]+7+header[2]+3*(header[2]+6)+4).to_bytes(2,'little'))
    put('wCurMapHeader',header); put('wTilesetBank',tileset[:11]); data[0x3522]=tileset[11]
    # Route22_Object's one ordinary doorway; actual entry loads the gate's
    # complete original NPC/warp data instead of reusing the source save's.
    put('wNumberOfWarps',1); put('wWarpEntries',bytes([5,8,0,193]))
    put('wMapMusicSoundID',rom_bytes[offset('MapSongBanks')+33*2:offset('MapSongBanks')+34*2])
    put('wMapPalOffset',0); put('wLastMap',33); put('wWalkBikeSurfState',0)
    put('wObtainedBadges',int(badge)); put('wRoute22GateCurScript',0)
    put('wSavedSpriteImageIndex',4); put('wPlayerMovingDirection',0); put('wPlayerLastStopDirection',4)
    put('wPlayerDirection',4)
    data[0x3523] = (~sum(data[0x2598:0x3523])) & 255
    return bytes(data)
results=[]
cases = [(4,False,False)]
for x,badge,direct in cases:
    name=f'{"badge" if badge else "no-badge"}-{x}-{"talk" if direct else "cross"}'
    data=fixture(x,badge); (OUT/('badge.sav' if badge else 'no-badge.sav')).write_bytes(data)
    p=PyBoy(str(rom),ram_file=io.BytesIO(data),window='null',sound_emulated=True,sound_volume=0)
    p.set_emulation_speed(0)
    held=None; inputs=[]; events=[]; field=[False]; movement=[]; control_start=None; menu_events=[]; menu_polls=[]
    def read(n): return int(p.memory[symbols[n][1]])
    def state(): return {'frame':p.frame_count,'map':read('wCurMap'),'x':read('wXCoord'),'y':read('wYCoord'),'script':read('wRoute22GateCurScript'),'facing':read('wSpritePlayerStateData1FacingDirection'),'walk':read('wWalkCounter'),'last':read('hJoyLast'),'held':read('hJoyHeld'),'pressed':read('hJoyPressed'),'joy_ignore':read('wJoyIgnore'),'sim_index':read('wSimulatedJoypadStatesIndex')}
    def hook(label):
        def record(_): events.append({'label':label,**state()})
        return record
    def field_hook(_): field[0]=True
    p.hook_register(*symbols['OverworldLoop'],field_hook,None)
    bank,addr=symbols['Route22GateGuardText']
    assert rom_bytes[bank*0x4000+addr%0x4000:bank*0x4000+addr%0x4000+4]==bytes([8,0xfa,0x56,0xd3])
    p.hook_register(bank,addr+1,hook('Route22GateGuardText'),None)
    p.hook_register(*symbols['Route22GateMovePlayerDownScript'],hook('Route22GateMovePlayerDownScript'),None)
    def print_hook(_):
        ptr=p.register_file.HL
        for label in ['Route22GateGuardGoRightAheadText','Route22GateGuardNoBoulderbadgeText']:
            if ptr==symbols[label][1] and read('wCurMap')==193:hook(label)(None)
    p.hook_register(*symbols['PrintText'],print_hook,None)
    def menu_hook(_):
        if control_start is not None and read('hTextID')==0: menu_events.append(state())
    def menu_poll(_):
        if menu_events: menu_polls.append(state())
    p.hook_register(*symbols['DisplayTextID'],menu_hook,None)
    p.hook_register(*symbols['HandleMenuInput'],menu_poll,None)

    def tick(key=None):
        global held, control_start
        if control_start is None and read("wCurMap")==193 and read("wRoute22GateCurScript")==1 and read("wWalkCounter")==7: control_start=p.frame_count
        if control_start is not None: key="start" if args.press_from<=p.frame_count-control_start<args.release_at else None
        if key!=held:
            if held:p.button_release(held)
            if key:p.button_press(key)
            held=key
        p.tick(1,False); inputs.append(key)
        if read("wCurMap")==193 and (read("wRoute22GateCurScript")==1 or (control_start is not None and p.frame_count-control_start<40)): movement.append(state())
    def wait(n):
        for _ in range(n):tick()
    try:
        for i in range(2400):
            if field[0] and (read('wCurMap'),read('wXCoord'),read('wYCoord'))==(33,8,6):break
            tick('start' if i%60<2 else 'a' if i%60 in [20,21] else None)
        assert field[0] and (read('wCurMap'),read('wXCoord'),read('wYCoord'))==(33,8,6),state()
        wait(64)
        for _ in range(300):
            if read('wCurMap')==193:break
            tick('up')
        wait(100)
        assert read('wCurMap')==193,state()
        assert read('wNumSprites')==1, 'normal doorway entry must load the original gate NPC'
        gate_entry=state()
        def step(key):
            xy=(read('wXCoord'),read('wYCoord'))
            for _ in range(80):
                tick(key)
                if (read('wXCoord'),read('wYCoord'))!=xy:break
            else: raise AssertionError(('no step',key,state()))
            wait(40)
        while read('wYCoord')>3:step('up')
        while read('wXCoord')<x:step('right')
        if direct:
            for _ in range(8):tick('up')
            wait(32)
            assert (read('wXCoord'),read('wYCoord'))==(x,3),state()
        initial=state()
        for _ in range(8): tick('a' if direct else 'up')
        wait(180); before_close=state()
        for i in range(500):tick('a' if i%16<2 else None)
        wait(64); end=state()
        assert any(e['label']=='Route22GateGuardText' for e in events),(name,events,end)
        assert (end['x'],end['y']) == (x, 4 if direct else 3),(name,events,end)
        assert end['script'] == (2 if badge else 0),(name,events,end)
        later=None
        results.append({'name':name,'fixture_sha256':sha(data),'gate_entry':gate_entry,'initial':initial,'before_close':before_close,'end':end,'later':later,'events':events,'inputs':inputs,'movement':movement,'menu_events':menu_events,'menu_polls':menu_polls})
        print({'case':name,'end':end,'events':events},flush=True)
    finally:p.stop(save=False)
(OUT/'results.json').write_text(json.dumps({'button':'start','press_from':args.press_from,'release_at':args.release_at,'scope':'Offline prepared SRAM fixtures, normal boot/CONTINUE, physical inputs, read-only hooks, no rendering, not fresh travel.','rom_sha256':sha(rom_bytes),'base_fixture_sha256':sha(base.read_bytes()),'symbols_sha256':sha(args.symbols.read_bytes()),'replay_sha256':sha(Path(__file__).read_bytes()),'source_ref':'pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c','results':results},indent=2)+'\n')

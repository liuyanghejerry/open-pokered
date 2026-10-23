import io, os, re, json, hashlib, argparse
from pathlib import Path
os.environ['SDL_AUDIODRIVER'] = 'dummy'
from pyboy import PyBoy
parser = argparse.ArgumentParser()
for name in ["rom", "symbols", "base-save", "out"]:
    parser.add_argument("--" + name, type=Path, required=True)
parser.add_argument("--mode", choices=["walking", "biking"], required=True)
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

mode=int(args.mode == "biking")
assert mode in (0,1)
data=bytearray(fixture(4,True))
def put_saved(name,values):
    if isinstance(values,int):values=bytes([values])
    off=0x25a3+symbols[name][1]-0xd2f7
    data[off:off+len(values)]=values
put_saved('wNumBagItems',1);put_saved('wBagItems',bytes([6,1,255]))
data[0x3523]=(~sum(data[0x2598:0x3523]))&255
(OUT/'fixture.sav').write_bytes(data)
p=PyBoy(str(rom),ram_file=io.BytesIO(data),window='null',sound_emulated=True,sound_volume=0)
p.set_emulation_speed(0)
held=None;inputs=[];rows=[];events=[];field=[0];moving=[False];first=[None]
def read(n):return int(p.memory[symbols[n][1]])
def state():return {'frame':p.frame_count,'map':read('wCurMap'),'x':read('wXCoord'),'y':read('wYCoord'),'walk':read('wWalkCounter'),'mode':read('wWalkBikeSurfState'),'script':read('wRoute23CurScript'),'npc_script':read('wNPCMovementScriptPointerTableNum'),'sim_index':read('wSimulatedJoypadStatesIndex')}
def field_hook(_):field[0]+=1
def move_hook(_):moving[0]=True;events.append(state())
p.hook_register(*symbols['OverworldLoop'],field_hook,None)
p.hook_register(*symbols['Route23MovePlayerDownScript'],move_hook,None)
def tick(key=None):
    global held
    if moving[0] and read('wWalkCounter')>0 and first[0] is None:first[0]=state()
    if first[0] is not None:key=None
    if key!=held:
        if held:p.button_release(held)
        if key:p.button_press(key)
        held=key
    p.tick(1,False);inputs.append(key)
    if first[0] is not None:rows.append(state())
def wait(n):
    for _ in range(n):tick()
def press(key,n=4,settle=60):
    for _ in range(n):tick(key)
    wait(settle)
try:
    for i in range(2400):
        if field[0] and (read('wCurMap'),read('wXCoord'),read('wYCoord'))==(33,8,6):break
        tick('start' if i%60<2 else 'a' if i%60 in [20,21] else None)
    assert field[0] and read('wCurMap')==33,state()
    wait(64)
    for _ in range(300):
        if read('wCurMap')==193:break
        tick('up')
    wait(100)
    for i in range(3000):
        if read('wCurMap')==34:break
        tick('a' if i%16<2 else 'up')
    wait(100)
    assert (read('wCurMap'),read('wYCoord'))==(34,139),state()
    assert read('wNumSprites')==7,state()
    if mode:
        before=field[0];press('start',settle=100)
        print('start menu',read('wCurrentMenuItem'),read('wMaxMenuItem'),flush=True)
        item_index=2 if read('wMaxMenuItem')>=6 else 1
        for _ in range(item_index):press('down',settle=15)
        press('a',settle=100);press('a',settle=100);press('a',settle=100)
        for _ in range(3):press('b',settle=30)
        assert read('wWalkBikeSurfState')==1,state()
        assert field[0]>before,state()
    while read('wYCoord')>137:
        y=read('wYCoord')
        for _ in range(80):
            tick('up')
            if read('wYCoord')!=y:break
        else:raise AssertionError(('no step',state()))
        wait(30)
    for _ in range(10):tick('up')
    wait(181)
    for i in range(3000):
        if first[0] is not None:break
        tick('a' if i%8==0 else None)
    assert first[0] is not None,(state(),events)
    wait(40)
    assert (read('wCurMap'),read('wXCoord'),read('wYCoord'),read('wWalkBikeSurfState'))==(34,7,137,mode),state()
finally:p.stop(save=False)
result={'mode':mode,'normal_continue_and_physical_gates':True,'bicycle_via_bag_buttons':bool(mode),'live_memory_writes':False,'original_font_rendering':False,'fixture_sha256':sha(data),'first':first[0],'rows':rows,'events':events,'inputs':inputs}
(OUT/'results.json').write_text(json.dumps(result,indent=2)+'\n')
print('stride',[[0,first[0]['walk']]]+[[r['frame']-first[0]['frame'],r['walk']] for r in rows[:18]],flush=True)

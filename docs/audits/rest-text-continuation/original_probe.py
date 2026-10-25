#!/usr/bin/env python3
"""Prepared original healthy Rest: observe whether sleep text requires an ACK."""
import argparse, hashlib, importlib.metadata, io, json, os
from pathlib import Path
os.environ.setdefault('SDL_AUDIODRIVER', 'dummy')
from pyboy import PyBoy

parser=argparse.ArgumentParser(description=__doc__)
for name in ['rom','state','symbols','output']:parser.add_argument('--'+name,type=Path,required=True)
a=parser.parse_args();rom_path=a.rom;state_path=a.state;sym_path=a.symbols
symbols={}
for line in sym_path.read_text().splitlines():
    parts=line.split()
    if len(parts)==2 and ':' in parts[0]:
        bank,pc=parts[0].split(':');symbols[parts[1]]=(int(bank,16),int(pc,16))
rom=rom_path.read_bytes();saved=state_path.read_bytes();rows=[]
for side in [0,1]:
    p=PyBoy(io.BytesIO(rom),window='null',sound_emulated=False);p.set_emulation_speed(0)
    seen=[];scroll=[];sleep=[]
    try:
        p.load_state(io.BytesIO(saved))
        def put(key,value):p.memory[symbols[key][1]]=value
        def word(key,value):
            pc=symbols[key][1];p.memory[pc]=value>>8;p.memory[pc+1]=value&255
        def read_word(key):
            pc=symbols[key][1];return p.memory[pc]*256+p.memory[pc+1]
        put('wLinkState',0);put('wIsInBattle',1);put('hWhoseTurn',side);put('wOptions',1)
        for key in ['hJoyHeld','hJoyPressed','hJoyReleased','hJoyInput']:put(key,0)
        for owner in ['Player','Enemy']:
            for n in [1,2,3]:put('w'+owner+'BattleStatus'+str(n),0)
        owner='Enemy' if side else 'Player';mon='wEnemyMon' if side else 'wBattleMon'
        put('w'+owner+'MoveNum',156);put('w'+owner+'MoveEffect',56)
        word(mon+'HP',600);word(mon+'MaxHP',630);put(mon+'Status',0)
        for key in ['wBattleMonNick','wEnemyMonNick']:
            pc=symbols[key][1];p.memory[pc:pc+11]=[0x80,0x50]+[0x50]*9
        p.hook_register(*symbols['ManualTextScroll'],lambda _:scroll.append(frame),None)
        p.hook_register(*symbols['HealEffect_.printRestText'],lambda _:sleep.append(frame),None)
        def animation(_):
            if not seen:seen.append({'frame':frame,'hp':read_word(mon+'HP'),'status':p.memory[symbols[mon+'Status'][1]],'scroll_calls_before_animation':len(scroll)})
        p.hook_register(*symbols['PlayCurrentMoveAnimation'],animation,None)
        bank,pc=symbols['HealEffect'];p.memory[0x2000]=bank;put('hLoadedROMBank',bank)
        reti=symbols['DelayFrame'][1]-1;assert rom[reti]==0xd9
        sp=p.register_file.SP-4;ret=symbols['SoftReset'][1]
        p.memory[sp:sp+4]=[pc&255,pc>>8,ret&255,ret>>8];p.register_file.SP=sp;p.register_file.PC=reti
        for frame in range(500):
            p.tick(1,render=False,sound=False)
            if seen:break
        assert seen,(side,p.register_file.PC,sleep,scroll)
        assert seen[0]['hp']==630 and seen[0]['status']==2 and seen[0]['scroll_calls_before_animation']==0,seen
        rows.append({'side':side,'sleep_text_frames':sleep,'animation_entry':seen[0],'input_events':[]})
    finally:p.stop(save=False)
sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
record={'status':'passed','scope':'Two prepared original healthy Rest calls through PrintText to animation entry, no input. No complete turn, actual image or cross-runtime frame timing claim.',
        'rom_sha256':sha(rom_path),'state_sha256':sha(state_path),'symbols_sha256':sha(sym_path),'source_sha256':sha(Path(__file__)),
        'pyboy':importlib.metadata.version('pyboy'),'cases':rows}
a.output.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record))

#!/usr/bin/env python3
"""Observe original protective-effect success/duplicate guards before animation."""
import argparse,hashlib,importlib.metadata,io,json,os
from pathlib import Path
os.environ.setdefault('SDL_AUDIODRIVER','dummy')
from pyboy import PyBoy
parser=argparse.ArgumentParser(description=__doc__)
for name in ['rom','symbols','state','output']:parser.add_argument('--'+name,type=Path,required=True)
a=parser.parse_args();symbols={}
for line in a.symbols.read_text().splitlines():
 words=line.split()
 if len(words)==2 and ':' in words[0]:
  bank,pc=words[0].split(':');symbols[words[1]]=(int(bank,16),int(pc,16))
rom=a.rom.read_bytes();saved=a.state.read_bytes();rows=[]
# Effect numbers come from constants/move_effect_constants.asm.
cases=[('Reflect','ReflectLightScreenEffect','ReflectLightScreenEffect_.moveFailed',115,65,3,4),
 ('LightScreen','ReflectLightScreenEffect','ReflectLightScreenEffect_.moveFailed',113,64,3,2),
 ('Mist','MistEffect','MistEffect_.mistAlreadyInUse',54,46,2,2),
 ('FocusEnergy','FocusEnergyEffect','FocusEnergyEffect_.alreadyUsing',116,47,2,4)]
for name,entry,veto,move_id,effect,byte,bit in cases:
 for side in [0,1]:
  for condition,flags in [('absent',0),('already_active',bit),('unrelated_flag',128),('opponent_active',0)]:
   p=PyBoy(io.BytesIO(rom),window='null',sound_emulated=False);p.set_emulation_speed(0);seen=[];draws=[]
   try:
    p.load_state(io.BytesIO(saved))
    def put(name,value):p.memory[symbols[name][1]]=value
    put('wLinkState',4);put('wLinkBattleRandomNumberListIndex',0);put('hWhoseTurn',side)
    owner='Enemy' if side else 'Player';status='w'+owner+'BattleStatus'+str(byte)
    for actor in ['Player','Enemy']:
     put('w'+actor+'MoveNum',move_id);put('w'+actor+'MoveEffect',effect)
     for n in [1,2,3]:put('w'+actor+'BattleStatus'+str(n),0)
    put(status,flags)
    if condition=='opponent_active':put('w'+('Player' if side else 'Enemy')+'BattleStatus'+str(byte),bit)
    p.hook_register(*symbols['BattleRandom'],lambda _:draws.append(1),None)
    def finish(branch):
     def observe(_):
      if not seen:seen.append({'branch':branch,'flags':p.memory[symbols[status][1]],'draws':len(draws)})
     return observe
    p.hook_register(*symbols['PlayCurrentMoveAnimation'],finish('success_before_animation'),None)
    p.hook_register(*symbols[veto],finish('duplicate_veto'),None)
    bank,pc=symbols[entry];p.memory[0x2000]=bank;put('hLoadedROMBank',bank)
    sp=p.register_file.SP-2;ret=symbols['SoftReset'][1];p.memory[sp]=ret&255;p.memory[sp+1]=ret>>8;p.register_file.SP=sp;p.register_file.PC=pc
    for _ in range(3):
     p.tick(1,render=False,sound=False)
     if seen:break
    assert len(seen)==1,(name,side,condition,p.register_file.PC)
    expected={'branch':'duplicate_veto' if condition=='already_active' else 'success_before_animation','flags':flags|bit,'draws':0}
    assert seen[0]==expected,(name,side,condition,seen[0],expected)
    rows.append({'move':name,'side':side,'condition':condition,'initial_flags':flags,'result':seen[0]})
   finally:p.stop(save=False)
record={'status':'passed','scope':'32 prepared original protective-effect calls. Read-only hooks observe duplicate veto or successful flag set at animation entry; no original full-turn, animation, caption timing, or later Haze execution claim.','source_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'rom_sha256':hashlib.sha256(rom).hexdigest(),'state_sha256':hashlib.sha256(saved).hexdigest(),'symbols_sha256':hashlib.sha256(a.symbols.read_bytes()).hexdigest(),'pyboy':importlib.metadata.version('pyboy'),'cases':rows}
a.output.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record))

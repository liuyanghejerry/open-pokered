#!/usr/bin/env python3
"""Observe original PoisonEffect eligibility and RNG before final-hit poison."""
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
cases=[('eligible',0,0,0,False),('burned',16,0,0,False),('poisoned',8,0,0,False),('paralyzed',64,0,0,False),('asleep',7,0,0,False),('frozen',32,0,0,False),('poison_type1',0,3,0,False),('poison_type2',0,0,3,False),('substitute',0,0,0,True)]
for side in [0,1]:
 for name,status,type1,type2,substitute in cases:
  for raw in [51,52]:
   p=PyBoy(io.BytesIO(rom),window='null',sound_emulated=False);p.set_emulation_speed(0);seen=[];draws=[]
   try:
    p.load_state(io.BytesIO(saved))
    def put(name,value):p.memory[symbols[name][1]]=value
    put('wLinkState',4);put('wLinkBattleRandomNumberListIndex',0);put('hWhoseTurn',side)
    for i in range(10):p.memory[symbols['wLinkBattleRandomNumberList'][1]+i]=raw
    for owner in ['Player','Enemy']:
     put('w'+owner+'MoveNum',41) # Twineedle; final dispatcher uses POISON_SIDE_EFFECT1.
     put('w'+owner+'MoveEffect',2)
     for n in [1,2,3]:put('w'+owner+'BattleStatus'+str(n),0)
    target='wBattleMon' if side else 'wEnemyMon'
    put(target+'Status',status);put(target+'Type1',type1);put(target+'Type2',type2)
    if substitute:put('w'+('Player' if side else 'Enemy')+'BattleStatus2',16)
    def rng(_):draws.append(p.memory[symbols['wLinkBattleRandomNumberListIndex'][1]])
    p.hook_register(*symbols['BattleRandom'],rng,None)
    def finish(branch):
     def observe(_):
      if not seen:seen.append({'branch':branch,'draws':len(draws),'draw_indices':list(draws),'target_status_before_infliction':p.memory[symbols[target+'Status'][1]]})
     return observe
    for label,branch in [('PoisonEffect.inflictPoison','inflict'),('PoisonEffect.noEffect','precondition_veto'),('SoftReset','chance_rejected')]:p.hook_register(*symbols[label],finish(branch),None)
    bank,pc=symbols['PoisonEffect'];p.memory[0x2000]=bank;put('hLoadedROMBank',bank)
    sp=p.register_file.SP-2;ret=symbols['SoftReset'][1];p.memory[sp]=ret&255;p.memory[sp+1]=ret>>8;p.register_file.SP=sp;p.register_file.PC=pc
    for _ in range(3):
     p.tick(1,render=False,sound=False)
     if seen:break
    assert len(seen)==1,(side,name,raw)
    rows.append({'side':side,'case':name,'raw':raw,'prepared_status':status,'types':[type1,type2],'substitute':substitute,'result':seen[0]})
   finally:p.stop(save=False)
record={'status':'passed','scope':'Prepared original PoisonEffect entry with POISON_SIDE_EFFECT1 as used by Twineedle final dispatch. Read-only hooks stop the observation at infliction entry, a precondition veto, or rejected-chance return. Confirms guard/RNG ordering only; no complete original multi-hit turn, second-hit doll-break dispatch, animation, narration or poison residual claim.','reference':'pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c engine/battle/effects.asm78-118','source_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'rom_sha256':hashlib.sha256(rom).hexdigest(),'state_sha256':hashlib.sha256(saved).hexdigest(),'symbols_sha256':hashlib.sha256(a.symbols.read_bytes()).hexdigest(),'pyboy':importlib.metadata.version('pyboy'),'cases':rows}
a.output.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'cases':len(rows),'outcomes':[{k:v for k,v in x.items() if k in ['side','case','raw','result']} for x in rows]}))

#!/usr/bin/env python3
"""Observe the original ordinary-move calculation pipeline with prepared RAM.
Requires PyBoy and matching ROM, symbols and state; no ROM is distributed.
"""
import argparse, hashlib, importlib.metadata, io, json, os
from pathlib import Path
os.environ.setdefault('SDL_AUDIODRIVER', 'dummy')
from pyboy import PyBoy
parser=argparse.ArgumentParser(description=__doc__)
for name in ['rom','symbols','state','output']:parser.add_argument('--'+name,type=Path,required=True)
a=parser.parse_args();symbols={}
for line in a.symbols.read_text().splitlines():
 words=line.split()
 if len(words)==2 and ':' in words[0]:
  bank,pc=words[0].split(':');symbols[words[1]]=(int(bank,16),int(pc,16))
rom=a.rom.read_bytes();state=a.state.read_bytes();rows=[]
cases=[('variance_then_miss',[255,179,255,0,255,255,255,255,255,255],False),('maximum_then_hit',[255,255,0,255,255,255,255,255,255,255],False),('minimum_then_hit',[255,179,0,255,255,255,255,255,255,255],False),('immune_control',[255,179,0,255,255,255,255,255,255,255],True)]
cases += [(name,[255,179,0,255,255,255,255,255,255,255],name=='immune_x_accuracy') for name in ['invulnerable_target','x_accuracy','swift','swift_invulnerable','x_accuracy_invulnerable','immune_x_accuracy']]
cases += [(move+'_'+outcome, [255,179,255 if outcome=='miss' else 0,255,255,255,255,255,255,255],False) for move in ['fly','dig'] for outcome in ['hit','miss']]
cases += [(name,[255,179,0,255,255,255,255,255,255,255],False) for name in ['dream_awake','dream_asleep']]
for side in [0,1]:
 for name,seeds,immune in cases:
  p=PyBoy(io.BytesIO(rom),window='null',sound_emulated=False);p.set_emulation_speed(0);trace=[];seen=[]
  try:
   p.load_state(io.BytesIO(state))
   def put(name,value):p.memory[symbols[name][1]]=value
   def word(name,value):
    addr=symbols[name][1];p.memory[addr]=value>>8;p.memory[addr+1]=value&255
   put('wLinkState',4);put('wLinkBattleRandomNumberListIndex',0);put('hWhoseTurn',side);put('wIsInBattle',1)
   for i,x in enumerate(seeds):p.memory[symbols['wLinkBattleRandomNumberList'][1]+i]=x
   for name0 in ['wMoveMissed','wCriticalHitOrOHKO','wDamageMultipliers','wPlayerMonNumber','wEnemyMonPartyPos']:put(name0,0)
   word('wDamage',0)
   move_num,effect,power,accuracy=(129,17,60,255) if name.startswith('swift') else (33,0,35,242)
   move_type=0
   if name.startswith('fly_'):move_num,effect,power,accuracy,move_type=19,43,70,242,2
   if name.startswith('dig_'):move_num,effect,power,accuracy,move_type=91,39,100,255,4
   if name.startswith('dream_'):move_num,effect,power,accuracy,move_type=138,8,100,255,24
   for owner in ['Player','Enemy']:
    for n in [1,2,3]:put('w'+owner+'BattleStatus'+str(n),0)
    for stat in ['Attack','Defense','Speed','Special','Accuracy','Evasion']:put('w'+owner+'Mon'+stat+'Mod',7)
    for field,x in [('Num',move_num),('Effect',effect),('Power',power),('Type',move_type),('Accuracy',accuracy),('MaxPP',35)]:put('w'+owner+'Move'+field,x)
    put('w'+owner+'SelectedMove',move_num)
   for index,prefix in enumerate(['wBattleMon','wEnemyMon']):
    actor=index==side
    put(prefix+'Species',131 if actor else 132);put(prefix+'Level',100);put(prefix+'Status',0)
    word(prefix+'HP',999);word(prefix+'MaxHP',999)
    for field,x in [('Attack',203),('Defense',30),('Speed',100 if actor else 200),('Special',100)]:word(prefix+field,x)
    for n in [1,2]:put(prefix+'Type'+str(n),24 if actor else 8 if immune else 0)
   if name=='dream_asleep':put(('wBattleMon' if side else 'wEnemyMon')+'Status',7)
   if 'invulnerable' in name:put('w'+('Player' if side else 'Enemy')+'BattleStatus1',1<<6)
   if 'x_accuracy' in name:put('w'+('Enemy' if side else 'Player')+'BattleStatus2',1)
   def observe_phase(label):
    def observe(_):trace.append({'phase':label,'index':p.memory[symbols['wLinkBattleRandomNumberListIndex'][1]]})
    return observe
   for label in ['CriticalHitTest','CalculateDamage','AdjustDamageForMoveType','RandomizeDamage','MoveHitTest']:
    p.hook_register(*symbols[label],observe_phase(label),None)
   def rng(_):
    idx=p.memory[symbols['wLinkBattleRandomNumberListIndex'][1]];trace.append({'phase':'BattleRandom','index':idx,'raw':p.memory[symbols['wLinkBattleRandomNumberList'][1]+idx]})
   p.hook_register(*symbols['BattleRandom'],rng,None)
   def reached(_):
    if not seen:
     addr=symbols['wDamage'][1];seen.append({'damage':p.memory[addr]*256+p.memory[addr+1],'missed':p.memory[symbols['wMoveMissed'][1]],'critical':p.memory[symbols['wCriticalHitOrOHKO'][1]],'draws':sum(x['phase']=='BattleRandom' for x in trace),'trace':list(trace)})
   p.hook_register(*symbols['HandleIfEnemyMoveMissed' if side else 'HandleIfPlayerMoveMissed'],reached,None)
   bank,pc=symbols['EnemyCalcMoveDamage' if side else 'PlayerCalcMoveDamage'];p.memory[0x2000]=bank;put('hLoadedROMBank',bank)
   sp=p.register_file.SP-2;ret=symbols['SoftReset'][1];p.memory[sp]=ret&255;p.memory[sp+1]=ret>>8;p.register_file.SP=sp;p.register_file.PC=pc
   for _ in range(3):
    p.tick(1,render=False,sound=False)
    if seen:break
   assert len(seen)==1,(side,name,trace)
   assert [x['phase'] for x in seen[0]['trace'] if x['phase']!='BattleRandom']==['CriticalHitTest','CalculateDamage','AdjustDamageForMoveType','RandomizeDamage','MoveHitTest']
   rows.append({'side':side,'case':name,'seeds':seeds,'immune':immune,'result':seen[0]})
  finally:p.stop(save=False)
proof={'status':'passed','scope':'Original PlayerCalcMoveDamage/EnemyCalcMoveDamage through return from MoveHitTest, with prepared move/mon/status/stat/modifier/link RAM, registers and stack. Read-only hooks observe the pipeline before animation. Not public inputs, complete original turns, narration, HP application or timing.','source_reference':'pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c engine/battle/core.asm3134-3148 and5517-5536','source_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'rom_sha256':hashlib.sha256(rom).hexdigest(),'state_sha256':hashlib.sha256(state).hexdigest(),'symbols_sha256':hashlib.sha256(a.symbols.read_bytes()).hexdigest(),'pyboy':importlib.metadata.version('pyboy'),'cases':rows}
a.output.write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps({'status':'passed','cases':[{k:v for k,v in row.items() if k!='seeds'} for row in rows]}))

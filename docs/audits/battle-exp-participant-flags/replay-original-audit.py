from pathlib import Path
import tempfile,shutil,hashlib
exec(open('/home/jerry/develop/open-pokered/.task-tmp/fidelity-39-plus/stats-59-tools/record_retail_stats.py').read().split('for trial in [1,2]:')[0])
import sys
case=sys.argv[1] if len(sys.argv)>1 else 'no-revive';repeat=int(sys.argv[2]) if len(sys.argv)>2 else 1
r=root/'.task-tmp/fidelity-39-plus';prepared=Path((r/'audit-863-original-forest-map-setup-path.txt').read_text().strip());out=Path(tempfile.mkdtemp(prefix=f'audit-867-original-faint-exp-{case}-{repeat}-',dir=r));rom=r/'movement-91-setup/walking-reference.gbc';private=out/'original.gbc';shutil.copyfile(rom,private);p=PyBoy(str(private),window='null',sound_emulated=True,color_palette=[0xffffff,0xaaaaaa,0x555555,0]);p.set_emulation_speed(0);p.load_state(io.BytesIO((prepared/'prepared-map.state').read_bytes()));prior=Path((r/'audit-860-original-tmhm-full-path.txt').read_text().strip());base=json.loads((prior/'tm-replace-1/record.json').read_text())['initial']['raw'];lead=base.copy();lead[1:3]=[0,1];lead[3]=lead[33]=5;lead[8:12]=[33,45,0,0];lead[12:14]=[0,0];lead[14:17]=[0,0,135];lead[17:27]=[0]*10;lead[29:33]=[35,40,0,0];lead[34:44]=[0,20,0,10,0,10,0,10,0,12];healthy=lead.copy();healthy[1:3]=[0,52];healthy[3]=healthy[33]=20;healthy[8:12]=[15,34,36,38];healthy[14:17]=[0,21,84];healthy[29:33]=[30,15,20,15];healthy[34:44]=[0,52,0,28,0,28,0,25,0,34];party=symbols['wPartyMons'][1];p.memory[party:party+88]=lead+healthy if case in ('no-revive','revive','revive-next') else healthy+lead;p.memory[symbols['wPartyCount'][1]]=2;p.memory[symbols['wPartySpecies'][1]:symbols['wPartySpecies'][1]+7]=[153,153,255,255,255,255,255];nick=[0x80+ord(c)-ord('A') for c in 'BULBASAUR']+[0x50]*2
for i in range(2):p.memory[symbols['wPartyMonNicks'][1]+11*i:symbols['wPartyMonNicks'][1]+11*(i+1)]=nick
p.memory[symbols['wPlayerID'][1]:symbols['wPlayerID'][1]+2]=[0,0];p.memory[symbols['wOptions'][1]]=0x41;p.memory[symbols['wObtainedBadges'][1]]=0;p.memory[symbols['wNumBagItems'][1]]=1 if case in ('revive','revive-next') else 0;p.memory[symbols['wBagItems'][1]:symbols['wBagItems'][1]+3]=[0x35,1,255] if case in ('revive','revive-next') else [255,0,0];p.memory[symbols['wPlayerMoney'][1]:symbols['wPlayerMoney'][1]+3]=[0,0x30,0];p.save_state((out/'constructed-start.state').open('wb'));actions=[];events=[];states=[];get=lambda key:int(p.memory[symbols[key][1]])
def snapshot():
 mons=[list(p.memory[party+i*44:party+(i+1)*44]) for i in range(2)];return dict(frame=p.frame_count,in_battle=get('wIsInBattle'),active=get('wPlayerMonNumber'),gain=get('wPartyGainExpFlags'),fought=get('wPartyFoughtCurrentEnemyFlags'),party_hp=[(m[1]<<8)|m[2] for m in mons],party_exp=[(m[14]<<16)|(m[15]<<8)|m[16] for m in mons],x=get('wXCoord'),y=get('wYCoord'),party=mons,cursor=get('wCurrentMenuItem'),menu_max=get('wMaxMenuItem'))
for label in ['DisplayBattleMenu','ChooseNextMon','GainExperience','RemoveFaintedPlayerMon','FaintEnemyPokemon','TrainerBattleVictory','EnemySendOut','EnemySendOutFirstMon','MainInBattleLoop','DrawStartMenu','SavePartyAndDexData']:
 if label in symbols:p.hook_register(*symbols[label],lambda _,label=label:events.append(dict(routine=label,state=snapshot())),None)
def tap(button,neutral=160,held=2):actions.append(dict(button=button,held=held,neutral=neutral,frame=p.frame_count));p.button_press(button);p.tick(held);p.button_release(button);p.tick(neutral)
def rows():
 tiles=list(p.memory[symbols['wTileMap'][1]:symbols['wTileMap'][1]+360]);return [''.join(chr(v-0x80+ord('A')) if 0x80<=v<=0x99 else (chr(v-0xa0+ord('a')) if 0xa0<=v<=0xb9 else (str(v-0xf6) if 0xf6<=v<=0xff else (' ' if v==0x7f else '.'))) for v in tiles[i:i+20]) for i in range(0,360,20)]
def shot(tag):p.screen.image.save(out/(tag+'.png'));s=snapshot();states.append(dict(tag=tag,state=s));print(tag,{k:v for k,v in s.items() if k!='party'},flush=True)
shot('initial');tap('down',64,16);entered=False;fainted=False;switched=False;opened_revive=False;revived=False
for i in range(200):
 s=snapshot();entered=entered or s['in_battle']==2
 if any(e['routine']=='RemoveFaintedPlayerMon' for e in events) and not fainted:fainted=True;shot('actual-faint')
 if entered and s['in_battle']==0 and any(e['routine']=='TrainerBattleVictory' for e in events):break
 text='\n'.join(rows())
 if 'gained' in text.lower() and 'exp-notice' not in [row['tag'] for row in states]:shot('exp-notice')
 if entered and s['party_hp'][0]==0 and s['active']==0 and s['menu_max']==1 and text.count('BULBASAUR')>=2 and 'WEEDLE' not in text:
  if s['cursor']!=1:tap('down',30)
  tap('a');switched=True;shot('forced-switch-input')
 elif case in ('revive','revive-next') and switched and not revived and (case=='revive' or opened_revive or 'CATERPIE' in text):
  if s['party_hp'][0]>0:revived=True;shot('actual-revive');tap('a')
  elif not opened_revive and 'FIGHT' in text and 'ITEM' in text:
   tap('up',12);tap('left',12);tap('down',30);tap('a');opened_revive=True;shot('opened-bag')
  elif 'REVIVE' in text:tap('a');shot('selected-revive')
  elif text.count('BULBASAUR')>=2 and 'WEEDLE' not in text:
   if s['cursor']!=0:tap('up',30)
   tap('a');shot('used-revive')
  else:tap('a')
 elif case in ('revive','revive-next') and revived and 'FIGHT' in text and 'ITEM' in text:
  tap('up',12);tap('left',12);tap('a')
 else:tap('a')
else:raise AssertionError(('battle did not finish',snapshot(),rows(),events[-5:]))
shot('after-battle');assert fainted and switched if case in ('no-revive','revive','revive-next') else not fainted;assert snapshot()['party_exp']==([168,5560] if case=='revive' else ([135,5593] if case=='revive-next' else ([135,5593] if case=='no-revive' else [5593,135]))),snapshot();p.save_state((out/'after-battle.state').open('wb'));p.tick(200);tap('a',100);tap('start',200);shot('start-for-save');save_index=4 if get('wMaxMenuItem')==7 else 3
for _ in range(8):
 if get('wCurrentMenuItem')==save_index:break
 tap('down',30)
tap('a')
for _ in range(8):
 if any(e['routine']=='SavePartyAndDexData' for e in events):break
 tap('a',250)
assert any(e['routine']=='SavePartyAndDexData' for e in events);p.tick(300);shot('saved');p.stop(save=True);ram=private.with_suffix('.gbc.ram').read_bytes();assert ram[0x3523]==(~sum(ram[0x2598:0x3523]))&255;(out/'record.json').write_text(json.dumps(dict(case=case,repeat=repeat,source_revision='fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c',rom_sha1=hashlib.sha1(rom.read_bytes()).hexdigest(),constructed_state_sha256=hashlib.sha256((out/'constructed-start.state').read_bytes()).hexdigest(),prepared_map_state_sha256=hashlib.sha256((prepared/'prepared-map.state').read_bytes()).hexdigest(),script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),actions=actions,events=events,states=states,normal_save=True,sram_checksum=ram[0x3523],sram_sha256=hashlib.sha256(ram).hexdigest(),party_sram_raw_hex=ram[0x2f34:0x2f8c].hex(),saved_money_hex=ram[0x25f3:0x25f6].hex(),saved_bag_hex=ram[0x25c9:0x25ce].hex(),actual_revive=revived),indent=2));(r/f'audit-867-original-faint-exp-{case}-{repeat}-path.txt').write_text(str(out)+'\n');print('out',out,flush=True)

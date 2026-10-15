from pathlib import Path
import tempfile,shutil,hashlib
exec(open('/home/jerry/develop/open-pokered/.task-tmp/fidelity-39-plus/stats-59-tools/record_retail_stats.py').read().split('for trial in [1,2]:')[0])
r=root/'.task-tmp/fidelity-39-plus';out=Path(tempfile.mkdtemp(prefix='audit-883-original-cut-grass-v2-',dir=r));(r/'audit-883-original-cut-grass-v2-path.txt').write_text(str(out)+'\n');rom=r/'movement-91-setup/walking-reference.gbc';base=r/'movement-91-setup/outside-after-save.state';prior=Path((r/'audit-860-original-tmhm-full-path.txt').read_text().strip());template=json.loads((prior/'tm-replace-1/record.json').read_text())['initial'];records=[]
# Preparation-only entry initialization: source EnterMap loads the real Route6
# map/header/sprites. No CUT routine is directly invoked during preparation.
prep=out/'map-preparation';prep.mkdir();private=prep/'original.gbc';shutil.copyfile(rom,private);p=PyBoy(str(private),window='null',sound_emulated=True,color_palette=[0xffffff,0xaaaaaa,0x555555,0]);p.set_emulation_speed(0);p.load_state(io.BytesIO(base.read_bytes()))
for key,value in [('wCurMap',17),('wXCoord',10),('wYCoord',22),('wRoute6CurScript',0),('wDestinationWarpID',255),('wYBlockCoord',0),('wXBlockCoord',0)]:p.memory[symbols[key][1]]=value
view=symbols['wOverworldMap'][1]+7+10+(10+6)*(22>>1)+(10>>1);a=symbols['wCurrentTileBlockMapViewPointer'][1];p.memory[a:a+2]=[view&255,view>>8];entry=symbols['EnterMap'][1];p.memory[0xdfe0:0xdfe4]=[0xfb,0xc3,entry&255,entry>>8];p.register_file.PC=0xdfe0
for button in ['a','b','start','select','up','down','left','right']:p.button_release(button)
p.tick(240);assert int(p.memory[symbols['wCurMap'][1]])==17 and int(p.memory[symbols['wXCoord'][1]])==10 and int(p.memory[symbols['wYCoord'][1]])==22 and int(p.memory[symbols['wIsInBattle'][1]])==0
prepared=prep/'prepared-map.state';p.save_state(prepared.open('wb'));p.screen.image.save(prep/'prepared-map.png');p.stop(save=False)
for health in ['healthy','fainted']:
 for repeat in [1,2]:
  trial=out/f'{health}-{repeat}';trial.mkdir();private=trial/'original.gbc';shutil.copyfile(rom,private);p=PyBoy(str(private),window='null',sound_emulated=True,color_palette=[0xffffff,0xaaaaaa,0x555555,0]);p.set_emulation_speed(0);p.load_state(io.BytesIO(prepared.read_bytes()));party=symbols['wPartyMons'][1];first=template['raw'].copy();first[1:3]=[0,35 if health=='healthy' else 0];second=template['partner'].copy();p.memory[party:party+88]=first+second;p.memory[symbols['wPartyCount'][1]]=2;p.memory[symbols['wPartySpecies'][1]:symbols['wPartySpecies'][1]+7]=[153,153,255,255,255,255,255];nick=[0x80+ord(c)-ord('A') for c in 'BULBASAUR']+[0x50]*2
  for i in range(2):p.memory[symbols['wPartyMonNicks'][1]+11*i:symbols['wPartyMonNicks'][1]+11*(i+1)]=nick
  
  # Valid campaign facts established before validation: Route6 trainers beaten.
  import sys
  sys.path.insert(0,str(root/'scripts'));from save_builder import event_flag_bit
  for number in range(6):
   bit=event_flag_bit(f'EVENT_BEAT_ROUTE_6_TRAINER_{number}');p.memory[symbols['wEventFlags'][1]+(bit>>3)]|=1<<(bit&7)
  p.memory[symbols['wObtainedBadges'][1]]=2;p.memory[symbols['wOptions'][1]]=0x41;p.memory[symbols['wNumBagItems'][1]]=0;p.memory[symbols['wBagItems'][1]]=255;p.memory[symbols['wPlayerMoney'][1]:symbols['wPlayerMoney'][1]+3]=[0,0x30,0]
  # Facing is part of the preboot construction, not a validation-time write.
  p.memory[symbols['wSpritePlayerStateData1FacingDirection'][1]]=0;p.memory[symbols['wPlayerDirection'][1]]=4
  for key in ['wBattleAndStartSavedMenuItem','wPartyAndBillsPCSavedMenuItem','wBagSavedMenuItem','wListScrollOffset']:p.memory[symbols[key][1]]=0
  p.save_state((trial/'constructed-start.state').open('wb'));actions=[];events=[];captures={};get=lambda k:int(p.memory[symbols[k][1]]);finished=[False];target=symbols['wOverworldMap'][1]+(11+3)*16+(5+3)
  def snapshot():
   raw=list(p.memory[party:party+44]);return {'frame':p.frame_count,'map':get('wCurMap'),'x':get('wXCoord'),'y':get('wYCoord'),'in_battle':get('wIsInBattle'),'current_block':int(p.memory[target]),'target_address':target,'standing_tile':get('wTilePlayerStandingOn'),'front_tile':get('wTileInFrontOfPlayer'),'cut_tile':get('wCutTile'),'facing':get('wSpritePlayerStateData1FacingDirection'),'hp':(raw[1]<<8)|raw[2],'pp':raw[29:33],'raw':raw,'partner':list(p.memory[party+44:party+88]),'cursor':get('wCurrentMenuItem'),'menu_max':get('wMaxMenuItem')}
  def hook(label):
   events.append({'routine':label,'state':snapshot()})
   if label=='UsedCut':
    sp=p.register_file.SP;pc=int(p.memory[sp])|(int(p.memory[sp+1])<<8);bank=0 if pc<0x4000 else get('hLoadedROMBank')
    def returned(_):
     if p.register_file.SP==sp+2:finished[0]=True;events.append({'routine':'UsedCutReturn','state':snapshot()})
    p.hook_register(bank,pc,returned,None)
  for label in ['UsedCut','ReplaceTreeTileBlock','InitCutAnimOAM','AnimCut','TryDoWildEncounter','DrawStartMenu','DisplayPartyMenu','SavePartyAndDexData']:
   if label in symbols:p.hook_register(*symbols[label],lambda _,label=label:hook(label),None)
  def tap(button,neutral=100,held=2):actions.append({'button':button,'held':held,'neutral':neutral,'frame':p.frame_count});p.button_press(button);p.tick(held);p.button_release(button);p.tick(neutral)
  def shot(tag):p.screen.image.save(trial/(tag+'.png'));s=snapshot();captures[tag]={'state':s,'sha256':hashlib.sha256((trial/(tag+'.png')).read_bytes()).hexdigest()};print(health,repeat,tag,{k:v for k,v in s.items() if k not in ['raw','partner']},flush=True);return s
  before=shot('before');assert before['current_block']==11 and before['facing']==0;tap('start');
  if get('wMaxMenuItem')==7:tap('down',30)
  tap('a');shot('party');tap('a');shot('field-actions');assert int(p.memory[symbols['wFieldMoves'][1]])==1 and get('wCurrentMenuItem')==0;tap('a',120)
  for _ in range(20):
   if finished[0]:break
   tap('a',120)
  else:raise AssertionError(('UsedCut did not return',snapshot(),events))
  p.tick(200);after=shot('after');assert after['current_block']==10 and after['raw']==first and after['partner']==second and after['cut_tile']==82;assert any(e['routine']=='ReplaceTreeTileBlock' for e in events);tap('down',32,16);stepped=shot('stepped-on-cleared');assert (stepped['x'],stepped['y'])==(10,23) and stepped['standing_tile']==44 and stepped['in_battle']==0,stepped
  p.save_state((trial/'after-cut.state').open('wb'));p.stop(save=False);record={'health':health,'repeat':repeat,'scope':'Original actual START→Pokemon→Cut, grass block0B→0A and real down step onto formerly grass tile; source EnterMap bootstrap and legal party construction occur before validation boundary, hooks observe only, no post-boundary RAM writes. Leave/reenter/SAVE not yet exercised.','source_revision':'fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c','rom_sha1':hashlib.sha1(rom.read_bytes()).hexdigest(),'rom_sha256':hashlib.sha256(rom.read_bytes()).hexdigest(),'base_state_sha256':hashlib.sha256(base.read_bytes()).hexdigest(),'prepared_state_sha256':hashlib.sha256(prepared.read_bytes()).hexdigest(),'constructed_state_sha256':hashlib.sha256((trial/'constructed-start.state').read_bytes()).hexdigest(),'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'before':before,'after':after,'stepped':stepped,'actions':actions,'events':events,'captures':captures};(trial/'record.json').write_text(json.dumps(record,indent=2));records.append(record)
(out/'verification.json').write_text(json.dumps({'scope':'Original actual grass CUT healthy/FNT with healthy bench, each repeated twice, block and standing grass predicate verification. No encounter RNG equivalence claimed.','trials':records},indent=2));print('output',out,flush=True)

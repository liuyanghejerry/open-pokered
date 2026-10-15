from pathlib import Path
import tempfile,hashlib,shutil
exec(open('/home/jerry/develop/open-pokered/.task-tmp/fidelity-39-plus/stats-59-tools/record_retail_stats.py').read().split('for trial in [1,2]:')[0])
r=root/'.task-tmp/fidelity-39-plus';out=Path(tempfile.mkdtemp(prefix='audit-860-original-tmhm-full-',dir=r));rom=r/'movement-91-setup/walking-reference.gbc';base=(r/'movement-91-setup/outside-after-save.state').read_bytes();previous=Path((r/'audit-841-original-vitamins-path.txt').read_text().strip());template=json.loads((previous/'rare-candy-fainted/record.json').read_text())['initial'];records=[]
flows=['tm-replace','tm-decline','tm-forget-cancel','tm-hm-refusal','hm-replace']
for flow in flows:
 for repeat in [1,2]:
  trial=out/f'{flow}-{repeat}';trial.mkdir();private_rom=trial/'original.gbc';shutil.copyfile(rom,private_rom);p=PyBoy(str(private_rom),window='null',sound_emulated=True,color_palette=[0xffffff,0xaaaaaa,0x555555,0]);p.set_emulation_speed(0);p.load_state(io.BytesIO(base));party=symbols['wPartyMons'][1];bag=symbols['wBagItems'][1];first=template['raw'].copy();first[1:3]=[0,35];first[8:12]=[33,45,73,15];first[29:33]=[35,40,10,30];item=221;qty=2
  if flow=='hm-replace':
   item=196;qty=1;first[1:3]=[0,37];first[3]=first[33]=13;first[14:17]=[0,4,237];first[8:12]=[33,45,73,22];first[29:33]=[35,40,10,10];first[34:44]=[0,37,0,20,0,20,0,18,0,23]
  second=template['other_raw'].copy();p.memory[symbols['wPartyCount'][1]]=2;p.memory[symbols['wPartySpecies'][1]:symbols['wPartySpecies'][1]+7]=[153,153,255,255,255,255,255];p.memory[party:party+88]=first+second
  nick=[0x80+ord(c)-ord('A') for c in 'BULBASAUR']+[0x50]*2
  for i in range(2):p.memory[symbols['wPartyMonNicks'][1]+11*i:symbols['wPartyMonNicks'][1]+11*(i+1)]=nick
  p.memory[symbols['wNumBagItems'][1]]=1;p.memory[bag:bag+3]=[item,qty,255];p.memory[symbols['wPlayerMoney'][1]:symbols['wPlayerMoney'][1]+3]=[0,0x30,0]
  for key in ['wBattleAndStartSavedMenuItem','wPartyAndBillsPCSavedMenuItem','wBagSavedMenuItem','wListScrollOffset']:p.memory[symbols[key][1]]=0
  p.save_state((trial/'constructed-start.state').open('wb'));actions=[];events=[];captures={};get=lambda key:int(p.memory[symbols[key][1]]);finished=[False];return_hook=[None]
  def snapshot():
   raw=list(p.memory[party:party+44]);return dict(frame=p.frame_count,level=raw[33],hp=(raw[1]<<8)|raw[2],moves=raw[8:12],pp=raw[29:33],raw=raw,partner=list(p.memory[party+44:party+88]),bag=list(p.memory[bag:bag+3]),money=list(p.memory[symbols['wPlayerMoney'][1]:symbols['wPlayerMoney'][1]+3]),menu=get('wCurrentMenuItem'),menu_max=get('wMaxMenuItem'),text_box_id=get('wTextBoxID'),party_menu_type=get('wPartyMenuTypeOrMessageID'))
  texts=['BootedUpTMText','BootedUpHMText','TeachMachineMoveText','TryingToLearnText','AbandonLearningText','DidNotLearnText','WhichMoveToForgetText','HMCantDeleteText','OneTwoAndText','LearnedMove1Text'];text_names={symbols[k][1]:k for k in texts if k in symbols}
  def hook(label):
   e=dict(routine=label,state=snapshot())
   if label=='PrintText':e['text_name']=text_names.get(p.register_file.HL);e['hl']=p.register_file.HL
   events.append(e)
   if label=='ItemUseTMHM' and return_hook[0] is None:
    sp=p.register_file.SP;pc=int(p.memory[sp])|(int(p.memory[sp+1])<<8);bank=0 if pc<0x4000 else get('hLoadedROMBank');return_hook[0]=(bank,pc,sp+2)
    def returned(_):
     if p.register_file.SP==sp+2:finished[0]=True;events.append(dict(routine='ItemUseTMHMReturn',state=snapshot()))
    p.hook_register(bank,pc,returned,None)
  for label in ['ItemUseTMHM','PrintText','LearnMove','TryingToLearn','AbandonLearning','PrintLearnedMove','RemoveUsedItem','DisplayTextBoxID','DisplayPartyMenu','SaveGameData','SavePartyAndDexData','DrawStartMenu']:
   if label in symbols:p.hook_register(*symbols[label],lambda _,label=label:hook(label),None)
  def tap(button,neutral=100):actions.append(dict(button=button,press=2,neutral=neutral,start_frame=p.frame_count));p.button_press(button);p.tick(2);p.button_release(button);p.tick(neutral)
  def shot(name):path=trial/(name+'.png');p.screen.image.save(path);captures[name]=dict(state=snapshot(),png_sha256=hashlib.sha256(path.read_bytes()).hexdigest());return captures[name]
  before=snapshot();tap('start')
  for _ in range(2 if get('wMaxMenuItem')==7 else 1):tap('down',30)
  tap('a');tap('a');tap('a');shot('booted-machine')
  def yesno_since(routine):
   matches=[e for e in events if e['routine']==routine];return bool(matches) and any(e['routine']=='DisplayTextBoxID' and e['state']['text_box_id']==20 and e['state']['frame']>=matches[0]['state']['frame'] for e in events)
  for _ in range(20):
   if any(e['routine']=='DisplayTextBoxID' and e['state']['text_box_id']==20 for e in events):break
   tap('a',120)
  else:raise AssertionError(('no machine teaching yes/no',flow,snapshot(),events))
  shot('teach-choice');tap('a',120);shot('party-selection');tap('a',120);shot('first-learning')
  for _ in range(20):
   if yesno_since('TryingToLearn'):break
   tap('a',120)
  else:raise AssertionError(('no original four-move yes/no',flow,snapshot(),events))
  shot('learn-choice');assert snapshot()['moves']==first[8:12] and snapshot()['bag']==[item,qty,255]
  if flow=='tm-decline':
   tap('down',30);tap('a',120);assert any(e['routine']=='AbandonLearning' for e in events);shot('abandon-choice');tap('a',120);shot('did-not-learn')
  else:
   tap('a',120);assert get('wMaxMenuItem')==3;shot('forget-menu')
   if flow=='tm-forget-cancel':
    tap('b',120);assert any(e['routine']=='AbandonLearning' for e in events);shot('abandon-choice');tap('a',120);shot('did-not-learn')
   else:
    if flow=='tm-hm-refusal':
     for _ in range(3):tap('down',30)
     tap('a',120);shot('hm-refusal');assert snapshot()['moves']==first[8:12] and snapshot()['bag']==[item,qty,255];tap('a',120);shot('hm-retry');assert get('wCurrentMenuItem')==0
    tap('down',30);tap('a',120);shot('forgot')
  for _ in range(30):
   if finished[0]:break
   tap('a',120)
   if any(e['routine']=='PrintLearnedMove' for e in events) and 'learned' not in captures:shot('learned')
  else:raise AssertionError(('item never returned',flow,snapshot(),return_hook,events))
  after=snapshot();shot('after-use');assert after['partner']==second and after['level']==before['level'] and after['hp']==before['hp'] and after['money']==[0,0x30,0]
  success=flow not in ['tm-decline','tm-forget-cancel'];expected_moves=first[8:12].copy();expected_pp=first[29:33].copy()
  if success:expected_moves[1]=15 if flow=='hm-replace' else 72;expected_pp[1]=30 if flow=='hm-replace' else 10
  expected_qty=qty-1 if success and flow!='hm-replace' else qty;assert after['moves']==expected_moves and after['pp']==expected_pp and after['bag']==[item,expected_qty,255],(flow,after)
  p.save_state((trial/'after-use.state').open('wb'));p.tick(250);start_count=len([e for e in events if e['routine']=='DrawStartMenu'])
  for _ in range(6):
   if len([e for e in events if e['routine']=='DrawStartMenu'])>start_count:break
   tap('b')
  else:raise AssertionError('could not return to START')
  save_index=4 if get('wMaxMenuItem')==7 else 3
  for _ in range(8):
   if get('wCurrentMenuItem')==save_index:break
   tap('down',30)
  tap('a')
  for _ in range(8):
   if any(e['routine']=='SavePartyAndDexData' for e in events):break
   tap('a',250)
  assert any(e['routine']=='SavePartyAndDexData' for e in events);p.tick(300);shot('saved');p.stop(save=True);ram=private_rom.with_suffix('.gbc.ram').read_bytes();bank,address=symbols['sPartyData'];offset=bank*8192+address-0xa000;assert list(ram[offset+8:offset+96])==after['raw']+second;checksum=(~sum(ram[0x2598:0x3523]))&255;assert checksum==ram[0x3523];assert ram[0x25f3:0x25f6]==bytes([0,0x30,0])
  record=dict(flow=flow,repeat=repeat,initial=before,after=after,captures=captures,actions=actions,events=events,rom_sha1=hashlib.sha1(rom.read_bytes()).hexdigest(),constructed_state_sha256=hashlib.sha256((trial/'constructed-start.state').read_bytes()).hexdigest(),normal_save=True,sram_sha256=hashlib.sha256(ram).hexdigest(),sram_checksum=checksum,saved_money=3000,return_hook=return_hook[0]);(trial/'record.json').write_text(json.dumps(record,indent=2));records.append(record);print(flow,repeat,'complete',after['moves'],after['pp'],'qty',after['bag'][1],'checksum',checksum,flush=True)
for flow in flows:
 a,b=[v for v in records if v['flow']==flow];assert a['actions']==b['actions'] and a['captures']==b['captures'] and a['after']['raw']==b['after']['raw']
(out/'verification.json').write_text(json.dumps(dict(scope='Original actual TM21/HM01 boot/teach/four-move replace/decline/forget cancel/HM refusal retry, ordinary SAVE; only initial construction before input boundary, executable hooks observe only; two repeats per case.',source_revision='fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c',script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),trials=records),indent=2));(r/'audit-860-original-tmhm-full-path.txt').write_text(str(out)+'\n');print('output',out,flush=True)

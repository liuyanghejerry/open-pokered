from pathlib import Path
import tempfile,shutil,hashlib
exec(open('/home/jerry/develop/open-pokered/.task-tmp/fidelity-39-plus/stats-59-tools/record_retail_stats.py').read().split('for trial in [1,2]:')[0])
r=root/'.task-tmp/fidelity-39-plus';prior=Path((r/'audit-883-original-cut-grass-v2-path.txt').read_text().strip());out=Path(tempfile.mkdtemp(prefix='audit-883-original-reenter-v4-',dir=r));(r/'audit-883-original-reenter-v4-path.txt').write_text(str(out)+'\n');records=[];rom=r/'movement-91-setup/walking-reference.gbc'
for health in ['healthy','fainted']:
 for repeat in [1,2]:
  trial=out/f'{health}-{repeat}';trial.mkdir();private=trial/'original.gbc';shutil.copyfile(rom,private);p=PyBoy(str(private),window='null',sound_emulated=True,color_palette=[0xffffff,0xaaaaaa,0x555555,0]);p.set_emulation_speed(0);checkpoint=prior/f'{health}-{repeat}/after-cut.state';p.load_state(io.BytesIO(checkpoint.read_bytes()));get=lambda k:int(p.memory[symbols[k][1]]);actions=[];states=[];events=[];target=symbols['wOverworldMap'][1]+232
  def state(tag):v={'tag':tag,'frame':p.frame_count,'map':get('wCurMap'),'x':get('wXCoord'),'y':get('wYCoord'),'in_battle':get('wIsInBattle'),'block_at_target_if_route6':int(p.memory[target]) if get('wCurMap')==17 else None};states.append(v);return v
  for label in ['TryDoWildEncounter','DrawStartMenu','SavePartyAndDexData','EnterMap']:p.hook_register(*symbols[label],lambda _,label=label:events.append({'routine':label,'state':state(label)}),None)
  def tap(button,neutral=16,held=16):actions.append({'button':button,'held':held,'neutral':neutral,'frame':p.frame_count});p.button_press(button);p.tick(held);p.button_release(button);p.tick(neutral)
  def text():
   tiles=list(p.memory[symbols['wTileMap'][1]:symbols['wTileMap'][1]+360]);return '\n'.join(''.join(chr(v-0x80+ord('A')) if 0x80<=v<=0x99 else (' ' if v==0x7f else '.') for v in tiles[i:i+20]) for i in range(0,360,20))
  def settle_battle():
   for _ in range(80):
    if not get('wIsInBattle'):return
    if 'FIGHT' in text() and 'ITEM' in text():
     tap('up',30,2);tap('right',30,2);tap('down',30,2);tap('a',160,2)
    else:tap('a',160,2)
   raise AssertionError(('battle remained',state('bad-battle'),text()))
  def goto(x,y,mapid=17):
   for _ in range(160):
    settle_battle();v=state('nav');
    if v['map']!=mapid:return v
    if (v['x'],v['y'])==(x,y):return v
    direction=('right' if v['x']<x else 'left') if v['x']!=x else ('down' if v['y']<y else 'up');tap(direction)
   raise AssertionError(('nav did not converge',x,y,state('bad-nav'),text()))
  initial=state('initial');assert initial['block_at_target_if_route6']==10;goto(14,23);goto(14,29);goto(9,29);goto(9,34)
  for _ in range(20):
   if get('wCurMap')!=17:break
   tap('down')
  for _ in range(40):
   if get('wCurMap')!=17:break
   p.tick(20)
  entered=state('entered-other-map');assert entered['map']==5,entered;p.screen.image.save(trial/'other-map.png')
  # Real north city connection back to Route6; no debug warp.
  for _ in range(80):
   if get('wCurMap')==17:break
   v=state('city');tap('up')
   if v['x']==get('wXCoord') and v['y']==get('wYCoord'):tap('a',120,2)
  returned=state('returned');assert returned['map']==17 and returned['block_at_target_if_route6']==11,returned;p.screen.image.save(trial/'returned.png');p.save_state((trial/'returned.state').open('wb'))
  # Ordinary START SAVE after return, independent SRAM checksum readback.
  tap('start',100,2);saveindex=4 if get('wMaxMenuItem')==7 else 3
  for _ in range(8):
   if get('wCurrentMenuItem')==saveindex:break
   tap('down',30,2)
  tap('a',100,2)
  for _ in range(8):
   if any(e['routine']=='SavePartyAndDexData' for e in events):break
   tap('a',250,2)
  assert any(e['routine']=='SavePartyAndDexData' for e in events);p.tick(300);p.stop(save=True);ram=private.with_suffix('.gbc.ram').read_bytes();assert ram[0x3523]==(~sum(ram[0x2598:0x3523]))&255;record={'health':health,'repeat':repeat,'scope':'Continuation from genuine captured after-CUT state; no RAM writes, real walking/wildRUN if necessary/Route6↔VermilionCity connection and ordinarySAVE, source block reset on reentry; no exact encounter RNG equivalence claim.','source_checkpoint_sha256':hashlib.sha256(checkpoint.read_bytes()).hexdigest(),'rom_sha256':hashlib.sha256(rom.read_bytes()).hexdigest(),'actions':actions,'states':states,'events':events,'initial':initial,'entered':entered,'returned':returned,'normal_save':True,'sram_sha256':hashlib.sha256(ram).hexdigest(),'sram_checksum':ram[0x3523],'party_raw_hex':ram[0x2f34:0x2f8c].hex()};(trial/'record.json').write_text(json.dumps(record,indent=2));records.append(record);print(health,repeat,'city+return+SAVE PASS',entered,returned,flush=True)
(out/'verification.json').write_text(json.dumps({'trials':records},indent=2));print('output',out,flush=True)

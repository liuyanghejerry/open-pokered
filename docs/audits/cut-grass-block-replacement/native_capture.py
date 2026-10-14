from pathlib import Path
import sys,os,json,tempfile,shutil,hashlib,socket,subprocess
root=Path('/home/jerry/develop/open-pokered');r=root/'.task-tmp/fidelity-39-plus';wt=root/'.task-tmp/fidelity-883-cut-grass';sys.path.insert(0,str(wt/'scripts'));from debug_drive import DebugClient;from save_builder import make_mon,event_flag_bit
stage=sys.argv[1];original=Path(sys.argv[2]);source=sys.argv[3];out=Path(tempfile.mkdtemp(prefix=f'audit-883-native-cut-grass-{stage}-',dir=r));(r/f'audit-883-native-cut-grass-{stage}-path.txt').write_text(str(out)+'\n');template=json.loads((root/'docs/audits/rare-candy-move-learning/fixture.json').read_text());env=os.environ.copy();env['LD_LIBRARY_PATH']='/tmp/pokered-build-lib';env['POKERED_MAPS_DIR']=str(wt/'crates/pokered-data/maps');records=[]
for health in ['healthy','fainted']:
 fixture=json.loads(json.dumps(template));first=make_mon('Bulbasaur',12,['Tackle','Growl','LeechSeed','Cut']);first['catch_rate']=45;first['hp']=35 if health=='healthy' else 0;second=make_mon('Bulbasaur',12,['Tackle','Growl']);second['catch_rate']=45;fixture['party']=[first,second];gd=fixture['game_data'];gd['obtained_badges']=2;gd['player_money']=3000;gd['options']['text_speed']='Fast';gd['bag']['items']=[];gd['position']={'map_id':17,'x':10,'y':22,'x_block':0,'y_block':0};gd['last_map']=5
 for number in range(6):
  bit=event_flag_bit(f'EVENT_BEAT_ROUTE_6_TRAINER_{number}');gd['event_flags'][bit>>3]|=1<<(bit&7)
 fp=out/(health+'-fixture.json');fp.write_text(json.dumps(fixture,indent=2))
 for repeat in [1,2]:
  trial=out/f'{health}-{repeat}';trial.mkdir();binary=trial/'pokered-app';shutil.copyfile(original,binary);binary.chmod(0o755);s=socket.socket();s.bind(('127.0.0.1',0));port=s.getsockname()[1];s.close();states=[];actions=[];captures={}
  with (trial/'game.log').open('w') as log:
   p=subprocess.Popen([str(binary),'run','--headless','--speed','0','--seed','42','--debug-port',str(port),'--skip-intro','--snapshot',str(fp),'--warp','Route6,10,22','--lang','en','--no-audio','--save',str(trial/'isolated.sav')],cwd=trial,env=env,stdout=log,stderr=subprocess.STDOUT)
   try:
    d=DebugClient(port);d.step(120)
    def state(tag):v=d.state();states.append({'tag':tag,'state':v});return v
    def tap(button,frames=32,held=1):actions.append({'frame':state('pre-'+button)['frame_count'],'button':button,'held':held,'frames':frames});assert d.cmd(cmd='press_timeline',buttons=[button]*held+[None]*(frames-held),advance=True)['ok']
    def shot(tag):st=state(tag);path=trial/(tag+'.png');assert d.cmd(cmd='capture_frame',path=str(path))['ok'];captures[tag]={'frame':st['frame_count'],'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'path':str(path)};print(stage,health,repeat,tag,st['frame_count'],st['screen'],st['player_x'],st['player_y'],st['party'][0]['hp'],st.get('field_menu'),flush=True);return st
    before=shot('before');assert before['map_name']=='Route6' and (before['player_x'],before['player_y'])==(10,22) and before['player_facing']=='Down' and before['map_blocks'][115]==11;tap('start',60,4);tap('down');tap('a');assert shot('party')['screen']=='party';tap('a');submenu=shot('field-actions');assert submenu['field_menu']['phase'].startswith('ActionMenu');tap('a');shot('cut-notice')
    for _ in range(12):
     st=state('cut-ack');
     if st['dialogue_state'] is None:break
     tap('a')
    else:raise AssertionError('cut text did not close')
    d.step(240);after=shot('after');assert after['screen']=='overworld' and after['party']==before['party'];assert after['map_blocks'][115]==(11 if stage=='before' else 10),(stage,after['map_blocks'][115]);d.cmd(cmd='press_timeline',buttons=['down']*16+[None]*32,advance=True);stepped=shot('stepped-on-target');assert (stepped['player_x'],stepped['player_y'])==(10,23),stepped
    # Finish any real encounter by ordinary RUN/menu inputs, never a debug setter.
    for _ in range(80):
     st=state('post-step');
     if st['screen']!='battle':break
     phase=st['battle_phase']
     if phase=='MainMenu':
      tap('up',12);tap('right',12);tap('down',12);tap('a')
     else:tap('a')
    else:raise AssertionError('could not return from actual encounter')
    def settle():
     for _ in range(100):
      st=state('nav-settle')
      if st['screen']!='battle' and st['dialogue_state'] is None:return st
      if st['screen']=='battle' and st['battle_phase']=='MainMenu':tap('up',12);tap('right',12);tap('down',12);tap('a',80)
      else:tap('a',80)
     raise AssertionError('navigation battle/dialogue did not settle')
    def goto(x,y,mapname='Route6'):
     for _ in range(150):
      st=settle()
      if st['map_name']!=mapname:return st
      if (st['player_x'],st['player_y'])==(x,y):return st
      direction=('right' if st['player_x']<x else 'left') if st['player_x']!=x else ('down' if st['player_y']<y else 'up');tap(direction,32,4)
     raise AssertionError(('navigation failed',x,y,state('failed-nav')))
    goto(14,23);goto(14,29);goto(9,29);goto(9,34)
    for _ in range(20):
     if state('connection-before')['map_name']!='Route6':break
     tap('down',48,40)
    entered=shot('entered-city');assert entered['map_name']=='VermilionCity',entered
    for _ in range(20):
     if state('connection-return')['map_name']=='Route6':break
     tap('up',48,40)
    returned=shot('returned');assert returned['map_name']=='Route6' and returned['map_blocks'][115]==11,returned
    st=state('before-save');tap('start',60,4);sv=state('start-for-save');assert sv['screen']=='start-menu',sv
    for _ in range(7):
     menu=state('save-cursor')['field_menu'];
     if menu['items'][menu['cursor']]=='Save':break
     tap('down')
    tap('a');assert state('save-open')['screen']=='save';d.step(200);tap('a',400);d.step(300);shot('saved');assert (trial/'isolated.sav').exists();d.close()
   finally:(trial/'states.json').write_text(json.dumps(states,indent=2));(trial/'actions.json').write_text(json.dumps(actions,indent=2));p.terminate();p.wait(timeout=10)
  cp=subprocess.run([str(binary),'import-snapshot','--input',str(trial/'isolated.sav'),'-o',str(trial/'readback.json')],cwd=trial,env=env,capture_output=True);assert cp.returncode==0,cp.stderr;rb=json.loads((trial/'readback.json').read_text());ram=(trial/'isolated.sav').read_bytes();assert ram[0x3523]==(~sum(ram[0x2598:0x3523]))&255;assert rb['game_data']['player_money']==3000;record={'stage':stage,'health':health,'repeat':repeat,'source_commit':source,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256(fp.read_bytes()).hexdigest(),'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'before':before,'after':after,'stepped':stepped,'entered_city':entered,'returned':returned,'actions':actions,'captures':captures,'normal_save':True,'saved_party':rb['party'],'sram_sha256':hashlib.sha256(ram).hexdigest(),'sram_checksum':ram[0x3523],'party_raw_hex':ram[0x2f34:0x2f8c].hex(),'scope':'Private preboot construction, actualSTART/Pokemon/CUT and ordinarySAVE; grass predicate derived from current map block/tileset. No post-boot setter or exact encounterRNG equivalence claim; Actual Route6↔VermilionCity walking resets the natural grass block to0B on return. Preboot all6 Route6trainer defeat facts avoid requiredtrainerbattles; no postboot setters.'};(trial/'record.json').write_text(json.dumps(record,indent=2));records.append(record)
(out/'verification.json').write_text(json.dumps({'stage':stage,'scope':'Actual native grass CUT healthy/FNT with healthy bench eachrepeat2; same actual inputs on before/after, preboot construction only.','trials':records},indent=2));print('output',out,flush=True)

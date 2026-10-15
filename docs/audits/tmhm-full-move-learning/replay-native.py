from pathlib import Path
import sys,os,socket,subprocess,json,hashlib,tempfile,shutil
root=Path(__file__).resolve().parents[3];r=root/'.task-tmp/fidelity-39-plus';sys.path.insert(0,str(root/'scripts'));from debug_drive import DebugClient
stage=sys.argv[1];original_binary=Path(sys.argv[2]);source_identity=sys.argv[3];expected_hash=hashlib.sha256(original_binary.read_bytes()).hexdigest();assert hashlib.sha256(original_binary.read_bytes()).hexdigest()==expected_hash;out=Path(tempfile.mkdtemp(prefix=f'audit-870-native-tmhm-{stage}-',dir=r));(r/f'audit-870-native-tmhm-{stage}-path.txt').write_text(str(out)+'\n');template=json.loads((Path(__file__).parent/'fixture-template.json').read_text());env=os.environ.copy();env['LD_LIBRARY_PATH']='/tmp/pokered-build-lib';env['POKERED_MAPS_DIR']=str(root/'crates/pokered-data/maps');records=[];flows=['tm-replace','tm-decline','tm-forget-cancel','tm-hm-refusal','hm-replace']
for flow in flows:
 initial=json.loads(json.dumps(template));mon=initial['party'][0];mon['hp']=35;mon['moves']=['Tackle','Growl','LeechSeed','Cut'];mon['pp']=[35,40,10,30];item='Tm21';qty=2
 if flow=='hm-replace':
  item='Hm01';qty=1;mon['level']=13;mon['total_exp']=1261;mon['hp']=mon['max_hp']=37;mon['attack']=mon['defense']=20;mon['speed']=18;mon['special']=23;mon['moves']=['Tackle','Growl','LeechSeed','VineWhip'];mon['pp']=[35,40,10,10]
 initial['game_data']['bag']['items']=[[item,qty]];fixture=out/(flow+'-fixture.json');fixture.write_text(json.dumps(initial,indent=2))
 for repeat in [1,2]:
  trial=out/f'{flow}-{repeat}';trial.mkdir();binary=trial/'pokered-app';shutil.copyfile(original_binary,binary);binary.chmod(0o755);assert not (trial/'pokered.script_flags.json').exists() and not (trial/'isolated.script_flags.json').exists();actions=[];states=[];captures={};sock=socket.socket();sock.bind(('127.0.0.1',0));port=sock.getsockname()[1];sock.close()
  with (trial/'game.log').open('w') as log:
   p=subprocess.Popen([str(binary),'run','--headless','--speed','0','--seed','42','--debug-port',str(port),'--skip-intro','--snapshot',str(fixture),'--lang','en','--no-audio','--save',str(trial/'isolated.sav')],cwd=trial,env=env,stdout=log,stderr=subprocess.STDOUT)
   try:
    d=DebugClient(port);d.step(120)
    def state(tag):v=d.state();states.append(dict(tag=tag,state=v));return v
    def tap(button,frames=32):
     st=state('pre-'+button);held=4 if button=='start' else 1;actions.append(dict(frame=st['frame_count'],button=button,held=held,frames=frames));d.press_timeline([button]*held+[None]*(frames-held));d.step(frames)
    def capture(tag):
     st=state(tag);path=trial/(tag+'.png');assert d.cmd(cmd='capture_frame',path=str(path))['ok'];captures[tag]=dict(frame=st['frame_count'],sha256=hashlib.sha256(path.read_bytes()).hexdigest(),path=str(path));agent=d.cmd(cmd='get_agent_state',level=3);(trial/(tag+'.agent.json')).write_text(json.dumps(agent,indent=2));assert d.state()['frame_count']==st['frame_count'];return st
    flags_before=d.cmd(cmd='get_flags');tap('start',60)
    for button in ['down','down','a','a','a']:tap(button)
    boot=capture('booted-machine');assert boot['screen']=='bag' and boot['field_menu']['phase'].startswith('MachineBoot');tap('a');teach=capture('teach-choice');assert teach['field_menu']['phase'].startswith('MachineTeach');tap('a');party=capture('party-selection');assert party['screen']=='party' and party['field_menu']['phase']=='Browsing'
    tap('a');st=capture('first-learning');assert st['party'][0]['moves']==mon['moves'];
    if stage=='after':assert st['field_menu']['machine_learning_phase']=='TryingToLearn(0)'
    else:assert st['field_menu']['phase']=='ChooseMove { cursor: 0 }'
    d.step(240);capture('forget-menu')
    if stage=='after':
     tap('a');capture('four-moves-warning');tap('a');st=capture('delete-confirm');assert st['field_menu']['machine_learning_phase']=='ReplaceConfirm { yes: true }'
     if flow not in ['tm-decline']:tap('a');st=capture('actual-forget-menu');assert st['field_menu']['phase']=='ChooseMove { cursor: 0 }'
    if flow=='tm-decline':
     # There is no delete-old-move NO question in native. Use the list's CANCEL
     # entry instead; this is explicitly an unavailable source path, not a
     # claim that native showed the source abandonment confirmation.
     if stage=='after':
      tap('down');tap('a');st=capture('abandon-confirm');assert st['field_menu']['machine_learning_phase']=='AbandonConfirm { yes: true }';tap('a');capture('did-not-learn');tap('a')
     else:
      for _ in range(4):tap('down')
      capture('list-cancel');tap('a');capture('canceled')
    elif flow=='tm-forget-cancel':
     tap('b');capture('canceled')
     if stage=='after':
      st=capture('abandon-confirm');assert st['field_menu']['machine_learning_phase']=='AbandonConfirm { yes: true }';tap('a');capture('did-not-learn');tap('a')
    else:
     if flow=='tm-hm-refusal':
      for _ in range(3):tap('down')
      tap('a');st=capture('hm-refusal');assert st['field_menu']['phase']=='MoveChoiceNotice' and st['party'][0]['moves']==mon['moves'];tap('a');st=capture('hm-retry');assert st['field_menu']['phase']==('ChooseMove { cursor: 0 }' if stage=='after' else 'ChooseMove { cursor: 3 }')
      # Original resets to zero. Native keeps Cut at cursor3, so adapt the
      # subsequent input to reach the same Growl replacement outcome.
      if stage=='after':tap('down')
      else:tap('up');tap('up')
     else:tap('down')
     tap('a')
     if stage=='after':
      st=capture('forgot');assert st['field_menu']['machine_learning_phase']=='Forgot { slot: 1 }';assert st['party'][0]['moves']==mon['moves'];tap('a');st=capture('learned');assert st['field_menu']['machine_learning_phase'].startswith('Learned(');assert json.loads((trial/'learned.agent.json').read_text())['data']['bag']==[{'item':item,'qty':qty}];capture('learned-message');d.step(200);capture('learned-before-ack');tap('a');st=capture('learned-acknowledged');assert st['screen']=='bag'
     else:
      st=capture('combined-forgot-notice');assert st['screen']=='overworld'
      for _ in range(20):
       ds=state('learned-ack');
       if ds['dialogue'] is None:break
       if 'learned' in ds['dialogue'] and 'learned-message' not in captures:capture('learned-message')
       tap('a')
      else:raise AssertionError(('dialogue never closed',flow,state('bad')))
    st=state('after-learning-control')
    if st['screen']=='party':
     assert st['field_menu']['phase']=='Browsing';tap('b')
    st=capture('after-use');assert st['screen'] in ['bag','overworld']
    if st['screen']=='bag':tap('b')
    else:tap('start',60)
    assert state('start-for-save')['screen']=='start-menu'
    for button in ['down','down','a']:tap(button)
    assert state('save-open')['screen']=='save'
    d.step(200);tap('a',400);d.step(300);capture('saved');assert (trial/'isolated.sav').exists();flags_after=d.cmd(cmd='get_flags');d.close()
   finally:
    (trial/'states.json').write_text(json.dumps(states,indent=2));(trial/'actions.json').write_text(json.dumps(actions,indent=2));p.terminate();p.wait(timeout=10)
  cp=subprocess.run([str(binary),'import-snapshot','--input',str(trial/'isolated.sav'),'-o',str(trial/'readback.json')],env=env,cwd=trial,capture_output=True);assert cp.returncode==0,cp.stderr;rb=json.loads((trial/'readback.json').read_text());m=rb['party'][0];success=flow not in ['tm-decline','tm-forget-cancel'];moves=mon['moves'].copy();pp=mon['pp'].copy()
  if success:moves[1]='Cut' if flow=='hm-replace' else 'MegaDrain';pp[1]=30 if flow=='hm-replace' else 10
  assert m['moves']==moves and m['pp']==pp and m['pp_ups']==[0,0,0,0]
  for key in ['level','hp','max_hp','attack','defense','speed','special','stat_exp','total_exp']:assert m[key]==mon[key],(key,m[key],mon[key])
  expected_qty=qty-1 if success and flow!='hm-replace' else qty;assert rb['game_data']['bag']['items']==[[item,expected_qty]] and rb['game_data']['player_money']==3000
  for key,value in initial['party'][1].items():assert rb['party'][1][key]==value
  ram=(trial/'isolated.sav').read_bytes();checksum=(~sum(ram[0x2598:0x3523]))&255;assert checksum==ram[0x3523]
  record=dict(flow=flow,repeat=repeat,binary_sha256=expected_hash,frozen_source_commit=source_identity,stage=stage,script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),scope=('Parent 901 binary: compiled1566+testedOakpatch, manifest Rust identical d455. Healthy full-moves paths unchanged by236HP0 gate; not relabeled newer master.' if stage=='before' else 'Exact recorded child source commit; full TM/HM LearnMove continuation actual menus and ordinary SAVE.'),fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),initial_party=initial['party'],after_party=rb['party'],after_items=rb['game_data']['bag']['items'],money=3000,actual_flags_before=flags_before,actual_flags_after=flags_after,sidecars_initially_absent=True,private_binary=True,captures=captures,actions=actions,normal_save=True,sram_sha256=hashlib.sha256(ram).hexdigest(),sram_checksum=checksum,first_party_raw_hex=ram[0x2f34:0x2f60].hex(),healthy_second_raw_hex=ram[0x2f60:0x2f8c].hex(),decline_source_path_absent=flow=='tm-decline' and stage!='after');(trial/'record.json').write_text(json.dumps(record,indent=2));records.append(record);print(flow,repeat,'complete',m['moves'],'qty',expected_qty,'checksum',checksum,'frames',{k:v['frame'] for k,v in captures.items()},flush=True)
for flow in flows:
 a,b=[v for v in records if v['flow']==flow];assert a['actions']==b['actions'] and a['after_party']==b['after_party'] and a['captures']=={k:dict(v,path=str(out/f'{flow}-1'/(k+'.png'))) for k,v in b['captures'].items()}
(out/'verification.json').write_text(json.dumps(dict(stage=stage,frozen_source_identity=source_identity,scope=('Parent BEFORE: actual five full-moves TM/HM flows repeated twice, no delete/abandon prompts; tm-decline uses native list-CANCEL surrogate explicitly. Private901 frozen build identity retained, not relabeled currentmaster.' if stage=='before' else 'AFTER: actual five full-moves TM/HM flows repeated twice including source DeleteNO/B→AbandonYES/DidNotLearn, HM refusal cursor0 retry, LearnedQty2 untilack thenTMQty1/HMQty1, ordinarySAVE. Only preboot construction, no runtime state patches.'),script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),trials=records),indent=2));print('output',out,flush=True)

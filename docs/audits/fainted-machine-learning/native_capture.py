#!/usr/bin/env python3
"""Replay a constructed fainted TM03/HM01 case through ordinary menus and SAVE.

Usage: python3 native_capture.py before /abs/base-debug /abs/new-evidence-dir
       python3 native_capture.py after /abs/fixed-debug /abs/same-evidence-dir
Both executables must have the debug-server feature. No debug state edits.
"""
from pathlib import Path
import sys,os,socket,subprocess,json,hashlib,tempfile,copy,shutil
root=Path(__file__).resolve().parents[3];out=Path(sys.argv[3]).resolve();out.mkdir(parents=True,exist_ok=True)
sys.path.insert(0,str(root/'scripts'));from debug_drive import DebugClient
label=sys.argv[1];source_binary=Path(sys.argv[2]).resolve();assert label in ['before','after']
template=json.loads((Path(__file__).parent/'fixture-template.json').read_text())
env=os.environ.copy();env['LD_LIBRARY_PATH']='/tmp/pokered-build-lib';env['POKERED_MAPS_DIR']=str(root/'crates/pokered-data/maps');records=[]
for item,move,qty in [('Tm03','SwordsDance',2),('Hm01','Cut',1)]:
 for repeat in [1,2]:
  trial=out/f'{item}-{label}-{repeat}';trial.mkdir();binary=trial/'private-debug';shutil.copy2(source_binary,binary)
  initial=copy.deepcopy(template);initial['game_data']['bag']['items']=[[item,qty]];fixture=trial/'fixture.json';fixture.write_text(json.dumps(initial))
  states=[];actions=[];captures={};sock=socket.socket();sock.bind(('127.0.0.1',0));port=sock.getsockname()[1];sock.close()
  with (trial/'game.log').open('w') as log:
   p=subprocess.Popen([str(binary),'run','--headless','--speed','0','--seed','42','--debug-port',str(port),'--skip-intro','--snapshot',str(fixture),'--lang','en','--no-audio','--save',str(trial/'isolated.sav')],cwd=trial,env=env,stdout=log,stderr=subprocess.STDOUT)
   try:
    d=DebugClient(port);d.step(120)
    def state(tag):v=d.state();states.append(dict(tag=tag,state=v));return v
    def tap(button,frames=32):
     st=state('pre-'+button);held=4 if button=='start' else 1;actions.append(dict(frame=st['frame_count'],button=button,held=held,frames=frames));d.press_timeline([button]*held+[None]*(frames-held));d.step(frames)
    def capture(tag):
     st=state(tag);path=trial/(tag+'.png');assert d.cmd(cmd='capture_frame',path=str(path))['ok'];captures[tag]=dict(frame=st['frame_count'],sha256=hashlib.sha256(path.read_bytes()).hexdigest());return st
    flags_before=d.cmd(cmd='get_flags');capture('initial');tap('start',60)
    for button in ['down','down','a']:tap(button)
    assert state('bag-open')['screen']=='bag'
    tap('a');tap('a');boot=capture('machine-boot');assert boot['field_menu']['phase'].startswith('MachineBoot'),boot
    tap('a');teach=capture('machine-teach');assert teach['field_menu']['phase'].startswith('MachineTeach'),teach
    tap('a');party=capture('party-select');assert party['screen']=='party' and party['party'][0]['hp']==0
    tap('a');d.step(240);result=capture('machine-result');assert result['screen']=='party' and result['field_menu']['phase'].startswith('ItemUseNotice'),result
    if label=='before':assert result['party'][0]['moves']==initial['party'][0]['moves']
    else:assert result['party'][0]['moves']==['Tackle','Growl',move,'None'] and result['party'][0]['hp']==0
    prefix_actions=copy.deepcopy(actions)
    tap('a')
    if label=='before':
     assert state('retry-party')['screen']=='party';tap('b')
    assert state('bag-return')['screen']=='bag'
    for button in ['b','down','down','a']:tap(button)
    assert state('save-open')['screen']=='save'
    d.step(200);tap('a',400);d.step(300);capture('saved');assert (trial/'isolated.sav').exists()
    flags_after=d.cmd(cmd='get_flags');d.close()
   finally:
    (trial/'states.json').write_text(json.dumps(states,indent=2));(trial/'actions.json').write_text(json.dumps(actions,indent=2));p.terminate();p.wait(timeout=10)
  cp=subprocess.run([str(binary),'import-snapshot','--input',str(trial/'isolated.sav'),'-o',str(trial/'readback.json')],env=env,cwd=trial,capture_output=True);assert cp.returncode==0,cp.stderr;rb=json.loads((trial/'readback.json').read_text());m=rb['party'][0]
  expected=copy.deepcopy(initial['party'][0])
  if label=='after':expected['moves'][2]=move;expected['pp'][2]=30
  for key,value in expected.items():assert m[key]==value,(item,label,key,m[key],value)
  for key,value in initial['party'][1].items():assert rb['party'][1][key]==value,(item,'healthy partner',key)
  assert rb['game_data']['bag']['items']==[[item,qty-int(label=='after' and item=='Tm03')]]
  assert rb['game_data']['player_money']==initial['game_data']['player_money'];assert flags_before==flags_after
  ram=(trial/'isolated.sav').read_bytes();checksum=(~sum(ram[0x2598:0x3523]))&255;assert ram[0x3523]==checksum
  rec=dict(item=item,label=label,repeat=repeat,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),initial_party=initial['party'],after_party=rb['party'],initial_items=initial['game_data']['bag']['items'],after_items=rb['game_data']['bag']['items'],flags_before=flags_before,flags_after=flags_after,captures=captures,prefix_actions=prefix_actions,actions=actions,normal_save=True,no_state_edits_after_boot=True,private_binary=True,sidecars_absent_before_boot=True,sram_sha256=hashlib.sha256(ram).hexdigest(),sram_checksum=checksum,party_raw_hex=ram[0x2f34:0x2f8c].hex())
  (trial/'record.json').write_text(json.dumps(rec,indent=2));records.append(rec);print(item,label,repeat,m['moves'],rb['game_data']['bag']['items'],captures,flush=True)
for item in ['Tm03','Hm01']:
 a,b=[v for v in records if v['item']==item];assert a['actions']==b['actions'] and a['after_party']==b['after_party'] and a['captures']==b['captures'] and a['sram_sha256']==b['sram_sha256']
(out/(label+'-verification.json')).write_text(json.dumps(dict(scope='Constructed before boot level6 fainted Bulbasaur with legitimate Tackle/Growl and a healthy partner. Ordinary START/ITEM/USE, existing machine boot/teach confirmation, party pick and SAVE/YES. Same fixture/prefix input/frame before/after; twice repeat each. No postboot state mutation or debug save.',script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),trials=records),indent=2));print('output',out,flush=True)

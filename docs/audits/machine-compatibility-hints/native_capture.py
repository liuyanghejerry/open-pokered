#!/usr/bin/env python3
"""Usage: native_capture.py before|after /abs/debug-client /abs/new-evidence-dir.
Requires the client's debug-server feature; uses ordinary SAVE, no state edits.
"""
from pathlib import Path
import sys,os,socket,subprocess,json,hashlib,tempfile,shutil
root=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(root/'scripts'));from debug_drive import DebugClient
label=sys.argv[1];assert label in ['before','after']
original_binary=Path(sys.argv[2]).resolve();expected_hash=hashlib.sha256(original_binary.read_bytes()).hexdigest()
out=Path(sys.argv[3]).resolve();out.mkdir(parents=True,exist_ok=False)
template=json.loads((Path(__file__).parent/'fixture-template.json').read_text())
env=os.environ.copy();env.setdefault('LD_LIBRARY_PATH','/tmp/pokered-build-lib');env['POKERED_MAPS_DIR']=str(root/'crates/pokered-data/maps');records=[];flows=['tm21','hm01']
for flow in flows:
 initial=json.loads(json.dumps(template));mon=initial['party'][0];mon['hp']=35;mon['moves']=['Tackle','Growl','LeechSeed','Cut'];mon['pp']=[35,40,10,30];item='Tm21';qty=2
 if flow=='hm01':
  item='Hm01';qty=1;mon['level']=13;mon['total_exp']=1261;mon['hp']=mon['max_hp']=37;mon['attack']=mon['defense']=20;mon['speed']=18;mon['special']=23;mon['moves']=['Tackle','Growl','LeechSeed','VineWhip'];mon['pp']=[35,40,10,10]
 partner=initial['party'][1];partner['species']='Caterpie';partner['attack']=14;partner['defense']=15;partner['special']=11;partner['type1']=partner['type2']='Bug';partner['moves']=['Tackle','StringShot','None','None'];partner['pp']=[35,40,0,0];partner['total_exp']=1728
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
     st=state(tag);path=trial/(tag+'.png');assert d.cmd(cmd='capture_frame',path=str(path))['ok'];captures[tag]=dict(frame=st['frame_count'],sha256=hashlib.sha256(path.read_bytes()).hexdigest(),path=str(path));return st
    flags_before=d.cmd(cmd='get_flags');tap('start',60)
    for button in ['down','down','a','a','a']:tap(button)
    boot=capture('booted-machine');assert boot['screen']=='bag' and boot['field_menu']['phase'].startswith('MachineBoot');tap('a');teach=capture('teach-choice');assert teach['field_menu']['phase'].startswith('MachineTeach');tap('a');party=capture('party-selection');assert party['screen']=='party' and party['field_menu']['phase']=='Browsing'
    assert party['party'][0]['species']=='Bulbasaur' and party['party'][1]['species']=='Caterpie'
    tap('down');st=capture('caterpie-selection');assert st['field_menu']['cursor']==1;tap('b');st=capture('after-cancel');assert st['screen']=='bag'
    for button in ['b','down','down','a']:tap(button)
    assert state('save-open')['screen']=='save'
    d.step(200);tap('a',400);d.step(300);capture('saved');assert (trial/'isolated.sav').exists();flags_after=d.cmd(cmd='get_flags');d.close()
   finally:
    (trial/'states.json').write_text(json.dumps(states,indent=2));(trial/'actions.json').write_text(json.dumps(actions,indent=2));p.terminate();p.wait(timeout=10)
  cp=subprocess.run([str(binary),'import-snapshot','--input',str(trial/'isolated.sav'),'-o',str(trial/'readback.json')],env=env,cwd=trial,capture_output=True);assert cp.returncode==0,cp.stderr;rb=json.loads((trial/'readback.json').read_text())
  for i in range(2):
   for key,value in initial['party'][i].items():assert rb['party'][i][key]==value,(i,key,rb['party'][i][key],value)
  assert rb['game_data']['bag']['items']==[[item,qty]] and rb['game_data']['player_money']==3000
  assert flags_before==flags_after
  ram=(trial/'isolated.sav').read_bytes();checksum=(~sum(ram[0x2598:0x3523]))&255;assert checksum==ram[0x3523]
  record=dict(flow=flow,repeat=repeat,binary_sha256=expected_hash,label=label,scope='Ordinary machine menus, move selection cursor then cancellation and ordinary SAVE; preboot construction only.',fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),initial_party=initial['party'],after_party=rb['party'],after_items=rb['game_data']['bag']['items'],money=3000,actual_flags_before=flags_before,actual_flags_after=flags_after,sidecars_initially_absent=True,private_binary=True,captures=captures,actions=actions,normal_save=True,sram_sha256=hashlib.sha256(ram).hexdigest(),sram_checksum=checksum,party_raw_hex=ram[0x2f34:0x2f8c].hex());(trial/'record.json').write_text(json.dumps(record,indent=2));records.append(record);print(flow,repeat,'complete: actual compatible Bulbasaur/incompatible Caterpie party menu, unchanged parties/item/money, checksum',checksum,flush=True)
for flow in flows:
 a,b=[v for v in records if v['flow']==flow];assert a['sram_sha256']==b['sram_sha256']
 assert a['actions']==b['actions'] and a['after_party']==b['after_party'] and a['captures']=={k:dict(v,path=str(out/f'{flow}-1'/(k+'.png'))) for k,v in b['captures'].items()}
(out/'verification.json').write_text(json.dumps(dict(scope='Native actual TM21/HM01 boot/teach and party selection with compatible Bulbasaur/incompatible Caterpie; select Caterpie cursor then cancel and ordinary SAVE; construction only before boot, two repeats, no state writes in validation; binary identities supplied by the caller and stored in each record.',script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),trials=records),indent=2));print('output',out,flush=True)

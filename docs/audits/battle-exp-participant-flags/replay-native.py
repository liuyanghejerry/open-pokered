from pathlib import Path
import sys,os,json,subprocess,socket,shutil,tempfile,hashlib
root=Path(__file__).resolve().parents[3];r=root/'.task-tmp/fidelity-39-plus';sys.path.insert(0,str(root/'scripts'));from debug_drive import DebugClient
stage=sys.argv[3];chosen=Path(sys.argv[4]);source_head=sys.argv[5];case=sys.argv[1] if len(sys.argv)>1 else 'no-revive';repeat=int(sys.argv[2]) if len(sys.argv)>2 else 1;out=Path(tempfile.mkdtemp(prefix=f'audit-867-native-faint-exp-{stage}-{case}-{repeat}-',dir=r));binary=out/'pokered-app';shutil.copyfile(chosen,binary);binary.chmod(0o755);initial=json.loads((Path(__file__).parent/'fixture-no-revive.json').read_text());lead=initial['party'][0];lead.update(level=5,hp=1,max_hp=20,attack=10,defense=10,speed=10,special=12,moves=['Tackle','Growl','None','None'],pp=[35,40,0,0],total_exp=135,stat_exp=[0]*5);healthy=json.loads(json.dumps(lead));healthy.update(level=20,hp=52,max_hp=52,attack=28,defense=28,speed=25,special=34,moves=['Cut','BodySlam','TakeDown','DoubleEdge'],pp=[30,15,20,15],total_exp=5460);initial['party']=[lead,healthy] if case in ('no-revive','revive','revive-next') else [healthy,lead];initial['game_data']['bag']['items']=[['Revive',1]] if case in ('revive','revive-next') else [];initial['game_data']['obtained_badges']=0;initial['game_data']['options']['battle_style']='Set';initial['game_data']['options']['text_speed']='Fast';initial['game_data']['event_flags'][0x562//8]&=~(1<<(0x562%8));fixture=out/'fixture.json';fixture.write_text(json.dumps(initial,indent=2));env=os.environ.copy();env['LD_LIBRARY_PATH']='/tmp/pokered-build-lib';env['POKERED_MAPS_DIR']=str(root/'crates/pokered-data/maps');sock=socket.socket();sock.bind(('127.0.0.1',0));port=sock.getsockname()[1];sock.close();states=[];actions=[]
with (out/'game.log').open('w') as log:
 p=subprocess.Popen([str(binary),'run','--headless','--speed','0','--seed','42','--debug-port',str(port),'--skip-intro','--snapshot',str(fixture),'--warp','ViridianForest,28,32','--lang','en','--no-audio','--save',str(out/'isolated.sav')],cwd=out,env=env,stdout=log,stderr=subprocess.STDOUT)
 try:
  d=DebugClient(port);d.step(120)
  def state(tag):v=d.state();states.append(dict(tag=tag,state=v));return v
  def tap(button,frames=64,held=1):s=state('pre-'+button);actions.append(dict(frame=s['frame_count'],button=button,held=held,frames=frames));d.press_timeline([button]*held+[None]*(frames-held));d.step(frames)
  s=state('initial');print('out',out,'initial',s['map_name'],s['player_x'],s['player_y'],flush=True);tap('down',64,16);started=False;fainted=False;opened_revive=False;revived=False
  for i in range(500):
   s=state('drive');ph=s['battle_phase'];live=s.get('battle_live') or {};
   if 'gained' in ph and 'current: 2' in ph and not (out/'exp-notice.png').exists():d.cmd(cmd='capture_frame',path=str(out/'exp-notice.png'))
   started=started or s['screen']=='battle'
   if live and live['player_party'][0]['hp']==0 and not fainted:
    fainted=True;print('actual KO',s['frame_count'],ph,flush=True);d.cmd(cmd='capture_frame',path=str(out/'actual-ko.png'))
   if started and s['screen']=='overworld' and s['active_script_effect'] is None and s['dialogue'] is None:break
   if s['screen']!='battle':tap('a')
   elif ph=='PlayerFaintSwitch':tap('a' if s['battle_party_cursor']==1 else 'down')
   elif case in ('revive','revive-next') and fainted and not revived and live and live['player_party'][0]['hp']>0:
    revived=True;d.cmd(cmd='capture_frame',path=str(out/'actual-revive.png'));tap('a')
   elif case in ('revive','revive-next') and fainted and not revived and ph=='PlayerMenu' and (case=='revive' or live['enemy']['species']=='Caterpie'):
    tap('up',8);tap('left',8);tap('down',8);tap('a',8);opened_revive=True
   elif case in ('revive','revive-next') and opened_revive and not revived and ph=='BagSelect':tap('a',32)
   elif case in ('revive','revive-next') and opened_revive and not revived and ph.startswith('ItemTargetSelect'):
    tap('a' if s['battle_party_cursor']==0 else 'up',32)
   elif ph=='PlayerMenu':tap('up',8);tap('left',8);tap('a',8)
   elif ph=='MoveSelect':
    menu=s['battle_moves'];target=0;cursor=menu['cursor'];tap('a' if cursor==target else 'up',32)
   else:tap('a')
  else:raise AssertionError(('battle did not finish',s['screen'],s['battle_phase'],s['map_name'],s['player_x'],s['player_y']))
  s=state('after');d.cmd(cmd='capture_frame',path=str(out/'after.png'));print('after',s['frame_count'],s['party'],flush=True);assert fainted if case in ('no-revive','revive','revive-next') else not fainted;tap('start',60);print('start',state('start')['field_menu'],flush=True)
  for button in ['down','down','down','down','a']:tap(button,32)
  assert state('save-open')['screen']=='save';d.step(200);tap('a',400);d.step(300);d.cmd(cmd='capture_frame',path=str(out/'saved.png'));d.close()
 finally:
  (out/'states.json').write_text(json.dumps(states,indent=2));(out/'actions.json').write_text(json.dumps(actions,indent=2));p.terminate();p.wait(timeout=10)
cp=subprocess.run([str(binary),'import-snapshot','--input',str(out/'isolated.sav'),'-o',str(out/'readback.json')],cwd=out,env=env,capture_output=True);assert cp.returncode==0,cp.stderr;rb=json.loads((out/'readback.json').read_text());assert [m['total_exp'] for m in rb['party']]==([168,5560] if case=='revive' else ([135,5560 if stage=='before' else 5593] if case=='revive-next' else ([135,5560 if stage=='before' else 5593] if case=='no-revive' else [5593,135])));ram=(out/'isolated.sav').read_bytes();assert ram[0x3523]==(~sum(ram[0x2598:0x3523]))&255;(out/'record.json').write_text(json.dumps(dict(case=case,repeat=repeat,initial_party=initial['party'],after_party=rb['party'],after_money=rb['game_data']['player_money'],after_bag=rb['game_data']['bag'],initial_bag=initial['game_data']['bag'],actual_revive=revived,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),frozen_source_commit=source_head,stage=stage,fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),actual_faint=fainted,states=states,actions=actions,normal_save=True,sram_sha256=hashlib.sha256(ram).hexdigest(),sram_checksum=ram[0x3523],party_sram_raw_hex=ram[0x2f34:0x2f8c].hex()),indent=2));(r/f'audit-867-native-faint-exp-{stage}-{case}-{repeat}-path.txt').write_text(str(out)+'\n');print('XP',[(m['hp'],m['total_exp'],m['stat_exp']) for m in rb['party']],flush=True)

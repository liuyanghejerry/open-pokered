"""Replay normal original startup/physical PC controls with readonly hooks.
Requires PyBoy 2.8.1, the pinned original ROM/symbols and disclosed offline fixture.
No runtime memory/register setters, snapshots, warp, RNG replacement or saves.
"""
from pathlib import Path
import argparse,os,re,io,json,gzip,hashlib
os.environ['SDL_AUDIODRIVER']='dummy'
from pyboy import PyBoy
parser=argparse.ArgumentParser(description=__doc__)
for name in ['rom','sym','fixture','controls','out']:parser.add_argument('--'+name,type=Path,required=True)
args=parser.parse_args();blob=args.rom.read_bytes();raw=args.fixture.read_bytes();assert hashlib.sha256(blob).hexdigest()=='5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b';reference=json.loads(gzip.decompress(args.controls.read_bytes()));assert hashlib.sha256(raw).hexdigest()==reference['fixture_sha256'];symbols={}
for line in args.sym.read_text().splitlines():
 m=re.match(r'([0-9a-fA-F]+):([0-9a-fA-F]+) (\S+)',line)
 if m:symbols[m[3]]=(int(m[1],16),int(m[2],16))
trials=[]
for case in reference['cases']:
 p=PyBoy(str(args.rom),ram_file=io.BytesIO(raw),window='null',sound_emulated=False);p.set_emulation_speed(0);stage=['boot'];league=[False];events=[];observations=[];expected={}
 for label,s in case['observations'].items():expected.setdefault(s['frame'],[]).append((label,s))
 def get(n):return int(p.memory[symbols[n][1]])
 def state():return {'frame':p.frame_count,'stage':stage[0],'player':[get('wCurMap'),get('wXCoord'),get('wYCoord')],'mon':get('wHoFMonSpecies'),'channels':list(p.memory[symbols['wChannelSoundIDs'][1]+4:symbols['wChannelSoundIDs'][1]+8]),'joy':{n:get(n)for n in ['hJoyHeld','hJoyPressed','hJoy5','hJoy6','hJoy7']},'menu_item':get('wCurrentMenuItem'),'max_menu_item':get('wMaxMenuItem')}
 def hook(label):
  if label=='pc':stage[0]='drawing_menu'
  elif label=='menu'and stage[0]=='drawing_menu':stage[0]='menu_ready'
  elif label=='league':league[0]=True;stage[0]='league'
  elif league[0]and label=='cry':stage[0]='cry'
  elif league[0]and label=='ack':stage[0]='ack'
  else:return
  events.append({'event':label,'state':state()})
 for label,symbol in [('pc','PCMainMenu'),('menu','HandleMenuInput'),('league','LeaguePCShowTeam'),('cry','PlayCry'),('ack','WaitForTextScrollButtonPress')]:p.hook_register(*symbols[symbol],hook,label)
 try:
  for command in case['inputs']:
   assert p.frame_count==command['frame']
   if command['key']:p.button_press(command['key'])
   for _ in range(command['frames']):
    p.tick(1,False)
    for label,want in expected.get(p.frame_count,[]):
     actual=state();assert actual==want,(case['mode'],label,actual,want);observations.append({'label':label,'state':actual})
   if command['key']:p.button_release(command['key'])
  assert events==case['events'];assert len(observations)==len(case['observations']);trials.append({'mode':case['mode'],'all_pass':True,'events':events,'observations':observations})
 finally:p.stop(save=False)
result={'scope':__doc__,'all_pass':True,'source_ref':'fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c','rom_sha256':hashlib.sha256(blob).hexdigest(),'fixture_sha256':hashlib.sha256(raw).hexdigest(),'rom_unchanged':args.rom.read_bytes()==blob,'fixture_unchanged':args.fixture.read_bytes()==raw,'cases':trials};assert result['rom_unchanged']and result['fixture_unchanged'];args.out.write_bytes(gzip.compress(json.dumps(result,indent=2).encode(),mtime=0));print({'all_pass':True,'cases':[x['mode']for x in trials]},flush=True)

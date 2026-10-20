"""Run from workspace root. Normal Continue; buttons and read-only RPCs only."""
from pathlib import Path
import sys,os,json,gzip,hashlib,tempfile,shutil,subprocess,traceback
root=Path.cwd();sys.path.insert(0,str(root/'scripts'));import playthrough as p
binary=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();fixtures=Path(sys.argv[3]).resolve();out.mkdir(parents=True,exist_ok=True)
for k in list(os.environ):
 if k.startswith('POKERED_'):del os.environ[k]
raw=gzip.decompress((fixtures/'fixture.json.gz').read_bytes());baseline=json.loads(gzip.decompress((fixtures/'prefix.json.gz').read_bytes()));mode=sys.argv[4] if len(sys.argv)>4 else 'after';assert mode in ['before','after']
AGREED='EVENT_BILL_SAID_USE_CELL_SEPARATOR';USED='EVENT_USED_CELL_SEPARATOR_ON_BILL';MET='EVENT_MET_BILL'
r={'source':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256(raw).hexdigest(),'scope':'Offline fixture, normal boot/Continue and actual buttons/read-only observations. Source comparison, not original-ROM runtime or fresh mainline. Simultaneous-key priority belongs to the existing FieldYesNo unit suite; this RPC accepts one key per frame.','cases':[],'complete':False}
try:
 for case in (['before'] if mode=='before' else ['yes','no','held-outer-a']):
  row={'case':case,'commands':[],'observations':[],'complete':False};r['cases'].append(row)
  with tempfile.TemporaryDirectory(prefix='bill-help-') as tmp:
   path=Path(tmp)/'fixture.json';path.write_bytes(raw);g=p.Game(binary=binary,snapshot=path,seed=42,speed=0);original=g.d.cmd
   def cmd(*a,**kw):
    assert kw['cmd'] in {'get_state','get_npcs','get_flags','get_bag','press_sequence','press_timeline','press','step_frames','wait_until','capture_frame'}
    row['commands'].append(kw);return original(*a,**kw)
   g.d.cmd=cmd
   def observe(label,capture=True):
    s=g.st();row['observations'].append({'label':label,'state':s})
    if capture:g.d.cmd(cmd='capture_frame',path=str(out/f'{case}-{label}.png'))
    return s
   def flag():
    f=g.d.cmd(cmd='get_flags')['data'];return {k:bool(f.get(k,False))for k in [AGREED,USED,MET]}
   def step(key=None,count=1):g.d.cmd(cmd='press_timeline',buttons=[key]*count,advance=True)
   try:
    for request in baseline['commands']:
     if request['cmd']=='capture_frame':break
     g.d.cmd(**request)
    assert g.st()['frame_count']==804
    # Both executables receive the same fixed timeline: each intermediate
    # page gets 240 neutral frames, one A frame and eight neutral frames.
    # This completes PrintLetterDelay and ProtectedDelay3 before each press.
    for page in range(1,7):
     step(count=240);s=g.st();d=s['dialogue_state']
     assert d and d['page']==page and d['total_pages']==7 and d['waiting_for_input'],s
     step('a');step(count=8)
    step(count=240)
    initial_bag=g.d.cmd(cmd='get_bag')['data'];s=observe('question')
    assert s['frame_count']==2538 and g.pos()==('BillsHouse',6,6) and s['player_facing']=='Up'
    assert not any(flag().values())
    if mode=='before':
     assert s['choice'] is None and s['dialogue_state']['waiting_for_input'] and s['dialogue_state']['page']==7
     assert 'here!' in s['dialogue'];row.update(complete=True,final_flags=flag());continue
    assert s['choice'] and s['choice']['options']==['YES','NO'] and s['choice']['input_enabled'],s
    assert s['script_effect']['effect']=='AskFieldYesNo'
    step('b' if case=='no' else 'a');s=observe('accepted',False);accept=s['frame_count'];assert s['audio_sfx']=={'id':63,'playing':True}
    assert s['choice'] and not s['choice']['input_enabled']
    step(count=14);s=observe('completion-14',False);assert s['frame_count']==accept+14 and s['choice']
    step();s=observe('completion-15',False);assert s['frame_count']==accept+15 and s['choice'] is None
    effect=s['script_effect'];assert effect and effect['effect']==('PrintFieldText' if case=='no' else 'ShowDialogue')
    assert effect['text'].startswith('No!?' if case=='no' else "When I'm in the TELEPORTER")
    row['menu_timing']={'accepted':accept,'still_menu':accept+14,'returned':accept+15}
    refused=False
    for read in range(200):
     s=g.st();d=s['dialogue_state'];effect=s['script_effect'] or {};text=effect.get('text','')
     if text.startswith('No!?'):
      refused=True
      if d and d['waiting_for_input'] and not row.get('refusal_captured'):
       observe('refusal');row['refusal_captured']=True
     instructions=effect.get('effect')=='ShowDialogue' and text.startswith("When I'm in the TELEPORTER")
     if instructions and d and d['waiting_for_input'] and d['page']==d['total_pages']:break
     if d and d['waiting_for_input']:
      g.step(10);step('a');g.step(4)
     else:g.step(30)
    else:raise AssertionError('instructions did not finish printing')
    assert refused==(case=='no');g.step(40);s=observe('instructions-outer-ack');before_audio=s['audio_sfx'];assert not before_audio['playing'];assert not any(flag().values())
    if case=='held-outer-a':
     step('a',20);s=observe('held-outer-ack');assert s['dialogue_state']['holding_open'] and not any(flag().values())
     assert s['audio_sfx']==before_audio;step()
    else:
     step('b');s=observe('silent-outer-ack',False);assert s['audio_sfx']==before_audio
    g.step(300);g.wait('control_ready',600);s=observe('bill-in-machine');assert g.pos()==('BillsHouse',6,6)
    assert flag()=={AGREED:True,USED:False,MET:False};npcs=g.d.cmd(cmd='get_npcs')['data'];bill=next(n for n in npcs if n['text_id']==1)
    assert not bill['visible'] and (bill['x'],bill['y'])==(6,2);assert all(not n['visible'] for n in npcs)
    assert g.d.cmd(cmd='get_bag')['data']==initial_bag;assert path.read_bytes()==raw
    step('right',32);s=g.st();assert s['player_x']>6 and s['player_y']==6 and s['map_name']=='BillsHouse'
    row.update(complete=True,refusal_seen=refused,final_flags=flag(),final_npcs=npcs,control_position=[s['player_x'],s['player_y']])
   finally:g.log.flush();shutil.copyfile(g.run_dir/'game.log',out/f'{case}.log');g.close()
 r['complete']=True
except BaseException:r['failure']=traceback.format_exc();print(r['failure'],flush=True)
finally:(out/'verification.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'complete':r['complete'],'cases':len(r['cases'])}),flush=True)
raise SystemExit(0 if r['complete']else 1)

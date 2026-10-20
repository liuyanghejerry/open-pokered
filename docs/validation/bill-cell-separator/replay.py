"""Run from workspace root: native ordinary Continue, actual input/read-only RPCs."""
from pathlib import Path
import sys,os,json,gzip,hashlib,tempfile,shutil,traceback,subprocess
root=Path.cwd();sys.path.insert(0,str(root/'scripts'));import playthrough as p
binary=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();fixtures=Path(sys.argv[3]).resolve();out.mkdir(parents=True,exist_ok=True)
for k in list(os.environ):
 if k.startswith('POKERED_'):del os.environ[k]
USED='EVENT_USED_CELL_SEPARATOR_ON_BILL';MET='EVENT_MET_BILL';MET2='EVENT_MET_BILL_2';machine_ids=[72,59,71,59,57]
r={'workspace_head':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'scope':'Offline native fixtures, normal boot/Continue, buttons/read-only observations. PCM/music proven by companion cold-SRAM app tests, not this RPC. Not original-ROM timing proof.','cases':[],'complete':False}
try:
 for case in ['up','left']:
  raw=gzip.decompress((fixtures/f'fixture-{case}.json.gz').read_bytes());row={'case':case,'fixture_sha256':hashlib.sha256(raw).hexdigest(),'commands':[],'states':[],'events':[],'complete':False};r['cases'].append(row)
  with tempfile.TemporaryDirectory(prefix='bill-separator-') as tmp:
   path=Path(tmp)/'fixture.json';path.write_bytes(raw);g=p.Game(binary=binary,snapshot=path,seed=42,speed=0);original=g.d.cmd
   def cmd(*a,**kw):
    assert kw['cmd'] in {'get_state','get_npcs','get_flags','get_bag','press_sequence','press_timeline','press','step_frames','wait_until','capture_frame'}
    row['commands'].append(kw);return original(*a,**kw)
   g.d.cmd=cmd
   def shot(label):g.d.cmd(cmd='capture_frame',path=str(out/f'{case}-{label}.png'))
   def flags():
    f=g.d.cmd(cmd='get_flags')['data'];return {k:bool(f.get(k,False)) for k in [USED,MET,MET2]}
   try:
    # The former adaptive tap stops when a dialogue opens. Replay the actual
    # baseline prefix so a silent side rejection receives identical input.
    before=json.loads(gzip.decompress((fixtures/f'before-{case}.json.gz').read_bytes()))
    for request in before['commands']:
     if request['cmd']=='capture_frame':break
     g.d.cmd(**request)
    expected_pos=('BillsHouse',1,5) if case=='up' else ('BillsHouse',2,4);assert g.pos()==expected_pos
    initial_bag=g.d.cmd(cmd='get_bag')['data'];s=g.st();row['comparison']=s
    assert s['frame_count']==804 and s['player_facing'].lower()==case;shot('initial')
    if case=='left':
     assert s['screen']=='overworld' and not s['dialogue'] and s['dialogue_state'] is None
     assert not any(flags().values());g.step(300);assert not any(flags().values())
     assert g.st()['dialogue_state'] is None
     assert all(not n['visible'] for n in g.d.cmd(cmd='get_npcs')['data'])
    else:
     assert s['screen']=='overworld' and s['player_name']+' initiated' in s['dialogue']
     texts=[];sounds=[];previous_sound=None;previous_phase=None;f=flags();visible_delay=0;sent_start=False;sent_right=False
     for index in range(6000):
      s=g.st();details=s.get('script_effect') or {};kind=details.get('effect') or s.get('active_script_effect')
      phase=(kind,details.get('frames'));audio=s['audio_sfx'];playing=audio and audio['playing'];id=audio['id'] if audio else None
      if playing and id!=previous_sound:
       if id!=63:sounds.append([s['frame_count'],id])
      previous_sound=id if playing else None
      if s['dialogue'] and (not texts or texts[-1]!=s['dialogue']):texts.append(s['dialogue'])
      if phase!=previous_phase:
       f=flags();npcs=g.d.cmd(cmd='get_npcs')['data'];row['events'].append({'frame':s['frame_count'],'effect':details,'flags':f,'npcs':npcs,'dialogue':s['dialogue'],'audio':audio})
       if id==72 and playing:shot('switch')
       previous_phase=phase
      npcs=g.d.cmd(cmd='get_npcs')['data'] if f[USED] else None
      if phase==('Delay',8):
       bill=next(n for n in npcs if n['text_id']==2);assert bill['visible'] and (bill['x'],bill['y'])==(1,2)
       assert f[USED] and not f[MET];visible_delay+=1
       if visible_delay==1:shot('bill-visible-before-walk')
      row['states'].append({k:s[k] for k in ['frame_count','screen','player_x','player_y','dialogue','dialogue_state','active_script_effect','script_effect','audio_sfx']})
      assert s['screen']=='overworld' and (s['player_x'],s['player_y'])==(1,5)
      if f[MET] and not s['script_running'] and s['active_script_effect'] is None:break
      if str(kind).lower()=='printfieldtext' and s['dialogue_state'] and s['dialogue_state']['waiting_for_input']:
       button='a' if index%4==0 else None
      elif str(kind).lower()=='waitsound' and not sent_start:button='start';sent_start=True
      elif str(kind).lower()=='delay' and not sent_right:button='right';sent_right=True
      else:button=None
      g.d.cmd(cmd='press_timeline',buttons=[button],advance=True)
     else:raise AssertionError('separator did not complete')
     assert [id for frame,id in sounds]==machine_ids,sounds
     assert visible_delay==8,visible_delay
     assert all(flags().values());assert all('back to normal' not in t and 'is running...' not in t for t in texts)
     row.update(sounds=sounds,visible_delay_frames=visible_delay,dialogue_pages=texts,blocked_start=sent_start,blocked_right=sent_right)
     assert sent_start and sent_right
     bill=next(n for n in g.d.cmd(cmd='get_npcs')['data'] if n['text_id']==2);assert bill['visible'] and (bill['x'],bill['y'])==(4,4);shot('finished')
    assert g.d.cmd(cmd='get_bag')['data']==initial_bag
    g.wait('control_ready',600);g.d.drive(['right']*32,frames=32);s=g.st();assert s['map_name']=='BillsHouse' and s['player_y']==expected_pos[2] and s['player_x']>expected_pos[1]
    row.update(complete=True,final_flags=flags(),control_position=[s['player_x'],s['player_y']])
   finally:g.log.flush();shutil.copy2(g.run_dir/'game.log',out/f'{case}.log');g.close()
 r['complete']=True
except BaseException:r['failure']=traceback.format_exc();print(r['failure'],flush=True)
finally:(out/'verification.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'complete':r['complete'],'cases':len(r['cases'])}),flush=True)
raise SystemExit(0 if r['complete'] else 1)

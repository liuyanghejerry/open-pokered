from pathlib import Path
import sys,os,json,tempfile,shutil,traceback,hashlib
D=Path(__file__).resolve().parent;R=D.parents[2]
sys.path.insert(0,str(R/'scripts'));import playthrough as p
role=sys.argv[1];binary=Path(sys.argv[2]).resolve();out=Path(sys.argv[3]).resolve();out.mkdir();(out/'tmp').mkdir();tempfile.tempdir=str(out/'tmp')
for k in list(os.environ):
 if k.startswith('POKERED_'):del os.environ[k]
x={'all_pass':False,'role':role,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'scope':'two constructed persistent fixtures on walkable adjacent tiles; ordinary Continue, actual NPC dialogue, menu inputs and script rewards; not fresh route','cases':[]}
def open_menu(g):
 for _ in range(300):
  s=g.st()
  if s['screen']=='filter-bag':g.step(30);return
  if s.get('choice'):
   g.tap('up',12);first=g.st()['choice']['selected']
   g.tap('up',12);second=g.st()['choice']['selected']
   data.setdefault('question_probes',[]).append({'up_at_first':first,'two_ups':second})
   assert first==(1 if role=='before'else 0) and second==0
   g.choose('YES')
  elif(s.get('dialogue_state')or{}).get('waiting_for_input'):g.tap('a',4)
  elif s.get('dialogue_state')or s.get('script_running')or s.get('active_script_effect'):g.step(12)
  else:
   target_id=1 if s['map_name']=='CinnabarLabFossilRoom'else 2
   npcs=g.d.npcs();npc=next(n for n in npcs if n['text_id']==target_id and n['visible'])
   try:g.approach_object(npc['x'],npc['y'],s['map_name'])
   except p.NavError:g.step(12);continue
   current=next(n for n in g.d.npcs()if n['text_id']==target_id and n['visible'])
   if(current['x'],current['y'])!=(npc['x'],npc['y'])or current['walk_counter']!=0:g.step(2);continue
   g.tap('a',4)
 raise AssertionError(('no filtered menu',g.st()))
try:
 for kind in ['fossils','drinks']:
  data={'kind':kind,'commands':{}};x['cases'].append(data);g=None
  try:
   g=p.Game(binary=binary,snapshot=D/(kind+'-fixture.json'),seed=42,speed=0);data['Game_pid']=g.proc.pid;data['argv']=g.proc.args
   original=g.d.cmd
   def cmd(*args,**kw):
    name=kw.get('cmd',args[0]if args else None);assert name in {'get_state','get_npcs','get_flags','get_party','get_bag','press_sequence','press_timeline','step_frames','wait_until','skip_dialogue','capture_frame'},name;data['commands'][name]=data['commands'].get(name,0)+1;r=original(*args,**kw);assert r['ok'],r
    if name=='get_state':(out/'last-state.json').write_text(json.dumps(r['data'],indent=2)+'\n')
    return r
   g.d.cmd=cmd;p.resume_reentry(g);assert g.cutscene();before_bag=g.d.cmd(cmd='get_bag')['data'];open_menu(g);data['opened']=g.st()
   for _ in range(3):g.tap('down',12)
   data['bottom']=g.st();assert g.d.cmd(cmd='capture_frame',path=str(out/(kind+'-bottom.png')))['ok']
   g.tap('b',4);assert g.cutscene();assert g.d.cmd(cmd='get_bag')['data']==before_bag;data['B_cancel_bag_unchanged']=True
   open_menu(g);g.tap('up',12);data['top']=g.st();assert g.d.cmd(cmd='capture_frame',path=str(out/(kind+'-top.png')))['ok'];g.tap('a',4)
   if kind=='fossils':g.dialogue_then_choice();g.choose('YES')
   assert g.cutscene();flags=g.d.cmd(cmd='get_flags')['data'];data['result_flags']={f:flags.get(f,False)for f in ['EVENT_REVIVING_KABUTO','EVENT_REVIVING_OMANYTE','EVENT_REVIVING_AERODACTYL','EVENT_GOT_TM13','EVENT_GOT_TM48','EVENT_GOT_TM49']};data['after_bag']=g.d.cmd(cmd='get_bag')['data']
   expected=('EVENT_REVIVING_AERODACTYL'if role=='before'else'EVENT_REVIVING_KABUTO')if kind=='fossils'else('EVENT_GOT_TM49'if role=='before'else'EVENT_GOT_TM13');assert flags.get(expected),data
   data['all_pass']=True;print(kind,'pass',data['result_flags'],flush=True)
  finally:
   if g:
    g.log.flush();shutil.copy2(g.run_dir/'game.log',out/(kind+'-game.log'));g.close();data['Game_exit']=g.proc.returncode
 x['all_pass']=True
except BaseException:x['failure']=traceback.format_exc();print(x['failure'],flush=True)
finally:(out/'result.json').write_text(json.dumps(x,indent=2)+'\n')
print(json.dumps({'all_pass':x['all_pass'],'result':str(out/'result.json')}),flush=True)
raise SystemExit(0 if x['all_pass']else 1)

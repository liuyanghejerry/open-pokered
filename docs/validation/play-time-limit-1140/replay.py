from pathlib import Path
import sys,os,json,tempfile,shutil,traceback,hashlib,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;sys.path.insert(0,str(R/'scripts'));import playthrough as p
binary=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();expected=int(sys.argv[3]);out.mkdir();(out/'tmp').mkdir();tempfile.tempdir=str(out/'tmp');origp=subprocess.Popen
for k in list(os.environ):
 if k.startswith('POKERED_'):del os.environ[k]
def popen(args,**kw):
 if isinstance(args,list)and Path(args[0]).resolve()==binary:kw['cwd']=str(R);kw['stderr']=open(out/'process-stderr.log','w')
 return origp(args,**kw)
subprocess.Popen=popen
src=D/'fixture.sav';save=out/'play.sav';shutil.copy2(src,save);x={'all_pass':False,'expected_minutes':expected,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256(src.read_bytes()).hexdigest(),'commands':{},'scope':'ordinary CLI Continue, trainer card and UI Save + independent Continue; offline boundary SRAM fixture; no runtime state setters, debug save or skip-intro'};g=None
try:
 def connect():
  global g
  g=p.Game(binary=binary,save_path=save,seed=42,speed=0);orig=g.d.cmd
  def cmd(*a,**kw):
   n=kw.get('cmd',a[0]if a else None);assert n in {'get_state','get_npcs','get_flags','get_party','get_bag','export_fixture','press_sequence','press_timeline','step_frames','wait_until','skip_dialogue','capture_frame'},n;x['commands'][n]=x['commands'].get(n,0)+1;return orig(*a,**kw)
  g.d.cmd=cmd;p.resume_reentry(g);assert g.cutscene();g.step(64);return g.st()
 def time_now():
  y=g.d.cmd(cmd='export_fixture');assert y['ok'];return y['data']['game_data']['play_time']
 def want_time():
  y=time_now();assert y=={'hours':255,'maxed':True,'minutes':expected,'seconds':expected,'frames':expected},y;return y
 def select(item):
  s=g.st();assert s['screen']=='start-menu';target=s['field_menu']['items'].index(item)
  for _ in range(12):
   if g.st()['field_menu']['cursor']==target:break
   g.tap('down',16)
  assert g.st()['field_menu']['cursor']==target;g.tap('a',32);return g.st()
 x['initial']=connect();x['time_after_continue']=want_time();g.tap('start',32);x['menu']=g.st();s=select('TrainerInfo');x['trainer_card']=s;assert 'trainer'in s['screen'];assert g.d.cmd(cmd='capture_frame',path=str(out/'trainer-card.png'))['ok'];x['time_on_card']=want_time();g.tap('b',32);x['card_return']=g.st();print('CARD_RETURN',json.dumps(x['card_return'].get('field_menu')),x['card_return']['screen'],flush=True)
 if g.st()['screen']=='overworld':g.tap('start',32)
 s=select('Save');x['save_start']=s;print('SAVE_START',json.dumps(s),flush=True)
 for _ in range(40):
  g.tap('a',32);s=g.st()
  if save.read_bytes()!=src.read_bytes()and s['screen'] in ['overworld','start-menu']:break
 x['save_end']=s;x['time_after_save']=want_time();assert save.read_bytes()!=src.read_bytes();saved=save.read_bytes();assert len(saved)==32768;assert saved[0x2ced:0x2cf2]==bytes([255,255,expected,expected,expected]),list(saved[0x2ced:0x2cf2]);x['saved_sha256']=hashlib.sha256(saved).hexdigest();x['saved_time_bytes']=list(saved[0x2ced:0x2cf2]);g.close();x['first_cleanup_exit']=g.proc.returncode;g=None
 x['independent_continue']=connect();x['time_after_independent_continue']=want_time();assert hashlib.sha256(save.read_bytes()).hexdigest()==x['saved_sha256'];x['all_pass']=True
except BaseException:x['failure']=traceback.format_exc();print(x['failure'],flush=True)
finally:
 if g:g.close();x['owner_cleanup_exit']=g.proc.returncode
 (out/'result.json').write_text(json.dumps(x,indent=2)+'\n')
print(json.dumps({'all_pass':x['all_pass'],'result':str(out/'result.json')}),flush=True);raise SystemExit(0 if x['all_pass']else 1)

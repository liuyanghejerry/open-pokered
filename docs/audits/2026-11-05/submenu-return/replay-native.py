import argparse, hashlib, json, gzip, os, socket, subprocess, sys, time
from pathlib import Path
w=Path(__file__).resolve().parents[4]
a=argparse.ArgumentParser();a.add_argument('--repo',type=Path,default=w);a.add_argument('--binary',type=Path,required=True);a.add_argument('--sha256',required=True);a.add_argument('--output',type=Path,required=True);a.add_argument('--candidate',action='store_true');a.add_argument('--fixture',type=Path,default=w/'docs/audits/2026-11-05/start-menu-close/fixture.sav');a.add_argument('--case',choices=['bag-neutral','bag-short','option-neutral','option-short']);args=a.parse_args()
w=args.repo;binary=args.binary;out=args.output;out.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(binary)==args.sha256

def writegz(path,data):
 path.write_bytes(gzip.compress((json.dumps(data,separators=(',',':'))+'\n').encode(),mtime=0))
class Native:
    def __init__(self, folder, save, snapshot=None):
        self.folder = folder
        folder.mkdir(parents=True, exist_ok=False)
        self.commands = []
        with socket.socket() as spare:
            spare.bind(('127.0.0.1', 0)); port = spare.getsockname()[1]
        self.argv = [str(binary), 'run', '--headless', '--debug-port', str(port), '--no-audio', '--save', str(save), '--seed', '42', '--speed', '0', '--lang', 'en']
        if snapshot: self.argv += ['--snapshot', str(snapshot), '--skip-intro']
        self.log = (folder / 'process.log').open('w')
        self.process = subprocess.Popen(self.argv, cwd=w, env={k:v for k,v in os.environ.items() if not k.startswith('POKERED_')}, stdout=self.log, stderr=self.log)
        try:
            end = time.monotonic() + 25
            while True:
                try:
                    self.sock = socket.create_connection(('127.0.0.1', port), timeout=1); break
                except OSError:
                    assert self.process.poll() is None, 'game exited during startup'
                    if time.monotonic() > end: raise
                    time.sleep(.05)
            self.sock.settimeout(45)
            self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            self.stream = self.sock.makefile('rw')
        except BaseException:
            self.close(); raise
    def cmd(self, **args):
        self.commands.append(args)
        self.stream.write(json.dumps(args) + '\n'); self.stream.flush()
        line = self.stream.readline(); assert line, 'closed debug connection'
        result = json.loads(line); assert result.get('ok', True) and not result.get('error'), result
        if hasattr(socket, 'TCP_QUICKACK'): self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_QUICKACK, 1)
        return result
    def state(self): return self.cmd(cmd='get_state')['data']
    def drive(self, keys): self.cmd(cmd='press_timeline', buttons=keys, advance=True)
    def boot(self):
        for _ in range(100):
            if self.state()['screen'] == 'overworld': return
            self.drive(['a', 'a'] + [None] * 30)
        raise AssertionError('ordinary CONTINUE did not reach field')
    def capture(self, name):
        path = self.folder / (name + '.png')
        self.cmd(cmd='capture_frame', path=str(path)); return sha(path)
    def close(self):
        if getattr(self, 'stream', None): self.stream.close()
        if getattr(self, 'sock', None): self.sock.close()
        self.process.terminate()
        try: self.process.wait(timeout=5)
        except subprocess.TimeoutExpired: self.process.kill(); self.process.wait(timeout=5)
        self.log.close()

fixture=args.fixture
fixture_sha=sha(fixture)
results=[]
for menu,button_name,expected in [('bag','Item','bag'),('option','Option','options')]:
 for variant in ['neutral','short']:
  case=menu+'-'+variant;records=[]
  if args.case is not None and case != args.case: continue
  for repeat in [1,2]:
   save=out/f'{case}-{repeat}.sav';save.write_bytes(fixture.read_bytes());game=Native(out/f'{case}-{repeat}',save)
   record={'case':case,'frames':[],'images':{}}
   try:
    game.boot();game.drive([None]*64);game.drive(['start','start']+[None]*96)
    for _ in range(8):
     st=game.state()
     if st['field_menu']['items'][st['field_menu']['cursor']]==button_name:break
     game.drive(['down','down']+[None]*24)
    st=game.state();assert st['field_menu']['items'][st['field_menu']['cursor']]==button_name,st
    record['selected_start']=st['field_menu']
    game.drive(['a','a']+[None]*240)
    assert game.state()['screen']==expected,game.state()
    record['initial']=game.state();record['images']['initial']=game.capture('initial')
    for tick in range(80):
     key='b' if tick<2 else 'down' if variant=='short' and tick in [2,3] else None
     game.drive([key]);st=game.state()
     row={k:st.get(k) for k in ['frame_count','screen','map_name','player_x','player_y','player_facing','player_movement_state','field_menu','field_restore','field_flash','source_foreground']}
     row.update(tick=tick,input=key,save_sha256=sha(save));record['frames'].append(row)
     for name in ['field_restore','field_flash']:assert not (st.get(name) or {}).get('error'),st
     record['images'][str(tick)]=game.capture(f'frame-{tick:03}')
    final=game.state();assert final['screen']=='start-menu',final
    assert final['field_menu']['ready_for_input'],final
    assert all(x['frame_count']==record['initial']['frame_count']+i+1 for i,x in enumerate(record['frames']))
    assert all((x['player_x'],x['player_y'],x['player_facing'])==(record['initial']['player_x'],record['initial']['player_y'],record['initial']['player_facing']) for x in record['frames'])
    assert sha(save)==fixture_sha
    record['final']=final
    if args.candidate:
     assert final['field_menu']['cursor']==record['selected_start']['cursor'],record['frames'][-1]
     game.drive(['a','a']+[None]*96);assert game.state()['screen']==expected,game.state()
     record['after_reopen']=game.state();record['images']['after-reopen']=game.capture('after-reopen')
   finally:
    record['commands']=game.commands;game.close();writegz(out/f'{case}-{repeat}/record.json.gz',record)
   records.append(record)
  assert records[0]['frames']==records[1]['frames'] and records[0]['images']==records[1]['images'],case
  row={'case':case,'repeats':2,'first_start':next(x['tick'] for x in record['frames'] if x['screen']=='start-menu'),'first_ready':next(x['tick'] for x in record['frames'] if x['screen']=='start-menu' and x['field_menu']['ready_for_input']),'final_cursor':record['final']['field_menu']['cursor'],'selected_cursor':record['selected_start']['cursor']};results.append(row);print(json.dumps(row),flush=True)
assert sha(binary)==args.sha256
(out/'result.json').write_text(json.dumps({'status':'passed','binary_sha256':sha(binary),'script_sha256':sha(Path(__file__)),'fixture_sha256':fixture_sha,'cases':results,'scope':'Cold CONTINUE and actual START -> Bag/Option -> B; optional Down2..3 during restoration. No recording-time state setters. All 80 physical update frames have matching captures/state; repeat twice. SRAM stays unchanged.'},indent=2)+'\n')

import argparse,hashlib,json,gzip,os,socket,subprocess,time
from pathlib import Path
w=Path(__file__).resolve().parents[4]
a=argparse.ArgumentParser();a.add_argument('--binary',type=Path,required=True);a.add_argument('--sha256',required=True);a.add_argument('--output',type=Path,required=True);args=a.parse_args()
binary=args.binary;out=args.output;out.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(binary)==args.sha256
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

fixture=w/'docs/audits/2026-11-05/start-menu-close/fixture.sav';summary=[]
for preamble,screen in [('Pokedex','pokedex'),('TrainerInfo','trainer-card'),('Pokemon','party')]:
 records=[]
 for repeat in [1,2]:
  save=out/f'{preamble}-{repeat}.sav';save.write_bytes(fixture.read_bytes());game=Native(out/f'{preamble}-{repeat}',save)
  record={'preamble':preamble,'frames':[],'images':{}}
  try:
   game.boot();game.drive([None]*64);game.drive(['start','start']+[None]*96)
   def select(item):
    for _ in range(8):
     menu=game.state()['field_menu']
     if menu['items'][menu['cursor']]==item:return
     game.drive(['down','down']+[None]*24)
    raise AssertionError(item)
   select(preamble);game.drive(['a','a']+[None]*240);assert game.state()['screen']==screen
   game.drive(['b','b']+[None]*160);assert game.state()['screen']=='start-menu'
   select('Save');game.drive(['a','a']+[None]*240);assert game.state()['screen']=='save'
   game.drive(['down','down']+[None]*24);game.drive(['a']*80)
   record['initial']=game.state();assert record['initial']['screen']=='save'
   flow=record['initial'].get('field_restore') or record['initial'].get('field_flash');assert flow['operation']=='WaitForARelease'
   record['images']['initial']=game.capture('initial')
   for tick in range(40):
    game.drive([None]);st=game.state();flow=st.get('field_restore') or st.get('field_flash')
    record['frames'].append({'tick':tick,'frame_count':st['frame_count'],'screen':st['screen'],'x':st['player_x'],'y':st['player_y'],'operation':(flow or {}).get('operation'),'foreground':st.get('source_foreground'),'irq':(flow or {}).get('irq'),'error':(flow or {}).get('error')})
    assert not record['frames'][-1]['error'];record['images'][str(tick)]=game.capture(f'frame-{tick:03}')
   assert game.state()['screen']=='overworld'
   assert sha(save)==sha(fixture)
   assert all((f['x'],f['y'])==(record['initial']['player_x'],record['initial']['player_y']) for f in record['frames'])
  finally:game.close()
  (out/f'{preamble}-{repeat}.json.gz').write_bytes(gzip.compress(json.dumps(record,separators=(',',':')).encode(),mtime=0));records.append(record)
 assert records[0]==records[1],preamble
 summary.append({'preamble':preamble,'first_map':next(f['tick'] for f in record['frames'] if f['screen']=='overworld'),'first_close':next(f['tick'] for f in record['frames'] if f['operation']!='WaitForARelease')})
 print(json.dumps(summary[-1]),flush=True)
assert sha(binary)==args.sha256
(out/'result.json').write_text(json.dumps({'status':'passed','scope':'Actual previous menu and SAVE NO/A held80; release trigger then40 consecutive actualframes x2, no state setters.','cases':summary},indent=2))

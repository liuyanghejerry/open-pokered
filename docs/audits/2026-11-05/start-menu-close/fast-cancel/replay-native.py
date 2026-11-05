import hashlib,json,gzip,os,socket,subprocess,time
from pathlib import Path
import argparse
args=argparse.ArgumentParser()
args.add_argument('--binary',type=Path,required=True)
args.add_argument('--sha256',required=True)
args.add_argument('--output',type=Path,required=True)
args.add_argument('--expect-fixed',action='store_true')
args=args.parse_args()
w=Path(__file__).resolve().parents[5]
binary=args.binary;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(binary)==args.sha256
out=args.output;out.mkdir(exist_ok=False)
fixture=Path(__file__).resolve().parent.parent/'fixture.sav'
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

for repeat in [1,2]:
 save=out/f'play-{repeat}.sav';save.write_bytes(fixture.read_bytes());g=Native(out/f'run-{repeat}',save);records=[]
 try:
  g.boot();g.drive([None]*64);g.drive(['start','start']);
  for tick in range(64):
   st=g.state();records.append({'phase':'open','tick':tick,'state':st})
   if st['screen']=='start-menu' and st['field_menu']['ready_for_input']:break
   g.drive([None])
  else:raise AssertionError('no START readiness')
  g.capture('before-b');g.drive(['b'])
  for tick in range(80):
   st=g.state();records.append({'phase':'close','tick':tick,'state':st})
   g.drive([None])
  g.capture('after-b-80')
  if args.expect_fixed:
   assert g.state()['screen']=='overworld'
   assert not any((r['state'].get('field_flash') or {}).get('error') for r in records)
   pos=(g.state()['player_x'],g.state()['player_y']);g.drive(['right']*64+[None]*16)
   assert (g.state()['player_x'],g.state()['player_y'])!=pos
   g.capture('after-walk');g.drive(['start','start']+[None]*96)
   assert g.state()['screen']=='start-menu' and g.state()['field_menu']['ready_for_input']
   g.capture('after-reopen');assert sha(save)==sha(fixture)
  st=g.state();print(json.dumps({'repeat':repeat,'final_screen':st['screen'],'error':(st.get('field_flash')or{}).get('error'),'clock':st.get('source_display')}),flush=True)
 finally:
  g.close();(out/f'run-{repeat}.json.gz').write_bytes(gzip.compress(json.dumps({'frames':records,'commands':g.commands}).encode(),mtime=0))

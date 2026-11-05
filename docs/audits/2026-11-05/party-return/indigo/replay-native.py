import argparse, hashlib, json, gzip, os, socket, subprocess, sys, time
from pathlib import Path
a=argparse.ArgumentParser();a.add_argument('--binary',type=Path,required=True);a.add_argument('--sha256',required=True);a.add_argument('--output',type=Path,required=True);a.add_argument('--cwd',type=Path,default=Path(__file__).resolve().parents[5]);a.add_argument('--fixture',type=Path,default=Path(__file__).with_name('fixture.sav'));a.add_argument('--expect-bug',action='store_true');args=a.parse_args()
w=args.cwd;binary=args.binary;out=args.output;out.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(binary)==args.sha256

def writegz(path,data):
    path.write_bytes(gzip.compress((json.dumps(data,ensure_ascii=False,separators=(',',':'))+'\n').encode(),mtime=0))
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
assert fixture.exists()
rows=[]
for case in ['neutral','short']:
    repeats=[]
    for repeat in [1,2]:
        save=out/f'{case}-{repeat}.sav';save.write_bytes(fixture.read_bytes())
        game=Native(out/f'{case}-{repeat}',save);record={'frames':[],'images':{}}
        try:
            game.boot();game.drive([None]*64);game.drive(['start','start']+[None]*96)
            for _ in range(8):
                menu=game.state()['field_menu']
                if menu['items'][menu['cursor']]=='Pokemon':break
                game.drive(['down','down']+[None]*12)
            assert menu['items'][menu['cursor']]=='Pokemon'
            record['start_cursor']=menu['cursor']
            game.drive(['a','a']+[None]*96);state=game.state()
            assert state['screen']=='party',state['screen']
            record['initial']={k:state.get(k) for k in ['screen','map_name','player_x','player_y','party']}
            record['images']['before']=game.capture('before')
            for tick in range(160):
                key='b' if tick<2 else 'down' if case=='short' and tick in [10,11] else 'a' if case=='short' and tick in [16,17] else None
                game.drive([key]);state=game.state();restore=state.get('field_flash') or {}
                record['frames'].append({'tick':tick,'input':key,'screen':state['screen'],'map_name':state['map_name'],'x':state['player_x'],'y':state['player_y'],'menu':state.get('field_menu'),'restore_error':restore.get('error'),'operation':restore.get('operation')})
                if tick in [4,40,80,159]:record['images'][str(tick)]=game.capture(f'frame-{tick:03}')
            if args.expect_bug:
                assert any('MissingSource(Sprite(41)' in str(f['restore_error']) for f in record['frames'])
                assert state['screen']=='party'
            else:
                assert all(f['restore_error'] is None for f in record['frames'])
                assert state['screen']=='start-menu',state['screen']
                assert state['field_menu']['cursor']==record['start_cursor']
                assert state['party']==record['initial']['party']
                assert (state['player_x'],state['player_y'])==(record['initial']['player_x'],record['initial']['player_y'])
            assert sha(save)==sha(fixture)
        finally:
            record['commands']=game.commands;game.close();writegz(out/f'{case}-{repeat}/record.json.gz',record)
        repeats.append(record)
    assert repeats[0]['frames']==repeats[1]['frames'] and repeats[0]['images']==repeats[1]['images']
    row={'case':case,'repeats':2,'old_failure_reproduced':args.expect_bug,'first_start':next((f['tick'] for f in record['frames'] if f['screen']=='start-menu'),None),'final_screen':record['frames'][-1]['screen']};rows.append(row);print(json.dumps(row),flush=True)
assert sha(binary)==args.sha256
(out/'result.json').write_text(json.dumps({'status':'passed','scope':'Diagnostic m44 fixture exported from failed fresh run, prepared into ordinary SRAM; formal replays cold CONTINUE and actual START/Pokemon/B only. Not a new full-route verdict.','binary_sha256':sha(binary),'fixture_sha256':sha(fixture),'script_sha256':sha(Path(__file__)),'cases':rows},indent=2)+'\n')

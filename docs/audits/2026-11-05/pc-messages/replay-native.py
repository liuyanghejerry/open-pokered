import argparse, hashlib, json, gzip, os, socket, subprocess, sys, time
from pathlib import Path
w=Path(__file__).resolve().parents[4];e=Path(__file__).resolve().parent
a=argparse.ArgumentParser();a.add_argument('--binary',type=Path,required=True);a.add_argument('--sha256',required=True);a.add_argument('--output',type=Path,required=True);a.add_argument('--candidate',action='store_true');args=a.parse_args()
binary=args.binary.resolve();out=args.output.resolve();out.mkdir(exist_ok=False)
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

fixture=e/'fixture.sav'

def wait_phase(game,target):
    for n in range(1500):
        st=game.state()
        if st.get('pc_phase')==target:
            game.drive([None]*24);return
        assert st['screen']=='pc' and st.get('pc_phase')=='Message',st
        game.drive(['a']+[None]*7)
    raise AssertionError('PC never reached '+target)

results=[]
for case in ['normal','early']:
    records=[]
    for repeat in [1,2]:
        save=out/f'{case}-{repeat}.sav';save.write_bytes(fixture.read_bytes());game=Native(out/f'{case}-{repeat}',save)
        record={'case':case,'frames':[],'images':{},'status':'running'}
        try:
            game.boot();game.drive([None]*64);game.drive(['up','up']+[None]*32);game.drive(['a','a']+[None]*8);wait_phase(game,'MainMenu')
            game.drive(['a','a']+[None]*32);wait_phase(game,'BillsMenu')
            for _ in range(3):game.drive(['down','down']+[None]*22)
            record['initial']=game.state();record['images']['initial']=game.capture('initial');record['initial_sram_sha256']=sha(save)
            for tick in range(93):
                key='a' if tick in ([0,1,4,5] if case=='early' else [0,1,7,8,27,28]) else 'up' if case=='early' and tick in [2,3] else None
                game.drive([key]);st=game.state()
                row={k:st.get(k) for k in ['screen','pc_phase','map_name','player_x','player_y','party','source_foreground']};row.update(tick=tick,input=key);record['frames'].append(row)
                if tick in [0,1,4,10,20,28,46,47,60,92]:record['images'][str(tick)]=game.capture(f'frame-{tick:03}')
            if args.candidate:
                if case=='early':assert all(x['pc_phase']=='Message' for x in record['frames']),record['frames']
                else:assert record['frames'][-1]['pc_phase']=='ChangeBoxConfirm',record['frames'][-1]
            record['final_sram_sha256']=sha(save)
            if args.candidate:assert record['initial_sram_sha256']==record['final_sram_sha256'],'warning input wrote SRAM'
            record['status']='passed'
        finally:
            record['commands']=game.commands;game.close();writegz(out/f'{case}-{repeat}/record.json.gz',record)
        records.append(record)
        print(json.dumps({'case':case,'repeat':repeat,'first_confirm':next((x['tick'] for x in record['frames'] if x['pc_phase']=='ChangeBoxConfirm'),None),'last_phase':record['frames'][-1]['pc_phase']}),flush=True)
    assert records[0]['frames']==records[1]['frames'] and records[0]['images']==records[1]['images'],case
    results.append({'case':case,'repeats':2,'first_confirm':next((x['tick'] for x in records[0]['frames'] if x['pc_phase']=='ChangeBoxConfirm'),None),'last_phase':records[0]['frames'][-1]['pc_phase']})
assert sha(binary)==args.sha256
(out/'result.json').write_text(json.dumps({'status':'passed','binary':str(binary),'binary_sha256':sha(binary),'source_sha256':sha(Path(__file__)),'fixture_sha256':sha(fixture),'cases':results,'scope':'Prepared ordinary SRAM, cold CONTINUE, real PC inputs, no recording-time debug edits. All frames and image hashes identical between two repeats per case; PCM active.'},indent=2)+'\n')

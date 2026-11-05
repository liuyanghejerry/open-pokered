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
def phase(g):return g.state().get('pc_phase')
def press(g,key,wait=24):g.drive([key,key]+[None]*wait)
def reach(g,target):
    for tick in range(5000):
        st=g.state()
        if st.get('pc_phase')==target:g.drive([None]*24);return
        assert st['screen']=='pc',st
        g.drive(['a']+[None]*7 if st.get('pc_phase')=='Message' else [None]*8)
    raise AssertionError('PC did not reach '+target)
def open_pc(g,sub):
    g.boot();g.drive([None]*64);press(g,'up');press(g,'a',8);reach(g,'MainMenu')
    for _ in range(sub):press(g,'down')
    press(g,'a');reach(g,'BillsMenu' if sub==0 else 'ItemMenu')
def close_pc(g):
    for _ in range(40):
        if g.state()['screen']=='overworld':return
        press(g,'b',48)
    raise AssertionError('PC did not close')
def export(save,path):
    subprocess.run([str(binary),'export-snapshot','--input',str(save),'-o',str(path)],cwd=w,check=True,capture_output=True)
    return json.loads(path.read_text())
def stocks(data):
    return {'party':[m['species'] for m in data['party']],'boxes':[[m['species'] for m in box] for box in data['pc_storage']['boxes']],'current_box':data['pc_storage']['current_box'],'bag':data['game_data']['bag']['items'],'pc_items':data['game_data']['pc_items']['items']}
results=[]
for case in ['change-box','release','toss']:
    repeated=[]
    for repeat in [1,2]:
        saved=out/f'{case}-{repeat}.sav';saved.write_bytes(fixture.read_bytes());g=Native(out/f'{case}-{repeat}',saved)
        record={'case':case,'repeat':repeat,'images':{},'status':'running'}
        try:
            before=stocks(export(saved,g.folder/'before.json'));open_pc(g,0 if case!='toss' else 1)
            for _ in range(3 if case=='change-box' else 2):press(g,'down')
            press(g,'a',0)
            if case=='release':reach(g,'MonList');press(g,'a',0)
            elif case=='toss':
                reach(g,'ItemList');press(g,'a',0);reach(g,'ItemQuantity');press(g,'a',0)
            assert phase(g)=='Message',g.state();record['images']['warning']=g.capture('warning')
            g.drive([None,None,'a','a']+[None]*4);assert phase(g)=='Message',g.state()
            assert stocks(export(saved,g.folder/'before-confirm.json'))==before
            confirm={'change-box':'ChangeBoxConfirm','release':'ReleaseConfirm','toss':'TossConfirm'}[case]
            reach(g,confirm);record['images']['confirmation']=g.capture('confirmation')
            assert stocks(export(saved,g.folder/'still-before-confirm.json'))==before
            press(g,'a')
            if case=='change-box':
                reach(g,'BoxList');press(g,'down');press(g,'a');reach(g,'BillsMenu')
                after=stocks(export(saved,g.folder/'after.json'))
                assert after['current_box']==1 and after['boxes']==before['boxes'],after
                assert after['party']==before['party'] and after['bag']==before['bag'] and after['pc_items']==before['pc_items']
            else:
                reach(g,'BillsMenu' if case=='release' else 'ItemList');close_pc(g);g.cmd(cmd='save')
                after=stocks(export(saved,g.folder/'after.json'))
                if case=='release':assert len(after['boxes'][0])==0 and len(before['boxes'][0])==1
                else:assert after['pc_items']==[['Potion',3]] and before['pc_items']==[['Potion',4]]
                assert after['party']==before['party'] and after['bag']==before['bag']
                if case=='release':assert after['pc_items']==before['pc_items']
                else:assert after['boxes']==before['boxes']
            record['after']=after;record['images']['after']=g.capture('after');record['status']='passed'
        finally:record['commands']=g.commands;g.close();writegz(out/f'{case}-{repeat}/record.json.gz',record)
        cold=Native(out/f'{case}-{repeat}-continue',saved)
        try:
            cold.boot();cold.drive([None]*64);assert cold.state()['screen']=='overworld';cold.cmd(cmd='save')
            reloaded=stocks(export(saved,cold.folder/'reloaded.json'));assert reloaded==after,(after,reloaded)
            writegz(cold.folder/'record.json.gz',{'commands':cold.commands,'reloaded':reloaded})
        finally:cold.close()
        record['independent_continue']=True;repeated.append(record);print(json.dumps({'case':case,'repeat':repeat,'status':'passed','continue':'passed'}),flush=True)
    assert repeated[0]['images']==repeated[1]['images'] and repeated[0]['after']==repeated[1]['after'],case
    results.append({'case':case,'repeats':2,'independent_continue':2,'repeat_images_and_stock_equal':True})
(out/'result.json').write_text(json.dumps({'status':'passed','binary_sha256':sha(binary),'source_sha256':sha(Path(__file__)),'fixture_sha256':sha(fixture),'cases':results,'scope':'Cold CONTINUE from prepared ordinary SRAM; actual warnings, confirmations and transactions. No recording-time game-state setters; change-box uses its own automatic save. Release/toss explicitly saved afterward, all independently continued and re-saved for runtime stock comparison.'},indent=2)+'\n')

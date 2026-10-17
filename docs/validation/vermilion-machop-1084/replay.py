#!/usr/bin/env python3
"""Replay the committed fixed inputs against a normal desktop debug-server CLI.
Standard library only; no snapshots, warps, runtime setters or adaptive input.
"""
from pathlib import Path
import argparse, collections, hashlib, json, os, shutil, signal, socket, struct
import subprocess, time, traceback
HERE = Path(__file__).resolve().parent
KEYS = ['screen', 'map_name', 'player_x', 'player_y', 'player_facing',
        'active_script_effect', 'script_effect', 'dialogue_state', 'audio_sfx']
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def interrupt(number, _frame): raise RuntimeError('interrupted: ' + str(number))
def stop(proc):
    signals = []
    for sig, timeout in [(signal.SIGTERM, 8), (signal.SIGKILL, 2)]:
        if proc.poll() is None:
            try: os.killpg(proc.pid, sig); signals.append(sig.name)
            except ProcessLookupError: pass
        try: proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired: continue
        break
    try: os.killpg(proc.pid, 0); gone = False
    except ProcessLookupError: gone = True
    assert gone and proc.poll() is not None
    return dict(pid=proc.pid, signals=signals, exit_code=proc.returncode,
                group_gone=gone, scope='owned diagnostic shutdown after recorded boundary')
def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--role', required=True, choices=['before', 'after'])
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    manifest = json.loads((HERE/'validation.json').read_text())
    for name in ['machop.sav', 'inputs.json', 'replay.py']:
        assert sha(HERE/name) == manifest['assets'][name]['sha256'], name
    packed = json.loads((HERE/'inputs.json').read_text()); buttons = []
    for key, count in packed['runs']:
        assert key in {None,'a','b','up','down','left','right','start'}
        assert type(count) is int and count > 0
        buttons.extend([key]*count)
    assert len(buttons) == packed['frames'] == manifest['input_frames']
    assert hashlib.sha256((json.dumps(buttons,ensure_ascii=False,indent=2)+'\n').encode()).hexdigest() == manifest['full_tape_sha256']
    expected = manifest['normal_cli'][args.role]
    binary = args.binary.resolve(); assert binary.is_file() and os.access(binary,os.X_OK)
    out = args.output.resolve(); out.mkdir(parents=True,exist_ok=False)
    save = out/'pokered.sav'; shutil.copyfile(HERE/'machop.sav',save)
    with socket.socket() as probe: probe.bind(('127.0.0.1',0)); port=probe.getsockname()[1]
    argv=[str(binary),'run','--headless','--speed','0','--seed','42','--debug-port',str(port),'--lang','en','--no-audio','--save',str(save)]
    env={k:v for k,v in os.environ.items() if not k.startswith('POKERED_')}
    result=dict(passed=False,role=args.role,binary_sha256=sha(binary),recorded_source_head=expected['source_head'],argv=argv,runtime_setters=0,captures=[])
    proc=sock=stream=states=None; requests=collections.Counter(); cries=[]; failure=None
    signal.signal(signal.SIGTERM,interrupt);signal.signal(signal.SIGINT,interrupt)
    try:
        with (out/'game.log').open('x') as log: proc=subprocess.Popen(argv,cwd=out,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        for _ in range(200):
            assert proc.poll() is None, ('early Game exit',proc.returncode)
            try: sock=socket.create_connection(('127.0.0.1',port),timeout=.1);break
            except (ConnectionRefusedError,TimeoutError):time.sleep(.05)
        assert sock is not None; sock.settimeout(30);sock.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1)
        stream=sock.makefile('rwb');states=(out/'states.jsonl').open('x')
        def rpc(command,**kw):
            assert command in {'get_state','get_flags','get_bag','press_timeline','capture_frame'}
            requests[command]+=1;stream.write((json.dumps(dict(cmd=command,**kw))+'\n').encode());stream.flush()
            raw=stream.readline(4*1024*1024)
            if hasattr(socket,'TCP_QUICKACK'):
                try:sock.setsockopt(socket.IPPROTO_TCP,socket.TCP_QUICKACK,1)
                except OSError:pass
            assert raw.endswith(b'\n'),'truncated reply';reply=json.loads(raw);assert reply.get('ok') is True,reply
            return reply.get('data')
        state=rpc('get_state');assert state['frame_count']==0;initial=None
        captures={c['frame']:c for c in expected['captures']}
        for frame,button in enumerate(buttons,1):
            ack=rpc('press_timeline',buttons=[button],start_at_frame=frame-1,advance=True)
            assert ack['frame_count']==frame and ack['frames']==1 and ack['padding_frames']==0
            state=rpc('get_state');assert state['frame_count']==frame
            projection={k:state.get(k) for k in KEYS}
            states.write(json.dumps(dict(frame=frame,button=button,state=projection))+'\n')
            if frame==manifest['first_overworld_frame']:
                assert state['screen']=='overworld' and state['map_name']=='VermilionCity'
                assert [state['player_x'],state['player_y']]==[29,10] and state['player_facing']=='Down'
                assert state['money']==1000 and rpc('get_bag')==[]
                assert len(state['party'])==1 and state['party'][0]['species']=='Pidgey' and state['party'][0]['level']==20 and state['party'][0]['hp']==50
                initial=dict(party=state['party'],flags=rpc('get_flags'))
            if state['audio_sfx']==dict(id=50,playing=True):
                cries.append(frame)
                if args.role=='after':
                    assert state['script_effect']['state']['phase']=='SoundWait'
                    assert state['dialogue_state']['text']=='MACHOP: Guoh! Gogogoh!' and state['dialogue_state']['waiting_for_input']
            if str(frame) in expected['sample_states']:assert projection==expected['sample_states'][str(frame)],('different recorded boundary',frame,projection)
            if frame in captures:
                case=captures[frame];path=out/(str(frame)+'-'+case['reason']+'.png')
                rpc('capture_frame',path=str(path));assert rpc('get_state')==state
                raw=path.read_bytes();assert raw[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',raw[16:24])==(160,144)
                result['captures'].append(dict(frame=frame,reason=case['reason'],path=str(path),sha256=sha(path),matches_recorded_bytes=sha(path)==case['sha256']))
        assert cries==list(range(expected['cry_frames'][0],expected['cry_frames'][1]+1))
        assert len(result['captures'])==4 and not state['script_running'] and state['dialogue_state'] is None
        assert state['map_name']=='VermilionCity' and [state['player_x'],state['player_y']]==[29,10]
        assert state['money']==1000 and state['party']==initial['party'] and rpc('get_bag')==[] and rpc('get_flags')==initial['flags']
        assert sha(save)==manifest['assets']['machop.sav']['sha256'] and not save.with_suffix('.script_flags.json').exists()
        result.update(passed=True,actual_frames=len(buttons),cry_frames=[min(cries),max(cries)])
    except BaseException:failure=traceback.format_exc();result['failure']=failure
    finally:
        signal.signal(signal.SIGTERM,signal.SIG_IGN);signal.signal(signal.SIGINT,signal.SIG_IGN)
        for resource in [states,stream,sock]:
            if resource is not None:
                try:resource.close()
                except OSError:pass
        try:
            if proc is not None:result['owned_shutdown']=stop(proc)
        except BaseException:failure=traceback.format_exc();result.update(passed=False,cleanup_failure=failure)
        result['RPC_counts']=dict(requests);(out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    assert failure is None and result['passed'],failure
    print('PASS',args.role,'fixed inputs',len(buttons),'four matched-frame captures')
if __name__=='__main__':main()

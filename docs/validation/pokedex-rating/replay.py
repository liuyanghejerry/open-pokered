#!/usr/bin/env python3
"""Replay recorded ordinary PC inputs; no warp, state setters or script overrides."""
from pathlib import Path
import argparse,hashlib,json,os,shutil,signal,socket,subprocess,time

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--role',choices=['before','after'],required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--port',type=int,default=19086)
    args=parser.parse_args();base=Path(__file__).resolve().parent
    validation=json.loads((base/'validation.json').read_text())
    tape=json.loads((base/'inputs.json').read_text())
    assert len(tape)==validation['frames'] and all(x in [None,'a','b','up','down','start']for x in tape)
    fixture=base/'fixture.sav';assert sha(fixture)==validation['fixture_sha256']
    out=args.output.resolve();out.mkdir();save=out/'pokered.sav';shutil.copyfile(fixture,save)
    env={k:v for k,v in os.environ.items() if not k.startswith('POKERED_')}
    argv=[str(args.binary.resolve()),'run','--headless','--speed','0','--seed','42','--debug-port',str(args.port),'--lang','en','--no-audio','--save',str(save)]
    process=None;stream=None;connection=None;result={'passed':False,'argv':argv,'role':args.role,'runtime_setters':0}
    try:
        with (out/'game.log').open('w') as log:
            process=subprocess.Popen(argv,cwd=out,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            for _ in range(200):
                assert process.poll()is None,'game exited during startup'
                try:connection=socket.create_connection(('127.0.0.1',args.port),timeout=.1);break
                except (ConnectionRefusedError,TimeoutError):time.sleep(.05)
            assert connection is not None
            connection.settimeout(30);connection.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1);stream=connection.makefile('rwb')
            def rpc(cmd,**data):
                assert cmd in ['get_state','press_timeline','capture_frame']
                stream.write((json.dumps({'cmd':cmd,**data})+'\n').encode());stream.flush()
                response=json.loads(stream.readline(4*1024*1024));assert response['ok'],response
                if hasattr(socket,'TCP_QUICKACK'):connection.setsockopt(socket.IPPROTO_TCP,socket.TCP_QUICKACK,1)
                return response.get('data')
            state=rpc('get_state');assert state['frame_count']==0
            checkpoints={sample['frame']:sample for sample in validation['roles'][args.role]['samples']}
            frame=0;observations=[]
            while frame<len(tape):
                next_sample=min([n for n in checkpoints if n>frame]+[len(tape)])
                end=min(frame+60,next_sample)
                receipt=rpc('press_timeline',buttons=tape[frame:end],start_at_frame=frame,advance=True)
                assert receipt['frame_count']==end and receipt['advanced'] is True
                frame=end
                if frame in checkpoints:
                    sample=checkpoints[frame];state=rpc('get_state')
                    assert state['frame_count']==frame
                    for key,value in sample['state'].items():assert state[key]==value,(frame,key,state[key],value)
                    if sample.get('capture'):
                        rpc('capture_frame',path=str(out/sample['capture']))
                        assert rpc('get_state')==state,'capture advanced game'
                    observations.append({'frame':frame,'state':{k:state[k]for k in sample['state']}})
            assert rpc('get_state')['pc_phase']=='MainMenu'
            assert sha(save)==validation['fixture_sha256']
            assert not save.with_suffix('.script_flags.json').exists()
            result.update(passed=True,frames=frame,observations=observations)
    finally:
        if stream:stream.close()
        if connection:connection.close()
        if process and process.poll()is None:
            os.killpg(process.pid,signal.SIGTERM)
            try:process.wait(timeout=8)
            except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait(timeout=3)
        result['owned_process_exit']=None if process is None else process.returncode
        (out/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'passed':result['passed'],'frames':result.get('frames'),'output':str(out)}))
if __name__=='__main__':main()

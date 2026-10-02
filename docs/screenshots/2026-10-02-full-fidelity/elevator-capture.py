import socket,json,sys
from pathlib import Path
port=int(sys.argv[1]);prefix=sys.argv[2]
s=socket.create_connection(('127.0.0.1',port));f=s.makefile('rwb')
log=[]
def call(x):
 f.write((json.dumps(x)+'\n').encode());f.flush();r=json.loads(f.readline());log.append({'request':x,'response':r});print(x,r);return r
call({'cmd':'step_frames','count':30})
call({'cmd':'get_nearby','radius':10})
call({'cmd':'interact_with','id':'sign:0'})
call({'cmd':'skip_dialogue'})
call({'cmd':'step_frames','count':1})
state=call({'cmd':'get_state'})['data']
# Keep the established baseline's global frame 70. Updated dialogue skipping
# reaches the same initialized menu sooner, so pad with neutral frames.
assert state['screen']=='elevator' and state['field_menu']['cursor']==0,state
assert state['frame_count']<=70,state
if state['frame_count']<70:
 call({'cmd':'step_frames','count':70-state['frame_count']})
 call({'cmd':'get_state'})
call({'cmd':'capture_frame','path':str(Path(__file__).resolve().parent/f'{prefix}-elevator-script.png')})
Path(f'/tmp/visual-elevator-{prefix}-trace.json').write_text(json.dumps(log,indent=2))

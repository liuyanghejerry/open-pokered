#!/usr/bin/env python3
"""Capture Route1's first NPC before/after with identical seed and frame inputs."""
import argparse,json,subprocess,sys
from pathlib import Path
ap=argparse.ArgumentParser();ap.add_argument('--before',required=True);ap.add_argument('--after',required=True);ap.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[3]);opt=ap.parse_args()
root=opt.root.resolve();sys.path.insert(0,str(root/'scripts'));from debug_drive import DebugClient
out=root/'docs/screenshots/fidelity-world';out.mkdir(parents=True,exist_ok=True);traces={}
for label,binary,port in [('before',opt.before,9411),('after',opt.after,9412)]:
    with open('/tmp/world-route1-'+label+'.log','w') as log:
        proc=subprocess.Popen([binary,'run','--skip-intro','--warp','Route1,7,24','--seed','123','--no-audio','--headless','--debug-port',str(port),'--speed','0'],stdout=log,stderr=log,cwd=root)
        try:
            client=DebugClient(port);traces[label]=[]
            for frame in [0,60,120,180,240]:
                if frame:client.cmd(cmd='step_frames',count=60)
                traces[label].append({'frame':frame,'nearby':client.cmd(cmd='get_nearby',radius=12),'npcs':client.cmd(cmd='get_npcs')})
                result=client.cmd(cmd='capture_frame',path=str(out/f'route1-npc-{label}-{frame:03d}.png'))
                assert result['ok'],result
        finally:
            proc.terminate();proc.wait(timeout=8)
(root/'docs/audits/2026-10-02-full-fidelity/route1-npc-trace.json').write_text(json.dumps(traces,indent=2)+'\n')
for label,entries in traces.items():
    print(label,[(r['frame'],r['npcs']['data'][0]['x'],r['npcs']['data'][0]['y'],r['npcs']['data'][0]['facing']) for r in entries])
assert traces['before'][-1]['npcs']['data'][0]['x']==6
assert traces['after'][-1]['npcs']['data'][0]['x']==5
print('Route1 first NPC: baseline horizontal deviation removed.')

#!/usr/bin/env python3
"""Capture Route1's first NPC before/after with identical seed and frame inputs."""
import argparse,json,subprocess,sys,hashlib
from pathlib import Path
ap=argparse.ArgumentParser();ap.add_argument('--before',help='Optional: omit to preserve previously verified before captures/trace');ap.add_argument('--after',required=True);ap.add_argument('--after-source',default='not specified');ap.add_argument('--after-font',default='not specified');ap.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[3]);opt=ap.parse_args()
root=opt.root.resolve();sys.path.insert(0,str(root/'scripts'));from debug_drive import DebugClient
out=root/'docs/screenshots/fidelity-world';out.mkdir(parents=True,exist_ok=True)
trace_path=root/'docs/audits/2026-10-02-full-fidelity/route1-npc-trace.json'
traces=json.loads(trace_path.read_text()) if trace_path.exists() else {}
if opt.before is None and 'before' not in traces:ap.error('after-only capture requires an existing verified before trace')
for label,binary,port in [('before',opt.before,9411),('after',opt.after,9412)]:
    if binary is None:continue
    traces.setdefault('_capture',{})[label]={'binary':str(Path(binary).resolve()),'binary_sha256':hashlib.sha256(Path(binary).read_bytes()).hexdigest(),'source':opt.after_source if label=='after' else '72ff719b39634c153cb82d3f3ece200bd413c4e0','font':opt.after_font if label=='after' else 'Fusion Pixel baseline'}
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
trace_path.write_text(json.dumps(traces,indent=2)+'\n')
for label in ['before','after']:
    entries=traces[label]
    print(label,[(r['frame'],r['npcs']['data'][0]['x'],r['npcs']['data'][0]['y'],r['npcs']['data'][0]['facing']) for r in entries])
assert traces['before'][-1]['npcs']['data'][0]['x']==6
assert traces['after'][-1]['npcs']['data'][0]['x']==5
print('Route1 first NPC: baseline horizontal deviation removed.')

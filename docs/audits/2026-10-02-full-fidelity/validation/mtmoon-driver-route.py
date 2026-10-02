import argparse,json,sys
from pathlib import Path
parser=argparse.ArgumentParser(description='Seeded real-input Mt Moon driver route regression; not a fresh playthrough.')
parser.add_argument('--fixture',required=True,type=Path)
parser.add_argument('--binary',required=True,type=Path)
args=parser.parse_args()
ROOT=Path(__file__).resolve().parents[4]
sys.path.insert(0,str(ROOT/'scripts'))
import playthrough as p
from debug_drive import DebugClient
out=ROOT/'docs/audits/2026-10-02-full-fidelity/validation/mtmoon-driver-route.json'
commands=[]
orig=DebugClient.cmd
def record(self,**kw):
    if kw.get('cmd') not in {'get_state','get_npcs','get_flags','get_bag','step_frames','wait_until','skip_dialogue','press_timeline'}:
        raise RuntimeError('non-input command prohibited: '+str(kw))
    commands.append(kw)
    return orig(self,**kw)
DebugClient.cmd=record
def atomic_drive(self,buttons,frames=None):
    timeline=list(buttons)
    count=len(timeline) if frames is None else frames
    timeline.extend([None]*(count-len(timeline)))
    return self.cmd(cmd='press_timeline',buttons=timeline,advance=True)
DebugClient.drive=atomic_drive
g=p.Game(binary=args.binary,snapshot=args.fixture,seed=42,speed=0)
observations=[]
original_st=g.st
def observed():
    s=original_st()
    state={k:s.get(k) for k in ('frame_count','screen','map_name','player_x','player_y','active_script_effect')}
    if not observations or observations[-1]!=state: observations.append(state)
    return s
g.st=observed
try:
    p.resume_reentry(g)
    before=g.evidence('mtmoon-route-before')
    assert g.pos()==('MtMoonB1F',25,9),g.pos()
    commands.clear();observations.clear()
    g.last_map='Route4'
    destination=g.nav_warp(21,17,'MtMoonB1F','MtMoonB2F')
    after=g.evidence('mtmoon-route-after')
    assert destination=='MtMoonB2F' and g.pos()[0]=='MtMoonB2F',g.pos()
    maps=[]
    for s in observations:
        if s['screen']=='overworld' and (not maps or maps[-1]!=s['map_name']):maps.append(s['map_name'])
    assert maps==['MtMoonB1F','MtMoon1F','MtMoonB1F','MtMoonB2F'],maps
    result={'scope':'Seeded driver-route regression only; not a fresh playthrough.',
        'binary':str(args.binary),
        'fixture':'SaveData::new position MtMoonB1F (25,9), last_map Route4; Ivysaur L20; no event flags edited.',
        'before':{k:before[k] for k in ('map_name','player_x','player_y','frame_count')},
        'after':{k:after[k] for k in ('map_name','player_x','player_y','frame_count')},
        'observed_maps':maps,'observations':observations,'commands':commands}
    out.write_text(json.dumps(result,indent=2)+'\n')
    print('PASS',maps,'commands',len(commands),'evidence',out,flush=True)
finally:
    g.close()

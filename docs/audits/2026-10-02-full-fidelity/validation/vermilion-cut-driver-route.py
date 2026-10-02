import argparse,json,sys
from pathlib import Path
parser=argparse.ArgumentParser(description='Seeded real-input Vermilion CUT driver route regression; not a fresh playthrough.')
parser.add_argument('--fixture',required=True,type=Path)
parser.add_argument('--binary',required=True,type=Path)
args=parser.parse_args()
ROOT=Path(__file__).resolve().parents[4]
sys.path.insert(0,str(ROOT/'scripts'))
import playthrough as p
from debug_drive import DebugClient
out=ROOT/'docs/audits/2026-10-02-full-fidelity/validation/vermilion-cut-driver-route.json'
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
    before=g.evidence('cut-route-before')
    assert g.pos()==('VermilionCity',15,17),g.pos()
    tree=9*p.MAPS['VermilionCity']['width']+7
    assert before['map_blocks'][tree]==0x35
    assert p.bfs('VermilionCity',(15,17),(12,20),p.warp_tiles('VermilionCity')) is None
    commands.clear();observations.clear()
    from playthrough_late import field_move
    g.face('down')
    original_step=g.step
    before_animation={}
    def observe_animation_handoff(frames):
        if frames==24:
            state=g.st()
            before_animation.update(frame_count=state['frame_count'],tree_block=state['map_blocks'][tree])
            assert before_animation['tree_block']==0x35
        return original_step(frames)
    g.step=observe_animation_handoff
    field_move(g,'Cut')
    g.step=original_step
    after_cut=g.st()
    assert after_cut['map_blocks'][tree]==0x4C,after_cut['map_blocks'][tree]
    assert p.MAPS['VermilionCity']['blocks'][tree]==0x4C
    assert p.bfs('VermilionCity',(15,17),(12,20),p.warp_tiles('VermilionCity')) is not None
    destination=g.nav_warp(12,19,'VermilionCity','VermilionGym')
    after=g.evidence('cut-route-after')
    assert destination=='VermilionGym' and g.pos()[0]=='VermilionGym',g.pos()
    maps=[]
    for s in observations:
        if s['screen']=='overworld' and (not maps or maps[-1]!=s['map_name']):maps.append(s['map_name'])
    assert maps==['VermilionCity','VermilionGym'],maps
    result={'scope':'Seeded driver-route regression only; not a fresh playthrough.',
        'binary':str(args.binary),
        'fixture':'SaveData::new position VermilionCity (15,17), last_map VermilionCity; Ivysaur L20 already knows CUT; Boulder/Cascade badges; event flags default.',
        'tree_before':before['map_blocks'][tree],'tree_after_cut':after_cut['map_blocks'][tree],
        'after_dialogue_before_animation':before_animation,
        'before':{k:before[k] for k in ('map_name','player_x','player_y','frame_count')},
        'after':{k:after[k] for k in ('map_name','player_x','player_y','frame_count')},
        'observed_maps':maps,'observations':observations,'commands':commands}
    out.write_text(json.dumps(result,indent=2)+'\n')
    print('PASS',maps,'commands',len(commands),'evidence',out,flush=True)
finally:
    g.close()

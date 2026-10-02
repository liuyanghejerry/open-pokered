#!/usr/bin/env python3
"""Two real TCP apps: receptionist/SAVE/Colosseum, asynchronous KO, heal.

Initial party values are synthetic SaveData snapshots; every interaction after
loading them uses the existing production buttons/navigation and TCP protocol.
No debug command starts the battle or mutates its combatants.
"""
import argparse, copy, hashlib, json, pathlib, subprocess, sys, tempfile

repo = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(repo / 'scripts'))
from debug_drive import DebugClient

parser = argparse.ArgumentParser()
parser.add_argument('binary')
parser.add_argument('--source', default='unknown')
parser.add_argument('--base-port', type=int, default=9501)
parser.add_argument('--tcp-port', type=int, default=9604)
parser.add_argument('--output-dir')
args = parser.parse_args()
binary = str(pathlib.Path(args.binary).resolve())
out = pathlib.Path(args.output_dir or tempfile.mkdtemp(prefix='pokered-colosseum-'))
out.mkdir(exist_ok=True, parents=True)
base = json.loads(pathlib.Path(__file__).with_name('systems-runtime-input.json').read_text())
evidence = {'source': args.source, 'binary_sha256': hashlib.sha256(pathlib.Path(binary).read_bytes()).hexdigest()}
processes, clients = [], []

def mon(species, level, hp, max_hp, move, pp, status, stats, typ):
    m = copy.deepcopy(base['party'][0])
    m.update(species=species, level=level, hp=hp, max_hp=max_hp,
             moves=[move, 'None', 'None', 'None'], pp=[pp, 0, 0, 0], status=status,
             attack=stats[0], defense=stats[1], speed=stats[2], special=stats[3],
             type1=typ, type2=typ, ot_id=25591)
    return m

parties = [
    [mon('Blastoise',100,100,300,'Surf',2,'Burn',[180,200,180,200],'Water'),
     mon('Pikachu',10,0,30,'Thundershock',0,'Poison',[20,20,30,20],'Electric')],
    [mon('Rattata',5,1,20,'Tackle',3,'Burn',[10,10,10,10],'Normal'),
     mon('Pidgey',5,0,20,'Gust',0,'Poison',[10,10,10,10],'Normal')],
]

def launch(name, port, party, extra):
    data=copy.deepcopy(base); data['party']=party
    fixture=out/f'{name}-input.json'; fixture.write_text(json.dumps(data,indent=2))
    save=out/f'{name}.sav'; save.unlink(missing_ok=True)
    save.with_suffix('.script_flags.json').unlink(missing_ok=True)
    log=open(out/f'{name}.log','w')
    p=subprocess.Popen([binary,'run','--save',str(save),'--skip-intro','--no-audio',
        '--headless','--speed','0','--debug-port',str(port),'--snapshot',str(fixture),
        '--warp','CeruleanPokecenter,11,5',*extra],cwd=repo,stdout=log,stderr=log)
    processes.append((p,log))
    d=DebugClient(port);clients.append(d);return d

def command(d, **kw):
    r=d.cmd(**kw);assert r.get('ok'),(kw,r);return r['data']
def step(d,n):return command(d,cmd='step_frames',count=n)
def tap(d,key):return command(d,cmd='press_timeline',buttons=[None,None,key,None,None,None,None],advance=True)
def state(d):return command(d,cmd='get_state')
def skip(d):return command(d,cmd='skip_dialogue')
def shot(d,name):return command(d,cmd='capture_frame',path=str(out/name))
def record(name,data):
    evidence[name]=data;(out/'evidence.json').write_text(json.dumps(evidence,indent=2))
    print(name,flush=True)

try:
    host=launch('host',args.base_port,parties[0],('--link-listen',str(args.tcp_port)))
    peer=launch('peer',args.base_port+1,parties[1],('--link-connect',f'127.0.0.1:{args.tcp_port}'))
    def jointly(n):step(host,n);step(peer,n)
    for _ in range(10):jointly(20)
    for name,d in [('host',host),('peer',peer)]:
        assert state(d)['link']['status']=='connected'
        command(d,cmd='interact_with',id='npc:3');skip(d);step(d,32)
        assert state(d)['link']['cable_phase']=='ReceptionText',state(d)
        skip(d);step(d,32);tap(d,'a');step(d,5)
        assert 'ReceptionMenu' in state(d)['link']['cable_phase'],state(d)
        tap(d,'down');tap(d,'a');step(d,180)
        assert state(d)['map_name']=='Colosseum',state(d)
        assert (out/f'{name}.sav').stat().st_size==32768
        record(name+'_room_before_battle',state(d))

    command(host,cmd='interact_with',id='sign:0')
    for _ in range(5):jointly(20)
    assert 'PeerPrompt' in state(peer)['link']['cable_phase'],state(peer)
    tap(peer,'a')
    for _ in range(10):jointly(20)
    for name,d in [('host',host),('peer',peer)]:
        s=state(d);assert s['screen']=='battle',s
        assert s['evaluation']['party']==evidence[name+'_room_before_battle']['evaluation']['party']
        record(name+'_battle_start_no_heal',s)
    for _ in range(30):
        for d in (host,peer):
            s=state(d)
            if s['battle_phase']!='PlayerMenu':tap(d,'a');step(d,60)
        if all(state(d)['battle_phase']=='PlayerMenu' for d in (host,peer)):break
    for name,d in [('host',host),('peer',peer)]:
        assert state(d)['battle_phase']=='PlayerMenu',state(d)
        tap(d,'a');step(d,3)
        assert state(d)['battle_phase']=='MoveSelect',state(d)
        record(name+'_move_selection',state(d));shot(d,name+'-moves-before.png')
    assert state(host)['battle_moves']['moves'][0]['pp']==2
    assert state(peer)['battle_moves']['moves'][0]['pp']==3
    tap(host,'a')
    for _ in range(5):jointly(20)
    assert state(host)['battle_phase']=='LinkWaiting',state(host)
    assert state(peer)['battle_phase']=='MoveSelect',state(peer)
    record('host_commits_100_frames_before_peer',{'host':state(host),'peer':state(peer)})
    tap(peer,'a')
    for _ in range(6):jointly(20)
    record('first_turn_resolution',{'host':state(host),'peer':state(peer)})
    assert state(host)['evaluation']['party'][0]['pp'][0]==1,state(host)
    for _ in range(80):
        for d in (host,peer):
            if state(d)['screen']=='battle':tap(d,'a');step(d,40)
        if all(state(d)['screen']=='overworld' for d in (host,peer)):break
    jointly(120)
    record('room_after_120_neutral_frames',{'host':state(host),'peer':state(peer)})
    shot(host,'host-room-after.png');shot(peer,'peer-room-after.png')
    max_pp={'Surf':15,'Thundershock':30,'Tackle':35,'Gust':35}
    for name,d in [('host',host),('peer',peer)]:
        s=state(d);assert s['screen']=='overworld' and s['map_name']=='Colosseum',s
        assert s['link']['cable_phase']=='InRoom',s
        for m in s['evaluation']['party']:
            assert m['hp']==m['max_hp'] and m['status']=='None',m
            assert m['pp'][0]==max_pp[m['moves'][0]],m
            assert m['total_exp']==2035,m
        assert s['money']==3000,s
        record(name+'_battle_finished_all_healed',s)
        tap(d,'start');step(d,5)
        menu=state(d)['field_menu'];assert menu['kind']=='start',state(d)
        for _ in range(menu['items'].index('Pokemon')-menu['cursor']):tap(d,'down')
        tap(d,'a');step(d,30);shot(d,name+'-party-healed-after.png')
    record('result','Both real TCP apps completed the battle and healed every party member only afterward.')
    print('all actual Colosseum runtime assertions passed',flush=True)
finally:
    for d in clients:
        try:d.close()
        except OSError:pass
    for p,log in processes:
        if p.poll() is None:
            p.terminate()
            try:p.wait(timeout=10)
            except subprocess.TimeoutExpired:p.kill();p.wait()
        log.close()

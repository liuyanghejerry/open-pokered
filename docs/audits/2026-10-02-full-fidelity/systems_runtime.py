#!/usr/bin/env python3
"""Run the actual NPC trade/save/reload and connected Cable Club UI paths.

Usage: python3 systems_runtime.py /path/to/pokered-app --base-port 9491
Each run creates independent ordinary .sav files in a temporary directory.
"""
import argparse, hashlib, json, pathlib, subprocess, sys, tempfile
repo = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(repo / 'scripts'))
from debug_drive import DebugClient

parser = argparse.ArgumentParser()
parser.add_argument('binary')
parser.add_argument('--base-port', type=int, default=9491)
parser.add_argument('--tcp-port', type=int, default=9603)
parser.add_argument('--output-dir')
parser.add_argument('--screenshots-dir')
parser.add_argument('--snapshot')
args = parser.parse_args()
binary = str(pathlib.Path(args.binary).resolve())
out = pathlib.Path(args.output_dir or tempfile.mkdtemp(prefix='pokered-systems-runtime-'))
out.mkdir(exist_ok=True, parents=True)
shots = pathlib.Path(args.screenshots_dir or repo / 'docs/screenshots/fidelity-systems')
shots.mkdir(exist_ok=True, parents=True)
fixture = pathlib.Path(args.snapshot or pathlib.Path(__file__).with_name('systems-runtime-input.json'))
evidence={'binary_sha256': hashlib.sha256(pathlib.Path(binary).read_bytes()).hexdigest()}
processes=[]

def launch(name, port, extra=(), snapshot=True, save_name=None):
    log=open(out/f'{name}.log','w')
    save_path=out/f'{save_name or name}.sav'
    if snapshot:
        save_path.unlink(missing_ok=True)
        save_path.with_suffix('.script_flags.json').unlink(missing_ok=True)
    args=[binary,'run','--save',str(save_path),
          '--skip-intro','--no-audio','--headless','--speed','0','--debug-port',str(port),*extra]
    if snapshot: args.extend(['--snapshot',str(fixture)])
    p=subprocess.Popen(args,stdout=log,stderr=log,cwd=repo)
    processes.append((p,log))
    return DebugClient(port)

def command(d, **kwargs):
    r=d.cmd(**kwargs)
    assert r.get('ok'), (kwargs,r)
    return r

def step(d,n): return command(d,cmd='step_frames',count=n)
def tap(d,key): return command(d,cmd='press_timeline',buttons=[None,None,key,None,None,None,None],advance=True)
def skip(d): return command(d,cmd='skip_dialogue')
def state(d): return command(d,cmd='get_state')['data']
def shot(d,name): return command(d,cmd='capture_frame',path=str(shots/name))
def save(stage,data):
    evidence[stage]=data
    (out/'evidence.json').write_text(json.dumps(evidence,indent=2))
    print(stage,flush=True)

try:
    npc=launch('npc',args.base_port)
    command(npc,cmd='warp',map='Route2TradeHouse',x=3,y=4)
    step(npc,90)
    command(npc,cmd='interact_with',id='npc:1')
    skip(npc);step(npc,32)
    assert state(npc)['choice'] is not None,state(npc)
    tap(npc,'a');step(npc,15)
    shot(npc,'npc-selector-after.png')
    save('npc_selector',state(npc))
    tap(npc,'a');step(npc,12)
    connecting=state(npc)
    connecting_flags=command(npc,cmd='get_flags')
    assert connecting['party'][0]['species']=='Abra',connecting
    assert connecting['dialogue'] is not None,connecting
    assert connecting_flags['data']['EVENT_TRADED_FOR_MARCEL'],connecting_flags
    save('npc_connect_before_mutation',{'state':connecting,'flags':connecting_flags})
    skipped=skip(npc)
    assert skipped['data'].get('dialogue_closed'),skipped
    step(npc,2000)
    for _ in range(3):skip(npc);step(npc,30)
    final=state(npc)
    flags=command(npc,cmd='get_flags')
    assert len(final['party'])==1 and final['party'][0]['species']=='MrMime',final
    assert flags['data']['EVENT_TRADED_FOR_MARCEL'],flags
    save('npc_complete',{'state':final,'flags':flags,'party':command(npc,cmd='get_party')})
    tap(npc,'start');step(npc,5)
    for _ in range(4):tap(npc,'down')
    tap(npc,'a');step(npc,25)
    assert state(npc)['screen'] in ('save','save_menu'),state(npc)
    tap(npc,'a');step(npc,175)
    save('npc_save_first_confirm_state',state(npc))
    if not (out/'npc.sav').exists():
        # Let the screen transition settle before another real A confirm.
        step(npc,60);tap(npc,'a');step(npc,250)
        save('npc_save_settled_confirm_state',state(npc))
    raw=(out/'npc.sav').read_bytes()
    assert raw[0x29e3]&2,raw[0x29e3]
    save('npc_ordinary_save',{'length':len(raw),'npc_flags_bytes':list(raw[0x29e3:0x29e5]),
        'party_species_internal':raw[0x2f34],'initialized_box_bit':raw[0x284c]&128,
        'checksum':raw[0x3523], 'independently_calculated_checksum':(~sum(raw[0x2598:0x3523]))&255})
    npc.close()
    processes[0][0].terminate();processes[0][0].wait(timeout=10)
    companion=out/'npc.script_flags.json'
    extras=json.loads(companion.read_text())
    extras['EVENT_TRADED_FOR_MARCEL']=False
    companion.write_text(json.dumps(extras))
    npc=launch('npc_reload',args.base_port,snapshot=False,save_name='npc')
    step(npc,90)
    reloaded=state(npc)
    flags=command(npc,cmd='get_flags')
    assert reloaded['party'][0]['species']=='MrMime',reloaded
    assert flags['data']['EVENT_TRADED_FOR_MARCEL'],flags
    save('npc_actual_sram_reload_with_stale_false_companion',{'state':reloaded,'flags':flags})
    command(npc,cmd='interact_with',id='npc:1');skip(npc);step(npc,32)
    assert state(npc)['choice'] is None,state(npc)
    save('npc_reload_prevents_repeat_trade',state(npc))
    npc.close();processes[1][0].terminate();processes[1][0].wait(timeout=10)

    host=launch('cable_host',args.base_port+1,('--warp','CeruleanPokecenter,11,5','--link-listen',str(args.tcp_port)))
    peer=launch('cable_peer',args.base_port+2,('--warp','CeruleanPokecenter,11,5','--link-connect',f'127.0.0.1:{args.tcp_port}'))
    for _ in range(10):step(host,20);step(peer,20)
    assert state(host)['link']['status']=='connected',state(host)
    command(host,cmd='interact_with',id='npc:3')
    skip(host);step(host,32)
    reception=state(host)
    assert reception['link']['cable_phase']=='ReceptionText',reception
    step(host,120)
    shot(host,'cable-reception-after.png')
    save('cable_apply',state(host))
    skip(host);step(host,32)
    assert 'ReceptionSave' in state(host)['link']['cable_phase'],state(host)
    tap(host,'a');step(host,5)
    assert 'ReceptionMenu' in state(host)['link']['cable_phase'],state(host)
    shot(host,'cable-room-selection-after.png')
    save('cable_room_menu',state(host))
    tap(host,'a');step(host,180)
    room=state(host)
    assert room['map_name']=='TradeCenter',room
    save('cable_room_entered',room)
    raw=(out/'cable_host.sav').read_bytes()
    save('ordinary_save',{'length':len(raw),'initialized_box_bit':raw[0x284c]&128,
        'party_species_internal':raw[0x2f34], 'checksum':raw[0x3523],
        'independently_calculated_checksum': (~sum(raw[0x2598:0x3523]))&255,
        'companion_bound_to_save':(out/'cable_host.script_flags.json').exists()})
    command(host,cmd='warp',map='CeruleanPokecenter',x=11,y=5);step(host,90)
    command(host,cmd='interact_with',id='npc:3');skip(host);step(host,32)
    skip(host);step(host,32);tap(host,'a');step(host,5)
    tap(host,'b');step(host,48)
    step(host,120)
    canceled=state(host)
    assert canceled['map_name']=='CeruleanPokecenter',canceled
    assert canceled['dialogue'] is not None,canceled
    shot(host,'cable-room-cancel-after.png')
    save('cable_room_canceled',canceled)
    host.close();peer.close()
    print('all runtime assertions passed',flush=True)
finally:
    for p,log in processes:
        if p.poll() is None:
            p.terminate()
            try:p.wait(timeout=10)
            except subprocess.TimeoutExpired:p.kill();p.wait()
        log.close()

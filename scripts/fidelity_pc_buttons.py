#!/usr/bin/env python3
"""Native PCM checks for PC menu silence and retained transfer sounds."""
import argparse
import copy
import json
from pathlib import Path
import debug_drive
import fidelity_stdio
import playthrough as pt
import fidelity_pc_audio as pc
import save_builder as sb


def open_players(g,bedroom=False):
    if bedroom:
        g.face('up');g.tap('a',4);pc.close_message(g)
    else:
        pc.open_pc(g,bills=False);g.tap('down',4);g.tap('a',4);pc.close_message(g)
    assert g.st()['pc_phase']=='ItemMenu',g.st()


def case(args,name,fresh,stored):
    fixture=copy.deepcopy(stored if name.startswith('bills') else fresh)
    if name=='bedroom-list':
        fixture['game_data']['position']={'map_id':sb.map_id('RedsHouse2F'),'x':0,'y':2,'x_block':0,'y_block':0}
    if name=='withdrawal-cue':fixture['game_data']['pc_items']['items']=[['Potion',4]]
    g=pc.boot(args,fixture)
    try:
        if name.startswith('bills') or name=='storage-cue':
            pc.open_pc(g)
            if name=='storage-cue':g.tap('down',4)
            if name in ['bills-action','storage-cue']:g.tap('a',4)
            if name=='storage-cue':g.tap('a',4)
        else:
            open_players(g,bedroom=name=='bedroom-list')
            if name=='withdrawal-cue':
                g.tap('a',4);g.tap('a',4)
                assert g.st()['pc_phase']=='ItemQuantity',g.st()
        g.step(3000-g.st()['frame_count'])
        assert not g.d.cmd(cmd='audio_state')['sound_playing']
        g.tap('a',1)
        state=g.st();audio=g.d.cmd(cmd='audio_state')
        if name in ['bills-list','bills-action','players-list','bedroom-list']:
            assert audio['sound_playing']==args.before,(name,audio)
            assert audio['sfx_channels']==([True,False,False,False] if args.before else [False]*4)
            expected='MonAction' if name=='bills-action' else 'MonList' if name=='bills-list' else 'ItemList'
            assert state['pc_phase']==expected,state
        elif name=='storage-cue':
            assert state['pc_phase']=='MonAction' and audio['sound_playing']
            checked=pc.wait_cry(g,3001,'Pikachu',initial=copy.deepcopy(audio))
            assert g.st()['pc_phase']=='Message' and g.st()['party_count']==1
            audio['cry_reference']=checked
        else:
            assert state['pc_phase']=='Message' and audio['sound_playing'],(state,audio)
            data=g.d.cmd(cmd='export_fixture')['data']['game_data']
            assert data['pc_items']['items']==[['Potion',3]] and data['bag']['items']==[['Potion',1]],data
        return {'state':state,'audio':audio}
    finally:g.close()


def run(args):
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=True)
    fidelity_stdio.install(pt,debug_drive,args.driver.resolve())
    fresh,stored=pc.prepare(args)
    results={name:case(args,name,fresh,stored) for name in ['bills-list','bills-action','players-list','bedroom-list','storage-cue','withdrawal-cue']}
    (args.output/'results.json').write_text(json.dumps(results,indent=2))
    (args.output/'fresh-fixture.json').write_text(json.dumps(fresh,indent=2))
    print('Four silent-PC-menu and two retained-transfer PCM cases completed')


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('driver',type=Path);p.add_argument('output',type=Path)
    p.add_argument('--before',action='store_true');run(p.parse_args())

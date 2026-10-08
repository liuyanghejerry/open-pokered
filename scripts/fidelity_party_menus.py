#!/usr/bin/env python3
"""Actual input checks for original party selection, wrapping and exits."""
import argparse
import copy
import json
from pathlib import Path
import debug_drive
import fidelity_stdio
import playthrough as pt
import playthrough_late as late
import fidelity_pc_audio as pc


def case(args,name,fresh):
    fixture=copy.deepcopy(fresh)
    if name=='single-switch':fixture['party']=fixture['party'][:1]
    if name=='item-cursor':fixture['game_data']['bag']['items']=[['Potion',2]]
    g=pc.boot(args,fixture)
    try:
        late.open_start(g,'Pokemon')
        if name in ['reopen-cursor','stats-cursor','wrap-down','item-cursor','pc-reset','battle-reset']:
            g.tap('down',4)
        if name=='reopen-cursor':g.tap('b',4)
        elif name=='stats-cursor':
            g.tap('a',4);g.tap('a',4);g.step(100);g.tap('a',4)
        elif name=='wrap-up':pass
        elif name=='wrap-down':pass
        elif name=='cancel':
            g.tap('a',4);g.tap('down',4);g.tap('down',4)
        elif name in ['single-switch','swap-wrap']:
            g.tap('a',4);g.tap('down',4)
            if name=='swap-wrap':g.tap('a',4)
        elif name in ['item-cursor','pc-reset','battle-reset']:
            g.tap('b',4);g.tap('b',4)
            if name=='item-cursor':late.open_start(g,'Item');g.tap('a',4)
            elif name=='pc-reset':
                pc.open_pc(g)
                for _ in range(8):
                    if g.st()['screen']=='overworld':break
                    g.tap('b',4)
                assert g.st()['screen']=='overworld'
                late.open_start(g,'Pokemon')
            else:
                # A valid seeded encounter exercises the production transition
                # and then resolves through real battle input.
                g.d.cmd(cmd='start_wild_battle',species='Caterpie',level=2)
                g.battle_loop(prefer='run');g.cutscene()
                late.open_start(g,'Pokemon')
        g.step(3000-g.st()['frame_count'])
        button='up' if name in ['wrap-up','swap-wrap'] else 'down' if name=='wrap-down' else 'a'
        if name in ['pc-reset','battle-reset']:g.step(3)
        else:g.tap(button,1)
        state=g.st();result={'state':state}
        if name=='reopen-cursor':assert state['field_menu']['cursor']==(0 if args.before else 1)
        elif name=='stats-cursor':
            if args.before:assert state['screen']=='stats'
            else:assert state['screen']=='party' and state['field_menu']['cursor']==1
        elif name=='wrap-up':assert state['field_menu']['cursor']==(0 if args.before else 1)
        elif name=='wrap-down':assert state['field_menu']['cursor']==(1 if args.before else 0)
        elif name=='cancel':assert state['screen']==('party' if args.before else 'start-menu')
        elif name=='single-switch':assert state['field_menu']['phase']==('SwitchTarget { source_index: 0 }' if args.before else 'Browsing')
        elif name=='item-cursor':
            assert state['field_menu']['cursor']==(0 if args.before else 1)
            assert state['field_menu']['mode']=='UseItem(Potion)'
        elif name in ['pc-reset','battle-reset']:assert state['field_menu']['cursor']==0
        elif name=='swap-wrap':
            assert state['field_menu']['cursor']==(0 if args.before else 1)
        g.d.cmd(cmd='capture_frame',path=str(args.output/(name+'.png')))
        if name=='swap-wrap':
            g.tap('a',4)
            data=g.d.cmd(cmd='export_fixture')['data']
            expected=fixture['party'] if args.before else list(reversed(fixture['party']))
            assert [mon['species'] for mon in data['party']]==[mon['species'] for mon in expected]
            assert g.st()['field_menu']['phase']=='Browsing'
            result['party']=data['party']
        return result
    finally:g.close()


def run(args):
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=True)
    fidelity_stdio.install(pt,debug_drive,args.driver.resolve())
    fresh,_=pc.prepare(args)
    names=['reopen-cursor','stats-cursor','wrap-up','wrap-down','cancel','single-switch','swap-wrap','item-cursor','pc-reset','battle-reset']
    results={name:case(args,name,fresh) for name in names}
    (args.output/'results.json').write_text(json.dumps(results,indent=2))
    (args.output/'fixture.json').write_text(json.dumps(fresh,indent=2))
    print('Ten party-menu actual-input cases completed')


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('driver',type=Path);p.add_argument('output',type=Path)
    p.add_argument('--before',action='store_true');run(p.parse_args())

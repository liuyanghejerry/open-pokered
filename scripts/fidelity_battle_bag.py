#!/usr/bin/env python3
"""Production input checks for complete battle bag, failed use and ghost balls."""
import argparse
import json
from pathlib import Path
import debug_drive
import fidelity_stdio
import playthrough as pt
import save_builder as sb


def menu(g):
    for _ in range(300):
        if g.st()['battle_phase'] == 'PlayerMenu': return
        g.tap('a', 10)
    raise AssertionError(g.st())


def settle_item(g):
    states = []
    for _ in range(500):
        s = g.st(); states.append(s)
        if s['screen'] != 'battle' or s['battle_phase'] in ['PlayerMenu', 'BagSelect']: break
        g.tap('a', 10)
    else: raise AssertionError(g.st())
    return states


def case(args, name, trainer=False, capture=False):
    fixture = sb.SaveBuilder().party_add('Abra',50,moves=['Teleport'])
    if name == 'inventory':
        fixture.give_item('TOWN_MAP',1).give_item('HELIX_FOSSIL',1).give_item('POKE_BALL',2)
    elif name == 'cancel' or name == 'no-effect': fixture.give_item('POTION',2)
    else:
        fixture.give_item('MASTER_BALL',2)
        if name == 'restless': fixture.give_item('SILPH_SCOPE',1)
    tower = name in ['ghost','restless']
    map_name,x,y = ('PokemonTower6F',10,14) if tower else ('PewterGym',4,2) if trainer else ('PalletTown',5,6)
    assert pt.walkable(map_name,x,y)
    fixture.position(map_name,x,y)
    g=pt.Game(snapshot=fixture.write(args.output/'fixture.json'),seed=0)
    try:
        pt.resume_reentry(g)
        if trainer:
            g.face('up');g.tap('a',1)
            for _ in range(100):
                if g.st()['screen']=='battle':break
                if g.st()['dialogue']:g.skip()
                else:g.step(2)
        else:g.d.cmd(cmd='start_wild_battle',species='Marowak' if tower else 'Caterpie',level=30 if tower else 5)
        g.wait('screen=battle',600);menu(g)
        hp=g.st()['battle_live']['player']['hp']
        g.d.drive(['down','left'],frames=10);g.tap('a',4)
        bag=g.st()['battle_bag']
        if name=='inventory':
            if not args.before:
                assert [i['item'] for i in bag['items']]==['TownMap','HelixFossil','PokeBall'],bag
            if not capture and not args.before:
                g.tap('a',4);states=settle_item(g)
                assert any('OAK: RED!' in (s['battle_message'] or '') for s in states),states
                assert g.st()['battle_phase']=='BagSelect' and g.st()['battle_bag']['cursor']==0
                assert g.st()['battle_live']['player']['hp']==hp
        elif name=='cancel':
            g.tap('a',4);assert g.st()['battle_phase'].startswith('ItemTargetSelect')
            g.tap('b',4)
            if not args.before: assert g.st()['battle_phase']=='BagSelect',g.st()
        elif name=='no-effect':
            g.tap('a',4);g.tap('a',4);states=settle_item(g)
            if not args.before:
                assert g.st()['battle_phase']=='BagSelect',g.st()
                assert g.st()['battle_live']['player']['hp']==hp
                assert any("It won't have any effect." in (s['battle_message'] or '') for s in states)
        else:
            g.tap('a',4)
            if capture:
                g.step(400);g.tap('a',4)
            else:
                states=settle_item(g)
                if not args.before:
                    assert g.st()['screen']=='battle' and g.st()['battle_phase']=='PlayerMenu',g.st()
                    assert any('dodging' in (s['battle_message'] or '') for s in states),states
                result={'states':states,'bag':g.d.cmd(cmd='get_bag'),'inventory':g.st()['battle_inventory']}
                if not args.before:
                    assert next(i['qty'] for i in result['inventory'] if i['item']=='MasterBall')==1,result
                    # Verify settlement writes the spent ball back to the save bag.
                    g.d.drive(['down','right'],frames=10);g.tap('a',4)
                    for _ in range(300):
                        if g.st()['screen']!='battle':break
                        g.tap('a',10)
                    assert g.st()['screen']=='overworld',g.st()
                    result['settled_bag']=g.d.cmd(cmd='get_bag')
                    assert next(i['qty'] for i in result['settled_bag']['data'] if i['item']=='MasterBall')==1
                    assert g.st()['party_count']==1
                return result
        if capture:
            assert g.st()['frame_count'] < 4000
            g.step(4000-g.st()['frame_count'])
            g.d.cmd(cmd='capture_frame',path=str(args.output/(name+'.png')))
        return {'state':g.st(),'bag':g.d.cmd(cmd='get_bag')}
    finally:g.close()


def run(args):
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=True)
    fidelity_stdio.install(pt,debug_drive,args.driver.resolve())
    g=pt.Game()
    try:
        pt.m01_boot(g);pt.m02_oak_speech(g);g.d.cmd(cmd='save');h=pt.Game(save_path=g.save_path)
        try:sb.SaveBuilder._tpl=h.d.cmd(cmd='export_fixture')['data']
        finally:h.close()
    finally:g.close()
    results={}
    for name in ['inventory','cancel','no-effect']:
        for trainer in [False,True]:results[f'{name}-{trainer}']=case(args,name,trainer)
    for name in ['ghost','restless']:results[name]=case(args,name)
    for name in ['inventory','cancel','restless']:results[name+'-capture']=case(args,name,capture=True)
    (args.output/'results.json').write_text(json.dumps(results,indent=2))
    print('Eight battle input scenarios and three frame-4000 captures completed')


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('driver',type=Path);p.add_argument('output',type=Path);p.add_argument('--before',action='store_true')
    run(p.parse_args())

#!/usr/bin/env python3
"""Actual PC/mon-view input, PCM cry checks and matching frame-2603 captures."""
import argparse
import copy
import json
import shutil
from pathlib import Path
import debug_drive
import fidelity_stdio
import playthrough as pt
import playthrough_late as late
import save_builder as sb


def close_message(g):
    for _ in range(40):
        if g.st()['pc_phase'] != 'Message': return
        g.tap('a',4)
    raise AssertionError(g.st())


def open_pc(g, bills=True):
    g.face('up');g.tap('a',4);close_message(g)
    assert g.st()['pc_phase']=='MainMenu',g.st()
    if bills:
        g.tap('a',4);close_message(g)
        assert g.st()['pc_phase']=='BillsMenu',g.st()


def wait_cry(g, start, species, before=False, initial=None):
    initial=initial or g.d.cmd(cmd='audio_state')
    reference=g.d.cmd(cmd='audio_reference_cry',species=species)
    for _ in range(600):
        if not g.d.cmd(cmd='audio_state')['sound_playing']:break
        g.step(1)
    else:raise AssertionError('sound did not end')
    ticks=g.st()['frame_count']-start
    if not before:
        assert initial['sfx_channels']==[True,True,False,True],initial
        # Wave-channel music and master mixer can vary with the map. The
        # three cry channels' readable registers must match the species cry.
        indexes=list(range(10))+list(range(16,20))
        assert [initial['registers'][i] for i in indexes]==[reference['registers'][i] for i in indexes],(initial,reference)
        assert ticks==reference['ticks'],(species,ticks,reference)
    return {'initial_audio':initial,'reference':reference,'ticks':ticks}


def boot(args,fixture,pcm=True):
    path=args.output/'fixture.json';path.write_text(json.dumps(fixture))
    g=pt.Game(snapshot=path,seed=0)
    if pcm:g.d.cmd(cmd='initialize_fixture',save=str(g.save_path),snapshot=str(path),seed=0,pcm_audio=True)
    pt.resume_reentry(g)
    return g


def prepare(args):
    g=pt.Game()
    try:
        pt.m01_boot(g);pt.m02_oak_speech(g);g.d.cmd(cmd='save');h=pt.Game(save_path=g.save_path)
        try:sb.SaveBuilder._tpl=h.d.cmd(cmd='export_fixture')['data']
        finally:h.close()
    finally:g.close()
    builder=sb.SaveBuilder().party_add('Pikachu',12).party_add('Bulbasaur',7).position('PewterPokecenter',13,4)
    fresh=copy.deepcopy(builder.data)
    g=boot(args,fresh,pcm=False)
    try:
        open_pc(g);g.tap('down',4);g.tap('a',4);g.tap('a',4);g.tap('a',4)
        for _ in range(600):
            if g.st()['pc_phase']=='Message':break
            g.step(1)
        assert g.st()['pc_phase']=='Message' and g.st()['party_count']==1
        stored=g.d.cmd(cmd='export_fixture')['data']
        # Preparing the stored fixture takes different time on the baseline.
        # Start each scenario from identical save time as well as box/party.
        stored['game_data']['play_time']=copy.deepcopy(fresh['game_data']['play_time'])
    finally:g.close()
    return fresh,stored


def case(args,name,fresh,stored):
    fixture=copy.deepcopy(fresh if name in ['deposit','party-stats'] else stored)
    if name=='withdraw-full':
        for _ in range(5):fixture['party'].append(sb.make_mon('Magikarp',5))
    g=boot(args,fixture)
    unchanged_before=g.d.cmd(cmd='export_fixture')['data']
    try:
        if name=='party-stats':
            late.open_start(g,'Pokemon');g.tap('a',4)
        else:
            open_pc(g)
            index=1 if name in ['deposit','deposit-last'] else 2 if name in ['release','release-no'] else 0
            for _ in range(index):g.tap('down',4)
            if name not in ['deposit-last','withdraw-full']:
                g.tap('a',4);assert g.st()['pc_phase']=='MonList',g.st()
                g.tap('a',4)
                if name in ['release','release-no']:
                    assert g.st()['pc_phase']=='ReleaseConfirm'
                    if name=='release':g.tap('up',4)
                else:
                    assert g.st()['pc_phase']=='MonAction'
                    if name=='box-stats':g.tap('down',4)
        assert g.st()['frame_count']<2600
        g.step(2600-g.st()['frame_count'])
        g.tap('a',1)
        start=g.st();initial_audio=g.d.cmd(cmd='audio_state');result={'start':start}
        if name in ['deposit-last','withdraw-full','release-no']:
            audio=g.d.cmd(cmd='audio_state');assert audio['sfx_channels']!=[True,True,False,True],audio
            expected='MonList' if name=='release-no' else 'Message'
            assert start['pc_phase']==expected,start
            unchanged=g.d.cmd(cmd='export_fixture')['data']
            assert unchanged['party']==unchanged_before['party']
            assert unchanged['pc_storage']==unchanged_before['pc_storage']
            result['audio']=audio
        else:
            if not args.before and name in ['deposit','withdraw']:
                assert start['pc_phase']=='MonAction',start
                g.d.drive(['b','down'],frames=2)
                assert g.st()['party_count']==start['party_count']
                assert g.st()['pc_phase']==start['pc_phase']
            if name in ['deposit','withdraw','release']:
                # Capture the exact same frame immediately after confirmation.
                # Reboot for captures below: the input-lock checks above advance.
                result['audio']=wait_cry(g,2601,'Pikachu',args.before,initial_audio)
                assert g.st()['pc_phase']=='Message'
                expected=1 if name in ['deposit','release'] else 2
                assert g.st()['party_count']==expected,g.st()
            else:
                result['audio']=wait_cry(g,2601,'Pikachu',args.before,initial_audio)
                assert g.st()['screen']=='stats',g.st()
        result['finish']=g.st()
        result['save']=g.d.cmd(cmd='export_fixture')['data']
        if name in ['deposit','withdraw','release'] and not args.before:
            close_message(g)
            for _ in range(8):
                if g.st()['screen']=='overworld':break
                g.tap('b',4)
            assert g.st()['screen']=='overworld'
            late.open_start(g,'Save')
            for _ in range(20):
                g.tap('a',30)
                if g.save_path.exists() and g.st()['screen']=='overworld' and not g.st().get('field_menu'):break
            assert g.save_path.exists(),g.st()
            check=pt.Game(save_path=g.save_path)
            try:
                pt.resume_reentry(check)
                restored=check.d.cmd(cmd='export_fixture')['data']
                assert restored['party']==result['save']['party']
                # The canonical 33-byte box record omits derived battle stats.
                persistent=lambda storage: {**storage,'boxes':[[{k:v for k,v in mon.items() if k not in ['max_hp','attack','defense','speed','special']} for mon in box] for box in storage['boxes']]}
                assert persistent(restored['pc_storage'])==persistent(result['save']['pc_storage'])
                result['continue']=check.st()
            finally:check.close()
        return result
    finally:g.close()


def capture(args,name,fixture):
    g=boot(args,fixture)
    try:
        open_pc(g)
        for _ in range(1 if name=='deposit' else 2 if name=='release' else 0):g.tap('down',4)
        g.tap('a',4);g.tap('a',4)
        if name=='release':g.tap('up',4)
        assert g.st()['frame_count']<2600
        g.step(2600-g.st()['frame_count']);g.tap('a',1)
        assert g.st()['frame_count']==2603,g.st()['frame_count']
        g.d.cmd(cmd='capture_frame',path=str(args.output/(name+'.png')))
        return g.st()
    finally:g.close()


def hof_case(args):
    source=pt.Game(save_path=args.hof_save.resolve())
    try:fixture=source.d.cmd(cmd='export_fixture')['data']
    finally:source.close()
    assert fixture['game_data']['num_hof_teams']>0 and fixture['hall_of_fame']
    fixture['game_data']['position']={'map_id':sb.map_id('PewterPokecenter'),'x':13,'y':4,'x_block':1,'y_block':0}
    g=boot(args,fixture)
    try:
        open_pc(g,bills=False)
        for _ in range(3):g.tap('down',4)
        g.tap('a',4)
        for _ in range(60):
            g.d.cmd(cmd='press_timeline',buttons=['a',None],advance=True)
            if g.st()['pc_phase']=='LeagueHoF':break
        assert g.st()['pc_phase']=='LeagueHoF',g.st()
        records=[]
        mons=[mon for team in fixture['hall_of_fame'] for mon in team]
        for index,mon in enumerate(mons):
            if index:
                frame=g.st()['frame_count'];g.tap('a',1);start=frame+1
            else:start=g.st()['frame_count']-1
            assert g.st()['pc_phase']=='LeagueHoF',g.st()
            species=sb.species_order()[mon['species']-1]
            records.append({'species':species,'audio':wait_cry(g,start,species,args.before)})
        g.tap('a',1)
        assert g.st()['pc_phase']=='MainMenu'
        assert g.d.cmd(cmd='audio_state')['sfx_channels']!=[True,True,False,True]
        return records
    finally:g.close()


def restored_box_stats(args,stored):
    fixture=copy.deepcopy(stored)
    fixture['pc_storage']['boxes'][0][0]['hp']=7
    fixture['current_box'][0]['hp']=7
    g=boot(args,fixture)
    try:
        late.open_start(g,'Save')
        for _ in range(20):
            g.tap('a',30)
            if g.save_path.exists() and g.st()['screen']=='overworld' and not g.st().get('field_menu'):break
        assert g.save_path.exists()
        save=args.output/'box-stats.sav'
        shutil.copy2(g.save_path,save)
    finally:g.close()
    g=pt.Game(save_path=save)
    try:
        pt.resume_reentry(g)
        original=g.d.cmd(cmd='export_fixture')['data']
        assert original['pc_storage']['boxes'][0][0]['attack']==0
        open_pc(g);g.tap('a',4);g.tap('a',4);g.tap('down',4)
        assert g.st()['pc_phase']=='MonAction'
        g.step(2600-g.st()['frame_count']);g.tap('a',1)
        assert g.st()['screen']=='stats' and g.st()['frame_count']==2603
        displayed=g.d.cmd(cmd='stats_state')['pokemon']
        assert displayed['hp']==7
        if args.before:
            assert [displayed[key] for key in ['attack','defense','speed','special']]==[0]*4
            assert displayed['max_hp']==7
        else:
            expected=stored['pc_storage']['boxes'][0][0]
            assert [displayed[key] for key in ['max_hp','attack','defense','speed','special']]==[expected[key] for key in ['max_hp','attack','defense','speed','special']]
        assert g.d.cmd(cmd='export_fixture')['data']['pc_storage']==original['pc_storage']
        g.d.cmd(cmd='capture_frame',path=str(args.output/'box-restored-stats.png'))
        return {'displayed':displayed,'state':g.st()}
    finally:g.close()


def run(args):
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=True)
    fidelity_stdio.install(pt,debug_drive,args.driver.resolve())
    fresh,stored=prepare(args)
    (args.output/'fresh-fixture.json').write_text(json.dumps(fresh,indent=2))
    (args.output/'stored-fixture.json').write_text(json.dumps(stored,indent=2))
    records={name:case(args,name,fresh,stored) for name in ['deposit','withdraw','release','deposit-last','withdraw-full','release-no','party-stats','box-stats']}
    records['restored-box-stats']=restored_box_stats(args,stored)
    if args.hof_save:records['hall-of-fame']=hof_case(args)
    (args.output/'results.json').write_text(json.dumps(records,indent=2))
    captures={name:capture(args,name,fresh if name=='deposit' else stored) for name in ['deposit','withdraw','release']}
    (args.output/'results.json').write_text(json.dumps(records,indent=2))
    (args.output/'captures.json').write_text(json.dumps(captures,indent=2))
    print('Eight PCM/input cases, restored-box stats, optional Hall of Fame and four same-frame captures completed')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('driver', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--before', action='store_true')
    parser.add_argument('--hof-save', type=Path, help='Save from an actual completed playthrough')
    run(parser.parse_args())

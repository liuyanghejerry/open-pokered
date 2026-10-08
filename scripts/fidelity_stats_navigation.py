#!/usr/bin/env python3
"""Actual party/PC StatsScreen A/B navigation and matching-frame captures."""
import argparse
import json
from pathlib import Path
import debug_drive
import fidelity_stdio
import playthrough as pt
import playthrough_late as late
import fidelity_pc_audio as pc


def scenario(args, owner, first, second, fresh, stored):
    g=pc.boot(args,fresh if owner=='party' else stored)
    try:
        if owner=='party':
            late.open_start(g,'Pokemon');g.tap('a',4)
        else:
            pc.open_pc(g);g.tap('a',4);g.tap('a',4);g.tap('down',4)
        g.step(2600-g.st()['frame_count']);g.tap('a',1)
        assert g.st()['screen']=='stats'
        assert g.d.cmd(cmd='stats_state')['page']=='Stats'
        g.step(3000-g.st()['frame_count']);g.tap(first,1)
        label=f'{owner}-{first}{second}'
        initial=g.st()
        if args.before and first=='b':
            assert initial['screen']==('party' if owner=='party' else 'pc'),initial
            first_page=None
        else:
            assert initial['screen']=='stats',initial
            first_page=g.d.cmd(cmd='stats_state')['page']
            assert first_page=='Moves'
        if first=='b' and second=='a':g.d.cmd(cmd='capture_frame',path=str(args.output/f'{owner}-first-b.png'))
        g.step(3300-g.st()['frame_count'])
        if first_page:g.tap(second,1)
        else:g.step(3)
        finish=g.st()
        if args.before and first_page:
            assert finish['screen']=='stats'
            assert g.d.cmd(cmd='stats_state')['page']=='Stats'
        elif not args.before:
            assert finish['screen']==('party' if owner=='party' else 'pc'),finish
            if owner=='pc':assert finish['pc_phase']=='MonAction'
            else:assert finish['field_menu']['phase']=='Browsing'
        if first=='a' and second=='a':g.d.cmd(cmd='capture_frame',path=str(args.output/f'{owner}-second-a.png'))
        return {'first':initial,'first_page':first_page,'finish':finish}
    finally:g.close()


def run(args):
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=True)
    fidelity_stdio.install(pt,debug_drive,args.driver.resolve())
    fresh,stored=pc.prepare(args)
    results={f'{owner}-{first}{second}':scenario(args,owner,first,second,fresh,stored)
             for owner in ['party','pc'] for first in ['a','b'] for second in ['a','b']}
    (args.output/'results.json').write_text(json.dumps(results,indent=2))
    (args.output/'fixtures.json').write_text(json.dumps([fresh,stored],indent=2))
    print('Eight real-input navigation cases and four matching captures completed')


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('driver',type=Path);p.add_argument('output',type=Path)
    p.add_argument('--before',action='store_true')
    run(p.parse_args())

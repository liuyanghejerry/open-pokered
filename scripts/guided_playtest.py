#!/usr/bin/env python3
"""Guide-derived optional quests within m10; real input, retained evidence.

The manifest separates online guide claims from coordinate resolution and
additional regression properties. No product changes or main-driver patches.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time

from content_regression import Session, require, ROOT
from playthrough import (Game, m01_boot, m02_oak_speech, m03_leave_house,
                        m04_oak_intercept, m05_take_starter, m06_rival_battle,
                        m07_mart_parcel, m08_deliver_parcel, walkable, DELTA)

MANIFEST = Path(__file__).with_name('guided_playtest_sources.json')


class GuidedSession(Session):
    def boot(self, reload=False):
        if reload:
            return super().boot(reload=True)
        # Reuse Session's tracing without seeding a starter in the real quest.
        self.g = Game(save_path=self.save)
        raw = self.g.d.cmd

        def traced(**kw):
            response = raw(**kw)
            self.trace.write(json.dumps({'request': kw, 'response': response}) + '\n')
            self.trace.flush()
            return response

        self.g.d.cmd = traced
        m01_boot(self.g)
        m02_oak_speech(self.g)


def visit_daisy(s):
    s.g.nav_to(13, 6, 'PalletTown')
    s.g.nav_warp(13, 5, 'PalletTown', 'BluesHouse')
    s.g.nav_to(2, 4, 'BluesHouse')
    s.g.face('up')
    return s.interact()


def leave_daisy(s):
    s.g.nav_warp(3, 7, 'BluesHouse', 'PalletTown', approach='down')


def town_map(s):
    m03_leave_house(s.g)
    before = visit_daisy(s)
    require(s.qty('TownMap') == 0, 'Town Map granted before Pokedex')
    require('lab' in before.lower(), f'Daisy early conversation missing: {before!r}')
    s.observe('before errand: no Town Map')
    leave_daisy(s)
    for milestone in (m04_oak_intercept, m05_take_starter, m06_rival_battle,
                      m07_mart_parcel, m08_deliver_parcel):
        milestone(s.g)
    s.g.nav_to_map(13, 6, 'PalletTown')
    visit_daisy(s)
    s.observe('Daisy reward after real parcel delivery')
    require(s.qty('TownMap') == 1, 'Daisy must grant Town Map after real errand')
    require(not next(n for n in s.g.d.npcs() if n['text_id'] == 3)['visible'],
            'table map remains visible after gift')
    s.interact()
    require(s.qty('TownMap') == 1, 'Daisy gift duplicated')
    s.reload()
    s.g.face('up')
    s.interact()
    require(s.qty('TownMap') == 1, 'Daisy gift lost or duplicated after restart')


def pickup(s, map_name, approach, facing, item):
    dx, dy = DELTA[facing]
    # A hidden item can occupy walkable ground. Arrive facing it rather than
    # using the legacy face() helper, whose directional tap can take a step.
    if walkable(map_name, approach[0] + dx, approach[1] + dy):
        s.g.nav_to(approach[0] - dx, approach[1] - dy, map_name=map_name)
    s.g.nav_to(*approach, map_name=map_name)
    if s.g.st()['player_facing'].lower() != facing:
        s.g.face(facing)
    ready = s.g.st()
    require((ready['player_x'], ready['player_y']) == approach
            and ready['player_facing'].lower() == facing,
            f'interaction alignment failed before A: {ready}')
    old = s.qty(item)
    text = s.interact()
    s.observe(f'{map_name} {approach} {facing}: {item}')
    require(s.qty(item) == old + 1, f'{item} pickup failed at {approach}: {text!r}')
    s.interact()
    require(s.qty(item) == old + 1, f'{item} duplicated at {approach}')


def viridian_potion(s):
    s.cmd(cmd='give_pokemon', species='Bulbasaur', level=5)
    s.warp('ViridianCity', 20, 32)  # city south entrance, not the item
    pickup(s, 'ViridianCity', (13, 4), 'right', 'Potion')
    s.g.nav_to(20, 32, 'ViridianCity')
    s.reload()
    s.g.nav_to(13, 4, 'ViridianCity')
    s.g.face('right')
    s.interact()
    require(s.qty('Potion') == 1, 'city Potion duplicated or lost on revisit')


def forest_collection(s):
    # Strong legal species/level fixture keeps trainer fights from dominating
    # this collection probe; it does not establish natural m10 reachability.
    s.cmd(cmd='give_pokemon', species='Bulbasaur', level=20)
    s.warp('ViridianForest', 17, 46)  # south entrance, one warp per scenario
    for approach, facing, item in [((15, 42), 'right', 'Antidote'),
                                   ((2, 31), 'left', 'PokeBall'),
                                   ((25, 12), 'up', 'Antidote'),
                                   ((12, 28), 'down', 'Potion'),
                                   ((1, 19), 'up', 'Potion')]:
        pickup(s, 'ViridianForest', approach, facing, item)
    expected = {'Antidote': 2, 'PokeBall': 1, 'Potion': 2}
    require({k: s.qty(k) for k in expected} == expected, 'forest collection totals differ')
    s.g.nav_warp(1, 0, 'ViridianForest', 'ViridianForestNorthGate')
    s.reload()
    require({k: s.qty(k) for k in expected} == expected, 'collection lost after exit/save/restart')


CASES = {'daisy-town-map-errand': town_map,
         'viridian-hidden-potion-walk': viridian_potion,
         'forest-five-item-tour': forest_collection}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--list', action='store_true')
    parser.add_argument('--only', choices=CASES)
    parser.add_argument('--repeat', type=int, default=1)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_text())
    require(set(CASES) <= set(manifest['cases']), 'case missing guide provenance')
    if args.list:
        for name in CASES:
            print(name, ':', manifest['cases'][name]['goal'])
        return 0
    if args.repeat < 1:
        parser.error('--repeat must be positive')
    output = args.output or Path(tempfile.mkdtemp(prefix='pokered-guided-'))
    require(not output.exists() or not any(output.iterdir()), 'output must be new or empty')
    output.mkdir(parents=True, exist_ok=True)
    report = {'engine_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'binary_sha256': hashlib.sha256((ROOT / 'target/debug/pokered-app').read_bytes()).hexdigest(),
              'driver_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'manifest_sha256': hashlib.sha256(MANIFEST.read_bytes()).hexdigest(),
              'sources': manifest['sources'], 'results': []}
    (output / 'sources.json').write_bytes(MANIFEST.read_bytes())
    started = time.monotonic()
    for attempt in range(1, args.repeat + 1):
        for name in ([args.only] if args.only else CASES):
            print(f'== {name} attempt {attempt}', flush=True)
            s = GuidedSession(output / f'{name}-{attempt}')
            result = {'id': name, 'attempt': attempt, 'status': 'pass',
                      'contract': manifest['cases'][name]}
            began = time.monotonic()
            try:
                s.boot()
                CASES[name](s)
                s.observe('completed')
            except Exception as error:
                result.update(status='fail', error=f'{type(error).__name__}: {error}')
                if s.g:
                    try:
                        s.observe('failure')
                    except Exception as capture:
                        result['capture_error'] = str(capture)
            finally:
                s.close()
            counts = Counter(json.loads(line)['request']['cmd']
                             for line in (s.output / 'protocol.jsonl').read_text().splitlines())
            result['protocol_command_counts'] = dict(counts)
            result['wall_seconds'] = round(time.monotonic() - began, 3)
            report['results'].append(result)
            report['wall_seconds'] = round(time.monotonic() - started, 3)
            (output / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
            print(json.dumps(result, ensure_ascii=False), flush=True)
    print(f'Report: {output / "report.json"}')
    return int(any(r['status'] != 'pass' for r in report['results']))


if __name__ == '__main__':
    raise SystemExit(main())

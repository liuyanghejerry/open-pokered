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
from playthrough import resume_reentry

MANIFEST = Path(__file__).with_name('guided_playtest_sources.json')


class GuidedSession(Session):
    def boot(self, reload=False, snapshot=None):
        if reload:
            return super().boot(reload=True)
        # Reuse Session's tracing without seeding a starter in the real quest.
        self.g = Game(save_path=self.save, snapshot=snapshot)
        raw = self.g.d.cmd

        def traced(**kw):
            response = raw(**kw)
            self.trace.write(json.dumps({'request': kw, 'response': response}) + '\n')
            self.trace.flush()
            return response

        self.g.d.cmd = traced
        if snapshot is not None:
            resume_reentry(self.g)
        else:
            m01_boot(self.g)
            m02_oak_speech(self.g)

    def money_fixture(self, amount):
        from save_builder import SaveBuilder
        builder = SaveBuilder().money(amount)
        path = self.output / 'money-fixture.json'
        builder.write(path)
        self.stop_game()
        self.boot(snapshot=path)
        require(self.g.st()['money'] == amount, 'money fixture did not load')


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


def museum(s, money, accept):
    s.money_fixture(money)
    s.warp('PewterCity', 14, 8)
    s.g.nav_warp(14, 7, 'PewterCity', 'Museum1F')
    s.g.nav_to(10, 4, 'Museum1F')
    choice = s.g.dialogue_then_choice()
    require([x.upper() for x in choice['options']] == ['YES', 'NO'],
            f'unexpected ticket choice: {choice}')
    s.observe('ticket offer')
    selected = 0 if accept else 1
    if choice['selected'] != selected:
        s.g.tap('down' if selected else 'up')
    s.g.tap('a')
    require(s.g.cutscene(), 'ticket dialogue never returned control')
    bought = accept and money >= 50
    s.observe('ticket decision')
    require(s.g.st()['money'] == money - (50 if bought else 0), 'wrong ticket charge')
    require(bool(s.cmd(cmd='get_flags').get('EVENT_BOUGHT_MUSEUM_TICKET')) == bought,
            'ticket flag contradicts payment')
    if not bought:
        require(s.g.st()['player_y'] == 5, 'denied visitor was not moved back')
        return
    s.g.nav_warp(7, 7, 'Museum1F', 'Museum2F')
    s.g.nav_to(7, 6, 'Museum2F')
    s.g.face('up')
    text = s.interact()
    s.observe('upstairs exhibit conversation')
    require('SPACE' in text.upper(), f'upstairs scientist dialogue missing: {text!r}')
    require(s.g.st()['money'] == money - 50, 'visit charged twice')


def rival_oak_balls(s):
    for milestone in (m03_leave_house, m04_oak_intercept, m05_take_starter,
                      m06_rival_battle, m07_mart_parcel, m08_deliver_parcel):
        milestone(s.g)
    require(s.qty('PokeBall') == 0, 'expected no balls before optional rival')
    s.g.nav_to_map(12, 7, 'Route1')
    heal = ((23, 25), 'ViridianCity', 'ViridianPokecenter')
    require(s.g.train_until(13, 'Route1', (12, 7), heal), 'rival preparation training stalled')
    s.g.heal_pokecenter(*heal)
    # The common Brock driver prioritizes Vine Whip, resisted by both of
    # this rival's Pokemon. Scope the actual player strategy to this quest.
    s.g.PREFERRED_MOVES = ['Tackle', 'VineWhip']
    for attempt in range(3):
        s.g.nav_to_map(30, 5, 'Route22')
        s.observe(f'before optional rival trigger {attempt + 1}')
        require(s.cmd(cmd='get_flags').get('EVENT_ROUTE22_RIVAL_WANTS_BATTLE'),
                'optional rival was not armed by real story progression')
        s.g.nav_to(29, 5, 'Route22')
        require(s.g.cutscene(), 'rival approach stalled')
        s.g.wait('screen=battle', 900)
        s.observe('optional rival battle')
        s.g.battle_loop(prefer='fight')
        s.g.wait('not_battle', 1800)
        require(s.g.cutscene(), 'rival departure stalled')
        s.observe('after optional rival')
        if s.cmd(cmd='get_flags').get('EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE'):
            break
    else:
        raise ExplorationIncomplete('rival not defeated in 3 attempts; Oak reward prerequisite unmet')
    require(s.qty('PokeBall') == 0, 'unexpected balls before Oak reward')
    s.g.nav_to_map(5, 3, 'OaksLab')
    s.g.face('up')
    text = s.interact()
    s.observe('Oak five-ball reward')
    require(s.qty('PokeBall') == 5, f'Oak five-ball reward missing: {text!r}')
    s.interact()
    require(s.qty('PokeBall') == 5, 'Oak reward duplicated')
    s.reload()
    s.g.face('up')
    s.interact()
    require(s.qty('PokeBall') == 5, 'Oak reward lost or duplicated after restart')


def museum_reentry(s):
    museum(s, 100, True)
    s.g.nav_warp(7, 7, 'Museum2F', 'Museum1F')
    s.g.nav_warp(10, 7, 'Museum1F', 'PewterCity', approach='down')
    s.observe('left museum')
    require(not s.cmd(cmd='get_flags').get('EVENT_BOUGHT_MUSEUM_TICKET'),
            'ticket should reset when leaving museum for Pewter')
    # Exit lands on the exterior door tile; step off before entering again.
    s.g.nav_to(14, 8, 'PewterCity')
    s.g.nav_warp(14, 7, 'PewterCity', 'Museum1F')
    s.g.nav_to(10, 4, 'Museum1F')
    choice = s.g.dialogue_then_choice()
    require(choice['options'] == ['YES', 'NO'], 'return visit must offer a fresh ticket')
    if choice['selected']:
        s.g.tap('up')
    s.g.tap('a')
    require(s.g.cutscene(), 'second ticket purchase stalled')
    s.observe('second museum admission')
    require(s.g.st()['money'] == 0, 'second admission must charge another 50')
    require(s.cmd(cmd='get_flags').get('EVENT_BOUGHT_MUSEUM_TICKET'), 'second ticket missing')


class ExplorationIncomplete(RuntimeError):
    """Random exploration budget exhausted, not proof of missing content."""


def forest_wild_catch(s, target=None):
    from scenarios import throw_balls_until_caught, dismiss_dex_screen
    s.cmd(cmd='give_pokemon', species='Bulbasaur', level=20)
    s.cmd(cmd='give_item', item='POKE_BALL', qty=20)
    s.warp('ViridianForest', 17, 46)
    s.g.nav_to(3, 30, 'ViridianForest')
    encountered = False
    for step in range(500):
        s.g.d.drive(['left' if step % 2 == 0 else 'right'] * 8, frames=12)
        if s.g.st()['screen'] != 'battle':
            continue
        # Natural encounters show an intro message that needs A.
        for _ in range(100):
            if s.g.st()['battle_phase'] == 'PlayerMenu':
                break
            s.g.tap('a', 20)
        require(s.g.st()['battle_phase'] == 'PlayerMenu', 'natural encounter intro did not settle')
        candidate = s.g.st()['battle_live']['enemy']
        s.observations.append({'label': 'encounter search', 'walking_segment': step,
                               'species': candidate['species'], 'level': candidate['level']})
        if target is None or candidate['species'] == target:
            encountered = True
            break
        s.g.battle_loop(prefer='run')
        s.g.wait('not_battle', 900)
        s.g.nav_to(3, 30, 'ViridianForest')
    if not encountered:
        raise ExplorationIncomplete(f'no matching forest encounter ({target}) in 500 walking segments')
    state = s.g.st()
    enemy = state['battle_live']['enemy']
    allowed = {'Caterpie': {3}, 'Metapod': {4}, 'Weedle': {3, 4, 5},
               'Kakuna': {4, 5, 6}, 'Pikachu': {3, 5}}
    s.observe('naturally encountered forest Pokemon')
    require(enemy['species'] in allowed and enemy['level'] in allowed[enemy['species']],
            f'forest encounter contradicts Red guide: {enemy}')
    try:
        throw_balls_until_caught(s.g, 20)
    except AssertionError:
        if s.qty('PokeBall') == 0:
            raise ExplorationIncomplete('20 balls exhausted without a catch')
        raise
    dismiss_dex_screen(s.g)
    party = s.cmd(cmd='get_party')
    require(len(party) == 2 and party[1]['species'] == enemy['species']
            and party[1]['level'] == enemy['level'], 'caught Pokemon differs from natural encounter')
    require(0 <= s.qty('PokeBall') < 20, 'capture consumed no Pokeballs')
    s.observe('caught natural forest Pokemon')
    s.reload()
    require(s.cmd(cmd='get_party') == party, 'caught party changed across save/restart')


CASES = {'daisy-town-map-errand': town_map,
         'viridian-hidden-potion-walk': viridian_potion,
         'forest-five-item-tour': forest_collection,
         'museum-insufficient-money': lambda s: museum(s, 49, True),
         'museum-exact-ticket-visit': lambda s: museum(s, 50, True),
         'museum-decline-ticket': lambda s: museum(s, 100, False),
         'museum-reentry-ticket-reset': museum_reentry,
         'route22-rival-oak-balls': rival_oak_balls,
         'forest-natural-encounter-catch': forest_wild_catch,
         'forest-pikachu-search-catch': lambda s: forest_wild_catch(s, 'Pikachu')}


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
                result.update(status='inconclusive' if isinstance(error, ExplorationIncomplete) else 'fail',
                              error=f'{type(error).__name__}: {error}')
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

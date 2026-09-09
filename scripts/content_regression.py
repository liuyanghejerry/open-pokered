#!/usr/bin/env python3
"""m01–m10 side-content MVP. Seed setup; exercise interactions through input.

No engine/data imports for expected results: oracle is pinned pret assembly.
Independent fresh games; exit 1 for any failed case, retain protocol evidence.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time

from playthrough import Game, m01_boot, m02_oak_speech, resume_reentry

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = Path(__file__).with_name('content_regression_fixtures')
CASES = {}


def case(name):
    def register(fn):
        CASES[name] = fn
        return fn
    return register


def require(condition, message):
    if not condition:
        raise AssertionError(message)


class Session:
    def __init__(self, output, expected_gift_qty=1):
        self.output = output
        output.mkdir(parents=True, exist_ok=False)
        self.expected_gift_qty = expected_gift_qty
        self.save = output / 'play.sav'
        self.trace = (output / 'protocol.jsonl').open('w')
        self.g = None
        self.observations = []

    def boot(self, reload=False):
        self.g = Game(save_path=self.save)
        raw = self.g.d.cmd
        def traced(**kw):
            response = raw(**kw)
            self.trace.write(json.dumps({'request': kw, 'response': response}) + '\n')
            self.trace.flush()
            return response
        self.g.d.cmd = traced
        if reload:
            resume_reentry(self.g)
        else:
            m01_boot(self.g)
            m02_oak_speech(self.g)
            self.cmd(cmd='give_pokemon', species='Bulbasaur', level=5)

    def cmd(self, **kw):
        r = self.g.d.cmd(**kw)
        require(r['ok'], f'protocol error {kw}: {r}')
        return r.get('data')

    def observe(self, label):
        data = {'label': label, 'state': self.g.st(), 'bag': self.cmd(cmd='get_bag'),
                'flags': self.cmd(cmd='get_flags'), 'npcs': self.g.d.npcs()}
        self.observations.append(data)
        (self.output / 'observations.json').write_text(json.dumps(self.observations, indent=2))
        return data

    def qty(self, item):
        return sum(i['qty'] for i in self.cmd(cmd='get_bag') if i['item'] == item)

    def warp(self, map_name, x, y):
        self.cmd(cmd='warp', map=map_name, x=x, y=y)
        self.g.step(40)
        self.g.wait('control_ready', 600)
        s = self.g.st()
        require((s['map_name'], s['player_x'], s['player_y']) == (map_name, x, y),
                f'warp setup failed: {s}')

    def interact(self):
        """Real edge-triggered A for every page; no internal skip_dialogue."""
        self.g.tap('a', 12)
        texts = []
        for _ in range(100):
            s = self.g.st()
            effect = s.get('script_effect') or {}
            dialogue = s.get('dialogue_state') or {}
            text = effect.get('text')
            if not text and dialogue.get('waiting_for_input'):
                text = dialogue.get('text')
            if text and (not texts or texts[-1] != text):
                texts.append(text)
            require(s['screen'] == 'overworld', f'unexpected screen {s["screen"]}')
            require(s.get('choice') is None, 'unexpected choice')
            if not s.get('script_running') and not s.get('dialogue_state'):
                self.observations.append({'dialogue': texts})
                return ' '.join(' '.join(texts).split())
            self.g.tap('a', 12)
        raise AssertionError('interaction did not return control in 100 A taps')

    def stop_game(self):
        if self.g:
            self.g.log.flush()
            with (self.output / 'game.log').open('a') as out:
                out.write((self.g.run_dir / 'game.log').read_text())
            self.g.close()
            self.g = None

    def reload(self):
        self.cmd(cmd='save')
        self.stop_game()
        self.boot(reload=True)

    def close(self):
        self.stop_game()
        self.trace.close()

    def full_bag(self, exclude=None):
        # Twenty distinct ordinary item stacks; no tested POTION/ANTIDOTE.
        items = ['POKE_BALL', 'GREAT_BALL', 'ULTRA_BALL', 'MASTER_BALL',
                 'BURN_HEAL', 'ICE_HEAL', 'AWAKENING', 'PARLYZ_HEAL',
                 'FULL_RESTORE', 'MAX_POTION', 'HYPER_POTION', 'SUPER_POTION',
                 'ESCAPE_ROPE', 'REPEL', 'MAX_REPEL', 'SUPER_REPEL',
                 'HP_UP', 'PROTEIN', 'IRON', 'CARBOS']
        if exclude in items:
            items[items.index(exclude)] = 'ETHER'
        for item in items:
            self.cmd(cmd='give_item', item=item, qty=1)
        require(len(self.cmd(cmd='get_bag')) == 20, 'full bag fixture not 20 stacks')

    def route_rep(self):
        # NPC walks vertically. Follow observed position without changing NPC state.
        for _ in range(12):
            npc = next(n for n in self.g.d.npcs() if n['text_id'] == 1)
            require((npc['home_x'], npc['home_y']) == (5, 24), 'Route1 rep home differs from pret')
            self.g.nav_to(npc['x'] + 1, npc['y'])
            self.g.face('left')
            s = self.g.st()
            npc = next(n for n in self.g.d.npcs() if n['text_id'] == 1)
            if (s['player_x'] - 1, s['player_y']) == (npc['x'], npc['y']):
                return self.interact()
        raise AssertionError('could not reach wandering Route1 rep')


@case('route1-sample-once-reload')
def route_sample(s):
    s.warp('Route1', 7, 24)
    s.route_rep()
    s.observe('first gift')
    require(s.qty('Potion') == s.expected_gift_qty,
            f'Route1 first gift expected {s.expected_gift_qty} Potion, observed {s.qty("Potion")}')
    require('also carry' in s.route_rep(), 'repeat must use post-gift dialogue')
    require(s.qty('Potion') == 1, 'Route1 sample duplicated')
    s.reload()
    require('also carry' in s.route_rep(), 'post-gift dialogue lost after restart')
    require(s.qty('Potion') == 1, 'Route1 sample duplicated after restart')


@case('route1-full-bag-consumes-offer')
def route_full(s):
    s.full_bag()
    s.warp('Route1', 7, 24)
    text = s.route_rep()
    s.observe('full-bag gift refusal')
    require('too much stuff' in text, f'expected bag-full dialogue, got {text!r}')
    require(s.qty('Potion') == 0, 'full bag unexpectedly gained Potion')
    # Original CheckAndSetEvent precedes GiveItem: unlike ground pickups,
    # the sample is permanently lost even on failure. Preserve this Gen I quirk.
    require('also carry' in s.route_rep(), 'full-bag offer must remain consumed in original')
    s.reload()
    require('also carry' in s.route_rep(), 'full-bag offer rearmed after restart')


@case('forest-npc-position-dialogue')
def forest_npc(s):
    s.warp('ViridianForest', 15, 44)
    npc = next(n for n in s.g.d.npcs() if n['text_id'] == 1)
    require((npc['x'], npc['y'], npc['visible']) == (16, 43, True), 'forest NPC placement differs')
    s.g.nav_to(16, 44)
    s.g.face('up')
    text = s.interact()
    require('I came here with some friends!' in text, f'wrong forest NPC dialogue: {text!r}')
    require("They're out for POKeMON fights!" in text, f'missing forest NPC text: {text!r}')
    require(s.cmd(cmd='get_bag') == [], 'flavor dialogue changed inventory')


def face_visible_antidote(s):
    s.warp('ViridianForest', 24, 12)
    s.g.nav_to(25, 12)
    s.g.face('up')


@case('forest-visible-antidote-once-reload')
def visible(s):
    face_visible_antidote(s)
    s.interact()
    s.observe('visible pickup')
    require(s.qty('Antidote') == 1, 'visible ANTIDOTE pickup not delivered')
    require(not next(n for n in s.g.d.npcs() if n['text_id'] == 5)['visible'], 'collected object remains visible')
    s.interact()
    require(s.qty('Antidote') == 1, 'visible item duplicated immediately')
    s.warp('Route1', 7, 24)
    face_visible_antidote(s)
    require(not next(n for n in s.g.d.npcs() if n['text_id'] == 5)['visible'], 'collected object returned on map reentry')
    s.reload()
    s.g.face('up')
    s.interact()
    require(s.qty('Antidote') == 1, 'visible item duplicated after restart')
    require(not next(n for n in s.g.d.npcs() if n['text_id'] == 5)['visible'], 'collected object returned after restart')


def visible_full_retry(s, item, name, text_id, x, y):
    s.full_bag(exclude=item)
    s.warp('ViridianForest', x, y + 1)
    s.g.face('up')
    flag = 'EVENT_GOT_VIRIDIAN_FOREST_' + item
    def assert_available(label):
        data = s.observe(label)
        require(next(n for n in data['npcs'] if n['text_id'] == text_id)['visible'],
                f'{item} object disappeared after refused pickup')
        require(flag not in data['flags'], f'{item} pickup flag set on failure')
        require(s.qty(name) == 0, f'full bag unexpectedly gained {item}')

    for attempt in range(2):
        text = s.interact()
        assert_available(f'{item} refusal {attempt + 1}')
        require('No more room for items!' in text, f'bag-full refusal missing: {text!r}')
    s.reload()
    assert_available(f'{item} refusal after restart')

    # Make room through the actual ITEM -> TOSS menu, not a debug mutation.
    from scenarios import pause_menu
    pause_menu(s.g)
    s.g.tap('down', 10)  # no Pokedex: POKEMON, ITEM
    s.g.tap('a', 10)
    require(s.g.st()['screen'] == 'bag', 'ITEM menu did not open')
    s.g.tap('a', 10)  # first stack -> USE / TOSS / CANCEL
    s.g.tap('down', 10)
    s.g.tap('a', 10)  # TOSS quantity (one)
    s.g.tap('a', 10)
    require(len(s.cmd(cmd='get_bag')) == 19, 'TOSS did not free a bag slot')
    for _ in range(10):
        if s.g.st()['screen'] == 'overworld':
            break
        s.g.tap('b', 10)
    s.g.face('up')
    s.interact()
    require(s.qty(name) == 1, f'{item} not collectible after making room')
    s.reload()
    require(s.qty(name) == 1, f'{item} quantity lost after restart')
    require(not next(n for n in s.g.d.npcs() if n['text_id'] == text_id)['visible'],
            f'{item} collected object returned after restart')
    s.g.face('up')
    s.interact()
    require(s.qty(name) == 1, f'{item} duplicated after restart')


@case('forest-visible-full-bag-keeps-object')
def visible_full(s):
    visible_full_retry(s, 'ANTIDOTE', 'Antidote', 5, 25, 11)


@case('forest-potion-full-bag-retry')
def potion_full(s):
    visible_full_retry(s, 'POTION', 'Potion', 6, 12, 29)


@case('forest-pokeball-full-bag-retry')
def pokeball_full(s):
    visible_full_retry(s, 'POKE_BALL', 'PokeBall', 7, 1, 31)


@case('forest-full-bag-existing-stack')
def visible_existing_stack(s):
    # A full set of distinct slots can still accept another Poke Ball.
    s.full_bag()
    s.warp('ViridianForest', 1, 32)
    s.g.face('up')
    text = s.interact()
    require('found' in text, f'existing stack should accept pickup: {text!r}')
    require(s.qty('PokeBall') == 2, 'pickup did not join existing stack')
    require(len(s.cmd(cmd='get_bag')) == 20, 'pickup changed distinct slot count')
    s.reload()
    require(s.qty('PokeBall') == 2, 'merged stack lost after restart')
    require(not next(n for n in s.g.d.npcs() if n['text_id'] == 7)['visible'],
            'collected Poke Ball returned after restart')


def face_hidden_antidote(s):
    s.warp('ViridianForest', 14, 42)
    s.g.nav_to(15, 42)
    s.g.face('right')


@case('forest-hidden-antidote-once-reload')
def hidden(s):
    face_hidden_antidote(s)
    s.interact()
    s.observe('hidden pickup')
    require(s.qty('Antidote') == 1, 'hidden ANTIDOTE at (16,42) not delivered')
    s.interact()
    require(s.qty('Antidote') == 1, 'hidden item duplicated immediately')
    s.reload()
    s.g.face('right')
    s.interact()
    require(s.qty('Antidote') == 1, 'hidden item duplicated after restart')


@case('forest-hidden-full-bag-repeatable')
def hidden_full(s):
    s.full_bag()
    face_hidden_antidote(s)
    for _ in range(2):
        text = s.interact()
        s.observe('full-bag hidden pickup')
        require(s.qty('Antidote') == 0, 'full bag unexpectedly gained Antidote')
        require('found' in text.lower() and 'room' in text.lower(),
                f'hidden item must remain discoverable and refuse full bag: {text!r}')


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--only', help='comma-separated case IDs')
    ap.add_argument('--list', action='store_true')
    ap.add_argument('--output', type=Path, help='new evidence directory (default /tmp)')
    ap.add_argument('--repeat', type=int, default=1)
    ap.add_argument('--calibrate', action='store_true', help='mutate Route1 gift quantity expectation from 1 to 2; run that case only')
    args = ap.parse_args()
    if args.list:
        print('\n'.join(CASES)); return 0
    if args.repeat < 1:
        ap.error('--repeat must be positive')
    selected = args.only.split(',') if args.only else list(CASES)
    if args.calibrate:
        selected = ['route1-sample-once-reload']
    if len(set(selected)) != len(selected):
        ap.error('duplicate case ID')
    if any(name not in CASES for name in selected):
        ap.error('unknown case ID')
    oracle = json.loads((FIXTURES / 'oracle.json').read_text())
    for entry in oracle['files']:
        require(hashlib.sha256((FIXTURES / entry['fixture']).read_bytes()).hexdigest() == entry['sha256'],
                f'oracle fixture changed: {entry["fixture"]}')
    output = args.output or Path(tempfile.mkdtemp(prefix='pokered-content-'))
    require(not output.exists() or not any(output.iterdir()), 'use an empty output directory to preserve evidence and avoid stale saves')
    output.mkdir(parents=True, exist_ok=True)
    results = []
    start = time.monotonic()
    for attempt in range(1, args.repeat + 1):
        for name in selected:
            print(f'== {name} attempt {attempt}', flush=True)
            s = Session(output / f'{name}-{attempt}', expected_gift_qty=2 if args.calibrate else 1)
            began = time.monotonic()
            result = {'id': name, 'attempt': attempt, 'status': 'pass'}
            try:
                s.boot()
                CASES[name](s)
                s.observe('final')
            except Exception as error:
                result.update(status='fail', error=f'{type(error).__name__}: {error}')
                if s.g:
                    try: s.observe('failure')
                    except Exception as capture_error: result['capture_error'] = str(capture_error)
            finally:
                s.close()
            result['wall_seconds'] = round(time.monotonic() - began, 3)
            results.append(result)
            print(json.dumps(result), flush=True)
    report = {'oracle_commit': oracle['commit'], 'oracle_repository': oracle['repository'],
              'engine_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True, cwd=ROOT).strip(),
              'setup': 'fresh power-on, seeded starter/bag, debug warp, local real-input navigation and interaction',
              'calibration': args.calibrate, 'wall_seconds': round(time.monotonic() - start, 3), 'results': results}
    (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f'Report: {output / "report.json"}')
    return int(any(r['status'] != 'pass' for r in results))


if __name__ == '__main__':
    raise SystemExit(main())

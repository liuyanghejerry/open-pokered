"""Jev decisions over the existing real-input playthrough skill library.

No scripted state writes are permitted. Navigation and button timing stay in
playthrough.Game; this module replaces tactical move selection with a bounded
judgment and caches decisions only when their complete abstract state matches.
"""
import json
import math
import re
import time

import playthrough as pt
import playthrough_late as late

from .client import AgentClient
from .judgment_agent import load_objectives
from .story_agent import DualStoryAgent, StoryStopped
from .story_rules import evaluate


ITEM_CATALOG = {item['id']: item for path in (late.DATA / 'data/items').glob('*.json')
                if (item := json.loads(path.read_text())).get('id')}
MEDICINES = {name: item for name, item in ITEM_CATALOG.items() if item.get('category') == 'Medicine'}


def medicine_options(party, bag):
    """Legal useful recovery candidates, grounded in public item effects."""
    for name, qty in bag.items():
        if qty <= 0 or name not in MEDICINES:
            continue
        item = MEDICINES[name]
        tags = item.get('tags', [])
        for index, mon in enumerate(party):
            status = mon.get('status', 'None')
            effect = item['effect']['type']
            restores_pp = (effect == 'PpRestore' and item['effect']['params'].get('all')
                and any(name != 'None' and late.move_data(name)['power'] > 0
                        and pp <= late.move_data(name)['pp']*.5 for name, pp in zip(mon.get('moves', []), mon.get('pp', []))))
            cures = status != 'None' and (effect in ('CureAllStatus', 'FullRestore') or
                any(effect == cure and status.startswith(condition) for cure, condition in
                    [('CurePoison', 'Poison'), ('CureSleep', 'Sleep'), ('CureParalysis', 'Paraly'),
                     ('CureBurn', 'Burn'), ('CureFreeze', 'Freez')]))
            useful = ('revive' in tags and mon['hp'] == 0 or mon['hp'] > 0 and
                      ('hp' in tags and mon['hp'] < mon['max_hp'] or
                       'cure' in tags and cures or restores_pp))
            if useful:
                yield name, index, {'item': name, 'quantity': qty, 'target': mon,
                                    'effect': item['effect'], 'tags': tags}


def effective_attacks(mon, enemy):
    types = {late.species_data(enemy)[key] for key in ('type1', 'type2')}
    return [name for name, pp in zip(mon['moves'], mon['pp']) if name != 'None' and pp > 0
            and late.move_data(name)['power'] > 0 and all(
                late.type_chart().get((late.move_data(name)['type'], typ), 1) > 0 for typ in types)]


class NavigationPause(RuntimeError):
    """Return to strategy after combat; local path retries must not swallow this."""


class ObservedProtocol:
    """Audit the actual debug commands; reject shortcuts before sending them."""
    ALLOWED = {
        'get_state', 'get_position', 'get_party', 'get_bag', 'get_flags',
        'get_npcs', 'get_agent_state', 'get_nearby', 'get_script_semantics',
        'get_world_graph', 'find_world_route', 'wait_until', 'skip_dialogue',
        'press', 'press_sequence', 'press_timeline', 'step_frames',
        'capture_frame', 'move_to', 'interact', 'interact_with', 'travel_to',
    }

    def __init__(self, raw, record, deadline):
        self.raw, self.record, self.deadline = raw, record, deadline
        self.counts = {}
        self.stop_requested = False

    def cmd(self, **kwargs):
        if self.stop_requested:
            raise StoryStopped('interrupted_at_command_boundary')
        name = kwargs['cmd']
        if name not in self.ALLOWED:
            raise StoryStopped(f'forbidden_debug_command:{name}')
        if time.monotonic() >= self.deadline:
            raise StoryStopped('wall_budget')
        if name == 'press_timeline':
            kwargs['advance'] = True
        self.counts[name] = self.counts.get(name, 0) + 1
        return self.raw.cmd(**kwargs)

    def drive(self, buttons, frames=None):
        # Queue and execute in one request. The driven-only loop can drain a
        # queued input between two RPCs, making queue-then-step overshoot.
        frames = len(buttons) if frames is None else frames
        if frames < len(buttons):
            raise ValueError('input timeline exceeds requested frame count')
        result = self.cmd(cmd='press_timeline', buttons=list(buttons) + [None] * (frames-len(buttons)))
        if not result['ok']:
            raise RuntimeError(result)
        data = result.get('data', {})
        if not data.get('advanced') or data.get('frame_count', 0)-data.get('queue_start_frame', 0) != frames:
            raise StoryStopped('input_timeline_not_advanced_atomically')
        return result

    def step(self, frames):
        return self.cmd(cmd='step_frames', count=frames)

    def close(self):
        self.raw.close()


def attack_profile(species, level, name):
    """Neutral-stage comparison, using the engine's Gen-1 critical formula.

    This is an expected power proxy, not an exact damage prediction: live
    defensive stats, status, screens and secondary effects can change outcomes.
    """
    mon, move = late.species_data(species), late.move_data(name)
    high = name in {'RazorLeaf', 'Slash', 'Crabhammer', 'KarateChop'}
    chance = min(255, mon['baseStats']['speed']//2 * (8 if high else 1))/256
    critical_multiplier = (4*level//5+2)/(2*level//5+2)
    stab = move['type'] in {mon['type1'], mon['type2']}
    expected = move['power'] * (move['accuracy']*255//100)/256
    expected *= (1.5 if stab else 1) * (1+chance*(critical_multiplier-1))
    return {'critical_probability_without_focus_energy': round(chance, 4),
            'neutral_expected_power': round(expected, 2),
            'same_type_bonus': stab}


def replacement_options(mon, learned):
    """Keep a stronger attack when a weaker same-type slot can be replaced."""
    if 'None' in mon['moves']:
        return [None]
    legal = [m for m in mon['moves'] if m not in {'Cut', 'Fly', 'Surf', 'Strength', 'Flash'}]
    incoming = late.move_data(learned)
    result = []
    for name in legal:
        old = late.move_data(name)
        score = attack_profile(mon['species'], mon['level'], name)['neutral_expected_power']
        upgraded = (incoming['type'] == old['type'] and
                    attack_profile(mon['species'], mon['level'], learned)['neutral_expected_power'] >= score)
        weaker = any(late.move_data(other)['type'] == old['type'] and
                     attack_profile(mon['species'], mon['level'], other)['neutral_expected_power'] < score
                     for other in legal if other != name)
        if old['power'] <= 0 or upgraded or not weaker:
            result.append(name)
    return result


def move_question(state, menu):
    """Compress only information that does not change the tactical judgment.

    PP availability/low reserve, HP bands and opposing types remain part of
    the cache key. A disabled/depleted move can never reuse a prior choice.
    """
    live = state['battle_live']
    player = late.species_data(live['player']['species'])
    enemy = late.species_data(live['enemy']['species'])
    choices, details = {}, {}
    for index, slot in enumerate(menu['moves']):
        if slot['pp'] <= 0 or slot['disabled']:
            continue
        move = late.move_data(slot['move'])
        if move['power'] <= 0:
            continue  # This skill's contract is a direct attack, not setup.
        typ = move['type']
        multiplier = math.prod(late.type_chart().get((typ, target), 1)
                               for target in {enemy['type1'], enemy['type2']})
        details[str(index)] = {
            **attack_profile(live['player']['species'], live['player'].get('level', 50), slot['move']),
            'move': slot['move'], 'power': move['power'],
            'effect': move['effect'],
            'accuracy': move['accuracy'], 'type': typ,
            'effectiveness': multiplier,
            'same_type_bonus': typ in {player['type1'], player['type2']},
            'pp_reserve': 'low' if slot['pp'] <= 3 else 'available',
            'heals_user': slot['move'] in {'MegaDrain', 'Absorb'},
            'high_critical_rate': slot['move'] in {'RazorLeaf', 'Slash', 'Crabhammer', 'KarateChop'},
        }
        details[str(index)]['effective_expected_power'] = round(details[str(index)]['neutral_expected_power'] * multiplier, 2)
        choices[str(index)] = slot['move']
    compact = {
        'player': live['player']['species'], 'enemy': live['enemy']['species'],
        'player_hp_band': 'hurt' if live['player']['hp'] < live['player']['max_hp'] * .65 else 'healthy',
        'enemy_hp_band': 'low' if live['enemy']['hp'] < live['enemy']['max_hp'] * .25 else 'healthy',
        'player_base_stats': player['baseStats'], 'enemy_base_stats': enemy['baseStats'],
        'player_level': live['player'].get('level'), 'enemy_level': live['enemy'].get('level'),
        'estimate_assumptions': 'Neutral stat stages and no Focus Energy; expected power includes accuracy, STAB and critical hits, but not attack/defense stats or secondary effects.',
        'moves': details,
    }
    return compact, choices


class JevGame(pt.Game):
    """Existing navigation/recovery skills with independently judged attacks."""
    def battle_party_target(self, state):
        live = state['battle_live']
        party = [{**base, **mon} for base, mon in zip(state['party'], live['player_party'])]
        pending = getattr(self, '_switch_target', None)
        if pending is not None and party[pending]['hp'] > 0:
            return pending
        signature = json.dumps([party, live['enemy']['species']], sort_keys=True)
        if getattr(self, '_party_signature', None) != signature:
            candidates = {str(i): json.dumps({'pokemon': mon,
                'usable_effective_attacks': effective_attacks(mon, live['enemy']['species'])})
                for i, mon in enumerate(party) if mon['hp'] > 0}
            self._party_target = int(self.judgments.choose('action', {'enemy': live['enemy']}, candidates,
                'Choose a conscious party member to battle this opponent. Compare level, remaining HP, '
                'usable effective attacks and type matchups. Even a weak remaining member can take a legal turn.'))
            self._party_signature = signature
        return self._party_target

    def learn_move(self, state):
        phase = state['battle_phase']
        name = re.search(r'move_id: (\w+)', phase)[1]
        index = int(re.search(r'party_index: (\d+)', phase)[1])
        mon = state['party'][index]
        signature = name, index, tuple(mon['moves'])
        if getattr(self, '_learn_signature', None) != signature:
            candidates = {'skip': 'Keep the current moves and decline the new move'}
            candidates.update({m: f'Learn {name}, replacing {m}' for m in replacement_options(mon, name) if m})
            self._learn_choice = self.judgments.choose('action', {
                'pokemon': mon, 'new_move': name,
                'move_data': {m: late.move_data(m) for m in [name, *mon['moves']] if m != 'None'}},
                candidates, 'Choose whether learning the new move improves this party member for exploration and battles. '
                'Preserve strong reliable attacks and useful coverage. Decline a weaker move if it would replace a better one.')
            self._learn_signature = signature
        if phase.startswith('LearnMoveAsk'):
            if self._learn_choice == 'skip':
                self.tap('b', 8)
            else:
                self.tap('up', 8)
                self.tap('a', 8)
        elif phase.startswith('LearnMoveGiveUpConfirm'):
            self.tap('up', 8)
            self.tap('a', 8)
        else:
            cursor = int(re.search(r'cursor: (\d+)', phase)[1])
            self.tap('a' if cursor == mon['moves'].index(self._learn_choice) else 'down', 8)

    def battle_recovery_plan(self, state):
        live = state['battle_live']
        # Field observations include status; battle party supplies current HP.
        party = [{**base, **mon} for base, mon in zip(state['party'], live['player_party'])]
        active = next(i for i, mon in enumerate(party) if mon['species'] == live['player']['species'])
        self._switch_target = None
        bag = {v['item']: v['qty'] for v in state['battle_inventory']}
        options = list(medicine_options(party, bag))
        candidates = {'fight': 'Attack this turn; preserve recovery supplies'}
        bindings = {}
        if not effective_attacks(party[active], live['enemy']['species']):
            for index, mon in enumerate(party):
                if index != active and mon['hp'] > 0 and effective_attacks(mon, live['enemy']['species']):
                    key = f'switch:{index}'
                    candidates[key] = json.dumps({'switch_to': mon, 'reason': 'The active battler has no usable attack that damages this opponent'})
                    bindings[key] = 'switch', index
        for item, index, details in options:
            mon = party[index]
            if ('pp' not in details['tags'] and mon['hp'] > 0
                    and mon['hp'] >= mon['max_hp'] * .65 and mon.get('status', 'None') == 'None'):
                continue
            key = f'item:{item}:{index}'
            candidates[key] = json.dumps(details)
            bindings[key] = item, index
        if not bindings:
            return None
        chosen = self.judgments.choose('action', {'battle': live}, candidates,
            'Choose attack, an offered switch, or one recovery item for this turn. Switching and items consume the turn and the enemy can attack. '
            'Keep the capable battler alive, cure disabling status, or revive a useful fainted teammate. '
            'Avoid healing loops when enemy damage exceeds recovery; use the strongest suitable medicine when needed.')
        return bindings.get(chosen)

    def remember_npcs(self, map_name, npcs):
        agent = getattr(self, 'judgments', None)
        if not hasattr(agent, 'maps'):
            return
        if not hasattr(self, 'stationary_npcs'):
            self.stationary_npcs = {}
        static = {n['textId'] for n in agent.maps.get(map_name, {}).get('npcs', [])
                  if n.get('movement') == 'Stationary'}
        # Stationary actors can disappear or move during story/battle scripts.
        # Their old cells are not a wandering patrol band: retaining a removed
        # blocker can reverse the route every time its map is re-entered.
        bands = getattr(self, 'observed_npcs', {}).get(map_name)
        if bands is not None:
            bands.difference_update(self.stationary_npcs.get(map_name, {}).values())
        self.stationary_npcs[map_name] = {n['text_id']: (n['x'], n['y']) for n in npcs
                                          if n.get('visible', True) and n['text_id'] in static}

    def navigation_barriers(self):
        blocked = {name: set(points) for name, points in super().navigation_barriers().items()}
        agent = getattr(self, 'judgments', None)
        index = getattr(agent, 'index', None)
        if index is None:
            return blocked
        facts = getattr(agent, 'navigation_facts', {})
        for name, npcs in getattr(self, 'stationary_npcs', {}).items():
            if name == getattr(self, '_prev_map', None):
                continue  # Current live positions override remembered ones.
            trainers = {npc['textId'] for npc in getattr(agent, 'maps', {}).get(name, {}).get('npcs', [])
                        if npc.get('isTrainer')}
            for text_id, position in npcs.items():
                if text_id in trainers:
                    # "Stationary" trainers still walk to engage, and map
                    # entry can reset their position. Re-observe them locally.
                    continue
                toggle = index.npc_toggles.get((name, text_id))
                observed = {**facts, 'object_visibility': {**facts.get('object_visibility', {}),
                                                          **({toggle[0]: True} if toggle else {})}}
                if toggle and evaluate({'Visible': list(toggle)}, observed) is False:
                    continue
                blocked.setdefault(name, set()).add(position)
        return blocked

    def st(self):
        state = super().st()
        if state.get('last_outside_map'):
            self.last_map = state['last_outside_map']
        agent = getattr(self, 'judgments', None)
        if agent is not None and hasattr(agent, 'visited'):
            agent.visited.add(state['map_name'])
            if hasattr(agent, 'cleared_terrain'):
                agent.invalidate_terrain(state)
        return state

    def cutscene(self, max_rounds=300):
        agent = self.judgments
        agent.settle(agent.active['target'] if agent.active else 'Continue exploring')
        return True

    def battle_loop(self, prefer='fight', max_iters=1200):
        before = self.st()
        entered = before['screen'] == 'battle'
        if entered and (before.get('battle_live') or {}).get('is_ghost'):
            if before.get('script_awaiting_battle') and hasattr(self.judgments, 'battle_requirements'):
                # Generic combat capability, not a route: the engine's ghost
                # rule disables attacks until the identification item is owned.
                self.judgments.battle_requirements['SILPH_SCOPE'] = {
                    'observed_map': before['map_name'], 'attack_blocked': 'unidentified_ghost',
                    'required_item': 'SILPH_SCOPE'}
                active = getattr(self.judgments, 'active', None)
                if isinstance(active, dict):
                    self.judgments.battle_requirements['SILPH_SCOPE']['blocked_goal'] = active['target']
            if prefer == 'fight':
                self.judgments.choose('action', {'battle': before['battle_live'],
                    'constraint': 'An unidentified ghost prevents all attacks; normal escape is allowed.'},
                    {'run': 'Escape and prepare or continue exploring'},
                    'Choose a legal operation that can end this currently unwinnable encounter.')
            prefer = 'run'
        super().battle_loop(prefer=prefer, max_iters=max_iters)
        state = self.st()
        if entered:
            self.battles_driven += 1
            if hasattr(self.judgments, 'observe_battle_result'):
                self.judgments.observe_battle_result(before, state)
            self.judgments.record('battle_skill_completed', prefer=prefer,
                                  result_phase=state['battle_phase'], party=state['party'],
                                  map=state['map_name'], frame=state['frame_count'])
        changed = before.get('party') != state.get('party') or before['map_name'] != state['map_name']
        if getattr(self, 'navigation_active', False) and (prefer == 'fight' or changed):
            raise NavigationPause('Battle ended; reassess travel, healing and preparation')

    def attach_judgments(self, model_client, *, model='jev-1.13.0', trace=None,
                         strategy_jev=True, action_jev=True, max_calls=2500,
                         wall_budget=7200, frame_budget=4000000):
        client = AgentClient.__new__(AgentClient)
        client.d = self.d
        self.judgments = DualStoryAgent(
            client, model_client, load_objectives(), model=model,
            strategy_jev=strategy_jev, action_jev=action_jev,
            max_calls=max_calls, wall_budget=wall_budget,
            frame_budget=frame_budget, trace=trace,
        )
        self.judgments.start_frame = self.st()['frame_count']
        self.d = ObservedProtocol(self.d, self.judgments.record,
                                  time.monotonic() + wall_budget)
        client.d = self.d
        self.move_cache = {}
        self.move_cache_hits = 0
        self.battles_driven = 0
        self.active_milestone = None

    def tap(self, btn, gap=pt.TAP_GAP):
        self.d.drive([None, btn, None], frames=gap + 3)

    def learn_machine(self, item, move, party_index, forget):
        """Follow observed machine boot, confirmation, target and replacement menus."""
        last_phase = None
        for _ in range(160):
            self.judgments.check_budget()
            state = self.st()
            learned = move in state['party'][party_index]['moves']
            menu = state.get('field_menu')
            if menu is None:
                if learned:
                    return
                late.open_start(self, 'Item')
                continue
            signature = (menu['kind'], menu.get('phase'))
            if signature != last_phase:
                self.judgments.record('machine_menu', item=item, learn=move, menu=menu)
                last_phase = signature
            if learned:
                self.tap('b', 12)
                continue
            phase = menu.get('phase', '')
            if menu['kind'] == 'bag':
                if phase == 'Browsing':
                    target = next(i for i, slot in enumerate(menu['items']) if slot['item'] == item)
                    self.tap('a' if menu['cursor'] == target else 'down', 12)
                elif phase.startswith(('ActionMenu', 'MachineTeach')):
                    cursor = int(re.search(r'cursor: (\d+)', phase)[1])
                    self.tap('a' if cursor == 0 else 'up', 12)
                elif phase.startswith('MachineBoot'):
                    self.tap('a', 12)
                else:
                    raise StoryStopped(f'unsupported_machine_bag_phase:{phase}')
            elif menu['kind'] == 'party':
                if phase.startswith('ChooseMove'):
                    if forget is None or forget not in menu['known_moves']:
                        raise StoryStopped('machine_replacement_not_authorized_by_action')
                    target = menu['known_moves'].index(forget)
                    cursor = int(re.search(r'cursor: (\d+)', phase)[1])
                    self.tap('a' if cursor == target else 'down', 12)
                elif phase == 'Browsing':
                    self.tap('a' if menu['cursor'] == party_index else 'down', 12)
                elif phase.startswith('ItemUseNotice'):
                    self.tap('a', 12)
                else:
                    raise StoryStopped(f'unsupported_machine_party_phase:{phase}')
            elif menu['kind'] == 'start':
                self.tap('a' if menu['items'][menu['cursor']] == 'Item' else 'down', 12)
            else:
                raise StoryStopped(f'unsupported_machine_menu:{menu}')
        raise StoryStopped('machine_menu_did_not_finish')

    def use_consumable(self, item, party_index):
        """Use one observed inventory item, then close its result menus."""
        initial = next(slot['qty'] for slot in self.d.cmd(cmd='get_bag')['data'] if slot['item'] == item)
        for _ in range(160):
            self.judgments.check_budget()
            state = self.st()
            remaining = next((slot['qty'] for slot in self.d.cmd(cmd='get_bag')['data'] if slot['item'] == item), 0)
            used = remaining < initial
            menu = state.get('field_menu')
            if menu is None:
                if used:
                    return
                late.open_start(self, 'Item')
                continue
            phase = menu.get('phase', '')
            if phase.startswith('LearnMove'):
                raise StoryStopped('consumable_pending_move_learning')
            if used:
                self.tap('a' if phase.startswith('ItemUseNotice') else 'b', 12)
            elif menu['kind'] == 'bag' and phase == 'Browsing':
                target = next(i for i, slot in enumerate(menu['items']) if slot['item'] == item)
                self.tap('a' if menu['cursor'] == target else 'down', 12)
            elif menu['kind'] == 'bag' and phase.startswith('ActionMenu'):
                cursor = int(re.search(r'cursor: (\d+)', phase)[1])
                self.tap('a' if cursor == 0 else 'up', 12)
            elif menu['kind'] == 'party' and phase == 'Browsing':
                self.tap('a' if menu['cursor'] == party_index else 'down', 12)
            elif menu['kind'] == 'start':
                self.tap('a' if menu['items'][menu['cursor']] == 'Item' else 'down', 12)
            else:
                raise StoryStopped(f'unsupported_consumable_menu:{menu}')
        raise StoryStopped('consumable_menu_did_not_finish')

    def _select_move(self):
        chosen = None
        for _ in range(24):
            state = self.st()
            if state['screen'] != 'battle' or state['battle_phase'] != 'MoveSelect':
                return
            menu = state.get('battle_moves')
            if not menu:
                self.step(4)
                continue
            compact, candidates = move_question(state, menu)
            if candidates and not any(m['effectiveness'] > 0 for m in compact['moves'].values()):
                # The PlayerMenu hook already considered conscious teammates.
                # Resolve an unavoidable loss through ordinary legal turns,
                # instead of abandoning the process in the middle of combat.
                chosen = next(iter(candidates))
                self.judgments.record('legal_turn_fallback', reason='all_remaining_attacks_immune',
                                      move=candidates[chosen], state=compact)
            if not candidates:
                # With only status PP remaining, repeatedly selecting an
                # exhausted attack never advances a turn. Finish the real
                # battle using a legal move, so an ordinary loss can recover.
                candidates = {str(i): slot['move'] for i, slot in enumerate(menu['moves'])
                              if slot['pp'] > 0 and not slot['disabled']}
                if not candidates:
                    return super()._select_move()  # Engine's all-PP-empty / Struggle path.
                chosen = next(iter(candidates))
                self.judgments.record('legal_turn_fallback', reason='no_usable_damaging_attack',
                                      move=candidates[chosen], state=compact)
            if chosen not in candidates:
                key = json.dumps(compact, sort_keys=True)
                if key in self.move_cache:
                    chosen = self.move_cache[key]
                    self.move_cache_hits += 1
                    self.judgments.record('action_cache_hit', decision='attack',
                                          choice=chosen, milestone=self.active_milestone)
                else:
                    chosen = self.judgments.choose(
                        'action', compact, candidates,
                        'Which usable attack gives the best progress on this turn? '
                        'This does not require a guaranteed battle victory. If the only available '
                        'attack deals nonzero damage, use it even when weak or nearly depleted. '
                        'Compare effective_expected_power (already includes accuracy, type effectiveness, '
                        'same-type bonus and critical probability), then physical/special stats and useful '
                        'secondary effects. Do not count those bonuses twice. Use draining attacks when healing matters. Avoid immunity and do not '
                        'spend a resisted low-PP attack when an effective alternative exists.')
                    self.move_cache[key] = chosen
                self.judgments.record('attack', milestone=self.active_milestone,
                                      choice=chosen, move=candidates[chosen], state=compact)
            target = int(chosen)
            current = menu['cursor']
            if current != target:
                count = len(menu['moves'])
                delta = (target - current) % count
                up = delta * 2 > count
                for _ in range(count - delta if up else delta):
                    self.tap('up' if up else 'down', 8)
                continue
            self.tap('a', 4)
            self.step(10)
            return
        raise StoryStopped('move_menu_did_not_settle')

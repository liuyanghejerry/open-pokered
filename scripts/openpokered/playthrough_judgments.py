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
from .story_rules import static_retreat_contract


ITEM_CATALOG = {item['id']: item for path in (late.DATA / 'data/items').glob('*.json')
                if (item := json.loads(path.read_text())).get('id')}
MEDICINES = {name: item for name, item in ITEM_CATALOG.items() if item.get('category') == 'Medicine'}


def capture_storage_full(state):
    """The native ball action rejects a full party plus a full current box."""
    if (state.get('battle_live') or {}).get('capture_blocked_reason') == 'storage_full':
        return True  # The engine's throw guard is authoritative during battle.
    counts = state.get('box_counts') or []
    index = state.get('current_box_index', 0)
    return (len(state.get('party', [])) >= 6 and 0 <= index < len(counts)
            and counts[index] >= 20)


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


BALLS = {name: item for name, item in ITEM_CATALOG.items() if 'ball' in item.get('tags', [])}


def capture_probability(ball, enemy, *, digits=4):
    """Exact success probability for a throw in the current battle state.

    This mirrors ``pokered-core/src/battle/capture.rs`` including Gen-I's
    rejection-sampled Rand1 windows.  Jev can compare the result directly
    instead of trying to reconstruct an unfamiliar, non-monotonic formula.
    """
    if ball == 'MasterBall':
        return 1.0
    threshold = {'GreatBall': 200, 'UltraBall': 150, 'SafariBall': 150}.get(ball, 255)
    factor = 8 if ball == 'GreatBall' else 12
    status = str(enemy.get('status', 'None')).lower()
    if status.startswith(('sleep', 'freeze')):
        status_subtract = 25
    elif status.startswith(('burn', 'paraly', 'poison')):
        status_subtract = 12
    else:
        status_subtract = 0
    quarter_hp = max(int(enemy['hp']) // 4, 1)
    w_raw = int(enemy['max_hp']) * 255 // factor // quarter_hp
    successes = 0
    for rand1 in range(threshold + 1):
        if status_subtract > rand1:
            successes += 256
        elif rand1 - status_subtract <= int(enemy['catch_rate']):
            successes += 256 if w_raw > 255 else min(w_raw, 255) + 1
    probability = successes / ((threshold + 1) * 256)
    return probability if digits is None else round(probability, digits)


def safari_ball_sequence(capture_probability, flee_probability, balls):
    """Ball-only encounter: success probability and expected balls spent.

    Capture is rolled first; only an unsuccessful throw can be followed by
    fleeing. No bait/rock, status changes, or ball replenishment is assumed.
    """
    reach_turn, success, spent = 1.0, 0.0, 0.0
    for _ in range(max(0, int(balls))):
        spent += reach_turn
        success += reach_turn * capture_probability
        reach_turn *= (1 - capture_probability) * (1 - flee_probability)
    return success, spent


def capture_species(enemy):
    """Native catch identity is separate from a mutable Transform battle form."""
    return enemy.get('capture_species') or enemy['species']


def capture_catch_rate(enemy):
    observed = enemy.get('capture_catch_rate')
    return observed if observed is not None else late.species_data(capture_species(enemy))['catchRate']


def ball_options(live, bag, owned_species=()):
    """Legal ball throws, grounded in public item and species data.

    Only wild encounters can be caught — the engine refuses a ball against a
    trainer's Pokémon — and a Safari encounter swaps the main menu for
    BALL/BAIT/ROCK/RUN with its own ball accounting, so it is not a bag throw.
    Whether the species is already registered is reported rather than filtered:
    whether a duplicate is worth a ball is the judgment under test.
    """
    if (not live.get('is_wild') or live.get('is_safari') or live.get('is_ghost')
            or live.get('capture_blocked_reason')):
        return
    enemy = live['enemy']
    catch_rate = capture_catch_rate(enemy)
    capture_state = {**enemy, 'catch_rate': catch_rate}
    owned = set(owned_species)
    for name, qty in bag.items():
        if qty <= 0 or name not in BALLS:
            continue
        yield name, None, {'ball': name, 'quantity': qty, 'enemy': enemy,
                           'catch_rate': catch_rate,
                           'capture_probability_now': capture_probability(name, capture_state),
                           'hp_percent': round(enemy['hp'] / max(1, enemy['max_hp']) * 100, 1),
                           'capture_species': capture_species(enemy),
                           'already_owned': capture_species(enemy) in owned}


def collection_capture_value(state, active):
    """Registration yield and explicitly requested possession are different."""
    species = capture_species(state['battle_live']['enemy'])
    owned = (state.get('pokedex') or {}).get('owned_species')
    registered = species in owned if isinstance(owned, list) else None
    context = active.get('context', {}) if isinstance(active, dict) else {}
    return {'capture_species': species, 'already_registered': registered,
            'registration_increment_if_caught': None if registered is None else int(not registered),
            'requested_held_copy': context.get('required_capture_species') == species,
            'active_subgoal': active.get('target') if isinstance(active, dict) else None,
            'balls_remaining': sum(row['qty'] for row in state.get('battle_inventory', [])
                                   if row['item'] in BALLS),
            'scope': 'A successful capture adds an individual, but an existing species registration is not counted again. A requested trade/evolution copy can be useful without adding a registration.'}


def capture_source_requested(state, judgments):
    """Collection intent does not disappear when the last ball is spent."""
    live = state['battle_live']
    if (not live.get('is_wild') or live.get('is_safari') or live.get('is_ghost')
            or live.get('capture_blocked_reason') not in (None, 'storage_full')):
        return False
    active = getattr(judgments, 'active', None)
    context = active.get('context', {}) if isinstance(active, dict) else {}
    species = capture_species(live['enemy'])
    return bool(context.get('required_capture_species') == species or
                (getattr(judgments, 'collects_dex', False) and species not in
                 (state.get('pokedex') or {}).get('owned_species', [])))


def capture_intent(state, judgments):
    if not capture_source_requested(state, judgments):
        return False
    usable_supply = not capture_storage_full(state) and any(
        slot['item'] in BALLS and slot['qty'] > 0
        for slot in state.get('battle_inventory', []))
    return usable_supply or capture_retreat(state, judgments) is not None


def capture_threat(enemy):
    """Natural wild moves from native creation rules, not hidden live PP/stages."""
    species = late.species_data(enemy['species'])
    moves = list(species['initialMoves'])
    for row in species.get('learnset', []):
        if row['level'] > enemy['level']:
            break
        move = row['moveId']
        if move == 'None' or move in moves:
            continue
        if 'None' in moves:
            moves[moves.index('None')] = move
        else:
            moves = moves[1:] + [move]
    details = [{'move': move, **late.move_data(move)} for move in moves if move != 'None']
    self_ko = [row['move'] for row in details if row['move'] in ('Selfdestruct', 'Explosion')]
    threat = {'inferred_natural_moves': details, 'self_knockout_moves': self_ko,
              'scope': 'Native creation learnset at observed wild level; not observed live moves, PP, stages or damage. Transform/Mimic can differ.'}
    if (self_ko and enemy.get('status') == 'None'
            and capture_species(enemy) == enemy['species']):
        # Wild pick_enemy_move_impl selects among nonempty positive-PP slots.
        # We do NOT observe those PP or future RNG: this is only a uniform-slot
        # reference, never an asserted probability of surviving the next turn.
        no_self_ko = 1 - len(self_ko) / len(details)
        threat['self_ko_selection_reference'] = {
            'self_ko_slots': len(self_ko), 'natural_move_slots': len(details),
            'no_self_ko_selection': [{'enemy_selections': turns, 'probability': no_self_ko ** turns}
                                     for turns in (1, 2)],
            'assumptions': 'Conditional reference: all inferred natural moves still have PP, unchanged moves, independent uniform slot selection, no forced move. These are move-selection probabilities, not survival or capture probabilities. Actual PP, Disable, Transform/Mimic, status prevention, speed order and damage can change the outcome; no future RNG is observed.'}
    return threat


def capture_turn_economy():
    """Native wild-battle action order, not a prediction of enemy execution."""
    return {
        'throw_now': {'earliest_ball_turn': 1, 'enemy_response_opportunities_before_throw': 0,
                      'order': 'Capture roll first; only a failed ball gives the enemy a response.'},
        'active_move_then_throw': {'earliest_ball_turn': 2,
                                   'enemy_response_opportunities_before_throw': 1,
                                   'order': 'Use the active Pokemon move this turn; throw next turn if the encounter remains.'},
        'switch_then_move_then_throw': {'earliest_ball_turn': 3,
                                        'enemy_response_opportunities_before_throw': 2,
                                        'order': 'Switch and allow an enemy response; use the incoming Pokemon move on a later turn; then throw.'},
        'scope': 'Earliest sequences if each planned action is legal and no extra recovery/setup is needed. Opportunities are not guaranteed attacks: speed/priority decides move order, and successful status can prevent a response. Switching itself neither applies status nor makes a capture attempt. Conditional capture odds after status exclude the chance of losing the target or support before that status and throw.'}


def capture_retreat(state, judgments):
    if not state.get('script_awaiting_battle'):
        return None  # Never infer static retryability from a travel goal alone.
    rules = getattr(getattr(judgments, 'index', None), 'rules', [])
    if not isinstance(rules, list):
        return None
    species = capture_species(state['battle_live']['enemy']).replace('_', '').upper()
    sources = [rule for rule in rules if rule.map == state['map_name']
               and rule.effect[0] == 'battle' and isinstance(rule.effect[1], str)
               and rule.effect[1].replace('_', '').upper() == species]
    contracts = [static_retreat_contract(rule, rules) for rule in sources]
    if contracts and all(row['menu_run_preserves_source'] for row in contracts):
        return {'contracts': contracts, 'escape_success_not_guaranteed': True,
                'purpose': 'Menu RUN to preserve this retryable source, recover and prepare; unlike a knockout, successful RUN does not consume it.'}
    return None


def capture_turn_key(state):
    live = state['battle_live']
    return json.dumps([live['enemy'], live['player'], live.get('player_party'),
                       state.get('battle_inventory')], sort_keys=True)


def capture_status_options(mon, enemy, bag):
    """Public PP and conditional capture benefit; not a guaranteed status hit."""
    if str(enemy.get('status', 'None')).lower() != 'none':
        return []
    result = []
    for index, (name, pp) in enumerate(zip(mon.get('moves', []), mon.get('pp', []))):
        if name == 'None' or pp <= 0:
            continue
        move = late.move_data(name)
        status = {'SleepEffect': 'Sleep(2)', 'ParalyzeEffect': 'Paralysis'}.get(move.get('effect'))
        if move.get('power') != 0 or not status:
            continue
        enemy_data = late.species_data(enemy['species'])
        # Gen-I SleepEffect has no type immunity; ParalyzeEffect checks Ground
        # only when the move is Electric. Do not apply the damage chart (e.g.
        # Sing/Glare work on Ghost, Stun Spore works on Grass). "1" denotes
        # type compatibility, never certainty of landing or a damage multiplier.
        enemy_types = {enemy_data['type1'], enemy_data['type2']}
        if move['effect'] == 'ParalyzeEffect' and move['type'] == 'Electric' and 'Ground' in enemy_types:
            continue
        projected = {ball: capture_probability(ball, {**enemy, 'status': status,
                         'catch_rate': capture_catch_rate(enemy)})
                     for ball, quantity in bag.items() if ball in BALLS and quantity > 0}
        result.append({'slot': index, 'move': name, 'pp': pp, 'power': 0,
                       'accuracy': move['accuracy'], 'effectiveness': 1,
                       'effect': move['effect'], 'status_if_successful': status,
                       'residual_damage_risk': False,
                       'capture_probability_if_status_lands': projected})
    return result


def safari_action_options(live, owned_species=()):
    """Exact current/immediate Safari trade-offs from the exposed engine state."""
    safari = live.get('safari') or {}
    enemy = live.get('enemy') or {}
    if not live.get('is_safari') or not safari or not enemy:
        return {}
    owned = enemy.get('species') in set(owned_species)

    def flee_probability(bait_factor, escape_factor):
        # Safari upkeep happens before the flee roll.  A factor of one expires
        # on this turn, so it no longer modifies that roll.
        bait_factor, escape_factor = int(bait_factor), int(escape_factor)
        if bait_factor:
            bait_factor -= 1
        elif escape_factor:
            escape_factor -= 1
        speed = int(safari.get('enemy_speed', 0)) & 0xff
        if speed > 127:
            return 1.0
        threshold = speed * 2
        if bait_factor:
            threshold >>= 2
        if escape_factor:
            threshold = min(255, threshold * 2)
        return round(threshold / 256, 4)

    def catch_probability(rate):
        state = {**enemy, 'catch_rate': max(0, min(255, rate))}
        return capture_probability('SafariBall', state)

    rate = int(safari['catch_rate'])
    base_rate = int(safari['base_catch_rate'])
    balls = int(safari['balls'])
    bait_flee = round(sum(flee_probability(amount, 0) for amount in range(1, 6)) / 5, 4)
    rock_flee = round(sum(flee_probability(0, amount) for amount in range(1, 6)) / 5, 4)
    bait_catch = catch_probability(rate >> 1)
    rocked_catch = catch_probability(min(255, rate * 2))
    # A one-turn anger factor is consumed immediately and restores the base
    # catch rate; the other four equally likely durations preserve the boost.
    rock_catch = round((catch_probability(base_rate) + 4 * rocked_catch) / 5, 4)
    options = {
        'run': {'effect': 'End this encounter without spending a Safari Ball',
                'already_owned': owned},
    }
    if balls:
        options['ball'] = {'effect': 'Spend one Safari Ball and attempt capture now',
            'balls_remaining': balls, 'already_owned': owned,
            'capture_probability_now': catch_probability(rate),
            'flee_probability_if_not_caught': flee_probability(
                safari.get('bait_factor', 0), safari.get('escape_factor', 0))}
        options['bait'] = {'effect': 'Halve catch rate, suppress anger, and reduce flee risk for 1-5 turns',
            'duration_turns_uniform': [1, 5],
            'projected_catch_probability': bait_catch,
            'projected_flee_probability_next_turn': bait_flee}
        options['rock'] = {'effect': 'Double catch rate, suppress bait, and increase flee risk for 1-5 turns',
            'duration_turns_uniform': [1, 5],
            'projected_catch_probability': rock_catch,
            'projected_flee_probability_next_turn': rock_flee}
    return options


def effective_attacks(mon, enemy):
    types = {late.species_data(enemy)[key] for key in ('type1', 'type2')}
    return [name for name, pp in zip(mon['moves'], mon['pp']) if name != 'None' and pp > 0
            and late.move_data(name)['power'] > 0 and all(
                late.type_chart().get((late.move_data(name)['type'], typ), 1) > 0 for typ in types)]


# Playstyle biases, not goals: each one steers both the instructions the model
# reads and the candidate data the code offers.
PREFERENCE_INSTRUCTIONS = {
    'level': 'Preference: level suppression — prefer earning experience and out-levelling the next '
             'opponent before challenging it. Training is progress even when the current opponent is '
             'beatable.',
    'type': 'Preference: type suppression — prefer the attacker and move whose type is super-effective '
            'against this opponent, and prefer acquiring or switching to a member that covers its type '
            'over raw power.',
    'tactic': 'Preference: tactical suppression — prefer status, stat-modifying and support moves, and '
              'carried items, over raw-damage attacks; aim to disable or outlast the opponent.',
}


def preference_suffix(judgments):
    """Bias text for a question this game asks; empty under the default preference."""
    bias = PREFERENCE_INSTRUCTIONS.get(getattr(judgments, 'preference', 'none'), '')
    return f' {bias}' if bias else ''


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
    ADVANCING = {
        'wait_until', 'skip_dialogue', 'press', 'press_sequence',
        'press_timeline', 'step_frames', 'move_to', 'interact',
        'interact_with', 'travel_to',
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
        reply = self.raw.cmd(**kwargs)
        if name in self.ADVANCING:
            data = reply.get('data')
            frame = data.get('frame_count') if isinstance(data, dict) else None
            if frame is None and isinstance(data, dict) and isinstance(data.get('state'), dict):
                frame = data['state'].get('frame_count')
            if not isinstance(frame, int) or isinstance(frame, bool) or frame < 0:
                frame = None
            self.record('native_input', request=kwargs, ok=reply.get('ok') is True, frame=frame)
        return reply

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


def capture_move_question(state, menu):
    compact, choices = move_question(state, menu)
    live = state['battle_live']
    for index, slot in enumerate(menu['moves']):
        if str(index) in compact['moves']:
            compact['moves'][str(index)]['direct_hit_preview'] = slot.get('direct_hit_preview')
    compact['direct_hit_preview_scope'] = (
        'Native formula ranges for one hit if it lands with current combat stats, stages, '
        'badge boosts, burn, screens and types unchanged. Includes critical damage. '
        'critical_threshold / 256 is the current critical probability. '
        'Not a whole-turn safety guarantee: excludes opponent action, move failure, '
        'secondary and residual damage. Null means unsupported or unavailable, NOT zero damage.')
    bag = {slot['item']: slot['qty'] for slot in state.get('battle_inventory', [])}
    mon = {'moves': [slot['move'] for slot in menu['moves']],
           'pp': [0 if slot['disabled'] else slot['pp'] for slot in menu['moves']]}
    for option in capture_status_options(mon, live['enemy'], bag):
        key = str(option['slot'])
        compact['moves'][key] = option
        choices[key] = option['move']
    compact.update(goal='capture_without_knocking_out', enemy_state=live['enemy'],
                   available_balls=list(ball_options(live, bag)),
                   capture_threat=capture_threat(live['enemy']))
    return compact, choices


class JevGame(pt.Game):
    """Existing navigation/recovery skills with independently judged attacks."""
    def battle_party_target(self, state):
        live = state['battle_live']
        party = [{**base, **mon} for base, mon in zip(state['party'], live['player_party'])]
        pending = getattr(self, '_switch_target', None)
        if pending is not None and party[pending]['hp'] > 0:
            return pending
        signature = json.dumps([party, live['enemy']['species'], state.get('battle_phase')], sort_keys=True)
        if getattr(self, '_party_signature', None) != signature:
            candidates = {str(i): json.dumps({'pokemon': mon,
                'usable_effective_attacks': effective_attacks(mon, live['enemy']['species'])})
                for i, mon in enumerate(party) if mon['hp'] > 0}
            self._party_target = int(self.judgments.choose('action', {
                'enemy': live['enemy'], 'battle': live, 'battle_phase': state.get('battle_phase')}, candidates,
                'Choose a conscious party member to battle this opponent. Compare level, remaining HP, '
                'usable effective attacks and type matchups. Even a weak remaining member can take a legal turn.'))
            self._party_signature = signature
        return self._party_target

    def safari_battle_action(self, state):
        if capture_storage_full(state):
            return 'run'
        live = state.get('battle_live') or {}
        owned = (state.get('pokedex') or {}).get('owned_species', [])
        options = safari_action_options(live, owned)
        if not options:
            return 'run'
        enemy = (live.get('enemy') or {}).get('species')
        objective = getattr(self.judgments, 'active', None)
        context = objective.get('context', {}) if isinstance(objective, dict) else {}
        required_source = context.get('required_capture_species') == enemy
        if (enemy in set(owned) and not required_source) or 'ball' not in options:
            return 'run'
        candidates = {name: json.dumps(details) for name, details in options.items()}
        return self.judgments.choose('action', {'battle': live}, candidates,
            'Choose one legal Safari action for this turn. The opponent is either unregistered or required '
            'as another held copy for the selected trade or evolution, so the goal is capture, '
            'not battle victory. Compare the supplied exact current capture probability, remaining Safari Balls, '
            'and current/projected flee probability. BALL is the direct baseline; use BAIT or ROCK only when its '
            'risk-adjusted future capture opportunity is better, and RUN only when capture is no longer viable.')

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
        objective = getattr(self.judgments, 'active', None)
        context = objective.get('context', {}) if isinstance(objective, dict) else {}
        switch_training = ((context.get('acquisition_method') == 'evolution'
                            or context.get('capture_support_training'))
                           and context.get('trigger') == 'level'
                           and party[active]['species'] == context.get('from_species'))
        capturing = capture_intent(state, self.judgments) and not capture_storage_full(state)
        seeking_source = capture_source_requested(state, self.judgments)
        retreat = capture_retreat(state, self.judgments) if seeking_source else None
        capturing = capturing or retreat is not None
        if retreat:
            candidates['run'] = json.dumps(retreat)
            bindings['run'] = 'run', None
        if capturing:
            candidates['fight'] = json.dumps({
                'active_party_index': active, 'active_pokemon': party[active],
                'usable_effective_attacks': effective_attacks(party[active], live['enemy']['species']),
                'capture_status_options': capture_status_options(party[active], live['enemy'], bag),
                'reason': 'Prepare capture using the current active Pokemon without switching. Open FIGHT and select its move; switching to another teammate does not use that teammate\'s move on the switching turn. A knockout loses this encounter, so do not select FIGHT just to win.',
                'availability_scope': 'Observed moves and PP, known target status and type immunities. The real move menu still checks disabled moves; status success and surviving to act are not guaranteed.'})
        if switch_training or capturing or not effective_attacks(party[active], live['enemy']['species']):
            for index, mon in enumerate(party):
                statuses = capture_status_options(mon, live['enemy'], bag) if capturing else []
                attacks = effective_attacks(mon, live['enemy']['species'])
                if index != active and mon['hp'] > 0 and (attacks or statuses):
                    if switch_training and mon['level'] <= party[active]['level']:
                        continue
                    key = f'switch:{index}'
                    candidates[key] = json.dumps({'switch_to': mon,
                        'usable_effective_attacks': attacks,
                        'capture_status_options': statuses,
                        'reason': ('The experience trainee has entered battle and can share experience if it remains conscious; a stronger teammate can finish efficiently'
                                   if switch_training else
                                   'Prepare capture using non-damaging status or weaker attacks; switching consumes a turn and does not guarantee survival'
                                   if capturing else
                                   'The active battler has no usable attack that damages this opponent')})
                    bindings[key] = 'switch', index
        for item, index, details in options:
            mon = party[index]
            if ('pp' not in details['tags'] and mon['hp'] > 0
                    and mon['hp'] >= mon['max_hp'] * .65 and mon.get('status', 'None') == 'None'):
                continue
            key = f'item:{item}:{index}'
            candidates[key] = json.dumps(details)
            bindings[key] = item, index
        owned = (state.get('pokedex') or {}).get('owned_species', [])
        balls = [] if capture_storage_full(state) else list(ball_options(live, bag, owned))
        required_source = context.get('required_capture_species') == capture_species(live['enemy'])
        for ball, _target, details in balls:
            key = f'ball:{ball}'
            if required_source:
                details = {**details, 'required_as_trade_or_evolution_source': True}
            candidates[key] = json.dumps(details)
            bindings[key] = ball, None
        if retreat and not balls:
            # No preparation can result in a catch this attempt. Keep recovery
            # and RUN available, but never offer victory as a capture fallback.
            candidates.pop('fight', None)
        if (capturing and bindings and getattr(self, '_capture_declined_fight', None)
                == capture_turn_key(state)):
            candidates.pop('fight', None)
        if (not effective_attacks(party[active], live['enemy']['species'])
                and not (capturing and capture_status_options(party[active], live['enemy'], bag))
                and any(key.startswith('switch:') for key in bindings)):
            # FIGHT only offers damaging moves to the attack judge. A status-
            # only fallback cannot finish this opponent, so do not advertise
            # it as an attack while a conscious, effective finisher exists.
            candidates.pop('fight', None)
        if not bindings:
            return None
        instruction = ('Choose attack, an offered switch, one recovery item, or one ball for this turn. Switching, items and balls consume the turn and the enemy can attack. '
            'Keep the capable battler alive, cure disabling status, or revive a useful fainted teammate. '
            'Avoid healing loops when enemy damage exceeds recovery; use the strongest suitable medicine when needed. '
            'A ball can only be thrown at a wild Pokémon: weigh the enemy species, its remaining HP, its catch rate, which ball you would spend, '
            'and whether that species is already registered, against simply attacking it.')
        if switch_training:
            instruction += (' The current goal is to train the active trainee through experience. It has already '
                'participated and receives a share of victory experience after switching out if it remains conscious. '
                'Compare defeating this opponent directly with switching to a stronger teammate: prefer switching '
                'when weak or resisted attacks would consume many turns or PP, or risk fainting. Do not spend '
                'recovery supplies just to keep an inefficient trainee attacking when a healthy finisher is available.')
        if required_source and balls:
            instruction += (' This opponent is already registered but no longer held, and another copy is required '
                'for the selected NPC trade or evolution. Capture it with a ball; defeating it does not satisfy '
                'that requirement. Registration and possession are different facts.')
        if getattr(self.judgments, 'collects_dex', False) and any(not d['already_owned'] for _, _, d in balls):
            # Framed purely around turn economy, defeating an unregistered
            # species reads as the safe play and the run collects nothing.
            instruction += (' This run exists to register species that are not in the Pokédex yet, not to win '
                'encounters: this opponent is unregistered, so defeating it spends the encounter without '
                'collecting anything. Compare throwing now with using FIGHT to apply a safe capture status '
                'or carefully weaken it, or switching to a teammate capable of these preparations. '
                'Switching and setup cost turns and expose the party to damage. Do not blindly spend a '
                'limited ball supply at full HP when viable preparation substantially raises capture odds; '
                'do not knock out the target or use residual poison/burn damage to prepare it.')
        if capturing:
            instruction += (' FIGHT describes the current active Pokemon and its capture_status_options, '
                'not just an attack. Compare using those existing tools now with the offered switches: '
                'switching spends this turn and does not apply the incoming teammate\'s status move. '
                'Do not switch back and forth just to obtain a tool the active teammate already has.')
            instruction += (' Use enemy.capture_species as the species that a successful ball registers, '
                'and enemy.species for the current combat form and type matchups. Transform can copy an '
                'already registered form without changing an unregistered capture target into a duplicate.')
            if retreat and not balls:
                instruction += (' No usable capture ball/storage capacity remains. Preserve this '
                    'retryable source through RUN; replenish balls or free the current box before retrying. '
                    'Do not attack or weaken it: capture is impossible this attempt.')
            instruction += (' Examine capture_threat: a self-knockout move can spend the target before setup succeeds. '
                'A low-level support may faint before acting or on the switch turn. Do not keep switching '
                'away from an immune/resistant survivor to fragile teammates just because they can apply status. '
                'If RUN is offered, the script has been checked to preserve the source after successful menu escape: '
                'compare retreat and proper preparation with spending this limited supply or risking a knockout. '
                'When safe preparation is no longer available, throwing a ball is capture progress; defeating '
                'the target is not a fallback success.')
            instruction += (' Compare capture_turn_economy with the immediate ball odds. A higher '
                'capture_probability_if_status_lands is conditional on reaching that prepared state, '
                'not the chance of capturing from here. Weigh losing the target during setup against '
                'the benefit: a ball rolls before any enemy response, while switching grants a response '
                'before the incoming teammate can use its move. Self-knockout selection references '
                'are conditional illustrations, not known live odds or guarantees of survival.')
        judgment_state = {'battle': live}
        if balls and getattr(self.judgments, 'collects_dex', False):
            value = collection_capture_value(state, objective)
            judgment_state['collection_capture_value'] = value
            if value['registration_increment_if_caught'] == 0 and not value['requested_held_copy']:
                instruction += (' This capture would register zero new species, and the selected goal '
                    'does not request this opponent as a held trade/evolution copy. Compare the concrete '
                    'utility of another individual with reserving these finite balls for unregistered '
                    'targets. Easy capture odds alone are not Pokédex progress. Duplicate capture remains '
                    'an option when its specific usefulness justifies the supply cost.')
        if capturing:
            judgment_state['capture_threat'] = capture_threat(live['enemy'])
            judgment_state['capture_turn_economy'] = capture_turn_economy()
            judgment_state['inventory_failure_at_current_state'] = math.prod(
                (1-details['capture_probability_now']) ** bag[ball]
                for ball, _, details in balls)
            judgment_state['inventory_failure_assumptions'] = 'All currently held balls thrown at fixed observed HP/status with independent rolls; excludes remaining party survival, enemy healing/status expiry and future damage.'
        chosen = self.judgments.choose('action', judgment_state, candidates,
                                       instruction + preference_suffix(self.judgments))
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
            if hasattr(self.judgments, 'battle_requirements'):
                # Generic combat capability, not a route: the engine's ghost
                # rule disables attacks until the identification item is owned.
                prior = self.judgments.battle_requirements.get('SILPH_SCOPE', {})
                requirement = {**prior,
                    'observed_map': before['map_name'], 'attack_blocked': 'unidentified_ghost',
                    'required_item': 'SILPH_SCOPE'}
                if not before.get('script_awaiting_battle'):
                    # Wild ghosts cannot be captured either. Remember only
                    # observed areas; unseen maps must not inherit a failure.
                    requirement['capture_blocked_maps'] = sorted(
                        set(prior.get('capture_blocked_maps', [])) | {before['map_name']})
                self.judgments.battle_requirements['SILPH_SCOPE'] = requirement
                active = getattr(self.judgments, 'active', None)
                if isinstance(active, dict) and before.get('script_awaiting_battle'):
                    requirement['blocked_goal'] = active['target']
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
        # A scripted battle can leave deferred dialogue, pushback, flags or
        # transport even after a run with unchanged party/map. Discard the
        # old direction string and let the caller settle/reobserve the script;
        # ordinary wild escapes still allow unchanged navigation to continue.
        scripted = entered and before.get('script_awaiting_battle') is True
        if getattr(self, 'navigation_active', False) and (prefer == 'fight' or changed or scripted):
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

    def use_field_item(self, item):
        """Select a reusable field item through START -> ITEM -> USE."""
        for _ in range(120):
            self.judgments.check_budget()
            state = self.st()
            menu = state.get('field_menu')
            if menu is None:
                if state['screen'] == 'overworld' and state.get('dialogue_state'):
                    return
                late.open_start(self, 'Item')
                continue
            phase = menu.get('phase', '')
            if menu['kind'] == 'bag' and phase == 'Browsing':
                target = next(i for i, slot in enumerate(menu['items']) if slot['item'] == item)
                self.tap('a' if menu['cursor'] == target else 'down', 12)
            elif menu['kind'] == 'bag' and phase.startswith('ActionMenu'):
                cursor = int(re.search(r'cursor: (\d+)', phase)[1])
                self.tap('a' if cursor == 0 else 'up', 12)
            elif menu['kind'] == 'start':
                self.tap('a' if menu['items'][menu['cursor']] == 'Item' else 'down', 12)
            else:
                raise StoryStopped(f'unsupported_field_item_menu:{menu}')
        raise StoryStopped('field_item_menu_did_not_finish')

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
            capturing = capture_intent(state, self.judgments)
            if (capturing and capture_retreat(state, self.judgments) is not None
                    and (capture_storage_full(state) or not any(
                        slot['item'] in BALLS and slot['qty'] > 0
                        for slot in state.get('battle_inventory', [])))):
                self.tap('b', 4)
                self.step(10)
                self.judgments.record('capture_menu_cancelled',
                                      reason='retryable_source_without_capture_capacity')
                return
            compact, candidates = (capture_move_question(state, menu) if capturing
                                   else move_question(state, menu))
            if capturing and not candidates:
                # A status-only support may have just landed sleep. Return to
                # PlayerMenu for a ball rather than repeat a useless status.
                self.tap('b', 4)
                self.step(10)
                return
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
                    instruction = ('Which usable move best prepares capture without knocking out the target? '
                        'Sleep or paralysis can improve the supplied capture probability without damaging '
                        'the target. Their projected odds are conditional on the status actually landing. '
                        'Compare accuracy, PP, target status and remaining balls. For damaging moves, '
                        'use direct_hit_preview when present: compare both normal and critical damage '
                        'ranges with current target HP, under direct_hit_preview_scope. A hit whose '
                        'maximum is below target HP cannot directly knock it out at unchanged stats, '
                        'but secondary/residual damage and the opponent acting first still matter. '
                        'Without a preview, levels, base stats and power are only proxies, not damage '
                        'guarantees. Prefer safe non-damaging preparation or a suitably weak attack '
                        'over a knockout; do not repeatedly try status that is already present.' if capturing else
                        'Which usable attack gives the best progress on this turn? '
                        'This does not require a guaranteed battle victory. If the only available '
                        'attack deals nonzero damage, use it even when weak or nearly depleted. '
                        'Compare effective_expected_power (already includes accuracy, type effectiveness, '
                        'same-type bonus and critical probability), then physical/special stats and useful '
                        'secondary effects. Do not count those bonuses twice. Use draining attacks when healing matters. Avoid immunity and do not '
                        'spend a resisted low-PP attack when an effective alternative exists.'
                        )
                    if capturing:
                        candidates['back'] = 'Cancel this move menu without spending a turn; return to compare balls, switches or verified retreat when no listed move safely prepares capture.'
                        instruction += ' Choose back when no offered move is suitable; do not attack merely because FIGHT was opened.'
                    try:
                        chosen = self.judgments.choose('action', compact, candidates,
                                                       instruction + preference_suffix(self.judgments))
                    except StoryStopped as error:
                        if not capturing or str(error) != 'action:no_selection':
                            raise
                        chosen = 'back'
                        self.judgments.record('capture_move_abstention', state=compact)
                    self.move_cache[key] = chosen
                if chosen == 'back':
                    self._capture_declined_fight = capture_turn_key(state)
                    self.judgments.record('capture_menu_cancelled', state=compact)
                    self.tap('b', 4)
                    self.step(10)
                    return
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

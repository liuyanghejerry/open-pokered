"""Observed registration producers, deliberately separate from planning intent.

This is a bounded telemetry collector, not a legality/completion gate. It uses
existing native observations only; it never issues commands or consults Jev.
Unwitnessed gifts, trades, encounter modes and old saves remain unknown.
"""
from collections import Counter
from copy import deepcopy

from .collection_planner import NPC_TRADES


# Native TradeAnimPhase values before apply_npc_trade mutates the party.
# Done is deliberately excluded: by then the outgoing member may be gone.
TRADE_PHASES = frozenset(('SlideInGiveMon', 'ShowGiveMon', 'GiveMonPoof',
    'GiveMonBallDrop', 'BallEnterCable', 'SlideOut', 'TextWentTo',
    'TextForSends', 'TextFarewell', 'SlideBack', 'ReceiveBallTilt',
    'ReceiveMonPoof', 'ShowReceiveMon', 'TextTakeCare', 'SlideTextBoxOff'))


def _name(value):
    return str(value).replace('_', '').upper()


def _snapshot(state):
    if not isinstance(state, dict):
        return None
    dex = state.get('pokedex', state.get('dex')) or {}
    party, stored = state.get('party'), state.get('stored_pokemon')
    owned = dex.get('owned_species')
    if (not isinstance(owned, list) or any(not isinstance(name, str) for name in owned)
            or len(set(owned)) != len(owned) or dex.get('owned') != len(owned)
            or not isinstance(party, list) or not isinstance(stored, list)):
        return None
    roster = party + stored
    if any(not isinstance(mon, dict) or not isinstance(mon.get('species'), str)
           or type(mon.get('level')) is not int or not 1 <= mon['level'] <= 100 for mon in roster):
        return None
    return {'owned': set(owned), 'roster': deepcopy(roster),
            'party': deepcopy(party), 'flags': deepcopy(state.get('flags', {})),
            'frame': state.get('frame_count', state.get('frame')),
            'map': state.get('map_name', state.get('map')),
            'bag': deepcopy(state.get('bag', {}))}


def _inventory(snapshot):
    return Counter(_name(mon['species']) for mon in snapshot['roster'])


def _bag(state):
    rows = state.get('battle_inventory')
    if not isinstance(rows, list) or any(not isinstance(row, dict)
            or not isinstance(row.get('item'), str) or type(row.get('qty')) is not int for row in rows):
        return {}
    return {_name(row['item']): row['qty'] for row in rows}


def native_capture_registration(before, after):
    """Match one actual wild capture, not an active capture/evolution target.

Generic wild_capture deliberately does not assert grass/water/rod/static mode.
Safari requires the native Safari battle marker and an observed ball cost.
"""
    old, new = _snapshot(before), _snapshot(after)
    if old is None or new is None:
        return {}
    old_live, new_live = before.get('battle_live') or {}, after.get('battle_live') or {}
    old_enemy, new_enemy = old_live.get('enemy') or {}, new_live.get('enemy') or {}
    gained = new['owned'] - old['owned']
    if (len(gained) != 1 or not old['owned'] <= new['owned']
            or old_live.get('is_wild') is not True or new_live.get('is_wild') is not True
            or old_live.get('is_ghost') is not False or old_live.get('capture_blocked_reason') is not None
            or old['map'] != new['map'] or old['map'] is None):
        return {}
    species = next(iter(gained))
    if (any(_name(enemy.get('capture_species')) != _name(species) for enemy in (old_enemy, new_enemy))
            or type(old_enemy.get('level')) is not int or old_enemy['level'] != new_enemy.get('level')
            or type(new_enemy.get('hp')) is not int or new_enemy['hp'] <= 0
            or _inventory(new) != _inventory(old) + Counter({_name(species): 1})):
        return {}
    caught = [mon for mon in new['roster'] if _name(mon['species']) == _name(species)]
    if len(caught) != 1 or caught[0]['level'] != new_enemy['level'] or caught[0].get('hp') != new_enemy['hp']:
        return {}
    old_bag, new_bag = _bag(before), _bag(after)
    costs = {item: old_bag.get(item, 0) - new_bag.get(item, 0)
             for item in ('POKEBALL', 'GREATBALL', 'ULTRABALL', 'MASTERBALL')}
    safari = old_live.get('is_safari') is True and new_live.get('is_safari') is True
    if safari:
        old_safari, new_safari = old_live.get('safari') or {}, new_live.get('safari') or {}
        old_balls, new_balls = old_safari.get('balls'), new_safari.get('balls')
        if type(old_balls) is not int or type(new_balls) is not int or not old_balls > new_balls >= 0:
            return {}
        costs = {'SAFARIBALL': old_balls - new_balls}
    elif any(cost < 0 for cost in costs.values()) or not any(cost > 0 for cost in costs.values()):
        return {}
    return {species: {'method': 'safari' if safari else 'wild_capture',
        'basis': 'native_battle_owned_bit_and_one_added_roster_member',
        'before_frame': old['frame'], 'after_frame': new['frame'], 'map': old['map'],
        'enemy_before': deepcopy(old_enemy), 'caught': deepcopy(caught[0]),
        'ball_costs': {item: cost for item, cost in costs.items() if cost},
        'scope': 'Observed native capture; not original-route legality or exact non-Safari encounter-mode certification.'}}


def _evolution_registration(old, new, graph, *, native_level_phase=False):
    if old is None or new is None:
        return {}
    removed, added = _inventory(old) - _inventory(new), _inventory(new) - _inventory(old)
    if sum(removed.values()) != 1 or sum(added.values()) != 1:
        return {}
    source, target = next(iter(removed)), next(iter(added))
    before = [mon for mon in old['roster'] if _name(mon['species']) == source]
    after = [mon for mon in new['roster'] if _name(mon['species']) == target]
    gained = new['owned'] - old['owned']
    if (len(before) != 1 or len(after) != 1 or len(gained) != 1
            or not old['owned'] <= new['owned'] or _name(next(iter(gained))) != target):
        return {}
    species = next(iter(gained))
    for edge in graph.get(species, []):
        if edge.get('method') != 'evolution' or _name(edge.get('from_species')) != source:
            continue
        if edge.get('trigger') == 'level':
            threshold = edge.get('level')
            if (not native_level_phase or type(threshold) is not int
                    or before[0]['level'] != after[0]['level'] or after[0]['level'] < threshold):
                continue
            basis = 'native_evolving_phase_and_one_roster_species_replacement'
        elif edge.get('trigger') == 'item':
            item = _name(edge.get('item'))
            old_qty, new_qty = old['bag'].get(item, 0), new['bag'].get(item, 0)
            if (before[0]['level'] != after[0]['level'] or type(old_qty) is not int
                    or type(new_qty) is not int or old_qty - new_qty != 1):
                continue
            basis = 'one_roster_species_replacement_and_actual_evolution_item_cost'
        else:
            continue
        return {species: {'method': 'evolution', 'basis': basis,
            'before_frame': old['frame'], 'after_frame': new['frame'],
            'before': deepcopy(before[0]), 'after': deepcopy(after[0]), 'edge': deepcopy(edge),
            'scope': 'Observed native replacement and trigger evidence; not individual-ID or full original-fidelity certification.'}}
    return {}


def _npc_trade_registration(previous, phase, current):
    """A timed native phase, actual exchange and newly set NPC receipt flag.

    get_state does not expose flags. Use the existing pre-operation facts for
    the flag baseline, never a new RPC or the selected trade goal. Ambiguous
    duplicate sources deliberately remain unknown.
    """
    if previous is None or phase is None or current is None:
        return {}
    if (previous['owned'] != phase['owned'] or not previous['owned'] <= current['owned']
            or len(current['owned'] - previous['owned']) != 1
            or previous['map'] != phase['map'] or phase['map'] != current['map']
            or any(type(s['frame']) is not int for s in (previous, phase, current))
            or not previous['frame'] <= phase['frame'] < current['frame']
            or not isinstance(previous['flags'], dict) or not isinstance(current['flags'], dict)):
        return {}
    removed, added = _inventory(phase) - _inventory(current), _inventory(current) - _inventory(phase)
    if sum(removed.values()) != 1 or sum(added.values()) != 1:
        return {}
    source, target = next(iter(removed)), next(iter(added))
    species = next(iter(current['owned'] - previous['owned']))
    before = [mon for mon in phase['party'] if _name(mon['species']) == source]
    after = [mon for mon in current['party'] if _name(mon['species']) == target]
    if (target != _name(species) or len(before) != 1 or len(after) != 1
            or sum(_name(mon['species']) == source for mon in phase['roster']) != 1
            or sum(_name(mon['species']) == target for mon in current['roster']) != 1
            or before[0]['level'] != after[0]['level']):
        return {}
    for give, receive, map_name, flag in NPC_TRADES:
        if (_name(give) != source or _name(receive) != target or map_name != current['map']
                or previous['flags'].get(flag, False) is not False
                or current['flags'].get(flag) is not True):
            continue
        return {species: {'method': 'npc_trade',
            'basis': 'native_trade_phase_one_party_exchange_and_new_npc_receipt_flag',
            'before_frame': phase['frame'], 'after_frame': current['frame'],
            'flag_baseline_frame': previous['frame'], 'native_phase': phase['native_trade_phase'],
            'map': map_name, 'before': deepcopy(before[0]), 'after': deepcopy(after[0]),
            'catalog_trade': {'from_species': give, 'species': receive, 'completion_flag': flag},
            'scope': 'Observed native NPC exchange and receipt; not full original-route or trade-stat fidelity certification.'}}
    return {}


class RegistrationEvidence:
    def __init__(self):
        self.battle_before = None
        self.level_phase = None
        self.trade_phase = None
        self.previous = None
        self.pending = {}

    def observe_battle(self, kind, state):
        if kind == 'battle_started':
            self.battle_before = deepcopy(state) if isinstance(state, dict) else None
            self.level_phase = None
            self.trade_phase = None
        elif kind == 'battle_resolved':
            if self.battle_before is not None and isinstance(state, dict):
                self.pending.update(native_capture_registration(self.battle_before, state))
            self.battle_before = None
            self.level_phase = (_snapshot(state) if isinstance(state, dict)
                                and state.get('evolution_phase') == 'IsEvolving' else None)

    def observe_trade_phase(self, state):
        if (not isinstance(state, dict) or state.get('npc_trade_phase') not in TRADE_PHASES
                or self.trade_phase is not None):
            return
        snapshot = _snapshot(state)
        if snapshot is not None:
            self.trade_phase = {**snapshot, 'native_trade_phase': state['npc_trade_phase']}

    def registrations(self, facts, graph):
        current = _snapshot(facts)
        if current is None:
            self.previous, self.level_phase, self.trade_phase, self.pending = None, None, None, {}
            return {}
        initial = self.previous is None
        gained = set() if initial else current['owned'] - self.previous['owned']
        evidence = _evolution_registration(self.level_phase, current, graph, native_level_phase=True)
        evidence.update(_evolution_registration(self.previous, current, graph))
        evidence.update(_npc_trade_registration(self.previous, self.trade_phase, current))
        evidence.update(self.pending)
        result = {species: evidence.get(species, {'method': 'unknown',
            'basis': 'no_matched_native_producer_witness',
            'scope': 'Planned goal is not evidence of the actual acquisition method.'}) for species in gained}
        self.previous, self.level_phase, self.trade_phase, self.pending = current, None, None, {}
        return result

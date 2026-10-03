"""Autonomous story planning with generic battle, healing and training skills.

No playthrough milestone or route handler is called. Story candidates come from
scene conditions; preparation candidates come from available encounters and the
party. The existing driver contributes only parameterized navigation and combat.
"""
import json
import math
import re
import hashlib
import time
from collections import Counter, deque
from copy import deepcopy
from functools import lru_cache
from pathlib import Path

import playthrough as pt
import playthrough_late as data

from .story_agent import DualStoryAgent, StoryStopped, attempt_key
from .typesafe import TypeSafeError
from .story_rules import Rule, requirements, evaluate, static_retreat_contract, spent_static_source
from .playthrough_judgments import (ObservedProtocol, NavigationPause, attack_profile, replacement_options,
                                    MEDICINES, BALLS, medicine_options, effective_attacks, ITEM_CATALOG,
                                    PREFERENCE_INSTRUCTIONS)
from .playthrough_judgments import capture_probability, capture_species, capture_status_options, capture_storage_full
from .playthrough_judgments import safari_ball_sequence
from .navigation_skills import (cut_requirement, surf_requirement, water_planning, water_tile,
                                hm_compatible, machine_compatible, HM_MOVES, TM_MOVES, CUT_TILES)
from .navigation_skills import surf_current_prerequisites, field_badge_prerequisites, surf_path_prerequisites
from .boulder_skills import BOULDER_TARGETS, boulder_sources, plan_pushes
from .collection_planner import (acquisition_contract, acquisition_graph, complete_acquisition_graph,
                                 fishing_profile, infer_solo_choices, solo_plan,
                                 table_profile, ENCOUNTER_SLOT_WEIGHTS)

# The level bias asks for more training than the pending fight strictly needs.
LEVEL_PREFERENCE_MARGIN = 2

# Registered-species gates the first playthrough enforces (Oak's lab, the
# Route 2 gate that hands over HM05, ...): the ladder a collector climbs.
DEX_RUNGS = (2, 10, 30, 50, 150)

# Catch rates a trip is compared against; a wild table is only worth
# travelling to relative to how hard its members are to capture.
CATCH_BANDS = ((200, 'easy'), (100, 'medium'))

# Exact slot widths from pokered-core's ENCOUNTER_SLOT_THRESHOLDS.  Maps store
# the ten species/level slots but not their probabilities, and treating the
# slots as equally likely makes a 1.2% species look as valuable as a 19.9%
# species to the strategy judge.
# Qualitative names are easier for Jev to compare than Gen-I's non-monotonic
# internal ball constants.  The game remains the authority for the actual
# capture roll; these labels only describe strategic inventory quality.
BALL_QUALITY = {
    'PokeBall': 'basic',
    'GreatBall': 'improved',
    'UltraBall': 'strong',
    'MasterBall': 'guaranteed',
}


def level_experience(species, level):
    """Native growth curves; level alone gives bounds, not exact current XP."""
    if level <= 1:
        return 0
    rate = data.species_data(species)['growthRate']
    num, den, quad, linear, sub = {
        'MediumFast': (1, 1, 0, 0, 0), 'MediumSlow': (6, 5, -15, 100, 140),
        'Fast': (4, 5, 0, 0, 0), 'Slow': (5, 4, 0, 0, 0),
        'SlightlyFast': (3, 4, 10, 0, 30), 'SlightlySlow': (3, 4, 20, 0, 70),
    }[rate]
    return max(0, num * level**3 // den + quad * level**2 + linear * level - sub)


def evolution_training_cost(mon, target_level):
    target = level_experience(mon['species'], target_level)
    floor = level_experience(mon['species'], mon['level'])
    next_floor = level_experience(mon['species'], min(100, mon['level'] + 1))
    return {'levels_remaining': max(0, target_level - mon['level']),
            'remaining_experience_min': max(0, target - max(floor, next_floor - 1)),
            'remaining_experience_max': max(0, target - floor),
            'scope': 'Bounds from observed level; exact accumulated experience is not controller telemetry'}


def training_yield(table, participants=1):
    """Slot-weighted wild victory XP, with explicit switch-training assumptions."""
    mons = (table or {}).get('mons', [])
    weighted = sum(weight * (data.species_data(mon['species'])['baseExp'] * mon['level'] // 7 // participants)
                   for weight, mon in zip(ENCOUNTER_SLOT_WEIGHTS, mons))
    total_weight = sum(ENCOUNTER_SLOT_WEIGHTS[:len(mons)])
    experience = weighted / total_weight if total_weight else 0
    rate = (table or {}).get('encounterRate', 0)
    return {'expected_experience_per_victory': round(experience, 2),
            'expected_encounter_attempts': round(256 / rate, 2) if rate else None,
            'participants': participants,
            'assumptions': 'Untraded conscious participants; wild victories only, no Exp All; healing and combat turns add cost'}


def evolution_training_effort(mon, target_level, table, participants=1):
    cost = evolution_training_cost(mon, target_level)
    yield_info = training_yield(table, participants)
    experience = yield_info['expected_experience_per_victory']
    if experience <= 0:
        return None
    minimum = math.ceil(cost['remaining_experience_min'] / experience)
    maximum = math.ceil(cost['remaining_experience_max'] / experience)
    attempts = yield_info['expected_encounter_attempts']
    return {**yield_info, 'estimated_victories_min': minimum,
            'estimated_victories_max': maximum,
            'estimated_encounter_steps_max': math.ceil(maximum * attempts) if attempts else None,
            'scope': 'Expectation using the slot-weighted wild table, not a guaranteed battle count; excludes travel, combat turns and healing'}


def catch_difficulty(species):
    rate = data.species_data(species).get('catchRate', 0)
    return {'species': species, 'catch_rate': rate,
            'band': next((band for threshold, band in CATCH_BANDS if rate >= threshold), 'hard')}


def capture_inventory_risk(species, balls):
    """Reference scenarios, explicitly not a forecast of an unseen battle."""
    rate = data.species_data(species)['catchRate']
    scenarios = []
    for hp, status in ((100, 'None'), (100, 'Sleep(2)'), (25, 'Sleep(2)')):
        enemy = {'hp': hp, 'max_hp': 100, 'status': status, 'catch_rate': rate}
        throws = [{'ball': row['ball'], 'quantity': row['quantity'],
                   'per_throw_probability': capture_probability(row['ball'], enemy)}
                  for row in balls if row['quantity'] > 0]
        failure = math.prod((1 - row['per_throw_probability']) ** row['quantity'] for row in throws)
        scenarios.append({'reference_hp_percent': hp, 'reference_status': status,
                          'throws': throws, 'inventory_failure_probability': round(failure, 4)})
    return {'scenarios': scenarios,
            'assumptions': 'Reference max HP 100; all carried balls used at the stated fixed HP/status with independent rolls. Not actual battle odds: excludes HP rounding differences, status expiry, enemy recovery, party survival and travel ball spending.'}


def capture_supply_reference(targets, carried, planned):
    """Compare the same conditional scenarios before and after a purchase."""
    inventory = lambda stock: [{'ball': name, 'quantity': qty} for name, qty in sorted(stock.items())]
    rows = []
    for target in targets:
        before = capture_inventory_risk(target['species'], inventory(carried))
        after = capture_inventory_risk(target['species'], inventory(planned))
        rows.append({**target, 'scenarios': [
            {'reference_hp_percent': old['reference_hp_percent'],
             'reference_status': old['reference_status'],
             'failure_before_purchase': old['inventory_failure_probability'],
             'failure_after_purchase': new['inventory_failure_probability']}
            for old, new in zip(before['scenarios'], after['scenarios'])]})
    return {'targets': rows,
            'scope': 'Conditional reference, not a guarantee: max HP 100, all carried balls used '
                     'at fixed HP/status with independent rolls. Excludes actual HP rounding, '
                     'status expiry, enemy recovery, party survival and balls spent en route. '
                     'Each target uses the same inventory separately, not a budget sufficient '
                     'for all targets together. Ready script guards do not prove navigation access.'}


@lru_cache(maxsize=512)
def safari_species_reference(species, level, balls):
    """Bounds over legal wild HP/Speed DVs, never a predicted hidden individual.

    stats.rs::calc_stat and extract_hp_iv: wild stat exp is zero, and HP DV
    bit 1 is Speed DV bit 0. safari.rs::flee_roll uses the Speed low byte.
    """
    mon = data.species_data(species)
    base = mon['baseStats']
    catches, escapes, successes, spent = [], [], [], []
    for speed_dv in range(16):
        speed = min(999, (base['speed'] + speed_dv) * 2 * level // 100 + 5) & 255
        flee = 1.0 if speed > 127 else speed * 2 / 256
        for hp_dv in range(16):
            if (hp_dv >> 1) & 1 != speed_dv & 1:
                continue
            hp = min(999, (base['hp'] + hp_dv) * 2 * level // 100 + level + 10)
            chance = capture_probability('SafariBall', {
                'hp': hp, 'max_hp': hp, 'catch_rate': mon['catchRate']}, digits=None)
            success, used = safari_ball_sequence(chance, flee, balls)
            catches.append(chance)
            escapes.append(flee)
            successes.append(success)
            spent.append(used)
    return tuple((min(values), max(values)) for values in (catches, escapes, successes, spent))


def safari_capture_reference(table, owned_species=(), balls=30):
    """Slot-weighted registration reference, separate from encounter-only yield."""
    owned = set(owned_species)
    slots = Counter()
    for weight, mon in zip(ENCOUNTER_SLOT_WEIGHTS, (table or {}).get('mons', [])):
        if mon['species'] not in owned:
            slots[(mon['species'], mon['level'])] += weight
    bounds, targets = [0.0, 0.0], []
    rate = int((table or {}).get('encounterRate', 0)) / 256

    def outward(values, digits=4):
        scale = 10 ** digits
        return [math.floor(values[0] * scale) / scale, math.ceil(values[1] * scale) / scale]

    for (species, level), weight in sorted(slots.items()):
        catch, flee, success, spent = safari_species_reference(species, level, balls)
        for index in range(2):
            bounds[index] += rate * weight / 256 * success[index]
        targets.append({'species': species, 'level': level, 'slot_weight_per_256': weight,
                        'per_ball_capture_probability_range': outward(catch),
                        'flee_after_failed_ball_probability_range': outward(flee),
                        'capture_before_flee_probability_range': outward(success),
                        'expected_balls_spent_in_encounter_range': outward(spent, 2)})
    return {'policy': 'ball_only_reference', 'ball_budget_per_encounter': balls,
            'scope': 'Reference bounds, not a forecast: full HP, no status, no bait/rock, zero wild stat exp, all legal HP/Speed DVs and independent rolls. '
                     'Ball budget is per encounter; admission supplies 30 shared balls, not 30 for every encounter. '
                     'Eligible encounter checks only; excludes travel, entry cost, step limit, earlier ball spending and changing owned species. '
                     'Stationary expectation with renewed budgets, not a guarantee within this visit.',
            'targets': targets,
            'new_registration_per_eligible_step_pct_range': outward([p * 100 for p in bounds]),
            'expected_eligible_steps_to_registration_range': (
                outward([1 / bounds[1], 1 / bounds[0]], 1) if bounds[0] > 0 else [None, None])}


def capture_preparation(party, bag, observation=None):
    ball_names = {name.replace('_', '').upper() for name in BALLS}
    preparation = {'balls': {name.replace('_', '').upper(): qty for name, qty in bag.items()
                      if qty > 0 and name.replace('_', '').upper() in ball_names},
            'party': [{key: mon.get(key) for key in ('species', 'level', 'hp', 'status', 'moves', 'pp')}
                      for mon in party]}
    observation = observation or {}
    counts = observation.get('box_counts') or []
    index = observation.get('current_box_index', 0)
    if (0 <= index < len(counts)
            or (observation.get('battle_live') or {}).get('capture_blocked_reason') == 'storage_full'):
        preparation['storage_full'] = capture_storage_full({**observation, 'party': party})
    return preparation


def accumulate_capture_retreat(totals, evidence):
    """Count recorded escapes, measuring costs only across observed inventories."""
    key = evidence['map'] + ':' + evidence['species']
    total = totals.setdefault(key, {'recorded_retreats': 0,
        'inventory_observed_retreats': 0, 'balls_spent': {}})
    total['recorded_retreats'] += 1
    start = evidence.get('preparation', {}).get('balls', {})
    end = (evidence.get('retreat_observation') or {}).get('inventory')
    # Legacy nonempty start stock is evidence; an absent/empty legacy field
    # cannot establish that a full inventory was observed.
    if not evidence.get('start_inventory_observed', bool(start)) or not isinstance(end, list):
        return
    total['inventory_observed_retreats'] += 1
    remaining = capture_preparation([], {row['item']: row['qty'] for row in end})['balls']
    for name, qty in start.items():
        name = name.replace('_', '').upper()
        spent = max(0, qty - remaining.get(name, 0))
        if spent:
            total['balls_spent'][name] = total['balls_spent'].get(name, 0) + spent


def capture_preparation_improvements(current, previous):
    """Public improvements only: movement, damage and spending do not reopen a retry."""
    changes = []
    if previous.get('storage_full') is True and current.get('storage_full') is False:
        changes.append('capture_storage_available')
    for name, qty in current['balls'].items():
        if qty > previous['balls'].get(name, 0):
            changes.append('more_ball_stock:' + name)
    for mon in current['party']:
        if (mon.get('hp') or 0) <= 0:
            continue
        old = [row for row in previous['party'] if row['species'] == mon['species']]
        if not old:
            changes.append('new_conscious_teammate:' + mon['species'])
            continue
        if mon['level'] > max(row['level'] for row in old):
            changes.append('higher_level:' + mon['species'])
        if mon['hp'] > max(row.get('hp') or 0 for row in old):
            changes.append('health_restored:' + mon['species'])
        if mon.get('status', 'None') == 'None' and all(row.get('status', 'None') != 'None' for row in old):
            changes.append('status_cured:' + mon['species'])
        for move, pp in zip(mon.get('moves') or [], mon.get('pp') or []):
            if move == 'None' or pp <= 0:
                continue
            previous_pp = max((p for row in old for m, p in zip(row.get('moves') or [], row.get('pp') or [])
                               if m == move), default=0)
            if pp > previous_pp:
                changes.append('usable_move_improved:' + mon['species'] + ':' + move)
    return changes


def encounter_value(map_data, owned_species=()):
    """Deterministic collection value of one grass table.

    Keep arithmetic out of Jev: it should weigh travel, scarcity and resources,
    not reconstruct encounter-slot probabilities from a ten-row table.
    Percentages are rounded only at the presentation boundary.
    """
    wild = ((map_data.get('wild') or {}).get('red') or {}).get('grass') or {}
    value = table_profile(wild, owned_species)
    # Compatibility names retained for traces/tests written for grass-only
    # collection; the unified planner uses the method-neutral names too.
    value['new_species_per_step_pct'] = value['new_species_per_attempt_pct']
    value['expected_steps_to_any_new_species'] = value['expected_attempts_to_any_new_species']
    for target in value['targets']:
        target.update(catch_difficulty(target['species']))
        target['per_step_pct'] = target['per_attempt_pct']
        target['expected_steps'] = target['expected_attempts']
    return value


def native_interaction_tiles():
    """Read engine-owned OnInteract bindings that scene OnStep metadata omits."""
    source = (data.DATA.parent / 'pokered-core/src/overworld/update.rs').read_text()
    result = {}
    for table, scale in [('card_key_doors', 2), ('mansion_statues', 1), ('cinnabar_quiz_machines', 1)]:
        body = source.split(f'let {table}:', 1)[1].split('\n        };', 1)[0]
        for name, entries in re.findall(r'MapId::(\w+)\s*=>\s*&\[(.*?)\]', body, re.S):
            for x, y, handler in re.findall(r'\((\d+),\s*(\d+),\s*"(\w+)"\)', entries):
                result[f'{name}:{handler}'] = [(int(x)*scale+dx, int(y)*scale+dy)
                                               for dx in range(scale) for dy in range(scale)]
    return result


NATIVE_INTERACTIONS = native_interaction_tiles()
FIRST_INDOOR_MAP = int(re.search(r'FIRST_INDOOR_MAP: u8 = (0x[0-9a-fA-F]+)',
    (data.DATA / 'src/map_constants.rs').read_text())[1], 16)

_HEADERS = (data.DATA / 'src/tileset_data.rs').read_text().split(
    'pub const TILESET_HEADERS:', 1)[1].split('];', 1)[0]
COUNTERS = [{int(v.strip(), 0) for v in values.split(',') if int(v.strip(), 0) >= 0}
            for values in re.findall(r'header\(([^,]+,[^,]+,[^,]+),', _HEADERS)]
ENCOUNTER_GRASS_TILES = [int(value.strip(), 0)
    for value in re.findall(r'header\([^,]+,[^,]+,[^,]+,([^,]+),', _HEADERS)]


def trigger_position_matches(rule, point):
    for guard, wanted in rule.guards:
        calls = set(re.findall(r'"callee":\s*"(?:game\.)?([^"]+)"', json.dumps(guard)))
        if calls and calls <= {'getPlayerX', 'getPlayerY'}:
            value = evaluate(guard, {'x': point[0], 'y': point[1]})
            if value is not None and bool(value) != wanted:
                return False
    return True


def counter_approaches(map_name, npc):
    tiles = COUNTERS[pt.MAPS[map_name]['tileset_id']]
    for direction, (dx, dy) in pt.DELTA.items():
        middle = npc['x']-dx, npc['y']-dy
        target = npc['x']-2*dx, npc['y']-2*dy
        if pt.tile_at(map_name, *middle) in tiles and pt.walkable(map_name, *target):
            yield target, direction


def training_tile(name, x, y):
    """A native grass-table stance with a valid right-hand rate anchor."""
    m = pt.MAPS[name]
    if not pt.walkable(name, x, y) or (x, y) in pt.warp_tiles(name):
        return False
    standing = pt.tile_at(name, x, y)
    right = pt.tile_at(name, x+1, y) if x+1 < m['width']*2 else standing
    grass = ENCOUNTER_GRASS_TILES[m['tileset_id']]
    # Native determine_encounter_type explicitly excludes Forest from the
    # indoor catch-all. Safari paths are not encounter terrain. Grass comes
    # from the actual tileset header, not the Overworld-only legacy helper.
    indoor = m['id'] >= FIRST_INDOOR_MAP and m['tileset_name'].lower() != 'forest'
    return (standing == grass and right == grass) or (
        indoor and standing not in (0x14, 0x15) and right != 0x14)


def battle_readiness(party, bag):
    medicine_names = {name.replace('_', '').upper() for name in MEDICINES}
    return {'party': [{'species': m['species'], 'level': m['level'], 'moves': m['moves'],
                       'hp_band': (4*m['hp']+m['max_hp']-1)//m['max_hp'],
                       'pp': m['pp'],
                       'status': m.get('status', 'None')} for m in party],
            'medicine': {name: qty for name, qty in bag.items() if name in medicine_names}}


def training_battler(party):
    """Return the strongest conscious member that can earn battle experience.

    Collection catches can replace the lead slot with a low-level Pokemon.  A
    story preparation threshold is about the party's capable battler, not that
    incidental slot order.  Prefer a member with a usable damaging move, then
    fall back to any conscious member so callers remain useful for sparse test
    fixtures and unusual early-game parties.
    """
    conscious = [mon for mon in party if mon.get('hp', 1) > 0]
    usable = [mon for mon in conscious if any(
        name != 'None' and pp > 0 and data.move_data(name)['power'] > 0
        for name, pp in zip(mon.get('moves', []), mon.get('pp', [])))]
    candidates = usable or conscious or list(party)
    return max(candidates, key=lambda mon: mon.get('level', 0)) if candidates else None


def storage_deposit_indices(party):
    """Preserve the main battler and sole carriers of required field moves."""
    if not party:
        return []
    main = max(range(len(party)), key=lambda index: party[index]['level'])
    protected = {main}
    for move in ('Cut', 'Surf', 'Strength'):
        carriers = [i for i, mon in enumerate(party) if move in mon.get('moves', [])]
        if len(carriers) == 1:
            protected.update(carriers)
    return [i for i in range(len(party)) if i not in protected]


def type_options(party, opponents):
    """Super-effective party moves per opponent species, from public type data."""
    chart = data.type_chart()
    options = {}
    for enemy in opponents:
        species = enemy.get('species')
        if not species or species in options:
            continue
        defenders = {data.species_data(species)[key] for key in ('type1', 'type2')}
        effective = {}
        for mon in party:
            for name in mon['moves']:
                if name == 'None' or name in effective:
                    continue
                move = data.move_data(name)
                multiplier = 1
                for typ in defenders:
                    multiplier *= chart.get((move['type'], typ), 1)
                if move['power'] > 0 and multiplier > 1:
                    effective[name] = {'pokemon': mon['species'], 'type': move['type'],
                                       'power': move['power'], 'effectiveness': multiplier}
        options[species] = {'types': sorted(defenders), 'super_effective_moves': effective}
    return options


def tactical_options(party, opponents):
    """Status and support moves the party can spend a battle turn on."""
    options = {}
    for mon in party:
        for name in mon['moves']:
            if name == 'None' or name in options:
                continue
            move = data.move_data(name)
            if move['power'] == 0:
                options[name] = {'pokemon': mon['species'], 'type': move['type'],
                                 'effect': move['effect']}
    return {'opponent_species': sorted({enemy['species'] for enemy in opponents}),
            'status_moves': options}


def reachable_grass(map_name, start, blocked=()):
    """Training ground in the player's actual connected component."""
    queue, seen = deque([start]), {start}
    blocked = set(blocked) | pt.warp_tiles(map_name)
    while queue:
        position = queue.popleft()
        neighbors = []
        for dx, dy in pt.DELTA.values():
            target = position[0]+dx, position[1]+dy
            if target not in blocked and pt.walkable_edge(map_name, position, target):
                neighbors.append(target)
                if target not in seen:
                    seen.add(target)
                    queue.append(target)
        if training_tile(map_name, *position) and any(training_tile(map_name, *p) for p in neighbors):
            return position
    return None


@lru_cache(maxsize=None)
def fishing_spots(map_name):
    """Walkable stances facing a tile accepted by the engine's rod check."""
    result = []
    for x in range(pt.MAPS[map_name]['width'] * 2):
        for y in range(pt.MAPS[map_name]['height'] * 2):
            if not pt.walkable(map_name, x, y):
                continue
            for direction, (dx, dy) in pt.DELTA.items():
                if water_tile(map_name, x + dx, y + dy):
                    result.append(((x, y), direction))
                    break
    return result


@lru_cache(maxsize=None)
def surf_spots(map_name):
    """Land stance, facing, and adjacent water tile for starting a water hunt."""
    return [(stance, direction,
             (stance[0] + pt.DELTA[direction][0], stance[1] + pt.DELTA[direction][1]))
            for stance, direction in fishing_spots(map_name)]


def compact_strategy_candidates(candidates):
    """Factor repeated navigation prose without dropping goals or blockers."""
    result = {}
    for key, value in candidates.items():
        try:
            candidate = json.loads(value)
        except (ValueError, TypeError):
            result[key] = value
            continue
        context = candidate.get('context') if isinstance(candidate, dict) else None
        if isinstance(context, dict):
            routes = context.get('trigger_navigation') or []
            if routes:
                scopes = list(dict.fromkeys(route.get('scope') for route in routes if route.get('scope')))
                if len(scopes) == 1:
                    context['navigation_scope'] = scopes[0]
                    routes = [{k: v for k, v in route.items() if k != 'scope'} for route in routes]
                # Keep every reachable cost and every blocked map. False
                # routes carry identical null costs, so a map list is lossless.
                blocked = [route for route in routes if route.get('tile_route_found') is False
                           and route.get('steps') is None and not route.get('requires_surf')
                           and set(route) <= {'map', 'tile_route_found', 'steps', 'requires_surf'}]
                if blocked:
                    context['unreachable_trigger_maps'] = [route['map'] for route in blocked]
                    routes = [route for route in routes if route not in blocked]
                context['trigger_navigation'] = routes
        result[key] = json.dumps(candidate, separators=(',', ':'), ensure_ascii=False)
    return result


def factor_strategy_evidence(state, candidates, min_chars=160):
    """Losslessly share decision evidence on either layer; never shortlist options.

    Retain the historical strategy-prefixed wire names for trace compatibility.
    """
    decoded = {}
    for key, value in candidates.items():
        try:
            decoded[key] = json.loads(value)
        except (ValueError, TypeError):
            decoded[key] = value
    counts, values = Counter(), {}

    def fingerprint(value):
        return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False)

    def collect(value):
        if isinstance(value, (dict, list, str)):
            serial = fingerprint(value)
            if len(serial) >= min_chars:
                counts[serial] += 1
                values[serial] = value
            for child in (value.values() if isinstance(value, dict) else
                          value if isinstance(value, list) else []):
                collect(child)
    collect(state)
    for value in decoded.values():
        collect(value)
    shared = {serial: f'e{index}' for index, serial in enumerate(
        serial for serial, count in counts.items() if count > 1)}
    if not shared:
        return state, candidates

    def encode(value, skip=None):
        if isinstance(value, (dict, list, str)):
            serial = fingerprint(value)
            if serial in shared and serial != skip:
                return {'shared_strategy_evidence_ref': shared[serial]}
            if isinstance(value, str):
                return value
            if isinstance(value, dict):
                return {key: encode(child) for key, child in value.items()}
            # Encounter tables, party snapshots and route records repeat the
            # same field names on every row. Keep all rows and all values, but
            # transmit those names once. Sharing entire identical objects alone
            # cannot compress tables whose species/levels differ on each row.
            if (len(value) >= 3 and all(isinstance(row, dict) for row in value)
                    and value[0] and all(set(row) == set(value[0]) for row in value)):
                columns = list(value[0])
                table = {'strategy_table': {'columns': columns,
                         'rows': [[encode(row[column]) for column in columns] for row in value]}}
                ordinary = [encode(child) for child in value]
                if len(fingerprint(table)) < len(fingerprint(ordinary)):
                    return table
            return [encode(child) for child in value]
        return value

    library = {key: encode(values[serial], skip=serial) for serial, key in shared.items()}
    factored_state = {key: encode(value) for key, value in state.items()}
    factored_state['shared_strategy_evidence'] = library
    factored_candidates = {key: json.dumps(encode(value), separators=(',', ':'), ensure_ascii=False)
                           if not isinstance(value, str) else value
                           for key, value in decoded.items()}
    # Table encoding can bypass shared row objects. Do not send unreachable
    # library entries: they are duplicate storage, not additional evidence.
    used = set()

    def mark(value):
        if isinstance(value, dict):
            if set(value) == {'shared_strategy_evidence_ref'}:
                key = value['shared_strategy_evidence_ref']
                if key not in used:
                    used.add(key)
                    mark(library[key])
            else:
                for child in value.values():
                    mark(child)
        elif isinstance(value, list):
            for child in value:
                mark(child)

    for key, value in factored_state.items():
        if key != 'shared_strategy_evidence':
            mark(value)
    for value in factored_candidates.values():
        try:
            mark(json.loads(value))
        except (ValueError, TypeError):
            pass
    factored_state['shared_strategy_evidence'] = {key: value for key, value in library.items()
                                                 if key in used}
    return factored_state, factored_candidates


def strategy_access_evidence(candidates):
    """Compare fresh trigger access without removing legal future goals."""
    result = {key: {} for key in ('path_found', 'field_action_needed', 'no_path_found', 'not_evaluated')}
    for key, value in candidates.items():
        try:
            row = json.loads(value)
        except (ValueError, TypeError):
            continue
        if not isinstance(row, dict) or 'establish' not in row:
            continue
        context = row.get('context') or {}
        routes = context.get('trigger_navigation') or []
        if any(route.get('tile_route_found') is True and not route.get('requires_surf') for route in routes):
            status = 'path_found'
        elif any(route.get('tile_route_found') is True for route in routes):
            status = 'field_action_needed'
        elif (routes or context.get('unreachable_trigger_maps')) and all(
                route.get('tile_route_found') is False for route in routes):
            status = 'no_path_found'
        else:
            status = 'not_evaluated'
        result[status][key] = row['establish']
    result['scope'] = ('Current planning evidence to actual trigger regions, not a victory or legal-action '
        'guarantee. No path found means access still needs resolving before the target can make progress; '
        'field_action_needed has only a water-relaxed path: an actual Surf action must occur first. '
        'it does not prove permanent impossibility. Not evaluated is unknown, not reachable. '
        'All candidates remain available, including exploration of unproven access.')
    return result


class AutonomousStoryAgent(DualStoryAgent):
    def __init__(self, *args, game, preference='none', **kwargs):
        super().__init__(*args, **kwargs)
        self.preference = preference
        self.game = game
        # Boot input, judgments and native commands share one trace clock.
        started = getattr(getattr(game, 'judgments', None), 'start_time', None)
        if isinstance(started, (int, float)):
            self.start_time = started
        game.judgments = self
        if isinstance(game.d, ObservedProtocol):
            game.d.record = self.record
        game.smart_moves = True
        self.maps = {p.parent.name: json.loads(p.read_text())
                     for p in data.DATA.glob('maps/*/map.json')}
        self.trainers = {p.stem: json.loads(p.read_text())
                         for p in data.DATA.glob('trainers/*.json')}
        self.visited = {self.client.state()['map_name']}
        self.training_sites = {}
        self.catch_navigation = {}
        self.catch_areas = {}
        goal_ids = {o.get('id') for o in self.objectives}
        self.collects_dex = 'collect-dex' in goal_ids
        self.maximizes_coverage = 'max-coverage' in goal_ids
        self.avoids_optional_preparation = 'fast-clear' in goal_ids
        self.healing_rules = []
        self.hof_baseline = self.client.state().get('hall_of_fame_count', 0)
        self.first_clear_verification = None
        self.navigation_blockage = None
        self.navigation_memory = {}
        self.navigation_history = {}
        self.observed_barrier_maps = set()
        self.field_requirements = {}
        self.route_requirements = {}
        self.cleared_terrain = set()
        self.crossed_passages = set()
        self.battle_requirements = {}
        self.battle_defeats = []
        self.capture_retreats = {}
        self.capture_retreat_totals = {}
        self.collection_audit_pending = {}
        self._audit_party = None
        self.defeat_preparation = 0
        self.replan_after_defeat = False
        self.mechanism_goal = None
        self._recorded_dex_species = None

    def facts(self):
        facts = super().facts()
        live = self.game.st()  # Refresh live geometry after field moves / map reloads.
        facts['party'] = [{k: mon.get(k) for k in
                           ('species', 'level', 'hp', 'max_hp', 'status', 'moves', 'pp')}
                          for mon in self.client.state().get('party', [])]
        facts['stored_pokemon'] = [{k: mon.get(k) for k in
                                    ('box', 'index', 'species', 'level', 'hp', 'max_hp',
                                     'status', 'moves', 'pp')}
                                   for mon in self.client.state().get('stored_pokemon', [])]
        self.observe_audit_evolution(facts['party'], live.get('frame_count'))
        facts['collection_audit_pending'] = sorted(getattr(self, 'collection_audit_pending', {}))
        facts['current_box_index'] = self.client.state().get('current_box_index', 0)
        facts['box_counts'] = list(self.client.state().get('box_counts', []))
        facts['fully_recovered'] = bool(facts['party']) and all(
            mon['hp'] == mon['max_hp'] and mon['status'] == 'None'
            and all(move == 'None' or pp >= data.move_data(move)['pp']
                    for move, pp in zip(mon['moves'], mon['pp']))
            for mon in facts['party'])
        self.visited.add(facts['map'])
        facts['object_visibility'] = {}
        if self.index:
            for npc in self.client.cmd(cmd='get_npcs'):
                toggle = self.index.npc_toggles.get((facts['map'], npc['text_id']))
                if toggle:
                    facts['object_visibility'][toggle[0]] = npc.get('visible', True)
        facts['cleared_terrain'] = sorted(self.cleared_terrain)
        facts['navigation_revision'] = len(self.visited) + len(self.crossed_passages)
        facts['block_values'] = {}
        facts['recent_battle_defeats'] = self.battle_defeats[-3:]
        dex = facts.get('dex') or {}
        owned_species = tuple(sorted(dex.get('owned_species', [])))
        if self.collects_dex and owned_species != self._recorded_dex_species:
            previous = set(self._recorded_dex_species or ())
            active_context = (self.active or {}).get('context', {})
            self.record('dex_progress', owned=dex.get('owned', len(owned_species)),
                        seen=dex.get('seen', len(dex.get('seen_species', []))),
                        acquired=sorted(set(owned_species) - previous),
                        owned_species=list(owned_species), map=facts['map'],
                        acquisition_method=active_context.get('acquisition_method'),
                        active_target=(self.active or {}).get('target'),
                        party_count=len(facts['party']),
                        stored_count=len(facts['stored_pokemon']), frame=live['frame_count'])
            self._recorded_dex_species = owned_species
        if self.collects_dex:
            self.require_static_sources(facts)
        if self.index:
            for rule in self.index.rules:
                if (rule.effect[0] != 'block' or 'load' not in rule.triggers or rule.map == facts['map']
                        or rule.missing(facts) or any(e[0] == 'battle' for e in rule.preceding)):
                    continue
                # Predict deterministic entry-time geometry for solved
                # mechanisms on other maps. The current map always uses
                # the actual read-only snapshot above.
                name, x, y = rule.effect[1].split(',')
                m = pt.MAPS[name]
                offset = int(y)*m['width'] + int(x)
                if offset < len(m['blocks']):
                    m['blocks'][offset] = rule.effect[2]
            for kind, key, value in self.index.by_effect:
                if kind == 'block':
                    name, x, y = key.split(',')
                    if name == facts['map']:
                        offset = int(y) * pt.MAPS[name]['width'] + int(x)
                        blocks = live.get('map_blocks', pt.MAPS[name]['blocks'])
                        if offset < len(blocks):
                            facts['block_values'][key] = blocks[offset]
        return facts

    def invalidate_terrain(self, state):
        """A tree regrows on map entry; invalidate on every observed transition."""
        for key in list(self.cleared_terrain):
            name, x, y = key.split(',')
            if name == state['map_name'] and pt.tile_at(name, int(x), int(y)) == CUT_TILES.get(pt.MAPS[name]['tileset_name']):
                self.cleared_terrain.remove(key)

    @staticmethod
    def needs_healing(facts, preserve_coverage=True):
        if not facts['party']:
            return False
        mon = facts['party'][0]
        attack_pp = sum(pp for move, pp in zip(mon['moves'], mon['pp'])
                        if move != 'None' and data.move_data(move)['power'] > 0)
        # Ordinary story/training preparation preserves each coverage move.
        # A capture trip can continue with adequate remaining attacks; recovery
        # is still offered while fully_recovered is false.
        depletion = any if preserve_coverage else all
        depleted_attack = depletion(pp <= data.move_data(move)['pp'] * .25
                              for move, pp in zip(mon['moves'], mon['pp'])
                              if move != 'None' and data.move_data(move)['power'] > 0)
        return (mon['hp'] < mon['max_hp'] * .7 or mon['status'] != 'None'
                or attack_pp < 6 or depleted_attack)

    @staticmethod
    def needs_capture_recovery(facts):
        return AutonomousStoryAgent.needs_healing(facts, preserve_coverage=False)

    def should_replan(self, facts):
        if self.replan_after_defeat:
            return True
        if (self.active and self.active['target'][0] in ('catch', 'held_species')
                and getattr(self, '_completed_hunts_since_strategy', 0) >= self.CATCH_WINDOW):
            return True  # Reassess a bounded hunt batch without blacklisting its valid terrain.
        if self.capture_resources_missing(facts):
            return True
        if (self.active and self.active.get('context', {}).get('acquisition_method') == 'static'
                and any(self.static_capture_deferred(self.active['context']['species'], rule.map, facts)
                        for rule in self.active.get('rules', []))):
            return True
        if (self.active and self.active['target'][0] in ('catch', 'held_species')
                and any(self.capture_area_blocked(rule.map, facts)
                        for rule in self.active.get('rules', []))):
            return True  # The just-observed encounter cannot satisfy this hunt.
        if (self.active and self.active['target'][0] == 'item' and self.active['target'][2]
                and len(facts.get('bag', {})) >= 20
                and not facts['bag'].get(self.active['target'][1].replace('_', '').upper())):
            return True
        party = facts.get('party', [])
        main = max(party, key=lambda mon: mon['level']) if party else None
        main_critical = bool(main and main.get('max_hp', 0) > 0
                             and main['hp'] <= main['max_hp'] * .25)
        return (self.active and self.active['target'][0] != 'heal'
                and (main_critical or (self.needs_skill_recovery(facts)
                     and getattr(self, '_selected_recovery_key', None) != self.recovery_replan_key(facts))))

    def recovery_replan_key(self, facts):
        """Exact recovery evidence accepted by the latest strategic choice.

        Coordinates are not fatigue: arriving at a selected shop with unchanged
        HP/PP must not cancel the purchase just because those PP were already
        low when Jev selected it. New injury, PP/status/party or medicine changes
        still re-open planning. Serialize to avoid aliasing mutable observations.
        """
        fields = ('species', 'level', 'hp', 'max_hp', 'status', 'moves', 'pp')
        medicine_names = {name.replace('_', '').upper() for name in MEDICINES}
        return json.dumps([
            (self.active or {}).get('target'),
            [{key: mon.get(key) for key in fields} for mon in facts.get('party', [])],
            {name: qty for name, qty in facts.get('bag', {}).items() if name in medicine_names},
        ], sort_keys=True)

    def capture_resources_missing(self, facts, method=None):
        """All captures need capacity; only Safari supplies its own balls."""
        active = getattr(self, 'active', None) or {}
        if method is None:
            if (active.get('target', [None])[0] not in ('catch', 'held_species')
                    and active.get('context', {}).get('acquisition_method') != 'static'):
                return False
            method = active.get('context', {}).get('acquisition_method', 'grass')
        return (capture_storage_full(facts)
                or method != 'safari' and 'bag' in facts and self.balls_held(facts) <= 0)

    def needs_skill_recovery(self, facts):
        """A status-only evolution trainee can share XP with a ready finisher."""
        active = getattr(self, 'active', None) or {}
        context = active.get('context', {})
        if active.get('target', [None])[0] in ('catch', 'held_species'):
            return self.needs_capture_recovery(facts)
        party = facts.get('party', [])
        if (party and (context.get('acquisition_method') == 'evolution'
                       or context.get('capture_support_training'))
                and context.get('trigger') == 'level'
                and self.same_species(party[0]['species'], context.get('from_species', ''))):
            trainee = party[0]
            if trainee['hp'] < trainee['max_hp'] * .7 or trainee['status'] != 'None':
                return True
            finisher = training_battler(party)
            if (finisher is not trainee and finisher and finisher['level'] > trainee['level']
                    and not self.needs_healing({'party': [finisher]})):
                return False
        return self.needs_healing(facts)

    def select_strategy(self, facts):
        super().select_strategy(facts)
        self._completed_hunts_since_strategy = 0
        self._selected_recovery_key = self.recovery_replan_key(facts)
        self.replan_after_defeat = False

    def completed_stochastic_attempt(self, operation, rule, result, resolved_battles):
        # Reaching an actual encounter and returning empty-handed is normal
        # for rare species. Do not mark the site unexecutable just because
        # bag/party/position ended unchanged after an escape. A label alone
        # is not enough: settle() must have observed a completed battle.
        observed = (operation.startswith('catch_encounter:')
            and rule.storyline == 'skill:catch_encounter'
            and result.get('result') == 'hunted' and resolved_battles > 0)
        if observed:
            self._completed_hunts_since_strategy = getattr(self, '_completed_hunts_since_strategy', 0) + 1
        return observed

    def augment_strategy_state(self, state, facts):
        continuation = getattr(self, 'route_continuation', None)
        if continuation:
            if (self.index.satisfied(continuation['goal'], facts)
                    or facts.get('map') != continuation['landing'][0]):
                self.route_continuation = None
            else:
                state['completed_route_prerequisite'] = continuation
        if self.collects_dex:
            state['dex_progress'] = self.dex_progress(facts)
            state['collection_audit_pending'] = getattr(self, 'collection_audit_pending', {})
            state['capture_retreats_requiring_preparation'] = [row for row in
                getattr(self, 'capture_retreats', {}).values()
                if self.static_capture_deferred(row['species'], row['map'], facts)]
            # An improved stock/level reopens a legal attempt, not proof that
            # the previous capture setup now survives. Keep its observed
            # outcome visible even when it no longer blocks the retry.
            preparation = capture_preparation(facts.get('party', []), facts.get('bag', {}), facts)
            state['capture_retry_evidence'] = [{**row,
                'recorded_history': getattr(self, 'capture_retreat_totals', {}).get(key, {}),
                'history_scope': 'Recorded menu escapes in this checkpoint lineage only; ball costs cover inventory_observed_retreats, not unobserved attempts. Not a prediction of retry success.',
                'preparation_changes_since_attempt': capture_preparation_improvements(
                    preparation, row['preparation'])}
                for key, row in getattr(self, 'capture_retreats', {}).items()]

    def dex_progress(self, facts):
        """Collection panel: what is missing, where, and what it unlocks."""
        dex = facts.get('dex') or {}
        owned = self.validated_owned(facts)
        seen = set(dex.get('seen_species', []))
        wild_graph = self.collection_graph()
        full_graph = self.complete_collection_graph()
        plan = solo_plan(full_graph, owned, infer_solo_choices(owned))
        targets = set(plan['reachable_species'])
        choice_targets = set(plan['choice_reachable_species'])
        # The 150-species diploma is not reachable under the solo/no-link
        # contract. Keep reachable early thresholds and finish at this plan's
        # actual ceiling; the final rung is completion, not an item reward.
        rungs = sorted({value for value in DEX_RUNGS if value <= plan['ceiling']}
                       | {plan['ceiling']})
        rung = next((value for value in rungs if value > len(owned)), None)
        missing = {}
        yield_by_area = {}
        for name in self.neighbourhood():
            value = encounter_value(self.maps.get(name, {}), owned)
            count = value['unregistered_species_count']
            if count:
                missing[name] = count
                yield_by_area[name] = {
                    key: value[key] for key in (
                        'unregistered_species_count', 'unregistered_encounter_share_pct',
                        'new_species_per_step_pct', 'expected_steps_to_any_new_species')}
        missing_methods = {}
        for species in sorted(choice_targets - owned):
            missing_methods[species] = sorted({method['method'] for method in full_graph[species]
                                               if not method.get('external_trade')})
        return {'owned': dex.get('owned', len(owned)), 'validated_owned': len(owned),
                'pending_source_validation': sorted(getattr(self, 'collection_audit_pending', {})),
                'seen': dex.get('seen', len(seen)), 'total': 151,
                'supported_wild_target_count': len(wild_graph),
                'supported_wild_owned': len(set(wild_graph) & owned),
                'supported_wild_remaining': len(set(wild_graph) - owned),
                'solo_target_count': plan['ceiling'],
                'solo_owned': len(targets & owned),
                'solo_remaining': len(targets - owned),
                'solo_choices': plan['choices'],
                'solo_choice_options': plan['optimal_choices'],
                'policy_unreachable_count': len(plan['unreachable_species']),
                'policy_unreachable_species': plan['unreachable_species'],
                'always_unreachable_count': len(plan['always_unreachable_species']),
                'always_unreachable_species': plan['always_unreachable_species'],
                'missing_acquisition_methods': missing_methods,
                # Seen does not prove solo availability (trainers may show
                # excluded species); retain that observation separately from
                # missing_acquisition_methods and the bounded solo progress.
                'seen_not_owned': sorted(seen - owned),
                'next_rung': None if rung is None else {'rung': rung, 'needs': rung,
                                                        'remaining': rung - len(owned)},
                'unregistered_by_area': dict(sorted(missing.items(), key=lambda row: (-row[1], row[0]))),
                'expected_yield_by_area': dict(sorted(yield_by_area.items())),
                'balls_held': self.balls_held(facts),
                'collection_resources': self.collection_resources(facts),
                'nearest_ball_source': self.nearest_ball_source(facts.get('map'))}

    def nearest_ball_source(self, origin):
        """Closest reachable shop selling any ball, from `self.maps` alone.

        The panel is assembled once per decision, so the distance comes from
        the local map graph rather than a route query per candidate mart.
        """
        normalized = {name.replace('_', '').upper(): name for name in BALLS}
        candidates = []
        for rule in self.index.rules:
            if rule.effect[0] != 'shop':
                continue
            stock = sorted({normalized[key.replace('_', '').upper()]
                            for key in rule.effect[1]
                            if key.replace('_', '').upper() in normalized})
            if not stock:
                continue
            hops = self.map_hops(origin, rule.map)
            if hops is not None:
                candidates.append((hops, rule.map, rule.id, stock))
        if not candidates:
            return None
        hops, name, _rule_id, stock = min(candidates)
        return {'map': name, 'hops': hops, 'stock': stock}

    def choose(self, layer, state, candidates, instruction):
        if layer == 'strategy':
            candidates = compact_strategy_candidates(candidates)
            access = strategy_access_evidence(candidates)
            if access['path_found'] or access['field_action_needed'] or access['no_path_found']:
                state = {**state, 'immediate_access_comparison': access}
                instruction += (' Use immediate_access_comparison to distinguish progress that can '
                    'currently be approached from goals still needing access. Compare reachable '
                    'prerequisites and local actions before repeating an inaccessible training, shopping '
                    'or collection destination. A cheap future goal is not cheap immediate progress '
                    'when its access remains unresolved; choosing it should have a concrete new access '
                    'hypothesis rather than repeating the unchanged failed approach. '
                    'field_action_needed is conditional access, not immediate walking access: compare '
                    'the actual offered field-move prerequisite at its reachable embarkation stance.')
            if any('downstream_context' in value for value in candidates.values()):
                instruction += (' A route unlock is an intermediate step, not a Pokédex registration. '
                    'Each route_unlocks entry carries its parent goal and downstream_context: '
                    'compare the remaining acquisition effort, inventory risk, preparation and source '
                    'constraints with the other goals before investing in this route. Opening access '
                    'does not itself solve the downstream capture, training, purchase or capacity need. '
                    'Reference capture scenarios are conditional estimates, not promised outcomes; '
                    'a difficult parent can still be worthwhile when its durable benefit justifies the cost.')
        if layer == 'strategy' and any('route_resets_won_battles' in value for value in candidates.values()):
            instruction += (' Compare every candidate travel route with its supplied story-reset cost. '
                'Training, retrieving teammates, shopping and hunting can cross the same reset entry '
                'as healing: their benefit must also outweigh replaying the already won battles. '
                'This evidence describes a proposed route, not a ban on leaving or a fixed route to follow. '
                'Depleted PP in one move does not require leaving when other usable attacks can handle '
                'the remaining opponents. Prefer preserving completed battles when continuing or using '
                'carried recovery is viable; retreat remains valid when the party cannot proceed.')
            instruction += (' Recovery is preparation, not a requirement to be fully replenished after every battle. '
                'When recovery resets won battles, do not repeat a full-heal loop just to top off HP or PP. '
                'Compare current health and usable effective attacks with the next opponent, and continue when '
                'those resources are sufficient. The urgently_needed field is a heuristic warning, not a '
                'requirement to refill each depleted move. A depleted attack can be replaced by another '
                'effective attack with PP remaining. Retreat only when its benefit outweighs replaying all reset battles.')
        if layer == 'strategy' and any('completion_resets_won_battles' in value for value in candidates.values()):
            instruction += (' completion_resets_won_battles is different from route_resets_won_battles: '
                'the former clears temporary flags while completing the selected ending and returning '
                'to its saved destination; it does not require replaying those already won battles '
                'to complete this exit. Remaining opponents and the ceremony still require real execution. '
                'Compare the exit and any reachable preparations with goals whose trigger regions '
                'currently have no walking path; remote training cannot grant experience before access is restored.')
        if layer == 'strategy' and getattr(self, 'maximizes_coverage', False):
            instruction += (' The terminal goal is coverage: visiting a new map is progress in itself, so once the '
                'current objective is satisfied prefer reaching an unexplored bordering area over optional '
                'preparation.')
        if layer == 'strategy' and getattr(self, 'avoids_optional_preparation', False):
            instruction += (' The terminal goal is speed: skip optional preparation unless the party genuinely '
                'cannot proceed, and prefer the shortest route to the objective.')
        if layer == 'strategy' and getattr(self, 'collects_dex', False):
            instruction += (' The terminal goal is the Pokédex, not the Hall of Fame: this run ends only when every '
                'species reachable in one Pokémon Red save without external link trades is registered. The first '
                'playthrough is the channel to more species — badges, HM moves and new routes open encounters, '
                'gifts, static Pokémon, NPC trades and evolution resources. Treat the '
                'story objectives as the way to reach new collecting grounds rather than as a finish line. '
                'Collect locally when yield and preparation costs are competitive; progress the story when '
                'new regions are more efficient than rare hunts or low-experience grinding, even if local '
                'species remain. Never stop at the Champion while species remain. Compare '
                'collection candidates using their supplied encounter probability, expected hunt steps, species '
                'scarcity, travel cost, recent yield, ball quality and safe status support. A larger species list is '
                'not automatically better when its missing species occupy rare slots or the current resources '
                'cannot realistically catch them. Compare evolution training_cost with alternative_sources '
                'and the value of unlocking new regions: a low-level trainee may require many victories, '
                'while a later wild capture can register the evolved species directly. Potential alternative '
                'sources are not guaranteed reachable; choose the prerequisites needed to reach them. '
                'script_unlocks describes scene guards that obtaining an item or flag would satisfy; '
                'weigh those durable opportunities against repeat preparation. It does not prove the '
                'resulting rooms reachable or battles won, and is not a promised registration count. '
                'Use training_effort_examples to compare the estimated number of victories and encounter '
                'steps, not just levels remaining. Hundreds of low-yield battles have an opportunity '
                'cost: acquiring an HM or resolving a story blocker may open better collecting and '
                'training grounds. Previously visited tables are examples, not proof of current access.')
            instruction += (' Compare capture_retry_evidence with current preparation: an observed '
                'retreat can show a status support fainted while the target remained healthy and unstatused. '
                'More balls do not make that support survive the switch or act; healing restores its prior '
                'condition, not its combat strength. Consider viable alternatives or prerequisites, stronger '
                'support, safer preparation, or a ball that needs no setup. A retry being offered means '
                'preparation changed, not that capture is now safe or likely. Missing retreat_observation '
                'fields mean unobserved, not zero HP or confirmed failure of a specific tactic. '
                'Compare recorded_history across repeated retreats with the durable unlocks and new '
                'registrations offered by other candidates. Replenishing balls or gaining one level '
                'does not erase this history; prior spending is not a reason to keep spending. '
                'Past failures also do not prove a materially different setup will fail.')
            instruction += (' Capture support training is a bounded experience step, not a complete '
                'capture setup. Compare the remaining level gap and training_cost_to_observed_target_level '
                'with alternate supports, ball capabilities and their acquisition prerequisites. Level '
                'parity itself does not guarantee surviving an unfavorable matchup; a single gained '
                'level should not erase the observed failure evidence.')
            instruction += (' Compare item_evolution_spending_reference on ball purchases: '
                'spending may remove the ability to buy a stone for an unregistered evolution '
                'of a Pokémon actually held in the party or PC. An already carried stone needs '
                'no repurchase; a registered but unheld source is not a ready evolution input. '
                'These are independent alternatives sharing money, stones and source individuals, '
                'not a joint registration yield. Prices are references, not proof of shop access, '
                'and PC sources still require withdrawal. Compare this opportunity cost with the '
                'capture benefit and other goals; it is not a fixed cash reserve or a ban on shopping.')
        if layer == 'action' and 'local_state' in state and getattr(self, 'active', None):
            state = {**state, 'strategy_context': self.active.get('context', {})}
        context = state.get('strategy_context') or {}
        mechanism = layer == 'action' and bool(context.get('planned_effects'))
        if mechanism:
            instruction += (' Use the strategy context to interpret this step toward its parent goal. '
                'A coupled mechanism may require opposite switch values at different positions; '
                'completing its reachable next step can be progress even when it reverses an earlier flag value.')
        training = layer == 'action' and state.get('subgoal', [None])[0] == 'level'
        if layer == 'action' and state.get('subgoal', [None])[0] == 'move':
            instruction = ('Choose a compatible party member and a move to replace so the selected move '
                'is learned while retaining useful battle capability. Compare the actual move effects, '
                'power, type coverage and field utility. Prefer replacing a weak or redundant status '
                'move over a strong attack or the only attack of a useful type. Preserve high-critical-rate '
                'attacks and strong same-type attacks when alternatives exist. The goal includes the '
                'resulting moveset, not merely completing the teaching menu.')
        grounded = training and candidates and all(
            (json.loads(value).get('navigation') or {}).get('tile_route_found')
            for value in candidates.values()) and not self.needs_healing(state['local_state'])
        if grounded:
            instruction = ('Strategy has already selected training. Choose which reachable encounter site '
                'to use for the next normal battle, comparing travel, experience and type matchups. '
                'Earning experience is progress even against a lower-level opponent; one encounter '
                'need not reach the target level or defeat the later story opponent. Travel interruptions '
                'and actual outcomes will return to strategy.')
        mechanism_grounded = mechanism and candidates and any(
            route.get('tile_route_found') for route in context.get('trigger_navigation', []))
        battle = state.get('battle') or {}
        # A forced trainer battle cannot be left to resume travel/healing.
        # Ground this exception in live HP, PP and type matchups, not in
        # candidate descriptions or a blanket ban on battle abstention.
        trainer_switch_grounded = (layer == 'action' and battle.get('is_wild') is False
            and bool((battle.get('enemy') or {}).get('species')) and any(
                f'switch:{index}' in candidates and mon.get('hp', 0) > 0
                and mon.get('species') != (battle.get('player') or {}).get('species')
                and effective_attacks(mon, battle['enemy']['species'])
                for index, mon in enumerate(battle.get('player_party') or [])))
        if trainer_switch_grounded:
            state = {**state, 'immediate_goal': 'Finish the forced trainer battle, then resume the overworld objective.'}
            instruction += (' This is a trainer battle: neither running away nor capturing the opponent is legal. '
                'Travel, healing at a nurse and collection must wait until this battle ends. '
                'A conscious teammate with usable effective attacks is available among the offered switches. '
                'Choose the best offered turn toward defeating the trainer while preserving the party; '
                'switching to a capable finisher is progress even though it does not itself register a species '
                'or reach the nurse. Compare effective attacks, level and health rather than continuing '
                'to use an immune or depleted active battler.')
        forced_replacement = (layer == 'action'
            and str(state.get('battle_phase') or '').startswith('PlayerFaintSwitch')
            and any(str(index) in candidates and mon.get('hp', 0) > 0
                    for index, mon in enumerate(battle.get('player_party') or [])))
        if forced_replacement:
            state = {**state, 'immediate_goal': 'Replace the fainted active Pokémon to restore legal battle input.'}
            instruction += (' The native battle is waiting for a mandatory replacement. '
                'Choose an offered conscious member even if it has only status moves or is much weaker '
                'than the opponent. Refusing cannot open RUN, BAG or FIGHT; those legal turns become '
                'available only after replacement. This does not guarantee escape, victory or capture.')
        if layer == 'action' and any('"transit_leader"' in value for value in candidates.values()):
            instruction += (' Travel exposes the current party leader to incidental wild encounters before '
                'the destination interaction. Changing the leader is a valid preparation step, even though '
                'it does not move the player. Compare current_leader and transit_leader HP, level and moves '
                'against route_encounters. A low-level capture status supporter can stay in the party '
                'until the capture battle; it need not lead the entire trip. Prefer continuing travel '
                'when the current leader is already suitable, rather than swapping repeatedly. '
                'Encounter ranges are possibilities, not a forecast or a survival guarantee.')
        # Appended after the rewrites above: the menu and training instructions
        # replace the incoming text, and the bias must still reach the question.
        bias = PREFERENCE_INSTRUCTIONS.get(getattr(self, 'preference', 'none'))
        if bias and layer in ('strategy', 'action'):
            instruction += f' {bias}'
        if layer == 'strategy':
            if state.get('completed_route_prerequisite'):
                instruction += (' The player just completed the crossing described in '
                    'state.completed_route_prerequisite for its recorded parent goal and destination. '
                    'Prefer continuing that goal, or a reachable prerequisite at that destination, '
                    'before choosing unrelated travel back across the same passage. This is not '
                    'proof the destination is unlocked: compare current trigger navigation and '
                    'native guards. Urgent healing, capture resources, a newly observed blocker, '
                    'or an unavailable parent can justify changing goals.')
        # Action choices can be larger than strategic ones (e.g. every legal
        # inventory disposal/teaching operation). Keep their complete evidence
        # and options under the same lossless request representation.
        state, candidates = factor_strategy_evidence(state, candidates)
        if 'shared_strategy_evidence' in state:
            instruction += (' Repeated evidence is stored once in state.shared_strategy_evidence. '
                'Each object containing only shared_strategy_evidence_ref means the complete '
                'entry with that key in this library, including nested references. Resolve '
                'those references when comparing candidates; no candidate or evidence was omitted.')
        if ('"strategy_table"' in json.dumps(state)
                or any('"strategy_table"' in value for value in candidates.values())):
            instruction += (' An object containing only strategy_table represents a list of records: '
                'columns names the fields, and each rows entry supplies their values in that order. '
                'All original records and values are retained, including nested evidence references.')
        if layer == 'strategy':
            return self.choose_bounded_strategy(state, candidates, instruction,
                allow_abstain=not (grounded or mechanism_grounded))
        return self.choose_bounded_choice(layer, state, candidates, instruction,
            allow_abstain=not (grounded or mechanism_grounded or trainer_switch_grounded or forced_replacement))

    def choose_bounded_strategy(self, state, candidates, instruction, *, allow_abstain=True):
        return self.choose_bounded_choice('strategy', state, candidates, instruction,
                                          allow_abstain=allow_abstain)

    def choose_bounded_choice(self, layer, state, candidates, instruction, *, allow_abstain=True):
        """On explicit context overflow, compare every option in bounded rounds.

        No code-ranked shortlist: Jev chooses each disjoint group's representative
        with the same full state, then judges those representatives together.
        This is a tournament, not an identical full-set probability distribution.
        """
        try:
            return super().choose(layer, state, candidates, instruction,
                                  allow_abstain=allow_abstain)
        except StoryStopped as error:
            if (not isinstance(error.__cause__, TypeSafeError)
                    or 'max_tokens_exceeded' not in str(error.__cause__)
                    or len(candidates) <= 2):
                raise
        keys = list(candidates)
        midpoint = len(keys) // 2
        partitions = [keys[:midpoint], keys[midpoint:]]
        self.record(f'{layer}_partition', candidate_ids=keys, partitions=partitions,
                    reason='max_tokens_exceeded', state_preserved=True)
        local_instruction = instruction + (
            f' This is one disjoint comparison group from a larger {layer} choice. '
            'Choose the best relative next step in this group using the full unchanged state. '
            'A separate final comparison will judge the group representatives; '
            'select one representative even if this group has no ideal option.')
        winners = [self.choose_bounded_choice(layer, state, {key: candidates[key] for key in group},
                    local_instruction, allow_abstain=False) for group in partitions]
        self.record(f'{layer}_partition_finalists', candidate_ids=keys, finalists=winners)
        return self.choose_bounded_choice(layer, state, {key: candidates[key] for key in winners},
            instruction + ' These candidates are the model-selected representatives of '
            'disjoint comparison groups. Compare them for the overall next step.',
            allow_abstain=allow_abstain)

    def annotate_navigation(self, groups, facts, previews=None, *, prune=True):
        """A failed destination region does not block every NPC on its map."""
        self.navigation_facts = facts
        self.game.script_navigation_barriers = self.observed_navigation_barriers(facts)
        barriers = self.game.navigation_barriers()
        barriers[facts['map']] = barriers.get(facts['map'], set()) | self.game.live_npcs(facts['map'])
        excluded = self.game.navigation_excluded_maps()
        previews = {} if previews is None else previews
        rule_routes = {}
        for group in groups.values():
            routes = []
            for rule in group['rules']:
                if rule.storyline.startswith('skill:'):
                    if group['target'][0] in ('bag_space', 'move', 'health', 'pp_reserve'):
                        routes.append({'map': facts['map'], 'tile_route_found': True, 'steps': 0,
                                       'scope': 'available through the current inventory menu; no travel needed'})
                    elif group['target'][0] == 'level' and rule.map in getattr(self, 'training_navigation', {}):
                        routes.append(self.training_navigation[rule.map])
                    elif group['target'][0] == 'catch' and group['target'][1] in getattr(self, 'catch_navigation', {}):
                        routes.append(self.catch_navigation[group['target'][1]])
                    elif rule.storyline == 'skill:surf':
                        obstacle = group.get('context', {})
                        stance = obstacle.get('stance')
                        name = obstacle.get('map')
                        if name and stance and obstacle.get('move') == 'Surf':
                            prerequisites = (field_badge_prerequisites('Surf', facts.get('flags', {}))
                                + surf_current_prerequisites(obstacle, facts.get('flags', {})))
                            knows = any('Surf' in mon.get('moves', []) for mon in facts.get('party', []))
                            # This candidate executes Surf. Its immediate
                            # approach is the observed dry embarkation stance,
                            # not the distant landing it has yet to reach.
                            path = pt.bfs_cross(facts['map'], (facts['x'], facts['y']), name, tuple(stance),
                                last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                                blocked_maps=barriers, excluded_maps=excluded)
                            available = bool(path) and knows and not prerequisites
                            routes.append({'map': name, 'stance': stance, 'field_action': 'Surf',
                                'tile_route_found': available, 'steps': len(path)-1 if available else None,
                                'unmet_native_field_prerequisites': prerequisites,
                                'knows_required_move': knows,
                                'scope': 'Walk to the observed dry embarkation stance, then execute Surf; landing and onward travel remain uncompleted'})
                    continue
                points = self.destination_points(rule.map, rule)
                key = rule.map, tuple(points)
                if key not in previews:
                    found = None
                    requires_surf = False
                    def search():
                        return pt.bfs_cross(facts['map'], (facts['x'], facts['y']), rule.map, points[0],
                            last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                            blocked_maps=barriers, excluded_maps=excluded,
                            goal_nodes={(rule.map, *p) for p in points}) if points else None
                    path = search()
                    if not path and any('Surf' in m['moves'] for m in facts.get('party', [])):
                        with water_planning():
                            path = search()
                        requires_surf = bool(path)
                    native_prerequisites = surf_path_prerequisites(path, facts.get('flags', {})) if requires_surf else []
                    if path and not native_prerequisites:
                        found = len(path)-1
                    previews[key] = {'map': rule.map, 'tile_route_found': found is not None,
                                     'steps': found, 'requires_surf': requires_surf,
                                     'unmet_native_field_prerequisites': native_prerequisites,
                                     'scope': 'this trigger region, using known geometry and observed obstacles; available Surf can be used en route'}
                if previews[key] not in routes:
                    routes.append(previews[key])
                rule_routes[id(rule)] = previews[key]
            if routes:
                group['context'] = {**group.get('context', {}), 'trigger_navigation': routes}
        # Repeatedly selecting a route already disproved by real execution
        # adds no information while reachable prerequisites remain. Keep
        # untried regions available for exploration and retain all options
        # when no ordinary trigger has a known route.
        if prune and any(route['tile_route_found'] for group in groups.values() if not group.get('context', {}).get('optional_preparation')
               for route in group.get('context', {}).get('trigger_navigation', [])):
            for key, group in list(groups.items()):
                routes = group.get('context', {}).get('trigger_navigation', [])
                deferred = [rule for rule in group['rules']
                            if rule.map in self.navigation_memory
                            and rule_routes.get(id(rule), {}).get('tile_route_found') is False]
                if deferred:
                    group['rules'] = [rule for rule in group['rules'] if rule not in deferred]
                    group.setdefault('context', {})['deferred_trigger_maps'] = sorted({rule.map for rule in deferred})
                if not group['rules']:
                    del groups[key]
                    continue
                if (routes and not any(route['tile_route_found'] for route in routes)
                        and all(rule.map in self.navigation_memory for rule in group['rules'])):
                    del groups[key]
        return previews

    def transport_frontiers(self, groups, facts):
        """Backchain menu transport when walking cannot reach a target region."""
        targets = {}
        for group in groups.values():
            unreachable = {route['map'] for route in group.get('context', {}).get('trigger_navigation', [])
                           if not route['tile_route_found']}
            for rule in group['rules']:
                if rule.map in unreachable:
                    targets.setdefault(rule.map, set()).update(self.destination_points(rule.map, rule))
            for name in unreachable:
                targets.setdefault(name, set())
        if not targets:
            return False
        barriers = self.game.navigation_barriers()
        excluded = self.game.navigation_excluded_maps()
        goal_nodes = {(name, *point) for name, points in targets.items() for point in points}
        arrivals = {}
        sources = {}
        added = False
        def can_approach(rule):
            points = self.destination_points(rule.map, rule)
            key = rule.map, tuple(points)
            if key not in sources:
                def search():
                    return bool(points) and bool(pt.bfs_cross(
                        facts['map'], (facts['x'], facts['y']), rule.map, points[0],
                        last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                        blocked_maps=barriers, excluded_maps=excluded,
                        goal_nodes={(rule.map, *p) for p in points}))
                sources[key] = search()
                if not sources[key] and any('Surf' in m['moves'] for m in facts.get('party', [])):
                    with water_planning():
                        sources[key] = search()
            return sources[key]
        for transport in self.index.rules:
            if transport.effect[0] != 'transport':
                continue
            arrival = transport.effect[1]
            if arrival not in arrivals:
                name, x, y = arrival
                # A scripted entrance may land in a different map from the
                # goal (e.g. a park entrance). Verify the remaining walking
                # path from its actual landing tile, rather than naming a
                # special itinerary or assuming every transport is useful.
                relevant_nodes = ({node for node in goal_nodes if node[0] == name}
                                  if transport.map in pt.ELEVATOR_MAPS else goal_nodes)
                reachable = {name} if name in targets and not targets[name] else set()
                if name in pt.MAPS and relevant_nodes:
                    goal = next(iter(relevant_nodes))
                    reached_nodes = pt.bfs_cross(
                        name, (x, y), goal[0], goal[1:], last_map=self.game.last_map,
                        allow_ledges=True, allow_spinners=True,
                        blocked_maps=barriers, excluded_maps=excluded, goal_nodes=relevant_nodes,
                        reachable_goals=True)
                    reachable.update(node[0] for node in reached_nodes)
                arrivals[arrival] = reachable
            if not arrivals[arrival]:
                continue
            entrance_prerequisites = []
            if all(k in facts for k in ('map', 'x', 'y')) and not can_approach(transport):
                entrance_prerequisites = self.discover_route_prerequisites(
                    {'map_name': facts['map'], 'player_x': facts['x'], 'player_y': facts['y']},
                    transport.map, self.destination_points(transport.map, transport))
                # The destination benefit alone cannot make an inaccessible
                # shortcut executable. Offer only a reachable causal producer
                # of its entrance, never an assumed teleport or fixed route.
                frontier = [rule for target in entrance_prerequisites
                            for rule in self.index.frontier(target, facts) if can_approach(rule)]
            else:
                frontier = self.index.frontier(transport.effect, facts)
            for rule in frontier:
                key = json.dumps(rule.effect)
                if key not in groups:
                    added = True
                    groups[key] = {'target': rule.effect, 'rules': [],
                                   'objectives': ['Enter a region with a walking path to an inaccessible story target'],
                                   'context': {'transport_script': transport.description(),
                                               'transport_entrance_prerequisites': entrance_prerequisites,
                                               'blocked_destinations': sorted(arrivals[arrival]),
                                               'reachability_scope': 'Only destinations with a verified walking path from this landing; not every blocked world goal'}}
                if rule not in groups[key]['rules']:
                    groups[key]['rules'].append(rule)
        return added

    def add_cut_route_frontiers(self, groups, facts):
        """Keep the executable first tree when an old destination is deferred.

        Walking previews cannot cross a tree. Pruning that destination before
        calling travel used to also remove the only opportunity to discover
        its Cut prerequisite, even with Cut already learned. Probe planning
        geometry only; the skill still walks to the stance and uses the menu.
        """
        if (not any('Cut' in mon.get('moves', []) and mon.get('hp', 0) > 0
                    for mon in facts.get('party', []))
                or field_badge_prerequisites('Cut', facts.get('flags', {}))):
            return
        targets = {}
        for group in groups.values():
            unreachable = {route['map'] for route in
                group.get('context', {}).get('trigger_navigation', [])
                if not route['tile_route_found']}
            for rule in group['rules']:
                if (rule.map in unreachable and rule.map in self.navigation_memory
                        and not rule.storyline.startswith('skill:')):
                    points = tuple(self.destination_points(rule.map, rule))
                    if points:
                        targets.setdefault((rule.map, points), []).append(group['target'])
        if not targets:
            return
        barriers = {name: set(tiles) for name, tiles in self.game.navigation_barriers().items()}
        barriers.setdefault(facts['map'], set()).update(self.game.live_npcs(facts['map']))
        state = {'map_name': facts['map'], 'player_x': facts['x'], 'player_y': facts['y']}
        excluded = self.game.navigation_excluded_maps()
        for (name, points), goals in targets.items():
            obstacle = cut_requirement(state, name, points, self.game.last_map, barriers, excluded)
            if not obstacle:
                continue
            key = ','.join(map(str, [obstacle['map'], *obstacle['tree']]))
            target = ('terrain', key, True)
            group = groups.setdefault('field:' + key, {'target': target,
                'rules': [Rule('field:' + key, obstacle['map'], 'skill:field', [], [], [], target, [])],
                'objectives': ['Clear a reachable tree on a route to a deferred goal'],
                'context': {**obstacle,
                    'scope': 'Planning identifies a reachable Cut stance; real navigation and the field menu remain required.'}})
            prerequisites = group['context'].setdefault('prerequisite_for_goals', [])
            for goal in goals:
                if goal not in prerequisites:
                    prerequisites.append(goal)

    def action_rejected(self, facts, reason):
        if reason not in ('action:no_selection', 'action:no_candidates'):
            return False
        for rule in self.active['rules']:
            self.failures[attempt_key(rule, facts)] = 2
        self.recent.append({'subgoal': self.active['target'], 'result': reason})
        self.record('action_rejected', subgoal=self.active['target'], reason=reason)
        self.active = None
        return True

    def mechanism_plan(self, destination, points, facts):
        """Search reachable switch/transport states, preserving coupled doors.

        This plans only reversible flags governing tile replacements. Every
        returned edge is still a real script interaction for Jev to select.
        """
        if not isinstance(getattr(self.index, 'by_effect', None), dict):
            return None
        def flag_reads(node):
            if isinstance(node, list):
                return set().union(*(flag_reads(v) for v in node))
            if not isinstance(node, dict):
                return set()
            call = node.get('Call', {})
            if call.get('callee', '').removeprefix('game.') == 'getFlag':
                return {evaluate(call['args'][0], {})}
            return set().union(*(flag_reads(v) for v in node.values()))
        blocks = [r for r in self.index.rules if r.effect[0] == 'block' and 'load' in r.triggers]
        writable = {r.effect for r in self.index.rules if r.effect[0] == 'flag'
                    and r.triggers and 'load' not in r.triggers
                    and not any(e[0] == 'battle' for e in r.preceding)}
        reversible = {name for _, name, value in writable if ('flag', name, not value) in writable}
        def controlled_at(name):
            return set().union(*(flag_reads([g for g, _ in r.guards]) for r in blocks if r.map == name)) & reversible
        keys = controlled_at(facts['map']) or controlled_at(destination)
        if not keys or len(keys) > 3 or not points:
            return None
        maps = {r.map for r in blocks if flag_reads([g for g, _ in r.guards]) & keys}
        if facts['map'] not in maps:
            return None
        exits = {}
        if destination not in maps:
            for name in maps:
                for warp in pt.MAPS[name]['warps']:
                    for arrival in pt.warp_edges_from(name, warp['x'], warp['y'], self.game.last_map):
                        if arrival[0] not in maps:
                            outside, x, y = arrival
                            exits.setdefault(outside, set()).update(
                                (x+dx, y+dy) for dx, dy in pt.DELTA.values()
                                if pt.walkable(outside, x+dx, y+dy)
                                and (x+dx, y+dy) not in pt.warp_tiles(outside))
            exits = {name: points for name, points in exits.items() if points}
            if not exits:
                return None
        # Include every coupled door; never combine the open half of mutually
        # exclusive layouts into an imaginary all-open mansion.
        blocks = [r for r in blocks if r.map in maps]
        operations = [r for r in self.index.rules if r.map in maps and 'load' not in r.triggers
            and not any(e[0] == 'battle' for e in r.preceding)
            and ((r.effect[0] == 'flag' and r.effect[1] in keys)
                 or (r.effect[0] == 'transport' and r.effect[1][0] in maps))]
        keys = sorted(keys)
        original = {name: pt.MAPS[name]['blocks'] for name in maps}
        barriers = self.game.navigation_barriers()
        barriers.setdefault(facts['map'], set()).update(self.game.live_npcs(facts['map']))
        excluded = self.game.navigation_excluded_maps()
        queue = deque([((facts['map'], facts['x'], facts['y']), tuple(bool(facts['flags'].get(k)) for k in keys), [])])
        seen = set()
        def path(position, name, goals):
            return pt.bfs_cross(position[0], position[1:], name, goals[0], last_map=self.game.last_map,
                allow_ledges=True, allow_spinners=True, blocked_maps=barriers, excluded_maps=excluded,
                goal_nodes={(name, *p) for p in goals}) if goals else None
        try:
            while queue and len(seen) < 100:
                position, values, steps = queue.popleft()
                if (position, values) in seen:
                    continue
                seen.add((position, values))
                planned = {**facts, 'map': position[0], 'x': position[1], 'y': position[2],
                           'flags': {**facts['flags'], **dict(zip(keys, values))}}
                for name, geometry in original.items():
                    pt.MAPS[name]['blocks'] = list(geometry)
                for rule in blocks:
                    if rule.missing(planned):
                        continue
                    name, x, y = rule.effect[1].split(',')
                    pt.MAPS[name]['blocks'][int(y)*pt.MAPS[name]['width']+int(x)] = rule.effect[2]
                destinations = [(name, list(targets)) for name, targets in exits.items()] if exits else [(destination, points)]
                for name, targets in destinations:
                    route = path(position, name, targets)
                    if route:
                        endpoint = route[-1][0] if len(route) > 1 else route[0]
                        return {'steps': steps, 'flags': keys, 'maps': maps,
                                'exit': tuple(endpoint) if exits else None}
                for rule in operations:
                    if rule.missing(planned):
                        continue
                    if rule.effect[0] == 'flag' and planned['flags'].get(rule.effect[1]) == rule.effect[2]:
                        continue
                    route = path(position, rule.map, self.destination_points(rule.map, rule))
                    if not route:
                        continue
                    next_values = tuple(rule.effect[2] if k == rule.effect[1] else value
                                        for k, value in zip(keys, values)) if rule.effect[0] == 'flag' else values
                    endpoint = route[-1][0] if len(route) > 1 else route[0]
                    arrival = tuple(rule.effect[1]) if rule.effect[0] == 'transport' else tuple(endpoint)
                    queue.append((arrival, next_values, steps + [rule]))
        finally:
            for name, geometry in original.items():
                pt.MAPS[name]['blocks'] = geometry
        return None

    def add_mechanism_groups(self, groups, facts):
        pending = getattr(self, 'mechanism_goal', None)
        if pending and self.index.satisfied(pending, facts):
            self.mechanism_goal = pending = None
        goals = list(groups.values())
        if pending and not any(tuple(g['target']) == tuple(pending) for g in goals):
            goals.append({'target': pending, 'rules': self.index.frontier(pending, facts)})
        plans = []
        for group in goals:
            if group['target'][0] not in ('item', 'flag'):
                continue
            for goal in group['rules']:
                plan = self.mechanism_plan(goal.map, self.destination_points(goal.map, goal), facts)
                if not plan or group['target'][0] == 'flag' and group['target'][1] in plan['flags']:
                    continue
                plans.append((group, goal, plan))
        # A route outside is only preparation for its eventual objective. It
        # must not evict a reachable prerequisite inside this mechanism just
        # because the outer objective appeared first in the frontier.
        plans.sort(key=lambda row: (bool(row[2].get('exit')), row[0]['target'] != pending))
        for group, goal, plan in plans[:1]:
            # Completing a switch can close the route back outside and
            # change the ordinary frontier. Retain the parent item goal
            # while executing its verified mechanism sequence.
            self.mechanism_goal = group['target']
            for key, candidate in list(groups.items()):
                kind, name, _ = candidate['target']
                if ((kind == 'flag' and name in plan['flags']) or
                        (kind == 'block' and name.split(',')[0] in plan['maps']) or
                        (kind == 'transport' and name[0] in plan['maps']
                         and any(r.map in plan['maps'] for r in candidate['rules']))):
                    del groups[key]
            first = plan['steps'][0] if plan['steps'] else goal
            if not plan['steps'] and plan.get('exit'):
                name, x, y = plan['exit']
                first = Rule('mechanism_exit', name, 'skill:leave_mechanism', [f'coord:({x},{y})'],
                             [], [], ('transport', plan['exit'], True), [])
            key = json.dumps(first.effect)
            groups[key] = {'target': first.effect, 'rules': [first],
                'objectives': ['Advance a reachable sequence of coupled switches and scripted passages'],
                'context': {'towards': group['target'], 'planned_effects': [r.effect for r in plan['steps']],
                            'scope': 'planning only; reobserve after each real interaction'}}
            return

    def fight_battle(self):
        self.game.battle_loop(prefer='fight', max_iters=1200)

    def observe_battle_result(self, before, after):
        phase = after.get('battle_phase', '')
        live = before.get('battle_live') or {}
        enemy = live.get('enemy') or {}
        captured_species = capture_species(enemy) if enemy else None
        if (getattr(self, 'collects_dex', False) and before.get('script_awaiting_battle') and live.get('is_wild')
                and 'escaped: true' in phase and captured_species not in
                (after.get('pokedex') or {}).get('owned_species', [])):
            party = [{**base, **mon} for base, mon in zip(before.get('party', []), live.get('player_party', []))]
            preparation = capture_preparation(party, {row['item']: row['qty']
                for row in before.get('battle_inventory', [])}, before)
            key = before['map_name'] + ':' + captured_species
            evidence = {'map': before['map_name'], 'species': captured_species,
                        'preparation': preparation,
                        'start_inventory_observed': isinstance(before.get('battle_inventory'), list),
                        'reason': 'native_menu_escape_without_registration'}
            result_live = after.get('battle_live') or {}
            evidence['retreat_observation'] = {
                'enemy': result_live.get('enemy'),
                'party': result_live.get('player_party') or after.get('party'),
                'inventory': after.get('battle_inventory'),
                'scope': 'Native observations at successful menu escape; not a damage forecast or proof of which move caused a faint.'}
            self.capture_retreats[key] = evidence
            if not hasattr(self, 'capture_retreat_totals'):
                self.capture_retreat_totals = {}
            accumulate_capture_retreat(self.capture_retreat_totals, evidence)
            self.record('capture_retreat', **evidence)
        opponents = (before.get('battle_live') or {}).get('enemy_party', [])
        signature = lambda team: [(m['species'], m.get('level')) for m in team]
        if 'player_won: true' in phase or 'won: true' in phase:
            for defeat in self.battle_defeats:
                if defeat['map'] == before['map_name'] and signature(defeat['opponents']) == signature(opponents):
                    defeat['resolved_by_victory'] = True
            self.defeat_preparation = max((d['proposed_training_level'] for d in self.battle_defeats
                                           if not d.get('resolved_by_victory')), default=0)
        lost = ('player_won: false' in phase
                or 'won: false' in phase and 'escaped: false' in phase)
        if not lost or not after.get('party'):
            return
        level = after['party'][0]['level']
        self.defeat_preparation = min(100, max(self.defeat_preparation, level+2))
        evidence = {'map': before['map_name'], 'party_level': level,
                    'opponents': opponents,
                    'preparation_signature': battle_readiness(before['party'],
                        {row['item'].replace('_', '').upper(): row['qty'] for row in before.get('battle_inventory', [])}) if before.get('party') else None,
                    'proposed_training_level': self.defeat_preparation}
        self.battle_defeats.append(evidence)
        self.replan_after_defeat = True
        self.record('battle_defeat', **evidence)

    def objective_satisfied(self, objective, facts):
        if objective['id'] == 'become-champion':
            return self.first_clear_verification is not None
        if objective['id'] == 'collect-dex':
            return self.dex_complete(facts)
        if objective['id'] == 'max-coverage':
            return self.coverage_complete(facts)
        return super().objective_satisfied(objective, facts)

    def coverage_complete(self, facts):
        """No map bordering the explored region is still unvisited."""
        if not self.visited:
            return False
        bordering = set()
        for name in self.visited:
            map_data = self.maps.get(name, {})
            bordering.update(c['targetMap'] for c in map_data.get('connections', {}).values())
            bordering.update(w['destMap'] for w in map_data.get('warps', []) if w.get('destMap'))
        return not (bordering - self.visited)

    def validated_owned(self, facts):
        return set((facts.get('dex') or {}).get('owned_species', [])) - set(
            getattr(self, 'collection_audit_pending', {}))

    def observe_audit_evolution(self, party, frame=None):
        """Accept a native party species replacement, not an existing invalid copy.

        Compare species multisets, not slots: battle switching can reorder the
        party. Require one source replacement, the active evolution objective,
        and a new target level not explained by an existing invalid copy.
        """
        current = [dict(mon) for mon in party]
        previous = getattr(self, '_audit_party', None)
        self._audit_party = current
        if not previous or len(previous) != len(current):
            return
        context = (getattr(self, 'active', None) or {}).get('context', {})
        if context.get('acquisition_method') != 'evolution':
            return
        normalize = lambda name: str(name).replace('_', '').upper()
        source, target_name = normalize(context.get('from_species')), normalize(context.get('species'))
        before_species = Counter(normalize(mon['species']) for mon in previous)
        after_species = Counter(normalize(mon['species']) for mon in current)
        if (before_species - after_species != Counter({source: 1})
                or after_species - before_species != Counter({target_name: 1})):
            return
        before_levels = Counter((normalize(mon['species']), mon['level']) for mon in previous)
        after_levels = Counter((normalize(mon['species']), mon['level']) for mon in current)
        removed = [key for key, count in (before_levels - after_levels).items()
                   for _ in range(count) if key[0] == source]
        added = [key for key, count in (after_levels - before_levels).items()
                 for _ in range(count) if key[0] == target_name]
        if len(removed) != 1 or len(added) != 1:
            return  # Ambiguous evidence cannot clear a pending source audit.
        old = next(mon for mon in previous if (normalize(mon['species']), mon['level']) == removed[0])
        new = next(mon for mon in current if (normalize(mon['species']), mon['level']) == added[0])
        for target in list(getattr(self, 'collection_audit_pending', {})):
            if not self.same_species(new['species'], target):
                continue
            for edge in self.complete_collection_graph().get(target, []):
                if (edge['method'] == 'evolution' and edge.get('trigger') == 'level'
                        and self.same_species(old['species'], edge['from_species'])
                        and new['level'] > old['level']
                        and new['level'] >= edge['level']):
                    evidence = self.collection_audit_pending.pop(target)
                    self.record('collection_audit_resolved', species=target,
                        acquisition_method='evolution', before=old, after=new,
                        invalid_acquisition=evidence, frame=frame)
                    break

    def dex_complete(self, facts):
        """Every species reachable under the Red solo/no-link policy is registered.

        Scoping this to already-explored areas made it vacuously true at spawn:
        the starting room has no encounter table, so "nothing unregistered here"
        held and the run reported success in six seconds without acting.
        """
        if getattr(self, 'index', None) is None:
            return False
        owned = self.validated_owned(facts)
        if not owned:
            return False
        plan = solo_plan(self.complete_collection_graph(), owned, infer_solo_choices(owned))
        return set(plan['reachable_species']) <= owned

    def require_static_sources(self, facts):
        """Stop promptly when every indexed source of a missing species is spent.

        Keep 124 as the requested target; do not lower it to match lost sources.
        Unknown/unindexed paths are not classified as permanently exhausted.
        """
        rules = getattr(getattr(self, 'index', None), 'rules', None)
        if not isinstance(rules, list):
            return
        owned = self.validated_owned(facts)
        losses = []
        for species, methods in self.complete_collection_graph().items():
            if species in owned:
                continue
            methods = [method for method in methods if not method.get('external_trade')
                       and method['method'] != 'unavailable']
            if not methods or any(method['method'] != 'static' for method in methods):
                continue
            sources = []
            for method in methods:
                candidates = [rule for rule in rules if rule.map == method['map']
                    and rule.storyline == method['map'] + ':' + method['storyline']
                    and rule.effect[0] == 'battle' and self.same_species(rule.effect[1], species)]
                if not candidates:
                    break  # Missing semantics never proves permanent loss.
                flags = [spent_static_source(rule, rules, facts) for rule in candidates]
                if not all(flags):
                    break
                sources.append({'map': method['map'], 'script': method['storyline'],
                                'completion_flags': sorted({flag for row in flags for flag in row})})
            else:
                losses.append({'species': species, 'spent_sources': sources})
        if losses:
            self.record('finite_collection_source_lost', losses=losses,
                        scope='All catalogued solo methods are static and blocked by observed monotone completion flags')
            raise StoryStopped('finite_collection_source_lost:' + ','.join(row['species'] for row in losses))

    def collection_graph(self):
        graph = getattr(self, '_collection_graph', None)
        if graph is None:
            graph = acquisition_graph(self.maps, (name for name in self.maps if fishing_spots(name)))
            self._collection_graph = graph
        return graph

    def complete_collection_graph(self):
        graph = getattr(self, '_complete_collection_graph', None)
        if graph is None:
            graph = complete_acquisition_graph(
                self.maps, (name for name in self.maps if fishing_spots(name)))
            self._complete_collection_graph = graph
        return graph

    @staticmethod
    def same_species(left, right):
        return str(left).replace('_', '').upper() == str(right).replace('_', '').upper()

    def static_capture_deferred(self, species, name, facts):
        previous = next((row for row in getattr(self, 'capture_retreats', {}).values()
                         if row['map'] == name and self.same_species(row['species'], species)), None)
        return bool(previous and not capture_preparation_improvements(
            capture_preparation(facts.get('party', []), facts.get('bag', {}), facts), previous['preparation']))

    def acquisition_story_rules(self, species, method):
        """Resolve one graph edge back to the exact executable scene rules."""
        if method['method'] == 'npc_trade':
            return list(self.index.by_effect.get(
                ('flag', method['completion_flag'], True), []))
        expected = 'battle' if method['method'] == 'static' else 'pokemon'
        suffix = ':' + method.get('storyline', '')
        return [rule for rule in self.index.rules
                if rule.map == method.get('map')
                and (not suffix or rule.storyline.endswith(suffix))
                and rule.effect[0] == expected
                and self.same_species(rule.effect[1], species)]

    def add_evolution_item_source(self, groups, facts, item, species):
        """Expose either a real pickup or shop purchase for a needed stone."""
        key = item.replace('_', '').upper()
        for effect in self.index.by_effect:
            if effect[0] != 'item' or not effect[2] or effect[1].replace('_', '').upper() != key:
                continue
            rules = self.index.frontier(effect, facts)
            if rules:
                groups[f'evolution-item:{species}:{item}'] = {
                    'target': effect, 'rules': rules,
                    'objectives': [f'Obtain {item} to evolve a held Pokémon into {species}'],
                    'context': {'acquisition_method': 'evolution', 'required_item': item,
                                'evolution_target': species}}
                return
        canonical = next((name for name in ITEM_CATALOG if name.replace('_', '').upper() == key), item)
        info = ITEM_CATALOG.get(canonical)
        if not info or not info.get('price') or facts.get('money', 0) < info['price']:
            return
        for rule in self.index.rules:
            if rule.effect[0] != 'shop' or rule.missing(facts):
                continue
            for stock_index, stock in enumerate(rule.effect[1]):
                if stock.replace('_', '').upper() != key:
                    continue
                target = ('supply', canonical, 1)
                groups[f'evolution-shop:{rule.id}:{species}'] = {
                    'target': target, 'rules': [rule],
                    'objectives': [f'Buy {canonical} to evolve a held Pokémon into {species}'],
                    'context': {'stock_index': stock_index, 'item': info, 'quantity_to_buy': 1,
                                'total_cost': info['price'], 'acquisition_method': 'evolution',
                                'evolution_target': species}}

    def add_nonwild_collection_groups(self, groups, facts):
        """Add only currently executable non-wild edges from the solo DAG."""
        if not self.collects_dex or facts.get('dex') is None:
            return
        owned = self.validated_owned(facts)
        plan = solo_plan(self.complete_collection_graph(), owned, infer_solo_choices(owned))
        reachable = set(plan['choice_reachable_species'])
        party = facts.get('party', [])
        held = [*party, *facts.get('stored_pokemon', [])]
        missing_sources = {}
        for species in sorted(reachable - owned):
            for method in self.complete_collection_graph().get(species, []):
                if method.get('external_trade') or method['method'] in (
                        'unavailable', 'grass', 'water', 'safari', 'fishing', 'version_trade'):
                    continue
                group = method.get('exclusive_group')
                if group and method.get('choice') not in plan['optimal_choices'].get(group, ()):
                    continue
                source = method.get('from_species')
                source_party = [i for i, mon in enumerate(party)
                                if source and self.same_species(mon.get('species'), source)]
                source_held = any(source and self.same_species(mon.get('species'), source)
                                  for mon in held)
                context = {'purpose': f'Register {species} through a deterministic non-wild source',
                           'species': species, 'acquisition_method': method['method'],
                           'acquisition_contract': acquisition_contract(species, method), **method}
                rules = []
                if method['method'] == 'evolution':
                    if not source_held:
                        if source in owned:
                            missing_sources.setdefault(source, set()).add(species)
                        continue
                    if not source_party:
                        self.add_storage_retrieval(groups, facts, source, species, method)
                        continue
                    if method['trigger'] == 'item':
                        item = method['item']
                        if not facts['bag'].get(item.replace('_', '').upper(), 0):
                            self.add_evolution_item_source(groups, facts, item, species)
                            continue
                    context['party_indices'] = source_party
                    if method['trigger'] == 'level':
                        trainee = party[source_party[0]]
                        if trainee['level'] >= 100:
                            continue  # Normal battle XP cannot trigger another level.
                        trigger_level = max(method['level'], trainee['level'] + 1)
                        context['experience_trigger_level'] = trigger_level
                        context['evolution_trigger_scope'] = (
                            'Level evolution is checked on a new level gain, not merely being above '
                            'the natural threshold. An already overlevel wild capture still needs '
                            'another real level-up; the threshold is not a zero-cost evolution action.')
                        context['training_cost'] = evolution_training_cost(trainee, trigger_level)
                        participants = 2 if any(mon.get('hp', 0) > 0 and mon['level'] > trainee['level']
                                                for mon in party) else 1
                        examples = []
                        for name in getattr(self, 'visited', ()):
                            if name.startswith('SafariZone'):
                                continue  # Capture-only encounters are not training victories.
                            table = ((getattr(self, 'maps', {}).get(name, {}).get('wild') or {})
                                     .get('red') or {}).get('grass')
                            effort = evolution_training_effort(trainee, trigger_level, table, participants)
                            if effort:
                                examples.append({'map': name, **effort,
                                    'navigation': getattr(self, 'training_navigation', {}).get(name),
                                    'access_scope': 'Previously visited table; a current tile route must still be verified'})
                        context['training_effort_examples'] = sorted(examples,
                            key=lambda example: example['estimated_victories_max'])[:3]
                        context['alternative_sources'] = [
                            {'method': alternative['method'], 'map': alternative.get('map'),
                             'visited': alternative.get('map') in getattr(self, 'visited', ()),
                             'scope': 'Potential source only; navigation and prerequisites still require verification'}
                            for alternative in self.complete_collection_graph().get(species, [])
                            if alternative['method'] in ('grass', 'water', 'fishing', 'safari')]
                    rules = [Rule(f'evolve:{source}:{species}', facts['map'],
                                  'skill:evolve', [], [], [],
                                  ('register', species, True), [])]
                elif method['method'] == 'npc_trade':
                    if facts['flags'].get(method['completion_flag']):
                        continue
                    if not source_held:
                        if source in owned:
                            missing_sources.setdefault(source, set()).add(species)
                        continue
                    if len(party) < 2:
                        continue
                    if not source_party:
                        self.add_storage_retrieval(groups, facts, source, species, method)
                        continue
                    rules = self.acquisition_story_rules(species, method)
                elif method['method'] == 'prize':
                    if facts.get('coins', 0) < method['coins']:
                        self.add_coin_source(groups, facts, method['coins'], species)
                        continue
                    if len(party) >= 6:
                        if self.acquisition_capacity_ready(facts, species, method):
                            self.add_party_space_group(groups, facts, species)
                        continue
                    rules = self.acquisition_story_rules(species, method)
                else:
                    if method['method'] == 'static' and self.static_capture_deferred(species, method['map'], facts):
                        continue  # Reopen after actual preparation improves, not map travel alone.
                    if method['method'] == 'static' and self.capture_resources_missing(facts, 'static'):
                        self.add_box_capacity_group(groups, facts)
                        continue
                    if method['method'] == 'gift' and len(party) >= 6:
                        if self.acquisition_capacity_ready(facts, species, method):
                            self.add_party_space_group(groups, facts, species)
                        continue
                    rules = self.acquisition_story_rules(species, method)
                    if method['method'] == 'static':
                        resources = self.collection_resources(facts)
                        ready_sources = sorted({candidate.get('map') for candidate in
                            self.complete_collection_graph().get(species, [])
                            if candidate['method'] == 'static' and any(not rule.missing(facts)
                                for rule in self.acquisition_story_rules(species, candidate))})
                        context.update(
                            purpose=f'Register {species} through a finite static encounter; capture is stochastic',
                            collection_resources=resources,
                            capture_inventory_risk=capture_inventory_risk(species, resources['ball_inventory']),
                            ready_static_source_maps=ready_sources,
                            last_currently_ready_static_source=len(ready_sources) == 1,
                            retreat_contracts=[static_retreat_contract(rule, self.index.rules) for rule in rules],
                            failure_warning='Running out of balls, fleeing or knocking out a static target can permanently spend this source. Preparation and extra supplies must be compared before triggering it; a ready script does not guarantee capture.')
                rules = [rule for rule in rules if not rule.missing(facts)]
                if not rules:
                    continue
                key = f"register:{species}:{method['method']}:{method.get('map', source or '')}"
                groups[key] = {'target': ('register', species, True), 'rules': rules,
                               'objectives': [f'Register {species} in the solo Pokédex'],
                               'context': context}
        self.add_source_reacquisition(groups, facts, missing_sources)

    def add_source_reacquisition(self, groups, facts, missing_sources):
        """Registered is not held: catch a replacement consumed by an edge."""
        if not missing_sources:
            return
        counts = facts.get('box_counts', [])
        if (len(facts.get('party', [])) >= 6 and counts
                and counts[facts.get('current_box_index', 0)] >= 20):
            return
        areas = dict(getattr(self, 'catch_areas', {}))
        navigation = dict(getattr(self, 'catch_navigation', {}))
        for source, targets in sorted(missing_sources.items()):
            found = self.find_catch_areas(facts, requested_species=(source,))
            for old_key, area in found.items():
                if area.get('method', 'grass') != 'safari' and not self.balls_held(facts):
                    continue
                key = f'source:{source}:{old_key}'
                areas[key] = {**area, 'key': key}
                navigation[key] = area.get('navigation') or self.catch_navigation.get(old_key)
                target = ('held_species', source, True)
                groups[key] = {'target': target,
                    'rules': [Rule(key, area['map'], 'skill:catch_encounter', [], [], [], target, [])],
                    'objectives': [f'Catch another {source} needed to register {name}' for name in sorted(targets)],
                    'context': {'acquisition_method': area['method'], 'catch_area': key,
                                'required_capture_species': source, 'required_for': sorted(targets),
                                'already_registered_but_not_held': True,
                                'balls_held': self.balls_held(facts),
                                'encounter_value': area.get('encounter_value'),
                                'trigger_navigation': [navigation[key]] if navigation[key] else []}}
        self.catch_areas, self.catch_navigation = areas, navigation

    def acquisition_capacity_ready(self, facts, species, method):
        """Do not churn party slots for a pickup beyond the explored frontier."""
        if method.get('map') not in {facts['map'], *getattr(self, 'visited', {})}:
            return False
        return any(not rule.missing(facts)
                   for rule in self.acquisition_story_rules(species, method))

    @staticmethod
    def post_withdrawal_acquisition(stored, target, method, facts):
        """Price the follow-up, not the PC action, using this actual box slot.

        A retrieval prerequisite otherwise hides the difference between an
        available stone, many real level gains, and giving away the source.
        These are alternatives for one individual, never cumulative rewards.
        """
        preview = {'species': target, 'acquisition_method': method['method'],
                   'withdrawal_registers_target': False,
                   'acquisition_contract': acquisition_contract(target, method)}
        if method['method'] == 'evolution':
            if method['trigger'] == 'level':
                can_level = stored['level'] < 100
                trigger = max(method['level'], stored['level'] + 1) if can_level else None
                preview.update(level_up_possible=can_level, experience_trigger_level=trigger)
                if can_level:
                    preview['training_cost'] = evolution_training_cost(stored, trigger)
            elif method['trigger'] == 'item':
                item = method['item']
                key = item.replace('_', '').upper()
                quantity = sum(count for name, count in facts.get('bag', {}).items()
                               if name.replace('_', '').upper() == key)
                info = next((info for name, info in ITEM_CATALOG.items()
                             if name.replace('_', '').upper() == key), {})
                preview.update(required_item=item, item_quantity_held=quantity,
                               item_missing=quantity < 1,
                               item_unit_price_reference=info.get('price') or None)
        elif method['method'] == 'npc_trade':
            if method.get('completion_flag'):
                preview['trade_already_completed'] = bool(facts.get('flags', {}).get(method['completion_flag']))
            preview['party_count_requirement_after_withdrawal_met'] = len(facts.get('party', [])) >= 1
        return preview

    def add_storage_retrieval(self, groups, facts, source, target, method=None):
        stored = next((mon for mon in facts.get('stored_pokemon', [])
                       if self.same_species(mon.get('species'), source)), None)
        if not stored:
            return
        rules = [rule for rule in self.index.by_effect.get(('pc', 'storage', True), [])
                 if not rule.missing(facts)]
        if not rules:
            return
        key = f'retrieve:{source}'
        entry = groups.setdefault(key, {
            'target': ('pokemon', source, None), 'rules': rules,
            'objectives': [],
            'context': {'storage_retrieval': True, 'stored_pokemon': stored,
                        'requires_party_deposit': len(facts.get('party', [])) >= 6,
                        'stored_pokemon_not_fully_healthy': (
                            stored.get('hp', 0) < stored.get('max_hp', 0)
                            or stored.get('status', 'None') != 'None'),
                        'required_for': []}})
        objective = f'Withdraw {source} from storage so it can produce {target}'
        if objective not in entry['objectives']:
            entry['objectives'].append(objective)
        if target not in entry['context']['required_for']:
            entry['context']['required_for'].append(target)
        if method is not None:
            context = entry['context']
            preview = self.post_withdrawal_acquisition(stored, target, method, facts)
            options = context.setdefault('post_withdrawal_acquisitions', [])
            if preview not in options:
                options.append(preview)
            context['post_withdrawal_options_share_one_individual'] = True
            context['post_withdrawal_scope'] = (
                'Possible follow-ups, not rewards of withdrawal or a guaranteed combined yield. '
                'Evolution changes this individual; NPC trade gives it away. Travel, access, '
                'any needed recovery and menu execution still need planning. Recovery is not '
                'required by every alternative; item price does not prove an accessible seller.')

    def add_stored_battler_retrieval(self, groups, facts):
        """Recover an earned main battler rather than train a replacement."""
        stored = facts.get('stored_pokemon', [])
        if not stored or not facts.get('party'):
            return
        main = max(stored, key=lambda mon: mon.get('level', 0))
        current_level = max(mon['level'] for mon in facts['party'])
        if main.get('level', 0) < max(current_level + 5, current_level * 1.5):
            return
        self.add_storage_retrieval(groups, facts, main['species'], 'story battle readiness')
        entry = groups.get(f'retrieve:{main["species"]}')
        if entry:
            entry['context']['restore_main_battler'] = True
            entry['objectives'] = [f'Restore the already trained {main["species"]} to the party before further training or battles']

    def add_box_capacity_group(self, groups, facts):
        counts = facts.get('box_counts', [])
        current = facts.get('current_box_index', 0)
        if not counts or counts[current] < 20:
            return False
        available = [index for index, count in enumerate(counts) if count < 20]
        rules = [rule for rule in self.index.by_effect.get(('pc', 'storage', True), [])
                 if not rule.missing(facts)]
        if available and rules:
            target = ('box_space', 'storage', True)
            groups['storage:change_box'] = {
                'target': target, 'rules': rules,
                'objectives': ['Change to a PC box with room before catching more Pokémon'],
                'context': {'storage_change_box': True, 'available_boxes': available,
                            'box_counts': counts}}
        return True

    def add_party_space_group(self, groups, facts, species):
        counts = facts.get('box_counts', [])
        current = facts.get('current_box_index', 0)
        if counts and counts[current] >= 20:
            self.add_box_capacity_group(groups, facts)
            return
        rules = [rule for rule in self.index.by_effect.get(('pc', 'storage', True), [])
                 if not rule.missing(facts)]
        if not rules:
            return
        entry = groups.setdefault('storage:party_space', {
            'target': ('party_space', 'party', True), 'rules': rules,
            'objectives': [], 'context': {'storage_party_space': True, 'required_for': []}})
        objective = f'Deposit one party member so the {species} acquisition can succeed'
        if objective not in entry['objectives']:
            entry['objectives'].append(objective)
        if species not in entry['context']['required_for']:
            entry['context']['required_for'].append(species)

    def add_coin_source(self, groups, facts, required, species):
        if not facts['bag'].get('COINCASE', 0):
            for effect in self.index.by_effect:
                if effect[0] != 'item' or not effect[2] or effect[1].replace('_', '').upper() != 'COINCASE':
                    continue
                rules = self.index.frontier(effect, facts)
                if rules:
                    groups['coins:case'] = {
                        'target': effect, 'rules': rules,
                        'objectives': [f'Obtain the Coin Case required to redeem {species}'],
                        'context': {'acquisition_method': 'prize', 'required_coins': required,
                                    'prize_species': species}}
                    return
        # The clerk is the deterministic, bounded source: ¥1000 -> 50 coins.
        rules = [rule for rule in self.index.rules
                 if rule.map == 'GameCorner' and rule.effect == ('coins', 50, True)
                 and rule.storyline.endswith(':talkClerk1') and not rule.missing(facts)]
        purchases = (required - facts.get('coins', 0) + 49) // 50
        if not rules or facts.get('money', 0) < purchases * 1000:
            return
        target = ('coin_supply', 'coins', required)
        key = f'coins:{required}'
        entry = groups.setdefault(key, {'target': target, 'rules': rules, 'objectives': [],
            'context': {'coin_purchase': True, 'required_coins': required,
                        'current_coins': facts.get('coins', 0), 'purchases': purchases,
                        'money_cost': purchases * 1000, 'prize_species': []}})
        objective = f'Buy enough Game Corner coins to redeem {species}'
        if objective not in entry['objectives']:
            entry['objectives'].append(objective)
        if species not in entry['context']['prize_species']:
            entry['context']['prize_species'].append(species)

    def settle_special(self, state):
        if (state.get('choice') and self.active
                and self.active.get('context', {}).get('coin_purchase')):
            menu = state['choice']
            yes = next((index for index, label in enumerate(menu['options'])
                        if str(label).lower() in ('yes', '是')), 0)
            self.tap('a' if menu['selected'] == yes else 'down')
            return True
        if state.get('evolution_phase'):
            # Evolution is the only cutscene where B can destroy progress.
            # A advances its optional intro text and is harmless during the
            # timed morph phases; never route this through skip_dialogue().
            self.tap('a')
            return True
        if state.get('npc_trade_phase'):
            self.client.step(10)
            return True
        if state.get('shop_phase') and self.active and self.active['target'][0] == 'sale':
            item = self.active['target'][1]
            money = state['money']
            data.sell(self.game, item)
            if self.client.state()['money'] <= money:
                raise StoryStopped('sale_did_not_increase_money')
            self.record('sold_treasure', item=item, money_after=self.client.state()['money'])
            return True
        if state.get('shop_phase') and self.active and self.active['target'][0] == 'supply':
            details = self.active['context']
            item = self.active['target'][1]
            current = sum(row['qty'] for row in self.client.cmd(cmd='get_bag') if row['item'] == item)
            quantity = self.active['target'][2] - current
            if quantity > 0:
                data.buy(self.game, item, details['stock_index'], quantity)
                self.record('purchased_supply', item=item, quantity=quantity)
            else:
                self.tap('b')
            return True
        if not state.get('hof_phase') and not state.get('credits_phase'):
            return False
        phases = []
        seen_hof = seen_credits = final_button = False
        expected = self.hof_baseline + 1
        for _ in range(1200):
            self.check_budget()
            state = self.client.state()
            seen_hof |= bool(state.get('hof_phase'))
            seen_credits |= bool(state.get('credits_phase'))
            signature = [state['screen'], state.get('hof_phase'), state.get('credits_phase')]
            if not phases or phases[-1]['phase'] != signature:
                phases.append({'phase': signature, 'frame': state['frame_count'],
                               'hall_of_fame_count': state['hall_of_fame_count']})
            if state.get('credits_final_button'):
                if state['hall_of_fame_count'] != expected:
                    raise StoryStopped('ending:missing_hall_of_fame_team')
                final_button = True
                self.tap('a')
            elif final_button and not state.get('credits_phase') and state['screen'] != 'overworld':
                break
            elif state.get('dialogue_state'):
                self.client.skip_dialogue()
            elif state['screen'] == 'battle':
                raise StoryStopped('ending:unexpected_battle')
            else:
                self.client.step(120)
        else:
            raise StoryStopped('ending:did_not_finish')
        if not (seen_hof and seen_credits and final_button):
            raise StoryStopped('ending:missing_phase_evidence')
        saved = self.game.save_path
        if not saved.exists() or saved.stat().st_size != 32768:
            raise StoryStopped('ending:autosave_missing')
        # This is a generic CONTINUE skill in a separate process. No debug
        # save, milestone callback or state seeding can manufacture the proof.
        digest = hashlib.sha256(saved.read_bytes()).hexdigest()
        check = pt.Game(save_path=saved, binary=self.game.binary,
                        seed=self.game.seed, speed=0)
        check.d = ObservedProtocol(check.d, self.record, time.monotonic()+120)
        try:
            pt.resume_reentry(check)
            restored = check.st()
            if not (restored['map_name'] == 'PalletTown' and restored['badges'] == 255
                    and restored['hall_of_fame_count'] == expected):
                raise StoryStopped('ending:continue_verification_failed')
            self.first_clear_verification = {
                'phases': phases, 'autosave_sha256': digest,
                'separate_process_continue': restored, 'verification_commands': check.d.counts,
            }
        finally:
            check.close()
        pt.resume_reentry(self.game)
        self.hof_baseline = expected
        self.record('first_clear_verified', verification=self.first_clear_verification)
        return True

    def record_travel(self, result):
        legs = result.get('legs', [])
        count = result.get('legs_completed', len(legs) if result.get('result') == 'reached' else 0)
        for leg in legs[:count]:
            self.visited.update((leg['from_map'], leg['to_map']))
        self.visited.add(self.client.state()['map_name'])

    def remember_travel_result(self, destination, result):
        if result['result'] == 'blocked':
            state = self.client.state()
            blocked_map = result.get('blockage_map', state['map_name'])
            self.navigation_blockage = {
                'map': blocked_map,
                'position': result.get('blockage_position', [state['player_x'], state['player_y']]),
                'destination': destination, 'goal': self.active['target'], 'detail': result.get('detail'),
                'blocking_trainers': result.get('blocking_trainers', []),
                'blocking_npcs': result.get('blocking_npcs', [])}
            self.navigation_memory[destination] = dict(self.navigation_blockage)
            self.navigation_history[json.dumps([destination, blocked_map])] = dict(self.navigation_blockage)
            self.observed_barrier_maps.add(blocked_map)
        elif result['result'] == 'reached':
            self.navigation_memory.pop(destination, None)

    def opponent_parties(self, rules, facts=None):
        parties = []
        seen = set()
        starter = infer_solo_choices(((facts or {}).get('dex') or {}).get('owned_species', [])).get('starter')
        for rule in rules:
            for effect in rule.preceding:
                if effect[0] != 'battle':
                    continue
                reference = effect[1]
                base = 0
                if isinstance(reference, (tuple, list)):
                    reference, base = reference
                if not isinstance(reference, str):
                    continue
                selected = None
                if reference.startswith('OPP_'):
                    name = reference[4:]
                    # Longest match keeps the digit in RIVAL1/2/3 part of the
                    # class; an optional suffix is a one-based trainer set.
                    for klass, trainer in sorted(self.trainers.items(),
                            key=lambda item: -len(item[1].get('constName', item[0].upper()))):
                        prefix = trainer.get('constName', klass.upper())
                        suffix = name.removeprefix(prefix)
                        if name.startswith(prefix) and (not suffix or suffix.isdigit()):
                            selected = (klass, max(0, int(suffix or 1) - 1))
                            break
                    if selected and selected[0] in ('Rival1', 'Rival2', 'Rival3'):
                        # Native starter-advantage selection uses the original
                        # starter, not the current lead or NPC metadata.
                        offset = {'Charmander': 0, 'Squirtle': 1, 'Bulbasaur': 2}.get(starter)
                        if (offset is None or not isinstance(base, (int, float))
                                or base < 0 or int(base) != base):
                            continue  # Unknown inputs must not invent a roster.
                        selected = (selected[0], int(base) + offset)
                elif ':' in reference:
                    klass, number = reference.rsplit(':', 1)
                    if klass in self.trainers and number.isdigit() and int(number) > 0:
                        selected = (klass, int(number) - 1)
                if selected and selected not in seen:
                    seen.add(selected)
                    trainer = self.trainers[selected[0]]
                    if selected[1] < len(trainer['parties']):
                        parties.extend(trainer['parties'][selected[1]]['pokemon'])
        return parties

    def nearby_healers(self, facts):
        ranked = []
        for rule in self.index.by_effect.get(('heal', 'party', True), []):
            if rule.missing(facts) or not any(t.startswith('npc:') for t in rule.triggers):
                continue
            route = self.client.route(facts['map'], rule.map)
            if route.get('found'):
                ranked.append((len(route.get('legs', [])), rule))
        ranked.sort(key=lambda item: item[0])
        available = []
        for _, rule in ranked:
            if rule.map in getattr(self, 'navigation_memory', {}):
                # Healing changes HP/PP, not locked doors. Retain an observed
                # route failure across recovery cycles until geometry changes.
                if not any(pt.bfs_cross(
                    facts['map'], (facts['x'], facts['y']), rule.map, point,
                    last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                    blocked_maps=self.game.navigation_barriers(),
                    excluded_maps=self.game.navigation_excluded_maps())
                    for point in self.destination_points(rule.map, rule)):
                    continue
            available.append(rule)
            if len(available) == 3:
                break
        return available or [rule for _, rule in ranked[:3]]

    def healing_route_costs(self, healers, facts):
        """Compatibility wrapper; reset costs apply to more than healing."""
        return self.route_battle_reset_costs(healers, facts)

    def route_battle_reset_costs(self, rules, facts, *, completed_scripts=()):
        """Current entry guards on map-level routes; not a reachability proof."""
        script_rules = getattr(self.index, 'rules', None)
        if not isinstance(script_rules, list):
            return {}
        won = {r.effect[1] for r in script_rules
               if r.effect[0] == 'flag' and r.effect[2]
               and facts.get('flags', {}).get(r.effect[1]) and any(e[0] == 'battle' for e in r.preceding)}
        resets = [r for r in script_rules if r.effect[0] == 'flag' and not r.effect[2]
                  and r.effect[1] in won and 'load' in r.triggers
                  and r.storyline not in completed_scripts
                  and not any(e[0] == 'battle' for e in r.preceding)]
        if not resets:
            return {}
        costs = {}
        # A decision may offer many interactions at the same PC/shop. Query
        # each destination once, and do not persist costs across flag changes.
        for destination in sorted({rule.map for rule in rules} - {facts['map']}):
            route = self.client.route(facts['map'], destination)
            if not route.get('found'):
                continue
            entered = {leg['to_map'] for leg in route.get('legs', [])}
            lost = sorted({r.effect[1] for r in resets if r.map in entered
                           and not r.missing({**facts, 'map': r.map})})
            if lost:
                costs[destination] = lost
        return costs

    def annotate_route_reset_costs(self, groups, facts):
        costs = self.route_battle_reset_costs(
            [rule for group in groups.values() for rule in group['rules']], facts)
        for group in groups.values():
            context = group.get('context', {})
            for key in ('route_resets_won_battles', 'route_reset_scope',
                        'completion_resets_won_battles', 'completion_reset_scope'):
                context.pop(key, None)
            applicable = costs
            rules = group['rules']
            if rules and all(rule.effect == group['target'] and 'load' in rule.triggers
                    and ('ending', 'hall_of_fame_and_credits', True) in rule.preceding for rule in rules):
                completion = {}
                for rule in rules:
                    cleared = {name for kind, name, wanted in rule.preceding
                               if kind == 'flag' and not wanted}
                    flags = sorted(cleared & set(costs.get(rule.map, [])))
                    if flags:
                        completion[rule.map] = flags
                if completion:
                    context['completion_resets_won_battles'] = completion
                    context['completion_reset_scope'] = (
                        'Temporary flags cleared by the selected completed ending, not a replay cost '
                        'before this exit. Actual remaining battles, ceremony, credits and saved '
                        'CONTINUE are still required. No flag is changed by this preview.')
                    # Exclude these exact scripts, not their flag names: an
                    # earlier lobby may clear the same flag as a real detour.
                    applicable = self.route_battle_reset_costs(rules, facts,
                        completed_scripts={rule.storyline for rule in rules})
            relevant = {name: applicable[name] for name in sorted({rule.map for rule in rules})
                        if name in applicable}
            group['context'] = context
            if relevant:
                group['context'] = {**context, 'route_resets_won_battles': relevant,
                    'route_reset_scope': 'Already won battle flags cleared by entry scripts on the proposed map-level route to each interaction or training site, using currently satisfied guards. Not a tile-path proof; alternate routes or changed guards may differ. Does not include effects after the destination interaction. No flag is changed by this preview.'}

    def find_training_sites(self, facts, *, shared_experience=False):
        ranked = []
        level = facts['party'][0]['level']
        active = getattr(self, 'active', None) or {}
        context = active.get('context', {})
        if (shared_experience or context.get('capture_support_training')
                or (context.get('acquisition_method') == 'evolution' and context.get('trigger') == 'level')):
            finisher = training_battler(facts['party'])
            if finisher:
                level = max(level, finisher['level'])
        barriers = self.game.navigation_barriers()
        barriers[facts['map']] = barriers.get(facts['map'], set()) | self.game.live_npcs(facts['map'])
        self.training_navigation = {}
        # Include the visible frontier around explored maps, so preparing
        # for the next fight need not grind forever in the starting grass.
        nearby = set(self.visited)
        for name in self.visited:
            nearby.update(c['targetMap'] for c in self.maps.get(name, {}).get('connections', {}).values())
            nearby.update(w['destMap'] for w in self.maps.get(name, {}).get('warps', []) if w.get('destMap'))
        for name in nearby:
            if name.startswith('SafariZone'):
                continue  # BALL/BAIT/ROCK/RUN cannot produce knockout XP.
            wild = ((self.maps.get(name, {}).get('wild') or {}).get('red') or {}).get('grass') or {}
            mons = wild.get('mons', [])
            if not mons or max(mon['level'] for mon in mons) > level + 2:
                continue
            route = self.client.route(facts['map'], name)
            if not route.get('found'):
                continue
            spots = [(x, y) for x in range(pt.MAPS[name]['width']*2)
                     for y in range(pt.MAPS[name]['height']*2) if training_tile(name, x, y)]
            if not spots:
                continue
            paths = pt.bfs_cross(facts['map'], (facts['x'], facts['y']), name, spots[0],
                last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                blocked_maps=barriers, excluded_maps=self.game.navigation_excluded_maps(),
                goal_nodes={(name, *p) for p in spots})
            if not paths and name in getattr(self, 'navigation_memory', {}):
                continue  # Include live NPCs when rechecking a disproved route.
            self.training_navigation[name] = {'map': name, 'tile_route_found': bool(paths),
                'steps': len(paths)-1 if paths else None,
                'scope': 'path to actual encounter terrain, including current NPC collisions; battles can interrupt travel'}
            if paths:
                endpoint = paths[-1][0] if len(paths) > 1 else paths[0]
                spots = [tuple(endpoint[1:])]  # Use the reachable component.
            hops = len(route.get('legs', []))
            experience = training_yield(wild)['expected_experience_per_victory']
            ranked.append((not bool(paths), -experience / (1 + .15*hops), hops, name, spots))
        ranked.sort(key=lambda item: item[:4])
        self.training_sites = {}
        for _, _, _, name, spots in ranked[:3]:
            # Prefer grass close to the current position or a normal entrance.
            origin = (facts['x'], facts['y']) if facts['map'] == name else (
                (self.maps[name].get('warps') or [{}])[0].get('x', 10),
                (self.maps[name].get('warps') or [{}])[0].get('y', 10))
            self.training_sites[name] = min(spots, key=lambda pos: abs(pos[0]-origin[0]) + abs(pos[1]-origin[1]))
        return self.training_sites

    @staticmethod
    def balls_held(facts):
        normalized = {name.replace('_', '').upper() for name in BALLS}
        return sum(qty for key, qty in facts.get('bag', {}).items() if key in normalized)

    def collection_resources(self, facts):
        """Grounded inventory and party tools that affect capture feasibility."""
        normalized = {name.replace('_', '').upper(): name for name in BALLS}
        balls = []
        for key, quantity in facts.get('bag', {}).items():
            if quantity > 0 and key in normalized:
                name = normalized[key]
                balls.append({'ball': name, 'quantity': quantity,
                              'quality': BALL_QUALITY.get(name, 'special')})
        status = []
        useful_effects = {
            'SleepEffect': ('strong', False),
            'FreezeEffect': ('strong', False),
            'ParalyzeEffect': ('moderate', False),
            'PoisonEffect': ('moderate', True),
            'BurnEffect': ('moderate', True),
        }
        for mon in facts.get('party', []):
            if mon.get('hp', 1) <= 0:
                continue
            for move, pp in zip(mon.get('moves', []), mon.get('pp', [])):
                if move == 'None' or pp <= 0:
                    continue
                details = data.move_data(move)
                if details.get('power', 0) != 0 or details.get('effect') not in useful_effects:
                    continue
                bonus, damage_risk = useful_effects[details['effect']]
                status.append({'pokemon': mon['species'], 'move': move, 'pp': pp,
                               'accuracy': details.get('accuracy'),
                               'capture_bonus': bonus,
                               'residual_damage_risk': damage_risk})
        return {'ball_inventory': sorted(balls, key=lambda row: row['ball']),
                'total_balls': sum(row['quantity'] for row in balls),
                'capture_status_moves': status,
                'can_apply_safe_capture_status': any(not row['residual_damage_risk'] for row in status)}

    def species_scarcity(self, species, current_map):
        """Other acquisition methods for a target; scarcity is a route decision."""
        areas = sorted({method['map'] for method in self.collection_graph().get(species, [])
                        if method['map'] != current_map})
        return {'other_known_area_count': len(areas), 'other_known_areas': areas[:6],
                'unique_to_this_known_area': not areas}

    def recent_catch_attempts(self, name):
        """Hunts this area saw recently, and how many registered something new."""
        attempts = [attempt for attempt in getattr(self, 'catch_attempts', []) if attempt['map'] == name]
        return {'hunts': len(attempts),
                'registered': sum(bool(attempt['registered']) for attempt in attempts)}

    def method_value(self, method, name, owned, rod=None):
        if method == 'fishing':
            value = fishing_profile(rod, name, owned)
        else:
            table_name = 'water' if method == 'water' else 'grass'
            table = (((self.maps.get(name, {}).get('wild') or {}).get('red') or {})
                     .get(table_name) or {})
            value = table_profile(table, owned)
            if method == 'safari':
                value['safari_registration_reference'] = safari_capture_reference(table, owned)
        if value:
            for target in value['targets']:
                target.update(catch_difficulty(target['species']))
        return value

    @staticmethod
    def grass_species(map_data):
        """Wild grass species of one map, in a stable order."""
        wild = ((map_data.get('wild') or {}).get('red') or {}).get('grass') or {}
        return sorted({mon['species'] for mon in wild.get('mons', [])})

    def neighbourhood(self, hops=1):
        """Explored maps plus `hops` of connection/warp topology."""
        nearby = set(self.visited)
        for _ in range(max(1, hops)):
            for name in list(nearby):
                map_data = self.maps.get(name, {})
                nearby.update(c['targetMap'] for c in map_data.get('connections', {}).values())
                nearby.update(w['destMap'] for w in map_data.get('warps', []) if w.get('destMap'))
        return nearby

    def map_adjacency(self):
        """Undirected map adjacency: connections and warps, both directions.

        Door warps are listed on the street side only and exit mats may not
        state their destination at all, so either side's listing must serve
        as the crossing in both directions.
        """
        adjacency = getattr(self, '_map_adjacency', None)
        if adjacency is None:
            adjacency = {name: set() for name in self.maps}
            for name, map_data in self.maps.items():
                crossings = [c['targetMap'] for c in map_data.get('connections', {}).values()]
                crossings += [w['destMap'] for w in map_data.get('warps', []) if w.get('destMap')]
                for other in crossings:
                    if other in adjacency:
                        adjacency[name].add(other)
                        adjacency[other].add(name)
            self._map_adjacency = adjacency
        return adjacency

    def map_hops(self, origin, destination):
        """Fewest map crossings between two maps, or None when disconnected."""
        adjacency = self.map_adjacency()
        if origin not in adjacency or destination not in adjacency:
            return None
        distances = {origin: 0}
        queue = deque([origin])
        while queue:
            name = queue.popleft()
            if name == destination:
                return distances[name]
            for other in adjacency[name]:
                if other not in distances:
                    distances[other] = distances[name] + 1
                    queue.append(other)
        return None

    def capture_area_blocked(self, name, facts):
        """Reopen an observed impossible hunt when its identification item exists."""
        return any(name in requirement.get('capture_blocked_maps', [])
                   and not facts.get('bag', {}).get(item.replace('_', '').upper())
                   for item, requirement in getattr(self, 'battle_requirements', {}).items())

    def find_catch_areas(self, facts, requested_species=None):
        """Executable grass, Safari, Surf, and fishing acquisition areas."""
        owned = self.validated_owned(facts)
        if requested_species is not None:
            owned = set(self.complete_collection_graph()) - set(requested_species)
        barriers = self.game.navigation_barriers()
        barriers[facts['map']] = barriers.get(facts['map'], set()) | self.game.live_npcs(facts['map'])
        ranked = []
        previous = set()
        for hops in (1, 2, 3, 4):
            nearby = self.neighbourhood(hops)
            if nearby == previous:
                break
            previous = nearby
            self.catch_navigation = {}
            ranked = self.rank_catch_areas(nearby, facts, owned, barriers)
            if ranked:
                break
        ranked.sort(key=lambda item: item[:5])
        self.catch_areas = {key: area for *_, key, area in ranked[:6]}
        return self.catch_areas

    def rank_catch_areas(self, nearby, facts, owned, barriers):
        """Rank executable acquisition methods by expected travel + hunt effort."""
        ranked = []
        for name in nearby:
            if self.capture_area_blocked(name, facts):
                continue
            route = self.client.route(facts['map'], name)
            if not route.get('found'):
                continue
            red = ((self.maps.get(name, {}).get('wild') or {}).get('red') or {})
            methods = []
            grass = (red.get('grass') or {}).get('mons', [])
            if grass:
                methods.append(('safari' if name.startswith('SafariZone') else 'grass', None,
                                [(x, y) for x in range(pt.MAPS[name]['width']*2)
                                 for y in range(pt.MAPS[name]['height']*2)
                                 if training_tile(name, x, y)], None))
            water = (red.get('water') or {}).get('mons', [])
            knows_surf = any('Surf' in mon['moves'] for mon in facts.get('party', []))
            if water and knows_surf:
                methods.append(('water', None, surf_spots(name), 'Surf'))
            held = facts.get('bag', {})
            for rod in ('OldRod', 'GoodRod', 'SuperRod'):
                if held.get(rod.upper(), 0) and fishing_profile(rod, name) and fishing_spots(name):
                    methods.append(('fishing', rod, fishing_spots(name), rod))
            for method, rod, raw_spots, requirement in methods:
                value = self.method_value(method, name, owned, rod)
                if not value or not value['unregistered_species_count'] or not raw_spots:
                    continue
                species = [target['species'] for target in value['targets']]
                stances = ([spot for spot, _ in raw_spots] if method == 'fishing'
                           else [spot for spot, _, _ in raw_spots] if method == 'water'
                           else raw_spots)
                paths = pt.bfs_cross(facts['map'], (facts['x'], facts['y']), name, stances[0],
                    last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                    blocked_maps=barriers, excluded_maps=self.game.navigation_excluded_maps(),
                    goal_nodes={(name, *p) for p in stances})
                if not paths and name in getattr(self, 'navigation_memory', {}):
                    continue
                endpoint = tuple((paths[-1][0] if len(paths) > 1 else paths[0])[1:]) if paths else stances[0]
                chosen = (endpoint if method in ('grass', 'safari') and paths else
                          next((spot for spot in raw_spots if spot[0] == endpoint), raw_spots[0]))
                # Keep the long-standing map key for ordinary grass hunts;
                # other modalities need a disambiguating method/rod prefix.
                key = name if method == 'grass' else ':'.join(filter(None, (method, rod, name)))
                navigation = {'map': name, 'tile_route_found': bool(paths),
                    'steps': len(paths)-1 if paths else None, 'requires': requirement,
                    'scope': 'path to the real stance/terrain, including current NPC collisions'}
                self.catch_navigation[key] = navigation
                attempts = self.recent_catch_attempts(key)
                hunt_steps = value['expected_attempts_to_any_new_species'] or float('inf')
                misses = attempts['hunts'] - attempts['registered']
                travel_steps = len(paths)-1 if paths else float('inf')
                estimated_effort = travel_steps + hunt_steps * (1 + misses)
                area = {'key': key, 'map': name, 'method': method, 'rod': rod,
                        'species': species, 'spot': chosen,
                        'spots': [chosen] if method in ('grass', 'safari') else [],
                        'reachable': bool(paths),
                        'encounter_value': value, 'navigation': navigation}
                ranked.append((not bool(paths), estimated_effort,
                               -value['unregistered_encounter_share_pct'], -len(species), key, area))
        return ranked

    def add_coverage_groups(self, groups, facts):
        """Offer bordering areas the first playthrough has not reached yet.

        Coverage needs candidates of its own: once the plot's flag frontier is
        exhausted the story groups stop, and without these the run would stall
        with reachable areas still unvisited rather than exploring them.
        """
        bordering = set()
        for name in self.visited:
            map_data = self.maps.get(name, {})
            bordering.update(c['targetMap'] for c in map_data.get('connections', {}).values())
            bordering.update(w['destMap'] for w in map_data.get('warps', []) if w.get('destMap'))
        for name in sorted(bordering - self.visited):
            spot = self.entry_tile(name)
            if spot is None or not self.client.route(facts['map'], name).get('found'):
                continue
            x, y = spot
            target = ('explore', name, True)
            groups.setdefault(f'explore:{name}', {
                'target': target,
                'rules': [Rule(f'explore:{name}', name, 'explore:new_area',
                               [f'coord:({x},{y})'], [], [], target, [])],
                'objectives': ['Reach a bordering area that has not been visited yet'],
                'context': {'purpose': 'Coverage: no bordering area should stay unexplored'},
            })

    @staticmethod
    def entry_tile(name):
        """A tile of `name` a traveller can arrive at: a warp landing, else any walkable tile."""
        map_data = pt.MAPS.get(name) or {}
        for warp in map_data.get('warps', []):
            if warp.get('x') is not None and warp.get('y') is not None:
                return int(warp['x']), int(warp['y'])
        for x in range(map_data.get('width', 0) * 2):
            for y in range(map_data.get('height', 0) * 2):
                if pt.walkable(name, x, y):
                    return x, y
        return None

    def remembered_goal_reachable(self, blockage, groups, facts, cache):
        """A fresh exact-trigger path supersedes an old route obstruction.

        Returning from the ending can put us on the other side of an old
        puzzle. Keep that history, but do not advertise its unlock as a
        prerequisite when the requested location is now walkable without it.
        A reusable goal (healing, PC retrieval, shopping) may also have a
        different live producer: reaching that exact trigger does not require
        reopening the failed route to an older provider of the same goal.
        Unknown geometry remains unknown; an entrance is not a trigger proof.
        """
        goal = blockage['goal']
        if goal[0] == 'catch':
            # Capture skills have no script coordinates. Their fresh path
            # ends at actual hunt terrain/rod stance, whereas falling back
            # to a generic doorway would not establish useful access.
            navigation = getattr(self, 'catch_navigation', {}).get(goal[1], {})
            if (navigation.get('map') == blockage['destination']
                    and navigation.get('tile_route_found') is True):
                return True
        if goal[0] == 'level':
            # A level goal is satisfied at any usable training site. Its old
            # failed region is not a prerequisite when a live alternative has
            # a fresh path to actual grass (not just a map entrance).
            for group in groups.values():
                if list(group['target']) == list(goal) and any(
                        getattr(self, 'training_navigation', {}).get(rule.map, {}).get('tile_route_found')
                        for rule in group['rules'] if rule.storyline.startswith('skill:')):
                    return True
        if facts.get('map') not in pt.MAPS or 'x' not in facts or 'y' not in facts:
            return False
        ways = {}
        if goal[0] == 'location':
            ways[goal[1][0]] = []
        else:
            for group in groups.values():
                if list(group['target']) != list(goal):
                    continue
                for rule in group['rules']:
                    ways.setdefault(rule.map, []).append(rule)
        # Check a local provider first; proving access to the PC beside us
        # should not require exhaustively searching every distant hotel.
        for destination in sorted(ways, key=lambda name: (
                name != facts['map'], name != blockage['destination'], name)):
            if destination not in pt.MAPS:
                continue
            points = ([tuple(goal[1][1:])] if goal[0] == 'location' else
                      [point for rule in ways[destination] for point in
                       self.destination_points(destination, rule, allow_entry_fallback=False)])
            if not points:
                continue
            points = sorted(set(points))
            key = destination, tuple(points)
            if key not in cache:
                cache[key] = bool(pt.bfs_cross(facts['map'], (facts['x'], facts['y']),
                    destination, points[0], last_map=self.game.last_map,
                    allow_ledges=True, allow_spinners=True,
                    blocked_maps=self.game.navigation_barriers(),
                    excluded_maps=self.game.navigation_excluded_maps(),
                    goal_nodes={(destination, *point) for point in points}))
            if cache[key]:
                return True
        return False

    def add_navigation_groups(self, groups, facts):
        blockages = dict(getattr(self, 'navigation_history', {}))
        for blockage in getattr(self, 'navigation_memory', {}).values():
            blockages[json.dumps([blockage['destination'], blockage['map']])] = blockage
        if self.navigation_blockage:
            blockages[json.dumps([self.navigation_blockage['destination'], self.navigation_blockage['map']])] = self.navigation_blockage
        def relevant_blockages():
            # Newly discovered prerequisites can themselves have observed
            # blockers. Expand to a fixed point, independent of memory order.
            # Only a live chain activates a remembered detour.
            pending = list(blockages.values())
            while pending:
                live_targets = [list(group['target']) for group in groups.values()]
                for move, obstacle in self.field_requirements.items():
                    if move == 'Cut' and obstacle.get('tree'):
                        live_targets.append(['terrain', ','.join(map(str, [obstacle['map'], *obstacle['tree']])), True])
                    elif move == 'Surf' and obstacle.get('landing'):
                        live_targets.append(['location', obstacle['landing'], True])
                ready = [b for b in pending if list(b['goal']) in live_targets]
                if not ready:
                    return
                for blockage in ready:
                    pending.remove(blockage)
                    yield blockage

        reachable = {}
        for blockage in relevant_blockages():
            if self.index.satisfied(blockage['goal'], facts):
                continue
            if self.remembered_goal_reachable(blockage, groups, facts, reachable):
                continue
            # Legacy failed paths sometimes recorded only collision tiles.
            # A remembered visible stationary actor standing on the actual
            # destination warp is causal evidence, not a map-wide NPC guess.
            blocked_npcs = set(blockage.get('blocking_npcs', []))
            entrances = {(warp['x'], warp['y']) for warp in pt.MAPS.get(blockage['map'], {}).get('warps', [])
                         if warp.get('dest_map_name') == blockage['destination']}
            for text_id, position in getattr(getattr(self, 'game', None), 'stationary_npcs', {}).get(blockage['map'], {}).items():
                if tuple(position) in entrances:
                    blocked_npcs.add(int(text_id))
            if blocked_npcs:
                blockage = {**blockage, 'blocking_npcs': sorted(blocked_npcs)}
            route = self.client.route(facts['map'], blockage['destination'])
            corridor = {facts['map'], blockage['destination'], *[leg['to_map'] for leg in route.get('legs', [])]}
            # The high-level graph joins outdoor regions directly and can
            # omit their gatehouses. Keep observed blockers in those real
            # warp-connected rooms as part of the corridor.
            corridor.update(warp['dest_map_name'] for name in list(corridor)
                for warp in pt.MAPS.get(name, {}).get('warps', []) if warp.get('dest_map_name'))
            if (route.get('found') and blockage['map'] not in corridor
                    and not blockage.get('blocking_npcs')
                    and not blockage.get('blocking_trainers')):
                # A resettable puzzle behind us is not a prerequisite for
                # the remaining route. But map topology can omit several
                # interior rooms (e.g. a cave exit), so it cannot disprove an
                # actually observed actor blocking the live goal. Fresh exact
                # access and already-won trainer flags still supersede that
                # memory above/below; no fixed route is imposed.
                continue
            for text_id in blockage.get('blocking_trainers', []):
                config = next((n for n in self.index.configs.get(blockage['map'], {}).get('npcs', [])
                               if n['id'] == text_id), {})
                storyline = f"{blockage['map']}:{config.get('talk', '')}"
                program = self.index.stories.get(storyline, {}).get('program', [])
                def flags_in(node):
                    if isinstance(node, list):
                        return set().union(*(flags_in(v) for v in node))
                    if not isinstance(node, dict):
                        return set()
                    call = node.get('Call', {})
                    if call.get('callee', '').removeprefix('game.') == 'getFlag':
                        value = evaluate(call['args'][0], {})
                        return {value} if isinstance(value, str) and value.startswith('EVENT_BEAT_') else set()
                    return set().union(*(flags_in(v) for v in node.values()))
                flags = flags_in(program)
                if len(flags) != 1:
                    continue
                flag = next(iter(flags))
                if facts['flags'].get(flag):
                    continue
                target = ('flag', flag, True)
                groups['trainer:' + flag] = {'target': target,
                    'rules': [Rule('trainer:' + flag, blockage['map'], storyline,
                                   [f'npc:{text_id}'], [], [], target, [('battle', 'blocking_trainer', True)])],
                    'objectives': [f"Challenge the trainer occupying the passage to {blockage['destination']}"],
                    'context': {'observed_navigation_blockage': blockage}}
            # A failed path is evidence that topology omitted a local story
            # prerequisite. Offer the actual map's unmet script effects and
            # let strategy decide what to investigate; no named-route recipe.
            pickups = {self.index.npc_toggles[(blockage['map'], npc['textId'])][0]
                       for npc in self.maps.get(blockage['map'], {}).get('npcs', [])
                       if npc.get('spriteName') == 'PokeBall'
                       and npc['textId'] not in blockage.get('blocking_npcs', [])
                       and (blockage['map'], npc['textId']) in self.index.npc_toggles}
            for local in self.index.rules:
                if (local.map not in (blockage['map'], blockage['destination'])
                        and local.map not in pt.ELEVATOR_MAPS):
                    continue
                frontiers = []
                if local.effect[0] == 'block' and not self.index.satisfied(local.effect, facts):
                    name, x, y = local.effect[1].split(',')
                    m = pt.MAPS[name]
                    offset = int(y) * m['width'] + int(x)
                    blocks = pt.BLOCKSETS[m['tileset_id']]
                    def openings(block):
                        return sum(blocks[block][i] in m['passable_tiles'] for i in (4, 6, 12, 14))
                    if offset < len(m['blocks']) and openings(local.effect[2]) > openings(m['blocks'][offset]):
                        frontiers.extend(self.index.frontier(local.effect, facts))
                elif (local.effect[0] == 'visibility' and not local.effect[2]
                      and any(self.index.npc_toggles.get((blockage['map'], text_id), (None,))[0] == local.effect[1]
                              for text_id in blockage.get('blocking_npcs', []))):
                    # The hide producer itself can be a ready coordinate-
                    # triggered battle. Backchain its whole effect, not only
                    # missing guards (which are empty once battle-ready).
                    frontiers.extend(self.index.frontier(local.effect, facts))
                elif (local.effect[0] == 'movement' and not local.missing(facts)
                      and self.index.coordinates(local)):
                    # A currently enabled push-back can be avoided by
                    # changing one of its branch guards (e.g. acquiring a ticket).
                    # An entry autowalk has no obstructing coordinate trigger;
                    # completing it is not evidence of removing an exit guard.
                    for guard, wanted in local.guards:
                        for missing in requirements(guard, not wanted, facts):
                            for prerequisite in missing:
                                frontiers.extend(self.index.frontier(prerequisite, facts))
                for rule in frontiers:
                    key = json.dumps(rule.effect)
                    group = groups.setdefault(key, {'target': rule.effect, 'rules': [],
                        'objectives': [f"Investigate the obstruction while travelling to {blockage['destination']}"],
                        'context': {'observed_navigation_blockage': blockage,
                                    'prerequisite_for': local.description()}})
                    if rule not in group['rules']:
                        group['rules'].append(rule)
            for (map_name, text_id), (toggle, _) in self.index.npc_toggles.items():
                if map_name != blockage['map'] or toggle in pickups:
                    continue
                if text_id not in blockage.get('blocking_npcs', []):
                    continue
                for rule in self.index.frontier(('visibility', toggle, False), facts):
                    key = json.dumps(rule.effect)
                    group = groups.setdefault(key, {'target': rule.effect, 'rules': [],
                        'objectives': [f"Find a way to move an NPC obstructing {blockage['destination']}"],
                        'context': {'npc_map': map_name, 'npc_text_id': text_id, 'toggle': toggle,
                                    'observed_navigation_blockage': blockage}})
                    if rule not in group['rules']:
                        group['rules'].append(rule)

    def defer_unusable_boulders(self, groups, facts):
        """Apply after ALL navigation frontiers, which can introduce new pushes."""
        pending = [rule for group in groups.values() for rule in group['rules']
                   if rule.id.startswith('boulder:')]
        known = any('Strength' in mon.get('moves', []) for mon in facts['party'])
        badges = field_badge_prerequisites('Strength', facts['flags'])
        if not pending or known and not badges:
            return
        obstacle = {'move': 'Strength', 'map': pending[0].map,
                    'puzzle_flags': sorted({rule.effect[1] for rule in pending})}
        self.field_requirements['Strength'] = obstacle
        for target in [('item', 'HM04', True), *badges]:
            for rule in self.index.frontier(target, facts):
                key = json.dumps(rule.effect)
                group = groups.setdefault(key, {'target': rule.effect, 'rules': [],
                    'objectives': ['Prepare Strength before attempting an engine-defined boulder puzzle'],
                    'context': {'required_move': 'Strength', 'terrain_obstruction': obstacle}})
                if rule not in group['rules']:
                    group['rules'].append(rule)
        if (facts['bag'].get('HM04') and not known
                and any(hm_compatible(mon['species'], 'Strength') for mon in facts['party'])):
            target = ('move', 'Strength', True)
            groups['learn:Strength'] = {'target': target,
                'rules': [Rule('learn:Strength', facts['map'], 'skill:learn', [], [], [], target, [])],
                'objectives': ['Learn Strength before attempting the observed boulder puzzle'], 'context': obstacle}
        for key, group in list(groups.items()):
            group['rules'] = [rule for rule in group['rules'] if not rule.id.startswith('boulder:')]
            if not group['rules']:
                del groups[key]

    def refresh_route_requirements(self, facts):
        """Retain destinations, not stale alternatives to already-open gates."""
        for name, requirement in list(getattr(self, 'route_requirements', {}).items()):
            if self.index.satisfied(requirement['goal'], facts):
                del self.route_requirements[name]
                continue
            points = requirement.get('trigger_points')
            if points is None:
                # Old in-memory records did not retain the exact trigger.
                # Re-ground it in the scene index, never in a fixed itinerary.
                points = [point for rule in self.index.rules
                          if rule.map == name and tuple(rule.effect) == tuple(requirement['goal'])
                          for point in self.destination_points(name, rule)]
            prerequisites = self.discover_route_prerequisites(self.game.st(), name, points)
            if prerequisites:
                self.route_requirements[name] = {**requirement,
                    'trigger_points': points, 'prerequisites': prerequisites}
            else:
                # No currently grounded blocker: do not re-offer a consumed
                # drink simply because a later gym battle remains unfinished.
                del self.route_requirements[name]

    def add_deferred_route_frontiers(self, groups, facts, previews):
        """Recover causal unlocks for goals added after the story frontier.

        Collection and supply sources are inserted late. Before pruning a
        known blocked target, backchain its exact trigger region, not merely
        its map. An alternative already reachable source needs no new door.
        Relaxed geometry supplies evidence only; Jev still selects a real
        script action and execution retains the actual collision checks.
        """
        pending = []
        for group in list(groups.values()):
            routes = group.get('context', {}).get('trigger_navigation', [])
            if any(route.get('tile_route_found') for route in routes):
                continue
            for rule in group['rules']:
                if rule.map not in getattr(self, 'navigation_memory', {}):
                    continue
                points = self.destination_points(rule.map, rule)
                key = rule.map, tuple(points)
                if previews.get(key, {}).get('tile_route_found') is False:
                    pending.append((key, points, group))
        if not pending:
            return
        # Use the same observed position as the reachability previews; do
        # not refresh geometry partway through a planning pass.
        state = {'map_name': facts['map'], 'player_x': facts['x'], 'player_y': facts['y']}
        discovered = {}
        for (destination, points_key), points, parent in pending:
            key = destination, points_key
            if key not in discovered:
                discovered[key] = self.discover_route_prerequisites(state, destination, points)
            for target in discovered[key]:
                for rule in self.index.frontier(target, facts):
                    if rule.effect == parent['target']:
                        continue
                    group = groups.setdefault(json.dumps(rule.effect), {
                        'target': rule.effect, 'rules': [],
                        'objectives': ['Open a route to an otherwise deferred acquisition or story goal'],
                        'context': {}})
                    if rule not in group['rules']:
                        group['rules'].append(rule)
                    context = group.setdefault('context', {})
                    goals = context.setdefault('prerequisite_for_goals', [])
                    if parent['target'] not in goals:
                        goals.append(parent['target'])
                    evidence = context.setdefault('route_unlocks', [])
                    requirement = {'destination': destination, 'goal': parent['target'],
                        'trigger_points': points, 'objectives': parent.get('objectives', []),
                        # Preserve the actual acquisition costs even after the
                        # inaccessible parent is pruned. Navigation bookkeeping
                        # belongs to the unlock, not to a recursively nested
                        # parent route; unknown economic/risk fields stay intact.
                        'downstream_context': deepcopy({key: value for key, value in
                            parent.get('context', {}).items() if key not in {
                                'trigger_navigation', 'deferred_trigger_maps', 'navigation_scope',
                                'route_unlocks', 'prerequisite_for_goals'}}),
                        'evidence': 'Causal blocker on a relaxed planning path; real execution and remaining obstacles still required'}
                    if requirement not in evidence:
                        evidence.append(requirement)

    def strategy_groups(self, facts):
        groups = super().strategy_groups(facts)
        self.navigation_facts = facts
        if getattr(self, 'navigation_memory', {}):
            self.game.script_navigation_barriers = self.observed_navigation_barriers(facts)
        self.refresh_route_requirements(facts)
        # A known blocked goal needs a newly grounded prerequisite before
        # asking strategy to select it again. This also reconstructs geometric
        # dependencies after a checkpoint without replaying an old itinerary.
        for group in list(groups.values()):
            for rule in group['rules']:
                if rule.map not in getattr(self, 'navigation_memory', {}):
                    continue
                points = self.destination_points(rule.map, rule)
                prerequisites = self.discover_route_prerequisites(self.game.st(), rule.map, points)
                if prerequisites:
                    self.route_requirements[rule.map] = {'destination': rule.map, 'goal': group['target'],
                        'trigger_points': points, 'prerequisites': prerequisites,
                        'evidence': 'Planning with doors relaxed; each prerequisite requires real execution'}
        for requirement in getattr(self, 'route_requirements', {}).values():
            if self.index.satisfied(requirement['goal'], facts):
                continue
            for target in requirement['prerequisites']:
                for rule in self.index.frontier(target, facts):
                    key = json.dumps(rule.effect)
                    group = groups.setdefault(key, {'target': rule.effect, 'rules': [],
                        'objectives': ['Resolve a terrain prerequisite on a possible route to the active goal'],
                        'context': requirement})
                    if rule not in group['rules']:
                        group['rules'].append(rule)
        for item, context in self.battle_requirements.items():
            if (context.get('blocked_goal') and not context.get('capture_blocked_maps')
                    and self.index.satisfied(context['blocked_goal'], facts)):
                continue
            for rule in self.index.frontier(('item', item, True), facts):
                key = json.dumps(rule.effect)
                group = groups.setdefault(key, {'target': rule.effect, 'rules': [],
                    'objectives': ['Prepare the item required by an observed blocked battle or wild capture'],
                    'context': context})
                if rule not in group['rules']:
                    group['rules'].append(rule)
        self.add_navigation_groups(groups, facts)
        surf_obstacle = self.field_requirements.get('Surf')
        current_prerequisites = surf_current_prerequisites(surf_obstacle, facts['flags']) if surf_obstacle else []
        for target in current_prerequisites:
            for rule in self.index.frontier(target, facts):
                key = json.dumps(rule.effect)
                group = groups.setdefault(key, {'target': rule.effect, 'rules': [],
                    'objectives': ['Stop the engine-gated current before embarking with Surf'],
                    'context': {'terrain_obstruction': surf_obstacle, 'prerequisites': current_prerequisites}})
                if rule not in group['rules']:
                    group['rules'].append(rule)
        pending_boulders = [r for group in groups.values() for r in group['rules']
                           if r.id.startswith('boulder:')]
        if pending_boulders and (not any('Strength' in m['moves'] for m in facts['party'])
                                or field_badge_prerequisites('Strength', facts['flags'])):
            self.field_requirements['Strength'] = {'move': 'Strength',
                'map': pending_boulders[0].map, 'puzzle_flags': [r.effect[1] for r in pending_boulders]}
            for key, group in list(groups.items()):
                group['rules'] = [r for r in group['rules'] if not r.id.startswith('boulder:')]
                if not group['rules']:
                    del groups[key]
        for move, obstacle in self.field_requirements.items():
            badge_prerequisites = field_badge_prerequisites(move, facts['flags'])
            for target in badge_prerequisites:
                for rule in self.index.frontier(target, facts):
                    key = json.dumps(rule.effect)
                    group = groups.setdefault(key, {'target': rule.effect, 'rules': [],
                        'objectives': [f'Obtain the badge required to use {move} outside battle'],
                        'context': {'required_move': move, 'terrain_obstruction': obstacle}})
                    if rule not in group['rules']:
                        group['rules'].append(rule)
            item = f'HM{HM_MOVES.index(move)+1:02d}'
            for rule in self.index.frontier(('item', item, True), facts):
                key = json.dumps(rule.effect)
                group = groups.setdefault(key, {'target': rule.effect, 'rules': [],
                    'objectives': [f"Obtain {move} to pass an observed terrain obstruction"],
                    'context': {'terrain_obstruction': obstacle}})
                if rule not in group['rules']:
                    group['rules'].append(rule)
            known = any(move in mon['moves'] for mon in facts['party'])
            compatible = any(hm_compatible(mon['species'], move) for mon in facts['party'])
            if not compatible and len(facts['party']) < 6:
                for effect in self.index.by_effect:
                    if effect[0] != 'pokemon' or not hm_compatible(effect[1], move):
                        continue
                    for rule in self.index.frontier(effect, facts):
                        key = json.dumps(rule.effect)
                        group = groups.setdefault(key, {'target': rule.effect, 'rules': [],
                            'objectives': [f'Obtain a Pokemon able to learn {move} for the observed terrain'],
                            'context': {'required_move': move, 'compatible_species': effect[1],
                                        'terrain_obstruction': obstacle}})
                        if rule not in group['rules']:
                            group['rules'].append(rule)
            if facts['bag'].get(item) and not known and compatible:
                target = ('move', move, True)
                groups['learn:' + move] = {'target': target,
                    'rules': [Rule('learn:' + move, facts['map'], 'skill:learn', [], [], [], target, [])],
                    'objectives': [f'Learn {move} to pass the terrain obstruction'], 'context': obstacle}
            if known:
                if badge_prerequisites or move == 'Surf' and current_prerequisites:
                    continue  # Known HM is not proof that the native field action is legal.
                if move == 'Strength':
                    continue  # Engine puzzle rules provide the actual push goal.
                if move == 'Surf':
                    target = ('location', obstacle['landing'], True)
                    key = json.dumps(target)
                    groups['surf:' + key] = {'target': target,
                        'rules': [Rule('surf:' + key, obstacle['map'], 'skill:surf', [], [], [], target, [])],
                        'objectives': ['Cross the observed water passage'], 'context': obstacle}
                    continue
                key = ','.join(map(str, [obstacle['map'], *obstacle['tree']]))
                if key not in self.cleared_terrain:
                    target = ('terrain', key, True)
                    groups['field:' + key] = {'target': target,
                        'rules': [Rule('field:' + key, obstacle['map'], 'skill:field', [], [], [], target, [])],
                        'objectives': [f'Use {move} to clear the terrain obstruction'], 'context': obstacle}
        self.add_navigation_groups(groups, facts)
        threats = []
        preference = getattr(self, 'preference', 'none')
        for group in groups.values():
            opponents = self.opponent_parties(group['rules'], facts)
            if opponents:
                context = {**group.get('context', {}), 'opponent_parties': opponents,
                           'battle_is_not_guaranteed_by_script_preconditions': True}
                # Each bias supplies the comparison data it asks the model to
                # use, so the offered candidates differ with the preference.
                if preference == 'type':
                    context['type_options'] = type_options(facts.get('party', []), opponents)
                elif preference == 'tactic':
                    context['tactical_options'] = tactical_options(facts.get('party', []), opponents)
                group['context'] = context
                threats.append((max(mon['level'] for mon in opponents), group['objectives']))
        if not facts['party']:
            return groups
        self.add_stored_battler_retrieval(groups, facts)
        self.add_recovery_groups(groups, facts)
        carried_healing = any(facts['bag'].get(name.replace('_', '').upper(), 0)
                              for name, item in MEDICINES.items() if 'hp' in item.get('tags', []))
        fatigue_defeats = [d for d in self.battle_defeats if not d.get('resolved_by_victory')
            and d.get('preparation_signature') and d['preparation_signature']['party'][0]['hp_band'] <= 2
            and not d['preparation_signature']['medicine']]
        previews = {}
        if fatigue_defeats and not carried_healing:
            previews = self.annotate_navigation(groups, facts, prune=False)
        if fatigue_defeats and not carried_healing and any(g['target'][0] == 'supply'
                and any(r['tile_route_found'] for r in g.get('context', {}).get('trigger_navigation', []))
                for g in groups.values()):
            battle_maps = {r.map for g in groups.values() if g.get('context', {}).get('opponent_parties') for r in g['rules']}
            for key, group in list(groups.items()):
                if group['target'][0] in ('flag', 'block', 'transport') and all(r.map in battle_maps for r in group['rules']):
                    del groups[key]
                elif group['target'][0] == 'supply':
                    group['context'].update(optional_preparation=False,
                        reason='An observed later battle began with low HP and no recovery supplies',
                        observed_defeats=fatigue_defeats[-2:])
        coverage_remedies = set()
        for operation, details in self.inventory_use_options(facts, free_slot=False):
            if not operation.startswith('teach_tm:') or details['forget'] is None:
                continue
            old, new = data.move_data(details['forget']), data.move_data(details['learn'])
            mon = details['pokemon']
            old_score = attack_profile(mon['species'], mon['level'], details['forget'])['neutral_expected_power']
            new_score = attack_profile(mon['species'], mon['level'], details['learn'])['neutral_expected_power']
            best_known = max(attack_profile(mon['species'], mon['level'], m)['neutral_expected_power']
                             for m in mon['moves'] if m != 'None' and data.move_data(m)['type'] == old['type'])
            improves_type = old['power'] > 0 and old['type'] == new['type'] and new_score > best_known * 1.3
            adds_coverage = (new['power'] >= 70 and mon['level'] >= max(m['level'] for m in facts['party']) * .75
                and new['type'] not in {data.move_data(m)['type'] for m in mon['moves']
                                      if m != 'None' and data.move_data(m)['power'] > 0})
            if improves_type or adds_coverage:
                move = details['learn']
                target = ('move', move, True)
                uncovered = sorted({enemy['species'] for group in groups.values()
                    for enemy in group.get('context', {}).get('opponent_parties', [])
                    if not effective_attacks(mon, enemy['species']) and effective_attacks(
                        {**mon, 'moves': [move], 'pp': [new['pp']]}, enemy['species'])})
                coverage_remedies.update(uncovered)
                groups['upgrade:' + move] = {'target': target,
                    'rules': [Rule('upgrade:' + move, facts['map'], 'skill:learn', [], [], [], target, [])],
                    'objectives': ['Improve attack strength or type coverage using an already owned compatible TM'],
                    'context': {'optional_preparation': not bool(uncovered), 'available_upgrade': details, 'expected_power_before': old_score,
                                'expected_power_after': new_score, 'adds_new_attack_type': adds_coverage,
                                'upcoming_opponents_current_moves_cannot_damage_but_new_move_can': uncovered}}
        for key, group in list(groups.items()):
            opponents = group.get('context', {}).get('opponent_parties', [])
            if any(enemy['species'] in coverage_remedies and not any(
                mon['hp'] > 0 and mon['level'] >= enemy['level'] * .75
                and effective_attacks(mon, enemy['species']) for mon in facts['party']) for enemy in opponents):
                # A ready-to-teach remedy exists, while every suitably levelled
                # battler is unable to damage this opponent. Complete that
                # preparation before offering the battle again.
                del groups[key]
        if len(facts['bag']) >= 20 and self.inventory_use_options(facts):
            target = ('bag_space', 'inventory', 20)
            groups['prepare:bag_space'] = {'target': target,
                'rules': [Rule('bag_space', facts['map'], 'skill:use_item', [], [], [], target, [])],
                'objectives': ['Make room for required story items by using a consumable or teaching a compatible TM through the real inventory menu'],
                'context': {'bag_capacity': 20, 'occupied_slots': len(facts['bag']),
                            'blocked_item_targets': [g['target'] for g in groups.values() if g['target'][0] == 'item']}}
            for key, group in list(groups.items()):
                if group['target'][0] == 'item' and group['target'][2] and not facts['bag'].get(group['target'][1].replace('_', '').upper()):
                    del groups[key]  # No empty slot: the pickup cannot succeed yet.
        self.healing_rules = self.nearby_healers(facts)
        if not facts['fully_recovered'] and self.healing_rules:
            groups['prepare:heal'] = {
                'target': ('heal', 'party', True),
                'objectives': ['Restore HP, status and move PP before continuing the story'],
                'rules': self.healing_rules,
                'context': {'urgently_needed': self.needs_healing(facts)},
            }
        if threats or self.defeat_preparation:
            target_level, objectives = (min(threats, key=lambda t: t[0]) if threats else
                                        (0, ['Prepare for an observed battle defeat']))
            target_level = max(target_level, self.defeat_preparation)
            if preference == 'level':
                target_level += LEVEL_PREFERENCE_MARGIN
            # The training skill stops at this same recovery threshold.
            # Offering it while recovery is needed creates a choose/exit loop.
            battler = training_battler(facts['party'])
            if battler and target_level > battler['level'] and not self.needs_healing(facts):
                sites = self.find_training_sites(facts)
                rules = [Rule(f'train:{name}:{target_level}', name, 'skill:train_encounter',
                              [], [], [], ('level', 'leader', target_level), [])
                         for name in sites]
                if rules:
                    groups['prepare:train'] = {
                        'target': ('level', 'leader', target_level), 'objectives': objectives,
                        'rules': rules,
                        'context': {'purpose': 'Earn experience in normal wild battles to prepare for the easiest pending story battle',
                                    'target_level': target_level,
                                    'training_regions': {name: ((self.maps[name].get('wild') or {}).get('red') or {}).get('grass')
                                                         for name in sites},
                                    'observed_defeats': self.battle_defeats[-3:],
                                    'training_battler': battler,
                                    'upcoming_moves': data.species_data(battler['species']).get('learnset', [])},
                    }
        if self.collects_dex:
            balls = self.balls_held(facts)
            owned = self.validated_owned(facts)
            resources = self.collection_resources(facts)
            box_full = self.add_box_capacity_group(groups, facts)
            catch_areas = {} if box_full and len(facts['party']) >= 6 else self.find_catch_areas(facts)
            for key, area in catch_areas.items():
                # Legacy/mocked tests and trace adapters may still provide the
                # former {map: {species, spots, ...}} shape.
                name = area.get('map', key)
                method = area.get('method', 'grass')
                all_method_species = {target['species'] for target in
                    (self.method_value(method, name, set(), area.get('rod')) or {}).get('targets', [])}
                area = {**area, 'map': name, 'method': method,
                        'encounter_value': area.get('encounter_value') or encounter_value(
                            self.maps[name], owned),
                        'navigation': area.get('navigation') or self.catch_navigation.get(key)}
                target = ('catch', key, True)
                groups.setdefault(f'collect:{key}', {
                    'target': target,
                    'objectives': ['Register wild species that are not in the Pokédex yet'],
                    'rules': [Rule(f'catch:{key}', name, 'skill:catch_encounter', [], [], [], target, [])],
                    'context': {'purpose': 'Use an executable acquisition method to catch unregistered wild species',
                                'acquisition_method': method, 'rod': area.get('rod'),
                                # A catch needs a ball. Offering this target without
                                # saying the bag is empty invites choosing a goal
                                # that cannot possibly complete.
                                'balls_held': balls,
                                'prerequisite': (
                                    'Safari admission supplies 30 Safari Balls; ordinary bag balls cannot be used'
                                    if method == 'safari' else
                                    'No balls are carried, so nothing found here can be caught until Poké Balls are bought or received'
                                    if balls == 0 else f'Catching spends balls; {balls} carried'),
                                'unregistered_species': [catch_difficulty(species) for species in area['species']],
                                'encounter_value': area['encounter_value'],
                                'species_scarcity': {species: self.species_scarcity(species, name)
                                                     for species in area['species']},
                                'collection_resources': resources,
                                'already_registered_here': sorted(all_method_species & owned),
                                'recent_attempts': self.recent_catch_attempts(key),
                                'navigation': area['navigation'],
                                'encounters': (((self.maps[name].get('wild') or {}).get('red') or {})
                                               .get('water' if method == 'water' else 'grass'))},
                })
            self.add_nonwild_collection_groups(groups, facts)
            self.add_capture_support_retrieval(groups, facts)
            self.add_capture_support_training(groups, facts)
        if self.maximizes_coverage:
            self.add_coverage_groups(groups, facts)
        # Collection/support training is added late. Activate remembered
        # dependencies only for these actual live goals, not every old level
        # attempt irrespective of whether training is currently useful.
        self.add_navigation_groups(groups, facts)
        # Keep the blocked goals until their entrances have been considered.
        readiness = battle_readiness(facts['party'], facts['bag'])
        failed_maps = {d['map'] for d in self.battle_defeats if not d.get('resolved_by_victory')
                       and d.get('preparation_signature') == readiness and d['map'] != facts['map']}
        for key, group in list(groups.items()):
            if group['target'][0] in ('flag', 'block', 'transport') and all(r.map in failed_maps for r in group['rules']):
                del groups[key]  # Repeating the same failed preparation is not a new plan.
        previews = self.annotate_navigation(groups, facts, previews, prune=False)
        self.add_deferred_route_frontiers(groups, facts, previews)
        self.add_cut_route_frontiers(groups, facts)
        self.transport_frontiers(groups, facts)
        self.add_mechanism_groups(groups, facts)
        self.defer_unusable_boulders(groups, facts)
        self.annotate_navigation(groups, facts, previews)
        for key, group in list(groups.items()):
            if (group['target'][0] in ('catch', 'held_species')
                    and self.capture_resources_missing(facts, group.get('context', {}).get('acquisition_method', 'grass'))):
                del groups[key]  # Retain supply, travel prerequisites and non-capture methods.
        restore = {key: group for key, group in groups.items()
                   if group.get('context', {}).get('restore_main_battler')
                   and any(route.get('tile_route_found') for route in
                           group['context'].get('trigger_navigation', []))}
        if restore:
            groups = restore
        if self.avoids_optional_preparation:
            for key, group in list(groups.items()):
                if group.get('context', {}).get('optional_preparation'):
                    del groups[key]  # Preparation the run can survive without only spends frames.
        self.prioritize_critical_recovery(groups, facts)
        if self.collects_dex:
            self.annotate_script_unlocks(groups, facts)
        self.annotate_route_reset_costs(groups, facts)
        return groups

    def annotate_script_unlocks(self, groups, facts):
        """Expose immediate script dependencies without simulating game progress."""
        rules = getattr(self.index, 'rules', None)
        if not isinstance(rules, list):
            return
        blocked = [rule for rule in rules if rule.missing(facts)]
        for group in groups.values():
            kind, name, wanted = group['target']
            if kind not in ('flag', 'item') or wanted is not True:
                continue
            hypothetical = {**facts, 'flags': dict(facts['flags']), 'bag': dict(facts['bag'])}
            if kind == 'flag':
                hypothetical['flags'][name] = True
            else:
                hypothetical['bag'][name.replace('_', '').upper()] = 1
            unlocked = [rule for rule in blocked if not rule.missing(hypothetical)]
            if not unlocked:
                continue
            # A single script may emit several flags alongside the same door
            # opening. Count distinct effects and scripts, not duplicated paths.
            effects = {(rule.map, json.dumps(rule.effect)): rule.effect for rule in unlocked}
            group.setdefault('context', {})['script_unlocks'] = {
                'scripts': len({(rule.map, rule.storyline) for rule in unlocked}),
                'maps': sorted({rule.map for rule in unlocked}),
                'effect_counts': dict(Counter(effect[0] for effect in effects.values())),
                'potential_gift_species': sorted({effect[1] for effect in effects.values()
                                                 if effect[0] == 'pokemon'}),
                'scope': 'Only guards newly satisfied if this target is obtained; preceding effects, navigation and battles still require execution. Not guaranteed rewards.'}

    @staticmethod
    def prioritize_critical_recovery(groups, facts):
        """Recover a critically hurt main battler when a nurse is executable.

        Script preconditions do not mean a party can survive the next battle.
        Keep recovery mandatory at <=25% HP, but only if an actual tile path
        exists without an unexecuted Surf crossing; otherwise retain the
        frontiers that can unlock that path. Knowing Surf is not using it.
        """
        party = facts.get('party', [])
        if not party:
            return
        main = max(party, key=lambda mon: mon['level'])
        if main.get('max_hp', 0) <= 0 or main['hp'] > main['max_hp'] * .25:
            return
        recovery = {}
        for key, group in groups.items():
            if group['target'][0] != 'heal':
                continue
            reachable = {route['map'] for route in
                         group.get('context', {}).get('trigger_navigation', [])
                         if route.get('tile_route_found') and not route.get('requires_surf')}
            rules = [rule for rule in group['rules'] if rule.map in reachable]
            if rules:
                recovery[key] = {**group, 'rules': rules, 'context': {
                    **group.get('context', {}), 'mandatory_recovery': True,
                    'critical_battler': main,
                    'reason': 'Main battler has at most 25% HP and a nurse has a confirmed tile path'}}
        if recovery:
            groups.clear()
            groups.update(recovery)

    # Collecting burns balls faster than the starting funds replace them, so a
    # collector restocks well before the bag is empty.
    BALL_RESERVE = 12

    # How many of the newest hunts the judge compares an area on.
    CATCH_WINDOW = 12

    def add_capture_support_retrieval(self, groups, facts):
        """Expose earned PC status users as preparation, not only evolution inputs.

        Each actual box slot remains a separate choice: duplicate species may
        have different levels/moves. Retrieval uses the existing real PC skill.
        """
        if not self.collects_dex:
            return
        owned = self.validated_owned(facts)
        targets = []
        for retreat in getattr(self, 'capture_retreats', {}).values():
            enemy = (retreat.get('retreat_observation') or {}).get('enemy') or {}
            if retreat['species'] not in owned and enemy.get('level'):
                targets.append({'species': retreat['species'], 'map': retreat['map'],
                                'observed_level': enemy['level']})
        if not targets:
            return
        rules = [rule for rule in self.index.by_effect.get(('pc', 'storage', True), [])
                 if not rule.missing(facts)]
        if not rules:
            return
        present = {mon['species'] for mon in facts.get('party', [])}
        for mon in facts.get('stored_pokemon', []):
            if mon['species'] in present:
                continue  # The existing retrieval goal establishes party presence.
            matchups = []
            for target in targets:
                # Compare known move effects/type immunity for a clean future
                # encounter. Do not project yesterday's HP/DVs into the retry.
                moves = capture_status_options(mon, {'species': target['species'],
                                                      'status': 'None'}, {})
                if moves:
                    matchups.append({**target, 'non_damaging_status_moves': [
                        {k: v for k, v in move.items() if k != 'capture_probability_if_status_lands'}
                        for move in moves]})
            if not matchups:
                continue
            key = f'prepare:retrieve-capture-support:{mon["box"]}:{mon["index"]}'
            groups[key] = {
                'target': ('pokemon', mon['species'], None), 'rules': rules,
                'objectives': [f'Withdraw {mon["species"]} as capture status support'],
                'context': {'optional_preparation': True, 'storage_retrieval': True,
                            'capture_support_retrieval': True, 'stored_pokemon': mon,
                            'required_for': sorted({row['species'] for row in matchups}),
                            'capture_support_matchups': matchups,
                            'requires_party_deposit': len(facts.get('party', [])) >= 6,
                            'requires_healing': mon['hp'] < mon['max_hp'] or mon.get('status', 'None') != 'None',
                            'current_party_capture_tools': self.collection_resources(facts),
                            'scope': 'Owned stored Pokemon with observed non-damaging sleep/paralysis PP. '
                                     'Normal PC withdrawal and any deposit/healing still required. '
                                     'Move compatibility assumes a clean encounter, not status success: '
                                     'compare level, HP, accuracy and matchup; switching consumes a turn '
                                     'and the support may faint before acting. No survival guarantee.'}}

    def add_capture_support_training(self, groups, facts):
        """Offer bounded, real XP preparation after an observed failed setup.

        A one-level step is not a claim that the next retry will be safe. Jev
        compares its cost with other preparation and acquisition candidates.
        """
        if not self.collects_dex:
            return
        owned = self.validated_owned(facts)
        failures = []
        for retreat in getattr(self, 'capture_retreats', {}).values():
            observation = retreat.get('retreat_observation') or {}
            enemy = observation.get('enemy') or {}
            if (retreat['species'] not in owned and enemy.get('level')
                    and any(mon.get('hp') == 0 for mon in observation.get('party') or [])):
                failures.append({'map': retreat['map'], 'species': retreat['species'],
                                 'level': enemy['level'], 'observation': observation})
        if not failures:
            return
        highest = max(row['level'] for row in failures)
        trainees = {}
        seen = set()
        for mon in facts.get('party', []):
            if mon['species'] in seen:
                continue
            seen.add(mon['species'])  # lead_with uses the first matching slot.
            if mon['hp'] < mon['max_hp'] * .7 or mon.get('status', 'None') != 'None':
                continue  # Heal through existing recovery first, then train.
            moves = [move for move in mon.get('moves', []) if move != 'None'
                     and data.move_data(move).get('power', 0) == 0
                     and data.move_data(move).get('effect') in ('SleepEffect', 'ParalyzeEffect')]
            if moves and mon['level'] < min(100, highest):
                trainees.setdefault(mon['species'], (mon, moves))
        if not trainees:
            return
        sites = self.find_training_sites(facts, shared_experience=True)
        for source, (mon, moves) in sorted(trainees.items()):
            target_level = mon['level'] + 1
            target = ('level', source, target_level)
            if self.index.satisfied(target, facts):
                continue
            rules = [Rule(f'train-support:{source}:{name}:{target_level}', name,
                          'skill:train_encounter', [], [], [], target, []) for name in sites]
            if not rules:
                continue
            participants = 2 if any(other['hp'] > 0 and other['level'] > mon['level']
                                     for other in facts['party']) else 1
            groups[f'prepare:capture-support:{source}'] = {
                'target': target, 'rules': rules,
                'objectives': ['Improve a capture status support through normal experience battles'],
                'context': {'optional_preparation': True, 'capture_support_training': True,
                            'trigger': 'level', 'from_species': source, 'level': target_level,
                            'trainee': mon, 'safe_status_moves': moves,
                            'level_gap_to_highest_observed_target': highest - mon['level'],
                            'observed_failed_capture_setups': failures,
                            'training_cost': evolution_training_cost(mon, target_level),
                            'training_cost_to_observed_target_level': evolution_training_cost(mon, highest),
                            'training_effort_examples': [{'map': name, **effort} for name in sites
                                if (effort := evolution_training_effort(mon, target_level,
                                    ((self.maps[name].get('wild') or {}).get('red') or {}).get('grass'),
                                    participants))],
                            'scope': 'One real level gain, then reassess; not a survival guarantee. '
                                     'Trainee must remain conscious to share victory experience; '
                                     'healing and switching still require normal menus.'}}

    def item_evolution_spending_reference(self, facts, money_after):
        """Expose alternative held-source stone costs; never reserve cash or cut options."""
        graph = self.complete_collection_graph()
        owned = self.validated_owned(facts)
        plan = solo_plan(graph, owned, infer_solo_choices(owned))
        party, stored = facts.get('party', []), facts.get('stored_pokemon', [])
        catalog = {name.replace('_', '').upper(): info for name, info in ITEM_CATALOG.items()}
        bag = Counter()
        for name, quantity in facts.get('bag', {}).items():
            bag[name.replace('_', '').upper()] += quantity
        options = []
        for species in sorted(set(plan['choice_reachable_species']) - owned):
            for method in graph.get(species, []):
                if (method['method'] != 'evolution' or method.get('trigger') != 'item'
                        or method.get('external_trade')):
                    continue
                group = method.get('exclusive_group')
                if group and method.get('choice') not in plan['optimal_choices'].get(group, ()):
                    continue
                source = method['from_species']
                party_count = sum(self.same_species(mon.get('species'), source) for mon in party)
                stored_count = sum(self.same_species(mon.get('species'), source) for mon in stored)
                if not party_count + stored_count:
                    continue
                item = method['item']
                key = item.replace('_', '').upper()
                price = catalog.get(key, {}).get('price') or 0
                price = price if price > 0 else None
                quantity = bag[key]
                needed = 0 if quantity > 0 else price
                before = facts['money'] >= needed if needed is not None else None
                after = money_after >= needed if needed is not None else None
                options.append({'species': species, 'from_species': source,
                    'source_party_count': party_count, 'source_stored_count': stored_count,
                    'required_item': item, 'item_quantity_held': quantity,
                    'item_unit_price_reference': price, 'cash_needed_for_one_evolution': needed,
                    'affordable_before_purchase': before, 'affordable_after_purchase': after,
                    'purchase_removes_affordability': before is True and after is False})
        return {'money_before_purchase': facts['money'], 'money_after_purchase': money_after,
                'held_source_options': options,
                'scope': 'Independent alternatives for observed party/PC sources, not a joint registration yield: '
                         'money, carried stones and source individuals are shared. Carried stones need no '
                         'repurchase; PC sources still require normal withdrawal. Positive catalog prices '
                         'are references, not proof of shop access, bag space or an executed evolution; '
                         'a missing/nonpositive purchase price is unknown, not a free stone. '
                         'No fixed cash reserve and no candidates are removed.'}

    def add_ball_supply(self, groups, facts):
        """Expose real scripted ball sources as well as normal shop restocking.

        That restock is defeat-triggered: a collector at full health would never
        be offered it and would simply stop catching once the bag ran dry.
        """
        if not self.collects_dex:
            return
        normalized = {name.replace('_', '').upper(): name for name in BALLS}
        carried = {normalized[key]: qty for key, qty in facts['bag'].items() if key in normalized}
        # A reserve of ordinary balls does not replace a different ball's
        # capture capability. Keep unclaimed gifts/pickups available even at
        # full reserve; frontier retains their real guards and prerequisites.
        effects = dict.fromkeys(rule.effect for rule in self.index.rules
            if rule.effect[0] == 'item' and rule.effect[2]
            and rule.effect[1].replace('_', '').upper() in normalized)
        for effect in effects:
            name = normalized[effect[1].replace('_', '').upper()]
            rules = self.index.frontier(effect, facts)
            if not rules:
                continue
            groups[f'ball-source:{effect[1]}'] = {'target': effect, 'rules': rules,
                'objectives': ['Acquire a scripted ball gift or pickup for future captures'],
                'context': {'optional_preparation': True, 'collecting': True,
                            'ball': name, 'item': BALLS[name],
                            'capture_behavior': ('Guaranteed capture of a catchable wild target without '
                                'weakening or status setup; consumed on use. Not usable on trainer Pokémon.'
                                if name == 'MasterBall' else
                                'Capture probability depends on the ball, species catch rate, HP and status; '
                                'not a guaranteed capture.'),
                            'source_kind': 'scripted_item',
                            'source_maps': sorted({r.map for r in self.index.rules if r.effect == effect}),
                            'occupied_bag_slots': len(facts['bag']), 'bag_capacity': 20,
                            'scope': 'Unclaimed script source, not an owned ball. Normal navigation, '
                                     'script guards and bag space still apply; one-time rewards cannot be replenished.'}}
        owned = self.validated_owned(facts)
        targets = []
        for species, methods in self.complete_collection_graph().items():
            if species in owned:
                continue
            ready = sorted({method['map'] for method in methods
                            if method['method'] == 'static' and any(not rule.missing(facts)
                                for rule in self.acquisition_story_rules(species, method))})
            if ready:
                targets.append({'species': species, 'catch_rate': data.species_data(species)['catchRate'],
                                'ready_source_maps': ready})
        held = sum(carried.values())
        if held >= self.BALL_RESERVE and not targets:
            return
        nearest = {}
        for rule in self.index.rules:
            if rule.effect[0] != 'shop' or rule.missing(facts):
                continue
            route = self.client.route(facts['map'], rule.map)
            if not route.get('found'):
                continue
            hops = len(route.get('legs', []))
            for stock_index, key in enumerate(rule.effect[1]):
                name = normalized.get(key.replace('_', '').upper())
                if not name:
                    continue
                # The two nearest shops per ball kind. The nearest by map hops
                # can still be tile-unreachable (a water crossing the map-level
                # router ignores), and one blocked trip strikes that rule out
                # after two failures — a runner-up keeps the supply line alive.
                entry = (hops, rule.map, rule.id, stock_index, rule)
                shops = nearest.setdefault(name, [])
                if entry not in shops:
                    shops.append(entry)
                    shops.sort(key=lambda item: item[:3])
                    del shops[2:]
        for name, shops in sorted(nearest.items()):
            info = BALLS[name]
            if len(facts['bag']) >= 20 and name not in carried:
                continue
            current = carried.get(name, 0)
            # The ordinary reserve is not a capture-sufficiency limit. Offer
            # larger optional stocks for known unregistered static sources;
            # Jev still compares money, setup and all other acquisition goals.
            batches = [('reserve', self.BALL_RESERVE - held)]
            if targets:
                batches += [('extended', self.BALL_RESERVE * 3 - current), ('stack', 99 - current)]
            offered = set()
            for batch, desired in batches:
                qty = min(desired, 99 - current, int(facts['money'] * .6) // max(1, info['price']))
                if qty < 1 or qty in offered:
                    continue
                offered.add(qty)
                total = current + qty
                cost = qty * info['price']
                reference = capture_supply_reference(targets, carried, {**carried, name: total}) if targets else None
                spending = self.item_evolution_spending_reference(facts, facts['money'] - cost)
                for hops, map_name, _rule_id, stock_index, rule in shops:
                    key = f'ball:{rule.id}:{name}' + (f':stock{total}' if batch != 'reserve' else '')
                    groups[key] = {'target': ('supply', name, total), 'rules': [rule],
                        'objectives': ['Buy balls to keep collecting unregistered species'],
                        'context': {'optional_preparation': True, 'stock_index': stock_index,
                                    'item': info, 'collecting': True, 'batch': batch,
                                    'purchase_quantity': qty, 'target_quantity': total,
                                    'total_cost': cost, 'money_after_purchase': facts['money'] - cost,
                                    'capture_supply_reference': reference,
                                    'item_evolution_spending_reference': spending,
                                    'map': map_name, 'map_hops': hops}}

    def add_recovery_groups(self, groups, facts):
        self.add_ball_supply(groups, facts)
        normalized = {name.replace('_', '').upper(): name for name in MEDICINES}
        bag = {normalized[key]: qty for key, qty in facts['bag'].items() if key in normalized}
        options = list(medicine_options(facts['party'], bag))
        if any('pp' not in details['tags'] for _, _, details in options):
            target = ('health', 'party', True)
            groups['prepare:medicine'] = {'target': target,
                'rules': [Rule('medicine', facts['map'], 'skill:medicine', [], [], [], target, [])],
                'objectives': ['Recover party HP or status with owned medicine'],
                'context': {'optional_preparation': True, 'available_medicine': bag}}
        if any('pp' in details['tags'] for _, _, details in options):
            target = ('pp_reserve', 'party', True)
            groups['prepare:pp'] = {'target': target,
                'rules': [Rule('pp', facts['map'], 'skill:medicine', [], [], [], target, [])],
                'objectives': ['Restore depleted attacks with owned PP medicine'],
                'context': {'optional_preparation': False, 'available_medicine': bag}}
        exhausted_defeats = [d for d in self.battle_defeats if not d.get('resolved_by_victory') and d.get('preparation_signature')
            and any(move != 'None' and data.move_data(move)['power'] > 0 and pp == 0
                    for move, pp in zip(d['preparation_signature']['party'][0]['moves'],
                                        d['preparation_signature']['party'][0].get('pp', [])))]
        if exhausted_defeats and not any('pp' in MEDICINES[name].get('tags', []) for name in bag):
            for name, item in MEDICINES.items():
                if item['effect']['type'] != 'PpRestore' or not item['effect']['params'].get('all'):
                    continue
                # Item identifiers in script effects may contain underscores.
                for effect in self.index.by_effect:
                    if effect[0] != 'item' or not effect[2] or effect[1].replace('_', '').upper() != name.upper():
                        continue
                    rules = [r for r in self.index.frontier(effect, facts) if r.map in self.visited]
                    if rules:
                        groups['reserve:' + name] = {'target': effect, 'rules': rules,
                            'objectives': ['Collect PP recovery for an observed failure with exhausted attacks'],
                            'context': {'optional_preparation': False, 'item': item,
                                        'observed_defeats': exhausted_defeats[-1:]}}
        # Only observed defeats justify stocking supplies. Limit travel and
        # spending, and let Jev compare these options with training/retrying.
        if not any(not d.get('resolved_by_victory') for d in self.battle_defeats):
            return
        hp_stock = sum(qty for name, qty in bag.items() if 'hp' in MEDICINES[name].get('tags', []))
        if hp_stock >= 6:
            return
        leader = facts['party'][0]
        for rule in self.index.rules:
            if rule.effect[0] != 'shop' or rule.map not in self.visited or rule.missing(facts):
                continue
            route = self.client.route(facts['map'], rule.map)
            if not route.get('found') or len(route.get('legs', [])) > 3:
                continue
            if facts['money'] < 6000:
                for name, item in ITEM_CATALOG.items():
                    qty = facts['bag'].get(name.replace('_', '').upper(), 0)
                    if qty and item.get('sellable') and not item.get('key_item') and 'treasure' in item.get('tags', []):
                        groups[f'sell:{rule.id}:{name}'] = {'target': ('sale', name, False), 'rules': [rule],
                            'objectives': ['Sell a treasure item to fund recovery supplies'],
                            'context': {'optional_preparation': True, 'item': item, 'quantity': qty,
                                        'expected_proceeds': qty*item['price']//2}}
            for stock_index, key in enumerate(rule.effect[1]):
                item = normalized.get(key.replace('_', '').upper())
                if not item:
                    continue
                info = MEDICINES[item]
                if 'hp' not in info.get('tags', []):
                    continue
                amount = info['effect'].get('params', {}).get('amount', leader['max_hp'])
                if amount < leader['max_hp'] * .7:
                    continue
                qty = min(6-hp_stock, int(facts['money']*.6)//max(1, info['price']))
                if qty < 1 or (len(facts['bag']) >= 20 and item not in bag):
                    continue
                target = ('supply', item, bag.get(item, 0)+qty)
                groups[f'supply:{rule.id}:{item}'] = {'target': target, 'rules': [rule],
                    'objectives': ['Buy recovery supplies for battles that caused an observed defeat'],
                    'context': {'optional_preparation': True, 'stock_index': stock_index, 'item': info,
                                'quantity_to_buy': qty, 'total_cost': qty*info['price'],
                                'remaining_money': facts['money']-qty*info['price'],
                                'use': 'Usable in battle or between fights when a nurse is inaccessible'}}

    def inventory_use_options(self, facts, free_slot=True):
        options = []
        for i, mon in enumerate(facts['party']):
            if facts['bag'].get('RARECANDY') == 1 and mon['level'] < 100:
                options.append((f'use_item:RareCandy,{i}', {'pokemon': mon,
                    'effect': 'Raise this party member one level and free the last Rare Candy slot'}))
            if mon['hp'] <= 0:
                continue
            for n, move in enumerate(TM_MOVES):
                qty = facts['bag'].get(f'TM{n+1:02d}', 0)
                if (qty != 1 if free_slot else qty < 1) or move in mon['moves'] or not machine_compatible(mon['species'], n):
                    continue
                forgets = replacement_options(mon, move)
                for forget in forgets:
                    options.append((f'teach_tm:Tm{n+1:02d},{move},{i},{forget or "None"}',
                        {'pokemon': mon, 'learn': move, 'forget': forget,
                         'move_data': {m: data.move_data(m) for m in [move, *mon['moves']] if m != 'None'},
                         'effect': 'Consume the last copy of this compatible TM, teach its move, and free one inventory slot'}))
        return options

    def action_candidates(self, facts):
        candidates, bindings = self._action_candidates(facts)
        if getattr(self, 'collects_dex', False):
            self.add_transit_lead_candidates(candidates, bindings, facts)
        return candidates, bindings

    def add_transit_lead_candidates(self, candidates, bindings, facts):
        """Expose ordinary party preparation beside travel, never select it."""
        trips = [(operation, rule) for operation, rule in bindings.values()
                 if operation.startswith(('travel_to:', 'reach_training:', 'surf:'))]
        party = facts.get('party', [])
        if not trips or len(party) < 2:
            return
        operations = {operation for operation, _ in bindings.values()}
        if any(operation.startswith('train_encounter:') for operation in operations):
            return  # Already at a training point: retain the trainee for experience.
        context = (getattr(self, 'active', None) or {}).get('context', {})
        trainee = (context.get('from_species') if context.get('capture_support_training')
                   or (context.get('acquisition_method') == 'evolution'
                       and context.get('trigger') == 'level') else None)
        regions = {facts.get('map'), context.get('map'), context.get('destination')}
        for key, (operation, _) in bindings.items():
            if operation.startswith(('travel_to:', 'reach_training:')):
                regions.add(operation.split(':', 1)[1].split(',')[0])
                regions.update((json.loads(candidates[key]).get('navigation') or {}).get('via', []))
        encounters = []
        for name in sorted(regions - {None}):
            for method, table in ((getattr(self, 'maps', {}).get(name, {}).get('wild') or {}).get('red') or {}).items():
                mons = table.get('mons', []) if table else []
                if not mons:
                    continue
                encounters.append({'map': name, 'terrain': method,
                    'level_range': [min(mon['level'] for mon in mons), max(mon['level'] for mon in mons)],
                    'species': sorted({mon['species'] for mon in mons})})
        for key, (operation, _) in bindings.items():
            if operation.startswith(('travel_to:', 'reach_training:', 'surf:')):
                candidates[key] = json.dumps({**json.loads(candidates[key]),
                    'current_leader': party[0], 'route_encounters': encounters,
                    'encounter_scope': 'Public wild tables for known trip regions, not predicted battles or complete intermediate-route coverage'})
        # lead_with names the first matching species. Do not describe a later
        # duplicate's HP/moves while the menu skill would select the first.
        offered = {party[0]['species']}
        for mon in party[1:]:
            if mon['species'] in offered:
                continue
            offered.add(mon['species'])
            operation = f'lead_with:{mon["species"]}'
            if mon['hp'] <= 0 or mon['species'] == trainee or operation in operations:
                continue
            key = f'action:{len(candidates)}'
            while key in candidates:
                key += ':lead'
            candidates[key] = json.dumps({
                'operation': operation, 'current_leader': party[0], 'transit_leader': mon,
                'route_encounters': encounters,
                'purpose': 'Optionally change the leader before normal travel or a water crossing; compare survival and escape capability against the current leader',
                'scope': 'Party preparation only, not experience or a capture status turn. Other party members remain available; selecting a different leader does not guarantee safety.'})
            bindings[key] = operation, trips[0][1]

    def _action_candidates(self, facts):
        preparing_training = self.active['target'][:2] == ('level', 'leader')
        preparing_capture = (self.active['target'][0] in ('catch', 'held_species')
                             and self.active.get('context', {}).get('acquisition_method') != 'safari')
        if preparing_training or preparing_capture:
            battler = training_battler(facts.get('party', []))
            if battler and facts['party'][0] is not battler:
                operation = f'lead_with:{battler["species"]}'
                return {'action:0': json.dumps({
                    'operation': operation,
                    'purpose': ('Put the strongest battle-ready party member in front before training'
                                if preparing_training else
                                'Put a capable battler in front to survive capture attempts and clear duplicate encounters'),
                    'target_level': self.active['target'][2] if preparing_training else None,
                    'training_battler': battler,
                })}, {'action:0': (operation, self.active['rules'][0])}
        if self.active.get('context', {}).get('coin_purchase'):
            candidates, bindings = {}, {}
            npcs = self.client.cmd(cmd='get_npcs')
            for rule in self.active['rules']:
                if facts['map'] != rule.map:
                    operation = f'travel_to:{rule.map}'
                    key = f'action:{len(candidates)}'
                    candidates[key] = json.dumps({'operation': operation,
                        'purpose': 'Reach the Game Corner coin counter'})
                    bindings[key] = operation, rule
                    continue
                ids = {int(trigger.split(':')[1]) for trigger in rule.triggers
                       if trigger.startswith('npc:')}
                for npc in npcs:
                    if npc.get('visible', True) and npc.get('text_id') in ids:
                        required = self.active['target'][2]
                        approaches = list(counter_approaches(rule.map, npc))
                        if approaches:
                            for (x, y), direction in approaches:
                                operation = (f'buy_coins:{required},{npc["npc_index"]},'
                                             f'{x},{y},{direction}')
                                key = f'action:{len(candidates)}'
                                candidates[key] = json.dumps({'operation': operation,
                                    'counter_approach': [x, y, direction], **self.active['context']})
                                bindings[key] = operation, rule
                        else:
                            operation = f'buy_coins:{required},{npc["npc_index"]}'
                            key = f'action:{len(candidates)}'
                            candidates[key] = json.dumps({'operation': operation,
                                **self.active['context']})
                            bindings[key] = operation, rule
            return candidates, bindings
        if self.active.get('context', {}).get('storage_party_space'):
            candidates, bindings = {}, {}
            for rule in self.active['rules']:
                if facts['map'] != rule.map:
                    operation = f'travel_to:{rule.map}'
                    key = f'action:{len(candidates)}'
                    candidates[key] = json.dumps({'operation': operation,
                        'purpose': 'Reach a PC to make one free party slot'})
                    bindings[key] = operation, rule
                    continue
                sign_ids = {int(trigger.split(':')[1]) for trigger in rule.triggers
                            if trigger.startswith('sign:')}
                signs = json.loads((self.index.maps_dir / rule.map / 'map.json').read_text()).get('signs', [])
                for sign_index, sign in enumerate(signs):
                    if sign.get('textId') not in sign_ids:
                        continue
                    for deposit in storage_deposit_indices(facts['party']):
                        mon = facts['party'][deposit]
                        operation = f'deposit_pc:{deposit},{sign_index}'
                        key = f'action:{len(candidates)}'
                        candidates[key] = json.dumps({'operation': operation,
                            'deposit': mon, 'required_for': self.active['context']['required_for']})
                        bindings[key] = operation, rule
            return candidates, bindings
        if self.active.get('context', {}).get('storage_change_box'):
            candidates, bindings = {}, {}
            for rule in self.active['rules']:
                if facts['map'] != rule.map:
                    operation = f'travel_to:{rule.map}'
                    key = f'action:{len(candidates)}'
                    candidates[key] = json.dumps({'operation': operation,
                        'purpose': 'Reach a PC to select a box with free capacity'})
                    bindings[key] = operation, rule
                    continue
                sign_ids = {int(trigger.split(':')[1]) for trigger in rule.triggers
                            if trigger.startswith('sign:')}
                signs = json.loads((self.index.maps_dir / rule.map / 'map.json').read_text()).get('signs', [])
                for sign_index, sign in enumerate(signs):
                    if sign.get('textId') not in sign_ids:
                        continue
                    for box_index in self.active['context']['available_boxes']:
                        operation = f'change_pc_box:{box_index},{sign_index}'
                        key = f'action:{len(candidates)}'
                        candidates[key] = json.dumps({'operation': operation,
                            'new_box': box_index, 'box_counts': facts.get('box_counts', [])})
                        bindings[key] = operation, rule
            return candidates, bindings
        if self.active.get('context', {}).get('storage_retrieval'):
            candidates, bindings = {}, {}
            stored = self.active['context']['stored_pokemon']
            for rule in self.active['rules']:
                if facts['map'] != rule.map:
                    operation = f'travel_to:{rule.map}'
                    key = f'action:{len(candidates)}'
                    candidates[key] = json.dumps({
                        'operation': operation,
                        'purpose': f'Reach a PC to withdraw {stored["species"]}',
                        'storage': stored})
                    bindings[key] = operation, rule
                    continue
                sign_ids = {int(trigger.split(':')[1]) for trigger in rule.triggers
                            if trigger.startswith('sign:')}
                signs = json.loads((self.index.maps_dir / rule.map / 'map.json').read_text()).get('signs', [])
                for sign_index, sign in enumerate(signs):
                    if sign.get('textId') not in sign_ids:
                        continue
                    deposits = [-1] if len(facts['party']) < 6 else storage_deposit_indices(facts['party'])
                    for deposit in deposits:
                        operation = (f'retrieve_pc:{stored["box"]},{stored["index"]},'
                                     f'{deposit},{sign_index}')
                        key = f'action:{len(candidates)}'
                        description = {
                            'operation': operation, 'withdraw': stored,
                            'purpose': 'Use the real PC menus to put the required stored Pokémon into the party'}
                        if deposit >= 0:
                            description['deposit_first'] = facts['party'][deposit]
                        candidates[key] = json.dumps(description)
                        bindings[key] = operation, rule
            return candidates, bindings
        if ((self.active['target'][0] == 'register'
                and self.active.get('context', {}).get('acquisition_method') == 'evolution')
                or self.active.get('context', {}).get('capture_support_training')):
            context = self.active['context']
            support_training = bool(context.get('capture_support_training'))
            source = context['from_species']
            indices = [i for i, mon in enumerate(facts['party'])
                       if self.same_species(mon['species'], source)]
            candidates, bindings = {}, {}
            if not indices:
                return candidates, bindings
            rule = self.active['rules'][0]
            index = indices[0]
            if context['trigger'] == 'item':
                item = context['item']
                operation = f'use_item:{item},{index}'
                candidates['action:0'] = json.dumps({
                    'operation': operation, 'pokemon': facts['party'][index],
                    'evolves_into': context['species'], 'item': item})
                bindings['action:0'] = operation, rule
                return candidates, bindings
            sites = self.find_training_sites(facts)
            position = (facts.get('map'), facts.get('x'), facts.get('y'))
            at_training_point = any(position == (name, *point) for name, point in sites.items())
            if index and at_training_point:
                operation = f'lead_with:{source}'
                candidates['action:0'] = json.dumps({
                    'operation': operation,
                    'purpose': f'Move {source} to the lead slot so it can participate and earn experience',
                    'evolves_into': context.get('species') if not support_training else None})
                bindings['action:0'] = operation, rule
                return candidates, bindings
            for name, (x, y) in sites.items():
                if position != (name, x, y):
                    operation = f'reach_training:{name},{x},{y}'
                    key = f'action:{len(candidates)}'
                    candidates[key] = json.dumps({
                        'operation': operation,
                        'purpose': f'Reach the training point before moving {source} to the lead; transit encounters give no experience when escaped',
                        'trainee': facts['party'][index], 'current_transit_leader': facts['party'][0],
                        'navigation': getattr(self, 'training_navigation', {}).get(name),
                        'scope': 'Normal travel only; avoid incidental grass where possible. Battles and healing may interrupt. No party reorder or training until arrival.'})
                    bindings[key] = operation, rule
                    continue
                operation = f'train_encounter:{name},{x},{y}'
                key = f'action:{len(candidates)}'
                candidates[key] = json.dumps({
                    'operation': operation,
                    'purpose': (f'Gain a level with {source} to improve capture status support'
                                if support_training else f'Gain a level with {source} to evolve it into {context["species"]}'),
                    'required_level': context.get('experience_trigger_level', context['level']),
                    'natural_evolution_level': context['level'] if not support_training else None,
                    'evolution_trigger_scope': context.get('evolution_trigger_scope'),
                    'current_level': facts['party'][0]['level'],
                    'training_cost': evolution_training_cost(facts['party'][0],
                        context.get('experience_trigger_level', context['level'])),
                    'encounter_yield': training_yield(
                        ((getattr(self, 'maps', {}).get(name, {}).get('wild') or {}).get('red') or {}).get('grass'),
                        2 if any(mon['hp'] > 0 and mon['level'] > facts['party'][0]['level']
                                 for mon in facts['party'][1:]) else 1),
                    'navigation': getattr(self, 'training_navigation', {}).get(name)})
                bindings[key] = operation, rule
            return candidates, bindings
        if self.active['target'][0] in ('health', 'pp_reserve'):
            names = {name.replace('_', '').upper(): name for name in MEDICINES}
            bag = {names[key]: qty for key, qty in facts['bag'].items() if key in names}
            candidates, bindings = {}, {}
            for item, index, details in medicine_options(facts['party'], bag):
                if ('pp' in details['tags']) != (self.active['target'][0] == 'pp_reserve'):
                    continue
                key = f'action:{len(candidates)}'
                operation = f'use_item:{item},{index}'
                candidates[key] = json.dumps({'operation': operation, **details})
                bindings[key] = operation, self.active['rules'][0]
            return candidates, bindings
        if self.active['target'][0] == 'bag_space':
            candidates, bindings = {}, {}
            for i, (operation, details) in enumerate(self.inventory_use_options(facts)):
                key = f'action:{i}'
                candidates[key] = json.dumps({'operation': operation, **details})
                bindings[key] = operation, self.active['rules'][0]
            return candidates, bindings
        if self.active['target'][0] == 'move':
            move = self.active['target'][1]
            candidates, bindings = {}, {}
            if move not in HM_MOVES:
                for operation, details in self.inventory_use_options(facts, free_slot=False):
                    if details.get('learn') == move:
                        key = f'action:{len(candidates)}'
                        candidates[key] = json.dumps({'operation': operation, **details})
                        bindings[key] = operation, self.active['rules'][0]
                return candidates, bindings
            for i, mon in enumerate(facts['party']):
                if mon['hp'] <= 0 or not hm_compatible(mon['species'], move):
                    continue
                forgets = replacement_options(mon, move)
                for forget in forgets:
                    key = f'action:{len(candidates)}'
                    operation = f'learn:{move},{i},{forget or "None"}'
                    candidates[key] = json.dumps({'operation': operation, 'pokemon': mon,
                        'forget': forget, 'learn': move, 'move_data': {m: data.move_data(m) for m in mon['moves'] if m != 'None'}})
                    bindings[key] = operation, self.active['rules'][0]
            return candidates, bindings
        if self.active['target'][0] in ('terrain', 'location'):
            obstacle = self.active['context']
            candidates, bindings = {}, {}
            for i, mon in enumerate(facts['party']):
                if obstacle['move'] in mon['moves']:
                    operation = f"{obstacle['move'].lower()}:{i}"
                    key = f'action:{i}'
                    candidates[key] = json.dumps({'operation': operation,
                        'purpose': f"Approach and use {obstacle['move']} through the party menu to pass this terrain", 'terrain': obstacle})
                    bindings[key] = operation, self.active['rules'][0]
            return candidates, bindings
        if self.active['target'][0] in ('catch', 'held_species'):
            candidates, bindings = {}, {}
            area_key = self.active.get('context', {}).get('catch_area', self.active['target'][1])
            area = getattr(self, 'catch_areas', {}).get(area_key)
            if area:
                rule = self.active['rules'][0]
                method, name = area.get('method', 'grass'), area.get('map', rule.map)
                spot = area.get('spot') or (area.get('spots') or [None])[0]
                if method == 'fishing':
                    (x, y), direction = spot
                    operation = f"catch_encounter:fishing,{name},{x},{y},{direction},{area['rod']}"
                elif method == 'water':
                    (x, y), direction, (wx, wy) = spot
                    operation = f'catch_encounter:water,{name},{x},{y},{direction},{wx},{wy}'
                else:
                    x, y = spot
                    operation = (f'catch_encounter:{name},{x},{y}' if method == 'grass'
                                 else f'catch_encounter:{method},{name},{x},{y}')
                key = 'action:0'
                candidates[key] = json.dumps({'operation': operation,
                    'purpose': f"Travel to {name} and use its {method} acquisition method until a wild battle starts; then decide whether to catch the observed species.",
                    'navigation': area.get('navigation') or self.catch_navigation.get(area_key), 'acquisition_method': method,
                    'rod': area.get('rod'), 'unregistered_species_here': area['species'],
                    'encounter_value': area.get('encounter_value')})
                bindings[key] = operation, rule
            return candidates, bindings
        if self.active['target'][0] != 'level':
            candidates, bindings = super().action_candidates(facts)
            npcs = {n['npc_index']: n for n in self.client.cmd(cmd='get_npcs')}
            for rule in self.active['rules']:
                if rule.map != facts['map'] or rule.missing(facts):
                    continue
                if rule.id.startswith('boulder:'):
                    holes = {tuple(v['target']) for v in BOULDER_TARGETS.values()
                             if v['map'] == rule.map and v['falls']}
                    occupied = {(n['x'], n['y']) for n in npcs.values() if n.get('visible', True)}
                    local_approach = any(pt.bfs(rule.map, (facts['x'], facts['y']), point,
                        blocked=occupied | holes | pt.warp_tiles(rule.map), allow_spinners=True)
                        for point in self.destination_points(rule.map, rule))
                    if not local_approach:
                        key = f'action:{len(candidates)}'
                        operation = f'travel_to:{rule.map}'
                        candidates[key] = json.dumps({'operation': operation,
                            'purpose': 'Reach the region containing the target boulder before using Strength',
                            'script_effects': rule.description()})
                        bindings[key] = operation, rule
                        continue
                    for i, mon in enumerate(facts['party']):
                        if 'Strength' not in mon['moves']:
                            continue
                        key = f'action:{len(candidates)}'
                        operation = f'push_puzzle:{i},{rule.effect[1]}'
                        candidates[key] = json.dumps({'operation': operation,
                            'purpose': 'Use Strength and search real boulder pushes to activate the engine-defined target',
                            'puzzle': BOULDER_TARGETS[rule.effect[1]]})
                        bindings[key] = operation, rule
                for x, y in NATIVE_INTERACTIONS.get(rule.storyline, []):
                    key = f'action:{len(candidates)}'
                    operation = f'interact_tile:{x},{y}'
                    candidates[key] = json.dumps({'operation': operation, 'script_effects': rule.description()})
                    bindings[key] = operation, rule
            for key, (operation, rule) in list(bindings.items()):
                if operation.startswith('move_to:'):
                    point = tuple(int(v) for v in operation.split(':')[1].split(','))
                    blocked = {(n['x'], n['y']) for n in npcs.values() if n.get('visible', True)}
                    if not pt.walkable(facts['map'], *point) or not trigger_position_matches(rule, point):
                        # A solid OnStep tile does not imply an A interaction.
                        # Engine-declared OnInteract bindings are added above.
                        del candidates[key], bindings[key]
                        continue
                    if not pt.bfs(facts['map'], (facts['x'], facts['y']), point,
                                    blocked=blocked | (pt.warp_tiles(facts['map']) - {point}), allow_spinners=True):
                        operation = f'travel_to:{rule.map}'
                    bindings[key] = operation, rule
                    candidates[key] = json.dumps({'operation': operation, 'script_effects': rule.description()})
                if operation.startswith('interact_tile:'):
                    point = tuple(int(v) for v in operation.split(':')[1].split(','))
                    blocked = {(n['x'], n['y']) for n in npcs.values() if n.get('visible', True)}
                    if not any(pt.bfs(facts['map'], (facts['x'], facts['y']), stance,
                                      blocked=blocked | pt.warp_tiles(facts['map']), allow_spinners=True)
                               for stance, _ in self.interaction_approaches(rule, *point)):
                        del candidates[key], bindings[key]
                        continue
                if operation.startswith('interact_with:npc:'):
                    index = int(operation.rsplit(':', 1)[1])
                    options = list(counter_approaches(facts['map'], npcs[index]))
                    blocked = {(n['x'], n['y']) for n in npcs.values() if n.get('visible', True)}
                    approaches = [p for p, _ in options] or [
                        (npcs[index]['x']+dx, npcs[index]['y']+dy) for dx, dy in pt.DELTA.values()]
                    if not any(pt.bfs(facts['map'], (facts['x'], facts['y']), point,
                                      blocked=blocked | pt.warp_tiles(facts['map']), allow_spinners=True)
                               for point in approaches):
                        # A single map can have disconnected sections joined
                        # through a gate. Use cross-map navigation to approach
                        # the trigger even when its map ID already matches.
                        operation = f'travel_to:{rule.map}'
                        bindings[key] = operation, rule
                        candidates[key] = json.dumps({'operation': operation, 'script_effects': rule.description()})
                        continue
                    if options:
                        (x, y), direction = min(options, key=lambda option:
                            abs(option[0][0]-facts['x'])+abs(option[0][1]-facts['y']))
                        operation = f'interact_counter:{x},{y},{direction},{index}'
                        bindings[key] = operation, rule
                        candidates[key] = json.dumps({'operation': operation,
                            'purpose': 'Walk to the accessible side of the counter, face the NPC, and talk across it',
                            'script_effects': rule.description()})
                if operation.startswith('travel_to:'):
                    route = self.client.route(facts['map'], rule.map)
                    via = [leg['to_map'] for leg in route.get('legs', [])]
                    description = json.loads(candidates[key])
                    description['navigation'] = {
                        'map_hops': len(via), 'via': via,
                        'unvisited_maps': [name for name in via if name not in self.visited],
                        'caution': 'Travel can consume HP and PP in encounters; recovery should prefer a short known route.',
                    }
                    candidates[key] = json.dumps(description)
            # A door's OnStep coordinate can lie on its inaccessible side
            # while its native A interaction is already reachable. The
            # converted travel then selects our current approach tile and
            # returns "reached" forever. Remove only that proven no-op, not
            # cross-region travel or actual reachable step triggers.
            interactable = {rule.id for operation, rule in bindings.values()
                            if operation.startswith('interact_tile:')}
            for key, (operation, rule) in list(bindings.items()):
                if (operation == f'travel_to:{facts["map"]}' and rule.id in interactable
                        and (facts['x'], facts['y']) in self.destination_points(rule.map, rule)):
                    del candidates[key], bindings[key]
            return candidates, bindings
        candidates, bindings = {}, {}
        rules = self.active['rules']
        reachable = [r for r in rules if getattr(self, 'training_navigation', {}).get(r.map, {}).get('tile_route_found')]
        for rule in reachable or rules:
            x, y = self.training_sites[rule.map]
            operation = f'train_encounter:{rule.map},{x},{y}'
            key = f'action:{len(candidates)}'
            candidates[key] = json.dumps({'operation': operation,
                                          'purpose': f'Travel to grass in {rule.map}, find and fight one normal wild encounter to earn experience. Repeat encounters to reach the target level.',
                                          'navigation': getattr(self, 'training_navigation', {}).get(rule.map),
                                          'encounters': ((self.maps[rule.map].get('wild') or {}).get('red') or {}).get('grass'),
                                          'target_level': self.active['target'][2]})
            bindings[key] = operation, rule
        return candidates, bindings

    def geometry_interaction(self, rule):
        # The chosen postcondition may be the unlock flag set just before
        # the block replacement; both belong to the same real interaction.
        return (rule.effect and rule.effect[0] == 'block') or any(
            other.storyline == rule.storyline and other.effect[0] == 'block'
            for other in self.index.rules)

    def interaction_approaches(self, rule, x, y):
        for direction, (dx, dy) in pt.DELTA.items():
            pose = {**getattr(self, 'navigation_facts', {}), 'x': x-dx, 'y': y-dy, 'facing': direction}
            if any('getPlayerFacing' in json.dumps(guard) and evaluate(guard, pose) is not None
                   and bool(evaluate(guard, pose)) != wanted for guard, wanted in rule.guards):
                continue
            yield (x-dx, y-dy), direction

    def destination_points(self, name, rule, *, allow_entry_fallback=True):
        """Ground a destination in its trigger geometry, never an itinerary."""
        points = []
        if rule.map == name:
            coordinates = [p for p in self.index.coordinates(rule) if trigger_position_matches(rule, p)]
            for point in coordinates:
                if point in pt.COORDINATE_WARPS.get(name, {}):
                    # Travel stops beside an automatic trigger. Its explicit
                    # move_to action then performs the fall, not an impossible
                    # stable standing position on the hole itself.
                    points.extend((point[0]+dx, point[1]+dy) for dx, dy in pt.DELTA.values()
                                  if pt.walkable_edge(name, (point[0]+dx, point[1]+dy), point))
                else:
                    points.append(point)
            if rule.id.startswith('boulder:'):
                target = BOULDER_TARGETS[rule.effect[1]]
                sources = boulder_sources(name, tuple(target['target']), str(self.index.maps_dir))
                holes = {tuple(v['target']) for v in BOULDER_TARGETS.values() if v['map'] == name and v['falls']}
                for npc in self.maps[name].get('npcs', []):
                    if npc.get('spriteName') == 'Boulder' and (not sources or npc['textId'] in sources):
                        points.extend(p for dx, dy in pt.DELTA.values()
                                      if (p := (npc['x']+dx, npc['y']+dy)) not in holes)
            for x, y in NATIVE_INTERACTIONS.get(rule.storyline, []):
                points.extend(point for point, _ in self.interaction_approaches(rule, x, y))
            sign_ids = {int(t.split(':')[1]) for t in rule.triggers if t.startswith('sign:')}
            for sign in self.maps[name].get('signs', []):
                if sign['textId'] in sign_ids:
                    points.extend((sign['x']+dx, sign['y']+dy) for dx, dy in pt.DELTA.values())
            ids = {int(t.split(':')[1]) for t in rule.triggers if t.startswith('npc:')}
            for npc in self.maps[name].get('npcs', []):
                if npc['textId'] in ids:
                    points.extend(p for p, _ in counter_approaches(name, npc))
                    points.extend((npc['x']+dx, npc['y']+dy) for dx, dy in pt.DELTA.values())
        if not points and allow_entry_fallback:
            for warp in pt.MAPS[name]['warps']:
                points.extend((warp['x']+dx, warp['y']+dy) for dx, dy in pt.DELTA.values())
        return [p for p in dict.fromkeys(points) if pt.walkable(name, *p)
                and p not in pt.warp_tiles(name)]

    def observed_navigation_barriers(self, facts):
        """Avoid coordinates whose observed script still pushes us back.

        Only failed navigation establishes a barrier. Re-evaluating the
        script guards removes it as soon as the real prerequisite changes.
        """
        blocked = {}
        if not getattr(self, 'index', None):
            return blocked
        for rule in self.index.rules:
            if (rule.map in getattr(self, 'observed_barrier_maps', set())
                    and rule.effect[0] == 'movement' and not rule.missing(facts)
                    and not any(effect[0] == 'battle' for effect in rule.preceding)):
                if rule.choices and any(
                    entry.storyline == rule.storyline and entry.choices
                    and not entry.missing(facts)
                    and not any(e[0] == 'battle' for e in entry.preceding)
                    and ((entry.effect[0] == 'transport' and entry.choices != rule.choices)
                         or (entry.effect[0] == 'flag' and rule.missing({**facts,
                             'flags': {**facts.get('flags', {}), entry.effect[1]: entry.effect[2]}})))
                    for entry in self.index.rules):
                    # A dialogue can offer transport or disable its own guard
                    # (including exit confirmation). Its conditional movement
                    # is not a wall; the driver must complete the real choice.
                    continue
                if facts.get('map') and rule.map != facts['map'] and any(
                    entry.map == rule.map and 'load' in entry.triggers and entry.effect[0] == 'movement'
                    and not entry.missing(facts) and not any(e[0] == 'battle' for e in entry.preceding)
                    for entry in self.index.rules):
                    # Entry scripts can carry an arriving player past a
                    # one-way exit guard. Do not turn that exit guard into
                    # a wall preventing entry; re-observe after arriving.
                    continue
                # A push-back after losing/escaping a battle is a possible
                # outcome, not a terrain wall before the battle is attempted.
                coordinates = self.index.coordinates(rule)
                if coordinates:
                    blocked.setdefault(rule.map, set()).update(coordinates)
        return blocked

    def remembered_route_blocker(self, state, destination, points, barriers, excluded):
        """Identify the first observed NPC obstructing a cross-map route."""
        occupied = {}
        for name, npcs in getattr(self.game, 'stationary_npcs', {}).items():
            for text_id, position in npcs.items():
                position = tuple(position)
                if position in barriers.get(name, set()):
                    occupied[(name, *position)] = int(text_id)
        for npc in self.client.cmd(cmd='get_npcs'):
            if npc.get('visible', True):
                occupied[(state['map_name'], npc['x'], npc['y'])] = npc['text_id']
        actual = {name: set(tiles) for name, tiles in barriers.items()}
        for name, x, y in occupied:
            actual.setdefault(name, set()).add((x, y))
        relaxed = {name: set(tiles) for name, tiles in actual.items()}
        for name, x, y in occupied:
            if (x, y) not in self.game.script_navigation_barriers.get(name, set()):
                relaxed[name].discard((x, y))
        route = self.client.route(state['map_name'], destination)
        targets = [(destination, points)]
        for leg in route.get('legs', []):
            stage = leg['to_map']
            if stage != destination:
                targets.append((stage, self.destination_points(stage, Rule('', stage, '', [], [], [], (), []))))
        for name, goals in targets:
            if not goals:
                continue
            def search(blocked):
                return pt.bfs_cross(state['map_name'], (state['player_x'], state['player_y']), name, goals[0],
                    last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                    excluded_maps=excluded, blocked_maps=blocked, goal_nodes={(name, *p) for p in goals})
            if search(actual):
                continue
            path = search(relaxed)
            for node, _ in (path or [])[1:]:
                if node in occupied:
                    text_id = occupied[node]
                    trainer = any(n['textId'] == text_id and n.get('isTrainer') for n in self.maps[node[0]].get('npcs', []))
                    return {'result': 'blocked', 'destination': destination,
                        'detail': 'A previously observed NPC obstructs the planned route',
                        'blockage_map': node[0], 'blockage_position': list(node[1:]),
                        'blocking_npcs': [text_id], 'blocking_trainers': [text_id] if trainer else []}
        return None

    def transit_grass_barriers(self, origin, name, barriers):
        """Block encounter grass on every map a trip only passes through.

        A collector walking to a catching ground keeps meeting the already
        registered species of the routes in between. The destination's own
        grass is the point of the trip and stays walkable.
        """
        legs = self.client.route(origin, name).get('legs') or []
        if not legs:
            return None  # Unknown topology: the ordinary search still runs.
        overlay = {map_name: set(tiles) for map_name, tiles in barriers.items()}
        stages = {origin, *(leg['to_map'] for leg in legs)}
        for stage in sorted(stages - {name}):
            if stage not in pt.MAPS:
                continue
            grass = pt.grass_tiles(stage)
            if grass:
                overlay.setdefault(stage, set()).update(grass)
        return overlay

    def navigate_trip(self, name, point, transit, **kwargs):
        """Walk to the destination, detouring around transit grass.

        The driver re-plans from every observation, so the detour has to reach
        it as navigation barriers rather than as a one-off planned path. The
        detour is a preference, never a lost trip: when a route a live NPC
        sealed cannot be walked, the same trip is retried plainly.
        """
        error = None
        for avoid_maps in ((transit, None) if transit else (None,)):
            try:
                return self.navigate_point(name, point, **kwargs,
                                           **({'avoid_maps': avoid_maps} if avoid_maps else {}))
            except pt.NavError as failure:
                error = failure
        raise error

    def travel(self, name, rule, points=None, avoid_encounters=False):
        self.navigation_intent = {'destination': name, 'target_script': rule.description()}
        state = self.game.st()
        excluded = self.game.navigation_excluded_maps()
        barriers = {}
        puzzle_holes = {tuple(v['target']) for v in BOULDER_TARGETS.values()
                        if v['map'] == name and v['falls']} if rule.id.startswith('boulder:') else set()
        if getattr(self, 'index', None):
            self.navigation_facts = self.facts()
            self.game.script_navigation_barriers = self.observed_navigation_barriers(self.navigation_facts)
            barriers = self.game.navigation_barriers()
            barriers.setdefault(state['map_name'], set()).update(self.game.live_npcs(state['map_name']))
        if puzzle_holes:
            barriers.setdefault(name, set()).update(puzzle_holes)
        paths = []
        points = points or self.destination_points(name, rule)
        transit = self.transit_grass_barriers(state['map_name'], name, barriers) if avoid_encounters else None
        for point in points:
            path = (pt.bfs_cross(state['map_name'], (state['player_x'], state['player_y']),
                                 name, point, last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                                 excluded_maps=excluded, blocked_maps=transit)
                    if transit is not None else None)
            if path is None:
                # Extra blocked tiles can make a route infeasible; the
                # ordinary search is the fallback, never a lost trip.
                path = pt.bfs_cross(state['map_name'], (state['player_x'], state['player_y']),
                                    name, point, last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                                    excluded_maps=excluded, blocked_maps=barriers)
            if path:
                paths.append((len(path), point))
        if not paths:
            npc_obstruction = self.remembered_route_blocker(state, name, points, barriers, excluded)
            obstruction = cut_requirement(state, name, points, self.game.last_map, barriers, excluded)
            if not obstruction:
                # A second obstacle near the goal can hide an earlier tree
                # from a whole-route search. Probe connected regions before
                # accepting an NPC dependency that may lead back to this goal.
                route = self.client.route(state['map_name'], name)
                for stage in reversed([leg['to_map'] for leg in route.get('legs', [])][:-1]):
                    stage_points = self.destination_points(stage, Rule('', stage, '', [], [], [], (), []))
                    if not stage_points:
                        continue
                    direct = pt.bfs_cross(state['map_name'], (state['player_x'], state['player_y']),
                        stage, stage_points[0], last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                        excluded_maps=excluded, blocked_maps=barriers,
                        goal_nodes={(stage, *p) for p in stage_points})
                    if direct:
                        continue
                    obstruction = cut_requirement(state, stage, stage_points, self.game.last_map, barriers, excluded)
                    if obstruction:
                        obstruction = {**obstruction, 'destination': name, 'frontier': stage}
                        break
            if obstruction:
                self.field_requirements[obstruction['move']] = obstruction
                return {**(npc_obstruction or {}), 'result': 'blocked', 'detail': 'An alternative route needs terrain clearance',
                        'field_obstruction': obstruction, 'destination': name}
            obstruction = surf_requirement(state, name, points, self.game.last_map, barriers, excluded)
            if obstruction:
                self.field_requirements['Surf'] = obstruction
                return {'result': 'blocked', 'detail': 'The target region requires crossing water',
                        'field_obstruction': obstruction, 'destination': name}
            prerequisites = self.discover_route_prerequisites(state, name, points)
            if prerequisites:
                self.route_requirements[name] = {'destination': name, 'goal': self.active['target'],
                    'trigger_points': points, 'prerequisites': prerequisites,
                    'evidence': 'A planning path with doors relaxed and NPC collisions omitted; all proposed prerequisites still require real execution'}
                return {'result': 'blocked', 'detail': 'The route crosses a closed mechanism',
                        'destination': name, 'prerequisites': prerequisites}
            # Reach the furthest connected map on the requested topology,
            # so the next observation identifies the actual local blocker.
            route = self.client.route(state['map_name'], name)
            for stage in reversed([leg['to_map'] for leg in route.get('legs', [])][:-1]):
                stage_points = self.destination_points(stage, Rule('', stage, '', [], [], [], (), []))
                for point in stage_points:
                    path = pt.bfs_cross(state['map_name'], (state['player_x'], state['player_y']),
                                       stage, point, last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                                       excluded_maps=excluded, blocked_maps=barriers)
                    if path:
                        try:
                            self.navigate_point(stage, point, tries=50)
                        except NavigationPause as error:
                            return {'result': 'paused_after_battle', 'detail': str(error), 'destination': name}
                        except pt.NavError as error:
                            return {'result': 'blocked', 'detail': str(error), 'destination': name}
                        return {'result': 'blocked', 'detail': 'Reached connected frontier; destination region still inaccessible',
                                'destination': name, 'stage': stage}
                # A distant goal can need several independent unlocks.
                # Discover the next water crossing even when an unrelated
                # locked room farther ahead makes the full path impossible.
                obstruction = surf_requirement(state, stage, stage_points, self.game.last_map, barriers, excluded)
                if obstruction:
                    self.field_requirements['Surf'] = obstruction
                    return {'result': 'blocked', 'detail': 'Water separates a reachable frontier from the final goal',
                            'field_obstruction': obstruction, 'destination': name, 'stage': stage}
            if npc_obstruction:
                return npc_obstruction
            return {'result': 'blocked', 'detail': 'No tile route to the requested trigger region', 'destination': name}
        _, point = min(paths)
        # The driver re-plans the walk from every observation, so the transit
        # detour has to reach it as navigation barriers too. The map the trip
        # starts on keeps its own preference: the driver already avoids grass
        # on the map it is standing on.
        walk_transit = ({map_name: tiles for map_name, tiles in transit.items()
                         if map_name != state['map_name']} if transit else None)
        try:
            position = self.navigate_trip(name, point, walk_transit,
                               **({'goal_points': points} if len(points) > 1 else {}),
                               **({'avoid_tiles': puzzle_holes} if puzzle_holes else {}))
            return {'result': 'reached', 'destination': name, 'position': position}
        except NavigationPause as error:
            return {'result': 'paused_after_battle', 'detail': str(error), 'destination': name}
        except pt.NavError as error:
            current = self.game.st()
            blocker_details = {}
            # With collisions temporarily omitted from the plan only, locate
            # trainers on the requested path. Actual execution still collides
            # normally and must approach/challenge them through real input.
            path = pt.bfs_cross(current['map_name'], (current['player_x'], current['player_y']),
                                name, point, last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                                excluded_maps=excluded, blocked_maps=barriers)
            if path:
                trainers = {n['textId'] for n in self.maps[current['map_name']].get('npcs', []) if n.get('isTrainer')}
                occupied = {(n['x'], n['y']): n['text_id'] for n in self.client.cmd(cmd='get_npcs')
                            if n.get('visible', True)}
                blockers = []
                for node, _ in path[1:]:
                    if node[0] == current['map_name'] and tuple(node[1:]) in occupied:
                        blockers = [occupied[tuple(node[1:])]]
                        break  # Later trainers are not approachable through this first obstruction.
                if blockers:
                    blocker_details = {'blocking_trainers': [n for n in blockers if n in trainers],
                                       'blocking_npcs': blockers}
            blocked = {**barriers, current['map_name']: self.game.live_npcs(current['map_name'])
                       | barriers.get(current['map_name'], set())}
            obstruction = cut_requirement(current, name, [point], self.game.last_map, blocked, excluded)
            if obstruction:
                self.field_requirements[obstruction['move']] = obstruction
                return {'result': 'blocked', 'detail': 'An alternative route needs terrain clearance',
                        'field_obstruction': obstruction, 'destination': name,
                        'navigation_error': str(error), **blocker_details}
            return {'result': 'blocked', 'detail': str(error), 'destination': name, **blocker_details}

    def discover_route_prerequisites(self, state, destination, points):
        """Find causal terrain/NPC producers across a multi-obstacle route.

        Only planning copies change. This is not an executable route until
        its actual switches, doors, battles and field moves are completed.
        """
        if not points or not isinstance(getattr(self.index, 'rules', None), list):
            return []
        originals, changed = {}, {}
        facts = self.navigation_facts
        observed_barriers = getattr(self.game, 'script_navigation_barriers', {})
        barriers = {name: set(points) for name, points in observed_barriers.items()}
        guarded_tiles = {}
        for rule in self.index.rules:
            if (rule.effect[0] != 'movement' or rule.missing(facts)
                    or any(effect[0] == 'battle' for effect in rule.preceding)):
                continue
            targets = []
            for guard, wanted in rule.guards:
                for alternative in requirements(guard, not wanted, facts):
                    targets.extend(target for target in alternative if self.index.frontier(target, facts))
            if not targets:
                continue
            for point in self.index.coordinates(rule):
                if tuple(point) in observed_barriers.get(rule.map, set()):
                    barriers[rule.map].discard(tuple(point))
                    guarded_tiles.setdefault((rule.map, *point), []).extend(targets)
        try:
            for rule in self.index.rules:
                if rule.effect[0] != 'block':
                    continue
                name, x, y = rule.effect[1].split(',')
                m = pt.MAPS[name]
                offset = int(y)*m['width'] + int(x)
                if offset >= len(m['blocks']):
                    continue
                blocks = pt.BLOCKSETS[m['tileset_id']]
                def openings(block):
                    return sum(blocks[block][i] in m['passable_tiles'] for i in (4, 6, 12, 14))
                if openings(rule.effect[2]) <= openings(m['blocks'][offset]):
                    continue
                if name not in originals:
                    originals[name] = m['blocks']
                    m['blocks'] = list(m['blocks'])
                m['blocks'][offset] = rule.effect[2]
                changed[name, offset] = rule.effect
            with water_planning():
                excluded = self.game.navigation_excluded_maps()
                def search(blocked, excluded_maps=excluded):
                    return pt.bfs_cross(state['map_name'], (state['player_x'], state['player_y']),
                        destination, points[0], last_map=self.game.last_map,
                        allow_ledges=True, allow_spinners=True, blocked_maps=blocked,
                        excluded_maps=excluded_maps,
                        goal_nodes={(destination, *point) for point in points})
                # A push-back tile may merely be a shortcut to some other
                # destination. Prefer paths preserving those known walls;
                # only relax them when no such path exists.
                path = search(observed_barriers)
                if not path and barriers != observed_barriers:
                    path = search(barriers)
                if not path and excluded and guarded_tiles:
                    # Execution correctly excludes a guarded region, but using
                    # that same exclusion to discover its unlock hides the
                    # guard's prerequisites forever. Relax only in this plan,
                    # and accept it only when a known causal guard is crossed
                    # BEFORE the first excluded region. Never expose the
                    # relaxed path as executable navigation.
                    relaxed = search(barriers, ())
                    for node, _ in (relaxed or [])[1:]:
                        if node[0] in excluded:
                            break
                        if node in guarded_tiles:
                            path = relaxed
                            break
        finally:
            for name, blocks in originals.items():
                pt.MAPS[name]['blocks'] = blocks
        if not path:
            return []
        observed_npcs = getattr(self.game, 'stationary_npcs', {})
        toggles = getattr(self.index, 'npc_toggles', {})
        if not isinstance(observed_npcs, dict) or not isinstance(toggles, dict):
            observed_npcs, toggles = {}, {}
        for node, _ in path[1:]:
            name, x, y = node
            if node in guarded_tiles:
                return list(dict.fromkeys(guarded_tiles[node]))
            effect = changed.get((name, (y//2)*pt.MAPS[name]['width'] + x//2))
            if effect and self.index.frontier(effect, facts):
                return [effect]
            # The relaxed plan intentionally omits NPC collisions. A real,
            # previously observed actor on that exact path can therefore be
            # the next prerequisite even after an earlier trainer was beaten.
            # Backchain its actual hide producer, not every pickup on a map.
            # The resulting script still needs normal approach/execution;
            # neither this path nor the predicted removal is a success proof.
            for text_id, position in observed_npcs.get(name, {}).items():
                toggle = toggles.get((name, int(text_id)))
                if tuple(position) != (x, y) or not toggle:
                    continue
                target = ('visibility', toggle[0], False)
                if self.index.frontier(target, facts):
                    return [target]
            boulders = {(n['x'], n['y']) for n in self.maps[name].get('npcs', [])
                        if n.get('spriteName') == 'Boulder'}
            if (x, y) in boulders:
                targets = [('flag', flag, True) for flag, target in BOULDER_TARGETS.items()
                           if target['map'] == name and not facts['flags'].get(flag)]
                if targets:
                    return targets
        return []

    def navigate_point(self, name, point, tries=80, avoid_tiles=(), avoid_maps=None, goal_points=None):
        previous = getattr(self.game, 'script_navigation_barriers', {})
        if avoid_tiles or avoid_maps:
            # `avoid_maps` supplies tiles the walk may not cross on the maps it
            # only passes through; the destination map stays fully walkable.
            self.game.script_navigation_barriers = {**previous,
                name: set(previous.get(name, ())) | set(avoid_tiles),
                **{map_name: set(previous.get(map_name, ())) | set(tiles)
                   for map_name, tiles in (avoid_maps or {}).items()}}
        self.game.navigation_active = True
        try:
            return self.game.nav_to_map(*point, name, tries=tries,
                **({'goal_points': goal_points} if goal_points is not None else {}))
        finally:
            self.game.navigation_active = False
            if avoid_tiles or avoid_maps:
                self.game.script_navigation_barriers = previous

    def retrieve_from_pc(self, box_index, mon_index, deposit_index, sign_index):
        """Drive Bill's PC with observed cursors; all mutations come from input."""
        self.client.interact_with(f'sign:{sign_index}')

        def move_cursor(current, target, count):
            down = (target - current) % count
            up = (current - target) % count
            self.tap('down' if down <= up else 'up')

        source = self.active['target'][1]
        desired_box = box_index
        for _ in range(500):
            self.check_budget()
            state = self.client.state()
            pc = state.get('pc_state')
            in_party = any(self.same_species(mon.get('species'), source)
                           for mon in state.get('party', []))
            if pc is None:
                if in_party and state.get('screen') == 'overworld':
                    return {'result': 'withdrew_pokemon', 'species': source,
                            'box': box_index, 'deposited_party_index': deposit_index}
                self.client.step(4)
                continue
            phase = pc['phase']
            if phase == 'Message':
                self.tap('a')
            elif phase == 'MainMenu':
                if in_party:
                    self.tap('b')
                elif pc['main_cursor'] == 0:
                    self.tap('a')
                else:
                    move_cursor(pc['main_cursor'], 0, len(pc['main_items']))
            elif phase == 'BillsMenu':
                if in_party:
                    self.tap('b')
                    continue
                current_box = state.get('current_box_index', 0)
                desired_box = box_index
                party_full = len(state.get('party', [])) >= 6
                if party_full:
                    counts = state.get('box_counts', [])
                    available = [i for i, count in enumerate(counts) if count < 20]
                    if not available:
                        raise StoryStopped('pc_retrieval_no_deposit_capacity')
                    # A full source box cannot receive the teammate who makes
                    # room for a withdrawal. Deposit elsewhere first, then
                    # return to the source box once the party has five slots.
                    desired_box = (current_box if current_box in available else
                                   box_index if box_index in available else available[0])
                if current_box != desired_box:
                    wanted = 3  # CHANGE BOX
                elif party_full:
                    wanted = 1  # DEPOSIT
                else:
                    wanted = 0  # WITHDRAW
                if pc['bills_cursor'] == wanted:
                    self.tap('a')
                else:
                    move_cursor(pc['bills_cursor'], wanted, 5)
            elif phase == 'ChangeBoxConfirm':
                self.tap('a' if pc['yes_selected'] else 'up')
            elif phase == 'BoxList':
                if pc['box_cursor'] == desired_box:
                    self.tap('a')
                else:
                    move_cursor(pc['box_cursor'], desired_box, 12)
            elif phase == 'MonList':
                depositing = pc['mon_mode'] == 'Deposit'
                if in_party or depositing and len(state.get('party', [])) < 6:
                    self.tap('b')
                    continue
                wanted = deposit_index if depositing else mon_index
                if pc['mon_cursor'] == wanted:
                    self.tap('a')
                else:
                    count = len(state.get('party', [])) if depositing else max(mon_index + 1, 20)
                    move_cursor(pc['mon_cursor'], wanted, count)
            elif phase == 'MonAction':
                if pc['mon_action_cursor'] == 0:
                    self.tap('a')
                else:
                    self.tap('up')
            else:
                raise StoryStopped(f'unsupported_pc_phase:{phase}')
        raise StoryStopped('pc_retrieval_did_not_finish')

    def change_pc_box(self, box_index, sign_index):
        """Select a non-full box through the same observed PC UI."""
        self.client.interact_with(f'sign:{sign_index}')
        for _ in range(300):
            self.check_budget()
            state = self.client.state()
            pc = state.get('pc_state')
            changed = state.get('current_box_index') == box_index
            if pc is None:
                if changed and state.get('screen') == 'overworld':
                    return {'result': 'changed_box', 'box': box_index}
                self.client.step(4)
                continue
            phase = pc['phase']
            if phase == 'Message':
                self.tap('a')
            elif phase == 'MainMenu':
                if changed:
                    self.tap('b')
                elif pc['main_cursor'] == 0:
                    self.tap('a')
                else:
                    self.tap('up')
            elif phase == 'BillsMenu':
                if changed:
                    self.tap('b')
                elif pc['bills_cursor'] == 3:
                    self.tap('a')
                else:
                    self.tap('down')
            elif phase == 'ChangeBoxConfirm':
                self.tap('a' if pc['yes_selected'] else 'up')
            elif phase == 'BoxList':
                if pc['box_cursor'] == box_index:
                    self.tap('a')
                else:
                    self.tap('down')
            else:
                raise StoryStopped(f'unsupported_pc_change_phase:{phase}')
        raise StoryStopped('pc_box_change_did_not_finish')

    def deposit_to_pc(self, party_index, sign_index):
        self.client.interact_with(f'sign:{sign_index}')
        original_count = len(self.client.state().get('party', []))
        for _ in range(300):
            self.check_budget()
            state = self.client.state()
            pc = state.get('pc_state')
            deposited = len(state.get('party', [])) < original_count
            if pc is None:
                if deposited and state.get('screen') == 'overworld':
                    return {'result': 'deposited_pokemon', 'party_index': party_index}
                self.client.step(4)
                continue
            phase = pc['phase']
            if phase == 'Message':
                self.tap('a')
            elif phase == 'MainMenu':
                if deposited:
                    self.tap('b')
                elif pc['main_cursor'] == 0:
                    self.tap('a')
                else:
                    self.tap('up')
            elif phase == 'BillsMenu':
                if deposited:
                    self.tap('b')
                elif pc['bills_cursor'] == 1:
                    self.tap('a')
                else:
                    self.tap('down')
            elif phase == 'MonList':
                if pc['mon_cursor'] == party_index:
                    self.tap('a')
                else:
                    self.tap('down')
            elif phase == 'MonAction':
                self.tap('a' if pc['mon_action_cursor'] == 0 else 'up')
            else:
                raise StoryStopped(f'unsupported_pc_deposit_phase:{phase}')
        raise StoryStopped('pc_deposit_did_not_finish')

    def prepare_transit_lead(self, operation, rule):
        """Ask about this selected journey, then apply only Jev's menu choice."""
        facts = self.facts()
        description = {'operation': operation}
        if operation.startswith(('travel_to:', 'reach_training:')):
            destination = operation.split(':', 1)[1].split(',')[0]
            description['navigation'] = {'via': [leg['to_map'] for leg in
                self.client.route(facts['map'], destination).get('legs', [])]}
        candidates = {'trip': json.dumps(description)}
        bindings = {'trip': (operation, rule)}
        self.add_transit_lead_candidates(candidates, bindings, facts)
        if len(bindings) == 1:
            return operation
        leaders = {'keep': json.dumps({'pokemon': facts['party'][0],
            'effect': 'Keep the current leader and begin the selected journey'})}
        for key, (candidate, _) in bindings.items():
            if candidate.startswith('lead_with:'):
                leaders[candidate] = json.dumps({'pokemon': json.loads(candidates[key])['transit_leader'],
                    'effect': 'Move this party member to the lead through the normal party menu, then begin the selected journey'})
        choice = super().choose('action', {'selected_journey': json.loads(candidates['trip']),
            'party': facts['party'], 'subgoal': self.active['target'], 'stage': 'transit_preparation'},
            leaders,
            'Which party member should lead this selected journey through incidental wild encounters? '
            'The journey itself has already been selected. Compare level, current HP and usable moves with '
            'the possible route encounters to preserve the party and escape safely. A capture status supporter '
            'can remain in the party for the destination battle without leading the trip. Keep the current '
            'leader when already suitable to avoid unnecessary swapping. This is not selection of an attack '
            'or training participant. Field moves may be used by a non-leading party member; the selected '
            'field-move user is unchanged by this choice. Table ranges do not guarantee what will be encountered.',
            allow_abstain=False)
        if choice == 'keep':
            return operation
        # The selected field-move user is an individual party member, not a
        # stable slot number. Reordering must not silently change that choice.
        actor = facts['party'][int(operation.split(':')[1])] if operation.startswith('surf:') else None
        self.execute(choice, rule)
        if actor is not None:
            party = self.facts()['party']
            index = next(i for i, mon in enumerate(party) if mon == actor)
            return f'surf:{index}'
        return operation

    def execute(self, operation, rule):
        if (getattr(self, 'collects_dex', False)
                and operation.startswith(('travel_to:', 'reach_training:', 'surf:'))):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            operation = self.prepare_transit_lead(operation, rule)
        if operation.startswith('buy_coins:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            values = operation.split(':', 1)[1].split(',')
            required, npc_index = (int(value) for value in values[:2])
            counter = ((int(values[2]), int(values[3])), values[4]) if len(values) == 5 else None
            purchases = 0
            while self.client.state().get('coins', 0) < required:
                self.check_budget()
                npc = next((row for row in self.client.cmd(cmd='get_npcs')
                            if row['npc_index'] == npc_index and row.get('visible', True)), None)
                if npc is None:
                    raise StoryStopped('coin_clerk_not_visible')
                before = self.client.state().get('coins', 0)
                if counter:
                    self.client.move_to(*counter[0])
                    self.game.face(counter[1])
                else:
                    self.game.approach_object(npc['x'], npc['y'], rule.map)
                self.tap('a')
                self.settle(self.active['target'], rule)
                after = self.client.state().get('coins', 0)
                if after <= before:
                    raise StoryStopped('coin_purchase_did_not_increase_balance')
                purchases += 1
            result = {'result': 'bought_coins', 'coins': self.client.state().get('coins', 0),
                      'purchases': purchases}
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith('deposit_pc:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            party_index, sign_index = (int(value) for value in operation.split(':', 1)[1].split(','))
            result = self.deposit_to_pc(party_index, sign_index)
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith('change_pc_box:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            box_index, sign_index = (int(value) for value in operation.split(':', 1)[1].split(','))
            result = self.change_pc_box(box_index, sign_index)
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith('retrieve_pc:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            box_index, mon_index, deposit_index, sign_index = (
                int(value) for value in operation.split(':', 1)[1].split(','))
            result = self.retrieve_from_pc(box_index, mon_index, deposit_index, sign_index)
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith('lead_with:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            species = operation.split(':', 1)[1]
            data.lead_with(self.game, species)
            result = {'result': 'party_reordered', 'leader': species}
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith('teach_tm:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            item, move, index, forget = operation.split(':', 1)[1].split(',')
            self.game.learn_machine(item, move, int(index), None if forget == 'None' else forget)
            result = {'result': 'used_machine', 'item': item, 'move': move, 'party_index': int(index)}
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith('use_item:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            item, index = operation.split(':', 1)[1].split(',')
            self.game.use_consumable(item, int(index))
            if self.active['target'][0] == 'register':
                self.settle(self.active['target'], rule)
            result = {'result': 'used_item', 'item': item, 'party_index': int(index)}
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith('push_puzzle:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            index, flag = operation.split(':', 1)[1].split(',')
            target = BOULDER_TARGETS[flag]
            state = self.game.st()
            boulder_ids = {n['textId'] for n in self.maps[rule.map].get('npcs', [])
                           if n.get('spriteName') == 'Boulder'}
            npcs = [n for n in self.client.cmd(cmd='get_npcs') if n.get('visible', True)]
            rocks = {(n['x'], n['y']) for n in npcs if n['text_id'] in boulder_ids}
            fixed = {(n['x'], n['y']) for n in npcs if n['text_id'] not in boulder_ids}
            plan = plan_pushes(rule.map, (state['player_x'], state['player_y']), rocks, fixed, target['target'])
            result = {'result': 'blocked', 'detail': 'No reachable push plan from the observed boulders'}
            if plan:
                self.record('push_plan', flag=flag, map=rule.map, pushes=plan)
                try:
                    data.field_move(self.game, 'Strength', int(index))
                    holes = {tuple(v['target']) for v in BOULDER_TARGETS.values()
                             if v['map'] == rule.map and v['falls']}
                    for push in plan:
                        self.navigate_point(rule.map, push['stance'], avoid_tiles=holes)
                        self.game.face(push['direction'])
                        self.game.d.drive([push['direction']] * 4, frames=36)
                        self.settle(self.active['target'], rule)
                        if self.client.flags().get(flag):
                            result = {'result': 'puzzle_solved', 'flag': flag}
                            break
                        after = self.client.cmd(cmd='get_npcs')
                        fell = any(v['map'] == rule.map and v['falls'] and tuple(v['target']) == tuple(push['landing'])
                                   and self.client.flags().get(event) for event, v in BOULDER_TARGETS.items())
                        if not fell and not any(n.get('visible', True) and (n['x'], n['y']) == tuple(push['landing'])
                                                and n['text_id'] in boulder_ids for n in after):
                            result = {'result': 'blocked', 'detail': 'Observed boulder position differs from the plan',
                                      'push': push}
                            break
                    else:
                        result = {'result': 'blocked', 'detail': 'Push plan finished without the expected event'}
                except NavigationPause as error:
                    result = {'result': 'paused_after_battle', 'detail': str(error)}
                except pt.NavError as error:
                    result = {'result': 'blocked', 'detail': str(error)}
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith('interact_with:npc:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            index = int(operation.rsplit(':', 1)[1])
            npc = next((n for n in self.client.cmd(cmd='get_npcs')
                        if n['npc_index'] == index and n.get('visible', True)), None)
            result = {'result': 'blocked', 'detail': 'NPC is no longer visible'}
            if npc is not None:
                self.game.navigation_active = True
                try:
                    # Use the same observed movement driver as cross-map
                    # travel. The compound debug interaction can stall on
                    # a freshly entered warp tile before approaching an NPC.
                    self.game.approach_object(npc['x'], npc['y'], rule.map)
                    self.tap('a')
                    result = {'result': 'interacted_with_npc', 'npc_index': index}
                except NavigationPause as error:
                    result = {'result': 'paused_after_battle', 'detail': str(error)}
                except pt.NavError as error:
                    result = {'result': 'blocked', 'detail': str(error)}
                finally:
                    self.game.navigation_active = False
                self.settle(self.active['target'], rule)
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith('interact_tile:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            x, y = (int(v) for v in operation.split(':')[1].split(','))
            self.game.navigation_active = True
            try:
                approaches = list(self.interaction_approaches(rule, x, y))
                if len(approaches) < 4:
                    state = self.game.st()
                    paths = [(path, point, direction) for point, direction in approaches
                             if (path := pt.bfs_cross(state['map_name'], (state['player_x'], state['player_y']),
                                 rule.map, point, last_map=self.game.last_map,
                                 blocked_maps={rule.map: self.game.live_npcs(rule.map)}, allow_spinners=True))]
                    if not paths:
                        raise pt.NavError('No reachable stance satisfies the interaction facing guard')
                    _, point, direction = min(paths, key=lambda row: len(row[0]))
                    self.navigate_point(rule.map, point)
                    self.game.face(direction)
                else:
                    self.game.approach_object(x, y, rule.map)
                self.tap('a')
                self.settle(self.active['target'], rule)
                result = {'result': 'interacted_with_tile', 'position': [x, y]}
            except NavigationPause as error:
                result = {'result': 'paused_after_battle', 'detail': str(error)}
            except pt.NavError as error:
                result = {'result': 'blocked', 'detail': str(error)}
            finally:
                self.game.navigation_active = False
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith(('learn:', 'cut:', 'surf:')):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            if operation.startswith('learn:'):
                move, index, forget = operation.split(':', 1)[1].split(',')
                self.game.learn_machine(f'Hm{HM_MOVES.index(move)+1:02d}', move, int(index),
                                        None if forget == 'None' else forget)
                result = {'result': 'used_machine', 'move': move}
            elif operation.startswith('surf:'):
                obstacle = self.active['context']
                parent = getattr(self, 'navigation_memory', {}).get(obstacle.get('destination'))
                prerequisites = surf_current_prerequisites(obstacle, self.client.flags())
                if prerequisites and self.game.st().get('player_transport') != 'Surfing':
                    result = {'result': 'blocked', 'detail': 'Native current blocks Surf until boulders fall',
                              'terrain': obstacle, 'prerequisites': prerequisites}
                    self.record('operation', operation=operation, result=result, script=rule.storyline)
                    return result
                if self.game.st().get('player_transport') != 'Surfing':
                    result = self.travel(obstacle['map'], rule, [tuple(obstacle['stance'])])
                    self.remember_travel_result(obstacle['map'], result)
                    if result['result'] != 'reached':
                        self.record('operation', operation=operation, result=result, script=rule.storyline)
                        return result
                    self.game.face(obstacle['direction'])
                    data.field_move(self.game, 'Surf', int(operation.split(':')[1]))
                if self.game.st().get('player_transport') == 'Surfing':
                    try:
                        # Use the same observed map/warp navigation as the
                        # shore planner. Currents and battles can invalidate
                        # a crossing; return that evidence to strategy.
                        with water_planning():
                            self.navigate_point(obstacle['landing'][0], tuple(obstacle['landing'][1:]), tries=20)
                        self.field_requirements.pop('Surf', None)
                        self.crossed_passages.add(json.dumps([obstacle['map'], obstacle['stance'], obstacle['landing']]))
                        result = {'result': 'crossed_water', 'landing': obstacle['landing']}
                        if parent and parent.get('goal'):
                            self.route_continuation = {
                                'goal': parent['goal'], 'destination': obstacle['destination'],
                                'landing': obstacle['landing'],
                                'evidence': 'Real Surf crossing completed; parent trigger still requires execution'}
                    except NavigationPause as error:
                        result = {'result': 'paused_after_battle', 'detail': str(error)}
                    except pt.NavError as error:
                        current = self.game.st()
                        self.observed_barrier_maps.update([obstacle['map'], current['map_name']])
                        self.field_requirements.pop('Surf', None)
                        result = {'result': 'blocked', 'detail': str(error), 'terrain': obstacle,
                                  'position': [current['map_name'], current['player_x'], current['player_y']]}
                else:
                    result = {'result': 'blocked', 'detail': 'Surf did not start', 'terrain': obstacle}
            else:
                obstacle = self.active['context']
                result = self.travel(obstacle['map'], rule, [tuple(obstacle['stance'])])
                self.remember_travel_result(obstacle['map'], result)
                if result['result'] == 'reached':
                    self.game.face(obstacle['direction'])
                    data.field_move(self.game, 'Cut', int(operation.split(':')[1]))
                    self.game.st()
                    if pt.tile_at(obstacle['map'], *obstacle['tree']) != CUT_TILES[pt.MAPS[obstacle['map']]['tileset_name']]:
                        self.cleared_terrain.add(self.active['target'][1])
                        result = {'result': 'tree_cleared', 'terrain': obstacle}
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith('reach_training:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            name, x, y = operation.split(':', 1)[1].split(',')
            result = self.travel(name, rule, [(int(x), int(y))], avoid_encounters=True)
            self.settle(self.active['target'], rule)
            self.remember_travel_result(name, result)
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith('travel_to:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            result = self.travel(operation.split(':', 1)[1], rule)
            self.settle(self.active['target'], rule)
            self.remember_travel_result(operation.split(':', 1)[1], result)
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if operation.startswith('interact_counter:'):
            if self.actions >= self.max_actions:
                raise StoryStopped('action_budget')
            self.actions += 1
            x, y, direction, index = operation.split(':', 1)[1].split(',')
            result = self.client.move_to(int(x), int(y))
            self.settle(self.active['target'], rule)
            state = self.client.state()
            if state['map_name'] == rule.map and (state['player_x'], state['player_y']) == (int(x), int(y)):
                self.game.face(direction)
                self.tap('a')
                self.settle(self.active['target'], rule)
                result = {'result': 'interacted_across_counter', 'npc_index': int(index)}
            self.record('operation', operation=operation, result=result, script=rule.storyline)
            return result
        if not operation.startswith(('train_encounter:', 'catch_encounter:')):
            result = super().execute(operation, rule)
            self.record_travel(result)
            return result
        if self.actions >= self.max_actions:
            raise StoryStopped('action_budget')
        self.actions += 1
        catching = operation.startswith('catch_encounter:')
        parts = operation.split(':', 1)[1].split(',')
        if catching:
            if len(parts) == 3:  # pre-unified grass operation
                name, x, y = parts
                method, method_args = 'grass', []
            else:
                method, name, x, y, *method_args = parts
            area_key = self.active.get('context', {}).get('catch_area', self.active['target'][1])
            if self.capture_resources_missing(self.facts(), method):
                result = {'result': 'blocked', 'detail': 'Ordinary capture balls exhausted; replan supply',
                          'required_capability': 'capture_balls'}
                self.active = None
                self.record('operation', operation=operation, result=result, script=rule.storyline)
                return result
        else:
            name, x, y = parts
            method, method_args, area_key = 'grass', [], name
            if name.startswith('SafariZone'):
                result = {'result': 'blocked', 'detail': 'Safari encounters provide no knockout experience',
                          'required_capability': 'experience_awarding_battle'}
                self.record('operation', operation=operation, result=result, script=rule.storyline)
                return result
        owned_before = self.client.state().get('pokedex', {}).get('owned')
        start_level = self.client.state()['party'][0]['level']
        if self.client.state()['map_name'] != name:
            # A catching trip is for this map's grass; the routes in between
            # are duplicate encounters, so their grass may be walked around.
            travel = self.travel(name, rule, [(int(x), int(y))], avoid_encounters=catching)
            self.record_travel(travel)
            self.settle(self.active['target'], rule)
            self.remember_travel_result(name, travel)
            if self.client.state()['map_name'] != name:
                self.record('operation', operation=operation, result=travel, script=rule.storyline)
                return travel
        current = self.game.st()  # refresh live map blocks for component search
        if catching and method == 'fishing':
            direction, rod = method_args
            self.game.face(direction)
            for _ in range(80):
                self.check_budget()
                self.game.use_field_item(rod)
                self.settle(self.active['target'], rule)
                after = self.client.state().get('pokedex', {}).get('owned')
                if (self.active and self.active['target'][0] == 'held_species'
                        and self.index.satisfied(self.active['target'], self.client.state())):
                    break
                if after is not None and owned_before is not None and after > owned_before:
                    break
                facts = self.facts()
                if self.capture_resources_missing(facts, method) or self.needs_capture_recovery(facts):
                    self.active = None
                    break
            result = {'result': 'hunted', 'method': method, 'map': name,
                      'owned_before': owned_before,
                      'owned_after': self.client.state().get('pokedex', {}).get('owned')}
            return self.finish_catch_operation(operation, rule, area_key, result)
        if catching and method == 'water':
            direction, wx, wy = method_args
            if current.get('player_transport') != 'Surfing':
                self.game.face(direction)
                surfer = next((i for i, mon in enumerate(current['party']) if 'Surf' in mon['moves']), None)
                if surfer is None:
                    return {'result': 'blocked', 'detail': 'No party member knows Surf'}
                data.field_move(self.game, 'Surf', surfer)
            try:
                with water_planning():
                    self.navigate_point(name, (int(wx), int(wy)), tries=20)
            except NavigationPause:
                result = {'result': 'hunted', 'method': method, 'map': name,
                          'owned_before': owned_before,
                          'owned_after': self.client.state().get('pokedex', {}).get('owned')}
                return self.finish_catch_operation(operation, rule, area_key, result)
            current = self.game.st()
        blocked = {(n['x'], n['y']) for n in self.client.cmd(cmd='get_npcs') if n['visible']}
        if method in ('grass', 'safari'):
            spot = reachable_grass(name, (current['player_x'], current['player_y']), blocked)
            if spot is None:
                return {'result': 'no_reachable_training_grass'}
            moved = self.client.move_to(*spot)
            self.settle(self.active['target'], rule)
            if moved.get('result') not in ('reached', 'interrupted', 'entered_battle'):
                return moved
        for cycle in range(600):
            self.check_budget()
            state = self.client.state()
            if state['screen'] == 'battle':
                self.settle(self.active['target'], rule)
                break
            if state['map_name'] != name:
                break
            facts = self.facts()
            if ((self.capture_resources_missing(facts, method) or self.needs_capture_recovery(facts))
                    if catching else self.needs_skill_recovery(facts)):
                self.active = None
                break
            px, py = state['player_x'], state['player_y']
            if method == 'water':
                with water_planning():
                    steps = [(direction, (px + dx, py + dy))
                             for direction, (dx, dy) in pt.DELTA.items()
                             if water_tile(name, px+dx, py+dy)
                             and pt.walkable_edge(name, (px, py), (px+dx, py+dy))]
            else:
                steps = [(direction, (px + dx, py + dy))
                         for direction, (dx, dy) in pt.DELTA.items()
                         if training_tile(name, px+dx, py+dy)
                         and pt.walkable_edge(name, (px, py), (px+dx, py+dy))]
            if not steps:
                raise StoryStopped(f'no_training_step:{name}:{px},{py}')
            # Alternate legal grass steps; Jev chooses the encounter site,
            # deterministic navigation owns the individual held frames.
            direction, _ = steps[cycle % len(steps)]
            self.game.d.drive([direction] * 8, frames=12)
        result = ({'result': 'hunted', 'map': name, 'owned_before': owned_before,
                   'owned_after': self.client.state().get('pokedex', {}).get('owned')} if catching else
                  {'result': 'trained', 'level_before': start_level,
                   'level_after': self.client.state()['party'][0]['level']})
        if catching:
            return self.finish_catch_operation(operation, rule, area_key, result)
        self.record('operation', operation=operation, result=result, script=rule.storyline)
        return result

    def finish_catch_operation(self, operation, rule, area_key, result):
        """Record a method-specific hunt without losing its Pokédex delta."""
        history = getattr(self, 'catch_attempts', [])
        history.append({'map': area_key,
                        'registered': result['owned_after'] is not None
                        and result['owned_before'] is not None
                        and result['owned_after'] > result['owned_before']})
        del history[:-self.CATCH_WINDOW]
        self.catch_attempts = history
        self.record('operation', operation=operation, result=result, script=rule.storyline)
        return result

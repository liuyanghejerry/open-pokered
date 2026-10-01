"""Autonomous story planning with generic battle, healing and training skills.

No playthrough milestone or route handler is called. Story candidates come from
scene conditions; preparation candidates come from available encounters and the
party. The existing driver contributes only parameterized navigation and combat.
"""
import json
import re
import hashlib
import time
from collections import deque
from functools import lru_cache
from pathlib import Path

import playthrough as pt
import playthrough_late as data

from .story_agent import DualStoryAgent, StoryStopped, attempt_key
from .story_rules import Rule, requirements, evaluate
from .playthrough_judgments import (ObservedProtocol, NavigationPause, attack_profile, replacement_options,
                                    MEDICINES, BALLS, medicine_options, effective_attacks, ITEM_CATALOG,
                                    PREFERENCE_INSTRUCTIONS)
from .navigation_skills import (cut_requirement, surf_requirement, water_planning, water_tile,
                                hm_compatible, machine_compatible, HM_MOVES, TM_MOVES, CUT_TILES)
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


def catch_difficulty(species):
    rate = data.species_data(species).get('catchRate', 0)
    return {'species': species, 'catch_rate': rate,
            'band': next((band for threshold, band in CATCH_BANDS if rate >= threshold), 'hard')}


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
    """Grass or ordinary indoor floor; table availability is checked by the caller."""
    m = pt.MAPS[name]
    # Forest-tileset interiors are encounter terrain everywhere you walk, like
    # caves — excluding them left ViridianForest and the whole Safari Zone with
    # zero huntable tiles, invisible to catch-area discovery.
    return pt.is_grass(name, x, y) or (
        m['id'] >= FIRST_INDOOR_MAP
        and pt.walkable(name, x, y) and pt.tile_at(name, x, y) not in (0x14, 0x15)
        and (x, y) not in pt.warp_tiles(name))


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


class AutonomousStoryAgent(DualStoryAgent):
    def __init__(self, *args, game, preference='none', **kwargs):
        super().__init__(*args, **kwargs)
        self.preference = preference
        self.game = game
        game.judgments = self
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
    def needs_healing(facts):
        if not facts['party']:
            return False
        mon = facts['party'][0]
        attack_pp = sum(pp for move, pp in zip(mon['moves'], mon['pp'])
                        if move != 'None' and data.move_data(move)['power'] > 0)
        depleted_attack = any(pp <= data.move_data(move)['pp'] * .25
                              for move, pp in zip(mon['moves'], mon['pp'])
                              if move != 'None' and data.move_data(move)['power'] > 0)
        return (mon['hp'] < mon['max_hp'] * .7 or mon['status'] != 'None'
                or attack_pp < 6 or depleted_attack)

    def should_replan(self, facts):
        if self.replan_after_defeat:
            return True
        if (self.active and self.active['target'][0] == 'item' and self.active['target'][2]
                and len(facts.get('bag', {})) >= 20
                and not facts['bag'].get(self.active['target'][1].replace('_', '').upper())):
            return True
        party = facts.get('party', [])
        main = max(party, key=lambda mon: mon['level']) if party else None
        main_critical = bool(main and main.get('max_hp', 0) > 0
                             and main['hp'] <= main['max_hp'] * .25)
        return (self.active and self.active['target'][0] != 'heal'
                and (main_critical or self.needs_skill_recovery(facts)))

    def needs_skill_recovery(self, facts):
        """A status-only evolution trainee can share XP with a ready finisher."""
        active = getattr(self, 'active', None) or {}
        context = active.get('context', {})
        party = facts.get('party', [])
        if (party and context.get('acquisition_method') == 'evolution'
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
        self.replan_after_defeat = False

    def augment_strategy_state(self, state, facts):
        if self.collects_dex:
            state['dex_progress'] = self.dex_progress(facts)

    def dex_progress(self, facts):
        """Collection panel: what is missing, where, and what it unlocks."""
        dex = facts.get('dex') or {}
        owned = set(dex.get('owned_species', []))
        seen = set(dex.get('seen_species', []))
        wild_graph = self.collection_graph()
        full_graph = self.complete_collection_graph()
        plan = solo_plan(full_graph, owned, infer_solo_choices(owned))
        targets = set(plan['reachable_species'])
        choice_targets = set(plan['choice_reachable_species'])
        rung = next((value for value in DEX_RUNGS if value > len(owned)), None)
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
        return {'owned': dex.get('owned', len(owned)), 'seen': dex.get('seen', len(seen)), 'total': 151,
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
                # Species already met are known-reachable, so this list is the
                # strongest lead the collector has; the rung says what
                # finishing the current count unlocks next.
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
        if layer == 'strategy' and any('route_resets_won_battles' in value for value in candidates.values()):
            instruction += (' Compare recovery travel with its supplied story-reset cost. '
                'Depleted PP in one move does not require leaving when other usable attacks can handle '
                'the remaining opponents. Prefer preserving completed battles when continuing or using '
                'carried recovery is viable; retreat remains valid when the party cannot proceed.')
            instruction += (' Recovery is preparation, not a requirement to be fully replenished after every battle. '
                'When recovery resets won battles, do not repeat a full-heal loop just to top off HP or PP. '
                'Compare current health and usable effective attacks with the next opponent, and continue when '
                'those resources are sufficient. The urgently_needed field is a heuristic warning, not a '
                'requirement to refill each depleted move. A depleted attack can be replaced by another '
                'effective attack with PP remaining. Retreat only when its benefit outweighs replaying all reset battles.')
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
                'sources are not guaranteed reachable; choose the prerequisites needed to reach them.')
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
        # Appended after the rewrites above: the menu and training instructions
        # replace the incoming text, and the bias must still reach the question.
        bias = PREFERENCE_INSTRUCTIONS.get(getattr(self, 'preference', 'none'))
        if bias and layer in ('strategy', 'action'):
            instruction += f' {bias}'
        return super().choose(layer, state, candidates, instruction,
                              allow_abstain=not (grounded or mechanism_grounded))

    def annotate_navigation(self, groups, facts, previews=None, *, prune=True):
        """A failed destination region does not block every NPC on its map."""
        self.navigation_facts = facts
        self.game.script_navigation_barriers = self.observed_navigation_barriers(facts)
        barriers = self.game.navigation_barriers()
        barriers[facts['map']] = barriers.get(facts['map'], set()) | self.game.live_npcs(facts['map'])
        excluded = self.game.navigation_excluded_maps()
        previews = {} if previews is None else previews
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
                    if path:
                        found = len(path)-1
                    previews[key] = {'map': rule.map, 'tile_route_found': found is not None,
                                     'steps': found, 'requires_surf': requires_surf,
                                     'scope': 'this trigger region, using known geometry and observed obstacles; available Surf can be used en route'}
                if previews[key] not in routes:
                    routes.append(previews[key])
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
                reachable = name in targets and not targets[name]
                if not reachable and name in pt.MAPS and relevant_nodes:
                    goal = next(iter(relevant_nodes))
                    reachable = bool(pt.bfs_cross(
                        name, (x, y), goal[0], goal[1:], last_map=self.game.last_map,
                        allow_ledges=True, allow_spinners=True,
                        blocked_maps=barriers, excluded_maps=excluded, goal_nodes=relevant_nodes))
                arrivals[arrival] = reachable
            if not arrivals[arrival]:
                continue
            if all(k in facts for k in ('map', 'x', 'y')):
                if transport.storyline not in sources:
                    points = self.destination_points(transport.map, transport)
                    def can_approach():
                        return bool(points) and bool(pt.bfs_cross(
                            facts['map'], (facts['x'], facts['y']), transport.map, points[0],
                            last_map=self.game.last_map, allow_ledges=True, allow_spinners=True,
                            blocked_maps=barriers, excluded_maps=excluded,
                            goal_nodes={(transport.map, *p) for p in points}))
                    sources[transport.storyline] = can_approach()
                    if not sources[transport.storyline] and any('Surf' in m['moves'] for m in facts.get('party', [])):
                        with water_planning():
                            sources[transport.storyline] = can_approach()
                if not sources[transport.storyline]:
                    continue  # A shortcut cannot help when its own entrance is behind the same wall.
            for rule in self.index.frontier(transport.effect, facts):
                key = json.dumps(rule.effect)
                if key not in groups:
                    added = True
                    groups[key] = {'target': rule.effect, 'rules': [],
                                   'objectives': ['Enter a region with a walking path to an inaccessible story target'],
                                   'context': {'transport_script': transport.description(),
                                               'blocked_destinations': sorted(targets)}}
                if rule not in groups[key]['rules']:
                    groups[key]['rules'].append(rule)
        return added

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

    def dex_complete(self, facts):
        """Every species reachable under the Red solo/no-link policy is registered.

        Scoping this to already-explored areas made it vacuously true at spawn:
        the starting room has no encounter table, so "nothing unregistered here"
        held and the run reported success in six seconds without acting.
        """
        if getattr(self, 'index', None) is None:
            return False
        owned = set((facts.get('dex') or {}).get('owned_species', []))
        if not owned:
            return False
        plan = solo_plan(self.complete_collection_graph(), owned, infer_solo_choices(owned))
        return set(plan['reachable_species']) <= owned

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
        owned = set((facts.get('dex') or {}).get('owned_species', []))
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
                        self.add_storage_retrieval(groups, facts, source, species)
                        continue
                    if method['trigger'] == 'item':
                        item = method['item']
                        if not facts['bag'].get(item.replace('_', '').upper(), 0):
                            self.add_evolution_item_source(groups, facts, item, species)
                            continue
                    context['party_indices'] = source_party
                    if method['trigger'] == 'level':
                        context['training_cost'] = evolution_training_cost(party[source_party[0]], method['level'])
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
                        self.add_storage_retrieval(groups, facts, source, species)
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
                    if method['method'] == 'static' and not self.balls_held(facts):
                        continue
                    if method['method'] == 'gift' and len(party) >= 6:
                        if self.acquisition_capacity_ready(facts, species, method):
                            self.add_party_space_group(groups, facts, species)
                        continue
                    rules = self.acquisition_story_rules(species, method)
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

    def add_storage_retrieval(self, groups, facts, source, target):
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
                        'required_for': []}})
        objective = f'Withdraw {source} from storage so it can produce {target}'
        if objective not in entry['objectives']:
            entry['objectives'].append(objective)
        if target not in entry['context']['required_for']:
            entry['context']['required_for'].append(target)

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

    def opponent_parties(self, rules):
        parties = []
        seen = set()
        for rule in rules:
            if not any(effect[0] == 'battle' for effect in rule.preceding):
                continue
            ids = {int(t.split(':')[1]) for t in rule.triggers if t.startswith('npc:')}
            for npc in self.maps.get(rule.map, {}).get('npcs', []):
                if ids and npc['textId'] not in ids:
                    continue
                key = (rule.map, npc['textId'])
                if key in seen:
                    continue
                seen.add(key)
                trainer = self.trainers.get(npc.get('trainerClass'))
                if trainer and npc.get('trainerSet'):
                    index = npc['trainerSet'] - 1
                    if index < len(trainer['parties']):
                        parties.extend(trainer['parties'][index]['pokemon'])
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
        """Describe victories lost by entry scripts on a proposed healing route."""
        won = {r.effect[1] for r in self.index.rules
               if r.effect[0] == 'flag' and r.effect[2]
               and facts['flags'].get(r.effect[1]) and any(e[0] == 'battle' for e in r.preceding)}
        resets = [r for r in self.index.rules if r.effect[0] == 'flag' and not r.effect[2]
                  and r.effect[1] in won and 'load' in r.triggers
                  and not any(e[0] == 'battle' for e in r.preceding)]
        if not resets:
            return {}
        costs = {}
        for healer in healers:
            route = self.client.route(facts['map'], healer.map)
            if not route.get('found'):
                continue
            entered = {leg['to_map'] for leg in route.get('legs', [])}
            lost = sorted({r.effect[1] for r in resets if r.map in entered
                           and not r.missing({**facts, 'map': r.map})})
            if lost:
                costs[healer.map] = lost
        return costs

    def find_training_sites(self, facts):
        ranked = []
        level = facts['party'][0]['level']
        active = getattr(self, 'active', None) or {}
        context = active.get('context', {})
        if context.get('acquisition_method') == 'evolution' and context.get('trigger') == 'level':
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

    def find_catch_areas(self, facts, requested_species=None):
        """Executable grass, Safari, Surf, and fishing acquisition areas."""
        owned = set((facts.get('dex') or {}).get('owned_species', []))
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
                ready = [b for b in pending if list(b['goal']) in live_targets or b['goal'][0] == 'level']
                if not ready:
                    return
                for blockage in ready:
                    pending.remove(blockage)
                    yield blockage

        for blockage in relevant_blockages():
            if self.index.satisfied(blockage['goal'], facts):
                continue
            route = self.client.route(facts['map'], blockage['destination'])
            corridor = {facts['map'], blockage['destination'], *[leg['to_map'] for leg in route.get('legs', [])]}
            # The high-level graph joins outdoor regions directly and can
            # omit their gatehouses. Keep observed blockers in those real
            # warp-connected rooms as part of the corridor.
            corridor.update(warp['dest_map_name'] for name in list(corridor)
                for warp in pt.MAPS.get(name, {}).get('warps', []) if warp.get('dest_map_name'))
            if route.get('found') and blockage['map'] not in corridor:
                # A resettable puzzle behind us is not a prerequisite for
                # the remaining route. Keep the memory for a later return.
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
                    for missing in local.alternatives(facts):
                        for prerequisite in missing:
                            frontiers.extend(self.index.frontier(prerequisite, facts))
                elif local.effect[0] == 'movement' and not local.missing(facts):
                    # A currently enabled push-back can be avoided by
                    # changing one of its branch guards (e.g. acquiring a ticket).
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

    def strategy_groups(self, facts):
        groups = super().strategy_groups(facts)
        self.navigation_facts = facts
        if getattr(self, 'navigation_memory', {}):
            self.game.script_navigation_barriers = self.observed_navigation_barriers(facts)
        # A known blocked goal needs a newly grounded prerequisite before
        # asking strategy to select it again. This also reconstructs geometric
        # dependencies after a checkpoint without replaying an old itinerary.
        for group in list(groups.values()):
            for rule in group['rules']:
                if rule.map not in getattr(self, 'navigation_memory', {}):
                    continue
                prerequisites = self.discover_route_prerequisites(
                    self.game.st(), rule.map, self.destination_points(rule.map, rule))
                if prerequisites:
                    self.route_requirements[rule.map] = {'destination': rule.map, 'goal': group['target'],
                        'prerequisites': prerequisites,
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
            if context.get('blocked_goal') and self.index.satisfied(context['blocked_goal'], facts):
                continue
            for rule in self.index.frontier(('item', item, True), facts):
                key = json.dumps(rule.effect)
                group = groups.setdefault(key, {'target': rule.effect, 'rules': [],
                    'objectives': ['Prepare the item required by an observed blocked story battle'],
                    'context': context})
                if rule not in group['rules']:
                    group['rules'].append(rule)
        self.add_navigation_groups(groups, facts)
        pending_boulders = [r for group in groups.values() for r in group['rules']
                           if r.id.startswith('boulder:')]
        if pending_boulders and not any('Strength' in m['moves'] for m in facts['party']):
            self.field_requirements['Strength'] = {'move': 'Strength',
                'map': pending_boulders[0].map, 'puzzle_flags': [r.effect[1] for r in pending_boulders]}
            for key, group in list(groups.items()):
                group['rules'] = [r for r in group['rules'] if not r.id.startswith('boulder:')]
                if not group['rules']:
                    del groups[key]
        for move, obstacle in self.field_requirements.items():
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
            opponents = self.opponent_parties(group['rules'])
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
            reset_costs = self.healing_route_costs(self.healing_rules, facts)
            if reset_costs:
                groups['prepare:heal']['context']['route_resets_won_battles'] = reset_costs
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
            owned = set((facts.get('dex') or {}).get('owned_species', []))
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
        if self.maximizes_coverage:
            self.add_coverage_groups(groups, facts)
        # Keep the blocked goals until their entrances have been considered.
        readiness = battle_readiness(facts['party'], facts['bag'])
        failed_maps = {d['map'] for d in self.battle_defeats if not d.get('resolved_by_victory')
                       and d.get('preparation_signature') == readiness and d['map'] != facts['map']}
        for key, group in list(groups.items()):
            if group['target'][0] in ('flag', 'block', 'transport') and all(r.map in failed_maps for r in group['rules']):
                del groups[key]  # Repeating the same failed preparation is not a new plan.
        previews = self.annotate_navigation(groups, facts, previews, prune=False)
        self.transport_frontiers(groups, facts)
        self.add_mechanism_groups(groups, facts)
        self.annotate_navigation(groups, facts, previews)
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
        return groups

    @staticmethod
    def prioritize_critical_recovery(groups, facts):
        """Recover a critically hurt main battler when a nurse is executable.

        Script preconditions do not mean a party can survive the next battle.
        Keep recovery mandatory at <=25% HP, but only if an actual tile path
        exists; otherwise retain the frontiers that can unlock that path.
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
                         if route.get('tile_route_found')}
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

    def add_ball_supply(self, groups, facts):
        """Buy balls through the same real shop path the recovery restock uses.

        That restock is defeat-triggered: a collector at full health would never
        be offered it and would simply stop catching once the bag ran dry.
        """
        if not self.collects_dex:
            return
        normalized = {name.replace('_', '').upper(): name for name in BALLS}
        carried = {normalized[key]: qty for key, qty in facts['bag'].items() if key in normalized}
        held = sum(carried.values())
        if held >= self.BALL_RESERVE:
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
            qty = min(self.BALL_RESERVE - held, int(facts['money'] * .6) // max(1, info['price']))
            if qty < 1 or (len(facts['bag']) >= 20 and name not in carried):
                continue
            for hops, map_name, _rule_id, stock_index, rule in shops:
                target = ('supply', name, carried.get(name, 0) + qty)
                groups[f'ball:{rule.id}:{name}'] = {'target': target, 'rules': [rule],
                    'objectives': ['Buy balls to keep collecting unregistered species'],
                    'context': {'optional_preparation': True, 'stock_index': stock_index,
                                'item': info, 'collecting': True,
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
        if (self.active['target'][0] == 'register'
                and self.active.get('context', {}).get('acquisition_method') == 'evolution'):
            context = self.active['context']
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
            if index:
                operation = f'lead_with:{source}'
                candidates['action:0'] = json.dumps({
                    'operation': operation,
                    'purpose': f'Move {source} to the lead slot so battle experience can evolve it',
                    'evolves_into': context['species']})
                bindings['action:0'] = operation, rule
                return candidates, bindings
            for name in self.find_training_sites(facts):
                x, y = self.training_sites[name]
                operation = f'train_encounter:{name},{x},{y}'
                key = f'action:{len(candidates)}'
                candidates[key] = json.dumps({
                    'operation': operation,
                    'purpose': f'Gain a level with {source} to evolve it into {context["species"]}',
                    'required_level': context['level'],
                    'current_level': facts['party'][0]['level'],
                    'training_cost': evolution_training_cost(facts['party'][0], context['level']),
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
                                    blocked=blocked | pt.warp_tiles(facts['map']), allow_spinners=True):
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

    def destination_points(self, name, rule):
        """Ground a destination in its trigger geometry, never an itinerary."""
        points = []
        if rule.map == name:
            coordinates = [p for p in self.index.coordinates(rule) if trigger_position_matches(rule, p)]
            points.extend(coordinates)
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
        if not points:
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
                self.navigate_point(name, point, **kwargs,
                                    **({'avoid_maps': avoid_maps} if avoid_maps else {}))
                return
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
                    'prerequisites': prerequisites,
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
            self.navigate_trip(name, point, walk_transit,
                               **({'avoid_tiles': puzzle_holes} if puzzle_holes else {}))
            return {'result': 'reached', 'destination': name, 'position': point}
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
        """Find causal terrain producers across a multi-obstacle route.

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
                def search(blocked):
                    return pt.bfs_cross(state['map_name'], (state['player_x'], state['player_y']),
                        destination, points[0], last_map=self.game.last_map,
                        allow_ledges=True, allow_spinners=True, blocked_maps=blocked,
                        excluded_maps=self.game.navigation_excluded_maps(),
                        goal_nodes={(destination, *point) for point in points})
                # A push-back tile may merely be a shortcut to some other
                # destination. Prefer paths preserving those known walls;
                # only relax them when no such path exists.
                path = search(observed_barriers)
                if not path and barriers != observed_barriers:
                    path = search(barriers)
        finally:
            for name, blocks in originals.items():
                pt.MAPS[name]['blocks'] = blocks
        if not path:
            return []
        for node, _ in path[1:]:
            name, x, y = node
            if node in guarded_tiles:
                return list(dict.fromkeys(guarded_tiles[node]))
            effect = changed.get((name, (y//2)*pt.MAPS[name]['width'] + x//2))
            if effect and self.index.frontier(effect, facts):
                return [effect]
            boulders = {(n['x'], n['y']) for n in self.maps[name].get('npcs', [])
                        if n.get('spriteName') == 'Boulder'}
            if (x, y) in boulders:
                targets = [('flag', flag, True) for flag, target in BOULDER_TARGETS.items()
                           if target['map'] == name and not facts['flags'].get(flag)]
                if targets:
                    return targets
        return []

    def navigate_point(self, name, point, tries=80, avoid_tiles=(), avoid_maps=None):
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
            self.game.nav_to_map(*point, name, tries=tries)
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
                if state.get('current_box_index', 0) != box_index:
                    wanted = 3  # CHANGE BOX
                elif len(state.get('party', [])) >= 6:
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
                if pc['box_cursor'] == box_index:
                    self.tap('a')
                else:
                    move_cursor(pc['box_cursor'], box_index, 12)
            elif phase == 'MonList':
                depositing = pc['mon_mode'] == 'Deposit'
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

    def execute(self, operation, rule):
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
        else:
            name, x, y = parts
            method, method_args, area_key = 'grass', [], name
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
                if self.needs_healing(self.facts()):
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
            if (self.needs_healing(self.facts()) if catching else self.needs_skill_recovery(self.facts())):
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

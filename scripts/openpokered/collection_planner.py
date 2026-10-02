"""Deterministic acquisition facts for Pokédex-oriented play.

The game engine owns encounter and capture randomness.  This module only
turns the checked-in Red encounter tables into typed, comparable acquisition
methods so Jev can choose between travel/resource trade-offs without having
to infer mechanics from prose.
"""
import itertools
import json
from collections import defaultdict
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SPECIES_DIR = ROOT / 'crates/pokered-data/pokemon'
STORY_GRAPH_PATH = ROOT / 'crates/pokered-data/story/graph.json'


ENCOUNTER_SLOT_WEIGHTS = (51, 51, 39, 25, 25, 25, 13, 13, 11, 3)

# pokered-data/src/wild_data.rs::super_rod_groups.  A successful Super Rod
# bite picks uniformly from its group's entries; every cast first has a 50%
# no-bite roll.
SUPER_ROD_GROUPS = (
    ((15, 'Tentacool'), (15, 'Poliwag')),
    ((15, 'Goldeen'), (15, 'Poliwag')),
    ((15, 'Psyduck'), (15, 'Goldeen'), (15, 'Krabby')),
    ((15, 'Krabby'), (15, 'Shellder')),
    ((23, 'Poliwhirl'), (15, 'Slowpoke')),
    ((15, 'Dratini'), (15, 'Krabby'), (15, 'Psyduck'), (15, 'Slowpoke')),
    ((5, 'Tentacool'), (15, 'Krabby'), (15, 'Goldeen'), (15, 'Magikarp')),
    ((15, 'Staryu'), (15, 'Horsea'), (15, 'Shellder'), (15, 'Goldeen')),
    ((23, 'Slowbro'), (23, 'Seaking'), (23, 'Kingler'), (23, 'Seadra')),
    ((23, 'Seaking'), (15, 'Krabby'), (15, 'Goldeen'), (15, 'Magikarp')),
)

SUPER_ROD_MAP_GROUP = {
    'PalletTown': 0, 'ViridianCity': 0,
    'Route22': 1,
    'CeruleanCity': 2, 'Route4': 2, 'Route24': 2, 'Route25': 2, 'CeruleanGym': 2,
    'VermilionCity': 3, 'Route6': 3, 'Route11': 3, 'VermilionDock': 3,
    'CeladonCity': 4, 'Route10': 4,
    'SafariZoneEast': 5, 'SafariZoneNorth': 5, 'SafariZoneWest': 5,
    'SafariZoneCenter': 5,
    'Route12': 6, 'Route13': 6, 'Route17': 6, 'Route18': 6,
    'CinnabarIsland': 7, 'Route19': 7, 'Route20': 7, 'Route21': 7,
    'SeafoamIslandsB3F': 7, 'SeafoamIslandsB4F': 7,
    'Route23': 8, 'CeruleanCave2F': 8, 'CeruleanCaveB1F': 8,
    'CeruleanCave1F': 8,
    'FuchsiaCity': 9,
}

GOOD_ROD_MONS = ((10, 'Goldeen'), (10, 'Poliwag'))
OLD_ROD_MONS = ((5, 'Magikarp'),)


def table_profile(table, owned_species=()):
    """Exact Gen-I ten-slot encounter value for grass or surfing."""
    mons = (table or {}).get('mons', [])
    owned = set(owned_species)
    weights, levels = defaultdict(int), defaultdict(list)
    for index, mon in enumerate(mons[:len(ENCOUNTER_SLOT_WEIGHTS)]):
        weights[mon['species']] += ENCOUNTER_SLOT_WEIGHTS[index]
        levels[mon['species']].append(mon['level'])
    missing = sorted(species for species in weights if species not in owned)
    novel_weight = sum(weights[species] for species in missing)
    encounter_rate = int((table or {}).get('encounterRate', 0))
    per_step = encounter_rate / 256 * novel_weight / 256
    targets = []
    for species in missing:
        share = weights[species] / 256
        species_per_step = encounter_rate / 256 * share
        targets.append({
            'species': species,
            'levels': [min(levels[species]), max(levels[species])],
            'encounter_share_pct': round(share * 100, 1),
            'per_attempt_pct': round(species_per_step * 100, 2),
            'expected_attempts': round(1 / species_per_step, 1) if species_per_step else None,
        })
    return {
        'scope': 'Encounter-only probabilities per eligible encounter check; seeing an unregistered species is not a successful capture.',
        'encounter_rate_per_256_steps': encounter_rate,
        'unregistered_species_count': len(missing),
        'unregistered_encounter_share_pct': round(novel_weight / 256 * 100, 1),
        'duplicate_encounter_share_pct': round((256 - novel_weight) / 256 * 100, 1),
        'new_species_per_attempt_pct': round(per_step * 100, 2),
        'expected_attempts_to_any_new_species': round(1 / per_step, 1) if per_step else None,
        'targets': targets,
    }


def fishing_profile(rod, map_name, owned_species=()):
    """Per-cast yield for a rod on a map, including the no-bite roll."""
    if rod == 'OldRod':
        entries, bite_probability = OLD_ROD_MONS, 1.0
    elif rod == 'GoodRod':
        entries, bite_probability = GOOD_ROD_MONS, .5
    elif rod == 'SuperRod' and map_name in SUPER_ROD_MAP_GROUP:
        entries = SUPER_ROD_GROUPS[SUPER_ROD_MAP_GROUP[map_name]]
        bite_probability = .5
    else:
        return None
    owned = set(owned_species)
    by_species = defaultdict(list)
    for level, species in entries:
        by_species[species].append(level)
    per_entry = bite_probability / len(entries)
    missing = sorted(species for species in by_species if species not in owned)
    novel = per_entry * sum(len(by_species[species]) for species in missing)
    return {
        'rod': rod,
        'bite_probability_pct': round(bite_probability * 100, 1),
        'unregistered_species_count': len(missing),
        'unregistered_encounter_share_pct': round(novel * 100, 1),
        'duplicate_encounter_share_pct': round((1 - novel) * 100, 1),
        'new_species_per_attempt_pct': round(novel * 100, 1),
        'expected_attempts_to_any_new_species': round(1 / novel, 1) if novel else None,
        'targets': [{
            'species': species,
            'levels': [min(by_species[species]), max(by_species[species])],
            'per_attempt_pct': round(per_entry * len(by_species[species]) * 100, 1),
            'expected_attempts': round(1 / (per_entry * len(by_species[species])), 1),
        } for species in missing],
    }


def acquisition_graph(maps, fishable_maps=()):
    """Species -> all deterministic wild acquisition methods in Pokémon Red."""
    graph = defaultdict(list)
    fishable = set(fishable_maps)
    for map_name, map_data in maps.items():
        red = ((map_data.get('wild') or {}).get('red') or {})
        for method in ('grass', 'water'):
            for mon in (red.get(method) or {}).get('mons', []):
                record = {'method': 'safari' if method == 'grass' and map_name.startswith('SafariZone') else method,
                          'map': map_name, 'level': mon['level']}
                if record not in graph[mon['species']]:
                    graph[mon['species']].append(record)
        if map_name not in fishable:
            continue
        for rod in ('OldRod', 'GoodRod', 'SuperRod'):
            profile = fishing_profile(rod, map_name)
            if not profile:
                continue
            entries = (OLD_ROD_MONS if rod == 'OldRod' else GOOD_ROD_MONS if rod == 'GoodRod'
                       else SUPER_ROD_GROUPS[SUPER_ROD_MAP_GROUP[map_name]])
            for level, species in entries:
                record = {'method': 'fishing', 'map': map_name, 'rod': rod, 'level': level}
                if record not in graph[species]:
                    graph[species].append(record)
    return dict(graph)


# Choices that permanently consume the only source available in one Red save.
# The selected Pokémon remains registered after evolving or being traded away,
# so only genuinely branching one-copy sources belong here.
SOLO_CHOICE_GROUPS = {
    'starter': ('Bulbasaur', 'Charmander', 'Squirtle'),
    'fossil': ('Kabuto', 'Omanyte'),
    'dojo': ('Hitmonchan', 'Hitmonlee'),
    'eevee_evolution': ('Flareon', 'Jolteon', 'Vaporeon'),
}

SOLO_CHOICE_BRANCHES = {
    'starter': {
        'Bulbasaur': {'Bulbasaur', 'Ivysaur', 'Venusaur'},
        'Charmander': {'Charmander', 'Charmeleon', 'Charizard'},
        'Squirtle': {'Squirtle', 'Wartortle', 'Blastoise'},
    },
    'fossil': {
        'Kabuto': {'Kabuto', 'Kabutops'},
        'Omanyte': {'Omanyte', 'Omastar'},
    },
    'dojo': {'Hitmonchan': {'Hitmonchan'}, 'Hitmonlee': {'Hitmonlee'}},
    'eevee_evolution': {
        'Flareon': {'Flareon'}, 'Jolteon': {'Jolteon'}, 'Vaporeon': {'Vaporeon'},
    },
}

RED_GAME_CORNER_PRIZES = {
    ('Abra', 9, 180), ('Clefairy', 8, 500), ('Nidorina', 17, 1200),
    ('Dratini', 18, 2800), ('Scyther', 25, 5500), ('Porygon', 26, 9999),
}

NPC_TRADES = (
    ('Nidorino', 'Nidorina', 'Route11Gate2F', 'EVENT_TRADED_FOR_TERRY'),
    ('Abra', 'MrMime', 'Route2TradeHouse', 'EVENT_TRADED_FOR_MARCEL'),
    ('Ponyta', 'Seel', 'CinnabarLabFossilRoom', 'EVENT_TRADED_FOR_SAILOR'),
    ('Spearow', 'Farfetchd', 'VermilionTradeHouse', 'EVENT_TRADED_FOR_DUX'),
    ('Slowbro', 'Lickitung', 'Route18Gate2F', 'EVENT_GOT_LICKITUNG_FROM_TRADE'),
    ('Poliwhirl', 'Jynx', 'CeruleanTradeHouse', 'EVENT_TRADED_FOR_LOLA'),
    ('Raichu', 'Electrode', 'CinnabarLabTradeRoom', 'EVENT_TRADED_FOR_DORIS'),
    ('Venonat', 'Tangela', 'CinnabarLabTradeRoom', 'EVENT_TRADED_FOR_CRINKLES'),
    ('NidoranM', 'NidoranF', 'UndergroundPathRoute5', 'EVENT_TRADED_FOR_SPOT'),
)


def _species_catalog(species_dir=SPECIES_DIR):
    records = {}
    for path in Path(species_dir).glob('*.json'):
        data = json.loads(path.read_text())
        records[data['species']] = data
    return records


def _story_sources(story_graph_path=STORY_GRAPH_PATH):
    """Catchable/gift producers from the generated story semantics graph."""
    sources = []
    for edge in json.loads(Path(story_graph_path).read_text())['edges']:
        if edge['kind'] not in ('gives', 'starts_battle') or not edge['to'].startswith('pokemon:'):
            continue
        script = edge['from'].split(':', 1)[1]
        map_name, storyline = script.split(':', 1)
        species = edge['to'].split(':', 1)[1]
        level = int(edge.get('detail', 'lv0').removeprefix('lv'))
        # The tower ghost cannot be caught. It exists to clear a story gate,
        # not as a Pokédex acquisition method.
        if species == 'MAROWAK' and storyline == 'coordGhostMarowak':
            continue
        sources.append({'species_key': species, 'method': (
            'static' if edge['kind'] == 'starts_battle' else
            'prize' if map_name == 'GameCornerPrizeRoom' else 'gift'),
            'map': map_name, 'storyline': storyline, 'level': level})
    return sources


def complete_acquisition_graph(maps, fishable_maps=(), species_dir=SPECIES_DIR,
                               story_graph_path=STORY_GRAPH_PATH):
    """All Gen-I species and every Red acquisition edge relevant to a solo save.

    Records with ``external_trade`` remain in the graph so an unreachable
    species has an explicit explanation; the solo closure deliberately rejects
    those edges. Mutually exclusive one-copy choices carry an
    ``exclusive_group`` and ``choice`` instead of being silently counted all at
    once.
    """
    catalog = _species_catalog(species_dir)
    names = {name.replace('_', '').upper(): name for name in catalog}
    graph = defaultdict(list, {species: list(rows) for species, rows in
                               acquisition_graph(maps, fishable_maps).items()})

    def add(species, record):
        if record not in graph[species]:
            graph[species].append(record)

    for source in _story_sources(story_graph_path):
        species = names.get(source.pop('species_key').replace('_', '').upper())
        if not species:
            continue
        if source['method'] == 'prize':
            # The semantic graph conservatively contains both Red and Blue
            # branches. Pair the Red level with its exact coin price here.
            match = next((price for sp, level, price in RED_GAME_CORNER_PRIZES
                          if sp == species and level == source['level']), None)
            if match is None:
                continue
            source['coins'] = match
        if species in SOLO_CHOICE_GROUPS['starter']:
            source.update(exclusive_group='starter', choice=species)
        elif species in SOLO_CHOICE_GROUPS['fossil']:
            source.update(exclusive_group='fossil', choice=species,
                          item='DomeFossil' if species == 'Kabuto' else 'HelixFossil')
        elif species in SOLO_CHOICE_GROUPS['dojo']:
            source.update(exclusive_group='dojo', choice=species)
        elif species == 'Aerodactyl':
            source['item'] = 'OldAmber'
        add(species, source)

    for give, receive, map_name, flag in NPC_TRADES:
        add(receive, {'method': 'npc_trade', 'map': map_name,
                      'from_species': give, 'completion_flag': flag})

    for species, data in catalog.items():
        for evolution in data.get('evolutions', []):
            target = evolution['species']
            record = {'method': 'evolution', 'from_species': species,
                      'trigger': evolution['method']}
            if evolution['method'] == 'level':
                record['level'] = evolution['level']
            elif evolution['method'] == 'item':
                record['item'] = evolution['item']
                if species == 'Eevee':
                    record.update(exclusive_group='eevee_evolution', choice=target)
            elif evolution['method'] == 'trade':
                record['external_trade'] = True
            add(target, record)

    # Blue-only wild sources explain Red's version exclusions. They are not
    # executable without another game, but they keep the graph complete and
    # make the ceiling audit mechanically inspectable.
    red_direct = set(acquisition_graph(maps, fishable_maps))
    for map_name, map_data in maps.items():
        blue = ((map_data.get('wild') or {}).get('blue') or {})
        for table in ('grass', 'water'):
            for mon in (blue.get(table) or {}).get('mons', []):
                species = mon['species']
                if species not in red_direct:
                    add(species, {'method': 'version_trade', 'source_version': 'blue',
                                  'map': map_name, 'level': mon['level'],
                                  'external_trade': True})

    for species in catalog:
        graph.setdefault(species, [])
    if not graph['Mew']:
        graph['Mew'].append({'method': 'unavailable',
                             'reason': 'No legitimate in-game Red acquisition source'})
    return dict(graph)


def acquisition_contract(species, method):
    """Normalize one graph edge into code-owned conditions and direct costs.

    Travel distance, encounter yield, current inventory and battle safety are
    live-state costs supplied by the autonomous agent.  This contract captures
    the invariant part of the mechanic so Jev compares feasible alternatives
    instead of reconstructing Pokémon rules from prose.
    """
    requirements = []
    costs = {'consumed_items': {}, 'relinquished_species': [],
             'coins': 0, 'minimum_level': None, 'random_attempts': False,
             'irreversible_choice': None, 'external_system': None}

    def require(kind, value):
        requirements.append({'kind': kind, 'value': value})

    name = method['method']
    if method.get('map'):
        require('visit_map', method['map'])
    if name in ('grass', 'water', 'safari', 'fishing', 'static'):
        require('capture_inventory', 'SafariBall' if name == 'safari' else 'any_ball')
        costs['random_attempts'] = True
    if name == 'static':
        costs['finite_encounter_opportunity'] = True
    if name == 'fishing':
        require('rod', method['rod'])
    source = method.get('from_species')
    if source:
        require('party_species', source)
    if name in ('grass', 'water', 'safari', 'fishing', 'static', 'gift', 'prize'):
        require('party_or_current_box_space', True)
    if name == 'npc_trade':
        # Gen I refuses to trade away the only party member.
        require('party_members_at_least', 2)
    if name == 'evolution':
        trigger = method.get('trigger')
        require('evolution_trigger', trigger)
        if trigger == 'level':
            costs['minimum_level'] = method['level']
        elif trigger == 'item':
            require('item', method['item'])
            costs['consumed_items'][method['item']] = 1
        elif trigger == 'trade':
            costs['external_system'] = 'link_trade'
    elif name == 'npc_trade':
        costs['relinquished_species'].append(method['from_species'])
    elif name == 'prize':
        require('coin_case', True)
        costs['coins'] = method['coins']
    elif name == 'version_trade':
        require('source_version', method['source_version'])
        costs['external_system'] = 'other_game_version_and_link_trade'
    elif name == 'unavailable':
        costs['external_system'] = 'no_legitimate_source'

    if method.get('item') and name == 'gift':
        require('item', method['item'])
        costs['consumed_items'][method['item']] = 1
    if method.get('exclusive_group'):
        choice = {'group': method['exclusive_group'], 'choice': method['choice']}
        require('exclusive_choice', choice)
        costs['irreversible_choice'] = choice
    return {'species': species, 'method': name, 'requirements': requirements,
            'direct_cost': costs}


def _choice_assignments(forced=None):
    forced = dict(forced or {})
    groups = sorted(SOLO_CHOICE_GROUPS)
    choices = [((forced[group],) if group in forced else SOLO_CHOICE_GROUPS[group])
               for group in groups]
    for values in itertools.product(*choices):
        yield dict(zip(groups, values))


def reachable_species(graph, choices, owned=(), allow_external_trade=False):
    """Fixed-point closure for one concrete set of irreversible choices."""
    reachable = set(owned)
    changed = True
    while changed:
        changed = False
        for species, methods in graph.items():
            if species in reachable:
                continue
            for method in methods:
                if method.get('external_trade') and not allow_external_trade:
                    continue
                if method['method'] == 'unavailable':
                    continue
                group = method.get('exclusive_group')
                if group and choices.get(group) != method.get('choice'):
                    continue
                source = method.get('from_species')
                if source and source not in reachable:
                    continue
                reachable.add(species)
                changed = True
                break
    return reachable


def solo_plan(graph, owned=(), forced_choices=None):
    """Maximum Red single-save/no-link-trade closure and its choice policy.

    ``choices`` is one stable representative assignment, useful for computing a
    concrete ceiling and explaining which species that particular save excludes.
    ``optimal_choices`` preserves every branch that occurs in any maximum-size
    assignment.  Callers should offer those bounded alternatives to Jev until an
    observed registration makes the irreversible choice concrete.
    """
    best_size = -1
    optimal = []
    for choices in _choice_assignments(forced_choices):
        reachable = reachable_species(graph, choices, owned)
        size = len(reachable)
        if size > best_size:
            best_size = size
            optimal = [(choices, reachable)]
        elif size == best_size:
            optimal.append((choices, reachable))
    # _choice_assignments has a declared, stable order.  Do not let species-name
    # lexicography silently become gameplay policy when several branches tie.
    choices, reachable = optimal[0]
    choice_reachable = set().union(*(species for _, species in optimal))
    optimal_choices = {
        group: [choice for choice in SOLO_CHOICE_GROUPS[group]
                if any(assignment[group] == choice for assignment, _ in optimal)]
        for group in sorted(SOLO_CHOICE_GROUPS)
    }
    return {'ceiling': len(reachable), 'reachable_species': sorted(reachable),
            'unreachable_species': sorted(set(graph) - reachable), 'choices': choices,
            'choice_reachable_species': sorted(choice_reachable),
            'always_unreachable_species': sorted(set(graph) - choice_reachable),
            'optimal_choices': optimal_choices,
            'optimal_assignment_count': len(optimal)}


def infer_solo_choices(owned_species=()):
    """Irreversible choices already proven by registered descendants."""
    owned = set(owned_species)
    forced = {}
    for group, branches in SOLO_CHOICE_BRANCHES.items():
        matches = [choice for choice, species in branches.items() if species & owned]
        if len(matches) == 1:
            forced[group] = matches[0]
    return forced

"""Deterministic acquisition facts for Pokédex-oriented play.

The game engine owns encounter and capture randomness.  This module only
turns the checked-in Red encounter tables into typed, comparable acquisition
methods so Jev can choose between travel/resource trade-offs without having
to infer mechanics from prose.
"""
from collections import defaultdict


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

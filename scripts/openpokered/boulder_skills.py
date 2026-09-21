"""Engine-derived push objectives and a map-local, real-input puzzle plan."""
import re
import json
from collections import deque
from functools import lru_cache
from pathlib import Path

import playthrough as pt


def boulder_targets():
    source = (pt.ROOT / 'crates/pokered-core/src/overworld/field_moves.rs').read_text()
    result = {}
    for table, falls in [('switches', False), ('holes', True)]:
        body = source.split(f'let {table}:', 1)[1].split('_ => &[]', 1)[0]
        for name, entries in re.findall(r'MapId::(\w+)\s*=>\s*\{?\s*&\[(.*?)\]', body, re.S):
            for x, y, flag in re.findall(r'\((\d+),\s*(\d+),\s*"(\w+)"', entries):
                result[flag] = {'map': name, 'target': (int(x), int(y)), 'falls': falls}
    # An engine-owned inter-floor drop is expressed as a conditional rather
    # than a table. Read its position and event; do not encode a push route.
    for name, x, y, flag in re.findall(
            r'if self\.state\.current_map == MapId::(\w+) && \(npc\.x, npc\.y\) == \((\d+), (\d+)\)\s*\{\s*'
            r'npc\.visible = false;\s*self\.unified_flags\.set\(pokered_data::event_flags::EventFlag::(\w+)\)', source):
        result[flag] = {'map': name, 'target': (int(x), int(y)), 'falls': True}
    return result


BOULDER_TARGETS = boulder_targets()


@lru_cache(maxsize=None)
def boulder_sources(name, target, maps_dir):
    """Find which configured stones can geometrically reach this objective."""
    path = Path(maps_dir) / name / 'map.json'
    if not path.exists():
        return ()
    sources = []
    for npc in json.loads(path.read_text()).get('npcs', []):
        if npc.get('spriteName') != 'Boulder':
            continue
        rock = npc['x'], npc['y']
        for dx, dy in pt.DELTA.values():
            stance = rock[0]+dx, rock[1]+dy
            if (pt.walkable(name, *stance) and stance not in pt.warp_tiles(name)
                    and plan_pushes(name, stance, [rock], [], target)):
                sources.append(npc['textId'])
                break
    return tuple(sources)


def plan_pushes(name, start, boulders, fixed, target, max_states=30000):
    """Search pushes, quotienting walking positions by reachable component.

    All boulders participate, so a movable obstruction can be rearranged.
    Stair/warp tiles and engine elevation-pair restrictions are respected.
    This never changes the game's blocks, NPCs, flags, or save state.
    """
    fixed, target = set(fixed), tuple(target)
    holes = {tuple(v['target']) for v in BOULDER_TARGETS.values() if v['map'] == name and v['falls']}
    no_walk = fixed | holes
    warps = pt.warp_tiles(name)
    def component(player, rocks):
        queue, seen = deque([player]), {player}
        blocked = no_walk | set(rocks)
        while queue:
            x, y = queue.popleft()
            for direction, (dx, dy) in pt.DELTA.items():
                q = x+dx, y+dy
                if q in warps and pt.warp_triggers(name, *q, direction):
                    continue
                if q not in seen and q not in blocked and pt.walkable_edge(name, (x, y), q):
                    seen.add(q)
                    queue.append(q)
        return seen
    rocks = tuple(sorted(tuple(p) for p in boulders))
    reach = component(tuple(start), rocks)
    key = (min(reach), rocks)
    queue, previous = deque([(key, reach)]), {key: None}
    while queue and len(previous) <= max_states:
        key, reach = queue.popleft()
        rocks = key[1]
        occupied = set(rocks) | fixed
        for rock in rocks:
            for direction, (dx, dy) in pt.DELTA.items():
                stance = rock[0]-dx, rock[1]-dy
                landing = rock[0]+dx, rock[1]+dy
                if stance not in reach or landing in occupied:
                    continue
                if not pt.walkable(name, *landing) or pt.tile_at(name, *landing) == 0x15:
                    continue
                if (pt.MAPS[name]['tileset_id'], frozenset((pt.tile_at(name, *stance),
                                                          pt.tile_at(name, *landing)))) in pt.LAND_PAIRS:
                    continue
                step = {'boulder': rock, 'stance': stance, 'direction': direction, 'landing': landing}
                if landing == target:
                    plan = [step]
                    while previous[key] is not None:
                        key, earlier = previous[key]
                        plan.append(earlier)
                    return list(reversed(plan))
                remaining = set(rocks) - {rock}
                if landing not in holes:
                    remaining.add(landing)
                next_rocks = tuple(sorted(remaining))
                next_reach = component(rock, next_rocks)
                next_key = (min(next_reach), next_rocks)
                if next_key not in previous:
                    previous[next_key] = key, step
                    queue.append((next_key, next_reach))
    return None

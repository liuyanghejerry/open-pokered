"""Geometry-derived field obstacles. Planning copies never alter game state."""
import re
from collections import deque
from contextlib import contextmanager
from functools import lru_cache

import playthrough as pt

_SOURCE = (pt.ROOT / 'crates/pokered-data/src/tileset_data.rs').read_text()
_SWAPS = _SOURCE.split('pub const CUT_TREE_BLOCK_SWAPS:', 1)[1].split('];', 1)[0]
CUT_SWAPS = {int(a, 16): int(b, 16) for a, b in re.findall(r'\((0x\w+), (0x\w+)\)', _SWAPS)}
CUT_TILES = {name: int(re.search(rf'CUT_TREE_TILE_{constant}: u8 = (0x\w+)', _SOURCE)[1], 16)
             for name, constant in [('Overworld', 'OVERWORLD'), ('Gym', 'GYM')]}
_HM_SOURCE = (pt.ROOT / 'crates/pokered-data/src/items.rs').read_text().split('pub const HM_MOVES:', 1)[1].split('];', 1)[0]
HM_MOVES = re.findall(r'MoveId::(\w+)', _HM_SOURCE)
_TM_SOURCE = (pt.ROOT / 'crates/pokered-data/src/items.rs').read_text().split('pub const TM_MOVES:', 1)[1].split('];', 1)[0]
TM_MOVES = re.findall(r'MoveId::(\w+)', _TM_SOURCE)
_WATER_SOURCE = _SOURCE.split('pub const WATER_TILESETS:', 1)[1].split('];', 1)[0]
WATER_TILESETS = set(re.findall(r'TilesetId::(\w+)', _WATER_SOURCE))
_WATER_PAIRS_SOURCE = pt._collision_source.split('pub const TILE_PAIR_COLLISIONS_WATER:', 1)[1].split('];', 1)[0]
WATER_PAIRS = {(int(ts), frozenset((int(a, 16), int(b, 16)))) for ts, a, b in re.findall(
    r'tileset:\s*(\d+),\s*tile1:\s*(0x[0-9A-Fa-f]+),\s*tile2:\s*(0x[0-9A-Fa-f]+)', _WATER_PAIRS_SOURCE)}
SPECIES_NAMES = {p.stem.replace('_', '').upper(): p.stem
                 for p in (pt.ROOT / 'crates/pokered-data/pokemon').glob('*.json')}


def hm_compatible(species, move):
    return machine_compatible(species, 50 + HM_MOVES.index(move))


def machine_compatible(species, bit):
    import playthrough_late as data
    species = SPECIES_NAMES.get(str(species).replace('_', '').upper())
    if species is None:
        return False
    return bool(data.species_data(species)['tmHmFlags'][bit // 8] & (1 << (bit % 8)))


def water_tile(name, x, y):
    tileset = pt.MAPS[name]['tileset_name']
    return tileset in WATER_TILESETS and pt.tile_at(name, x, y) in (
        {0x14, 0x48} if tileset == 'ShipPort' else {0x14, 0x48, 0x32})


@lru_cache(maxsize=1)
def forced_bike_region():
    """Walk-connected road from the engine's forced-bike entry tiles.

    Gate warps end this component and clear the lock in the engine.
    Water there cannot be used as a shortcut around the cycling road.
    """
    source = (pt.ROOT / 'crates/pokered-core/src/overworld/forced_bike.rs').read_text()
    table = source.split('pub const FORCED_BIKE_TILES:', 1)[1].split('];', 1)[0]
    seeds = {(name, int(x), int(y)) for name, x, y in re.findall(r'\(MapId::(\w+), (\d+), (\d+)\)', table)}
    queue, seen = deque(seeds), set(seeds)
    while queue:
        name, x, y = queue.popleft()
        for direction in pt.DELTA:
            node = pt.cross_step(name, x, y, direction)
            if node is None:
                continue
            if node in seen or tuple(node[1:]) in pt.warp_tiles(node[0]):
                continue
            seen.add(node)
            queue.append(node)
    return frozenset(seen)


@contextmanager
def water_planning():
    """Relax only the in-memory planner; restore even when search fails."""
    bike_region = forced_bike_region()
    walkable, edge, cross = pt.walkable, pt.walkable_edge, pt.cross_step
    def passable(name, x, y):
        return walkable(name, x, y) or water_tile(name, x, y)
    def passage(name, start, end):
        if (name, *start) in bike_region and water_tile(name, *end):
            return False
        if water_tile(name, *start) or water_tile(name, *end):
            return passable(name, *end) and (pt.MAPS[name]['tileset_id'], frozenset(
                (pt.tile_at(name, *start), pt.tile_at(name, *end)))) not in WATER_PAIRS
        return edge(name, start, end)
    def crossing(name, x, y, direction):
        node = cross(name, x, y, direction)
        # The field-menu skill starts Surf at an adjacent tile of the
        # current map. A relaxed BFS must not prefer a land-to-water map
        # connection that has no executable embarkation, hiding a valid
        # shore elsewhere. Surfing across water-to-water connections and
        # landing on the next map remain valid.
        if node and node[0] != name and not water_tile(name, x, y) and water_tile(*node):
            return None
        return node
    try:
        pt.walkable, pt.walkable_edge, pt.cross_step = passable, passage, crossing
        yield
    finally:
        pt.walkable, pt.walkable_edge, pt.cross_step = walkable, edge, cross


def surf_requirement(state, destination, points, last_map, blocked_maps=None, excluded_maps=()):
    """Find a useful water crossing with an already reachable embarkation."""
    if not points:
        return None
    with water_planning():
        path = pt.bfs_cross(state['map_name'], (state['player_x'], state['player_y']),
                            destination, points[0], last_map=last_map, allow_ledges=True, allow_spinners=True,
                            blocked_maps=blocked_maps, excluded_maps=excluded_maps,
                            goal_nodes={(destination, *point) for point in points})
    if not path:
        return None
    root = path[0]
    embark = None
    already_surfing = state.get('player_transport') == 'Surfing' and water_tile(*root)
    if already_surfing:
        embark = {'map': root[0], 'stance': list(root[1:]), 'direction': None}
    previous = root
    crossings = []
    for node, direction in path[1:]:
        if water_tile(*node) and embark is None:
            if node[0] != previous[0] or direction not in pt.DELTA:
                break  # Earlier crossings can still reach a valid frontier.
            embark = {'map': previous[0], 'stance': list(previous[1:]), 'direction': direction}
        elif embark and not water_tile(*node):
            crossing = {'move': 'Surf', **embark, 'landing': list(node), 'destination': destination}
            if already_surfing:
                return crossing
            crossings.append(crossing)
            embark = None
        previous = node
    def land_path(name, point):
        return pt.bfs_cross(root[0], root[1:], name, tuple(point), last_map=last_map,
            allow_ledges=True, allow_spinners=True, blocked_maps=blocked_maps, excluded_maps=excluded_maps)
    # The shortest water-relaxed route may repeatedly cut across ponds
    # that walking can bypass. Start at the furthest reachable shore on
    # that path, and require its landing to be outside our land component.
    for crossing in reversed(crossings):
        if not land_path(crossing['map'], crossing['stance']):
            continue
        landing = crossing['landing']
        if not land_path(landing[0], landing[1:]):
            return crossing
    return None


def cut_requirement(state, destination, points, last_map, blocked_maps=None, excluded_maps=()):
    """Find the first tree on a path that exists only after relaxing CUT.

    Restore every planning map before returning. The returned stance must
    still be reached and CUT selected through real game menus.
    """
    originals = {}
    paths = []
    try:
        for name, map_data in pt.MAPS.items():
            tree_tile = CUT_TILES.get(map_data['tileset_name'])
            if tree_tile is None:
                continue
            swaps = {old: new for old, new in CUT_SWAPS.items()
                     if old < len(pt.BLOCKSETS[map_data['tileset_id']])
                     and tree_tile in [pt.BLOCKSETS[map_data['tileset_id']][old][i]
                                       for i in (4, 6, 12, 14)]}
            if any(block in swaps for block in map_data['blocks']):
                originals[name] = map_data['blocks']
                map_data['blocks'] = [swaps.get(block, block) for block in map_data['blocks']]
        for point in points:
            path = pt.bfs_cross(state['map_name'], (state['player_x'], state['player_y']),
                                destination, point, last_map=last_map, allow_ledges=True, allow_spinners=True,
                                blocked_maps=blocked_maps, excluded_maps=excluded_maps)
            if path:
                paths.append(path)
    finally:
        for name, blocks in originals.items():
            pt.MAPS[name]['blocks'] = blocks
    if not paths:
        return None
    path = min(paths, key=len)
    previous = path[0]
    for node, direction in path[1:]:
        name, x, y = node
        if (previous[0] == name and direction in pt.DELTA
                and not pt.walkable_edge(name, previous[1:], (x, y))):
            # CUT replaces the entire block. A newly open path may cross
            # the tree's companion tile, rather than its visible trunk.
            trees = set()
            for bx, by in [(x // 2, y // 2), (previous[1] // 2, previous[2] // 2)]:
                for tx in (2*bx, 2*bx+1):
                    for ty in (2*by, 2*by+1):
                        if pt.tile_at(name, tx, ty) == CUT_TILES.get(pt.MAPS[name]['tileset_name']):
                            trees.add((tx, ty))
            approaches = []
            for tx, ty in sorted(trees):
                for face, (dx, dy) in pt.DELTA.items():
                    stance = tx-dx, ty-dy
                    if not pt.walkable(name, *stance):
                        continue
                    route = pt.bfs_cross(state['map_name'], (state['player_x'], state['player_y']),
                                         name, stance, last_map=last_map, allow_ledges=True, allow_spinners=True,
                                         blocked_maps=blocked_maps, excluded_maps=excluded_maps)
                    if route:
                        approaches.append((len(route), tx, ty, stance, face))
            if approaches:
                _, tx, ty, stance, face = min(approaches)
                return {'move': 'Cut', 'map': name, 'tree': [tx, ty],
                        'stance': list(stance), 'direction': face,
                        'destination': destination}
        previous = node
    return None

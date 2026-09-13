//! M3 geographic world graph: every map as a node, cardinal connections
//! and warp points as directed edges, plus route queries.
//!
//! Geography only — no story flags, HM gates, or item requirements. Edge
//! kinds are kept explicit (`Connection` / `Warp`) so requirement
//! annotation (surf/cut/flash, key items) can attach later without
//! reshaping the graph.
//!
//! LAST_MAP warps (`dest_map: null` in map.json — exit mats and gate
//! exits) have dynamic destinations resolved by the engine at runtime
//! from its tracked last-outside map. Statically the graph resolves them
//! to candidate parents: maps holding an explicit warp INTO the source
//! map, plus the engine's `scripted_last_map` overrides (the underground
//! path entrances). Travel execution re-resolves authoritatively against
//! the live game's `last_map`.

use pokered_data::map_connections::get_map_connections;
use pokered_data::map_objects::get_map_warps;
use pokered_data::maps::{MapId, NUM_MAPS};
use pokered_core::overworld::map_loading::get_map_tileset;
use pokered_core::overworld::special_terrain::is_outside_map;
use serde::{Deserialize, Serialize};

use crate::Position;

/// Whether `map` is an "outside" map for last_map tracking — the same
/// `is_outside_map` (overworld/plateau tileset) the engine's warp/
/// connection commits use.
fn is_outside(map: MapId) -> bool {
    is_outside_map(get_map_tileset(map))
}

/// How a route leg crosses between two maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteLegKind {
    /// Walking off the map edge (north/south/west/east connection).
    Connection,
    /// Stepping onto a warp tile (door, stairs, exit mat).
    Warp,
}

/// One directed edge of the world graph. Serialized for the wire
/// (`get_world_graph` / route responses); `from`/`to` MapIds stay
/// internal (no serde on MapId).
#[derive(Debug, Clone, Serialize)]
pub struct WorldEdge {
    #[serde(skip)]
    pub from: MapId,
    #[serde(skip)]
    pub to: MapId,
    pub kind: RouteLegKind,
    /// Map names in the repo's PascalCase convention (MapId Debug form).
    pub from_map: String,
    pub to_map: String,
    /// Connection legs: "north" / "south" / "west" / "east".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Connection legs: the map.json connection offset (blocks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i8>,
    /// Warp legs: index into the source map's warp list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warp_index: Option<usize>,
    /// Warp legs: the warp tile on the source map (step units).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_pos: Option<Position>,
    /// Approximate arrival tile on the destination map (the destination
    /// warp's own tile) when statically derivable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_pos: Option<Position>,
    /// True for LAST_MAP warps: `to_map` is a static candidate; the
    /// engine resolves the actual destination at runtime from its
    /// tracked last-outside map.
    pub dynamic_destination: bool,
}

/// One leg of a planned route — the same shape as [`WorldEdge`], reused.
pub type RouteLeg = WorldEdge;

/// Outcome of a `travel_to` run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TravelResult {
    /// Player stands on the destination map.
    Reached,
    /// No route, no tile-level path, or progress repeatedly failed.
    Blocked,
    /// A battle started that the auto-resolver could not finish.
    EnteredBattle,
    /// A script/cutscene took control mid-travel.
    Interrupted,
    /// A crossing landed on an unexpected map (carried in
    /// `expected_map` / `actual_map`).
    MapMismatch,
    /// The party blacked out in an auto-resolved battle.
    Blackout,
    /// The target map does not exist in the world data.
    InvalidTarget,
}

/// Wire/report form of a `travel_to` run.
#[derive(Debug, Clone, Serialize)]
pub struct TravelOutcome {
    pub result: TravelResult,
    /// The map-level plan followed (or attempted last).
    pub legs: Vec<RouteLeg>,
    pub legs_completed: usize,
    /// Game frames simulated.
    pub frames: u32,
    /// Battles auto-resolved along the way.
    pub battles: u32,
    #[serde(rename = "final")]
    pub final_pos: Position,
    pub final_map: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_map: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_map: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// The static geographic graph over all maps.
pub struct WorldGraph {
    edges: Vec<WorldEdge>,
    /// Edge indices grouped by source map id.
    by_from: Vec<Vec<usize>>,
}

fn map_name(map: MapId) -> String {
    format!("{:?}", map)
}

impl WorldGraph {
    /// The process-wide shared graph (static data; built once).
    pub fn shared() -> &'static WorldGraph {
        static GRAPH: std::sync::OnceLock<WorldGraph> = std::sync::OnceLock::new();
        GRAPH.get_or_init(WorldGraph::build)
    }

    /// Build the graph from static map data (connections + warps).
    pub fn build() -> Self {
        // Parents for LAST_MAP resolution: every explicit warp M → D
        // makes M a parent of D.
        let mut parents: Vec<Vec<MapId>> = vec![Vec::new(); NUM_MAPS];
        for i in 0..NUM_MAPS {
            let Some(from) = MapId::from_u8(i as u8) else {
                continue;
            };
            for warp in get_map_warps(from) {
                if let Some(dest) = warp.dest_map {
                    parents[dest as usize].push(from);
                }
            }
        }

        let mut edges = Vec::new();
        let mut by_from: Vec<Vec<usize>> = vec![Vec::new(); NUM_MAPS];
        for i in 0..NUM_MAPS {
            let Some(from) = MapId::from_u8(i as u8) else {
                continue;
            };

            // Cardinal connections (declared per map; each side declares
            // its own, so the reverse leg exists as its own edge).
            let conns = get_map_connections(from);
            for (direction, data) in [
                ("north", conns.north),
                ("south", conns.south),
                ("west", conns.west),
                ("east", conns.east),
            ]
            .into_iter()
            .filter_map(|(dir, data)| data.map(|d| (dir, d)))
            {
                by_from[from as usize].push(edges.len());
                edges.push(WorldEdge {
                    from,
                    to: data.target_map,
                    kind: RouteLegKind::Connection,
                    from_map: map_name(from),
                    to_map: map_name(data.target_map),
                    direction: Some(direction.to_string()),
                    offset: Some(data.offset),
                    warp_index: None,
                    from_pos: None,
                    to_pos: None,
                    dynamic_destination: false,
                });
            }

            // Warps.
            let warps = get_map_warps(from);
            for (warp_index, warp) in warps.iter().enumerate() {
                let from_pos = Some(Position {
                    x: warp.x as i32,
                    y: warp.y as i32,
                });
                match warp.dest_map {
                    Some(dest) => {
                        by_from[from as usize].push(edges.len());
                        edges.push(WorldEdge {
                            from,
                            to: dest,
                            kind: RouteLegKind::Warp,
                            from_map: map_name(from),
                            to_map: map_name(dest),
                            direction: None,
                            offset: None,
                            warp_index: Some(warp_index),
                            from_pos,
                            to_pos: resolve_dest_pos(dest, warp.dest_warp_id),
                            dynamic_destination: false,
                        });
                    }
                    None => {
                        // LAST_MAP: engine's scripted overrides first,
                        // then parent maps — filtered to OUTSIDE maps,
                        // since the engine's last_map only updates on
                        // outside tilesets (exit mats return outdoors).
                        // Unfiltered parents remain as a fallback for
                        // maps with no outdoor parent (deep dungeons).
                        let mut candidates: Vec<MapId> = pokered_core::overworld::map_loading::scripted_last_map(from).into_iter().collect();
                        if candidates.is_empty() {
                            candidates.extend(
                                parents[from as usize]
                                    .iter()
                                    .copied()
                                    .filter(|&p| is_outside(p)),
                            );
                            if candidates.is_empty() {
                                candidates.extend(parents[from as usize].iter().copied());
                            }
                        }
                        for dest in candidates {
                            by_from[from as usize].push(edges.len());
                            edges.push(WorldEdge {
                                from,
                                to: dest,
                                kind: RouteLegKind::Warp,
                                from_map: map_name(from),
                                to_map: map_name(dest),
                                direction: None,
                                offset: None,
                                warp_index: Some(warp_index),
                                from_pos,
                                to_pos: resolve_dest_pos(dest, warp.dest_warp_id),
                                dynamic_destination: true,
                            });
                        }
                    }
                }
            }
        }
        Self { edges, by_from }
    }

    pub fn edges(&self) -> &[WorldEdge] {
        &self.edges
    }

    /// Edges leaving `from` (empty for maps that exist in the enum but
    /// have no map data).
    pub fn edges_from(&self, from: MapId) -> impl Iterator<Item = &WorldEdge> {
        self.by_from[from as usize]
            .iter()
            .map(move |&i| &self.edges[i])
    }

    /// BFS shortest route (fewest legs) from `from` to `to`. Returns the
    /// legs in traversal order; `Some(vec![])` when `from == to`;
    /// `None` when unreachable.
    pub fn find_route(&self, from: MapId, to: MapId) -> Option<Vec<RouteLeg>> {
        if from == to {
            return Some(Vec::new());
        }
        let mut prev: Vec<Option<(usize, usize)>> = vec![None; NUM_MAPS]; // map -> (parent map, edge idx)
        let mut visited = vec![false; NUM_MAPS];
        let mut queue = std::collections::VecDeque::new();
        visited[from as usize] = true;
        queue.push_back(from);
        while let Some(current) = queue.pop_front() {
            for &edge_idx in &self.by_from[current as usize] {
                let edge = &self.edges[edge_idx];
                let next = edge.to;
                if visited[next as usize] {
                    continue;
                }
                visited[next as usize] = true;
                prev[next as usize] = Some((current as usize, edge_idx));
                if next == to {
                    let mut legs = Vec::new();
                    let mut cur = to as usize;
                    while let Some((parent, edge_idx)) = prev[cur] {
                        legs.push(self.edges[edge_idx].clone());
                        cur = parent;
                    }
                    legs.reverse();
                    return Some(legs);
                }
                queue.push_back(next);
            }
        }
        None
    }
}

/// Arrival tile on `dest` for a warp pointing at its `dest_warp_id`'th
/// warp entry (step units), when the static data has one.
fn resolve_dest_pos(dest: MapId, dest_warp_id: u8) -> Option<Position> {
    let warps = get_map_warps(dest);
    warps.get(dest_warp_id as usize).map(|w| Position {
        x: w.x as i32,
        y: w.y as i32,
    })
}

// ── Tile-level cross-map routing ────────────────────────────────────
//
// Travel execution needs more than the map-granular route: crossing a
// map like Route 2 (south connection in, north connection out) can
// require warp transit through gate buildings INSIDE the map. This BFS
// runs over (map, x, y) nodes — local steps via the game's own
// collision ([`NavGrid::step`]), warp teleports (explicit destinations
// only; LAST_MAP mats are runtime-dynamic and not plannable), and
// connection crossings — but only connections that land on the goal
// map, which keeps the search scoped to the leg at hand.

use std::collections::{HashMap, VecDeque};

use pokered_core::overworld::Direction;
use pokered_data::map_data_loader::get_map_json;

use crate::nav::NavGrid;

/// How a tile-level leg ends: the crossing out of its map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileCross {
    /// Step onto the leg's final tile (a warp) and let it fire.
    Warp { to_map: MapId, warp_index: usize },
    /// Walk off the map edge in `direction` from the leg's final tile.
    Connection { to_map: MapId, direction: Direction },
}

/// One walk-and-cross segment of a tile-level route: walk `tiles` on
/// `map` (start-exclusive, crossing tile last), then take `cross`
/// (`None` on the final segment — the goal map is reached).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileLeg {
    pub map: MapId,
    pub tiles: Vec<(u16, u16)>,
    pub cross: Option<TileCross>,
}

/// Match `PokemonCollisionProvider`'s `apply_connection_offset`:
/// arrival coordinates shift by `-2 * offset` (offset in blocks).
fn apply_connection_offset(coord: u16, offset: i8) -> u16 {
    (coord as i32 - offset as i32 * 2).max(0) as u16
}

/// One expansion step of the tile BFS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TileStep {
    Walk,
    /// Stepping onto `tile` (a warp tile) teleports to another map.
    WarpCross { warp_index: usize, tile: (u16, u16) },
    ConnCross { direction: Direction },
}

type TileNode = (MapId, u16, u16, MapId);

/// Tile-level route from (`from_map`, `from_pos`) to ANY tile on
/// `goal_map`. `grid_for` supplies a [`NavGrid`] per map (live NPC
/// overlay for the current map, empty for transit maps — the caller's
/// choice). `initial_last_map` is the game's tracked last-outside map
/// for resolving LAST_MAP warps (exit mats), following the engine's
/// semantics: the remembered map updates whenever a transition DEPARTS
/// an outside map. Returns walk/cross segments in order, or `None` when
/// the goal is unreachable through walkable tiles, warps, and
/// goal-bound connections.
pub fn find_tile_route(
    grid_for: &dyn Fn(MapId) -> Option<NavGrid>,
    from_map: MapId,
    from_pos: (u16, u16),
    goal_map: MapId,
    initial_last_map: Option<MapId>,
) -> Option<Vec<TileLeg>> {
    let remembered0 = if is_outside(from_map) {
        from_map
    } else {
        initial_last_map.unwrap_or(from_map)
    };
    let start: TileNode = (from_map, from_pos.0, from_pos.1, remembered0);
    if from_map == goal_map {
        return Some(vec![TileLeg {
            map: from_map,
            tiles: Vec::new(),
            cross: None,
        }]);
    }

    let mut grids: HashMap<MapId, Option<NavGrid>> = HashMap::new();
    let mut prev: HashMap<TileNode, Option<(TileNode, TileStep)>> = HashMap::new();
    prev.insert(start, None);
    let mut queue = VecDeque::from([start]);
    let mut reached: Option<TileNode> = None;

    'bfs: while let Some(cur) = queue.pop_front() {
        let (cur_map, cx, cy, remembered) = cur;
        let cached = grids.entry(cur_map).or_insert_with(|| grid_for(cur_map));
        let Some(grid) = cached.as_ref() else {
            continue;
        };
        // Every transition departing an outside map re-baselines the
        // remembered last-outside map (engine semantics).
        let next_remembered = if is_outside(cur_map) {
            cur_map
        } else {
            remembered
        };
        for dir in [
            Direction::Down,
            Direction::Up,
            Direction::Left,
            Direction::Right,
        ] {
            // Warp teleports and plain steps.
            if let Some((nx, ny)) = grid.step(cx, cy, dir) {
                if grid.is_warp(nx, ny) {
                    // Stepping onto a warp tile fires it — never a plain
                    // walk step.
                    let warps = get_map_warps(cur_map);
                    if let Some((warp_index, warp)) = warps
                        .iter()
                        .enumerate()
                        .find(|(_, w)| (w.x as u16, w.y as u16) == (nx, ny))
                    {
                        // Explicit destination, or the remembered
                        // last-outside map for LAST_MAP mats.
                        let dest = warp.dest_map.or(Some(remembered));
                        if let Some(dest) = dest {
                            if let Some(arrival) = resolve_dest_pos(dest, warp.dest_warp_id) {
                                let next = (dest, arrival.x as u16, arrival.y as u16, next_remembered);
                                if !prev.contains_key(&next) {
                                    prev.insert(
                                        next,
                                        Some((
                                            cur,
                                            TileStep::WarpCross {
                                                warp_index,
                                                tile: (nx, ny),
                                            },
                                        )),
                                    );
                                    if dest == goal_map {
                                        reached = Some(next);
                                        break 'bfs;
                                    }
                                    queue.push_back(next);
                                }
                            }
                        }
                    }
                } else {
                    let next = (cur_map, nx, ny, remembered);
                    if !prev.contains_key(&next) {
                        prev.insert(next, Some((cur, TileStep::Walk)));
                        queue.push_back(next);
                    }
                }
            }
            // Connection crossings into the goal map.
            let conns = get_map_connections(cur_map);
            let (w_blocks, h_blocks) = cur_map.dimensions();
            let (w_tiles, h_tiles) = (w_blocks as u16 * 2, h_blocks as u16 * 2);
            let data = match dir {
                Direction::Up if cy == 0 => conns.north,
                Direction::Down if cy == h_tiles - 1 => conns.south,
                Direction::Left if cx == 0 => conns.west,
                Direction::Right if cx == w_tiles - 1 => conns.east,
                _ => None,
            };
            if let Some(data) = data {
                if data.target_map == goal_map {
                    let (gw_blocks, gh_blocks) = goal_map.dimensions();
                    let arrival = match dir {
                        Direction::Up => (
                            apply_connection_offset(cx, data.offset),
                            gh_blocks as u16 * 2 - 1,
                        ),
                        Direction::Down => (apply_connection_offset(cx, data.offset), 0),
                        Direction::Left => (
                            gw_blocks as u16 * 2 - 1,
                            apply_connection_offset(cy, data.offset),
                        ),
                        Direction::Right => (0, apply_connection_offset(cy, data.offset)),
                        _ => unreachable!(),
                    };
                    let next = (goal_map, arrival.0, arrival.1, next_remembered);
                    if !prev.contains_key(&next) {
                        prev.insert(next, Some((cur, TileStep::ConnCross { direction: dir })));
                        reached = Some(next);
                        break 'bfs;
                    }
                }
            }
        }
    }

    let end = reached?;
    // Reconstruct the node path.
    let mut nodes = vec![end];
    let mut cur = end;
    while let Some(Some((parent, _))) = prev.get(&cur).copied() {
        nodes.push(parent);
        cur = parent;
    }
    nodes.reverse();

    // Compress into walk/cross legs.
    let mut legs: Vec<TileLeg> = Vec::new();
    for i in 1..nodes.len() {
        let node = nodes[i];
        let (_, step) = prev.get(&node).copied().flatten()?;
        if node.0 == nodes[i - 1].0 {
            // Same-map walk: accumulate onto the open leg.
            match legs.last_mut() {
                Some(leg) if leg.cross.is_none() => leg.tiles.push((node.1, node.2)),
                _ => {
                    legs.push(TileLeg {
                        map: node.0,
                        tiles: vec![(node.1, node.2)],
                        cross: None,
                    });
                }
            }
        } else {
            // Crossing: close the open leg with the cross action (a warp
            // cross also appends the warp tile itself as the leg's final
            // walk target — stepping onto it is what fires the warp),
            // then open the leg on the new map.
            let (cross, warp_tile) = match step {
                TileStep::WarpCross { warp_index, tile } => (
                    TileCross::Warp {
                        to_map: node.0,
                        warp_index,
                    },
                    Some(tile),
                ),
                TileStep::ConnCross { direction } => (
                    TileCross::Connection {
                        to_map: node.0,
                        direction,
                    },
                    None,
                ),
                TileStep::Walk => unreachable!("map change without a crossing"),
            };
            if let Some(warp_tile) = warp_tile {
                match legs.last_mut() {
                    Some(leg) if leg.cross.is_none() => leg.tiles.push(warp_tile),
                    _ => {
                        legs.push(TileLeg {
                            map: nodes[i - 1].0,
                            tiles: vec![warp_tile],
                            cross: None,
                        });
                    }
                }
            }
            if let Some(leg) = legs.last_mut() {
                leg.cross = Some(cross);
            } else {
                legs.push(TileLeg {
                    map: nodes[i - 1].0,
                    tiles: Vec::new(),
                    cross: Some(cross),
                });
            }
            legs.push(TileLeg {
                map: node.0,
                tiles: Vec::new(),
                cross: None,
            });
        }
    }
    Some(legs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::map_data_loader::resolve_map_id;

    fn graph() -> WorldGraph {
        WorldGraph::build()
    }

    #[test]
    fn graph_covers_all_maps_and_pallet_edges() {
        let g = graph();
        // Pallet Town: 3 warps + 2 connections.
        let pallet: Vec<_> = g.edges_from(MapId::PalletTown).collect();
        assert_eq!(
            pallet
                .iter()
                .filter(|e| e.kind == RouteLegKind::Warp)
                .count(),
            3
        );
        assert_eq!(
            pallet
                .iter()
                .filter(|e| e.kind == RouteLegKind::Connection)
                .count(),
            2
        );
        // Pallet → Route 1 connection edge, northbound.
        let route1 = pallet
            .iter()
            .find(|e| e.kind == RouteLegKind::Connection && e.to_map == "Route1")
            .expect("Pallet → Route1 connection");
        assert_eq!(route1.direction.as_deref(), Some("north"));
        // Pallet → RedsHouse1F warp edge with both positions.
        let door = pallet
            .iter()
            .find(|e| e.kind == RouteLegKind::Warp && e.to_map == "RedsHouse1F")
            .expect("Pallet → RedsHouse1F warp");
        assert_eq!(door.from_pos, Some(Position { x: 5, y: 5 }));
        assert!(door.to_pos.is_some());
        assert!(!door.dynamic_destination);
    }

    #[test]
    fn last_map_warps_resolve_to_parents() {
        let g = graph();
        // RedsHouse1F's exit mat (dest_map null) → PalletTown, the only
        // map with an explicit warp into RedsHouse1F.
        let exits: Vec<_> = g
            .edges_from(MapId::RedsHouse1F)
            .filter(|e| e.dynamic_destination)
            .collect();
        assert_eq!(exits.len(), 2, "two exit mats (2x1 door span)");
        assert!(exits.iter().all(|e| e.to_map == "PalletTown"));
        // Underground path entrances use the scripted_last_map override.
        assert!(
            g.edges_from(MapId::UndergroundPathRoute5)
                .any(|e| e.dynamic_destination && e.to_map == "Route5"),
            "scripted override resolves the exit to Route5"
        );
    }

    #[test]
    fn route_pallet_to_viridian() {
        let g = graph();
        let legs = g
            .find_route(MapId::PalletTown, MapId::ViridianCity)
            .expect("route exists");
        assert_eq!(legs.len(), 2);
        assert!(legs.iter().all(|e| e.kind == RouteLegKind::Connection));
        let seq: Vec<&str> = legs.iter().map(|e| e.to_map.as_str()).collect();
        assert_eq!(seq, ["Route1", "ViridianCity"]);
    }

    #[test]
    fn route_pallet_to_pewter_is_plausible() {
        let g = graph();
        let legs = g
            .find_route(MapId::PalletTown, MapId::PewterCity)
            .expect("route exists");
        let mut maps: Vec<String> = legs.iter().map(|e| e.from_map.clone()).collect();
        maps.push(legs.last().unwrap().to_map.clone());
        // Map-level route: straight up the connection chain. Crossing
        // Route 2 (through the forest gates) is a tile-level concern —
        // find_tile_route handles it (see below).
        assert_eq!(
            maps,
            ["PalletTown", "Route1", "ViridianCity", "Route2", "PewterCity"]
        );
        assert!(legs.iter().all(|e| e.kind == RouteLegKind::Connection));
        // Route 2's forest-gate warps are graph edges for tile routing.
        let gate_warps: Vec<_> = g
            .edges_from(MapId::Route2)
            .filter(|e| e.kind == RouteLegKind::Warp)
            .collect();
        assert!(gate_warps.iter().any(|e| e.to_map == "ViridianForestSouthGate"));
        assert!(gate_warps.iter().any(|e| e.to_map == "ViridianForestNorthGate"));
    }

    #[test]
    fn route_to_self_is_empty_and_invalid_is_none() {
        let g = graph();
        assert!(
            g.find_route(MapId::PalletTown, MapId::PalletTown)
                .unwrap()
                .is_empty()
        );
        // UnusedMap0B has no map data — unreachable.
        assert!(g.find_route(MapId::PalletTown, MapId::UnusedMap0B).is_none());
    }

    #[test]
    fn resolve_map_id_names_round_trip() {
        assert_eq!(resolve_map_id("PalletTown"), Some(MapId::PalletTown));
        assert_eq!(resolve_map_id("PewterCity"), Some(MapId::PewterCity));
    }

    /// Real-data grids without NPC overlays (transit planning).
    fn concrete_grid(map: MapId) -> Option<NavGrid> {
        get_map_json(map)?;
        let (map_data, _) =
            pokered_core::overworld::map_data_loading::load_full_map_data_concrete(map);
        Some(NavGrid::build(&map_data, &[]))
    }

    #[test]
    fn tile_route_pallet_north_into_route1() {
        // From Pallet's north edge to Route 1: pure connection crossing.
        let legs = find_tile_route(
            &concrete_grid,
            MapId::PalletTown,
            (10, 9),
            MapId::Route1,
            None,
        )
        .expect("tile route");
        let last = legs.last().unwrap();
        assert_eq!(last.map, MapId::Route1);
        // Exactly one connection crossing Pallet → Route1.
        let crosses: Vec<_> = legs.iter().filter_map(|l| l.cross.clone()).collect();
        assert_eq!(crosses.len(), 1);
        assert!(matches!(
            crosses[0],
            TileCross::Connection {
                to_map: MapId::Route1,
                direction: Direction::Up
            }
        ));
    }

    #[test]
    fn tile_route_route2_south_to_pewter_crosses_forest_gates() {
        // The money test for intra-map warp transit: from Route 2's
        // south edge to Pewter City must route through the forest
        // gatehouses (explicit door warps) and the forest itself.
        let legs = find_tile_route(
            &concrete_grid,
            MapId::Route2,
            (10, 70),
            MapId::PewterCity,
            Some(MapId::Route2),
        )
        .expect("tile route through the forest");
        let maps: Vec<MapId> = legs.iter().map(|l| l.map).collect();
        assert_eq!(maps.first(), Some(&MapId::Route2));
        assert!(maps.contains(&MapId::ViridianForestSouthGate), "{maps:?}");
        assert!(maps.contains(&MapId::ViridianForest), "{maps:?}");
        assert!(maps.contains(&MapId::ViridianForestNorthGate), "{maps:?}");
        // Back onto Route 2 before the final Pewter connection.
        let route2_return = maps.iter().rposition(|m| *m == MapId::Route2).unwrap();
        assert!(route2_return > 0, "{maps:?}");
        assert_eq!(legs.last().unwrap().map, MapId::PewterCity);
        // The north gate's LAST_MAP exit resolves to Route 2.
        let north_gate_leg = legs
            .iter()
            .find(|l| l.map == MapId::ViridianForestNorthGate)
            .unwrap();
        assert!(matches!(
            north_gate_leg.cross,
            Some(TileCross::Warp {
                to_map: MapId::Route2,
                ..
            })
        ));
    }
}

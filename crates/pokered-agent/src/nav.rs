//! Local navigation: passability grid + BFS over the step-unit grid.
//!
//! Every edge check goes through the game's own collision —
//! [`check_movement_collision`] with [`PokemonCollisionProvider`] — so
//! planning and actual movement share one source of truth (pair
//! collisions/elevation, ledges, water, counter tiles, NPC sprites). No
//! collision tables are duplicated here; the grid only pre-samples the
//! per-cell tile ids and overlays NPC occupancy and warp tiles.
//!
//! Coordinates are step units (1 step = 2 GB tiles = half a block), the
//! same space the player position and `map.json` warps/NPCs use. Map
//! edges (connections) are treated as blocked: M2 navigation is local to
//! the current map.

use pokered_core::overworld::collision::{
    check_movement_collision, direction_to_pad_input, CollisionProvider, CollisionResult,
    PokemonCollisionProvider, SpritePosition,
};
use pokered_core::overworld::player_movement::direction_delta;
use pokered_core::overworld::{Direction, MapData, TransportMode};
use pokered_data::tilesets::TilesetId;
use serde::{Deserialize, Serialize};

use crate::nearby::NpcObs;
use crate::Position;

/// Outcome of a `move_to` walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationResult {
    /// Player stands on the target tile (an event that fired on the
    /// arrival tile is the caller's to handle — read the final state).
    Reached,
    /// No path exists, or progress repeatedly failed (stuck).
    Blocked,
    /// A script/cutscene took control mid-walk.
    Interrupted,
    /// A battle started mid-walk (wild encounter, trainer sight).
    EnteredBattle,
    /// A dialogue or choice prompt opened mid-walk.
    EnteredDialogue,
    /// The walk crossed a warp or map edge; the map changed.
    MapChanged,
}

/// Outcome of an `interact` / `interact_with` attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractResult {
    /// A dialogue box (or choice prompt) opened.
    Dialogue,
    /// The interaction started a battle (e.g. talking to a trainer).
    Battle,
    /// The A press produced nothing (bare tile, wall).
    Nothing,
    /// A script/cutscene took control instead.
    Interrupted,
    /// The interaction (or approach) crossed a warp; the map changed.
    MapChanged,
    /// No approach path to the entity (`interact_with`).
    Blocked,
    /// Unknown entity id, or the entity is gone/hidden (`interact_with`).
    NotFound,
    /// Navigation toward the entity was interrupted by a battle.
    EnteredBattle,
    /// Navigation toward the entity was interrupted by a dialogue.
    EnteredDialogue,
}

/// Wire/report form of a `move_to` run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationOutcome {
    pub result: NavigationResult,
    /// Tiles successfully walked.
    pub steps: u32,
    /// Game frames simulated.
    pub frames: u32,
    pub start: Position,
    pub target: Position,
    #[serde(rename = "final")]
    pub final_pos: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Wire/report form of an `interact` / `interact_with` run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractOutcome {
    pub result: InteractResult,
    /// The entity id acted on, when one was resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Game frames simulated (including `interact_with` navigation).
    pub frames: u32,
    /// The approach navigation outcome (`interact_with` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<NavigationOutcome>,
    #[serde(rename = "final")]
    pub final_pos: Position,
    pub facing: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Passability grid for one map: pre-sampled tile ids per step cell,
/// plus NPC-occupancy and warp-tile overlays.
pub struct NavGrid {
    map_width_blocks: u8,
    map_height_blocks: u8,
    width_tiles: u16,
    height_tiles: u16,
    tileset: TilesetId,
    tiles: Vec<u8>,
    npc_blocked: Vec<bool>,
    warp: Vec<bool>,
    npc_positions: Vec<SpritePosition>,
    provider: PokemonCollisionProvider,
}

impl NavGrid {
    /// Build the grid from the live map blocks and NPC states. Invisible
    /// NPCs do not block (matching sprite collision); defeated trainers
    /// still stand in the way, as in-game.
    pub fn build(map: &MapData, npcs: &[NpcObs]) -> Self {
        let width_tiles = (map.width as u16) * 2;
        let height_tiles = (map.height as u16) * 2;
        let tileset = pokered_data::tilesets::resolve_concrete(&map.tileset);
        let provider = PokemonCollisionProvider::new(map.id, tileset);
        let cell_count = (width_tiles * height_tiles) as usize;

        let mut tiles = vec![0u8; cell_count];
        for y in 0..height_tiles {
            for x in 0..width_tiles {
                tiles[(y * width_tiles + x) as usize] =
                    provider.get_tile_at_position(tileset, &map.blocks, map.width, x, y);
            }
        }

        let mut npc_blocked = vec![false; cell_count];
        let npc_positions: Vec<SpritePosition> = npcs
            .iter()
            .filter(|n| n.visible)
            .map(|n| SpritePosition { x: n.x, y: n.y })
            .collect();
        for pos in &npc_positions {
            if pos.x < width_tiles && pos.y < height_tiles {
                npc_blocked[(pos.y * width_tiles + pos.x) as usize] = true;
            }
        }

        let mut warp = vec![false; cell_count];
        for w in &map.warps {
            let (x, y) = (w.x as u16, w.y as u16);
            if x < width_tiles && y < height_tiles {
                warp[(y * width_tiles + x) as usize] = true;
            }
        }

        Self {
            map_width_blocks: map.width,
            map_height_blocks: map.height,
            width_tiles,
            height_tiles,
            tileset,
            tiles,
            npc_blocked,
            warp,
            npc_positions,
            provider,
        }
    }

    pub fn in_bounds(&self, x: u16, y: u16) -> bool {
        x < self.width_tiles && y < self.height_tiles
    }

    pub fn tile_at(&self, x: u16, y: u16) -> u8 {
        self.tiles[(y * self.width_tiles + x) as usize]
    }

    /// NPC-occupied cell (visible NPCs only).
    pub fn is_npc_blocked(&self, x: u16, y: u16) -> bool {
        self.in_bounds(x, y) && self.npc_blocked[(y * self.width_tiles + x) as usize]
    }

    /// Warp tile (stepping on it warps; planners avoid it unless it's the goal).
    pub fn is_warp(&self, x: u16, y: u16) -> bool {
        self.in_bounds(x, y) && self.warp[(y * self.width_tiles + x) as usize]
    }

    /// One movement step from `(x, y)` in `dir`, checked with the game's
    /// own collision. Returns the landing cell — two cells on for a ledge
    /// hop — or `None` when the step is blocked (wall, elevation pair,
    /// NPC sprite, counter, water, or map edge).
    pub fn step(&self, x: u16, y: u16, dir: Direction) -> Option<(u16, u16)> {
        if !self.in_bounds(x, y) {
            return None;
        }
        let (dx, dy) = direction_delta(dir);
        let (nx_i, ny_i) = (x as i32 + dx as i32, y as i32 + dy as i32);
        if nx_i < 0 || ny_i < 0 || !self.in_bounds(nx_i as u16, ny_i as u16) {
            // Map edge (connection crossing) is out of scope for local nav.
            return None;
        }
        let (nx, ny) = (nx_i as u16, ny_i as u16);
        let result = check_movement_collision(
            x,
            y,
            dir,
            self.tileset,
            self.map_width_blocks,
            self.map_height_blocks,
            self.tile_at(x, y),
            self.tile_at(nx, ny),
            TransportMode::Walking,
            &self.npc_positions,
            direction_to_pad_input(dir),
            &self.provider,
        );
        match result {
            CollisionResult::Passable => Some((nx, ny)),
            CollisionResult::LedgeJump => {
                let (lx_i, ly_i) = (x as i32 + 2 * dx as i32, y as i32 + 2 * dy as i32);
                if lx_i >= 0 && ly_i >= 0 && self.in_bounds(lx_i as u16, ly_i as u16) {
                    Some((lx_i as u16, ly_i as u16))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// BFS tile path from `start` to `goal`. NPC-occupied cells never route
/// (an NPC on `goal` means unreachable — use `interact_with` semantics);
/// warp cells are avoided mid-path but allowed as `goal`. Returns the
/// cells to walk (goal last, start excluded), `Some(vec![])` when
/// `start == goal`, `None` when unreachable.
pub fn find_path(grid: &NavGrid, start: (u16, u16), goal: (u16, u16)) -> Option<Vec<(u16, u16)>> {
    if start == goal {
        return Some(Vec::new());
    }
    if !grid.in_bounds(start.0, start.1) || !grid.in_bounds(goal.0, goal.1) {
        return None;
    }
    if grid.is_npc_blocked(goal.0, goal.1) {
        return None;
    }

    let width = grid.width_tiles as usize;
    let index_of = |x: u16, y: u16| -> usize { y as usize * width + x as usize };
    let total = (grid.width_tiles * grid.height_tiles) as usize;
    let mut visited = vec![false; total];
    let mut prev: Vec<Option<(u16, u16)>> = vec![None; total];
    let mut queue = std::collections::VecDeque::new();

    visited[index_of(start.0, start.1)] = true;
    queue.push_back(start);

    while let Some((x, y)) = queue.pop_front() {
        for dir in [
            Direction::Down,
            Direction::Up,
            Direction::Left,
            Direction::Right,
        ] {
            let Some((nx, ny)) = grid.step(x, y, dir) else {
                continue;
            };
            // A ledge hop's landing cell must not be NPC-occupied either
            // (the one-cell target already is sprite-checked by `step`).
            if grid.is_npc_blocked(nx, ny) {
                continue;
            }
            if grid.is_warp(nx, ny) && (nx, ny) != goal {
                continue;
            }
            let nidx = index_of(nx, ny);
            if visited[nidx] {
                continue;
            }
            visited[nidx] = true;
            prev[nidx] = Some((x, y));
            if (nx, ny) == goal {
                let mut rev = vec![goal];
                let mut cur = goal;
                while cur != start {
                    cur = prev[index_of(cur.0, cur.1)]?;
                    if cur != start {
                        rev.push(cur);
                    }
                }
                rev.reverse();
                return Some(rev);
            }
            queue.push_back((nx, ny));
        }
    }
    None
}

/// Direction from `from` toward an adjacent (1 or 2 cells, single-axis)
/// cell, matching the paths [`find_path`] produces (2-cell moves are
/// ledge hops).
pub fn direction_between(from: (u16, u16), to: (u16, u16)) -> Option<Direction> {
    let dx = to.0 as i32 - from.0 as i32;
    let dy = to.1 as i32 - from.1 as i32;
    match (dx, dy) {
        (1..=2, 0) => Some(Direction::Right),
        (-2..=-1, 0) => Some(Direction::Left),
        (0, 1..=2) => Some(Direction::Down),
        (0, -2..=-1) => Some(Direction::Up),
        _ => None,
    }
}

/// Path to a cell adjacent to `target`, plus the direction to face from
/// there toward `target`. The target cell itself is never the approach
/// cell (interaction happens facing it). Warp cells are never approach
/// cells. Picks the shortest candidate; `None` when no side is reachable.
pub fn find_approach(
    grid: &NavGrid,
    start: (u16, u16),
    target: (u16, u16),
) -> Option<(Vec<(u16, u16)>, Direction)> {
    let mut best: Option<(Vec<(u16, u16)>, Direction)> = None;
    for dir in [
        Direction::Down,
        Direction::Up,
        Direction::Left,
        Direction::Right,
    ] {
        let (dx, dy) = direction_delta(dir);
        // Standing on `target - delta` and facing `dir` faces the target.
        let (cx_i, cy_i) = (target.0 as i32 - dx as i32, target.1 as i32 - dy as i32);
        if cx_i < 0 || cy_i < 0 {
            continue;
        }
        let cell = (cx_i as u16, cy_i as u16);
        if !grid.in_bounds(cell.0, cell.1)
            || grid.is_npc_blocked(cell.0, cell.1)
            || grid.is_warp(cell.0, cell.1)
        {
            continue;
        }
        let candidate = if cell == start {
            Some((Vec::new(), dir))
        } else {
            find_path(grid, start, cell).map(|path| (path, dir))
        };
        if let Some((path, dir)) = candidate {
            if best.as_ref().map_or(true, |(b, _)| path.len() < b.len()) {
                best = Some((path, dir));
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_core::overworld::MapConnections;
    use pokered_data::maps::MapId;
    use pokered_data::music::MusicId;

    /// Discover one passable and one impassable Overworld block (same
    /// trick as pokered-core's collision tests).
    fn passable_and_wall_blocks() -> (u8, u8) {
        let mut passable = None;
        let mut wall = None;
        for block_id in 0u8..=u8::MAX {
            let Some(tiles) = pokered_data::blockset_data::block_tiles(TilesetId::Overworld, block_id)
            else {
                break;
            };
            // Player standing sub-tile (sub_x = sub_y = 0) → index 4.
            let tile = tiles[4];
            if pokered_data::collision::is_tile_passable(TilesetId::Overworld, tile) {
                passable.get_or_insert(block_id);
            } else {
                wall.get_or_insert(block_id);
            }
            if passable.is_some() && wall.is_some() {
                break;
            }
        }
        (passable.unwrap(), wall.unwrap())
    }

    /// rows of block ids, each row `width` long (tile grid = 2x blocks).
    fn block_map(width: u8, height: u8, rows: &[u8]) -> MapData {
        assert_eq!(rows.len(), (width as usize) * (height as usize));
        MapData::new(
            MapId::PalletTown,
            width,
            height,
            TilesetId::Overworld,
            MusicId::PalletTown,
            rows.to_vec(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            MapConnections::default(),
        )
    }

    fn npc(index: u8, x: u16, y: u16) -> NpcObs {
        NpcObs {
            npc_index: index,
            x,
            y,
            visible: true,
            defeated: false,
        }
    }

    #[test]
    fn straight_path_on_open_floor() {
        let (floor, _) = passable_and_wall_blocks();
        let map = block_map(4, 2, &[floor; 8]);
        let grid = NavGrid::build(&map, &[]);
        let path = find_path(&grid, (0, 0), (7, 1)).unwrap();
        assert_eq!(path.last(), Some(&(7, 1)));
        assert_eq!(path.len(), 8, "Manhattan-length path on open floor");
    }

    #[test]
    fn wall_forces_detour_and_unreachable_returns_none() {
        let (floor, wall) = passable_and_wall_blocks();
        // 2x2 blocks: bottom-right block is a wall; tile grid 4x4 with
        // cells (2..4, 2..4) blocked.
        let map = block_map(2, 2, &[floor, floor, floor, wall]);
        let grid = NavGrid::build(&map, &[]);
        let path = find_path(&grid, (0, 0), (3, 0)).unwrap();
        assert_eq!(path.last(), Some(&(3, 0)));
        assert!(path.iter().all(|&(x, y)| !(x >= 2 && y >= 2)));
        // Target inside the wall block: unreachable.
        assert_eq!(find_path(&grid, (0, 0), (3, 3)), None);
    }

    #[test]
    fn npc_occupied_cells_are_avoided() {
        let (floor, _) = passable_and_wall_blocks();
        // 3x1 blocks → 6x2 tile grid.
        let map = block_map(3, 1, &[floor; 3]);
        let grid = NavGrid::build(&map, &[npc(0, 2, 0)]);
        assert_eq!(find_path(&grid, (0, 0), (2, 0)), None, "NPC tile is not a goal");
        // The path detours through the second row, avoiding the NPC cell.
        let path = find_path(&grid, (0, 0), (4, 0)).unwrap();
        assert!(!path.contains(&(2, 0)), "path avoids the NPC cell");
        // NPCs on both rows seal the corridor.
        let grid = NavGrid::build(&map, &[npc(0, 2, 0), npc(1, 2, 1)]);
        assert_eq!(find_path(&grid, (0, 0), (4, 0)), None, "NPCs seal the corridor");
        // Invisible NPCs don't block.
        let mut hidden = npc(0, 2, 0);
        hidden.visible = false;
        let grid = NavGrid::build(&map, &[hidden, npc(1, 2, 1)]);
        let path = find_path(&grid, (0, 0), (4, 0)).unwrap();
        assert!(path.contains(&(2, 0)), "route crosses the invisible NPC's cell");
    }

    #[test]
    fn warp_cells_are_avoided_mid_path_but_allowed_as_goal() {
        let (floor, _) = passable_and_wall_blocks();
        // 3x1 blocks → 6x2 tile grid.
        let mut map = block_map(3, 1, &[floor; 3]);
        map.warps
            .push(pokered_core::overworld::WarpPoint::new(2, 0, MapId::RedsHouse1F, 0));
        let grid = NavGrid::build(&map, &[]);
        // The path detours through the second row, avoiding the warp cell.
        let path = find_path(&grid, (0, 0), (4, 0)).unwrap();
        assert!(!path.contains(&(2, 0)), "path avoids the warp cell");
        // Warps on both rows seal the corridor.
        map.warps
            .push(pokered_core::overworld::WarpPoint::new(2, 1, MapId::RedsHouse1F, 0));
        let grid = NavGrid::build(&map, &[]);
        assert_eq!(find_path(&grid, (0, 0), (4, 0)), None, "warps seal the corridor");
        // …but a warp is allowed as the goal itself.
        let path = find_path(&grid, (0, 0), (2, 0)).unwrap();
        assert_eq!(path.last(), Some(&(2, 0)), "warp is allowed as the goal");
    }

    #[test]
    fn approach_picks_reachable_side_and_facing() {
        let (floor, wall) = passable_and_wall_blocks();
        // 3x3 blocks, wall in the center block; target the wall's center
        // tile (3, 3)… actually target an open cell with one walled side.
        let map = block_map(3, 3, &[floor, floor, floor, floor, wall, floor, floor, floor, floor]);
        let grid = NavGrid::build(&map, &[]);
        // Target (2, 2) — inside the wall block, unreachable as a goal…
        assert_eq!(find_path(&grid, (0, 0), (2, 2)), None);
        // …but approachable: shortest side wins and faces the target.
        let (path, facing) = find_approach(&grid, (0, 0), (2, 2)).unwrap();
        let stand = path.last().copied().unwrap_or((0, 0));
        let (dx, dy) = direction_delta(facing);
        assert_eq!(
            (stand.0 as i32 + dx as i32, stand.1 as i32 + dy as i32),
            (2, 2),
            "facing from the approach cell points at the target"
        );
    }

    #[test]
    fn direction_between_covers_single_and_ledge_moves() {
        assert_eq!(direction_between((4, 4), (5, 4)), Some(Direction::Right));
        assert_eq!(direction_between((4, 4), (4, 6)), Some(Direction::Down));
        assert_eq!(direction_between((4, 4), (4, 3)), Some(Direction::Up));
        assert_eq!(direction_between((4, 4), (2, 4)), Some(Direction::Left));
        assert_eq!(direction_between((4, 4), (5, 5)), None);
        assert_eq!(direction_between((4, 4), (4, 4)), None);
    }
}

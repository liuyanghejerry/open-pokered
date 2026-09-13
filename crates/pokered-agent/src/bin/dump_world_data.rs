//! Dump the static world data M7 variant tooling needs, as one JSON object
//! on stdout:
//!
//! ```json
//! {
//!   "maps":  { "<MapName>": { "width_tiles": W, "height_tiles": H,
//!                             "walkable": ["0101...", ...] } },
//!   "edges": [ <WorldEdge JSON, same shape as get_world_graph> ]
//! }
//! ```
//!
//! `walkable` has one string per step-cell row (the game's half-block
//! movement cells, `2*width_blocks` columns). Cell codes: `1` = walkable,
//! `2` = water (surfable but not walkable — item balls may be faced from
//! a surfing tile), `0` = solid. Walkability samples the exact tile the
//! engine's collision lookup (`get_tile_at_position`) samples; water
//! detection mirrors `is_water_tile` ($14/$48/$32, ShipPort excepted).
//!
//! The map source is pokered-data's filesystem loader, so `POKERED_MAPS_DIR`
//! selects the maps tree (base `crates/pokered-data/maps` or a generated
//! variant). Run from the workspace root:
//!
//! ```sh
//! POKERED_MAPS_DIR=target/agent/variants/v1 cargo run -p pokered-agent --bin dump_world_data
//! ```

use pokered_data::blockset_data::block_tiles;
use pokered_data::collision::is_tile_passable;
use pokered_data::map_data_loader::{all_map_names, get_block_data, get_map_json, resolve_map_id};

fn main() {
    let mut maps = serde_json::Map::new();
    let mut names = all_map_names();
    names.sort();
    for name in names {
        let Some(map_id) = resolve_map_id(name) else {
            continue;
        };
        let Some(json) = get_map_json(map_id) else {
            continue;
        };
        let blocks = get_block_data(map_id);
        let (w, h) = (json.header.width as usize, json.header.height as usize);
        if blocks.len() < w * h || w == 0 || h == 0 {
            eprintln!("warn: {name}: block data {} bytes for {w}x{h} blocks; skipped", blocks.len());
            continue;
        }
        let tileset = pokered_core::overworld::map_loading::get_map_tileset(map_id);
        let width_tiles = w * 2;
        let height_tiles = h * 2;
        let mut rows = Vec::with_capacity(height_tiles);
        for y in 0..height_tiles {
            let mut row = String::with_capacity(width_tiles);
            for x in 0..width_tiles {
                let block_id = blocks[(y / 2) * w + (x / 2)];
                let code = block_tiles(tileset, block_id)
                    .map(|t| {
                        let tile = t[((y % 2) * 2 + 1) * 4 + (x % 2) * 2];
                        if is_tile_passable(tileset, tile) {
                            '1'
                        } else if tile == 0x14
                            || tile == 0x48
                            || (tile == 0x32
                                && tileset != pokered_data::tilesets::TilesetId::ShipPort)
                        {
                            '2'
                        } else {
                            '0'
                        }
                    })
                    .unwrap_or('0');
                row.push(code);
            }
            rows.push(row);
        }
        maps.insert(
            name.to_string(),
            serde_json::json!({
                "width_tiles": width_tiles,
                "height_tiles": height_tiles,
                "walkable": rows,
            }),
        );
    }

    let graph = pokered_agent::world::WorldGraph::build();
    let out = serde_json::json!({
        "maps": maps,
        "edges": graph.edges(),
    });
    println!("{}", serde_json::to_string(&out).expect("world data serializes"));
}

use std::sync::OnceLock;

use crate::map_data_loader::{get_map_json, resolve_map_id};
use crate::maps::MapId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarpData {
    pub x: u8,
    pub y: u8,
    pub dest_map: Option<MapId>,
    pub dest_warp_id: u8,
}

pub struct MapWarpEntry {
    pub warps: &'static [WarpData],
}

fn convert_warp(warp: &crate::map_json::WarpJson) -> WarpData {
    WarpData {
        x: warp.x,
        y: warp.y,
        dest_map: warp.dest_map.as_ref().and_then(|name| resolve_map_id(name)),
        dest_warp_id: warp.dest_warp_id,
    }
}

struct WarpDataCache {
    entries: Vec<Vec<WarpData>>,
}

static WARP_CACHE: OnceLock<WarpDataCache> = OnceLock::new();

fn get_cache() -> &'static WarpDataCache {
    WARP_CACHE.get_or_init(|| {
        let mut entries = Vec::with_capacity(248);
        for i in 0..248u8 {
            if let Some(map_id) = MapId::from_u8(i) {
                let warps = get_map_json(map_id)
                    .map(|json| json.warps.iter().map(convert_warp).collect::<Vec<_>>())
                    .unwrap_or_default();
                entries.push(warps);
            } else {
                entries.push(Vec::new());
            }
        }
        WarpDataCache { entries }
    })
}

pub fn get_map_warps(map: MapId) -> &'static [WarpData] {
    let idx = map as usize;
    let cache = get_cache();
    if idx < cache.entries.len() {
        &cache.entries[idx]
    } else {
        &[]
    }
}

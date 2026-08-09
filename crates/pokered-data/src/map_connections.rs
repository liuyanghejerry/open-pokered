use std::sync::OnceLock;

use crate::map_data_loader::{get_map_json, resolve_map_id};
use crate::maps::MapId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionData {
    pub target_map: MapId,
    pub offset: i8,
}

#[derive(Debug, Clone, Copy)]
pub struct MapConnectionEntry {
    pub north: Option<ConnectionData>,
    pub south: Option<ConnectionData>,
    pub west: Option<ConnectionData>,
    pub east: Option<ConnectionData>,
}

impl MapConnectionEntry {
    pub const NONE: Self = Self {
        north: None,
        south: None,
        west: None,
        east: None,
    };

    pub const fn connection_count(&self) -> u8 {
        let mut c = 0;
        if self.north.is_some() {
            c += 1;
        }
        if self.south.is_some() {
            c += 1;
        }
        if self.west.is_some() {
            c += 1;
        }
        if self.east.is_some() {
            c += 1;
        }
        c
    }
}

fn convert_connection_entry(
    entry: &crate::map_json::ConnectionEntryJson,
) -> Option<ConnectionData> {
    let target_map = resolve_map_id(&entry.target_map)?;
    Some(ConnectionData {
        target_map,
        offset: entry.offset,
    })
}

fn build_connection_entry(map_id: MapId) -> MapConnectionEntry {
    match get_map_json(map_id) {
        Some(json) => MapConnectionEntry {
            north: json
                .connections
                .north
                .as_ref()
                .and_then(convert_connection_entry),
            south: json
                .connections
                .south
                .as_ref()
                .and_then(convert_connection_entry),
            west: json
                .connections
                .west
                .as_ref()
                .and_then(convert_connection_entry),
            east: json
                .connections
                .east
                .as_ref()
                .and_then(convert_connection_entry),
        },
        None => MapConnectionEntry::NONE,
    }
}

struct ConnectionCache {
    entries: Vec<MapConnectionEntry>,
}

static CONN_CACHE: OnceLock<ConnectionCache> = OnceLock::new();

fn get_cache() -> &'static ConnectionCache {
    CONN_CACHE.get_or_init(|| {
        let mut entries = Vec::with_capacity(248);
        for i in 0..248u8 {
            if let Some(map_id) = MapId::from_u8(i) {
                entries.push(build_connection_entry(map_id));
            } else {
                entries.push(MapConnectionEntry::NONE);
            }
        }
        ConnectionCache { entries }
    })
}

pub fn get_map_connections(map: MapId) -> MapConnectionEntry {
    let idx = map as usize;
    let cache = get_cache();
    if idx < cache.entries.len() {
        cache.entries[idx]
    } else {
        MapConnectionEntry::NONE
    }
}

/// Lazily-initialized slice of all 248 map connection entries, indexed by MapId.
/// Provides the same interface as the old `static MAP_CONNECTIONS: [MapConnectionEntry; 248]`.
pub static MAP_CONNECTIONS: LazyConnections = LazyConnections;

pub struct LazyConnections;

impl LazyConnections {
    pub fn iter(&self) -> std::slice::Iter<'static, MapConnectionEntry> {
        get_cache().entries.iter()
    }
}

impl std::ops::Index<usize> for LazyConnections {
    type Output = MapConnectionEntry;
    fn index(&self, index: usize) -> &Self::Output {
        &get_cache().entries[index]
    }
}

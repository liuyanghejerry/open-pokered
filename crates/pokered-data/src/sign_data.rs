use std::sync::OnceLock;

use crate::map_data_loader::get_map_json;
use crate::maps::MapId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignEntry {
    pub x: u8,
    pub y: u8,
    pub text_id: u8,
}

struct SignDataCache {
    entries: Vec<Vec<SignEntry>>,
}

static SIGN_CACHE: OnceLock<SignDataCache> = OnceLock::new();

fn get_cache() -> &'static SignDataCache {
    SIGN_CACHE.get_or_init(|| {
        let mut entries = Vec::with_capacity(248);
        for i in 0..248u8 {
            if let Some(map_id) = MapId::from_u8(i) {
                let signs = get_map_json(map_id)
                    .map(|json| {
                        json.signs
                            .iter()
                            .map(|s| SignEntry {
                                x: s.x,
                                y: s.y,
                                text_id: s.text_id,
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                entries.push(signs);
            } else {
                entries.push(Vec::new());
            }
        }
        SignDataCache { entries }
    })
}

pub fn get_map_signs(map: MapId) -> &'static [SignEntry] {
    let idx = map as usize;
    let cache = get_cache();
    if idx < cache.entries.len() {
        &cache.entries[idx]
    } else {
        &[]
    }
}

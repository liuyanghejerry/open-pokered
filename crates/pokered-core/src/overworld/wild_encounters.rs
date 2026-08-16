use dotzuki_engine::tileset::TilesetTrait;
use pokered_data::map_constants::FIRST_INDOOR_MAP;
use pokered_data::maps::MapId;
use pokered_data::tileset_data::get_tileset_header;
use pokered_data::tilesets::TilesetId;
use pokered_data::wild_data::{wild_data_for_map, GameVersion, MapWildData, WildEncounterTable};

use crate::battle::wild::{
    try_wild_encounter, try_wild_encounter_with_rate, EncounterContext, WildEncounterRandoms,
    WildEncounterResult,
};

pub const WATER_TILE: u8 = 0x14;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileEncounterType {
    Grass,
    Water,
    IndoorCave,
    None,
}

pub fn determine_encounter_type<T: TilesetTrait>(
    standing_tile: u8,
    tileset: T,
    map_id: MapId,
) -> TileEncounterType {
    let concrete = TilesetId::from_u8(tileset.id()).unwrap_or(TilesetId::Overworld);
    let header = get_tileset_header(concrete);

    if header.is_grass_tile(standing_tile) {
        return TileEncounterType::Grass;
    }

    if standing_tile == WATER_TILE {
        return TileEncounterType::Water;
    }

    if (map_id as u8) >= FIRST_INDOOR_MAP && tileset.name() != "forest" {
        return TileEncounterType::IndoorCave;
    }

    TileEncounterType::None
}

/// The RATE anchor tile: the original rolls the encounter check against the
/// tile at screen (9,9) — the player's standing position shifted HALF a block
/// right, i.e. the tile RIGHT of the standing tile (wild_encounters.asm:28-47).
/// Standing on a shore tile whose right neighbour is water therefore rolls the
/// WATER rate; standing on grass whose right neighbour is not grass rolls
/// NOTHING outdoors.
pub fn determine_rate_encounter_type<T: TilesetTrait>(
    right_tile: u8,
    tileset: T,
    map_id: MapId,
) -> TileEncounterType {
    // Same classification, applied to the right-neighbour tile. The indoor
    // catch-all keys on the MAP, not the tile, so it holds for the shifted
    // anchor too.
    determine_encounter_type(right_tile, tileset, map_id)
}

pub fn select_encounter_table(
    encounter_type: TileEncounterType,
    wild_data: &MapWildData,
) -> Option<&WildEncounterTable> {
    match encounter_type {
        TileEncounterType::Grass | TileEncounterType::IndoorCave => Some(&wild_data.grass),
        TileEncounterType::Water => Some(&wild_data.water),
        TileEncounterType::None => Option::None,
    }
}

pub fn should_check_encounter(
    on_warp_tile: bool,
    npc_script_active: bool,
    encounter_cooldown: u8,
) -> bool {
    !on_warp_tile && !npc_script_active && encounter_cooldown == 0
}

/// The per-anchor encounter RATE, exactly like the original: the rate comes
/// from the RATE anchor tile (grass-rate / water-rate / indoor=grass-rate),
/// INDEPENDENT of which table supplies the encounter list (the "left shore"
/// split, wild_encounters.asm:28-72). Returns None when the anchor gives no
/// roll (outdoor, anchor not grass/water).
fn anchor_encounter_rate(
    rate_type: TileEncounterType,
    wild_data: &MapWildData,
) -> Option<u8> {
    match rate_type {
        TileEncounterType::Grass | TileEncounterType::IndoorCave => Some(wild_data.grass.encounter_rate),
        TileEncounterType::Water => Some(wild_data.water.encounter_rate),
        TileEncounterType::None => Option::None,
    }
}

pub fn check_wild_encounter<T: TilesetTrait>(
    map_id: MapId,
    tileset: T,
    standing_tile: u8,
    right_tile: u8,
    version: GameVersion,
    randoms: &WildEncounterRandoms,
    context: &EncounterContext,
    on_warp_tile: bool,
    npc_script_active: bool,
    encounter_cooldown: u8,
) -> WildEncounterResult {
    if !should_check_encounter(on_warp_tile, npc_script_active, encounter_cooldown) {
        return WildEncounterResult::NoEncounter;
    }

    let wild_data = match wild_data_for_map(map_id, version) {
        Some(data) => data,
        Option::None => return WildEncounterResult::NoEncounter,
    };

    // Two anchors, exactly like the original (wild_encounters.asm:28-72):
    //   * the RATE anchor is screen (9,9) — the tile RIGHT of the standing
    //     tile — deciding the encounter RATE (grass-rate / water-rate / none);
    //   * the TABLE anchor is screen (8,9) — the STANDING tile — choosing
    //     between the grass and water encounter LISTS. The split produces the
    //     famous "left shore" quirk: standing on a shore tile whose right
    //     neighbour is water rolls the WATER rate but reads the GRASS list.
    let rate_type = determine_rate_encounter_type(right_tile, tileset, map_id);
    let rate = match anchor_encounter_rate(rate_type, &wild_data) {
        Some(r) => r,
        Option::None => return WildEncounterResult::NoEncounter,
    };
    let table_type = determine_encounter_type(standing_tile, tileset, map_id);
    let table = match table_type {
        TileEncounterType::Water => &wild_data.water,
        _ => &wild_data.grass, // asm treats (8,9) ≠ $14 as the grass list
    };

    try_wild_encounter_with_rate(table, rate, randoms, context)
}

/// Game-agnostic [`dotzuki_engine::overworld::encounter::EncounterProvider`] impl
/// for pokered.
///
/// Adapts the existing Gen-1 wild-encounter path ([`check_wild_encounter`] /
/// [`try_wild_encounter`] + the [`select_encounter_table`] tables) to the
/// engine's encounter driver, so [`EncounterEngine::on_step`] can own the
/// step -> maybe-encounter control flow while pokered keeps owning every table,
/// rate, slot distribution, and the repel/cooldown quirks.
///
/// The struct carries all the legacy inputs the engine signature does not pass
/// (tileset, standing tile, game version, repel context, gating flags). The
/// `map_id`/`x`/`y` the engine passes are advisory here - the real map is the
/// captured [`MapId`] - matching how pokered already resolves encounters from
/// captured overworld state rather than raw coordinates.
///
/// [`EncounterEngine::on_step`]: dotzuki_engine::overworld::encounter::EncounterEngine::on_step
pub struct PokeredEncounterProvider<T: TilesetTrait> {
    map_id: MapId,
    tileset: T,
    standing_tile: u8,
    version: GameVersion,
    context: EncounterContext,
    on_warp_tile: bool,
    npc_script_active: bool,
    encounter_cooldown: u8,
}

impl<T: TilesetTrait> PokeredEncounterProvider<T> {
    /// Build a provider snapshot from the same inputs [`check_wild_encounter`]
    /// takes.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        map_id: MapId,
        tileset: T,
        standing_tile: u8,
        version: GameVersion,
        context: EncounterContext,
        on_warp_tile: bool,
        npc_script_active: bool,
        encounter_cooldown: u8,
    ) -> Self {
        Self {
            map_id,
            tileset,
            standing_tile,
            version,
            context,
            on_warp_tile,
            npc_script_active,
            encounter_cooldown,
        }
    }
}

impl<T: TilesetTrait + Copy> dotzuki_engine::overworld::encounter::EncounterProvider
    for PokeredEncounterProvider<T>
{
    type Species = pokered_data::species::Species;

    fn is_encounter_tile(&self, _map_id: u32, _x: i32, _y: i32) -> bool {
        // Cheap gate, no RNG: the step must be allowed, a wild table must exist
        // for this map, and the standing tile must classify as grass/water/cave.
        if !should_check_encounter(
            self.on_warp_tile,
            self.npc_script_active,
            self.encounter_cooldown,
        ) {
            return false;
        }
        let Some(wild_data) = wild_data_for_map(self.map_id, self.version) else {
            return false;
        };
        let encounter_type =
            determine_encounter_type(self.standing_tile, self.tileset, self.map_id);
        select_encounter_table(encounter_type, &wild_data).is_some()
    }

    fn roll_encounter(
        &self,
        _map_id: u32,
        _x: i32,
        _y: i32,
        _mode: dotzuki_engine::overworld::encounter::EncounterMode,
        rng: &mut dyn dotzuki_engine::battle::rng::BattleRng,
    ) -> Option<(Self::Species, u8)> {
        // Draw exactly the legacy two bytes, IN THE LEGACY ORDER:
        // hRandomAdd (encounter rate roll) first, then hRandomSub (slot roll).
        let randoms = WildEncounterRandoms {
            encounter_roll: rng.next_u8(),
            slot_roll: rng.next_u8(),
        };

        let wild_data = wild_data_for_map(self.map_id, self.version)?;
        let encounter_type =
            determine_encounter_type(self.standing_tile, self.tileset, self.map_id);
        let table = select_encounter_table(encounter_type, &wild_data);

        match try_wild_encounter(table, &randoms, &self.context) {
            WildEncounterResult::Encounter { level, species } => Some((species, level)),
            WildEncounterResult::NoEncounter | WildEncounterResult::RepelBlocked => None,
        }
    }
}

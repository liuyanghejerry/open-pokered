use std::collections::HashMap;
use std::sync::OnceLock;

use crate::map_json::MapJson;
use crate::maps::{MapId, NUM_MAPS};

struct MapDataStore {
    maps: HashMap<String, MapJson>,
    blocks: HashMap<String, Vec<u8>>,
}

static MAP_DATA: OnceLock<MapDataStore> = OnceLock::new();

fn get_store() -> &'static MapDataStore {
    MAP_DATA.get_or_init(|| init_map_data())
}

fn build_name_to_id() -> HashMap<String, MapId> {
    let mut map = HashMap::new();
    for i in 0..NUM_MAPS {
        if let Some(id) = MapId::from_u8(i as u8) {
            map.insert(format!("{:?}", id), id);
        }
    }
    map
}

pub fn get_map_json(map_id: MapId) -> Option<&'static MapJson> {
    let name = format!("{:?}", map_id);
    // Editor-injected runtime override shadows the baseline (embedded or disk).
    if let Some(ov) = crate::runtime_overrides::map_override(&name) {
        return Some(ov);
    }
    get_store().maps.get(&name)
}

pub fn get_block_data(map_id: MapId) -> &'static [u8] {
    let name = format!("{:?}", map_id);
    // Editor-injected runtime override shadows the baseline (embedded or disk).
    if let Some(ov) = crate::runtime_overrides::blk_override(&name) {
        return ov;
    }
    get_store()
        .blocks
        .get(&name)
        .map(|v| v.as_slice())
        .unwrap_or(&[])
}

pub fn all_map_names() -> Vec<&'static str> {
    get_store().maps.keys().map(|s| s.as_str()).collect()
}

pub fn name_to_map_id() -> &'static HashMap<String, MapId> {
    static NAME_MAP: OnceLock<HashMap<String, MapId>> = OnceLock::new();
    NAME_MAP.get_or_init(build_name_to_id)
}

pub fn resolve_map_id(name: &str) -> Option<MapId> {
    name_to_map_id().get(name).copied()
}

// ── Embedded mode ──────────────────────────────────────────────────────────

#[cfg(feature = "embedded-map-data")]
use crate::map_json::{
    SignJson, StaticConnectionEntryJson, StaticConnectionsJson, StaticMapHeaderJson,
    StaticMapJson, StaticMapTextJson, StaticNpcJson, StaticTextPageJson, StaticVersionWildJson,
    StaticWarpJson, StaticWildDataJson, StaticWildEncounterTableJson, StaticWildMonJson,
};

#[cfg(feature = "embedded-map-data")]
include!(concat!(env!("OUT_DIR"), "/map_data_gen.rs"));

#[cfg(feature = "embedded-map-data")]
fn embedded_blk_sources() -> &'static [(&'static str, &'static [u8])] {
    &[
        ("AgathasRoom", include_bytes!("../maps/AgathasRoom/map.blk")),
        ("BikeShop", include_bytes!("../maps/BikeShop/map.blk")),
        ("BillsHouse", include_bytes!("../maps/BillsHouse/map.blk")),
        ("BluesHouse", include_bytes!("../maps/BluesHouse/map.blk")),
        ("BrunosRoom", include_bytes!("../maps/BrunosRoom/map.blk")),
        (
            "CeladonChiefHouse",
            include_bytes!("../maps/CeladonChiefHouse/map.blk"),
        ),
        ("CeladonCity", include_bytes!("../maps/CeladonCity/map.blk")),
        (
            "CeladonDiner",
            include_bytes!("../maps/CeladonDiner/map.blk"),
        ),
        ("CeladonGym", include_bytes!("../maps/CeladonGym/map.blk")),
        (
            "CeladonHotel",
            include_bytes!("../maps/CeladonHotel/map.blk"),
        ),
        (
            "CeladonMansion1F",
            include_bytes!("../maps/CeladonMansion1F/map.blk"),
        ),
        (
            "CeladonMansion2F",
            include_bytes!("../maps/CeladonMansion2F/map.blk"),
        ),
        (
            "CeladonMansion3F",
            include_bytes!("../maps/CeladonMansion3F/map.blk"),
        ),
        (
            "CeladonMansionRoof",
            include_bytes!("../maps/CeladonMansionRoof/map.blk"),
        ),
        (
            "CeladonMansionRoofHouse",
            include_bytes!("../maps/CeladonMansionRoofHouse/map.blk"),
        ),
        (
            "CeladonMart1F",
            include_bytes!("../maps/CeladonMart1F/map.blk"),
        ),
        (
            "CeladonMart2F",
            include_bytes!("../maps/CeladonMart2F/map.blk"),
        ),
        (
            "CeladonMart3F",
            include_bytes!("../maps/CeladonMart3F/map.blk"),
        ),
        (
            "CeladonMart4F",
            include_bytes!("../maps/CeladonMart4F/map.blk"),
        ),
        (
            "CeladonMart5F",
            include_bytes!("../maps/CeladonMart5F/map.blk"),
        ),
        (
            "CeladonMartElevator",
            include_bytes!("../maps/CeladonMartElevator/map.blk"),
        ),
        (
            "CeladonMartRoof",
            include_bytes!("../maps/CeladonMartRoof/map.blk"),
        ),
        (
            "CeladonPokecenter",
            include_bytes!("../maps/CeladonPokecenter/map.blk"),
        ),
        (
            "CeruleanBadgeHouse",
            include_bytes!("../maps/CeruleanBadgeHouse/map.blk"),
        ),
        (
            "CeruleanCave1F",
            include_bytes!("../maps/CeruleanCave1F/map.blk"),
        ),
        (
            "CeruleanCave2F",
            include_bytes!("../maps/CeruleanCave2F/map.blk"),
        ),
        (
            "CeruleanCaveB1F",
            include_bytes!("../maps/CeruleanCaveB1F/map.blk"),
        ),
        (
            "CeruleanCity",
            include_bytes!("../maps/CeruleanCity/map.blk"),
        ),
        ("CeruleanGym", include_bytes!("../maps/CeruleanGym/map.blk")),
        (
            "CeruleanMart",
            include_bytes!("../maps/CeruleanMart/map.blk"),
        ),
        (
            "CeruleanPokecenter",
            include_bytes!("../maps/CeruleanPokecenter/map.blk"),
        ),
        (
            "CeruleanTradeHouse",
            include_bytes!("../maps/CeruleanTradeHouse/map.blk"),
        ),
        (
            "CeruleanTrashedHouse",
            include_bytes!("../maps/CeruleanTrashedHouse/map.blk"),
        ),
        (
            "ChampionsRoom",
            include_bytes!("../maps/ChampionsRoom/map.blk"),
        ),
        ("CinnabarGym", include_bytes!("../maps/CinnabarGym/map.blk")),
        (
            "CinnabarIsland",
            include_bytes!("../maps/CinnabarIsland/map.blk"),
        ),
        ("CinnabarLab", include_bytes!("../maps/CinnabarLab/map.blk")),
        (
            "CinnabarLabFossilRoom",
            include_bytes!("../maps/CinnabarLabFossilRoom/map.blk"),
        ),
        (
            "CinnabarLabMetronomeRoom",
            include_bytes!("../maps/CinnabarLabMetronomeRoom/map.blk"),
        ),
        (
            "CinnabarLabTradeRoom",
            include_bytes!("../maps/CinnabarLabTradeRoom/map.blk"),
        ),
        (
            "CinnabarMart",
            include_bytes!("../maps/CinnabarMart/map.blk"),
        ),
        (
            "CinnabarPokecenter",
            include_bytes!("../maps/CinnabarPokecenter/map.blk"),
        ),
        ("Colosseum", include_bytes!("../maps/Colosseum/map.blk")),
        (
            "CopycatsHouse1F",
            include_bytes!("../maps/CopycatsHouse1F/map.blk"),
        ),
        (
            "CopycatsHouse2F",
            include_bytes!("../maps/CopycatsHouse2F/map.blk"),
        ),
        ("Daycare", include_bytes!("../maps/Daycare/map.blk")),
        (
            "DiglettsCave",
            include_bytes!("../maps/DiglettsCave/map.blk"),
        ),
        (
            "DiglettsCaveRoute11",
            include_bytes!("../maps/DiglettsCaveRoute11/map.blk"),
        ),
        (
            "DiglettsCaveRoute2",
            include_bytes!("../maps/DiglettsCaveRoute2/map.blk"),
        ),
        (
            "FightingDojo",
            include_bytes!("../maps/FightingDojo/map.blk"),
        ),
        (
            "FuchsiaBillsGrandpasHouse",
            include_bytes!("../maps/FuchsiaBillsGrandpasHouse/map.blk"),
        ),
        ("FuchsiaCity", include_bytes!("../maps/FuchsiaCity/map.blk")),
        (
            "FuchsiaGoodRodHouse",
            include_bytes!("../maps/FuchsiaGoodRodHouse/map.blk"),
        ),
        ("FuchsiaGym", include_bytes!("../maps/FuchsiaGym/map.blk")),
        ("FuchsiaMart", include_bytes!("../maps/FuchsiaMart/map.blk")),
        (
            "FuchsiaMeetingRoom",
            include_bytes!("../maps/FuchsiaMeetingRoom/map.blk"),
        ),
        (
            "FuchsiaPokecenter",
            include_bytes!("../maps/FuchsiaPokecenter/map.blk"),
        ),
        ("GameCorner", include_bytes!("../maps/GameCorner/map.blk")),
        (
            "GameCornerPrizeRoom",
            include_bytes!("../maps/GameCornerPrizeRoom/map.blk"),
        ),
        ("HallOfFame", include_bytes!("../maps/HallOfFame/map.blk")),
        (
            "IndigoPlateau",
            include_bytes!("../maps/IndigoPlateau/map.blk"),
        ),
        (
            "IndigoPlateauLobby",
            include_bytes!("../maps/IndigoPlateauLobby/map.blk"),
        ),
        ("LancesRoom", include_bytes!("../maps/LancesRoom/map.blk")),
        (
            "LavenderCuboneHouse",
            include_bytes!("../maps/LavenderCuboneHouse/map.blk"),
        ),
        (
            "LavenderMart",
            include_bytes!("../maps/LavenderMart/map.blk"),
        ),
        (
            "LavenderPokecenter",
            include_bytes!("../maps/LavenderPokecenter/map.blk"),
        ),
        (
            "LavenderTown",
            include_bytes!("../maps/LavenderTown/map.blk"),
        ),
        (
            "LoreleisRoom",
            include_bytes!("../maps/LoreleisRoom/map.blk"),
        ),
        (
            "MrFujisHouse",
            include_bytes!("../maps/MrFujisHouse/map.blk"),
        ),
        (
            "MrPsychicsHouse",
            include_bytes!("../maps/MrPsychicsHouse/map.blk"),
        ),
        ("MtMoon1F", include_bytes!("../maps/MtMoon1F/map.blk")),
        ("MtMoonB1F", include_bytes!("../maps/MtMoonB1F/map.blk")),
        ("MtMoonB2F", include_bytes!("../maps/MtMoonB2F/map.blk")),
        (
            "MtMoonPokecenter",
            include_bytes!("../maps/MtMoonPokecenter/map.blk"),
        ),
        ("Museum1F", include_bytes!("../maps/Museum1F/map.blk")),
        ("Museum2F", include_bytes!("../maps/Museum2F/map.blk")),
        (
            "NameRatersHouse",
            include_bytes!("../maps/NameRatersHouse/map.blk"),
        ),
        ("OaksLab", include_bytes!("../maps/OaksLab/map.blk")),
        ("PalletTown", include_bytes!("../maps/PalletTown/map.blk")),
        ("PewterCity", include_bytes!("../maps/PewterCity/map.blk")),
        ("PewterGym", include_bytes!("../maps/PewterGym/map.blk")),
        ("PewterMart", include_bytes!("../maps/PewterMart/map.blk")),
        (
            "PewterNidoranHouse",
            include_bytes!("../maps/PewterNidoranHouse/map.blk"),
        ),
        (
            "PewterPokecenter",
            include_bytes!("../maps/PewterPokecenter/map.blk"),
        ),
        (
            "PewterSpeechHouse",
            include_bytes!("../maps/PewterSpeechHouse/map.blk"),
        ),
        (
            "PokemonFanClub",
            include_bytes!("../maps/PokemonFanClub/map.blk"),
        ),
        (
            "PokemonMansion1F",
            include_bytes!("../maps/PokemonMansion1F/map.blk"),
        ),
        (
            "PokemonMansion2F",
            include_bytes!("../maps/PokemonMansion2F/map.blk"),
        ),
        (
            "PokemonMansion3F",
            include_bytes!("../maps/PokemonMansion3F/map.blk"),
        ),
        (
            "PokemonMansionB1F",
            include_bytes!("../maps/PokemonMansionB1F/map.blk"),
        ),
        (
            "PokemonTower1F",
            include_bytes!("../maps/PokemonTower1F/map.blk"),
        ),
        (
            "PokemonTower2F",
            include_bytes!("../maps/PokemonTower2F/map.blk"),
        ),
        (
            "PokemonTower3F",
            include_bytes!("../maps/PokemonTower3F/map.blk"),
        ),
        (
            "PokemonTower4F",
            include_bytes!("../maps/PokemonTower4F/map.blk"),
        ),
        (
            "PokemonTower5F",
            include_bytes!("../maps/PokemonTower5F/map.blk"),
        ),
        (
            "PokemonTower6F",
            include_bytes!("../maps/PokemonTower6F/map.blk"),
        ),
        (
            "PokemonTower7F",
            include_bytes!("../maps/PokemonTower7F/map.blk"),
        ),
        ("PowerPlant", include_bytes!("../maps/PowerPlant/map.blk")),
        ("RedsHouse1F", include_bytes!("../maps/RedsHouse1F/map.blk")),
        ("RedsHouse2F", include_bytes!("../maps/RedsHouse2F/map.blk")),
        (
            "RockTunnel1F",
            include_bytes!("../maps/RockTunnel1F/map.blk"),
        ),
        (
            "RockTunnelB1F",
            include_bytes!("../maps/RockTunnelB1F/map.blk"),
        ),
        (
            "RockTunnelPokecenter",
            include_bytes!("../maps/RockTunnelPokecenter/map.blk"),
        ),
        (
            "RocketHideoutB1F",
            include_bytes!("../maps/RocketHideoutB1F/map.blk"),
        ),
        (
            "RocketHideoutB2F",
            include_bytes!("../maps/RocketHideoutB2F/map.blk"),
        ),
        (
            "RocketHideoutB3F",
            include_bytes!("../maps/RocketHideoutB3F/map.blk"),
        ),
        (
            "RocketHideoutB4F",
            include_bytes!("../maps/RocketHideoutB4F/map.blk"),
        ),
        (
            "RocketHideoutElevator",
            include_bytes!("../maps/RocketHideoutElevator/map.blk"),
        ),
        ("Route1", include_bytes!("../maps/Route1/map.blk")),
        ("Route10", include_bytes!("../maps/Route10/map.blk")),
        ("Route11", include_bytes!("../maps/Route11/map.blk")),
        (
            "Route11Gate1F",
            include_bytes!("../maps/Route11Gate1F/map.blk"),
        ),
        (
            "Route11Gate2F",
            include_bytes!("../maps/Route11Gate2F/map.blk"),
        ),
        ("Route12", include_bytes!("../maps/Route12/map.blk")),
        (
            "Route12Gate1F",
            include_bytes!("../maps/Route12Gate1F/map.blk"),
        ),
        (
            "Route12Gate2F",
            include_bytes!("../maps/Route12Gate2F/map.blk"),
        ),
        (
            "Route12SuperRodHouse",
            include_bytes!("../maps/Route12SuperRodHouse/map.blk"),
        ),
        ("Route13", include_bytes!("../maps/Route13/map.blk")),
        ("Route14", include_bytes!("../maps/Route14/map.blk")),
        ("Route15", include_bytes!("../maps/Route15/map.blk")),
        (
            "Route15Gate1F",
            include_bytes!("../maps/Route15Gate1F/map.blk"),
        ),
        (
            "Route15Gate2F",
            include_bytes!("../maps/Route15Gate2F/map.blk"),
        ),
        ("Route16", include_bytes!("../maps/Route16/map.blk")),
        (
            "Route16FlyHouse",
            include_bytes!("../maps/Route16FlyHouse/map.blk"),
        ),
        (
            "Route16Gate1F",
            include_bytes!("../maps/Route16Gate1F/map.blk"),
        ),
        (
            "Route16Gate2F",
            include_bytes!("../maps/Route16Gate2F/map.blk"),
        ),
        ("Route17", include_bytes!("../maps/Route17/map.blk")),
        ("Route18", include_bytes!("../maps/Route18/map.blk")),
        (
            "Route18Gate1F",
            include_bytes!("../maps/Route18Gate1F/map.blk"),
        ),
        (
            "Route18Gate2F",
            include_bytes!("../maps/Route18Gate2F/map.blk"),
        ),
        ("Route19", include_bytes!("../maps/Route19/map.blk")),
        ("Route2", include_bytes!("../maps/Route2/map.blk")),
        ("Route20", include_bytes!("../maps/Route20/map.blk")),
        ("Route21", include_bytes!("../maps/Route21/map.blk")),
        ("Route22", include_bytes!("../maps/Route22/map.blk")),
        ("Route22Gate", include_bytes!("../maps/Route22Gate/map.blk")),
        ("Route23", include_bytes!("../maps/Route23/map.blk")),
        ("Route24", include_bytes!("../maps/Route24/map.blk")),
        ("Route25", include_bytes!("../maps/Route25/map.blk")),
        ("Route2Gate", include_bytes!("../maps/Route2Gate/map.blk")),
        (
            "Route2TradeHouse",
            include_bytes!("../maps/Route2TradeHouse/map.blk"),
        ),
        ("Route3", include_bytes!("../maps/Route3/map.blk")),
        ("Route4", include_bytes!("../maps/Route4/map.blk")),
        ("Route5", include_bytes!("../maps/Route5/map.blk")),
        ("Route5Gate", include_bytes!("../maps/Route5Gate/map.blk")),
        ("Route6", include_bytes!("../maps/Route6/map.blk")),
        ("Route6Gate", include_bytes!("../maps/Route6Gate/map.blk")),
        ("Route7", include_bytes!("../maps/Route7/map.blk")),
        ("Route7Gate", include_bytes!("../maps/Route7Gate/map.blk")),
        ("Route8", include_bytes!("../maps/Route8/map.blk")),
        ("Route8Gate", include_bytes!("../maps/Route8Gate/map.blk")),
        ("Route9", include_bytes!("../maps/Route9/map.blk")),
        ("SSAnne1F", include_bytes!("../maps/SSAnne1F/map.blk")),
        (
            "SSAnne1FRooms",
            include_bytes!("../maps/SSAnne1FRooms/map.blk"),
        ),
        ("SSAnne2F", include_bytes!("../maps/SSAnne2F/map.blk")),
        (
            "SSAnne2FRooms",
            include_bytes!("../maps/SSAnne2FRooms/map.blk"),
        ),
        ("SSAnne3F", include_bytes!("../maps/SSAnne3F/map.blk")),
        ("SSAnneB1F", include_bytes!("../maps/SSAnneB1F/map.blk")),
        (
            "SSAnneB1FRooms",
            include_bytes!("../maps/SSAnneB1FRooms/map.blk"),
        ),
        ("SSAnneBow", include_bytes!("../maps/SSAnneBow/map.blk")),
        (
            "SSAnneCaptainsRoom",
            include_bytes!("../maps/SSAnneCaptainsRoom/map.blk"),
        ),
        (
            "SSAnneKitchen",
            include_bytes!("../maps/SSAnneKitchen/map.blk"),
        ),
        (
            "SafariZoneCenter",
            include_bytes!("../maps/SafariZoneCenter/map.blk"),
        ),
        (
            "SafariZoneCenterRestHouse",
            include_bytes!("../maps/SafariZoneCenterRestHouse/map.blk"),
        ),
        (
            "SafariZoneEast",
            include_bytes!("../maps/SafariZoneEast/map.blk"),
        ),
        (
            "SafariZoneEastRestHouse",
            include_bytes!("../maps/SafariZoneEastRestHouse/map.blk"),
        ),
        (
            "SafariZoneGate",
            include_bytes!("../maps/SafariZoneGate/map.blk"),
        ),
        (
            "SafariZoneNorth",
            include_bytes!("../maps/SafariZoneNorth/map.blk"),
        ),
        (
            "SafariZoneNorthRestHouse",
            include_bytes!("../maps/SafariZoneNorthRestHouse/map.blk"),
        ),
        (
            "SafariZoneSecretHouse",
            include_bytes!("../maps/SafariZoneSecretHouse/map.blk"),
        ),
        (
            "SafariZoneWest",
            include_bytes!("../maps/SafariZoneWest/map.blk"),
        ),
        (
            "SafariZoneWestRestHouse",
            include_bytes!("../maps/SafariZoneWestRestHouse/map.blk"),
        ),
        ("SaffronCity", include_bytes!("../maps/SaffronCity/map.blk")),
        ("SaffronGym", include_bytes!("../maps/SaffronGym/map.blk")),
        ("SaffronMart", include_bytes!("../maps/SaffronMart/map.blk")),
        (
            "SaffronPidgeyHouse",
            include_bytes!("../maps/SaffronPidgeyHouse/map.blk"),
        ),
        (
            "SaffronPokecenter",
            include_bytes!("../maps/SaffronPokecenter/map.blk"),
        ),
        (
            "SeafoamIslands1F",
            include_bytes!("../maps/SeafoamIslands1F/map.blk"),
        ),
        (
            "SeafoamIslandsB1F",
            include_bytes!("../maps/SeafoamIslandsB1F/map.blk"),
        ),
        (
            "SeafoamIslandsB2F",
            include_bytes!("../maps/SeafoamIslandsB2F/map.blk"),
        ),
        (
            "SeafoamIslandsB3F",
            include_bytes!("../maps/SeafoamIslandsB3F/map.blk"),
        ),
        (
            "SeafoamIslandsB4F",
            include_bytes!("../maps/SeafoamIslandsB4F/map.blk"),
        ),
        ("SilphCo10F", include_bytes!("../maps/SilphCo10F/map.blk")),
        ("SilphCo11F", include_bytes!("../maps/SilphCo11F/map.blk")),
        ("SilphCo1F", include_bytes!("../maps/SilphCo1F/map.blk")),
        ("SilphCo2F", include_bytes!("../maps/SilphCo2F/map.blk")),
        ("SilphCo3F", include_bytes!("../maps/SilphCo3F/map.blk")),
        ("SilphCo4F", include_bytes!("../maps/SilphCo4F/map.blk")),
        ("SilphCo5F", include_bytes!("../maps/SilphCo5F/map.blk")),
        ("SilphCo6F", include_bytes!("../maps/SilphCo6F/map.blk")),
        ("SilphCo7F", include_bytes!("../maps/SilphCo7F/map.blk")),
        ("SilphCo8F", include_bytes!("../maps/SilphCo8F/map.blk")),
        ("SilphCo9F", include_bytes!("../maps/SilphCo9F/map.blk")),
        (
            "SilphCoElevator",
            include_bytes!("../maps/SilphCoElevator/map.blk"),
        ),
        ("TradeCenter", include_bytes!("../maps/TradeCenter/map.blk")),
        (
            "UndergroundPathNorthSouth",
            include_bytes!("../maps/UndergroundPathNorthSouth/map.blk"),
        ),
        (
            "UndergroundPathRoute5",
            include_bytes!("../maps/UndergroundPathRoute5/map.blk"),
        ),
        (
            "UndergroundPathRoute6",
            include_bytes!("../maps/UndergroundPathRoute6/map.blk"),
        ),
        (
            "UndergroundPathRoute7",
            include_bytes!("../maps/UndergroundPathRoute7/map.blk"),
        ),
        (
            "UndergroundPathRoute8",
            include_bytes!("../maps/UndergroundPathRoute8/map.blk"),
        ),
        (
            "UndergroundPathWestEast",
            include_bytes!("../maps/UndergroundPathWestEast/map.blk"),
        ),
        (
            "VermilionCity",
            include_bytes!("../maps/VermilionCity/map.blk"),
        ),
        (
            "VermilionDock",
            include_bytes!("../maps/VermilionDock/map.blk"),
        ),
        (
            "VermilionGym",
            include_bytes!("../maps/VermilionGym/map.blk"),
        ),
        (
            "VermilionMart",
            include_bytes!("../maps/VermilionMart/map.blk"),
        ),
        (
            "VermilionOldRodHouse",
            include_bytes!("../maps/VermilionOldRodHouse/map.blk"),
        ),
        (
            "VermilionPidgeyHouse",
            include_bytes!("../maps/VermilionPidgeyHouse/map.blk"),
        ),
        (
            "VermilionPokecenter",
            include_bytes!("../maps/VermilionPokecenter/map.blk"),
        ),
        (
            "VermilionTradeHouse",
            include_bytes!("../maps/VermilionTradeHouse/map.blk"),
        ),
        (
            "VictoryRoad1F",
            include_bytes!("../maps/VictoryRoad1F/map.blk"),
        ),
        (
            "VictoryRoad2F",
            include_bytes!("../maps/VictoryRoad2F/map.blk"),
        ),
        (
            "VictoryRoad3F",
            include_bytes!("../maps/VictoryRoad3F/map.blk"),
        ),
        (
            "ViridianCity",
            include_bytes!("../maps/ViridianCity/map.blk"),
        ),
        (
            "ViridianForest",
            include_bytes!("../maps/ViridianForest/map.blk"),
        ),
        (
            "ViridianForestNorthGate",
            include_bytes!("../maps/ViridianForestNorthGate/map.blk"),
        ),
        (
            "ViridianForestSouthGate",
            include_bytes!("../maps/ViridianForestSouthGate/map.blk"),
        ),
        ("ViridianGym", include_bytes!("../maps/ViridianGym/map.blk")),
        (
            "ViridianMart",
            include_bytes!("../maps/ViridianMart/map.blk"),
        ),
        (
            "ViridianNicknameHouse",
            include_bytes!("../maps/ViridianNicknameHouse/map.blk"),
        ),
        (
            "ViridianPokecenter",
            include_bytes!("../maps/ViridianPokecenter/map.blk"),
        ),
        (
            "ViridianSchoolHouse",
            include_bytes!("../maps/ViridianSchoolHouse/map.blk"),
        ),
        (
            "WardensHouse",
            include_bytes!("../maps/WardensHouse/map.blk"),
        ),
    ]
}

#[cfg(feature = "embedded-map-data")]
fn init_map_data() -> MapDataStore {
    let mut maps = HashMap::with_capacity(MAP_TABLE.len());
    for (name, static_map) in MAP_TABLE {
        maps.insert((*name).to_string(), MapJson::from(*static_map));
    }

    let mut blocks = HashMap::new();
    for (name, blk_data) in embedded_blk_sources() {
        blocks.insert(name.to_string(), blk_data.to_vec());
    }

    MapDataStore { maps, blocks }
}

// ── Filesystem mode ────────────────────────────────────────────────────────

#[cfg(not(feature = "embedded-map-data"))]
fn init_map_data() -> MapDataStore {
    let maps_dir = find_maps_directory();
    let mut maps = HashMap::new();
    let mut blocks = HashMap::new();

    if let Some(dir) = &maps_dir {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let map_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };

                let json_path = path.join("map.json");
                if json_path.exists() {
                    match std::fs::read_to_string(&json_path) {
                        Ok(json_str) => match serde_json::from_str::<MapJson>(&json_str) {
                            Ok(map_json) => {
                                maps.insert(map_name.clone(), map_json);
                            }
                            Err(e) => {
                                log::warn!("Failed to parse {}: {}", json_path.display(), e);
                            }
                        },
                        Err(e) => {
                            log::warn!("Failed to read {}: {}", json_path.display(), e);
                        }
                    }
                }

                let blk_path = path.join("map.blk");
                if blk_path.exists() {
                    match std::fs::read(&blk_path) {
                        Ok(data) => {
                            blocks.insert(map_name, data);
                        }
                        Err(e) => {
                            log::warn!("Failed to read {}: {}", blk_path.display(), e);
                        }
                    }
                }
            }
        }
    }

    log::info!(
        "MapDataLoader: loaded {} maps, {} block files from filesystem",
        maps.len(),
        blocks.len()
    );
    MapDataStore { maps, blocks }
}

#[cfg(not(feature = "embedded-map-data"))]
fn find_maps_directory() -> Option<std::path::PathBuf> {
    // 0. Explicit override: POKERED_MAPS_DIR points directly at the maps directory.
    //    Takes precedence so the binary can be launched from any working directory.
    if let Ok(dir) = std::env::var("POKERED_MAPS_DIR") {
        let p = std::path::PathBuf::from(&dir);
        if p.is_dir() {
            log::info!("MapDataLoader: using POKERED_MAPS_DIR {:?}", p);
            return Some(p);
        }
        log::warn!(
            "MapDataLoader: POKERED_MAPS_DIR {:?} is not a directory; falling back",
            dir
        );
    }

    // 1. Working-directory-relative candidates, for launches where the maps
    //    directory happens to sit near the cwd (e.g. `cargo run` from the
    //    workspace root, or a packaged build that ships `maps/` alongside).
    let candidates = [
        std::path::PathBuf::from("crates/pokered-data/maps"),
        std::path::PathBuf::from("crates/pokered-data/maps"),
        std::path::PathBuf::from("../pokered-data/maps"),
        std::path::PathBuf::from("pokered-data/maps"),
        std::path::PathBuf::from("maps"),
    ];
    for candidate in &candidates {
        if candidate.is_dir() {
            log::info!("MapDataLoader: using maps directory {:?}", candidate);
            return Some(candidate.clone());
        }
    }

    // 2. Compile-time crate manifest directory. This absolute path is baked in
    //    at build time, so it resolves regardless of the working directory and
    //    works when the binary is launched directly (e.g. `target/debug/pokered-app`).
    //    The runtime `CARGO_MANIFEST_DIR` env var is only set by `cargo run`, not
    //    for a directly-invoked binary, so we must use the `env!` macro here.
    let from_manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("maps");
    if from_manifest.is_dir() {
        log::info!("MapDataLoader: using maps directory {:?}", from_manifest);
        return Some(from_manifest);
    }

    log::warn!("MapDataLoader: could not find maps directory");
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_to_map_id_roundtrip() {
        let id_map = name_to_map_id();
        assert_eq!(id_map.get("PalletTown"), Some(&MapId::PalletTown));
        assert_eq!(id_map.get("Route1"), Some(&MapId::Route1));
        assert_eq!(id_map.get("OaksLab"), Some(&MapId::OaksLab));
        assert_eq!(id_map.get("NonExistent"), None);
    }

    #[test]
    fn test_resolve_map_id() {
        assert_eq!(resolve_map_id("PalletTown"), Some(MapId::PalletTown));
        assert_eq!(resolve_map_id("CeruleanCity"), Some(MapId::CeruleanCity));
        assert_eq!(resolve_map_id("Bogus"), None);
    }
}

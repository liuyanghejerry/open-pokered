//! Toggleable object definitions.
//!
//! Maps toggle_id strings (used in script_config.json) to bit indices
//! in GameData.toggleable_object_flags array.
//!
//! Based on constants/toggle_constants.asm from the original pokered disassembly.
//! The toggleable_object_flags array is 32 bytes (256 bits), where each bit
//! controls whether an object is hidden (OFF/$11) or shown (ON/$15).

/// Toggleable object bit index.
/// Corresponds to TOGGLE_* constants in toggle_constants.asm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleableObject(u8);

impl ToggleableObject {
    /// Get the bit index (0-255) for this toggleable object.
    pub fn bit_index(self) -> u16 {
        self.0 as u16
    }

    /// Get the byte offset in the toggleable_object_flags array.
    pub fn byte_offset(self) -> usize {
        (self.0 / 8) as usize
    }

    /// Get the bit mask within the byte.
    pub fn bit_mask(self) -> u8 {
        1 << (self.0 % 8)
    }

    /// All toggleable objects for OAKS_LAB map.
    pub const OAKS_LAB_RIVAL: ToggleableObject = ToggleableObject(0x2A);
    pub const STARTER_BALL_1: ToggleableObject = ToggleableObject(0x2B); // Charmander
    pub const STARTER_BALL_2: ToggleableObject = ToggleableObject(0x2C); // Squirtle
    pub const STARTER_BALL_3: ToggleableObject = ToggleableObject(0x2D); // Bulbasaur
    pub const OAKS_LAB_OAK_1: ToggleableObject = ToggleableObject(0x2E);
    pub const POKEDEX_1: ToggleableObject = ToggleableObject(0x2F);
    pub const POKEDEX_2: ToggleableObject = ToggleableObject(0x30);
    pub const OAKS_LAB_OAK_2: ToggleableObject = ToggleableObject(0x31);

    // PALLET_TOWN
    pub const PALLET_TOWN_OAK: ToggleableObject = ToggleableObject(0x00);

    // VIRIDIAN_CITY
    pub const LYING_OLD_MAN: ToggleableObject = ToggleableObject(0x01);
    pub const OLD_MAN: ToggleableObject = ToggleableObject(0x02);

    // PEWTER_CITY
    pub const MUSEUM_GUY: ToggleableObject = ToggleableObject(0x03);
    pub const GYM_GUY: ToggleableObject = ToggleableObject(0x04);

    // CERULEAN_CITY
    pub const CERULEAN_RIVAL: ToggleableObject = ToggleableObject(0x05);
    pub const CERULEAN_ROCKET: ToggleableObject = ToggleableObject(0x06);
    pub const CERULEAN_GUARD_1: ToggleableObject = ToggleableObject(0x07);
    pub const CERULEAN_CAVE_GUY: ToggleableObject = ToggleableObject(0x08);
    pub const CERULEAN_GUARD_2: ToggleableObject = ToggleableObject(0x09);

    // ROUTE_22
    pub const ROUTE_22_RIVAL_1: ToggleableObject = ToggleableObject(0x22);
    pub const ROUTE_22_RIVAL_2: ToggleableObject = ToggleableObject(0x23);

    // BLUES_HOUSE
    pub const DAISY_SITTING: ToggleableObject = ToggleableObject(0x27);
    pub const DAISY_WALKING: ToggleableObject = ToggleableObject(0x28);
    pub const TOWN_MAP: ToggleableObject = ToggleableObject(0x29);

    // BILLS_HOUSE
    pub const BILL_POKEMON: ToggleableObject = ToggleableObject(0x61);
    pub const BILL_1: ToggleableObject = ToggleableObject(0x62);
    pub const BILL_2: ToggleableObject = ToggleableObject(0x63);

    // SS_ANNE_2F
    pub const SS_ANNE_2F_RIVAL: ToggleableObject = ToggleableObject(0x71);

    // POKEMON_TOWER_2F
    pub const POKEMON_TOWER_2F_RIVAL: ToggleableObject = ToggleableObject(0x38);

    // FIGHTING_DOJO
    pub const HITMONLEE_POKE_BALL: ToggleableObject = ToggleableObject(0x4A);
    pub const HITMONCHAN_POKE_BALL: ToggleableObject = ToggleableObject(0x4B);

    // SILPH_CO
    pub const SILPH_CO_7F_RIVAL: ToggleableObject = ToggleableObject(0xA7);

    // CHAMPIONS_ROOM
    pub const CHAMPIONS_ROOM_OAK: ToggleableObject = ToggleableObject(0xD6);

    // MR_FUJIS_HOUSE
    pub const MR_FUJI: ToggleableObject = ToggleableObject(0x44);

    // ROUTE_12 SNORLAX
    pub const ROUTE_12_SNORLAX: ToggleableObject = ToggleableObject(0x1D);

    // ROUTE_16 SNORLAX
    pub const ROUTE_16_SNORLAX: ToggleableObject = ToggleableObject(0x21);

    // MUSEUM_1F
    pub const OLD_AMBER: ToggleableObject = ToggleableObject(0x34);

    // CELADON_MANSION_ROOF_HOUSE
    pub const EEVEE_GIFT: ToggleableObject = ToggleableObject(0x45);

    // GAME_CORNER
    pub const GAME_CORNER_ROCKET: ToggleableObject = ToggleableObject(0x46);

    // VIRIDIAN_GYM
    pub const VIRIDIAN_GYM_GIOVANNI: ToggleableObject = ToggleableObject(0x32);

    // POWER_PLANT - Zapdos and Voltorbs
    pub const ZAPDOS: ToggleableObject = ToggleableObject(0x55);

    // VICTORY_ROAD - Moltres
    pub const MOLTRES: ToggleableObject = ToggleableObject(0x5B);

    // SEAFOAM_ISLANDS - Articuno
    pub const ARTICUNO: ToggleableObject = ToggleableObject(0xE3);

    // CERULEAN_CAVE - Mewtwo
    pub const MEWTWO: ToggleableObject = ToggleableObject(0xD1);

    // MT_MOON fossils
    pub const DOME_FOSSIL: ToggleableObject = ToggleableObject(0x6D);
    pub const HELIX_FOSSIL: ToggleableObject = ToggleableObject(0x6E);

    // The remaining bits (SAFARI_ZONE items, SEAFOAM boulders, map objects
    // without a named constant here) are covered directly in
    // toggle_id_to_bit_index below, with the original TOGGLE_* name noted.
}

/// Size of the toggleable_object_flags array in bytes.
pub const TOGGLEABLE_OBJECT_FLAGS_SIZE: usize = 32;

/// Lookup toggle_id string to get ToggleableObject bit index.
/// Returns None if the toggle_id is not recognized.
///
/// This maps the script_config.json toggleId values to their bit indices.
pub fn toggle_id_to_bit_index(toggle_id: &str) -> Option<u16> {
    // Map script_config.json toggleId to ToggleableObject
    // Format in script_config.json: "OAKS_LAB_OBJ_1", "OAKS_LAB_OBJ_2", etc.
    // Format in toggle_constants.asm: TOGGLE_OAKS_LAB_RIVAL, TOGGLE_STARTER_BALL_1, etc.

    match toggle_id {
        // OAKS_LAB (script uses OAKS_LAB_OBJ_1 through OAKS_LAB_OBJ_8)
        "OAKS_LAB_OBJ_1" => Some(ToggleableObject::OAKS_LAB_RIVAL.bit_index()),
        "OAKS_LAB_OBJ_2" => Some(ToggleableObject::STARTER_BALL_1.bit_index()), // Charmander ball
        "OAKS_LAB_OBJ_3" => Some(ToggleableObject::STARTER_BALL_2.bit_index()), // Squirtle ball
        "OAKS_LAB_OBJ_4" => Some(ToggleableObject::STARTER_BALL_3.bit_index()), // Bulbasaur ball
        "OAKS_LAB_OBJ_5" => Some(ToggleableObject::OAKS_LAB_OAK_1.bit_index()),
        "OAKS_LAB_OBJ_6" => Some(ToggleableObject::POKEDEX_1.bit_index()),
        "OAKS_LAB_OBJ_7" => Some(ToggleableObject::POKEDEX_2.bit_index()),
        "OAKS_LAB_OBJ_8" => Some(ToggleableObject::OAKS_LAB_OAK_2.bit_index()),

        // PALLET_TOWN
        // script_config.json uses "PALLET_TOWN_OBJ_1" for Oak; without this
        // alias the hideObject cleanup never persists the hidden bit to SRAM.
        "PALLET_TOWN_OBJ_1" | "PALLETTOWN_OAK" | "PALLET_TOWN_OAK" => {
            Some(ToggleableObject::PALLET_TOWN_OAK.bit_index())
        }

        // VIRIDIAN_CITY
        "VIRIDIANCITY_OLD_MAN_SLEEPY" | "LYING_OLD_MAN" => {
            Some(ToggleableObject::LYING_OLD_MAN.bit_index())
        }
        "VIRIDIANCITY_OLD_MAN" | "OLD_MAN" => Some(ToggleableObject::OLD_MAN.bit_index()),

        // PEWTER_CITY
        "PEWTERCITY_SUPER_NERD1" | "MUSEUM_GUY" | "PEWTER_CITY_OBJ_3" => {
            Some(ToggleableObject::MUSEUM_GUY.bit_index())
        }
        "PEWTERCITY_YOUNGSTER" | "GYM_GUY" | "PEWTER_CITY_OBJ_5" => {
            Some(ToggleableObject::GYM_GUY.bit_index())
        }

        // CERULEAN_CITY
        "CERULEANCITY_RIVAL" | "CERULEAN_RIVAL" => {
            Some(ToggleableObject::CERULEAN_RIVAL.bit_index())
        }
        "CERULEANCITY_ROCKET" | "CERULEAN_ROCKET" => {
            Some(ToggleableObject::CERULEAN_ROCKET.bit_index())
        }
        "CERULEANCITY_GUARD1" | "CERULEAN_GUARD_1" => {
            Some(ToggleableObject::CERULEAN_GUARD_1.bit_index())
        }
        "CERULEANCITY_SUPER_NERD3" | "CERULEAN_CAVE_GUY" => {
            Some(ToggleableObject::CERULEAN_CAVE_GUY.bit_index())
        }
        "CERULEANCITY_GUARD2" | "CERULEAN_GUARD_2" => {
            Some(ToggleableObject::CERULEAN_GUARD_2.bit_index())
        }

        // SAFFRON_CITY — rocket blockade + citizens
        // (TOGGLE_SAFFRON_CITY_1..F, original object order in
        // data/maps/toggleable_objects.asm)
        "SAFFRON_CITY_OBJ_1" => Some(0x0A),  // TOGGLE_SAFFRON_CITY_1 (ROCKET1)
        "SAFFRON_CITY_OBJ_2" => Some(0x0B),  // TOGGLE_SAFFRON_CITY_2 (ROCKET2)
        "SAFFRON_CITY_OBJ_3" => Some(0x0C),  // TOGGLE_SAFFRON_CITY_3 (ROCKET3)
        "SAFFRON_CITY_OBJ_4" => Some(0x0D),  // TOGGLE_SAFFRON_CITY_4 (ROCKET4)
        "SAFFRON_CITY_OBJ_5" => Some(0x0E),  // TOGGLE_SAFFRON_CITY_5 (ROCKET5)
        "SAFFRON_CITY_OBJ_6" => Some(0x0F),  // TOGGLE_SAFFRON_CITY_6 (ROCKET6)
        "SAFFRON_CITY_OBJ_7" => Some(0x10),  // TOGGLE_SAFFRON_CITY_7 (ROCKET7)
        "SAFFRON_CITY_OBJ_8" => Some(0x11),  // TOGGLE_SAFFRON_CITY_8 (SCIENTIST)
        "SAFFRON_CITY_OBJ_9" => Some(0x12),  // TOGGLE_SAFFRON_CITY_9 (SILPH_WORKER_M)
        "SAFFRON_CITY_OBJ_10" => Some(0x13), // TOGGLE_SAFFRON_CITY_A (SILPH_WORKER_F)
        "SAFFRON_CITY_OBJ_11" => Some(0x14), // TOGGLE_SAFFRON_CITY_B (GENTLEMAN)
        "SAFFRON_CITY_OBJ_12" => Some(0x15), // TOGGLE_SAFFRON_CITY_C (PIDGEOT)
        "SAFFRON_CITY_OBJ_13" => Some(0x16), // TOGGLE_SAFFRON_CITY_D (ROCKER)
        "SAFFRON_CITY_OBJ_14" => Some(0x17), // TOGGLE_SAFFRON_CITY_E (ROCKET8)
        "SAFFRON_CITY_OBJ_15" => Some(0x18), // TOGGLE_SAFFRON_CITY_F (ROCKET9)

        // ROUTE_2
        "ROUTE_2_OBJ_1" => Some(0x19), // TOGGLE_ROUTE_2_ITEM_1 (MOON_STONE)
        "ROUTE_2_OBJ_2" => Some(0x1A), // TOGGLE_ROUTE_2_ITEM_2 (HP_UP)

        // ROUTE_4
        "ROUTE_4_OBJ_3" => Some(0x1B), // TOGGLE_ROUTE_4_ITEM (TM_WHIRLWIND)

        // ROUTE_9
        "ROUTE_9_OBJ_10" => Some(0x1C), // TOGGLE_ROUTE_9_ITEM (TM_TELEPORT)

        // ROUTE_12
        "ROUTE_12_OBJ_1" | "ROUTE12_SNORLAX" => Some(ToggleableObject::ROUTE_12_SNORLAX.bit_index()),
        "ROUTE_12_OBJ_9" => Some(0x1E),  // TOGGLE_ROUTE_12_ITEM_1 (TM_PAY_DAY)
        "ROUTE_12_OBJ_10" => Some(0x1F), // TOGGLE_ROUTE_12_ITEM_2 (IRON)

        // ROUTE_15
        "ROUTE_15_OBJ_11" => Some(0x20), // TOGGLE_ROUTE_15_ITEM (TM_RAGE)

        // ROUTE_16
        "ROUTE_16_OBJ_7" | "ROUTE16_SNORLAX" => Some(ToggleableObject::ROUTE_16_SNORLAX.bit_index()),

        // ROUTE_22
        "ROUTE22_RIVAL1" | "ROUTE_22_RIVAL_1" => {
            Some(ToggleableObject::ROUTE_22_RIVAL_1.bit_index())
        }
        "ROUTE22_RIVAL2" | "ROUTE_22_RIVAL_2" => {
            Some(ToggleableObject::ROUTE_22_RIVAL_2.bit_index())
        }

        // ROUTE_24
        "ROUTE_24_OBJ_1" => Some(0x24), // TOGGLE_NUGGET_BRIDGE_GUY (COOLTRAINER_M1)
        "ROUTE_24_OBJ_8" => Some(0x25), // TOGGLE_ROUTE_24_ITEM (TM_THUNDER_WAVE)

        // ROUTE_25
        "ROUTE_25_OBJ_10" => Some(0x26), // TOGGLE_ROUTE_25_ITEM (TM_SEISMIC_TOSS)

        // BLUES_HOUSE
        "BLUESHOUSE_DAISY1" | "DAISY_SITTING" => Some(ToggleableObject::DAISY_SITTING.bit_index()),
        "BLUESHOUSE_DAISY2" | "DAISY_WALKING" => Some(ToggleableObject::DAISY_WALKING.bit_index()),
        "BLUESHOUSE_TOWN_MAP" | "TOWN_MAP" | "BLUES_HOUSE_OBJ_3" => {
            Some(ToggleableObject::TOWN_MAP.bit_index())
        }

        // BILLS_HOUSE
        "BILLSHOUSE_BILL_POKEMON" | "BILL_POKEMON" | "BILLS_HOUSE_OBJ_1" => {
            Some(ToggleableObject::BILL_POKEMON.bit_index())
        }
        "BILLSHOUSE_BILL1" | "BILL_1" | "BILLS_HOUSE_OBJ_2" => {
            Some(ToggleableObject::BILL_1.bit_index())
        }
        "BILLSHOUSE_BILL2" | "BILL_2" | "BILLS_HOUSE_OBJ_3" => {
            Some(ToggleableObject::BILL_2.bit_index())
        }

        // SS_ANNE_2F
        "SSANNE2F_RIVAL" | "SS_ANNE_2F_RIVAL" => {
            Some(ToggleableObject::SS_ANNE_2F_RIVAL.bit_index())
        }

        // SS_ANNE_1F_ROOMS
        "SS_ANNE_1F_ROOMS_OBJ_10" => Some(0x72), // TOGGLE_SS_ANNE_1F_ROOMS_ITEM (TM_BODY_SLAM)

        // SS_ANNE_2F_ROOMS
        "SS_ANNE_2F_ROOMS_OBJ_6" => Some(0x73), // TOGGLE_SS_ANNE_2F_ROOMS_ITEM_1 (MAX_ETHER)
        "SS_ANNE_2F_ROOMS_OBJ_9" => Some(0x74), // TOGGLE_SS_ANNE_2F_ROOMS_ITEM_2 (RARE_CANDY)

        // SS_ANNE_B1F_ROOMS
        "SS_ANNE_B1F_ROOMS_OBJ_9" => Some(0x75),  // TOGGLE_SS_ANNE_B1F_ROOMS_ITEM_1 (ETHER)
        "SS_ANNE_B1F_ROOMS_OBJ_10" => Some(0x76), // TOGGLE_SS_ANNE_B1F_ROOMS_ITEM_2 (TM_REST)
        "SS_ANNE_B1F_ROOMS_OBJ_11" => Some(0x77), // TOGGLE_SS_ANNE_B1F_ROOMS_ITEM_3 (MAX_POTION)

        // MUSEUM_1F
        "MUSEUM1F_OLD_AMBER" | "OLD_AMBER" | "MUSEUM_1F_OBJ_5" => {
            Some(ToggleableObject::OLD_AMBER.bit_index())
        }

        // CERULEAN_CAVE_1F
        "CERULEAN_CAVE_1F_OBJ_1" => Some(0x35), // TOGGLE_CERULEAN_CAVE_1F_ITEM_1 (FULL_RESTORE)
        "CERULEAN_CAVE_1F_OBJ_2" => Some(0x36), // TOGGLE_CERULEAN_CAVE_1F_ITEM_2 (MAX_ELIXER)
        "CERULEAN_CAVE_1F_OBJ_3" => Some(0x37), // TOGGLE_CERULEAN_CAVE_1F_ITEM_3 (NUGGET)

        // POKEMON_TOWER_2F
        "POKEMONTOWER2F_RIVAL" | "POKEMON_TOWER_2F_RIVAL" => {
            Some(ToggleableObject::POKEMON_TOWER_2F_RIVAL.bit_index())
        }

        // POKEMON_TOWER_3F
        "POKEMON_TOWER_3F_OBJ_4" => Some(0x39), // TOGGLE_POKEMON_TOWER_3F_ITEM (ESCAPE_ROPE)

        // POKEMON_TOWER_4F
        "POKEMON_TOWER_4F_OBJ_4" => Some(0x3A), // TOGGLE_POKEMON_TOWER_4F_ITEM_1 (ELIXER)
        "POKEMON_TOWER_4F_OBJ_5" => Some(0x3B), // TOGGLE_POKEMON_TOWER_4F_ITEM_2 (AWAKENING)
        "POKEMON_TOWER_4F_OBJ_6" => Some(0x3C), // TOGGLE_POKEMON_TOWER_4F_ITEM_3 (HP_UP)

        // POKEMON_TOWER_5F
        "POKEMON_TOWER_5F_OBJ_6" => Some(0x3D), // TOGGLE_POKEMON_TOWER_5F_ITEM (NUGGET)

        // POKEMON_TOWER_6F
        "POKEMON_TOWER_6F_OBJ_4" => Some(0x3E), // TOGGLE_POKEMON_TOWER_6F_ITEM_1 (RARE_CANDY)
        "POKEMON_TOWER_6F_OBJ_5" => Some(0x3F), // TOGGLE_POKEMON_TOWER_6F_ITEM_2 (X_ACCURACY)

        // FIGHTING_DOJO
        "FIGHTINGDOJO_HITMONLEE_POKE_BALL" => {
            Some(ToggleableObject::HITMONLEE_POKE_BALL.bit_index())
        }
        "FIGHTINGDOJO_HITMONCHAN_POKE_BALL" => {
            Some(ToggleableObject::HITMONCHAN_POKE_BALL.bit_index())
        }

        // CELADON_MANSION
        "CELADONMANSION_ROOF_HOUSE_EEVEE_POKEBALL" | "EEVEE_GIFT"
        | "CELADON_MANSION_ROOF_HOUSE_OBJ_2" => {
            Some(ToggleableObject::EEVEE_GIFT.bit_index())
        }

        // GAME_CORNER
        "GAMECORNER_ROCKET" | "GAME_CORNER_ROCKET" | "GAME_CORNER_OBJ_11" => {
            Some(ToggleableObject::GAME_CORNER_ROCKET.bit_index())
        }

        // WARDENS_HOUSE
        "WARDENS_HOUSE_OBJ_2" => Some(0x47), // TOGGLE_WARDENS_HOUSE_ITEM (RARE_CANDY)

        // POKEMON_MANSION_1F
        "POKEMON_MANSION_1F_OBJ_2" => Some(0x48), // TOGGLE_POKEMON_MANSION_1F_ITEM_1 (ESCAPE_ROPE)
        "POKEMON_MANSION_1F_OBJ_3" => Some(0x49), // TOGGLE_POKEMON_MANSION_1F_ITEM_2 (CARBOS)

        // SILPH_CO_1F
        "SILPH_CO_1F_OBJ_1" => Some(0x4C), // TOGGLE_SILPH_CO_1F_RECEPTIONIST

        // VIRIDIAN_GYM
        "VIRIDIANGYM_GIOVANNI" | "VIRIDIAN_GYM_GIOVANNI" => {
            Some(ToggleableObject::VIRIDIAN_GYM_GIOVANNI.bit_index())
        }

        // POWER_PLANT
        "POWERPLANT_ZAPDOS" | "ZAPDOS" => Some(ToggleableObject::ZAPDOS.bit_index()),

        // VICTORY_ROAD_2F
        "VICTORYROAD2F_MOLTRES" | "MOLTRES" => Some(ToggleableObject::MOLTRES.bit_index()),
        "VICTORY_ROAD_2F_OBJ_7" => Some(0x5C),  // TOGGLE_VICTORY_ROAD_2F_ITEM_1 (TM_SUBMISSION)
        "VICTORY_ROAD_2F_OBJ_8" => Some(0x5D),  // TOGGLE_VICTORY_ROAD_2F_ITEM_2 (FULL_HEAL)
        "VICTORY_ROAD_2F_OBJ_9" => Some(0x5E),  // TOGGLE_VICTORY_ROAD_2F_ITEM_3 (TM_MEGA_KICK)
        "VICTORY_ROAD_2F_OBJ_10" => Some(0x5F), // TOGGLE_VICTORY_ROAD_2F_ITEM_4 (GUARD_SPEC)

        // VIRIDIAN_FOREST
        "VIRIDIAN_FOREST_OBJ_5" => Some(0x64), // TOGGLE_VIRIDIAN_FOREST_ITEM_1 (ANTIDOTE)
        "VIRIDIAN_FOREST_OBJ_6" => Some(0x65), // TOGGLE_VIRIDIAN_FOREST_ITEM_2 (POTION)
        "VIRIDIAN_FOREST_OBJ_7" => Some(0x66), // TOGGLE_VIRIDIAN_FOREST_ITEM_3 (POKE_BALL)

        // MT_MOON_1F
        "MT_MOON_1F_OBJ_8" => Some(0x67),  // TOGGLE_MT_MOON_1F_ITEM_1 (POTION1)
        "MT_MOON_1F_OBJ_9" => Some(0x68),  // TOGGLE_MT_MOON_1F_ITEM_2 (MOON_STONE)
        "MT_MOON_1F_OBJ_10" => Some(0x69), // TOGGLE_MT_MOON_1F_ITEM_3 (RARE_CANDY)
        "MT_MOON_1F_OBJ_11" => Some(0x6A), // TOGGLE_MT_MOON_1F_ITEM_4 (ESCAPE_ROPE)
        "MT_MOON_1F_OBJ_12" => Some(0x6B), // TOGGLE_MT_MOON_1F_ITEM_5 (POTION2)
        "MT_MOON_1F_OBJ_13" => Some(0x6C), // TOGGLE_MT_MOON_1F_ITEM_6 (TM_WATER_GUN)

        // MT_MOON_B2F
        "MTMOONB2F_DOME_FOSSIL" | "DOME_FOSSIL" => Some(ToggleableObject::DOME_FOSSIL.bit_index()),
        "MTMOONB2F_HELIX_FOSSIL" | "HELIX_FOSSIL" => {
            Some(ToggleableObject::HELIX_FOSSIL.bit_index())
        }
        "MT_MOON_B2F_OBJ_8" => Some(0x6F), // TOGGLE_MT_MOON_B2F_ITEM_1 (HP_UP)
        "MT_MOON_B2F_OBJ_9" => Some(0x70), // TOGGLE_MT_MOON_B2F_ITEM_2 (TM_MEGA_PUNCH)

        // VICTORY_ROAD_3F
        "VICTORY_ROAD_3F_OBJ_5" => Some(0x78), // TOGGLE_VICTORY_ROAD_3F_ITEM_1 (MAX_REVIVE)
        "VICTORY_ROAD_3F_OBJ_6" => Some(0x79), // TOGGLE_VICTORY_ROAD_3F_ITEM_2 (TM_EXPLOSION)

        // ROCKET_HIDEOUT_B1F
        "ROCKET_HIDEOUT_B1F_OBJ_6" => Some(0x7B), // TOGGLE_ROCKET_HIDEOUT_B1F_ITEM_1 (ESCAPE_ROPE)
        "ROCKET_HIDEOUT_B1F_OBJ_7" => Some(0x7C), // TOGGLE_ROCKET_HIDEOUT_B1F_ITEM_2 (HYPER_POTION)

        // ROCKET_HIDEOUT_B2F
        "ROCKET_HIDEOUT_B2F_OBJ_2" => Some(0x7D), // TOGGLE_ROCKET_HIDEOUT_B2F_ITEM_1 (MOON_STONE)
        "ROCKET_HIDEOUT_B2F_OBJ_3" => Some(0x7E), // TOGGLE_ROCKET_HIDEOUT_B2F_ITEM_2 (NUGGET)
        "ROCKET_HIDEOUT_B2F_OBJ_4" => Some(0x7F), // TOGGLE_ROCKET_HIDEOUT_B2F_ITEM_3 (TM_HORN_DRILL)
        "ROCKET_HIDEOUT_B2F_OBJ_5" => Some(0x80), // TOGGLE_ROCKET_HIDEOUT_B2F_ITEM_4 (SUPER_POTION)

        // ROCKET_HIDEOUT_B3F
        "ROCKET_HIDEOUT_B3F_OBJ_3" => Some(0x81), // TOGGLE_ROCKET_HIDEOUT_B3F_ITEM_1 (TM_DOUBLE_EDGE)
        "ROCKET_HIDEOUT_B3F_OBJ_4" => Some(0x82), // TOGGLE_ROCKET_HIDEOUT_B3F_ITEM_2 (RARE_CANDY)

        // ROCKET_HIDEOUT_B4F
        "ROCKET_HIDEOUT_B4F_OBJ_1" => Some(0x83), // TOGGLE_ROCKET_HIDEOUT_B4F_GIOVANNI
        "ROCKET_HIDEOUT_B4F_OBJ_5" => Some(0x84), // TOGGLE_ROCKET_HIDEOUT_B4F_ITEM_1 (HP_UP)
        "ROCKET_HIDEOUT_B4F_OBJ_6" => Some(0x85), // TOGGLE_ROCKET_HIDEOUT_B4F_ITEM_2 (TM_RAZOR_WIND)
        "ROCKET_HIDEOUT_B4F_OBJ_7" => Some(0x86), // TOGGLE_ROCKET_HIDEOUT_B4F_ITEM_3 (IRON)
        "ROCKET_HIDEOUT_B4F_OBJ_8" => Some(0x87), // TOGGLE_ROCKET_HIDEOUT_B4F_ITEM_4 (SILPH_SCOPE)
        "ROCKET_HIDEOUT_B4F_OBJ_9" => Some(0x88), // TOGGLE_ROCKET_HIDEOUT_B4F_ITEM_5 (LIFT_KEY)

        // SILPH_CO_3F
        "SILPH_CO_3F_OBJ_4" => Some(0x90), // TOGGLE_SILPH_CO_3F_ITEM (HYPER_POTION)

        // SILPH_CO_4F
        "SILPH_CO_4F_OBJ_5" => Some(0x94), // TOGGLE_SILPH_CO_4F_ITEM_1 (FULL_HEAL)
        "SILPH_CO_4F_OBJ_6" => Some(0x95), // TOGGLE_SILPH_CO_4F_ITEM_2 (MAX_REVIVE)
        "SILPH_CO_4F_OBJ_7" => Some(0x96), // TOGGLE_SILPH_CO_4F_ITEM_3 (ESCAPE_ROPE)

        // SILPH_CO_5F
        "SILPH_CO_5F_OBJ_6" => Some(0x9B), // TOGGLE_SILPH_CO_5F_ITEM_1 (TM_TAKE_DOWN)
        "SILPH_CO_5F_OBJ_7" => Some(0x9C), // TOGGLE_SILPH_CO_5F_ITEM_2 (PROTEIN)
        "SILPH_CO_5F_OBJ_8" => Some(0x9D), // TOGGLE_SILPH_CO_5F_ITEM_3 (CARD_KEY)

        // SILPH_CO_6F
        "SILPH_CO_6F_OBJ_9" => Some(0xA1),  // TOGGLE_SILPH_CO_6F_ITEM_1 (HP_UP)
        "SILPH_CO_6F_OBJ_10" => Some(0xA2), // TOGGLE_SILPH_CO_6F_ITEM_2 (X_ACCURACY)

        // SILPH_CO_7F
        "SILPH_CO_7F_OBJ_5" => Some(0xA3), // TOGGLE_SILPH_CO_7F_1 (ROCKET1)
        "SILPH_CO_7F_OBJ_6" => Some(0xA4), // TOGGLE_SILPH_CO_7F_2 (SCIENTIST)
        "SILPH_CO_7F_OBJ_7" => Some(0xA5), // TOGGLE_SILPH_CO_7F_3 (ROCKET2)
        "SILPH_CO_7F_OBJ_8" => Some(0xA6), // TOGGLE_SILPH_CO_7F_4 (ROCKET3)
        "SILPHCO7F_RIVAL" | "SILPH_CO_7F_RIVAL" => {
            Some(ToggleableObject::SILPH_CO_7F_RIVAL.bit_index())
        }
        "SILPH_CO_7F_OBJ_10" => Some(0xA8), // TOGGLE_SILPH_CO_7F_ITEM_1 (CALCIUM)
        "SILPH_CO_7F_OBJ_11" => Some(0xA9), // TOGGLE_SILPH_CO_7F_ITEM_2 (TM_SWORDS_DANCE)

        // SILPH_CO_9F
        "SILPH_CO_9F_OBJ_2" => Some(0xAE), // TOGGLE_SILPH_CO_9F_1 (ROCKET1)
        "SILPH_CO_9F_OBJ_3" => Some(0xAF), // TOGGLE_SILPH_CO_9F_2 (SCIENTIST)
        "SILPH_CO_9F_OBJ_4" => Some(0xB0), // TOGGLE_SILPH_CO_9F_3 (ROCKET2)

        // SILPH_CO_10F
        "SILPH_CO_10F_OBJ_1" => Some(0xB1), // TOGGLE_SILPH_CO_10F_1 (ROCKET)
        "SILPH_CO_10F_OBJ_2" => Some(0xB2), // TOGGLE_SILPH_CO_10F_2 (SCIENTIST)
        "SILPH_CO_10F_OBJ_4" => Some(0xB4), // TOGGLE_SILPH_CO_10F_ITEM_1 (TM_EARTHQUAKE)
        "SILPH_CO_10F_OBJ_5" => Some(0xB5), // TOGGLE_SILPH_CO_10F_ITEM_2 (RARE_CANDY)
        "SILPH_CO_10F_OBJ_6" => Some(0xB6), // TOGGLE_SILPH_CO_10F_ITEM_3 (CARBOS)

        // SILPH_CO_11F
        "SILPH_CO_11F_OBJ_3" => Some(0xB7), // TOGGLE_SILPH_CO_11F_1 (GIOVANNI)
        "SILPH_CO_11F_OBJ_4" => Some(0xB8), // TOGGLE_SILPH_CO_11F_2 (ROCKET1)
        "SILPH_CO_11F_OBJ_5" => Some(0xB9), // TOGGLE_SILPH_CO_11F_3 (ROCKET2)

        // UNUSED_MAP_F4
        "UNUSED_MAP_F4_OBJ_2" => Some(0xBA), // TOGGLE_UNUSED_MAP_F4_1

        // POKEMON_MANSION_2F
        "POKEMON_MANSION_2F_OBJ_2" => Some(0xBB), // TOGGLE_POKEMON_MANSION_2F_ITEM (CALCIUM)

        // POKEMON_MANSION_3F
        "POKEMON_MANSION_3F_OBJ_3" => Some(0xBC), // TOGGLE_POKEMON_MANSION_3F_ITEM_1 (MAX_POTION)
        "POKEMON_MANSION_3F_OBJ_4" => Some(0xBD), // TOGGLE_POKEMON_MANSION_3F_ITEM_2 (IRON)

        // POKEMON_MANSION_B1F
        "POKEMON_MANSION_B1F_OBJ_3" => Some(0x0BE), // TOGGLE_POKEMON_MANSION_B1F_ITEM_1 (RARE_CANDY)
        "POKEMON_MANSION_B1F_OBJ_4" => Some(0x0BF), // TOGGLE_POKEMON_MANSION_B1F_ITEM_2 (FULL_RESTORE)
        "POKEMON_MANSION_B1F_OBJ_5" => Some(0x0C0), // TOGGLE_POKEMON_MANSION_B1F_ITEM_3 (TM_BLIZZARD)
        "POKEMON_MANSION_B1F_OBJ_6" => Some(0x0C1), // TOGGLE_POKEMON_MANSION_B1F_ITEM_4 (TM_SOLARBEAM)
        "POKEMON_MANSION_B1F_OBJ_8" => Some(0x0C2), // TOGGLE_POKEMON_MANSION_B1F_ITEM_5 (SECRET_KEY)

        // SAFARI_ZONE_EAST
        "SAFARI_ZONE_EAST_OBJ_1" => Some(0x0C3), // TOGGLE_SAFARI_ZONE_EAST_ITEM_1 (FULL_RESTORE)
        "SAFARI_ZONE_EAST_OBJ_2" => Some(0x0C4), // TOGGLE_SAFARI_ZONE_EAST_ITEM_2 (MAX_RESTORE)
        "SAFARI_ZONE_EAST_OBJ_3" => Some(0x0C5), // TOGGLE_SAFARI_ZONE_EAST_ITEM_3 (CARBOS)
        "SAFARI_ZONE_EAST_OBJ_4" => Some(0x0C6), // TOGGLE_SAFARI_ZONE_EAST_ITEM_4 (TM_EGG_BOMB)

        // SAFARI_ZONE_NORTH
        "SAFARI_ZONE_NORTH_OBJ_1" => Some(0x0C7), // TOGGLE_SAFARI_ZONE_NORTH_ITEM_1 (PROTEIN)
        "SAFARI_ZONE_NORTH_OBJ_2" => Some(0x0C8), // TOGGLE_SAFARI_ZONE_NORTH_ITEM_2 (TM_SKULL_BASH)

        // SAFARI_ZONE_WEST
        "SAFARI_ZONE_WEST_OBJ_1" => Some(0x0C9), // TOGGLE_SAFARI_ZONE_WEST_ITEM_1 (MAX_POTION)
        "SAFARI_ZONE_WEST_OBJ_2" => Some(0x0CA), // TOGGLE_SAFARI_ZONE_WEST_ITEM_2 (TM_DOUBLE_TEAM)
        "SAFARI_ZONE_WEST_OBJ_3" => Some(0x0CB), // TOGGLE_SAFARI_ZONE_WEST_ITEM_3 (MAX_REVIVE)
        "SAFARI_ZONE_WEST_OBJ_4" => Some(0x0CC), // TOGGLE_SAFARI_ZONE_WEST_ITEM_4 (GOLD_TEETH)

        // SAFARI_ZONE_CENTER
        "SAFARI_ZONE_CENTER_OBJ_1" => Some(0x0CD), // TOGGLE_SAFARI_ZONE_CENTER_ITEM (NUGGET)

        // CERULEAN_CAVE_2F
        "CERULEAN_CAVE_2F_OBJ_1" => Some(0x0CE), // TOGGLE_CERULEAN_CAVE_2F_ITEM_1 (PP_UP)
        "CERULEAN_CAVE_2F_OBJ_2" => Some(0x0CF), // TOGGLE_CERULEAN_CAVE_2F_ITEM_2 (ULTRA_BALL)
        "CERULEAN_CAVE_2F_OBJ_3" => Some(0x0D0), // TOGGLE_CERULEAN_CAVE_2F_ITEM_3 (FULL_RESTORE)

        // CERULEAN_CAVE_B1F
        "CERULEANCAVEB1F_MEWTWO" | "MEWTWO" => Some(ToggleableObject::MEWTWO.bit_index()),
        "CERULEAN_CAVE_B1F_OBJ_2" => Some(0x0D2), // TOGGLE_CERULEAN_CAVE_B1F_ITEM_1 (ULTRA_BALL)
        "CERULEAN_CAVE_B1F_OBJ_3" => Some(0x0D3), // TOGGLE_CERULEAN_CAVE_B1F_ITEM_2 (MAX_REVIVE)

        // VICTORY_ROAD_1F
        "VICTORY_ROAD_1F_OBJ_3" => Some(0x0D4), // TOGGLE_VICTORY_ROAD_1F_ITEM_1 (TM_SKY_ATTACK)
        "VICTORY_ROAD_1F_OBJ_4" => Some(0x0D5), // TOGGLE_VICTORY_ROAD_1F_ITEM_2 (RARE_CANDY)

        // CHAMPIONS_ROOM
        "CHAMPIONSROOM_OAK" | "CHAMPIONS_ROOM_OAK" | "CHAMPIONS_ROOM_OBJ_2" => {
            Some(ToggleableObject::CHAMPIONS_ROOM_OAK.bit_index())
        }

        // MR_FUJIS_HOUSE
        "MRFUJISHOUSE_MR_FUJI" | "MR_FUJI" => Some(ToggleableObject::MR_FUJI.bit_index()),

        // SEAFOAM_ISLANDS_1F
        "SEAFOAM_ISLANDS_1F_OBJ_1" => Some(0x0D7), // TOGGLE_SEAFOAM_ISLANDS_1F_BOULDER_1
        "SEAFOAM_ISLANDS_1F_OBJ_2" => Some(0x0D8), // TOGGLE_SEAFOAM_ISLANDS_1F_BOULDER_2

        // SEAFOAM_ISLANDS_B1F
        "SEAFOAM_ISLANDS_B1F_OBJ_1" => Some(0x0D9), // TOGGLE_SEAFOAM_ISLANDS_B1F_BOULDER_1
        "SEAFOAM_ISLANDS_B1F_OBJ_2" => Some(0x0DA), // TOGGLE_SEAFOAM_ISLANDS_B1F_BOULDER_2

        // SEAFOAM_ISLANDS_B2F
        "SEAFOAM_ISLANDS_B2F_OBJ_1" => Some(0x0DB), // TOGGLE_SEAFOAM_ISLANDS_B2F_BOULDER_1
        "SEAFOAM_ISLANDS_B2F_OBJ_2" => Some(0x0DC), // TOGGLE_SEAFOAM_ISLANDS_B2F_BOULDER_2

        // SEAFOAM_ISLANDS_B3F
        "SEAFOAM_ISLANDS_B3F_OBJ_1" => Some(0x0DD), // TOGGLE_SEAFOAM_ISLANDS_B3F_BOULDER_2
        "SEAFOAM_ISLANDS_B3F_OBJ_2" => Some(0x0DE), // TOGGLE_SEAFOAM_ISLANDS_B3F_BOULDER_3

        // SEAFOAM_ISLANDS_B4F
        "SEAFOAM_ISLANDS_B4F_OBJ_1" => Some(0x0E1), // TOGGLE_SEAFOAM_ISLANDS_B4F_BOULDER_1
        "SEAFOAM_ISLANDS_B4F_OBJ_2" => Some(0x0E2), // TOGGLE_SEAFOAM_ISLANDS_B4F_BOULDER_2
        "SEAFOAMISLANDSB4F_ARTICUNO" | "ARTICUNO" => Some(ToggleableObject::ARTICUNO.bit_index()),

        _ => None,
    }
}

/// Check if an object is hidden based on toggleable_object_flags.
/// Returns true if the object should be hidden (bit is set to OFF state).
///
/// In the original game, OFF state ($11) means the object is hidden.
/// The bit being SET means the object is hidden.
pub fn is_object_hidden(flags: &[u8; TOGGLEABLE_OBJECT_FLAGS_SIZE], bit_index: u16) -> bool {
    let byte_offset = (bit_index / 8) as usize;
    let bit_in_byte = bit_index % 8;
    if byte_offset < flags.len() {
        (flags[byte_offset] & (1 << bit_in_byte)) != 0
    } else {
        false
    }
}

/// Set an object as hidden in toggleable_object_flags.
pub fn set_object_hidden(flags: &mut [u8; TOGGLEABLE_OBJECT_FLAGS_SIZE], bit_index: u16) {
    let byte_offset = (bit_index / 8) as usize;
    let bit_in_byte = bit_index % 8;
    if byte_offset < flags.len() {
        flags[byte_offset] |= 1 << bit_in_byte;
    }
}

/// Set an object as shown (clear hidden flag) in toggleable_object_flags.
pub fn set_object_shown(flags: &mut [u8; TOGGLEABLE_OBJECT_FLAGS_SIZE], bit_index: u16) {
    let byte_offset = (bit_index / 8) as usize;
    let bit_in_byte = bit_index % 8;
    if byte_offset < flags.len() {
        flags[byte_offset] &= !(1 << bit_in_byte);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starter_ball_indices() {
        assert_eq!(ToggleableObject::STARTER_BALL_1.bit_index(), 0x2B);
        assert_eq!(ToggleableObject::STARTER_BALL_2.bit_index(), 0x2C);
        assert_eq!(ToggleableObject::STARTER_BALL_3.bit_index(), 0x2D);
    }

    #[test]
    fn test_toggle_id_mapping() {
        assert_eq!(toggle_id_to_bit_index("OAKS_LAB_OBJ_2"), Some(0x2B));
        assert_eq!(toggle_id_to_bit_index("OAKS_LAB_OBJ_3"), Some(0x2C));
        assert_eq!(toggle_id_to_bit_index("OAKS_LAB_OBJ_4"), Some(0x2D));
        assert_eq!(toggle_id_to_bit_index("OAKS_LAB_OBJ_5"), Some(0x2E));
    }

    #[test]
    fn test_extended_map_object_mappings() {
        // Spot checks for the script_config.json toggleIds added alongside
        // the original TOGGLE_* bit layout (constants/toggle_constants.asm).
        assert_eq!(toggle_id_to_bit_index("PEWTER_CITY_OBJ_3"), Some(0x03)); // TOGGLE_MUSEUM_GUY
        assert_eq!(toggle_id_to_bit_index("PEWTER_CITY_OBJ_5"), Some(0x04)); // TOGGLE_GYM_GUY
        assert_eq!(toggle_id_to_bit_index("SAFFRON_CITY_OBJ_15"), Some(0x18)); // TOGGLE_SAFFRON_CITY_F
        assert_eq!(toggle_id_to_bit_index("ROUTE_2_OBJ_1"), Some(0x19)); // TOGGLE_ROUTE_2_ITEM_1
        assert_eq!(toggle_id_to_bit_index("BLUESHOUSE_TOWN_MAP"), Some(0x29)); // TOGGLE_TOWN_MAP
        assert_eq!(toggle_id_to_bit_index("BLUES_HOUSE_OBJ_3"), Some(0x29)); // script alias
        assert_eq!(toggle_id_to_bit_index("MT_MOON_1F_OBJ_13"), Some(0x6C)); // TOGGLE_MT_MOON_1F_ITEM_6
        assert_eq!(toggle_id_to_bit_index("SILPH_CO_5F_OBJ_6"), Some(0x9B)); // TOGGLE_SILPH_CO_5F_ITEM_1
        assert_eq!(toggle_id_to_bit_index("SILPH_CO_6F_OBJ_9"), Some(0xA1)); // TOGGLE_SILPH_CO_6F_ITEM_1
        assert_eq!(toggle_id_to_bit_index("SILPH_CO_7F_OBJ_8"), Some(0xA6)); // TOGGLE_SILPH_CO_7F_4
        assert_eq!(toggle_id_to_bit_index("POKEMON_MANSION_B1F_OBJ_8"), Some(0xC2)); // SECRET_KEY
        assert_eq!(toggle_id_to_bit_index("SEAFOAM_ISLANDS_B4F_OBJ_2"), Some(0xE2)); // BOULDER_2
        assert_eq!(toggle_id_to_bit_index("CHAMPIONS_ROOM_OBJ_2"), Some(0xD6)); // TOGGLE_CHAMPIONS_ROOM_OAK
        assert_eq!(toggle_id_to_bit_index("ROUTE_12_OBJ_1"), Some(0x1D)); // SNORLAX
        assert_eq!(toggle_id_to_bit_index("ROUTE_16_OBJ_7"), Some(0x21)); // SNORLAX
        assert_eq!(toggle_id_to_bit_index("BILLS_HOUSE_OBJ_1"), Some(0x61)); // BILL_POKEMON
        assert_eq!(toggle_id_to_bit_index("GAME_CORNER_OBJ_11"), Some(0x46)); // GAME_CORNER_ROCKET
        assert_eq!(
            toggle_id_to_bit_index("CELADON_MANSION_ROOF_HOUSE_OBJ_2"),
            Some(0x45) // EEVEE_GIFT
        );
        assert_eq!(toggle_id_to_bit_index("MUSEUM_1F_OBJ_5"), Some(0x34)); // OLD_AMBER
    }

    #[test]
    fn test_byte_offset_and_mask() {
        // Bit 0x2B = 43 -> byte 5, bit 3
        let ball1 = ToggleableObject::STARTER_BALL_1;
        assert_eq!(ball1.byte_offset(), 5);
        assert_eq!(ball1.bit_mask(), 0x08);

        // Bit 0x2C = 44 -> byte 5, bit 4
        let ball2 = ToggleableObject::STARTER_BALL_2;
        assert_eq!(ball2.byte_offset(), 5);
        assert_eq!(ball2.bit_mask(), 0x10);
    }

    #[test]
    fn test_hide_show_flags() {
        let mut flags = [0u8; 32];

        // Hide Charmander ball (bit 0x2B)
        set_object_hidden(&mut flags, 0x2B);
        assert!(is_object_hidden(&flags, 0x2B));
        assert!(!is_object_hidden(&flags, 0x2C)); // Squirtle ball not affected

        // Show it again
        set_object_shown(&mut flags, 0x2B);
        assert!(!is_object_hidden(&flags, 0x2B));
    }
}

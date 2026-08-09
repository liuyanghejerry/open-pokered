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

    // SAFARI_ZONE hidden objects use bit indices > 0xC3
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
        "PALLETTOWN_OAK" | "PALLET_TOWN_OAK" => Some(ToggleableObject::PALLET_TOWN_OAK.bit_index()),

        // VIRIDIAN_CITY
        "VIRIDIANCITY_OLD_MAN_SLEEPY" | "LYING_OLD_MAN" => {
            Some(ToggleableObject::LYING_OLD_MAN.bit_index())
        }
        "VIRIDIANCITY_OLD_MAN" | "OLD_MAN" => Some(ToggleableObject::OLD_MAN.bit_index()),

        // PEWTER_CITY
        "PEWTERCITY_SUPER_NERD1" | "MUSEUM_GUY" => Some(ToggleableObject::MUSEUM_GUY.bit_index()),
        "PEWTERCITY_YOUNGSTER" | "GYM_GUY" => Some(ToggleableObject::GYM_GUY.bit_index()),

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

        // ROUTE_22
        "ROUTE22_RIVAL1" | "ROUTE_22_RIVAL_1" => {
            Some(ToggleableObject::ROUTE_22_RIVAL_1.bit_index())
        }
        "ROUTE22_RIVAL2" | "ROUTE_22_RIVAL_2" => {
            Some(ToggleableObject::ROUTE_22_RIVAL_2.bit_index())
        }

        // BLUES_HOUSE
        "BLUESHOUSE_DAISY1" | "DAISY_SITTING" => Some(ToggleableObject::DAISY_SITTING.bit_index()),
        "BLUESHOUSE_DAISY2" | "DAISY_WALKING" => Some(ToggleableObject::DAISY_WALKING.bit_index()),
        "BLUESHOUSE_TOWN_MAP" | "TOWN_MAP" => Some(ToggleableObject::TOWN_MAP.bit_index()),

        // BILLS_HOUSE
        "BILLSHOUSE_BILL_POKEMON" | "BILL_POKEMON" => {
            Some(ToggleableObject::BILL_POKEMON.bit_index())
        }
        "BILLSHOUSE_BILL1" | "BILL_1" => Some(ToggleableObject::BILL_1.bit_index()),
        "BILLSHOUSE_BILL2" | "BILL_2" => Some(ToggleableObject::BILL_2.bit_index()),

        // SS_ANNE_2F
        "SSANNE2F_RIVAL" | "SS_ANNE_2F_RIVAL" => {
            Some(ToggleableObject::SS_ANNE_2F_RIVAL.bit_index())
        }

        // POKEMON_TOWER_2F
        "POKEMONTOWER2F_RIVAL" | "POKEMON_TOWER_2F_RIVAL" => {
            Some(ToggleableObject::POKEMON_TOWER_2F_RIVAL.bit_index())
        }

        // FIGHTING_DOJO
        "FIGHTINGDOJO_HITMONLEE_POKE_BALL" => {
            Some(ToggleableObject::HITMONLEE_POKE_BALL.bit_index())
        }
        "FIGHTINGDOJO_HITMONCHAN_POKE_BALL" => {
            Some(ToggleableObject::HITMONCHAN_POKE_BALL.bit_index())
        }

        // SILPH_CO_7F
        "SILPHCO7F_RIVAL" | "SILPH_CO_7F_RIVAL" => {
            Some(ToggleableObject::SILPH_CO_7F_RIVAL.bit_index())
        }

        // CHAMPIONS_ROOM
        "CHAMPIONSROOM_OAK" | "CHAMPIONS_ROOM_OAK" => {
            Some(ToggleableObject::CHAMPIONS_ROOM_OAK.bit_index())
        }

        // MR_FUJIS_HOUSE
        "MRFUJISHOUSE_MR_FUJI" | "MR_FUJI" => Some(ToggleableObject::MR_FUJI.bit_index()),

        // ROUTE_12 SNORLAX
        "ROUTE12_SNORLAX" => Some(ToggleableObject::ROUTE_12_SNORLAX.bit_index()),

        // ROUTE_16 SNORLAX
        "ROUTE16_SNORLAX" => Some(ToggleableObject::ROUTE_16_SNORLAX.bit_index()),

        // MUSEUM_1F
        "MUSEUM1F_OLD_AMBER" | "OLD_AMBER" => Some(ToggleableObject::OLD_AMBER.bit_index()),

        // CELADON_MANSION
        "CELADONMANSION_ROOF_HOUSE_EEVEE_POKEBALL" | "EEVEE_GIFT" => {
            Some(ToggleableObject::EEVEE_GIFT.bit_index())
        }

        // GAME_CORNER
        "GAMECORNER_ROCKET" | "GAME_CORNER_ROCKET" => {
            Some(ToggleableObject::GAME_CORNER_ROCKET.bit_index())
        }

        // VIRIDIAN_GYM
        "VIRIDIANGYM_GIOVANNI" | "VIRIDIAN_GYM_GIOVANNI" => {
            Some(ToggleableObject::VIRIDIAN_GYM_GIOVANNI.bit_index())
        }

        // POWER_PLANT
        "POWERPLANT_ZAPDOS" | "ZAPDOS" => Some(ToggleableObject::ZAPDOS.bit_index()),

        // VICTORY_ROAD
        "VICTORYROAD2F_MOLTRES" | "MOLTRES" => Some(ToggleableObject::MOLTRES.bit_index()),

        // SEAFOAM_ISLANDS
        "SEAFOAMISLANDSB4F_ARTICUNO" | "ARTICUNO" => Some(ToggleableObject::ARTICUNO.bit_index()),

        // CERULEAN_CAVE
        "CERULEANCAVEB1F_MEWTWO" | "MEWTWO" => Some(ToggleableObject::MEWTWO.bit_index()),

        // MT_MOON
        "MTMOONB2F_DOME_FOSSIL" | "DOME_FOSSIL" => Some(ToggleableObject::DOME_FOSSIL.bit_index()),
        "MTMOONB2F_HELIX_FOSSIL" | "HELIX_FOSSIL" => {
            Some(ToggleableObject::HELIX_FOSSIL.bit_index())
        }

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

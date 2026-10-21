//! The two original 16 × 16-byte sprite tables, including the player slot.
//! Keep unknown bytes on import/export instead of silently discarding them.
use crate::alloc_prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteData {
    pub first: [[u8; 16]; 16],
    pub second: [[u8; 16]; 16],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{sram_export::export_sram, sram_import::import_sram, SaveData};

    #[test]
    fn all_sprite_bytes_survive_sram_and_json_roundtrips_and_clear() {
        let bytes: Vec<u8> = (0..512).map(|i| (i * 37 + 11) as u8).collect();
        let mut save = SaveData::new();
        save.sprite_data = SpriteData::from_bytes(&bytes);
        let raw = export_sram(&save);
        let imported = import_sram(&raw).unwrap();
        assert_eq!(imported.sprite_data, save.sprite_data);
        let json = serde_json::to_value(&save).unwrap();
        let decoded: SaveData = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.sprite_data, save.sprite_data);
        let mut serialized = Vec::new();
        imported.sprite_data.write_into(&mut serialized);
        assert_eq!(serialized, bytes);
        save.clear();
        assert_eq!(save.sprite_data, SpriteData::default());
    }

    #[test]
    fn old_json_without_sprite_data_keeps_zero_default() {
        let mut old = serde_json::to_value(SaveData::new()).unwrap();
        old.as_object_mut().unwrap().remove("sprite_data");
        let save: SaveData = serde_json::from_value(old).unwrap();
        assert_eq!(save.sprite_data, SpriteData::default());
    }

    #[test]
    fn sprite_bytes_participate_in_the_canonical_checksum() {
        let mut save = SaveData::new();
        let before = save.compute_checksum();
        save.sprite_data.second[7][15] = 1;
        assert_ne!(save.compute_checksum(), before);
        assert_eq!(
            import_sram(&export_sram(&save)).unwrap().sprite_data,
            save.sprite_data
        );
    }

    #[test]
    fn displacement_matches_all_controlled_original_rom_probes() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("npc_displacement_golden.json")).unwrap();
        assert_eq!(cases.as_array().unwrap().len(), 132);
        for case in cases.as_array().unwrap() {
            let mut data = SpriteData::default();
            data.second[1][2] = case["y"].as_u64().unwrap() as u8;
            data.second[1][3] = case["x"].as_u64().unwrap() as u8;
            let allowed = if case["movement_byte1"].as_u64().unwrap() < 0xfe {
                // The source scripted branch returns before displacement stores.
                true
            } else {
                data.try_displacement(
                    1,
                    case["dx"].as_i64().unwrap() as i8,
                    case["dy"].as_i64().unwrap() as i8,
                )
            };
            assert_eq!(allowed, case["allowed"].as_bool().unwrap(), "{case}");
            assert_eq!(
                data.second[1][2],
                case["after_y"].as_u64().unwrap() as u8,
                "{case}"
            );
            assert_eq!(
                data.second[1][3],
                case["after_x"].as_u64().unwrap() as u8,
                "{case}"
            );
        }
    }
}

impl SpriteData {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert_eq!(bytes.len(), 512);
        let mut result = Self::default();
        for slot in 0..16 {
            result.first[slot].copy_from_slice(&bytes[slot * 16..slot * 16 + 16]);
            result.second[slot].copy_from_slice(&bytes[256 + slot * 16..272 + slot * 16]);
        }
        result
    }

    pub fn write_into(&self, output: &mut Vec<u8>) {
        for row in &self.first {
            output.extend_from_slice(row);
        }
        for row in &self.second {
            output.extend_from_slice(row);
        }
    }

    /// CanWalkOntoTile's deliberately asymmetric byte arithmetic. Scripted
    /// movement returns before these stores and must not call this method.
    pub fn try_displacement(&mut self, slot: usize, dx: i8, dy: i8) -> bool {
        let row = &mut self.second[slot];
        let y = if dy < 0 {
            let Some(y) = row[2].checked_sub(1) else {
                return false;
            };
            y
        } else {
            let y = row[2].wrapping_add(dy as u8);
            if y < 5 {
                return false;
            }
            y
        };
        let x = if dx < 0 {
            let Some(x) = row[3].checked_sub(1) else {
                return false;
            };
            x
        } else {
            // Original CP 5 has no conditional jump on the non-left branch.
            row[3].wrapping_add(dx as u8)
        };
        row[2] = y;
        row[3] = x;
        true
    }
}

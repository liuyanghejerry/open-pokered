use serde::{Deserialize, Serialize};

pub const SRAM_BANK_SIZE: usize = 0x2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveError {
    DataTooShort,
    BadChecksum,
    InvalidData,
}

impl core::fmt::Display for SaveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SaveError::DataTooShort => write!(f, "save data too short"),
            SaveError::BadChecksum => write!(f, "bad checksum"),
            SaveError::InvalidData => write!(f, "invalid save data"),
        }
    }
}

impl std::error::Error for SaveError {}

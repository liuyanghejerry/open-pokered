//! An instruction offered to the single external clock/APU owner.
//! Reads return the hardware value at the stated access cycle. The owner must
//! reject before any time or register side effect; successful reads return Some.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum ApuAccess {
    Read {
        address: u16,
        at_tcycle: u8,
    },
    Write {
        address: u16,
        value: u8,
        at_tcycle: u8,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct ApuInstruction {
    pub tcycles: u8,
    pub access: Option<ApuAccess>,
}
impl ApuInstruction {
    pub const fn plain(tcycles: u8) -> Self {
        Self {
            tcycles,
            access: None,
        }
    }
    pub const fn read(tcycles: u8, address: u16) -> Self {
        Self {
            tcycles,
            access: Some(ApuAccess::Read {
                address,
                at_tcycle: tcycles - 4,
            }),
        }
    }
    pub const fn write(tcycles: u8, address: u16, value: u8) -> Self {
        Self {
            tcycles,
            access: Some(ApuAccess::Write {
                address,
                value,
                at_tcycle: tcycles - 4,
            }),
        }
    }
}

/// Actual command-result values supplied when entering the pitch/output child.
/// Wave bytes are required only on a source path that reads its wave pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct NoteBusInput {
    pub note_delay: u8,
    pub volume: u8,
    pub duty: u8,
    pub panning: u8,
    pub wave: Option<[u8; 16]>,
}

/// Missing explicit source data is refused before admitting an instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusStepError<E> {
    MissingWave,
    Execute(E),
}

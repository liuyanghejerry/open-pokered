//! Source foreground video callers, sharing their attached owner's bus.
//! No clock, interrupt implementation, idle-frame estimate or synthetic wake
//! fallback is owned here. A runtime must attach its actual common device/IRQ
//! owner before these callers can progress.

pub mod copy;
pub mod delay;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DisplayAccess {
    Read { port: u16, at_tcycle: u8 },
    Write { port: u16, value: u8, at_tcycle: u8 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DisplayInstruction {
    pub tcycles: u8,
    pub access: Option<DisplayAccess>,
}
pub trait DisplayInstructionBus {
    type Error;
    /// An error occurs before execution. A read receipt supplies its actual
    /// access-cycle value; the owner runs intervening device/IRQ events.
    fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, Self::Error>;
    /// Optional exact effect-free prefix. The owner must prove that no IRQ
    /// can interleave the accepted prefix; accesses/effects use execute.
    /// Returns an accepted instruction count, never an estimated duration.
    fn execute_plain_run<I: Iterator<Item = DisplayInstruction>>(
        &mut self,
        _instructions: I,
    ) -> Result<usize, Self::Error>
    where
        Self: Sized,
    {
        Ok(0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CopyFormat {
    TwoBpp,
    OneBppDouble,
}
pub const ROM_BANK_TEMP: u16 = 0xff8b;
pub const LOADED_ROM_BANK: u16 = 0xffb8;
pub const ROM_BANK_SELECT: u16 = 0x2000;

#[cfg(test)]
mod tests;

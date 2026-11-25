//! Finite DisableLCD/EnableLCD children from home/lcd.asm.
//!
//! Instructions consume the owning shared bus time; this work has no clock.
//! IO access is at the start of the final M-cycle of LDH (opcode/immediate/IO
//! at 0/4/8 T). This is a bus-schedule derivation, not the earlier PyBoy MB
//! method-call timestamp, and does not claim independent hardware validation.
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
/// Implemented by the common native device/elapsed-time owner. It must run
/// intervening hardware/IRQ work before an access. An error must occur before
/// executing this instruction; a successful Read returns Some(value).
pub trait DisplayInstructionBus {
    type Error;
    fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, Self::Error>;
    /// Optional exact execution of a prefix with no port accesses or caller
    /// effects. Return the accepted instruction count, preserving the normal
    /// start-before-deadline budget rule. Zero means use ordinary stepping.
    /// Implementations must prove interrupts cannot interleave the prefix;
    /// this is not permission to replace a scene with a captured duration.
    fn execute_plain_run<I: Iterator<Item = DisplayInstruction>>(
        &mut self,
        _instructions: I,
    ) -> Result<usize, Self::Error>
    where
        Self: Sized,
    {
        Ok(0)
    }
    fn execute_plain_cycles(&mut self, cycles: &[u8]) -> Result<usize, Self::Error>
    where
        Self: Sized,
    {
        self.execute_plain_run(cycles.iter().map(|&tcycles| DisplayInstruction {
            tcycles,
            access: None,
        }))
    }
    fn execute_plain_cycles_total(
        &mut self,
        cycles: &[u8],
        _total: u64,
    ) -> Result<usize, Self::Error>
    where
        Self: Sized,
    {
        self.execute_plain_cycles(cycles)
    }
    fn execute_plain_total(&mut self, _total: u32) -> Result<bool, Self::Error> {
        Ok(false)
    }
    fn execute_joypad_reads(
        &mut self,
        _select: Option<u8>,
        _count: u8,
    ) -> Result<Option<(u8, u8)>, Self::Error> {
        Ok(None)
    }
    fn execute_irq_random(
        &mut self,
        _carry: bool,
    ) -> Result<Option<(u8, u8, bool, u8, u8)>, Self::Error> {
        Ok(None)
    }
    fn execute_sprite_bank_call(
        &mut self,
        _target_bank: u8,
        _restore_bank: Option<u8>,
    ) -> Result<Option<u8>, Self::Error> {
        Ok(None)
    }
    fn execute_audio_bank_dispatch(
        &mut self,
        _bank: u8,
    ) -> Result<Option<(u8, bool)>, Self::Error> {
        Ok(None)
    }
    fn execute_joypad_frame(&mut self) -> Result<Option<(u8, u8)>, Self::Error> {
        Ok(None)
    }
    fn execute_buffer_copy(
        &mut self,
        _source: &[u8],
        _destination: &mut [u8],
        _finish_at_final_store: bool,
    ) -> Result<usize, Self::Error>
    where
        Self: Sized,
    {
        Ok(0)
    }
    /// Optional IRQ prefix containing only plain instructions and shadow-OAM
    /// stores. The owner must exclude DMA and interrupt interleaving, so these
    /// RAM stores cannot become visible until the accepted prefix returns.
    /// Each accepted instruction still obeys the ordinary budget boundary.
    fn execute_shadow_oam_run<I: Iterator<Item = DisplayInstruction>>(
        &mut self,
        _instructions: I,
    ) -> Result<usize, Self::Error>
    where
        Self: Sized,
    {
        Ok(0)
    }
    /// Execute an already compiled, wholly affordable shadow-OAM program.
    /// Stores retain their page-relative byte addresses; false requests the
    /// ordinary instruction path without consuming time or changing memory.
    fn execute_shadow_oam_total<I: Iterator<Item = (u8, u8)>>(
        &mut self,
        _stores: I,
        _cycles: u32,
    ) -> Result<bool, Self::Error>
    where
        Self: Sized,
    {
        Ok(false)
    }
    /// Execute a complete shadow-OAM clear with the same affordability rules
    /// as `execute_shadow_oam_total`; false requests ordinary stepping.
    fn execute_shadow_oam_clear(&mut self, _cycles: u32) -> Result<bool, Self::Error>
    where
        Self: Sized,
    {
        Ok(false)
    }
    /// Poll LY through its actual 12/8/12-cycle instructions. None means use
    /// ordinary stepping; Some is the last LY, with 145 ending the poll. The
    /// owner must reject this run while VBlank dispatch remains enabled.
    fn execute_ly145_run(&mut self) -> Result<Option<u8>, Self::Error> {
        Ok(None)
    }
}
pub(crate) struct PlainInstructionBus(pub(crate) u8);
impl DisplayInstructionBus for PlainInstructionBus {
    type Error = core::convert::Infallible;
    fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
        assert_eq!(instruction.access, None);
        self.0 = instruction.tcycles;
        Ok(None)
    }
}
impl super::pattern_copy_work::PatternInstructionBus for PlainInstructionBus {
    type Error = core::convert::Infallible;
    fn execute(
        &mut self,
        instruction: super::pattern_copy_work::PatternInstruction,
    ) -> Result<Option<u16>, Self::Error> {
        assert_eq!(instruction.access, None);
        self.0 = instruction.tcycles;
        Ok(None)
    }
}

/// Speculatively advance private work, then commit only instructions admitted
/// by the actual clock. Work using this helper must not access ports or state
/// observed by the display device. Unsupported buses retain ordinary stepping.
pub(crate) fn run_plain_work<W: Clone, B: DisplayInstructionBus>(
    work: &mut W,
    bus: &mut B,
    finished: impl Fn(&W) -> bool,
    step: impl Fn(&mut W, &mut PlainInstructionBus) -> Result<bool, core::convert::Infallible>,
    step_bus: impl Fn(&mut W, &mut B) -> Result<bool, B::Error>,
) -> Result<(), B::Error> {
    while !finished(work) {
        let mut trial = work.clone();
        let mut generated = 0usize;
        let mut halted = false;
        let accepted = bus.execute_plain_run(core::iter::from_fn(|| {
            let mut plain = PlainInstructionBus(0);
            match step(&mut trial, &mut plain) {
                Ok(true) => {
                    generated += 1;
                    Some(DisplayInstruction {
                        tcycles: plain.0,
                        access: None,
                    })
                }
                Ok(false) => {
                    halted = true;
                    None
                }
                Err(error) => match error {},
            }
        }))?;
        if accepted == 0 {
            if !step_bus(work, bus)? {
                return Ok(());
            }
        } else if accepted == generated {
            *work = trial;
            if halted {
                return Ok(());
            }
        } else {
            let mut plain = PlainInstructionBus(0);
            for _ in 0..accepted {
                let _ = step(work, &mut plain);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum DisableStep {
    ClearA,
    ClearIf,
    ReadIe,
    SaveIe,
    MaskVBlank,
    WriteMaskedIe,
    ReadLy,
    CompareLy,
    PollLy,
    ReadLcdc,
    ClearEnable,
    WriteLcdc,
    RestoreA,
    WriteIe,
    Return,
    Done,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DisableLcdWork {
    step: DisableStep,
    a: u8,
    b: u8,
}
impl Default for DisableLcdWork {
    fn default() -> Self {
        Self {
            step: DisableStep::ClearA,
            a: 0,
            b: 0,
        }
    }
}
fn plain(tcycles: u8) -> DisplayInstruction {
    DisplayInstruction {
        tcycles,
        access: None,
    }
}
fn read(port: u16) -> DisplayInstruction {
    DisplayInstruction {
        tcycles: 12,
        access: Some(DisplayAccess::Read { port, at_tcycle: 8 }),
    }
}
fn write(port: u16, value: u8) -> DisplayInstruction {
    DisplayInstruction {
        tcycles: 12,
        access: Some(DisplayAccess::Write {
            port,
            value,
            at_tcycle: 8,
        }),
    }
}
impl DisableLcdWork {
    pub fn finished(&self) -> bool {
        self.step == DisableStep::Done
    }
    #[inline(always)]
    pub fn instruction(&self) -> Option<DisplayInstruction> {
        use DisableStep::*;
        Some(match self.step {
            ClearA | SaveIe | RestoreA => plain(4),
            ClearIf => write(0xff0f, self.a),
            ReadIe => read(0xffff),
            MaskVBlank | CompareLy | ClearEnable => plain(8),
            WriteMaskedIe | WriteIe => write(0xffff, self.a),
            ReadLy => read(0xff44),
            PollLy => plain(if self.a == 145 { 8 } else { 12 }),
            ReadLcdc => read(0xff40),
            WriteLcdc => write(0xff40, self.a),
            Return => plain(16),
            Done => return None,
        })
    }
    /// Run the same instructions until return or a bus refusal. The hot LY
    /// loop may use the bus's interrupt-safe polling path; its reads, compares,
    /// branches and exact instruction timing remain unchanged. Commit each
    /// continuation only after the corresponding instructions succeed.
    pub fn run<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<(), B::Error> {
        use DisableStep::*;
        loop {
            if self.step == ReadLy {
                if let Some(ly) = bus.execute_ly145_run()? {
                    self.a = ly;
                    self.step = if ly == 145 { ReadLcdc } else { ReadLy };
                    continue;
                }
                let value = bus.execute(read(0xff44))?;
                self.a = value.expect("display read receipt");
                self.step = CompareLy;
                bus.execute(plain(8))?;
                self.step = PollLy;
                bus.execute(plain(if self.a == 145 { 8 } else { 12 }))?;
                self.step = if self.a == 145 { ReadLcdc } else { ReadLy };
            } else if self.step(bus)?.is_none() {
                return Ok(());
            }
        }
    }

    /// None means the child has returned. A blocked shared bus leaves the
    /// exact native branch/register state intact for the next host slice.
    #[inline(always)]
    pub fn step<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<Option<DisplayInstruction>, B::Error> {
        use DisableStep::*;
        let Some(instruction) = self.instruction() else {
            return Ok(None);
        };
        let value = bus.execute(instruction)?;
        self.step = match self.step {
            ClearA => {
                self.a = 0;
                ClearIf
            }
            ClearIf => ReadIe,
            ReadIe => {
                self.a = value.expect("display read receipt");
                SaveIe
            }
            SaveIe => {
                self.b = self.a;
                MaskVBlank
            }
            MaskVBlank => {
                self.a &= !1;
                WriteMaskedIe
            }
            WriteMaskedIe => ReadLy,
            ReadLy => {
                self.a = value.expect("display read receipt");
                CompareLy
            }
            CompareLy => PollLy,
            PollLy => {
                if self.a == 145 {
                    ReadLcdc
                } else {
                    ReadLy
                }
            }
            ReadLcdc => {
                self.a = value.expect("display read receipt");
                ClearEnable
            }
            ClearEnable => {
                self.a &= 0x7f;
                WriteLcdc
            }
            WriteLcdc => RestoreA,
            RestoreA => {
                self.a = self.b;
                WriteIe
            }
            WriteIe => Return,
            Return => Done,
            Done => Done,
        };
        Ok(Some(instruction))
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum EnableStep {
    #[default]
    ReadLcdc,
    SetEnable,
    WriteLcdc,
    Return,
    Done,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EnableLcdWork {
    step: EnableStep,
    a: u8,
}
impl EnableLcdWork {
    pub fn finished(&self) -> bool {
        self.step == EnableStep::Done
    }
    pub fn instruction(&self) -> Option<DisplayInstruction> {
        Some(match self.step {
            EnableStep::ReadLcdc => read(0xff40),
            EnableStep::SetEnable => plain(8),
            EnableStep::WriteLcdc => write(0xff40, self.a),
            EnableStep::Return => plain(16),
            EnableStep::Done => return None,
        })
    }
    pub fn step<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<Option<DisplayInstruction>, B::Error> {
        let Some(instruction) = self.instruction() else {
            return Ok(None);
        };
        let value = bus.execute(instruction)?;
        self.step = match self.step {
            EnableStep::ReadLcdc => {
                self.a = value.expect("display read receipt");
                EnableStep::SetEnable
            }
            EnableStep::SetEnable => {
                self.a |= 0x80;
                EnableStep::WriteLcdc
            }
            EnableStep::WriteLcdc => EnableStep::Return,
            EnableStep::Return | EnableStep::Done => EnableStep::Done,
        };
        Ok(Some(instruction))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::*;
    struct Ports {
        ie: u8,
        iflags: u8,
        lcdc: u8,
        reads: u8,
        writes: Vec<(u16, u8)>,
        block: bool,
    }
    impl DisplayInstructionBus for Ports {
        type Error = ();
        fn execute(&mut self, i: DisplayInstruction) -> Result<Option<u8>, ()> {
            if self.block {
                return Err(());
            }
            Ok(match i.access {
                Some(DisplayAccess::Read { port, .. }) => Some(match port {
                    0xffff => self.ie,
                    0xff40 => self.lcdc,
                    // LY advances on the shared device, never on the child.
                    0xff44 => {
                        self.reads += 1;
                        if self.reads == 1 {
                            144
                        } else {
                            145
                        }
                    }
                    _ => panic!(),
                }),
                Some(DisplayAccess::Write { port, value, .. }) => {
                    self.writes.push((port, value));
                    match port {
                        0xffff => self.ie = value,
                        0xff0f => self.iflags = value,
                        0xff40 => self.lcdc = value,
                        _ => panic!(),
                    };
                    None
                }
                None => None,
            })
        }
    }
    fn ports() -> Ports {
        Ports {
            ie: 13,
            iflags: 3,
            lcdc: 0x93,
            reads: 0,
            writes: Vec::new(),
            block: false,
        }
    }
    #[test]
    fn disable_preserves_other_lcdc_bits_and_restores_ie_after_ly145() {
        let mut b = ports();
        let mut child = DisableLcdWork::default();
        let mut instructions = Vec::new();
        while let Some(i) = child.step(&mut b).unwrap() {
            instructions.push(i)
        }
        assert_eq!(
            b.writes,
            [(0xff0f, 0), (0xffff, 12), (0xff40, 0x13), (0xffff, 13)]
        );
        assert_eq!(b.reads, 2);
        assert!(child.finished());
        assert!(instructions
            .iter()
            .any(|i| i.tcycles == 12 && i.access.is_none())); // taken LY poll
        let mut enable = EnableLcdWork::default();
        while enable.step(&mut b).unwrap().is_some() {}
        assert_eq!(b.lcdc, 0x93);
        assert!(enable.finished());
    }
    #[test]
    fn pending_if_after_masking_is_not_cleared_when_lcd_disables() {
        let mut b = ports();
        let mut child = DisableLcdWork::default();
        for _ in 0..6 {
            child.step(&mut b).unwrap();
        }
        assert_eq!(b.ie, 12);
        b.iflags |= 1;
        while child.step(&mut b).unwrap().is_some() {}
        assert_eq!(
            b.iflags & b.ie & 1,
            1,
            "already pending VBlank remains dispatchable after IE restore"
        );
    }
    #[test]
    fn blocked_instruction_and_json_roundtrip_retain_the_same_branch() {
        let mut b = ports();
        let mut a = DisableLcdWork::default();
        for _ in 0..7 {
            a.step(&mut b).unwrap();
        }
        let saved = a;
        b.block = true;
        assert_eq!(a.step(&mut b), Err(()));
        assert_eq!(a, saved);
        let other: DisableLcdWork =
            serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(a, other);
        b.block = false;
        while a.step(&mut b).unwrap().is_some() {}
        assert!(a.finished());
    }
}

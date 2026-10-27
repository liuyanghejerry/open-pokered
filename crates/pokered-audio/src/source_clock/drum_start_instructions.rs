//! Audio1/3_PlaySound for the canonical one-channel noise headers (IDs 1..19).
//! These paths have no APU access. The owning frame applies the resulting
//! virtual CHAN8 constructor only after this retained call has returned.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct DrumStartResult {
    pub instrument: u8,
    pub started: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum Stage {
    Entry(u8),
    Header(u8),
    Channel(u8),
    Busy(u8),
    ClearPointers(u8),
    ClearFields(u8),
    Defaults(u8),
    Sweep(u8),
    Common(u8),
    FindChannel(u8, u8),
    Install(u8),
    Cry(u8),
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct DrumStartInstructions {
    instrument: u8,
    busy: bool,
    stage: Stage,
    cycles: u32,
}

impl DrumStartInstructions {
    pub fn new(instrument: u8, busy: bool) -> Option<Self> {
        (1..=19).contains(&instrument).then_some(Self {
            instrument,
            busy,
            stage: Stage::Entry(0),
            cycles: 0,
        })
    }

    pub fn result(&self) -> Option<DrumStartResult> {
        matches!(self.stage, Stage::Done).then_some(DrumStartResult {
            instrument: self.instrument,
            started: !self.busy,
        })
    }

    pub fn cycles(&self) -> u32 {
        self.cycles
    }

    pub(crate) fn peek(&self) -> Option<u8> {
        use Stage::*;
        Some(match self.stage {
            Entry(n) => [16, 8, 12, 8, 12, 16][n as usize],
            Header(n) => [4, 4, 8, 4, 8, 8, 12, 8, 4, 16, 4, 16, 8, 8, 4, 4, 4][n as usize],
            Channel(n) => {
                if n == 20 {
                    if self.busy {
                        8
                    } else {
                        12
                    }
                } else {
                    [
                        4, 4, 4, 4, 4, 8, 16, 4, 16, 4, 8, 4, 8, 8, 4, 8, 12, 8, 8, 4,
                    ][n as usize]
                }
            }
            Busy(n) => [4, 8, 8, 16, 8, 8, 16][n as usize],
            ClearPointers(n) => [4, 16, 4, 4, 8, 4, 4, 12, 8, 8, 8, 12, 8, 8, 8, 12][n as usize],
            // Eighteen independent LD HL / ADD HL,DE / LD [HL],A stores.
            ClearFields(n) => [12, 8, 8][n as usize % 3],
            Defaults(n) => {
                if n == 0 {
                    8
                } else {
                    [12, 8, 8][(n as usize - 1) % 3]
                }
            }
            Sweep(n) => [4, 8, 12, 4, 4, 16][n as usize],
            Common(n) => [
                16, 4, 4, 8, 4, 8, 8, 12, 8, 4, 4, 12, 8, 4, 4, 4, 8, 4, 4, 8, 4, 4, 8, 8,
            ][n as usize],
            FindChannel(channel, n) => {
                if channel == 7 {
                    [4, 12][n as usize]
                } else {
                    [4, 8, 4, 8, 8, 12][n as usize]
                }
            }
            Install(n) => [
                16, 16, 16, 8, 4, 12, 8, 16, 8, 12, 8, 8, 12, 8, 16, 12, 12, 8, 8, 8, 8, 8, 8, 4,
                4, 4, 4, 8, 8, 8,
            ][n as usize],
            Cry(n) => [16, 8, 8, 12, 16][n as usize],
            Done => return None,
        })
    }

    pub fn try_step<E>(&mut self, execute: impl FnOnce(u8) -> Result<(), E>) -> Result<bool, E> {
        let Some(t) = self.peek() else {
            return Ok(true);
        };
        execute(t)?;
        self.accept();
        Ok(self.result().is_some())
    }

    pub(crate) fn accept(&mut self) {
        use Stage::*;
        let t = self.peek().expect("unfinished sound start");
        self.stage = match self.stage {
            Entry(5) => Header(0),
            Entry(n) => Entry(n + 1),
            Header(16) => Channel(0),
            Header(n) => Header(n + 1),
            Channel(20) => {
                if self.busy {
                    Busy(0)
                } else {
                    ClearPointers(0)
                }
            }
            Channel(n) => Channel(n + 1),
            Busy(6) => Done,
            Busy(n) => Busy(n + 1),
            ClearPointers(15) => ClearFields(0),
            ClearPointers(n) => ClearPointers(n + 1),
            ClearFields(53) => Defaults(0),
            ClearFields(n) => ClearFields(n + 1),
            Defaults(9) => Sweep(0),
            Defaults(n) => Defaults(n + 1),
            Sweep(5) => Common(0),
            Sweep(n) => Sweep(n + 1),
            Common(23) => FindChannel(0, 0),
            Common(n) => Common(n + 1),
            FindChannel(7, 1) => Install(0),
            FindChannel(c, 5) => FindChannel(c + 1, 0),
            FindChannel(c, n) => FindChannel(c, n + 1),
            Install(29) => Cry(0),
            Install(n) => Install(n + 1),
            Cry(4) => Done,
            Cry(n) => Cry(n + 1),
            Done => unreachable!(),
        };
        self.cycles += u32::from(t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Case {
        bank: u8,
        instrument: u8,
        busy: bool,
        trace: alloc::vec::Vec<(u16, u8)>,
        result: DrumStartResult,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: alloc::vec::Vec<Case>,
    }

    #[test]
    fn every_drum_start_and_busy_drop_matches_both_original_banks() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("drum_start_golden.json")).unwrap();
        assert_eq!(fixture.cases.len(), 76);
        for case in fixture.cases {
            let mut work = DrumStartInstructions::new(case.instrument, case.busy).unwrap();
            for (index, (pc, t)) in case.trace.iter().copied().enumerate() {
                let before = work;
                assert_eq!(work.try_step(|_| Err("budget")), Err("budget"));
                assert_eq!(work, before);
                work = serde_json::from_slice(&serde_json::to_vec(&work).unwrap()).unwrap();
                let done = work
                    .try_step(|actual| {
                        assert_eq!(
                            actual, t,
                            "bank{} drum{} busy{} step{index} pc{pc:04x}",
                            case.bank, case.instrument, case.busy
                        );
                        Ok::<_, ()>(())
                    })
                    .unwrap();
                assert_eq!(done, index + 1 == case.trace.len());
            }
            assert_eq!(work.result(), Some(case.result));
            assert_eq!(
                work.cycles(),
                case.trace.iter().map(|(_, t)| u32::from(*t)).sum::<u32>()
            );
            assert_eq!(
                work.try_step(|_| -> Result<(), ()> { panic!("completed start replayed") }),
                Ok(true)
            );
        }
    }
}

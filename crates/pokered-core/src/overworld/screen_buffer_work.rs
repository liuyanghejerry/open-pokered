//! LoadScreenTilesFromBuffer2 and its CopyData child (home/tilemap.asm).
//! The live tile buffer changes on each successful WRAM store. AutoBG stays
//! disabled until all 360 bytes have been copied; the common bus owns IRQs.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};
use super::video_copy_caller_work::AUTO_BG_ENABLED;
use crate::alloc_prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    Call,
    DisableCall,
    Clear,
    Disable,
    Source,
    Destination,
    Count,
    CopyCall,
    Read,
    Write,
    IncrementDestination,
    DecrementCount,
    LoadCountLow,
    OrCountHigh,
    Repeat,
    CopyReturn,
    DisableReturn,
    LoadOne,
    Enable,
    Return,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScreenBufferWork {
    step: Step,
    backup: Vec<u8>,
    source: u16,
    destination: u16,
    remaining: u16,
    a: u8,
}

impl ScreenBufferWork {
    /// The saved 20×18 logical tile buffer. Native UI handles and map tile IDs
    /// keep their own namespaces; this copy does not reinterpret either one.
    pub fn new(backup: Vec<u8>) -> Option<Self> {
        (backup.len() == 360).then_some(Self {
            step: Step::Call,
            backup,
            source: 0,
            destination: 0,
            remaining: 360,
            a: 0,
        })
    }

    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }

    pub fn copied_bytes(&self) -> usize {
        usize::from(self.destination) + usize::from(self.step == Step::IncrementDestination)
    }

    /// Keep foreground instruction dispatch inside the child until a bus
    /// refusal or RET. Yield once after the final store, while AutoBG is off,
    /// so native callers can bind the completed buffer's pattern namespace.
    /// Every instruction and WRAM store still executes separately.
    pub fn run<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
        tiles: &mut [u8; 360],
    ) -> Result<bool, B::Error> {
        let copying = self.copied_bytes() < 360;
        loop {
            if !self.step(bus, tiles)? {
                return Ok(false);
            }
            if copying && self.copied_bytes() == 360 {
                return Ok(true);
            }
        }
    }

    /// Execute one instruction. A refusal changes neither the continuation nor
    /// the destination; IRQ consumers can observe only completed stores.
    pub fn step<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
        tiles: &mut [u8; 360],
    ) -> Result<bool, B::Error> {
        use Step::*;
        let tcycles = match self.step {
            Call | DisableCall | CopyCall => 24,
            Clear | LoadCountLow | OrCountHigh => 4,
            Disable | Source | Destination | Count | Enable => 12,
            Read | Write | IncrementDestination | DecrementCount | LoadOne => 8,
            Repeat => {
                if self.a == 0 {
                    8
                } else {
                    12
                }
            }
            CopyReturn | DisableReturn | Return => 16,
            Done => return Ok(false),
        };
        let access = matches!(self.step, Disable | Enable).then_some(DisplayAccess::Write {
            port: AUTO_BG_ENABLED,
            value: self.a,
            at_tcycle: 8,
        });
        bus.execute(DisplayInstruction { tcycles, access })?;
        self.step = match self.step {
            Call => DisableCall,
            DisableCall => Clear,
            Clear => {
                self.a = 0;
                Disable
            }
            Disable => Source,
            Source => Destination,
            Destination => Count,
            Count => CopyCall,
            CopyCall => Read,
            Read => {
                self.a = self.backup[usize::from(self.source)];
                self.source += 1;
                Write
            }
            Write => {
                tiles[usize::from(self.destination)] = self.a;
                IncrementDestination
            }
            IncrementDestination => {
                self.destination += 1;
                DecrementCount
            }
            DecrementCount => {
                self.remaining -= 1;
                LoadCountLow
            }
            LoadCountLow => {
                self.a = self.remaining as u8;
                OrCountHigh
            }
            OrCountHigh => {
                self.a |= (self.remaining >> 8) as u8;
                Repeat
            }
            Repeat => {
                if self.a == 0 {
                    CopyReturn
                } else {
                    Read
                }
            }
            CopyReturn => DisableReturn,
            DisableReturn => LoadOne,
            LoadOne => {
                self.a = 1;
                Enable
            }
            Enable => Return,
            Return => Done,
            Done => unreachable!(),
        };
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bus {
        cycles: u32,
        grant: usize,
        bg: u8,
        writes: Vec<(u32, u8)>,
    }
    impl DisplayInstructionBus for Bus {
        type Error = ();
        fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, ()> {
            if self.grant == 0 {
                return Err(());
            }
            self.grant -= 1;
            if let Some(DisplayAccess::Write {
                port,
                value,
                at_tcycle,
            }) = instruction.access
            {
                assert_eq!(port, AUTO_BG_ENABLED);
                self.writes
                    .push((self.cycles + u32::from(at_tcycle), value));
                self.bg = value;
            }
            self.cycles += u32::from(instruction.tcycles);
            Ok(None)
        }
    }

    #[test]
    fn original_buffer_copy_masks_auto_bg_until_every_store_and_matches_observed_cycles() {
        let source: Vec<_> = (0..360).map(|i| (i % 251) as u8).collect();
        let mut work = ScreenBufferWork::new(source.clone()).unwrap();
        let mut tiles = [255; 360];
        let mut bus = Bus {
            cycles: 0,
            grant: usize::MAX,
            bg: 1,
            writes: Vec::new(),
        };
        let mut stores = 0;
        loop {
            let before = tiles;
            if !work.step(&mut bus, &mut tiles).unwrap() {
                break;
            }
            if tiles != before {
                assert_eq!(bus.bg, 0, "a partial logical buffer cannot reach AutoBG");
                // Read-only original hooks observe every completed store at
                // 116 + 52*n cycles after callee entry (plus our outer CALL).
                assert_eq!(bus.cycles, 24 + 116 + 52 * stores as u32);
                assert_eq!(tiles[stores], source[stores]);
                assert_eq!(&tiles[stores + 1..], &before[stores + 1..]);
                stores += 1;
            }
            if bus.bg == 1 && bus.cycles > 64 {
                assert_eq!(tiles.as_slice(), source);
            }
            assert_eq!(work.copied_bytes(), stores);
        }
        assert!(work.finished());
        assert_eq!(stores, 360);
        // Normal Red, fbcf7d0: four actual menu exits, twice each. The callee
        // entry-to-RET interval is 18,884 T-cycles; this child includes CALL.
        assert_eq!(bus.cycles, 24 + 18_884);
        assert_eq!(bus.writes, [(60, 0), (18_888, 1)]);
    }

    #[test]
    fn refused_instructions_and_serialized_resumes_do_not_repeat_or_skip_stores() {
        let source: Vec<_> = (0..360).map(|i| (i * 37 % 256) as u8).collect();
        for grant in [1, 7, 503] {
            let mut work = ScreenBufferWork::new(source.clone()).unwrap();
            let mut tiles = [0; 360];
            let mut bus = Bus {
                cycles: 0,
                grant,
                bg: 1,
                writes: Vec::new(),
            };
            loop {
                match work.step(&mut bus, &mut tiles) {
                    Ok(true) => (),
                    Ok(false) => break,
                    Err(()) => {
                        let previous = (work.clone(), tiles, bus.cycles, bus.writes.clone());
                        assert_eq!(work.step(&mut bus, &mut tiles), Err(()));
                        assert_eq!(
                            (work.clone(), tiles, bus.cycles, bus.writes.clone()),
                            previous
                        );
                        work =
                            serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
                        bus.grant = grant;
                    }
                }
            }
            assert_eq!(tiles.as_slice(), source);
            assert_eq!(bus.cycles, 18_908);
            assert_eq!(bus.bg, 1);
            assert_eq!(bus.writes.len(), 2);
        }
        assert!(ScreenBufferWork::new(vec![0; 359]).is_none());
    }

    #[test]
    fn child_dispatch_preserves_every_preemption_boundary_and_yields_before_auto_bg_enable() {
        let source: Vec<_> = (0..360).map(|i| (i * 37 % 256) as u8).collect();
        for grant in [1, 2, 7, 503, usize::MAX] {
            let mut work = ScreenBufferWork::new(source.clone()).unwrap();
            let mut reference = work.clone();
            let mut tiles = [0; 360];
            let mut expected = tiles;
            let mut bus = Bus {
                cycles: 0,
                grant,
                bg: 1,
                writes: Vec::new(),
            };
            let mut stepped = Bus {
                cycles: 0,
                grant: usize::MAX,
                bg: 1,
                writes: Vec::new(),
            };
            let mut yielded = false;
            loop {
                let result = work.run(&mut bus, &mut tiles);
                while stepped.cycles < bus.cycles {
                    assert!(reference.step(&mut stepped, &mut expected).unwrap());
                }
                assert_eq!(
                    (&work, tiles, bus.cycles, bus.bg, &bus.writes),
                    (
                        &reference,
                        expected,
                        stepped.cycles,
                        stepped.bg,
                        &stepped.writes
                    )
                );
                match result {
                    Ok(true) => {
                        assert!(!yielded);
                        assert_eq!(bus.bg, 0);
                        assert_eq!(work.copied_bytes(), 360);
                        assert_eq!(tiles.as_slice(), source);
                        yielded = true;
                    }
                    Ok(false) => break,
                    Err(()) => bus.grant = grant,
                }
            }
            assert!(yielded && work.finished());
            assert_eq!(bus.cycles, 18_908);
        }
    }
}

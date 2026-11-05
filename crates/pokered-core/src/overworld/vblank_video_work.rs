//! Common VBlank video prefix: hardware entry through PrepareOAMData RET.
//! BodyReady explicitly leaves the interrupt active. Random/music/playtime/
//! input/epilogue belong to the remaining ISR owner, before a real RETI.
use super::{
    auto_bg_transfer_work::AutoBgTransferWork,
    bg_redraw_work::BgRedrawWork,
    display_delay_work::DelayFrameBus,
    display_dma_work::DisplayDmaWork,
    display_interrupt_entry_work::VBlankEntryWork,
    display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus},
    display_work_time::{DisplayWorkError, DisplayWorkTime},
    far_pattern_copy_work::{LOADED_ROM_BANK, ROM_BANK_SELECT},
    field_restore::{
        CopyFormat, PatternAssets, PatternMemory, RestoreError, VideoCopyWork, VideoServiceError,
    },
    moving_bg_work::MovingBgWork,
    prepare_oam_work::PrepareOamWork,
    queued_bg_copy_work::QueuedBgCopyWork,
    vblank_pattern_work::VBlankPatternWork,
    vblank_prologue_work::{VBlankPrologueWork, VBlankVideoShadows},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase {
    Entry,
    Prologue,
    AutoBg,
    CallQueued,
    Queued,
    CallRedraw,
    Redraw,
    CallPattern,
    Pattern,
    CallDouble,
    Double,
    CallMoving,
    Moving,
    CallDma,
    Dma,
    PrepareBank(u8),
    Prepare,
    BodyReady,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct VBlankVideoWork {
    phase: Phase,
    entry: VBlankEntryWork,
    prologue: VBlankPrologueWork,
    auto_bg: AutoBgTransferWork,
    queued: QueuedBgCopyWork,
    redraw: BgRedrawWork,
    pattern: VBlankPatternWork,
    double: VBlankPatternWork,
    moving: MovingBgWork,
    dma: DisplayDmaWork,
    prepare: PrepareOamWork,
    #[serde(default)]
    foreground_copy: Option<(u64, VideoCopyWork)>,
    #[serde(default)]
    entry_returns: Option<u64>,
}
/// Prepared logical inputs from the native video producer. Construction does
/// not transfer tiles, prepare/publish OAM, or acknowledge an interrupt.
pub struct VBlankVideoInputs {
    pub shadows: VBlankVideoShadows,
    pub auto_bg: AutoBgTransferWork,
    pub queued: QueuedBgCopyWork,
    pub redraw: BgRedrawWork,
    pub pattern: VBlankPatternWork,
    pub double: VBlankPatternWork,
    pub moving: MovingBgWork,
    pub prepare: PrepareOamWork,
}
impl VBlankVideoWork {
    pub fn select(ime: bool, pending: u8, input: VBlankVideoInputs) -> Option<Self> {
        if !input.pattern.unstarted() || !input.double.unstarted()
            || input.pattern.format() != CopyFormat::TwoBpp
            || input.double.format() != CopyFormat::OneBppDouble
        {
            return None;
        }
        Some(Self {
            phase: Phase::Entry,
            entry: VBlankEntryWork::select(ime, pending)?,
            prologue: VBlankPrologueWork::new(input.shadows),
            auto_bg: input.auto_bg,
            queued: input.queued,
            redraw: input.redraw,
            pattern: input.pattern,
            double: input.double,
            moving: input.moving,
            dma: DisplayDmaWork::default(),
            prepare: input.prepare,
            foreground_copy: None,
            entry_returns: None,
        })
    }
    /// Move the actual foreground request under the shared video owner.
    /// Failed validation leaves both the owner and request untouched.
    pub fn attach_foreground_copy(
        &mut self,
        epoch: u64,
        copy: &mut Option<VideoCopyWork>,
    ) -> Result<(), RestoreError> {
        if self.phase != Phase::Entry
            || self.entry_returns.is_some()
            || self.foreground_copy.is_some()
        {
            return Err(RestoreError::ServiceInProgress);
        }
        let request = copy.as_ref().ok_or(RestoreError::NoRequest)?;
        let pending = request.pending_vblank_copy(epoch)?;
        // Match the slot requested by this producer. The other live
        // request in VBlankVideoInputs still executes in source order.
        let slot = if pending.format == CopyFormat::TwoBpp {
            &mut self.pattern
        } else {
            &mut self.double
        };
        // Live ports already have a foreground owner. Attaching the legacy
        // protocol would create a second request and bypass those port writes.
        if slot.uses_live_ports() || slot.request() != Some(pending) {
            return Err(RestoreError::ConflictingRequest);
        }
        self.foreground_copy = Some((epoch, copy.take().unwrap()));
        Ok(())
    }
    /// Reclaim and resume the foreground caller only after the real ISR RETI.
    /// Pattern size clear, pattern RET and PrepareOAM RET are insufficient.
    pub fn resume_foreground_copy(
        &mut self,
        time: &DisplayWorkTime,
    ) -> Result<VideoCopyWork, RestoreError> {
        if !self.body_ready()
            || time.active_vblank()
            || self
                .entry_returns
                .is_none_or(|n| time.interrupt_returns() <= n)
        {
            return Err(RestoreError::NotServiced);
        }
        let (_, copy) = self
            .foreground_copy
            .as_mut()
            .ok_or(RestoreError::NoRequest)?;
        let completed = *copy.vblank_work().ok_or(RestoreError::NotServiced)?;
        copy.resume()?;
        if copy.format() == CopyFormat::TwoBpp {
            self.pattern = completed;
        } else {
            self.double = completed;
        }
        Ok(self.foreground_copy.take().unwrap().1)
    }
    pub fn body_ready(&self) -> bool {
        self.phase == Phase::BodyReady
    }
    pub fn random_entry_carry(&self) -> Option<bool> {
        if self.body_ready() {
            self.prepare.return_carry()
        } else {
            None
        }
    }
    pub fn saved_bank(&self) -> Option<u8> {
        self.prologue.saved_bank()
    }
    pub fn next_bg_portion(&self) -> u8 {
        self.auto_bg.next_portion()
    }
    pub fn bg_request(&self) -> super::queued_bg_copy_work::BgCopyRequest {
        self.queued.request()
    }
    pub fn pattern_work(&self) -> &VBlankPatternWork {
        self.foreground_copy
            .as_ref()
            .filter(|(_, c)| c.format() == CopyFormat::TwoBpp)
            .and_then(|(_, c)| c.vblank_work())
            .unwrap_or(&self.pattern)
    }
    pub fn double_work(&self) -> &VBlankPatternWork {
        self.foreground_copy
            .as_ref()
            .filter(|(_, c)| c.format() == CopyFormat::OneBppDouble)
            .and_then(|(_, c)| c.vblank_work())
            .unwrap_or(&self.double)
    }
    pub fn redraw_mode(&self) -> u8 {
        self.redraw.mode()
    }
    pub fn moving_state(&self) -> super::moving_bg_work::MovingBgState {
        self.moving.state()
    }
    pub fn prepared_oam(&self) -> &super::prepare_oam_work::PrepareOamState {
        self.prepare.state()
    }
    /// Run the same children to their next real budget/error boundary while
    /// retaining each child's exclusive bus lease. No instruction, CALL, DMA
    /// or RET is replaced by an aggregate duration. BodyReady is not RETI.
    pub fn run_until_body_ready<A: PatternAssets>(
        &mut self,
        time: &mut DisplayWorkTime,
        memory: &mut PatternMemory,
        assets: &A,
    ) -> Result<(), DisplayWorkError> {
        use Phase::*;
        loop {
            self.phase = match self.phase {
                Prologue => {
                    let mut bus = time.display_bus();
                    while self.prologue.step(&mut bus)? {}
                    AutoBg
                }
                AutoBg => {
                    let mut bus = time.pattern_bus(memory, assets);
                    while self.auto_bg.step(&mut bus)? {}
                    CallQueued
                }
                Queued => {
                    let mut bus = time.pattern_bus(memory, assets);
                    while self.queued.step(&mut bus)? {}
                    CallRedraw
                }
                Redraw => {
                    let mut bus = time.pattern_bus(memory, assets);
                    while self.redraw.step(&mut bus)? {}
                    CallPattern
                }
                Pattern | Double if self.foreground_copy.as_ref().is_none_or(|(_, copy)| {
                    copy.format() != if self.phase == Pattern { CopyFormat::TwoBpp } else { CopyFormat::OneBppDouble }
                }) => {
                    let child = if self.phase == Pattern { &mut self.pattern } else { &mut self.double };
                    let mut bus = time.pattern_bus(memory, assets);
                    child.run(&mut bus)?;
                    if self.phase == Pattern { CallDouble } else { CallMoving }
                }
                Moving => {
                    let mut bus = time.pattern_bus(memory, assets);
                    while self.moving.step(&mut bus)? {}
                    CallDma
                }
                Dma => {
                    let mut bus = time.display_bus();
                    while self.dma.step(&mut bus)?.is_some() {}
                    PrepareBank(0)
                }
                Prepare => {
                    let mut bus = time.display_bus();
                    self.prepare.run(&mut bus)?;
                    BodyReady
                }
                BodyReady => return Ok(()),
                _ => {
                    // Acceptance and legacy foreground-request ownership keep
                    // their existing checked single-instruction protocol.
                    self.step(time, memory, assets)?;
                    continue;
                }
            };
        }
    }
    pub fn step<A: PatternAssets>(
        &mut self,
        time: &mut DisplayWorkTime,
        memory: &mut PatternMemory,
        assets: &A,
    ) -> Result<bool, DisplayWorkError> {
        use Phase::*;
        loop {
            self.phase = match self.phase {
                Entry => {
                    if self.entry_returns.is_none() {
                        if self
                            .foreground_copy
                            .as_ref()
                            .is_some_and(|(epoch, _)| *epoch != time.device().vblank_epoch())
                        {
                            return Err(DisplayWorkError::Resource(RestoreError::StaleVBlank));
                        }
                        self.entry_returns = Some(time.interrupt_returns());
                    }
                    if self.entry.step(time)? {
                        return Ok(true);
                    }
                    Prologue
                }
                Prologue => {
                    if self.prologue.step(time)? {
                        return Ok(true);
                    }
                    AutoBg
                }
                AutoBg => {
                    if self.auto_bg.step(&mut time.pattern_bus(memory, assets))? {
                        return Ok(true);
                    }
                    CallQueued
                }
                Queued => {
                    if self.queued.step(&mut time.pattern_bus(memory, assets))? {
                        return Ok(true);
                    }
                    CallRedraw
                }
                Redraw => {
                    if self.redraw.step(&mut time.pattern_bus(memory, assets))? {
                        return Ok(true);
                    }
                    CallPattern
                }
                Pattern | Double => {
                    let format = if self.phase == Pattern {
                        CopyFormat::TwoBpp
                    } else {
                        CopyFormat::OneBppDouble
                    };
                    if let Some((epoch, copy)) = self
                        .foreground_copy
                        .as_mut()
                        .filter(|(_, copy)| copy.format() == format)
                    {
                        match copy
                            .service_instruction(*epoch, &mut time.pattern_bus(memory, assets))
                        {
                            Ok(done) => {
                                if done {
                                    self.phase = if format == CopyFormat::TwoBpp {
                                        CallDouble
                                    } else {
                                        CallMoving
                                    };
                                }
                                // The child's RET is already an instruction.
                                // Do not execute the next CALL in this step.
                                return Ok(true);
                            }
                            Err(VideoServiceError::Bus(error)) => return Err(error),
                            Err(VideoServiceError::Protocol(error)) => {
                                return Err(DisplayWorkError::Resource(error))
                            }
                        }
                    } else {
                        let child = if self.phase == Pattern {
                            &mut self.pattern
                        } else {
                            &mut self.double
                        };
                        if child.step(&mut time.pattern_bus(memory, assets))?.is_some() {
                            return Ok(true);
                        }
                    }
                    if self.phase == Pattern {
                        CallDouble
                    } else {
                        CallMoving
                    }
                }
                Moving => {
                    if self.moving.step(&mut time.pattern_bus(memory, assets))? {
                        return Ok(true);
                    }
                    CallDma
                }
                Dma => {
                    if self.dma.step(time)?.is_some() {
                        return Ok(true);
                    }
                    PrepareBank(0)
                }
                Prepare => {
                    if self.prepare.step(time)?.is_some() {
                        return Ok(true);
                    }
                    BodyReady
                }
                BodyReady => return Ok(false),
                p @ (CallQueued | CallRedraw | CallPattern | CallDouble | CallMoving | CallDma) => {
                    time.execute(DisplayInstruction {
                        tcycles: 24,
                        access: None,
                    })?;
                    self.phase = match p {
                        CallQueued => Queued,
                        CallRedraw => Redraw,
                        CallPattern => Pattern,
                        CallDouble => Double,
                        CallMoving => Moving,
                        CallDma => Dma,
                        _ => unreachable!(),
                    };
                    return Ok(true);
                }
                PrepareBank(n) => {
                    // ld a,1; ldh [hLoadedROMBank],a; ld [rROMB],a; call PrepareOAMData
                    let (tcycles, access) = match n {
                        0 => (8, None),
                        1 => (
                            12,
                            Some(DisplayAccess::Write {
                                port: LOADED_ROM_BANK,
                                value: 1,
                                at_tcycle: 8,
                            }),
                        ),
                        2 => (
                            16,
                            Some(DisplayAccess::Write {
                                port: ROM_BANK_SELECT,
                                value: 1,
                                at_tcycle: 12,
                            }),
                        ),
                        3 => (24, None),
                        _ => unreachable!(),
                    };
                    time.execute(DisplayInstruction { tcycles, access })?;
                    self.phase = if n == 3 { Prepare } else { PrepareBank(n + 1) };
                    return Ok(true);
                }
            };
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::alloc_prelude::*;
    use crate::overworld::{
        display_delay_work::DelayFrameBus,
        field_restore::{PatternCopy, PatternSource},
        moving_bg_work::MovingBgState,
        prepare_oam_work::PrepareOamState,
        queued_bg_copy_work::BgCopyRequest,
    };
    struct NoAssets;
    impl PatternAssets for NoAssets {
        fn read_pattern_byte(&self, _: PatternSource, _: u16) -> Option<u8> {
            None
        }
    }
    pub(crate) fn build() -> (
        DisplayWorkTime,
        PatternMemory,
        VBlankVideoWork,
        u64,
        serde_json::Value,
    ) {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("auto_bg_real_case_1136.json")).unwrap();
        let leaf: serde_json::Value =
            serde_json::from_str(include_str!("vblank_video_case_1139.json")).unwrap();
        let o: serde_json::Value =
            serde_json::from_str(include_str!("prepare_oam_cases_1113.json")).unwrap();
        let case = o["cases"][leaf["prepare_case_index"].as_u64().unwrap() as usize].clone();
        let get = |k: &str| v["state"][k].as_u64().unwrap();
        let tiles: Vec<u8> = serde_json::from_value(v["state"]["tiles"].clone()).unwrap();
        let before: Vec<u8> = serde_json::from_value(v["bg_maps_before"].clone()).unwrap();
        let mut bytes = vec![0; 0x2000];
        bytes[0x1800..].copy_from_slice(&before);
        let memory = PatternMemory::from_bytes(bytes).unwrap();
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        for (port, value) in [
            (0xffff, 0),
            (
                LOADED_ROM_BANK,
                v["prologue"]["loaded_bank"].as_u64().unwrap() as u8,
            ),
        ] {
            time.execute(DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port,
                    value,
                    at_tcycle: 8,
                }),
            })
            .unwrap();
        }
        time.idle_to_target().unwrap();
        time.begin_native_update();
        time.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
        time.execute(DisplayInstruction {
            tcycles: 12,
            access: Some(DisplayAccess::Write {
                port: 0xffff,
                value: 1,
                at_tcycle: 8,
            }),
        })
        .unwrap();
        let shadow: Vec<u8> = serde_json::from_value(case["before"]["shadow_oam"].clone()).unwrap();
        time.set_shadow_oam(&shadow).unwrap();
        let pattern = PatternCopy {
            source: PatternSource::WalkingPlayer,
            source_offset: 0,
            destination: 0x8000,
            tiles: leaf["pattern_tiles"].as_u64().unwrap() as u8,
            format: CopyFormat::TwoBpp,
        };
        let request: BgCopyRequest = serde_json::from_value(v["next_bg_request"].clone()).unwrap();
        let input = VBlankVideoInputs {
            shadows: serde_json::from_value(v["prologue"].clone()).unwrap(),
            auto_bg: AutoBgTransferWork::new(
                get("enabled") as u8,
                get("portion") as u8,
                get("destination") as u16,
                get("sp") as u16,
                tiles,
            )
            .unwrap(),
            queued: QueuedBgCopyWork::new(request, get("sp") as u16, vec![]).unwrap(),
            redraw: BgRedrawWork::new(0, 0, vec![]).unwrap(),
            pattern: VBlankPatternWork::new(pattern),
            double: VBlankPatternWork::new(PatternCopy {
                format: CopyFormat::OneBppDouble,
                tiles: leaf["double_tiles"].as_u64().unwrap() as u8,
                ..pattern
            }),
            moving: MovingBgWork::new(
                serde_json::from_value(leaf["moving"].clone()).unwrap(),
                [[0; 16]; 3],
            ),
            prepare: PrepareOamWork::new(serde_json::from_value(case["before"].clone()).unwrap())
                .unwrap(),
        };
        let work = VBlankVideoWork::select(true, time.device().pending_interrupts(), input).unwrap();
        let start = time.elapsed();
        assert_eq!(time.interrupt_returns(), 0);
        assert!(!time.active_vblank());
        (time, memory, work, start, case)
    }
    #[test]
    fn leased_video_runner_matches_single_steps_at_budget_and_snapshot_boundaries() {
        struct Bytes;
        impl PatternAssets for Bytes {
            fn read_pattern_byte(&self, _: PatternSource, offset: u16) -> Option<u8> {
                Some((offset as u8).wrapping_mul(7))
            }
        }
        for tiles in [0, 1, 8] {
            for budget in (0..=12_000u64).step_by(113).chain([4, 12, 20, 24, 65535]) {
                let (time, memory, mut work, _, _) = build();
                for (format, destination) in [(CopyFormat::TwoBpp, 0x8200), (CopyFormat::OneBppDouble, 0x8800)] {
                    let copy = VBlankPatternWork::new(PatternCopy {
                        source: PatternSource::WalkingPlayer,
                        source_offset: 3,
                        destination,
                        tiles,
                        format,
                    });
                    if format == CopyFormat::TwoBpp { work.pattern = copy; } else { work.double = copy; }
                }
                let mut state = serde_json::to_value(&time).unwrap();
                state["deadline"] = (time.elapsed() + budget).into();
                let mut time: DisplayWorkTime = serde_json::from_value(state).unwrap();
                let (mut reference, mut expected_memory, mut expected_work) = (time.clone(), memory.clone(), work.clone());
                let mut memory = memory;
                for _ in 0..2 {
                    let actual = work.run_until_body_ready(&mut time, &mut memory, &Bytes);
                    let expected = loop {
                        match expected_work.step(&mut reference, &mut expected_memory, &Bytes) {
                            Ok(true) => (),
                            Ok(false) => break Ok(()),
                            Err(error) => break Err(error),
                        }
                    };
                    assert_eq!(actual, expected, "tiles={tiles} budget={budget}");
                    assert_eq!(serde_json::to_value((&time, &memory, &work)).unwrap(),
                        serde_json::to_value((&reference, &expected_memory, &expected_work)).unwrap());
                    assert_eq!(time.palette_store_receipts(), reference.palette_store_receipts());
                    if actual.is_ok() {
                        assert!(work.body_ready());
                        assert!(time.active_vblank());
                        assert_eq!(time.interrupt_returns(), 0, "video completion must not return the IRQ");
                        break;
                    }
                    assert_eq!(actual, Err(DisplayWorkError::Budget));
                    (time, memory, work) = serde_json::from_value(serde_json::to_value((&time, &memory, &work)).unwrap()).unwrap();
                    (reference, expected_memory, expected_work) = serde_json::from_value(serde_json::to_value((&reference, &expected_memory, &expected_work)).unwrap()).unwrap();
                    time.begin_native_update();
                    reference.begin_native_update();
                }
                assert!(work.body_ready());
            }
        }
    }
    #[test]
    fn common_video_owner_matches_real_pending_lcd_off_irq_through_prepare_ret_without_reti() {
        let (mut t, mut mem, mut w, start, case) = build();
        assert_eq!(w.random_entry_carry(), None);
        while w.step(&mut t, &mut mem, &NoAssets).unwrap() {}
        assert!(w.body_ready());
        // Passive original Random entry has F=$c0 (carry clear).
        assert_eq!(w.random_entry_carry(), Some(false));
        assert_eq!(t.elapsed() - start, 10716);
        assert!(t.active_vblank());
        assert_eq!(t.interrupt_returns(), 0);
        assert_eq!(w.saved_bank(), Some(4));
        assert_eq!(t.source_loaded_rom_bank(), 1);
        assert_eq!(t.selected_rom_bank(), 1);
        assert_eq!(w.next_bg_portion(), 1);
        assert_eq!(
            w.moving_state(),
            MovingBgState {
                animations: 1,
                counter1: 19,
                counter2: 0
            }
        );
        let expected: PrepareOamState = serde_json::from_value(case["after"].clone()).unwrap();
        assert_eq!(w.prepared_oam(), &expected);
        let shadow: Vec<u8> = serde_json::from_value(case["after"]["shadow_oam"].clone()).unwrap();
        assert_eq!(t.shadow_oam(), shadow);
        assert!(shadow.iter().any(|&b| b != 0));
        assert!(
            t.device().oam().iter().all(|&b| b == 0),
            "DMA occurs before PrepareOAM; new shadow must not publish early"
        );
        let end = t.elapsed();
        assert!(!w.step(&mut t, &mut mem, &NoAssets).unwrap());
        assert_eq!(t.elapsed(), end);
        assert_eq!(t.interrupt_returns(), 0);
    }
    #[test]
    fn common_video_budget_split_and_mid_dma_snapshot_preserve_single_transfer_and_saved_bank() {
        let (mut t, mut mem, mut w, start, case) = build();
        while w.phase != Phase::Dma || !t.device().dma_active() {
            assert!(w.step(&mut t, &mut mem, &NoAssets).unwrap());
        }
        let snapshot = serde_json::to_string(&(&t, &mem, &w)).unwrap();
        let (mut ot, mut om, mut ow): (DisplayWorkTime, PatternMemory, VBlankVideoWork) =
            serde_json::from_str(&snapshot).unwrap();
        // A grant boundary is not an ISR/child return. Idle explicitly fills
        // the grant here; both copies must retain their interrupted work.
        t.idle_to_target().unwrap();
        ot.idle_to_target().unwrap();
        let dt = t.elapsed();
        assert_eq!(
            w.step(&mut t, &mut mem, &NoAssets),
            Err(DisplayWorkError::Budget)
        );
        assert_eq!(t.elapsed(), dt);
        assert!(t.active_vblank());
        assert_eq!(t.interrupt_returns(), 0);
        t.begin_native_update();
        ot.begin_native_update();
        while w.step(&mut t, &mut mem, &NoAssets).unwrap() {}
        while ow.step(&mut ot, &mut om, &NoAssets).unwrap() {}
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            serde_json::to_value(&ow).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&t).unwrap(),
            serde_json::to_value(&ot).unwrap()
        );
        assert_eq!(mem.bytes(), om.bytes());
        assert!(t.elapsed() - start > 10716);
        let expected: Vec<u8> =
            serde_json::from_value(case["after"]["shadow_oam"].clone()).unwrap();
        assert_eq!(t.shadow_oam(), expected);
        assert_eq!(t.interrupt_returns(), 0);
    }
    #[test]
    fn foreground_requests_share_the_video_owner_and_cannot_resume_at_size_clear_or_pattern_ret() {
        struct Bytes;
        impl PatternAssets for Bytes {
            fn read_pattern_byte(&self, _: PatternSource, offset: u16) -> Option<u8> {
                Some((offset as u8).wrapping_mul(7).wrapping_add(3))
            }
        }
        for format in [CopyFormat::TwoBpp, CopyFormat::OneBppDouble] {
            for tiles in [0, 1, 8, 12] {
                let (mut time, mut memory, mut owner, _, _) = build();
                let mut producer = super::super::field_restore::VideoCopyWork::new(PatternCopy {
                    source: PatternSource::WalkingPlayer,
                    source_offset: 3,
                    destination: 0x8200,
                    tiles,
                    format,
                });
                let epoch = time.device().vblank_epoch();
                producer.arm(epoch - 1).unwrap();
                let pending = producer.pending_vblank_copy(epoch).unwrap();
                if format == CopyFormat::TwoBpp {
                    owner.pattern = VBlankPatternWork::new(pending);
                } else {
                    owner.double = VBlankPatternWork::new(pending);
                }
                let mut producer = Some(producer);
                owner.attach_foreground_copy(epoch, &mut producer).unwrap();
                assert!(producer.is_none(), "the ISR owns the actual pending caller");
                let mut snapshot = false;
                while {
                    let before = time.elapsed();
                    let more = owner.step(&mut time, &mut memory, &Bytes).unwrap();
                    assert!(
                        time.elapsed() - before <= 24,
                        "one step must not execute two instructions"
                    );
                    more
                } {
                    let copy = &owner.foreground_copy.as_ref().unwrap().1;
                    if copy.requested_tiles() == Some(0) && !snapshot {
                        assert_eq!(
                            owner.resume_foreground_copy(&time),
                            Err(RestoreError::NotServiced)
                        );
                        // Persist the partially executed request with its one
                        // child; do not reconstruct it from scene constants.
                        let saved = serde_json::to_vec(&(&time, &memory, &owner)).unwrap();
                        let restored: (DisplayWorkTime, PatternMemory, VBlankVideoWork) =
                            serde_json::from_slice(&saved).unwrap();
                        (time, memory, owner) = restored;
                        snapshot = true;
                    }
                }
                assert!(snapshot);
                assert!(time.active_vblank());
                assert_eq!(time.interrupt_returns(), 0);
                assert_eq!(
                    owner.resume_foreground_copy(&time),
                    Err(RestoreError::NotServiced)
                );
                let copy = &owner.foreground_copy.as_ref().unwrap().1;
                assert_eq!(copy.requested_tiles(), Some(0));
                assert!(!copy.finished());
                assert!(copy.vblank_work().unwrap().finished());
                let stride = if format == CopyFormat::TwoBpp { 1 } else { 2 };
                for n in 0..usize::from(tiles.min(8)) * 16 {
                    let offset = 3 + n / stride;
                    assert_eq!(
                        memory.bytes()[0x200 + n],
                        (offset as u8).wrapping_mul(7).wrapping_add(3)
                    );
                }
                // Isolated ownership-contract control: execute the real
                // epilogue instructions/RETI, rather than forging a return
                // counter. This is NOT a complete ISR or Game timing proof;
                // the runtime must execute music/timer/input before this tail.
                let mut epilogue = super::super::vblank_return_work::VBlankReturnWork::new(
                    owner.saved_bank().unwrap(),
                );
                while epilogue.step(&mut time).unwrap() {}
                assert_eq!(time.interrupt_returns(), 1);
                let mut resumed = owner.resume_foreground_copy(&time).unwrap();
                assert_eq!(resumed.finished(), tiles < 8);
                assert!(if format == CopyFormat::TwoBpp {
                    owner.pattern_work().finished()
                } else {
                    owner.double_work().finished()
                });
                assert_eq!(
                    owner.resume_foreground_copy(&time),
                    Err(RestoreError::NoRequest)
                );
                if tiles >= 8 {
                    resumed.arm(epoch).unwrap();
                    let next = resumed.pending_vblank_copy(epoch + 1).unwrap();
                    assert_eq!(next.tiles, tiles - 8);
                    assert_eq!(next.source_offset, 3 + 8 * format.source_bytes_per_tile());
                    assert_eq!(next.destination, 0x8280);
                }
            }
        }
    }

    #[test]
    fn mismatched_or_stale_foreground_requests_are_not_consumed() {
        let (mut time, mut memory, mut owner, _, _) = build();
        let epoch = time.device().vblank_epoch();
        let mut producer = VideoCopyWork::new(PatternCopy {
            source: PatternSource::Font,
            source_offset: 3,
            destination: 0x8200,
            tiles: 8,
            format: CopyFormat::TwoBpp,
        });
        producer.arm(epoch).unwrap();
        let mut producer = Some(producer);
        let before = producer.clone();
        assert_eq!(
            owner.attach_foreground_copy(epoch, &mut producer),
            Err(RestoreError::StaleVBlank)
        );
        assert_eq!(producer, before);
        assert_eq!(
            owner.attach_foreground_copy(epoch + 1, &mut producer),
            Err(RestoreError::ConflictingRequest)
        );
        assert_eq!(producer, before);
        let pending = producer
            .as_ref()
            .unwrap()
            .pending_vblank_copy(epoch + 1)
            .unwrap();
        owner.pattern = VBlankPatternWork::new(pending);
        owner
            .attach_foreground_copy(epoch + 1, &mut producer)
            .unwrap();
        let before_time = time.elapsed();
        assert_eq!(
            owner.step(&mut time, &mut memory, &NoAssets),
            Err(DisplayWorkError::Resource(RestoreError::StaleVBlank))
        );
        assert_eq!(time.elapsed(), before_time);
        assert!(!time.active_vblank());
    }
}

//! Bounded native PrepareOAMData expansion from engine/gfx/sprite_oam.asm.
//! Frozen IRQ inputs select source branches. Each emitted source instruction
//! executes on the common bus; shadow-OAM writes occur at their bus access,
//! never at expansion time. This is not an aggregate OAM duration surcharge.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};
use crate::alloc_prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OamSprite {
    pub picture: u8,
    pub image: u8,
    pub y: u8,
    pub x: u8,
    pub grass: u8,
    pub adjusted_y: u8,
    pub adjusted_x: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PrepareOamState {
    pub enabled: u8,
    pub movement_flags: u8,
    pub sprites: Vec<OamSprite>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareOamError {
    InvalidSpriteRecords,
    ShadowPageOverflow,
}
// Most entries are plain cycles. Decode bus accesses only at execution so the
// retained instruction uses three bytes even on GBA.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
enum Instruction {
    Plain(u8),
    WriteOam { offset: u8, value: u8 },
    Enabled(u8),
    AdjustY { slot: u8, value: u8 },
    AdjustX { slot: u8, value: u8 },
}
impl Instruction {
    fn bus(self) -> DisplayInstruction {
        match self {
            Self::Plain(tcycles) => DisplayInstruction {
                tcycles,
                access: None,
            },
            Self::WriteOam { offset, value } => DisplayInstruction {
                tcycles: 8,
                access: Some(DisplayAccess::Write {
                    port: 0xc300 + u16::from(offset),
                    value,
                    at_tcycle: 4,
                }),
            },
            Self::Enabled(value) => DisplayInstruction {
                tcycles: 16,
                access: Some(DisplayAccess::Write {
                    port: super::sprite_update_control_work::SPRITE_UPDATES_PORT,
                    value,
                    at_tcycle: 12,
                }),
            },
            Self::AdjustY { .. } | Self::AdjustX { .. } => DisplayInstruction {
                tcycles: 8,
                access: None,
            },
        }
    }
}

trait InstructionSink {
    fn push(&mut self, instruction: Instruction);
    fn plain(&mut self, cycles: &[u8]);
}
impl InstructionSink for usize {
    fn push(&mut self, _: Instruction) {
        *self += 1;
    }
    fn plain(&mut self, cycles: &[u8]) {
        *self += cycles.len();
    }
}
impl InstructionSink for Vec<Instruction> {
    fn push(&mut self, instruction: Instruction) {
        Vec::push(self, instruction);
    }
    fn plain(&mut self, cycles: &[u8]) {
        self.extend(cycles.iter().copied().map(Instruction::Plain));
    }
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PrepareOamWork {
    state: PrepareOamState,
    code: Vec<Instruction>,
    next: usize,
    return_carry: bool,
}
fn emit(p: &mut impl InstructionSink, cycles: &[u8]) {
    p.plain(cycles);
}
fn write(p: &mut impl InstructionSink, offset: usize, value: u8) {
    debug_assert!(offset < 160);
    p.push(Instruction::WriteOam {
        offset: offset as u8,
        value,
    });
}
fn screen_xy(p: &mut impl InstructionSink, slot: u8, sprite: OamSprite) {
    emit(p, &[4, 4, 8, 12, 4, 4, 8, 12, 8, 4, 4, 12, 8, 8]);
    p.push(Instruction::AdjustY {
        slot,
        value: sprite.y.wrapping_add(4) & 0xf0,
    });
    emit(p, &[4, 12, 8]);
    p.push(Instruction::AdjustX {
        slot,
        value: sprite.x & 0xf0,
    });
    emit(p, &[16]);
}
impl PrepareOamWork {
    /// Disabled control paths do not read actors or movement flags. Bind no
    /// actor snapshot at all; callers must not publish these absent fields as
    /// observed WRAM. Only the returned enabled flag/shadow writes are effects.
    pub fn control_only(enabled: u8) -> Result<Self, PrepareOamError> {
        if enabled == 1 {
            return Err(PrepareOamError::InvalidSpriteRecords);
        }
        Self::compile(PrepareOamState {
            enabled,
            movement_flags: 0,
            sprites: Vec::new(),
        })
    }
    pub fn new(state: PrepareOamState) -> Result<Self, PrepareOamError> {
        if state.sprites.len() != 16 {
            return Err(PrepareOamError::InvalidSpriteRecords);
        }
        Self::compile(state)
    }
    fn compile(state: PrepareOamState) -> Result<Self, PrepareOamError> {
        // Every enabled render path ends on CP L with equal pointers.
        // HideSprites (input 0) ends its last ADD HL,DE inside the shadow
        // page, with carry clear. Other disabled entries return immediately
        // from (enabled-1) < $ff with carry set.
        let return_carry = state.enabled > 1;
        let mut count = 0;
        Self::expand(&state, &mut count)?;
        let mut code = Vec::with_capacity(count);
        Self::expand(&state, &mut code)?;
        debug_assert_eq!(code.len(), count);
        Ok(Self {
            state,
            code,
            next: 0,
            return_carry,
        })
    }
    fn expand(
        state: &PrepareOamState,
        p: &mut impl InstructionSink,
    ) -> Result<(), PrepareOamError> {
        if state.enabled != 1 {
            emit(p, &[16, 4, 8, 8, if state.enabled == 0 { 8 } else { 20 }]);
            if state.enabled == 0 {
                p.push(Instruction::Enabled(0xff));
                emit(p, &[16]); // JP HideSprites
                emit(p, &[8, 12, 12, 8]);
                for slot in 0..40 {
                    write(p, slot * 4, 160);
                    emit(p, &[8, 4, if slot == 39 { 8 } else { 12 }]);
                }
                emit(p, &[16]);
            }
            return Ok(());
        }
        emit(p, &[16, 4, 12, 4, 12]);
        let mut used = 0usize;
        for (slot, &sprite) in state.sprites.iter().enumerate() {
            emit(
                p,
                &[
                    12,
                    8,
                    12,
                    4,
                    8,
                    4,
                    if sprite.picture == 0 { 16 } else { 12 },
                ],
            );
            if sprite.picture != 0 {
                emit(
                    p,
                    &[4, 4, 8, 16, 8, if sprite.image == 0xff { 8 } else { 12 }],
                );
                if sprite.image == 0xff {
                    emit(p, &[24]);
                    screen_xy(p, slot as u8, sprite);
                    emit(p, &[12]);
                } else {
                    if used + 16 > 160 {
                        return Err(PrepareOamError::ShadowPageOverflow);
                    }
                    let still = sprite.image >= 0xa0;
                    emit(p, &[8, if still { 8 } else { 12 }]);
                    if still {
                        emit(p, &[8, 8, 12]);
                    } else {
                        emit(p, &[8]);
                    }
                    // .next through CALL GetSpriteScreenXY, including table reads.
                    emit(
                        p,
                        &[
                            4, 16, 4, 4, 8, 4, 8, 8, 12, 12, 8, 12, 8, 8, 8, 8, 4, 8, 4, 8, 8, 4,
                            24,
                        ],
                    );
                    screen_xy(p, slot as u8, sprite);
                    emit(p, &[12, 4, 8]);
                    let low = sprite.image & 15;
                    let flip = !still && (low >= 12 || (low < 8 && low & 3 == 3));
                    let pattern = if still {
                        0
                    } else {
                        (if low < 4 {
                            0
                        } else if low < 8 {
                            4
                        } else {
                            8
                        }) + if low & 1 != 0 { 128 } else { 0 }
                    };
                    for tile in 0..4usize {
                        let y =
                            sprite
                                .y
                                .wrapping_add(16)
                                .wrapping_add(if tile >= 2 { 8 } else { 0 });
                        let x = sprite
                            .x
                            .wrapping_add(8)
                            .wrapping_add(if (tile & 1 != 0) ^ flip { 8 } else { 0 });
                        emit(p, &[12, 8, 8]);
                        write(p, used + tile * 4, y);
                        emit(p, &[8, 12, 8, 8, 4]);
                        write(p, used + tile * 4 + 1, x);
                        emit(
                            p,
                            &[
                                4,
                                8,
                                8,
                                16,
                                4,
                                16,
                                8,
                                8,
                                8,
                                if sprite.image >> 4 == 11 { 8 } else { 12 },
                            ],
                        );
                        if sprite.image >> 4 == 11 {
                            emit(p, &[8, 12]);
                        } else {
                            emit(p, &[8, 8, 4, 8, 4]);
                        }
                        emit(p, &[4, 12]);
                        let base = if sprite.image >> 4 == 11 {
                            124
                        } else {
                            (sprite.image >> 4) * 12
                        };
                        write(
                            p,
                            used + tile * 4 + 2,
                            base.wrapping_add(pattern).wrapping_add(tile as u8),
                        );
                        emit(p, &[8, 4, 8, 8, if tile >= 2 { 8 } else { 12 }]);
                        if tile >= 2 {
                            emit(p, &[12, 8]);
                        }
                        emit(p, &[8]);
                        let attrs = (if flip { 0x20 } else { 0 })
                            | (if tile >= 2 {
                                2 | sprite.grass & 0x80
                            } else {
                                0
                            })
                            | u8::from(tile == 3);
                        write(p, used + tile * 4 + 3, attrs);
                        emit(p, &[4, 8, if tile == 3 { 8 } else { 12 }]);
                    }
                    emit(p, &[4, 12]);
                    used += 16;
                }
            }
            emit(p, &[12, 8, 8, if slot == 15 { 12 } else { 16 }]);
        }
        let preserve = state.movement_flags & 0x40 != 0;
        emit(
            p,
            &[12, 4, 8, 12, 8, 16, 8, 8, if preserve { 8 } else { 12 }],
        );
        if preserve {
            emit(p, &[8]);
        }
        let end = if preserve { 144 } else { 160 };
        // Source can alias adjacent WRAM for overfull pages. This bounded
        // backend rejects that case explicitly; no all-state support claimed.
        if used > end {
            return Err(PrepareOamError::ShadowPageOverflow);
        }
        for offset in (used..end).step_by(4) {
            emit(p, &[4, 8]);
            write(p, offset, 160);
            emit(p, &[8, 12]);
        }
        emit(p, &[4, 20]);
        Ok(())
    }
    /// The next Random call receives this flag only after the actual RET.
    pub fn return_carry(&self) -> Option<bool> {
        self.finished().then_some(self.return_carry)
    }
    pub fn state(&self) -> &PrepareOamState {
        &self.state
    }
    pub fn finished(&self) -> bool {
        self.next == self.code.len()
    }
    /// Retain all observable instructions/effects and the exact cursor at a
    /// refused instruction. Only the bus can opt into an effect-free prefix.
    pub fn run<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<(), B::Error> {
        while !self.finished() {
            let count = bus.execute_plain_run(
                self.code[self.next..]
                    .iter()
                    .take_while(|i| matches!(i, Instruction::Plain(_)))
                    .map(|i| i.bus()),
            )?;
            self.next += count;
            if count == 0 {
                self.step(bus)?;
            }
        }
        Ok(())
    }
    pub fn step<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<Option<DisplayInstruction>, B::Error> {
        let Some(i) = self.code.get(self.next).copied() else {
            return Ok(None);
        };
        let instruction = i.bus();
        bus.execute(instruction)?;
        match i {
            Instruction::Enabled(value) => self.state.enabled = value,
            Instruction::AdjustY { slot, value } => {
                self.state.sprites[usize::from(slot)].adjusted_y = value
            }
            Instruction::AdjustX { slot, value } => {
                self.state.sprites[usize::from(slot)].adjusted_x = value
            }
            _ => (),
        }
        self.next += 1;
        Ok(Some(instruction))
    }
}

#[cfg(test)]
mod tests {
    use super::super::display_work_time::{DisplayWorkError, DisplayWorkTime};
    use super::*;
    #[test]
    fn instruction_storage_is_compact_and_preallocated() {
        for data in [
            include_str!("prepare_oam_cases_1113.json"),
            include_str!("prepare_oam_constructed_1113.json"),
            include_str!("prepare_oam_live_cases_1158.json"),
        ] {
            let fixture: serde_json::Value = serde_json::from_str(data).unwrap();
            for case in fixture["cases"].as_array().unwrap() {
                let input = case.get("input").unwrap_or(&case["before"]);
                let work =
                    PrepareOamWork::new(serde_json::from_value(input.clone()).unwrap()).unwrap();
                assert_eq!(work.code.capacity(), work.code.len());
                assert!(core::mem::size_of::<Instruction>() <= 3);
            }
        }
    }
    #[test]
    fn irq_runner_preserves_each_oam_effect_budget_cursor_and_snapshot_resume() {
        for data in [include_str!("prepare_oam_cases_1113.json"), include_str!("prepare_oam_constructed_1113.json")] {
            let fixture: serde_json::Value = serde_json::from_str(data).unwrap();
            for case in fixture["cases"].as_array().unwrap() {
                let state: PrepareOamState = serde_json::from_value(case["before"].clone()).unwrap();
                let template = PrepareOamWork::new(state).unwrap();
                let shadow: Vec<u8> = serde_json::from_value(case["before"]["shadow_oam"].clone()).unwrap();
                for budget in (0..4096u64).step_by(97).chain([4, 12, 20, 65535]) {
                    let mut time = DisplayWorkTime::default();
                    time.begin_native_update();
                    time.device_mut().request_interrupt(1);
                    time.enter_pending_vblank().unwrap();
                    time.set_shadow_oam(&shadow).unwrap();
                    let mut value = serde_json::to_value(&time).unwrap();
                    value["deadline"] = (time.elapsed() + budget).into();
                    let mut time: DisplayWorkTime = serde_json::from_value(value).unwrap();
                    let mut reference = time.clone();
                    let (mut work, mut expected_work) = (template.clone(), template.clone());
                    for _ in 0..2 {
                        let actual = work.run(&mut time.display_bus());
                        let expected = loop {
                            match expected_work.step(&mut reference) {
                                Ok(Some(_)) => (),
                                Ok(None) => break Ok(()),
                                Err(error) => break Err(error),
                            }
                        };
                        assert_eq!(actual, expected);
                        assert_eq!(serde_json::to_value((&time, &work)).unwrap(), serde_json::to_value((&reference, &expected_work)).unwrap());
                        if actual.is_ok() { break; }
                        assert_eq!(actual, Err(DisplayWorkError::Budget));
                        (time, work) = serde_json::from_value(serde_json::to_value((&time, &work)).unwrap()).unwrap();
                        (reference, expected_work) = serde_json::from_value(serde_json::to_value((&reference, &expected_work)).unwrap()).unwrap();
                        time.begin_native_update(); reference.begin_native_update();
                    }
                    assert!(work.finished());
                    assert!(time.active_vblank());
                }
            }
        }
    }
    #[test]
    fn every_original_flash_oam_case_matches_bytes_adjustments_and_cpu_cost() {
        for (data, count, constructed) in [
            (include_str!("prepare_oam_cases_1113.json"), 6, false),
            (include_str!("prepare_oam_constructed_1113.json"), 32, true),
        ] {
            let fixture: serde_json::Value = serde_json::from_str(data).unwrap();
            let cases = fixture["cases"].as_array().unwrap();
            assert_eq!(cases.len(), count);
            if constructed {
                assert_eq!(fixture["normal_playthrough_proof"], false);
            }
            for (n, case) in cases.iter().enumerate() {
                let state: PrepareOamState =
                    serde_json::from_value(case["before"].clone()).unwrap();
                let expected: PrepareOamState =
                    serde_json::from_value(case["after"].clone()).unwrap();
                let shadow: Vec<u8> =
                    serde_json::from_value(case["before"]["shadow_oam"].clone()).unwrap();
                let expected_shadow: Vec<u8> =
                    serde_json::from_value(case["after"]["shadow_oam"].clone()).unwrap();
                let mut time = DisplayWorkTime::default();
                time.begin_native_update();
                time.set_shadow_oam(&shadow).unwrap();
                let mut work = PrepareOamWork::new(state).unwrap();
                assert_eq!(time.shadow_oam(), shadow, "construction may not publish");
                while work.step(&mut time).unwrap().is_some() {}
                assert_eq!(work.state(), &expected, "case{n}");
                assert_eq!(time.shadow_oam(), expected_shadow, "case{n}");
                assert_eq!(time.elapsed(), case["tcycles"].as_u64().unwrap(), "case{n}");
            }
        }
    }
    #[test]
    fn blocked_instruction_and_mid_program_snapshot_do_not_publish_early() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("prepare_oam_cases_1113.json")).unwrap();
        let case = fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["tcycles"].as_u64().unwrap() > 56)
            .unwrap();
        let state: PrepareOamState = serde_json::from_value(case["before"].clone()).unwrap();
        let shadow: Vec<u8> = serde_json::from_value(case["before"]["shadow_oam"].clone()).unwrap();
        let mut time = DisplayWorkTime::default();
        time.set_shadow_oam(&shadow).unwrap();
        let mut work = PrepareOamWork::new(state).unwrap();
        assert_eq!(work.step(&mut time), Err(DisplayWorkError::Budget));
        assert_eq!(time.shadow_oam(), shadow);
        time.begin_native_update();
        for _ in 0..100 {
            work.step(&mut time).unwrap();
        }
        let saved = serde_json::to_string(&(&work, &time)).unwrap();
        let (mut other, mut other_time): (PrepareOamWork, DisplayWorkTime) =
            serde_json::from_str(&saved).unwrap();
        while work.step(&mut time).unwrap().is_some() {}
        while other.step(&mut other_time).unwrap().is_some() {}
        assert_eq!(work.state(), other.state());
        assert_eq!(time.shadow_oam(), other_time.shadow_oam());
        assert_eq!(time.elapsed(), other_time.elapsed());
    }
}

#[cfg(test)]
mod normal_flash_live_controls_1158 {
    use super::*;
    use crate::overworld::display_work_time::DisplayWorkTime;
    #[test]
    fn all_six_observed_contracts_match_the_normal_rom_shadow_outputs_and_return_cycles() {
        let data: serde_json::Value =
            serde_json::from_str(include_str!("prepare_oam_live_cases_1158.json")).unwrap();
        for case in data["cases"].as_array().unwrap() {
            let state: PrepareOamState = serde_json::from_value(case["input"].clone()).unwrap();
            let before: Vec<u8> = serde_json::from_value(case["shadow_before"].clone()).unwrap();
            let after: Vec<u8> = serde_json::from_value(case["shadow_after"].clone()).unwrap();
            let expected: PrepareOamState = serde_json::from_value(case["after"].clone()).unwrap();
            let mut time = DisplayWorkTime::default();
            time.set_shadow_oam(&before).unwrap();
            time.begin_native_update();
            let mut work = PrepareOamWork::new(state).unwrap();
            while work.step(&mut time).unwrap().is_some() {}
            assert_eq!(time.elapsed(), case["tcycles"].as_u64().unwrap());
            assert_eq!(time.shadow_oam(), after);
            assert_eq!(work.state(), &expected);
            assert!(
                !time.active_vblank(),
                "this is the child contract, not a fabricated full ISR"
            );
        }
    }
}

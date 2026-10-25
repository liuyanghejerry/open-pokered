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
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
enum Effect {
    Enabled(u8),
    AdjustY(u8, u8),
    AdjustX(u8, u8),
}
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
struct Instruction {
    bus: DisplayInstruction,
    after: Option<Effect>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PrepareOamWork {
    state: PrepareOamState,
    code: Vec<Instruction>,
    next: usize,
    return_carry: bool,
}
fn emit(p: &mut Vec<Instruction>, cycles: &[u8]) {
    for &tcycles in cycles {
        p.push(Instruction {
            bus: DisplayInstruction {
                tcycles,
                access: None,
            },
            after: None,
        });
    }
}
fn effect(p: &mut Vec<Instruction>, tcycles: u8, after: Effect) {
    p.push(Instruction {
        bus: DisplayInstruction {
            tcycles,
            access: match after {
                Effect::Enabled(value) => Some(DisplayAccess::Write {
                    port: super::sprite_update_control_work::SPRITE_UPDATES_PORT,
                    value,
                    at_tcycle: 12,
                }),
                _ => None,
            },
        },
        after: Some(after),
    });
}
fn write(p: &mut Vec<Instruction>, offset: usize, value: u8) {
    p.push(Instruction {
        bus: DisplayInstruction {
            tcycles: 8,
            access: Some(DisplayAccess::Write {
                port: 0xc300 + offset as u16,
                value,
                at_tcycle: 4,
            }),
        },
        after: None,
    });
}
fn screen_xy(p: &mut Vec<Instruction>, slot: u8, sprite: OamSprite) {
    emit(p, &[4, 4, 8, 12, 4, 4, 8, 12, 8, 4, 4, 12, 8, 8]);
    effect(p, 8, Effect::AdjustY(slot, sprite.y.wrapping_add(4) & 0xf0));
    emit(p, &[4, 12, 8]);
    effect(p, 8, Effect::AdjustX(slot, sprite.x & 0xf0));
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
        let mut p = Vec::new();
        if state.enabled != 1 {
            emit(
                &mut p,
                &[16, 4, 8, 8, if state.enabled == 0 { 8 } else { 20 }],
            );
            if state.enabled == 0 {
                effect(&mut p, 16, Effect::Enabled(0xff));
                emit(&mut p, &[16]); // JP HideSprites
                emit(&mut p, &[8, 12, 12, 8]);
                for slot in 0..40 {
                    write(&mut p, slot * 4, 160);
                    emit(&mut p, &[8, 4, if slot == 39 { 8 } else { 12 }]);
                }
                emit(&mut p, &[16]);
            }
            return Ok(Self {
                state,
                code: p,
                next: 0,
                return_carry,
            });
        }
        emit(&mut p, &[16, 4, 12, 4, 12]);
        let mut used = 0usize;
        for (slot, &sprite) in state.sprites.iter().enumerate() {
            emit(
                &mut p,
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
                    &mut p,
                    &[4, 4, 8, 16, 8, if sprite.image == 0xff { 8 } else { 12 }],
                );
                if sprite.image == 0xff {
                    emit(&mut p, &[24]);
                    screen_xy(&mut p, slot as u8, sprite);
                    emit(&mut p, &[12]);
                } else {
                    if used + 16 > 160 {
                        return Err(PrepareOamError::ShadowPageOverflow);
                    }
                    let still = sprite.image >= 0xa0;
                    emit(&mut p, &[8, if still { 8 } else { 12 }]);
                    if still {
                        emit(&mut p, &[8, 8, 12]);
                    } else {
                        emit(&mut p, &[8]);
                    }
                    // .next through CALL GetSpriteScreenXY, including table reads.
                    emit(
                        &mut p,
                        &[
                            4, 16, 4, 4, 8, 4, 8, 8, 12, 12, 8, 12, 8, 8, 8, 8, 4, 8, 4, 8, 8, 4,
                            24,
                        ],
                    );
                    screen_xy(&mut p, slot as u8, sprite);
                    emit(&mut p, &[12, 4, 8]);
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
                        emit(&mut p, &[12, 8, 8]);
                        write(&mut p, used + tile * 4, y);
                        emit(&mut p, &[8, 12, 8, 8, 4]);
                        write(&mut p, used + tile * 4 + 1, x);
                        emit(
                            &mut p,
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
                            emit(&mut p, &[8, 12]);
                        } else {
                            emit(&mut p, &[8, 8, 4, 8, 4]);
                        }
                        emit(&mut p, &[4, 12]);
                        let base = if sprite.image >> 4 == 11 {
                            124
                        } else {
                            (sprite.image >> 4) * 12
                        };
                        write(
                            &mut p,
                            used + tile * 4 + 2,
                            base.wrapping_add(pattern).wrapping_add(tile as u8),
                        );
                        emit(&mut p, &[8, 4, 8, 8, if tile >= 2 { 8 } else { 12 }]);
                        if tile >= 2 {
                            emit(&mut p, &[12, 8]);
                        }
                        emit(&mut p, &[8]);
                        let attrs = (if flip { 0x20 } else { 0 })
                            | (if tile >= 2 {
                                2 | sprite.grass & 0x80
                            } else {
                                0
                            })
                            | u8::from(tile == 3);
                        write(&mut p, used + tile * 4 + 3, attrs);
                        emit(&mut p, &[4, 8, if tile == 3 { 8 } else { 12 }]);
                    }
                    emit(&mut p, &[4, 12]);
                    used += 16;
                }
            }
            emit(&mut p, &[12, 8, 8, if slot == 15 { 12 } else { 16 }]);
        }
        let preserve = state.movement_flags & 0x40 != 0;
        emit(
            &mut p,
            &[12, 4, 8, 12, 8, 16, 8, 8, if preserve { 8 } else { 12 }],
        );
        if preserve {
            emit(&mut p, &[8]);
        }
        let end = if preserve { 144 } else { 160 };
        // Source can alias adjacent WRAM for overfull pages. This bounded
        // backend rejects that case explicitly; no all-state support claimed.
        if used > end {
            return Err(PrepareOamError::ShadowPageOverflow);
        }
        for offset in (used..end).step_by(4) {
            emit(&mut p, &[4, 8]);
            write(&mut p, offset, 160);
            emit(&mut p, &[8, 12]);
        }
        emit(&mut p, &[4, 20]);
        Ok(Self {
            state,
            code: p,
            next: 0,
            return_carry,
        })
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
    pub fn step<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<Option<DisplayInstruction>, B::Error> {
        let Some(i) = self.code.get(self.next).copied() else {
            return Ok(None);
        };
        bus.execute(i.bus)?;
        match i.after {
            Some(Effect::Enabled(v)) => self.state.enabled = v,
            Some(Effect::AdjustY(slot, v)) => self.state.sprites[usize::from(slot)].adjusted_y = v,
            Some(Effect::AdjustX(slot, v)) => self.state.sprites[usize::from(slot)].adjusted_x = v,
            None => (),
        }
        self.next += 1;
        Ok(Some(i.bus))
    }
}

#[cfg(test)]
mod tests {
    use super::super::display_work_time::{DisplayWorkError, DisplayWorkTime};
    use super::*;
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

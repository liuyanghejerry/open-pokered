//! One native time/device owner for resumable display instructions.
//! A native update grants 70224 T-cycles; grants never finish a child or move
//! time. The caller executes real instructions or explicit idle intervals.
//! This is a native projection, not a recovered original startup/IRQ phase.
use super::display_device::{DisplayDevice, DisplayDeviceError};
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};
use super::field_restore::{PatternAssets, PatternMemory, RestoreError};
use super::pattern_copy_work::{PatternAccess, PatternInstruction, PatternInstructionBus};
use crate::alloc_prelude::*;

pub const NATIVE_FRAME_T_CYCLES: u64 = 154 * 456;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayWorkError {
    Budget,
    NoPendingVBlank,
    InterruptBusy,
    NoActiveVBlank,
    Device(DisplayDeviceError),
    Resource(RestoreError),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DisplayWorkTime {
    elapsed: u64,
    deadline: u64,
    pub device: DisplayDevice,
    shadow_oam: Vec<u8>,
    /// Actual native caller/control effects. Missing old snapshot fields stay
    /// unknown until a caller writes them; default native boot projects1.
    #[serde(default)]
    sprite_updates_enabled: Option<u8>,
    #[serde(default)]
    legacy_party_sprite_owner: bool,
    #[serde(default)]
    legacy_sprite_hide: Option<super::prepare_oam_work::PrepareOamWork>,
    #[serde(default)]
    vblank_flag: u8,
    #[serde(default)]
    interrupt_returns: u64,
    #[serde(default = "projected_bank")]
    loaded_rom_bank: u8,
    #[serde(default)]
    rom_bank_temp: u8,
    #[serde(default = "projected_bank")]
    selected_rom_bank: u8,
    #[serde(default)]
    active_vblank: bool,
    /// Native divider projection; original boot/reset phase is not recovered.
    #[serde(default)]
    divider_counter: u16,
    #[serde(default)]
    frame_counter: u8,
    #[serde(default)]
    random_add: u8,
    #[serde(default)]
    random_sub: u8,
    #[serde(default)]
    physical_buttons: u8,
    #[serde(default = "joypad_deselected")]
    joypad_select: u8,
    #[serde(default)]
    joy_input: u8,
    #[serde(default)]
    disable_joypad_polling: u8,
}
fn joypad_deselected() -> u8 {
    0x30
}

// Native bank projection, not recovery of the original startup bank.
fn projected_bank() -> u8 {
    1
}

impl Default for DisplayWorkTime {
    fn default() -> Self {
        let mut device = DisplayDevice::powered_off(0);
        // Native boot projection. Ordinary presentation owns its own existing
        // pixels; this initial device phase is not labeled as original boot.
        device.write_port(0xff40, 0x91, 0xc3).unwrap();
        device.write_port(0xffff, 1, 0xc3).unwrap();
        Self {
            elapsed: 0,
            deadline: 0,
            device,
            shadow_oam: vec![0; 160],
            sprite_updates_enabled: Some(1),
            legacy_party_sprite_owner: false,
            legacy_sprite_hide: None,
            vblank_flag: 0,
            interrupt_returns: 0,
            loaded_rom_bank: projected_bank(),
            rom_bank_temp: 0,
            selected_rom_bank: projected_bank(),
            active_vblank: false,
            divider_counter: 0,
            frame_counter: 0,
            random_add: 0,
            random_sub: 0,
            physical_buttons: 0,
            joypad_select: joypad_deselected(),
            joy_input: 0,
            disable_joypad_polling: 0,
        }
    }
}

impl DisplayWorkTime {
    /// Before display-device snapshots existed, native frames never changed
    /// LCD phase. Recreate that revision's fixed-frame projection explicitly;
    /// this is not recovery of an original hardware startup phase.
    pub fn legacy_native_projection(frames: u64) -> Self {
        let mut time = Self::default();
        time.deadline = frames
            .checked_mul(NATIVE_FRAME_T_CYCLES)
            .expect("legacy display deadline overflow");
        time.idle_to_target()
            .expect("valid legacy native projection");
        time
    }
    /// Called by the shared IRQ owner only after executing its RETI. This
    /// receipt never clears the wait flag, acknowledges IF, or advances time.
    pub(crate) fn record_interrupt_return(&mut self) {
        self.active_vblank = false;
        self.interrupt_returns = self
            .interrupt_returns
            .checked_add(1)
            .expect("interrupt return counter overflow");
    }
    pub fn active_vblank(&self) -> bool {
        self.active_vblank
    }
    /// Hardware acceptance in this bounded native device subset. The owner
    /// must already have selected VBlank with IME enabled. This does not model
    /// CPU-stack/IE alias cancellation. IF is acknowledged before elapsed time
    /// advances, so a subsequent device edge can request another interrupt.
    pub fn enter_pending_vblank(&mut self) -> Result<(), DisplayWorkError> {
        if self.active_vblank {
            return Err(DisplayWorkError::InterruptBusy);
        }
        if self.device.pending_interrupts() & 1 == 0 {
            return Err(DisplayWorkError::NoPendingVBlank);
        }
        self.reserve()?;
        self.device.acknowledge_interrupt(1);
        self.active_vblank = true;
        self.elapse(20)
    }
    pub fn source_loaded_rom_bank(&self) -> u8 {
        self.loaded_rom_bank
    }
    pub fn source_rom_bank_temp(&self) -> u8 {
        self.rom_bank_temp
    }
    /// Raw bank selector written by the source caller. This is not a
    /// general-purpose cartridge mapper or a readable hardware port.
    pub fn selected_rom_bank(&self) -> u8 {
        self.selected_rom_bank
    }
    /// Current frontend buttons in the source A/B/SELECT/START/RIGHT/LEFT/
    /// UP/DOWN bit order. Feeding physical input does not poll JOYP or
    /// update the foreground Joypad history. JOYP interrupt edges/electrical
    /// settling are outside this bounded native input-device projection.
    pub fn set_physical_buttons(&mut self, buttons: u8) {
        self.physical_buttons = buttons;
    }
    pub fn source_joy_input(&self) -> u8 {
        self.joy_input
    }
    pub fn sprite_updates_enabled(&self) -> Option<u8> {
        self.sprite_updates_enabled
    }
    /// StartMenu_Pokemon's explicit hide request, adapted from the existing
    /// native UI caller. This native store does not claim source UI cycles.
    pub fn begin_legacy_party_sprite_hide(&mut self) -> Result<(), DisplayWorkError> {
        if self.active_vblank || self.legacy_sprite_hide.is_some() {
            return Err(DisplayWorkError::InterruptBusy);
        }
        self.sprite_updates_enabled = Some(0);
        self.legacy_party_sprite_owner = true;
        self.legacy_sprite_hide = Some(
            super::prepare_oam_work::PrepareOamWork::control_only(0)
                .expect("disabled control path needs no actors"),
        );
        Ok(())
    }
    /// Existing native party return adapts its already-completed map restore.
    /// FLASH's executed inline store independently establishes1 before this.
    pub fn finish_legacy_party_sprite_owner(&mut self) {
        if self.legacy_party_sprite_owner {
            assert!(self.legacy_sprite_hide.is_none());
            self.sprite_updates_enabled = Some(1);
            self.legacy_party_sprite_owner = false;
        }
    }
    /// Execute the real hide child on native publication time. This is a
    /// legacy adapter, not a VBlank body: never accept IF or issue RETI here.
    #[inline]
    pub fn service_legacy_sprite_hide(&mut self) -> Result<bool, DisplayWorkError> {
        // Do not move a large optional resumable child on every idle update.
        // Keep the common no-work check inline; only the pending path takes
        // ownership of its registers and instruction buffers.
        if self.legacy_sprite_hide.is_none() {
            return Ok(true);
        }
        self.service_pending_legacy_sprite_hide()
    }

    #[inline(never)]
    fn service_pending_legacy_sprite_hide(&mut self) -> Result<bool, DisplayWorkError> {
        let Some(mut child) = self.legacy_sprite_hide.take() else {
            return Ok(true);
        };
        loop {
            match child.step(self) {
                Ok(Some(_)) => self.sprite_updates_enabled = Some(child.state().enabled),
                Ok(None) => return Ok(true),
                Err(DisplayWorkError::Budget) => {
                    self.legacy_sprite_hide = Some(child);
                    return Ok(false);
                }
                Err(error) => {
                    self.legacy_sprite_hide = Some(child);
                    return Err(error);
                }
            }
        }
    }

    fn read_display_port(&self, port: u16) -> Result<u8, DisplayDeviceError> {
        if port == super::sprite_update_control_work::SPRITE_UPDATES_PORT {
            self.sprite_updates_enabled
                .ok_or(DisplayDeviceError::UnsupportedPort(port))
        } else if port == 0xff00 {
            let select = self.joypad_select & 0x30;
            let mut low = 0xf;
            if select & 0x10 == 0 {
                low &= !(self.physical_buttons >> 4) & 0xf;
            }
            if select & 0x20 == 0 {
                low &= !self.physical_buttons & 0xf;
            }
            Ok(0xc0 | select | low)
        } else if port == 0xfff8 {
            Ok(self.joy_input)
        } else if port == 0xfff9 {
            Ok(self.disable_joypad_polling)
        } else if port == 0xff04 {
            Ok((self.divider_counter >> 8) as u8)
        } else if port == 0xffd3 {
            Ok(self.random_add)
        } else if port == 0xffd4 {
            Ok(self.random_sub)
        } else if port == 0xffd5 {
            Ok(self.frame_counter)
        } else if port == super::far_pattern_copy_work::ROM_BANK_TEMP {
            Ok(self.rom_bank_temp)
        } else if port == super::far_pattern_copy_work::LOADED_ROM_BANK {
            Ok(self.loaded_rom_bank)
        } else if port == super::display_delay_work::VBLANK_FLAG_PORT {
            Ok(self.vblank_flag)
        } else if (0xc300..0xc3a0).contains(&port) {
            self.shadow_oam
                .get(usize::from(port - 0xc300))
                .copied()
                .ok_or(DisplayDeviceError::InvalidOam)
        } else {
            self.device.read_port(port)
        }
    }
    /// Source wShadowOAM ($c300). The native IRQ owner publishes its prepared
    /// bytes here; the device reads them as DMA progresses, not at DMA entry.
    pub fn shadow_oam(&self) -> &[u8] {
        &self.shadow_oam
    }
    pub fn set_shadow_oam(&mut self, bytes: &[u8]) -> Result<(), DisplayDeviceError> {
        if bytes.len() != 160 {
            return Err(DisplayDeviceError::InvalidOam);
        }
        self.shadow_oam.copy_from_slice(bytes);
        Ok(())
    }
    pub fn elapsed(&self) -> u64 {
        self.elapsed
    }
    pub fn remaining(&self) -> u64 {
        self.deadline.saturating_sub(self.elapsed)
    }
    #[inline(always)]
    pub fn begin_native_update(&mut self) {
        self.deadline = self
            .deadline
            .checked_add(NATIVE_FRAME_T_CYCLES)
            .expect("display deadline overflow");
    }
    #[inline(always)]
    fn reserve(&self) -> Result<(), DisplayWorkError> {
        self.device.validate().map_err(DisplayWorkError::Device)?;
        if self.device.observed_tcycles() != self.elapsed {
            return Err(DisplayWorkError::Device(
                DisplayDeviceError::TimeNotSynchronized,
            ));
        }
        if self.elapsed >= self.deadline {
            return Err(DisplayWorkError::Budget);
        }
        Ok(())
    }
    #[inline(always)]
    fn elapse(&mut self, cycles: u64) -> Result<(), DisplayWorkError> {
        let next = self
            .elapsed
            .checked_add(cycles)
            .expect("display time overflow");
        self.device
            .observe_time(next, &self.shadow_oam)
            .map_err(DisplayWorkError::Device)?;
        self.divider_counter = self.divider_counter.wrapping_add(cycles as u16);
        self.elapsed = next;
        Ok(())
    }
    #[inline(always)]
    pub fn idle_to_target(&mut self) -> Result<(), DisplayWorkError> {
        // An untouched one-period grant in settled VBlank has an exact
        // periodic device state. Avoid generic elapsed/event dispatch there,
        // preserving the same global time, divider and real VBlank request.
        if self.deadline.checked_sub(self.elapsed) == Some(NATIVE_FRAME_T_CYCLES)
            && self.shadow_oam.len() == 160
            && self.device.try_idle_vblank_period(self.deadline)
        {
            self.divider_counter = self
                .divider_counter
                .wrapping_add(NATIVE_FRAME_T_CYCLES as u16);
            self.elapsed = self.deadline;
            return Ok(());
        }
        self.elapse(self.remaining())
    }
    /// Wait only as far as the next actual device event or host target.
    /// Caller still must observe/acknowledge its own VBlank request.
    pub fn idle_to_device_event(&mut self) -> Result<(), DisplayWorkError> {
        let cycles = self
            .device
            .tcycles_until_event()
            .map(u64::from)
            .unwrap_or(self.remaining())
            .min(self.remaining());
        self.elapse(cycles)
    }
    pub fn pattern_bus<'a, A: PatternAssets>(
        &'a mut self,
        patterns: &'a mut PatternMemory,
        assets: &'a A,
    ) -> PatternBus<'a, A> {
        PatternBus {
            time: self,
            patterns,
            assets,
        }
    }
}

impl DisplayInstructionBus for DisplayWorkTime {
    type Error = DisplayWorkError;
    #[inline(always)]
    fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
        if instruction.access.is_none() {
            self.reserve()?;
            self.elapse(u64::from(instruction.tcycles))?;
            return Ok(None);
        }
        self.execute_display_general(instruction)
    }
}

impl DisplayWorkTime {
    fn execute_display_general(
        &mut self,
        instruction: DisplayInstruction,
    ) -> Result<Option<u8>, DisplayWorkError> {
        // Validate ports without mutation before accepting this instruction.
        if let Some(access) = instruction.access {
            match access {
                DisplayAccess::Read { port, .. } => {
                    self.read_display_port(port)
                        .map_err(DisplayWorkError::Device)?;
                }
                DisplayAccess::Write { port, .. }
                    if [
                        0xff00,
                        0xfff8,
                        0xfff9,
                        0xff04,
                        0xffd3,
                        0xffd4,
                        0xffd5,
                        super::display_delay_work::VBLANK_FLAG_PORT,
                        super::sprite_update_control_work::SPRITE_UPDATES_PORT,
                        super::far_pattern_copy_work::ROM_BANK_TEMP,
                        super::far_pattern_copy_work::LOADED_ROM_BANK,
                        super::far_pattern_copy_work::ROM_BANK_SELECT,
                    ]
                    .contains(&port) =>
                {
                    ()
                }
                DisplayAccess::Write { port, .. } if (0xc300..0xc3a0).contains(&port) => {
                    self.read_display_port(port)
                        .map_err(DisplayWorkError::Device)?;
                }
                DisplayAccess::Write { port, value, .. } => {
                    self.device
                        .validate_write_port(port, value, 0xc3)
                        .map_err(DisplayWorkError::Device)?;
                }
            }
        }
        self.reserve()?;
        let (value, consumed) = match instruction.access {
            Some(DisplayAccess::Read { port, at_tcycle }) => {
                self.elapse(u64::from(at_tcycle))?;
                (
                    Some(
                        self.read_display_port(port)
                            .map_err(DisplayWorkError::Device)?,
                    ),
                    at_tcycle,
                )
            }
            Some(DisplayAccess::Write {
                port,
                value,
                at_tcycle,
            }) => {
                self.elapse(u64::from(at_tcycle))?;
                if port == super::sprite_update_control_work::SPRITE_UPDATES_PORT {
                    self.sprite_updates_enabled = Some(value);
                } else if port == 0xff00 {
                    self.joypad_select = value & 0x30;
                } else if port == 0xfff8 {
                    self.joy_input = value;
                } else if port == 0xfff9 {
                    self.disable_joypad_polling = value;
                } else if port == 0xff04 {
                    self.divider_counter = 0;
                } else if port == 0xffd3 {
                    self.random_add = value;
                } else if port == 0xffd4 {
                    self.random_sub = value;
                } else if port == 0xffd5 {
                    self.frame_counter = value;
                } else if port == super::far_pattern_copy_work::ROM_BANK_TEMP {
                    self.rom_bank_temp = value;
                } else if port == super::far_pattern_copy_work::LOADED_ROM_BANK {
                    self.loaded_rom_bank = value;
                } else if port == super::far_pattern_copy_work::ROM_BANK_SELECT {
                    self.selected_rom_bank = value;
                } else if port == super::display_delay_work::VBLANK_FLAG_PORT {
                    self.vblank_flag = value;
                } else if (0xc300..0xc3a0).contains(&port) {
                    self.shadow_oam[usize::from(port - 0xc300)] = value;
                } else {
                    self.device
                        .write_port(port, value, 0xc3)
                        .map_err(DisplayWorkError::Device)?;
                }
                (None, at_tcycle)
            }
            None => (None, 0),
        };
        self.elapse(u64::from(instruction.tcycles - consumed))?;
        Ok(value)
    }
}

impl super::display_delay_work::DelayFrameBus for DisplayWorkTime {
    fn interrupt_returns(&self) -> u64 {
        self.interrupt_returns
    }
}

impl super::vblank_return_work::VBlankReturnBus for DisplayWorkTime {
    fn check_active_vblank(&self) -> Result<(), DisplayWorkError> {
        if self.active_vblank {
            Ok(())
        } else {
            Err(DisplayWorkError::NoActiveVBlank)
        }
    }
    fn execute_reti(&mut self) -> Result<(), DisplayWorkError> {
        self.check_active_vblank()?;
        self.reserve()?;
        self.elapse(16)?;
        self.record_interrupt_return();
        Ok(())
    }
}

pub struct PatternBus<'a, A> {
    time: &'a mut DisplayWorkTime,
    patterns: &'a mut PatternMemory,
    assets: &'a A,
}
impl<A> DisplayInstructionBus for PatternBus<'_, A> {
    type Error = DisplayWorkError;
    fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
        DisplayInstructionBus::execute(self.time, instruction)
    }
}

impl<A: PatternAssets> PatternInstructionBus for PatternBus<'_, A> {
    type Error = DisplayWorkError;
    #[inline(always)]
    fn execute(&mut self, instruction: PatternInstruction) -> Result<Option<u16>, Self::Error> {
        if instruction.access.is_none() {
            if self.patterns.bytes().len() != 0x2000 {
                return Err(DisplayWorkError::Resource(
                    RestoreError::InvalidPatternMemory,
                ));
            }
            self.time.reserve()?;
            self.time.elapse(u64::from(instruction.tcycles))?;
            return Ok(None);
        }
        self.execute_pattern_general(instruction)
    }
}

impl<A: PatternAssets> PatternBus<'_, A> {
    fn execute_pattern_general(
        &mut self,
        instruction: PatternInstruction,
    ) -> Result<Option<u16>, DisplayWorkError> {
        if self.patterns.bytes().len() != 0x2000 {
            return Err(DisplayWorkError::Resource(
                RestoreError::InvalidPatternMemory,
            ));
        }
        let mut read = match instruction.access {
            Some(PatternAccess::ReadVram { address, .. }) => {
                self.patterns
                    .read(address)
                    .ok_or(DisplayWorkError::Resource(
                        RestoreError::InvalidDestination(address),
                    ))?;
                None
            }
            Some(PatternAccess::Read { source, offset, .. }) => {
                Some(u16::from(
                    self.assets.read_pattern_byte(source, offset).ok_or(
                        DisplayWorkError::Resource(RestoreError::MissingSource(source, offset)),
                    )?,
                ))
            }
            Some(PatternAccess::ReadPair { source, offset, .. }) => {
                let low = self.assets.read_pattern_byte(source, offset).ok_or(
                    DisplayWorkError::Resource(RestoreError::MissingSource(source, offset)),
                )?;
                let high_offset = offset.wrapping_add(1);
                let high = self.assets.read_pattern_byte(source, high_offset).ok_or(
                    DisplayWorkError::Resource(RestoreError::MissingSource(source, high_offset)),
                )?;
                Some(u16::from(low) | (u16::from(high) << 8))
            }
            Some(PatternAccess::Write { destination, .. }) => {
                self.patterns
                    .read(destination)
                    .ok_or(DisplayWorkError::Resource(
                        RestoreError::InvalidDestination(destination),
                    ))?;
                None
            }
            None => None,
        };
        self.time.reserve()?;
        let consumed = match instruction.access {
            Some(PatternAccess::ReadVram { address, at_tcycle }) => {
                self.time.elapse(u64::from(at_tcycle))?;
                read = Some(u16::from(if self.time.device.vram_accessible() {
                    self.patterns.read(address).expect("validated VRAM address")
                } else {
                    0xff
                }));
                at_tcycle
            }
            Some(PatternAccess::Read { at_tcycle, .. }) => {
                self.time.elapse(u64::from(at_tcycle))?;
                at_tcycle
            }
            Some(PatternAccess::ReadPair {
                first_at_tcycle,
                second_at_tcycle,
                ..
            }) => {
                self.time.elapse(u64::from(first_at_tcycle))?;
                self.time
                    .elapse(u64::from(second_at_tcycle - first_at_tcycle))?;
                second_at_tcycle
            }
            Some(PatternAccess::Write {
                destination,
                value,
                at_tcycle,
            }) => {
                self.time.elapse(u64::from(at_tcycle))?;
                if self.time.device.vram_accessible() {
                    self.patterns
                        .write(destination, value)
                        .map_err(DisplayWorkError::Resource)?;
                }
                at_tcycle
            }
            None => 0,
        };
        self.time
            .elapse(u64::from(instruction.tcycles - consumed))?;
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        display_lcd_work::{DisableLcdWork, EnableLcdWork},
        field_restore::PatternSource,
        pattern_copy_work::PatternCopyDataWork,
    };
    use super::*;
    struct Bytes;
    impl PatternAssets for Bytes {
        fn read_pattern_byte(&self, _: PatternSource, offset: u16) -> Option<u8> {
            Some(offset as u8)
        }
    }
    #[test]
    fn quiet_bus_dispatch_matches_general_path_and_rejects_bad_state() {
        let mut seed = DisplayWorkTime::default();
        seed.deadline = 3 * NATIVE_FRAME_T_CYCLES;
        for sample in (0..2 * NATIVE_FRAME_T_CYCLES).step_by(113) {
            seed.elapse(sample - seed.elapsed).unwrap();
            for cycles in [0, 4, 8, 12, 16, 24] {
                let display = DisplayInstruction {
                    tcycles: cycles,
                    access: None,
                };
                let mut fast = seed.clone();
                let mut reference = seed.clone();
                assert_eq!(
                    DisplayInstructionBus::execute(&mut fast, display),
                    reference.execute_display_general(display)
                );
                assert_eq!(
                    serde_json::to_value(&fast).unwrap(),
                    serde_json::to_value(&reference).unwrap()
                );
                let pattern = PatternInstruction {
                    tcycles: cycles,
                    access: None,
                };
                let mut a = PatternMemory::default();
                let mut b = a.clone();
                let mut fast = seed.clone();
                let mut reference = seed.clone();
                assert_eq!(
                    PatternInstructionBus::execute(&mut fast.pattern_bus(&mut a, &Bytes), pattern),
                    reference
                        .pattern_bus(&mut b, &Bytes)
                        .execute_pattern_general(pattern)
                );
                assert_eq!(a, b);
                assert_eq!(
                    serde_json::to_value(&fast).unwrap(),
                    serde_json::to_value(&reference).unwrap()
                );
            }
        }
        for bad in 0..6 {
            let mut seed = DisplayWorkTime::default();
            seed.begin_native_update();
            match bad {
                0 => seed.shadow_oam.clear(),
                1 => seed.elapsed = 1,
                2 => seed.deadline = 0,
                3 => seed.deadline = 1,
                4 => {
                    let mut value = serde_json::to_value(&seed).unwrap();
                    value["device"]["stat_enable"] = 8.into();
                    seed = serde_json::from_value(value).unwrap();
                }
                5 => {
                    let mut value = serde_json::to_value(&seed).unwrap();
                    value["device"]["oam"] = serde_json::json!([]);
                    seed = serde_json::from_value(value).unwrap();
                }
                _ => unreachable!(),
            }
            let instruction = DisplayInstruction {
                tcycles: 24,
                access: None,
            };
            let mut fast = seed.clone();
            let mut reference = seed.clone();
            assert_eq!(
                DisplayInstructionBus::execute(&mut fast, instruction),
                reference.execute_display_general(instruction)
            );
            assert_eq!(
                serde_json::to_value(&fast).unwrap(),
                serde_json::to_value(&reference).unwrap()
            );
            for malformed_pattern in [false, true] {
                let mut a = if malformed_pattern {
                    serde_json::from_str("{\"bytes\":[]}").unwrap()
                } else {
                    PatternMemory::default()
                };
                let mut b = a.clone();
                let mut fast = seed.clone();
                let mut reference = seed.clone();
                let instruction = PatternInstruction {
                    tcycles: 24,
                    access: None,
                };
                assert_eq!(
                    PatternInstructionBus::execute(
                        &mut fast.pattern_bus(&mut a, &Bytes),
                        instruction
                    ),
                    reference
                        .pattern_bus(&mut b, &Bytes)
                        .execute_pattern_general(instruction)
                );
                assert_eq!(a, b);
                assert_eq!(
                    serde_json::to_value(&fast).unwrap(),
                    serde_json::to_value(&reference).unwrap()
                );
            }
        }
    }

    #[test]
    fn idle_fast_path_does_not_hide_invalid_shadow_memory() {
        let mut time = DisplayWorkTime::default();
        for _ in 0..2 {
            time.begin_native_update();
            time.idle_to_target().unwrap();
        }
        let elapsed = time.elapsed();
        time.shadow_oam.pop();
        time.begin_native_update();
        assert!(matches!(
            time.idle_to_target(),
            Err(DisplayWorkError::Device(DisplayDeviceError::InvalidOam))
        ));
        assert_eq!(time.elapsed(), elapsed);
        assert_eq!(time.device.observed_tcycles(), elapsed);
    }

    #[test]
    fn idle_whole_grants_match_complete_time_state_after_every_update() {
        for wy in [0, 7, 144] {
            let mut fast = DisplayWorkTime::default();
            fast.device.write_port(0xff4a, wy, 0xc3).unwrap();
            let mut small = fast.clone();
            for frame in 0..64 {
                fast.begin_native_update();
                small.begin_native_update();
                fast.idle_to_target().unwrap();
                while small.remaining() > 0 {
                    small.elapse(small.remaining().min(4096)).unwrap();
                }
                assert_eq!(
                    serde_json::to_value(&fast).unwrap(),
                    serde_json::to_value(&small).unwrap(),
                    "frame={frame} wy={wy}"
                );
            }
        }
    }

    #[test]
    fn grants_do_not_advance_the_device_and_lcd_work_waits_for_real_ly() {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        assert_eq!(time.elapsed(), 0);
        let mut disable = DisableLcdWork::default();
        while !disable.finished() {
            disable.step(&mut time).unwrap();
        }
        assert!(!time.device.lcd_enabled());
        assert!(
            time.elapsed() >= 145 * 456,
            "LY145 must actually be reached"
        );
        assert_eq!(time.device.read_port(0xffff).unwrap(), 1);
        let mut enable = EnableLcdWork::default();
        while !enable.finished() {
            enable.step(&mut time).unwrap();
        }
        assert!(time.device.lcd_enabled());
        let epoch = time.device.vblank_epoch();
        time.idle_to_target().unwrap();
        time.begin_native_update();
        while time.remaining() > 0 && time.device.vblank_epoch() == epoch {
            time.idle_to_device_event().unwrap();
        }
        assert!(time.device.vblank_epoch() > epoch);
    }
    #[test]
    fn copied_bytes_consume_shared_time_and_do_not_finish_on_a_new_grant() {
        let mut time = DisplayWorkTime::default();
        time.device.write_port(0xff40, 0x11, 0xc3).unwrap();
        let mut patterns = PatternMemory::default();
        let mut work = PatternCopyDataWork::new(PatternSource::Sprite(3), 0, 0x8000, 4096);
        assert_eq!(
            work.step(&mut time.pattern_bus(&mut patterns, &Bytes)),
            Err(DisplayWorkError::Budget)
        );
        while !work.finished() {
            let before = work;
            let elapsed = time.elapsed();
            time.begin_native_update();
            assert_eq!(time.elapsed(), elapsed);
            assert_eq!(work, before);
            loop {
                match work.step(&mut time.pattern_bus(&mut patterns, &Bytes)) {
                    Ok(None) | Err(DisplayWorkError::Budget) => break,
                    Ok(Some(_)) => (),
                    Err(error) => panic!("{error:?}"),
                }
            }
        }
        assert_eq!(time.elapsed(), 4096 * 52 + 12);
        for n in 0..4096 {
            assert_eq!(patterns.read(0x8000 + n), Some(n as u8));
        }
    }

    #[test]
    fn unsupported_port_write_stops_before_consuming_time_or_mutating_device() {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        let device = time.device.clone();
        let result = time.execute(DisplayInstruction {
            tcycles: 12,
            access: Some(DisplayAccess::Write {
                port: 0xff41,
                value: 0x48,
                at_tcycle: 8,
            }),
        });
        assert_eq!(
            result,
            Err(DisplayWorkError::Device(
                DisplayDeviceError::UnsupportedStatInterrupts(0x48)
            ))
        );
        assert_eq!(time.elapsed(), 0);
        assert_eq!(time.device, device);
    }
    #[test]
    fn earlier_display_snapshots_default_missing_foreground_wait_state() {
        let mut old = serde_json::to_value(DisplayWorkTime::default()).unwrap();
        old.as_object_mut().unwrap().remove("vblank_flag");
        old.as_object_mut().unwrap().remove("interrupt_returns");
        let mut time: DisplayWorkTime = serde_json::from_value(old).unwrap();
        assert_eq!(time.interrupt_returns, 0);
        time.begin_native_update();
        assert_eq!(
            time.execute(DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Read {
                    port: super::super::display_delay_work::VBLANK_FLAG_PORT,
                    at_tcycle: 8
                })
            })
            .unwrap(),
            Some(0)
        );
    }

    #[test]
    fn unsupported_dma_write_is_rejected_before_time_or_device_changes() {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        let prior = time.device.clone();
        assert_eq!(
            time.execute(DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port: 0xff46,
                    value: 0xc2,
                    at_tcycle: 8
                })
            }),
            Err(DisplayWorkError::Device(
                DisplayDeviceError::UnsupportedDmaSource(0xc2)
            ))
        );
        assert_eq!(time.elapsed(), 0);
        assert_eq!(time.device, prior);
    }
}

#[cfg(test)]
mod sprite_control_tests_1159 {
    use super::*;
    use crate::overworld::{
        display_delay_work::DelayFrameBus,
        prepare_oam_work::PrepareOamWork,
        sprite_update_control_work::{EnableSpriteUpdatesWork, SPRITE_UPDATES_PORT},
    };
    #[test]
    fn party_request_executes_hide_writes_and_inline_enable_without_fabricating_irq_return() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        t.set_shadow_oam(&[0x55; 160]).unwrap();
        t.begin_legacy_party_sprite_hide().unwrap();
        assert_eq!(t.sprite_updates_enabled(), Some(0));
        assert!(t.service_legacy_sprite_hide().unwrap());
        assert_eq!(t.sprite_updates_enabled(), Some(0xff));
        for (i, &byte) in t.shadow_oam().iter().enumerate() {
            assert_eq!(byte, if i % 4 == 0 { 160 } else { 0x55 });
        }
        assert!(!t.active_vblank());
        assert_eq!(t.interrupt_returns(), 0);
        let mut inline = EnableSpriteUpdatesWork::default();
        let start = t.elapsed();
        assert!(inline.step(&mut t).unwrap());
        assert_eq!(t.sprite_updates_enabled(), Some(0xff));
        assert!(inline.step(&mut t).unwrap());
        assert_eq!(t.sprite_updates_enabled(), Some(1));
        assert!(!inline.step(&mut t).unwrap());
        assert_eq!(t.elapsed() - start, 24);
        let value = t
            .execute(DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Read {
                    port: SPRITE_UPDATES_PORT,
                    at_tcycle: 8,
                }),
            })
            .unwrap();
        assert_eq!(value, Some(1));
        assert_eq!(t.interrupt_returns(), 0);
        t.finish_legacy_party_sprite_owner();
        assert_eq!(t.sprite_updates_enabled(), Some(1));
    }
    #[test]
    fn interrupted_native_hide_retains_its_child_and_old_snapshot_flags_remain_unknown() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        t.begin_legacy_party_sprite_hide().unwrap();
        t.deadline = 24;
        assert!(!t.service_legacy_sprite_hide().unwrap());
        assert_eq!(t.sprite_updates_enabled(), Some(0));
        let saved = serde_json::to_vec(&t).unwrap();
        let mut copy: DisplayWorkTime = serde_json::from_slice(&saved).unwrap();
        t.begin_native_update();
        copy.begin_native_update();
        assert!(t.service_legacy_sprite_hide().unwrap());
        assert!(copy.service_legacy_sprite_hide().unwrap());
        assert_eq!(
            serde_json::to_value(&t).unwrap(),
            serde_json::to_value(&copy).unwrap()
        );
        let mut old = serde_json::to_value(&t).unwrap();
        old.as_object_mut()
            .unwrap()
            .remove("sprite_updates_enabled");
        let mut old: DisplayWorkTime = serde_json::from_value(old).unwrap();
        assert_eq!(old.sprite_updates_enabled(), None);
        old.begin_legacy_party_sprite_hide().unwrap();
        assert_eq!(old.sprite_updates_enabled(), Some(0));
        assert!(old.service_legacy_sprite_hide().unwrap());
        assert_eq!(old.sprite_updates_enabled(), Some(0xff));
    }
    #[test]
    fn disabled_control_does_not_require_or_replace_an_actor_snapshot() {
        for flag in 0..=255 {
            if flag == 1 {
                assert!(PrepareOamWork::control_only(flag).is_err());
                continue;
            }
            let mut t = DisplayWorkTime::default();
            t.begin_native_update();
            t.set_shadow_oam(&[0x33; 160]).unwrap();
            let mut w = PrepareOamWork::control_only(flag).unwrap();
            while w.step(&mut t).unwrap().is_some() {}
            assert!(w.state().sprites.is_empty());
            assert_eq!(w.state().enabled, if flag == 0 { 255 } else { flag });
            if flag != 0 {
                assert_eq!(t.elapsed(), 56);
                assert_eq!(t.shadow_oam(), &[0x33; 160]);
            }
            assert_eq!(t.interrupt_returns(), 0);
        }
    }
}

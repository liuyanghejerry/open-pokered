//! One native time/device owner for resumable display instructions.
//! A native update grants 70224 T-cycles; grants never finish a child or move
//! time. The caller executes real instructions or explicit idle intervals.
//! This is a native projection, not a recovered original startup/IRQ phase.
use super::display_device::{CheckedFramePeriod, DisplayDevice, DisplayDeviceError};
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};
use super::field_restore::{CopyFormat, PatternAssets, PatternMemory, PatternSource, RestoreError};
use super::pattern_copy_work::{PatternAccess, PatternInstruction, PatternInstructionBus};
use crate::alloc_prelude::*;
#[path = "native_display_clock.rs"]
mod native_display_clock;
pub use native_display_clock::NativeDisplayClock;

pub const NATIVE_FRAME_T_CYCLES: u64 = 154 * 456;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayWorkError {
    Budget,
    NoPendingVBlank,
    InterruptBusy,
    /// A foreground instruction has not executed: the attached owner must
    /// service the pending VBlank before retrying that same continuation.
    InterruptPending,
    NoActiveVBlank,
    Device(DisplayDeviceError),
    Resource(RestoreError),
}

/// Passive hosted diagnostic captured at an actual source-bus palette store.
/// It neither advances time nor authorizes a display/IRQ continuation. Unlike
/// saved device state, an observation is not restored from a snapshot.
#[cfg(not(target_os = "none"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PaletteStoreReceipt {
    pub port: u16,
    pub before: u8,
    pub value: u8,
    pub tcycles: u64,
    pub ly: u8,
    pub mode: u8,
    pub vblank_epoch: u64,
    pub lcd_enabled: bool,
    pub blank_first_frame: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DisplayWorkTime {
    elapsed: u64,
    deadline: u64,
    device: DisplayDevice,
    // Bounded diagnostics only: one latest observation per BGP/OBP port.
    // Kept off GBA entirely and never serialized as causal source history.
    #[cfg(not(target_os = "none"))]
    #[serde(skip)]
    palette_stores: [Option<PaletteStoreReceipt>; 3],
    // Ephemeral: snapshots always restart the full device check. Device
    // mutation is exposed only through an invalidating mutable accessor.
    #[serde(skip)]
    idle_period_cached: bool,
    #[serde(skip)]
    device_validated: bool,
    // Test oracle: retain the original fully checked interval observer while
    // comparing the reserved instruction path at every access boundary.
    #[cfg(test)]
    #[serde(skip)]
    check_each_display_interval: bool,
    shadow_oam: Vec<u8>,
    #[serde(default)]
    video_requests: super::video_request_ports::VideoRequestPorts,
    /// LCD epoch at the last native transfer-control change or IRQ handoff.
    /// Synchronous native UI publication advances AutoBG once per real edge;
    /// keep idle runs lazy without resetting phase when a new caller attaches.
    #[serde(default)]
    native_bg_epoch: u64,
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
    /// Opt-in only when a complete native IRQ owner is attached. Legacy
    /// projections must not stop at an interrupt they cannot yet service.
    #[serde(default)]
    vblank_dispatch_enabled: bool,
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
            #[cfg(not(target_os = "none"))]
            palette_stores: [None; 3],
            idle_period_cached: false,
            device_validated: false,
            #[cfg(test)]
            check_each_display_interval: false,
            shadow_oam: vec![0; 160],
            video_requests: super::video_request_ports::VideoRequestPorts::default(),
            native_bg_epoch: 0,
            sprite_updates_enabled: Some(1),
            legacy_party_sprite_owner: false,
            legacy_sprite_hide: None,
            vblank_flag: 0,
            interrupt_returns: 0,
            loaded_rom_bank: projected_bank(),
            rom_bank_temp: 0,
            selected_rom_bank: projected_bank(),
            active_vblank: false,
            vblank_dispatch_enabled: false,
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
    /// Handoff from the synchronous native producer: it has no outstanding
    /// video request. Establish idle sizes/control only, never ROM addresses
    /// or a recovered original initialization/IRQ history.
    pub fn establish_native_video_idle(&mut self, enabled: u8) -> Result<(), DisplayWorkError> {
        self.set_native_background_enabled(enabled)?;
        use super::video_copy_caller_work::{COPY_SIZE, DOUBLE_SIZE};
        for (port, value) in [(COPY_SIZE, 0), (DOUBLE_SIZE, 0)] {
            self.video_requests.write(port, value);
        }
        Ok(())
    }
    /// Current native/source hAutoBGTransferPortion. Native publication uses
    /// actual LCD edges, not grants, host draw calls or a menu-specific offset.
    pub fn background_portion(&self) -> Option<u8> {
        use super::video_copy_caller_work::{AUTO_BG_ENABLED, AUTO_BG_PORTION};
        let portion = self.video_requests.read(AUTO_BG_PORTION)?;
        if self.vblank_dispatch_enabled || self.video_requests.read(AUTO_BG_ENABLED)? == 0 {
            return Some(portion);
        }
        let count = self.device.vblank_epoch().checked_sub(self.native_bg_epoch)?;
        if count == 0 { return Some(portion); }
        // AutoBgMapTransfer treats any value other than 0/1 as bottom.
        let next = if portion < 2 { portion + 1 } else { 0 };
        Some((next + ((count - 1) % 3) as u8) % 3)
    }
    fn materialize_native_background(&mut self) {
        if let Some(portion) = self.background_portion() {
            self.video_requests.write(super::video_copy_caller_work::AUTO_BG_PORTION, portion);
        }
        self.native_bg_epoch = self.device.vblank_epoch();
    }
    /// The synchronous renderer changes native UI ownership; source callers
    /// instead write the same HRAM ports on their admitted instruction bus.
    pub fn set_native_background_enabled(&mut self, enabled: u8) -> Result<(), DisplayWorkError> {
        if self.active_vblank || self.vblank_dispatch_enabled { return Err(DisplayWorkError::InterruptBusy); }
        use super::video_copy_caller_work::{AUTO_BG_ENABLED, AUTO_BG_PORTION};
        self.materialize_native_background();
        // Old snapshots did not retain this native projection. Seed it once
        // at migration; subsequent callers must preserve the live value.
        if self.video_requests.read(AUTO_BG_PORTION).is_none() {
            self.video_requests.write(AUTO_BG_PORTION, 0);
        }
        self.video_requests.write(AUTO_BG_ENABLED, enabled);
        Ok(())
    }
    pub fn video_requests(&self) -> &super::video_request_ports::VideoRequestPorts {
        &self.video_requests
    }

    /// The synchronous native owner has already performed this update's
    /// audio/timer work. A same-update handoff must not replay its retained
    /// VBlank request. This acknowledges only that native-owned request: it
    /// advances no time and records no source IRQ execution or RETI.
    pub fn acknowledge_native_vblank(&mut self) -> Result<(), DisplayWorkError> {
        if self.active_vblank || self.vblank_dispatch_enabled {
            return Err(DisplayWorkError::InterruptBusy);
        }
        self.device.acknowledge_interrupt(1);
        Ok(())
    }

    /// Actual write-point observations on this source instruction bus only.
    /// Direct diagnostic device mutations and snapshot restore provide none.
    #[cfg(not(target_os = "none"))]
    pub fn palette_store_receipts(&self) -> &[Option<PaletteStoreReceipt>; 3] {
        &self.palette_stores
    }

    #[inline(always)]
    pub(super) fn idle_period_cached(&self) -> bool {
        self.idle_period_cached
    }

    #[inline(always)]
    pub fn device(&self) -> &DisplayDevice {
        &self.device
    }
    #[inline(always)]
    pub fn device_mut(&mut self) -> &mut DisplayDevice {
        self.idle_period_cached = false;
        self.device_validated = false;
        &mut self.device
    }

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
    /// Attach/detach the bounded IME-enabled, non-nested VBlank dispatcher.
    /// This neither accepts IF nor advances a suspended instruction.
    pub fn set_vblank_dispatch_enabled(&mut self, enabled: bool) {
        if enabled != self.vblank_dispatch_enabled {
            if enabled { self.materialize_native_background(); }
            self.native_bg_epoch = self.device.vblank_epoch();
        }
        self.vblank_dispatch_enabled = enabled;
    }
    pub fn pending_vblank_dispatch(&self) -> bool {
        self.vblank_dispatch_enabled && !self.active_vblank
            && self.device.pending_interrupts() & 1 != 0
    }
    #[inline(always)]
    pub fn attached_pending_vblank_dispatch(&self) -> bool {
        debug_assert!(self.vblank_dispatch_enabled);
        !self.active_vblank && self.device.pending_interrupts() & 1 != 0
    }
    #[inline(always)]
    fn check_interrupt_dispatch(&self) -> Result<(), DisplayWorkError> {
        if self.pending_vblank_dispatch() {
            Err(DisplayWorkError::InterruptPending)
        } else {
            Ok(())
        }
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
        // Hardware acceptance is the dispatcher itself, not a foreground
        // instruction. Budget failure must leave IF and active state intact.
        self.reserve_time()?;
        self.device.acknowledge_interrupt(1);
        self.active_vblank = true;
        self.elapse(20)
    }
    /// Consume decoded work only inside the attached ISR. This bounded audio
    /// bridge has no nested interrupt or VRAM access; foreground stays blocked
    /// until all work and the actual epilogue finish. APU IO phase is separate.
    pub(crate) fn consume_vblank_body_work(&mut self, remaining: &mut u32) -> Result<(), DisplayWorkError> {
        super::vblank_return_work::VBlankReturnBus::check_active_vblank(self)?;
        self.reserve_time()?;
        let consumed = u64::from(*remaining).min(self.remaining());
        self.elapse(consumed)?;
        *remaining -= consumed as u32;
        Ok(())
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

    #[inline(always)]
    fn read_display_port(&self, port: u16) -> Result<u8, DisplayDeviceError> {
        if port == super::video_copy_caller_work::AUTO_BG_PORTION {
            return self.background_portion().ok_or(DisplayDeviceError::UnsupportedPort(port));
        }
        if super::video_request_ports::VideoRequestPorts::contains(port) {
            return self.video_requests.read(port).ok_or(DisplayDeviceError::UnsupportedPort(port));
        }
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
    /// Execute an IRQ child's access to an external device on this same clock.
    /// Admission precedes all time and device effects. A final instruction may
    /// overrun a native grant, exactly like the display bus; never re-admit its
    /// suffix after performing the access. The external owner holds its device
    /// exclusively across this call and supplies an infallible register access.
    pub fn execute_external_irq_access<T>(
        &mut self,
        tcycles: u8,
        at_tcycle: u8,
        access: impl FnOnce() -> T,
    ) -> Result<T, DisplayWorkError> {
        self.display_bus()
            .execute_external_irq_access(tcycles, at_tcycle, access)
    }
    #[inline(always)]
    pub fn begin_native_update(&mut self) {
        self.deadline = self
            .deadline
            .checked_add(NATIVE_FRAME_T_CYCLES)
            .expect("display deadline overflow");
    }
    #[inline(always)]
    fn reserve(&mut self) -> Result<(), DisplayWorkError> {
        self.reserve_time()?;
        if self.active_vblank {
            Ok(())
        } else {
            self.check_interrupt_dispatch()
        }
    }
    #[inline(always)]
    fn reserve_time(&mut self) -> Result<(), DisplayWorkError> {
        if !self.device_validated {
            self.device.validate().map_err(DisplayWorkError::Device)?;
            if self.shadow_oam.len() != 160 {
                return Err(DisplayWorkError::Device(DisplayDeviceError::InvalidOam));
            }
            self.device_validated = true;
        }
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
        self.idle_period_cached = false;
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
    // Only reserved instructions call this. Their exclusive borrow prevents
    // external replacement of time/device state during the instruction.
    #[inline(always)]
    fn elapse_reserved(&mut self, cycles: u8) -> Result<(), DisplayWorkError> {
        self.idle_period_cached = false;
        let next = self
            .elapsed
            .checked_add(u64::from(cycles))
            .expect("display time overflow");
        self.device
            .observe_reserved_interval_trusted(next, u64::from(cycles), &self.shadow_oam)
            .map_err(DisplayWorkError::Device)?;
        self.divider_counter = self.divider_counter.wrapping_add(u16::from(cycles));
        self.elapsed = next;
        Ok(())
    }

    #[inline(always)]
    fn elapse_reserved_interval(&mut self, cycles: u64) -> Result<(), DisplayWorkError> {
        self.idle_period_cached = false;
        let next = self
            .elapsed
            .checked_add(cycles)
            .expect("display time overflow");
        self.device
            .observe_reserved_interval_trusted(next, cycles, &self.shadow_oam)
            .map_err(DisplayWorkError::Device)?;
        self.divider_counter = self.divider_counter.wrapping_add(cycles as u16);
        self.elapsed = next;
        Ok(())
    }
    /// The direct instruction owner has just validated and reserved this
    /// instruction. Its exclusive borrow lasts through both sides of a port
    /// access, whose checked store preserves the device invariants. Reuse the
    /// same bounded observer as the existing exclusive display/pattern buses.
    #[inline(always)]
    fn elapse_display_instruction(&mut self, cycles: u8) -> Result<(), DisplayWorkError> {
        #[cfg(test)]
        if self.check_each_display_interval {
            return self.elapse(u64::from(cycles));
        }
        self.elapse_reserved(cycles)
    }
    #[inline(always)]
    pub fn idle_to_target(&mut self) -> Result<(), DisplayWorkError> {
        // An untouched one-period grant in settled VBlank has an exact
        // periodic device state. Avoid generic elapsed/event dispatch there,
        // preserving the same global time, divider and real VBlank request.
        if self.shadow_oam.len() == 160 {
            if let Some(period) = CheckedFramePeriod::new(self.elapsed, self.deadline) {
                let advanced = if self.idle_period_cached {
                    self.device.advance_eligible_period(period);
                    true
                } else {
                    let ok = self.device.try_idle_checked_period(period);
                    self.idle_period_cached = ok;
                    ok
                };
                if advanced {
                    self.divider_counter = self
                        .divider_counter
                        .wrapping_add(NATIVE_FRAME_T_CYCLES as u16);
                    self.elapsed = self.deadline;
                    return Ok(());
                }
            }
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
    /// Wait only to the next real VBlank edge (or this host grant's end).
    /// No interrupt acceptance, request acknowledgement or RETI is inferred.
    pub fn idle_to_vblank_edge(&mut self) -> Result<(), DisplayWorkError> {
        if let Some(distance) = self.device.tcycles_until_vblank() {
            self.elapse(u64::from(distance).min(self.remaining()))
        } else {
            self.idle_to_device_event()
        }
    }
    /// An exclusive read/plain-instruction lease. Checked writes invalidate
    /// its validation receipt; loading a snapshot always creates a new lease.
    pub fn display_bus(&mut self) -> DisplayBus<'_> {
        DisplayBus {
            time: self,
            validated: false,
        }
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
            validated: false,
            memory_valid: false,
            #[cfg(test)]
            check_each_instruction: false,
        }
    }
}

impl DisplayInstructionBus for DisplayWorkTime {
    type Error = DisplayWorkError;
    #[inline(always)]
    fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
        if instruction.access.is_none() {
            self.reserve()?;
            self.elapse_display_instruction(instruction.tcycles)?;
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
                    if !matches!(port, 0xff0f | 0xffff | 0xff40..=0xff45 | 0xff47..=0xff4b)
                        && ![
                            0xff00,
                            0xfff8,
                            0xfff9,
                            0xff04,
                            0xffd3,
                            0xffd4,
                            0xffd5,
                            super::display_delay_work::VBLANK_FLAG_PORT,
                            super::far_pattern_copy_work::ROM_BANK_TEMP,
                            super::far_pattern_copy_work::LOADED_ROM_BANK,
                        ]
                        .contains(&port)
                    {
                        self.read_display_port(port)
                            .map_err(DisplayWorkError::Device)?;
                    }
                }
                DisplayAccess::Write { port, .. } if super::video_request_ports::VideoRequestPorts::contains(port) => (),
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
                self.elapse_display_instruction(at_tcycle)?;
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
                self.elapse_display_instruction(at_tcycle)?;
                if super::video_request_ports::VideoRequestPorts::contains(port) {
                    if !self.vblank_dispatch_enabled && matches!(port,
                        super::video_copy_caller_work::AUTO_BG_ENABLED | super::video_copy_caller_work::AUTO_BG_PORTION) {
                        self.materialize_native_background();
                    }
                    self.video_requests.write(port, value);
                } else if port == super::sprite_update_control_work::SPRITE_UPDATES_PORT {
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
                    #[cfg(not(target_os = "none"))]
                    let receipt = (0xff47..=0xff49).contains(&port).then(|| PaletteStoreReceipt {
                        port,
                        before: self.device.read_port(port).expect("validated palette port"),
                        value,
                        tcycles: self.elapsed,
                        ly: self.device.read_port(0xff44).expect("LY port"),
                        mode: self.device.mode(),
                        vblank_epoch: self.device.vblank_epoch(),
                        lcd_enabled: self.device.lcd_enabled(),
                        blank_first_frame: self.device.blank_first_frame(),
                    });
                    self.device
                        .write_port(port, value, 0xc3)
                        .map_err(DisplayWorkError::Device)?;
                    #[cfg(not(target_os = "none"))]
                    if let Some(receipt) = receipt {
                        self.palette_stores[usize::from(port - 0xff47)] = Some(receipt);
                    }
                }
                (None, at_tcycle)
            }
            None => (None, 0),
        };
        self.elapse_display_instruction(instruction.tcycles - consumed)?;
        Ok(value)
    }
}

impl super::display_delay_work::DelayFrameBus for DisplayWorkTime {
    fn interrupt_returns(&self) -> u64 {
        self.interrupt_returns
    }
}

impl super::display_delay_work::DelayFrameBus for DisplayBus<'_> {
    fn interrupt_returns(&self) -> u64 {
        self.time.interrupt_returns
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

pub struct DisplayBus<'a> {
    time: &'a mut DisplayWorkTime,
    validated: bool,
}
impl DisplayBus<'_> {
    /// Execute external IO while this bus exclusively owns display time.
    /// The callback cannot replace the display device or finish its IRQ, so
    /// successive audio instructions reuse the validated device. Each still
    /// admits its own budget and advances both sides of the actual IO cycle.
    #[inline(always)]
    pub fn execute_external_irq_access<T>(
        &mut self,
        tcycles: u8,
        at_tcycle: u8,
        access: impl FnOnce() -> T,
    ) -> Result<T, DisplayWorkError> {
        self.reserve_external_irq_access(tcycles, at_tcycle)?;
        let result = (|| {
            if at_tcycle != 0 {
                self.time.elapse_reserved(at_tcycle)?;
            }
            let result = access();
            self.time.elapse_reserved(tcycles - at_tcycle)?;
            Ok(result)
        })();
        if result.is_err() {
            self.validated = false;
        }
        result
    }

    #[inline(always)]
    pub fn reserve_external_irq_access(
        &mut self,
        tcycles: u8,
        at_tcycle: u8,
    ) -> Result<(), DisplayWorkError> {
        if !self.time.active_vblank {
            return Err(DisplayWorkError::NoActiveVBlank);
        }
        if tcycles == 0 || tcycles & 3 != 0 || at_tcycle > tcycles || at_tcycle & 3 != 0 {
            return Err(DisplayWorkError::Device(DisplayDeviceError::InvalidPhase));
        }
        self.reserve()
    }

    #[inline(always)]
    fn reserve(&mut self) -> Result<(), DisplayWorkError> {
        if !self.validated {
            self.time.reserve()?;
            self.validated = true;
        } else {
            debug_assert!(self.time.device.validate().is_ok());
            debug_assert_eq!(self.time.device.observed_tcycles(), self.time.elapsed);
            if self.time.elapsed >= self.time.deadline {
                return Err(DisplayWorkError::Budget);
            }
            if !self.time.active_vblank {
                self.time.check_interrupt_dispatch()?;
            }
        }
        Ok(())
    }

    #[inline(always)]
    pub fn execute_plain(&mut self, tcycles: u8) -> Result<(), DisplayWorkError> {
        self.reserve()?;
        self.time.elapse_reserved(tcycles)
    }

    #[inline(always)]
    pub fn remaining(&self) -> u64 {
        self.time.remaining()
    }

    #[inline(always)]
    pub fn active_vblank(&self) -> bool {
        self.time.active_vblank
    }

    /// Settle instructions already admitted by an external audio bus. Their
    /// deferred total may cross the grant boundary, just like the final
    /// ordinary instruction; a later instruction still needs its own reserve.
    #[inline(always)]
    pub fn elapse_accepted_interval(&mut self, cycles: u64) -> Result<(), DisplayWorkError> {
        if !self.time.active_vblank {
            return Err(DisplayWorkError::NoActiveVBlank);
        }
        if !self.validated {
            self.time.reserve()?;
            self.validated = true;
        }
        if cycles != 0 {
            self.time.elapse_reserved_interval(cycles)?;
        }
        Ok(())
    }

    #[inline(always)]
    pub fn execute_plain_cycles(&mut self, instructions: &[u8]) -> Result<usize, DisplayWorkError> {
        if !self.time.active_vblank || instructions.is_empty() {
            return Ok(0);
        }
        self.reserve()?;
        let available = self
            .time
            .remaining()
            .min(u64::from(u32::MAX - u32::from(u8::MAX))) as u32;
        let (mut cycles, mut count) = (0u32, 0usize);
        for &instruction in instructions {
            if cycles >= available {
                break;
            }
            cycles += u32::from(instruction);
            count += 1;
        }
        self.time.elapse_reserved_interval(u64::from(cycles))?;
        Ok(count)
    }

    pub fn execute_buffer_copy(
        &mut self,
        source: &[u8],
        destination: &mut [u8],
        finish_at_final_store: bool,
    ) -> Result<usize, DisplayWorkError> {
        if source.is_empty()
            || source.len() != destination.len()
            || self.time.pending_vblank_dispatch()
        {
            return Ok(0);
        }
        self.reserve()?;
        let available = if self.time.active_vblank || !self.time.vblank_dispatch_enabled {
            self.time.remaining()
        } else {
            let distance = if self.time.device.dma_active() {
                self.time.device.tcycles_until_event().map(u32::from)
            } else {
                self.time.device.tcycles_until_vblank()
            };
            distance.map_or(self.time.remaining(), |distance| {
                self.time.remaining().min(u64::from(distance))
            })
        };
        let count = (available / 52).min(source.len() as u64) as usize;
        if count == 0 {
            return Ok(0);
        }
        destination[..count].copy_from_slice(&source[..count]);
        let tail = if finish_at_final_store && count == source.len() {
            28
        } else {
            0
        };
        self.time
            .elapse_reserved_interval(count as u64 * 52 - tail)?;
        Ok(count)
    }
}

#[cfg(test)]
#[path = "display_external_irq_bus_tests.rs"]
mod external_irq_bus_tests;
#[cfg(test)]
#[path = "display_shadow_oam_run_tests.rs"]
mod shadow_oam_run_tests;
impl DisplayInstructionBus for DisplayBus<'_> {
    type Error = DisplayWorkError;
    fn execute_ly145_run(&mut self) -> Result<Option<u8>, Self::Error> {
        if self.time.device.read_port(0xffff).unwrap() & 1 != 0
            || self.time.active_vblank
            || self.time.remaining() < 32
        {
            return Ok(None);
        }
        self.reserve()?;
        let Some(vblank) = self.time.device.tcycles_until_vblank().map(u64::from) else {
            return Ok(None);
        };
        // The next LY145 starts 456 dots after the next LY144 edge. Poll reads
        // are eight dots into each 32-dot read/compare/not-taken-branch loop.
        let ly145 = vblank + 456;
        let poll = ly145.saturating_sub(8).div_ceil(32);
        let read_at = 8 + 32 * poll;
        if read_at >= vblank + 912 {
            // A poll can miss a partially elapsed LY145 line. Execute that poll
            // normally; the next call can target the following frame's line.
            return Ok(None);
        }
        if self.time.remaining() >= read_at + 20 {
            self.time.elapse_reserved_interval(read_at)?;
            let ly = self
                .time
                .read_display_port(0xff44)
                .map_err(DisplayWorkError::Device)?;
            debug_assert_eq!(ly, 145);
            self.time.elapse_reserved(20)?;
            return Ok(Some(ly));
        }
        // Preserve the start-before-deadline rule near a grant boundary. Only
        // whole unsuccessful loops are coalesced; an overhanging final read is
        // left to the ordinary instruction path on the next call.
        let count = (self.time.remaining() / 32).min(poll);
        if count == 0 {
            return Ok(None);
        }
        self.time.elapse_reserved_interval(32 * (count - 1) + 8)?;
        let ly = self
            .time
            .read_display_port(0xff44)
            .map_err(DisplayWorkError::Device)?;
        debug_assert_ne!(ly, 145);
        self.time.elapse_reserved(24)?;
        Ok(Some(ly))
    }

    fn execute_shadow_oam_run<I: Iterator<Item = DisplayInstruction>>(
        &mut self,
        instructions: I,
    ) -> Result<usize, Self::Error> {
        if !self.time.active_vblank || self.time.device.dma_active()
            || self.time.shadow_oam.len() != 160
        {
            return Ok(0);
        }
        let eligible = |i: &DisplayInstruction| match i.access {
            None => true,
            Some(DisplayAccess::Write { port, at_tcycle, .. }) =>
                (0xc300..0xc3a0).contains(&port) && at_tcycle <= i.tcycles,
            _ => false,
        };
        let mut instructions = instructions.peekable();
        if !instructions.peek().is_some_and(eligible) { return Ok(0); }
        self.reserve()?;
        let available = self
            .time
            .remaining()
            .min(u64::from(u32::MAX - u32::from(u8::MAX))) as u32;
        let (mut cycles, mut count) = (0u32, 0usize);
        for instruction in instructions {
            if cycles >= available || !eligible(&instruction) { break; }
            if let Some(DisplayAccess::Write { port, value, .. }) = instruction.access {
                self.time.shadow_oam[usize::from(port - 0xc300)] = value;
            }
            cycles += u32::from(instruction.tcycles);
            count += 1;
        }
        // No instruction above starts DMA, writes display IO or permits a
        // nested IRQ. LCD work reads hardware OAM, not this RAM page. Advance
        // its real edges once against unchanged hardware state, then expose
        // exactly the RAM bytes/cursor of the last admitted instruction.
        self.time.elapse_reserved_interval(u64::from(cycles))?;
        Ok(count)
    }
    fn execute_shadow_oam_total<I: Iterator<Item = (u8, u8)>>(
        &mut self,
        stores: I,
        cycles: u32,
    ) -> Result<bool, Self::Error> {
        let foreground_limited = !self.time.active_vblank
            && (!self.time.vblank_dispatch_enabled || self.time.pending_vblank_dispatch());
        if foreground_limited
            || self.time.device.dma_active()
            || self.time.shadow_oam.len() != 160
            || cycles == 0
            || self.time.remaining() < u64::from(cycles)
            || (!self.time.active_vblank
                && self.time.device.tcycles_until_vblank().map_or(0, u64::from) < u64::from(cycles))
        {
            return Ok(false);
        }
        let mut stores = stores;
        let Some((first_offset, first_value)) = stores.next() else {
            return Ok(false);
        };
        self.reserve()?;
        let first_index = usize::from(first_offset);
        assert!(first_index < 160);
        unsafe {
            self.time
                .shadow_oam
                .as_mut_ptr()
                .add(first_index)
                .write(first_value)
        };
        for (offset, value) in stores {
            let index = usize::from(offset);
            assert!(index < 160);
            unsafe { self.time.shadow_oam.as_mut_ptr().add(index).write(value) };
        }
        self.time.elapse_reserved_interval(u64::from(cycles))?;
        Ok(true)
    }

    fn execute_shadow_oam_clear(&mut self, cycles: u32) -> Result<bool, Self::Error> {
        let foreground_limited = !self.time.active_vblank
            && (!self.time.vblank_dispatch_enabled || self.time.pending_vblank_dispatch());
        if foreground_limited
            || self.time.device.dma_active()
            || self.time.shadow_oam.len() != 160
            || cycles == 0
            || self.time.remaining() < u64::from(cycles)
            || (!self.time.active_vblank
                && self.time.device.tcycles_until_vblank().map_or(0, u64::from) < u64::from(cycles))
        {
            return Ok(false);
        }
        self.reserve()?;
        self.time.shadow_oam.fill(0);
        self.time.elapse_reserved_interval(u64::from(cycles))?;
        Ok(true)
    }

    fn execute_plain_run<I: Iterator<Item = DisplayInstruction>>(
        &mut self,
        instructions: I,
    ) -> Result<usize, Self::Error> {
        let interrupt_limited = !self.time.active_vblank && self.time.vblank_dispatch_enabled;
        if !self.time.active_vblank
            && (!self.time.vblank_dispatch_enabled || self.time.pending_vblank_dispatch())
        {
            return Ok(0);
        }
        let mut instructions = instructions.peekable();
        if instructions.peek().is_none_or(|i| i.access.is_some()) {
            return Ok(0);
        }
        self.reserve()?;
        // A foreground instruction that starts before VBlank still finishes
        // before interrupt dispatch. Its successor observes the pending edge.
        let available = self
            .time
            .remaining()
            .min(if interrupt_limited {
                self.time
                    .device
                    .tcycles_until_vblank()
                    .map(u64::from)
                    .unwrap_or(u64::MAX)
            } else {
                u64::MAX
            })
            .min(u64::from(u32::MAX - u32::from(u8::MAX))) as u32;
        let (mut cycles, mut count) = (0u32, 0usize);
        for instruction in instructions {
            if instruction.access.is_some() || cycles >= available {
                break;
            }
            cycles += u32::from(instruction.tcycles);
            count += 1;
        }
        // No source writes/effects occur inside this interval. The real
        // device still processes every LCD/DMA edge against unchanged shadow
        // OAM; divider wrap and final-instruction budget overrun are exact.
        self.time.elapse_reserved_interval(u64::from(cycles))?;
        Ok(count)
    }
    #[inline(always)]
    fn execute_plain_cycles(&mut self, cycles: &[u8]) -> Result<usize, Self::Error> {
        DisplayBus::execute_plain_cycles(self, cycles)
    }
    #[inline(always)]
    fn execute_plain_cycles_total(
        &mut self,
        cycles: &[u8],
        total: u64,
    ) -> Result<usize, Self::Error> {
        if self.time.active_vblank && !cycles.is_empty() && total <= self.time.remaining() {
            self.reserve()?;
            self.time.elapse_reserved_interval(total)?;
            return Ok(cycles.len());
        }
        self.execute_plain_cycles(cycles)
    }
    #[inline]
    fn execute_plain_total(&mut self, total: u32) -> Result<bool, Self::Error> {
        let interrupt_limited = !self.time.active_vblank && self.time.vblank_dispatch_enabled;
        if total == 0
            || (!self.time.active_vblank
                && (!self.time.vblank_dispatch_enabled || self.time.pending_vblank_dispatch()))
        {
            return Ok(false);
        }
        let limit = self.time.remaining().min(if interrupt_limited {
            self.time
                .device
                .tcycles_until_vblank()
                .map_or(u64::MAX, u64::from)
        } else {
            u64::MAX
        });
        if u64::from(total) > limit {
            return Ok(false);
        }
        self.reserve()?;
        self.time.elapse_reserved_interval(u64::from(total))?;
        Ok(true)
    }
    fn execute_joypad_reads(
        &mut self,
        select: Option<u8>,
        count: u8,
    ) -> Result<Option<(u8, u8)>, Self::Error> {
        if !self.time.active_vblank || count == 0 {
            return Ok(None);
        }
        self.reserve()?;
        let prefix = u64::from(select.is_some()) * 12;
        let accepted =
            ((self.time.remaining().saturating_sub(prefix) / 12).min(u64::from(count))) as u8;
        if let Some(select) = select {
            self.time.joypad_select = select & 0x30;
        }
        self.time
            .elapse_reserved_interval(prefix + u64::from(accepted) * 12)?;
        let value = self
            .time
            .read_display_port(0xff00)
            .map_err(DisplayWorkError::Device)?;
        Ok(Some((accepted, value)))
    }
    fn execute_irq_random(
        &mut self,
        carry: bool,
    ) -> Result<Option<(u8, u8, bool, u8, u8)>, Self::Error> {
        if !self.time.active_vblank || self.time.remaining() < 104 {
            return Ok(None);
        }
        self.reserve()?;
        let first_div = (self.time.divider_counter.wrapping_add(8) >> 8) as u8;
        let second_div = (self.time.divider_counter.wrapping_add(52) >> 8) as u8;
        let add = self.time.random_add;
        let sub = self.time.random_sub;
        let sum = u16::from(add) + u16::from(first_div) + u16::from(carry);
        let new_add = sum as u8;
        let rhs = u16::from(second_div) + u16::from(sum > 255);
        let new_carry = u16::from(sub) < rhs;
        let new_sub = sub.wrapping_sub(rhs as u8);
        self.time.elapse_reserved_interval(104)?;
        self.time.random_add = new_add;
        self.time.random_sub = new_sub;
        Ok(Some((new_add, new_sub, new_carry, new_sub, second_div)))
    }
    fn execute_sprite_bank_call(
        &mut self,
        target_bank: u8,
        restore_bank: Option<u8>,
    ) -> Result<Option<u8>, Self::Error> {
        let cycles = if restore_bank.is_some() { 60 } else { 136 };
        if !self.time.active_vblank || self.time.remaining() < cycles {
            return Ok(None);
        }
        self.reserve()?;
        let bank = restore_bank.unwrap_or(self.time.loaded_rom_bank);
        self.time.elapse_reserved_interval(cycles)?;
        self.time.loaded_rom_bank = bank;
        self.time.selected_rom_bank = bank;
        Ok(Some(bank))
    }
    fn execute_audio_bank_dispatch(&mut self, bank: u8) -> Result<Option<(u8, bool)>, Self::Error> {
        let (cycles, engine, alarm) = match bank {
            2 => (84, 1, false),
            8 => (104, 2, true),
            _ => (108, 3, false),
        };
        if !self.time.active_vblank || self.time.remaining() < cycles {
            return Ok(None);
        }
        self.reserve()?;
        self.time.elapse_reserved_interval(cycles)?;
        self.time.loaded_rom_bank = bank;
        self.time.selected_rom_bank = bank;
        Ok(Some((engine, alarm)))
    }
    fn execute_joypad_frame(&mut self) -> Result<Option<(u8, u8)>, Self::Error> {
        const CYCLES: u64 = 328;
        if !self.time.active_vblank || self.time.remaining() <= CYCLES - 16 {
            return Ok(None);
        }
        self.reserve()?;
        let directions = self.time.physical_buttons & 0xf0;
        let input = directions | (self.time.physical_buttons & 15);
        self.time.elapse_reserved_interval(CYCLES)?;
        self.time.joy_input = input;
        self.time.joypad_select = 0x30;
        Ok(Some((0x30, directions)))
    }
    fn execute_buffer_copy(
        &mut self,
        source: &[u8],
        destination: &mut [u8],
        finish_at_final_store: bool,
    ) -> Result<usize, Self::Error> {
        DisplayBus::execute_buffer_copy(self, source, destination, finish_at_final_store)
    }
    #[inline(always)]
    fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
        match instruction.access {
            None => {
                self.reserve()?;
                self.time.elapse_reserved(instruction.tcycles)?;
                Ok(None)
            }
            Some(DisplayAccess::Read { port, at_tcycle }) => {
                // Preserve the checked path's port-error-before-budget order.
                self.time
                    .read_display_port(port)
                    .map_err(DisplayWorkError::Device)?;
                self.reserve()?;
                self.time.elapse_reserved(at_tcycle)?;
                let value = self
                    .time
                    .read_display_port(port)
                    .map_err(DisplayWorkError::Device)?;
                self.time.elapse_reserved(instruction.tcycles - at_tcycle)?;
                Ok(Some(value))
            }
            Some(DisplayAccess::Write {
                port,
                value,
                at_tcycle,
            }) if super::video_request_ports::VideoRequestPorts::contains(port) => {
                self.reserve()?;
                self.time.elapse_reserved(at_tcycle)?;
                if !self.time.vblank_dispatch_enabled
                    && matches!(
                        port,
                        super::video_copy_caller_work::AUTO_BG_ENABLED
                            | super::video_copy_caller_work::AUTO_BG_PORTION
                    )
                {
                    self.time.materialize_native_background();
                    self.validated = false;
                }
                self.time.video_requests.write(port, value);
                self.time.elapse_reserved(instruction.tcycles - at_tcycle)?;
                Ok(None)
            }
            Some(DisplayAccess::Write {
                port,
                value,
                at_tcycle,
            }) if matches!(port, 0xff0f | 0xffff | 0xff40..=0xff46 | 0xff4a | 0xff4b) => {
                self.time
                    .device
                    .validate_write_port(port, value, 0xc3)
                    .map_err(DisplayWorkError::Device)?;
                self.reserve()?;
                self.time.elapse_reserved(at_tcycle)?;
                self.time
                    .device
                    .write_port(port, value, 0xc3)
                    .map_err(DisplayWorkError::Device)?;
                self.validated = false;
                self.time.elapse_reserved(instruction.tcycles - at_tcycle)?;
                Ok(None)
            }
            Some(DisplayAccess::Write {
                port,
                value,
                at_tcycle,
            }) if matches!(
                port,
                super::far_pattern_copy_work::LOADED_ROM_BANK
                    | super::far_pattern_copy_work::ROM_BANK_SELECT
                    | super::far_pattern_copy_work::ROM_BANK_TEMP
                    | super::display_delay_work::VBLANK_FLAG_PORT
                    | super::sprite_update_control_work::SPRITE_UPDATES_PORT
                    | 0xff00
                    | 0xff04
                    | 0xfff8
                    | 0xfff9
                    | 0xffd3
                    | 0xffd4
                    | 0xffd5
            ) || (0xc300..0xc3a0).contains(&port) =>
            {
                self.reserve()?;
                self.time.elapse_reserved(at_tcycle)?;
                match port {
                    super::far_pattern_copy_work::LOADED_ROM_BANK => {
                        self.time.loaded_rom_bank = value
                    }
                    super::far_pattern_copy_work::ROM_BANK_SELECT => {
                        self.time.selected_rom_bank = value
                    }
                    super::far_pattern_copy_work::ROM_BANK_TEMP => self.time.rom_bank_temp = value,
                    super::display_delay_work::VBLANK_FLAG_PORT => self.time.vblank_flag = value,
                    super::sprite_update_control_work::SPRITE_UPDATES_PORT => {
                        self.time.sprite_updates_enabled = Some(value);
                    }
                    0xff00 => self.time.joypad_select = value & 0x30,
                    0xff04 => self.time.divider_counter = 0,
                    0xfff8 => self.time.joy_input = value,
                    0xfff9 => self.time.disable_joypad_polling = value,
                    0xffd3 => self.time.random_add = value,
                    0xffd4 => self.time.random_sub = value,
                    0xffd5 => self.time.frame_counter = value,
                    0xc300..=0xc39f => self.time.shadow_oam[usize::from(port - 0xc300)] = value,
                    _ => unreachable!(),
                }
                self.time.elapse_reserved(instruction.tcycles - at_tcycle)?;
                Ok(None)
            }
            Some(DisplayAccess::Write { .. }) => {
                self.validated = false;
                DisplayInstructionBus::execute(self.time, instruction)
            }
        }
    }
}

impl super::vblank_return_work::VBlankReturnBus for DisplayBus<'_> {
    fn check_active_vblank(&self) -> Result<(), Self::Error> {
        super::vblank_return_work::VBlankReturnBus::check_active_vblank(self.time)
    }
    fn execute_reti(&mut self) -> Result<(), Self::Error> {
        self.validated = false;
        super::vblank_return_work::VBlankReturnBus::execute_reti(self.time)
    }
}

pub struct PatternBus<'a, A> {
    time: &'a mut DisplayWorkTime,
    patterns: &'a mut PatternMemory,
    assets: &'a A,
    // These checks are valid only for this exclusive borrow. A new bus starts
    // unchecked, including after loading or replacing any snapshot.
    validated: bool,
    memory_valid: bool,
    #[cfg(test)]
    check_each_instruction: bool,
}
impl<A> DisplayInstructionBus for PatternBus<'_, A> {
    type Error = DisplayWorkError;
    #[inline(always)]
    fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
        #[cfg(test)]
        if self.check_each_instruction {
            return DisplayInstructionBus::execute(self.time, instruction);
        }
        // Both instruction kinds share one exclusive time lease. A checked
        // display write invalidates it just as it does on DisplayBus; plain
        // instructions and reads can keep the same validated device.
        let mut display = DisplayBus { time: self.time, validated: self.validated };
        let result = display.execute(instruction);
        self.validated = display.validated;
        result
    }
}

impl<A: PatternAssets> PatternInstructionBus for PatternBus<'_, A> {
    type Error = DisplayWorkError;

    #[inline(always)]
    fn execute_plain_cycles(&mut self, instructions: &[u8]) -> Result<usize, Self::Error> {
        #[cfg(test)]
        if self.check_each_instruction {
            return Ok(0);
        }
        let mut display = DisplayBus {
            time: self.time,
            validated: self.validated,
        };
        let result = display.execute_plain_cycles(instructions);
        self.validated = display.validated;
        result
    }

    fn try_copydata_run(
        &mut self,
        source: PatternSource,
        source_offset: u16,
        destination: u16,
        remaining: u16,
    ) -> Result<Option<u16>, Self::Error> {
        #[cfg(test)]
        if self.check_each_instruction {
            return Ok(None);
        }
        if remaining == 0 || self.patterns.bytes().len() != 0x2000 {
            return Ok(None);
        }
        let mut count = remaining.min(256);
        if self.time.pending_vblank_dispatch() {
            return Ok(None);
        }
        if !self.time.active_vblank && self.time.device.lcd_enabled() {
            let Some(distance) = self.time.device.tcycles_until_vblank() else {
                return Ok(None);
            };
            count = count.min((u64::from(distance).saturating_sub(1) / 52).min(256) as u16);
        }
        count = count.min((self.time.remaining() / 52).min(256) as u16);
        if count == 0 {
            return Ok(None);
        }
        if let PatternSource::Tileset(raw) = source {
            let mut allowed = 0;
            while allowed < count {
                use super::tileset_pattern_metadata::{tileset_pattern_byte, TilesetPatternByte};
                let offset = source_offset.wrapping_add(allowed);
                if matches!(
                    tileset_pattern_byte(raw, offset),
                    Some(TilesetPatternByte::Vram(_))
                ) {
                    break;
                }
                allowed += 1;
            }
            count = allowed;
        }
        if count == 0
            || self.patterns.read(destination).is_none()
            || self
                .patterns
                .read(destination.wrapping_add(count - 1))
                .is_none()
        {
            return Ok(None);
        }
        let mut values = [0u8; 256];
        if self
            .assets
            .read_pattern_run(source, source_offset, &mut values[..usize::from(count)])
            .is_none()
        {
            return Ok(None);
        }
        let accepted = count;
        let cycles = 52 * u64::from(accepted) - if accepted == remaining { 4 } else { 0 };
        if self.time.remaining() < cycles || !self.time.device.has_no_event_within_long(cycles) {
            return Ok(None);
        }
        self.check_memory()?;
        self.reserve()?;
        let accessible = self.time.device.vram_accessible();
        if accessible {
            self.patterns
                .write_run(destination, &values[..usize::from(accepted)])
                .expect("validated CopyData destination");
            self.time
                .device
                .record_native_background_run(destination, accepted);
        }
        self.time.elapse(cycles)?;
        Ok(Some(accepted))
    }

    fn try_vblank_live_header(
        &mut self,
        format: CopyFormat,
        size: u8,
        source: u16,
        destination: u16,
    ) -> Result<bool, Self::Error> {
        let request = self.time.video_requests.snapshot(format);
        if !self.time.active_vblank
            || self.time.remaining() < 172
            || request.size != Some(size)
            || request.source != Some(source)
            || request.destination != Some(destination)
        {
            return Ok(false);
        }
        self.reserve()?;
        self.time.elapse_reserved_interval(172)?;
        let port = if format == CopyFormat::TwoBpp {
            super::video_copy_caller_work::COPY_SIZE
        } else {
            super::video_copy_caller_work::DOUBLE_SIZE
        };
        self.time.video_requests.write(port, 0);
        Ok(true)
    }

    fn try_vblank_live_footer(
        &mut self,
        format: CopyFormat,
        size: u8,
        source: u16,
        destination: u16,
        final_source: u16,
        final_destination: u16,
    ) -> Result<bool, Self::Error> {
        let request = self.time.video_requests.snapshot(format);
        if !self.time.active_vblank
            || self.time.remaining() < 132
            || request.size != Some(0)
            || request.source != Some(source)
            || request.destination != Some(destination)
            || size == 0
        {
            return Ok(false);
        }
        self.reserve()?;
        self.time.elapse_reserved_interval(132)?;
        let port = if format == CopyFormat::TwoBpp {
            super::video_copy_caller_work::COPY_SIZE
        } else {
            super::video_copy_caller_work::DOUBLE_SIZE
        };
        self.time.video_requests.write(port + 1, final_source as u8);
        self.time
            .video_requests
            .write(port + 2, (final_source >> 8) as u8);
        self.time
            .video_requests
            .write(port + 3, final_destination as u8);
        self.time
            .video_requests
            .write(port + 4, (final_destination >> 8) as u8);
        Ok(true)
    }

    #[inline(always)]
    fn try_vblank_pop_pair_writes(
        &mut self,
        destination: u16,
        de: u16,
    ) -> Result<Option<u16>, Self::Error> {
        #[cfg(test)]
        if self.check_each_instruction {
            return Ok(None);
        }
        if !self.time.active_vblank
            || self.patterns.bytes().len() != 0x2000
            || (!self.validated && self.time.shadow_oam.len() != 160)
            || !(0x8000..0xa000).contains(&destination)
            || self.time.remaining() < 36
            || self.time.elapsed.checked_add(36).is_none()
            || !self.time.device.unobserved_vram_window(36)
        {
            return Ok(None);
        }
        self.check_memory()?;
        self.reserve()?;
        if self.time.device.vram_accessible() {
            self.patterns
                .write(destination, de as u8)
                .expect("validated AutoBG destination");
            self.time.device.record_native_background_write(destination);
            let high = (destination & 0xff00) | u16::from((destination as u8).wrapping_add(1));
            self.patterns
                .write(high, (de >> 8) as u8)
                .expect("validated AutoBG destination");
            self.time.device.record_native_background_write(high);
        }
        self.time.elapse_reserved_interval(36)?;
        Ok(Some(
            (destination & 0xff00) | u16::from((destination as u8).wrapping_add(2)),
        ))
    }

    fn try_vblank_pop_row_writes(
        &mut self,
        source: &[u8],
        destination: u16,
    ) -> Result<Option<u16>, Self::Error> {
        const CYCLES: u64 = 356;
        #[cfg(test)]
        if self.check_each_instruction {
            return Ok(None);
        }
        if source.len() != 20
            || !self.time.active_vblank
            || self.patterns.bytes().len() != 0x2000
            || (!self.validated && self.time.shadow_oam.len() != 160)
            || !(0x8000..0xa000).contains(&destination)
            || destination & 0xff > 236
            || self.time.remaining() < CYCLES
            || self.time.elapsed.checked_add(CYCLES).is_none()
            || !self.time.device.unobserved_vram_window(CYCLES)
        {
            return Ok(None);
        }
        self.check_memory()?;
        self.reserve()?;
        if self.time.device.vram_accessible() {
            self.patterns
                .write_run(destination, source)
                .expect("validated AutoBG row destination");
            self.time
                .device
                .record_native_background_run(destination, 20);
        }
        self.time.elapse_reserved_interval(CYCLES)?;
        Ok(Some(
            (destination & 0xff00) | u16::from((destination as u8).wrapping_add(19)),
        ))
    }

    fn try_vblank_pop_row_run(
        &mut self,
        source: &[u8],
        destination: u16,
        rows: u8,
    ) -> Result<Option<(u8, u16, u16, u16, bool)>, Self::Error> {
        #[cfg(test)]
        if self.check_each_instruction {
            return Ok(None);
        }
        if !self.time.active_vblank
            || rows == 0
            || self.patterns.bytes().len() != 0x2000
            || (!self.validated && self.time.shadow_oam.len() != 160)
            || !(0x8000..0xa000).contains(&destination)
        {
            return Ok(None);
        }
        let Some(window) = self.time.device.unobserved_vram_limit() else {
            return Ok(None);
        };
        let mut count = u64::from(rows)
            .min((self.time.remaining() + 4) / 400)
            .min(window.saturating_add(3) / 400)
            .min((source.len() / 20) as u64);
        loop {
            if count == 0 {
                return Ok(None);
            }
            let cycles = count * 400 - if count == u64::from(rows) { 4 } else { 0 };
            let last_destination = (0..count).try_fold(destination, |at, _| {
                (at & 0xff <= 236).then_some(at.wrapping_add(32))
            });
            if cycles <= self.time.remaining()
                && cycles < window
                && last_destination.is_some()
                && self.time.elapsed.checked_add(cycles).is_some()
            {
                break;
            }
            count -= 1;
        }
        self.check_memory()?;
        self.reserve()?;
        let accessible = self.time.device.vram_accessible();
        let mut sp = 0u16;
        let mut hl = destination;
        let mut de = 0;
        let mut carry = false;
        for index in 0..count {
            let at = destination.wrapping_add((index * 32) as u16);
            if accessible {
                let row = &source[index as usize * 20..index as usize * 20 + 20];
                self.patterns
                    .write_run(at, row)
                    .expect("validated row-run VRAM destination");
                self.time.device.record_native_background_run(at, 20);
            }
            sp = sp.wrapping_add(20);
            let row = &source[index as usize * 20..index as usize * 20 + 20];
            de = u16::from_be_bytes([row[19], row[18]]);
            let last = at.wrapping_add(19);
            let sum = 13 + u16::from(last as u8);
            carry = sum > 255;
            hl = last.wrapping_add(13);
        }
        let cycles = count * 400 - if count == u64::from(rows) { 4 } else { 0 };
        self.time.elapse_reserved_interval(cycles)?;
        Ok(Some((count as u8, sp, hl, de, carry)))
    }

    fn try_vblank_tile_run<const DOUBLE: bool>(
        &mut self,
        source: PatternSource,
        source_offset: u16,
        destination: u16,
        tiles: u8,
    ) -> Result<Option<(u8, u16, u16, u16)>, Self::Error> {
        #[cfg(test)]
        if self.check_each_instruction {
            return Ok(None);
        }
        let source_bytes = if DOUBLE { 8u64 } else { 16 };
        let tile_cycles = if DOUBLE { 260u64 } else { 308 };
        if !self.time.active_vblank
            || tiles == 0
            || self.patterns.bytes().len() != 0x2000
            || (!self.validated && self.time.shadow_oam.len() != 160)
            || !(0x8000..0xa000).contains(&destination)
            || destination & 0xff > 0xf0
        {
            return Ok(None);
        }
        let Some(window) = self.time.device.unobserved_vram_limit() else {
            return Ok(None);
        };
        let mut count = u64::from(tiles)
            .min((self.time.remaining() + 4) / tile_cycles)
            .min(window.saturating_add(3) / tile_cycles)
            .min(u64::from(0xa000 - destination) / 16);
        loop {
            if count == 0 {
                return Ok(None);
            }
            let cycles = count * tile_cycles - if count == u64::from(tiles) { 4 } else { 0 };
            if cycles <= self.time.remaining() && cycles < window {
                break;
            }
            count -= 1;
        }
        if self
            .time
            .elapsed
            .checked_add(count * tile_cycles - 4)
            .is_none()
        {
            return Ok(None);
        }
        self.check_memory()?;
        self.reserve()?;
        let accessible = self.time.device.vram_accessible();
        let mut accepted = 0;
        let mut last_de = 0;
        for index in 0..count {
            let offset = source_offset.wrapping_add((index * source_bytes) as u16);
            let destination = destination.wrapping_add((index * 16) as u16);
            let mut values = [0; 16];
            if self
                .assets
                .read_pattern_run(source, offset, &mut values[..source_bytes as usize])
                .is_none()
            {
                break;
            }
            if accessible {
                if DOUBLE {
                    let mut expanded = [0; 16];
                    for (out, &value) in expanded
                        .chunks_exact_mut(2)
                        .zip(&values[..source_bytes as usize])
                    {
                        out.fill(value);
                    }
                    self.patterns
                        .write_run(destination, &expanded)
                        .expect("validated tile-run VRAM destination");
                } else {
                    self.patterns
                        .write_run(destination, &values)
                        .expect("validated tile-run VRAM destination");
                }
                self.time
                    .device
                    .record_native_background_run(destination, 16);
            }
            last_de = u16::from_le_bytes([
                values[source_bytes as usize - 2],
                values[source_bytes as usize - 1],
            ]);
            accepted += 1;
        }
        if accepted == 0 {
            return Ok(None);
        }
        self.time.elapse_reserved_interval(
            accepted * tile_cycles - if accepted == u64::from(tiles) { 4 } else { 0 },
        )?;
        Ok(Some((
            accepted as u8,
            source_offset.wrapping_add((accepted * source_bytes) as u16),
            destination.wrapping_add((accepted * 16) as u16),
            last_de,
        )))
    }

    #[inline(always)]
    fn try_vblank_tile_writes<const DOUBLE: bool>(
        &mut self,
        source: PatternSource,
        source_offset: u16,
        destination: u16,
    ) -> Result<Option<(u16, u16, u16)>, Self::Error> {
        #[cfg(test)]
        if self.check_each_instruction {
            return Ok(None);
        }
        let writes = if DOUBLE { 4 } else { 2 };
        let pairs = if DOUBLE { 4 } else { 8 };
        let source_bytes = if DOUBLE { 8 } else { 16 };
        let cycles = pairs * (12 + writes * 12) + 4;
        if !self.time.active_vblank
            || self.patterns.bytes().len() != 0x2000
            || (!self.validated && self.time.shadow_oam.len() != 160)
            || !(0x8000..0xa000).contains(&destination)
            || destination & 0xff > 0xf0
            || self.time.remaining() < u64::from(cycles)
            || self.time.elapsed.checked_add(u64::from(cycles)).is_none()
            || !self.time.device.unobserved_vram_window(u64::from(cycles))
        {
            return Ok(None);
        }
        let mut values = [0; 16];
        if self
            .assets
            .read_pattern_run(source, source_offset, &mut values[..source_bytes])
            .is_none()
        {
            return Ok(None);
        }
        self.check_memory()?;
        self.reserve()?;
        if self.time.device.vram_accessible() {
            if DOUBLE {
                let mut expanded = [0; 16];
                for (out, &value) in expanded.chunks_exact_mut(2).zip(&values[..source_bytes]) {
                    out.fill(value);
                }
                self.patterns
                    .write_run(destination, &expanded)
                    .expect("validated tile VRAM destination");
            } else {
                self.patterns
                    .write_run(destination, &values)
                    .expect("validated tile VRAM destination");
            }
            self.time
                .device
                .record_native_background_run(destination, 16);
        }
        self.time.elapse_reserved_interval(cycles)?;
        let de = u16::from_le_bytes([values[source_bytes - 2], values[source_bytes - 1]]);
        Ok(Some((
            source_offset.wrapping_add(source_bytes as u16),
            destination.wrapping_add(16),
            de,
        )))
    }

    #[inline(always)]
    fn try_vblank_pair_writes<const DOUBLE: bool>(
        &mut self,
        mut destination: u16,
        de: u16,
        last_pair: bool,
    ) -> Result<Option<u16>, Self::Error> {
        #[cfg(test)]
        if self.check_each_instruction {
            return Ok(None);
        }
        let writes = if DOUBLE { 4 } else { 2 };
        // LD [HL],r (8), INC L (4); the final INC HL takes 8 instead.
        let cycles = writes * 12 + if last_pair { 4 } else { 0 };
        if !self.time.active_vblank
            || self.patterns.bytes().len() != 0x2000
            || (!self.validated && self.time.shadow_oam.len() != 160)
            || !(0x8000..0xa000).contains(&destination)
            || self.time.remaining() < u64::from(cycles)
            || self.time.elapsed.checked_add(u64::from(cycles)).is_none()
            || !self.time.device.unobserved_vram_window(u64::from(cycles))
        {
            return Ok(None);
        }
        // Every store stays in this same 256-byte page. Only the increment
        // after the last store can change the high byte. The checked VBlank
        // window keeps VRAM accessible without a background scanout observer.
        // Refusal and other raster phases use execute() instead.
        self.check_memory()?;
        self.reserve()?;
        let accessible = self.time.device.vram_accessible();
        for write in 0..writes {
            let low = if DOUBLE { write < 2 } else { write == 0 };
            if accessible {
                self.patterns
                    .write(destination, if low { de as u8 } else { (de >> 8) as u8 })
                    .expect("validated same-page VRAM destination");
                self.time.device.record_native_background_write(destination);
            }
            destination = if last_pair && write + 1 == writes {
                destination.wrapping_add(1)
            } else {
                (destination & 0xff00) | u16::from((destination as u8).wrapping_add(1))
            };
        }
        self.elapse(cycles)?;
        Ok(Some(destination))
    }
    #[inline(always)]
    fn execute(&mut self, instruction: PatternInstruction) -> Result<Option<u16>, Self::Error> {
        if let Some(PatternAccess::Display(access)) = instruction.access {
            return DisplayInstructionBus::execute(self, DisplayInstruction {
                tcycles: instruction.tcycles, access: Some(access),
            }).map(|value| value.map(u16::from));
        }
        if instruction.access.is_none() {
            self.check_memory()?;
            self.reserve()?;
            self.elapse(instruction.tcycles)?;
            return Ok(None);
        }
        self.execute_pattern_general(instruction)
    }
}

#[cfg(test)]
#[path = "display_vblank_pair_tests.rs"]
mod vblank_pair_tests;

impl<A: PatternAssets> PatternBus<'_, A> {
    #[cfg(test)]
    fn checking_each_instruction(mut self) -> Self {
        self.check_each_instruction = true;
        self
    }

    #[inline(always)]
    fn check_memory(&mut self) -> Result<(), DisplayWorkError> {
        #[cfg(test)]
        if self.check_each_instruction {
            return if self.patterns.bytes().len() == 0x2000 {
                Ok(())
            } else {
                Err(DisplayWorkError::Resource(
                    RestoreError::InvalidPatternMemory,
                ))
            };
        }
        if !self.memory_valid {
            if self.patterns.bytes().len() != 0x2000 {
                return Err(DisplayWorkError::Resource(
                    RestoreError::InvalidPatternMemory,
                ));
            }
            self.memory_valid = true;
        }
        Ok(())
    }
    #[inline(always)]
    fn reserve(&mut self) -> Result<(), DisplayWorkError> {
        #[cfg(test)]
        if self.check_each_instruction {
            return self.time.reserve();
        }
        if !self.validated {
            self.time.reserve()?;
            self.validated = true;
        } else {
            debug_assert!(self.time.device.validate().is_ok());
            debug_assert_eq!(self.time.device.observed_tcycles(), self.time.elapsed);
            if self.time.elapsed >= self.time.deadline {
                return Err(DisplayWorkError::Budget);
            }
            self.time.check_interrupt_dispatch()?;
        }
        Ok(())
    }
    #[inline(always)]
    fn elapse(&mut self, cycles: u8) -> Result<(), DisplayWorkError> {
        #[cfg(test)]
        if self.check_each_instruction {
            return self.time.elapse(u64::from(cycles));
        }
        self.time.elapse_reserved(cycles)
    }

    #[inline(always)]
    fn execute_pattern_general(
        &mut self,
        instruction: PatternInstruction,
    ) -> Result<Option<u16>, DisplayWorkError> {
        self.check_memory()?;
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
            Some(PatternAccess::ReadPair { source, offset, .. }) => Some(
                self.assets
                    .read_pattern_pair(source, offset)
                    .map_err(|offset| {
                        DisplayWorkError::Resource(RestoreError::MissingSource(source, offset))
                    })?,
            ),
            Some(PatternAccess::Write { destination, .. }) => {
                self.patterns
                    .read(destination)
                    .ok_or(DisplayWorkError::Resource(
                        RestoreError::InvalidDestination(destination),
                    ))?;
                None
            }
            Some(PatternAccess::Display(_)) => unreachable!("delegated before pattern validation"),
            None => None,
        };
        self.reserve()?;
        let consumed = match instruction.access {
            Some(PatternAccess::ReadVram { address, at_tcycle }) => {
                self.elapse(at_tcycle)?;
                read = Some(u16::from(if self.time.device.vram_accessible() {
                    self.patterns.read(address).expect("validated VRAM address")
                } else {
                    0xff
                }));
                at_tcycle
            }
            Some(PatternAccess::Read { at_tcycle, .. }) => {
                self.elapse(at_tcycle)?;
                at_tcycle
            }
            Some(PatternAccess::ReadPair {
                first_at_tcycle,
                second_at_tcycle,
                ..
            }) => {
                self.elapse(first_at_tcycle)?;
                self.elapse(second_at_tcycle - first_at_tcycle)?;
                second_at_tcycle
            }
            Some(PatternAccess::Write {
                destination,
                value,
                at_tcycle,
            }) => {
                self.elapse(at_tcycle)?;
                if self.time.device.vram_accessible() {
                    self.patterns
                        .write(destination, value)
                        .map_err(DisplayWorkError::Resource)?;
                    self.time.device.record_native_background_write(destination);
                }
                at_tcycle
            }
            Some(PatternAccess::Display(_)) => unreachable!("delegated before pattern validation"),
            None => 0,
        };
        self.elapse(instruction.tcycles - consumed)?;
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
    #[test]
    fn native_background_phase_tracks_lcd_edges_and_survives_irq_and_snapshot_handoffs() {
        use super::super::video_copy_caller_work::AUTO_BG_PORTION;
        use super::super::vblank_return_work::VBlankReturnBus;
        let mut time = DisplayWorkTime::default();
        time.set_native_background_enabled(1).unwrap();
        time.begin_native_update();
        assert_eq!(time.background_portion(), Some(0), "grant is not a transfer");
        time.idle_to_target().unwrap();
        assert_eq!(time.background_portion(), Some(1));
        time.set_native_background_enabled(0).unwrap();
        for _ in 0..4 { time.begin_native_update(); time.idle_to_target().unwrap(); }
        assert_eq!(time.background_portion(), Some(1), "disabled AutoBG must keep its phase");
        time.set_native_background_enabled(1).unwrap();
        time.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
        let epoch = time.device().vblank_epoch();
        for _ in 0..3 { time.begin_native_update(); time.idle_to_target().unwrap(); }
        assert_eq!(time.device().vblank_epoch(), epoch);
        assert_eq!(time.background_portion(), Some(1), "LCD-off updates are not transfers");
        time.device_mut().write_port(0xff40, 0x91, 0xc3).unwrap();
        for _ in 0..4 { time.begin_native_update(); time.idle_to_target().unwrap(); }
        let expected = (1 + (time.device().vblank_epoch() - epoch) % 3) as u8 % 3;
        assert_eq!(time.background_portion(), Some(expected));
        time.set_vblank_dispatch_enabled(true);
        for _ in 0..2 { time.begin_native_update(); time.idle_to_target().unwrap(); }
        assert_eq!(time.background_portion(), Some(expected), "source IRQ owns its own stores");
        time.begin_native_update();
        time.enter_pending_vblank().unwrap();
        time.execute(DisplayInstruction { tcycles: 12, access: Some(DisplayAccess::Write {
            port: AUTO_BG_PORTION, value: 2, at_tcycle: 8,
        }) }).unwrap();
        time.execute_reti().unwrap();
        time.set_vblank_dispatch_enabled(false);
        assert_eq!(time.background_portion(), Some(2));
        time.idle_to_target().unwrap();
        assert_eq!(time.background_portion(), Some(0), "native resumes from the actual source store");
        let mut restored: DisplayWorkTime = serde_json::from_slice(&serde_json::to_vec(&time).unwrap()).unwrap();
        for owner in [&mut time, &mut restored] {
            assert_eq!(owner.background_portion(), Some(0));
            owner.begin_native_update(); owner.idle_to_target().unwrap();
            assert_eq!(owner.background_portion(), Some(1));
        }
    }

    struct Bytes;
    impl PatternAssets for Bytes {
        fn read_pattern_byte(&self, _: PatternSource, offset: u16) -> Option<u8> {
            Some(offset as u8)
        }
    }
    #[test]
    fn far_copy_runner_preserves_mixed_bus_trace_banks_and_partial_instructions() {
        use super::super::{
            far_pattern_copy_work::FarPatternCopyCall,
            field_restore::{CopyFormat, PatternCopy},
        };
        #[derive(Debug, PartialEq, Eq)]
        enum Receipt {
            Display(DisplayInstruction, Result<Option<u8>, DisplayWorkError>),
            Pattern(PatternInstruction, Result<Option<u16>, DisplayWorkError>),
        }
        struct Recording<'a, B> {
            bus: &'a mut B,
            trace: &'a mut Vec<Receipt>,
        }
        impl<B: DisplayInstructionBus<Error = DisplayWorkError>> DisplayInstructionBus
            for Recording<'_, B>
        {
            type Error = DisplayWorkError;
            fn execute(&mut self, i: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
                let result = DisplayInstructionBus::execute(self.bus, i);
                self.trace.push(Receipt::Display(i, result));
                result
            }
        }
        impl<B: PatternInstructionBus<Error = DisplayWorkError>> PatternInstructionBus
            for Recording<'_, B>
        {
            type Error = DisplayWorkError;
            fn execute(&mut self, i: PatternInstruction) -> Result<Option<u16>, Self::Error> {
                let result = PatternInstructionBus::execute(self.bus, i);
                self.trace.push(Receipt::Pattern(i, result));
                result
            }
        }
        for lcdc in [0x11, 0x93] {
            for tiles in [0, 1, 12] {
                let budgets: Vec<u64> = if tiles == 1 {
                    (0..=1400).step_by(4).chain([4096, 70224]).collect()
                } else {
                    vec![
                        0, 4, 128, 128, 140, 152, 256, 512, 1024, 4096, 8000, 9500, 11_000, 70224,
                    ]
                };
                for (destination, source_offset) in [(0x8800, 17), (0x9fff, 65535)] {
                    for &budget in &budgets {
                        let mut time = DisplayWorkTime::default();
                        time.device_mut().write_port(0xff40, lcdc, 0xc3).unwrap();
                        time.deadline = budget;
                        let mut reference = time.clone();
                        let mut work = FarPatternCopyCall::from_2bpp(PatternCopy {
                            source: PatternSource::Sprite(14),
                            source_offset,
                            destination,
                            tiles,
                            format: CopyFormat::TwoBpp,
                        })
                        .unwrap();
                        let mut expected_work = work;
                        let mut memory = PatternMemory::default();
                        let mut expected_memory = memory.clone();
                        for _ in 0..3 {
                            let mut actual_trace = Vec::new();
                            let actual = work.run(&mut Recording {
                                bus: &mut time.pattern_bus(&mut memory, &Bytes),
                                trace: &mut actual_trace,
                            });
                            let mut expected_trace = Vec::new();
                            let expected = loop {
                                match expected_work.step(&mut Recording {
                                    bus: &mut reference
                                        .pattern_bus(&mut expected_memory, &Bytes)
                                        .checking_each_instruction(),
                                    trace: &mut expected_trace,
                                }) {
                                    Ok(true) => (),
                                    Ok(false) => break Ok(()),
                                    Err(error) => break Err(error),
                                }
                            };
                            assert_eq!(actual, expected);
                            assert_eq!(actual_trace, expected_trace);
                            assert_eq!(work, expected_work);
                            assert_eq!(memory, expected_memory);
                            assert_eq!(
                                serde_json::to_value(&time).unwrap(),
                                serde_json::to_value(&reference).unwrap()
                            );
                            if actual != Err(DisplayWorkError::Budget) {
                                break;
                            }
                            work = serde_json::from_value(serde_json::to_value(work).unwrap())
                                .unwrap();
                            expected_work = serde_json::from_value(
                                serde_json::to_value(expected_work).unwrap(),
                            )
                            .unwrap();
                            time = serde_json::from_value(serde_json::to_value(time).unwrap())
                                .unwrap();
                            reference =
                                serde_json::from_value(serde_json::to_value(reference).unwrap())
                                    .unwrap();
                            time.begin_native_update();
                            reference.begin_native_update();
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn video_runner_matches_single_instruction_receipts_and_snapshot_resume() {
        use super::super::field_restore::{
            CopyFormat, PatternCopy, VideoCopyWork, VideoServiceError,
        };
        struct Recording<'a, B> {
            bus: &'a mut B,
            trace: &'a mut Vec<(PatternInstruction, Result<Option<u16>, DisplayWorkError>)>,
        }
        impl<B: PatternInstructionBus<Error = DisplayWorkError>> PatternInstructionBus
            for Recording<'_, B>
        {
            type Error = DisplayWorkError;
            fn execute(
                &mut self,
                instruction: PatternInstruction,
            ) -> Result<Option<u16>, Self::Error> {
                let result = self.bus.execute(instruction);
                self.trace.push((instruction, result));
                result
            }
        }
        for (lcdc, offset) in [(0x11, 0), (0x91, 0), (0x93, 65_660)] {
            for format in [CopyFormat::TwoBpp, CopyFormat::OneBppDouble] {
                for tiles in [0, 1, 8] {
                    // Include the header, POP/write/inc loop and footer;
                    // checking only small budgets would miss copy resumptions.
                    for budget in (0..=3200).step_by(4).chain([4096, 70224]) {
                        let mut time = DisplayWorkTime::default();
                        time.device_mut().write_port(0xff40, lcdc, 0xc3).unwrap();
                        time.elapse(offset).unwrap();
                        time.deadline = time.elapsed + budget;
                        let mut reference = time.clone();
                        let mut work = VideoCopyWork::new(PatternCopy {
                            source: PatternSource::Font,
                            source_offset: 17,
                            destination: 0x8800,
                            tiles,
                            format,
                        });
                        let epoch = time.device.vblank_epoch() + 1;
                        work.arm(epoch - 1).unwrap();
                        let mut expected_work = work.clone();
                        let mut memory = PatternMemory::default();
                        let mut expected_memory = memory.clone();
                        for _ in 0..3 {
                            let mut actual_trace = Vec::new();
                            let actual = work.service_until_blocked(
                                epoch,
                                &mut Recording {
                                    bus: &mut time.pattern_bus(&mut memory, &Bytes),
                                    trace: &mut actual_trace,
                                },
                            );
                            let mut expected_trace = Vec::new();
                            let expected = loop {
                                match expected_work.service_instruction(
                                    epoch,
                                    &mut Recording {
                                        bus: &mut reference
                                            .pattern_bus(&mut expected_memory, &Bytes)
                                            .checking_each_instruction(),
                                        trace: &mut expected_trace,
                                    },
                                ) {
                                    Ok(false) => (),
                                    Ok(true) => break Ok(()),
                                    Err(error) => break Err(error),
                                }
                            };
                            assert_eq!(actual, expected);
                            assert_eq!(actual_trace, expected_trace);
                            assert_eq!(work, expected_work);
                            assert_eq!(memory, expected_memory);
                            assert_eq!(
                                serde_json::to_value(&time).unwrap(),
                                serde_json::to_value(&reference).unwrap()
                            );
                            match actual {
                                Ok(()) => {
                                    assert!(work.vblank_work().unwrap().finished());
                                    assert!(matches!(
                                        work.service_until_blocked(
                                            epoch,
                                            &mut time.pattern_bus(&mut memory, &Bytes)
                                        ),
                                        Err(VideoServiceError::Protocol(
                                            RestoreError::AlreadyServiced
                                        ))
                                    ));
                                    break;
                                }
                                Err(VideoServiceError::Bus(DisplayWorkError::Budget)) => (),
                                other => panic!("unexpected video result: {other:?}"),
                            }
                            work = serde_json::from_value(serde_json::to_value(work).unwrap())
                                .unwrap();
                            expected_work = serde_json::from_value(
                                serde_json::to_value(expected_work).unwrap(),
                            )
                            .unwrap();
                            time = serde_json::from_value(serde_json::to_value(time).unwrap())
                                .unwrap();
                            reference =
                                serde_json::from_value(serde_json::to_value(reference).unwrap())
                                    .unwrap();
                            time.begin_native_update();
                            reference.begin_native_update();
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn lcd_runner_preserves_every_bus_receipt_budget_and_snapshot_continuation() {
        struct Recording<'a, B> {
            bus: &'a mut B,
            trace: &'a mut Vec<(DisplayInstruction, Result<Option<u8>, DisplayWorkError>)>,
        }
        impl<B: DisplayInstructionBus<Error = DisplayWorkError>> DisplayInstructionBus
            for Recording<'_, B>
        {
            type Error = DisplayWorkError;
            fn execute(
                &mut self,
                instruction: DisplayInstruction,
            ) -> Result<Option<u8>, Self::Error> {
                let result = self.bus.execute(instruction);
                self.trace.push((instruction, result.clone()));
                result
            }
        }
        for lcdc in [0x11, 0x91, 0x93, 0xb1] {
            for offset in [0, 80, 65_660] {
                for budget in (0..=124).step_by(4).chain([4096, 70224]) {
                    let mut time = DisplayWorkTime::default();
                    time.device_mut().write_port(0xff40, lcdc, 0xc3).unwrap();
                    time.elapse(offset).unwrap();
                    if offset == 80 {
                        time.device_mut().write_port(0xff46, 0xc3, 0xc3).unwrap();
                    }
                    time.deadline = time.elapsed + budget;
                    let mut reference = time.clone();
                    let mut work = DisableLcdWork::default();
                    let mut expected_work = work;
                    for _ in 0..3 {
                        let mut actual_trace = Vec::new();
                        let actual = work.run(&mut Recording {
                            bus: &mut time.display_bus(),
                            trace: &mut actual_trace,
                        });
                        let mut expected_trace = Vec::new();
                        let expected = loop {
                            match expected_work.step(&mut Recording {
                                bus: &mut reference,
                                trace: &mut expected_trace,
                            }) {
                                Ok(Some(_)) => (),
                                Ok(None) => break Ok(()),
                                Err(error) => break Err(error),
                            }
                        };
                        assert_eq!(actual, expected);
                        assert_eq!(actual_trace, expected_trace);
                        assert_eq!(work, expected_work);
                        assert_eq!(
                            serde_json::to_value(&time).unwrap(),
                            serde_json::to_value(&reference).unwrap()
                        );
                        match actual {
                            Ok(()) => break,
                            Err(DisplayWorkError::Budget) => (),
                            other => panic!("unexpected LCD result: {other:?}"),
                        }
                        work = serde_json::from_value(serde_json::to_value(work).unwrap()).unwrap();
                        expected_work =
                            serde_json::from_value(serde_json::to_value(expected_work).unwrap())
                                .unwrap();
                        time = serde_json::from_value(serde_json::to_value(time).unwrap()).unwrap();
                        reference =
                            serde_json::from_value(serde_json::to_value(reference).unwrap())
                                .unwrap();
                        time.begin_native_update();
                        reference.begin_native_update();
                    }
                }
            }
        }
    }

    #[test]
    fn exclusive_display_bus_matches_every_lcd_poll_and_snapshot_resume() {
        for lcdc in [0x11, 0x91, 0x93, 0xb1] {
            for offset in [0, 80, 65_660] {
                for budget in [0, 4, 4096, 70224] {
                    let mut time = DisplayWorkTime::default();
                    time.device_mut().write_port(0xff40, lcdc, 0xc3).unwrap();
                    time.elapse(offset).unwrap();
                    if offset == 80 {
                        time.device_mut().write_port(0xff46, 0xc3, 0xc3).unwrap();
                    }
                    time.deadline = time.elapsed + budget;
                    let mut reference = time.clone();
                    reference.check_each_display_interval = true;
                    let mut work = DisableLcdWork::default();
                    let mut expected_work = work;
                    for _ in 0..3 {
                        let result = {
                            let mut bus = time.display_bus();
                            let mut result = Ok(None);
                            for index in 0..22_000 {
                                let instruction = work.instruction();
                                result = work.step(&mut bus);
                                assert_eq!(result, expected_work.step(&mut reference));
                                assert_eq!(work, expected_work);
                                assert_eq!(bus.time.device, reference.device);
                                assert_eq!(bus.time.elapsed, reference.elapsed);
                                assert_eq!(bus.time.divider_counter, reference.divider_counter);
                                if index % 256 == 0
                                    || instruction.is_some_and(|i| {
                                        matches!(i.access, Some(DisplayAccess::Write { .. }))
                                    })
                                {
                                    assert_eq!(
                                        serde_json::to_value(&*bus.time).unwrap(),
                                        serde_json::to_value(&reference).unwrap()
                                    );
                                }
                                if !matches!(result, Ok(Some(_))) {
                                    break;
                                }
                            }
                            result
                        };
                        assert_eq!(
                            serde_json::to_value(&time).unwrap(),
                            serde_json::to_value(&reference).unwrap()
                        );
                        match result {
                            Ok(None) => break,
                            Err(DisplayWorkError::Budget) => (),
                            other => panic!("LCD work did not pause or return: {other:?}"),
                        }
                        time =
                            serde_json::from_value(serde_json::to_value(&time).unwrap()).unwrap();
                        reference =
                            serde_json::from_value(serde_json::to_value(&reference).unwrap())
                                .unwrap();
                        reference.check_each_display_interval = true;
                        time.begin_native_update();
                        reference.begin_native_update();
                    }
                }
            }
        }
    }

    #[test]
    fn active_irq_plain_runs_match_instruction_budgets_dma_and_lcd_edges() {
        let code: Vec<_> = [4, 12, 0, 16, 8, 24, 4, 8].into_iter().cycle().take(8192)
            .map(|tcycles| DisplayInstruction { tcycles, access: None }).collect();
        for lcdc in [0x11, 0x91, 0x93, 0xb3] {
            for offset in [0, 79, 252, 65_660] {
                for dma in [false, true] {
                    for budget in (0..=256u64).step_by(4).chain([511, 4096, 65535, 70224]) {
                        let mut time = DisplayWorkTime::default();
                        time.begin_native_update();
                        time.device_mut().request_interrupt(1);
                        time.enter_pending_vblank().unwrap();
                        time.device_mut().write_port(0xff40, lcdc, 0xc3).unwrap();
                        time.device_mut().write_port(0xff4b, 7, 0xc3).unwrap();
                        time.elapse(offset).unwrap();
                        time.set_shadow_oam(&(0..160).map(|i| i as u8).collect::<Vec<_>>()).unwrap();
                        if dma { time.device_mut().write_port(0xff46, 0xc3, 0xc3).unwrap(); }
                        time.deadline = time.elapsed + budget;
                        let mut reference = time.clone();
                        let actual = time.display_bus().execute_plain_run(code.iter().copied());
                        let mut count = 0;
                        let expected = loop {
                            let Some(&instruction) = code.get(count) else { break Ok(count); };
                            match reference.execute(instruction) {
                                Ok(_) => count += 1,
                                Err(DisplayWorkError::Budget) if count > 0 => break Ok(count),
                                Err(error) => break Err(error),
                            }
                        };
                        assert_eq!(actual, expected);
                        assert_eq!(serde_json::to_value(&time).unwrap(), serde_json::to_value(&reference).unwrap());
                    }
                }
            }
        }
    }

    #[test]
    fn buffer_copy_fast_path_matches_every_original_store_budget_and_cursor() {
        use super::super::screen_buffer_work::ScreenBufferWork;
        struct InstructionBus<'a>(&'a mut DisplayWorkTime);
        impl DisplayInstructionBus for InstructionBus<'_> {
            type Error = DisplayWorkError;
            fn execute(
                &mut self,
                instruction: DisplayInstruction,
            ) -> Result<Option<u8>, Self::Error> {
                self.0.execute(instruction)
            }
        }
        for lcdc in [0x11, 0x91, 0xb3] {
            for offset in [0, 252, 65_660] {
                for budget in (0..=20_000u64).step_by(997).chain([52, 18_908, 70_224]) {
                    let mut time = DisplayWorkTime::default();
                    time.begin_native_update();
                    time.device_mut().write_port(0xff40, lcdc, 0xc3).unwrap();
                    time.elapse(offset).unwrap();
                    time.set_vblank_dispatch_enabled(true);
                    time.deadline = time.elapsed + budget;
                    let source: Vec<_> = (0..360).map(|i| (i * 37 % 256) as u8).collect();
                    let mut work = ScreenBufferWork::new(source).unwrap();
                    let mut reference_work = work.clone();
                    let mut reference_time = time.clone();
                    let mut tiles = [0; 360];
                    let mut expected_tiles = tiles;
                    let actual = work.run(&mut time.display_bus(), &mut tiles);
                    let expected = reference_work.run(
                        &mut InstructionBus(&mut reference_time),
                        &mut expected_tiles,
                    );
                    assert_eq!(
                        actual, expected,
                        "lcdc={lcdc:x} offset={offset} budget={budget}"
                    );
                    assert_eq!(work, reference_work);
                    assert_eq!(tiles, expected_tiles);
                    assert_eq!(
                        serde_json::to_value(&time).unwrap(),
                        serde_json::to_value(&reference_time).unwrap()
                    );
                }
            }
        }
    }

    #[test]
    fn plain_run_refuses_foreground_and_stops_before_port_access_without_effects() {
        let code = [
            DisplayInstruction { tcycles: 4, access: None },
            DisplayInstruction { tcycles: 12, access: Some(DisplayAccess::Write { port: 0xff47, value: 0xe4, at_tcycle: 8 }) },
        ];
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        time.set_vblank_dispatch_enabled(true);
        time.device_mut().request_interrupt(1);
        let before = serde_json::to_value(&time).unwrap();
        assert_eq!(time.display_bus().execute_plain_run(code.into_iter()), Ok(0));
        assert_eq!(serde_json::to_value(&time).unwrap(), before);
        assert_eq!(time.execute(code[0]), Err(DisplayWorkError::InterruptPending));
        time.enter_pending_vblank().unwrap();
        let start = time.elapsed();
        assert_eq!(time.display_bus().execute_plain_run(code.into_iter()), Ok(1));
        assert_eq!(time.elapsed(), start + 4);
        assert_eq!(time.device().read_port(0xff47).unwrap(), 0);
        time.execute(code[1]).unwrap();
        assert_eq!(time.device().read_port(0xff47).unwrap(), 0xe4);
        assert!(time.active_vblank());
        assert_eq!(time.interrupt_returns, 0);
    }

    #[test]
    fn foreground_plain_runs_stop_at_the_same_budget_and_vblank_instruction_boundaries() {
        let code: Vec<_> = [4, 12, 16, 8, 24, 4, 8]
            .into_iter()
            .cycle()
            .take(8192)
            .map(|tcycles| DisplayInstruction {
                tcycles,
                access: None,
            })
            .collect();
        for offset in [0, 79, 252, 65_500, 65_660, 65_664, 65_668] {
            for budget in (0..=256u64).step_by(4).chain([511, 4096, 70_224]) {
                let mut time = DisplayWorkTime::default();
                time.begin_native_update();
                time.elapse(offset).unwrap();
                time.set_vblank_dispatch_enabled(true);
                time.deadline = time.elapsed + budget;
                let initially_pending = time.pending_vblank_dispatch();
                let mut reference = time.clone();
                let actual = time.display_bus().execute_plain_run(code.iter().copied());
                let mut count = 0;
                let expected = loop {
                    let Some(&instruction) = code.get(count) else {
                        break Ok(count);
                    };
                    match reference.execute(instruction) {
                        Ok(_) => count += 1,
                        Err(DisplayWorkError::Budget | DisplayWorkError::InterruptPending)
                            if count > 0 =>
                        {
                            break Ok(count)
                        }
                        Err(DisplayWorkError::InterruptPending) => break Ok(count),
                        Err(DisplayWorkError::Budget) if initially_pending => break Ok(count),
                        Err(error) => break Err(error),
                    }
                };
                assert_eq!(actual, expected, "offset={offset} budget={budget}");
                assert_eq!(
                    serde_json::to_value(&time).unwrap(),
                    serde_json::to_value(&reference).unwrap(),
                    "offset={offset} budget={budget}"
                );
            }
        }
    }

    #[test]
    fn direct_display_intervals_match_checked_observer_through_dma_ports_and_resume() {
        let read = |port| DisplayInstruction {
            tcycles: 12,
            access: Some(DisplayAccess::Read { port, at_tcycle: 8 }),
        };
        let write = |port, value| DisplayInstruction {
            tcycles: 12,
            access: Some(DisplayAccess::Write { port, value, at_tcycle: 8 }),
        };
        let instructions = [
            DisplayInstruction { tcycles: 4, access: None },
            read(0xff44), read(0xff04), write(0xc300, 17),
            write(0xff46, 0xc3), write(0xc300, 23),
            write(0xff47, 0xe4), write(0xff48, 0xd0),
            write(0xff43, 3), write(0xff4a, 8),
            write(0xff40, 0x11), read(0xff44),
            write(0xff40, 0xb3), read(0xff41),
            write(0xff46, 0xc2), read(0xff01),
            write(0xff41, 8), write(0xff0f, 0),
        ];
        for lcdc in [0x11, 0x91, 0x93, 0xb1] {
            for offset in [0, 3, 4, 79, 80, 251, 65_660, 70_220] {
                for budget in [0, 4, 12, 70224] {
                    let mut time = DisplayWorkTime::default();
                    time.device_mut().write_port(0xff40, lcdc, 0xc3).unwrap();
                    time.elapse(offset).unwrap();
                    time.deadline = time.elapsed + budget;
                    let mut reference = time.clone();
                    reference.check_each_display_interval = true;
                    for (index, instruction) in instructions.into_iter().cycle().take(72).enumerate() {
                        let actual = time.execute(instruction);
                        assert_eq!(actual, reference.execute(instruction),
                            "lcdc={lcdc} offset={offset} budget={budget} index={index}");
                        assert_eq!(serde_json::to_value(&time).unwrap(),
                            serde_json::to_value(&reference).unwrap());
                        assert_eq!(time.palette_store_receipts(), reference.palette_store_receipts());
                        if actual == Err(DisplayWorkError::Budget) {
                            time.begin_native_update();
                            reference.begin_native_update();
                        }
                        if index % 9 == 8 {
                            time = serde_json::from_value(serde_json::to_value(&time).unwrap()).unwrap();
                            reference = serde_json::from_value(serde_json::to_value(&reference).unwrap()).unwrap();
                            assert!(!time.check_each_display_interval);
                            assert!(!reference.check_each_display_interval);
                            reference.check_each_display_interval = true;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn display_bus_preserves_writes_port_errors_and_malformed_snapshot_order() {
        let instructions = [
            DisplayInstruction {
                tcycles: 4,
                access: None,
            },
            DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Read {
                    port: 0xff44,
                    at_tcycle: 8,
                }),
            },
            DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Read {
                    port: 0xff01,
                    at_tcycle: 8,
                }),
            },
            DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port: 0xff40,
                    value: 0x91,
                    at_tcycle: 8,
                }),
            },
            DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port: 0xff46,
                    value: 0xc3,
                    at_tcycle: 8,
                }),
            },
            DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Read {
                    port: 0xff44,
                    at_tcycle: 8,
                }),
            },
            DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port: 0xff41,
                    value: 8,
                    at_tcycle: 8,
                }),
            },
            DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port: 0xff46,
                    value: 0xc2,
                    at_tcycle: 8,
                }),
            },
            DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port: 0xff40,
                    value: 0x11,
                    at_tcycle: 8,
                }),
            },
            DisplayInstruction {
                tcycles: 4,
                access: None,
            },
        ];
        for case in 0..7 {
            let mut seed = DisplayWorkTime::default();
            seed.begin_native_update();
            match case {
                0 => (),
                1 => seed.shadow_oam.clear(),
                2 => seed.elapsed = 1,
                3 => seed.deadline = 0,
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
                6 => seed.deadline = 4,
                _ => unreachable!(),
            }
            let mut time = seed.clone();
            let mut reference = seed;
            reference.check_each_display_interval = true;
            let mut bus = time.display_bus();
            for instruction in instructions {
                assert_eq!(
                    bus.execute(instruction),
                    reference.execute(instruction),
                    "case={case} instruction={instruction:?}"
                );
                assert_eq!(
                    serde_json::to_value(&*bus.time).unwrap(),
                    serde_json::to_value(&reference).unwrap()
                );
            }
        }
    }

    #[test]
    fn exclusive_pattern_bus_matches_checked_instructions_io_and_snapshot_resume() {
        use super::super::field_restore::{CopyFormat, PatternCopy};
        use super::super::vblank_pattern_work::VBlankPatternWork;
        struct Source {
            limit: Option<u16>,
            reads: core::cell::RefCell<Vec<(PatternSource, u16)>>,
        }
        impl PatternAssets for Source {
            fn read_pattern_byte(&self, source: PatternSource, offset: u16) -> Option<u8> {
                self.reads.borrow_mut().push((source, offset));
                if self.limit.is_some_and(|limit| offset >= limit) {
                    None
                } else {
                    Some((offset as u8).wrapping_mul(3))
                }
            }
        }
        for format in [CopyFormat::TwoBpp, CopyFormat::OneBppDouble] {
            for lcdc in [0x11, 0x91] {
                for available in [0, 4, 12, 256, 70224] {
                    for interleave in [false, true] {
                        for limit in [None, Some(24)] {
                            let mut time = DisplayWorkTime::default();
                            time.device_mut().write_port(0xff40, lcdc, 0xc3).unwrap();
                            time.begin_native_update();
                            while time.remaining() > available {
                                let n = (time.remaining() - available).min(24) as u8;
                                DisplayInstructionBus::execute(
                                    &mut time,
                                    DisplayInstruction {
                                        tcycles: n,
                                        access: None,
                                    },
                                )
                                .unwrap();
                            }
                            let mut reference_time = time.clone();
                            let mut memory = PatternMemory::default();
                            let mut reference_memory = memory.clone();
                            let copy = PatternCopy {
                                source: PatternSource::WalkingPlayer,
                                source_offset: 17,
                                destination: 0x8800,
                                tiles: 8,
                                format,
                            };
                            let mut work = VBlankPatternWork::new(copy);
                            let mut reference_work = work;
                            let source = Source {
                                limit,
                                reads: core::cell::RefCell::new(Vec::new()),
                            };
                            let reference_source = Source {
                                limit,
                                reads: core::cell::RefCell::new(Vec::new()),
                            };
                            let mut done = false;
                            for _ in 0..3 {
                                let result = {
                                    let mut bus = time.pattern_bus(&mut memory, &source);
                                    let mut reference = reference_time
                                        .pattern_bus(&mut reference_memory, &reference_source)
                                        .checking_each_instruction();
                                    let mut last = None;
                                    for index in 0..2000 {
                                        let actual = work.step(&mut bus);
                                        let expected = reference_work.step(&mut reference);
                                        assert_eq!(actual, expected);
                                        assert_eq!(work, reference_work);
                                        if interleave && matches!(actual, Ok(Some(_))) {
                                            let port = if index % 37 == 0 {
                                                Some((0xff46, 0xc3))
                                            } else if index % 29 == 0 {
                                                Some((
                                                    0xff40,
                                                    if index % 2 == 0 { 0x11 } else { 0x91 },
                                                ))
                                            } else if index % 31 == 0 {
                                                Some((0xff41, 8))
                                            } else {
                                                None
                                            };
                                            if let Some((port, value)) = port {
                                                let io = DisplayInstruction {
                                                    tcycles: 12,
                                                    access: Some(DisplayAccess::Write {
                                                        port,
                                                        value,
                                                        at_tcycle: 8,
                                                    }),
                                                };
                                                assert_eq!(
                                                    DisplayInstructionBus::execute(&mut bus, io),
                                                    DisplayInstructionBus::execute(
                                                        &mut reference,
                                                        io
                                                    )
                                                );
                                            }
                                        }
                                        assert_eq!(bus.patterns, reference.patterns);
                                        assert_eq!(
                                            serde_json::to_value(&*bus.time).unwrap(),
                                            serde_json::to_value(&*reference.time).unwrap()
                                        );
                                        assert_eq!(
                                            *source.reads.borrow(),
                                            *reference_source.reads.borrow()
                                        );
                                        if !matches!(actual, Ok(Some(_))) {
                                            last = Some(actual);
                                            break;
                                        }
                                    }
                                    last.expect("bounded instruction stream")
                                };
                                if result == Err(DisplayWorkError::Budget) {
                                    time.idle_to_target().unwrap();
                                    reference_time.idle_to_target().unwrap();
                                    // The bus lease/cache is gone. Real snapshot reloads must
                                    // restart validation, rather than trusting a saved flag.
                                    time = serde_json::from_value(
                                        serde_json::to_value(&time).unwrap(),
                                    )
                                    .unwrap();
                                    memory = serde_json::from_value(
                                        serde_json::to_value(&memory).unwrap(),
                                    )
                                    .unwrap();
                                    time.begin_native_update();
                                    reference_time.begin_native_update();
                                } else {
                                    done = true;
                                    break;
                                }
                            }
                            assert!(done);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn vblank_edge_wait_matches_fine_events_with_budget_and_dynamic_fallback() {
        for sample in [0, 4, 80, 252, 65660, 70224, 135892] {
            for budget in [0, 4, 80, 4096, 70224] {
                for variant in 0..6 {
                    let mut fast = DisplayWorkTime::default();
                    fast.deadline = sample + budget;
                    fast.elapse(sample).unwrap();
                    match variant {
                        1 => fast.device_mut().write_port(0xff40, 0x93, 0xc3).unwrap(),
                        2 => fast.device_mut().write_port(0xff40, 0xb1, 0xc3).unwrap(),
                        3 => fast.device_mut().write_port(0xff46, 0xc3, 0xc3).unwrap(),
                        4 => fast.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap(),
                        5 => {
                            let mut value = serde_json::to_value(fast.device()).unwrap();
                            value["stat_enable"] = 8.into();
                            *fast.device_mut() = serde_json::from_value(value).unwrap();
                        }
                        _ => (),
                    }
                    let mut reference = fast.clone();
                    let epoch = fast.device().vblank_epoch();
                    let actual = loop {
                        if let Err(e) = fast.idle_to_vblank_edge() {
                            break Err(e);
                        }
                        if fast.remaining() == 0 || fast.device().vblank_epoch() > epoch {
                            break Ok(());
                        }
                    };
                    let expected = loop {
                        if let Err(e) = reference.idle_to_device_event() {
                            break Err(e);
                        }
                        if reference.remaining() == 0 || reference.device().vblank_epoch() > epoch {
                            break Ok(());
                        }
                    };
                    assert_eq!(
                        actual, expected,
                        "sample={sample} budget={budget} variant={variant}"
                    );
                    assert_eq!(
                        serde_json::to_value(&fast).unwrap(),
                        serde_json::to_value(&reference).unwrap(),
                        "sample={sample} budget={budget} variant={variant}"
                    );
                }
            }
        }
    }

    #[test]
    fn cached_idle_period_is_invalidated_by_mutation_and_snapshot_reload() {
        fn settled() -> DisplayWorkTime {
            let mut time = DisplayWorkTime::default();
            for _ in 0..3 {
                time.begin_native_update();
                time.idle_to_target().unwrap();
            }
            assert!(time.idle_period_cached);
            time
        }
        for case in 0..12 {
            let mut fast = settled();
            match case {
                0 => {
                    let _ = fast.device_mut();
                }
                1 => {
                    fast.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
                }
                2 => {
                    fast.device_mut().write_port(0xff43, 3, 0xc3).unwrap();
                }
                3 => {
                    fast.device_mut().write_port(0xff4a, 144, 0xc3).unwrap();
                }
                4 => {
                    fast.device_mut().write_port(0xff46, 0xc3, 0xc3).unwrap();
                }
                5 => {
                    fast.device_mut().write_port(0xffff, 0, 0xc3).unwrap();
                }
                6 => {
                    assert!(fast.device_mut().write_port(0xff41, 8, 0xc3).is_err());
                }
                7 => {
                    fast.device_mut().write_port(0xff47, 0xe4, 0xc3).unwrap();
                }
                8 => {
                    fast.begin_native_update();
                    DisplayInstructionBus::execute(
                        &mut fast,
                        DisplayInstruction {
                            tcycles: 24,
                            access: None,
                        },
                    )
                    .unwrap();
                }
                9 => {
                    fast.begin_native_update();
                    let mut memory = PatternMemory::default();
                    PatternInstructionBus::execute(
                        &mut fast.pattern_bus(&mut memory, &Bytes),
                        PatternInstruction {
                            tcycles: 24,
                            access: None,
                        },
                    )
                    .unwrap();
                }
                10 => {
                    let now = fast.elapsed;
                    *fast.device_mut() = DisplayDevice::powered_off(now);
                }
                11 => {
                    let mut value = serde_json::to_value(fast.device()).unwrap();
                    value["stat_enable"] = 8.into();
                    *fast.device_mut() = serde_json::from_value(value).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(!fast.idle_period_cached, "case={case}");
            let mut reference = fast.clone();
            if !matches!(case, 8 | 9) {
                fast.begin_native_update();
                reference.begin_native_update();
            }
            assert_eq!(
                fast.idle_to_target(),
                reference.elapse(reference.remaining()),
                "case={case}"
            );
            assert_eq!(
                serde_json::to_value(&fast).unwrap(),
                serde_json::to_value(&reference).unwrap(),
                "case={case}"
            );
        }
        let base = settled();
        let mut value = serde_json::to_value(&base).unwrap();
        assert!(value.get("idle_period_cached").is_none());
        value["idle_period_cached"] = true.into();
        let mut reloaded: DisplayWorkTime = serde_json::from_value(value).unwrap();
        assert!(
            !reloaded.idle_period_cached,
            "saved input cannot grant trusted eligibility"
        );
        let mut reference = base.clone();
        reloaded.begin_native_update();
        reference.begin_native_update();
        reloaded.idle_to_target().unwrap();
        reference.elapse(reference.remaining()).unwrap();
        assert_eq!(
            serde_json::to_value(&reloaded).unwrap(),
            serde_json::to_value(&reference).unwrap()
        );
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
        assert_eq!(time.device().observed_tcycles(), elapsed);
    }

    #[test]
    fn idle_whole_grants_match_complete_time_state_after_every_update() {
        for wy in [0, 7, 144] {
            let mut fast = DisplayWorkTime::default();
            fast.device_mut().write_port(0xff4a, wy, 0xc3).unwrap();
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
        assert!(!time.device().lcd_enabled());
        assert!(
            time.elapsed() >= 145 * 456,
            "LY145 must actually be reached"
        );
        assert_eq!(time.device().read_port(0xffff).unwrap(), 1);
        let mut enable = EnableLcdWork::default();
        while !enable.finished() {
            enable.step(&mut time).unwrap();
        }
        assert!(time.device().lcd_enabled());
        let epoch = time.device().vblank_epoch();
        time.idle_to_target().unwrap();
        time.begin_native_update();
        while time.remaining() > 0 && time.device().vblank_epoch() == epoch {
            time.idle_to_device_event().unwrap();
        }
        assert!(time.device().vblank_epoch() > epoch);
    }
    #[test]
    fn copied_bytes_consume_shared_time_and_do_not_finish_on_a_new_grant() {
        let mut time = DisplayWorkTime::default();
        time.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
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
        let device = time.device().clone();
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
        let prior = time.device().clone();
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

#[cfg(all(test, not(target_os = "none")))]
mod palette_store_receipt_tests {
    use super::*;
    fn instruction(port: u16, value: u8) -> DisplayInstruction {
        DisplayInstruction { tcycles: 12, access: Some(DisplayAccess::Write { port, value, at_tcycle: 8 }) }
    }
    #[test]
    fn palette_receipt_samples_actual_io_cycle_across_lcd_boundaries_without_advancing_device() {
        for offset in [0, 4, 76, 80, 248, 252, 452, 456, 65560, 65664, 70212] {
            for port in 0xff47..=0xff49 {
                let mut time = DisplayWorkTime::default();
                time.begin_native_update();
                time.elapse(offset).unwrap();
                let mut expected = time.device().clone();
                expected.observe_time(offset + 8, time.shadow_oam()).unwrap();
                let ly = expected.read_port(0xff44).unwrap();
                let mode = expected.mode();
                let epoch = expected.vblank_epoch();
                expected.write_port(port, 0xe4, 0xc3).unwrap();
                expected.observe_time(offset + 12, time.shadow_oam()).unwrap();
                time.display_bus().execute(instruction(port, 0xe4)).unwrap();
                let receipt = time.palette_store_receipts()[usize::from(port - 0xff47)].unwrap();
                assert_eq!((receipt.port, receipt.before, receipt.value), (port, 0, 0xe4));
                assert_eq!((receipt.tcycles, receipt.ly, receipt.mode, receipt.vblank_epoch), (offset + 8, ly, mode, epoch));
                assert_eq!(time.elapsed(), offset + 12);
                assert_eq!(time.device(), &expected);
            }
        }
    }
    #[test]
    fn unchanged_palette_writes_are_observed_but_denied_instructions_are_not() {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        time.execute(instruction(0xff47, 0)).unwrap();
        assert_eq!(time.palette_store_receipts()[0].unwrap().tcycles, 8);
        time.execute(instruction(0xff48, 0xd0)).unwrap();
        time.execute(instruction(0xff49, 0xe0)).unwrap();
        let receipts = *time.palette_store_receipts();
        assert!(receipts.iter().all(Option::is_some));
        time.idle_to_target().unwrap();
        let state = serde_json::to_value(&time).unwrap();
        assert_eq!(time.execute(instruction(0xff47, 0xe4)), Err(DisplayWorkError::Budget));
        assert_eq!(*time.palette_store_receipts(), receipts);
        assert_eq!(serde_json::to_value(&time).unwrap(), state);
        assert!(time.execute(instruction(0xabcd, 0xe4)).is_err());
        assert_eq!(*time.palette_store_receipts(), receipts);
    }
    #[test]
    fn snapshot_restores_device_but_never_fabricates_past_observations() {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        time.execute(instruction(0xff47, 0xe4)).unwrap();
        let json = serde_json::to_value(&time).unwrap();
        assert!(json.get("palette_stores").is_none());
        let restored: DisplayWorkTime = serde_json::from_value(json).unwrap();
        assert_eq!(restored.device(), time.device());
        assert_eq!(restored.elapsed(), time.elapsed());
        assert!(restored.palette_store_receipts().iter().all(Option::is_none));
    }
}

#[cfg(test)]
#[path = "display_irq_dispatch_tests.rs"]
mod irq_dispatch_tests;

#[cfg(test)]
mod native_vblank_handoff_tests {
    use super::*;

    #[test]
    fn native_vblank_handoff_preserves_time_and_requires_native_ownership() {
        let mut time = DisplayWorkTime::legacy_native_projection(3);
        assert_eq!(time.device().pending_interrupts() & 1, 1);
        let elapsed = time.elapsed();
        let epoch = time.device().vblank_epoch();
        time.acknowledge_native_vblank().unwrap();
        assert_eq!(time.elapsed(), elapsed);
        assert_eq!(time.device().vblank_epoch(), epoch);
        assert_eq!(time.interrupt_returns, 0);
        assert_eq!(time.device().pending_interrupts() & 1, 0);
        time.set_vblank_dispatch_enabled(true);
        time.begin_native_update();
        time.idle_to_vblank_edge().unwrap();
        assert!(time.pending_vblank_dispatch(), "a new edge still belongs to the source owner");
        let before = serde_json::to_value(&time).unwrap();
        assert_eq!(time.acknowledge_native_vblank(), Err(DisplayWorkError::InterruptBusy));
        assert_eq!(serde_json::to_value(&time).unwrap(), before);
        time.enter_pending_vblank().unwrap();
        assert_eq!(time.acknowledge_native_vblank(), Err(DisplayWorkError::InterruptBusy));
    }
}

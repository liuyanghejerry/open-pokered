//! Shared native LCD/interrupt/DMA device state.
//!
//! The Game elapsed-time owner calls `observe_time` with its absolute T-cycle.
//! This device has no `tick_frame`, FLASH timer, or independently advanceable
//! clock. Local line/phase and DMA counters are hardware state. LCD disable
//! stops new LCD edges, not already pending IF bits or an in-progress DMA.
//!
//! Steady scanlines use 456 dots. Startup follows the finite mode-0/3/0 and
//! LY/mode-2 transition sequence of the pinned original provider (audit950),
//! not its measured total white-screen duration. IO stores take effect at the
//! start of their final M-cycle; enable startup is released when that M-cycle
//! completes. This bus-phase derivation still needs matched runtime validation.
//!
//! This is the source game's bounded DMG display subset, not a pixel PPU:
//! STAT interrupts, CPU OAM corruption and mid-mode-2 DMA selection are not
//! implemented. Normal restoration uses STAT enables zero and starts DMA in
//! the VBlank service. Mode-3 fetch costs use the documented data algorithm;
//! no claim is made that every provider's mid-scanline quirk is reproduced.
use crate::alloc_prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DisplayDeviceError {
    TimeReversed,
    TimeNotSynchronized,
    InvalidOam,
    InvalidPhase,
    UnsupportedPort(u16),
    UnsupportedDmaSource(u8),
    UnsupportedStatInterrupts(u8),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum RasterPhase {
    Off,
    /// The remainder of the store's last M-cycle, not a measured frame shift.
    EnableStore,
    StartupMode0,
    Mode2,
    Mode3,
    HBlank,
    AdvanceLy,
    VBlank,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct OamDma {
    next: u8,
    cycles_until_byte: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DisplayDevice {
    observed_tcycles: u64,
    lcdc: u8,
    ly: u8,
    lyc: u8,
    stat_enable: u8,
    stat_irq_line: bool,
    scx: u8,
    scy: u8,
    wx: u8,
    wy: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    window_y_reached: bool,
    phase: RasterPhase,
    phase_remaining: u16,
    mode3_length: u16,
    first_frame: bool,
    /// Counts actual LCD VBlank edges, including edges not accepted by CPU.
    vblank_epoch: u64,
    iflags: u8,
    ie: u8,
    dma: Option<OamDma>,
    oam: Vec<u8>,
}
// A constructor-checked interval keeps the one-period proof local and avoids
// recomputing the same 64-bit subtraction in both the time owner and device.
pub(super) struct CheckedFramePeriod {
    from: u64,
    target: u64,
}
impl CheckedFramePeriod {
    #[inline(always)]
    pub(super) fn new(from: u64, target: u64) -> Option<Self> {
        (target.checked_sub(from) == Some(154 * 456)).then_some(Self { from, target })
    }
}
impl DisplayDevice {
    /// Passive event distance for the shared CPU's HALT. It grants no time.
    pub fn tcycles_until_event(&self) -> Option<u16> {
        let lcd = self.lcd_enabled().then_some(self.phase_remaining);
        let dma = self.dma.map(|dma| u16::from(dma.cycles_until_byte));
        match (lcd, dma) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
    /// Passive distance to the next VBlank request for an unchanged, empty
    /// raster. The owner still advances the real device through every event.
    /// Dynamic OBJ/window/DMA and malformed states use ordinary event waits.
    pub fn tcycles_until_vblank(&self) -> Option<u32> {
        if self.validate().is_err()
            || self.lcdc & 0xa2 != 0x80
            || self.dma.is_some()
            || self.stat_irq_line
            || self.mode3_length != 172 + u16::from(self.scx & 7)
        {
            return None;
        }
        use RasterPhase::*;
        let r = u32::from(self.phase_remaining);
        let ly = u32::from(self.ly);
        if self.phase == VBlank {
            if !(144..=153).contains(&self.ly) {
                return None;
            }
            return Some(
                r + (153 - ly) * 456 + if self.first_frame && ly < 153 { 8 } else { 0 } + 144 * 456,
            );
        }
        if ly > 143 {
            return None;
        }
        let future = (143 - ly) * 456;
        let startup_short = if self.first_frame { 4 } else { 0 };
        Some(match self.phase {
            EnableStore => r + 76 + 376 + future - startup_short,
            StartupMode0 | Mode2 => r + 376 + future - startup_short,
            Mode3 => r + 376 - u32::from(self.mode3_length) + future - startup_short,
            HBlank => {
                let limit = if ly < 143 || self.first_frame {
                    372
                } else {
                    376
                } - u32::from(self.mode3_length);
                if r > limit {
                    return None;
                }
                r + if ly < 143 { 4 } else { 0 } + future
                    - if self.first_frame && ly < 143 { 4 } else { 0 }
            }
            AdvanceLy => {
                if ly == 0 {
                    return None;
                }
                r + (144 - ly) * 456 - startup_short
            }
            Off | VBlank => return None,
        })
    }

    pub fn observed_tcycles(&self) -> u64 {
        self.observed_tcycles
    }
    #[inline(always)]
    pub fn validate(&self) -> Result<(), DisplayDeviceError> {
        if self.oam.len() != 160 {
            return Err(DisplayDeviceError::InvalidOam);
        }
        if self.ly > 153
            || self.mode3_length > 376
            || (self.lcd_enabled() && (self.phase == RasterPhase::Off || self.phase_remaining == 0))
            || (!self.lcd_enabled() && self.phase != RasterPhase::Off)
            || self
                .dma
                .is_some_and(|dma| dma.next >= 160 || dma.cycles_until_byte == 0)
        {
            return Err(DisplayDeviceError::InvalidPhase);
        }
        if self.stat_enable != 0 {
            return Err(DisplayDeviceError::UnsupportedStatInterrupts(
                self.stat_enable,
            ));
        }
        let limit = match self.phase {
            RasterPhase::Off => 0,
            RasterPhase::EnableStore | RasterPhase::AdvanceLy => 4,
            RasterPhase::StartupMode0 => 76,
            RasterPhase::Mode2 => 80,
            RasterPhase::Mode3 => self.mode3_length,
            RasterPhase::HBlank => 376,
            RasterPhase::VBlank => 456 + (80 - 76) + (204 - 200),
        };
        if self.phase_remaining > limit || self.dma.is_some_and(|dma| dma.cycles_until_byte > 8) {
            return Err(DisplayDeviceError::InvalidPhase);
        }
        Ok(())
    }
    /// Fresh device at boot, not an initialization performed on FLASH entry.
    /// The sole Game time owner supplies the initial absolute time. The caller
    /// enables LCD through the same register bus used by later loader work.
    pub fn powered_off(at_tcycle: u64) -> Self {
        Self {
            observed_tcycles: at_tcycle,
            lcdc: 0,
            ly: 0,
            lyc: 0,
            stat_enable: 0,
            stat_irq_line: false,
            scx: 0,
            scy: 0,
            wx: 0,
            wy: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            window_y_reached: false,
            phase: RasterPhase::Off,
            phase_remaining: 0,
            mode3_length: 172,
            first_frame: false,
            vblank_epoch: 0,
            iflags: 0,
            ie: 0,
            dma: None,
            oam: vec![0; 160],
        }
    }
    pub fn mode(&self) -> u8 {
        match self.phase {
            RasterPhase::Mode2 => 2,
            RasterPhase::Mode3 => 3,
            RasterPhase::VBlank => 1,
            _ => 0,
        }
    }
    pub fn lcd_enabled(&self) -> bool {
        self.lcdc & 0x80 != 0
    }
    pub fn blank_first_frame(&self) -> bool {
        self.first_frame || !self.lcd_enabled()
    }
    pub fn vram_accessible(&self) -> bool {
        !self.lcd_enabled() || self.mode() != 3
    }
    pub fn oam(&self) -> &[u8] {
        &self.oam
    }
    pub fn vblank_epoch(&self) -> u64 {
        self.vblank_epoch
    }
    pub fn pending_interrupts(&self) -> u8 {
        self.iflags & self.ie & 0x1f
    }
    pub fn request_interrupt(&mut self, mask: u8) {
        self.iflags |= mask & 0x1f;
    }
    pub fn acknowledge_interrupt(&mut self, mask: u8) {
        self.iflags &= !(mask & 0x1f);
    }
    pub fn dma_active(&self) -> bool {
        self.dma.is_some()
    }
    pub fn read_port(&self, port: u16) -> Result<u8, DisplayDeviceError> {
        Ok(match port {
            0xff0f => self.iflags | 0xe0,
            0xffff => self.ie,
            0xff40 => self.lcdc,
            0xff41 => 0x80 | self.stat_enable | self.mode() | u8::from(self.ly == self.lyc) * 4,
            0xff42 => self.scy,
            0xff43 => self.scx,
            0xff44 => self.ly,
            0xff45 => self.lyc,
            0xff47 => self.bgp,
            0xff48 => self.obp0,
            0xff49 => self.obp1,
            0xff4a => self.wy,
            0xff4b => self.wx,
            _ => return Err(DisplayDeviceError::UnsupportedPort(port)),
        })
    }
    /// Validate a write before the time owner accepts its instruction. A valid
    /// DMA store must not depend on support for reading the DMA register.
    pub fn validate_write_port(
        &self,
        port: u16,
        value: u8,
        shadow_oam_page: u8,
    ) -> Result<(), DisplayDeviceError> {
        match port {
            0xff41 if value & 0x78 != 0 => {
                Err(DisplayDeviceError::UnsupportedStatInterrupts(value))
            }
            0xff46 if value != shadow_oam_page => {
                Err(DisplayDeviceError::UnsupportedDmaSource(value))
            }
            0xff0f | 0xffff | 0xff40..=0xff4b => Ok(()),
            _ => Err(DisplayDeviceError::UnsupportedPort(port)),
        }
    }
    /// Called at the access cycle after the owning timeline has synchronized
    /// the device to that cycle. Source DMA is the actual wShadowOAM buffer;
    /// arbitrary source pages are rejected rather than invented.
    pub fn write_port(
        &mut self,
        port: u16,
        value: u8,
        shadow_oam_page: u8,
    ) -> Result<(), DisplayDeviceError> {
        self.validate_write_port(port, value, shadow_oam_page)?;
        match port {
            0xff0f => self.iflags = value & 0x1f,
            0xffff => self.ie = value,
            0xff40 => {
                let was_enabled = self.lcd_enabled();
                self.lcdc = value;
                if was_enabled && !self.lcd_enabled() {
                    self.ly = 0;
                    self.phase = RasterPhase::Off;
                    self.phase_remaining = 0;
                    self.first_frame = false;
                    self.window_y_reached = false;
                    // Disabling does not perform an LYC comparison/IRQ edge.
                    self.stat_irq_line = false;
                } else if !was_enabled && self.lcd_enabled() {
                    self.ly = 0;
                    self.phase = RasterPhase::EnableStore;
                    self.phase_remaining = 4;
                    self.first_frame = true;
                    self.window_y_reached = self.wy == 0;
                    self.stat_edge();
                }
            }
            0xff41 => {
                // The source game keeps these interrupt enables clear. STAT
                // write glitches/midline IRQ behavior require a separate
                // hardware-validated child; do not silently approximate them.
                if value & 0x78 != 0 {
                    return Err(DisplayDeviceError::UnsupportedStatInterrupts(value));
                }
                self.stat_enable = value & 0x78;
                self.stat_edge();
            }
            0xff42 => self.scy = value,
            0xff43 => self.scx = value,
            0xff44 => (), // LY is read-only.
            0xff45 => {
                self.lyc = value;
                self.stat_edge();
            }
            0xff46 => {
                if value != shadow_oam_page {
                    return Err(DisplayDeviceError::UnsupportedDmaSource(value));
                }
                // OAM DMA starts a fresh transfer on each register write.
                // Complete the write M-cycle, then transfer the first byte.
                // Restarting before byte zero uses the already armed startup.
                let delay = if self.dma.is_some_and(|dma| dma.next == 0) {
                    4
                } else {
                    8
                };
                self.dma = Some(OamDma {
                    next: 0,
                    cycles_until_byte: delay,
                });
            }
            0xff4a => self.wy = value,
            0xff4b => self.wx = value,
            0xff47 => self.bgp = value,
            0xff48 => self.obp0 = value,
            0xff49 => self.obp1 = value,
            _ => return Err(DisplayDeviceError::UnsupportedPort(port)),
        }
        Ok(())
    }
    fn stat_edge(&mut self) {
        let line = self.lcd_enabled()
            && ((self.mode() == 0 && self.stat_enable & 8 != 0)
                || (self.mode() == 1 && self.stat_enable & 16 != 0)
                || (self.mode() == 2 && self.stat_enable & 32 != 0)
                || (self.ly == self.lyc && self.stat_enable & 64 != 0));
        if line && !self.stat_irq_line {
            self.iflags |= 2;
        }
        self.stat_irq_line = line;
    }
    /// Stateless scanline work from current SCX/window/OAM data. No NPC count,
    /// decoded sprite resource, visible-actor heuristic or scene duration.
    /// The selected ten objects count by OAM Y even if their X is offscreen.
    /// The fetch penalties follow Pan Docs Rendering / OBJ penalty algorithm.
    fn mode3_dots(&self) -> u16 {
        let scroll = u16::from(self.scx & 7);
        let window = self.lcdc & 0x21 == 0x21 && self.window_y_reached && self.wx <= 166;
        let window_x = i16::from(self.wx) - 7;
        let mut result = 172 + scroll + if window { 6 } else { 0 };
        if self.lcdc & 2 == 0 {
            return result;
        }
        let height = if self.lcdc & 4 != 0 { 16 } else { 8 };
        let mut selected = [(0u8, 0usize); 10];
        let mut len = 0;
        for (index, object) in self.oam.chunks_exact(4).enumerate() {
            let top = i16::from(object[0]) - 16;
            if top <= i16::from(self.ly) && i16::from(self.ly) < top + height {
                selected[len] = (object[1], index);
                len += 1;
                if len == 10 {
                    break;
                }
            }
        }
        // Stable X order; original OAM index breaks ties. Fixed ten slots.
        selected[..len].sort_unstable();
        let mut considered = [(false, 0i16); 10];
        let mut considered_len = 0;
        for &(x, _) in &selected[..len] {
            if x >= 168 {
                continue;
            }
            if x == 0 {
                result += 11;
                continue;
            }
            let pixel = i16::from(x) - 8;
            let in_window = window && pixel >= window_x;
            let within = if in_window {
                pixel - window_x
            } else {
                pixel + i16::from(self.scx)
            };
            let tile = (in_window, within.div_euclid(8));
            if !considered[..considered_len].contains(&tile) {
                considered[considered_len] = tile;
                considered_len += 1;
                result += (5 - within.rem_euclid(8)).max(0) as u16;
            }
            result += 6;
        }
        result
    }
    fn transition(&mut self) {
        use RasterPhase::*;
        match self.phase {
            Off => unreachable!(),
            EnableStore => {
                self.phase = StartupMode0;
                self.phase_remaining = 76;
            }
            StartupMode0 | Mode2 => {
                self.phase = Mode3;
                self.mode3_length = self.mode3_dots();
                self.phase_remaining = self.mode3_length;
            }
            Mode3 => {
                self.phase = HBlank;
                // Non-final visible lines advance LY four dots before mode2;
                // startup also shortens the final visible HBlank by four.
                let prelude = if self.ly < 143 || self.first_frame {
                    372
                } else {
                    376
                };
                self.phase_remaining = prelude - self.mode3_length;
            }
            HBlank if self.ly == 143 => {
                self.ly = 144;
                self.phase = VBlank;
                self.phase_remaining = 456;
                self.vblank_epoch += 1;
                self.iflags |= 1;
                self.window_y_reached = false;
            }
            HBlank => {
                self.phase = AdvanceLy;
                self.phase_remaining = 4;
                // LY is advanced at the end of HBlank, four dots before mode2.
                self.ly += 1;
            }
            AdvanceLy => {
                self.phase = Mode2;
                self.phase_remaining = 80;
                self.window_y_reached |= self.ly == self.wy;
            }
            VBlank if self.ly == 153 => {
                self.ly = 0;
                self.phase = Mode2;
                self.phase_remaining = 80;
                self.first_frame = false;
                self.window_y_reached = self.wy == 0;
            }
            VBlank => {
                self.ly += 1;
                self.phase_remaining = 456;
                if self.ly == 153 && self.first_frame {
                    // Restore the ordinary frame period after the shortened
                    // startup mode2 (80→76) and final HBlank (204→200).
                    self.phase_remaining += (80 - 76) + (204 - 200);
                }
            }
        }
        self.stat_edge();
    }
    /// Exact periodic state in an unmodified VBlank. No OBJ/window/STAT/DMA
    /// work can change a line length or publish data during this interval.
    /// The bounded predicate also rejects malformed deserialized state; all
    /// other states continue through the fully validated event path.
    #[cfg(test)]
    fn try_idle_vblank_period(&mut self, target: u64) -> bool {
        let Some(period) = CheckedFramePeriod::new(self.observed_tcycles, target) else {
            return false;
        };
        self.try_idle_checked_period(period)
    }
    #[inline(always)]
    fn idle_period_eligible(&self, from: u64) -> bool {
        if self.phase != RasterPhase::VBlank
            || self.lcdc & 0xa2 != 0x80
            || (self.first_frame | self.stat_irq_line | self.window_y_reached)
            || !(144..=153).contains(&self.ly)
            || !(1..=456).contains(&self.phase_remaining)
            || self.mode3_length != 172 + u16::from(self.scx & 7)
            || self.dma.is_some()
            || self.stat_enable != 0
            || self.oam.len() != 160
            || self.observed_tcycles != from
        {
            return false;
        }
        true
    }
    #[inline(always)]
    pub(super) fn try_idle_checked_period(&mut self, period: CheckedFramePeriod) -> bool {
        if !self.idle_period_eligible(period.from) {
            return false;
        }
        self.advance_eligible_period(period);
        true
    }
    // DisplayWorkTime retains eligibility only while its private device is
    // unchanged except for these exact periodic advances.
    #[inline(always)]
    pub(super) fn advance_eligible_period(&mut self, period: CheckedFramePeriod) {
        debug_assert!(self.idle_period_eligible(period.from));
        self.observed_tcycles = period.target;
        self.vblank_epoch += 1;
        self.iflags |= 1;
    }

    /// Synchronize to the sole owner's absolute time. No service is inferred
    /// from the number of host calls; splitting an interval is identical.
    /// DMA reads each byte from the live canonical shadow buffer at that byte's
    /// arrival cycle, so writes between calls remain observable.
    #[inline(always)]
    pub fn observe_time(
        &mut self,
        tcycles: u64,
        shadow_oam: &[u8],
    ) -> Result<(), DisplayDeviceError> {
        if tcycles < self.observed_tcycles {
            return Err(DisplayDeviceError::TimeReversed);
        }
        self.validate()?;
        if shadow_oam.len() != 160 || self.oam.len() != 160 {
            return Err(DisplayDeviceError::InvalidOam);
        }
        if self.lcd_enabled() && self.phase_remaining == 0 {
            return Err(DisplayDeviceError::InvalidPhase);
        }
        self.observe_time_after_validation(tcycles, shadow_oam)
    }

    /// The exclusive PatternBus has validated the device and retains its
    /// mutable borrow. Its checked writes and event transitions preserve
    /// validity; no external snapshot/device replacement can interleave.
    #[inline(always)]
    pub(super) fn observe_reserved_time(
        &mut self,
        tcycles: u64,
        instruction_tcycles: u8,
        shadow_oam: &[u8],
    ) -> Result<(), DisplayDeviceError> {
        debug_assert!(self.validate().is_ok());
        // The exclusive bus reserved a synchronized instruction and checked
        // elapsed + duration for overflow. Its duration is already known;
        // do not recover it with another absolute 64-bit subtraction.
        debug_assert_eq!(
            tcycles.checked_sub(self.observed_tcycles),
            Some(u64::from(instruction_tcycles))
        );
        if shadow_oam.len() != 160 || self.oam.len() != 160 {
            return Err(DisplayDeviceError::InvalidOam);
        }
        if self.dma.is_none()
            && (!self.lcd_enabled() || u16::from(instruction_tcycles) < self.phase_remaining)
        {
            if self.lcd_enabled() {
                self.phase_remaining -= u16::from(instruction_tcycles);
            }
            self.observed_tcycles = tcycles;
            return Ok(());
        }
        self.observe_time_events(tcycles, shadow_oam)
    }

    #[inline(always)]
    fn observe_time_after_validation(
        &mut self,
        tcycles: u64,
        shadow_oam: &[u8],
    ) -> Result<(), DisplayDeviceError> {
        let remaining = tcycles - self.observed_tcycles;
        // A source instruction often ends before the next device event.
        // Without DMA, only elapsed time (and the current LCD phase's
        // remainder) can change. Exact boundaries stay on the event path.
        if self.dma.is_none()
            && (!self.lcd_enabled() || remaining < u64::from(self.phase_remaining))
        {
            if self.lcd_enabled() {
                self.phase_remaining -= remaining as u16;
            }
            self.observed_tcycles = tcycles;
            return Ok(());
        }
        self.observe_time_events(tcycles, shadow_oam)
    }

    fn observe_time_events(
        &mut self,
        tcycles: u64,
        shadow_oam: &[u8],
    ) -> Result<(), DisplayDeviceError> {
        let mut remaining = tcycles - self.observed_tcycles;
        while remaining != 0 {
            // With unchanged empty OBJ state, no window/STAT/DMA and a
            // settled scan, whole periods return to precisely this phase.
            // Account their real VBlank edges without walking every line.
            // Startup, changed scroll/window latches and live transfers keep
            // the ordinary event path below.
            if remaining >= 154 * 456
                && self.lcd_enabled()
                && !self.first_frame
                && self.dma.is_none()
                && self.stat_enable == 0
                && self.lcdc & 0x20 == 0
                && self.mode3_length == 172 + u16::from(self.scx & 7)
                // The window latch clears on VBlank entry, not when LY wraps.
                // AdvanceLy exposes the next LY four dots before its latch.
                && self.window_y_reached
                    == (self.ly < 144
                        && (self.wy < self.ly
                            || (self.wy == self.ly && self.phase != RasterPhase::AdvanceLy)))
                && (self.lcdc & 2 == 0 || self.oam.iter().all(|&byte| byte == 0))
            {
                let frames = remaining / (154 * 456);
                self.vblank_epoch += frames;
                self.iflags |= 1;
                remaining %= 154 * 456;
                continue;
            }
            // An idle interval may span several complete visible scanlines.
            // With OBJ/window/STAT/DMA disabled, each settled mode2 start
            // returns after exactly 456 dots. Stop before the last visible
            // line, preserving its distinct VBlank edge on the event path.
            if remaining >= 456
                && self.phase == RasterPhase::Mode2
                && self.phase_remaining == 80
                && self.ly < 143
                && !self.first_frame
                && self.lcdc & 0xa2 == 0x80
                && self.dma.is_none()
                && self.stat_enable == 0
            {
                let lines = (remaining / 456).min(u64::from(143 - self.ly)) as u8;
                let previous_ly = self.ly;
                self.ly += lines;
                self.mode3_length = 172 + u16::from(self.scx & 7);
                self.window_y_reached |= previous_ly < self.wy && self.wy <= self.ly;
                self.stat_irq_line = false;
                remaining -= u64::from(lines) * 456;
                continue;
            }
            let lcd_next = if self.lcd_enabled() {
                u64::from(self.phase_remaining)
            } else {
                u64::MAX
            };
            let dma_next = self
                .dma
                .map_or(u64::MAX, |dma| u64::from(dma.cycles_until_byte));
            let part = remaining.min(lcd_next).min(dma_next);
            if self.lcd_enabled() {
                self.phase_remaining -= part as u16;
            }
            if let Some(dma) = self.dma.as_mut() {
                dma.cycles_until_byte -= part as u8;
            }
            remaining -= part;
            if self.dma.is_some_and(|dma| dma.cycles_until_byte == 0) {
                let dma = self.dma.as_mut().unwrap();
                self.oam[usize::from(dma.next)] = shadow_oam[usize::from(dma.next)];
                if dma.next == 159 {
                    self.dma = None;
                } else {
                    dma.next += 1;
                    dma.cycles_until_byte = 4;
                }
            }
            if self.lcd_enabled() && self.phase_remaining == 0 {
                self.transition();
            }
        }
        self.observed_tcycles = tcycles;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_instruction_duration_matches_checked_time_in_all_phases() {
        let shadow = core::array::from_fn::<_, 160, _>(|i| i as u8);
        for lcdc in [0, 0x91, 0x93, 0xb3] {
            let mut initial = DisplayDevice::powered_off(0);
            initial.write_port(0xff40, lcdc, 0xc3).unwrap();
            initial.write_port(0xff43, 7, 0xc3).unwrap();
            initial.write_port(0xff4a, 7, 0xc3).unwrap();
            initial.oam[0..4].copy_from_slice(&[32, 24, 1, 0]);
            for sample in (0..2 * 70224)
                .step_by(137)
                .chain([4, 80, 252, 65660, 70224])
            {
                let mut seed = initial.clone();
                seed.observe_time(sample, &shadow).unwrap();
                for dma in [false, true] {
                    let mut base = seed.clone();
                    if dma {
                        base.write_port(0xff46, 0xc3, 0xc3).unwrap();
                    }
                    for duration in [0, 1, 4, 8, 12, 16, 24, 80, 255] {
                        let mut reserved = base.clone();
                        let mut checked = base.clone();
                        assert_eq!(
                            reserved.observe_reserved_time(
                                sample + u64::from(duration),
                                duration,
                                &shadow
                            ),
                            checked.observe_time(sample + u64::from(duration), &shadow)
                        );
                        assert_eq!(
                            reserved, checked,
                            "lcdc={lcdc} sample={sample} dma={dma} dt={duration}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn short_instruction_advances_match_event_path_in_every_raster_phase() {
        let shadow = core::array::from_fn::<_, 160, _>(|i| i as u8);
        for lcdc in [0, 0x91, 0x93, 0xb3] {
            let mut state = DisplayDevice::powered_off(0);
            if lcdc != 0 {
                state.write_port(0xff40, lcdc, 0xc3).unwrap();
            }
            for sample in (0..2 * 70224u64).step_by(73) {
                state.observe_time_events(sample, &shadow).unwrap();
                let boundary = u64::from(state.phase_remaining);
                for delta in [
                    0,
                    1,
                    4,
                    8,
                    12,
                    16,
                    24,
                    boundary.saturating_sub(1),
                    boundary,
                    boundary + 1,
                    70224,
                ] {
                    let mut fast = state.clone();
                    let mut reference = state.clone();
                    fast.observe_time(sample + delta, &shadow).unwrap();
                    reference
                        .observe_time_events(sample + delta, &shadow)
                        .unwrap();
                    assert_eq!(
                        fast, reference,
                        "LCD={lcdc:02x}, sample={sample}, delta={delta}"
                    );
                }
            }
        }
        // A live DMA byte must never be bypassed, including an LCD-off bus.
        for lcdc in [0, 0x91] {
            let mut state = DisplayDevice::powered_off(0);
            if lcdc != 0 {
                state.write_port(0xff40, lcdc, 0xc3).unwrap();
            }
            state.write_port(0xff46, 0xc3, 0xc3).unwrap();
            for sample in 0..650u64 {
                let mut fast = state.clone();
                let mut reference = state.clone();
                fast.observe_time(sample, &shadow).unwrap();
                reference.observe_time_events(sample, &shadow).unwrap();
                assert_eq!(fast, reference, "DMA lcd={lcdc:02x}, sample={sample}");
            }
        }
    }

    #[test]
    fn short_advance_keeps_validation_and_time_reversal_errors() {
        let mut state = DisplayDevice::powered_off(10);
        assert_eq!(
            state.observe_time(9, &[0; 159]),
            Err(DisplayDeviceError::TimeReversed)
        );
        assert_eq!(
            state.observe_time(10, &[0; 159]),
            Err(DisplayDeviceError::InvalidOam)
        );
        state.stat_enable = 8;
        assert_eq!(
            state.observe_time(14, &[0; 160]),
            Err(DisplayDeviceError::UnsupportedStatInterrupts(8))
        );
        state.stat_enable = 0;
        state.phase_remaining = 1;
        assert_eq!(
            state.observe_time(14, &[0; 160]),
            Err(DisplayDeviceError::InvalidPhase)
        );
    }

    #[test]
    fn checked_frame_period_rejects_wrong_lengths_overflow_and_desynchronized_devices() {
        for (from, target) in [
            (0, 70223),
            (0, 70225),
            (1, 0),
            (u64::MAX, 0),
            (u64::MAX - 10, u64::MAX),
        ] {
            assert!(CheckedFramePeriod::new(from, target).is_none());
        }
        assert!(CheckedFramePeriod::new(u64::MAX - 70224, u64::MAX).is_some());
        let mut device = DisplayDevice::powered_off(0);
        device.write_port(0xff40, 0x91, 0xc3).unwrap();
        device.observe_time(2 * 70224, &[0; 160]).unwrap();
        let before = device.clone();
        let period = CheckedFramePeriod::new(2 * 70224 + 4, 3 * 70224 + 4).unwrap();
        assert!(!device.try_idle_checked_period(period));
        assert_eq!(device, before);
    }

    #[test]
    fn passive_vblank_forecast_matches_single_cycle_event_runner() {
        for scx in [0, 3, 7] {
            let mut device = DisplayDevice::powered_off(0);
            device.write_port(0xff40, 0x91, 0xc3).unwrap();
            device.write_port(0xff43, scx, 0xc3).unwrap();
            let mut samples = Vec::new();
            let mut entries = Vec::new();
            let mut epoch = 0;
            for at in 0..4 * 70224u64 {
                device.observe_time(at, &[0; 160]).unwrap();
                if device.vblank_epoch() != epoch {
                    entries.push((at, device.clone()));
                    epoch = device.vblank_epoch();
                }
                if at < 3 * 70224 && (at % 113 == 0 || device.phase_remaining == 1) {
                    let before = device.clone();
                    let prediction = device.tcycles_until_vblank();
                    assert_eq!(device, before, "query must not advance any state");
                    if let Some(distance) = prediction {
                        samples.push((at, before, distance));
                    }
                }
            }
            assert!(!samples.is_empty());
            for (at, mut state, distance) in samples {
                let (edge, expected) = entries.iter().find(|(edge, _)| *edge > at).unwrap();
                assert_eq!(
                    u64::from(distance),
                    edge - at,
                    "SCX={scx}, at={at}, state={state:?}"
                );
                state
                    .observe_time(at + u64::from(distance), &[0; 160])
                    .unwrap();
                assert_eq!(&state, expected, "SCX={scx}, at={at}");
            }
        }
    }

    #[test]
    fn vblank_forecast_rejects_dynamic_off_and_invalid_rasters() {
        let mut base = DisplayDevice::powered_off(0);
        assert_eq!(base.tcycles_until_vblank(), None);
        base.write_port(0xff40, 0x91, 0xc3).unwrap();
        base.observe_time(2 * 70224, &[0; 160]).unwrap();
        for case in 0..8 {
            let mut state = base.clone();
            match case {
                0 => state.lcdc |= 2,
                1 => state.lcdc |= 0x20,
                2 => state.stat_enable = 8,
                3 => {
                    state.dma = Some(OamDma {
                        next: 0,
                        cycles_until_byte: 8,
                    })
                }
                4 => state.oam.clear(),
                5 => state.ly = 154,
                6 => state.phase_remaining = 0,
                _ => state.stat_irq_line = true,
            }
            assert_eq!(state.tcycles_until_vblank(), None, "case={case}");
        }
    }

    #[test]
    fn idle_visible_lines_match_single_dot_steps_including_partial_endpoints() {
        let shadow = [0; 160];
        for scx in [0, 3, 7] {
            for wy in [0, 7, 144, 255] {
                for ly in [0, 6, 7, 142, 143] {
                    let mut base = DisplayDevice::powered_off(0);
                    base.write_port(0xff40, 0x91, 0xc3).unwrap();
                    base.scx = scx;
                    base.wy = wy;
                    base.ly = ly;
                    base.phase = RasterPhase::Mode2;
                    base.phase_remaining = 80;
                    base.mode3_length = 172 + u16::from(scx & 7);
                    base.first_frame = false;
                    base.window_y_reached = wy <= ly;
                    // OBJ is off; nonempty OAM must remain untouched.
                    base.oam[0] = 32;
                    for duration in [455, 456, 457, 912, 30_003, 65_664] {
                        let mut fast = base.clone();
                        fast.observe_time(duration, &shadow).unwrap();
                        let mut reference = base.clone();
                        for at in 1..=duration {
                            reference.observe_time(at, &shadow).unwrap();
                        }
                        assert_eq!(fast, reference, "scx={scx} wy={wy} ly={ly} dt={duration}");
                    }
                }
            }
        }
    }

    #[test]
    fn empty_steady_frame_fast_forward_matches_small_device_steps() {
        for (scx, wy) in [(0, 0), (3, 7), (7, 144)] {
            let mut fast = DisplayDevice::powered_off(0);
            fast.write_port(0xff40, 0x91, 0xc3).unwrap();
            fast.write_port(0xff43, scx, 0xc3).unwrap();
            fast.write_port(0xff4a, wy, 0xc3).unwrap();
            let mut small = fast.clone();
            let end = 64 * 154 * 456 + 183;
            fast.observe_time(end, &[0; 160]).unwrap();
            let mut time = 0;
            while time < end {
                time = (time + 4096).min(end);
                small.observe_time(time, &[0; 160]).unwrap();
            }
            assert_eq!(fast, small);
        }
    }
    #[test]
    fn steady_frame_skip_preserves_vblank_and_visible_line_latches() {
        // Native whole-update grants normally land in VBlank, where LY >= WY
        // but the window latch is cleared. Compare every field against small
        // steps that cannot enter the whole-frame shortcut.
        for wy in [0, 7, 144] {
            for offset in [0, 4, 80, 256, 456, 10_000] {
                for lcdc in [0x91, 0x93] {
                    let mut fast = DisplayDevice::powered_off(0);
                    fast.write_port(0xff40, lcdc, 0xc3).unwrap();
                    fast.write_port(0xff4a, wy, 0xc3).unwrap();
                    // Nonempty OAM is irrelevant when OBJ rendering is off.
                    if lcdc & 2 == 0 {
                        fast.oam[0] = 32;
                    }
                    let start = 2 * 154 * 456 + offset;
                    fast.observe_time(start, &[0; 160]).unwrap();
                    let mut small = fast.clone();
                    let end = start + 16 * 154 * 456 + 183;
                    fast.observe_time(end, &[0; 160]).unwrap();
                    let mut time = start;
                    while time < end {
                        time = (time + 4096).min(end);
                        small.observe_time(time, &[0; 160]).unwrap();
                    }
                    assert_eq!(fast, small, "wy={wy} offset={offset} lcdc={lcdc}");
                }
            }
        }
    }
    #[test]
    fn one_period_native_grants_match_small_steps_after_every_update() {
        for wy in [0, 7, 144] {
            let mut fast = DisplayDevice::powered_off(0);
            fast.write_port(0xff40, 0x91, 0xc3).unwrap();
            fast.write_port(0xff4a, wy, 0xc3).unwrap();
            let mut small = fast.clone();
            let mut time = 0;
            for frame in 1..=32 {
                let target = frame * 154 * 456;
                fast.observe_time(target, &[0; 160]).unwrap();
                while time < target {
                    time = (time + 4096).min(target);
                    small.observe_time(time, &[0; 160]).unwrap();
                }
                assert_eq!(fast, small, "frame={frame} wy={wy}");
            }
        }
    }
    #[test]
    fn idle_vblank_period_rejects_nonperiodic_or_malformed_snapshots() {
        let mut base = DisplayDevice::powered_off(0);
        base.write_port(0xff40, 0x91, 0xc3).unwrap();
        let start = 2 * 154 * 456;
        base.observe_time(start, &[0; 160]).unwrap();
        let target = start + 154 * 456;
        let mut fast = base.clone();
        assert!(fast.try_idle_vblank_period(target));
        let mut small = base.clone();
        let mut at = start;
        while at < target {
            at = (at + 4096).min(target);
            small.observe_time(at, &[0; 160]).unwrap();
        }
        assert_eq!(fast, small);
        for case in 0..12 {
            let mut state = base.clone();
            match case {
                0 => state.lcdc |= 2,
                1 => state.lcdc |= 0x20,
                2 => state.first_frame = true,
                3 => state.ly = 154,
                4 => state.phase_remaining = 0,
                5 => state.phase_remaining = 457,
                6 => state.mode3_length += 1,
                7 => state.stat_enable = 1,
                8 => state.stat_irq_line = true,
                9 => state.window_y_reached = true,
                10 => {
                    state.oam.pop();
                }
                _ => {
                    state.dma = Some(OamDma {
                        next: 0,
                        cycles_until_byte: 8,
                    })
                }
            }
            let before = state.clone();
            assert!(!state.try_idle_vblank_period(target), "case={case}");
            assert_eq!(state, before, "fallback must not partially advance");
        }
        assert!(!base.try_idle_vblank_period(target - 1));
        assert!(!base.try_idle_vblank_period(target + 1));
    }
    fn enabled() -> DisplayDevice {
        let mut device = DisplayDevice::powered_off(0);
        device.write_port(0xff40, 0xe3, 0xc3).unwrap();
        device
    }
    #[test]
    fn lcd_off_preserves_pending_if_and_stops_only_future_edges() {
        let mut device = enabled();
        let shadow = [0; 160];
        // Literal startup phase sequence, not a measured FLASH duration.
        let first = 4 + 76 + 172 + 200 + 4 + 142 * 456 + 80 + 172 + 200;
        device.observe_time(first - 1, &shadow).unwrap();
        assert_eq!(device.vblank_epoch(), 0);
        device.observe_time(first, &shadow).unwrap();
        assert_eq!(device.read_port(0xff44), Ok(144));
        assert_eq!(device.vblank_epoch(), 1);
        device.observe_time(first + 456, &shadow).unwrap();
        assert_eq!(device.read_port(0xff44), Ok(145));
        device.write_port(0xffff, 12, 0xc3).unwrap();
        device.write_port(0xff40, 0x63, 0xc3).unwrap();
        assert_eq!(device.read_port(0xff44), Ok(0));
        assert_eq!(device.pending_interrupts(), 0);
        device.write_port(0xffff, 13, 0xc3).unwrap();
        assert_eq!(device.pending_interrupts(), 1);
        device.observe_time(first + 3 * 70224, &shadow).unwrap();
        assert_eq!(device.vblank_epoch(), 1);
        assert_eq!(device.pending_interrupts(), 1);
        device.acknowledge_interrupt(1);
        assert_eq!(device.pending_interrupts(), 0);
    }
    #[test]
    fn elapsed_time_partition_and_midphase_snapshot_do_not_change_device_edges() {
        let mut whole = enabled();
        let mut split = whole.clone();
        let shadow = [0; 160];
        whole.observe_time(4 + 7 * 70224 + 179, &shadow).unwrap();
        split.observe_time(701, &shadow).unwrap();
        let json = serde_json::to_vec(&split).unwrap();
        let mut split: DisplayDevice = serde_json::from_slice(&json).unwrap();
        let target = 4 + 7 * 70224 + 179;
        let mut now = 701;
        while now < target {
            now = (now + 73).min(target);
            split.observe_time(now, &shadow).unwrap();
        }
        assert_eq!(split, whole);
        assert!(!whole.blank_first_frame());
        let before = split.clone();
        assert_eq!(
            split.observe_time(target - 1, &shadow),
            Err(DisplayDeviceError::TimeReversed)
        );
        assert_eq!(split, before);
    }
    #[test]
    fn dma_reads_live_shadow_bytes_and_continues_while_lcd_is_off() {
        let mut device = DisplayDevice::powered_off(0);
        let mut shadow = [42; 160];
        device.write_port(0xff46, 0xc3, 0xc3).unwrap();
        device.observe_time(7, &shadow).unwrap();
        assert_eq!(device.oam()[0], 0);
        device.observe_time(8, &shadow).unwrap();
        assert_eq!(device.oam()[0], 42);
        shadow[0] = 77;
        shadow[1] = 99;
        device.observe_time(12, &shadow).unwrap();
        assert_eq!(&device.oam()[..2], &[42, 99]);
        let json = serde_json::to_vec(&device).unwrap();
        let mut restored: DisplayDevice = serde_json::from_slice(&json).unwrap();
        device.observe_time(644, &shadow).unwrap();
        restored.observe_time(644, &shadow).unwrap();
        assert_eq!(device, restored);
        assert!(!device.dma_active());
        assert_eq!(device.vblank_epoch(), 0);
        assert_eq!(device.oam()[159], 42);
    }
    #[test]
    fn mode3_uses_raw_ten_object_selection_tile_groups_and_window_scroll() {
        let mut device = enabled();
        device.lcdc = 0x83;
        device.scx = 0;
        device.window_y_reached = false;
        assert_eq!(device.mode3_dots(), 172);
        for object in device.oam[..40].chunks_exact_mut(4) {
            object[0] = 16;
            object[1] = 200; // consumes one selected slot without fetching
        }
        device.oam[40] = 16;
        device.oam[41] = 16; // eleventh object must not be fetched
        assert_eq!(device.mode3_dots(), 172);
        device.oam[1] = 0; // X=0 has its hardware-specific eleven-dot cost
        assert_eq!(device.mode3_dots(), 183);
        device.oam[1] = 16;
        device.oam[5] = 16; // same background tile is considered only once
        assert_eq!(device.mode3_dots(), 189);
        device.lcdc &= !2;
        device.lcdc |= 0x20;
        device.scx = 3;
        device.wx = 7;
        device.window_y_reached = true;
        assert_eq!(device.mode3_dots(), 181);
    }
    #[test]
    fn unsupported_sources_or_stat_irq_do_not_fake_device_progress() {
        let mut device = enabled();
        let before = device.clone();
        assert_eq!(
            device.write_port(0xff46, 0x80, 0xc3),
            Err(DisplayDeviceError::UnsupportedDmaSource(0x80))
        );
        assert_eq!(
            device.write_port(0xff41, 0x48, 0xc3),
            Err(DisplayDeviceError::UnsupportedStatInterrupts(0x48))
        );
        assert_eq!(
            device.observe_time(100, &[0; 159]),
            Err(DisplayDeviceError::InvalidOam)
        );
        assert_eq!(device, before);
    }
}

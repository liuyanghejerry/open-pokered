//! Defer only untouched, periodic native idle grants. Device/instruction
//! access and snapshots see precisely the same time as immediate stepping.
//! This does not service interrupts, poll input, or advance a source child.
use super::{
    DisplayInstruction, DisplayInstructionBus, DisplayWorkError, DisplayWorkTime,
    NATIVE_FRAME_T_CYCLES,
};
use crate::alloc_prelude::*;
use core::{
    cell::OnceCell,
    ops::{Deref, DerefMut},
};

#[derive(Debug)]
#[repr(C)]
pub struct NativeDisplayClock {
    // Counts of whole grants and completed idle periods since `time`.
    // Keeping both preserves reads/mutations made during an unconsumed grant.
    grants: u32,
    pending_periods: u32,
    // Zero disables deferral. Otherwise this is an exclusive validity receipt
    // and the maximum number of grants before a checked counter can overflow.
    capacity: u32,
    view: OnceCell<Box<DisplayWorkTime>>,
    // Keep the frequently accessed counters at a small fixed offset on
    // Thumb-1. The large, inactive instruction state follows that prefix.
    time: DisplayWorkTime,
}
impl Default for NativeDisplayClock {
    fn default() -> Self {
        DisplayWorkTime::default().into()
    }
}
impl From<DisplayWorkTime> for NativeDisplayClock {
    fn from(time: DisplayWorkTime) -> Self {
        let capacity = Self::idle_capacity(&time);
        Self {
            time,
            grants: 0,
            pending_periods: 0,
            capacity,
            view: OnceCell::new(),
        }
    }
}
impl NativeDisplayClock {
    fn idle_capacity(time: &DisplayWorkTime) -> u32 {
        if time.elapsed != time.deadline
            || time.active_vblank
            || time.vblank_dispatch_enabled
            || time.legacy_sprite_hide.is_some()
            || time.shadow_oam.len() != 160
            || !time.device.idle_period_eligible(time.elapsed)
        {
            return 0;
        }
        ((u64::MAX - time.deadline) / NATIVE_FRAME_T_CYCLES)
            .min(u64::MAX - time.device.vblank_epoch())
            .min(u64::from(u32::MAX)) as u32
    }
    fn project(time: &mut DisplayWorkTime, grants: u32, completed: u32) {
        debug_assert!(completed <= grants);
        time.deadline = time
            .deadline
            .checked_add(u64::from(grants) * NATIVE_FRAME_T_CYCLES)
            .expect("display deadline overflow");
        if completed != 0 {
            let cycles = u64::from(completed) * NATIVE_FRAME_T_CYCLES;
            let target = time
                .elapsed
                .checked_add(cycles)
                .expect("display time overflow");
            debug_assert!(target <= time.deadline);
            time.device.advance_eligible_periods(completed, target);
            time.divider_counter = time.divider_counter.wrapping_add(cycles as u16);
            time.elapsed = target;
        }
    }
    #[inline(never)]
    fn synchronize(&mut self) {
        if self.grants != 0 {
            if let Some(view) = self.view.take() {
                self.time = *view;
            } else {
                Self::project(&mut self.time, self.grants, self.pending_periods);
            }
        }
        self.grants = 0;
        self.pending_periods = 0;
        self.capacity = 0;
    }
    // Keep the cloned view's large destructor out of ordinary update code.
    #[inline(never)]
    fn discard_view(&mut self) {
        self.view.take();
    }
    #[inline]
    fn invalidate_view(&mut self) {
        if self.view.get().is_some() {
            self.discard_view();
        }
    }
    /// Normalize completed idle periods and granted budget separately. No
    /// current grant is consumed by a read or snapshot.
    #[inline]
    pub fn as_time(&self) -> &DisplayWorkTime {
        if self.grants == 0 {
            &self.time
        } else {
            self.view.get_or_init(|| {
                let mut view = Box::new(self.time.clone());
                Self::project(&mut view, self.grants, self.pending_periods);
                view
            })
        }
    }
    #[inline]
    pub fn as_time_mut(&mut self) -> &mut DisplayWorkTime {
        if self.capacity != 0 {
            self.synchronize();
        }
        debug_assert_eq!(self.grants, 0);
        &mut self.time
    }
    #[inline(always)]
    pub fn begin_native_update(&mut self) {
        if self.capacity != 0 && self.grants < self.capacity {
            self.invalidate_view();
            self.grants += 1;
        } else {
            self.begin_instruction_grant();
        }
    }

    #[inline(never)]
    fn begin_instruction_grant(&mut self) {
        if self.capacity != 0 {
            self.synchronize();
        }
        self.time.begin_native_update();
    }

    /// Game's native frame entry. A retained idle receipt proves there is no
    /// hide child to service; a caller mutation invalidates that receipt.
    #[inline]
    pub fn begin_native_frame(&mut self) -> Result<bool, DisplayWorkError> {
        self.begin_native_update();
        if self.capacity != 0 {
            Ok(true)
        } else {
            self.time.service_legacy_sprite_hide()
        }
    }

    /// Finish a frame that ran the foreground, including a hide child newly
    /// requested there. No additional child attempt follows a refused entry.
    #[inline]
    pub fn finish_native_frame(&mut self) -> Result<(), DisplayWorkError> {
        if self.capacity == 0 {
            self.time.service_legacy_sprite_hide()?;
        }
        self.idle_to_target()
    }
    #[inline(always)]
    pub fn idle_to_target(&mut self) -> Result<(), DisplayWorkError> {
        if self.capacity != 0 {
            if self.pending_periods != self.grants {
                self.invalidate_view();
            }
            self.pending_periods = self.grants;
            return Ok(());
        }
        self.idle_instruction_grant()
    }

    #[inline(never)]
    fn idle_instruction_grant(&mut self) -> Result<(), DisplayWorkError> {
        self.time.idle_to_target()?;
        self.capacity = Self::idle_capacity(&self.time);
        Ok(())
    }
    #[inline]
    pub fn service_legacy_sprite_hide(&mut self) -> Result<bool, DisplayWorkError> {
        if self.time.legacy_sprite_hide.is_none() {
            Ok(true)
        } else {
            self.as_time_mut().service_legacy_sprite_hide()
        }
    }
    #[inline]
    pub fn finish_legacy_party_sprite_owner(&mut self) {
        if self.time.legacy_party_sprite_owner {
            self.as_time_mut().finish_legacy_party_sprite_owner();
        }
    }
}
impl Deref for NativeDisplayClock {
    type Target = DisplayWorkTime;
    #[inline]
    fn deref(&self) -> &DisplayWorkTime {
        self.as_time()
    }
}
impl DerefMut for NativeDisplayClock {
    #[inline]
    fn deref_mut(&mut self) -> &mut DisplayWorkTime {
        self.as_time_mut()
    }
}
impl DisplayInstructionBus for NativeDisplayClock {
    type Error = DisplayWorkError;
    #[inline]
    fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
        self.as_time_mut().execute(instruction)
    }
}
impl crate::overworld::display_delay_work::DelayFrameBus for NativeDisplayClock {
    fn interrupt_returns(&self) -> u64 {
        self.as_time().interrupt_returns
    }
}
impl crate::overworld::vblank_return_work::VBlankReturnBus for NativeDisplayClock {
    fn check_active_vblank(&self) -> Result<(), DisplayWorkError> {
        self.as_time().check_active_vblank()
    }
    fn execute_reti(&mut self) -> Result<(), DisplayWorkError> {
        self.as_time_mut().execute_reti()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::display_lcd_work::DisplayAccess;
    use crate::overworld::vblank_return_work::VBlankReturnBus;

    // Compare the entire serialized causal state, not just elapsed cycles.
    // Each oracle interval is shorter than a frame; it cannot use a whole
    // period shortcut in either DisplayWorkTime or DisplayDevice.
    fn idle_small(time: &mut DisplayWorkTime) {
        while time.remaining() != 0 {
            time.elapse(time.remaining().min(4096)).unwrap();
        }
    }
    fn same(clock: &NativeDisplayClock, oracle: &DisplayWorkTime) {
        assert_eq!(
            serde_json::to_value(clock.as_time()).unwrap(),
            serde_json::to_value(oracle).unwrap()
        );
        assert_eq!(clock.device(), oracle.device());
    }
    fn advance(clock: &mut NativeDisplayClock, oracle: &mut DisplayWorkTime, count: usize) {
        for _ in 0..count {
            clock.begin_native_update();
            oracle.begin_native_update();
            assert!(clock.service_legacy_sprite_hide().unwrap());
            clock.idle_to_target().unwrap();
            idle_small(oracle);
        }
    }
    fn pair() -> (NativeDisplayClock, DisplayWorkTime) {
        (NativeDisplayClock::default(), DisplayWorkTime::default())
    }

    #[test]
    fn grant_alone_does_not_raise_a_new_vblank_request() {
        let mut oracle = DisplayWorkTime::legacy_native_projection(4);
        oracle.device_mut().acknowledge_interrupt(1);
        let mut clock: NativeDisplayClock = oracle.clone().into();
        assert_ne!(clock.capacity, 0);
        clock.begin_native_update();
        oracle.begin_native_update();
        same(&clock, &oracle);
        assert_eq!(clock.device().pending_interrupts(), 0);
        clock.idle_to_target().unwrap();
        idle_small(&mut oracle);
        same(&clock, &oracle);
        assert_eq!(clock.device().pending_interrupts(), 1);
    }

    #[test]
    fn bounded_grant_counter_flushes_without_consuming_the_next_grant() {
        let mut oracle = DisplayWorkTime::legacy_native_projection(4);
        let mut clock: NativeDisplayClock = oracle.clone().into();
        // A conservative smaller batch exercises the same checked rollover
        // without needing billions of native updates in a unit test.
        clock.capacity = 2;
        for frame in 0..7 {
            clock.begin_native_update();
            oracle.begin_native_update();
            if frame == 2 {
                assert_eq!(clock.grants, 0);
                assert_eq!(clock.capacity, 0);
            }
            same(&clock, &oracle);
            clock.idle_to_target().unwrap();
            idle_small(&mut oracle);
            same(&clock, &oracle);
        }
    }

    #[test]
    fn deadline_overflow_is_rejected_by_the_same_begin_call() {
        let settled = DisplayWorkTime::legacy_native_projection(4);
        let mut value = serde_json::to_value(settled).unwrap();
        let start = u64::MAX - NATIVE_FRAME_T_CYCLES;
        value["elapsed"] = start.into();
        value["deadline"] = start.into();
        value["device"]["observed_tcycles"] = start.into();
        let mut oracle: DisplayWorkTime = serde_json::from_value(value).unwrap();
        let mut clock: NativeDisplayClock = oracle.clone().into();
        assert_eq!(clock.capacity, 1);
        advance(&mut clock, &mut oracle, 1);
        same(&clock, &oracle);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || clock.begin_native_update()
        ))
        .is_err());
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || oracle.begin_native_update()
        ))
        .is_err());
        same(&clock, &oracle);
    }

    #[test]
    fn deferred_idle_matches_small_steps_across_divider_wraps_and_reads() {
        let (mut clock, mut oracle) = pair();
        for count in [1, 2, 3, 32, 256, 4097] {
            advance(&mut clock, &mut oracle, count);
            same(&clock, &oracle);
            same(&clock, &oracle); // reusing an immutable view changes nothing
        }
        assert!(clock.pending_periods > 4000);
        assert_eq!(clock.interrupt_returns, 0);
    }

    #[test]
    fn read_during_current_grant_does_not_consume_that_grant() {
        let (mut clock, mut oracle) = pair();
        advance(&mut clock, &mut oracle, 15);
        clock.begin_native_update();
        oracle.begin_native_update();
        same(&clock, &oracle);
        assert_eq!(clock.remaining(), NATIVE_FRAME_T_CYCLES);
        clock.idle_to_target().unwrap();
        idle_small(&mut oracle);
        same(&clock, &oracle);
        // begin must discard/adopt the cached view's old deadline.
        clock.begin_native_update();
        oracle.begin_native_update();
        same(&clock, &oracle);
    }

    #[test]
    fn mutable_access_flushes_before_palette_scroll_dma_and_lcd_effects() {
        for (port, value) in [
            (0xff47, 0),
            (0xff43, 5),
            (0xff46, 0xc3),
            (0xff40, 0xb1),
            (0xff40, 0x11),
        ] {
            let (mut clock, mut oracle) = pair();
            advance(&mut clock, &mut oracle, 21);
            same(&clock, &oracle); // mutation must adopt this read view too
            assert_eq!(
                clock.device_mut().write_port(port, value, 0xc3),
                oracle.device_mut().write_port(port, value, 0xc3)
            );
            assert_eq!(clock.pending_periods, 0);
            advance(&mut clock, &mut oracle, 7);
            same(&clock, &oracle);
        }
    }

    #[test]
    fn visible_phase_after_lcd_restart_can_batch_without_a_phase_reset() {
        let (mut clock, mut oracle) = pair();
        advance(&mut clock, &mut oracle, 7);
        for time in [clock.as_time_mut(), &mut oracle] {
            time.begin_native_update();
            time.elapse(12344).unwrap();
            time.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
            time.elapse(1200).unwrap();
            time.device_mut().write_port(0xff40, 0x91, 0xc3).unwrap();
        }
        clock.idle_to_target().unwrap();
        idle_small(&mut oracle);
        advance(&mut clock, &mut oracle, 19);
        assert!(clock.pending_periods > 10);
        assert!(clock.device().read_port(0xff44).unwrap() < 144);
        same(&clock, &oracle);
    }

    #[test]
    fn irq_attachment_observes_all_edges_and_does_not_invent_returns() {
        let (mut clock, mut oracle) = pair();
        advance(&mut clock, &mut oracle, 31);
        clock.set_vblank_dispatch_enabled(true);
        oracle.set_vblank_dispatch_enabled(true);
        clock.begin_native_update();
        oracle.begin_native_update();
        let instruction = DisplayInstruction {
            tcycles: 4,
            access: None,
        };
        assert_eq!(
            clock.execute(instruction),
            Err(DisplayWorkError::InterruptPending)
        );
        assert_eq!(
            oracle.execute(instruction),
            Err(DisplayWorkError::InterruptPending)
        );
        assert_eq!(clock.enter_pending_vblank(), oracle.enter_pending_vblank());
        assert_eq!(clock.execute(instruction), oracle.execute(instruction));
        assert_eq!(clock.execute_reti(), oracle.execute_reti());
        clock.idle_to_target().unwrap();
        idle_small(&mut oracle);
        same(&clock, &oracle);
        assert_eq!(clock.interrupt_returns, 1);
        assert_eq!(clock.capacity, 0);
    }

    #[test]
    fn legacy_hide_runs_at_its_actual_grant_after_deferred_idle() {
        let (mut clock, mut oracle) = pair();
        advance(&mut clock, &mut oracle, 13);
        clock.begin_native_update();
        oracle.begin_native_update();
        clock.begin_legacy_party_sprite_hide().unwrap();
        oracle.begin_legacy_party_sprite_hide().unwrap();
        assert_eq!(
            clock.service_legacy_sprite_hide(),
            oracle.service_legacy_sprite_hide()
        );
        same(&clock, &oracle);
        clock.finish_legacy_party_sprite_owner();
        oracle.finish_legacy_party_sprite_owner();
        clock.idle_to_target().unwrap();
        idle_small(&mut oracle);
        advance(&mut clock, &mut oracle, 12);
        same(&clock, &oracle);
    }

    #[test]
    fn skipped_idle_and_instruction_overrun_do_not_add_an_extra_period() {
        let (mut clock, mut oracle) = pair();
        advance(&mut clock, &mut oracle, 5);
        // Multiple grants without idle still leave time at the completed one.
        for _ in 0..2 {
            clock.begin_native_update();
            oracle.begin_native_update();
        }
        same(&clock, &oracle);
        clock.idle_to_target().unwrap();
        idle_small(&mut oracle);
        clock.begin_native_update();
        oracle.begin_native_update();
        for time in [clock.as_time_mut(), &mut oracle] {
            time.elapse(time.remaining() - 4).unwrap();
        }
        let instruction = DisplayInstruction {
            tcycles: 12,
            access: Some(DisplayAccess::Read {
                port: 0xff44,
                at_tcycle: 8,
            }),
        };
        assert_eq!(clock.execute(instruction), oracle.execute(instruction));
        assert_eq!(clock.remaining(), 0);
        clock.idle_to_target().unwrap();
        oracle.idle_to_target().unwrap();
        advance(&mut clock, &mut oracle, 7);
        same(&clock, &oracle);
    }

    #[test]
    fn serialized_snapshot_contains_completed_periods_and_resumes_without_cache() {
        let (mut clock, mut oracle) = pair();
        advance(&mut clock, &mut oracle, 33);
        clock.begin_native_update();
        oracle.begin_native_update();
        let encoded = serde_json::to_string(clock.as_time()).unwrap();
        let time: DisplayWorkTime = serde_json::from_str(&encoded).unwrap();
        let mut restored: NativeDisplayClock = time.into();
        assert_eq!(restored.pending_periods, 0);
        same(&restored, &oracle);
        restored.idle_to_target().unwrap();
        idle_small(&mut oracle);
        advance(&mut restored, &mut oracle, 19);
        same(&restored, &oracle);
    }
}

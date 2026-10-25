//! Foreground instruction preemption, not a fabricated ISR implementation.
use super::*;
use crate::overworld::{
    display_delay_work::DelayFrameBus, display_lcd_work::DisableLcdWork,
    vblank_return_work::VBlankReturnWork,
};

fn plain() -> DisplayInstruction {
    DisplayInstruction {
        tcycles: 8,
        access: None,
    }
}

fn near_vblank() -> DisplayWorkTime {
    let mut time = DisplayWorkTime::default();
    time.begin_native_update();
    let distance = time.device.tcycles_until_vblank().unwrap();
    time.elapse(u64::from(distance - 4)).unwrap();
    assert_eq!(time.device.pending_interrupts() & 1, 0);
    time.set_vblank_dispatch_enabled(true);
    time
}

#[test]
fn pending_irq_stops_the_next_instruction_on_checked_and_cached_display_buses() {
    for cached in [false, true] {
        let mut time = near_vblank();
        let mut bus = time.display_bus();
        if cached {
            bus.execute(plain()).unwrap();
        } else {
            DisplayInstructionBus::execute(bus.time, plain()).unwrap();
        }
        let before = bus.time.elapsed();
        let flags = bus.time.device.read_port(0xff0f).unwrap();
        assert!(bus.time.pending_vblank_dispatch());
        let result = if cached {
            bus.execute(plain())
        } else {
            DisplayInstructionBus::execute(bus.time, plain())
        };
        assert_eq!(result, Err(DisplayWorkError::InterruptPending));
        assert_eq!(bus.time.elapsed(), before);
        assert_eq!(bus.time.device.read_port(0xff0f).unwrap(), flags);
        assert_eq!(bus.time.interrupt_returns(), 0);
        assert!(!bus.time.active_vblank());
    }
}

struct NoAssets;

#[test]
fn external_irq_access_finishes_an_admitted_instruction_across_the_grant_once() {
    use core::cell::Cell;
    let mut time = DisplayWorkTime::default();
    time.begin_native_update();
    time.device_mut().request_interrupt(1);
    time.enter_pending_vblank().unwrap();
    time.elapse(time.remaining() - 4).unwrap();
    let before = time.elapsed();
    let writes = Cell::new(0);
    assert_eq!(time.execute_external_irq_access(12, 8, || {
        writes.set(writes.get() + 1);
        0x77
    }), Ok(0x77));
    assert_eq!(time.elapsed(), before + 12);
    assert_eq!(time.remaining(), 0);
    assert_eq!(writes.get(), 1);
    let retained = serde_json::to_value(&time).unwrap();
    assert_eq!(time.execute_external_irq_access(12, 8, || writes.set(2)), Err(DisplayWorkError::Budget));
    assert_eq!(writes.get(), 1);
    assert_eq!(serde_json::to_value(&time).unwrap(), retained);
    time.begin_native_update();
    time.execute_external_irq_access(12, 8, || writes.set(2)).unwrap();
    assert_eq!(writes.get(), 2);
    assert_eq!(time.elapsed(), before + 24);
}

#[test]
fn external_access_requires_the_live_irq_and_valid_instruction_before_effects() {
    use core::cell::Cell;
    let mut time = DisplayWorkTime::default();
    time.begin_native_update();
    let touched = Cell::new(false);
    assert_eq!(time.execute_external_irq_access(12, 8, || touched.set(true)), Err(DisplayWorkError::NoActiveVBlank));
    assert!(!touched.get());
    time.device_mut().request_interrupt(1);
    time.enter_pending_vblank().unwrap();
    for (total, access) in [(0, 0), (3, 0), (12, 16), (12, 5)] {
        let before = serde_json::to_value(&time).unwrap();
        assert!(time.execute_external_irq_access(total, access, || touched.set(true)).is_err());
        assert!(!touched.get());
        assert_eq!(serde_json::to_value(&time).unwrap(), before);
    }
}
impl PatternAssets for NoAssets {
    fn read_pattern_byte(
        &self,
        _: super::super::field_restore::PatternSource,
        _: u16,
    ) -> Option<u8> {
        None
    }
}

#[test]
fn pattern_bus_cache_cannot_run_past_a_new_interrupt_request() {
    for checked in [false, true] {
        let mut time = near_vblank();
        let mut memory = PatternMemory::default();
        let mut bus = time.pattern_bus(&mut memory, &NoAssets);
        bus.check_each_instruction = checked;
        let instruction = PatternInstruction {
            tcycles: 8,
            access: None,
        };
        PatternInstructionBus::execute(&mut bus, instruction).unwrap();
        let before = bus.time.elapsed();
        assert_eq!(
            PatternInstructionBus::execute(&mut bus, instruction),
            Err(DisplayWorkError::InterruptPending)
        );
        assert_eq!(bus.time.elapsed(), before);
    }
}

#[test]
fn disable_lcd_yields_after_restoring_ie_before_its_foreground_ret() {
    let mut time = DisplayWorkTime::default();
    time.begin_native_update();
    time.set_vblank_dispatch_enabled(true);
    let mut child = DisableLcdWork::default();
    loop {
        match child.run(&mut time.display_bus()) {
            Err(DisplayWorkError::InterruptPending) => break,
            Err(error) => panic!("unexpected stop {error:?}"),
            Ok(()) => panic!("DisableLCD RET ran ahead of its pending interrupt"),
        }
    }
    assert!(!time.device.lcd_enabled());
    assert_eq!(time.device.read_port(0xffff).unwrap(), 1);
    assert!(time.pending_vblank_dispatch());
    assert!(!child.finished());
    assert_eq!(
        child.instruction(),
        Some(DisplayInstruction {
            tcycles: 16,
            access: None
        })
    );
    let suspended = serde_json::to_value(child).unwrap();
    let before = time.elapsed();
    assert_eq!(
        child.run(&mut time.display_bus()),
        Err(DisplayWorkError::InterruptPending)
    );
    assert_eq!(serde_json::to_value(child).unwrap(), suspended);
    assert_eq!(time.elapsed(), before);
    time.enter_pending_vblank().unwrap();
    assert_eq!(time.elapsed(), before + 20);
    assert!(time.active_vblank());
    assert!(!time.pending_vblank_dispatch());
    // The real epilogue exercises RETI. Body work is intentionally omitted
    // in this instruction-boundary unit test, never a full-IRQ timing claim.
    let mut epilogue = VBlankReturnWork::new(4);
    while epilogue.step(&mut time).unwrap() {}
    assert_eq!(time.interrupt_returns(), 1);
    assert!(!time.active_vblank());
    let after_irq = time.elapsed();
    child.run(&mut time.display_bus()).unwrap();
    assert!(child.finished());
    assert_eq!(time.elapsed(), after_irq + 16);
}

#[test]
fn hardware_entry_budget_failure_preserves_if_and_cannot_receipt_a_return() {
    let mut time = near_vblank();
    time.idle_to_target().unwrap();
    let before = serde_json::to_value(&time).unwrap();
    assert_eq!(time.enter_pending_vblank(), Err(DisplayWorkError::Budget));
    assert_eq!(serde_json::to_value(&time).unwrap(), before);
    time.begin_native_update();
    time.enter_pending_vblank().unwrap();
    time.device.request_interrupt(1);
    assert_eq!(
        time.enter_pending_vblank(),
        Err(DisplayWorkError::InterruptBusy)
    );
    assert!(!time.pending_vblank_dispatch());
    DisplayInstructionBus::execute(&mut time, plain()).unwrap();
    assert_eq!(time.interrupt_returns(), 0);
}

#[test]
fn legacy_projection_does_not_attach_a_dispatcher_implicitly() {
    let mut time = DisplayWorkTime::default();
    time.begin_native_update();
    time.device.request_interrupt(1);
    DisplayInstructionBus::execute(&mut time, plain()).unwrap();
    let mut json = serde_json::to_value(&time).unwrap();
    json.as_object_mut()
        .unwrap()
        .remove("vblank_dispatch_enabled");
    let restored: DisplayWorkTime = serde_json::from_value(json).unwrap();
    assert!(!restored.pending_vblank_dispatch());
    assert_eq!(restored.interrupt_returns(), 0);
}

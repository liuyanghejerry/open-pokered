use super::*;

// The original external-IO path revalidates for admission and both clock
// intervals. Keep it independent of the exclusive bus as a causal oracle.
fn checked<T>(
    time: &mut DisplayWorkTime,
    tcycles: u8,
    at_tcycle: u8,
    access: impl FnOnce() -> T,
) -> Result<T, DisplayWorkError> {
    if !time.active_vblank {
        return Err(DisplayWorkError::NoActiveVBlank);
    }
    if tcycles == 0 || tcycles & 3 != 0 || at_tcycle > tcycles || at_tcycle & 3 != 0 {
        return Err(DisplayWorkError::Device(DisplayDeviceError::InvalidPhase));
    }
    time.reserve()?;
    time.elapse(u64::from(at_tcycle))?;
    let result = access();
    time.elapse(u64::from(tcycles - at_tcycle))?;
    Ok(result)
}

fn same(actual: &DisplayWorkTime, expected: &DisplayWorkTime) {
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    assert_eq!(actual.device(), expected.device());
}

fn irq(lcdc: u8, offset: u64, dma: bool, budget: u64) -> DisplayWorkTime {
    let mut time = DisplayWorkTime::default();
    time.begin_native_update();
    time.device_mut().request_interrupt(1);
    time.enter_pending_vblank().unwrap();
    time.device_mut().write_port(0xff40, lcdc, 0xc3).unwrap();
    time.device_mut().write_port(0xff4b, 7, 0xc3).unwrap();
    time.elapse(offset).unwrap();
    time.set_shadow_oam(&(0..160).map(|i| i as u8).collect::<Vec<_>>())
        .unwrap();
    if dma {
        time.device_mut().write_port(0xff46, 0xc3, 0xc3).unwrap();
    }
    time.deadline = time.elapsed + budget;
    time
}

fn access(registers: &mut [u8; 16], index: usize) -> u8 {
    let slot = index % registers.len();
    let previous = registers[slot];
    if index % 3 != 0 {
        registers[slot] = previous.wrapping_add(index as u8 + 1);
    }
    previous
}

#[test]
fn external_irq_bus_matches_checked_path_across_grants_dma_and_snapshot_resume() {
    let code = [(4, 0), (8, 4), (12, 8), (16, 0), (24, 24), (12, 12)];
    for lcdc in [0x11, 0x91, 0x93, 0xb1] {
        for offset in [0, 3, 4, 79, 80, 251, 65_532, 65_660, 70_220] {
            for dma in [false, true] {
                for budget in [0, 1, 4, 12, 48, 70_224] {
                    let mut time = irq(lcdc, offset, dma, budget);
                    let mut reference = time.clone();
                    let mut actual_io = [0; 16];
                    let mut expected_io = actual_io;
                    let mut index = 0;
                    while index < 72 {
                        let mut denied = false;
                        {
                            let mut bus = time.display_bus();
                            for _ in 0..9 {
                                let (duration, offset) = code[index % code.len()];
                                let actual =
                                    bus.execute_external_irq_access(duration, offset, || {
                                        access(&mut actual_io, index)
                                    });
                                let expected = checked(&mut reference, duration, offset, || {
                                    access(&mut expected_io, index)
                                });
                                assert_eq!(actual, expected);
                                assert_eq!(actual_io, expected_io);
                                same(bus.time, &reference);
                                if actual == Err(DisplayWorkError::Budget) {
                                    denied = true;
                                    break;
                                }
                                actual.unwrap();
                                index += 1;
                                if index == 72 {
                                    break;
                                }
                            }
                        }
                        if denied {
                            time.begin_native_update();
                            reference.begin_native_update();
                        }
                        time =
                            serde_json::from_value(serde_json::to_value(&time).unwrap()).unwrap();
                        reference =
                            serde_json::from_value(serde_json::to_value(&reference).unwrap())
                                .unwrap();
                    }
                    assert_eq!(time.interrupt_returns, 0);
                    assert!(time.active_vblank());
                }
            }
        }
    }
}

#[test]
fn rejected_external_irq_instructions_preserve_error_order_and_have_no_io() {
    for active in [false, true] {
        for budget in [0, 4] {
            for (duration, offset) in [(0, 0), (2, 0), (12, 2), (12, 16), (12, 8)] {
                let mut base = irq(0x91, 80, false, budget);
                base.active_vblank = active;
                for mutation in 0..6 {
                    let mut value = serde_json::to_value(&base).unwrap();
                    match mutation {
                        1 => value["device"]["ly"] = 154.into(),
                        2 => value["device"]["oam"] = serde_json::json!([]),
                        3 => value["shadow_oam"] = serde_json::json!([]),
                        4 => value["device"]["observed_tcycles"] = 1.into(),
                        5 => value["device"]["stat_enable"] = 8.into(),
                        _ => (),
                    }
                    let mut actual: DisplayWorkTime = serde_json::from_value(value).unwrap();
                    let mut reference = actual.clone();
                    let mut actual_calls = 0;
                    let mut expected_calls = 0;
                    let result = actual.execute_external_irq_access(duration, offset, || {
                        actual_calls += 1;
                    });
                    let expected = checked(&mut reference, duration, offset, || {
                        expected_calls += 1;
                    });
                    assert_eq!(result, expected);
                    assert_eq!(actual_calls, expected_calls);
                    if result.is_err() {
                        assert_eq!(actual_calls, 0);
                    }
                    same(&actual, &reference);
                }
            }
        }
    }
}

#[test]
fn external_irq_bus_revalidates_after_device_writes_and_snapshot_replacement() {
    let mut time = irq(0x91, 70, false, 70_224);
    let mut reference = time.clone();
    {
        let mut bus = time.display_bus();
        for (port, value) in [(0xff43, 7), (0xff46, 0xc3), (0xff40, 0x11), (0xff40, 0xb1)] {
            assert_eq!(
                bus.execute_external_irq_access(12, 8, || 9),
                checked(&mut reference, 12, 8, || 9)
            );
            let write = DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port,
                    value,
                    at_tcycle: 8,
                }),
            };
            assert_eq!(bus.execute(write), reference.execute(write));
            assert!(!bus.validated);
            assert_eq!(
                bus.execute_external_irq_access(16, 4, || 7),
                checked(&mut reference, 16, 4, || 7)
            );
            same(bus.time, &reference);
        }
    }
    let mut value = serde_json::to_value(&time).unwrap();
    value["device"]["ly"] = 154.into();
    let mut restored: DisplayWorkTime = serde_json::from_value(value).unwrap();
    assert_eq!(
        restored
            .display_bus()
            .execute_external_irq_access(4, 0, || panic!("invalid device reached IO")),
        Err(DisplayWorkError::Device(DisplayDeviceError::InvalidPhase))
    );
}

#[test]
fn interval_failure_drops_the_exclusive_validation_receipt() {
    let mut time = irq(0x91, 0, false, 70_224);
    time.shadow_oam.clear();
    let mut reference = time.clone();
    let mut bus = time.display_bus();
    for _ in 0..2 {
        assert_eq!(
            bus.execute_external_irq_access(12, 8, || panic!("invalid shadow reached IO")),
            checked(&mut reference, 12, 8, || panic!(
                "invalid shadow reached IO"
            ))
        );
        assert!(!bus.validated);
        same(bus.time, &reference);
    }
}

#[test]
fn overflow_preserves_the_same_prefix_and_external_side_effects() {
    for (remaining, offset) in [(4, 8), (8, 8), (8, 4)] {
        let mut value = serde_json::to_value(irq(0x11, 0, false, 4)).unwrap();
        let elapsed = u64::MAX - remaining;
        value["elapsed"] = elapsed.into();
        value["deadline"] = u64::MAX.into();
        value["device"]["observed_tcycles"] = elapsed.into();
        let mut time: DisplayWorkTime = serde_json::from_value(value).unwrap();
        let mut reference = time.clone();
        let mut actual_calls = 0;
        let mut expected_calls = 0;
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            time.display_bus()
                .execute_external_irq_access(12, offset, || actual_calls += 1)
        }))
        .is_err());
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            checked(&mut reference, 12, offset, || expected_calls += 1)
        }))
        .is_err());
        assert_eq!(actual_calls, expected_calls);
        assert_eq!(actual_calls, usize::from(remaining >= u64::from(offset)));
        same(&time, &reference);
    }
}

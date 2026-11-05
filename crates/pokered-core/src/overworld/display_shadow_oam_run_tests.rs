use super::*;

#[test]
fn shadow_oam_runs_keep_dma_lcd_io_budgets_and_snapshot_resumption_exact() {
    let mut code = Vec::new();
    for index in 0..160 {
        code.push(DisplayInstruction { tcycles: 12, access: None });
        code.push(DisplayInstruction { tcycles: 8, access: Some(DisplayAccess::Write {
            port: 0xc300 + index, value: (index as u8).wrapping_mul(13), at_tcycle: 4,
        }) });
        if index % 31 == 0 {
            code.push(DisplayInstruction { tcycles: 12, access: Some(DisplayAccess::Read { port: 0xff44, at_tcycle: 8 }) });
        }
    }
    code.push(DisplayInstruction { tcycles: 12, access: Some(DisplayAccess::Write { port: 0xff47, value: 0xe4, at_tcycle: 8 }) });
    for lcdc in [0, 0x93, 0xb3] {
        for offset in [0, 73, 80, 251, 455, 65660, 70220] {
            for dma in [false, true] {
                for budget in (0..512).step_by(17).chain([1, 4, 8, 12, 19, 20, 21, 65535]) {
                    let mut time = DisplayWorkTime::default();
                    time.device.write_port(0xff40, lcdc, 0xc3).unwrap();
                    time.device.write_port(0xff43, 7, 0xc3).unwrap();
                    time.device.write_port(0xff4a, 2, 0xc3).unwrap();
                    time.set_shadow_oam(&[0x28; 160]).unwrap();
                    time.elapse(offset).unwrap();
                    time.active_vblank = true;
                    if dma { time.device.write_port(0xff46, 0xc3, 0xc3).unwrap(); }
                    time.deadline = time.elapsed + budget;
                    let mut reference = time.clone();
                    let (mut next, mut expected_next) = (0, 0);
                    for _ in 0..2 {
                        let mut actual_reads = Vec::new();
                        let actual = (|| {
                            while next < code.len() {
                                let count = time.display_bus().execute_shadow_oam_run(code[next..].iter().copied())?;
                                if count != 0 { next += count; continue; }
                                if let Some(value) = time.display_bus().execute(code[next])? { actual_reads.push(value); }
                                next += 1;
                            }
                            Ok::<_, DisplayWorkError>(())
                        })();
                        let mut expected_reads = Vec::new();
                        let expected = (|| {
                            while expected_next < code.len() {
                                if let Some(value) = reference.execute(code[expected_next])? { expected_reads.push(value); }
                                expected_next += 1;
                            }
                            Ok::<_, DisplayWorkError>(())
                        })();
                        assert_eq!(actual, expected, "lcdc={lcdc} offset={offset} dma={dma} budget={budget}");
                        assert_eq!(next, expected_next);
                        assert_eq!(actual_reads, expected_reads);
                        assert_eq!(serde_json::to_value(&time).unwrap(), serde_json::to_value(&reference).unwrap());
                        if actual.is_ok() { break; }
                        assert_eq!(actual, Err(DisplayWorkError::Budget));
                        time = serde_json::from_value(serde_json::to_value(&time).unwrap()).unwrap();
                        reference = serde_json::from_value(serde_json::to_value(&reference).unwrap()).unwrap();
                        time.begin_native_update(); reference.begin_native_update();
                    }
                    assert_eq!(next, code.len());
                }
            }
        }
    }
}

#[test]
fn shadow_oam_batch_refuses_foreground_dma_and_malformed_ram_without_effects() {
    let code = [DisplayInstruction { tcycles: 8, access: Some(DisplayAccess::Write { port: 0xc300, value: 99, at_tcycle: 4 }) }];
    for kind in 0..3 {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        time.active_vblank = kind != 0;
        if kind == 1 { time.device.write_port(0xff46, 0xc3, 0xc3).unwrap(); }
        if kind == 2 { time.shadow_oam.pop(); }
        let before = serde_json::to_value(&time).unwrap();
        assert_eq!(time.display_bus().execute_shadow_oam_run(code.into_iter()), Ok(0));
        assert_eq!(serde_json::to_value(&time).unwrap(), before);
    }
}

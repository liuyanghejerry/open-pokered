//! Logical HRAM request ports shared by the foreground caller and VBlank.
//! Old/missing bytes stay unknown. An idle size does not invent source or
//! destination addresses. Resource identity comes from the native producer,
//! because distinct raw sprites can alias the same ROM address.
use super::field_restore::{CopyFormat, PatternCopy, PatternSource};
use super::video_copy_caller_work::{AUTO_BG_ENABLED, AUTO_BG_PORTION, COPY_SIZE, DOUBLE_SIZE};
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VideoRequestPorts {
    auto_bg: Option<u8>,
    #[serde(default)]
    auto_bg_portion: Option<u8>,
    copy: [Option<u8>; 5],
    double: [Option<u8>; 5],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VideoCopyRegisters {
    pub size: Option<u8>,
    pub source: Option<u16>,
    pub destination: Option<u16>,
}
impl VideoRequestPorts {
    pub fn contains(port: u16) -> bool {
        matches!(port, AUTO_BG_ENABLED | AUTO_BG_PORTION) || (COPY_SIZE..DOUBLE_SIZE + 5).contains(&port)
    }
    pub fn read(&self, port: u16) -> Option<u8> {
        if port == AUTO_BG_ENABLED {
            self.auto_bg
        } else if port == AUTO_BG_PORTION {
            self.auto_bg_portion
        } else if (COPY_SIZE..DOUBLE_SIZE).contains(&port) {
            self.copy[usize::from(port - COPY_SIZE)]
        } else if (DOUBLE_SIZE..DOUBLE_SIZE + 5).contains(&port) {
            self.double[usize::from(port - DOUBLE_SIZE)]
        } else {
            None
        }
    }
    pub fn write(&mut self, port: u16, value: u8) {
        assert!(Self::contains(port));
        if port == AUTO_BG_ENABLED {
            self.auto_bg = Some(value)
        } else if port == AUTO_BG_PORTION {
            self.auto_bg_portion = Some(value)
        } else if port < DOUBLE_SIZE {
            self.copy[usize::from(port - COPY_SIZE)] = Some(value)
        } else {
            self.double[usize::from(port - DOUBLE_SIZE)] = Some(value)
        }
    }
    pub fn snapshot(&self, format: CopyFormat) -> VideoCopyRegisters {
        let bytes = if format == CopyFormat::TwoBpp {
            self.copy
        } else {
            self.double
        };
        let word = |lo: Option<u8>, hi: Option<u8>| Some(u16::from_le_bytes([lo?, hi?]));
        VideoCopyRegisters {
            size: bytes[0],
            source: word(bytes[1], bytes[2]),
            destination: word(bytes[3], bytes[4]),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PatternRomBinding {
    pub source: PatternSource,
    pub address: u16,
    pub bytes: u16,
    pub bank: u8,
    pub format: CopyFormat,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoBindingError {
    ActiveRequest,
    UnknownPorts,
    WrongBank,
    WrongSource,
    SourceRange,
    DestinationRange,
    WrongFormat,
}
impl PatternRomBinding {
    /// Metadata of the pinned retail Red resources, not raster font data.
    pub fn for_source(source: PatternSource) -> Option<Self> {
        use PatternSource::*;
        let (bank, address, bytes, format) = match source {
            Sprite(raw) => {
                let m = super::sprite_sheet_metadata::sprite_sheet_metadata(raw)?;
                let still = raw >= 0x3d;
                if m.bytes != if still { 64 } else { 192 } {
                    return None;
                }
                (
                    m.bank,
                    m.address,
                    if still { 64 } else { 384 },
                    CopyFormat::TwoBpp,
                )
            }
            WalkingPlayer => (5, 0x4180, 384, CopyFormat::TwoBpp),
            BicyclePlayer => (5, 0x4000, 384, CopyFormat::TwoBpp),
            SurfingPlayer => (5, 0x76c0, 384, CopyFormat::TwoBpp),
            Font => (4, 0x5a80, 1024, CopyFormat::OneBppDouble),
            TextBox => (4, 0x6288, 512, CopyFormat::TwoBpp),
            Tileset(raw) => {
                let metadata = super::tileset_pattern_metadata::tileset_pattern_metadata(raw)?;
                (metadata.bank, metadata.address, 1536, CopyFormat::TwoBpp)
            }
            Flower(frame @ 1..=3) => (
                0,
                0x1f19 + u16::from(frame - 1) * 16,
                16,
                CopyFormat::TwoBpp,
            ),
            Flower(_) => return None,
        };
        Some(Self {
            source,
            address,
            bytes,
            bank,
            format,
        })
    }
    /// Bind the producer's symbolic request to its real B/DE/HL inputs.
    pub fn caller(
        &self,
        copy: PatternCopy,
    ) -> Result<super::video_copy_caller_work::VideoCopyCallerWork, VideoBindingError> {
        if copy.source != self.source {
            return Err(VideoBindingError::WrongSource);
        }
        if copy.format != self.format {
            return Err(VideoBindingError::WrongFormat);
        }
        self.validate_range(copy.source_offset, copy.tiles, copy.destination)?;
        Ok(super::video_copy_caller_work::VideoCopyCallerWork::new(
            self.bank,
            copy.tiles,
            self.address
                .checked_add(copy.source_offset)
                .ok_or(VideoBindingError::SourceRange)?,
            copy.destination,
            copy.format,
        ))
    }
    /// Read the actual request written on the shared bus. No size clamping,
    /// actor-ID reverse lookup, or fixture substitution is permitted here.
    pub fn request(
        &self,
        ports: &VideoRequestPorts,
        bank: u8,
    ) -> Result<PatternCopy, VideoBindingError> {
        if bank != self.bank {
            return Err(VideoBindingError::WrongBank);
        }
        let r = ports.snapshot(self.format);
        let (size, source, destination) = (
            r.size.ok_or(VideoBindingError::UnknownPorts)?,
            r.source.ok_or(VideoBindingError::UnknownPorts)?,
            r.destination.ok_or(VideoBindingError::UnknownPorts)?,
        );
        let offset = source
            .checked_sub(self.address)
            .ok_or(VideoBindingError::SourceRange)?;
        self.validate_range(offset, size, destination)?;
        Ok(PatternCopy {
            source: self.source,
            source_offset: offset,
            destination,
            tiles: size,
            format: self.format,
        })
    }
    fn validate_range(&self, offset: u16, tiles: u8, dest: u16) -> Result<(), VideoBindingError> {
        let len = u16::from(tiles) * self.format.source_bytes_per_tile();
        if offset.checked_add(len).is_none_or(|end| end > self.bytes) {
            return Err(VideoBindingError::SourceRange);
        }
        let written = u16::from(tiles) * 16;
        if dest < 0x8000
            || dest > 0xa000
            || dest.checked_add(written).is_none_or(|end| end > 0xa000)
        {
            return Err(VideoBindingError::DestinationRange);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        auto_bg_transfer_work::AutoBgTransferWork,
        bg_redraw_work::BgRedrawWork,
        display_delay_work::{DelayFrameBus, DelayProgress},
        display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus},
        display_work_time::{DisplayWorkError, DisplayWorkTime},
        far_pattern_copy_work::{LOADED_ROM_BANK, ROM_BANK_SELECT},
        field_restore::{FieldRestoreWork, PatternAssets, PatternMemory, RestoreOperation},
        moving_bg_work::{MovingBgState, MovingBgWork},
        prepare_oam_work::PrepareOamWork,
        queued_bg_copy_work::{BgCopyRequest, QueuedBgCopyWork},
        vblank_pattern_work::VBlankPatternWork,
        vblank_prologue_work::VBlankVideoShadows,
        vblank_return_work::VBlankReturnWork,
        vblank_video_work::{VBlankVideoInputs, VBlankVideoWork},
    };
    use super::*;
    use crate::alloc_prelude::*;
    fn write(t: &mut DisplayWorkTime, port: u16, value: u8) {
        t.execute(DisplayInstruction {
            tcycles: 12,
            access: Some(DisplayAccess::Write {
                port,
                value,
                at_tcycle: 8,
            }),
        })
        .unwrap();
    }
    fn read(t: &mut DisplayWorkTime, port: u16) -> Result<Option<u8>, DisplayWorkError> {
        t.execute(DisplayInstruction {
            tcycles: 12,
            access: Some(DisplayAccess::Read { port, at_tcycle: 8 }),
        })
    }
    #[test]
    fn old_and_partial_snapshots_never_turn_unknown_request_bytes_into_zero() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        let start = t.elapsed();
        assert!(read(&mut t, AUTO_BG_ENABLED).is_err());
        assert_eq!(t.elapsed(), start);
        write(&mut t, AUTO_BG_ENABLED, 0xa5);
        write(&mut t, COPY_SIZE + 1, 0x34);
        assert_eq!(
            t.video_requests().snapshot(CopyFormat::TwoBpp),
            VideoCopyRegisters {
                size: None,
                source: None,
                destination: None
            }
        );
        let raw = serde_json::to_value(&t).unwrap();
        let mut restored: DisplayWorkTime = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(read(&mut restored, AUTO_BG_ENABLED).unwrap(), Some(0xa5));
        assert_eq!(read(&mut restored, COPY_SIZE + 1).unwrap(), Some(0x34));
        assert!(read(&mut restored, COPY_SIZE + 2).is_err());
        let mut old = raw;
        old.as_object_mut().unwrap().remove("video_requests");
        let mut old: DisplayWorkTime = serde_json::from_value(old).unwrap();
        assert!(read(&mut old, AUTO_BG_ENABLED).is_err());
        assert!(read(&mut old, COPY_SIZE + 1).is_err());
        write(&mut t, COPY_SIZE + 2, 0x12);
        assert_eq!(
            t.video_requests().snapshot(CopyFormat::TwoBpp).source,
            Some(0x1234)
        );
        while t.remaining() > 0 {
            t.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        let before = t.video_requests().clone();
        let elapsed = t.elapsed();
        assert_eq!(
            t.execute(DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port: COPY_SIZE,
                    value: 8,
                    at_tcycle: 8
                })
            }),
            Err(DisplayWorkError::Budget)
        );
        assert_eq!(t.video_requests(), &before);
        assert_eq!(t.elapsed(), elapsed);
    }
    #[derive(serde::Deserialize)]
    struct Case {
        bank: u8,
        tiles: u8,
        source: u16,
        destination: u16,
        double: bool,
        bg_before: u8,
        loaded_before: u8,
    }
    fn normal_copies() -> Vec<PatternCopy> {
        let screen = super::super::screen::OverworldScreen::new(
            pokered_data::maps::MapId::RockTunnel1F,
            None,
            pokered_data::impl_traits::PokemonRedData,
        );
        let mut plan = FieldRestoreWork::new(screen.field_restore_input(1));
        let mut out = Vec::new();
        while let Some(op) = plan.operation() {
            if let RestoreOperation::VideoPatternCopy(copy) = op {
                out.push(*copy);
            }
            plan.child_returned();
        }
        out
    }
    struct Bytes;
    impl PatternAssets for Bytes {
        fn read_pattern_byte(&self, _: PatternSource, offset: u16) -> Option<u8> {
            Some((offset % 251 + 1) as u8)
        }
    }
    // Controlled video inputs, deliberately distinct from an ordinary Game
    // producer: real caller ports, explicit inactive BG queues/animation and
    // disabled sprites. This tests the complete video prefix, not the rest ISR.
    fn controlled_video_inputs(
        t: &DisplayWorkTime,
        binding: PatternRomBinding,
    ) -> VBlankVideoInputs {
        let active = VBlankPatternWork::from_live_request(t, binding).unwrap();
        let other = if binding.format == CopyFormat::TwoBpp {
            CopyFormat::OneBppDouble
        } else {
            CopyFormat::TwoBpp
        };
        let idle = VBlankPatternWork::idle_from_live(t, other).unwrap();
        let (pattern, double) = if binding.format == CopyFormat::TwoBpp {
            (active, idle)
        } else {
            (idle, active)
        };
        VBlankVideoInputs {
            shadows: VBlankVideoShadows {
                scx: 0,
                scy: 0,
                wy: 144,
                disable_wy: 0,
            },
            auto_bg: AutoBgTransferWork::new(
                t.video_requests().read(AUTO_BG_ENABLED).unwrap(),
                0,
                0x9c00,
                0xdfff,
                (0..360).map(|x| x as u8).collect(),
            )
            .unwrap(),
            queued: QueuedBgCopyWork::new(
                BgCopyRequest {
                    source: 0,
                    destination: 0,
                    rows: 0,
                },
                0xdfff,
                vec![],
            )
            .unwrap(),
            redraw: BgRedrawWork::new(0, 0, vec![]).unwrap(),
            pattern,
            double,
            moving: MovingBgWork::new(
                MovingBgState {
                    animations: 0,
                    counter1: 0,
                    counter2: 0,
                },
                [[0x55; 16]; 3],
            ),
            prepare: PrepareOamWork::control_only(t.sprite_updates_enabled().unwrap()).unwrap(),
        }
    }
    fn controlled_video_prefix(t: &DisplayWorkTime, binding: PatternRomBinding) -> VBlankVideoWork {
        VBlankVideoWork::select(
            true,
            t.device().pending_interrupts(),
            controlled_video_inputs(t, binding),
        )
        .unwrap()
    }
    #[test]
    fn normal_native_requests_match_rom_registers_through_full_video_prefix_and_actual_reti() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("video_copy_callers_1162.json")).unwrap();
        let copies = normal_copies();
        assert_eq!(copies.len(), cases.len());
        let mut total_waits = 0;
        let mut budget_splits = 0;
        for (copy, c) in copies.into_iter().zip(cases) {
            let binding = PatternRomBinding::for_source(copy.source).unwrap();
            assert_eq!(
                (
                    binding.bank,
                    copy.tiles,
                    binding.address + copy.source_offset,
                    copy.destination,
                    copy.format == CopyFormat::OneBppDouble
                ),
                (c.bank, c.tiles, c.source, c.destination, c.double)
            );
            let mut caller = binding.caller(copy).unwrap();
            let mut t = DisplayWorkTime::default();
            t.begin_native_update();
            write(&mut t, AUTO_BG_ENABLED, c.bg_before);
            write(&mut t, LOADED_ROM_BANK, c.loaded_before);
            write(&mut t, ROM_BANK_SELECT, c.loaded_before);
            write(
                &mut t,
                super::super::sprite_update_control_work::SPRITE_UPDATES_PORT,
                0,
            );
            write(&mut t, if c.double { COPY_SIZE } else { DOUBLE_SIZE }, 0);
            let mut memory = PatternMemory::default();
            let mut waits = 0;
            while !caller.finished() {
                let progress = match caller.step(&mut t) {
                    Ok(p) => p,
                    Err(DisplayWorkError::Budget) => {
                        t.begin_native_update();
                        continue;
                    }
                    Err(e) => panic!("{e:?}"),
                };
                if progress != DelayProgress::Sleeping {
                    continue;
                }
                waits += 1;
                total_waits += 1;
                assert_eq!(t.video_requests().read(AUTO_BG_ENABLED), Some(0));
                let request = binding
                    .request(t.video_requests(), t.source_loaded_rom_bank())
                    .unwrap();
                assert_eq!(
                    request.tiles,
                    copy.tiles.saturating_sub((waits - 1) * 8).min(8)
                );
                while t.device().pending_interrupts() & 1 == 0 {
                    if t.remaining() == 0 {
                        t.begin_native_update();
                    }
                    t.idle_to_device_event().unwrap();
                }
                if t.remaining() == 0 {
                    t.begin_native_update();
                }
                let before_returns = t.interrupt_returns();
                let mut video = controlled_video_prefix(&t, binding);
                if waits == 1 {
                    let mut legacy = super::super::field_restore::VideoCopyWork::new(copy);
                    let epoch = t.device().vblank_epoch();
                    legacy.arm(epoch - 1).unwrap();
                    assert_eq!(legacy.pending_vblank_copy(epoch).unwrap(), request);
                    let mut legacy = Some(legacy);
                    let before = serde_json::to_value(&video).unwrap();
                    assert_eq!(
                        video.attach_foreground_copy(epoch, &mut legacy),
                        Err(super::super::field_restore::RestoreError::ConflictingRequest)
                    );
                    assert!(legacy.is_some());
                    assert_eq!(serde_json::to_value(&video).unwrap(), before);
                }
                let before = memory.clone();
                let mut saw_clear_before_pixels = request.tiles == 0;
                let mut saved = false;
                loop {
                    match video.step(&mut t, &mut memory, &Bytes) {
                        Ok(false) => break,
                        Ok(true) => (),
                        Err(DisplayWorkError::Budget) => {
                            budget_splits += 1;
                            assert!(t.active_vblank());
                            assert_eq!(caller.step(&mut t).unwrap(), DelayProgress::Sleeping);
                            let state =
                                serde_json::to_vec(&(&caller, &t, &memory, &video)).unwrap();
                            (caller, t, memory, video) = serde_json::from_slice(&state).unwrap();
                            t.begin_native_update();
                            continue;
                        }
                        Err(error) => panic!("{error:?}"),
                    }
                    if t.video_requests().snapshot(copy.format).size == Some(0) && memory == before
                    {
                        saw_clear_before_pixels = true;
                        if !saved {
                            assert_eq!(caller.step(&mut t).unwrap(), DelayProgress::Sleeping);
                            let bytes =
                                serde_json::to_vec(&(&caller, &t, &memory, &video)).unwrap();
                            (caller, t, memory, video) = serde_json::from_slice(&bytes).unwrap();
                            saved = true;
                        }
                    }
                }
                assert!(video.body_ready());
                assert!(t.active_vblank());
                let idle = if c.double {
                    video.pattern_work()
                } else {
                    video.double_work()
                };
                assert_eq!(idle.request(), None);
                assert_eq!(idle.source_offset(), None);
                assert_eq!(idle.destination(), None);
                assert!(saw_clear_before_pixels);
                let returned = t.video_requests().snapshot(copy.format);
                assert_eq!(returned.size, Some(0));
                assert_eq!(
                    returned.source,
                    Some(
                        binding.address
                            + request.source_offset
                            + u16::from(request.tiles) * copy.format.source_bytes_per_tile()
                    )
                );
                assert_eq!(
                    returned.destination,
                    Some(request.destination + u16::from(request.tiles) * 16)
                );
                assert_eq!(t.interrupt_returns(), before_returns);
                assert_eq!(caller.step(&mut t).unwrap(), DelayProgress::Sleeping);
                // Entire video prefix returned, but Random/audio/timer/input
                // are omitted here. Only the actual epilogue RETI wakes the
                // caller; never claim complete ISR/Game timing from this seam.
                if t.remaining() == 0 {
                    t.begin_native_update();
                }
                write(
                    &mut t,
                    super::super::display_delay_work::VBLANK_FLAG_PORT,
                    0,
                );
                assert_eq!(caller.step(&mut t).unwrap(), DelayProgress::Sleeping);
                let mut tail = VBlankReturnWork::new(video.saved_bank().unwrap());
                loop {
                    match tail.step(&mut t) {
                        Ok(false) => break,
                        Ok(true) => (),
                        Err(DisplayWorkError::Budget) => t.begin_native_update(),
                        Err(error) => panic!("{error:?}"),
                    }
                }
                assert_eq!(t.interrupt_returns(), before_returns + 1);
                assert_eq!(t.video_requests().read(AUTO_BG_ENABLED), Some(0));
            }
            assert_eq!(waits, copy.tiles / 8 + 1);
            assert_eq!(t.video_requests().read(AUTO_BG_ENABLED), Some(c.bg_before));
            assert_eq!(t.source_loaded_rom_bank(), c.loaded_before);
            for n in 0..u16::from(copy.tiles) * 16 {
                let offset = copy.source_offset + if c.double { n / 2 } else { n };
                assert_eq!(
                    memory.read(copy.destination + n),
                    Some((offset % 251 + 1) as u8)
                );
            }
        }
        assert_eq!(total_waits, 36);
        assert!(
            budget_splits > 0,
            "full video prefix must retain its real cross-grant IRQ work"
        );
    }
    #[test]
    fn bindings_preserve_alias_identity_and_reject_missing_wrong_or_out_of_range_requests() {
        let a = PatternRomBinding::for_source(PatternSource::Sprite(31)).unwrap();
        let b = PatternRomBinding::for_source(PatternSource::Sprite(32)).unwrap();
        assert_eq!((a.bank, a.address), (b.bank, b.address));
        assert_ne!(a.source, b.source);
        assert!(PatternRomBinding::for_source(PatternSource::Sprite(0)).is_none());
        assert!(PatternRomBinding::for_source(PatternSource::Flower(4)).is_none());
        let ports = VideoRequestPorts::default();
        assert_eq!(
            a.request(&ports, a.bank),
            Err(VideoBindingError::UnknownPorts)
        );
        assert_eq!(
            a.request(&ports, a.bank + 1),
            Err(VideoBindingError::WrongBank)
        );
        let mut ports = ports;
        for (i, v) in [8, a.address as u8, (a.address >> 8) as u8, 0, 0x80]
            .into_iter()
            .enumerate()
        {
            ports.write(COPY_SIZE + i as u16, v);
        }
        assert_eq!(a.request(&ports, a.bank).unwrap().source, a.source);
        assert_eq!(b.request(&ports, b.bank).unwrap().source, b.source);
        ports.write(COPY_SIZE + 2, 0);
        assert_eq!(
            a.request(&ports, a.bank),
            Err(VideoBindingError::SourceRange)
        );
        let invalid = PatternCopy {
            source: a.source,
            source_offset: a.bytes,
            destination: 0x8000,
            tiles: 1,
            format: a.format,
        };
        assert_eq!(a.caller(invalid), Err(VideoBindingError::SourceRange));
    }
    #[test]
    fn every_binding_matches_original_rom_pointer_table_and_named_resource_symbols() {
        let expected: Vec<PatternRomBinding> =
            serde_json::from_str(include_str!("video_binding_metadata_1163.json")).unwrap();
        assert_eq!(expected.len(), 80);
        for binding in expected {
            assert_eq!(PatternRomBinding::for_source(binding.source), Some(binding));
        }
    }
    #[test]
    fn known_idle_reads_only_size_and_never_invents_resource_or_pointer_values() {
        for format in [CopyFormat::TwoBpp, CopyFormat::OneBppDouble] {
            let mut t = DisplayWorkTime::default();
            t.begin_native_update();
            assert_eq!(
                VBlankPatternWork::idle_from_live(&t, format),
                Err(VideoBindingError::UnknownPorts)
            );
            let port = if format == CopyFormat::TwoBpp {
                COPY_SIZE
            } else {
                DOUBLE_SIZE
            };
            write(&mut t, port, 1);
            assert_eq!(
                VBlankPatternWork::idle_from_live(&t, format),
                Err(VideoBindingError::ActiveRequest)
            );
            write(&mut t, port, 0);
            let mut idle = VBlankPatternWork::idle_from_live(&t, format).unwrap();
            assert_eq!(idle.request(), None);
            assert_eq!(idle.source_offset(), None);
            assert_eq!(idle.destination(), None);
            let mut memory = PatternMemory::default();
            let before = memory.clone();
            let start = t.elapsed();
            while idle
                .step(&mut t.pattern_bus(&mut memory, &Bytes))
                .unwrap()
                .is_some()
            {}
            assert_eq!(t.elapsed() - start, 36);
            assert_eq!(memory, before);
            assert_eq!(idle.format(), format);
            let registers = t.video_requests().snapshot(format);
            assert_eq!(
                registers,
                VideoCopyRegisters {
                    size: Some(0),
                    source: None,
                    destination: None
                }
            );
        }
    }
    #[test]
    fn a_new_video_owner_rejects_already_started_pattern_inputs() {
        let binding = PatternRomBinding::for_source(PatternSource::WalkingPlayer).unwrap();
        let mut caller = binding
            .caller(PatternCopy {
                source: binding.source,
                source_offset: 0,
                destination: 0x8000,
                tiles: 12,
                format: binding.format,
            })
            .unwrap();
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        write(&mut t, AUTO_BG_ENABLED, 1);
        write(&mut t, DOUBLE_SIZE, 0);
        write(
            &mut t,
            super::super::sprite_update_control_work::SPRITE_UPDATES_PORT,
            0,
        );
        while caller.step(&mut t).unwrap() != DelayProgress::Sleeping {}
        t.idle_to_target().unwrap();
        t.begin_native_update();
        let mut input = controlled_video_inputs(&t, binding);
        let mut memory = PatternMemory::default();
        input
            .pattern
            .step(&mut t.pattern_bus(&mut memory, &Bytes))
            .unwrap();
        assert!(VBlankVideoWork::select(true, t.device().pending_interrupts(), input).is_none());
        assert!(VBlankVideoWork::select(
            true,
            t.device().pending_interrupts(),
            controlled_video_inputs(&t, binding)
        )
        .is_some());
    }
}

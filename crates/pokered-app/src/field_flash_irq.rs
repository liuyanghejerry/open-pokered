//! Native foreground -> bounded common IRQ handoff for FLASH.
//! Synchronous native boot/UI resource state is an explicit projection; it
//! does not supply captured original registers, inactive substitutes, or RETI.
use super::*;
use pokered_core::overworld::{
    auto_bg_transfer_work::AutoBgTransferWork,
    bg_redraw_work::BgRedrawWork,
    display_delay_work::{DelayFramesWork, DelayProgress},
    fade_audio_work::FadeAudioState,
    field_restore::{CopyFormat, PatternAssets, PatternCopy, SpritePatternPlan},
    moving_bg_work::{MovingBgState, MovingBgWork},
    prepare_oam_work::PrepareOamWork,
    queued_bg_copy_work::{BgCopyRequest, QueuedBgCopyWork},
    track_play_time_work::TrackTimeState,
    vblank_irq_work::{IrqError, IrqProgress, VBlankIrqWork},
    vblank_pattern_work::VBlankPatternWork,
    vblank_prologue_work::VBlankVideoShadows,
    vblank_video_work::{VBlankVideoInputs, VBlankVideoWork},
    video_copy_caller_work::{VideoCopyCallerWork, AUTO_BG_ENABLED},
    video_request_ports::PatternRomBinding,
};

struct FieldAudioBus<'a> {
    time: pokered_core::overworld::display_work_time::DisplayBus<'a>,
}

#[cfg(test)]
#[path = "field_flash_audio_bus_tests.rs"]
mod audio_bus_tests;
impl pokered_audio::source_clock::apu_bus::SourceAudioFrameBus for FieldAudioBus<'_> {
    type Error = DisplayWorkError;

    fn execute(
        &mut self,
        apu: &mut pokered_audio::apu::Apu,
        instruction: pokered_audio::source_clock::apu_bus::ApuInstruction,
    ) -> Result<Option<u8>, Self::Error> {
        use pokered_audio::source_clock::apu_bus::ApuAccess;
        let offset = match instruction.access {
            Some(ApuAccess::Read { at_tcycle, .. }) | Some(ApuAccess::Write { at_tcycle, .. }) => {
                at_tcycle
            }
            None => 0,
        };
        self.time
            .execute_external_irq_access(instruction.tcycles, offset, || match instruction.access {
                Some(ApuAccess::Read { address, .. }) => Some(apu.read_register(address)),
                Some(ApuAccess::Write { address, value, .. }) => {
                    apu.write_register(address, value);
                    None
                }
                None => None,
            })
    }

    fn execute_plain_run(
        &mut self,
        _apu: &mut pokered_audio::apu::Apu,
        tcycles: &[u8],
    ) -> Result<usize, Self::Error> {
        use pokered_core::overworld::display_lcd_work::{
            DisplayInstruction, DisplayInstructionBus,
        };
        self.time
            .execute_plain_run(tcycles.iter().map(|&tcycles| DisplayInstruction {
                tcycles,
                access: None,
            }))
    }
}

#[derive(serde::Serialize)]
pub(super) struct FieldIrq {
    pub attached: bool,
    pub work: Option<VBlankIrqWork>,
    #[serde(skip)]
    audio: Option<Box<pokered_audio::source_clock::frame_instructions::SourceAudioFrame>>,
    pub delay: Option<DelayFramesWork>,
    pub copy: Option<VideoCopyCallerWork>,
    pub binding: Option<PatternRomBinding>,
    pub initial_slots: Vec<SpriteSlot>,
    pub map_tiles: crate::render::MapTileSnapshot,
    pub ui: super::super::field_video_inputs::NativeUiTiles,
    pub background_is_map: bool,
    native_grass: Option<[Option<u8>; 16]>,
    pub portion: u8,
    pub shadows: VBlankVideoShadows,
    pub completed: u64,
    flowers: [[u8; 16]; 3],
}
impl FieldIrq {
    pub(super) fn new(game: &mut PokemonGame, flow: &mut FieldFlash) -> Result<Self, String> {
        let map_tiles = crate::render::stationary_map_tile_snapshot(&game.overworld)
            .map_err(|e| format!("native map input: {e:?}"))?;
        let sprite_plan = SpritePatternPlan::new(flow.work.raw_sprite_pictures(), false);
        let native_grass = if flow.origin.returns_to_map_with_actors() {
            Some(
                super::super::field_oam_inputs::stationary_grass_inputs(
                    &game.overworld,
                    &map_tiles.tiles,
                )
                .map_err(|e| format!("native stationary grass: {e:?}"))?,
            )
        } else {
            None
        };
        let resources = game
            .resources
            .as_mut()
            .ok_or("graphics resources unavailable")?;
        let tileset = pokered_data::tilesets::TilesetId::from_name(&map_tiles.tileset)
            .ok_or("unknown native tileset")?;
        let tiles = resources
            .load_tileset(tileset.tileset_name())
            .map_err(|e| format!("tileset input: {e:?}"))?;
        if tiles.tileset.len() > 128 {
            return Err("native tileset exceeds its BG pattern bank".into());
        }
        for byte in 0..tiles.tileset.len() * 16 {
            let value = super::super::field_restore_assets::pattern_byte(
                &tiles.tileset,
                byte as u16,
                false,
            )
            .ok_or("native map pattern unavailable")?;
            flow.patterns
                .initialize_native_byte(0x9000 + byte as u16, value)
                .map_err(|e| format!("{e:?}"))?;
        }
        // The native background's real retained rotation is part of the input.
        let shift = i32::from(game.overworld.tile_anim.water_shift()).rem_euclid(8) as u32;
        for byte in 0..16 {
            let a = 0x9140 + byte;
            let value = flow
                .patterns
                .read(a)
                .ok_or("water pattern unavailable")?
                .rotate_right(shift);
            flow.patterns
                .initialize_native_byte(a, value)
                .map_err(|e| format!("{e:?}"))?;
        }
        let assets = NativePatternAssets::new(resources);
        if flow.origin == RestorationOrigin::Submenu {
            // The native field already owns these stationary patterns. Bind
            // their actual assets before timed ownership; this is not a new
            // reload or a completion of the later textbox request.
            let player = game.overworld.field_restore_input(1).player_source;
            let player_copy = PatternCopy {
                source: player,
                source_offset: 0,
                destination: 0x8000,
                tiles: 12,
                format: CopyFormat::TwoBpp,
            };
            for copy in core::iter::once(&player_copy).chain(
                sprite_plan
                    .copies
                    .iter()
                    .filter(|copy| copy.destination < 0x8800),
            ) {
                for offset in 0..u16::from(copy.tiles) * 16 {
                    let byte = assets
                        .read_pattern_byte(copy.source, copy.source_offset + offset)
                        .ok_or("native standing sprite pattern unavailable")?;
                    flow.patterns
                        .initialize_native_byte(copy.destination + offset, byte)
                        .map_err(|e| format!("{e:?}"))?;
                }
            }
        }
        let mut flowers = [[0; 16]; 3];
        for (frame, bytes) in flowers.iter_mut().enumerate() {
            for (offset, byte) in bytes.iter_mut().enumerate() {
                *byte = assets
                    .read_pattern_byte(
                        pokered_core::overworld::field_restore::PatternSource::Flower(
                            frame as u8 + 1,
                        ),
                        offset as u16,
                    )
                    .ok_or("flower resource unavailable")?;
            }
        }
        if let Some(frame) = game.overworld.tile_anim.flower_frame() {
            for byte in 0..16 {
                flow.patterns
                    .initialize_native_byte(
                        0x9030 + byte as u16,
                        flowers[usize::from(frame - 1)][byte],
                    )
                    .map_err(|e| format!("{e:?}"))?;
            }
        }
        // Font bytes remain transfer data only. The visible UI and its tile
        // namespace below come exclusively from the Fusion Pixel framebuffer.
        for byte in 0..1024u16 {
            let value = assets
                .read_pattern_byte(
                    pokered_core::overworld::field_restore::PatternSource::Font,
                    byte,
                )
                .ok_or("party font transfer data unavailable")?;
            for plane in 0..2 {
                flow.patterns
                    .initialize_native_byte(0x8800 + byte * 2 + plane, value)
                    .map_err(|e| format!("{e:?}"))?;
            }
        }
        // The native text caller still owns presentation until it returns.
        // Its final raster is sampled once at the IRQ handoff, not on every
        // text-reveal frame (which is particularly expensive on GBA).
        let ui = super::super::field_video_inputs::NativeUiTiles::default();
        game.source_display
            .establish_native_video_idle(1)
            .map_err(|e| format!("{e:?}"))?;
        Ok(Self {
            attached: false,
            work: None,
            audio: None,
            delay: None,
            copy: None,
            binding: None,
            initial_slots: sprite_plan.slots,
            map_tiles,
            ui,
            background_is_map: false,
            native_grass,
            portion: game
                .source_display
                .background_portion()
                .ok_or("unknown native BG portion")?,
            shadows: VBlankVideoShadows {
                scx: 0,
                scy: 0,
                wy: 144,
                disable_wy: 0,
            },
            completed: 0,
            flowers,
        })
    }
}
impl PokemonGame {
    pub(super) fn restore_submenu_images(&mut self, flow: &mut FieldFlash) -> Result<(), String> {
        let irq = flow.irq.as_mut().ok_or("missing menu IRQ owner")?;
        let covered = pokered_ui::menus::start::sprite_occlusion_tiles(
            &self.start_menu,
            &self.player_name,
            &pokered_data::ui_layout::schema::START_DEFAULT_LAYOUT,
            self.state.config.language,
        );
        let hides = |x: u8, y: u8| {
            let at = usize::from(y) * 20 + usize::from(x);
            x >= 20 || y >= 18 || covered[at] || irq.map_tiles.tiles[at] >= 0x60
        };
        self.overworld
            .player_presentation
            .restore_menu_image(self.overworld.state.player.facing, hides(8, 9));
        self.overworld
            .npc_presentation
            .restore_menu_images(&self.overworld.npc_states, hides);
        irq.native_grass = Some(
            super::super::field_oam_inputs::stationary_grass_inputs(
                &self.overworld,
                &irq.map_tiles.tiles,
            )
            .map_err(|e| format!("menu grass input: {e:?}"))?,
        );
        Ok(())
    }

    /// Abandoning a caller starts a fresh native projection, never a fake
    /// completion of its interrupted source IRQ. Normal FLASH RET has already
    /// detached and taken its caller, so it does not enter this path.
    pub(in crate::game) fn discard_field_flash(&mut self) {
        if let Some(flow) = self.field_flash.take() {
            if flow.irq.as_ref().is_some_and(|irq| irq.attached) {
                self.source_display = Box::new(pokered_core::overworld::display_work_time::DisplayWorkTime::legacy_native_projection(self.frame_count).into());
            }
            #[cfg(any(test, feature = "debug-server"))]
            {
                self.source_audio_work = None;
            }
        }
    }
    pub(in crate::game) fn field_irq_owns_clock(&self) -> bool {
        self.field_flash
            .as_ref()
            .and_then(|f| f.irq.as_ref())
            .is_some_and(|irq| irq.attached)
    }
    fn sample_field_irq_ui(&self, flow: &mut FieldFlash) -> Result<(), String> {
        let irq = flow
            .irq
            .as_mut()
            .ok_or("FLASH native IRQ inputs unavailable")?;
        if irq.attached || irq.background_is_map {
            return Ok(());
        }
        if let Some(dialogue) = self.overworld.displayed_field_dialogue() {
            if let Some((first, second)) = dialogue.get_display_text() {
                let text = if second.is_empty() {
                    first
                } else {
                    format!("{first}\n{second}")
                };
                let arrow = flow.text.as_ref().is_some_and(|text| !text.returned())
                    && dialogue.waiting_for_input()
                    && (self.frame_count / 16) % 2 == 0;
                let mut painter = pokered_ui::backends::FrameBufferPainter::new(&mut flow.party);
                pokered_ui::menus::dialog::draw_paginated(
                    &text,
                    arrow,
                    &pokered_data::ui_layout::schema::DIALOG_DEFAULT_LAYOUT,
                    &mut pokered_ui::Ui::new(&mut painter),
                    self.state.config.language,
                );
            }
        }
        irq.ui
            .sample(&flow.party)
            .map_err(|error| format!("native UI input: {error:?}"))
    }
    pub(super) fn attach_field_irq(&mut self, flow: &mut FieldFlash) -> Result<(), String> {
        self.sample_field_irq_ui(flow)?;
        let irq = flow
            .irq
            .as_mut()
            .ok_or("FLASH native IRQ inputs unavailable")?;
        if !irq.attached {
            if flow.origin == RestorationOrigin::Submenu {
                // Both native submenu consumers currently show no field
                // objects. Bind that existing visibility at the handoff.
                self.source_display
                    .set_shadow_oam(&[0; 160])
                    .map_err(|e| format!("{e:?}"))?;
                self.source_display
                    .device_mut()
                    .begin_native_object_scanout(&[0; 160], [0xd0, 0xe0])
                    .map_err(|e| format!("{e:?}"))?;
            }
            // The retained native party uses canonical shade indices. Bind
            // that existing image once; subsequent palette changes are actual
            // source stores, published only by completed display scanout.
            self.source_display
                .device_mut()
                .begin_native_background_scanout(0xe4);
            irq.attached = true;
            self.source_display.set_vblank_dispatch_enabled(true);
        }
        Ok(())
    }
    pub(super) fn begin_field_video_copy(
        &mut self,
        flow: &mut FieldFlash,
        copy: PatternCopy,
    ) -> Result<(), String> {
        let binding = PatternRomBinding::for_source(copy.source)
            .ok_or("unsupported video source metadata")?;
        if binding.format != copy.format {
            return Err("video source format mismatch".into());
        }
        let irq = flow.irq.as_mut().ok_or("FLASH IRQ input missing")?;
        if irq.copy.is_some() {
            return Err("nested foreground video caller".into());
        }
        irq.copy = Some(VideoCopyCallerWork::new(
            binding.bank,
            copy.tiles,
            binding
                .address
                .checked_add(copy.source_offset)
                .ok_or("source address overflow")?,
            copy.destination,
            copy.format,
        ));
        irq.binding = Some(binding);
        Ok(())
    }
    /// Service one whole IRQ continuation or yield only at its real budget.
    pub(super) fn service_field_irq(&mut self, flow: &mut FieldFlash) -> Result<bool, String> {
        let irq = flow.irq.as_mut().ok_or("FLASH IRQ inputs unavailable")?;
        if irq.work.is_none() {
            if !self.source_display.pending_vblank_dispatch() {
                return Ok(true);
            }
            let bind = |format| -> Result<VBlankPatternWork, String> {
                match self.source_display.video_requests().snapshot(format).size {
                    Some(0) => VBlankPatternWork::idle_from_live(&*self.source_display, format)
                        .map_err(|e| format!("{e:?}")),
                    Some(_) => {
                        let binding = irq
                            .binding
                            .filter(|b| b.format == format)
                            .ok_or("active video source identity unavailable")?;
                        VBlankPatternWork::from_live_request(&*self.source_display, binding)
                            .map_err(|e| format!("{e:?}"))
                    }
                    None => Err("unknown native video request size".into()),
                }
            };
            let enabled = self
                .source_display
                .sprite_updates_enabled()
                .ok_or("unknown sprite control")?;
            let prepare = if enabled != 1 {
                PrepareOamWork::control_only(enabled)
            } else {
                // Indoor reloads build an assignment prefix while VBlank can
                // still see the previous published images. That prefix is
                // not a complete binding table for the visible actors: keep
                // their entry bindings until all assignments are available.
                let slots = if flow.slots.len() == irq.initial_slots.len() {
                    &flow.slots
                } else {
                    &irq.initial_slots
                };
                let input = super::super::field_oam_inputs::field_oam_input_with_grass(
                    &self.overworld,
                    &*self.source_display,
                    slots,
                    irq.native_grass.as_ref(),
                )
                .map_err(|e| format!("native actor input: {e:?}"))?;
                PrepareOamWork::new(input)
            }
            .map_err(|e| format!("{e:?}"))?;
            let bg_enabled = self
                .source_display
                .video_requests()
                .read(AUTO_BG_ENABLED)
                .ok_or("unknown BG control")?;
            let bg = if irq.background_is_map {
                &irq.map_tiles.tiles
            } else {
                &irq.ui.tiles
            };
            let (counter1, counter2) = self.overworld.tile_anim.counters();
            let animations = if flow.origin == RestorationOrigin::TrainerCard {
                0
            } else {
                match self.overworld.tile_anim.kind() {
                    pokered_core::overworld::presentation::TileAnimKind::None => 0,
                    pokered_core::overworld::presentation::TileAnimKind::Water => 1,
                    pokered_core::overworld::presentation::TileAnimKind::WaterFlower => 3,
                }
            };
            let video = VBlankVideoWork::select(
                true,
                self.source_display.device().pending_interrupts(),
                VBlankVideoInputs {
                    shadows: irq.shadows,
                    // Native logical buffers have no Game Boy CPU stack. Zero is
                    // its symbolic native stack token, not an original SP claim.
                    auto_bg: AutoBgTransferWork::new(
                        bg_enabled,
                        irq.portion,
                        0x9800,
                        0,
                        bg.clone(),
                    )
                    .ok_or("invalid native BG tiles")?
                    .with_live_portion(),
                    queued: QueuedBgCopyWork::new(
                        BgCopyRequest {
                            source: 0,
                            destination: 0,
                            rows: 0,
                        },
                        0,
                        Vec::new(),
                    )
                    .ok_or("invalid idle BG producer")?,
                    redraw: BgRedrawWork::new(0, 0, Vec::new())
                        .ok_or("invalid idle row producer")?,
                    pattern: bind(CopyFormat::TwoBpp)?,
                    double: bind(CopyFormat::OneBppDouble)?,
                    moving: MovingBgWork::new(
                        MovingBgState {
                            animations,
                            counter1,
                            counter2,
                        },
                        irq.flowers,
                    ),
                    prepare,
                },
            )
            .ok_or("VBlank selection refused")?;
            let context = self
                .audio
                .as_ref()
                .ok_or("native audio owner unavailable")?
                .source_irq_context()
                .map_err(|e| format!("{e:?}"))?;
            if self.overworld.native_joy_ignore_mask() != 0 {
                return Err("native ignore countdown not yet bound".into());
            }
            // With the actual native ignore mask zero the countdown helper
            // returns before reading flags5/pressed/held. These opaque values
            // are never applied to the foreground history or claimed known.
            let timer = TrackTimeState {
                ignore_counter: 0,
                flags5: 0,
                flags6: u8::from(self.game_timer_active()),
                joy_pressed: 0,
                joy_held: 0,
                time: self.save_data.game_data.play_time,
            };
            irq.work = Some(VBlankIrqWork::new(
                video,
                FadeAudioState {
                    control: 0,
                    counter: 0,
                    reload: 0,
                    flags2: 0,
                    nr50: context.nr50,
                    audio_bank: context.bank,
                    saved_bank: context.bank,
                    new_sound: 0,
                },
                timer,
            ));
        }
        // Keep the read-through adapter for the whole IRQ service grant.
        // Recreating it per source instruction discards its immutable-ROM
        // cache between successive bytes of the very same tile transfer.
        // Every byte still passes through the original timed instruction bus.
        let resources = self
            .resources
            .as_mut()
            .ok_or("graphics resources unavailable")?;
        let assets = NativePatternAssets::new(resources);
        loop {
            match irq.work.as_mut().unwrap().run_until_boundary(
                &mut *self.source_display,
                &mut flow.patterns,
                &assets,
            ) {
                Ok(IrqProgress::Instruction) => (),
                Ok(IrqProgress::NeedsAudio) => {
                    use pokered_audio::source_clock::{
                        frame_instructions::FrameStepError, AudioAdmissionError,
                    };
                    let audio = self
                        .audio
                        .as_ref()
                        .ok_or("native audio owner unavailable")?;
                    let bank = irq.work.as_ref().unwrap().fade_state().audio_bank;
                    // One exclusive display bus spans the audio continuation;
                    // each instruction still observes its own IO/budget edge.
                    let result = audio.service_source_audio_frame(
                        &mut irq.audio,
                        bank,
                        0,
                        FieldAudioBus {
                            time: self.source_display.display_bus(),
                        },
                    );
                    let work = match result {
                        Ok(work) => work,
                        Err(AudioAdmissionError::Admission(FrameStepError::Clock(
                            DisplayWorkError::Budget,
                        ))) => return Ok(false),
                        Err(error) => return Err(format!("source audio continuation: {error:?}")),
                    };
                    #[cfg(any(test, feature = "debug-server"))]
                    {
                        self.source_audio_work = Some((self.frame_count, Ok(work)));
                    }
                    irq.audio = None;
                    irq.work
                        .as_mut()
                        .unwrap()
                        .audio_returned(work.cycles)
                        .map_err(|e| format!("{e:?}"))?;
                }
                Ok(IrqProgress::Returned) => {
                    let work = irq.work.take().unwrap();
                    irq.portion = work.video().next_bg_portion();
                    let moving = work.video().moving_state();
                    let (old1, old2) = self.overworld.tile_anim.counters();
                    if moving.counter2 != old2 {
                        self.overworld.tile_anim.set_counter1(19);
                        self.overworld.tile_anim.tick();
                    } else if old1 == 20 && moving.counter1 == 0 {
                        self.overworld.tile_anim.set_counter1(20);
                        self.overworld.tile_anim.tick();
                    }
                    self.overworld.tile_anim.set_counter1(moving.counter1);
                    self.overworld.tile_anim.set_counter2(moving.counter2);
                    // Publish a hidden native view only after the real DMA
                    // has made every hardware object off-screen. Logical
                    // current images remain independent for the next prepare.
                    if self
                        .source_display
                        .device()
                        .oam()
                        .chunks_exact(4)
                        .all(|object| object[0] == 0 || object[0] >= 160)
                    {
                        let map = self.overworld.state.current_map;
                        self.overworld.player_presentation.publish_cleared_oam(map);
                        self.overworld.npc_presentation.publish_cleared_oam(map);
                    }
                    self.save_data.game_data.play_time = work.timer_state().time;
                    if let Some(owner) = self.overworld.source_foreground_mut() {
                        owner.read_raw(self.source_display.source_joy_input());
                    }
                    irq.completed += 1;
                    return Ok(true);
                }
                Err(IrqError::Display(DisplayWorkError::Budget)) => return Ok(false),
                Err(error) => return Err(format!("common VBlank: {error:?}")),
            }
        }
    }
    pub(super) fn step_field_wait(&mut self, flow: &mut FieldFlash) -> Result<bool, String> {
        let irq = flow.irq.as_mut().ok_or("FLASH IRQ input missing")?;
        let result = if let Some(delay) = irq.delay.as_mut() {
            delay.step(&mut *self.source_display)
        } else if let Some(copy) = irq.copy.as_mut() {
            copy.step(&mut *self.source_display)
        } else {
            return Ok(true);
        };
        match result {
            Ok(DelayProgress::Instruction) => Ok(false),
            Ok(DelayProgress::Sleeping) => {
                self.source_display
                    .idle_to_vblank_edge()
                    .map_err(|e| format!("{e:?}"))?;
                Ok(false)
            }
            Ok(DelayProgress::Returned) => {
                if irq.delay.take().is_some() {
                    let receipt = flow
                        .waits
                        .last_mut()
                        .ok_or("missing foreground wait observation")?;
                    receipt.completed_returns = Some(pokered_core::overworld::display_delay_work::DelayFrameBus::interrupt_returns(&*self.source_display));
                    receipt.completed_tcycles = Some(self.source_display.elapsed());
                } else {
                    irq.copy = None;
                    irq.binding = None;
                    if let Some(parent) = flow.indoor_transfer.as_mut() {
                        if !parent.copy_returned() {
                            return Err("video caller returned outside transfer boundary".into());
                        }
                    }
                }
                flow.work.child_returned();
                Ok(true)
            }
            Err(DisplayWorkError::InterruptPending) => Ok(false),
            Err(DisplayWorkError::Budget) => Ok(false),
            Err(error) => Err(format!("foreground wait: {error:?}")),
        }
    }
}

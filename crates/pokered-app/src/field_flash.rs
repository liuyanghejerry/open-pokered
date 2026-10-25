//! Shared FLASH caller, retaining the real party parent until PrintText returns.
//! Native resource operations are explicit; no scene-total frame countdown.
//! LCD-off children execute against the shared Game display time/device.
//! VBlank upload/IRQ integration and original/native timing still need validation.
use super::field_restore_assets::NativePatternAssets;
#[path = "field_flash_irq.rs"]
mod irq;
use super::*;
use pokered_core::overworld::display_clear_oam_work::ClearShadowOamWork;
use pokered_core::overworld::display_lcd_work::{DisableLcdWork, EnableLcdWork};
use pokered_core::overworld::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};
use pokered_core::overworld::display_palette_work::DisplayPaletteWork;
use pokered_core::overworld::display_work_time::DisplayWorkError;
use pokered_core::overworld::far_pattern_copy_work::FarPatternCopyCall;
use pokered_core::overworld::field_restore::{
    FieldRestoreWork, PatternMemory, RestoreOperation, SpriteSlot,
};
use pokered_core::overworld::indoor_sprite_clear_work::IndoorSpriteClearWork;
use pokered_core::overworld::indoor_sprite_prefix_work::IndoorSpritePrefixWork;
use pokered_core::overworld::indoor_sprite_slot_work::IndoorSpriteSlotWork;
use pokered_core::overworld::indoor_sprite_transfer_work::IndoorSpriteTransferWork;
use pokered_core::overworld::party_field_text::PartyFieldText;
use pokered_core::overworld::sprite_bank_call_work::SpriteBankCallWork;

// Retained images need only four shades; two GBA linear copies exhaust the heap.
type FlashFrame = dotzuki_renderer::RgbaIndexedFrameBuffer<dotzuki_renderer::palette::GbColor>;

#[cfg(test)]
fn retain_flash_frame<const LINEAR: bool>(
    frame: dotzuki_renderer::RgbaIndexedFrameBuffer<dotzuki_renderer::palette::GbColor, LINEAR>,
) -> FlashFrame {
    let mut retained = FlashFrame::new(RenderConfig::new(160, 144), Rgba::WHITE);
    copy_flash_frame(&frame, &mut retained);
    retained
}

fn copy_flash_frame<const LINEAR: bool>(
    frame: &dotzuki_renderer::RgbaIndexedFrameBuffer<dotzuki_renderer::palette::GbColor, LINEAR>,
    retained: &mut FlashFrame,
) {
    for y in 0..144 {
        for x in 0..160 {
            retained.set_pixel_index(x, y, frame.get_index(x, y).unwrap());
        }
    }
    retained.set_palette(*frame.display_palette());
}

fn draw_flash_frame(frame: &FlashFrame, fb: &mut FrameBuffer) {
    #[cfg(not(all(target_os = "none", target_arch = "arm")))]
    fb.copy_from(frame);
    #[cfg(all(target_os = "none", target_arch = "arm"))]
    unpack_flash_frame(frame, fb);
}

fn apply_flash_palette(raster: &pokered_core::overworld::display_device::BackgroundPaletteRaster, fb: &mut FrameBuffer) {
    for (index, span) in raster.spans().iter().enumerate() {
        if span.palette == 0xe4 { continue; }
        let end = raster.spans().get(index + 1).map_or(160 * 144, |next| next.start);
        let mut pixel = span.start;
        while pixel < end {
            let y = u32::from(pixel / 160);
            let from = u32::from(pixel % 160);
            let until = (end - (pixel / 160) * 160).min(160);
            if span.palette == 0 {
                fb.fill_rect(from, y, u32::from(until) - from, 1, Rgba::WHITE);
            } else {
                for x in from..u32::from(until) {
                    let color = fb.get_index(x, y).unwrap() as u8;
                    fb.set_pixel_index(x, y, dotzuki_renderer::palette::GbColor::from_u8(
                        (span.palette >> (color * 2)) & 3));
                }
            }
            pixel += until - from as u16;
        }
    }
}

// Four monochrome plane bits expanded into four little-endian index bytes.
// Combining the two planes needs just two word writes per eight pixels.
#[cfg(any(test, all(target_os = "none", target_arch = "arm")))]
fn unpack_flash_frame(
    frame: &FlashFrame,
    fb: &mut dotzuki_renderer::LinearRgbaIndexedFrameBuffer<dotzuki_renderer::palette::GbColor>,
) {
    const NIBBLES: [u32; 16] = {
        let mut table = [0; 16];
        let mut n = 0;
        while n < 16 {
            table[n] = ((n as u32 & 8) >> 3)
                | ((n as u32 & 4) << 6)
                | ((n as u32 & 2) << 15)
                | ((n as u32 & 1) << 24);
            n += 1;
        }
        table
    };
    for (planes, out) in frame
        .packed()
        .chunks_exact(2)
        .zip(fb.indices_mut().chunks_exact_mut(8))
    {
        let first = NIBBLES[(planes[0] >> 4) as usize] | (NIBBLES[(planes[1] >> 4) as usize] << 1);
        let second = NIBBLES[(planes[0] & 15) as usize] | (NIBBLES[(planes[1] & 15) as usize] << 1);
        out[..4].copy_from_slice(&first.to_le_bytes());
        out[4..].copy_from_slice(&second.to_le_bytes());
    }
    fb.set_palette(*frame.display_palette());
}

enum DisplayChild {
    Disable(DisableLcdWork),
    Enable { work: EnableLcdWork, called: bool },
    Copy(FarPatternCopyCall),
    Palette(DisplayPaletteWork),
    EnableSprites(pokered_core::overworld::sprite_update_control_work::EnableSpriteUpdatesWork),
    ClearOam(ClearShadowOamWork),
    IndoorSpritePrefix(IndoorSpritePrefixWork),
    IndoorSpriteSlot(IndoorSpriteSlotWork),
    IndoorSpriteTransfer(IndoorSpriteTransferWork),
    IndoorSpriteClear(IndoorSpriteClearWork),
    ColdSpriteBank(SpriteBankCallWork),
}

/// Passive observations of real foreground waits and ClearSprites return.
#[derive(serde::Serialize)]
struct FieldWaitReceipt {
    requested: u8,
    armed_returns: u64,
    armed_tcycles: u64,
    completed_returns: Option<u64>,
    completed_tcycles: Option<u64>,
}

pub(crate) struct FieldFlash {
    text: PartyFieldText,
    work: FieldRestoreWork,
    patterns: PatternMemory,
    waits: Vec<FieldWaitReceipt>,
    cleared_oam_at_ret: Option<(u64, bool)>,
    last_bulk_copy_temp_at_ret: Option<u8>,
    #[cfg(feature = "debug-server")]
    traced_operation: Option<usize>,
    party: FrameBuffer,
    map: FlashFrame,
    map_key: crate::render::session::FlashMapVisualKey,
    show_map: bool,
    display_child: Option<DisplayChild>,
    font_flags: u8,
    sprite_set: u8,
    slots: Vec<SpriteSlot>,
    indoor_pictures: Vec<u8>,
    indoor_slot: Option<IndoorSpriteSlotWork>,
    indoor_transfer: Option<IndoorSpriteTransferWork>,
    cold_sprite_bank: Option<SpriteBankCallWork>,
    facing: Vec<pokered_core::overworld::Direction>,
    error: Option<String>,
    irq: Option<irq::FieldIrq>,
}

impl PokemonGame {
    pub(super) fn begin_field_flash(&mut self, message: &str) {
        // One live staging buffer serves both party text and the final map
        // redraw. Reusing it avoids a late contiguous allocation on GBA.
        let mut party = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        crate::render::draw_overworld_background(
            &mut self.overworld,
            &mut self.resources,
            &mut party,
            self.state.config.language,
        );
        let mut map = FlashFrame::new(RenderConfig::new(160, 144), Rgba::WHITE);
        copy_flash_frame(&party, &mut map);
        party.clear(Rgba::WHITE);
        crate::render::draw_party_screen(
            &self.party_screen,
            self.resources.as_mut(),
            self.frame_count,
            &mut party,
            self.state.config.language,
        );
        // StartMenu_Pokemon entered through DisplayPartyMenu, which has the
        // font loaded. The source plan deliberately performs both reloads.
        let work = FieldRestoreWork::new(self.overworld.field_restore_input(1));
        let facing = self
            .overworld
            .npc_states
            .iter()
            .map(|npc| npc.facing)
            .collect();
        let text = self.overworld.begin_party_field_text(message);
        self.field_flash = Some(Box::new(FieldFlash {
            text,
            work,
            patterns: PatternMemory::default(),
            waits: Vec::new(),
            cleared_oam_at_ret: None,
            last_bulk_copy_temp_at_ret: None,
            #[cfg(feature = "debug-server")]
            traced_operation: None,
            party,
            map,
            map_key: crate::render::session::FlashMapVisualKey::new(),
            show_map: false,
            display_child: None,
            font_flags: 1,
            sprite_set: 0,
            slots: Vec::new(),
            indoor_pictures: Vec::new(),
            indoor_slot: None,
            indoor_transfer: None,
            cold_sprite_bank: None,
            facing,
            error: None,
            irq: None,
        }));
        let mut flow = self.field_flash.take().unwrap();
        match irq::FieldIrq::new(self, &mut flow) {
            Ok(irq) => flow.irq = Some(irq),
            Err(error) => flow.error = Some(error),
        }
        self.field_flash = Some(flow);
    }

    pub(super) fn update_field_flash(&mut self, input: &InputState) {
        let Some(mut flow) = self.field_flash.take() else {
            return;
        };
        let raw = [
            GbButton::A,
            GbButton::B,
            GbButton::Select,
            GbButton::Start,
            GbButton::Right,
            GbButton::Left,
            GbButton::Up,
            GbButton::Down,
        ]
        .iter()
        .enumerate()
        .fold(0, |bits, (i, &key)| {
            bits | (u8::from(input.is_held(key)) << i)
        });
        if flow.error.is_some() {
            self.field_flash = Some(flow);
            return;
        }
        self.source_display.set_physical_buttons(raw);
        let printing = !flow.text.returned();
        let returned = if printing { self.overworld.tick_party_field_text(&mut flow.text, raw) } else { true };
        if printing && self.overworld.sfx_event == OverworldSfxEvent::TextAdvance {
            if let Some(audio) = &self.audio {
                audio.play_sfx(SfxId::PressAB);
            }
        }
        if !returned {
            if self.game_timer_active() { self.save_data.game_data.play_time.tick(); }
            if let Some(audio) = &self.audio {
                audio.update_frame();
            }
            self.field_flash = Some(flow);
            return;
        }
        match self.service_field_flash(&mut flow) {
            Ok(true) => self.handle_transition(GameScreen::Overworld),
            Ok(false) => self.field_flash = Some(flow),
            Err(error) => {
                log::error!("FLASH restoration stopped: {error}");
                flow.error = Some(error);
                self.field_flash = Some(flow);
            }
        }
    }

    fn service_field_flash(&mut self, flow: &mut FieldFlash) -> Result<bool, String> {
        if self.overworld.state.current_map as u8 != flow.work.map_id() {
            return Err("map changed under FLASH caller".into());
        }
        self.attach_field_irq(flow)?;
        loop {
            if !self.service_field_irq(flow)? { return Ok(false); }
            if self.source_display.remaining() == 0 { return Ok(false); }
            if flow.irq.as_ref().is_some_and(|i| i.delay.is_some() || i.copy.is_some()) {
                self.step_field_wait(flow)?;
                continue;
            }
            // A child may exhaust the host grant between any two source
            // instructions. Keep it, its registers and already written bytes;
            // the next native update grants time without replaying the work.
            if let Some(child) = flow.display_child.as_mut() {
                let result = match child {
                    DisplayChild::Disable(work) => {
                        let mut bus = self.source_display.display_bus();
                        work.run(&mut bus).map(|()| false)
                    }
                    DisplayChild::Enable { work, called } => {
                        if !*called {
                            self.source_display.execute(DisplayInstruction {tcycles:24,access:None}).map(|_| {
                                *called=true;
                                #[cfg(feature="debug-server")]
                                {log::info!(target:"pokered::overworld","FLASH_ENABLE_LCD_ENTRY {}",serde_json::json!({"tcycles":self.source_display.elapsed()}));pokered_core::debug_log::flush();}
                                true
                            })
                        } else {
                            work.step(&mut *self.source_display).map(|i| i.is_some())
                        }
                    }
                    DisplayChild::Palette(work) => {
                        work.step(&mut *self.source_display).map(|i| i.is_some())
                    }
                    DisplayChild::EnableSprites(work) => work.step(&mut *self.source_display),
                    DisplayChild::ClearOam(work) => {
                        work.step(&mut *self.source_display).map(|i| i.is_some())
                    }
                    DisplayChild::IndoorSpritePrefix(work) => work.step(&mut *self.source_display),
                    DisplayChild::IndoorSpriteSlot(work) => work.step(&mut *self.source_display),
                    DisplayChild::IndoorSpriteTransfer(work) => work.step(&mut *self.source_display),
                    DisplayChild::IndoorSpriteClear(work) => work.step(&mut *self.source_display),
                    DisplayChild::ColdSpriteBank(work) => work.step(&mut *self.source_display),
                    DisplayChild::Copy(work) => {
                        let resources = self
                            .resources
                            .as_mut()
                            .ok_or("graphics resources unavailable")?;
                        let assets = NativePatternAssets::new(resources);
                        // Keep the same source adapter while the child runs;
                        // it can resolve its immutable ROM slice once while
                        // every original instruction still reaches the bus.
                        let mut bus = self.source_display.pattern_bus(&mut flow.patterns, &assets);
                        work.run(&mut bus).map(|()| false)
                    }
                };
                match result {
                    Ok(true) => continue,
                    Ok(false) => {
                        if let DisplayChild::ColdSpriteBank(work) = child {
                            flow.cold_sprite_bank =
                                if work.finished() { None } else { Some(*work) };
                        }
                        if let DisplayChild::IndoorSpriteClear(work) = child {
                            let raw = work
                                .pictures()
                                .ok_or("sprite clear returned before final RET")?;
                            let count = flow.indoor_pictures.len();
                            flow.indoor_pictures.copy_from_slice(&raw[1..count + 1]);
                        }
                        if let DisplayChild::IndoorSpriteTransfer(work) = child {
                            flow.indoor_transfer = if work.finished() {
                                None
                            } else {
                                Some(work.clone())
                            };
                        }
                        if let DisplayChild::IndoorSpritePrefix(work) = child {
                            let pictures = work
                                .initialized_pictures()
                                .ok_or("indoor sprite prefix returned before initialization")?;
                            let slots =
                                pokered_core::overworld::field_restore::SpritePatternPlan::new(
                                    pictures,
                                    flow.font_flags & 1 != 0,
                                )
                                .slots;
                            if !matches!(flow.work.operation(), Some(RestoreOperation::InstallSpriteSlots(expected)) if *expected == slots)
                            {
                                return Err(
                                    "initialized sprite RAM differs from raw-slot plan".into()
                                );
                            }
                            flow.indoor_pictures = pictures.to_vec();
                            flow.slots.clear();
                        }
                        if let DisplayChild::IndoorSpriteSlot(work) = child {
                            if let Some(RestoreOperation::SearchIndoorSpriteSlot(index)) =
                                flow.work.operation()
                            {
                                if usize::from(*index) != flow.slots.len() {
                                    return Err("indoor slot search returned out of order".into());
                                }
                                let slot = work
                                    .slot()
                                    .ok_or("indoor slot search returned without assignment")?;
                                let expected =
                                    pokered_core::overworld::field_restore::SpritePatternPlan::new(
                                        &flow.indoor_pictures,
                                        flow.font_flags & 1 != 0,
                                    )
                                    .slots;
                                if expected.get(usize::from(*index)) != Some(&slot) {
                                    return Err(
                                        "executed slot search disagrees with transfer plan".into(),
                                    );
                                }
                                flow.slots.push(slot);
                                flow.indoor_slot = Some(work.clone());
                            }
                        }
                        if matches!(
                            flow.work.operation(),
                            Some(RestoreOperation::ClearShadowOam)
                        ) {
                            let map = self.overworld.state.current_map;
                            self.overworld.player_presentation.clear_shadow_oam(map);
                            self.overworld.npc_presentation.clear_shadow_oam(map);
                            flow.cleared_oam_at_ret = Some((self.source_display.elapsed(),
                                self.source_display.shadow_oam().iter().all(|&b| b == 0)));
                        }
                        flow.display_child = None;
                        if matches!(
                            flow.work.operation(),
                            Some(RestoreOperation::BulkPatternCopy(_))
                        ) {
                            flow.last_bulk_copy_temp_at_ret = Some(self.source_display.source_rom_bank_temp());
                            if let Some(parent) = flow.indoor_transfer.as_mut() {
                                if !parent.copy_returned() {
                                    return Err(
                                        "indoor copy returned outside transfer boundary".into()
                                    );
                                }
                            }
                        }
                        flow.work.child_returned();
                    }
                    Err(DisplayWorkError::Budget) => return Ok(false),
                    Err(DisplayWorkError::InterruptPending) => continue,
                    Err(error) => return Err(format!("{error:?}")),
                }
            }
            // Do not enter a new child after the final instruction overran a
            // grant. The Game owner retains that overrun in its next budget.
            if self.source_display.remaining() == 0 {
                return Ok(false);
            }
            #[cfg(feature = "debug-server")]
            if flow.traced_operation != Some(flow.work.operation_index()) {
                flow.traced_operation = Some(flow.work.operation_index());
                // One log entry per plan stage, including multiple stages
                // inside a single host update. No frame/CPU state is changed.
                log::info!(target: "pokered::overworld", "FLASH_RESTORE_STAGE {}", serde_json::json!({
                    "index": flow.work.operation_index(), "operation": flow.work.operation(),
                    "host_frame": self.frame_count, "tcycles": self.source_display.elapsed(),
                    "vblank_epoch": self.source_display.device().vblank_epoch(),
                    "ly": self.source_display.device().read_port(0xff44).ok(),
                }));
                pokered_core::debug_log::flush();
            }
            let Some(operation) = flow.work.operation().cloned() else {
                return Ok(true);
            };
            match operation {
                RestoreOperation::WhitePalettes => {
                    flow.display_child =
                        Some(DisplayChild::Palette(DisplayPaletteWork::white_out()));
                    continue;
                }
                RestoreOperation::WaitVBlanks(frames) => {
                    flow.waits.push(FieldWaitReceipt { requested: frames,
                        armed_returns: pokered_core::overworld::display_delay_work::DelayFrameBus::interrupt_returns(&*self.source_display),
                        armed_tcycles: self.source_display.elapsed(), completed_returns: None, completed_tcycles: None });
                    flow.irq.as_mut().ok_or("missing IRQ owner")?.delay = Some(pokered_core::overworld::display_delay_work::DelayFramesWork::new(frames));
                    continue;
                }
                RestoreOperation::DisableLcd => {
                    flow.display_child = Some(DisplayChild::Disable(DisableLcdWork::default()));
                    continue;
                }
                RestoreOperation::EnableLcd => {
                    flow.display_child = Some(DisplayChild::Enable {
                        work: EnableLcdWork::default(),
                        called: false,
                    });
                    continue;
                }
                RestoreOperation::EnterColdSpriteBank => {
                    if flow.cold_sprite_bank.is_some() {
                        return Err("nested cold sprite bank call".into());
                    }
                    flow.display_child =
                        Some(DisplayChild::ColdSpriteBank(SpriteBankCallWork::default()));
                    continue;
                }
                RestoreOperation::LeaveColdSpriteBank => {
                    let mut call = flow
                        .cold_sprite_bank
                        .take()
                        .ok_or("missing cold sprite bank caller")?;
                    if !call.body_returned() {
                        return Err("sprite bank caller not awaiting body".into());
                    }
                    flow.display_child = Some(DisplayChild::ColdSpriteBank(call));
                    continue;
                }
                RestoreOperation::ClearIndoorSpritePictures => {
                    if flow.indoor_transfer.is_some() || flow.indoor_slot.is_some() {
                        return Err("sprite IDs cleared before child return".into());
                    }
                    let work = IndoorSpriteClearWork::new(&flow.indoor_pictures)
                        .ok_or("invalid sprite clear input")?;
                    flow.display_child = Some(DisplayChild::IndoorSpriteClear(work));
                    continue;
                }
                RestoreOperation::BulkPatternCopy(copy) => {
                    if flow
                        .indoor_transfer
                        .as_ref()
                        .is_some_and(|parent| parent.copy() != Some(copy))
                    {
                        return Err("prepared indoor bulk copy differs from transfer plan".into());
                    }
                    if self.source_display.device().lcd_enabled() {
                        return Err("bulk transfer while LCD enabled".into());
                    }
                    let work = FarPatternCopyCall::from_2bpp(copy)
                        .ok_or("unsupported LCD-off pattern conversion")?;
                    flow.display_child = Some(DisplayChild::Copy(work));
                    continue;
                }
                RestoreOperation::VideoPatternCopy(copy) => {
                    if flow
                        .indoor_transfer
                        .as_ref()
                        .is_some_and(|parent| parent.copy() != Some(copy))
                    {
                        return Err("prepared indoor video copy differs from transfer plan".into());
                    }
                    self.begin_field_video_copy(flow, copy)?;
                    continue;
                }
                RestoreOperation::SetFontFlags(flags) => flow.font_flags = flags,
                RestoreOperation::SetSpriteSet(set) => flow.sprite_set = set,
                RestoreOperation::InstallSpriteSlots(slots) => {
                    if flow.work.map_id() >= 0x25 {
                        let work = IndoorSpritePrefixWork::new(
                            flow.work.map_id(),
                            flow.work.raw_sprite_pictures(),
                        )
                        .ok_or("unsupported indoor sprite RAM input")?;
                        flow.display_child = Some(DisplayChild::IndoorSpritePrefix(work));
                        continue;
                    }
                    flow.slots = slots;
                }
                RestoreOperation::SearchIndoorSpriteSlot(index) => {
                    if usize::from(index) != flow.slots.len() || flow.indoor_slot.is_some() {
                        return Err("indoor slot search entered out of order".into());
                    }
                    let work = IndoorSpriteSlotWork::new(&flow.indoor_pictures, &flow.slots)
                        .ok_or("invalid initialized indoor sprite RAM")?;
                    flow.display_child = Some(DisplayChild::IndoorSpriteSlot(work));
                    continue;
                }
                RestoreOperation::AdvanceIndoorSpriteSlot(index) => {
                    if flow.indoor_transfer.is_some() {
                        return Err("indoor slot advanced before transfer parent returned".into());
                    }
                    if usize::from(index) + 1 != flow.slots.len() {
                        return Err("indoor slot tail entered out of order".into());
                    }
                    let mut work = flow
                        .indoor_slot
                        .take()
                        .ok_or("missing indoor slot parent")?;
                    if !work.begin_tail() {
                        return Err("indoor slot parent has not returned from search".into());
                    }
                    flow.display_child = Some(DisplayChild::IndoorSpriteSlot(work));
                    continue;
                }
                RestoreOperation::PrepareIndoorSpriteTransfer { index, resume } => {
                    let work = if resume {
                        flow.indoor_transfer
                            .take()
                            .ok_or("missing indoor transfer parent")?
                    } else {
                        if flow.indoor_transfer.is_some() {
                            return Err("nested indoor transfer parent".into());
                        }
                        let slot = *flow
                            .slots
                            .get(usize::from(index))
                            .ok_or("missing executed sprite assignment")?;
                        let remaining = flow
                            .indoor_pictures
                            .len()
                            .checked_sub(usize::from(index))
                            .ok_or("invalid sprite index")?;
                        let four = u8::from(
                            flow.slots[..usize::from(index)]
                                .iter()
                                .any(|s| s.vram_slot >= 11),
                        );
                        IndoorSpriteTransferWork::new(
                            index,
                            remaining as u8,
                            slot,
                            flow.font_flags,
                            four,
                        )
                        .ok_or("unsupported indoor transfer input")?
                    };
                    if work.copy().is_some() {
                        return Err("indoor transfer resumed before copy returned".into());
                    }
                    flow.display_child = Some(DisplayChild::IndoorSpriteTransfer(work));
                    continue;
                }
                RestoreOperation::SetWindowY(144) => {
                    self.overworld.clear_retained_field_text();
                    flow.show_map = true;
                }
                RestoreOperation::RestoreNpcFacing => {
                    if self.overworld.npc_states.len() != flow.facing.len() {
                        return Err("sprite slots changed under FLASH caller".into());
                    }
                    for (npc, &facing) in self.overworld.npc_states.iter_mut().zip(&flow.facing) {
                        npc.facing = facing;
                    }
                }
                RestoreOperation::LoadCurrentMapView => {
                    // Party text has returned and the retained map is on
                    // screen. Its old staging buffer can now render the new
                    // map without another full framebuffer allocation.
                    crate::render::draw_overworld_background(
                        &mut self.overworld,
                        &mut self.resources,
                        &mut flow.party,
                        self.state.config.language,
                    );
                    copy_flash_frame(&flow.party, &mut flow.map);
                    flow.map_key = crate::render::session::FlashMapVisualKey::new();
                }
                RestoreOperation::LoadMapPalettes => {
                    if self.overworld.dark_cave.is_dark() {
                        return Err("lit FLASH palette requested with nonzero map darkness".into());
                    }
                    flow.display_child = Some(DisplayChild::Palette(DisplayPaletteWork::lit_map()));
                    continue;
                }
                RestoreOperation::UpdateSprites => {
                    // Updating ImageIndex prepares future OAM. It does not
                    // publish it or redraw actors into the retained BG image.
                    if flow.font_flags & 1 != 0 {
                        self.overworld.player_presentation.suppress_current_image();
                        self.overworld.npc_presentation.suppress_current_images();
                    } else {
                        self.overworld
                            .player_presentation
                            .tick(self.overworld.state.player.facing, false);
                        self.overworld
                            .npc_presentation
                            .update_images_before_movement(&self.overworld.npc_states);
                    }
                }
                RestoreOperation::EnableSpriteUpdates => {
                    flow.display_child = Some(DisplayChild::EnableSprites(Default::default()));
                    continue;
                }
                RestoreOperation::ClearShadowOam => {
                    flow.display_child =
                        Some(DisplayChild::ClearOam(ClearShadowOamWork::default()));
                    continue;
                }
                // The native renderer resolves raw sprite IDs directly from
                // the same resources (no physical OAM/VRAM slot indirection).
                // Screen buffer2 is the actual retained `flow.map` image.
                RestoreOperation::LoadScreenBuffer2 => {
                    flow.irq.as_mut().ok_or("missing IRQ owner")?.background_is_map = true;
                }
                RestoreOperation::DefaultPaletteCommand => {},
                RestoreOperation::SetAutoBg(enabled) => {
                    self.source_display.execute(DisplayInstruction { tcycles: 12,
                        access: Some(pokered_core::overworld::display_lcd_work::DisplayAccess::Write {
                            port: pokered_core::overworld::video_copy_caller_work::AUTO_BG_ENABLED,
                            value: u8::from(enabled), at_tcycle: 8 }) }).map_err(|e| format!("{e:?}"))?;
                }
                RestoreOperation::Return => {
                    match self.source_display.execute(DisplayInstruction { tcycles: 16, access: None }) {
                        Ok(_) => {
                            self.source_display.set_vblank_dispatch_enabled(false);
                            self.source_display.device_mut().end_native_background_scanout();
                            return Ok(true);
                        }
                        Err(DisplayWorkError::InterruptPending) => continue,
                        Err(DisplayWorkError::Budget) => return Ok(false),
                        Err(error) => return Err(format!("foreground RET: {error:?}")),
                    }
                }
                RestoreOperation::SetWindowY(y) => {
                    return Err(format!("unsupported FLASH window y={y}"))
                }
            }
            flow.work.child_returned();
        }
    }

    /// Read-only causal trace for ordinary debug-server replays.
    #[cfg(feature = "debug-server")]
    pub(super) fn debug_field_flash(&self) -> Option<serde_json::Value> {
        let flow = self.field_flash.as_ref()?;
        Some(serde_json::json!({
            "operation": flow.work.operation(),
            "palette_store_receipts": self.source_display.palette_store_receipts(),
            "background_palette_raster": self.source_display.device().background_palette_raster(),
            "show_map": flow.show_map,
            "irq": &flow.irq,
            "wait_clock": "common_IRQ_RETI",
            "waits": &flow.waits,
            "cleared_oam_at_ret": flow.cleared_oam_at_ret,
            "last_bulk_copy_temp_at_ret": flow.last_bulk_copy_temp_at_ret,
            "sprite_slots": &flow.slots,
            "indoor_slot_parent": &flow.indoor_slot,
            "indoor_transfer_parent": &flow.indoor_transfer,
            "cold_sprite_bank_parent": &flow.cold_sprite_bank,
            "display_child": flow.display_child.as_ref().map(|child| match child {
                DisplayChild::Disable(_) => "disable_lcd",
                DisplayChild::EnableSprites(_) => "enable_sprites",
                DisplayChild::Enable {..} => "enable_lcd",
                DisplayChild::Copy(_) => "bulk_copy",
                DisplayChild::Palette(_) => "palette",
                DisplayChild::ClearOam(_) => "clear_shadow_oam",
                DisplayChild::IndoorSpritePrefix(_) => "indoor_sprite_prefix",
                DisplayChild::IndoorSpriteSlot(_) => "indoor_sprite_slot",
                DisplayChild::IndoorSpriteTransfer(_) => "indoor_sprite_transfer",
                DisplayChild::IndoorSpriteClear(_) => "indoor_sprite_clear",
                DisplayChild::ColdSpriteBank(_) => "cold_sprite_bank",
            }),
            "error": &flow.error,
        }))
    }

    pub(crate) fn field_flash_white_frame(&self) -> bool {
        self.field_flash_frame_visible() && self.field_flash_is_white()
    }

    fn field_flash_frame_visible(&self) -> bool {
        self.state.screen == GameScreen::PartyScreen
            && self.black_screen_frames == 0
            && self.post_capture_flow.is_none()
            && self.trade_anim.is_none()
            && self.evolution_anim.is_none()
            && self.hof_ceremony.is_none()
            && self.credits.is_none()
    }

    pub(crate) fn field_flash_map_key(&self) -> Option<crate::render::session::FlashMapVisualKey> {
        if !self.field_flash_frame_visible() || self.field_flash_is_white() { return None; }
        self.field_flash.as_ref().filter(|flow| flow.show_map).map(|flow| {
            flow.map_key.clone().with_palette_revision(self.source_display.device()
                .background_palette_raster().map_or(0, |raster| raster.revision()))
        })
    }

    pub(super) fn field_flash_is_white(&self) -> bool {
        self.field_flash.is_some() && self.source_display.device()
            .background_palette_raster().is_some_and(|raster| raster.is_white())
    }

    pub(super) fn draw_field_flash(&self, fb: &mut FrameBuffer) {
        let flow = self.field_flash.as_ref().unwrap();
        if self.field_flash_is_white() {
            fb.clear(Rgba::WHITE);
            return;
        }
        if flow.show_map {
            draw_flash_frame(&flow.map, fb);
            if let Some(raster) = self.source_display.device().background_palette_raster() {
                apply_flash_palette(raster, fb);
            }
            return;
        }
        fb.copy_from(&flow.party);
        if let Some(dialogue) = self.overworld.displayed_field_dialogue() {
            if let Some((first, second)) = dialogue.get_display_text() {
                let text = if second.is_empty() {
                    first
                } else {
                    format!("{first}\n{second}")
                };
                let arrow = !flow.text.returned()
                    && dialogue.waiting_for_input()
                    && (self.frame_count / 16) % 2 == 0;
                let mut painter = pokered_ui::backends::FrameBufferPainter::new(fb);
                pokered_ui::menus::dialog::draw_paginated(
                    &text,
                    arrow,
                    &pokered_data::ui_layout::schema::DIALOG_DEFAULT_LAYOUT,
                    &mut pokered_ui::Ui::new(&mut painter),
                    self.state.config.language,
                );
            }
        }
        if let Some(raster) = self.source_display.device().background_palette_raster() {
            apply_flash_palette(raster, fb);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32"), not(target_os = "none")))]
mod tests {
    use super::*;

    // Component fixtures enter after the source party drawer's UpdateSprites
    // and use the same real virtual audio owner as --no-audio normal boot.
    fn begin_fixture_flash(game: &mut PokemonGame, message: &str) {
        game.audio = Some(AudioOutput::new_pcm());
        let audio = game.audio.as_ref().unwrap();
        audio.play_music(pokered_audio::music_data::MusicId::DUNGEON3);
        audio.update_frame();
        game.overworld.publish_npc_frame(false);
        game.overworld.player_presentation.suppress_current_image();
        game.overworld.npc_presentation.suppress_current_images();
        game.begin_field_flash(message);
        assert!(game.field_flash.as_ref().unwrap().error.is_none(), "{:?}", game.field_flash.as_ref().unwrap().error);
    }


    #[test]
    fn flash_samples_the_final_native_caption_only_when_text_returns() {
        let mut game = PokemonGame::new(GameVersion::Red);
        game.overworld = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
        game.state.screen = GameScreen::PartyScreen;
        game.overworld.set_no_text_delay(false);
        begin_fixture_flash(&mut game, "A blinding FLASH lights the area!");
        for _ in 0..600 {
            game.update(&InputState::new());
            let irq = game.field_flash.as_ref().unwrap().irq.as_ref().unwrap();
            assert!(!irq.attached);
            assert!(irq.ui.tiles.is_empty(), "printing must not sample a raster no IRQ can read");
            assert!(irq.ui.patterns.is_empty());
        }
        assert!(game.overworld.displayed_field_dialogue().unwrap().waiting_for_input());
        let mut acknowledge = InputState::new();
        acknowledge.press(GbButton::A);
        game.update(&acknowledge);
        let flow = game.field_flash.as_ref().unwrap();
        assert!(flow.error.is_none(), "{:?}", flow.error);
        assert!(flow.text.returned());
        let irq = flow.irq.as_ref().unwrap();
        assert!(irq.attached);
        assert_eq!(irq.ui.revision, 1);
        assert_eq!(irq.ui.tiles.len(), 360);
        for y in 0..144 {
            for x in 0..160 {
                let tile = irq.ui.tiles[(y / 8 * 20 + x / 8) as usize];
                let pattern = irq.ui.patterns[usize::from(tile)];
                let row = (y % 8) as usize;
                let shift = 7 - x % 8;
                let shade = ((pattern[row * 2] >> shift) & 1)
                    | (((pattern[row * 2 + 1] >> shift) & 1) << 1);
                assert_eq!(shade, flow.party.get_index(x, y).unwrap() as u8);
            }
        }
        let final_raster = irq.ui.clone();
        game.update(&InputState::new());
        assert_eq!(game.field_flash.as_ref().unwrap().irq.as_ref().unwrap().ui, final_raster);
    }

    #[test]
    fn gba_flash_unpack_matches_every_bitplane_pair() {
        use dotzuki_renderer::palette::GbColor;
        let mut packed = FlashFrame::new(RenderConfig::new(160, 144), Rgba::WHITE);
        let mut linear = dotzuki_renderer::LinearRgbaIndexedFrameBuffer::<GbColor>::new(
            RenderConfig::new(160, 144),
            Rgba::WHITE,
        );
        packed.apply_bgp(0x1b);
        let groups = packed.packed().len() / 2;
        for base in (0..65536usize).step_by(groups) {
            for (i, planes) in packed.packed_mut().chunks_exact_mut(2).enumerate() {
                planes.copy_from_slice(&((base + i) as u16).to_le_bytes());
            }
            unpack_flash_frame(&packed, &mut linear);
            for (i, planes) in packed.packed().chunks_exact(2).enumerate() {
                for bit in 0..8 {
                    let expected =
                        ((planes[0] >> (7 - bit)) & 1) | (((planes[1] >> (7 - bit)) & 1) << 1);
                    assert_eq!(linear.indices()[i * 8 + bit], expected);
                    let pixel = i * 8 + bit;
                    assert_eq!(
                        linear.get_pixel((pixel % 160) as u32, (pixel / 160) as u32),
                        packed.get_pixel((pixel % 160) as u32, (pixel / 160) as u32)
                    );
                }
            }
        }
    }

    #[test]
    fn packed_flash_snapshots_preserve_linear_pixels_and_display_palette() {
        use dotzuki_renderer::palette::GbColor;
        let mut linear = dotzuki_renderer::LinearRgbaIndexedFrameBuffer::<GbColor>::new(
            RenderConfig::new(160, 144),
            Rgba::WHITE,
        );
        for y in 0..144 {
            for x in 0..160 {
                linear.set_pixel_index(x, y, GbColor::from_u8(((x + y * 3) % 4) as u8));
            }
        }
        linear.apply_bgp(0x1b);
        let expected: Vec<_> = (0..144)
            .flat_map(|y| (0..160).map(move |x| (x, y)))
            .map(|(x, y)| linear.get_pixel(x, y).unwrap())
            .collect();
        let packed = retain_flash_frame(linear);
        assert_eq!(packed.packed().len(), 5760);
        for (i, color) in expected.into_iter().enumerate() {
            assert_eq!(
                packed.get_pixel((i % 160) as u32, (i / 160) as u32),
                Some(color)
            );
        }
    }

    #[test]
    fn flash_three_vblank_wait_cannot_return_after_only_two_new_edges() {
        let mut game = PokemonGame::new_with_options(
            GameVersion::Red,
            None,
            None,
            None,
            false,
            None,
            false,
            true,
            #[cfg(feature = "debug-server")]
            None,
        );
        game.overworld = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
        begin_fixture_flash(&mut game, "FLASH");
        let mut flow = game.field_flash.take().unwrap();
        assert!(flow.error.is_none(), "FLASH input initialization: {:?}", flow.error);
        // Isolate the real plan's Delay3→return boundary so later LCD polling
        // cannot mask an early wait return. This is a unit fixture, not a
        // game-runtime setter or an ordinary-playthrough claim.
        let mut plan = serde_json::to_value(&flow.work).unwrap();
        plan["operations"] = serde_json::json!([{"WaitVBlanks":3}, "Return"]);
        plan["next"] = serde_json::json!(0);
        flow.work = serde_json::from_value(plan).unwrap();
        game.source_display.begin_native_update();
        while game.source_display.device().vblank_epoch() == 0 {
            game.source_display.idle_to_device_event().unwrap();
        }
        let armed = game.source_display.device().vblank_epoch();
        game.frame_count = 1;
        assert!(!game.service_field_flash(&mut flow).unwrap());
        game.source_display.idle_to_target().unwrap();
        let mut returned = false;
        for _ in 0..5 {
            game.frame_count += 1;
            game.source_display.begin_native_update();
            returned = game.service_field_flash(&mut flow).unwrap();
            if returned {
                assert!(
                    game.source_display.device().vblank_epoch() >= armed + 3,
                    "host frame reached the deadline before the third new VBlank"
                );
                break;
            }
            game.source_display.idle_to_target().unwrap();
        }
        assert!(returned);
    }

    #[test]
    fn flash_video_waits_for_a_new_device_vblank_even_when_host_frame_advanced() {
        use pokered_core::overworld::field_restore::{CopyFormat, PatternCopy, PatternSource};
        let mut game = PokemonGame::new_with_options(
            GameVersion::Red,
            None,
            None,
            None,
            false,
            None,
            false,
            true,
            #[cfg(feature = "debug-server")]
            None,
        );
        game.overworld = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
        begin_fixture_flash(&mut game, "FLASH");
        let mut flow = game.field_flash.take().unwrap();
        assert!(flow.error.is_none(), "FLASH input initialization: {:?}", flow.error);
        game.source_display.begin_native_update();
        while game.source_display.device().vblank_epoch() == 0 {
            game.source_display.idle_to_device_event().unwrap();
        }
        // Arm after this grant's VBlank. Its remaining time ends before the
        // next VBlank, regardless of the unrelated host frame counter.
        let edge = game.source_display.device().vblank_epoch();
        // Controlled unit boundary: the earlier legacy edge is retired.
        // This sets fixture IF only, never an interrupt-return receipt or
        // evidence of a normally executed previous IRQ.
        game.source_display.device_mut().acknowledge_interrupt(1);
        let before = flow.patterns.bytes()[..192].to_vec();
        let copy = PatternCopy {
            source: PatternSource::WalkingPlayer,
            source_offset: 0,
            destination: 0x8000,
            tiles: 12,
            format: CopyFormat::TwoBpp,
        };
        let mut plan = serde_json::to_value(&flow.work).unwrap();
        plan["operations"] = serde_json::json!([{"VideoPatternCopy": copy}, "Return"]);
        plan["next"] = serde_json::json!(0);
        flow.work = serde_json::from_value(plan).unwrap();
        game.frame_count = edge + 1;
        assert!(!game.service_field_flash(&mut flow).unwrap());
        assert_eq!(game.source_display.device().vblank_epoch(), edge);
        assert_eq!(game.source_display.video_requests().snapshot(CopyFormat::TwoBpp).size, Some(8));
        assert_eq!(&flow.patterns.bytes()[..192], &before);
    }

    #[test]
    fn flash_restore_runs_lcd_and_copy_children_on_game_time_without_moving_player() {
        let mut game = PokemonGame::new_with_options(
            GameVersion::Red,
            None,
            None,
            None,
            false,
            None,
            false,
            true,
            #[cfg(feature = "debug-server")]
            None,
        );
        game.overworld = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
        game.overworld.state.player.x = 15;
        game.overworld.state.player.y = 3;
        game.state.screen = GameScreen::PartyScreen;
        let mon = pokered_core::pokemon::stats::create_pokemon(
            pokered_data::species::Species::Bulbasaur,
            12,
            [0xff, 0xff],
        )
        .unwrap();
        let outcome = game.overworld.use_field_move(
            pokered_data::moves::MoveId::Flash,
            &mon,
            1,
            MapId::PalletTown,
        );
        let pokered_core::overworld::field_moves::FieldMoveOutcome::Flash { message } = outcome
        else {
            panic!("successful FLASH required");
        };
        begin_fixture_flash(&mut game, &message);
        let mut flow = game.field_flash.take().unwrap();
        assert!(flow.error.is_none(), "FLASH input initialization: {:?}", flow.error);
        game.source_display.set_shadow_oam(&[0x5a; 160]).unwrap();
        let mut saw_lcd_off = false;
        let mut saw_pending_child = false;
        let mut done = false;
        for _ in 0..120 {
            game.frame_count += 1;
            game.source_display.begin_native_update();
            done = game.service_field_flash(&mut flow).unwrap();
            saw_lcd_off |= !game.source_display.device().lcd_enabled();
            saw_pending_child |= flow.display_child.is_some();
            assert_eq!(
                (game.overworld.state.player.x, game.overworld.state.player.y),
                (15, 3)
            );
            game.source_display.idle_to_target().unwrap();
            if done {
                break;
            }
        }
        assert!(done, "the real restoration plan must return");
        assert!(
            saw_pending_child,
            "LCD polling/copies must suspend across host grants"
        );
        assert!(
            saw_lcd_off,
            "real LCD state must remain off during suspended copies"
        );
        assert!(game.source_display.device().lcd_enabled());
        assert!(flow.display_child.is_none());
        assert!(
            flow.cleared_oam_at_ret.is_some_and(|(_, zero)| zero),
            "ClearSprites must clear the actual shadow page at its own RET"
        );
        assert!(
            game.source_display.device().oam().chunks_exact(4).all(|object| object == [160, 0, 0, 0]),
            "subsequent actual PrepareOAM/DMA hides every unused object at Y=160"
        );
        assert_eq!(
            game.source_display.device().read_port(0xff47).unwrap(),
            0xe4,
            "LoadGBPal must actually restore BGP, not just a drawing flag"
        );
        assert_eq!(
            game.source_display.device().read_port(0xff48).unwrap(),
            0xd0
        );
        assert_eq!(
            game.source_display.device().read_port(0xff49).unwrap(),
            0xe0
        );
        assert_eq!(
            flow.last_bulk_copy_temp_at_ret,
            Some(5),
            "observe FarCopyData2 at its RET before later video callers reuse the shared temp port"
        );
        let map = game.overworld.state.current_map;
        assert!(
            game.overworld.player_presentation.image_suppressed(map),
            "UpdateSprites must not publish the prepared image immediately"
        );
        game.overworld.publish_npc_frame(false);
        assert!(game.overworld.player_presentation.image_suppressed(map));
        game.overworld.publish_npc_frame(false);
        assert!(!game.overworld.player_presentation.image_suppressed(map));
    }
    #[test]
    fn actual_flash_updates_block_held_movement_through_whiteout_and_repeat_use() {
        use pokered_core::overworld::field_moves::FieldMoveOutcome;
        use pokered_data::moves::MoveId;
        for (map, x, y) in [(MapId::RockTunnel1F, 15, 3), (MapId::PalletTown, 10, 12)] {
            let mut game = PokemonGame::new_with_options(
                GameVersion::Red,
                None,
                None,
                None,
                false,
                None,
                false,
                true,
                #[cfg(feature = "debug-server")]
                None,
            );
            game.overworld = OverworldScreen::new(map, None, PokemonRedData);
            game.overworld.state.player.x = x;
            game.overworld.state.player.y = y;
            let mon = pokered_core::pokemon::stats::create_pokemon(
                pokered_data::species::Species::Bulbasaur,
                12,
                [0xff, 0xff],
            )
            .unwrap();
            for _ in 0..2 {
                game.state.screen = GameScreen::PartyScreen;
                let result =
                    game.overworld
                        .use_field_move(MoveId::Flash, &mon, 1, MapId::PalletTown);
                let FieldMoveOutcome::Flash { message } = result else {
                    panic!("FLASH caller");
                };
                begin_fixture_flash(&mut game, &message);
                let mut input = InputState::new();
                let mut saw_white = false;
                let mut returned = false;
                let mut saw_no_irq_lcd_off = false;
                for t in 0..500 {
                    input.begin_frame();
                    input.press(GbButton::Down);
                    if t % 20 == 0 {
                        input.press(GbButton::A);
                    } else {
                        input.release(GbButton::A);
                    }
                    let suspended_audio = (!game.source_display.device().lcd_enabled())
                        .then(|| format!("{:?}", game.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.channels));
                    let old_time = game.save_data.game_data.play_time;
                    let old_returns = pokered_core::overworld::display_delay_work::DelayFrameBus::interrupt_returns(&*game.source_display);
                    game.update(&input);
                    if let Some(channels) = suspended_audio {
                        if game.source_audio_work.is_none() {
                            let elapsed = |t: pokered_core::save::game_data::PlayTime| u64::from(t.hours) * 216000
                                + u64::from(t.minutes) * 3600 + u64::from(t.seconds) * 60 + u64::from(t.frames);
                            assert_eq!(elapsed(game.save_data.game_data.play_time) - elapsed(old_time),
                                pokered_core::overworld::display_delay_work::DelayFrameBus::interrupt_returns(&*game.source_display) - old_returns,
                                "play time follows completed source IRQs, including an epilogue split across host grants");
                            assert_eq!(format!("{:?}", game.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.channels), channels);
                            saw_no_irq_lcd_off = true;
                        }
                    }
                    assert_eq!(
                        (game.overworld.state.player.x, game.overworld.state.player.y),
                        (x, y)
                    );
                    assert_eq!(
                        game.overworld.state.player.movement_state,
                        pokered_core::overworld::MovementState::Idle
                    );
                    let Some(_) = game.field_flash.as_ref() else {
                        returned = true;
                        break;
                    };
                    assert_eq!(game.state.screen, GameScreen::PartyScreen);
                    if game.field_flash_is_white() {
                        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::BLACK);
                        game.draw_field_flash(&mut fb);
                        assert!((0..144)
                            .all(|y| (0..160).all(|x| fb.get_pixel(x, y) == Some(Rgba::WHITE))));
                        saw_white = true;
                    }
                }
                assert!(
                    returned && saw_white,
                    "notice, white-out and full restoration must return: map={map:?} error={:?}",
                    game.field_flash.as_ref().and_then(|f| f.error.as_ref())
                );
                assert!(saw_no_irq_lcd_off, "the case must exercise a grant beginning LCD-off without a new audio update");
                assert_eq!(game.state.screen, GameScreen::Overworld);
                assert_eq!(game.overworld.state.current_map, map);
                assert!(!game.overworld.dark_cave.is_dark());
            }
        }
    }
    #[test]
    fn retained_renderer_matches_flash_typing_whiteout_and_map_return() {
        use pokered_core::overworld::Direction;
        for (map, px, py) in [(MapId::RockTunnel1F, 15, 3), (MapId::RedsHouse2F, 3, 3)] {
            for facing in [
                Direction::Down,
                Direction::Up,
                Direction::Left,
                Direction::Right,
            ] {
                let mut game = PokemonGame::new(GameVersion::Red);
                game.audio = None;
                game.overworld = OverworldScreen::new(map, None, PokemonRedData);
                game.overworld.state.player.x = px;
                game.overworld.state.player.y = py;
                game.overworld.state.player.facing = facing;
                game.state.screen = GameScreen::PartyScreen;
                let mut mon = pokered_core::pokemon::stats::create_pokemon(
                    pokered_data::species::Species::Bulbasaur,
                    12,
                    [0xff, 0xff],
                )
                .unwrap();
                mon.moves = [
                    pokered_data::moves::MoveId::Flash,
                    pokered_data::moves::MoveId::Tackle,
                    pokered_data::moves::MoveId::None,
                    pokered_data::moves::MoveId::None,
                ];
                mon.pp = [20, 35, 0, 0];
                game.save_data.party.add(mon.clone()).unwrap();
                game.party_screen = PartyScreenState::new(game.save_data.party.to_vec());
                game.overworld.set_no_text_delay(false);
                let outcome = game.overworld.use_field_move(
                    pokered_data::moves::MoveId::Flash,
                    &mon,
                    1,
                    MapId::PalletTown,
                );
                let pokered_core::overworld::field_moves::FieldMoveOutcome::Flash { message } =
                    outcome
                else {
                    panic!("FLASH caller required");
                };
                begin_fixture_flash(&mut game, &message);
                let mut session = crate::render::session::RenderSession::new();
                let mut retained = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| {};
                let mut returned = false;
                let mut returned_at = None;
                let mut saw_white = false;
                let mut reused_white = false;
                let mut reused_map = false;
                let mut checked_interruption = false;
                let mut checked_map_interruption = false;
                let mut last_map_key = None;
                for frame in 0..500 {
                    let mut input = InputState::new();
                    if frame % 20 == 0 {
                        input.press(GbButton::A);
                    }
                    game.update(&input);
                    let map_key = game.field_flash_map_key();
                    let reused = matches!(
                        session.render(&mut game, &mut retained, &mut scroll),
                        crate::render::session::FrameUpdate::Reuse
                    );
                    reused_white |= reused && game.field_flash_is_white();
                    reused_map |= reused && map_key.is_some();
                    if map_key.is_some() && map_key != last_map_key { assert!(!reused); }
                    last_map_key = map_key.clone();
                    let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                    game.draw(&mut full);
                    for y in 0..144 {
                        for x in 0..160 {
                            assert_eq!(retained.get_pixel(x,y), full.get_pixel(x,y),
                        "FLASH retained map={map:?} facing={facing:?} frame={frame} pixel=({x},{y})");
                        }
                    }
                    if reused_white && !checked_interruption {
                        // A different takeover must invalidate the retained
                        // white frame even though FLASH itself is unchanged.
                        game.black_screen_frames = 1;
                        assert!(matches!(
                            session.render(&mut game, &mut retained, &mut scroll),
                            crate::render::session::FrameUpdate::Full
                        ));
                        assert!((0..144).all(|y| (0..160)
                            .all(|x| retained.get_pixel(x, y) == Some(Rgba::BLACK))));
                        game.black_screen_frames = 0;
                        assert!(matches!(
                            session.render(&mut game, &mut retained, &mut scroll),
                            crate::render::session::FrameUpdate::Full
                        ));
                        assert!((0..144).all(|y| (0..160)
                            .all(|x| retained.get_pixel(x, y) == full.get_pixel(x, y))));
                        checked_interruption = true;
                    }
                    if reused && map_key.is_some() && !checked_map_interruption {
                        game.black_screen_frames = 1;
                        assert!(matches!(session.render(&mut game, &mut retained, &mut scroll), crate::render::session::FrameUpdate::Full));
                        assert!((0..144).all(|y| (0..160).all(|x| retained.get_pixel(x,y) == Some(Rgba::BLACK))));
                        game.black_screen_frames = 0;
                        assert!(matches!(session.render(&mut game, &mut retained, &mut scroll), crate::render::session::FrameUpdate::Full));
                        assert!((0..144).all(|y| (0..160).all(|x| retained.get_pixel(x,y) == full.get_pixel(x,y))));
                        checked_map_interruption = true;
                    }
                    if game.field_flash.is_some() {
                        saw_white |= game.field_flash_is_white();
                    } else {
                        let first = *returned_at.get_or_insert(frame);
                        if frame - first >= 8 {
                            returned = true;
                            break;
                        }
                    }
                }
                assert!(returned && saw_white && reused_white && checked_interruption);
                assert!(reused_map && checked_map_interruption, "frozen map reuse and takeover restoration must be exercised");
                assert_eq!(game.state.screen, GameScreen::Overworld);
            }
        }
    }
    #[test]
    fn frozen_flash_map_cache_invalidates_snapshot_writes_and_replaced_callers() {
        let mut game = PokemonGame::new(GameVersion::Red);
        game.overworld = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
        game.state.screen = GameScreen::PartyScreen;
        begin_fixture_flash(&mut game, "FLASH");
        // Constructed renderer fixture: no source continuation is advanced.
        let flow = game.field_flash.as_mut().unwrap();
        flow.show_map = true;
        flow.map.clear(Rgba::BLACK);
        let mut session = crate::render::session::RenderSession::new();
        let mut retained = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
        let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| {};
        assert!(matches!(session.render(&mut game, &mut retained, &mut scroll), crate::render::session::FrameUpdate::Full));
        assert!(matches!(session.render(&mut game, &mut retained, &mut scroll), crate::render::session::FrameUpdate::Reuse));
        assert_eq!(retained.get_pixel(0,0), Some(Rgba::BLACK));
        let flow = game.field_flash.as_mut().unwrap();
        flow.map.clear(Rgba::WHITE);
        flow.map_key = crate::render::session::FlashMapVisualKey::new();
        assert!(matches!(session.render(&mut game, &mut retained, &mut scroll), crate::render::session::FrameUpdate::Full));
        assert_eq!(retained.get_pixel(0,0), Some(Rgba::WHITE));
        assert!(matches!(session.render(&mut game, &mut retained, &mut scroll), crate::render::session::FrameUpdate::Reuse));
        // Keep the old identity alive in both this variable and the session;
        // restarting without an intervening render must never reuse its page.
        let old_key = game.field_flash_map_key().unwrap();
        game.discard_field_flash();
        begin_fixture_flash(&mut game, "FLASH AGAIN");
        let flow = game.field_flash.as_mut().unwrap();
        flow.show_map = true;
        flow.map.clear(Rgba::BLACK);
        assert_ne!(game.field_flash_map_key().unwrap(), old_key);
        assert!(matches!(session.render(&mut game, &mut retained, &mut scroll), crate::render::session::FrameUpdate::Full));
        assert_eq!(retained.get_pixel(0,0), Some(Rgba::BLACK));
    }

    #[test]
    fn completed_palette_raster_invalidates_the_retained_map_without_revealing_unscanned_pixels() {
        use pokered_core::overworld::display_lcd_work::DisplayAccess;
        use crate::render::session::FrameUpdate;
        let mut game = PokemonGame::new(GameVersion::Red);
        game.overworld = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
        game.state.screen = GameScreen::PartyScreen;
        begin_fixture_flash(&mut game, "FLASH");
        let flow = game.field_flash.as_mut().unwrap();
        flow.show_map = true;
        flow.map.clear(Rgba::BLACK);
        // Renderer fixture uses the real time/device bus, without pretending
        // that these controlled writes are a complete gameplay continuation.
        for _ in 0..2 {
            game.source_display.begin_native_update();
            game.source_display.idle_to_target().unwrap();
        }
        game.source_display.device_mut().begin_native_background_scanout(0xe4);
        let mut session = crate::render::session::RenderSession::new();
        let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
        let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| {};
        assert!(matches!(session.render(&mut game, &mut fb, &mut scroll), FrameUpdate::Full));
        for (line, palette, top, bottom) in [
            (20, 0, Rgba::BLACK, Rgba::WHITE),
            (7, 0xe4, Rgba::WHITE, Rgba::BLACK),
        ] {
            game.source_display.begin_native_update();
            while game.source_display.device().read_port(0xff44).unwrap() != line {
                game.source_display.execute(DisplayInstruction { tcycles:4, access:None }).unwrap();
            }
            game.source_display.execute(DisplayInstruction { tcycles:12,
                access:Some(DisplayAccess::Write { port:0xff47,value:palette,at_tcycle:8 }) }).unwrap();
            assert!(matches!(session.render(&mut game, &mut fb, &mut scroll), FrameUpdate::Reuse),
                "a register store must not redraw pixels the LCD has not completed");
            game.source_display.idle_to_target().unwrap();
            assert!(matches!(session.render(&mut game, &mut fb, &mut scroll), FrameUpdate::Full));
            for y in 0..144 {
                for x in 0..160 {
                    assert_eq!(fb.get_pixel(x,y),Some(if y<u32::from(line) {top}else{bottom}));
                }
            }
            assert!(matches!(session.render(&mut game, &mut fb, &mut scroll), FrameUpdate::Reuse));
            game.source_display.begin_native_update();
            game.source_display.idle_to_target().unwrap();
            assert!(matches!(session.render(&mut game, &mut fb, &mut scroll), FrameUpdate::Full));
            assert!((0..144).all(|y| (0..160).all(|x| fb.get_pixel(x,y)==Some(bottom))));
        }
    }
    #[test]
    fn abandoning_an_attached_flash_discards_active_irq_and_lcd_off_continuations() {
        for seek_lcd_off in [false, true] {
            for exit in 0..3 {
                let mut game = PokemonGame::new_with_options(GameVersion::Red, None, None, None,
                    false, None, false, true, #[cfg(feature = "debug-server")] None);
                game.overworld = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
                game.overworld.state.player.x = 15; game.overworld.state.player.y = 3;
                game.state.screen = GameScreen::PartyScreen;
                begin_fixture_flash(&mut game, "FLASH");
                let mut reached = false;
                for frame in 0..200 {
                    let mut input = InputState::new();
                    if frame % 20 == 0 { input.press(GbButton::A); }
                    game.update(&input);
                    let attached = game.field_irq_owns_clock();
                    if attached && if seek_lcd_off { !game.source_display.device().lcd_enabled() }
                        else { game.source_display.active_vblank() } { reached = true; break; }
                    assert!(game.field_flash.as_ref().is_some_and(|f| f.error.is_none()), "fixture must reach a real busy source caller");
                }
                assert!(reached, "must exercise the selected active IRQ/LCD-off state");
                let path = std::env::temp_dir().join(format!("flash-irq-abandon-{}-{}-{}.sav", std::process::id(), seek_lcd_off, exit));
                match exit {
                    0 => { std::fs::write(&path, export_sram(&game.save_data)).unwrap(); game.load_from_path(&path).unwrap(); std::fs::remove_file(&path).unwrap(); }
                    1 => game.handle_transition(GameScreen::TitleScreen),
                    _ => { game.save_path = Some(path); game.soft_reset(); }
                }
                assert!(game.field_flash.is_none());
                assert!(!game.source_display.active_vblank());
                assert!(!game.source_display.pending_vblank_dispatch());
                assert!(game.source_display.device().lcd_enabled());
                assert_eq!(game.source_audio_work, None);
                // A subsequent native UI owner can acquire the bus; no stale
                // attached IRQ is mistaken for an interrupted continuation.
                game.source_display.begin_legacy_party_sprite_hide().unwrap();
                game.update(&InputState::new());
                assert!(!game.source_display.active_vblank());
            }
        }
    }

}

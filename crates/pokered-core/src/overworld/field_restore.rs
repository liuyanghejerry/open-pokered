//! Field-menu return operations from start_sub_menus.asm and palettes.asm.
//!
//! The ordered plan and outstanding video transfer carry no host-frame timer.
//! A caller must service each real resource request at a later VBlank and
//! acknowledge its return before continuing. This module is the protocol;
//! the app/display owner supplies execution, palettes, buffers and input lock.
use crate::alloc_prelude::*;

/// Source asset identity is deliberately the raw picture ID. Two IDs may name
/// the same resource but still take separate VRAM slots in InitMapSprites.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PatternSource {
    Sprite(u8),
    WalkingPlayer,
    BicyclePlayer,
    SurfingPlayer,
    Font,
    TextBox,
    Flower(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CopyFormat {
    TwoBpp,
    /// CopyVideoDataDouble duplicates each source byte into both bitplanes.
    OneBppDouble,
}
impl CopyFormat {
    pub fn source_bytes_per_tile(self) -> u16 {
        match self {
            Self::TwoBpp => 16,
            Self::OneBppDouble => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PatternCopy {
    pub source: PatternSource,
    pub source_offset: u16,
    pub destination: u16,
    pub tiles: u8,
    pub format: CopyFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpriteSlot {
    pub picture: u8,
    pub vram_slot: u8,
}

/// Result of the source's prior-slot search. Hidden slots are supplied too;
/// visibility and decoded-resource identity are intentionally irrelevant.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpritePatternPlan {
    pub slots: Vec<SpriteSlot>,
    pub copies: Vec<PatternCopy>,
}
impl SpritePatternPlan {
    /// `pictures` is the selected complete outdoor set, or the first
    /// wNumSprites indoor raw object slots. The player reserves slot 1.
    /// `font_loaded` selects the original upper-only reload branch.
    pub fn new(pictures: &[u8], font_loaded: bool) -> Self {
        let mut slots: Vec<SpriteSlot> = Vec::new();
        let mut copies = Vec::new();
        let mut four_tile_count = 0u8;
        for &picture in pictures {
            if let Some(prior) = slots.iter().find(|slot| slot.picture == picture) {
                slots.push(*prior);
                continue;
            }
            let highest = slots
                .iter()
                .map(|slot| slot.vram_slot)
                .filter(|&slot| slot < 11)
                .max()
                .unwrap_or(1);
            let slot = if picture >= 0x3d {
                11 + four_tile_count
            } else {
                highest + 1
            };
            let still_slot = slot >= 11;
            let destination = if still_slot {
                let address = if four_tile_count == 0 { 0x8780 } else { 0x87c0 };
                // Source increments this only when assigning the first still
                // picture. Subsequent still IDs share its second 4-tile slot.
                if four_tile_count == 0 {
                    four_tile_count = 1;
                }
                address
            } else {
                0x8000 + u16::from(slot - 1) * 12 * 16
            };
            slots.push(SpriteSlot {
                picture,
                vram_slot: slot,
            });
            if !font_loaded {
                copies.push(PatternCopy {
                    source: PatternSource::Sprite(picture),
                    source_offset: 0,
                    destination,
                    tiles: if picture >= 0x3d { 4 } else { 12 },
                    format: CopyFormat::TwoBpp,
                });
            }
            if !still_slot {
                copies.push(PatternCopy {
                    source: PatternSource::Sprite(picture),
                    source_offset: 12 * 16,
                    destination: destination + 0x800,
                    tiles: 12,
                    format: CopyFormat::TwoBpp,
                });
            }
        }
        Self { slots, copies }
    }
}

/// A real pattern memory, not a "loaded" flag. Native resource decoding may
/// cache source assets; each restoration still rewrites the target bytes.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PatternMemory {
    bytes: Vec<u8>,
}
impl Default for PatternMemory {
    fn default() -> Self {
        Self {
            bytes: vec![0; 0x2000],
        }
    }
}
impl PatternMemory {
    /// FarCopyData2 runs directly while the caller has disabled the LCD.
    /// Unlike CopyVideoData, it does not create a VBlank request. Validate the
    /// entire source before changing memory, just as the queued service does.
    pub fn copy_lcd_disabled(
        &mut self,
        copy: PatternCopy,
        assets: &impl PatternAssets,
    ) -> Result<(), RestoreError> {
        if self.bytes.len() != 0x2000 {
            return Err(RestoreError::InvalidPatternMemory);
        }
        let length = u16::from(copy.tiles) * copy.format.source_bytes_per_tile();
        let end = copy
            .source_offset
            .checked_add(length)
            .ok_or(RestoreError::SourceOverflow)?;
        let bytes: Vec<_> = (copy.source_offset..end)
            .map(|offset| {
                assets
                    .read_pattern_byte(copy.source, offset)
                    .ok_or(RestoreError::MissingSource(copy.source, offset))
            })
            .collect::<Result<_, _>>()?;
        let written = u16::from(copy.tiles) * 16;
        if written > 0
            && (copy.destination < 0x8000
                || copy
                    .destination
                    .checked_add(written - 1)
                    .is_none_or(|end| end >= 0xa000))
        {
            return Err(RestoreError::InvalidDestination(copy.destination));
        }
        for (i, byte) in bytes.into_iter().enumerate() {
            let offset = i as u16
                * if copy.format == CopyFormat::OneBppDouble {
                    2
                } else {
                    1
                };
            self.write(copy.destination + offset, byte)?;
            if copy.format == CopyFormat::OneBppDouble {
                self.write(copy.destination + offset + 1, byte)?;
            }
        }
        Ok(())
    }
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, RestoreError> {
        if bytes.len() != 0x2000 {
            return Err(RestoreError::InvalidPatternMemory);
        }
        Ok(Self { bytes })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn read(&self, address: u16) -> Option<u8> {
        self.bytes
            .get(usize::from(address.checked_sub(0x8000)?))
            .copied()
    }
    pub(super) fn write(&mut self, address: u16, value: u8) -> Result<(), RestoreError> {
        let offset = address
            .checked_sub(0x8000)
            .ok_or(RestoreError::InvalidDestination(address))?;
        let byte = self
            .bytes
            .get_mut(usize::from(offset))
            .ok_or(RestoreError::InvalidDestination(address))?;
        *byte = value;
        Ok(())
    }
}

pub trait PatternAssets {
    /// No fallback to a different raw picture or a fabricated zero-filled
    /// source. Unsupported raw IDs must suspend/fail before claiming success.
    fn read_pattern_byte(&self, source: PatternSource, offset: u16) -> Option<u8>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RestoreError {
    MissingSource(PatternSource, u16),
    InvalidPatternMemory,
    SourceOverflow,
    InvalidDestination(u16),
    NoRequest,
    AlreadyArmed,
    AlreadyServiced,
    StaleVBlank,
    NotServiced,
    ServiceInProgress,
    ConflictingRequest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct CopyRequest {
    armed_epoch: u64,
    /// The physical hVBlankCopy[Double]Size port. The ISR clears this before
    /// the first byte write; that does not mean the ISR has returned.
    size: u8,
    serviced_epoch: Option<u64>,
}

/// CopyVideoData / CopyVideoDataDouble foreground and ISR share exactly this
/// request. Even the final size-zero request must finish a later DelayFrame.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VideoCopyWork {
    copy: PatternCopy,
    remaining: u8,
    source_offset: u16,
    destination: u16,
    request: Option<CopyRequest>,
    #[serde(default)]
    isr: Option<(u64, super::vblank_pattern_work::VBlankPatternWork)>,
    returned: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VideoServiceError<E> {
    Protocol(RestoreError),
    Bus(E),
}
impl VideoCopyWork {
    pub fn new(copy: PatternCopy) -> Self {
        Self {
            remaining: copy.tiles,
            source_offset: copy.source_offset,
            destination: copy.destination,
            copy,
            request: None,
            isr: None,
            returned: false,
        }
    }
    pub fn finished(&self) -> bool {
        self.returned
    }
    pub fn requested_tiles(&self) -> Option<u8> {
        self.request.map(|r| r.size)
    }
    /// Device epoch at which the pending request was armed. A later host
    /// update does not itself authorize the transfer.
    pub fn armed_epoch(&self) -> Option<u64> {
        self.request.map(|r| r.armed_epoch)
    }
    pub fn source(&self) -> PatternSource {
        self.copy.source
    }
    pub fn format(&self) -> CopyFormat {
        self.copy.format
    }
    /// Validate a foreground request before moving it into the common ISR.
    /// This is a view of the live request, not a reconstructed scene fixture.
    pub fn pending_vblank_copy(&self, epoch: u64) -> Result<PatternCopy, RestoreError> {
        let request = self.request.ok_or(RestoreError::NoRequest)?;
        if self.isr.is_some() {
            return Err(RestoreError::ServiceInProgress);
        }
        if request.serviced_epoch.is_some() {
            return Err(RestoreError::AlreadyServiced);
        }
        if epoch <= request.armed_epoch {
            return Err(RestoreError::StaleVBlank);
        }
        Ok(PatternCopy {
            source: self.copy.source,
            source_offset: self.source_offset,
            destination: self.destination,
            tiles: request.size,
            format: self.copy.format,
        })
    }
    pub(crate) fn vblank_work(&self) -> Option<&super::vblank_pattern_work::VBlankPatternWork> {
        self.isr.as_ref().map(|(_, child)| child)
    }
    pub fn arm(&mut self, epoch: u64) -> Result<(), RestoreError> {
        if self.request.is_some() || self.returned {
            return Err(RestoreError::AlreadyArmed);
        }
        self.request = Some(CopyRequest {
            armed_epoch: epoch,
            size: self.remaining.min(8),
            serviced_epoch: None,
        });
        Ok(())
    }
    /// Execute one instruction of an accepted VBlank transfer. The owner must
    /// provide the actual device epoch and keep foreground DelayFrame blocked
    /// until the rest of its ISR returns. Clearing size alone is not return.
    #[inline(always)]
    pub fn service_instruction<B: super::pattern_copy_work::PatternInstructionBus>(
        &mut self,
        epoch: u64,
        bus: &mut B,
    ) -> Result<bool, VideoServiceError<B::Error>> {
        let request = self
            .request
            .ok_or(VideoServiceError::Protocol(RestoreError::NoRequest))?;
        if request.serviced_epoch.is_some() {
            return Err(VideoServiceError::Protocol(RestoreError::AlreadyServiced));
        }
        if self.isr.is_none() {
            let pending = self
                .pending_vblank_copy(epoch)
                .map_err(VideoServiceError::Protocol)?;
            self.isr = Some((
                epoch,
                super::vblank_pattern_work::VBlankPatternWork::new(pending),
            ));
        }
        let (accepted_epoch, child) = self.isr.as_mut().unwrap();
        if epoch != *accepted_epoch {
            return Err(VideoServiceError::Protocol(RestoreError::StaleVBlank));
        }
        child.step(bus).map_err(VideoServiceError::Bus)?;
        self.request.as_mut().unwrap().size = child.size_port();
        if !child.finished() {
            return Ok(false);
        }
        self.source_offset = child.source_offset();
        self.destination = child.destination();
        self.request.as_mut().unwrap().serviced_epoch = Some(*accepted_epoch);
        Ok(true)
    }

    /// Called by the common display service at an actual VBlank. Complete all
    /// source validation before any byte write, so a missing asset cannot leave
    /// an acknowledged partial transfer or silently fabricate a picture.
    pub fn service(
        &mut self,
        epoch: u64,
        memory: &mut PatternMemory,
        assets: &impl PatternAssets,
    ) -> Result<u16, RestoreError> {
        let request = self.request.ok_or(RestoreError::NoRequest)?;
        if self.isr.is_some() {
            return Err(RestoreError::ServiceInProgress);
        }
        if epoch <= request.armed_epoch {
            return Err(RestoreError::StaleVBlank);
        }
        if request.serviced_epoch.is_some() {
            return Err(RestoreError::AlreadyServiced);
        }
        if memory.bytes.len() != 0x2000 {
            return Err(RestoreError::InvalidPatternMemory);
        }
        let source_len = u16::from(request.size) * self.copy.format.source_bytes_per_tile();
        let next_source = self
            .source_offset
            .checked_add(source_len)
            .ok_or(RestoreError::SourceOverflow)?;
        let mut source = Vec::with_capacity(usize::from(source_len));
        for i in 0..source_len {
            let offset = self.source_offset + i;
            source.push(
                assets
                    .read_pattern_byte(self.copy.source, offset)
                    .ok_or(RestoreError::MissingSource(self.copy.source, offset))?,
            );
        }
        let written = u16::from(request.size) * 16;
        if written != 0
            && (self.destination < 0x8000
                || self
                    .destination
                    .checked_add(written - 1)
                    .is_none_or(|end| end >= 0xa000))
        {
            return Err(RestoreError::InvalidDestination(self.destination));
        }
        for (i, byte) in source.into_iter().enumerate() {
            let offset = i as u16
                * if self.copy.format == CopyFormat::OneBppDouble {
                    2
                } else {
                    1
                };
            memory.write(self.destination + offset, byte)?;
            if self.copy.format == CopyFormat::OneBppDouble {
                memory.write(self.destination + offset + 1, byte)?;
            }
        }
        self.source_offset = next_source;
        self.destination += written;
        self.request.as_mut().unwrap().size = 0;
        self.request.as_mut().unwrap().serviced_epoch = Some(epoch);
        Ok(written)
    }
    /// DelayFrame has returned after the service. The >=8 path subtracts eight
    /// and loops; a <8 request returns. Hence 8/32/128 require a terminal zero.
    pub fn resume(&mut self) -> Result<(), RestoreError> {
        let request = self.request.ok_or(RestoreError::NoRequest)?;
        if request.serviced_epoch.is_none() {
            return Err(RestoreError::NotServiced);
        }
        self.request = None;
        self.isr = None;
        if self.remaining >= 8 {
            self.remaining -= 8;
        } else {
            self.returned = true;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RestoreInput {
    pub map_id: u8,
    pub pictures: Vec<u8>,
    /// Original one-based set ID, not native SpriteSetId's zero-based value.
    pub outdoor_set: Option<u8>,
    pub font_flags: u8,
    pub player_source: PatternSource,
    pub fly_warp: bool,
}

/// Snapshot only the immutable loader inputs at the current field caller.
/// The live actor images/visibility and map stay on OverworldScreen. Hidden
/// interior slots and distinct raw aliases are deliberately retained.
impl<G: dotzuki_engine::GameData<Tileset = pokered_data::tilesets::TilesetId>>
    super::screen::OverworldScreen<G>
{
    pub fn field_restore_input(&self, font_flags: u8) -> RestoreInput {
        use super::TransportMode;
        let set = pokered_data::sprite_set_data::sprite_set_at(
            self.state.current_map,
            self.state.player.x,
            self.state.player.y,
        );
        let pictures = match set {
            Some(set) => set.sprites().iter().map(|&s| s as u8).collect(),
            None => self.npc_states.iter().map(|n| n.sprite_id).collect(),
        };
        RestoreInput {
            map_id: self.state.current_map as u8,
            pictures,
            outdoor_set: set.map(|set| set as u8 + 1),
            font_flags,
            player_source: match self.state.player.transport {
                TransportMode::Walking => PatternSource::WalkingPlayer,
                TransportMode::Biking => PatternSource::BicyclePlayer,
                TransportMode::Surfing => PatternSource::SurfingPlayer,
            },
            // This entry is StartMenu_Pokemon.flash, not the FLY .canFly
            // continuation; no FLY_WARP may be active on this menu caller.
            fly_warp: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RestoreOperation {
    WhitePalettes,
    WaitVBlanks(u8),
    ClearShadowOam,
    EnableSpriteUpdates,
    SetFontFlags(u8),
    SetSpriteSet(u8),
    /// DisableLCD's LY145 / IF / IE sequence is a child, not an immediate
    /// boolean change. The common device must acknowledge its real return.
    DisableLcd,
    EnableLcd,
    /// FarCopyData2 while the source font bit is clear.
    BulkPatternCopy(PatternCopy),
    /// CopyVideoData / Double: one shared request, resumed after ISR service.
    VideoPatternCopy(PatternCopy),
    UpdateSprites,
    LoadScreenBuffer2,
    DefaultPaletteCommand,
    SetWindowY(u8),
    LoadMapPalettes,
    SetAutoBg(bool),
    RestoreNpcFacing,
    /// Publish source slot assignments to the current map's existing NPC
    /// state. Outdoor assignment must look up raw IDs in its selected set.
    InstallSpriteSlots(Vec<SpriteSlot>),
    SearchIndoorSpriteSlot(u8),
    AdvanceIndoorSpriteSlot(u8),
    PrepareIndoorSpriteTransfer {
        index: u8,
        resume: bool,
    },
    ClearIndoorSpritePictures,
    EnterColdSpriteBank,
    LeaveColdSpriteBank,
    LoadCurrentMapView,
    Return,
}

/// Exact finite call order of StartMenu_Pokemon.flash → .goBackToMap.
/// A child is advanced through `operation`/`child_returned`; no operation is
/// considered complete because of elapsed host frames or a known scene total.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FieldRestoreWork {
    input: RestoreInput,
    operations: Vec<RestoreOperation>,
    next: usize,
}
impl FieldRestoreWork {
    pub fn new(input: RestoreInput) -> Self {
        use RestoreOperation::*;
        let first = SpritePatternPlan::new(&input.pictures, false);
        let mut operations = vec![
            WhitePalettes,
            WaitVBlanks(3),
            ClearShadowOam,
            EnableSpriteUpdates,
            SetFontFlags(input.font_flags & !1),
            SetSpriteSet(0),
            DisableLcd,
            EnterColdSpriteBank,
        ];
        if let Some(set) = input.outdoor_set {
            operations.push(SetSpriteSet(set));
        }
        operations.push(InstallSpriteSlots(first.slots));
        Self::sprite_copies(&input, false, first.copies, &mut operations);
        operations.extend([
            LeaveColdSpriteBank,
            EnableLcd,
            SetFontFlags(input.font_flags),
        ]);
        Self::player_copies(input.player_source, &mut operations);
        operations.extend([
            VideoPatternCopy(PatternCopy {
                source: PatternSource::Font,
                source_offset: 0,
                destination: 0x8800,
                tiles: 128,
                format: CopyFormat::OneBppDouble,
            }),
            UpdateSprites,
            LoadScreenBuffer2,
            VideoPatternCopy(PatternCopy {
                source: PatternSource::TextBox,
                source_offset: 0,
                destination: 0x9600,
                tiles: 32,
                format: CopyFormat::TwoBpp,
            }),
            DefaultPaletteCommand,
            WaitVBlanks(3),
            SetWindowY(144),
            WaitVBlanks(1),
            LoadMapPalettes,
            SetAutoBg(false),
            RestoreNpcFacing,
        ]);
        // InitOutsideMapSprites skips the second reload only if the font bit
        // is clear and the selected set is unchanged after the first reload.
        // Indoor InitMapSprites always rebuilds its raw-slot plan.
        let font_loaded = input.font_flags & 1 != 0;
        if input.outdoor_set.is_none() || font_loaded {
            let second = SpritePatternPlan::new(&input.pictures, font_loaded);
            if let Some(set) = input.outdoor_set {
                operations.push(SetSpriteSet(set));
            }
            operations.push(InstallSpriteSlots(second.slots));
            Self::sprite_copies(&input, font_loaded, second.copies, &mut operations);
        } else {
            // InitOutsideMapSprites.skipLoadingSpriteSet bypasses only the
            // transfers. Its storeVRAMSlotsLoop still remaps all live raw
            // actor IDs, even on an unchanged cached outdoor set.
            operations.push(InstallSpriteSlots(
                SpritePatternPlan::new(&input.pictures, false).slots,
            ));
        }
        operations.push(SetFontFlags(input.font_flags & !1));
        if !input.fly_warp {
            Self::player_copies(input.player_source, &mut operations);
        }
        operations.extend([LoadCurrentMapView, UpdateSprites, Return]);
        Self {
            input,
            operations,
            next: 0,
        }
    }
    fn player_copies(source: PatternSource, operations: &mut Vec<RestoreOperation>) {
        for (source_offset, destination) in [(0, 0x8000), (192, 0x8800)] {
            operations.push(RestoreOperation::VideoPatternCopy(PatternCopy {
                source,
                source_offset,
                destination,
                tiles: 12,
                format: CopyFormat::TwoBpp,
            }));
        }
    }
    fn sprite_copies(
        input: &RestoreInput,
        font_loaded: bool,
        copies: Vec<PatternCopy>,
        operations: &mut Vec<RestoreOperation>,
    ) {
        let wrap = |copy| {
            if font_loaded {
                RestoreOperation::VideoPatternCopy(copy)
            } else {
                RestoreOperation::BulkPatternCopy(copy)
            }
        };
        if input.outdoor_set.is_some() {
            operations.extend(copies.into_iter().map(wrap));
            return;
        }
        for (index, &picture) in input.pictures.iter().enumerate() {
            operations.push(RestoreOperation::SearchIndoorSpriteSlot(index as u8));
            if !input.pictures[..index].contains(&picture) {
                operations.push(RestoreOperation::PrepareIndoorSpriteTransfer {
                    index: index as u8,
                    resume: false,
                });
                for copy in copies
                    .iter()
                    .copied()
                    .filter(|copy| copy.source == PatternSource::Sprite(picture))
                {
                    operations.push(wrap(copy));
                    operations.push(RestoreOperation::PrepareIndoorSpriteTransfer {
                        index: index as u8,
                        resume: true,
                    });
                }
            }
            operations.push(RestoreOperation::AdvanceIndoorSpriteSlot(index as u8));
        }
        if !input.pictures.is_empty() {
            operations.push(RestoreOperation::ClearIndoorSpritePictures);
        }
    }
    pub fn map_id(&self) -> u8 {
        self.input.map_id
    }
    pub fn raw_sprite_pictures(&self) -> &[u8] {
        &self.input.pictures
    }
    /// Stable index for read-only runtime traces; this does not advance work.
    pub fn operation_index(&self) -> usize {
        self.next
    }
    pub fn operation(&self) -> Option<&RestoreOperation> {
        self.operations.get(self.next)
    }
    /// Called only after the existing display service's active child has
    /// returned. Source callbacks that wait on a transfer cannot call this
    /// before VideoCopyWork::finished, including its final zero-size wait.
    pub fn child_returned(&mut self) {
        if self.next < self.operations.len() {
            self.next += 1;
        }
    }
    pub fn finished(&self) -> bool {
        self.next == self.operations.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bytes;
    impl PatternAssets for Bytes {
        fn read_pattern_byte(&self, _: PatternSource, offset: u16) -> Option<u8> {
            Some((offset as u8).wrapping_mul(7).wrapping_add(3))
        }
    }
    fn copy(tiles: u8, format: CopyFormat) -> PatternCopy {
        PatternCopy {
            source: PatternSource::Font,
            source_offset: 0,
            destination: 0x8800,
            tiles,
            format,
        }
    }

    #[test]
    fn observed_flash_transfers_keep_the_zero_size_final_wait() {
        // Retail Red 1099 hooks: 12 tiles take two waits; font128 takes17;
        // textbox32 takes5, including .done C=0 -> DelayFrame.
        for (tiles, format, waits) in [
            (12, CopyFormat::TwoBpp, 2),
            (128, CopyFormat::OneBppDouble, 17),
            (32, CopyFormat::TwoBpp, 5),
            (8, CopyFormat::TwoBpp, 2),
        ] {
            let mut work = VideoCopyWork::new(copy(tiles, format));
            let mut memory = PatternMemory::default();
            let mut epoch = 0;
            let mut bytes = 0;
            while !work.finished() {
                work.arm(epoch).unwrap();
                assert_eq!(work.resume(), Err(RestoreError::NotServiced));
                assert_eq!(
                    work.service(epoch, &mut memory, &Bytes),
                    Err(RestoreError::StaleVBlank)
                );
                epoch += 1;
                bytes += work.service(epoch, &mut memory, &Bytes).unwrap();
                assert!(!work.finished());
                assert_eq!(
                    work.service(epoch, &mut memory, &Bytes),
                    Err(RestoreError::AlreadyServiced)
                );
                work.resume().unwrap();
            }
            assert_eq!(epoch, waits);
            assert_eq!(bytes, u16::from(tiles) * 16);
            for i in 0..u16::from(tiles) * 16 {
                let source_offset = i / if format == CopyFormat::OneBppDouble {
                    2
                } else {
                    1
                };
                assert_eq!(
                    memory.read(0x8800 + i),
                    Bytes.read_pattern_byte(PatternSource::Font, source_offset)
                );
            }
        }
    }

    #[test]
    fn missing_resource_does_not_acknowledge_or_partially_copy() {
        struct Truncated;
        impl PatternAssets for Truncated {
            fn read_pattern_byte(&self, _: PatternSource, offset: u16) -> Option<u8> {
                (offset < 7).then_some(0x77)
            }
        }
        let mut work = VideoCopyWork::new(copy(12, CopyFormat::TwoBpp));
        let mut memory = PatternMemory::default();
        let before = memory.clone();
        work.arm(42).unwrap();
        assert_eq!(
            work.service(43, &mut memory, &Truncated),
            Err(RestoreError::MissingSource(PatternSource::Font, 7))
        );
        assert_eq!(memory, before);
        assert_eq!(work.resume(), Err(RestoreError::NotServiced));
        assert_eq!(work.requested_tiles(), Some(8));
        assert_eq!(
            memory.copy_lcd_disabled(copy(12, CopyFormat::TwoBpp), &Truncated),
            Err(RestoreError::MissingSource(PatternSource::Font, 7))
        );
        assert_eq!(memory, before);
        let mut invalid = PatternMemory { bytes: vec![0; 10] };
        let invalid_before = invalid.clone();
        assert_eq!(
            invalid.copy_lcd_disabled(copy(12, CopyFormat::TwoBpp), &Bytes),
            Err(RestoreError::InvalidPatternMemory)
        );
        assert_eq!(invalid, invalid_before);
    }

    #[test]
    fn snapshot_preserves_pending_request_without_inventing_completion() {
        let mut work = VideoCopyWork::new(copy(8, CopyFormat::TwoBpp));
        let mut memory = PatternMemory::default();
        work.arm(5).unwrap();
        let bytes = serde_json::to_vec(&work).unwrap();
        let mut restored: VideoCopyWork = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(work, restored);
        assert_eq!(restored.resume(), Err(RestoreError::NotServiced));
        restored.service(6, &mut memory, &Bytes).unwrap();
        restored.resume().unwrap();
        assert!(!restored.finished());
        restored.arm(6).unwrap();
        assert_eq!(restored.requested_tiles(), Some(0));
        restored.service(7, &mut memory, &Bytes).unwrap();
        restored.resume().unwrap();
        assert!(restored.finished());
    }

    fn input() -> RestoreInput {
        RestoreInput {
            map_id: 82,
            pictures: vec![1, 2, 1, 0x3d],
            outdoor_set: None,
            font_flags: 1,
            player_source: PatternSource::WalkingPlayer,
            fly_warp: false,
        }
    }

    #[test]
    fn second_sprite_reload_is_retained_after_whiteout_and_map_palette() {
        let mut work = FieldRestoreWork::new(input());
        let mut operations = Vec::new();
        while let Some(op) = work.operation() {
            operations.push(op.clone());
            // Merely observing an operation cannot acknowledge its child.
            assert_eq!(work.operation(), Some(op));
            work.child_returned();
        }
        assert!(work.finished());
        let palette = operations
            .iter()
            .position(|op| matches!(op, RestoreOperation::LoadMapPalettes))
            .unwrap();
        let after = &operations[palette + 1..];
        // Distinct raw moving pictures1/2 are reloaded, duplicate1 is not;
        // the still sprite has no upper walking half under BIT_FONT_LOADED.
        let upper: Vec<_> = after
            .iter()
            .filter_map(|op| match op {
                RestoreOperation::VideoPatternCopy(c)
                    if matches!(c.source, PatternSource::Sprite(_)) =>
                {
                    Some(c.source)
                }
                _ => None,
            })
            .collect();
        assert_eq!(upper, [PatternSource::Sprite(1), PatternSource::Sprite(2)]);
        assert_eq!(
            after
                .iter()
                .filter(|op| matches!(
                    op,
                    RestoreOperation::VideoPatternCopy(PatternCopy {
                        source: PatternSource::WalkingPlayer,
                        ..
                    })
                ))
                .count(),
            2
        );
        assert!(matches!(operations.last(), Some(RestoreOperation::Return)));
    }

    #[test]
    fn outdoor_cache_guard_and_transport_use_actual_loader_inputs() {
        let mut raw = input();
        raw.outdoor_set = Some(3);
        raw.font_flags = 0;
        raw.player_source = PatternSource::BicyclePlayer;
        let work = FieldRestoreWork::new(raw);
        let palette = work
            .operations
            .iter()
            .position(|op| matches!(op, RestoreOperation::LoadMapPalettes))
            .unwrap();
        assert!(!work.operations[palette..].iter().any(|op| matches!(
            op,
            RestoreOperation::VideoPatternCopy(PatternCopy {
                source: PatternSource::Sprite(_),
                ..
            }) | RestoreOperation::BulkPatternCopy(_)
        )));
        assert_eq!(
            work.operations
                .iter()
                .filter(|op| matches!(
                    op,
                    RestoreOperation::VideoPatternCopy(PatternCopy {
                        source: PatternSource::BicyclePlayer,
                        ..
                    })
                ))
                .count(),
            4
        );
        assert_eq!(
            work.operations
                .iter()
                .filter(|op| matches!(op, RestoreOperation::InstallSpriteSlots(_)))
                .count(),
            2
        );
    }
}

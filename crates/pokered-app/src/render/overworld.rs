use crate::alloc_prelude::*;
use dotzuki_engine::overworld::types::TransportMode;
use dotzuki_engine::tileset::TilesetProvider;
use dotzuki_renderer::transition::{FadePalette, FADE_PALETTES};
use pokered_core::data::blockset_data;
use pokered_core::data::map_data_loader::{get_block_data, get_map_json, resolve_map_id};
use pokered_core::data::maps::MapId;
use pokered_core::data::sprites::SpriteId;
use pokered_core::data::tileset_data;
use pokered_core::overworld::presentation::{
    npc_walk_anim_phase, npc_walk_pixel_offset, ANIM_FLOWER_TILE, ANIM_WATER_TILE,
    SHIP_DEPARTURE_PUFF_START_SCREEN_X, SHIP_DEPARTURE_SMOKESTACK_TILE_X,
};
use pokered_core::overworld::screen::{WarpFadeState, WARP_FADE_DELAY, WARP_FADE_IN_FRAMES};
use pokered_core::overworld::{Direction, MovementState, OverworldScreen};
use pokered_data::impl_traits::PokemonTilesetData;
use pokered_data::map_json::MapJson;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::palette::{GbColor, Palette, GRAYSCALE_PALETTE};
use pokered_renderer::resource::{AssetCategory, ResourceManager};
#[cfg(test)]
use pokered_renderer::RenderConfig;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use pokered_data::ui_layout::schema::{DIALOG_DEFAULT_LAYOUT, YES_NO_DEFAULT_LAYOUT};
use pokered_renderer::tile::{Tile, TileSet};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{menus, Ui};

use super::apply_gb_palette;
use super::blit_single_tile_flipped;

fn blit_tile_clipped(
    fb: &mut FrameBuffer,
    tileset: &TileSet,
    tile_idx: usize,
    x: i32,
    y: i32,
    palette: &Palette,
) {
    if tile_idx >= tileset.len() {
        return;
    }
    if x + TILE_SIZE as i32 <= 0
        || x >= fb.width() as i32
        || y + TILE_SIZE as i32 <= 0
        || y >= fb.height() as i32
    {
        return;
    }
    fb.blit_gb_tile(x, y, tileset.get(tile_idx), palette, true, false, false);
}

fn blit_tile_clipped_flipped(
    fb: &mut FrameBuffer,
    tileset: &TileSet,
    tile_idx: usize,
    x: i32,
    y: i32,
    palette: &Palette,
    flip_horizontal: bool,
) {
    if tile_idx >= tileset.len() {
        return;
    }
    if x + TILE_SIZE as i32 <= 0
        || x >= fb.width() as i32
        || y + TILE_SIZE as i32 <= 0
        || y >= fb.height() as i32
    {
        return;
    }
    fb.blit_gb_tile(
        x,
        y,
        tileset.get(tile_idx),
        palette,
        true,
        flip_horizontal,
        false,
    );
}

#[inline]
fn blit_priority_bg_tile(fb: &mut FrameBuffer, tile: &Tile, x: i32, y: i32) {
    fb.blit_gb_tile_indices(x, y, tile, true, false, false);
}

/// Script-driven entry overlay (`showPokedexEntry`): resolve the scene species
/// token and draw the real dex data (`pokered_data::pokedex`, ported from
/// `data/pokemon/dex_entries.asm`) via the shared entry renderer. Previews
/// (starter balls, gift mons, fossils) show the full entry, as the previous
/// hardcoded starter previews did.
fn draw_pokedex_entry(
    species: &str,
    page: usize,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    is_zh: bool,
) -> usize {
    match pokered_data::species::Species::from_scene_name(species) {
        Some(sp) => super::pokedex::draw_entry_for_species(sp, page, true, is_zh, res, fb),
        None => {
            fb.clear(Rgba::WHITE);
            let t = TILE_SIZE;
            draw_text(species, t, t, Rgba::BLACK, fb);
            draw_text("No data.", t, t * 3, Rgba::BLACK, fb);
            1
        }
    }
}

fn resolve_block_with_connections(
    map_json: Option<&MapJson>,
    map_w: u8,
    map_h: u8,
    blk: &[u8],
    border_block: u8,
    bx: i32,
    by: i32,
) -> u8 {
    if bx >= 0 && by >= 0 && (bx as u8) < map_w && (by as u8) < map_h && !blk.is_empty() {
        // The original north/south underground path declares 24 rows but
        // ships only 23. The viewport margin can sample that absent row.
        return blk
            .get(by as usize * map_w as usize + bx as usize)
            .copied()
            .unwrap_or(border_block);
    }

    let map_json = match map_json {
        Some(j) => j,
        None => return border_block,
    };
    let conns = &map_json.connections;

    if by < 0 {
        if let Some(conn) = conns.north.as_ref() {
            if let Some(target) = resolve_map_id(&conn.target_map) {
                let (tw, th) = target.dimensions();
                let target_blk = get_block_data(target);
                let target_bx = bx - conn.offset as i32;
                let target_by = th as i32 + by;
                if target_bx >= 0
                    && (target_bx as u8) < tw
                    && target_by >= 0
                    && (target_by as u8) < th
                    && !target_blk.is_empty()
                {
                    return target_blk
                        .get(target_by as usize * tw as usize + target_bx as usize)
                        .copied()
                        .unwrap_or(border_block);
                }
            }
        }
        return border_block;
    }

    if by >= map_h as i32 {
        if let Some(conn) = conns.south.as_ref() {
            if let Some(target) = resolve_map_id(&conn.target_map) {
                let (tw, th) = target.dimensions();
                let target_blk = get_block_data(target);
                let target_bx = bx - conn.offset as i32;
                let target_by = by - map_h as i32;
                if target_bx >= 0
                    && (target_bx as u8) < tw
                    && target_by >= 0
                    && (target_by as u8) < th
                    && !target_blk.is_empty()
                {
                    let idx = target_by as usize * tw as usize + target_bx as usize;
                    if idx < target_blk.len() {
                        return target_blk[idx];
                    }
                }
            }
        }
        return border_block;
    }

    if bx < 0 {
        if let Some(conn) = conns.west.as_ref() {
            if let Some(target) = resolve_map_id(&conn.target_map) {
                let (tw, th) = target.dimensions();
                let target_blk = get_block_data(target);
                let target_bx = tw as i32 + bx;
                let target_by = by - conn.offset as i32;
                if target_bx >= 0
                    && (target_bx as u8) < tw
                    && target_by >= 0
                    && (target_by as u8) < th
                    && !target_blk.is_empty()
                {
                    return target_blk
                        .get(target_by as usize * tw as usize + target_bx as usize)
                        .copied()
                        .unwrap_or(border_block);
                }
            }
        }
        return border_block;
    }

    if bx >= map_w as i32 {
        if let Some(conn) = conns.east.as_ref() {
            if let Some(target) = resolve_map_id(&conn.target_map) {
                let (tw, th) = target.dimensions();
                let target_blk = get_block_data(target);
                let target_bx = bx - map_w as i32;
                let target_by = by - conn.offset as i32;
                if target_bx >= 0
                    && (target_bx as u8) < tw
                    && target_by >= 0
                    && (target_by as u8) < th
                    && !target_blk.is_empty()
                {
                    return target_blk
                        .get(target_by as usize * tw as usize + target_bx as usize)
                        .copied()
                        .unwrap_or(border_block);
                }
            }
        }
        return border_block;
    }

    border_block
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct OverworldBackgroundKey {
    map: u8,
    camera_x: i32,
    camera_y: i32,
    tile_anim_kind: u8,
    water_shift: i8,
    flower_frame: Option<u8>,
    map_hash: u32,
}

impl OverworldBackgroundKey {
    fn same_scene(self, other: Self) -> bool {
        self.map == other.map
            && self.tile_anim_kind == other.tile_anim_kind
            && self.water_shift == other.water_shift
            && self.flower_frame == other.flower_frame
            && self.map_hash == other.map_hash
    }
}

#[derive(Clone, Copy)]
enum BackgroundDamage {
    None,
    Full,
    Scrolled { dx: i32, dy: i32 },
}

type ScrollIndexedPixels<'a> = dyn FnMut(&mut [u8], usize, usize, i32, i32, u8) + 'a;

impl BackgroundDamage {
    fn intersects_tile(self, x: i32, y: i32, width: i32, height: i32) -> bool {
        match self {
            Self::None => false,
            Self::Full => true,
            Self::Scrolled { dx, dy } => {
                let tile_right = x + TILE_SIZE as i32;
                let tile_bottom = y + TILE_SIZE as i32;
                (dx > 0 && x < dx && tile_right > 0)
                    || (dx < 0 && x < width && tile_right > width + dx)
                    || (dy > 0 && y < dy && tile_bottom > 0)
                    || (dy < 0 && y < height && tile_bottom > height + dy)
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct FrameDamageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

const FOREGROUND_PATCH_SIDE: usize = (TILE_SIZE * 2) as usize;
const FOREGROUND_PATCH_PIXELS: usize = FOREGROUND_PATCH_SIDE * FOREGROUND_PATCH_SIDE;

struct ForegroundPatch {
    rect: FrameDamageRect,
    pixels: [u8; FOREGROUND_PATCH_PIXELS],
}

impl ForegroundPatch {
    fn capture(source: &FrameBuffer, rect: FrameDamageRect) -> Self {
        assert!(rect.width as usize <= FOREGROUND_PATCH_SIDE);
        assert!(rect.height as usize <= FOREGROUND_PATCH_SIDE);
        let mut patch = Self {
            rect,
            pixels: [0; FOREGROUND_PATCH_PIXELS],
        };
        let width = rect.width as usize;
        let height = rect.height as usize;

        #[cfg(all(target_os = "none", target_arch = "arm"))]
        for row in 0..height {
            let source_offset = (rect.y as usize + row) * source.width() as usize + rect.x as usize;
            let patch_offset = row * width;
            patch.pixels[patch_offset..patch_offset + width]
                .copy_from_slice(&source.indices()[source_offset..source_offset + width]);
        }

        #[cfg(not(all(target_os = "none", target_arch = "arm")))]
        for row in 0..height {
            for column in 0..width {
                patch.pixels[row * width + column] = source
                    .indexed()
                    .get_pixel(rect.x + column as u32, rect.y + row as u32)
                    .expect("foreground patch is clipped")
                    as u8;
            }
        }

        patch
    }

    fn restore(&self, destination: &mut FrameBuffer) {
        let width = self.rect.width as usize;
        let height = self.rect.height as usize;

        #[cfg(all(target_os = "none", target_arch = "arm"))]
        for row in 0..height {
            let destination_offset =
                (self.rect.y as usize + row) * destination.width() as usize + self.rect.x as usize;
            let patch_offset = row * width;
            destination.indices_mut()[destination_offset..destination_offset + width]
                .copy_from_slice(&self.pixels[patch_offset..patch_offset + width]);
        }

        #[cfg(not(all(target_os = "none", target_arch = "arm")))]
        for row in 0..height {
            for column in 0..width {
                destination.set_pixel_index(
                    self.rect.x + column as u32,
                    self.rect.y + row as u32,
                    GbColor::from_u8(self.pixels[row * width + column]),
                );
            }
        }
    }
}

impl FrameDamageRect {
    #[inline]
    fn clipped(
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        frame_width: u32,
        frame_height: u32,
    ) -> Option<Self> {
        let frame_width = frame_width as i32;
        let frame_height = frame_height as i32;
        let left = x.clamp(0, frame_width);
        let top = y.clamp(0, frame_height);
        let right = x.saturating_add(width as i32).clamp(0, frame_width);
        let bottom = y.saturating_add(height as i32).clamp(0, frame_height);
        (left < right && top < bottom).then_some(Self {
            x: left as u32,
            y: top as u32,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        })
    }
}

/// Incremental map-layer cache used by the GBA frontend. The visible output
/// keeps the current background in place; this buffer stores the pixels that
/// were underneath the previous player/NPC sprites, so the next frame can
/// restore those small regions before scrolling or drawing new foreground.
pub struct OverworldBackgroundCache {
    width: u32,
    height: u32,
    key: Option<OverworldBackgroundKey>,
    output_key: Option<OverworldBackgroundKey>,
    foreground_patches: Vec<ForegroundPatch>,
    presentation_damage: Vec<FrameDamageRect>,
    partial_present: bool,
}

impl OverworldBackgroundCache {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            key: None,
            output_key: None,
            foreground_patches: Vec::with_capacity(16),
            presentation_damage: Vec::with_capacity(64),
            partial_present: false,
        }
    }

    /// Regions whose pixels changed during the most recent cached draw.
    /// `None` means the frontend must submit the complete framebuffer.
    #[cfg(any(target_os = "none", test))]
    pub fn presentation_damage(&self) -> Option<&[FrameDamageRect]> {
        self.partial_present
            .then_some(self.presentation_damage.as_slice())
    }

    fn invalidate(&mut self) {
        self.key = None;
        self.invalidate_output();
    }

    fn invalidate_output(&mut self) {
        self.output_key = None;
        self.foreground_patches.clear();
    }

    fn require_full_present(&mut self) {
        self.partial_present = false;
        self.presentation_damage.clear();
    }

    fn begin_partial_present(&mut self) {
        self.presentation_damage.clear();
        self.presentation_damage
            .extend(self.foreground_patches.iter().map(|patch| patch.rect));
        self.partial_present = true;
    }

    #[inline(never)]
    fn save_foreground_rect(
        &mut self,
        source: &FrameBuffer,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) {
        if let Some(rect) = FrameDamageRect::clipped(x, y, width, height, self.width, self.height) {
            if self.output_key.is_some() {
                self.foreground_patches
                    .push(ForegroundPatch::capture(source, rect));
            }
            if self.partial_present {
                self.presentation_damage.push(rect);
            }
        }
    }

    fn prepare(
        &mut self,
        output: &mut FrameBuffer,
        key: OverworldBackgroundKey,
        scroll_pixels: Option<&mut ScrollIndexedPixels<'_>>,
    ) -> BackgroundDamage {
        output.reset_palette();
        let Some(previous) = self.key else {
            output.clear(Rgba::WHITE);
            return BackgroundDamage::Full;
        };
        if !previous.same_scene(key) {
            output.clear(Rgba::WHITE);
            return BackgroundDamage::Full;
        }

        if self.output_key != Some(previous) {
            output.clear(Rgba::WHITE);
            return BackgroundDamage::Full;
        }
        restore_foreground_regions(output, self);

        let dx = previous.camera_x - key.camera_x;
        let dy = previous.camera_y - key.camera_y;
        if dx == 0 && dy == 0 {
            return BackgroundDamage::None;
        }
        if dx.unsigned_abs() >= output.width() || dy.unsigned_abs() >= output.height() {
            output.clear(Rgba::WHITE);
            return BackgroundDamage::Full;
        }

        #[cfg(all(target_os = "none", target_arch = "arm"))]
        if let Some(scroll_pixels) = scroll_pixels {
            let width = output.width() as usize;
            let height = output.height() as usize;
            scroll_pixels(
                output.indices_mut(),
                width,
                height,
                dx,
                dy,
                GbColor::White as u8,
            );
        } else {
            output.scroll_indices(dx, dy, GbColor::White);
        }
        #[cfg(not(all(target_os = "none", target_arch = "arm")))]
        {
            let _ = scroll_pixels;
            output.scroll_indices(dx, dy, GbColor::White);
        }
        BackgroundDamage::Scrolled { dx, dy }
    }
}

#[inline(never)]
fn restore_foreground_regions(fb: &mut FrameBuffer, cache: &OverworldBackgroundCache) {
    // Later sprites may overlap earlier ones, so their saved underlay can
    // contain pixels from an earlier foreground draw. Undo in reverse order.
    for patch in cache.foreground_patches.iter().rev() {
        patch.restore(fb);
    }
    // A prior dark-cave frame may have changed only the display palette.
    // Foreground is always drawn against the base palette before the current
    // effect is applied.
    fb.reset_palette();
}

/// Keep the partial restore path deliberately narrower than the renderer's
/// full feature set. These states add overlays, move sprites outside their
/// ordinary 16×16 bounds, or temporarily own the whole framebuffer.
fn can_reuse_composited_frame(screen: &OverworldScreen) -> bool {
    screen.naming_flash_frames == 0
        && screen.pending_naming_screen.is_none()
        && screen.pending_party_select.is_none()
        && screen.pending_pokedex_entry.is_none()
        && screen.pending_dialogue.is_none()
        && screen.cut_retained_dialogue.is_none()
        && screen.pending_choice.is_none()
        && screen.pending_emotion_bubble.is_none()
        && screen.pending_healing_machine.is_none()
        && screen.connection_npc_preview.is_none()
        && screen.ledge_jump.is_none()
        && screen.field_move_step.is_none()
        && screen.field_move_restore.is_none()
        && screen.cut_anim.is_none()
        && screen.elevator_shake.is_none()
        && screen.teleport_spin.is_none()
        && screen.fly_departure.is_none()
        && screen.enter_map_anim.is_none()
        && screen.enter_map_fly_anim.is_none()
        && !screen.pending_fly_arrival
        && screen.fly_arrival_delay_frames == 0
        && screen.fishing_anim.is_none()
        && screen.ship_departure.is_none()
        && screen.flash_lit_frames == 0
        && !screen.boulder_dust.is_active()
        && matches!(screen.warp_fade_state, WarpFadeState::Idle)
}

#[allow(clippy::too_many_arguments)]
fn draw_background_tiles(
    fb: &mut FrameBuffer,
    damage: BackgroundDamage,
    ts: &TileSet,
    flower_ts: Option<&TileSet>,
    shifted_water: Option<&Tile>,
    tile_start_tx: i32,
    tile_start_ty: i32,
    tiles_w: i32,
    tiles_h: i32,
    camera_x: i32,
    camera_y: i32,
    departure_active: bool,
    map_json: Option<&MapJson>,
    map_w: u8,
    map_h: u8,
    blk: &[u8],
    border_block: u8,
    blockset: &[u8],
    shake_offset_y: i32,
) {
    if matches!(damage, BackgroundDamage::None) {
        return;
    }
    let width = fb.width() as i32;
    let height = fb.height() as i32;
    let tile_span = |start: i32, end: i32, camera: i32, count: i32| {
        let first = (start + camera).div_euclid(TILE_SIZE as i32).clamp(0, count);
        let end = ((end - 1 + camera).div_euclid(TILE_SIZE as i32) + 1).clamp(0, count);
        first..end
    };
    let visible_x = tile_span(0, width, camera_x, tiles_w);
    let visible_y = tile_span(0, height, camera_y, tiles_h);
    let (tile_x, tile_y, filter_damage) = match damage {
        BackgroundDamage::Scrolled { dx, dy } if dx != 0 && dy == 0 => {
            let dirty_x = if dx > 0 { 0..dx } else { width + dx..width };
            (
                tile_span(dirty_x.start, dirty_x.end, camera_x, tiles_w),
                visible_y,
                false,
            )
        }
        BackgroundDamage::Scrolled { dx, dy } if dx == 0 && dy != 0 => {
            let dirty_y = if dy > 0 { 0..dy } else { height + dy..height };
            (
                visible_x,
                tile_span(dirty_y.start, dirty_y.end, camera_y, tiles_h),
                false,
            )
        }
        _ => (visible_x, visible_y, true),
    };
    for ty in tile_y {
        let screen_y = ty * TILE_SIZE as i32 - camera_y;
        if screen_y + TILE_SIZE as i32 <= 0 || screen_y >= height {
            continue;
        }
        let mut last_block: Option<(i32, i32, u8)> = None;
        for tx in tile_x.clone() {
            let screen_x = tx * TILE_SIZE as i32 - camera_x;
            if screen_x + TILE_SIZE as i32 <= 0 || screen_x >= width {
                continue;
            }
            if filter_damage && !damage.intersects_tile(screen_x, screen_y, width, height) {
                continue;
            }
            let mut world_tx = tile_start_tx + tx;
            let world_ty = tile_start_ty + ty;

            if departure_active {
                world_tx = world_tx.rem_euclid(map_w as i32 * 4);
            }

            let bx = world_tx.div_euclid(4);
            let mut by = world_ty.div_euclid(4);
            if shake_offset_y != 0 {
                by = by.rem_euclid(map_h as i32);
            }
            let sub_x = world_tx.rem_euclid(4) as usize;
            let sub_y = world_ty.rem_euclid(4) as usize;

            let block_id = match last_block {
                Some((last_bx, last_by, block_id)) if last_bx == bx && last_by == by => block_id,
                _ => {
                    let block_id = resolve_block_with_connections(
                        map_json,
                        map_w,
                        map_h,
                        blk,
                        border_block,
                        bx,
                        by,
                    );
                    last_block = Some((bx, by, block_id));
                    block_id
                }
            };

            let block_offset = block_id as usize * blockset_data::BLOCK_SIZE;
            let tile_idx = blockset
                .get(block_offset + sub_y * 4 + sub_x)
                .copied()
                .map(usize::from)
                .unwrap_or(0);

            let tile = if tile_idx == ANIM_FLOWER_TILE as usize {
                flower_ts.map_or_else(|| ts.get(tile_idx), |fts| fts.get(0))
            } else if tile_idx == ANIM_WATER_TILE as usize {
                shifted_water.unwrap_or_else(|| ts.get(tile_idx))
            } else {
                ts.get(tile_idx)
            };
            fb.blit_gb_tile_indices(screen_x, screen_y, tile, false, false, false);
        }
    }
}

fn background_map_hash(screen: &OverworldScreen) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    let Some(map) = screen.map_data.as_ref() else {
        return hash;
    };
    for byte in [map.width, map.height]
        .into_iter()
        .chain(map.blocks.iter().copied())
    {
        hash = (hash ^ byte as u32).wrapping_mul(0x0100_0193);
    }
    hash
}

pub fn draw_overworld(
    screen: &mut OverworldScreen,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    language: pokered_core::game_state::Lang,
) {
    draw_overworld_impl(screen, res, fb, language, None, None, None);
}

pub(crate) fn draw_overworld_cached(
    screen: &mut OverworldScreen,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    language: pokered_core::game_state::Lang,
    cache: &mut OverworldBackgroundCache,
) {
    draw_overworld_impl(screen, res, fb, language, Some(cache), None, None);
}

pub(crate) fn draw_overworld_cached_with(
    screen: &mut OverworldScreen,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    language: pokered_core::game_state::Lang,
    cache: &mut OverworldBackgroundCache,
    scroll_background: &mut ScrollIndexedPixels<'_>,
    reuse_composited: bool,
) {
    draw_overworld_impl(
        screen,
        res,
        fb,
        language,
        Some(cache),
        Some(scroll_background),
        Some(reuse_composited),
    );
}

fn draw_overworld_impl(
    screen: &mut OverworldScreen,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    language: pokered_core::game_state::Lang,
    mut background_cache: Option<&mut OverworldBackgroundCache>,
    mut scroll_background: Option<&mut ScrollIndexedPixels<'_>>,
    reuse_composited_hint: Option<bool>,
) {
    if let Some(cache) = background_cache.as_deref_mut() {
        cache.require_full_present();
    }
    let owns_full_screen = screen.naming_flash_frames > 0
        || screen.pending_naming_screen.is_some()
        || screen.pending_party_select.is_some();
    if background_cache.is_none() || owns_full_screen || res.is_none() {
        fb.clear(Rgba::WHITE);
    }
    if owns_full_screen || res.is_none() {
        if let Some(cache) = background_cache.as_deref_mut() {
            cache.invalidate();
        }
    }

    // Naming screen open/submit white flash (GBPalWhiteOutWithDelay3).
    if screen.naming_flash_frames > 0 {
        return;
    }

    if let Some(ref naming) = screen.pending_naming_screen {
        super::draw_naming_screen(naming, fb, language);
        return;
    }

    if let Some(ref sel) = screen.pending_party_select {
        super::draw_party_screen(
            sel.screen(),
            res.as_mut(),
            screen.frame_counter as u64,
            fb,
            language,
        );
        return;
    }

    // Sprite palette: color 0 is transparent (matches Game Boy OBP0/OBP1 behavior).
    let sprite_pal = Palette::new(&[
        Rgba::TRANSPARENT,
        Rgba::rgb(0xAA, 0xAA, 0xAA),
        Rgba::rgb(0x55, 0x55, 0x55),
        Rgba::rgb(0x00, 0x00, 0x00),
    ]);

    let player_tx = screen.state.player.x as i32 * 2;
    let player_ty = screen.state.player.y as i32 * 2;
    let screen_center_tx = 9_i32;
    let screen_center_ty = 8_i32;
    let view_origin_tx = player_tx - screen_center_tx;
    let view_origin_ty = player_ty - screen_center_ty;

    // Sub-pixel viewport offset: scrolls the world smoothly during player walking.
    // Original GB uses SCX/SCY registers to scroll the background 2px/frame.
    let (view_sub_x, view_sub_y) = if let Some(jump) = screen.ledge_jump {
        jump.camera_residual_px()
    } else if let Some(step) = screen.field_move_step {
        step.camera_residual_px()
    } else if screen.state.player.movement_state == MovementState::Walking {
        let elapsed = (8u8.saturating_sub(screen.state.walk_counter)) as i32;
        let px = elapsed * 2;
        match screen.state.player.facing {
            Direction::Down => (0i32, px),
            Direction::Up => (0, -px),
            Direction::Left => (-px, 0),
            Direction::Right => (px, 0),
        }
    } else {
        (0, 0)
    };

    if let Some(ref mut rm) = res {
        let current_map: MapId = screen.state.current_map;
        let map_json = get_map_json(current_map);
        let tileset_id = map_json
            .and_then(|j| PokemonTilesetData.tileset_by_name(&j.header.tileset))
            .unwrap_or_else(|| PokemonTilesetData.tileset_by_id(0).unwrap());
        let border_block = map_json.map(|j| j.header.border_block).unwrap_or(0);
        let tileset_name = tileset_id.tileset_name();

        // UpdateMovingBgTiles (home/vcopy.asm): the water tile ($14) rotates
        // one pixel per animation update; the flower tile ($03) is replaced by
        // the current flower frame (flower1/2/3) on WATER_FLOWER tilesets.
        let tile_anim_kind = screen.tile_anim.kind();
        let water_shift = screen.tile_anim.water_shift() as i32;
        let flower_ts =
            if tile_anim_kind == pokered_core::overworld::presentation::TileAnimKind::WaterFlower {
                screen.tile_anim.flower_frame().and_then(|f| {
                    rm.load_asset(AssetCategory::Tileset, &format!("flower/flower{}.png", f))
                        .ok()
                        .map(|c| c.tileset.clone())
                })
            } else {
                None
            };

        // ShakeElevator: the BG scrolls ±1px vertically (hSCY); sprites stay.
        let shake_offset_y = screen.elevator_shake.as_ref().map_or(0, |s| s.offset_y());

        // S.S. Anne departure (VermilionDockSSAnneLeavesScript): the view
        // scrolls east up to 16 tiles while the ship sails away — the map
        // content slides left (wMapViewVRAMPointer += 2 per iteration +
        // the LY-split SCX ramp).
        let departure_scroll = screen.ship_departure.as_ref().map_or(0, |d| d.scroll_px());
        let departure_active = screen.ship_departure.is_some();

        let (map_w, map_h) = current_map.dimensions();
        // Read the LIVE block grid: scripted tile swaps (CUT trees, gym gates,
        // hideout doors) mutate screen.map_data — the static .blk never
        // changes, so reading it kept rendering pre-swap trees (audit:
        // gym-tree-after identical SHA1 to gym-tree-before).
        let blk: &[u8] = match screen.map_data.as_ref() {
            Some(live) if live.width == map_w && live.height == map_h => &live.blocks,
            _ => get_block_data(current_map),
        };
        let blockset = blockset_data::blockset_for_tileset(tileset_id);

        if let Ok(cached) = rm.load_tileset(tileset_name) {
            let ts = &cached.tileset;

            // ── Visible background tiles ────────────────────────────────
            // Render complete 8×8 GB tiles directly. Index (0,0) corresponds to world tile
            // (tile_start_tx, tile_start_ty). The camera is offset by `margin * TILE_SIZE`
            // so that the direct blitter computes matching tile indices. This
            // avoids rebuilding a generic Tilemap and resolving the same tile
            // and palette once for every pixel in the viewport.

            let margin = 2i32;
            let tile_start_tx = view_origin_tx - margin;
            let tile_start_ty = view_origin_ty - margin;
            let tiles_w = (fb.width() / TILE_SIZE) as i32 + margin * 2;
            let tiles_h = (fb.height() / TILE_SIZE) as i32 + margin * 2;

            // Camera position: tile index 0 ↔ world tile tile_start_tx.
            // The raster position computes tile_x = (camera_x + screen_x) / TILE_SIZE,
            // so camera_x = margin * TILE_SIZE + view_sub_x yields tile_x = margin at
            // screen_x=0 (when view_sub_x=0), pointing at tilemap column=margin which
            // maps to world tile tile_start_tx + margin = view_origin_tx.
            let camera_x = margin * TILE_SIZE as i32 + view_sub_x + departure_scroll;
            let camera_y = margin * TILE_SIZE as i32 + view_sub_y + shake_offset_y;

            // UpdateMovingBgTiles rotates the single water tile once, then all
            // water cells can use the indexed framebuffer's fast tile blitter.
            let shifted_water = if water_shift != 0 {
                let source = ts.get(ANIM_WATER_TILE as usize);
                let mut shifted = Tile::blank();
                for py in 0..TILE_SIZE as usize {
                    for px in 0..TILE_SIZE as usize {
                        let source_x =
                            (px as i32 - water_shift).rem_euclid(TILE_SIZE as i32) as usize;
                        shifted.pixels[py][px] = source.pixels[py][source_x];
                    }
                }
                Some(shifted)
            } else {
                None
            };

            let cache_allowed = shake_offset_y == 0 && !departure_active;
            if cache_allowed {
                if let Some(cache) = background_cache.as_deref_mut() {
                    let key = OverworldBackgroundKey {
                        map: current_map as u8,
                        camera_x: view_origin_tx * TILE_SIZE as i32 + view_sub_x,
                        camera_y: view_origin_ty * TILE_SIZE as i32 + view_sub_y,
                        tile_anim_kind: tile_anim_kind as u8,
                        water_shift: screen.tile_anim.water_shift(),
                        flower_frame: screen.tile_anim.flower_frame(),
                        map_hash: background_map_hash(screen),
                    };
                    let damage = cache.prepare(fb, key, scroll_background.as_deref_mut());
                    draw_background_tiles(
                        fb,
                        damage,
                        ts,
                        flower_ts.as_ref(),
                        shifted_water.as_ref(),
                        tile_start_tx,
                        tile_start_ty,
                        tiles_w,
                        tiles_h,
                        camera_x,
                        camera_y,
                        departure_active,
                        map_json,
                        map_w,
                        map_h,
                        blk,
                        border_block,
                        blockset,
                        shake_offset_y,
                    );
                    cache.key = Some(key);
                    let reuse_composited =
                        reuse_composited_hint.unwrap_or_else(|| can_reuse_composited_frame(screen));
                    if matches!(damage, BackgroundDamage::None)
                        && reuse_composited
                        && cache.output_key == Some(key)
                    {
                        cache.begin_partial_present();
                    }
                    if reuse_composited {
                        cache.foreground_patches.clear();
                        cache.output_key = Some(key);
                    } else {
                        cache.invalidate_output();
                    }
                } else {
                    draw_background_tiles(
                        fb,
                        BackgroundDamage::Full,
                        ts,
                        flower_ts.as_ref(),
                        shifted_water.as_ref(),
                        tile_start_tx,
                        tile_start_ty,
                        tiles_w,
                        tiles_h,
                        camera_x,
                        camera_y,
                        departure_active,
                        map_json,
                        map_w,
                        map_h,
                        blk,
                        border_block,
                        blockset,
                        shake_offset_y,
                    );
                }
            } else {
                if let Some(cache) = background_cache.as_deref_mut() {
                    cache.invalidate();
                    fb.clear(Rgba::WHITE);
                }
                draw_background_tiles(
                    fb,
                    BackgroundDamage::Full,
                    ts,
                    flower_ts.as_ref(),
                    shifted_water.as_ref(),
                    tile_start_tx,
                    tile_start_ty,
                    tiles_w,
                    tiles_h,
                    camera_x,
                    camera_y,
                    departure_active,
                    map_json,
                    map_w,
                    map_h,
                    blk,
                    border_block,
                    blockset,
                    shake_offset_y,
                );
            }
        } else {
            if let Some(cache) = background_cache.as_deref_mut() {
                cache.invalidate();
            }
            fb.clear(Rgba::WHITE);
        }
        // Player sprite: 16×96 sheet = 6 frames of 16×16
        // Frame layout: DownStand=0, UpStand=1, LeftStand=2, DownWalk=3, UpWalk=4, LeftWalk=5
        // Right uses Left frames with horizontal flip
        // LoadPlayerSpriteGraphics selects RedSprite / RedBikeSprite /
        // SeelSprite for walking / biking / surfing (home/overworld.asm).
        // All three sheets share the facing and animation layout below.
        let player_sprite = match screen.state.player.transport {
            TransportMode::Walking => "red",
            TransportMode::Biking => "red_bike",
            TransportMode::Surfing => "seel",
        };
        // _LeaveMapAnim spin-out (TELEPORT/DIG/ESCAPE ROPE): the facing
        // spins and the sprite rises off the top of the screen.
        let spin = screen.teleport_spin.as_ref();
        // EnterMapAnim spin-in (FLY/TELEPORT/DIG/ESCAPE ROPE/dungeon
        // arrivals): the sprite descends from off the top and spins in
        // place after the fade-in-from-white.
        let enter = screen.enter_map_anim.as_ref();
        // FishingAnim (player_animations.asm:378-469): while the rod is
        // out, the player sprite is swapped to the fishing pose
        // (RedFishingTiles — the bottom two tiles) and the sprite shakes
        // ±1 px vertically on a bite (.ShakePlayerSprite).
        let fishing = screen.fishing_anim.as_ref();
        let player_facing = fishing
            .map(|f| f.facing())
            .or_else(|| enter.map(|s| s.facing()))
            .or_else(|| spin.map(|s| s.facing()))
            .unwrap_or(screen.state.player.facing);
        let fishing_pose = fishing.map_or(false, |f| f.pose_active());
        // Load the optional pose first so the common player sheet can remain
        // borrowed from the resource cache instead of cloning its TileSet.
        let pose_ts = if fishing_pose {
            let asset = match player_facing {
                Direction::Down => "red_fish_front.png",
                Direction::Up => "red_fish_back.png",
                Direction::Left | Direction::Right => "red_fish_side.png",
            };
            rm.load_asset(AssetCategory::Overworld, asset)
                .ok()
                .map(|c| c.tileset.clone())
        } else {
            None
        };
        if let Ok(cached) = rm.load_sprite(player_sprite) {
            let ts = &cached.tileset;

            let spin_y_offset = spin.map_or(0, |s| s.player_y_offset());
            let enter_y_offset = enter.map_or(0, |s| s.player_y_offset());
            let fishing_shake_offset = fishing.map_or(0, |f| f.player_shake_offset());
            let player_visible = spin.map_or(true, |s| s.player_visible());
            let player_visible = enter.map_or(player_visible, |s| s.player_visible());
            // FLY arrival (EnterMapAnim .flyAnimation): the BIRD replaces the
            // player sprite while it glides in; the player reappears when it
            // lands.
            let fly = screen.enter_map_fly_anim.as_ref();
            let fly_player_visible = if let Some(departure) = screen.fly_departure.as_ref() {
                departure.player_visible()
            } else {
                !screen.pending_fly_arrival
                    && screen.fly_arrival_delay_frames == 0
                    && fly.is_none_or(|s| s.is_done())
            };
            let player_visible =
                player_visible && fly_player_visible && screen.field_move_restore.is_none();

            let (frame, flip_h) = if screen.state.player.movement_state == MovementState::Walking
                || screen.state.player.movement_state == MovementState::Jumping
            {
                // 4-frame walk cycle (facings.asm:3-18 + movement.asm:298-320):
                // stand → step → stand → step(mirrored) for Down/Up; Left/
                // Right have no mirror (Right = mirrored Left sheet). The
                // NPC path below uses the same cadence.
                let anim_frame = if screen.state.walk_counter > 6 {
                    0 // 7,8: stand
                } else if screen.state.walk_counter > 4 {
                    1 // 5,6: step
                } else if screen.state.walk_counter > 2 {
                    2 // 3,4: stand
                } else {
                    3 // 1,2: step (mirrored for Down/Up)
                };
                match player_facing {
                    Direction::Down => {
                        if anim_frame == 0 || anim_frame == 2 {
                            (0, false)
                        } else if anim_frame == 1 {
                            (3, false)
                        } else {
                            (3, true)
                        }
                    }
                    Direction::Up => {
                        if anim_frame == 0 || anim_frame == 2 {
                            (1, false)
                        } else if anim_frame == 1 {
                            (4, false)
                        } else {
                            (4, true)
                        }
                    }
                    Direction::Left => {
                        if anim_frame == 0 || anim_frame == 2 {
                            (2, false)
                        } else {
                            (5, false)
                        }
                    }
                    Direction::Right => {
                        if anim_frame == 0 || anim_frame == 2 {
                            (2, true)
                        } else {
                            (5, true)
                        }
                    }
                }
            } else if screen.bump_anim_counter > 0 {
                let walk_frame = (screen.bump_anim_counter / 4) % 2 == 1;
                match player_facing {
                    Direction::Down => (if walk_frame { 3 } else { 0 }, false),
                    Direction::Up => (if walk_frame { 4 } else { 1 }, false),
                    Direction::Left => (if walk_frame { 5 } else { 2 }, false),
                    Direction::Right => (if walk_frame { 5 } else { 2 }, true),
                }
            } else {
                match player_facing {
                    Direction::Down => (0, false),
                    Direction::Up => (1, false),
                    Direction::Left => (2, false),
                    Direction::Right => (2, true),
                }
            };

            let base_tile = frame * 4;
            let tpr = cached.source_size.0 / TILE_SIZE;

            let player_px_x = screen_center_tx as u32 * TILE_SIZE;
            let player_px_y = screen_center_ty as u32 * TILE_SIZE;

            // The player stays centered while the camera traverses both tiles;
            // only the original PlayerJumpingYScreenCoords arc moves the sprite.
            let jump_arc_offset = screen.ledge_jump.map_or(0, |jump| jump.player_y_offset());
            if screen.state.player.movement_state == MovementState::Jumping && jump_arc_offset < 0 {
                let shadow_cx = player_px_x as i32 + 8;
                let shadow_cy = player_px_y as i32 + 15;
                let rx: i32 = 7;
                let ry: i32 = 3;
                let shadow_color = Rgba::rgb(0x55, 0x55, 0x55);
                for dy in -ry..=ry {
                    for dx in -rx..=rx {
                        if dx * dx * ry * ry + dy * dy * rx * rx <= rx * rx * ry * ry {
                            let sx = shadow_cx + dx;
                            let sy = shadow_cy + dy;
                            if sx >= 0
                                && sy >= 0
                                && (sx as u32) < fb.width()
                                && (sy as u32) < fb.height()
                            {
                                fb.set_pixel(sx as u32, sy as u32, shadow_color);
                            }
                        }
                    }
                }
            }

            let draw_x = player_px_x;
            let draw_y = (player_px_y as i32
                + jump_arc_offset
                + spin_y_offset
                + enter_y_offset
                + fishing_shake_offset)
                .max(0) as u32;

            if player_visible {
                if let Some(cache) = background_cache.as_deref_mut() {
                    if cache.output_key.is_some() || cache.partial_present {
                        cache.save_foreground_rect(
                            fb,
                            draw_x as i32,
                            draw_y as i32,
                            TILE_SIZE * 2,
                            TILE_SIZE * 2,
                        );
                    }
                }
            }

            if player_visible {
                for row in 0..2_u32 {
                    for col in 0..2_u32 {
                        let src_col = if flip_h { 1 - col } else { col };
                        let (tile_idx, tile_ts) = match (&pose_ts, row) {
                            // Bottom half of the fishing pose (2 tiles: bottom-
                            // left, bottom-right).
                            (Some(p), 1) => (src_col as usize, p),
                            _ => (
                                base_tile + (row as usize * tpr as usize) + src_col as usize,
                                ts,
                            ),
                        };
                        if tile_idx >= tile_ts.len() {
                            continue;
                        }

                        if flip_h {
                            // The player OBJ palette is identity-mapped for
                            // indices 1..=3, with index zero transparent.
                            // Preserve those indices and skip RGBA palette
                            // conversion on the mirrored hot path.
                            fb.blit_gb_tile_indices(
                                (draw_x + col * TILE_SIZE) as i32,
                                (draw_y + row * TILE_SIZE) as i32,
                                tile_ts.get(tile_idx),
                                true,
                                true,
                                false,
                            );
                        } else {
                            blit_single_tile_flipped(
                                fb,
                                tile_ts,
                                tile_idx,
                                draw_x + col * TILE_SIZE,
                                draw_y + row * TILE_SIZE,
                                &sprite_pal,
                                false,
                            );
                        }
                    }
                }
            }
        }
        // Grass overlay: redraw BG grass tile over the player sprite's bottom
        // half, replicating Game Boy OAM_PRIO behavior where non-zero BG pixels
        // render on top of sprites with the priority bit set.
        if screen.state.player.movement_state != MovementState::Jumping {
            if let Some(grass_id) = tileset_data::get_grass_tile(tileset_id) {
                if let Ok(bg_cached) = rm.load_tileset(tileset_name) {
                    let bg_ts = &bg_cached.tileset;
                    let overlay_x = screen_center_tx as u32 * TILE_SIZE;
                    let overlay_y = screen_center_ty as u32 * TILE_SIZE + TILE_SIZE;
                    for col_off in 0..2i32 {
                        let world_tx = player_tx + col_off;
                        let world_ty = player_ty + 1;
                        let bx = world_tx.div_euclid(4);
                        let by = world_ty.div_euclid(4);
                        let sub_x = world_tx.rem_euclid(4) as usize;
                        let sub_y = world_ty.rem_euclid(4) as usize;
                        let block_id = resolve_block_with_connections(
                            map_json,
                            map_w,
                            map_h,
                            blk,
                            border_block,
                            bx,
                            by,
                        );
                        let block_offset = block_id as usize * blockset_data::BLOCK_SIZE;
                        let bg_tile_idx = blockset
                            .get(block_offset + sub_y * 4 + sub_x)
                            .copied()
                            .map(usize::from)
                            .unwrap_or(0)
                            .min(bg_ts.len().saturating_sub(1));
                        if bg_tile_idx == grass_id as usize {
                            let tile = bg_ts.get(bg_tile_idx);
                            let gx = overlay_x as i32 + col_off * TILE_SIZE as i32;
                            blit_priority_bg_tile(fb, tile, gx, overlay_y as i32);
                        }
                    }
                }
            }
        }
        for npc in &screen.npc_states {
            if screen.field_move_restore.is_some() {
                break;
            }
            if !npc.visible {
                continue;
            }

            let sprite_id = match SpriteId::from_u8(npc.sprite_id) {
                Some(id) => id,
                None => continue,
            };

            let sprite_name = sprite_id.sprite_name();
            if let Ok(cached) = rm.load_sprite(sprite_name) {
                let ts = &cached.tileset;
                let num_frames = (cached.source_size.1 / TILE_SIZE) as usize;

                let npc_facing = npc.facing;

                let (frame, flip_h) = if let Some(sf) = npc.scripted_frame {
                    (sf as usize, false)
                } else if num_frames >= 6 {
                    // AnimFrame 0-3: 0/2=stand, 1=walk, 3=walk+flip — phases
                    // spread evenly over the NPC's 16-frame step (4 each).
                    let anim_frame = npc_walk_anim_phase(npc.walk_counter);

                    match npc_facing {
                        Direction::Down => {
                            if anim_frame == 0 || anim_frame == 2 {
                                (0, false)
                            } else if anim_frame == 1 {
                                (3, false)
                            } else {
                                (3, true)
                            }
                        }
                        Direction::Up => {
                            if anim_frame == 0 || anim_frame == 2 {
                                (1, false)
                            } else if anim_frame == 1 {
                                (4, false)
                            } else {
                                (4, true)
                            }
                        }
                        Direction::Left => {
                            if anim_frame == 0 || anim_frame == 2 {
                                (2, false)
                            } else {
                                (5, false)
                            }
                        }
                        Direction::Right => {
                            if anim_frame == 0 || anim_frame == 2 {
                                (2, true)
                            } else {
                                (5, true)
                            }
                        }
                    }
                } else if num_frames >= 3 {
                    match npc_facing {
                        Direction::Down => (0, false),
                        Direction::Up => (1, false),
                        Direction::Left => (2, false),
                        Direction::Right => (2, true),
                    }
                } else {
                    (0, false)
                };

                let base_tile = frame * 4;
                let tpr = cached.source_size.0 / TILE_SIZE;

                let npc_screen_tx = npc.x as i32 * 2 - view_origin_tx;
                let npc_screen_ty = npc.y as i32 * 2 - view_origin_ty;

                // Smooth pixel interpolation during movement. Classic GB
                // walkers advance 1px/frame over their 16-frame step
                // (16px/tile) — unlike the player's 2px/frame over 8.
                let (walk_dx, walk_dy) = if npc.walk_counter > 0 {
                    let px = npc_walk_pixel_offset(npc.walk_counter);
                    match npc.facing {
                        Direction::Down => (0i32, px),
                        Direction::Up => (0, -px),
                        Direction::Left => (-px, 0),
                        Direction::Right => (px, 0),
                    }
                } else {
                    (0, 0)
                };

                let npc_px_x = npc_screen_tx * TILE_SIZE as i32 + walk_dx - view_sub_x;
                let npc_px_y = npc_screen_ty * TILE_SIZE as i32 + walk_dy - view_sub_y;

                let sprite_size = (TILE_SIZE * 2) as i32;
                if npc_px_x <= -sprite_size
                    || npc_px_x >= fb.width() as i32
                    || npc_px_y <= -sprite_size
                    || npc_px_y >= fb.height() as i32
                {
                    continue;
                }

                if let Some(cache) = background_cache.as_deref_mut() {
                    if cache.output_key.is_some() || cache.partial_present {
                        cache.save_foreground_rect(
                            fb,
                            npc_px_x,
                            npc_px_y,
                            TILE_SIZE * 2,
                            TILE_SIZE * 2,
                        );
                    }
                }

                for row in 0..2_u32 {
                    for col in 0..2_u32 {
                        let src_col = if flip_h { 1 - col } else { col };
                        let tile_idx = base_tile + (row as usize * tpr as usize) + src_col as usize;
                        if tile_idx >= ts.len() {
                            continue;
                        }

                        let tx = npc_px_x + (col * TILE_SIZE) as i32;
                        let ty = npc_px_y + (row * TILE_SIZE) as i32;
                        blit_tile_clipped_flipped(fb, ts, tile_idx, tx, ty, &sprite_pal, flip_h);
                    }
                }
            }
        }
        // Render destination-map NPCs offset into the old viewport during
        // a connection walk, so they scroll into view before the map swap.
        if let Some(ref preview) = screen.connection_npc_preview {
            for npc in &preview.npcs {
                if !npc.visible {
                    continue;
                }
                let sprite_id = match SpriteId::from_u8(npc.sprite_id) {
                    Some(id) => id,
                    None => continue,
                };
                let sprite_name = sprite_id.sprite_name();
                if let Ok(cached) = rm.load_sprite(sprite_name) {
                    let ts = &cached.tileset;
                    let tpr = cached.source_size.0 / TILE_SIZE;
                    let base_tile = 0usize;
                    let npc_screen_tx = (npc.x as i32 + preview.step_offset_x) * 2 - view_origin_tx;
                    let npc_screen_ty = (npc.y as i32 + preview.step_offset_y) * 2 - view_origin_ty;
                    let npc_px_x = npc_screen_tx * TILE_SIZE as i32 - view_sub_x;
                    let npc_px_y = npc_screen_ty * TILE_SIZE as i32 - view_sub_y;

                    let sprite_size = (TILE_SIZE * 2) as i32;
                    if npc_px_x <= -sprite_size
                        || npc_px_x >= fb.width() as i32
                        || npc_px_y <= -sprite_size
                        || npc_px_y >= fb.height() as i32
                    {
                        continue;
                    }

                    for row in 0..2_u32 {
                        for col in 0..2_u32 {
                            let tile_idx = base_tile + (row as usize * tpr as usize) + col as usize;
                            if tile_idx >= ts.len() {
                                continue;
                            }
                            let tx = npc_px_x + (col * TILE_SIZE) as i32;
                            let ty = npc_px_y + (row * TILE_SIZE) as i32;
                            blit_tile_clipped_flipped(fb, ts, tile_idx, tx, ty, &sprite_pal, false);
                        }
                    }
                }
            }
        }

        // The player owns the first OAM entries: the FLY bird must cover NPCs
        // it crosses, even though NPCs are drawn later above.
        // BirdSprite uses the same six-frame sheet as walking sprites:
        // image indexes $8/$9 select LeftStand/LeftWalk (frames 2/5).
        // The departure and arrival coordinate tables contain sprite-state
        // coordinates, not OAM coordinates. Anchor ($40,$3c) at our player
        // position; PrepareOAMData's hardware bias is not a screen offset.
        let bird_pose = screen
            .fly_departure
            .as_ref()
            .and_then(|fly| fly.bird_pose())
            .or_else(|| {
                screen
                    .enter_map_fly_anim
                    .as_ref()
                    .filter(|fly| !fly.is_done())
                    .map(|fly| {
                        let (y, x) = fly.bird_pos();
                        (y, x, fly.flap_frame())
                    })
            });
        if let Some((oy, ox, flap)) = bird_pose {
            if let Ok(bird) = rm.load_sprite("bird") {
                let bts = &bird.tileset;
                let bird_pal = Palette::new(&[
                    Rgba::TRANSPARENT,
                    GRAYSCALE_PALETTE.colors[1],
                    GRAYSCALE_PALETTE.colors[2],
                    GRAYSCALE_PALETTE.colors[3],
                ]);
                let bx = screen_center_tx * TILE_SIZE as i32 + ox as i32 - 0x40;
                let by = screen_center_ty * TILE_SIZE as i32 + oy as i32 - 0x3c;
                let base_tile = [2, 5][flap as usize] * 4;
                for r in 0..2u32 {
                    for c in 0..2u32 {
                        let tile_idx = base_tile + (r * 2 + c) as usize;
                        if tile_idx < bts.len() {
                            blit_tile_clipped(
                                fb,
                                bts,
                                tile_idx,
                                bx + (c * TILE_SIZE) as i32,
                                by + (r * TILE_SIZE) as i32,
                                &bird_pal,
                            );
                        }
                    }
                }
            }
        }

        if let Some(ref bubble) = screen.pending_emotion_bubble {
            let emote_asset = match bubble.emotion.as_str() {
                "exclamation" => "shock",
                "question" => "question",
                "happy" => "happy",
                _ => "shock",
            };
            if let Ok(cached) = rm.load_emote(emote_asset) {
                let ts = &cached.tileset;
                let tpr = cached.source_size.0 / TILE_SIZE;
                if let Some(npc) = screen
                    .npc_states
                    .iter()
                    .find(|n| n.visible && format!("{}", n.npc_index) == bubble.npc_id)
                {
                    let npc_screen_tx = npc.x as i32 * 2 - view_origin_tx;
                    let npc_screen_ty = npc.y as i32 * 2 - view_origin_ty;
                    let (walk_dx, walk_dy) = if npc.walk_counter > 0 {
                        let px = npc_walk_pixel_offset(npc.walk_counter);
                        match npc.facing {
                            Direction::Down => (0i32, px),
                            Direction::Up => (0, -px),
                            Direction::Left => (-px, 0),
                            Direction::Right => (px, 0),
                        }
                    } else {
                        (0, 0)
                    };
                    let npc_px_x = npc_screen_tx * TILE_SIZE as i32 + walk_dx - view_sub_x;
                    let npc_px_y = npc_screen_ty * TILE_SIZE as i32 + walk_dy - view_sub_y;
                    let emote_x = npc_px_x;
                    let emote_y = npc_px_y - TILE_SIZE as i32 * 2;
                    for row in 0..2_u32 {
                        for col in 0..2_u32 {
                            let tile_idx = row as usize * tpr as usize + col as usize;
                            if tile_idx >= ts.len() {
                                continue;
                            }
                            let tx = emote_x + (col * TILE_SIZE) as i32;
                            let ty = emote_y + (row * TILE_SIZE) as i32;
                            blit_tile_clipped(fb, ts, tile_idx, tx, ty, &sprite_pal);
                        }
                    }
                }
            }
        }

        // Fishing rod OAM piece — FishingAnim's wShadowOAMSprite39: a single
        // 8×8 sprite from the 8×24 fishing_rod sheet. `rod_piece` returns the
        // FishingRodOAM offsets (player_animations.asm:471-476) relative to
        // the player sprite's top-left (the original's absolute OAM coords,
        // authored for its bottom-anchored player, re-anchored to this port's
        // centered player at screen (72,64)). Drawn on top of the player/NPCs
        // like OAM sprite 39.
        if let Some(anim) = screen.fishing_anim.as_ref() {
            if anim.rod_visible() {
                let (rod_dx, rod_dy, rod_tile, rod_flip) =
                    pokered_core::overworld::presentation::FishingAnimState::rod_piece(
                        anim.facing(),
                    );
                let rod_x = screen_center_tx as i32 * TILE_SIZE as i32 + rod_dx;
                let rod_y = screen_center_ty as i32 * TILE_SIZE as i32 + rod_dy;
                // The bite shake toggles the rod's OAM Y too
                // (.ShakePlayerSprite, player_animations.asm:413-416).
                let rod_y = rod_y + anim.player_shake_offset();
                if let Ok(cached) = rm.load_asset(AssetCategory::Overworld, "fishing_rod.png") {
                    let rod_ts = &cached.tileset;
                    blit_tile_clipped_flipped(
                        fb,
                        rod_ts,
                        rod_tile as usize,
                        rod_x,
                        rod_y,
                        &sprite_pal,
                        rod_flip,
                    );
                }
            }
        }

        // FishingAnim's "!" bubble — EmotionBubble (emotion_bubbles.asm)
        // shows EXCLAMATION_BUBBLE over the PLAYER for 60 frames on a bite
        // (wEmotionBubbleSpriteIndex 0; only the rod flow uses it here, so
        // only "!" is driven from the animation state).
        if let Some(anim) = screen.fishing_anim.as_ref() {
            if anim.bubble_active() {
                if let Ok(cached) = rm.load_emote("shock") {
                    let ts = &cached.tileset;
                    let tpr = cached.source_size.0 / TILE_SIZE;
                    let emote_x = screen_center_tx as i32 * TILE_SIZE as i32;
                    let emote_y = screen_center_ty as i32 * TILE_SIZE as i32 - TILE_SIZE as i32 * 2;
                    for row in 0..2_u32 {
                        for col in 0..2_u32 {
                            let tile_idx = row as usize * tpr as usize + col as usize;
                            if tile_idx >= ts.len() {
                                continue;
                            }
                            let tx = emote_x + (col * TILE_SIZE) as i32;
                            let ty = emote_y + (row * TILE_SIZE) as i32;
                            blit_tile_clipped(fb, ts, tile_idx, tx, ty, &sprite_pal);
                        }
                    }
                }
            }
        }

        if let Some(ref healing_state) = screen.pending_healing_machine {
            if let Ok(cached) = rm.load_asset(AssetCategory::Overworld, "heal_machine.png") {
                let ts = &cached.tileset;

                // rOBP1=$e0: idx 0→transparent, 1→white, 2→dark gray, 3→black
                let obp1_pal = Palette::new(&[
                    Rgba::TRANSPARENT,
                    Rgba::rgb(0xFF, 0xFF, 0xFF),
                    Rgba::rgb(0x55, 0x55, 0x55),
                    Rgba::rgb(0x00, 0x00, 0x00),
                ]);
                // rOBP1 XOR $28 = $c8: idx 0→transparent, 1→dark gray, 2→white, 3→black
                let obp1_flash = Palette::new(&[
                    Rgba::TRANSPARENT,
                    Rgba::rgb(0x55, 0x55, 0x55),
                    Rgba::rgb(0xFF, 0xFF, 0xFF),
                    Rgba::rgb(0x00, 0x00, 0x00),
                ]);
                let heal_pal = if healing_state.flash_active {
                    &obp1_flash
                } else {
                    &obp1_pal
                };

                // PokeCenterOAMData offsets relative to nurse sprite top-left.
                // Nurse renders at map pos (3,1) as a 16×16 NPC sprite.
                // Original OAM screen positions (player at (3,3), nurse screen=(72,32)):
                //   monitor: (44,20) → delta (-28,-12)
                //   balls: (40,27)(48,27)(40,32)(48,32)(40,37)(48,37)
                const MONITOR_DX: i32 = -20;
                const MONITOR_DY: i32 = -12;
                const BALL_OAM: [(i32, i32, bool); 6] = [
                    (-24, -5, false),
                    (-16, -5, true),
                    (-24, 0, false),
                    (-16, 0, true),
                    (-24, 5, false),
                    (-16, 5, true),
                ];

                let nurse_x = 3_i32;
                let nurse_y = 1_i32;
                let nurse_px_x = (nurse_x * 2 - view_origin_tx) * TILE_SIZE as i32 - view_sub_x;
                let nurse_px_y = (nurse_y * 2 - view_origin_ty) * TILE_SIZE as i32 - view_sub_y;

                if ts.len() > 0 {
                    blit_tile_clipped(
                        fb,
                        ts,
                        0,
                        nurse_px_x + MONITOR_DX,
                        nurse_px_y + MONITOR_DY,
                        heal_pal,
                    );
                }

                let count = (healing_state.pokeballs_visible as usize).min(BALL_OAM.len());
                for i in 0..count {
                    let (dx, dy, flip) = BALL_OAM[i];
                    if 1 < ts.len() {
                        blit_tile_clipped_flipped(
                            fb,
                            ts,
                            1,
                            nurse_px_x + dx,
                            nurse_px_y + dy,
                            heal_pal,
                            flip,
                        );
                    }
                }
            }
        }

        // Boulder push dust — AnimateBoulderDust (engine/overworld/
        // dust_smoke.asm): a 2×2 OAM block of 8×8 smoke tiles
        // (gfx/overworld/smoke.2bpp) kicked up at the boulder's base.
        // Positioned from the player sprite's top-left + per-facing
        // BoulderDustAnimationOffsets (cut.asm:170-176), anchored to the
        // player's tile at push time. Each of the 8 steps (3 frames each)
        // drifts the block 1px against the push direction and flashes the
        // smoke palette (rOBP1 XOR %01100100).
        if screen.boulder_dust.is_active() {
            let dust = screen.boulder_dust;
            let (ax, ay) = dust.anchor();
            let anchor_px_x = (ax as i32 * 2 - view_origin_tx) * TILE_SIZE as i32;
            let anchor_px_y = (ay as i32 * 2 - view_origin_ty) * TILE_SIZE as i32;
            let (bx, by) = dust.base_offset();
            let step = dust.step() as i32;
            if let Ok(cached) = rm.load_asset(AssetCategory::Overworld, "smoke.png") {
                let ts = &cached.tileset;
                // rOBP1=%11100100: idx 0→transparent, 1→white, 2→light gray,
                // 3→dark gray; the step flash XORs %01100100, swapping idx 2/3.
                let obp1_pal = Palette::new(&[
                    Rgba::TRANSPARENT,
                    Rgba::rgb(0xFF, 0xFF, 0xFF),
                    Rgba::rgb(0xAA, 0xAA, 0xAA),
                    Rgba::rgb(0x55, 0x55, 0x55),
                ]);
                let obp1_flash = Palette::new(&[
                    Rgba::TRANSPARENT,
                    Rgba::rgb(0xFF, 0xFF, 0xFF),
                    Rgba::rgb(0x55, 0x55, 0x55),
                    Rgba::rgb(0xAA, 0xAA, 0xAA),
                ]);
                let dust_pal = if dust.palette_flipped() {
                    &obp1_flash
                } else {
                    &obp1_pal
                };
                let drifts = dust.tile_drifts();
                for i in 0..4 {
                    let col = (i % 2) as i32;
                    let row = (i / 2) as i32;
                    let (ddx, ddy) = drifts[i];
                    let tx = anchor_px_x + bx + col * TILE_SIZE as i32 + ddx * step;
                    let ty = anchor_px_y + by + row * TILE_SIZE as i32 + ddy * step;
                    blit_tile_clipped(fb, ts, 0, tx, ty, dust_pal);
                }
            }
        }

        // CUT — InitCutAnimOAM + AnimCut. The map block underneath has already
        // been replaced; this 2×2 OAM copy of the tree holds the old shape,
        // then separates its rows horizontally one pixel per update.
        if let Some(cut) = screen.cut_anim {
            let player_x = screen_center_tx * TILE_SIZE as i32;
            let player_y = screen_center_ty * TILE_SIZE as i32;
            let (base_x, base_y) = cut.base_offset();
            let spread = cut.tree_spread_px();
            let normal = Palette::new(&[
                Rgba::TRANSPARENT,
                Rgba::rgb(0xFF, 0xFF, 0xFF),
                Rgba::rgb(0xAA, 0xAA, 0xAA),
                Rgba::rgb(0x55, 0x55, 0x55),
            ]);
            let flipped = Palette::new(&[
                Rgba::TRANSPARENT,
                Rgba::rgb(0xFF, 0xFF, 0xFF),
                Rgba::rgb(0x55, 0x55, 0x55),
                Rgba::rgb(0xAA, 0xAA, 0xAA),
            ]);
            let cut_pal = if cut.palette_flipped() {
                &flipped
            } else {
                &normal
            };
            match cut.kind {
                pokered_core::overworld::presentation::CutAnimKind::Tree => {
                    if let Ok(cached) = rm.load_tileset("overworld") {
                        let tree = &cached.tileset;
                        for (tile, col, row) in [
                            (0x2dusize, 0i32, 0i32),
                            (0x2e, 1, 0),
                            (0x3d, 0, 1),
                            (0x3e, 1, 1),
                        ] {
                            let dx = if row == 0 { spread } else { -spread };
                            blit_tile_clipped(
                                fb,
                                tree,
                                tile,
                                player_x + base_x + col * TILE_SIZE as i32 + dx,
                                player_y + base_y + row * TILE_SIZE as i32,
                                cut_pal,
                            );
                        }
                    }
                }
                pokered_core::overworld::presentation::CutAnimKind::Grass => {
                    if let Ok(cached) = rm.load_battle("move_anim_0") {
                        let leaves = &cached.tileset;
                        let drift = cut.frame.saturating_sub(2) as i32;
                        for (col, row, dx) in [
                            (0, 0, drift),
                            (1, 0, drift * 2),
                            (0, 1, -drift * 2),
                            (1, 1, -drift),
                        ] {
                            blit_tile_clipped(
                                fb,
                                leaves,
                                6,
                                player_x + base_x + col * TILE_SIZE as i32 + dx,
                                player_y + base_y + row * TILE_SIZE as i32 + drift / 4,
                                cut_pal,
                            );
                        }
                    }
                }
            }
        }

        // S.S. Anne departure smoke puffs (VermilionDockSSAnneLeavesScript,
        // scripts/VermilionDock.asm:76-88): a 2×2 block of smoke tiles
        // (gfx/overworld/smoke.2bpp) emitted above the smokestack once per
        // scroll iteration, drifting right (VermilionDock_EmitSmokePuff +
        // VermilionDock_AnimSmokePuffDriftRight). OAM sprites — they do not
        // move with the BG scroll. rOBP1 = 0 in the original (white smoke);
        // the port's smoke.png has one tile instead of the original's four
        // ($fc-$ff), so the tile is repeated across the 2×2 block.
        if let Some(dep) = screen.ship_departure.as_ref() {
            if dep.puff_count() > 0 {
                if let Ok(cached) = rm.load_asset(AssetCategory::Overworld, "smoke.png") {
                    let ts = &cached.tileset;
                    // rOBP1=%00000000: idx 0→transparent, 1-3→white.
                    let obp1_pal =
                        Palette::new(&[Rgba::TRANSPARENT, Rgba::WHITE, Rgba::WHITE, Rgba::WHITE]);
                    // The smokestack's screen position at departure start
                    // (map tile (16, 10.5) → view-relative px).
                    let anchor_x = SHIP_DEPARTURE_SMOKESTACK_TILE_X as i32 * TILE_SIZE as i32
                        - view_origin_tx * 4;
                    let anchor_y = dep.puff_screen_y() - view_origin_ty * 4;
                    let rebase = anchor_x - SHIP_DEPARTURE_PUFF_START_SCREEN_X;
                    for i in 0..dep.puff_count() {
                        let px = rebase + dep.puff_x_offset(i);
                        for (col, row) in [(0i32, 0i32), (1, 0), (0, 1), (1, 1)] {
                            blit_tile_clipped(
                                fb,
                                ts,
                                0,
                                px + col * TILE_SIZE as i32,
                                anchor_y + row * TILE_SIZE as i32,
                                &obp1_pal,
                            );
                        }
                    }
                }
            }
        }
    } else {
        let map_name = format!("Map: {:?}", screen.state.current_map);
        draw_text(&map_name, 10, 10, Rgba::BLACK, fb);
        let player_pos = format!(
            "Player: ({}, {})",
            screen.state.player.x, screen.state.player.y
        );
        draw_text(&player_pos, 10, 30, Rgba::BLACK, fb);
        let facing = format!("Facing: {:?}", screen.state.player.facing);
        draw_text(&facing, 10, 50, Rgba::BLACK, fb);
        draw_text("Graphics resources not loaded", 10, 80, Rgba::BLACK, fb);
        draw_text(
            "Use native build for full graphics",
            10,
            100,
            Rgba::BLACK,
            fb,
        );
    }

    // Fullscreen Pokédex entry overlay — takes over the entire screen.
    if let Some(ref mut dex_state) = screen.pending_pokedex_entry {
        let total = draw_pokedex_entry(
            &dex_state.species,
            dex_state.page,
            res,
            fb,
            language == pokered_core::game_state::Lang::Zh,
        );
        dex_state.total_pages = total;
        return;
    }

    if let Some(dlg) = screen
        .pending_dialogue
        .as_ref()
        .or(screen.cut_retained_dialogue.as_ref())
    {
        if let Some((d1, d2)) = dlg.get_display_text() {
            // Keep the script-authored line break: joining with ' ' and
            // re-wrapping loses it (and CJK pages re-wrap at wrong points).
            let combined = if d2.is_empty() {
                d1.to_string()
            } else {
                format!("{}\n{}", d1, d2)
            };
            let show_arrow = dlg.waiting_for_input() && (screen.frame_counter / 16) % 2 == 0;
            let mut painter = FrameBufferPainter::new(fb);
            let mut ui = Ui::new(&mut painter);
            menus::dialog::draw(
                &combined,
                show_arrow,
                &DIALOG_DEFAULT_LAYOUT,
                &mut ui,
                language,
            );
        }

        if let Some(ref choice) = screen.pending_choice {
            let mut painter = FrameBufferPainter::new(fb);
            let mut ui = Ui::new(&mut painter);
            menus::yes_no::draw(
                &choice.options,
                choice.selected,
                &YES_NO_DEFAULT_LAYOUT,
                &mut ui,
            );
        }

        return;
    }

    if let Some(ref choice) = screen.pending_choice {
        let mut painter = FrameBufferPainter::new(fb);
        let mut ui = Ui::new(&mut painter);
        menus::yes_no::draw(
            &choice.options,
            choice.selected,
            &YES_NO_DEFAULT_LAYOUT,
            &mut ui,
        );
        return;
    }

    // ── GB palette effects (home/fade.asm) ─────────────────────────
    // Priority: FLASH white-out > dark cave (LoadGBPal with wMapPalOffset=6)
    // > warp fade. Entering a dark cave fades OUT to black, but the arrival
    // applies the dark palette instantly via LoadGBPal (no fade-in), which
    // this ordering reproduces.
    if screen
        .field_move_restore
        .as_ref()
        .is_some_and(|restore| restore.force_white())
    {
        // SURF's party-menu teardown reloads VRAM while the GB palettes are
        // white. The final 23 restoration frames show the map but no OAM.
        fb.clear(Rgba::WHITE);
        return;
    }
    if screen.flash_lit_frames > 0 {
        // GBPalWhiteOutWithDelay3: all palettes to white.
        fb.clear(Rgba::WHITE);
        return;
    }
    if screen
        .fly_departure
        .as_ref()
        .is_some_and(|fly| fly.force_white())
    {
        fb.clear(Rgba::WHITE);
        return;
    }
    if screen.dark_cave.is_dark() {
        apply_gb_palette(fb, &dotzuki_renderer::transition::load_gb_pal(6));
        return;
    }
    if let Some(pal) = warp_fade_palette(screen) {
        apply_gb_palette(fb, &pal);
    }
}

/// Map every framebuffer pixel through a GB palette byte (rBGP) — shared
/// helper lives in `render::mod` (`apply_gb_palette`).
///
/// The fade palette to apply this frame of the warp transition, following
/// home/fade.asm: GBFadeOutToBlack on normal warps (FadePal4→1),
/// GBFadeOutToWhite on escape/fly warps (FadePal6→8), GBFadeInFromWhite on
/// arrival (FadePal7→5).
fn warp_fade_palette(screen: &OverworldScreen) -> Option<FadePalette> {
    match screen.warp_fade_state {
        WarpFadeState::Idle => None,
        WarpFadeState::BlackScreen => Some(if screen.warp_fade_to_white {
            FADE_PALETTES[7] // FadePal8 — all white
        } else {
            FADE_PALETTES[0] // FadePal1 — all black
        }),
        WarpFadeState::FadingOut { frames_remaining } => {
            let (total, seq): (u8, &[usize]) = if screen.warp_fade_to_white {
                (
                    pokered_core::overworld::screen::WARP_FADE_OUT_WHITE_FRAMES,
                    &[5, 6, 7],
                )
            } else {
                (
                    pokered_core::overworld::screen::WARP_FADE_OUT_FRAMES,
                    &[3, 2, 1, 0],
                )
            };
            let step = ((total - frames_remaining) / WARP_FADE_DELAY) as usize;
            Some(FADE_PALETTES[seq[step.min(seq.len() - 1)]])
        }
        WarpFadeState::FadingIn { frames_remaining } => {
            let step = ((WARP_FADE_IN_FRAMES - frames_remaining) / WARP_FADE_DELAY) as usize;
            Some(FADE_PALETTES[[6, 5, 4][step.min(2)]])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_core::overworld::screen::{
        OverworldScreen, WarpFadeState, WARP_FADE_IN_FRAMES, WARP_FADE_OUT_FRAMES,
        WARP_FADE_OUT_WHITE_FRAMES,
    };
    use pokered_data::impl_traits::PokemonRedData;
    use pokered_data::maps::MapId;

    fn screen() -> OverworldScreen<PokemonRedData> {
        OverworldScreen::new(MapId::PalletTown, None, PokemonRedData)
    }

    fn reference_sprite_tile_blit(
        fb: &mut FrameBuffer,
        tile: &Tile,
        x: i32,
        y: i32,
        palette: &Palette,
        flip_horizontal: bool,
    ) {
        for row in 0..TILE_SIZE as i32 {
            let sy = y + row;
            if sy < 0 || sy >= fb.height() as i32 {
                continue;
            }
            let rgba_row = tile.render_row(row as usize, palette);
            for col in 0..TILE_SIZE as i32 {
                let sx = x + col;
                if sx < 0 || sx >= fb.width() as i32 {
                    continue;
                }
                let src_col = if flip_horizontal {
                    TILE_SIZE as i32 - 1 - col
                } else {
                    col
                };
                let color = rgba_row[src_col as usize];
                if color != Rgba::TRANSPARENT {
                    fb.set_pixel(sx as u32, sy as u32, color);
                }
            }
        }
    }

    fn reference_priority_bg_tile_blit(
        fb: &mut FrameBuffer,
        tile: &Tile,
        x: i32,
        y: i32,
        palette: &Palette,
    ) {
        for row in 0..TILE_SIZE as i32 {
            for col in 0..TILE_SIZE as i32 {
                let color_index = tile.pixels[row as usize][col as usize];
                if color_index == 0 {
                    continue;
                }
                let sx = x + col;
                let sy = y + row;
                if sx >= 0 && sx < fb.width() as i32 && sy >= 0 && sy < fb.height() as i32 {
                    fb.set_pixel(
                        sx as u32,
                        sy as u32,
                        palette.color(GbColor::from_u8(color_index)),
                    );
                }
            }
        }
    }

    #[test]
    fn clipped_sprite_helpers_match_reference_for_palettes_and_flips() {
        let tileset = TileSet::from_2bpp(&[
            0xAA, 0xCC, 0xF0, 0x5A, 0x33, 0x0F, 0x81, 0x7E, 0x66, 0x99, 0x18, 0xE7, 0xC3, 0x3C,
            0xA5, 0x5A,
        ]);
        let palettes = [
            Palette::new(&[
                Rgba::TRANSPARENT,
                Rgba::rgb(0xAA, 0xAA, 0xAA),
                Rgba::rgb(0x55, 0x55, 0x55),
                Rgba::BLACK,
            ]),
            // A non-identity palette with a transparent non-zero entry
            // exercises the generic palette mapping and transparency rules.
            Palette::new(&[
                Rgba::TRANSPARENT,
                Rgba::BLACK,
                Rgba::TRANSPARENT,
                Rgba::rgb(0xAA, 0xAA, 0xAA),
            ]),
        ];
        let cases = [
            (-3, 2, false),
            (5, -2, true),
            (7, 6, false),
            (3, 7, true),
            (-9, 3, false),
        ];

        for (palette_index, palette) in palettes.iter().enumerate() {
            for &(x, y, flip_horizontal) in &cases {
                let config = RenderConfig::new(10, 10);
                let background = Rgba::rgb(0x55, 0x55, 0x55);
                let mut expected = FrameBuffer::new(config.clone(), background);
                let mut actual = FrameBuffer::new(config, background);
                reference_sprite_tile_blit(
                    &mut expected,
                    tileset.get(0),
                    x,
                    y,
                    palette,
                    flip_horizontal,
                );
                if flip_horizontal {
                    blit_tile_clipped_flipped(&mut actual, &tileset, 0, x, y, palette, true);
                } else {
                    blit_tile_clipped(&mut actual, &tileset, 0, x, y, palette);
                }
                assert_eq!(
                    actual.packed(),
                    expected.packed(),
                    "palette={palette_index}, x={x}, y={y}, flip={flip_horizontal}",
                );
            }
        }
    }

    #[test]
    fn priority_background_tile_blit_matches_grass_overlay_reference() {
        let tileset = TileSet::from_2bpp(&[
            0xAA, 0xCC, 0xF0, 0x5A, 0x33, 0x0F, 0x81, 0x7E, 0x66, 0x99, 0x18, 0xE7, 0xC3, 0x3C,
            0xA5, 0x5A,
        ]);
        let tile = tileset.get(0);
        for &(x, y) in &[(1, 1), (7, 6)] {
            let config = RenderConfig::new(10, 10);
            let background = Rgba::rgb(0x55, 0x55, 0x55);
            let mut expected = FrameBuffer::new(config.clone(), background);
            let mut actual = FrameBuffer::new(config, background);
            reference_priority_bg_tile_blit(&mut expected, tile, x, y, &GRAYSCALE_PALETTE);
            blit_priority_bg_tile(&mut actual, tile, x, y);
            assert_eq!(actual.packed(), expected.packed(), "x={x}, y={y}");
        }
    }

    #[test]
    fn mirrored_player_index_blit_matches_sprite_palette_blit() {
        let mut tile = Tile::blank();
        for row in 0..TILE_SIZE as usize {
            for column in 0..TILE_SIZE as usize {
                tile.pixels[row][column] = ((row * 3 + column) & 3) as u8;
            }
        }
        let sprite_palette = Palette::new(&[
            Rgba::TRANSPARENT,
            Rgba::rgb(0xAA, 0xAA, 0xAA),
            Rgba::rgb(0x55, 0x55, 0x55),
            Rgba::BLACK,
        ]);
        let config = RenderConfig::new(10, 10);
        let mut palette_blit = FrameBuffer::new(config.clone(), Rgba::rgb(0x55, 0x55, 0x55));
        let mut index_blit = FrameBuffer::new(config, Rgba::rgb(0x55, 0x55, 0x55));

        palette_blit.blit_gb_tile(1, 1, &tile, &sprite_palette, true, true, false);
        index_blit.blit_gb_tile_indices(1, 1, &tile, true, true, false);

        assert_eq!(palette_blit.packed(), index_blit.packed());
    }

    #[test]
    fn underground_south_arrival_renders_short_original_block_data() {
        let mut s = OverworldScreen::new(MapId::UndergroundPathNorthSouth, None, PokemonRedData);
        s.state.player.x = 2;
        s.state.player.y = 41;
        let root = pokered_renderer::resource::AssetRoot::auto_detect().expect("test graphics");
        let mut resources = Some(ResourceManager::new(root));
        let mut frame = FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            Rgba::WHITE,
        );
        draw_overworld(
            &mut s,
            &mut resources,
            &mut frame,
            pokered_core::game_state::Lang::En,
        );
        assert_eq!(frame.width(), 160);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn screen_on(map: MapId) -> OverworldScreen<PokemonRedData> {
        OverworldScreen::new(map, None, PokemonRedData)
    }

    #[test]
    fn warp_fade_out_to_black_sequence() {
        let mut s = screen();
        // GBFadeOutToBlack: FadePal4 → FadePal1 in 4 steps of 8 frames.
        let expected = [3, 2, 1, 0];
        for (step, pal_idx) in expected.iter().enumerate() {
            s.warp_fade_state = WarpFadeState::FadingOut {
                frames_remaining: WARP_FADE_OUT_FRAMES - (step as u8) * WARP_FADE_DELAY,
            };
            let pal = warp_fade_palette(&s).expect("palette during fade-out");
            assert_eq!(pal, FADE_PALETTES[*pal_idx], "step {}", step);
        }
        s.warp_fade_state = WarpFadeState::BlackScreen;
        assert_eq!(warp_fade_palette(&s), Some(FADE_PALETTES[0]));
    }

    #[test]
    fn warp_fade_out_to_white_sequence() {
        let mut s = screen();
        s.warp_fade_to_white = true;
        // GBFadeOutToWhite: FadePal6 → FadePal8 in 3 steps of 8 frames.
        let expected = [5, 6, 7];
        for (step, pal_idx) in expected.iter().enumerate() {
            s.warp_fade_state = WarpFadeState::FadingOut {
                frames_remaining: WARP_FADE_OUT_WHITE_FRAMES - (step as u8) * WARP_FADE_DELAY,
            };
            let pal = warp_fade_palette(&s).expect("palette during fade-out");
            assert_eq!(pal, FADE_PALETTES[*pal_idx], "step {}", step);
        }
        s.warp_fade_state = WarpFadeState::BlackScreen;
        assert_eq!(warp_fade_palette(&s), Some(FADE_PALETTES[7]));
    }

    #[test]
    fn warp_fade_in_from_white_sequence() {
        let mut s = screen();
        // GBFadeInFromWhite: FadePal7 → FadePal5 in 3 steps of 8 frames.
        let expected = [6, 5, 4];
        for (step, pal_idx) in expected.iter().enumerate() {
            s.warp_fade_state = WarpFadeState::FadingIn {
                frames_remaining: WARP_FADE_IN_FRAMES - (step as u8) * WARP_FADE_DELAY,
            };
            let pal = warp_fade_palette(&s).expect("palette during fade-in");
            assert_eq!(pal, FADE_PALETTES[*pal_idx], "step {}", step);
        }
        s.warp_fade_state = WarpFadeState::Idle;
        assert_eq!(warp_fade_palette(&s), None);
    }

    #[test]
    fn apply_gb_palette_maps_shades() {
        let mut fb = FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(4, 1),
            Rgba::WHITE,
        );
        fb.set_pixel(1, 0, Rgba::rgb(0xAA, 0xAA, 0xAA));
        fb.set_pixel(2, 0, Rgba::rgb(0x55, 0x55, 0x55));
        fb.set_pixel(3, 0, Rgba::BLACK);
        // FadePal2 (dark cave): color0→2, everything else→3. The indices
        // underneath stay untouched (draws are unaffected); only the
        // display palette remaps.
        apply_gb_palette(&mut fb, &FADE_PALETTES[1]);
        assert_eq!(fb.get_pixel(0, 0).unwrap(), Rgba::rgb(0x55, 0x55, 0x55));
        assert_eq!(fb.get_pixel(1, 0).unwrap(), Rgba::BLACK);
        assert_eq!(fb.get_pixel(2, 0).unwrap(), Rgba::BLACK);
        assert_eq!(fb.get_pixel(3, 0).unwrap(), Rgba::BLACK);
        assert_eq!(fb.get_index(0, 0), Some(GbColor::White));
        assert_eq!(fb.get_index(1, 0), Some(GbColor::LightGray));
    }

    #[test]
    fn overlapping_foreground_patches_restore_background_in_reverse_order() {
        let mut frame = FrameBuffer::new(RenderConfig::new(32, 32), Rgba::WHITE);
        for y in 0..frame.height() {
            for x in 0..frame.width() {
                frame.set_pixel_index(x, y, GbColor::from_u8(((x + y) & 3) as u8));
            }
        }
        let background = frame.packed().to_vec();
        let mut cache = OverworldBackgroundCache::new(frame.width(), frame.height());
        let first = FrameDamageRect {
            x: 4,
            y: 4,
            width: 16,
            height: 16,
        };
        let second = FrameDamageRect {
            x: 12,
            y: 12,
            width: 16,
            height: 16,
        };

        cache
            .foreground_patches
            .push(ForegroundPatch::capture(&frame, first));
        for y in first.y..first.y + first.height {
            for x in first.x..first.x + first.width {
                frame.set_pixel_index(x, y, GbColor::Black);
            }
        }
        cache
            .foreground_patches
            .push(ForegroundPatch::capture(&frame, second));
        for y in second.y..second.y + second.height {
            for x in second.x..second.x + second.width {
                frame.set_pixel_index(x, y, GbColor::LightGray);
            }
        }

        restore_foreground_regions(&mut frame, &cache);
        assert_eq!(frame.packed(), background);
    }

    #[test]
    fn dark_cave_palette_is_fadepal2() {
        // LoadGBPal with wMapPalOffset=6 reads FadePal4 - 6 bytes = FadePal2.
        let pal = dotzuki_renderer::transition::load_gb_pal(6);
        assert_eq!(pal, FADE_PALETTES[1]);
        assert_eq!(pal.bgp, 0xFE); // dc 3,3,3,2
    }

    /// Render the overworld to a framebuffer and count the GRAYSCALE shades.
    #[cfg(not(target_arch = "wasm32"))]
    fn shade_histogram(screen: &mut OverworldScreen) -> [usize; 4] {
        let mut res = pokered_renderer::resource::AssetRoot::auto_detect()
            .ok()
            .map(pokered_renderer::resource::ResourceManager::new);
        let mut fb = FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            Rgba::WHITE,
        );
        draw_overworld(
            screen,
            &mut res,
            &mut fb,
            pokered_core::game_state::Lang::En,
        );
        // Classify by the *displayed* color: the fade now lives in the
        // display palette (indices stay as drawn), so get_pixel applies it.
        let mut counts = [0usize; 4];
        for y in 0..fb.height() {
            for x in 0..fb.width() {
                let c = fb.get_pixel(x, y).unwrap();
                let idx = match (c.r, c.g, c.b) {
                    (0xFF, 0xFF, 0xFF) => 0,
                    (0xAA, 0xAA, 0xAA) => 1,
                    (0x55, 0x55, 0x55) => 2,
                    _ => 3,
                };
                counts[idx] += 1;
            }
        }
        counts
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn rock_tunnel_renders_dark_until_flash() {
        let mut s = screen_on(MapId::RockTunnel1F);
        assert!(s.dark_cave.is_dark());
        let [white, light, dark, _black] = shade_histogram(&mut s);
        // FadePal2 (dc 3,3,3,2): white→dark gray, all other shades→black.
        assert_eq!(white, 0, "no pure-white pixels in a dark cave");
        assert_eq!(light, 0, "no light-gray pixels in a dark cave");
        assert!(dark > 0, "cave walls remain visible as dark gray");
        // FLASH (wMapPalOffset=0) restores the normal palette.
        s.dark_cave.use_flash();
        let [white, light, _dark, _black] = shade_histogram(&mut s);
        assert!(white > 0, "lit cave shows white pixels again");
        assert!(light > 0, "lit cave shows light-gray pixels again");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn water_tile_animation_changes_pixels() {
        let mut s = screen_on(MapId::PalletTown);
        // Stand at the south end so the water (blocks 2-3 × 7-8, the south
        // pond) is inside the viewport.
        s.state.player.x = 5;
        s.state.player.y = 15;
        assert_eq!(
            s.tile_anim.kind(),
            pokered_core::overworld::presentation::TileAnimKind::WaterFlower
        );
        let mut res = pokered_renderer::resource::AssetRoot::auto_detect()
            .ok()
            .map(pokered_renderer::resource::ResourceManager::new);
        let mut fb_a = FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            Rgba::WHITE,
        );
        draw_overworld(
            &mut s,
            &mut res,
            &mut fb_a,
            pokered_core::game_state::Lang::En,
        );
        // 20 ticks = one water update (one-pixel rotation of tile $14).
        for _ in 0..20 {
            s.tile_anim.tick();
        }
        assert_eq!(s.tile_anim.water_shift(), 1);
        let mut fb_b = FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            Rgba::WHITE,
        );
        draw_overworld(
            &mut s,
            &mut res,
            &mut fb_b,
            pokered_core::game_state::Lang::En,
        );
        assert_ne!(
            fb_a.packed(),
            fb_b.packed(),
            "water rotation changes the frame"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn incremental_background_matches_full_render_while_walking() {
        let root = pokered_renderer::resource::AssetRoot::auto_detect().expect("test graphics");
        let mut full_resources = Some(ResourceManager::new(root.clone()));
        let mut cached_resources = Some(ResourceManager::new(root));
        let mut s = screen_on(MapId::PalletTown);
        s.state.player.x = 12;
        s.state.player.y = 12;
        let mut cache = OverworldBackgroundCache::new(160, 144);
        let mut incremental = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);

        let mut compare = |screen: &mut OverworldScreen| -> Option<usize> {
            let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            draw_overworld(
                screen,
                &mut full_resources,
                &mut full,
                pokered_core::game_state::Lang::En,
            );
            let reuse_composited = can_reuse_composited_frame(screen);
            draw_overworld_cached_with(
                screen,
                &mut cached_resources,
                &mut incremental,
                pokered_core::game_state::Lang::En,
                &mut cache,
                &mut |_, _, _, _, _, _| {
                    unreachable!("host framebuffer must use its planar scroll implementation")
                },
                reuse_composited,
            );
            assert_eq!(
                full.packed(),
                incremental.packed(),
                "cached background must preserve every pixel"
            );
            assert_eq!(full.display_palette(), incremental.display_palette());
            cache.presentation_damage().map(<[_]>::len)
        };

        assert_eq!(
            compare(&mut s),
            None,
            "the cold frame draws directly into the output"
        );
        s.state.player.facing = Direction::Right;
        let damage_count = compare(&mut s);
        assert!(damage_count.is_some_and(|count| count >= 2));
        {
            let npc = s
                .npc_states
                .iter_mut()
                .find(|npc| npc.visible)
                .expect("Pallet Town has a visible NPC");
            npc.facing = Direction::Right;
            npc.walk_counter = 8;
        }
        let damage_count = compare(&mut s);
        assert!(damage_count.is_some_and(|count| count >= 2));
        s.npc_states
            .iter_mut()
            .find(|npc| npc.visible)
            .unwrap()
            .walk_counter = 0;
        let _ = compare(&mut s);
        for direction in [
            Direction::Down,
            Direction::Up,
            Direction::Left,
            Direction::Right,
        ] {
            s.state.player.facing = direction;
            s.state.player.movement_state = MovementState::Walking;
            s.state.walk_counter = 8;
            let _ = compare(&mut s);
            for counter in (1..8).rev() {
                s.state.walk_counter = counter;
                let _ = compare(&mut s);
            }
            let (dx, dy) = match direction {
                Direction::Down => (0, 1),
                Direction::Up => (0, -1),
                Direction::Left => (-1, 0),
                Direction::Right => (1, 0),
            };
            s.state.player.x = (s.state.player.x as i32 + dx) as u16;
            s.state.player.y = (s.state.player.y as i32 + dy) as u16;
            s.state.player.movement_state = MovementState::Idle;
            s.state.walk_counter = 0;
            let _ = compare(&mut s);
        }

        // Non-standard camera changes can move both axes at once. They keep
        // the conservative union filter rather than the single-strip ranges.
        s.state.player.x += 1;
        s.state.player.y += 1;
        let _ = compare(&mut s);

        let map = s.map_data.as_mut().expect("live map");
        map.blocks[0] = map.blocks[0].wrapping_add(1);
        let _ = compare(&mut s);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn exact_background_hit_preserves_dark_cave_palette() {
        let root = pokered_renderer::resource::AssetRoot::auto_detect().expect("test graphics");
        let mut full_resources = Some(ResourceManager::new(root.clone()));
        let mut cached_resources = Some(ResourceManager::new(root));
        let mut s = screen_on(MapId::RockTunnel1F);
        assert!(s.dark_cave.is_dark());
        let mut cache = OverworldBackgroundCache::new(160, 144);
        let mut cached = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);

        draw_overworld_cached(
            &mut s,
            &mut cached_resources,
            &mut cached,
            pokered_core::game_state::Lang::En,
            &mut cache,
        );
        s.state.player.facing = Direction::Right;
        let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        draw_overworld(
            &mut s,
            &mut full_resources,
            &mut full,
            pokered_core::game_state::Lang::En,
        );
        draw_overworld_cached(
            &mut s,
            &mut cached_resources,
            &mut cached,
            pokered_core::game_state::Lang::En,
            &mut cache,
        );

        assert_eq!(full.packed(), cached.packed());
        assert_eq!(full.display_palette(), cached.display_palette());
    }
}

#[cfg(test)]
mod elevator_edge_tests {
    use super::*;
    use pokered_core::overworld::presentation::ElevatorShakeState;
    use pokered_core::overworld::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    use pokered_data::maps::MapId;

    fn render_screen(screen: &mut OverworldScreen) -> FrameBuffer {
        let mut res = pokered_renderer::resource::AssetRoot::auto_detect()
            .ok()
            .map(pokered_renderer::resource::ResourceManager::new);
        let mut fb = FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            Rgba::WHITE,
        );
        draw_overworld(
            screen,
            &mut res,
            &mut fb,
            pokered_core::game_state::Lang::En,
        );
        fb
    }

    fn render_with_shake_at(map: MapId, x: u16, y: u16, offset_frame: u16) -> FrameBuffer {
        let mut screen = OverworldScreen::new(map, None, PokemonRedData);
        screen.state.player.x = x;
        screen.state.player.y = y;
        let mut shake = ElevatorShakeState::new(
            pokered_core::overworld::doors_elevators::elevator_shake_params(),
        );
        for _ in 0..offset_frame {
            shake.tick();
        }
        screen.elevator_shake = Some(shake);
        render_screen(&mut screen)
    }

    fn render_with_shake(offset_frame: u16) -> FrameBuffer {
        let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
        // Player in the open field so the viewport sits over real map tiles.
        screen.state.player.x = 12;
        screen.state.player.y = 12;
        let mut shake = ElevatorShakeState::new(
            pokered_core::overworld::doors_elevators::elevator_shake_params(),
        );
        for _ in 0..offset_frame {
            shake.tick();
        }
        screen.elevator_shake = Some(shake);
        render_screen(&mut screen)
    }

    /// The real elevator maps (Silph Co, 30×18 tiles): the 18-row viewport
    /// spans the whole map, so the ±1px shake reveals rows beyond the map —
    /// the wrapped-row fix keeps them showing the map's own (wrapped) rows
    /// instead of the border. Verified content-agnostically: the exposed
    /// edge row must be pixel-identical to the same world row in the
    /// no-shake reference (the ±1px camera shift aligns them).
    #[test]
    fn silph_co_shake_edges_show_adjacent_rows() {
        let mut ref_screen = OverworldScreen::new(MapId::SilphCo5F, None, PokemonRedData);
        ref_screen.state.player.x = 15;
        ref_screen.state.player.y = 9;
        let reference = render_screen(&mut ref_screen);

        // +1px: the top edge reveals the row above the viewport — for a map
        // as tall as the screen that row is past the map edge and must wrap
        // to the map's own bottom row, matching the reference's row 1.
        let plus = render_with_shake_at(MapId::SilphCo5F, 15, 9, 2);
        for x in 0..160 {
            assert_eq!(
                plus.get_pixel(x, 0),
                reference.get_pixel(x, 1),
                "Silph +1px top edge wraps to map content at x={x}"
            );
        }
        // -1px: the bottom edge reveals the row below the viewport.
        let minus = render_with_shake_at(MapId::SilphCo5F, 15, 9, 1);
        for x in 0..160 {
            assert_eq!(
                minus.get_pixel(x, 143),
                reference.get_pixel(x, 142),
                "Silph -1px bottom edge wraps to map content at x={x}"
            );
        }
        let path = std::env::temp_dir().join("elevator_silph_minus.png");
        minus.save_png(&path).expect("save png");
    }

    /// Celadon Mansion floors are 8×12 tiles — SHORTER than the 18-tile
    /// viewport — so every shake frame reveals rows past the map edge on
    /// both sides. The wrapped-row fix must show the map's own rows there,
    /// never the border block.
    #[test]
    fn celadon_mansion_shake_wraps_past_short_map_edges() {
        let mut ref_screen = OverworldScreen::new(MapId::CeladonMansion3F, None, PokemonRedData);
        ref_screen.state.player.x = 6;
        ref_screen.state.player.y = 5;
        let reference = render_screen(&mut ref_screen);

        let plus = render_with_shake_at(MapId::CeladonMansion3F, 6, 5, 2);
        for x in 0..160 {
            assert_eq!(
                plus.get_pixel(x, 0),
                reference.get_pixel(x, 1),
                "mansion +1px top edge wraps to map content at x={x}"
            );
        }
        let minus = render_with_shake_at(MapId::CeladonMansion3F, 6, 5, 1);
        for x in 0..160 {
            assert_eq!(
                minus.get_pixel(x, 143),
                reference.get_pixel(x, 142),
                "mansion -1px bottom edge wraps to map content at x={x}"
            );
        }
        let path = std::env::temp_dir().join("elevator_mansion_minus.png");
        minus.save_png(&path).expect("save png");
    }

    /// ShakeElevator scrolls the BG ±1px (hSCY): the exposed edge row must
    /// show the ADJACENT map row (wrapped when past the map edge), never a
    /// background/void line. Verified by matching the shake frame's edge row
    /// against the no-shake frame's row at the same world position (the ±1px
    /// shift makes them pixel-identical).
    #[test]
    fn shake_edges_show_adjacent_map_rows_not_gaps() {
        let mut ref_screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
        ref_screen.state.player.x = 12;
        ref_screen.state.player.y = 12;
        let reference = render_screen(&mut ref_screen);

        // +1px: the viewport moves down — the top edge shows the row that
        // was the reference's second row.
        let plus = render_with_shake(2);
        for x in 0..160 {
            assert_eq!(
                plus.get_pixel(x, 0),
                reference.get_pixel(x, 1),
                "+1px top edge must show the adjacent map row at x={x}"
            );
        }
        // -1px: the viewport moves up — the bottom edge shows the row that
        // was the reference's second-to-last row.
        let minus = render_with_shake(1);
        for x in 0..160 {
            assert_eq!(
                minus.get_pixel(x, 143),
                reference.get_pixel(x, 142),
                "-1px bottom edge must show the adjacent map row at x={x}"
            );
        }

        // At the map edge the revealed row is out of bounds: the shake wraps
        // it back into the map (GB tilemap wrap) instead of drawing the
        // border — the wrapped row is real terrain, not a blank line. Player
        // at the north edge: the -1px top edge reveals rows above the map,
        // which wrap to the south (beach) row.
        let mut edge_screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
        edge_screen.state.player.x = 12;
        edge_screen.state.player.y = 1;
        let mut shake = ElevatorShakeState::new(
            pokered_core::overworld::doors_elevators::elevator_shake_params(),
        );
        shake.tick(); // offset -1
        edge_screen.elevator_shake = Some(shake);
        let edge_fb = render_screen(&mut edge_screen);
        let has_content = (0..160).any(|x| edge_fb.get_pixel(x, 0) != Some(Rgba::WHITE));
        assert!(
            has_content,
            "wrapped edge row at the map edge shows map content, not a blank row"
        );
    }

    /// An old-man-tutorial-style boulder push draws smoke pixels at the
    /// boulder's base: the app renderer's AnimateBoulderDust port.
    #[test]
    fn boulder_push_draws_dust_pixels_at_the_boulder() {
        use pokered_core::overworld::Direction;
        use pokered_data::blockset_data;
        use pokered_data::collision;
        use pokered_data::tilesets::TilesetId;

        // Open ground so the push's destination tile is clear.
        let block = (0u8..=255)
            .find(|&b| {
                let Some(tiles) = blockset_data::block_tiles(TilesetId::Overworld, b) else {
                    return false;
                };
                [4usize, 6, 12, 14]
                    .iter()
                    .all(|&i| collision::is_tile_passable(TilesetId::Overworld, tiles[i]))
            })
            .expect("blockset has a fully passable block");

        let mut s = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
        let map = s.map_data.as_mut().expect("map_data present");
        for by in 0..map.height {
            for bx in 0..map.width {
                map.set_block(bx, by, block);
            }
        }
        s.state.player.x = 5;
        s.state.player.y = 5;
        s.state.player.facing = Direction::Down;
        s.npc_states
            .push(dotzuki_engine::overworld::npc_movement::NpcRuntimeState {
                npc_index: 0,
                sprite_id: pokered_data::sprites::SpriteId::Boulder as u8,
                x: 5,
                y: 6,
                home_x: 5,
                home_y: 6,
                facing: Direction::Down,
                scripted_frame: None,
                movement_type: dotzuki_engine::overworld::NpcMovementType::Stationary,
                wander_axis: dotzuki_engine::overworld::NpcWanderAxis::Any,
                range: 0,
                walk_counter: 0,
                delay_counter: 0,
                text_id: 0,
                defeated: false,
                visible: true,
                scripted_path: std::collections::VecDeque::new(),
            });
        s.strength_active = true;

        let before = render_screen(&mut s);
        assert!(!s.boulder_dust.is_active(), "no dust before the push");

        // Hold DOWN: frame 1 arms BIT_TRIED_PUSH_BOULDER, frame 2 pushes.
        let hold_down = pokered_core::overworld::OverworldInput::new(
            false, true, false, false, false, false, false, false,
        );
        s.update_frame(hold_down);
        s.update_frame(hold_down);
        assert!(s.boulder_dust.is_active(), "push started the dust");

        let after = render_screen(&mut s);
        // The dust block for a DOWN push sits at the boulder's base: player
        // screen top-left (72,64) + BoulderDustAnimationOffsets (8,52) →
        // (80,116), a 16×16 block of 8×8 smoke tiles (dust_smoke.asm).
        // Sample the RIGHT column (88..96): the boulder sprite (16×16,
        // x 72..88) never covers it before or after the slide, so any pixel
        // change there must come from the dust.
        let dust_area_changed =
            (88..96).any(|x| (116..132).any(|y| before.get_pixel(x, y) != after.get_pixel(x, y)));
        assert!(
            dust_area_changed,
            "dust pixels appear at the boulder's base during the push"
        );
    }

    /// The player's bike sprite: while TransportMode::Biking the renderer
    /// must draw red_bike.png — the original swaps the sheet via
    /// LoadBikePlayerSpriteGraphics (home/overworld.asm:1977-1990, RedBikeSprite
    /// in gfx/sprites.asm:34) while wWalkBikeSurfState == 1 — instead of
    /// red.png, with the SAME 6-frame layout (DownStand=0, UpStand=1,
    /// LeftStand=2, DownWalk=3, UpWalk=4, LeftWalk=5) and frame/flip
    /// selection. Both sheets are 16×96 and share no frame, so with identical
    /// game state the only pixel difference at the sprite must come from the
    /// sheet swap.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn biking_renders_red_bike_sheet_not_red() {
        use dotzuki_engine::overworld::types::TransportMode;

        let mut walking = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
        let mut biking = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
        for s in [&mut walking, &mut biking] {
            s.state.player.x = 12;
            s.state.player.y = 12;
            s.state.player.facing = Direction::Down;
            // Moving with walk_counter > 4 → frame 3 (DownWalk) on both sheets.
            s.state.player.movement_state = MovementState::Walking;
            s.state.walk_counter = 6;
        }
        biking.state.player.transport = TransportMode::Biking;

        let walk_fb = render_screen(&mut walking);
        let bike_fb = render_screen(&mut biking);

        // Player sprite rect: screen center (72,64), 16×16. Compare the top
        // half (y 64..72) only — the bottom half can be redrawn by the grass
        // overlay, the top half is sheet pixels alone.
        let top_half = |fb: &FrameBuffer| {
            (0..8)
                .flat_map(|dy| (0..16).map(move |dx| (dy, dx)))
                .map(|(dy, dx)| fb.get_pixel(72 + dx as u32, 64 + dy as u32))
                .collect::<Vec<_>>()
        };
        let walk_top = top_half(&walk_fb);
        let bike_top = top_half(&bike_fb);

        assert!(
            walk_top.iter().any(|&p| p != Some(Rgba::WHITE)),
            "walking sprite ink present on foot"
        );
        assert!(
            bike_top.iter().any(|&p| p != Some(Rgba::WHITE)),
            "bike sprite ink present while biking"
        );
        assert_ne!(
            walk_top, bike_top,
            "biking must draw the red_bike sheet, not red.png"
        );
    }
}

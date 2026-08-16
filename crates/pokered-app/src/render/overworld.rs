use pokered_core::data::blockset_data;
use pokered_core::data::map_data_loader::{get_block_data, get_map_json, resolve_map_id};
use pokered_core::data::maps::MapId;
use pokered_core::data::sprites::SpriteId;
use pokered_core::data::tileset_data;
use pokered_core::overworld::presentation::{
    ANIM_FLOWER_TILE, ANIM_WATER_TILE, SHIP_DEPARTURE_PUFF_START_SCREEN_X,
    SHIP_DEPARTURE_SMOKESTACK_TILE_X,
};
use pokered_core::overworld::screen::{WarpFadeState, WARP_FADE_DELAY, WARP_FADE_IN_FRAMES};
use pokered_core::overworld::{Direction, MovementState, OverworldScreen};
use pokered_data::impl_traits::PokemonTilesetData;
use dotzuki_engine::overworld::types::TransportMode;
use dotzuki_engine::tileset::TilesetProvider;
use dotzuki_renderer::transition::{FadePalette, FADE_PALETTES};
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::palette::{GbColor, Palette, GRAYSCALE_PALETTE};
use pokered_renderer::resource::{AssetCategory, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use dotzuki_engine::render::MapLayer;
use dotzuki_engine::tilemap::{Tilemap, TilemapEntry};
use pokered_renderer::layer_renderer::render_layers;
use pokered_renderer::tile::TileSet;
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{menus, Ui};
use pokered_data::ui_layout::schema::{DIALOG_DEFAULT_LAYOUT, YES_NO_DEFAULT_LAYOUT};

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
    let tile = tileset.get(tile_idx);
    for row in 0..TILE_SIZE as i32 {
        let sy = y + row;
        if sy < 0 || sy >= fb.height() as i32 {
            continue;
        }
        let rgba_row = tile.render_row(row as usize, palette);
        for col in 0..TILE_SIZE as i32 {
            let sx = x + col;
            if sx >= 0 && sx < fb.width() as i32 {
                let c = rgba_row[col as usize];
                if c != Rgba::TRANSPARENT {
                    fb.set_pixel(sx as u32, sy as u32, c);
                }
            }
        }
    }
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
    let tile = tileset.get(tile_idx);
    for row in 0..TILE_SIZE as i32 {
        let sy = y + row;
        if sy < 0 || sy >= fb.height() as i32 {
            continue;
        }
        let rgba_row = tile.render_row(row as usize, palette);
        for col in 0..TILE_SIZE as i32 {
            let sx = x + col;
            if sx >= 0 && sx < fb.width() as i32 {
                let src_col = if flip_horizontal {
                    TILE_SIZE as i32 - 1 - col
                } else {
                    col
                };
                let c = rgba_row[src_col as usize];
                if c != Rgba::TRANSPARENT {
                    fb.set_pixel(sx as u32, sy as u32, c);
                }
            }
        }
    }
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
) -> usize {
    match pokered_data::species::Species::from_scene_name(species) {
        Some(sp) => super::pokedex::draw_entry_for_species(sp, page, true, res, fb),
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
    current_map: MapId,
    map_w: u8,
    map_h: u8,
    blk: &[u8],
    border_block: u8,
    bx: i32,
    by: i32,
) -> u8 {
    if bx >= 0 && by >= 0 && (bx as u8) < map_w && (by as u8) < map_h && !blk.is_empty() {
        return blk[by as usize * map_w as usize + bx as usize];
    }

    let map_json = match get_map_json(current_map) {
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
                    return target_blk[target_by as usize * tw as usize + target_bx as usize];
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
                    return target_blk[target_by as usize * tw as usize + target_bx as usize];
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
                    return target_blk[target_by as usize * tw as usize + target_bx as usize];
                }
            }
        }
        return border_block;
    }

    border_block
}

pub fn draw_overworld(
    screen: &mut OverworldScreen,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    language: pokered_core::game_state::Lang,
) {
    fb.clear(Rgba::WHITE);

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

    let pal = &GRAYSCALE_PALETTE;

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
    let (view_sub_x, view_sub_y) = if screen.state.player.movement_state == MovementState::Walking {
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
        let flower_ts = if tile_anim_kind == pokered_data::tileset_data::TileAnimation::WaterFlower {
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
        let departure_scroll =
            screen.ship_departure.as_ref().map_or(0, |d| d.scroll_px());
        let departure_active = screen.ship_departure.is_some();

        let (map_w, map_h) = current_map.dimensions();
        let blk = get_block_data(current_map);

        if let Ok(cached) = rm.load_tileset(tileset_name) {
            let ts = cached.tileset.clone();

            // ── Multi-layer tilemap rendering via render_layers ────────────
            // Layer 0 (ground, z=0): sub_y ∈ {0,1} — walking surface / base tiles
            // Layer 1 (decoration, z=1): sub_y ∈ {2,3} — tree canopies, building tops
            //
            // The tilemap is built so that index (0,0) corresponds to world tile
            // (tile_start_tx, tile_start_ty). The camera is offset by `margin * TILE_SIZE`
            // so that `render_single_layer` computes matching tilemap indices.

            let margin = 2i32;
            let tile_start_tx = view_origin_tx - margin;
            let tile_start_ty = view_origin_ty - margin;
            let tiles_w = (fb.width() / TILE_SIZE) as i32 + margin * 2;
            let tiles_h = (fb.height() / TILE_SIZE) as i32 + margin * 2;

            // Camera position: tilemap index 0 ↔ world tile tile_start_tx.
            // render_single_layer computes tile_x = (camera_x + screen_x) / TILE_SIZE,
            // so camera_x = margin * TILE_SIZE + view_sub_x yields tile_x = margin at
            // screen_x=0 (when view_sub_x=0), pointing at tilemap column=margin which
            // maps to world tile tile_start_tx + margin = view_origin_tx.
            let camera_x = margin * TILE_SIZE as i32 + view_sub_x + departure_scroll;
            let camera_y = margin * TILE_SIZE as i32 + view_sub_y + shake_offset_y;

            let mut ground_tm = Tilemap::new(tiles_w as u16, tiles_h as u16);
            let mut deco_tm = Tilemap::new(tiles_w as u16, tiles_h as u16);

            for ty in 0..tiles_h {
                for tx in 0..tiles_w {
                    let mut world_tx = tile_start_tx + tx;
                    let world_ty = tile_start_ty + ty;

                    // S.S. Anne departure scroll: the revealed east columns
                    // wrap back to the map's west edge (the original's
                    // ScheduleEastColumnRedraw re-draws the east column from
                    // the screen tile buffer, which wraps at the map width).
                    if departure_active {
                        world_tx = world_tx.rem_euclid(map_w as i32 * 4);
                    }

                    let bx = world_tx.div_euclid(4);
                    let mut by = world_ty.div_euclid(4);
                    // ShakeElevator edge wrap: the ±1px hSCY scroll exposes a
                    // 1px row of the tilemap row adjacent to the viewport.
                    // The original's tilemap keeps that row a map row
                    // (ShakeElevatorRedrawRow); the port's border tiles would
                    // render as a white/void line at the map edge, so while
                    // the shake is active the exposed rows WRAP back into the
                    // map (the GB's tilemap-wrap semantics). In-bounds rows
                    // are unchanged.
                    if shake_offset_y != 0 {
                        by = by.rem_euclid(map_h as i32);
                    }
                    let sub_x = world_tx.rem_euclid(4) as usize;
                    let sub_y = world_ty.rem_euclid(4) as usize;

                    let block_id = resolve_block_with_connections(
                        current_map,
                        map_w,
                        map_h,
                        blk,
                        border_block,
                        bx,
                        by,
                    );

                    let tile_idx = blockset_data::block_tiles(tileset_id, block_id)
                        .map(|t| t[sub_y * 4 + sub_x] as u16)
                        .unwrap_or(0);

                    let ground_entry = TilemapEntry {
                        tile_id: if sub_y <= 1 { tile_idx } else { 0 },
                        palette_group: if sub_y <= 1 { 0 } else { 255 },
                        ..Default::default()
                    };
                    let deco_entry = TilemapEntry {
                        tile_id: if sub_y <= 1 { 0 } else { tile_idx },
                        palette_group: if sub_y <= 1 { 255 } else { 0 },
                        ..Default::default()
                    };

                    ground_tm.set(tx as u16, ty as u16, ground_entry);
                    deco_tm.set(tx as u16, ty as u16, deco_entry);
                }
            }

            let ground_layer = MapLayer::new(ground_tm, 0);
            let deco_layer = MapLayer::new(deco_tm, 1);

            // Tile-to-colour callback: palette_group 255 is the transparent sentinel.
            let tile_color = |tile_id: u16, pal_group: u8, px: u8, py: u8| -> Rgba {
                if pal_group == 255 {
                    return Rgba::TRANSPARENT;
                }
                // Animated flower: tile $03 shows the current flower frame.
                if tile_id == ANIM_FLOWER_TILE as u16 {
                    if let Some(ref fts) = flower_ts {
                        let tile = fts.get(0);
                        let color_idx = tile.pixels[py as usize][px as usize];
                        return pal.color(GbColor::from_u8(color_idx));
                    }
                }
                let tile = ts.get(tile_id as usize);
                // Animated water: tile $14's rows rotate horizontally.
                if tile_id == ANIM_WATER_TILE as u16 && water_shift != 0 {
                    let sx = (px as i32 - water_shift).rem_euclid(TILE_SIZE as i32) as usize;
                    let color_idx = tile.pixels[py as usize][sx];
                    return pal.color(GbColor::from_u8(color_idx));
                }
                let color_idx = tile.pixels[py as usize][px as usize];
                pal.color(GbColor::from_u8(color_idx))
            };

            render_layers(
                fb,
                &[ground_layer, deco_layer],
                camera_x,
                camera_y,
                fb.width(),
                fb.height(),
                tile_color,
            );
        }

        // Player sprite: 16×96 sheet = 6 frames of 16×16
        // Frame layout: DownStand=0, UpStand=1, LeftStand=2, DownWalk=3, UpWalk=4, LeftWalk=5
        // Right uses Left frames with horizontal flip
        // Biking swaps the sheet to red_bike.png (same 6-frame layout) — the
        // original's LoadBikePlayerSpriteGraphics loads RedBikeSprite
        // (gfx/sprites.asm:34) while wWalkBikeSurfState == 1; the frame and
        // flip selection below is shared by both sheets.
        let player_sprite = if screen.state.player.transport == TransportMode::Biking {
            "red_bike"
        } else {
            "red"
        };
        if let Ok(cached) = rm.load_sprite(player_sprite) {
            let ts = cached.tileset.clone();

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
            let spin_y_offset = spin.map_or(0, |s| s.player_y_offset());
            let enter_y_offset = enter.map_or(0, |s| s.player_y_offset());
            let fishing_shake_offset = fishing.map_or(0, |f| f.player_shake_offset());
            let player_visible = spin.map_or(true, |s| s.player_visible());
            let player_visible = enter.map_or(player_visible, |s| s.player_visible());
            let fishing_pose = fishing.map_or(false, |f| f.pose_active());

            let (frame, flip_h) = if screen.state.player.movement_state == MovementState::Walking
                || screen.state.player.movement_state == MovementState::Jumping
            {
                let walk_frame = screen.state.walk_counter > 4;
                match player_facing {
                    Direction::Down => (if walk_frame { 3 } else { 0 }, false),
                    Direction::Up => (if walk_frame { 4 } else { 1 }, false),
                    Direction::Left => (if walk_frame { 5 } else { 2 }, false),
                    Direction::Right => (if walk_frame { 5 } else { 2 }, true),
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

            // Ledge jump: compute vertical arc offset from the original game's
            // PlayerJumpingYScreenCoords table. The table stores absolute Y positions;
            // we convert to offsets from baseline. walk_counter counts 16 → 0, so
            // jump frame index = 16 - walk_counter.
            //
            // Original table (baseline $3C = 60):
            //   $38,$36,$34,$32,$31,$30,$30,$30,$31,$32,$33,$34,$36,$38,$3C,$3C
            // Offsets from baseline:
            //   -4, -6, -8,-10,-11,-12,-12,-12,-11,-10, -9, -8, -6, -4,  0,  0
            const JUMP_Y_OFFSETS: [i32; 16] = [
                -4, -6, -8, -10, -11, -12, -12, -12, -11, -10, -9, -8, -6, -4, 0, 0,
            ];

            // Original game scrolls the background via AdvancePlayerSprite (2px/frame)
            // while _HandleMidJump applies the arc. Our map is tile-snapped, so we
            // offset the sprite from screen center: 1px/frame × 16 frames = 16px = 2 tiles.
            let (jump_translate_x, jump_translate_y, jump_arc_offset) =
                if screen.state.player.movement_state == MovementState::Jumping {
                    let wc = screen.state.walk_counter as i32;
                    let elapsed = 16 - wc;
                    let (tdx, tdy) = match screen.state.player.facing {
                        Direction::Down => (0, elapsed),
                        Direction::Up => (0, -elapsed),
                        Direction::Left => (-elapsed, 0),
                        Direction::Right => (elapsed, 0),
                    };
                    let idx = (elapsed as usize).min(JUMP_Y_OFFSETS.len() - 1);
                    (tdx, tdy, JUMP_Y_OFFSETS[idx])
                } else {
                    (0, 0, 0)
                };
            if screen.state.player.movement_state == MovementState::Jumping && jump_arc_offset < 0 {
                let shadow_cx = (player_px_x as i32 + jump_translate_x + 8) as i32;
                let shadow_cy = (player_px_y as i32 + jump_translate_y + 15) as i32;
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

            let draw_x = (player_px_x as i32 + jump_translate_x).max(0) as u32;
            let draw_y = (player_px_y as i32
                + jump_translate_y
                + jump_arc_offset
                + spin_y_offset
                + enter_y_offset
                + fishing_shake_offset)
                .max(0) as u32;

            // Fishing pose: RedFishingTilesFront/Back/Side (gfx/fishing.asm)
            // replace the BOTTOM two tiles of the standing sprite while the
            // rod is out (the original loads them at the standing sprite's
            // tile slots $02/$06/$0a); the top half keeps the normal head.
            // Right uses the side pose with a horizontal flip.
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
                            &ts,
                        ),
                    };
                    if tile_idx >= tile_ts.len() {
                        continue;
                    }

                    blit_single_tile_flipped(
                        fb,
                        tile_ts,
                        tile_idx,
                        draw_x + col * TILE_SIZE,
                        draw_y + row * TILE_SIZE,
                        &sprite_pal,
                        flip_h,
                    );
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
                    let bg_ts = bg_cached.tileset.clone();
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
                            current_map,
                            map_w,
                            map_h,
                            blk,
                            border_block,
                            bx,
                            by,
                        );
                        let bg_tile_idx = blockset_data::block_tiles(tileset_id, block_id)
                            .map(|t| t[sub_y * 4 + sub_x] as usize)
                            .unwrap_or(0)
                            .min(bg_ts.len().saturating_sub(1));
                        if bg_tile_idx == grass_id as usize {
                            let tile = bg_ts.get(bg_tile_idx);
                            let gx = overlay_x + col_off as u32 * TILE_SIZE;
                            for row in 0..TILE_SIZE {
                                for col in 0..TILE_SIZE {
                                    let ci = tile.pixels[row as usize][col as usize];
                                    if ci != 0 {
                                        let sx = gx + col;
                                        let sy = overlay_y + row;
                                        if sx < fb.width() && sy < fb.height() {
                                            fb.set_pixel(sx, sy, pal.color(GbColor::from_u8(ci)));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        for npc in &screen.npc_states {
            if !npc.visible {
                continue;
            }

            let sprite_id = match SpriteId::from_u8(npc.sprite_id) {
                Some(id) => id,
                None => continue,
            };

            let sprite_name = sprite_id.sprite_name();
            if let Ok(cached) = rm.load_sprite(sprite_name) {
                let ts = cached.tileset.clone();
                let num_frames = (cached.source_size.1 / TILE_SIZE) as usize;

                let npc_facing = npc.facing;

                let (frame, flip_h) = if let Some(sf) = npc.scripted_frame {
                    (sf as usize, false)
                } else if num_frames >= 6 {
                    // 4-frame cycle with even timing: each frame 2 walk_counter units
                    // AnimFrame 0-3: 0/2=stand, 1=walk, 3=walk+flip
                    let anim_frame = if npc.walk_counter == 0 {
                        0 // Idle
                    } else if npc.walk_counter > 6 {
                        0 // 7,8: stand
                    } else if npc.walk_counter > 4 {
                        1 // 5,6: walk
                    } else if npc.walk_counter > 2 {
                        2 // 3,4: stand
                    } else {
                        3 // 1,2: walk+flip
                    };

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

                // Smooth pixel interpolation during movement.
                // Original GB moves sprites 2px/frame (16px/tile ÷ 8 frames).
                let (walk_dx, walk_dy) = if npc.walk_counter > 0 {
                    let elapsed = (8u8.saturating_sub(npc.walk_counter)) as i32;
                    let px = elapsed * 2;
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

                for row in 0..2_u32 {
                    for col in 0..2_u32 {
                        let src_col = if flip_h { 1 - col } else { col };
                        let tile_idx = base_tile + (row as usize * tpr as usize) + src_col as usize;
                        if tile_idx >= ts.len() {
                            continue;
                        }

                        let tx = npc_px_x + (col * TILE_SIZE) as i32;
                        let ty = npc_px_y + (row * TILE_SIZE) as i32;
                        blit_tile_clipped_flipped(fb, &ts, tile_idx, tx, ty, &sprite_pal, flip_h);
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
                    let ts = cached.tileset.clone();
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
                            blit_tile_clipped_flipped(
                                fb,
                                &ts,
                                tile_idx,
                                tx,
                                ty,
                                &sprite_pal,
                                false,
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
                let ts = cached.tileset.clone();
                let tpr = cached.source_size.0 / TILE_SIZE;
                if let Some(npc) = screen
                    .npc_states
                    .iter()
                    .find(|n| n.visible && format!("{}", n.npc_index) == bubble.npc_id)
                {
                    let npc_screen_tx = npc.x as i32 * 2 - view_origin_tx;
                    let npc_screen_ty = npc.y as i32 * 2 - view_origin_ty;
                    let (walk_dx, walk_dy) = if npc.walk_counter > 0 {
                        let elapsed = (8u8.saturating_sub(npc.walk_counter)) as i32;
                        let px = elapsed * 2;
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
                            blit_tile_clipped(fb, &ts, tile_idx, tx, ty, &sprite_pal);
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
                    let rod_ts = cached.tileset.clone();
                    blit_tile_clipped_flipped(
                        fb,
                        &rod_ts,
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
                    let ts = cached.tileset.clone();
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
                            blit_tile_clipped(fb, &ts, tile_idx, tx, ty, &sprite_pal);
                        }
                    }
                }
            }
        }

        if let Some(ref healing_state) = screen.pending_healing_machine {
            if let Ok(cached) = rm.load_asset(AssetCategory::Overworld, "heal_machine.png") {
                let ts = cached.tileset.clone();

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
                let heal_pal = if healing_state.flash_active { &obp1_flash } else { &obp1_pal };

                // PokeCenterOAMData offsets relative to nurse sprite top-left.
                // Nurse renders at map pos (3,1) as a 16×16 NPC sprite.
                // Original OAM screen positions (player at (3,3), nurse screen=(72,32)):
                //   monitor: (44,20) → delta (-28,-12)
                //   balls: (40,27)(48,27)(40,32)(48,32)(40,37)(48,37)
                const MONITOR_DX: i32 = -20;
                const MONITOR_DY: i32 = -12;
                const BALL_OAM: [(i32, i32, bool); 6] = [
                    (-24, -5, false), (-16, -5, true),
                    (-24,  0, false), (-16,  0, true),
                    (-24,  5, false), (-16,  5, true),
                ];

                let nurse_x = 3_i32;
                let nurse_y = 1_i32;
                let nurse_px_x = (nurse_x * 2 - view_origin_tx) * TILE_SIZE as i32 - view_sub_x;
                let nurse_px_y = (nurse_y * 2 - view_origin_ty) * TILE_SIZE as i32 - view_sub_y;

                if ts.len() > 0 {
                    blit_tile_clipped(
                        fb, &ts, 0,
                        nurse_px_x + MONITOR_DX, nurse_px_y + MONITOR_DY,
                        heal_pal,
                    );
                }

                let count = (healing_state.pokeballs_visible as usize).min(BALL_OAM.len());
                for i in 0..count {
                    let (dx, dy, flip) = BALL_OAM[i];
                    if 1 < ts.len() {
                        blit_tile_clipped_flipped(
                            fb, &ts, 1,
                            nurse_px_x + dx, nurse_px_y + dy,
                            heal_pal, flip,
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
                let ts = cached.tileset.clone();
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
                    blit_tile_clipped(fb, &ts, 0, tx, ty, dust_pal);
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
                    let ts = cached.tileset.clone();
                    // rOBP1=%00000000: idx 0→transparent, 1-3→white.
                    let obp1_pal = Palette::new(&[
                        Rgba::TRANSPARENT,
                        Rgba::WHITE,
                        Rgba::WHITE,
                        Rgba::WHITE,
                    ]);
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
                                &ts,
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
        let total = draw_pokedex_entry(&dex_state.species, dex_state.page, res, fb);
        dex_state.total_pages = total;
        return;
    }

    if let Some(ref dlg) = screen.pending_dialogue {
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
            menus::dialog::draw(&combined, show_arrow, &DIALOG_DEFAULT_LAYOUT, &mut ui, language);
        }

        if let Some(ref choice) = screen.pending_choice {
            let mut painter = FrameBufferPainter::new(fb);
            let mut ui = Ui::new(&mut painter);
            menus::yes_no::draw(&choice.options, choice.selected, &YES_NO_DEFAULT_LAYOUT, &mut ui);
        }

        return;
    }

    if let Some(ref choice) = screen.pending_choice {
        let mut painter = FrameBufferPainter::new(fb);
        let mut ui = Ui::new(&mut painter);
        menus::yes_no::draw(&choice.options, choice.selected, &YES_NO_DEFAULT_LAYOUT, &mut ui);
        return;
    }

    // ── GB palette effects (home/fade.asm) ─────────────────────────
    // Priority: FLASH white-out > dark cave (LoadGBPal with wMapPalOffset=6)
    // > warp fade. Entering a dark cave fades OUT to black, but the arrival
    // applies the dark palette instantly via LoadGBPal (no fade-in), which
    // this ordering reproduces.
    if screen.flash_lit_frames > 0 {
        // GBPalWhiteOutWithDelay3: all palettes to white.
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
    fn dark_cave_palette_is_fadepal2() {
        // LoadGBPal with wMapPalOffset=6 reads FadePal4 - 6 bytes = FadePal2.
        let pal = dotzuki_renderer::transition::load_gb_pal(6);
        assert_eq!(pal, FADE_PALETTES[1]);
        assert_eq!(pal.bgp, 0xFE); // dc 3,3,3,2
    }

    /// Render the overworld to a framebuffer and count the GRAYSCALE shades.
    #[cfg(not(target_arch = "wasm32"))]
    fn shade_histogram(screen: &mut OverworldScreen) -> [usize; 4] {
        let mut res = pokered_renderer::resource::AssetRoot::auto_detect().ok().map(pokered_renderer::resource::ResourceManager::new);
        let mut fb = FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            Rgba::WHITE,
        );
        draw_overworld(screen, &mut res, &mut fb, pokered_core::game_state::Lang::En);
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
            pokered_data::tileset_data::TileAnimation::WaterFlower
        );
        let mut res = pokered_renderer::resource::AssetRoot::auto_detect().ok().map(pokered_renderer::resource::ResourceManager::new);
        let mut fb_a = FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            Rgba::WHITE,
        );
        draw_overworld(&mut s, &mut res, &mut fb_a, pokered_core::game_state::Lang::En);
        // 20 ticks = one water update (one-pixel rotation of tile $14).
        for _ in 0..20 {
            s.tile_anim.tick();
        }
        assert_eq!(s.tile_anim.water_shift(), 1);
        let mut fb_b = FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            Rgba::WHITE,
        );
        draw_overworld(&mut s, &mut res, &mut fb_b, pokered_core::game_state::Lang::En);
        assert_ne!(fb_a.packed(), fb_b.packed(), "water rotation changes the frame");
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
        draw_overworld(screen, &mut res, &mut fb, pokered_core::game_state::Lang::En);
        fb
    }

    fn render_with_shake_at(map: MapId, x: u16, y: u16, offset_frame: u16) -> FrameBuffer {
        let mut screen = OverworldScreen::new(map, None, PokemonRedData);
        screen.state.player.x = x;
        screen.state.player.y = y;
        let mut shake = ElevatorShakeState::new();
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
        let mut shake = ElevatorShakeState::new();
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
        let mut ref_screen =
            OverworldScreen::new(MapId::CeladonMansion3F, None, PokemonRedData);
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
        let mut shake = ElevatorShakeState::new();
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
        s.npc_states.push(dotzuki_engine::overworld::npc_movement::NpcRuntimeState {
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
        let dust_area_changed = (88..96).any(|x| {
            (116..132)
                .any(|y| before.get_pixel(x, y) != after.get_pixel(x, y))
        });
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

use pokered_core::data::blockset_data;
use pokered_core::data::map_data_loader::{get_block_data, get_map_json, resolve_map_id};
use pokered_core::data::maps::MapId;
use pokered_core::data::sprites::SpriteId;
use pokered_core::data::tileset_data;
use pokered_core::overworld::{Direction, MovementState, OverworldScreen};
use pokered_data::impl_traits::PokemonTilesetData;
use dotzuki_engine::overworld::types::TransportMode;
use dotzuki_engine::tileset::TilesetProvider;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::palette::{GbColor, Palette, GRAYSCALE_PALETTE};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use pokered_renderer::tile::TileSet;

use super::{blit_single_tile_flipped, draw_text_box, species_to_sprite_name};

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

struct DexEntry {
    dex_num: u16,
    category: &'static str,
    height_ft: u8,
    height_in: u8,
    weight_lb: &'static str,
    description: &'static [&'static str],
}

fn get_starter_dex_entry(species: &str) -> Option<DexEntry> {
    match species.to_uppercase().as_str() {
        "BULBASAUR" => Some(DexEntry {
            dex_num: 1,
            category: "SEED",
            height_ft: 2,
            height_in: 4,
            weight_lb: "15.0",
            description: &[
                "A strange seed was",
                "planted on its",
                "back at birth.",
                "The plant sprouts",
                "and grows with",
                "this POKeMON",
            ],
        }),
        "CHARMANDER" => Some(DexEntry {
            dex_num: 4,
            category: "LIZARD",
            height_ft: 2,
            height_in: 0,
            weight_lb: "19.0",
            description: &[
                "Obviously prefers",
                "hot places. When",
                "it rains, steam",
                "is said to spout",
                "from the tip of",
                "its tail",
            ],
        }),
        "SQUIRTLE" => Some(DexEntry {
            dex_num: 7,
            category: "TINYTURTLE",
            height_ft: 1,
            height_in: 8,
            weight_lb: "20.0",
            description: &[
                "After birth, its",
                "back swells and",
                "hardens into a",
                "shell. Powerfully",
                "sprays foam from",
                "its mouth",
            ],
        }),
        _ => None,
    }
}

fn draw_pokedex_entry(
    species: &str,
    page: usize,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) -> usize {
    // Layout from engine/menus/pokedex.asm ShowPokedexDataInternal
    fb.clear(Rgba::WHITE);
    let pal = &GRAYSCALE_PALETTE;
    let fg = Rgba::BLACK;
    let bg = Rgba::WHITE;
    let t = TILE_SIZE;

    let entry = match get_starter_dex_entry(species) {
        Some(e) => e,
        None => {
            draw_text(species, t, t, fg, fb);
            draw_text("No data.", t, t * 3, fg, fb);
            return 1;
        }
    };

    use pokered_renderer::embedded_font::{box_tiles, draw_box_tile, fill_tile};

    draw_box_tile(&box_tiles::TOP_LEFT, &box_tiles::outside::TOP_LEFT, 0, 0, fg, bg, fb);
    draw_box_tile(&box_tiles::TOP_RIGHT, &box_tiles::outside::TOP_RIGHT, 19 * t, 0, fg, bg, fb);
    draw_box_tile(&box_tiles::BOTTOM_LEFT, &box_tiles::outside::BOTTOM_LEFT, 0, 17 * t, fg, bg, fb);
    draw_box_tile(&box_tiles::BOTTOM_RIGHT, &box_tiles::outside::BOTTOM_RIGHT, 19 * t, 17 * t, fg, bg, fb);
    for col in 1..19u32 {
        draw_box_tile(&box_tiles::HORIZONTAL, &box_tiles::outside::HORIZONTAL, col * t, 0, fg, bg, fb);
        draw_box_tile(&box_tiles::HORIZONTAL_BOTTOM, &box_tiles::outside::HORIZONTAL_BOTTOM, col * t, 17 * t, fg, bg, fb);
    }
    for row in 1..17u32 {
        draw_box_tile(&box_tiles::VERTICAL_LEFT, &box_tiles::outside::VERTICAL_LEFT, 0, row * t, fg, bg, fb);
        draw_box_tile(&box_tiles::VERTICAL_RIGHT, &box_tiles::outside::VERTICAL_RIGHT, 19 * t, row * t, fg, bg, fb);
    }
    for row in 1..17u32 {
        for col in 1..19u32 {
            fill_tile(col * t, row * t, bg, fb);
        }
    }

    // Sprite at (1,1) in 7×7 tile area. Smaller sprites are bottom-aligned and centered.
    // Horizontal flip: reverse tile column order + flip each tile's pixels.
    if let Some(ref mut rm) = res {
        let sprite_name = species_to_sprite_name(species);
        if let Ok(cached) = rm.load_pokemon_front(&sprite_name) {
            let ts = cached.tileset.clone();
            let sprite_w = cached.source_size.0;
            let sprite_h = cached.source_size.1;
            let tiles_w = sprite_w / t;
            let tiles_h = sprite_h / t;
            let area_x = 1 * t;
            let area_y = 1 * t;
            let x_offset = ((7 - tiles_w + 1) / 2) * t;
            let y_offset = (7 - tiles_h) * t;
            for idx in 0..ts.len() {
                let tx = (idx as u32) % tiles_w;
                let ty = (idx as u32) / tiles_w;
                let flipped_tx = tiles_w - 1 - tx;
                blit_single_tile_flipped(
                    fb,
                    &ts,
                    idx,
                    area_x + x_offset + flipped_tx * t,
                    area_y + y_offset + ty * t,
                    pal,
                    true,
                );
            }
        }
    }

    let display_name = species.to_uppercase();
    draw_text(&display_name, 9 * t, 2 * t, fg, fb); // (9,2)
    draw_text(entry.category, 9 * t, 4 * t, fg, fb); // (9,4)
    draw_text("HT", 9 * t, 6 * t, fg, fb); // (9,6)
    let feet_str = format!("{}", entry.height_ft);
    draw_text(&feet_str, 12 * t, 6 * t, fg, fb); // (12,6)
    draw_text("'", (12 + feet_str.len() as u32) * t, 6 * t, fg, fb);
    let inches_str = format!("{:02}", entry.height_in);
    draw_text(&inches_str, 15 * t, 6 * t, fg, fb); // (15,6)
    draw_text("\"", 17 * t, 6 * t, fg, fb);
    draw_text("WT", 9 * t, 8 * t, fg, fb); // (9,8)
    let wt_str = format!("{}lb", entry.weight_lb);
    draw_text(&wt_str, 11 * t, 8 * t, fg, fb); // (11,8)
    let num_line = format!("No.{:03}", entry.dex_num);
    draw_text(&num_line, 2 * t, 8 * t, fg, fb); // (2,8)

    for col in 0..20u32 {
        let px = col * t;
        let py = 9 * t;
        for y_off in 3..5u32 {
            for x_off in 0..t {
                let sx = px + x_off;
                let sy = py + y_off;
                if sx < fb.width() && sy < fb.height() {
                    fb.set_pixel(sx, sy, fg);
                }
            }
        }
    }

    // Description: 2-tile row spacing, 3 lines per page (rows 11-16 = 6 tile rows)
    let lines_per_page = 3;
    let total_pages = (entry.description.len() + lines_per_page - 1) / lines_per_page;
    let page = page.min(total_pages.saturating_sub(1));
    let start = page * lines_per_page;
    let page_lines = &entry.description[start..entry.description.len().min(start + lines_per_page)];
    for (i, line) in page_lines.iter().enumerate() {
        draw_text(line, 1 * t, (11 + i as u32 * 2) * t, fg, fb); // (1,11), (1,13), (1,15)
    }
    if page + 1 < total_pages {
        draw_text("▼", 18 * t, 16 * t, fg, fb);
    }
    total_pages
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
) {
    fb.clear(Rgba::WHITE);

    // Naming screen open/submit white flash (GBPalWhiteOutWithDelay3).
    if screen.naming_flash_frames > 0 {
        return;
    }

    if let Some(ref naming) = screen.pending_naming_screen {
        super::draw_naming_screen(naming, fb);
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

        let (map_w, map_h) = current_map.dimensions();
        let blk = get_block_data(current_map);

        // S.S. Anne departure (VermilionDockSSAnneLeavesScript): the view
        // scrolls east up to 16 tiles while the ship sails away — the map
        // content slides left (wMapViewVRAMPointer += 2 per iteration).
        let departure_scroll =
            screen.ship_departure.as_ref().map_or(0, |d| d.scroll_px());
        let departure_active = screen.ship_departure.is_some();

        if let Ok(cached) = rm.load_tileset(tileset_name) {
            let ts = cached.tileset.clone();

            let sx_start: i32 = if view_sub_x < 0 { -2 } else { 0 };
            let sx_end: i32 = if view_sub_x > 0 { 22 } else { 20 };
            let sy_start: i32 = if view_sub_y < 0 { -2 } else { 0 };
            let sy_end: i32 = if view_sub_y > 0 { 20 } else { 18 };

            for sy in sy_start..sy_end {
                for sx in sx_start..sx_end {
                    let mut world_tx =
                        view_origin_tx + sx + departure_scroll / 4;
                    let world_ty = view_origin_ty + sy;

                    // S.S. Anne departure scroll: the revealed east columns
                    // wrap back to the map's west edge (the original's
                    // ScheduleEastColumnRedraw re-draws the east column from
                    // the screen tile buffer, which wraps at the map width).
                    if departure_active {
                        world_tx = world_tx.rem_euclid(map_w as i32 * 4);
                    }

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

                    let tile_idx = blockset_data::block_tiles(tileset_id, block_id)
                        .map(|t| t[sub_y * 4 + sub_x] as usize)
                        .unwrap_or(0)
                        .min(ts.len().saturating_sub(1));

                    let draw_x = sx * TILE_SIZE as i32 - view_sub_x;
                    let draw_y = sy * TILE_SIZE as i32 - view_sub_y;
                    blit_tile_clipped(fb, &ts, tile_idx, draw_x, draw_y, pal);
                }
            }
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

            // FishingAnim (player_animations.asm:378-469): while the rod is
            // out, the player sprite is swapped to the fishing pose
            // (RedFishingTiles — the bottom two tiles) and the sprite shakes
            // ±1 px vertically on a bite (.ShakePlayerSprite).
            let fishing = screen.fishing_anim.as_ref();
            // EnterMapAnim spin-in (FLY/TELEPORT/DIG/ESCAPE ROPE/dungeon
            // arrivals): the sprite descends from off the top and spins in
            // place after the fade-in-from-white.
            let enter = screen.enter_map_anim.as_ref();
            let player_facing = fishing
                .map(|f| f.facing())
                .or_else(|| enter.map(|s| s.facing()))
                .unwrap_or(screen.state.player.facing);
            let enter_y_offset = enter.map_or(0, |s| s.player_y_offset());
            let player_visible = enter.map_or(true, |s| s.player_visible());
            let fishing_shake_offset = fishing.map_or(0, |f| f.player_shake_offset());
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
                rm.load_asset(pokered_renderer::resource::AssetCategory::Overworld, asset)
                    .ok()
                    .map(|c| c.tileset.clone())
            } else {
                None
            };

            for row in 0..2_u32 {
                if !player_visible {
                    break;
                }
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
                if let Ok(cached) = rm.load_asset(
                    pokered_renderer::resource::AssetCategory::Overworld,
                    "fishing_rod.png",
                ) {
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
            if let Ok(cached) = rm.load_asset(
                pokered_renderer::resource::AssetCategory::Overworld,
                "smoke.png",
            ) {
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
                if let Ok(cached) = rm.load_asset(
                    pokered_renderer::resource::AssetCategory::Overworld,
                    "smoke.png",
                ) {
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
                    let anchor_x =
                        pokered_core::overworld::presentation::SHIP_DEPARTURE_SMOKESTACK_TILE_X
                            as i32
                            * TILE_SIZE as i32
                            - view_origin_tx * 4;
                    let anchor_y = dep.puff_screen_y() - view_origin_ty * 4;
                    let rebase = anchor_x
                        - pokered_core::overworld::presentation::SHIP_DEPARTURE_PUFF_START_SCREEN_X;
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
            let text_box_x = 0_u32;
            let text_box_y = 12 * TILE_SIZE;
            draw_text_box(fb, text_box_x, text_box_y, 18, 4, Rgba::BLACK);
            draw_text(
                &d1,
                text_box_x + TILE_SIZE,
                text_box_y + TILE_SIZE,
                Rgba::BLACK,
                fb,
            );
            draw_text(
                &d2,
                text_box_x + TILE_SIZE,
                text_box_y + TILE_SIZE * 3,
                Rgba::BLACK,
                fb,
            );
            // Blinking ▼ arrow indicator (matches original ManualTextScroll behavior).
            // Toggle visibility every 16 frames (~267ms at 60fps). Same tile
            // (18,16) as the native frontend (bottom-right of the 20×6 box).
            if dlg.waiting_for_input() {
                let blink_visible = (screen.frame_counter / 16) % 2 == 0;
                if blink_visible {
                    let arrow_x = 18 * TILE_SIZE;
                    let arrow_y = 16 * TILE_SIZE;
                    draw_text("▼", arrow_x, arrow_y, Rgba::BLACK, fb);
                }
            }
        }

        if let Some(ref choice) = screen.pending_choice {
            // Match the native yes/no layout: 9×6 box at tile (11,8) — i.e.
            // 7×4 interior for draw_text_box — cursor at tile 12, options at 13.
            let box_x = 11 * TILE_SIZE;
            let box_y = 8 * TILE_SIZE;
            let box_w = 7_u32;
            let box_h = 4_u32;
            draw_text_box(fb, box_x, box_y, box_w, box_h, Rgba::BLACK);
            for (i, opt) in choice.options.iter().enumerate() {
                let opt_y = box_y + TILE_SIZE * (1 + i as u32 * 2);
                draw_text(opt, box_x + 2 * TILE_SIZE, opt_y, Rgba::BLACK, fb);
            }
            let cursor_y = box_y + TILE_SIZE * (1 + choice.selected as u32 * 2);
            draw_text("▶", box_x + TILE_SIZE, cursor_y, Rgba::BLACK, fb);
        }

        return;
    }

    if let Some(ref choice) = screen.pending_choice {
        // Match the native yes/no layout: 9×6 box at tile (11,8) — i.e.
        // 7×4 interior for draw_text_box — cursor at tile 12, options at 13.
        let box_x = 11 * TILE_SIZE;
        let box_y = 8 * TILE_SIZE;
        let box_w = 7_u32;
        let box_h = 4_u32;
        draw_text_box(fb, box_x, box_y, box_w, box_h, Rgba::BLACK);
        for (i, opt) in choice.options.iter().enumerate() {
            let opt_y = box_y + TILE_SIZE * (1 + i as u32 * 2);
            draw_text(opt, box_x + 2 * TILE_SIZE, opt_y, Rgba::BLACK, fb);
        }
        let cursor_y = box_y + TILE_SIZE * (1 + choice.selected as u32 * 2);
        draw_text("▶", box_x + TILE_SIZE, cursor_y, Rgba::BLACK, fb);
        return;
    }

    let fade_progress = screen.warp_fade_progress();
    if fade_progress > 0.0 {
        let darkness = (fade_progress.clamp(0.0, 1.0) * 255.0) as u8;
        for y in 0..fb.height() {
            for x in 0..fb.width() {
                if let Some(pixel) = fb.get_pixel(x, y) {
                    let r = ((pixel.r as u16) * (255 - darkness as u16) / 255) as u8;
                    let g = ((pixel.g as u16) * (255 - darkness as u16) / 255) as u8;
                    let b = ((pixel.b as u16) * (255 - darkness as u16) / 255) as u8;
                    fb.set_pixel(x, y, Rgba::rgb(r, g, b));
                }
            }
        }
    }
}

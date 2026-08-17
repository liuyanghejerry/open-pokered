//! Integration tests for `OverworldScreen::use_field_move` and the boulder
//! push — the live wiring of the HM field effects (CUT / SURF / STRENGTH /
//! FLY / FLASH / DIG / TELEPORT) from the party menu.

use super::field_moves::{FieldMoveOutcome, BOULDER_DUST_FRAMES};
use super::hm_effects;
use super::screen::{OverworldScreen, PendingWarp, WarpFadeState};
use super::{Direction, TransportMode};
use dotzuki_engine::overworld::npc_movement::NpcRuntimeState;
use dotzuki_engine::overworld::types::NpcMovementType;
use pokered_data::blockset_data;
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;
use pokered_data::moves::MoveId;
use pokered_data::tileset_data::{
    cut_tree_replacement, CUT_TREE_TILE_OVERWORLD,
};
use pokered_data::tilesets::TilesetId;

const NO_BADGES: u8 = 0;
const CASCADE: u8 = 1 << 1; // BIT_CASCADEBADGE
const THUNDER: u8 = 1 << 2; // BIT_THUNDERBADGE
const RAINBOW: u8 = 1 << 3; // BIT_RAINBOWBADGE
const SOUL: u8 = 1 << 4; // BIT_SOULBADGE
const BOULDER: u8 = 1 << 0; // BIT_BOULDERBADGE

fn screen_on(map: MapId) -> OverworldScreen<PokemonRedData> {
    OverworldScreen::new(map, None, PokemonRedData)
}

fn test_mon() -> crate::battle::state::Pokemon {
    crate::pokemon::stats::create_pokemon(pokered_data::species::Species::Squirtle, 5, [0xFF, 0xFF])
        .unwrap()
}

fn dialogue_text(screen: &OverworldScreen<PokemonRedData>) -> Option<String> {
    let dlg = screen.pending_dialogue.as_ref()?;
    let page = dlg.current()?;
    Some(format!("{}\n{}", page.line1, page.line2))
}

/// Find a block in the Overworld blockset whose tile at a player-readable
/// sub-index (`(sub_y*2+1)*4 + sub_x*2` ∈ {4,6,12,14}) equals `tile`,
/// and that also satisfies `extra`. Returns (block_id, sub_x, sub_y).
fn find_block_with_tile(tile: u8, extra: impl Fn(u8) -> bool) -> Option<(u8, u16, u16)> {
    for block in 0u8..=255 {
        if !extra(block) {
            continue;
        }
        let Some(tiles) = blockset_data::block_tiles(TilesetId::Overworld, block) else {
            break;
        };
        for (sub_x, sub_y) in [(0u16, 0u16), (1, 0), (0, 1), (1, 1)] {
            let idx = ((sub_y * 2 + 1) * 4 + sub_x * 2) as usize;
            if tiles[idx] == tile {
                return Some((block, sub_x, sub_y));
            }
        }
    }
    None
}

/// Place `block` at map block (bx,by) and position the player so that the
/// tile in front of them (facing `dir`) reads sub-tile (sub_x,sub_y) of it.
fn place_block_in_front(
    screen: &mut OverworldScreen<PokemonRedData>,
    block: u8,
    sub_x: u16,
    sub_y: u16,
    dir: Direction,
) {
    let (bx, by) = (5u8, 5u8);
    screen
        .map_data
        .as_mut()
        .expect("map_data present")
        .set_block(bx, by, block);
    let front_x = (bx as u16) * 2 + sub_x;
    let front_y = (by as u16) * 2 + sub_y;
    let (dx, dy) = match dir {
        Direction::Down => (0i16, 1),
        Direction::Up => (0, -1),
        Direction::Left => (-1, 0),
        Direction::Right => (1, 0),
    };
    screen.state.player.x = (front_x as i16 - dx) as u16;
    screen.state.player.y = (front_y as i16 - dy) as u16;
    screen.state.player.facing = dir;
}

/// Find a block whose tile at a readable sub-index satisfies `pred`.
fn find_block_matching(pred: impl Fn(u8) -> bool) -> Option<(u8, u16, u16)> {
    for block in 0u8..=255 {
        let Some(tiles) = blockset_data::block_tiles(TilesetId::Overworld, block) else {
            break;
        };
        for (sub_x, sub_y) in [(0u16, 0u16), (1, 0), (0, 1), (1, 1)] {
            let idx = ((sub_y * 2 + 1) * 4 + sub_x * 2) as usize;
            if pred(tiles[idx]) {
                return Some((block, sub_x, sub_y));
            }
        }
    }
    None
}

/// Find a block passable at every player-readable sub-index, and fill the
/// whole map with it — guarantees open ground for boulder-push tests.
fn fill_map_with_passable_block(screen: &mut OverworldScreen<PokemonRedData>) {
    let block = (0u8..=255)
        .find(|&b| {
            let Some(tiles) = blockset_data::block_tiles(TilesetId::Overworld, b) else {
                return false;
            };
            [4usize, 6, 12, 14]
                .iter()
                .all(|&i| pokered_data::collision::is_tile_passable(TilesetId::Overworld, tiles[i]))
        })
        .expect("blockset has a fully passable block");
    let map = screen.map_data.as_mut().expect("map_data present");
    let (w, h) = (map.width, map.height);
    for by in 0..h {
        for bx in 0..w {
            map.set_block(bx, by, block);
        }
    }
}

fn make_boulder(x: u16, y: u16) -> NpcRuntimeState {
    NpcRuntimeState {
        npc_index: 0,
        sprite_id: pokered_data::sprites::SpriteId::Boulder as u8,
        x,
        y,
        home_x: x,
        home_y: y,
        facing: Direction::Down,
        scripted_frame: None,
        movement_type: NpcMovementType::Stationary,
        wander_axis: dotzuki_engine::overworld::NpcWanderAxis::Any,
        range: 0,
        walk_counter: 0,
        delay_counter: 0,
        text_id: 0,
        defeated: false,
        visible: true,
        scripted_path: std::collections::VecDeque::new(),
    }
}

// ══════════════════════════════════════════════════════════════════════
//  CUT
// ══════════════════════════════════════════════════════════════════════

#[test]
fn cut_without_badge_shows_badge_message() {
    let mut screen = screen_on(MapId::PalletTown);
    let mon = test_mon();
    let outcome = screen.use_field_move(MoveId::Cut, &mon, NO_BADGES, MapId::PalletTown);
    assert_eq!(outcome, FieldMoveOutcome::Done);
    assert_eq!(
        dialogue_text(&screen).as_deref(),
        Some("No! A new BADGE\nis required.")
    );
}

#[test]
fn cut_tree_replaces_block_and_plays_sfx() {
    let (tree_block, sub_x, sub_y) =
        find_block_with_tile(CUT_TREE_TILE_OVERWORLD, |b| cut_tree_replacement(b).is_some())
            .expect("blockset has a swappable cut-tree block");
    let replacement = cut_tree_replacement(tree_block).unwrap();

    let mut screen = screen_on(MapId::PalletTown);
    place_block_in_front(&mut screen, tree_block, sub_x, sub_y, Direction::Up);
    let mon = test_mon();
    let outcome = screen.use_field_move(MoveId::Cut, &mon, CASCADE, MapId::PalletTown);

    assert_eq!(outcome, FieldMoveOutcome::Done);
    let map = screen.map_data.as_ref().unwrap();
    assert_eq!(
        super::collision::get_block_at(10, 10, map.width, &map.blocks),
        Some(replacement),
        "the tree block was swapped for its cut-down replacement"
    );
    assert!(
        screen
            .audio_requests
            .iter()
            .any(|r| matches!(r, super::screen::OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_CUT")),
        "SFX_CUT plays"
    );
    assert!(dialogue_text(&screen)
        .unwrap_or_default()
        .contains("hacked\naway with CUT!"));
}

#[test]
fn cut_with_nothing_in_front() {
    let mut screen = screen_on(MapId::PalletTown);
    // Face the player at an ordinary passable tile (no tree/grass).
    screen.state.player.x = 5;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Up;
    let mon = test_mon();
    let outcome = screen.use_field_move(MoveId::Cut, &mon, CASCADE, MapId::PalletTown);
    assert_eq!(outcome, FieldMoveOutcome::Done);
    assert_eq!(
        dialogue_text(&screen).as_deref(),
        Some("There isn't\nanything to CUT!")
    );
}

// ══════════════════════════════════════════════════════════════════════
//  SURF
// ══════════════════════════════════════════════════════════════════════

#[test]
fn surf_without_badge_shows_badge_message() {
    let mut screen = screen_on(MapId::PalletTown);
    let mon = test_mon();
    let outcome = screen.use_field_move(MoveId::Surf, &mon, NO_BADGES, MapId::PalletTown);
    assert_eq!(outcome, FieldMoveOutcome::Done);
    assert_eq!(
        dialogue_text(&screen).as_deref(),
        Some("No! A new BADGE\nis required.")
    );
}

#[test]
fn surf_not_facing_water() {
    let mut screen = screen_on(MapId::PalletTown);
    screen.state.player.x = 5;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Up;
    let mon = test_mon();
    let outcome = screen.use_field_move(MoveId::Surf, &mon, SOUL, MapId::PalletTown);
    assert_eq!(outcome, FieldMoveOutcome::Done);
    assert!(dialogue_text(&screen)
        .unwrap_or_default()
        .starts_with("No SURFing on"));
}

#[test]
fn surf_starts_on_water_and_sets_surfing_transport() {
    let (water_block, sub_x, sub_y) = find_block_with_tile(0x14, |_| true)
        .expect("blockset has a water block");

    let mut screen = screen_on(MapId::PalletTown);
    place_block_in_front(&mut screen, water_block, sub_x, sub_y, Direction::Up);
    let player_x = screen.state.player.x;
    let player_y = screen.state.player.y;
    let mon = test_mon();
    let outcome = screen.use_field_move(MoveId::Surf, &mon, SOUL, MapId::PalletTown);

    assert_eq!(outcome, FieldMoveOutcome::Done);
    assert_eq!(
        screen.state.player.transport,
        TransportMode::Surfing,
        "surfing assigns TransportMode::Surfing"
    );
    // .makePlayerMoveForward: the player walks one tile onto the water.
    assert_eq!(
        screen.scripted_player_path.front().copied(),
        Some((player_x, player_y - 1))
    );
    // PlayDefaultMusic: map music re-request (app maps Surfing -> MUSIC_SURFING).
    assert!(screen
        .audio_requests
        .iter()
        .any(|r| matches!(r, super::screen::OverworldAudioRequest::PlayMapMusic { .. })));
    assert!(dialogue_text(&screen)
        .unwrap_or_default()
        .contains("got on"));
}

#[test]
fn surf_stop_onto_land() {
    let (land_block, sub_x, sub_y) = find_block_matching(|t| {
        pokered_data::collision::is_tile_passable(TilesetId::Overworld, t)
    })
    .expect("blockset has a passable land block");

    let mut screen = screen_on(MapId::PalletTown);
    place_block_in_front(&mut screen, land_block, sub_x, sub_y, Direction::Up);
    screen.state.player.transport = TransportMode::Surfing;
    let mon = test_mon();
    let outcome = screen.use_field_move(MoveId::Surf, &mon, SOUL, MapId::PalletTown);

    assert_eq!(outcome, FieldMoveOutcome::Done);
    assert_eq!(
        screen.state.player.transport,
        TransportMode::Walking,
        "stepping off the water returns to walking"
    );
    assert!(screen.pending_dialogue.is_none(), "dismount shows no text");
}

#[test]
fn surf_stop_facing_water_is_refused() {
    let (water_block, sub_x, sub_y) = find_block_with_tile(0x14, |_| true).unwrap();

    let mut screen = screen_on(MapId::PalletTown);
    place_block_in_front(&mut screen, water_block, sub_x, sub_y, Direction::Up);
    screen.state.player.transport = TransportMode::Surfing;
    let mon = test_mon();
    screen.use_field_move(MoveId::Surf, &mon, SOUL, MapId::PalletTown);

    assert_eq!(screen.state.player.transport, TransportMode::Surfing);
    assert_eq!(
        dialogue_text(&screen).as_deref(),
        Some("There's no place\nto get off!")
    );
}

// ══════════════════════════════════════════════════════════════════════
//  STRENGTH + boulder push
// ══════════════════════════════════════════════════════════════════════

#[test]
fn strength_without_badge_shows_badge_message() {
    let mut screen = screen_on(MapId::PalletTown);
    let mon = test_mon();
    screen.use_field_move(MoveId::Strength, &mon, NO_BADGES, MapId::PalletTown);
    assert!(!screen.strength_active);
    assert_eq!(
        dialogue_text(&screen).as_deref(),
        Some("No! A new BADGE\nis required.")
    );
}

#[test]
fn strength_activates_and_plays_cry() {
    let mut screen = screen_on(MapId::PalletTown);
    let mon = test_mon();
    screen.use_field_move(MoveId::Strength, &mon, RAINBOW, MapId::PalletTown);
    assert!(screen.strength_active);
    assert!(screen
        .audio_requests
        .iter()
        .any(|r| matches!(r, super::screen::OverworldAudioRequest::PlayCry { .. })));
    // Two Gen-1 texts: "<MON> used STRENGTH." then "<MON> can move boulders."
    let dlg = screen.pending_dialogue.as_ref().expect("strength text shown");
    assert!(dlg.has_more_pages(), "both STRENGTH texts are queued");
    assert!(dialogue_text(&screen)
        .unwrap_or_default()
        .contains("used\nSTRENGTH."));
}

#[test]
fn strength_wears_off_on_map_change() {
    let mut screen = screen_on(MapId::PalletTown);
    screen.strength_active = true;
    screen.pending_warp = Some(PendingWarp {
        dest_map: MapId::Route1,
        dest_x: 5,
        dest_y: 5,
        save_last_map: false,
        arrival_spin: false,
    });
    screen.commit_pending_warp();
    assert!(
        !screen.strength_active,
        "EnterMap resets BIT_STRENGTH_ACTIVE"
    );
}

/// The position of the test boulder (PalletTown has its own NPCs, so locate
/// ours by the boulder sprite id rather than by index).
fn boulder_pos(screen: &OverworldScreen<PokemonRedData>) -> (u16, u16) {
    let b = screen
        .npc_states
        .iter()
        .find(|n| n.sprite_id == pokered_data::sprites::SpriteId::Boulder as u8)
        .expect("test boulder present");
    (b.x, b.y)
}

#[test]
fn boulder_push_requires_two_frames_and_moves_boulder() {
    let mut screen = screen_on(MapId::PalletTown);
    fill_map_with_passable_block(&mut screen);
    screen.state.player.x = 5;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Down;
    screen.npc_states.push(make_boulder(5, 6));
    screen.strength_active = true;

    // First contact: sets BIT_TRIED_PUSH_BOULDER, no movement yet.
    screen.tick_boulder_push(Some(Direction::Down));
    assert_eq!(boulder_pos(&screen), (5, 6), "first push only arms the flag");

    // Second frame (still holding): the boulder slides one tile.
    screen.tick_boulder_push(Some(Direction::Down));
    assert_eq!(boulder_pos(&screen), (5, 7), "second push moves the boulder");
    assert!(screen
        .audio_requests
        .iter()
        .any(|r| matches!(r, super::screen::OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_PUSH_BOULDER")));

    // The dust lockout (BIT_BOULDER_DUST) blocks immediate re-pushes.
    screen.tick_boulder_push(Some(Direction::Down));
    assert_eq!(boulder_pos(&screen), (5, 7), "dust lockout blocks re-push");
}

#[test]
fn boulder_push_requires_strength() {
    let mut screen = screen_on(MapId::PalletTown);
    fill_map_with_passable_block(&mut screen);
    screen.state.player.x = 5;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Down;
    screen.npc_states.push(make_boulder(5, 6));

    screen.tick_boulder_push(Some(Direction::Down));
    screen.tick_boulder_push(Some(Direction::Down));
    assert_eq!(boulder_pos(&screen), (5, 6), "no STRENGTH, no push");
}

#[test]
fn boulder_push_wrong_direction_keeps_boulder() {
    let mut screen = screen_on(MapId::PalletTown);
    fill_map_with_passable_block(&mut screen);
    screen.state.player.x = 5;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Down;
    screen.npc_states.push(make_boulder(5, 6));
    screen.strength_active = true;

    screen.tick_boulder_push(Some(Direction::Down)); // arms the flag
    screen.tick_boulder_push(Some(Direction::Left)); // held != facing
    assert_eq!(boulder_pos(&screen), (5, 6));
}

#[test]
fn boulder_push_blocked_by_wall() {
    let mut screen = screen_on(MapId::PalletTown);
    // Boulder against the north map edge: its destination is out of bounds,
    // which counts as blocked (no map-bounds walk-off).
    screen.state.player.x = 1;
    screen.state.player.y = 1;
    screen.state.player.facing = Direction::Up;
    screen.npc_states.push(make_boulder(1, 0));
    screen.strength_active = true;

    screen.tick_boulder_push(Some(Direction::Up)); // arms
    screen.tick_boulder_push(Some(Direction::Up)); // destination off-map → blocked
    assert_eq!(boulder_pos(&screen), (1, 0), "boulder can't leave the map");
}

#[test]
fn boulder_push_starts_the_dust_at_the_push_spot() {
    let mut screen = screen_on(MapId::PalletTown);
    fill_map_with_passable_block(&mut screen);
    screen.state.player.x = 5;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Down;
    screen.npc_states.push(make_boulder(5, 6));
    screen.strength_active = true;
    assert!(!screen.boulder_dust.is_active(), "no dust before the push");

    screen.tick_boulder_push(Some(Direction::Down)); // arms the flag
    screen.tick_boulder_push(Some(Direction::Down)); // push
    assert!(
        screen.boulder_dust.is_active(),
        "the push starts the dust puff (AnimateBoulderDust)"
    );
    assert_eq!(screen.boulder_dust.facing(), Direction::Down);
    // Anchored to the player's tile at push time (the original writes the
    // OAM block once from the player's sprite position).
    assert_eq!(screen.boulder_dust.anchor(), (5, 5));
}

#[test]
fn boulder_dust_runs_24_frames_independent_of_the_lockout() {
    let mut screen = screen_on(MapId::PalletTown);
    fill_map_with_passable_block(&mut screen);
    screen.state.player.x = 5;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Down;
    screen.npc_states.push(make_boulder(5, 6));
    screen.strength_active = true;

    screen.tick_boulder_push(Some(Direction::Down)); // arms
    screen.tick_boulder_push(Some(Direction::Down)); // push
    assert_eq!(screen.boulder_dust_frames, BOULDER_DUST_FRAMES);

    // The push lockout ends after 16 frames, but the dust keeps playing its
    // full 8-step × 3-frame timeline (24 frames).
    for _ in 0..16 {
        screen.tick_boulder_push(None);
    }
    assert_eq!(screen.boulder_dust_frames, 0, "lockout cleared");
    assert!(screen.boulder_dust.is_active(), "dust still playing");
    assert_eq!(screen.boulder_dust.step(), 5, "16 ticks = 5 steps + 1 frame");

    for _ in 0..8 {
        screen.tick_boulder_push(None);
    }
    assert!(!screen.boulder_dust.is_active(), "24 ticks = 8 steps, done");
}

#[test]
fn boulder_dust_completion_plays_sfx_cut_once() {
    use super::screen::OverworldAudioRequest;

    let mut screen = screen_on(MapId::PalletTown);
    fill_map_with_passable_block(&mut screen);
    screen.state.player.x = 5;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Down;
    screen.npc_states.push(make_boulder(5, 6));
    screen.strength_active = true;
    let cut_requests = |screen: &OverworldScreen<PokemonRedData>| {
        screen
            .audio_requests
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    OverworldAudioRequest::PlaySound { sound_id }
                        if sound_id == "SFX_CUT"
                )
            })
            .count()
    };

    screen.tick_boulder_push(Some(Direction::Down)); // arms
    screen.tick_boulder_push(Some(Direction::Down)); // push (SFX_PUSH_BOULDER)
    assert!(
        screen.boulder_dust.is_active(),
        "dust starts on the push frame"
    );
    assert_eq!(
        cut_requests(&screen),
        0,
        "no SFX_CUT while the dust is still playing"
    );

    // DoBoulderDustAnimation (push_boulder.asm:89-103) plays SFX_CUT exactly
    // when the 8-step × 3-frame dust animation completes (24th tick) — and
    // BIT_BOULDER_DUST is cleared in the same routine, so it fires once.
    for tick in 0..24 {
        screen.tick_boulder_push(None);
        let expected = if tick == 23 { 1 } else { 0 };
        assert_eq!(
            cut_requests(&screen),
            expected,
            "SFX_CUT fires only on the dust-completion tick (tick {tick})"
        );
    }
    assert!(!screen.boulder_dust.is_active(), "dust done after 24 ticks");
}

#[test]
fn boulder_dust_restarts_on_a_new_push() {
    let mut screen = screen_on(MapId::PalletTown);
    fill_map_with_passable_block(&mut screen);
    screen.state.player.x = 5;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Down;
    screen.npc_states.push(make_boulder(5, 6));
    screen.strength_active = true;

    screen.tick_boulder_push(Some(Direction::Down)); // arms
    screen.tick_boulder_push(Some(Direction::Down)); // push #1
    for _ in 0..16 {
        screen.tick_boulder_push(None); // wait out the lockout
    }
    assert!(screen.boulder_dust.is_active(), "dust from push #1 still up");

    // The player steps forward to stand in front of the moved boulder.
    screen.state.player.y = 6;
    // A second push (allowed once the lockout clears, still inside the
    // first puff's window) restarts the dust at the new spot.
    screen.tick_boulder_push(Some(Direction::Down)); // arms
    screen.tick_boulder_push(Some(Direction::Down)); // push #2
    assert!(screen.boulder_dust.is_active());
    assert_eq!(screen.boulder_dust.step(), 0, "dust restarted from step 0");
    assert_eq!(screen.boulder_dust.anchor(), (5, 6), "anchored at the new player tile");
}

// ══════════════════════════════════════════════════════════════════════
//  FLASH + dark cave state
// ══════════════════════════════════════════════════════════════════════

#[test]
fn rock_tunnel_loads_as_dark_cave() {
    let screen = screen_on(MapId::RockTunnel1F);
    assert!(screen.dark_cave.is_dark(), "Rock Tunnel starts dark");
    let screen = screen_on(MapId::PalletTown);
    assert!(!screen.dark_cave.is_dark());
}

#[test]
fn flash_lights_dark_cave() {
    let mut screen = screen_on(MapId::RockTunnel1F);
    let mon = test_mon();
    screen.use_field_move(MoveId::Flash, &mon, BOULDER, MapId::PalletTown);
    assert!(!screen.dark_cave.is_dark(), "FLASH clears the dark state");
    assert_eq!(
        dialogue_text(&screen).as_deref(),
        Some("A blinding FLASH\nlights the area!")
    );
}

#[test]
fn flash_without_badge_shows_badge_message() {
    let mut screen = screen_on(MapId::RockTunnel1F);
    let mon = test_mon();
    screen.use_field_move(MoveId::Flash, &mon, NO_BADGES, MapId::PalletTown);
    assert!(screen.dark_cave.is_dark(), "still dark without the badge");
    assert_eq!(
        dialogue_text(&screen).as_deref(),
        Some("No! A new BADGE\nis required.")
    );
}

// ══════════════════════════════════════════════════════════════════════
//  FLY
// ══════════════════════════════════════════════════════════════════════

#[test]
fn fly_without_badge_shows_badge_message() {
    let mut screen = screen_on(MapId::Route1);
    let mon = test_mon();
    let outcome = screen.use_field_move(MoveId::Fly, &mon, NO_BADGES, MapId::PalletTown);
    assert_eq!(outcome, FieldMoveOutcome::Done);
    assert_eq!(
        dialogue_text(&screen).as_deref(),
        Some("No! A new BADGE\nis required.")
    );
}

#[test]
fn fly_indoors_is_refused() {
    let mut screen = screen_on(MapId::RedsHouse1F);
    let mon = test_mon();
    let outcome = screen.use_field_move(MoveId::Fly, &mon, THUNDER, MapId::PalletTown);
    assert_eq!(outcome, FieldMoveOutcome::Done);
    assert!(dialogue_text(&screen)
        .unwrap_or_default()
        .contains("can't\nFLY here."));
}

#[test]
fn fly_outside_opens_fly_map() {
    let mut screen = screen_on(MapId::Route1);
    let mon = test_mon();
    let outcome = screen.use_field_move(MoveId::Fly, &mon, THUNDER, MapId::PalletTown);
    assert_eq!(outcome, FieldMoveOutcome::OpenFlyMap);
}

#[test]
fn fly_warp_to_queues_fade_warp() {
    let mut screen = screen_on(MapId::Route1);
    let dest = hm_effects::fly_destination_for_map(MapId::CeruleanCity).unwrap();
    screen.fly_warp_to(dest.map, dest.x, dest.y);
    let warp = screen.pending_warp.as_ref().expect("fly warp queued");
    assert_eq!(warp.dest_map, MapId::CeruleanCity);
    assert_eq!((warp.dest_x, warp.dest_y), (19, 18));
    assert!(matches!(
        screen.warp_fade_state,
        WarpFadeState::FadingOut { .. }
    ));
}

// ══════════════════════════════════════════════════════════════════════
//  DIG
// ══════════════════════════════════════════════════════════════════════

#[test]
fn dig_warps_to_last_pokemon_center() {
    // Gen-1 `.dig` (start_sub_menus.asm:195-199) loads ESCAPE_ROPE as a
    // pseudo-item: ItemUseEscapeRope sets BIT_ESCAPE_WARP and
    // LoadSpecialWarpData warps to wLastBlackoutMap's fly point
    // (special_warps.asm:76-80) — the last Pokémon Center, NOT the dungeon
    // entrance.
    let mut screen = screen_on(MapId::MtMoon1F);
    let mon = test_mon();
    let outcome =
        screen.use_field_move(MoveId::Dig, &mon, NO_BADGES, MapId::CeruleanCity);

    assert_eq!(outcome, FieldMoveOutcome::Done);
    let warp = screen.pending_warp.as_ref().expect("dig queues the escape warp");
    assert_eq!(warp.dest_map, MapId::CeruleanCity);
    assert_eq!((warp.dest_x, warp.dest_y), (19, 18));
    // DIG is a move, not an item: nothing is consumed (the `consumed` flag
    // of the pseudo-item flow is discarded by field_dig).
    assert!(screen.pending_dialogue.is_none(), "warp replaces dialogue");
}

#[test]
fn dig_warp_uses_last_healed_map_not_entrance() {
    // The warp target follows the LAST CENTER the player healed at, even
    // when the recorded dungeon entrance is a different map — the pre-fix
    // behavior (warp to last_map/last_map_entry) must be gone.
    let mut screen = screen_on(MapId::MtMoon1F);
    screen.last_map = Some(MapId::Route4);
    screen.last_map_entry = Some((10, 12));
    let mon = test_mon();
    screen.use_field_move(MoveId::Dig, &mon, NO_BADGES, MapId::FuchsiaCity);
    let warp = screen.pending_warp.as_ref().expect("dig queues the escape warp");
    assert_eq!(warp.dest_map, MapId::FuchsiaCity);
    assert_eq!((warp.dest_x, warp.dest_y), (19, 28));
}

#[test]
fn dig_refused_outside() {
    let mut screen = screen_on(MapId::Route1);
    screen.last_map = Some(MapId::Route2);
    screen.last_map_entry = Some((5, 5));
    let mon = test_mon();
    screen.use_field_move(MoveId::Dig, &mon, NO_BADGES, MapId::PalletTown);
    assert!(screen.pending_warp.is_none());
    assert!(dialogue_text(&screen).is_some(), "refusal message shown");
}

// ══════════════════════════════════════════════════════════════════════
//  TELEPORT
// ══════════════════════════════════════════════════════════════════════

#[test]
fn teleport_indoors_is_refused() {
    let mut screen = screen_on(MapId::MtMoon1F);
    let mon = test_mon();
    screen.use_field_move(MoveId::Teleport, &mon, NO_BADGES, MapId::PalletTown);
    // "<MON> can't / use TELEPORT / now." paginates over two boxes.
    assert!(dialogue_text(&screen)
        .unwrap_or_default()
        .contains("can't\nuse TELEPORT"));
}

#[test]
fn teleport_outside_warps_to_last_center_after_text() {
    let mut screen = screen_on(MapId::Route1);
    let mon = test_mon();
    let outcome =
        screen.use_field_move(MoveId::Teleport, &mon, NO_BADGES, MapId::CeruleanCity);
    assert_eq!(outcome, FieldMoveOutcome::Done);
    assert_eq!(
        dialogue_text(&screen).as_deref(),
        Some("Warp to the last\n#MON CENTER.")
    );
    // The warp fires only once the text is dismissed (post-dialogue warp).
    assert!(screen.pending_warp.is_none());
    assert!(matches!(screen.warp_fade_state, WarpFadeState::Idle));
}

#[test]
fn teleport_deferred_warp_fires_when_dialogue_closes() {
    let mut screen = screen_on(MapId::Route1);
    let mon = test_mon();
    screen.use_field_move(MoveId::Teleport, &mon, NO_BADGES, MapId::CeruleanCity);

    // Dismiss the queued message, then run one frame: the deferred warp is
    // queued and the leave-map spin (_LeaveMapAnim) starts; the fade follows
    // once the spin finishes.
    screen.pending_dialogue = None;
    let input = super::OverworldInput::new(
        false, false, false, false, false, false, false, false,
    );
    screen.update_frame(input);
    let warp = screen.pending_warp.as_ref().expect("warp fired after text");
    assert_eq!(warp.dest_map, MapId::CeruleanCity);
    assert_eq!((warp.dest_x, warp.dest_y), (19, 18));
    assert!(screen.teleport_spin.is_some(), "spin-out plays first");
    assert!(screen.warp_fade_to_white, "escape warps fade to white");
    while screen.teleport_spin.is_some() {
        screen.update_frame(input);
    }
    assert!(matches!(
        screen.warp_fade_state,
        WarpFadeState::FadingOut { .. }
    ));
}

// ══════════════════════════════════════════════════════════════════════
//  Town-visited tracking (FLY destination gating)
// ══════════════════════════════════════════════════════════════════════

#[test]
fn starting_in_a_city_marks_it_visited() {
    let screen = screen_on(MapId::PalletTown);
    assert!(screen.game_data_requests.iter().any(|r| matches!(
        r,
        super::screen::OverworldGameDataRequest::MarkTownVisited { map } if *map == MapId::PalletTown
    )));
}

#[test]
fn town_visited_flags_drive_fly_destinations() {
    let mut data = crate::save::game_data::GameData::new();
    assert!(data.fly_destinations().is_empty(), "nothing visited yet");
    data.mark_town_visited(MapId::PalletTown);
    data.mark_town_visited(MapId::CeruleanCity);
    assert!(data.is_town_visited(MapId::PalletTown));
    assert!(!data.is_town_visited(MapId::ViridianCity));
    assert_eq!(
        data.fly_destinations(),
        vec![MapId::PalletTown, MapId::CeruleanCity],
        "visited cities in map-ID order (BuildFlyLocationsList)"
    );
    // Routes are not city maps and are ignored.
    data.mark_town_visited(MapId::Route1);
    assert!(!data.is_town_visited(MapId::Route1));
}

// ══════════════════════════════════════════════════════════════════════
//  SOFTBOILED — the 9th Gen-1 field move (start_sub_menus.asm .softboiled)
// ══════════════════════════════════════════════════════════════════════

/// A mon that knows SOFTBOILED, with a controllable HP.
fn softboiled_mon() -> crate::battle::state::Pokemon {
    let mut mon = test_mon();
    mon.moves = [MoveId::Softboiled, MoveId::None, MoveId::None, MoveId::None];
    mon
}

#[test]
fn softboiled_healthy_user_opens_target_pick() {
    let mut screen = screen_on(MapId::PalletTown);
    let mut mon = softboiled_mon();
    // max_hp / 5, with current HP above it.
    mon.hp = mon.max_hp;
    let outcome = screen.use_field_move(MoveId::Softboiled, &mon, 0, MapId::PalletTown);
    assert_eq!(
        outcome,
        FieldMoveOutcome::ChooseSoftboiledTarget,
        "healthy user: the party menu reopens to pick a target"
    );
    assert!(screen.pending_dialogue.is_none(), "no text yet");
}

#[test]
fn softboiled_user_not_healthy_enough_refused() {
    let mut screen = screen_on(MapId::PalletTown);
    let mut mon = softboiled_mon();
    mon.hp = mon.max_hp / 5; // current HP <= max/5 → "Not healthy enough."
    let outcome = screen.use_field_move(MoveId::Softboiled, &mon, 0, MapId::PalletTown);
    assert_eq!(outcome, FieldMoveOutcome::Done);
    // _NotHealthyEnoughText (data/text/text_5.asm:55-58).
    let dlg = screen.pending_dialogue.as_ref().expect("refusal text");
    let page = dlg.current().unwrap();
    assert_eq!(format!("{}\n{}", page.line1, page.line2), "Not healthy\nenough.");
}

#[test]
fn softboiled_fainted_user_refused() {
    let mut screen = screen_on(MapId::PalletTown);
    let mut mon = softboiled_mon();
    mon.hp = 0;
    let outcome = screen.use_field_move(MoveId::Softboiled, &mon, 0, MapId::PalletTown);
    assert_eq!(outcome, FieldMoveOutcome::Done);
    assert!(
        screen
            .pending_dialogue
            .as_ref()
            .and_then(|d| d.current())
            .map(|p| p.line1 == "Not healthy")
            .unwrap_or(false),
        "a fainted user has 0 HP, which is never > max/5"
    );
}

#[test]
fn softboiled_no_pp_cost_and_no_badge_gate() {
    // .softboiled has no badge check (field_move_required_badge = None) and
    // the heal never touches PP — both locked by the table + the heal fn.
    assert_eq!(hm_effects::field_move_required_badge(MoveId::Softboiled), None);
    let mut screen = screen_on(MapId::PalletTown);
    let mut mon = softboiled_mon();
    mon.hp = mon.max_hp;
    // NO_BADGES passes: the heal path is ungated.
    let outcome = screen.use_field_move(MoveId::Softboiled, &mon, 0, MapId::PalletTown);
    assert_eq!(outcome, FieldMoveOutcome::ChooseSoftboiledTarget);
}

// ══════════════════════════════════════════════════════════════════════
//  SOFTBOILED heal math (engine/items/item_effects.asm pseudo-item path)
// ══════════════════════════════════════════════════════════════════════

#[test]
fn softboiled_heals_target_by_users_1_5th_max_hp() {
    use crate::items::bag_use::{apply_softboiled, ItemApplyOutcome};
    let mut user = softboiled_mon();
    let mut target = test_mon();
    let cost = user.max_hp / 5;
    user.hp = user.max_hp;
    target.hp = target.max_hp - cost; // room for the full heal

    let outcome = apply_softboiled(&mut user, &mut target);
    match outcome {
        ItemApplyOutcome::Used { message, consume } => {
            assert!(!consume, "SOFTBOILED consumes nothing (no item, no PP)");
            assert!(
                message.contains("recovered by"),
                "POTION_MSG convention: '{{name}} recovered by {{N}}!' — got {message:?}"
            );
        }
        other => panic!("expected Used, got {other:?}"),
    }
    assert_eq!(user.hp, user.max_hp - cost, "the user loses 1/5 max HP");
    assert_eq!(target.hp, target.max_hp, "the target gains exactly the cost");
}

#[test]
fn softboiled_target_heal_capped_at_max_hp() {
    use crate::items::bag_use::{apply_softboiled, ItemApplyOutcome};
    let mut user = softboiled_mon();
    let mut target = test_mon();
    let cost = user.max_hp / 5;
    user.hp = user.max_hp;
    target.hp = target.max_hp - 1; // only 1 HP missing

    match apply_softboiled(&mut user, &mut target) {
        ItemApplyOutcome::Used { .. } => {}
        other => panic!("expected Used, got {other:?}"),
    }
    assert_eq!(target.hp, target.max_hp, "heal capped at max HP");
    assert_eq!(user.hp, user.max_hp - cost);
}

#[test]
fn softboiled_refused_for_full_hp_or_fainted_target() {
    use crate::items::bag_use::{apply_softboiled, ItemApplyOutcome};
    // Full-HP target: no effect, user keeps its HP (the original's
    // .healingItemNoEffect path).
    let mut user = softboiled_mon();
    let mut target = test_mon();
    user.hp = user.max_hp;
    let user_hp = user.hp;
    let outcome = apply_softboiled(&mut user, &mut target);
    assert!(matches!(outcome, ItemApplyOutcome::NoEffect { .. }));
    assert_eq!(user.hp, user_hp, "no HP is spent on a refused target");
    assert_eq!(target.hp, target.max_hp);

    // Fainted target: same refusal.
    let mut user = softboiled_mon();
    let mut target = test_mon();
    target.hp = 0;
    user.hp = user.max_hp;
    let outcome = apply_softboiled(&mut user, &mut target);
    assert!(matches!(outcome, ItemApplyOutcome::NoEffect { .. }));
    assert_eq!(user.hp, user.max_hp);
}

#[test]
fn softboiled_truncates_the_fifth_like_gen1_divide() {
    use crate::items::bag_use::{apply_softboiled, ItemApplyOutcome};
    // The asm uses `b=2; call Divide` — truncating integer division.
    let mut user = softboiled_mon();
    let mut target = test_mon();
    user.max_hp = 99; // 99/5 = 19 remainder 4
    user.hp = 99;
    target.hp = 1;
    let cost = user.max_hp / 5;
    assert_eq!(cost, 19);
    match apply_softboiled(&mut user, &mut target) {
        ItemApplyOutcome::Used { message, .. } => {
            assert!(message.contains("19"), "heal amount 19 in the message: {message:?}");
        }
        other => panic!("expected Used, got {other:?}"),
    }
    assert_eq!(user.hp, 99 - 19);
    assert_eq!(target.hp, 1 + 19);
}

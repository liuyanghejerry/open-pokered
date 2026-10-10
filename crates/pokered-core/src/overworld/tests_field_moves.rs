//! Integration tests for `OverworldScreen::use_field_move` and the boulder
//! push — the live wiring of the HM field effects (CUT / SURF / STRENGTH /
//! FLY / FLASH / DIG / TELEPORT) from the party menu.

use crate::alloc_prelude::*;
use super::field_moves::FieldMoveOutcome;
use super::hm_effects;
use super::presentation;
use super::screen::{OverworldScreen, PendingWarp, WarpFadeState};
use super::{Direction, OverworldInput, TransportMode};
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

fn no_input() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
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
fn cut_tree_waits_for_text_then_animates_and_plays_sfx() {
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
        Some(tree_block),
        "the tree remains while UsedCutText is open"
    );
    assert!(screen.pending_cut.is_some());
    assert!(screen.cut_anim.is_none());
    assert!(!screen
        .audio_requests
        .iter()
        .any(|r| matches!(r, super::screen::OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_CUT")));
    assert!(dialogue_text(&screen)
        .unwrap_or_default()
        .contains("hacked\naway with CUT!"));

    screen
        .pending_dialogue
        .as_mut()
        .expect("UsedCutText")
        .skip_to_full_page();
    let press_a = OverworldInput::new(false, false, false, false, true, false, false, false);
    screen.update_frame(press_a);
    screen.update_frame(no_input());
    assert!(screen.pending_dialogue.is_none());
    assert!(screen.cut_retained_dialogue.is_some());

    screen.update_frame(no_input());
    let map = screen.map_data.as_ref().unwrap();
    assert_eq!(
        super::collision::get_block_at(10, 10, map.width, &map.blocks),
        Some(replacement),
        "the tree block swaps when AnimCut starts"
    );
    assert!(screen.cut_anim.is_some());
    for _ in 0..8 {
        screen.update_frame(no_input());
    }
    assert!(
        screen.cut_retained_dialogue.is_some(),
        "the original text-box BG remains during AnimCut setup"
    );
    screen.update_frame(no_input());
    assert!(
        screen.cut_retained_dialogue.is_none(),
        "RedrawMapView clears the text box as separation starts"
    );
    for _ in 9..presentation::CUT_ANIM_FRAMES {
        screen.update_frame(no_input());
    }
    assert!(screen.cut_anim.is_none());
    assert!(
        screen
            .audio_requests
            .iter()
            .any(|r| matches!(r, super::screen::OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_CUT")),
        "SFX_CUT plays"
    );
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
fn surf_waits_for_text_then_uses_original_step_cadence() {
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
    assert!(screen.scripted_player_path.is_empty());
    assert!(screen.pending_field_move_step.is_some());
    assert_eq!((screen.state.player.x, screen.state.player.y), (player_x, player_y));
    // PlayDefaultMusic: map music re-request (app maps Surfing -> MUSIC_SURFING).
    assert!(screen
        .audio_requests
        .iter()
        .any(|r| matches!(r, super::screen::OverworldAudioRequest::PlayMapMusic { .. })));
    assert!(dialogue_text(&screen)
        .unwrap_or_default()
        .contains("got on"));

    // PrintText blocks the simulated forward press. Once it closes, the
    // original's graphics restoration exposes 37 white and 23 map-only frames
    // before movement spans 18 visible frames.
    for _ in 0..1000 {
        if screen
            .pending_dialogue
            .as_ref()
            .is_some_and(|dialogue| dialogue.waiting_for_input())
        {
            break;
        }
        screen.update_frame(no_input());
    }
    let press_a = OverworldInput::new(false, false, false, false, true, false, false, false);
    screen.update_frame(press_a);
    assert!(screen
        .pending_dialogue
        .as_ref()
        .is_some_and(|dialogue| dialogue.holding_open()));
    screen.update_frame(no_input());
    assert!(screen.pending_dialogue.is_none());
    assert_eq!(
        screen.field_move_restore,
        Some(presentation::FieldMoveRestoreState::new())
    );
    for _ in 0..presentation::FIELD_MOVE_RESTORE_FRAMES {
        screen.update_frame(no_input());
    }
    assert!(screen.field_move_step.is_some());
    assert_eq!((screen.state.player.x, screen.state.player.y), (player_x, player_y));
    for _ in 0..18 {
        screen.update_frame(no_input());
    }
    assert!(screen.field_move_step.is_none());
    assert_eq!((screen.state.player.x, screen.state.player.y), (player_x, player_y - 1));
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
    assert_eq!(boulder_pos(&screen), (5, 6), "second contact starts the scripted slide");
    for _ in 0..38 {screen.tick_boulder_push(None);}
    assert_eq!(boulder_pos(&screen), (5, 7), "slide reaches its resting tile");
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
    assert!(!screen.boulder_dust.is_active(), "MoveSprite finishes before smoke is loaded");
    for _ in 0..42 {screen.tick_boulder_push(None);}
    assert!(screen.boulder_dust.is_active(), "smoke follows the scripted slide and graphics copy");
    assert_eq!(screen.boulder_dust.facing(), Direction::Down);
    // Anchored to the player's tile at push time (the original writes the
    // OAM block once from the player's sprite position).
    assert_eq!(screen.boulder_dust.anchor(), (5, 5));
}

#[test]
fn boulder_push_blocks_player_and_inputs_until_graphics_restore() {
    use crate::game_state::ScreenAction;
    let mut screen=screen_on(MapId::PalletTown);fill_map_with_passable_block(&mut screen);
    screen.state.player.x=5;screen.state.player.y=5;screen.state.player.facing=Direction::Down;
    screen.npc_states.push(make_boulder(5,6));screen.strength_active=true;
    screen.tick_boulder_push(Some(Direction::Down));screen.tick_boulder_push(Some(Direction::Down));
    for frame in 1..=69 {
        // Physical controls during the blocking routine must neither move
        // the player nor open a menu/dialogue or start another push.
        let noisy=super::OverworldInput::new(true,true,true,true,true,true,true,true);
        assert_eq!(screen.update_frame(noisy),ScreenAction::Continue);
        assert_eq!((screen.state.player.x,screen.state.player.y),(5,5),"player waits at {frame}");
        assert!(screen.pending_dialogue.is_none());
        assert_eq!(screen.boulder_dust.is_active(),(42..=66).contains(&frame),"logical smoke stage {frame}");
        // Source TryWalking updates MapY/MapX before any visible slide.
        assert_eq!(boulder_pos(&screen),if frame<2 {(5,6)} else {(5,7)});
        assert_eq!(screen.boulder_push.is_some(),frame<72);
    }
    let idle=super::OverworldInput::new(false,false,false,false,false,false,false,false);
    screen.update_frame(idle);screen.update_frame(idle);screen.update_frame(idle);
    assert_eq!(screen.boulder_dust_frames,0);
    // A released/new direction after the routine can move normally.
    let idle=super::OverworldInput::new(false,false,false,false,false,false,false,false);
    screen.update_frame(idle);
    for _ in 0..4 { screen.update_frame(super::OverworldInput::new(false,true,false,false,false,false,false,false)); }
    assert_eq!(screen.state.player.movement_state,super::MovementState::Walking);
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
    assert!(!screen.boulder_dust.is_active(),"no smoke before the slide");
    for frame in 1..=72 {
        screen.tick_boulder_push(None);
        assert_eq!(cut_requests(&screen),usize::from(frame>=70),"SFX_CUT after graphics restoration at {frame}");
    }
    assert!(!screen.boulder_dust.is_active());
    for _ in 0..10 {screen.tick_boulder_push(None);}
    assert_eq!(cut_requests(&screen),1,"the completion cue is emitted once");
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
    for _ in 0..75 {screen.tick_boulder_push(None);}
    assert!(screen.boulder_push.is_none());
    screen.state.player.y=6;
    screen.tick_boulder_push(Some(Direction::Down));screen.tick_boulder_push(Some(Direction::Down));
    assert!(!screen.boulder_dust.is_active(),"a new push starts with a slide");
    for _ in 0..42 {screen.tick_boulder_push(None);}
    assert_eq!(screen.boulder_dust.step(),0);
    assert_eq!(screen.boulder_dust.anchor(),(5,6));
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
fn fly_warp_to_runs_departure_before_fade_and_commits_on_original_frame() {
    let mut screen = screen_on(MapId::Route1);
    let dest = hm_effects::fly_destination_for_map(MapId::CeruleanCity).unwrap();
    screen.fly_warp_to(dest.map, dest.x, dest.y);
    let warp = screen.pending_warp.as_ref().expect("fly warp queued");
    assert_eq!(warp.dest_map, MapId::CeruleanCity);
    assert_eq!((warp.dest_x, warp.dest_y), (19, 18));
    assert!(matches!(screen.warp_fade_state, WarpFadeState::Idle));
    assert!(screen.fly_departure.is_some());

    for _ in 0..227 {
        screen.update_frame(no_input());
    }
    assert_eq!(screen.state.current_map, MapId::Route1);
    screen.update_frame(no_input());
    assert_eq!(screen.state.current_map, MapId::CeruleanCity);
    assert_eq!(screen.fly_arrival_delay_frames, presentation::FLY_ARRIVAL_POST_FADE_DELAY_FRAMES);

    for _ in 228..297 {
        screen.update_frame(no_input());
    }
    assert_eq!(screen.enter_map_fly_anim.as_ref().map(|fly| fly.frame), Some(0));
    for _ in 0..presentation::FLY_ANIM_FRAMES {
        screen.update_frame(no_input());
    }
    assert!(screen.enter_map_fly_anim.is_none());
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
            .map(|p| p.line1.as_ref() == "Not healthy")
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

#[test]
fn direction_history_survives_idle_facing_changes_and_system_save_restore() {
    let mut screen=screen_on(MapId::PalletTown);fill_map_with_passable_block(&mut screen);
    screen.state.player.x=5;screen.state.player.y=5;
    let left=super::OverworldInput::new(false,false,true,false,false,false,false,false);
    let idle=super::OverworldInput::new(false,false,false,false,false,false,false,false);
    screen.update_frame(left);
    assert_eq!(screen.player_moving_direction,2);
    assert_eq!(screen.player_last_stop_direction,0);
    for _ in 0..32 {screen.update_frame(idle);}
    assert_eq!(screen.player_moving_direction,0);
    assert_eq!(screen.player_last_stop_direction,2);
    // Continue resets only the visible facing; idle must retain the stop.
    screen.state.player.facing=Direction::Down;
    for _ in 0..120 {screen.update_frame(idle);}
    assert_eq!(screen.player_last_stop_direction,2);
    let mut data=crate::save::SaveData::new().game_data;
    screen.write_system_save_state(&mut data);
    assert_eq!(data.player_last_stop_direction,2);
    assert_eq!(data.player_moving_direction,0);
    let mut restored=screen_on(MapId::PalletTown);
    restored.restore_system_save_state(&data);
    restored.update_frame(idle);
    assert_eq!(restored.player_last_stop_direction,2);
    assert_eq!(restored.player_moving_direction,0);
}

#[test]
fn player_start_pulse_is_discarded_midstep_but_held_start_opens_after_landing() {
    use crate::game_state::{ScreenAction,GameScreen};
    use super::MovementState;
    for transport in [TransportMode::Walking,TransportMode::Biking] {
        for held in [false,true] {
            let mut screen=screen_on(MapId::PalletTown);fill_map_with_passable_block(&mut screen);
            screen.state.player.x=5;screen.state.player.y=5;screen.state.player.transport=transport;
            let down=OverworldInput::new(false,true,false,false,false,false,false,false);
            let start=OverworldInput::new(false,true,false,false,false,false,true,false);
            assert_eq!(screen.update_frame(down),ScreenAction::Continue);
            assert_eq!(screen.state.player.movement_state,MovementState::Walking);
            assert_eq!(screen.update_frame(start),ScreenAction::Continue,"START must wait for a tile");
            let pending=if held {start} else {down};
            for _ in 0..32 {
                if screen.state.player.movement_state==MovementState::Idle {break;}
                assert_eq!(screen.update_frame(pending),ScreenAction::Continue);
            }
            assert_eq!(screen.state.player.movement_state,MovementState::Idle);
            assert_eq!((screen.state.player.x,screen.state.player.y),(5,6));
            screen.update_frame(pending); // first DelayFrame after landing
            assert_eq!(screen.update_frame(pending),if held {
                ScreenAction::Transition(GameScreen::StartMenu)
            } else {ScreenAction::Continue});
        }
    }
}

#[test]
fn start_precedes_a_at_the_pokemon_center_pc() {
    use crate::game_state::{ScreenAction,GameScreen};
    let mut screen=screen_on(MapId::ViridianPokecenter);
    screen.state.player.x=13;screen.state.player.y=4;screen.state.player.facing=Direction::Up;
    let both=OverworldInput::new(false,false,false,false,true,false,true,false);
    assert_eq!(screen.update_frame(both),ScreenAction::Transition(GameScreen::StartMenu));
    assert!(screen.pending_pc.is_none());assert!(screen.pending_dialogue.is_none());
}

#[test]
fn held_a_is_sampled_only_after_step() {
    use crate::game_state::ScreenAction;
    use super::MovementState;
    let mut screen=screen_on(MapId::ViridianPokecenter);
    screen.state.player.x=13;screen.state.player.y=5;screen.state.player.facing=Direction::Up;
    screen.state.player.movement_state=MovementState::Walking;screen.state.walk_counter=2;
    screen.player_moving_direction=8;
    let held=OverworldInput::new(false,false,false,false,true,false,false,false);
    assert_eq!(screen.update_frame(held),ScreenAction::Continue);
    assert!(screen.pending_pc.is_none());assert!(screen.pending_dialogue.is_none());
    screen.update_frame(held);screen.update_frame(held);
    assert_eq!((screen.state.player.x,screen.state.player.y),(13,4));
    assert!(screen.pending_pc.is_none());assert!(screen.pending_dialogue.is_none());
    screen.update_frame(held);screen.update_frame(held);
    assert!(screen.pending_pc.is_some() || screen.pending_dialogue.is_some() || screen.active_script_effect.is_some(),"held A interacts after landing");
}

#[test]
fn seafoam_hole_waits_for_dust_and_keeps_the_lower_floor_event() {
    use pokered_data::event_flags::EventFlag;
    let mut screen=screen_on(MapId::SeafoamIslands1F);
    screen.npc_states.clear();
    screen.npc_states.push(make_boulder(17,5));
    screen.npc_states[0].walk_counter=16;
    screen.boulder_push=Some(presentation::BoulderPushState {
        npc_index:0,direction:Direction::Down,anchor:(17,4),
        origin:(17,5),destination:(17,6),frame:0,switch_block:None,redraw_remaining:0,walk_wait:0,
    });
    let flag=EventFlag::EVENT_SEAFOAM1_BOULDER1_DOWN_HOLE;
    for frame in 1..=72 {
        screen.advance_boulder_push();
        assert_eq!(screen.npc_states[0].visible,frame<70);
        assert_eq!(screen.unified_flags.check(flag),frame>=70);
    }
    assert_eq!((screen.npc_states[0].x,screen.npc_states[0].y),(17,6));
    assert!(screen.boulder_push.is_none());
    let saved=screen.unified_flags.to_event_bytes();
    let mut lower=screen_on(MapId::SeafoamIslandsB1F);
    lower.set_event_flags_bytes(&saved);
    lower.run_on_load();
    for _ in 0..120 {lower.update_frame(OverworldInput::new(false,false,false,false,false,false,false,false));}
    assert!(lower.npc_states.iter().any(|n|n.visible && n.sprite_id==pokered_data::sprites::SpriteId::Boulder as u8));
}

#[test]
fn victory_road_switch_commits_during_slide_and_redraw_depends_on_view_address() {
    use pokered_data::event_flags::EventFlag;
    for (map, x, y, flag, bx, by, block, pause) in [
        (MapId::VictoryRoad1F,17,11,EventFlag::EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH,4,6,29,9),
        (MapId::VictoryRoad2F,1,14,EventFlag::EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1,3,4,21,0),
        (MapId::VictoryRoad2F,9,14,EventFlag::EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH2,11,7,29,9),
    ] {
        let mut screen=screen_on(map);
        screen.state.player.x=x;screen.state.player.y=y;
        screen.npc_states.clear();screen.npc_states.push(make_boulder(x,y+1));
        screen.npc_states[0].walk_counter=16;
        let old=screen.map_data.as_ref().unwrap().blocks[by as usize*screen.map_data.as_ref().unwrap().width as usize+bx as usize];
        assert_ne!(old,block);
        screen.boulder_push=Some(presentation::BoulderPushState {
            npc_index:0,direction:Direction::Down,anchor:(x,y),origin:(x,y+1),
            destination:(x,y+2),frame:0,switch_block:None,redraw_remaining:0,walk_wait:0,
        });
        for elapsed in 1..=72+pause {
            screen.advance_boulder_push();
            assert_eq!(screen.unified_flags.check(flag),elapsed>=4,"{map:?} elapsed {elapsed}");
            assert_eq!(screen.npc_states[0].y,if elapsed<2 {y+1} else {y+2});
            let m=screen.map_data.as_ref().unwrap();
            assert_eq!(m.blocks[by as usize*m.width as usize+bx as usize],if elapsed<6 {old} else {block});
            if (6..=6+pause).contains(&elapsed) {assert_eq!(screen.boulder_push.unwrap().frame,6);}
            assert_eq!(screen.boulder_push.is_none(),elapsed==72+pause);
        }
        assert_eq!(screen.npc_states[0].walk_counter,0);
    }
}

#[test]
fn ordinary_steps_match_original_counter_trace_and_sample_after_landing() {
    use crate::game_state::{ScreenAction, GameScreen};
    use super::MovementState;
    // Original AdvancePlayerSprite with an already matching stopped direction:
    // the first redraw holds counter 7 across one extra hardware frame.
    for (transport, trace, landing) in [
        (TransportMode::Walking, vec![7,7,7,6,6,5,5,4,4,3,3,2,2,1,1,0],15),
        (TransportMode::Biking, vec![7,6,6,4,4,2,2,0],7),
    ] {
        let mut screen=screen_on(MapId::PalletTown);fill_map_with_passable_block(&mut screen);
        screen.state.player.x=5;screen.state.player.y=5;screen.state.player.facing=Direction::Down;
        screen.state.player.transport=transport;
        screen.player_last_stop_direction=4;screen.check_player_turn=true;
        let down=OverworldInput::new(false,true,false,false,false,false,false,false);
        let start=OverworldInput::new(false,true,false,false,false,false,true,false);
        for (t, counter) in trace.into_iter().enumerate() {
            assert_eq!(screen.update_frame(if t>=5 {start} else {down}),ScreenAction::Continue);
            assert_eq!(screen.state.walk_counter,counter,"{transport:?}, t{t}");
            assert_eq!(screen.state.player.y,if t<landing {5} else {6});
        }
        assert_eq!(screen.state.player.movement_state,MovementState::Idle);
        assert_eq!(screen.update_frame(start),ScreenAction::Continue,"first wait after landing");
        assert_eq!(screen.update_frame(start),ScreenAction::Transition(GameScreen::StartMenu));
    }
}

#[test]
fn ordinary_turn_wait_uses_last_stop_instead_of_visible_facing() {
    use super::MovementState;
    let idle=OverworldInput::new(false,false,false,false,false,false,false,false);
    let down=OverworldInput::new(false,true,false,false,false,false,false,false);
    let mut screen=screen_on(MapId::PalletTown);fill_map_with_passable_block(&mut screen);
    screen.state.player.x=5;screen.state.player.y=5;screen.state.player.facing=Direction::Down;
    screen.player_last_stop_direction=2;screen.check_player_turn=true;
    // Same visible facing still needs the turn delay after Continue reset it.
    screen.update_frame(down);
    assert_eq!(screen.state.player.movement_state,MovementState::Idle);
    assert_eq!(screen.player_moving_direction,4);
    screen.update_frame(idle);screen.update_frame(idle);
    assert_eq!((screen.state.player.x,screen.state.player.y),(5,5),"short turn pulse does not walk");
    assert_eq!(screen.player_last_stop_direction,4);
    screen.update_frame(down);screen.update_frame(down);
    assert_eq!(screen.state.walk_counter,7,"matching stopped direction now walks");
}

#[test]
fn boulder_map_script_arms_on_idle_and_uses_the_previous_field_sample() {
    let mut screen=screen_on(MapId::PalletTown);fill_map_with_passable_block(&mut screen);
    screen.state.player.x=5;screen.state.player.y=5;screen.state.player.facing=Direction::Down;
    screen.player_last_stop_direction=4;screen.strength_active=true;
    screen.npc_states.push(make_boulder(5,6));
    let idle=OverworldInput::new(false,false,false,false,false,false,false,false);
    let down=OverworldInput::new(false,true,false,false,false,false,false,false);
    screen.update_frame(idle);screen.update_frame(idle);
    assert!(screen.tried_push_boulder,"RunMapScript arms even with hJoyHeld=0");
    screen.update_frame(down);
    assert!(screen.boulder_push.is_none(),"script precedes the new Joypad sample");
    screen.update_frame(idle);
    screen.update_frame(idle);
    assert!(screen.boulder_push.is_some(),"next script sees the previous held direction even after physical release");
    assert_eq!(screen.boulder_push.unwrap().frame,0,"MoveSprite is the phase origin");
    screen.update_frame(idle);screen.update_frame(idle);
    assert_eq!(boulder_pos(&screen),(5,7),"TryWalking commits two hardware frames after MoveSprite");
}

#[test]
fn menu_joypad_replaces_stale_direction_before_boulder_script_runs() {
    use crate::game_state::{ScreenAction,GameScreen};
    let mut screen=screen_on(MapId::PalletTown);fill_map_with_passable_block(&mut screen);
    screen.state.player.x=5;screen.state.player.y=5;screen.state.player.facing=Direction::Down;
    screen.player_last_stop_direction=4;screen.strength_active=true;
    screen.npc_states.push(make_boulder(5,6));
    let idle=OverworldInput::new(false,false,false,false,false,false,false,false);
    let down_start=OverworldInput::new(false,true,false,false,false,false,true,false);
    let menu_a=OverworldInput::new(false,false,false,false,true,false,false,false);
    screen.update_frame(idle);screen.update_frame(idle);
    assert!(screen.tried_push_boulder);
    assert_eq!(screen.update_frame(down_start),ScreenAction::Transition(GameScreen::StartMenu));
    // EXIT read A after the user released d-pad: the original menu Joypad
    // replaced hJoyHeld. Neither the old direction nor its START may survive.
    screen.synchronize_player_input(menu_a);
    screen.update_frame(menu_a);screen.update_frame(menu_a);
    assert!(screen.boulder_push.is_none(),"menu close must not reuse the pre-menu Down");
    assert_eq!(boulder_pos(&screen),(5,6));
    assert!(screen.pending_dialogue.is_none(),"held menu confirmation is not a new field A");
}

#[test]
fn victory_road_hole_event_precedes_final_oam_image() {
    use pokered_data::event_flags::EventFlag;
    let mut screen=screen_on(MapId::VictoryRoad3F);
    screen.npc_states.clear();
    screen.npc_states.push(make_boulder(22,15));
    screen.boulder_push=Some(presentation::BoulderPushState {
        npc_index:0,direction:Direction::Right,anchor:(21,15),
        origin:(22,15),destination:(23,15),frame:0,switch_block:None,redraw_remaining:0,walk_wait:0,
    });
    // Original MoveSprite at t3, HideObject/ShowObject/SFX_CUT at t73.
    // The final displayed boulder persists until t75; no time shift.
    for frame in 1..=72 {
        screen.advance_boulder_push();
        assert_eq!(screen.unified_flags.check(EventFlag::EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2),frame>=70);
        assert_eq!(screen.npc_states[0].visible,frame<70);
        assert_eq!(screen.boulder_push.is_some(),frame<72);
    }
}

#[test]
fn boulder_completion_samples_start_before_the_last_lcd_image() {
    use crate::game_state::{ScreenAction,GameScreen};
    let mut screen=screen_on(MapId::VictoryRoad3F);
    screen.npc_states.clear();screen.npc_states.push(make_boulder(22,15));
    screen.state.player.x=21;screen.state.player.y=15;screen.state.player.facing=Direction::Right;
    screen.boulder_push=Some(presentation::BoulderPushState {
        npc_index:0,direction:Direction::Right,anchor:(21,15),origin:(22,15),destination:(23,15),frame:69,switch_block:None,redraw_remaining:0,walk_wait:0,
    });
    let start=OverworldInput::new(false,false,false,false,false,false,true,false);
    let idle=OverworldInput::new(false,false,false,false,false,false,false,false);
    assert_eq!(screen.update_frame(start),ScreenAction::Continue);
    assert_eq!(screen.boulder_push.unwrap().frame,70);
    assert!(!screen.boulder_blocks_control());
    // A single physical pulse on original Joypad t73 survives processing t74,
    // while OAM still contains the boulder. It need not remain held until75.
    assert_eq!(screen.update_frame(idle),ScreenAction::Transition(GameScreen::StartMenu));
    assert_eq!(screen.boulder_push.unwrap().frame,71);
    screen.tick_boulder_presentation_during_ui();
    assert!(screen.boulder_push.is_none(),"opening START cannot freeze the hidden stone's LCD image");
}


#[test]
fn ordinary_player_pose_and_camera_match_original_hardware_frames() {
    // Independent golden transitions from original Red actual SRAM Continue,
    // road (20,30), 32-frame foot / 16-frame bicycle inputs. Each reference
    // was recorded twice. Camera registers are latched into the next LCD frame;
    // sprite poses were classified from opaque original gfx pixels.
    type PoseTrace = &'static [(i32, usize, bool)];
    type CameraTrace = &'static [(i32, i16, i16)];
    let cases: &[(bool, Direction, PoseTrace, CameraTrace)] = &[
        (false, Direction::Left, &[(-1, 0, false), (2, 2, false), (9, 5, false), (17, 2, false), (26, 5, false), (34, 2, false)], &[(0, 0, 0), (2, -2, 0), (4, -4, 0), (6, -6, 0), (8, -8, 0), (10, -10, 0), (12, -12, 0), (14, -14, 0), (16, -16, 0), (19, -18, 0), (21, -20, 0), (23, -22, 0), (25, -24, 0), (27, -26, 0), (29, -28, 0), (31, -30, 0), (33, -32, 0)]),
        (false, Direction::Right, &[(-1, 0, false), (4, 2, true), (11, 5, true), (19, 2, true), (28, 5, true), (36, 2, true)], &[(0, 0, 0), (4, 2, 0), (6, 4, 0), (8, 6, 0), (10, 8, 0), (12, 10, 0), (14, 12, 0), (16, 14, 0), (18, 16, 0), (21, 18, 0), (23, 20, 0), (25, 22, 0), (27, 24, 0), (29, 26, 0), (31, 28, 0), (33, 30, 0), (35, 32, 0)]),
        (false, Direction::Up, &[(-1, 0, false), (4, 1, false), (11, 4, false), (19, 1, false), (28, 4, true), (36, 1, false)], &[(0, 0, 0), (4, 0, -2), (6, 0, -4), (8, 0, -6), (10, 0, -8), (12, 0, -10), (14, 0, -12), (16, 0, -14), (18, 0, -16), (21, 0, -18), (23, 0, -20), (25, 0, -22), (27, 0, -24), (29, 0, -26), (31, 0, -28), (33, 0, -30), (35, 0, -32)]),
        (false, Direction::Down, &[(-1, 0, false), (11, 3, false), (19, 0, false), (28, 3, true), (36, 0, false)], &[(0, 0, 0), (4, 0, 2), (6, 0, 4), (8, 0, 6), (10, 0, 8), (12, 0, 10), (14, 0, 12), (16, 0, 14), (18, 0, 16), (21, 0, 18), (23, 0, 20), (25, 0, 22), (27, 0, 24), (29, 0, 26), (31, 0, 28), (33, 0, 30), (35, 0, 32)]),
        (true, Direction::Left, &[(-1, 0, false), (3, 2, false), (10, 5, false), (19, 2, false)], &[(0, 0, 0), (3, -4, 0), (5, -8, 0), (7, -12, 0), (9, -16, 0), (12, -20, 0), (14, -24, 0), (16, -28, 0), (18, -32, 0)]),
        (true, Direction::Right, &[(-1, 0, false), (5, 2, true), (12, 5, true), (21, 2, true)], &[(0, 0, 0), (5, 4, 0), (7, 8, 0), (9, 12, 0), (11, 16, 0), (14, 20, 0), (16, 24, 0), (18, 28, 0), (20, 32, 0)]),
        (true, Direction::Up, &[(-1, 0, false), (5, 1, false), (12, 4, false), (21, 1, false)], &[(0, 0, 0), (5, 0, -4), (7, 0, -8), (9, 0, -12), (11, 0, -16), (14, 0, -20), (16, 0, -24), (18, 0, -28), (20, 0, -32)]),
        (true, Direction::Down, &[(-1, 0, false), (12, 3, false), (21, 0, false)], &[(0, 0, 0), (5, 0, 4), (7, 0, 8), (9, 0, 12), (11, 0, 16), (14, 0, 20), (16, 0, 24), (18, 0, 28), (20, 0, 32)]),
    ];
    let idle = OverworldInput::new(false,false,false,false,false,false,false,false);
    for &(bike,direction,poses,cameras) in cases {
        let mut screen=screen_on(MapId::PalletTown);fill_map_with_passable_block(&mut screen);
        screen.state.player.x=5;screen.state.player.y=5;screen.state.player.facing=Direction::Down;
        screen.state.player.transport=if bike {TransportMode::Biking} else {TransportMode::Walking};
        screen.player_last_stop_direction=2;screen.check_player_turn=true;
        screen.field_loop_wait=if bike {1} else {0};
        let held=OverworldInput::new(direction==Direction::Up,direction==Direction::Down,
            direction==Direction::Left,direction==Direction::Right,false,false,false,false);
        for t in 0..100 {
            screen.update_frame(if t < if bike {16} else {32} {held} else {idle});
            let &(_,frame,flip)=poses.iter().rev().find(|&&(start,_,_)| start<=t).unwrap();
            assert_eq!(screen.ordinary_player_sprite_frame(),Some((frame,flip)),"{bike}/{direction:?} pose t{t}");
            let &(_,x,y)=cameras.iter().rev().find(|&&(start,_,_)| start<=t).unwrap();
            let view=screen.ordinary_player_camera().unwrap();
            assert_eq!(((view.x as i16-5)*16+view.sub_x,(view.y as i16-5)*16+view.sub_y),
                (x,y),"{bike}/{direction:?} camera t{t}");
        }
    }
}


#[test]
fn mid_step_snapshot_replays_pending_player_presentation() {
    use crate::snapshot::OverworldSnapshot;
    let idle=OverworldInput::new(false,false,false,false,false,false,false,false);
    let down=OverworldInput::new(false,true,false,false,false,false,false,false);
    for bike in [false,true] {
        let mut original=screen_on(MapId::PalletTown);fill_map_with_passable_block(&mut original);
        original.state.player.x=5;original.state.player.y=5;
        original.state.player.transport=if bike {TransportMode::Biking} else {TransportMode::Walking};
        original.player_last_stop_direction=2;original.check_player_turn=true;
        original.field_loop_wait=if bike {1} else {0};
        for _ in 0..10 {original.update_frame(down);}
        // This boundary includes an in-flight OAM pose and a latched viewport.
        let encoded=serde_json::to_string(&OverworldSnapshot::capture(&original)).unwrap();
        let snapshot: OverworldSnapshot=serde_json::from_str(&encoded).unwrap();
        let mut restored=screen_on(MapId::PalletTown);snapshot.restore_into(&mut restored);
        for t in 10..100 {
            let input=if t < if bike {16} else {32} {down} else {idle};
            original.update_frame(input);restored.update_frame(input);
            assert_eq!(serde_json::to_string(&OverworldSnapshot::capture(&original)).unwrap(),
                serde_json::to_string(&OverworldSnapshot::capture(&restored)).unwrap(),"{bike} t{t}");
        }
    }
}

#[test]
fn boulder_logical_counter_matches_original_during_map_redraw() {
    use pokered_data::event_flags::EventFlag;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/boulder-counter-139.json"
    )).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let (map, x, y, flag) = match case["case"].as_str().unwrap() {
            "1f" => (MapId::VictoryRoad1F, 17, 11, EventFlag::EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH),
            "2f1" => (MapId::VictoryRoad2F, 1, 14, EventFlag::EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1),
            _ => unreachable!(),
        };
        let mut screen = screen_on(map);
        screen.state.player.x = x;
        screen.state.player.y = y;
        screen.state.player.facing = Direction::Down;
        screen.strength_active = true;
        screen.npc_states.clear();
        screen.npc_states.push(make_boulder(x, y + 1));
        // The actual native Continue/Strength capture starts MoveSprite at
        // hardware t3. Original2F1 starts t2 but its actor update crosses
        // VBlank; both logical TryWalking updates land at t5. These primary
        // counter traces do not prove the unmodelled CPU/OAM initialization.
        for row in case["trace"].as_array().unwrap() {
            let values = row.as_array().unwrap();
            let t = values[0].as_i64().unwrap();
            if t == 3 {
                screen.tick_boulder_push(Some(Direction::Down));
                screen.tick_boulder_push(Some(Direction::Down));
                assert!(screen.boulder_push.is_some());
            } else if t > 3 {
                screen.advance_boulder_push();
            }
            let npc = &screen.npc_states[0];
            assert_eq!(
                [u64::from(npc.x), u64::from(npc.y), u64::from(npc.walk_counter),
                    u64::from(screen.unified_flags.check(flag))],
                [values[1].as_u64().unwrap(), values[2].as_u64().unwrap(),
                    values[3].as_u64().unwrap(), values[4].as_u64().unwrap()],
                "{map:?} hardware t{t}"
            );
            if let Some(push) = screen.boulder_push {
                let encoded = serde_json::to_value(push).unwrap();
                let restored: presentation::BoulderPushState = serde_json::from_value(encoded.clone()).unwrap();
                assert_eq!(restored, push, "walking update phase survives JSON at t{t}");
                let mut legacy = encoded;
                legacy.as_object_mut().unwrap().remove("walk_wait");
                let restored: presentation::BoulderPushState = serde_json::from_value(legacy).unwrap();
                assert_eq!(restored.walk_wait, u8::MAX, "legacy phase is derived on resume");
            }
        }
    }
}

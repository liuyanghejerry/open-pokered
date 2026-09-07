//! Visual verification for the 2026-09 fidelity-gap batch — the surfaces that
//! change on-screen output:
//!
//! 1. Bench guys (tx_pre hidden text events): A on the Pewter Pokécenter
//!    bench tile now shows the "Yawn! When JIGGLYPUFF sings..." text.
//! 2. Wall TOWN MAP (bookshelf tile table `House` $3D): "A TOWN MAP." text
//!    and the TownMap screen hand-off.
//! 3. FLY arrival bird (EnterMapAnim `.flyAnimation`): the bird sprite
//!    glides in instead of the spin-in.
//! 4. Trainer sight engagement (TalkToTrainer): the BEFORE-battle text now
//!    displays after the "!" walk-up (the fight fires once it closes).
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_gap_batch -- --nocapture
//!
//! PNGs land under docs/screenshots/2026-09-gaps/ (committed PR evidence).

use dotzuki_engine::render_config::RenderConfig;
use pokered_app::render::draw_overworld;
use pokered_core::game_state::Lang;
use pokered_core::overworld::{Direction, OverworldInput, OverworldScreen};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;
use pokered_renderer::{resource::ResourceManager, FrameBuffer, Rgba};

fn maps_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pokered-data/maps")
}

fn out_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/screenshots/2026-09-gaps")
}

fn input(a: bool) -> OverworldInput {
    OverworldInput::new(false, false, false, false, a, false, false, false)
}

fn new_rm() -> Option<ResourceManager> {
    let root = pokered_renderer::resource::AssetRoot::auto_detect()
        .expect("cannot auto-detect gfx/ asset root — run scripts/fetch-gfx.sh first");
    Some(ResourceManager::new(root))
}

fn render_and_save(screen: &mut OverworldScreen, rm: &mut Option<ResourceManager>, path: &std::path::Path) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_overworld(screen, rm, &mut fb, Lang::default());
    let mut img = image::RgbaImage::new(fb.width(), fb.height());
    for y in 0..fb.height() {
        for x in 0..fb.width() {
            if let Some(color) = fb.get_pixel(x, y) {
                let c = color.to_array();
                img.put_pixel(x, y, image::Rgba(c));
            }
        }
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    img.save(path).expect("failed to save PNG");
    eprintln!("saved: {}", path.display());
}

fn render_bench_zh(screen: &mut OverworldScreen, rm: &mut Option<ResourceManager>, path: &std::path::Path) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_overworld(screen, rm, &mut fb, Lang::Zh);
    let mut img = image::RgbaImage::new(fb.width(), fb.height());
    for y in 0..fb.height() {
        for x in 0..fb.width() {
            if let Some(color) = fb.get_pixel(x, y) {
                let c = color.to_array();
                img.put_pixel(x, y, image::Rgba(c));
            }
        }
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    img.save(path).unwrap();
    eprintln!("saved: {}", path.display());
}

fn settle_dialogue(screen: &mut OverworldScreen, press_a_every: u32, frames: u32) -> bool {
    let mut saw_dialogue = false;
    for frame in 0..frames {
        let a = frame % press_a_every == 0;
        screen.update_frame(input(a));
        if screen.pending_dialogue.is_some() {
            saw_dialogue = true;
        }
    }
    saw_dialogue
}

/// 1. Bench guy: stand right of the Pewter Pokécenter bench (0,4) facing
/// LEFT and press A — the "Yawn! When JIGGLYPUFF sings..." text shows.
#[test]
fn render_bench_guy_text() {
    let mut rm = new_rm();
    let mut screen =
        OverworldScreen::new(MapId::PewterPokecenter, Some(maps_dir()), PokemonRedData);
    screen.state.player.x = 1;
    screen.state.player.y = 4;
    screen.state.player.facing = Direction::Left;

    assert!(
        settle_dialogue(&mut screen, 40, 400),
        "bench guy text never appeared"
    );
    render_bench_zh(&mut screen, &mut rm, &out_dir().join("bench-guy-after.png"));
}

/// 2. Wall TOWN MAP in BluesHouse: face up at the bookshelf row (House $3D),
/// press A — "A TOWN MAP." shows and the TownMap hand-off is queued.
#[test]
fn render_wall_town_map_text() {
    let mut rm = new_rm();
    let mut screen = OverworldScreen::new(MapId::BluesHouse, Some(maps_dir()), PokemonRedData);
    // The wall map hangs at (3,0) (House tile $3D).
    screen.state.player.x = 3;
    screen.state.player.y = 1;
    screen.state.player.facing = Direction::Up;

    let mut saw_town_map_pending = false;
    for frame in 0..400 {
        let a = frame % 40 == 0;
        screen.update_frame(input(a));
        if screen.pending_town_map {
            saw_town_map_pending = true;
            break;
        }
    }
    assert!(saw_town_map_pending, "wall town map never triggered");
    // Let the typewriter reveal the text before capturing.
    for _ in 0..40 {
        screen.update_frame(input(false));
    }
    render_and_save(
        &mut screen,
        &mut rm,
        &out_dir().join("wall-town-map-after.png"),
    );
}

/// 3. FLY arrival bird: fly to Pallet Town and render a mid-flight frame —
/// the bird sprite is on screen gliding toward the landing spot, and the
/// player is hidden until it lands.
#[test]
fn render_fly_bird_mid_arrival() {
    let mut rm = new_rm();
    let mut screen = OverworldScreen::new(MapId::Route1, Some(maps_dir()), PokemonRedData);
    screen.fly_warp_to(MapId::PalletTown, 5, 6);

    // Step through the fade-out/warp-commit/fade-in until the bird anim is
    // running, then stop mid-flight for the capture.
    let mut captured = false;
    for _ in 0..400 {
        screen.update_frame(input(false));
        if let Some(fly) = screen.enter_map_fly_anim.as_ref() {
            if fly.frame >= 15 {
                captured = true;
                break;
            }
        }
    }
    assert!(captured, "fly arrival bird never reached mid-flight");
    assert!(screen.enter_map_fly_anim.as_ref().unwrap().is_done() == false);
    render_and_save(
        &mut screen,
        &mut rm,
        &out_dir().join("fly-bird-after.png"),
    );
}

/// 4. Trainer sight engagement: stand on the Viridian Forest Bug Catcher's
/// sight line — after the "!" walk-up the BEFORE-battle text displays (the
/// fight fires once it closes).
#[test]
fn render_trainer_prebattle_text() {
    let mut rm = new_rm();
    let mut screen =
        OverworldScreen::new(MapId::ViridianForest, Some(maps_dir()), PokemonRedData);
    screen.state.player.x = 29;
    screen.state.player.y = 33;

    let mut engaged = false;
    for _ in 0..240 {
        screen.update_frame(input(false));
        if screen.pending_dialogue.is_some() {
            engaged = true;
            break;
        }
    }
    assert!(engaged, "sight trainer's before-battle text never showed");
    // Let the typewriter reveal the text before capturing.
    for _ in 0..40 {
        screen.update_frame(input(false));
    }
    render_and_save(
        &mut screen,
        &mut rm,
        &out_dir().join("trainer-prebattle-text-after.png"),
    );
}


// ── Surfaces previously noted as "not headless-capturable" ─────────────────

use pokered_app::render::draw_battle;
use pokered_app::render::BattleVisualEffects;
use pokered_core::battle::state::StatusCondition;
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::overworld::poison;
use pokered_core::save::SaveData;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_core::overworld::screen::{OverworldGameDataRequest, PendingWarp, WarpFadeState};

fn battle_render_and_save(
    battle: &mut BattleScreen,
    rm: &mut Option<ResourceManager>,
    path: &std::path::Path,
) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut effects = BattleVisualEffects::default();
    draw_battle(battle, rm, &mut fb, &mut effects, Lang::default());
    let mut img = image::RgbaImage::new(fb.width(), fb.height());
    for y in 0..fb.height() {
        for x in 0..fb.width() {
            if let Some(color) = fb.get_pixel(x, y) {
                let c = color.to_array();
                img.put_pixel(x, y, image::Rgba(c));
            }
        }
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    img.save(path).expect("failed to save PNG");
    eprintln!("saved: {}", path.display());
}

fn battle_input(a: bool, down: bool) -> BattleInput {
    BattleInput { down, a, ..BattleInput::none() }
}

fn gap_battle() -> BattleScreen {
    let player = vec![pokered_core::pokemon::stats::create_pokemon_with_moves(
        Species::Pikachu,
        20,
        [0x9A, 0x78],
        [MoveId::Thundershock, MoveId::Growl, MoveId::TailWhip, MoveId::QuickAttack],
    )
    .unwrap()];
    let enemy = vec![pokered_core::pokemon::stats::create_pokemon(
        Species::Pidgey,
        5,
        [0x9A, 0x78],
    )
    .unwrap()];
    let mut b = BattleScreen::from_parties(false, &player, &enemy, None);
    b.player_name = Some("RED".to_string());
    b
}

/// 5a. Full-slot learn prompt — the TryingToLearn text + YES/NO menu
/// (learnmove.asm `TryingToLearn`). This UI did not exist before.
#[test]
fn render_learn_move_ask_menu() {
    let mut rm = new_rm();
    let mut battle = gap_battle();
    battle.pending_learn_moves = vec![(0, MoveId::Thunderbolt)];
    let mut msgs = vec!["PIKACHU grew to level 21!".to_string()];
    let next = battle.wrap_learn_prompt(&mut msgs, BattlePhase::PlayerMenu);
    battle.phase = next;
    battle.post_text_transition();
    battle_render_and_save(&mut battle, &mut rm, &out_dir().join("learn-move-ask-after.png"));
    assert!(matches!(battle.phase, BattlePhase::LearnMoveAsk { .. }));
}

/// 5b. The forget list ("Which move should be forgotten?" over the 4-move
/// menu). YES from the ask opens it.
#[test]
fn render_learn_move_forget_list() {
    let mut rm = new_rm();
    let mut battle = gap_battle();
    battle.pending_learn_moves = vec![(0, MoveId::Thunderbolt)];
    let mut msgs = vec![];
    battle.phase = battle.wrap_learn_prompt(&mut msgs, BattlePhase::PlayerMenu);
    battle.post_text_transition();
    battle.update_frame(BattleInput { up: true, ..BattleInput::none() }); // toggle to YES
    battle.update_frame(battle_input(true, false)); // A on YES → the list
    assert!(matches!(battle.phase, BattlePhase::LearnMoveChoose { .. }));
    battle_render_and_save(&mut battle, &mut rm, &out_dir().join("learn-move-choose-after.png"));
}

/// 5c. BEFORE representation: a full-slot level-up used to silently overwrite
/// the 4th move — the only thing the player ever saw was "learned!" (rendered
/// through the same unchanged ShowingText path master used).
#[test]
fn render_learn_move_silent_before() {
    let mut rm = new_rm();
    let mut battle = gap_battle();
    battle.phase = BattlePhase::ShowingText {
        messages: vec!["PIKACHU learned
THUNDERBOLT!".to_string()],
        current: 0,
        wait_frames: 0,
        next_phase: Box::new(BattlePhase::PlayerMenu),
    };
    battle.current_message = Some("PIKACHU learned
THUNDERBOLT!".to_string());
    battle_render_and_save(&mut battle, &mut rm, &out_dir().join("learn-move-silent-before.png"));
}

/// 6. Poké Doll in a TRAINER battle: the OAK refusal shows and the doll is
/// NOT consumed. (Master escaped the trainer battle with it.)
#[test]
fn render_poke_doll_refusal() {
    let mut rm = new_rm();
    let mut battle = gap_battle();
    battle.player_bag.add_item(ItemId::PokeDoll, 1).unwrap();
    battle.phase = BattlePhase::PlayerMenu;
    // Open the bag and use the doll: cursor Down → BAG → A → A.
    // PlayerMenu: cursor Down → BAG → A opens the bag → A uses the doll.
    battle.update_frame(BattleInput { down: true, ..BattleInput::none() });
    battle.update_frame(battle_input(true, false));
    battle.update_frame(battle_input(true, false));
    assert!(
        matches!(battle.phase, BattlePhase::ShowingText { .. }),
        "expected the refusal text, got {:?}",
        battle.phase
    );
    battle_render_and_save(&mut battle, &mut rm, &out_dir().join("poke-doll-refusal-after.png"));
}

/// 7. Seafoam B3F boulder chain: on FIRST entry (no fall flags) boulders
/// 5/6 (npc 5/6 at the top channel) are HIDDEN, faithful to
/// toggleable_objects.asm; master showed them immediately.
#[test]
fn render_seafoam_b3f_first_visit() {
    let mut rm = new_rm();
    let mut screen =
        OverworldScreen::new(MapId::SeafoamIslandsB3F, Some(maps_dir()), PokemonRedData);
    let hidden = |s: &OverworldScreen, i: usize| !s.npc_states[i].visible;
    assert!(
        hidden(&screen, 4) && hidden(&screen, 5),
        "boulders 5/6 must start hidden"
    );
    render_and_save(
        &mut screen,
        &mut rm,
        &out_dir().join("seafoam-b3f-first-visit-after.png"),
    );
}

/// 8. Seafoam B3F forced current: before both B2F boulders fall, standing on
/// (15,8) sweeps the player DOWN→RIGHT→DOWN (MoveObject RLE). Master left
/// the player standing. Before-side renders the same map without the sweep.
#[test]
fn render_seafoam_b3f_current_sweep() {
    let mut rm = new_rm();
    let mut screen =
        OverworldScreen::new(MapId::SeafoamIslandsB3F, Some(maps_dir()), PokemonRedData);
    screen.set_flag_live("EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE", false);
    screen.set_flag_live("EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE", false);
    // Re-enter through the real warp path so the @load showObject runs, then
    // land above the trigger tile and step onto (15,8).
    screen.pending_warp = Some(PendingWarp {
        dest_map: MapId::SeafoamIslandsB3F,
        dest_x: 15,
        dest_y: 7,
        save_last_map: false,
        arrival_spin: false,
    });
    screen.warp_fade_to_white = true;
    screen.warp_fade_state = WarpFadeState::FadingOut {
        frames_remaining: pokered_core::overworld::screen::WARP_FADE_OUT_WHITE_FRAMES,
    };
    // Wait for the warp to settle, then STEP onto the trigger tile (15,8).
    let mut settled = false;
    for _ in 0..200 {
        screen.update_frame(input(false));
        if screen.warp_fade_state == WarpFadeState::Idle && screen.pending_warp.is_none() {
            settled = true;
            break;
        }
    }
    assert!(settled, "warp never settled");
    assert_eq!((screen.state.player.x, screen.state.player.y), (15, 7));
    // (15,8) is water — the sweep targets a SURFING player (the original
    // arrives here surfing the waterfall channel).
    screen.state.player.transport = dotzuki_engine::overworld::TransportMode::Surfing;
    // Hold DOWN until the step onto (15,8) completes (a step is ~16 frames).
    for _ in 0..24 {
        screen.update_frame(OverworldInput::new(
            false, true, false, false, false, false, false, false,
        ));
    }
    let mut sweeping = false;
    for _ in 0..300 {
        screen.update_frame(input(false));
        if screen.state.player.y >= 9 {
            sweeping = true;
            break;
        }
    }
    assert!(sweeping, "the current never swept the player (y={})", screen.state.player.y);
    render_and_save(
        &mut screen,
        &mut rm,
        &out_dir().join("seafoam-b3f-current-after.png"),
    );
}

/// 9. Out-of-battle poison: a poisoned party member at 1 HP faints on the
/// tick with its own text box (poison.asm). Master had no such mechanic —
/// the before-side renders the same field with nothing happening.
#[test]
fn render_poison_faint_text() {
    let mut rm = new_rm();
    let mut screen = OverworldScreen::new(MapId::Route1, Some(maps_dir()), PokemonRedData);
    let mut save = SaveData::new();
    let mut mon = pokered_core::pokemon::stats::create_pokemon(
        Species::Rattata,
        10,
        [0x9A, 0x78],
    )
    .unwrap();
    mon.status = StatusCondition::Poison;
    mon.hp = 1;
    save.party.add(mon).unwrap();
    poison::apply_out_of_battle_poison_damage(&mut save, &mut screen);
    assert!(screen.pending_dialogue.is_some(), "faint text never queued");
    for _ in 0..30 {
        screen.update_frame(input(false));
    }
    render_and_save(&mut screen, &mut rm, &out_dir().join("poison-faint-after.png"));
}

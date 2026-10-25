use super::*;
use pokered_core::battle::{BattleInput, state};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::species::Species;
use pokered_renderer::resource::AssetRoot;

fn fixture(enemy: bool, first: bool, condition: &str) -> BattleScreen {
    let make = |species, mv| create_pokemon_with_moves(species, 40, [0x9a, 0x78],
        [mv, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let mut actor = make(Species::Mewtwo, MoveId::Transform); actor.set_nickname("COPY");
    let mut target = make(Species::Snorlax, MoveId::Splash); target.set_nickname("TANK");
    if condition == "frozen" { actor.status = state::StatusCondition::Freeze; }
    let bench = make(Species::Tangela, MoveId::Splash);
    let (player, foe) = if enemy { (target, actor) } else { (actor, target) };
    let mut b = BattleScreen::from_parties(true, &[player, bench], &[foe], None);
    b.link_rng = Some(LinkRng::new([255, 255, 0, 255, 3, 82, 73, 64, 55, 46]));
    b.link_enemy_move_override = Some((foe.moves[0], 0));
    let bs = b.battle_state.as_mut().unwrap();
    let (actor, target) = if enemy { (&mut bs.enemy, &mut bs.player) } else { (&mut bs.player, &mut bs.enemy) };
    actor.active_mon_mut().speed = if first { 200 } else { 100 };
    target.active_mon_mut().speed = if first { 100 } else { 200 };
    if condition == "self_invulnerable" { actor.set_status1(state::status1::INVULNERABLE); }
    b.set_presentation_enabled(true); b.phase = BattlePhase::PlayerMenu; b
}

fn tick(b: &mut BattleScreen, fx: &mut BattleVisualEffects, input: BattleInput) {
    if b.presentation.waiting && fx.is_frame_stable() { b.complete_move_presentation(); }
    b.update_frame(input); fx.update(b);
}

fn turn(b: &mut BattleScreen, fx: &mut BattleVisualEffects, mut check: impl FnMut(u32, &BattleScreen, &BattleVisualEffects)) {
    let mut started = false;
    for frame in 0..4000 {
        tick(b, fx, BattleInput { a: frame == 0 || frame == 2 || (frame > 2 && frame % 16 == 0), ..BattleInput::none() });
        check(frame, b, fx);
        if b.phase == BattlePhase::PlayerMenu {
            if started { return; }
        } else { started = true; }
    }
    panic!("turn did not finish: {:?}", b.phase);
}

#[test]
fn transform_picture_changes_at_the_command_with_animations_on_or_off() {
    for enemy in [false, true] { for first in [false, true] { for enabled in [false, true] {
        let mut b = fixture(enemy, first, "normal");
        let mut fx = BattleVisualEffects::default(); fx.animations_enabled = enabled;
        let mut transfer_seen = false; let mut result_seen = false;
        turn(&mut b, &mut fx, |frame, b, fx| {
            let species = fx.displayed_species(b)[usize::from(enemy)];
            if frame == 3 && (enabled || !first) { assert_eq!(species, Species::Mewtwo, "early enemy={enemy} first={first} animations={enabled}"); }
            if fx.transform_raster.is_some() && b.presentation.current_transform_target().is_some() {
                transfer_seen = true;
                assert_eq!(species, Species::Snorlax);
                // The raster/animation owns the preview; the core publishes
                // the stable identity at the following result page.
                assert_eq!(if enemy { b.enemy_species } else { b.player_species }, Species::Mewtwo);
                if !enabled { assert_eq!(fx.anim_disabled_wait, 0); }
            }
            if b.current_message.as_ref().is_some_and(|m| m.contains("transformed")) {
                result_seen = true;
                assert_eq!(species, Species::Snorlax);
                assert_eq!(if enemy { b.enemy_species } else { b.player_species }, Species::Snorlax);
            }
        });
        assert!(transfer_seen && result_seen);
        assert_eq!(fx.displayed_species(&b)[usize::from(enemy)], Species::Snorlax);
        assert_eq!(b.link_rng.as_ref().unwrap().consumed(), 0);
    }}}
}

#[test]
fn failed_or_frozen_transform_never_changes_the_picture() {
    for (enemy, condition) in [(false, "self_invulnerable"), (false, "frozen"), (true, "frozen")] {
        for first in [false, true] { for enabled in [false, true] {
            let mut b = fixture(enemy, first, condition);
            let mut fx = BattleVisualEffects::default(); fx.animations_enabled = enabled;
            turn(&mut b, &mut fx, |_, b, fx| {
                assert_eq!(fx.displayed_species(b)[usize::from(enemy)], Species::Mewtwo);
                assert!(!fx.transform_picture_changed);
            });
            assert_eq!(b.link_rng.as_ref().unwrap().consumed(), 0);
        }}
    }
}

#[test]
fn transformed_enemy_keeps_the_copied_picture_after_public_party_switch() {
    let mut b = fixture(true, false, "normal"); let mut fx = BattleVisualEffects::default();
    turn(&mut b, &mut fx, |_, _, _| {});
    b.link_enemy_move_override = Some((MoveId::Splash, 0));
    // PKMN -> second party member -> SWITCH, through public menu input.
    for step in 0..=8 {
        tick(&mut b, &mut fx, BattleInput { right: step == 0, down: step == 4,
            a: matches!(step, 2 | 6 | 8), ..BattleInput::none() });
    }
    assert_eq!(b.battle_state.as_ref().unwrap().player.active_pokemon_index, 1);
    for frame in 0..4000 {
        if b.phase == BattlePhase::PlayerMenu { break; }
        tick(&mut b, &mut fx, BattleInput { a: frame % 16 == 0, ..BattleInput::none() });
    }
    assert_eq!(b.phase, BattlePhase::PlayerMenu);
    assert_eq!(b.enemy_species, Species::Snorlax);
    assert_eq!(b.player_species, Species::Tangela);
    let mut resources = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
    let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
    let mut actual = FrameBuffer::new(config, Rgba::WHITE);
    draw_battle(&b, &mut resources, &mut actual, &mut fx, Lang::En);
    let make = |species| create_pokemon_with_moves(species, 40, [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let mut expected = BattleScreen::from_parties(true, &[make(Species::Tangela)], &[make(Species::Snorlax)], None);
    expected.phase = BattlePhase::PlayerMenu;
    let mut expected_fx = BattleVisualEffects::default(); expected_fx.update(&expected);
    let mut reference = FrameBuffer::new(config, Rgba::WHITE);
    draw_battle(&expected, &mut resources, &mut reference, &mut expected_fx, Lang::En);
    let mut ink = 0;
    for y in 0..56 { for x in 96..152 {
        let pixel = reference.get_pixel(x, y);
        ink += usize::from(pixel != Some(Rgba::WHITE));
        assert_eq!(actual.get_pixel(x, y), pixel, "enemy picture at {x},{y}");
    }}
    assert!(ink > 100);
    assert_eq!(b.link_rng.as_ref().unwrap().consumed(), 0);
}

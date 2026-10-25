//! The original redraws the active HUD after GainedText, before GrewLevelText.
//! Prepared public FIGHT turns; rendering and original-ROM timing are separate.
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_core::snapshot::BattleSnapshot;
use pokered_data::{items::ItemId, moves::MoveId, species::Species};

fn a() -> BattleInput { BattleInput { a: true, ..BattleInput::none() } }
fn hud(b: &BattleScreen) -> (u8, u16, u16) { (b.player_level, b.player_hp, b.player_max_hp) }
fn fixture(exp_all: bool, first: bool, enabled: bool) -> BattleScreen {
    let mon = |species, level, mv| create_pokemon_with_moves(species, level, [0x9a, 0x78],
        [mv, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let mut active = mon(Species::Mewtwo, if exp_all { 5 } else { 20 }, MoveId::DoubleKick);
    active.hp -= 7; active.set_nickname("ACE");
    let mut party = vec![active];
    if exp_all { let mut other = mon(Species::Bulbasaur, 5, MoveId::Splash); other.set_nickname("BENCH"); party.push(other); }
    let mut target = mon(Species::Snorlax, 100, MoveId::Splash); target.hp = 1;
    let mut b = BattleScreen::from_parties(true, &party, &[target], None);
    if exp_all { b.player_bag.add_item(ItemId::ExpAll, 1).unwrap(); }
    b.link_rng = Some(LinkRng::new([255, 255, 0, 255, 3, 82, 73, 64, 55, 46]));
    b.link_enemy_move_override = Some((MoveId::Splash, 0));
    let state = b.battle_state.as_mut().unwrap();
    state.player.active_mon_mut().speed = if first { 200 } else { 100 };
    state.enemy.active_mon_mut().speed = if first { 100 } else { 200 };
    b.set_presentation_enabled(enabled); b.phase = BattlePhase::PlayerMenu;
    b
}

fn drain(b: &mut BattleScreen, exp_all: bool, restore: bool) -> Vec<String> {
    let mut expected = if exp_all { (5, 19, 26) } else { (20, 69, 76) };
    let mut errors = Vec::new(); let mut previous = None; let mut growth = Vec::new();
    let mut restored = false; let mut pages = Vec::new();
    for frame in 0..8000 {
        if b.current_message != previous {
            if let Some(text) = &b.current_message {
                let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if text == "ACE grew to level 21!" { expected = (21, 73, 80); growth.push(21); }
                if text == "ACE grew to level 10!" { expected = (10, 36, 43); growth.push(10); }
                if text == "ACE grew to level 11!" { expected = (11, 39, 46); growth.push(11); }
                if hud(b) != expected { errors.push(format!("{text}: {:?}, expected {expected:?}", hud(b))); }
                pages.push(text.clone());
                // Persist after the gained page is visible, before any growth.
                if restore && !restored && text == (if exp_all { "ACE gained 1100 exp. points!" } else { "ACE gained 2200 exp. points!" }) {
                    let saved = BattleSnapshot::capture(b);
                    saved.restore_into(b); restored = true;
                }
            }
            previous = b.current_message.clone();
        }
        if !matches!(b.phase, BattlePhase::ShowingText { .. }) {
            assert_eq!(growth, if exp_all { vec![10, 11] } else { vec![21] }, "{pages:?}");
            if hud(b) != expected { errors.push(format!("After last page: {:?}, expected {expected:?}", hud(b))); }
            if exp_all { assert!(pages.iter().any(|p| p.starts_with("BENCH grew to level")), "{pages:?}"); }
            assert_eq!(restored, restore);
            return errors;
        }
        if b.presentation.waiting { b.complete_move_presentation(); }
        b.update_frame(if frame % 2 == 0 { a() } else { BattleInput::none() });
    }
    panic!("presentation did not finish: {:?}", b.phase);
}

#[test]
fn level_hud_changes_only_at_growth_notice_in_either_action_order() {
    let mut errors = Vec::new();
    for first in [false, true] {
        let mut b = fixture(false, first, true); assert_eq!(hud(&b), (20, 69, 76));
        b.update_frame(a()); b.update_frame(a());
        errors.extend(drain(&mut b, false, false));
    }
    assert!(errors.is_empty(), "{} premature/late updates:\n{}", errors.len(), errors.join("\n"));
}

#[test]
fn exp_all_keeps_each_awards_growth_and_bench_notices_separate() {
    let mut errors = Vec::new();
    for first in [false, true] {
        let mut b = fixture(true, first, true); assert_eq!(hud(&b), (5, 19, 26));
        b.update_frame(a()); b.update_frame(a());
        errors.extend(drain(&mut b, true, false));
    }
    assert!(errors.is_empty(), "{} premature/late updates:\n{}", errors.len(), errors.join("\n"));
}

#[test]
fn saved_gained_page_preserves_future_hud_updates() {
    let mut errors = Vec::new();
    for exp_all in [false, true] {
        let mut b = fixture(exp_all, false, true);
        b.update_frame(a()); b.update_frame(a());
        errors.extend(drain(&mut b, exp_all, true));
    }
    assert!(errors.is_empty(), "{} lost updates:\n{}", errors.len(), errors.join("\n"));
}

#[test]
fn consumers_without_presentation_still_receive_resolved_stats_immediately() {
    for exp_all in [false, true] {
        let mut b = fixture(exp_all, false, false);
        b.update_frame(a()); b.update_frame(a());
        let mon = b.battle_state.as_ref().unwrap().player.active_mon();
        assert_eq!((b.player_level, b.player_max_hp), (mon.level, mon.max_hp));
        assert_eq!(hud(&b), if exp_all { (11, 39, 46) } else { (21, 73, 80) });
    }
}

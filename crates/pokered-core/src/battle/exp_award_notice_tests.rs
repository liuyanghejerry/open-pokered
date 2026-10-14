use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

fn screen(traded: bool, exp_all: bool) -> BattleScreen {
    let mut active = create_pokemon_with_moves(
        Species::Bulbasaur, 20, [0x9a, 0x78],
        [MoveId::Cut, MoveId::BodySlam, MoveId::TakeDown, MoveId::DoubleEdge],
    ).unwrap();
    active.is_traded = traded;
    active.ot_id = if traded { 7 } else { 0 };
    let partner = create_pokemon_with_moves(
        Species::Bulbasaur, 5, [0x9a, 0x78],
        [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
    ).unwrap();
    let enemy = create_pokemon_with_moves(
        Species::Weedle, 6, [0x9a, 0x78],
        [MoveId::PoisonSting, MoveId::StringShot, MoveId::None, MoveId::None],
    ).unwrap();
    let mut screen = BattleScreen::from_parties(
        false, &[active, partner], &[enemy], Some(TrainerClass::BugCatcher),
    );
    if exp_all { screen.player_bag.add_item(ItemId::ExpAll, 1).unwrap(); }
    screen.battle_state.as_mut().unwrap().enemy.party[0].hp = 0;
    screen
}

#[test]
fn exp_award_notice_ordinary_and_traded_first_pass_keep_source_wording() {
    for (traded, amount, text) in [
        (false, 66, "BULBASAUR gained 66 exp. points!"),
        (true, 99, "BULBASAUR gained a boosted 99 exp. points!"),
    ] {
        let mut s = screen(traded, false);
        let before = s.battle_state.as_ref().unwrap().player.party[0].total_exp;
        assert_eq!(s.process_exp_gain(), [text]);
        let bs = s.battle_state.as_ref().unwrap();
        assert_eq!(bs.player.party[0].total_exp - before, amount);
        assert_eq!(bs.player.party[1].total_exp, 135);
    }
}

#[test]
fn exp_award_notice_exp_all_marks_second_pass_and_prioritizes_it_over_trade() {
    for (traded, shared, total, wording) in [
        (false, 16, 49, "BULBASAUR gained 33 exp. points!"),
        (true, 24, 73, "BULBASAUR gained a boosted 49 exp. points!"),
    ] {
        let mut s = screen(traded, true);
        let before = s.battle_state.as_ref().unwrap().player.party[0].total_exp;
        let texts = s.process_exp_gain();
        assert_eq!(texts, [
            wording.to_string(),
            format!("BULBASAUR gained with EXP.ALL, {shared} exp. points!"),
            "BULBASAUR gained with EXP.ALL, 16 exp. points!".to_string(),
        ]);
        let bs = s.battle_state.as_ref().unwrap();
        assert_eq!(bs.player.party[0].total_exp - before, total);
        assert_eq!(bs.player.party[1].total_exp, 151);
        assert_eq!(s.player_bag.items(), &[(ItemId::ExpAll, 1)]);
        // These exact amounts/order come from the actual original Weedle6
        // battle: the participant pass precedes the two full-party notices.
    }
}

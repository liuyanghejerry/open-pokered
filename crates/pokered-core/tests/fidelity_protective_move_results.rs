//! Original self-protection guards inspect the user, then announce success or
//! duplicate failure. Prepared FIGHT turns preserve original zero-RNG behavior.
use pokered_core::battle::{state::StatusCondition, BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

const MOVES: [MoveId; 4] = [MoveId::Reflect, MoveId::LightScreen, MoveId::Mist, MoveId::FocusEnergy];
fn flag(mv: MoveId) -> (u8, u8) {
    match mv { MoveId::Reflect => (3,4), MoveId::LightScreen => (3,2), MoveId::Mist => (2,2), MoveId::FocusEnergy => (2,4), _ => unreachable!() }
}
fn set_flag(b: &mut BattleScreen, enemy: bool, mv: MoveId) {
    let bs=b.battle_state.as_mut().unwrap(); let side=if enemy {&mut bs.enemy} else {&mut bs.player};
    let (byte,bit)=flag(mv); if byte==2 {side.set_status2(bit);} else {side.set_status3(bit);}
}
fn has_flag(b: &BattleScreen, enemy: bool, mv: MoveId) -> bool {
    let bs=b.battle_state.as_ref().unwrap(); let side=if enemy {&bs.enemy} else {&bs.player};
    let (byte,bit)=flag(mv); if byte==2 {side.has_status2(bit)} else {side.has_status3(bit)}
}
fn result(mv: MoveId, enemy: bool) -> String {
    let name=if enemy {"Enemy WARD"} else {"WARD"};
    match mv {
        MoveId::Reflect => format!("{name} gained armor!"),
        MoveId::LightScreen => format!("{name}'s protected against special attacks!"),
        MoveId::Mist => format!("{name}'s shrouded in mist!"),
        MoveId::FocusEnergy => format!("{name}'s getting pumped!"),
        _ => unreachable!(),
    }
}
fn fixture(mv: MoveId, enemy: bool, first: bool, haze: bool) -> BattleScreen {
    let make=|species,mv| create_pokemon_with_moves(species,40,[0x9a,0x78], [mv,MoveId::None,MoveId::None,MoveId::None]).unwrap();
    let mut actor=make(Species::Mewtwo,mv);actor.set_nickname("WARD");
    let mut other=make(Species::Snorlax,if haze {MoveId::Haze} else {MoveId::Splash});other.set_nickname("TARGET");
    let (player,foe)=if enemy {(other,actor)} else {(actor,other)};
    let mut b=BattleScreen::from_parties(true,&[player],&[foe],None);
    b.link_rng=Some(LinkRng::new([255,255,0,255,3,82,73,64,55,46]));
    b.link_enemy_move_override=Some((foe.moves[0],0));
    let bs=b.battle_state.as_mut().unwrap();
    let (actor,other)=if enemy {(&mut bs.enemy,&mut bs.player)} else {(&mut bs.player,&mut bs.enemy)};
    actor.active_mon_mut().speed=if first {200} else {100};other.active_mon_mut().speed=if first {100} else {200};
    b.set_presentation_enabled(true);b.phase=BattlePhase::PlayerMenu;b
}
fn turn(b: &mut BattleScreen, mv: MoveId, enemy: bool) -> (String, Vec<bool>) {
    let state=b.battle_state.as_ref().unwrap();let hp=[state.player.active_mon().hp,state.enemy.active_mon().hp];
    let side=if enemy {&state.enemy} else {&state.player};
    let pp=side.active_mon().pp[0];let frozen=side.active_mon().status==StatusCondition::Freeze;
    let draws=b.link_rng.as_ref().unwrap().consumed();
    let a=BattleInput{a:true,..BattleInput::none()};b.update_frame(a);b.update_frame(a);
    let mut out=Vec::new();let mut prev=None;let mut serial=None;let mut missed=Vec::new();
    for frame in 0..5000 {
        if b.current_message!=prev {
            if let Some(m)=&b.current_message {out.push(m.split_whitespace().collect::<Vec<_>>().join(" "));}
            prev=b.current_message.clone();
        }
        if let Some((id,move_id,player))=b.presentation.current_move() {
            if serial!=Some(id) && move_id==mv && player!=enemy {missed.push(b.presentation.current_move_missed());serial=Some(id);}
        }
        if b.phase==BattlePhase::PlayerMenu {
            let state=b.battle_state.as_ref().unwrap();let side=if enemy {&state.enemy} else {&state.player};
            assert_eq!([state.player.active_mon().hp,state.enemy.active_mon().hp],hp);
            // EnemyCanExecuteMove omits the player's DecrementPP call.
            assert_eq!(side.active_mon().pp[0],pp-u8::from(!frozen && !enemy));
            assert_eq!(b.link_rng.as_ref().unwrap().consumed(),draws);
            return (out.join(" "),missed);
        }
        if b.presentation.waiting {b.complete_move_presentation();}
        b.update_frame(if frame%2==0 {a} else {BattleInput::none()});
    }
    panic!("turn did not drain: {:?}",b.phase);
}
fn check(errors: &mut Vec<String>, mv: MoveId, enemy: bool, text: &str, animation: &[bool], success: bool) {
    if text.matches(&result(mv,enemy)).count()!=usize::from(success)
        || text.matches("But, it failed!").count()!=usize::from(!success)
        || animation!=[!success] {
        errors.push(format!("{mv:?}, enemy={enemy}, success={success}, animation={animation:?}: {text}"));
    }
}
fn finish(errors: Vec<String>) {
    assert!(errors.is_empty(), "{} wrong results (first six):\n{}",errors.len(),errors.iter().take(6).cloned().collect::<Vec<_>>().join("\n"));
}
#[test]
fn first_and_repeated_protection_use_correct_results_in_both_orders() {
    let mut errors=Vec::new();
    for mv in MOVES {for enemy in [false,true] {for first in [false,true] {
        let mut b=fixture(mv,enemy,first,false);
        for success in [true,false] {
            let (text,animation)=turn(&mut b,mv,enemy);
            check(&mut errors,mv,enemy,&text,&animation,success);
            assert!(has_flag(&b,enemy,mv));
        }
    }}}
    finish(errors);
}
#[test]
fn opponents_protection_does_not_block_the_users_own_effect() {
    let mut errors=Vec::new();
    for mv in MOVES {for enemy in [false,true] {for self_active in [false,true] {
        let mut b=fixture(mv,enemy,true,false);set_flag(&mut b,!enemy,mv);
        if self_active {set_flag(&mut b,enemy,mv);}
        let (text,animation)=turn(&mut b,mv,enemy);
        check(&mut errors,mv,enemy,&text,&animation,!self_active);
        if !has_flag(&b,enemy,mv) {errors.push(format!("{mv:?}, enemy={enemy}: opponent vetoed user's flag"));}
        assert!(has_flag(&b,!enemy,mv));
    }}}
    finish(errors);
}
#[test]
fn haze_before_or_after_protection_does_not_rewrite_its_action_result() {
    let mut errors=Vec::new();
    for mv in MOVES {for enemy in [false,true] {for first in [false,true] {
        let mut b=fixture(mv,enemy,first,true);
        if !first {set_flag(&mut b,enemy,mv);}
        let (text,animation)=turn(&mut b,mv,enemy);
        check(&mut errors,mv,enemy,&text,&animation,true);
        assert_eq!(has_flag(&b,enemy,mv),!first);
        if let Some(index)=text.find(&result(mv,enemy)) {
            assert_eq!(index<text.find("used HAZE!").unwrap(),first);
        }
    }}}
    finish(errors);
}
#[test]
fn frozen_users_do_not_announce_success_failure_or_play_the_move() {
    for mv in MOVES {for enemy in [false,true] {
        let mut b=fixture(mv,enemy,true,false);
        let bs=b.battle_state.as_mut().unwrap();let side=if enemy {&mut bs.enemy} else {&mut bs.player};
        side.active_mon_mut().status=StatusCondition::Freeze;
        let (text,animation)=turn(&mut b,mv,enemy);
        assert!(!text.contains(&result(mv,enemy)) && !text.contains("But, it failed!"));
        assert!(animation.is_empty());assert!(!has_flag(&b,enemy,mv));
    }}
}

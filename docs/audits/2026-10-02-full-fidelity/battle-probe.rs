//! Temporary audit probe: asserts observed production behavior, not desired ROM behavior.
use dotzuki_engine::battle::{BattleAction, BattlerRef};
use dotzuki_engine::battle::rng::ScriptedRng;
use dotzuki_engine::battle::stack::{StackDriver, TurnEvent};
use pokered_core::battle::{BattleScreen, BattlePhase, BattleInput};
use pokered_core::battle::menu::{MoveMenuState, MoveSlot};
use pokered_core::battle::pokered_rules::{self as rules, PokeredRules};
use pokered_core::battle::state::*;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType, move_data::MoveData};

fn mon(species: Species, level: u8, mv: MoveId) -> Pokemon {
    Pokemon { species, nickname: [0x50;11], level, hp: 400, max_hp: 400,
        attack:100, defense:100, speed:100, special:100,
        type1:PokemonType::Normal, type2:PokemonType::Normal,
        moves:[mv,MoveId::None,MoveId::None,MoveId::None], pp:[20,0,0,0],
        pp_ups:[0;4], status:StatusCondition::None, dv_bytes:[0xff;2],
        stat_exp:[0;5], total_exp:0, is_traded:false, ot_id:0, ot_name:[0x50;11] }
}
fn state(mv: MoveId, level: u8) -> BattleState {
    let mut p=mon(Species::Pikachu,level,mv); p.speed=200;
    new_battle_state(BattleType::Wild, vec![p], vec![mon(Species::Snorlax,50,MoveId::Splash)])
}
fn turn(bs: &mut BattleState, mv: MoveId, bytes: Vec<u8>) -> Vec<TurnEvent<PokeredRules>> {
    rules::install_canonical(); rules::clear_current_moves(); rules::clear_levels();
    rules::set_current_move(BattlerRef::PLAYER,*MoveData::get(mv).unwrap());
    rules::set_current_move(BattlerRef::OPPONENT,*MoveData::get(MoveId::Splash).unwrap());
    rules::set_last_move_live(BattlerRef::OPPONENT, bs.enemy.last_move_used);
    let (mut es,mut fx)=rules::runtime::engine_state_from_legacy(bs);
    let (_,log)=StackDriver::execute_turn_logged(&PokeredRules,&mut es,&mut fx,
        [BattleAction::Fight{move_:mv},BattleAction::Nothing],&mut ScriptedRng::new(bytes));
    rules::runtime::apply_engine_to_legacy(bs,&es,&fx);
    log.events
}
fn screen_turn(screen:&mut BattleScreen, mv:MoveId) {
    screen.rng=rules::runtime::StdBattleRng::from_seed(42);
    screen.phase=BattlePhase::MoveSelect;
    screen.move_menu=Some(MoveMenuState::new(vec![MoveSlot{move_id:mv,current_pp:20,max_pp:20,is_disabled:false}]));
    screen.update_frame(BattleInput{a:true,..BattleInput::none()});
}

#[test]
fn production_audit_observations() {
    let mut bs=state(MoveId::Tackle,50);
    bs.player.set_status2(status2::USING_X_ACCURACY);
    let log=turn(&mut bs,MoveId::Tackle,vec![255,255,255,255]);
    assert!(log.iter().any(|e|matches!(e,TurnEvent::Missed{actor} if *actor==BattlerRef::PLAYER)));
    println!("X_ACCURACY: missed at accuracy byte 255; ROM would hit");

    let mut bs=state(MoveId::Tackle,50);
    bs.player.set_status1(status1::CONFUSED); bs.player.confused_turns_left=3;
    turn(&mut bs,MoveId::Tackle,vec![0,255,255,255]);
    assert_eq!(bs.player.active_mon().hp,372);
    println!("CONFUSION: 50 level / 100 attack / 100 defense self hit = 28; ROM = 19");

    for mv in [MoveId::SeismicToss,MoveId::NightShade] {
        let mut bs=state(mv,10);
        turn(&mut bs,mv,vec![0,255,255,255]);
        assert_eq!(bs.enemy.active_mon().hp,350);
        println!("FIXED_LEVEL: {:?} at level 10 damage = 50; ROM = 10",mv);
    }

    let mut bs=state(MoveId::SwordsDance,50);
    bs.enemy.set_status1(status1::CHARGING_UP | status1::INVULNERABLE);
    bs.enemy.selected_move=MoveId::Dig;
    turn(&mut bs,MoveId::SwordsDance,vec![0,255,255]);
    assert_eq!(bs.player.stat_stages.attack,0);
    println!("SELF_BOOST: Swords Dance while foe digs failed; ROM attack stage = +2");

    let mut bs=state(MoveId::Disable,50);
    turn(&mut bs,MoveId::Disable,vec![0,0,0,0]);
    assert_eq!(bs.enemy.disabled_move,0);
    println!("DISABLE: target has known move but has never acted => no disabled move; ROM chooses known move");

    let mut bs=state(MoveId::Recover,50);
    bs.player.active_mon_mut().hp=145; // max-current = 255, original recovery bug
    turn(&mut bs,MoveId::Recover,vec![0,255,255]);
    assert_eq!(bs.player.active_mon().hp,345);
    println!("RECOVER: max HP 400 / HP 145 heals to 345; ROM fails at HP deficit 255");

    let mut p=mon(Species::Pikachu,10,MoveId::Tackle); p.status=StatusCondition::Sleep(3);
    let e=mon(Species::Snorlax,50,MoveId::Splash);
    let mut screen=BattleScreen::from_parties(true,&[p],&[e],None);
    screen_turn(&mut screen,MoveId::Tackle);
    assert_eq!(screen.battle_state.as_ref().unwrap().player.active_mon().pp[0],19);
    println!("PP_SLEEP: slept through turn, Tackle PP 20 -> 19; ROM retains 20");

    let p=mon(Species::Pikachu,50,MoveId::Mimic);
    let mut e=mon(Species::Snorlax,50,MoveId::Splash);
    e.moves=[MoveId::Splash,MoveId::Growl,MoveId::None,MoveId::None];
    e.pp=[20,20,0,0];
    let mut screen=BattleScreen::from_parties(true,&[p],&[e],None);
    screen.battle_state.as_mut().unwrap().enemy.last_move_used=MoveId::Growl;
    screen_turn(&mut screen,MoveId::Mimic);
    let p=screen.battle_state.as_ref().unwrap().player.active_mon();
    assert_eq!(p.moves[0],MoveId::Growl); assert_eq!(p.pp[0],5);
    println!("MIMIC: copies prior Growl and sets PP=5; ROM player chooses foe move and retains Mimic remaining PP");

    let p=mon(Species::Ditto,50,MoveId::Transform);
    let e=mon(Species::Snorlax,50,MoveId::Splash);
    let mut screen=BattleScreen::from_parties(true,&[p],&[e],None);
    screen_turn(&mut screen,MoveId::Transform);
    screen.battle_state.as_mut().unwrap().player.reset_volatile_status();
    assert_eq!(screen.battle_state.as_ref().unwrap().player.active_mon().species,Species::Snorlax);
    let mut save=pokered_core::save::SaveData::new();
    let mut ow=pokered_core::overworld::screen::OverworldScreen::new(
        pokered_data::maps::MapId::Route1,None,pokered_data::impl_traits::PokemonRedData);
    pokered_core::battle::settlement::settle_battle_into_save(&mut screen,&mut save,&mut ow);
    assert_eq!(save.party.get(0).unwrap().species,Species::Snorlax);
    println!("TRANSFORM_WRITEBACK: Ditto -> Snorlax remains after volatile reset AND save writeback; ROM remains Ditto");

    let mut bs=state(MoveId::Tackle,50);
    bs.player.party.push(mon(Species::Charmander,50,MoveId::Scratch));
    bs.party_gain_exp_flags[0]=true; bs.party_gain_exp_flags[1]=true;
    pokered_core::battle::experience::gain::gain_experience(&mut bs,Species::Snorlax,50,false);
    assert_eq!(bs.party_gain_exp_flags[..2],[true,true]);
    println!("EXP_FLAGS: two participants remain flagged after KO; ROM keeps only active mon flagged");
}

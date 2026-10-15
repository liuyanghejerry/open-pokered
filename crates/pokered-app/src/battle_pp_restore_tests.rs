use super::*;
use pokered_core::battle::{BattleInput, BattlePhase, BattleTextBackdrop};
use pokered_core::pokemon::stats::create_pokemon;
use pokered_data::{items::ItemId, species::Species};

fn on_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(16*1024*1024).spawn(test).unwrap().join().unwrap();
}
fn game() -> PokemonGame {
    let mut g=PokemonGame::new(GameVersion::Red); g.audio=None;
    let lead=create_pokemon(Species::Bulbasaur,20,[0x9a,0x78]).unwrap();
    let mut bench=create_pokemon(Species::Pikachu,12,[0x9a,0x78]).unwrap(); bench.pp[0]=0;
    g.battle=BattleScreen::from_parties(true,&[lead,bench],
        &[create_pokemon(Species::Rattata,5,[0x9a,0x78]).unwrap()],None);
    g.battle.player_bag.add_item(ItemId::Ether,2).unwrap();
    g.battle.phase=BattlePhase::PlayerMenu;
    for input in [BattleInput {down:true,..BattleInput::none()},
        BattleInput {a:true,..BattleInput::none()},BattleInput {a:true,..BattleInput::none()},
        BattleInput {down:true,..BattleInput::none()},BattleInput {a:true,..BattleInput::none()},
        BattleInput {a:true,..BattleInput::none()}] { g.battle.update_frame(input); }
    g.state.screen=GameScreen::Battle;g
}
#[test]
fn pp_result_app_snapshot_keeps_battle_party_and_pending_item_until_ack() {
    on_stack(|| {
        let mut g=game();let field_party=serde_json::to_value(&g.save_data.party).unwrap();
        assert!(matches!(g.battle.phase,BattlePhase::ShowingText {
            backdrop:BattleTextBackdrop::PpRestoreParty {party_index:1,move_row:Some(0),consume_on_ack:Some(ItemId::Ether)},..}));
        let saved=g.agent_save_state().unwrap();
        for _ in 0..120 {g.update(&InputState::new());}
        g.agent_restore_state(&saved).unwrap();
        assert_eq!(g.battle.player_bag.items(),&[(ItemId::Ether,2)]);
        assert_eq!(g.battle.party_cursor,1);
        for _ in 0..120 {g.update(&InputState::new());}
        assert_eq!(serde_json::to_value(&g.save_data.party).unwrap(),field_party);
        let mut b=InputState::new();b.press(GbButton::B);g.update(&b);
        assert_eq!(g.battle.phase,BattlePhase::EnemyFreeTurnAfterItem);
        assert_eq!(g.battle.player_bag.items(),&[(ItemId::Ether,1)]);
    });
}
#[test]
fn pp_result_real_soft_reset_continue_and_new_battle_drop_the_old_continuation() {
    on_stack(|| {
        let mut g=game();
        let mut replacement=pokered_core::save::SaveData::new();
        replacement.party.add(create_pokemon(Species::Charmander,8,[0x9a,0x78]).unwrap()).unwrap();
        replacement.game_data.bag.add_item(ItemId::Ether,2).unwrap();
        let path=std::env::temp_dir().join(format!("pokered-pp-result-reset-{}.sav",std::process::id()));
        std::fs::write(&path,pokered_core::save::sram_export::export_sram(&replacement)).unwrap();
        g.save_path=Some(path.clone());
        let mut reset=InputState::new();
        for button in [GbButton::A,GbButton::B,GbButton::Start,GbButton::Select] {reset.press(button);}
        for _ in 0..16 {g.update(&reset);}
        std::fs::remove_file(path).unwrap();
        assert_eq!(g.state.screen,GameScreen::TitleScreen);
        g.handle_transition(GameScreen::MainMenu);
        g.main_menu.last_choice=Some(pokered_core::game_state::MainMenuChoice::Continue);
        g.handle_transition(GameScreen::Overworld);
        for _ in 0..120 {g.update(&InputState::new());}
        assert_eq!(g.save_data.party.get(0).unwrap().species,Species::Charmander);
        g.start_wild_battle(Species::Rattata,5);
        assert!(matches!(g.battle.phase,BattlePhase::Intro {..}));
        assert_eq!(g.battle.player_bag.items(),&[(ItemId::Ether,2)]);
        assert!(g.battle.current_message.is_none());
    });
}

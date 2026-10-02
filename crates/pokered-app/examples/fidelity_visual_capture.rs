//! Fixed-state visual capture fixture, compatible with audit base 72ff719.
//! Run this identical source against base and fix libraries; argv = outdir prefix.
use pokered_app::PokemonGame;
use pokered_core::{game_state::{GameScreen,Lang}, naming_screen::{NamingScreenState,NamingScreenType}, oak_speech::OakSpeechPhase, party_screen::PartyScreenState, pokemon::stats::create_pokemon, evolution_screen::{EvolutionScreenState,PendingEvolution,EvolutionInput}, trade::TradeAnim, pokedex_screen::PokedexScreenState, pokemon::pokedex::Pokedex};
use pokered_data::{species::Species, wild_data::GameVersion};
use pokered_renderer::{FrameBuffer,Rgba};
use dotzuki_engine::render_config::RenderConfig;
use std::path::Path;
fn game() -> PokemonGame {
    PokemonGame::new_with_options(GameVersion::Red,None,None,None,true,None,false,true,None)
}
fn save(game: &mut PokemonGame, out: &Path, prefix: &str, name: &str) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
    game.draw(&mut fb);
    fb.save_png(&out.join(format!("{prefix}-{name}.png"))).unwrap();
}
fn battle_fixtures(out: &Path, prefix: &str) {
    use pokered_core::battle::{BattleScreen, BattlePhase, BattleInput};
    use pokered_core::battle::menu::{MoveMenuState, MoveSlot};
    use pokered_core::battle::pokered_rules::runtime::StdBattleRng;
    use pokered_core::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::moves::MoveId;
    let mk = |species, level, moves| create_pokemon_with_moves(species, level, [0xff;2], moves).unwrap();
    let mut g = game();
    let mut player = mk(Species::Pikachu, 50, [MoveId::Mimic,MoveId::None,MoveId::None,MoveId::None]);
    player.pp[0]=20;
    let enemy = mk(Species::Snorlax, 50, [MoveId::Splash,MoveId::Growl,MoveId::None,MoveId::None]);
    g.battle = BattleScreen::from_parties(true, &[player], &[enemy], None);
    g.battle.battle_state.as_mut().unwrap().enemy.last_move_used=MoveId::Growl;
    g.battle.rng=StdBattleRng::from_seed(42);
    g.battle.phase=BattlePhase::MoveSelect;
    g.battle.move_menu=Some(MoveMenuState::new(vec![MoveSlot { move_id:MoveId::Mimic,current_pp:20,max_pp:20,is_disabled:false }]));
    g.battle.update_frame(BattleInput { a:true,..BattleInput::none() });
    let copied=g.battle.battle_state.as_ref().unwrap().player.active_mon();
    println!("mimic phase {:?}, move {:?}, PP {}", g.battle.phase,copied.moves[0],copied.pp[0]);
    if copied.moves[0]!=MoveId::Mimic {
        let slot=MoveSlot { move_id:copied.moves[0],current_pp:copied.pp[0],max_pp:pokered_data::move_data::MoveData::get(copied.moves[0]).unwrap().pp,is_disabled:false };
        g.battle.phase=BattlePhase::MoveSelect;
        g.battle.current_message=None;
        g.battle.move_menu=Some(MoveMenuState::new(vec![slot]));
    }
    g.state.screen=GameScreen::Battle;
    save(&mut g,out,prefix,"battle-mimic-menu");

    let mut g=game();
    let player=mk(Species::Pikachu, 50, [MoveId::Tackle,MoveId::None,MoveId::None,MoveId::None]);
    let bench=mk(Species::Charmander, 10, [MoveId::Scratch,MoveId::None,MoveId::None,MoveId::None]);
    let mut enemy=mk(Species::Snorlax, 30, [MoveId::Splash,MoveId::None,MoveId::None,MoveId::None]); enemy.hp=1;
    g.battle=BattleScreen::from_parties(true,&[player,bench],&[enemy],None);
    g.battle.set_presentation_enabled(false);
    g.battle.battle_state.as_mut().unwrap().party_gain_exp_flags[..2].fill(true);
    g.battle.rng=StdBattleRng::from_seed(42);
    g.battle.phase=BattlePhase::MoveSelect;
    g.battle.move_menu=Some(MoveMenuState::new(vec![MoveSlot {move_id:MoveId::Tackle,current_pp:35,max_pp:35,is_disabled:false}]));
    g.battle.update_frame(BattleInput {a:true,..BattleInput::none()});
    g.state.screen=GameScreen::Battle;
    for _ in 0..5000 {
        if g.battle.current_message.as_deref().is_some_and(|text| text.contains("gained")) {
            println!("EXP message {:?}",g.battle.current_message);
            save(&mut g,out,prefix,"battle-exp-share");
            break;
        }
        g.battle.update_frame(BattleInput {a:true,..BattleInput::none()});
    }
    // TrainerAI must replace a forced Fly strike, retaining its charge flags.
    let mut g=game();
    let player=mk(Species::Pikachu,50,[MoveId::Splash,MoveId::None,MoveId::None,MoveId::None]);
    let mut enemy=mk(Species::Snorlax,50,[MoveId::Fly,MoveId::None,MoveId::None,MoveId::None]);
    enemy.status=pokered_core::battle::state::StatusCondition::Poison;
    g.battle=BattleScreen::from_parties(false,&[player],&[enemy],Some(pokered_data::trainer_data::TrainerClass::Brock));
    g.battle.trainer_name=Some("BROCK".to_string());
    g.battle.set_presentation_enabled(false);
    {
        let bs=g.battle.battle_state.as_mut().unwrap();
        bs.enemy.selected_move=MoveId::Fly;
        bs.enemy.set_status1(pokered_core::battle::state::status1::CHARGING_UP | pokered_core::battle::state::status1::INVULNERABLE);
    }
    g.battle.rng=StdBattleRng::from_seed(42);
    g.battle.phase=BattlePhase::MoveSelect;
    g.battle.move_menu=Some(MoveMenuState::new(vec![MoveSlot {move_id:MoveId::Splash,current_pp:40,max_pp:40,is_disabled:false}]));
    g.battle.update_frame(BattleInput {a:true,..BattleInput::none()});
    g.state.screen=GameScreen::Battle;
    for _ in 0..5000 {
        let action_page = match &g.battle.phase {
            BattlePhase::ShowingText { messages, current, .. } => messages.get(*current).is_some_and(|text| {
                let normalized=text.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_uppercase();
                normalized.contains("FULL HEAL") || normalized.contains("FLY")
            }),
            _ => false,
        };
        if action_page {
            // Neutral input lets the last page become visible; continuous A
            // would set then clear current_message in the same update.
            for _ in 0..20 { g.battle.update_frame(BattleInput::none()); }
            let bs=g.battle.battle_state.as_ref().unwrap();
            println!("AI forced Fly {:?}; playerHP={} enemyStatus={:?} charge={} AIcount={}",g.battle.current_message,bs.player.active_mon().hp,bs.enemy.active_mon().status,bs.enemy.has_status1(pokered_core::battle::state::status1::CHARGING_UP),g.battle.enemy_ai_count);
            save(&mut g,out,prefix,"battle-ai-forced-fly");
            break;
        }
        g.battle.update_frame(BattleInput {a:true,..BattleInput::none()});
    }
}

fn main() {
    let args:Vec<String>=std::env::args().collect(); let out=Path::new(&args[1]); let prefix=&args[2];
    std::fs::create_dir_all(out).unwrap();
    battle_fixtures(out,prefix);
    // Same phase-relative frame for custom credits tiles and scrolling cadence.
    use pokered_core::credits::{CreditsState, CreditsPhase, CreditsInput};
    for (index, phase, frames, name) in [
        (0, CreditsPhase::MonScroll, 7, "credits-scroll-7"),
        (34, CreditsPhase::Hold, 0, "credits-copyright-0"),
        (34, CreditsPhase::Hold, 5, "credits-copyright-5"),
        (34, CreditsPhase::Hold, 10, "credits-copyright-10"),
        (34, CreditsPhase::Hold, 15, "credits-copyright-15"),
        (35, CreditsPhase::TheEnd, 16, "credits-the-end-16"),
        (35, CreditsPhase::TheEnd, 21, "credits-the-end-21"),
        (35, CreditsPhase::TheEnd, 26, "credits-the-end-26"),
        (35, CreditsPhase::TheEnd, 31, "credits-the-end-31"),
    ] {
        let mut roll = CreditsState::new(GameVersion::Red);
        for _ in 0..10000 {
            if roll.screen_index() == index && roll.phase() == phase { break; }
            roll.update_frame(CreditsInput::none());
        }
        assert_eq!((roll.screen_index(),roll.phase()),(index,phase));
        for _ in 0..frames { roll.update_frame(CreditsInput::none()); }
        println!("{name}: screen {}, {:?}, scroll step {}, palette step {}",roll.screen_index(),roll.phase(),roll.mon_scroll_step(),roll.fade_step());
        let mut g=game();g.credits=Some(roll);save(&mut g,out,prefix,name);
    }
    for (facing,label) in [
        (pokered_core::overworld::Direction::Down,"down"),
        (pokered_core::overworld::Direction::Left,"left"),
        (pokered_core::overworld::Direction::Right,"right"),
    ] {
        let mut g=game();g.overworld.state.player.facing=facing;
        save(&mut g,out,prefix,&format!("overworld-obj-{label}-0"));
    }
    // Identical state and frame: tight front-picture padding, mirror and GB palette.
    let mut g=game();
    let mut evo=EvolutionScreenState::new(vec![PendingEvolution{party_index:0,from:Species::Bulbasaur,to:Species::Ivysaur,name:"BULBASAUR".into(),force:false}],None,false);
    for _ in 0..190 { evo.tick(EvolutionInput::none()); }
    println!("evolution-190 {:?}",evo.phase()); g.evolution_anim=Some(evo);save(&mut g,out,prefix,"evolution-190");
    let mut g=game();g.state.screen=GameScreen::OakSpeech;g.oak_speech.phase=OakSpeechPhase::ShowNidorino{page_index:0,char_index:0,waiting_for_input:false};g.oak_speech.phase_frame=65;save(&mut g,out,prefix,"oak-nidorino-65");
    for (kind,frame) in [(NamingScreenType::Player,5),(NamingScreenType::Pokemon,17)] {
        let mut g=game();g.state.screen=GameScreen::OakSpeech;g.oak_speech.phase=OakSpeechPhase::PlayerNaming;
        let mut naming=NamingScreenState::new(kind);
        for _ in 0..frame { naming.update_frame(pokered_core::naming_screen::NamingInput::none(),false); }
        #[cfg(fidelity_after)] { if kind==NamingScreenType::Pokemon { naming.species=Some(Species::Lapras); } }
        g.oak_speech.naming_screen=Some(naming);save(&mut g,out,prefix,if kind==NamingScreenType::Player {"naming-player-5"} else {"naming-lapras-17"});
    }
    for species in [Species::Raichu,Species::Gengar] {
        let mut g=game();let mut dex=Pokedex::new();dex.set_seen(species);dex.set_owned(species);g.pokedex_screen=PokedexScreenState::new_entry(dex,species,GameVersion::Red);g.state.screen=GameScreen::Pokedex;save(&mut g,out,prefix,&format!("dex-{:?}-0",species));
    }
    let mut g=game();let mut trade=TradeAnim::new(Species::Nidorino,Species::Raichu,"RED".into(),false);for _ in 0..40 {trade.tick();}g.trade_anim=Some(trade);save(&mut g,out,prefix,"trade-40");
    // One selected mon per capture: all ten icon kinds and their second frames.
    for species in [Species::Rhydon,Species::Clefairy,Species::Pidgey,Species::Lapras,Species::Voltorb,Species::Omanyte,Species::Caterpie,Species::Bulbasaur,Species::Ekans,Species::Pikachu] {
        let mut g=game();let mut mon=create_pokemon(species,20,[0x9a,0x78]).unwrap();mon.hp=mon.max_hp;g.party_screen=PartyScreenState::new(vec![mon]);g.state.screen=GameScreen::PartyScreen;g.frame_count=6;save(&mut g,out,prefix,&format!("party-{:?}-green-6",species));
    }
    for (hp,frame,label) in [(20,17,"yellow"),(5,33,"red")] {
        let mut g=game();let mut mon=create_pokemon(Species::Pidgey,20,[0x9a,0x78]).unwrap();mon.max_hp=48;mon.hp=hp;g.party_screen=PartyScreenState::new(vec![mon]);g.state.screen=GameScreen::PartyScreen;g.frame_count=frame;save(&mut g,out,prefix,&format!("party-Pidgey-{label}-{frame}"));
    }
    let mut g=game();g.state.screen=GameScreen::Elevator;g.elevator_screen=Some(pokered_core::elevator_screen::ElevatorScreen::new(vec!["1F".into(),"2F".into(),"3F".into(),"4F".into(),"5F".into()]));save(&mut g,out,prefix,"elevator-floor-0");
    // Blue version uses the original special title tiles, independently of CLI default.
    let mut g=game();g.state.config.version=GameVersion::Blue;g.title_screen=pokered_core::title_screen::TitleScreenState::new(GameVersion::Blue);g.title_screen.skip_to_waiting_for_input();g.state.screen=GameScreen::TitleScreen;save(&mut g,out,prefix,"title-blue-0");
    let mut g=game();g.state.screen=GameScreen::Diploma;g.state.config.language=Lang::Zh;save(&mut g,out,prefix,"diploma-zh-0");
}

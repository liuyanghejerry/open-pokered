//! Identical fixed-state capture fixture for master cb131bb and the split presentation branch.
//! Run this identical source against base and fix libraries; argv = outdir prefix.
use pokered_app::PokemonGame;
use pokered_core::{game_state::{GameScreen,Lang}, naming_screen::{NamingScreenState,NamingScreenType}, oak_speech::OakSpeechPhase, party_screen::PartyScreenState, pokemon::stats::create_pokemon, evolution_screen::{EvolutionScreenState,PendingEvolution,EvolutionInput}, trade::TradeAnim, pokedex_screen::PokedexScreenState, pokemon::pokedex::Pokedex};
use pokered_data::{species::Species, wild_data::GameVersion};
use pokered_renderer::{FrameBuffer,Rgba};
use dotzuki_engine::render_config::RenderConfig;
use std::path::Path;
fn game() -> PokemonGame {
    PokemonGame::new_with_options(GameVersion::Red,None,None,None,true,None,false,true,
        #[cfg(feature = "debug-server")] None)
}
fn save(game: &mut PokemonGame, out: &Path, prefix: &str, name: &str) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
    game.draw(&mut fb);
    fb.save_png(&out.join(format!("{prefix}-{name}.png"))).unwrap();
}
fn main() {
    let args:Vec<String>=std::env::args().collect(); let out=Path::new(&args[1]); let prefix=&args[2];
    std::fs::create_dir_all(out).unwrap();
    let mut g=game();g.state.screen=GameScreen::CopyrightSplash;save(&mut g,out,prefix,"copyright-0");
    let mut g=game();g.state.screen=GameScreen::GameFreakSplash;g.gamefreak_splash=pokered_core::gamefreak_splash::GameFreakSplashState::new();
    for _ in 0..244 {g.gamefreak_splash.update_frame(pokered_core::gamefreak_splash::SplashInput::none());}
    save(&mut g,out,prefix,"gamefreak-244");
    for frames in [121,441] {
        let mut g=game();g.state.screen=GameScreen::TitleScreen;g.title_screen=pokered_core::title_screen::TitleScreenState::new(GameVersion::Red);
        for _ in 0..frames {g.title_screen.update_frame(false);}
        save(&mut g,out,prefix,&format!("title-{frames}"));
    }
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
        g.oak_speech.naming_screen=Some(naming);save(&mut g,out,prefix,if kind==NamingScreenType::Player {"naming-player-5"} else {"naming-pokemon-17"});
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
    let mut g=game();g.state.screen=GameScreen::TrainerCard;save(&mut g,out,prefix,"trainer-card");
    let mut g=game();g.state.screen=GameScreen::Diploma;g.state.config.language=Lang::Zh;save(&mut g,out,prefix,"diploma-zh-0");

    use pokered_core::hof_ceremony::{HofCeremonyState,HofEntry,HofPlayerStats};
    let mut g=game();
    let mut hof=HofCeremonyState::new(vec![HofEntry {species:Species::Pikachu,level:50,nickname:"PIKACHU".into()}],HofPlayerStats {name:"RED".into(),play_time_hours:25,play_time_minutes:30,money:99999,dex_seen:50,dex_owned:50,rating:"Good job!"});
    for _ in 0..488 {hof.update_frame();}
    g.hof_ceremony=Some(hof);save(&mut g,out,prefix,"hof-488");
    use pokered_data::items::ItemId;
    let mut g=game();
    let player=create_pokemon(Species::Charmander,20,[0x9a,0x78]).unwrap();
    let enemy=create_pokemon(Species::Bulbasaur,20,[0x9a,0x78]).unwrap();
    g.battle=pokered_core::battle::BattleScreen::from_parties(true,&[player],&[enemy],None);
    g.battle.phase=pokered_core::battle::BattlePhase::BagSelect;
    g.battle.bag_menu=Some(pokered_core::battle::menu::BagMenuState::new(vec![(ItemId::SuperPotion,99),(ItemId::FullRestore,99),(ItemId::MaxRepel,99),(ItemId::PokeBall,99)]));
    g.state.screen=GameScreen::Battle;save(&mut g,out,prefix,"battle-bag-en-0");
    use pokered_core::items::shop::{MartState,ShopInventory};
    use dotzuki_app::InputState;
    use dotzuki_renderer::input::GbButton;
    for language in [Lang::En,Lang::Zh] {
        let mut g=game();pokered_app::tools::apply_lang(&mut g,language);
        g.save_data.game_data.player_money=999999;
        for id in [ItemId::SuperPotion,ItemId::FullRestore,ItemId::MaxRepel,ItemId::PokeBall] {g.save_data.game_data.bag.add_item(id,99).unwrap();}
        for (screen,name) in [(GameScreen::Bag,"bag"),(GameScreen::TownMap,"town-map")] {g.handle_transition(screen);save(&mut g,out,prefix,&format!("{name}-{:?}",language));}
        g.handle_transition(GameScreen::Shop(MartState::new(ShopInventory::new(vec![ItemId::SuperPotion,ItemId::FullRestore,ItemId::MaxRepel,ItemId::PokeBall]))));
        let mut input=InputState::new();input.press(GbButton::A);g.update(&input);
        save(&mut g,out,prefix,&format!("mart-buy-{:?}",language));
    }
}

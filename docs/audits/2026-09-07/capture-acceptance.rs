// Shared deterministic capture fixture, run unchanged on master and PR HEAD.
use dotzuki_engine::render_config::RenderConfig;
use pokered_core::{game_state::Lang, overworld::{OverworldInput, OverworldScreen},
    hof_ceremony::{HofCeremonyState,HofEntry,HofPlayerStats}, credits::{CreditsState,CreditsInput},
    elevator_screen::ElevatorScreen, bag_screen::{BagScreenState,BagScreenInput},
    party_screen::PartyScreenState, start_menu::{StartMenuState,SafariZoneInfo}};
use pokered_data::{impl_traits::PokemonRedData,maps::MapId,species::Species,items::ItemId,moves::MoveId,wild_data::GameVersion};
use pokered_renderer::{FrameBuffer,Rgba,resource::{AssetRoot,ResourceManager}};
fn main() {
    let out=std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&out).unwrap();
    let mut res=Some(ResourceManager::new(AssetRoot::auto_detect().expect("gfx")));
    let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
    let idle=OverworldInput::new(false,false,false,false,false,false,false,false);
    for (name,map,x,y,flags) in [
        ("gates",MapId::CinnabarGym,17,7,vec![]),
        ("quiz-solved",MapId::CinnabarGym,17,7,vec!["EVENT_CINNABAR_GYM_GATE1_UNLOCKED"]),
        ("zapdos-reload",MapId::PowerPlant,4,10,vec!["EVENT_BEAT_ZAPDOS"]),
        ("articuno-reload",MapId::SeafoamIslandsB4F,6,2,vec!["EVENT_BEAT_ARTICUNO"]),
        ("moltres-reload",MapId::VictoryRoad2F,11,6,vec!["EVENT_BEAT_MOLTRES"]),
        ("boulder-before-drop",MapId::VictoryRoad2F,22,16,vec![]),
        ("boulder-after-drop",MapId::VictoryRoad3F,21,15,vec!["EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2"]),
        ("bruno-door",MapId::BrunosRoom,4,2,vec!["EVENT_AUTOWALKED_INTO_BRUNOS_ROOM"]),
    ] {
        let mut screen=OverworldScreen::new(map,None,PokemonRedData);
        screen.state.player.x=x; screen.state.player.y=y;
        for flag in flags { screen.set_flag_live(flag,true); }
        screen.run_on_load();
        for _ in 0..120 { screen.update_frame(idle); }
        capture_overworld(&mut screen,&mut res,&mut fb);
        fb.save_png(&out.join(format!("{name}.png"))).unwrap();
    }
    let mut empty=OverworldScreen::new(MapId::RedsHouse2F,None,PokemonRedData);
    empty.state.player.x=4; empty.state.player.y=3;
    let mut save=pokered_core::save::SaveData::new();
    pokered_core::overworld::poison::apply_out_of_battle_poison_damage(&mut save,&mut empty);
    for _ in 0..120 { empty.update_frame(idle); }
    capture_overworld(&mut empty,&mut res,&mut fb); fb.save_png(&out.join("empty-party.png")).unwrap();
    let mut cut=OverworldScreen::new(MapId::VermilionCity,None,PokemonRedData);
    cut.state.player.x=15; cut.state.player.y=19;
    cut.run_on_load(); for _ in 0..120 { cut.update_frame(idle); }
    cut.state.player.facing=pokered_core::overworld::Direction::Up;
    let cutter=pokered_core::pokemon::stats::create_pokemon(Species::Venusaur,50,[255,255]).unwrap();
    cut.use_field_move(MoveId::Cut,&cutter,255,MapId::PalletTown);
    for _ in 0..120 { cut.update_frame(idle); }
    cut.pending_dialogue=None;
    capture_overworld(&mut cut,&mut res,&mut fb); fb.save_png(&out.join("cut-tree.png")).unwrap();
    let mut bill=OverworldScreen::new(MapId::BillsHouse,None,PokemonRedData);
    bill.state.player.x=6; bill.state.player.y=6;
    bill.run_on_load(); for _ in 0..60 { bill.update_frame(idle); }
    bill.state.player.facing=pokered_core::overworld::Direction::Up;
    bill.seed_script_query_state(0,&[],0,0,0,0,&[],0,0,0);
    bill.update_frame(OverworldInput::new(false,false,false,false,true,false,false,false));
    for frame in 0..400 {
        if let Some(d)=bill.pending_dialogue.as_mut() { d.skip_to_full_page(); }
        bill.update_frame(OverworldInput::new(false,false,false,false,frame%2==1,false,false,false));
    }
    capture_overworld(&mut bill,&mut res,&mut fb); fb.save_png(&out.join("bill-machine.png")).unwrap();
    render::draw_filter_bag(&ElevatorScreen::new(vec!["FRESH_WATER".into(),"SODA_POP".into(),"LEMONADE".into()]),&mut fb,Lang::En);
    fb.save_png(&out.join("drinks.png")).unwrap();
    let mut bag=BagScreenState::new([ItemId::Potion,ItemId::SuperPotion,ItemId::HyperPotion,ItemId::MaxPotion,ItemId::FullRestore,ItemId::Antidote,ItemId::ParlyzHeal,ItemId::Awakening,ItemId::BurnHeal,ItemId::IceHeal,ItemId::FullHeal,ItemId::Revive,ItemId::MaxRevive,ItemId::Ether,ItemId::MaxEther,ItemId::Elixer,ItemId::MaxElixer,ItemId::Hm01,ItemId::Hm03,ItemId::Hm04].into_iter().map(|id|(id,1)).collect());
    for _ in 0..16 { bag.update_frame(BagScreenInput{down:true,..BagScreenInput::none()}); }
    render::draw_bag(&bag,&mut fb,Lang::En); fb.save_png(&out.join("bag.png")).unwrap();
    let mon=pokered_core::pokemon::stats::create_pokemon_with_moves(Species::Venusaur,50,[255,255],[MoveId::LeechSeed,MoveId::Poisonpowder,MoveId::SleepPowder,MoveId::RazorLeaf]).unwrap();
    render::draw_party_screen(&PartyScreenState::new_for_move_choice(vec![mon],0),res.as_mut(),0,&mut fb,Lang::En);
    fb.save_png(&out.join("move-choice.png")).unwrap();
    let mut start=StartMenuState::new(true,true,false); start.safari_info=Some(SafariZoneInfo{steps:10,balls:7});
    render::draw_start_menu(&start,"RED",&mut fb,Lang::En); fb.save_png(&out.join("safari.png")).unwrap();
    for species in [Species::Doduo,Species::Lapras] {
        let mut hof=HofCeremonyState::new(vec![HofEntry{species,level:24,nickname:format!("{species:?}").to_uppercase()}],HofPlayerStats{name:"RED".into(),play_time_hours:3,play_time_minutes:7,money:31210,dex_seen:100,dex_owned:80,rating:"80"});
        for _ in 0..250 { hof.update_frame(); }
        render::draw_hof_ceremony(&hof,&mut res,&mut fb,Lang::En); fb.save_png(&out.join(format!("hof-{species:?}.png"))).unwrap();
    }
    let mut credits=CreditsState::new(GameVersion::Red);
    for _ in 0..10 { credits.update_frame(CreditsInput{a:false,b:false}); }
    render::draw_credits(&credits,&mut res,&mut fb); fb.save_png(&out.join("credits.png")).unwrap();
}

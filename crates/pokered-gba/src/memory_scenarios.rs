//! Seeded stress scenarios through the real GBA update/render loop.
use pokered_app::game::PokemonGame;
use pokered_core::{
    game_state::GameScreen, pokedex_screen::PokedexScreenMode, pokemon::stats::create_pokemon,
};
use pokered_data::{maps::MapId, species::Species};
use pokered_renderer::input::{GbButton, InputState};

#[derive(Default)]
pub struct Scenarios {
    started: bool,
    step: u32,
    phase: u8,
    case: u32,
    since: u32,
    observed: bool,
    case_started: bool,
    next_input: u32,
}

impl Scenarios {
    pub fn drive(&mut self, game: &mut PokemonGame, frame: u32, input: &mut InputState) {
        if frame < 4300 {
            return;
        }
        input.set_from_bitmask(0);
        if self.phase == 9 {
            return;
        }
        if self.phase != 0 {
            self.other_scenarios(game, frame, input);
            return;
        }
        if !self.started {
            for n in 1..=151 {
                game.save_data
                    .game_data
                    .pokedex
                    .set_owned(Species::from_index_id(n));
                game.save_data
                    .game_data
                    .pokedex
                    .set_seen(Species::from_index_id(n));
            }
            game.handle_transition(GameScreen::Pokedex);
            self.started = true;
            agb::println!("memory: START pokedex");
            return;
        }
        if frame < self.next_input {
            return;
        }
        self.next_input = frame + 16;
        match self.step % 4 {
            0 => {
                assert_eq!(game.pokedex_screen.mode(), PokedexScreenMode::List);
                input.press(GbButton::A);
            }
            1 => {
                assert_eq!(game.pokedex_screen.mode(), PokedexScreenMode::SideMenu);
                input.press(GbButton::A);
            }
            2 => {
                assert_eq!(game.pokedex_screen.mode(), PokedexScreenMode::Entry);
                agb::println!(
                    "memory: dex={} free={}B stack={}B",
                    game.pokedex_screen.cursor(),
                    pokered_app::game::largest_free_block(),
                    crate::unused_stack_bytes()
                );
                input.press(GbButton::B);
            }
            _ => {
                assert_eq!(game.pokedex_screen.mode(), PokedexScreenMode::List);
                if game.pokedex_screen.cursor() == 151 {
                    if self.case == 0 {
                        self.case = 1;
                        game.state.config.language = pokered_core::game_state::Lang::Zh;
                        game.pokedex_screen = pokered_core::pokedex_screen::PokedexScreenState::new(
                            game.save_data.game_data.pokedex.clone(),
                            game.state.config.version,
                        );
                        self.step += 1;
                        return;
                    }
                    game.state.config.language = pokered_core::game_state::Lang::En;
                    self.pass("pokedex", 302);
                    self.phase = 1;
                    self.case = 0;
                    self.since = frame;
                    game.handle_transition(GameScreen::Overworld);
                    for flag in [
                        "EVENT_GOT_STARTER",
                        "EVENT_GOT_POKEDEX",
                        "EVENT_BATTLED_RIVAL_IN_OAKS_LAB",
                    ] {
                        game.overworld.set_flag_live(flag, true);
                    }
                    game.save_data.party.clear();
                    for id in [6, 9, 3, 130, 143, 149] {
                        game.save_data
                            .party
                            .add(create_pokemon(Species::from_index_id(id), 80, [255; 2]).unwrap())
                            .unwrap();
                    }
                    // Maximal persistent storage, allocated before the rest of
                    // the sweep so every later scene runs with a full save.
                    for b in 0..12 {
                        let bx = game.save_data.pc_storage.get_box_mut(b).unwrap();
                        bx.clear();
                        for n in 0..20 {
                            bx.deposit(
                                create_pokemon(
                                    Species::from_index_id(((b * 20 + n) % 151 + 1) as u8),
                                    80,
                                    [255; 2],
                                )
                                .unwrap(),
                            )
                            .unwrap();
                        }
                    }
                    for team_no in 0..50 {
                        use pokered_core::save::hall_of_fame::{HofMon, HofTeam};
                        let mut team = HofTeam::new();
                        for n in 0..6 {
                            team.add_mon(HofMon::new(
                                ((team_no * 6 + n) % 151 + 1) as u8,
                                80,
                                &[0x80; 10],
                            ));
                        }
                        game.save_data.hall_of_fame.push_team(team);
                    }
                } else {
                    input.press(GbButton::Down);
                }
            }
        }
        self.step += 1;
    }

    fn pass(&self, name: &str, count: u32) {
        let stack = crate::unused_stack_bytes();
        assert!(stack >= 4096, "memory scenario exhausted stack margin");
        agb::println!(
            "memory: PASS case={} count={} free={}B stack={}B",
            name,
            count,
            pokered_app::game::largest_free_block(),
            stack
        );
    }

    fn next(&mut self, phase: u8, frame: u32) {
        self.phase = phase;
        self.case = 0;
        self.since = frame;
        self.observed = false;
        self.case_started = false;
        self.step = 0;
    }

    fn other_scenarios(&mut self, game: &mut PokemonGame, frame: u32, input: &mut InputState) {
        let elapsed = frame - self.since;
        match self.phase {
            1 => {
                // A complete fade-out/load/fade-in needs more than 56 ticks.
                // Never replace an outstanding warp with the next request.
                if elapsed < 80 {
                    return;
                }
                if self.case > 0 {
                    let expected = MapId::from_u8((self.case - 1) as u8).unwrap();
                    assert_eq!(
                        game.overworld.state.current_map, expected,
                        "map warp did not commit"
                    );
                    assert_eq!(game.overworld.map_data.as_ref().unwrap().id, expected);
                }
                if self.case == 248 {
                    self.pass("maps", 248);
                    self.next(2, frame);
                    return;
                }
                let map = MapId::from_u8(self.case as u8).unwrap();
                agb::println!("memory: map={} {:?}", self.case, map);
                // Map-entry allocation/render sweep; the separately tested
                // ending stage completes the Hall of Fame takeover.
                game.hof_ceremony = None;
                game.credits = None;
                game.handle_transition(GameScreen::Overworld);
                game.overworld.warp_to_map(map, 1, 1);
                self.case += 1;
                self.since = frame;
            }
            2 => {
                if !self.case_started {
                    let map = if self.case < 17 {
                        MapId::OaksLab
                    } else {
                        MapId::CinnabarLabFossilRoom
                    };
                    if self.step == 0 {
                        // Map-entry scripts may change story flags during the
                        // sweep. Seed each dialogue fixture independently.
                        for flag in [
                            "EVENT_GOT_STARTER",
                            "EVENT_BATTLED_RIVAL_IN_OAKS_LAB",
                            "EVENT_FOLLOWED_OAK_INTO_LAB",
                        ] {
                            game.overworld.set_flag_live(flag, true);
                        }
                        game.overworld
                            .set_flag_live("EVENT_GOT_POKEDEX", self.case != 0);
                        game.overworld
                            .set_flag_live("EVENT_PALLET_AFTER_GETTING_POKEBALLS", true);
                        game.handle_transition(GameScreen::Overworld);
                        game.overworld.warp_to_map(map, 5, 3);
                        self.step = 1;
                    }
                    assert!(elapsed < 3000, "script fixture warp timed out");
                    if elapsed < 60 || game.overworld.state.current_map != map {
                        return;
                    }
                    // Seed query inputs after the destination interpreter
                    // exists, just like debug-server scenario fixtures.
                    if self.case == 0 {
                        game.save_data
                            .game_data
                            .bag
                            .add_item(pokered_data::items::ItemId::OaksParcel, 1)
                            .unwrap();
                    } else if self.case < 17 {
                        game.save_data.game_data.pokedex =
                            pokered_core::pokemon::pokedex::Pokedex::new();
                        for n in 1..=((self.case - 1) * 10) as u8 {
                            game.save_data
                                .game_data
                                .pokedex
                                .set_owned(Species::from_index_id(n));
                        }
                    }
                    self.case_started = true;
                    game.overworld.state.player.facing = pokered_core::overworld::Direction::Up;
                    let index = if self.case < 17 { 4 } else { 0 };
                    let npc = &mut game.overworld.npc_states[index];
                    npc.visible = true;
                    npc.movement_type = dotzuki_engine::overworld::NpcMovementType::Stationary;
                    agb::println!(
                        "memory: script={:?} case={} free={}B stack={}B",
                        map,
                        self.case,
                        pokered_app::game::largest_free_block(),
                        crate::unused_stack_bytes()
                    );
                }
                if elapsed > 60 && elapsed % 32 < 8 {
                    input.press(GbButton::A);
                }
                self.observed |= !game.overworld.script_engine_idle();
                if elapsed > 200 && self.observed && game.overworld.script_engine_idle() {
                    if self.case == 0 {
                        assert!(game.overworld.unified_flags().get_flag("EVENT_GOT_POKEDEX"));
                        assert!(!game
                            .save_data
                            .game_data
                            .bag
                            .has_item(pokered_data::items::ItemId::OaksParcel, 1));
                        self.pass("oak-parcel", 1);
                    } else if self.case == 16 {
                        self.pass("oak-dex", 16);
                    } else if self.case == 17 {
                        self.pass("fossil", 1);
                    }
                    self.case += 1;
                    self.since = frame;
                    self.observed = false;
                    self.case_started = false;
                    self.step = 0;
                    if self.case == 18 {
                        game.overworld.warp_to_map(MapId::GameCorner, 3, 3);
                        self.next(10, frame);
                    }
                }
                assert!(elapsed < 6000, "script stress timed out");
            }
            3 => {
                if elapsed < 32 {
                    return;
                }
                const COUNT: u32 = 12;
                if self.case == COUNT * 4 {
                    self.pass("menus", COUNT * 4);
                    game.handle_transition(GameScreen::Overworld);
                    for (flag, value) in [
                        ("EVENT_BEAT_BROCK", true),
                        ("EVENT_1ST_ROUTE22_RIVAL_BATTLE", false),
                        ("EVENT_2ND_ROUTE22_RIVAL_BATTLE", true),
                        ("EVENT_BEAT_ROUTE22_RIVAL_2ND_BATTLE", false),
                        ("EVENT_ROUTE22_RIVAL_WANTS_BATTLE", true),
                    ] {
                        game.overworld.set_flag_live(flag, value);
                    }
                    game.overworld.warp_to_map(MapId::Route22, 30, 4);
                    self.next(4, frame);
                    return;
                }
                let n = self.case % COUNT;
                let screen = match n {
                    0 => GameScreen::Overworld,
                    1 => GameScreen::StartMenu,
                    2 => GameScreen::PartyScreen,
                    3..=8 => GameScreen::PokemonStatsScreen((n - 3) as usize),
                    9 => GameScreen::Bag,
                    10 => GameScreen::TownMap,
                    _ => GameScreen::TrainerCard,
                };
                agb::println!("memory: menu={} {:?}", self.case, screen);
                game.handle_transition(screen);
                self.case += 1;
                self.since = frame;
            }
            4 => {
                if !self.case_started {
                    if game.state.screen == GameScreen::Battle
                        && matches!(
                            game.battle.phase,
                            pokered_core::battle::BattlePhase::PlayerMenu
                        )
                    {
                        assert_eq!(game.battle.enemy_party_size, 6);
                        self.case_started = true;
                        self.since = frame;
                        agb::println!("memory: moves retain Route22 battle script");
                    } else {
                        if (60..100).contains(&elapsed) {
                            input.press(GbButton::Left);
                        }
                        if elapsed > 100 && elapsed % 32 < 8 {
                            input.press(GbButton::A);
                        }
                        assert!(
                            elapsed < 3000,
                            "Route22 animation fixture did not reach battle menu"
                        );
                    }
                    return;
                }
                if !self.observed {
                    agb::println!("memory: move={} side={}", self.case / 2 + 1, self.case % 2);
                    game.debug_start_memory_move(
                        pokered_data::moves::MoveId::from_id((self.case / 2 + 1) as u8),
                        self.case % 2 == 0,
                    );
                    self.observed = true;
                    self.since = frame;
                    return;
                }
                if elapsed > 30 && game.debug_memory_move_finished() {
                    self.case += 1;
                    self.observed = false;
                    if self.case == 330 {
                        self.pass("moves", 330);
                        game.handle_transition(GameScreen::Overworld);
                        // End the synthetic animation fixture. Trades and
                        // ending movies cannot run inside Blue's paused battle.
                        game.overworld.warp_to_map(MapId::ViridianPokecenter, 5, 5);
                        self.next(5, frame);
                    }
                }
                assert!(elapsed < 2000, "move animation timed out");
            }
            5 => {
                if elapsed < 96 {
                    return;
                }
                assert_eq!(game.overworld.state.current_map, MapId::ViridianPokecenter);
                if !self.observed {
                    let (from, to) = [
                        (Species::Bulbasaur, Species::Ivysaur),
                        (Species::Magikarp, Species::Gyarados),
                        (Species::Dragonair, Species::Dragonite),
                    ][self.case as usize];
                    agb::println!("memory: evolution={}", self.case);
                    game.debug_play_evolution(from, to);
                    self.observed = true;
                    self.since = frame;
                }
                if elapsed % 32 < 8 {
                    input.press(GbButton::A);
                }
                if elapsed > 60 && game.evolution_anim.is_none() {
                    self.case += 1;
                    self.observed = false;
                    if self.case == 3 {
                        self.pass("evolution", 3);
                        self.next(6, frame);
                    }
                }
                assert!(elapsed < 3000, "evolution timed out");
            }
            6 => {
                if !self.observed {
                    agb::println!("memory: trade={}", self.case);
                    game.trade_anim = Some(pokered_core::trade::TradeAnim::new(
                        Species::Clefairy,
                        Species::MrMime,
                        alloc::string::String::from("RED"),
                        false,
                    ));
                    self.observed = true;
                    self.since = frame;
                }
                if elapsed > 60 && game.trade_anim.is_none() {
                    self.case += 1;
                    self.observed = false;
                    if self.case == 3 {
                        self.pass("trade", 3);
                        game.overworld.pending_hof_ceremony = true;
                        self.next(7, frame);
                    }
                }
                assert!(elapsed < 3000, "trade timed out");
            }
            7 => {
                self.observed |= game.credits.is_some();
                if elapsed % 32 < 8 {
                    input.press(GbButton::A);
                }
                if self.observed && game.credits.is_none() {
                    self.pass("ending", 1);
                    game.handle_transition(GameScreen::Overworld);
                    game.overworld.warp_to_map(MapId::ViridianPokecenter, 5, 5);
                    game.overworld.pending_pc = Some(alloc::string::String::from("pokecenter"));
                    self.next(8, frame);
                }
                assert!(elapsed < 16000, "ending timed out");
            }
            8 => {
                assert!(elapsed < 18000, "PC Hall of Fame viewer timed out");
                if frame < self.next_input {
                    return;
                }
                self.next_input = frame + 16;
                let Some(pc) = game.pc_screen.as_ref() else {
                    return;
                };
                use pokered_core::pc_screen::PcPhase;
                match pc.phase() {
                    PcPhase::MainMenu if self.case == 300 => {}
                    PcPhase::MainMenu => {
                        if pc.main_menu().items()[pc.main_menu().cursor()]
                            == pokered_core::pokemon::pc_menu::PcMainMenuTarget::PkmnLeague
                        {
                            input.press(GbButton::A);
                        } else {
                            input.press(GbButton::Down);
                        }
                        return;
                    }
                    PcPhase::Message => {
                        input.press(GbButton::A);
                        return;
                    }
                    PcPhase::LeagueHoF => {
                        assert_eq!(pc.league_hof_progress(), ((self.case / 6) as usize, 50));
                        let expected = game
                            .save_data
                            .hall_of_fame
                            .get_team((self.case / 6) as usize)
                            .unwrap()
                            .mons()[(self.case % 6) as usize]
                            .species;
                        assert_eq!(pc.league_hof_mon().unwrap().1.species as u8, expected);
                        self.case += 1;
                        input.press(GbButton::A);
                        return;
                    }
                    _ => panic!("unexpected PC viewer phase"),
                }
                self.pass("pc-full", 300);
                game.handle_transition(GameScreen::Overworld);
                game.debug_save_now();
                game.save_data.party.clear();
                game.save_data.pc_storage.clear();
                game.try_load_sram_save();
                assert_eq!(game.save_data.party.count(), 6);
                assert_eq!(game.save_data.pc_storage.total_stored(), 240);
                assert_eq!(game.save_data.hall_of_fame.team_count(), 50);
                self.pass("save-full", 1);
                agb::println!("memory: ALL PASS stack={}B", crate::unused_stack_bytes());
                self.next(9, frame);
            }
            10 => {
                // Visit slots before later battles: its decoded cabinet
                // sheets remain resident for the rest of the process.
                if elapsed < 96 {
                    return;
                }
                assert_eq!(game.overworld.state.current_map, MapId::GameCorner);
                if !self.case_started {
                    game.save_data.game_data.player_coins = 100;
                    game.overworld.pending_slots = Some(true);
                    self.case_started = true;
                    self.since = frame;
                    return;
                }
                if let Some(slots) = game.slots_screen.as_ref() {
                    self.observed = true;
                    if elapsed % 600 == 0 {
                        agb::println!(
                            "memory: slots={} phase={:?} reel={} payout={}",
                            self.case,
                            slots.phase,
                            slots.current_reel,
                            slots.payout_remaining
                        );
                    }
                    if elapsed % 32 < 8 {
                        input.press(
                            if slots.phase == pokered_core::slots_screen::SlotsPhase::Result {
                                GbButton::B
                            } else {
                                GbButton::A
                            },
                        );
                    }
                } else if self.observed && game.state.screen == GameScreen::Overworld {
                    self.case += 1;
                    self.case_started = false;
                    self.observed = false;
                    self.since = frame;
                    if self.case == 3 {
                        self.pass("slots", 3);
                        self.next(3, frame);
                    }
                }
                assert!(elapsed < 24000, "slot-machine spin/exit timed out");
            }
            _ => unreachable!(),
        }
    }
}

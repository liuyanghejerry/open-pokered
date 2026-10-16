//! Real GBA regression: walk onto both Route 22 triggers, complete the Blue
//! battles and exits, then repeat with a warm/fragmented heap. Diagnostic only.
use pokered_app::game::PokemonGame;
use pokered_core::{game_state::GameScreen, pokemon::stats::create_pokemon};
use pokered_data::{event_flags::EventFlag, maps::MapId, species::Species};
use pokered_renderer::input::{GbButton, InputState};

// name, trigger y, parcel delivered, late-game encounter
const CASES: [(&str, u8, bool, bool); 5] = [
    ("before-parcel-lower", 5, false, false),
    ("before-parcel-upper", 4, false, false),
    ("after-parcel-lower", 5, true, false),
    ("final-upper", 4, true, true),
    ("final-lower", 5, true, true),
];
const ROUNDS: usize = CASES.len() * 8;

#[derive(Default)]
pub struct Repro {
    started: Option<u32>,
    round: usize,
    next_at: u32,
    saw_battle: bool,
    last_report: u32,
}

impl Repro {
    pub fn drive(&mut self, game: &mut PokemonGame, frame: u32, input: &mut InputState) {
        if frame < 4300 {
            return;
        }
        input.set_from_bitmask(0);
        if self.round == ROUNDS || frame < self.next_at {
            return;
        }
        let (name, y, parcel, late) = CASES[self.round % CASES.len()];
        if self.started.is_none() {
            agb::println!("route22: START round={} case={}", self.round + 1, name);
            game.save_data.party.clear();
            // A full party stresses retained battle state. Use a damaging
            // first move: the default level-50 learnset starts with LEER.
            let mut mon =
                create_pokemon(Species::Charizard, if late { 100 } else { 50 }, [0xFF; 2]).unwrap();
            mon.moves[0] = pokered_data::moves::MoveId::Flamethrower;
            mon.pp[0] = 15;
            for _ in 0..6 {
                game.save_data.party.add(mon).unwrap();
            }
            for (flag, value) in [
                ("EVENT_GOT_STARTER", true),
                ("EVENT_GOT_POKEDEX", parcel),
                ("EVENT_OAK_GOT_PARCEL", parcel),
                ("EVENT_BEAT_BROCK", late),
                ("EVENT_1ST_ROUTE22_RIVAL_BATTLE", !late),
                ("EVENT_2ND_ROUTE22_RIVAL_BATTLE", late),
                ("EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE", false),
                ("EVENT_BEAT_ROUTE22_RIVAL_2ND_BATTLE", false),
                ("EVENT_ROUTE22_RIVAL_WANTS_BATTLE", true),
            ] {
                game.overworld.set_flag_live(flag, value);
            }
            game.overworld.warp_to_map(MapId::Route22, 30, y);
            game.overworld.party_count = game.save_data.party.count() as u8;
            game.overworld.party_lead_level = game.save_data.party.leader_level();
            self.started = Some(frame);
        }
        let elapsed = frame - self.started.unwrap();
        if (60..100).contains(&elapsed) {
            input.press(GbButton::Left);
        } else if elapsed > 100 && elapsed % 32 < 8 {
            input.press(GbButton::A);
        }
        if game.state.screen == GameScreen::Battle {
            self.saw_battle = true;
            assert_eq!(game.battle.enemy_party_size, if late { 6 } else { 2 });
        }
        if frame - self.last_report >= 600 {
            self.last_report = frame;
            let stack = crate::unused_stack_bytes();
            agb::println!(
                "route22: frame={} round={} screen={:?} free={}B stack={}B",
                frame,
                self.round + 1,
                game.state.screen,
                pokered_app::game::largest_free_block(),
                stack
            );
            assert!(stack >= 4096, "Route22 exhausted stack safety margin");
        }
        let won = if late {
            EventFlag::EVENT_BEAT_ROUTE22_RIVAL_2ND_BATTLE
        } else {
            EventFlag::EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE
        };
        if self.saw_battle
            && game.state.screen == GameScreen::Overworld
            && game.overworld.unified_flags().check(won)
            && !game
                .overworld
                .unified_flags()
                .check(EventFlag::EVENT_ROUTE22_RIVAL_WANTS_BATTLE)
        {
            agb::println!("route22: PASS round={} case={}", self.round + 1, name);
            self.round += 1;
            self.started = None;
            self.saw_battle = false;
            // Let the final clearJoyIgnore command settle before reloading.
            self.next_at = frame + 120;
            if self.round == ROUNDS {
                game.debug_save_now();
                agb::println!(
                    "route22: ALL PASS rounds={} save=ok stack={}B",
                    ROUNDS,
                    crate::unused_stack_bytes()
                );
            }
        }
        assert!(elapsed < 16000, "Route22 encounter timed out");
    }
}

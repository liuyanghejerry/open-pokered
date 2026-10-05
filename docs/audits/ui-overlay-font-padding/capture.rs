//! Run this identical fixture against master and the PR branch.
//! Copy to crates/pokered-app/examples/capture_ui_overlay_padding.rs;
//! cargo run --locked -p pokered-app --example capture_ui_overlay_padding -- OUT before|after
use std::path::Path;

use dotzuki_engine::render_config::RenderConfig;
use pokered_app::{render, PokemonGame};
use pokered_core::main_menu::MenuInput;
use pokered_core::{
    bag_screen::{BagScreenInput, BagScreenState},
    game_state::{GameScreen, Lang},
    hof_ceremony::{HofCeremonyState, HofEntry, HofPhase, HofPlayerStats},
    oak_speech::OakSpeechPhase,
    party_screen::{PartyNoticeReturn, PartyScreenInput, PartyScreenState},
    pc_screen::{HofMonView, HofTeamRecord, PcContext, PcEntry, PcOpenContext, PcPhase, PcScreen},
    pokedex_screen::{PokedexScreenInput, PokedexScreenMode, PokedexScreenState},
    pokemon::{pokedex::Pokedex, stats::create_pokemon},
    save::SaveData,
    slots_screen::{SlotsPhase, SlotsScreen},
};
use pokered_data::{items::ItemId, species::Species, wild_data::GameVersion};
use pokered_renderer::{FrameBuffer, Rgba};
use pokered_ui::{backends::FrameBufferPainter, menus, Ui};

fn game(lang: Lang) -> PokemonGame {
    let mut g = PokemonGame::new_with_options(
        GameVersion::Red,
        None,
        None,
        None,
        true,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    pokered_app::tools::apply_lang(&mut g, lang);
    g.frame_count = 0;
    g
}

fn save(g: &mut PokemonGame, out: &Path, name: &str, suffix: &str) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    g.draw(&mut fb);
    fb.save_png(&out.join(format!("{name}-{suffix}.png")))
        .unwrap();
}

fn party() -> PartyScreenState {
    PartyScreenState::new(
        [
            Species::Bulbasaur,
            Species::Charmander,
            Species::Squirtle,
            Species::Pikachu,
            Species::Pidgey,
            Species::Rattata,
        ]
        .into_iter()
        .map(|sp| create_pokemon(sp, 20, [255, 255]).unwrap())
        .collect(),
    )
}

fn party_press(state: &mut PartyScreenState, down: bool, a: bool) {
    state.update_frame(PartyScreenInput {
        down,
        a,
        ..PartyScreenInput::none()
    });
}

fn hof(phase: HofPhase, species: Species) -> HofCeremonyState {
    let mut state = HofCeremonyState::new(
        vec![HofEntry {
            species,
            level: 100,
            nickname: format!("{species:?}").to_uppercase(),
        }],
        HofPlayerStats {
            name: "RED".into(),
            play_time_hours: 255,
            play_time_minutes: 59,
            money: 999999,
            dex_seen: 151,
            dex_owned: 151,
            rating: "Good job!\nKeep it up!",
        },
    );
    for _ in 0..3000 {
        if state.phase() == phase {
            return state;
        }
        state.update_frame();
    }
    panic!("did not reach {phase:?}");
}

fn pc_press(pc: &mut PcScreen, save: &mut SaveData, down: bool, a: bool) {
    let mut context = PcContext {
        party: &mut save.party,
        pc_storage: &mut save.pc_storage,
        bag: &mut save.game_data.bag,
        pc_items: &mut save.game_data.pc_items,
        pokedex: &save.game_data.pokedex,
    };
    pc.update_frame(
        MenuInput {
            down,
            a,
            up: false,
            b: false,
        },
        &mut context,
    );
}

fn pc_skip(pc: &mut PcScreen, save: &mut SaveData) {
    for _ in 0..100 {
        if pc.phase() != PcPhase::Message {
            return;
        }
        pc_press(pc, save, false, true);
    }
    panic!("PC message did not close");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = Path::new(&args[1]);
    let suffix = &args[2];
    std::fs::create_dir_all(out).unwrap();
    for (lang, tag) in [(Lang::En, "en"), (Lang::Zh, "zh")] {
        let mut g = game(lang);
        g.state.screen = GameScreen::Bag;
        g.bag_screen = BagScreenState::new(vec![(ItemId::Potion, 99), (ItemId::SuperPotion, 99)]);
        g.bag_screen.update_frame(BagScreenInput {
            a: true,
            ..BagScreenInput::none()
        });
        save(&mut g, out, &format!("bag-action-{tag}"), suffix);

        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        render::draw_bag(&g.bag_screen, &mut fb, lang);
        let mut painter = FrameBufferPainter::new(&mut fb).with_lang(lang);
        menus::bag::draw_machine_prompt(
            ItemId::from_id(201),
            Some(1),
            &mut Ui::new(&mut painter),
            lang,
        );
        fb.save_png(&out.join(format!("bag-machine-{tag}-{suffix}.png")))
            .unwrap();

        let mut g = game(lang);
        g.state.screen = GameScreen::PartyScreen;
        g.party_screen = party();
        party_press(&mut g.party_screen, false, true);
        save(&mut g, out, &format!("party-action-{tag}"), suffix);
        for _ in 0..(g.party_screen.selected_field_moves().len() + 1) {
            party_press(&mut g.party_screen, true, false);
        }
        party_press(&mut g.party_screen, false, true);
        save(&mut g, out, &format!("party-switch-{tag}"), suffix);

        g.party_screen = party();
        g.party_screen.show_move_choice_notice(if lang == Lang::Zh {
            "无法使用这个招式！\n请重新选择。\n当前宝可梦\n需要其他招式。".into()
        } else {
            "This move can't\nbe used now!\nChoose another\nmove instead.".into()
        });
        save(&mut g, out, &format!("party-move-notice-{tag}"), suffix);
        g.party_screen.show_item_use_notice(
            if lang == Lang::Zh {
                "妙蛙种子的\n体力恢复了！".into()
            } else {
                "BULBASAUR's\nHP was restored!".into()
            },
            0,
            PartyNoticeReturn::Bag,
        );
        save(&mut g, out, &format!("party-item-notice-{tag}"), suffix);

        let mut g = game(lang);
        g.state.screen = GameScreen::OakSpeech;
        g.oak_speech.phase = OakSpeechPhase::PlayerNameChoice { cursor: 2 };
        save(&mut g, out, &format!("oak-name-prompt-{tag}"), suffix);

        for species in [Species::Lapras, Species::Pikachu] {
            let mut g = game(lang);
            g.hof_ceremony = Some(hof(HofPhase::MonText, species));
            save(&mut g, out, &format!("hof-{species:?}-{tag}"), suffix);
        }
        let mut g = game(lang);
        g.hof_ceremony = Some(hof(HofPhase::PlayerStats, Species::Lapras));
        save(&mut g, out, &format!("hof-player-{tag}"), suffix);

        let mut g = game(lang);
        let open = PcOpenContext {
            has_pokedex: false,
            met_bill: true,
            beaten_league: true,
            player_name: "RED".into(),
            hof_teams: vec![HofTeamRecord {
                team_no: 1,
                mons: vec![HofMonView {
                    species: Species::Lapras,
                    level: 100,
                    nickname: "LAPRAS".into(),
                }],
            }],
        };
        let mut pc = PcScreen::new(PcEntry::PokemonCenter, &open);
        pc_skip(&mut pc, &mut g.save_data);
        pc_press(&mut pc, &mut g.save_data, true, false);
        pc_press(&mut pc, &mut g.save_data, true, false);
        pc_press(&mut pc, &mut g.save_data, false, true);
        pc_skip(&mut pc, &mut g.save_data);
        assert_eq!(pc.phase(), PcPhase::LeagueHoF);
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        render::draw_pc(&pc, &g.save_data, &mut g.resources, &mut fb, lang);
        fb.save_png(&out.join(format!("league-pc-{tag}-{suffix}.png")))
            .unwrap();

        for result in [false, true] {
            let mut state = SlotsScreen::new(false, 100, 42);
            if result {
                state.phase = SlotsPhase::Result;
                state.message = "Play again?".into();
            }
            let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            render::draw_slots(&state, &mut fb, lang);
            let name = if result { "slots-result" } else { "slots-bet" };
            fb.save_png(&out.join(format!("{name}-{tag}-{suffix}.png")))
                .unwrap();
        }

        let mut g = game(lang);
        let mut dex = Pokedex::new();
        dex.set_seen(Species::Bulbasaur);
        let mut state = PokedexScreenState::new(dex, GameVersion::Red);
        state.update_frame(PokedexScreenInput {
            a: true,
            ..PokedexScreenInput::none()
        });
        for _ in 0..2 {
            state.update_frame(PokedexScreenInput {
                down: true,
                ..PokedexScreenInput::none()
            });
        }
        state.update_frame(PokedexScreenInput {
            a: true,
            ..PokedexScreenInput::none()
        });
        assert_eq!(state.mode(), PokedexScreenMode::Area);
        assert!(state.area_maps().is_empty());
        g.state.screen = GameScreen::Pokedex;
        g.pokedex_screen = state;
        save(&mut g, out, &format!("dex-area-unknown-{tag}"), suffix);
    }
}

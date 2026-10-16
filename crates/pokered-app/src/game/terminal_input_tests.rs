//! Canonical Game caller tests. Only the forthcoming scene capability is
//! opted in explicitly; no test supplies known input history or text phases.
//! Boot, talk, battle, terminal glyph/PROMPT, GO and field use ordinary input.
use super::cinnabar_victory_tests::Session;
use super::*;
use pokered_core::foreground_input::{A, SELECT, START};
use pokered_core::text::terminal_input::TerminalTextKind;

const LIMIT: usize = 12000; // stuck-test bound, not a timing oracle

fn reach_prize_prompt(session: &mut Session) {
    let mut opted_in = false;
    for frame in 0..LIMIT {
        if !opted_in && session.game.state.screen == GameScreen::Battle {
            let owner = session.game.overworld.begin_source_foreground();
            assert_eq!(owner.last(), None, "never inject a preceding source sample");
            opted_in = true;
        }
        if session
            .game
            .battle
            .terminal_text_work()
            .is_some_and(|work| work.kind == TerminalTextKind::Prize && work.waiting_for_input())
        {
            assert!(opted_in);
            assert!(
                session
                    .game
                    .overworld
                    .source_foreground()
                    .unwrap()
                    .last()
                    .is_some(),
                "actual Game letter polls must establish the prompt's preceding history"
            );
            return;
        }
        session.step(if frame % 32 == 0 { &[GbButton::A] } else { &[] });
    }
    panic!("ordinary battle never reached the returning Money prompt");
}

#[test]
fn real_money_prompt_and_go_return_held_or_new_start_without_a_physical_prev_guess() {
    for held_start in [false, true] {
        let mut session = Session::boot(0, false);
        reach_prize_prompt(&mut session);
        session.step(&[]); // actual ManualTextScroll poll, not an injected last
        let buttons = if held_start {
            &[GbButton::A, GbButton::Start, GbButton::Select][..]
        } else {
            &[GbButton::A, GbButton::Select][..]
        };
        let (id, busy, pcm) = session.step(buttons);
        assert_eq!(id, SfxId::PressAB as u8);
        assert!(
            busy && pcm,
            "the actual ACK receipt must own real PressAB PCM"
        );
        let ack_last = A | SELECT | if held_start { START } else { 0 };
        assert_eq!(
            session.game.overworld.source_foreground().unwrap().last(),
            Some(ack_last)
        );
        let mut go_seen = false;
        let mut returned = false;
        for _ in 0..LIMIT {
            let (id, busy, _) = session.step(&[GbButton::Start, GbButton::Select]);
            if id == 86 && busy {
                go_seen = true;
            }
            let Some(owner) = session.game.overworld.source_foreground() else {
                returned = true;
                break;
            };
            assert_eq!(
                owner.last(),
                Some(ack_last),
                "sound/script waits do not poll"
            );
            assert!(session.game.overworld.source_foreground_error().is_none());
        }
        assert!(
            go_seen && returned,
            "must reach the actual first field poll"
        );
        assert_eq!(
            session.game.state.screen,
            if held_start {
                GameScreen::Overworld
            } else {
                GameScreen::StartMenu
            }
        );
        assert_eq!(std::fs::read(&session.path).unwrap(), session.bytes);
    }
}

#[test]
fn explicit_unknown_field_return_blocks_without_inventing_neutral_or_reemitting_sound() {
    let mut session = Session::boot(0, false);
    let mut ready = false;
    for frame in 0..LIMIT {
        session.step(if frame % 32 == 0 { &[GbButton::A] } else { &[] });
        if session
            .game
            .overworld
            .script_flags()
            .get("EVENT_BEAT_CINNABAR_GYM_TRAINER_0")
            == Some(&true)
            && session.game.state.screen == GameScreen::Overworld
            && session.game.overworld.script_engine_idle()
        {
            ready = true;
            break;
        }
    }
    assert!(
        ready,
        "ordinary legacy battle must return to a real idle field"
    );
    session.game.overworld.begin_source_foreground();
    for _ in 0..8 {
        session.step(&[GbButton::Start]);
    }
    assert_eq!(
        session.game.overworld.source_foreground_error(),
        Some(pokered_core::text::terminal_input::TerminalInputError::UnknownReturningField)
    );
    assert_eq!(session.game.state.screen, GameScreen::Overworld);
    assert_eq!(
        session.game.overworld.source_foreground().unwrap().last(),
        None
    );
    assert_eq!(
        session.game.overworld.sfx_event,
        pokered_core::overworld::OverworldSfxEvent::None
    );
}

#[test]
fn actual_battle_levelups_evolve_then_finish_hm_retry_replace_or_abandon_before_next_mon() {
    use crate::rare_candy::RareCandyPhase;
    use pokered_core::party_screen::PartyScreenPhase;
    use pokered_core::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::{moves::MoveId, species::Species};
    // Both are valid preboot records one EXP short of a real level-up. The
    // physical EXP.ALL, actual enemy faint and settlement create both events.
    for abandon in [false, true] {
        let lead = create_pokemon_with_moves(
            Species::Blastoise,
            100,
            [0x9a, 0x78],
            [MoveId::Surf, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap();
        let mut fire = create_pokemon_with_moves(
            Species::Charmeleon,
            35,
            [0x9a, 0x78],
            [MoveId::Cut, MoveId::Scratch, MoveId::Ember, MoveId::Leer],
        )
        .unwrap();
        let mut fish = create_pokemon_with_moves(
            Species::Magikarp,
            19,
            [0x9a, 0x78],
            [MoveId::Splash, MoveId::Tackle, MoveId::None, MoveId::None],
        )
        .unwrap();
        for (mon, next) in [(&mut fire, 36), (&mut fish, 20)] {
            let growth = pokered_data::pokemon_data::get_base_stats(mon.species)
                .unwrap()
                .growth_rate;
            mon.total_exp =
                pokered_core::battle::experience::growth::exp_for_level(growth, next) - 1;
        }
        let mut session = Session::boot_with_party(0, false, vec![lead, fire, fish], true);
        reach_prize_prompt(&mut session);
        session.step(&[]);
        session.step(&[GbButton::A]);
        let mut order = Vec::new();
        let mut saw_hm_retry = false;
        let mut finished = false;
        for frame in 0..LIMIT {
            if let Some(anim) = &session.game.evolution_anim {
                if let Some(current) = anim.current() {
                    if order.last() != Some(&current.party_index) {
                        order.push(current.party_index);
                    }
                }
            }
            let mut button = GbButton::A;
            if session.game.post_battle_learning_active() {
                let phase = session.game.rare_candy_flow.as_ref().unwrap().phase;
                if matches!(phase, RareCandyPhase::ReplaceConfirm { .. }) && abandon {
                    button = GbButton::B;
                }
                if matches!(phase, RareCandyPhase::ChooseMove) {
                    match session.game.party_screen.phase() {
                        PartyScreenPhase::MoveChoiceNotice => {
                            saw_hm_retry = true;
                            assert_eq!(
                                session.game.save_data.party.get(1).unwrap().moves[0],
                                MoveId::Cut
                            );
                        }
                        PartyScreenPhase::ChooseMove { cursor: 0 } if saw_hm_retry => {
                            button = GbButton::Down;
                        }
                        _ => {}
                    }
                }
                assert_eq!(
                    session
                        .game
                        .evolution_anim
                        .as_ref()
                        .unwrap()
                        .current()
                        .unwrap()
                        .party_index,
                    if session.game.save_data.party.get(2).unwrap().species == Species::Gyarados {
                        2
                    } else {
                        1
                    },
                    "next party evolution must wait for this learning result"
                );
            }
            session.step(if frame % 8 == 0 {
                std::slice::from_ref(&button)
            } else {
                &[]
            });
            assert!(session.game.overworld.source_foreground_error().is_none());
            if session.game.evolution_anim.is_none()
                && !session.game.post_battle_learning_active()
                && session.game.overworld.source_foreground().is_none()
            {
                finished = true;
                break;
            }
        }
        assert!(finished);
        assert_eq!(order, vec![1, 2]);
        assert_eq!(
            session.game.save_data.party.get(1).unwrap().species,
            Species::Charizard
        );
        assert_eq!(
            session.game.save_data.party.get(1).unwrap().moves[0],
            MoveId::Cut
        );
        assert_eq!(
            session.game.save_data.party.get(1).unwrap().moves[1],
            if abandon {
                MoveId::Scratch
            } else {
                MoveId::Slash
            }
        );
        assert_eq!(saw_hm_retry, !abandon);
        let fish = session.game.save_data.party.get(2).unwrap();
        assert_eq!(fish.species, Species::Gyarados);
        assert_eq!(fish.moves[2], MoveId::Bite);
        assert_eq!(
            fish.pp[2],
            pokered_core::pokemon::move_learning::get_move_max_pp(MoveId::Bite)
        );
        assert_eq!(std::fs::read(&session.path).unwrap(), session.bytes);
    }
}

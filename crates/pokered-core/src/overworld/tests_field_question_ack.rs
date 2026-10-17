//! Inner DONE/@ questions return to their menu, without an outer text ACK.
//! These exercise embedded callers and the ordinary field input driver. They
//! are component tests, not physical-SRAM or original-hardware replay evidence.

use super::screen::OverworldGameDataRequest;
use super::script_bridge::ScriptEffect;
use super::{Direction, OverworldInput, OverworldScreen};
use crate::alloc_prelude::*;
use crate::snapshot::OverworldSnapshot;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};

type Field = OverworldScreen<PokemonRedData>;

#[derive(Clone, Copy, Debug)]
enum Caller {
    Bike,
    Admission,
    EarlyLoad,
    EarlyRow,
}

// Bike's genuine NO_TEXT_DELAY question has no glyph poll: an A pressed
// during preparation is fresh at the first menu poll. Its distinct input
// contract and snapshots are covered in bike_shop::tests. Safari keeps the
// ordinary typing/held-A contract below; Bike business controls stay here.
const CALLERS: [Caller; 3] = [Caller::Admission, Caller::EarlyLoad, Caller::EarlyRow];

fn input(a: bool, b: bool) -> OverworldInput {
    OverworldInput::new(false, false, false, false, a, b, false, false)
}

fn text(field: &Field) -> Option<&str> {
    match &field.active_script_effect {
        Some(ScriptEffect::ShowDialogue { text } | ScriptEffect::PrintFieldText { text, .. }) => {
            Some(text)
        }
        Some(ScriptEffect::BikeShopOffer { .. }) => field
            .pending_dialogue
            .as_ref()
            .and_then(|dialogue| dialogue.current())
            .map(|page| page.line1.as_ref()),
        _ => None,
    }
}

fn question(field: &Field, caller: Caller) -> bool {
    text(field).is_some_and(|text| {
        text.starts_with(match caller {
            Caller::Bike => "It's a cool BIKE!",
            Caller::Admission => "For just ¥500,",
            Caller::EarlyLoad | Caller::EarlyRow => "Leaving early?",
        })
    })
}

fn at_question(caller: Caller, delay: u16, money: u32) -> Field {
    let map = match caller {
        Caller::Bike => MapId::BikeShop,
        _ => MapId::SafariZoneGate,
    };
    let mut field = Field::new(map, None, PokemonRedData);
    let (x, y, facing) = match caller {
        Caller::Bike => (6, 4, Direction::Up),
        Caller::Admission => (4, 3, Direction::Up),
        Caller::EarlyLoad => (4, 0, Direction::Down),
        Caller::EarlyRow => (4, 1, Direction::Down),
    };
    field.state.player.x = x;
    field.state.player.y = y;
    field.state.player.facing = facing;
    field.set_text_delay_frames(delay);
    field.seed_script_query_state(money, &[], 0, 0, 0, 0, &[], 0, 0, 0);
    if matches!(caller, Caller::EarlyLoad | Caller::EarlyRow) {
        field.start_safari_game();
        field.safari_steps = 450;
        field.safari_balls = 20;
    }
    if matches!(caller, Caller::EarlyLoad) {
        // The production entry owner, after persistent flags/counters exist.
        field.run_on_load();
    } else {
        // NPC talk goes through the counter; other callers walk onto the row.
        // EarlyRow intentionally tests the duplicate coordinate handler, not
        // a claim that Continue bypasses the map's ordinary OnLoad question.
        for _ in 0..120 {
            field.update_frame(input(false, false));
        }
        for _ in 0..120 {
            let held = match caller {
                Caller::Bike => input(true, false),
                Caller::Admission => {
                    OverworldInput::new(true, false, false, false, false, false, false, false)
                }
                Caller::EarlyRow => {
                    OverworldInput::new(false, true, false, false, false, false, false, false)
                }
                Caller::EarlyLoad => unreachable!(),
            };
            field.update_frame(held);
            if field.active_script_effect.is_some() {
                break;
            }
        }
        assert!(
            field.active_script_effect.is_some(),
            "{caller:?} never started"
        );
    }
    // Acknowledge only preceding dialogue, never the question under test.
    let mut previous_page_acks = 0;
    for frame in 0..4000 {
        if question(&field, caller) {
            if matches!(caller, Caller::Bike | Caller::Admission) {
                assert!(previous_page_acks > 0, "welcome still needs its text input");
            }
            field.update_frame(input(false, false));
            return field;
        }
        assert!(
            field.pending_choice.is_none(),
            "unexpected menu before question"
        );
        let a = frame % 2 == 0
            && field
                .pending_dialogue
                .as_ref()
                .is_some_and(|d| d.waiting_for_input());
        previous_page_acks += usize::from(a);
        field.update_frame(input(a, false));
    }
    panic!("{caller:?} did not reach its question");
}

fn restore(field: &mut Field) {
    let json = serde_json::to_string(&OverworldSnapshot::capture(field)).unwrap();
    let snapshot: OverworldSnapshot = serde_json::from_str(&json).unwrap();
    let mut fresh = Field::new(field.state.current_map, None, PokemonRedData);
    snapshot.restore_into(&mut fresh);
    *field = fresh;
}

fn open_question(field: &mut Field, caller: Caller, hold: bool, roundtrip: bool) {
    if matches!(caller, Caller::Bike) {
        // No final ACK is supplied here. DONE plus cursor preparation reaches
        // the real HandleMenuInput owner; visible labels alone are not input.
        for _ in 0..4000 {
            if field.choice_accepts_input() {
                assert!(field.pending_dialogue.is_none());
                assert_eq!(field.pending_choice.as_ref().unwrap().selected, 0);
                let (top, bottom) = field
                    .displayed_field_dialogue()
                    .unwrap()
                    .get_display_text()
                    .unwrap();
                assert!(format!("{top} {bottom}").contains("want it?"));
                assert!(!field.dialogue_needs_button());
                return;
            }
            field.update_frame(input(false, false));
        }
        panic!("Bike DONE incorrectly waited for an extra final-page ACK");
    }
    let mut restored_final_delay = false;
    let mut intermediate_pages = 0;
    for frame in 0..4000 {
        if field.pending_choice.is_some() {
            assert!(
                field.pending_dialogue.is_none(),
                "text must not consume the answer"
            );
            assert!(!field.dialogue_needs_button());
            assert_eq!(field.pending_choice.as_ref().unwrap().selected, 0);
            let (top, bottom) = field
                .displayed_field_dialogue()
                .unwrap()
                .get_display_text()
                .unwrap();
            assert!(format!("{top} {bottom}").contains(match caller {
                Caller::Bike => "want it?",
                Caller::Admission => "hunt?",
                Caller::EarlyLoad | Caller::EarlyRow => "Leaving early?",
            }));
            if matches!(caller, Caller::Admission) {
                assert!(intermediate_pages > 0, "the pitch still has earlier pages");
                assert!(field.script_money_box.is_some());
            }
            if roundtrip {
                assert!(
                    restored_final_delay,
                    "must restore during the final glyph's wait"
                );
                let shown = serde_json::to_value(field.displayed_field_dialogue()).unwrap();
                restore(field);
                assert_eq!(
                    serde_json::to_value(field.displayed_field_dialogue()).unwrap(),
                    shown
                );
                assert_eq!(field.pending_choice.as_ref().unwrap().selected, 0);
            }
            assert_eq!(field.prev_a_pressed, hold, "{caller:?} lost its held input");
            return;
        }
        // DONE retires the dialogue before the next update creates the menu.
        // Keep the same physical hold across that no-dialogue handoff too.
        let mut a = hold && field.pending_dialogue.is_none();
        if let Some(d) = &field.pending_dialogue {
            if d.is_last_page() {
                if roundtrip
                    && !restored_final_delay
                    && d.char_index() == d.total_chars()
                    && !d.waiting_for_input()
                {
                    restore(field);
                    restored_final_delay = true;
                    // The next ordinary update must retire the saved final
                    // letter wait, not invent an extra final-page button.
                }
                a = hold;
            } else if d.waiting_for_input() && frame % 2 == 0 {
                // Required earlier PARA/CONT confirmations remain required.
                let before = serde_json::to_value(field.pending_dialogue.as_ref()).unwrap();
                for _ in 0..4 {
                    field.update_frame(input(false, false));
                }
                assert_eq!(
                    serde_json::to_value(field.pending_dialogue.as_ref()).unwrap(),
                    before
                );
                intermediate_pages += 1;
                a = true;
            }
        }
        field.update_frame(input(a, false));
    }
    panic!("{caller:?} waited for an extra final-page ACK");
}

fn next_move(field: &mut Field) -> Vec<(i16, i16)> {
    for frame in 0..4000 {
        if let Some(ScriptEffect::MovePlayerRelative { steps, .. }) = &field.active_script_effect {
            return steps.clone();
        }
        let a = frame % 2 == 0
            && field
                .pending_dialogue
                .as_ref()
                .is_some_and(|d| d.waiting_for_input());
        field.update_frame(input(a, false));
    }
    panic!("answer did not reach its existing movement continuation");
}

#[test]
fn three_safari_questions_open_without_final_ack_at_each_speed_and_restore() {
    for caller in CALLERS {
        for delay in [1, 3, 5] {
            let mut field = at_question(caller, delay, 1000);
            open_question(&mut field, caller, false, true);
            for _ in 0..24 {
                field.update_frame(input(false, false));
                assert_eq!(field.pending_choice.as_ref().unwrap().selected, 0);
                assert!(
                    field.game_data_requests.is_empty(),
                    "opening must not answer/pay"
                );
            }
        }
    }
}

#[test]
fn typing_hold_does_not_answer_the_menu_and_fresh_b_takes_cancel() {
    for caller in CALLERS {
        let mut field = at_question(caller, 3, 1000);
        // Both row callers reach their trigger by one ordinary completed
        // step; unlike snapshot restore, this test retains its queued tick.
        let entry_requests = field.game_data_requests.clone();
        assert_eq!(
            entry_requests,
            if matches!(caller, Caller::Admission | Caller::EarlyRow) {
                vec![OverworldGameDataRequest::TickDaycareExp]
            } else {
                vec![]
            },
            "{caller:?} entry-step requests"
        );
        open_question(&mut field, caller, true, false);
        for frame in 0..20 {
            field.update_frame(input(true, false));
            assert_eq!(
                field.pending_choice.as_ref().map(|c| c.selected),
                Some(0),
                "{caller:?} accepted held A on menu frame {frame}"
            );
        }
        field.update_frame(input(false, false));
        field.update_frame(input(false, true));
        assert!(
            field.pending_choice.is_none(),
            "one fresh B is the menu answer"
        );
        assert_eq!(
            field.game_data_requests, entry_requests,
            "{caller:?} cancel must not add persistent-data requests"
        );
        for frame in 0..1000 {
            if let Some(response) = text(&field) {
                assert!(response.starts_with(match caller {
                    Caller::Bike => "Come back again",
                    Caller::Admission => "OK! Please come again!",
                    Caller::EarlyLoad | Caller::EarlyRow => "Good Luck!",
                }));
                break;
            }
            assert!(frame < 999, "missing NO/cancel response");
            field.update_frame(input(false, false));
        }
    }
}

#[test]
fn safari_admission_keeps_fee_threshold_and_refusal_shove() {
    for (money, yes, admitted) in [(499, true, false), (500, true, true), (1000, false, false)] {
        let mut field = at_question(Caller::Admission, 1, money);
        open_question(&mut field, Caller::Admission, false, false);
        field.update_frame(input(false, false));
        field.update_frame(input(yes, !yes));
        let path = next_move(&mut field);
        assert_eq!(
            path,
            if admitted {
                vec![(0, -1); 3]
            } else {
                vec![(0, 1)]
            }
        );
        assert_eq!(
            field
                .script_flags()
                .get("EVENT_IN_SAFARI_ZONE")
                .copied()
                .unwrap_or(false),
            admitted
        );
        assert!(field.script_money_box.is_none());
        let payments: Vec<_> = field
            .game_data_requests
            .iter()
            .filter_map(|r| match r {
                OverworldGameDataRequest::TakeMoney { amount } => Some(*amount),
                _ => None,
            })
            .collect();
        // Core emits the request; actual SaveData application is App-owned.
        assert_eq!(payments, if admitted { vec![500] } else { vec![] });
    }
}

#[test]
fn both_early_exit_callers_preserve_or_clear_allowance_before_the_return_path() {
    for caller in [Caller::EarlyLoad, Caller::EarlyRow] {
        for yes in [false, true] {
            let mut field = at_question(caller, 1, 1000);
            // EarlyRow's real step onto the coordinate trigger also requests
            // one ordinary Day Care tick. EarlyLoad's scripted step does not.
            let entry_requests = field.game_data_requests.clone();
            assert_eq!(
                entry_requests,
                if matches!(caller, Caller::EarlyRow) {
                    vec![OverworldGameDataRequest::TickDaycareExp]
                } else {
                    vec![]
                },
                "{caller:?} entry-step requests"
            );
            open_question(&mut field, caller, false, false);
            field.update_frame(input(false, false));
            field.update_frame(input(yes, !yes));
            assert_eq!(
                next_move(&mut field),
                if yes { vec![(0, 1); 3] } else { vec![(0, -1)] }
            );
            assert_eq!(field.is_safari_game_active(), !yes);
            assert_eq!(field.safari_steps_remaining(), if yes { 0 } else { 450 });
            assert_eq!(field.safari_balls_remaining(), if yes { 0 } else { 20 });
            assert_eq!(
                field.game_data_requests, entry_requests,
                "{caller:?} answer must not add persistent-data requests"
            );
        }
    }
}

#[test]
fn bike_sale_and_both_cancel_inputs_keep_money_items_and_final_response_owner() {
    for answer in [0, 1, 2] {
        // A/B/Down+A; the typed caller now also keeps accepted B provenance.
        let mut field = at_question(Caller::Bike, 1, 1000);
        open_question(&mut field, Caller::Bike, false, false);
        field.update_frame(input(false, false));
        if answer == 2 {
            field.update_frame(OverworldInput::new(
                false, true, false, false, false, false, false, false,
            ));
            // Direction-only HandleMenuInput redraws the cursor and waits3;
            // no Joypad poll may swallow the later fresh answer during it.
            for _ in 0..3 {
                field.update_frame(input(false, false));
            }
        }
        field.update_frame(input(answer != 1, answer == 1));
        let mut unaffordable = false;
        let mut goodbye = false;
        for frame in 0..4000 {
            if let Some(response) = text(&field) {
                unaffordable |= response.starts_with("Sorry! You can't afford it!");
                goodbye |= response.starts_with("Come back again some time!");
            }
            if field.script_engine_idle() && field.active_script_effect.is_none() {
                assert!(field.pending_dialogue.is_none());
                assert!(field.displayed_field_dialogue().is_none());
                break;
            }
            assert!(frame < 3999, "bike conversation never closed");
            let a = frame % 2 == 0
                && field
                    .pending_dialogue
                    .as_ref()
                    .is_some_and(|d| d.waiting_for_input());
            field.update_frame(input(a, false));
        }
        assert_eq!(unaffordable, answer == 0);
        assert!(goodbye);
        assert!(field.game_data_requests.is_empty());
        assert!(!field
            .script_flags()
            .get("EVENT_GOT_BICYCLE")
            .copied()
            .unwrap_or(false));
        if answer == 0 {
            // Ownership is a pre-existing separate branch: talking again
            // must show the cycling advice rather than this new menu seam.
            field.set_flag_live("EVENT_GOT_BICYCLE", true);
            // The previous ACK and the original interaction both held A.
            // Release through one complete two-VBlank idle sampling cycle
            // before asking the NPC a second time with a fresh press.
            for _ in 0..2 {
                field.update_frame(input(false, false));
            }
            assert!(!field.prev_a_pressed);
            assert!(!field.sampled_player_input.a());
            for _ in 0..120 {
                field.update_frame(input(true, false));
                if field.active_script_effect.is_some() {
                    break;
                }
            }
            assert!(text(&field).is_some_and(|s| s.starts_with("How do you like your new")));
            assert!(field.pending_choice.is_none());
            assert!(field.game_data_requests.is_empty());
        }
    }
}

//! The embedded NPC scripts drive these controls; audio completion is mocked
//! only at the core boundary. Native cold-SRAM recordings cover actual audio.
use super::*;
use crate::foreground_input::{ForegroundJoypad, DOWN, UP};
use crate::overworld::script_bridge::ScriptEffect;
use crate::overworld::{screen::OverworldGameDataRequest, Direction};
use crate::snapshot::OverworldSnapshot;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
type Field = OverworldScreen<PokemonRedData>;

const CASES: [(MapId, u16, u16, &str); 2] = [
    (MapId::CinnabarLabMetronomeRoom, 7, 3, "EVENT_GOT_TM35"),
    (MapId::SafariZoneSecretHouse, 3, 4, "EVENT_GOT_HM03"),
];

fn phase(field: &Field) -> Option<Phase> {
    match &field.active_script_effect {
        Some(ScriptEffect::FieldRewardText { state }) => Some(state.phase),
        _ => None,
    }
}

fn step(field: &mut Field, buttons: u8, playing: bool) -> (usize, usize) {
    field.script_sfx_playing = playing;
    field.update_frame(ForegroundJoypad::as_field_input(buttons));
    assert_eq!(field.source_foreground_error(), None);
    let sounds = core::mem::take(&mut field.audio_requests).into_iter().filter(|request| {
        matches!(request, OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_GET_ITEM_1" || sound_id == "SFX_GET_KEY_ITEM")
    }).count();
    let gifts = core::mem::take(&mut field.game_data_requests)
        .into_iter()
        .filter(|request| matches!(request, OverworldGameDataRequest::GiveItem { .. }))
        .count();
    (sounds, gifts)
}

fn begin(case: (MapId, u16, u16, &str), already_received: bool) -> Field {
    let (map, x, y, flag) = case;
    let mut field = Field::new(map, None, PokemonRedData);
    field.state.player.x = x;
    field.state.player.y = y;
    field.state.player.facing = Direction::Up;
    field.player_name = "RED".into();
    field.set_text_delay_frames(1);
    field.set_flag_live(flag, already_received);
    if map == MapId::BillsHouse {
        for name in ["EVENT_MET_BILL", "EVENT_MET_BILL_2", "EVENT_USED_CELL_SEPARATOR_ON_BILL", "EVENT_BILL_SAID_USE_CELL_SEPARATOR"] {
            field.set_flag_live(name, true);
        }
        let mut flags = *field.toggleable_object_flags();
        for (bit, hidden) in [(0x61, true), (0x62, false), (0x63, true), (7, true), (9, false)] {
            if hidden { flags[bit / 8] |= 1 << (bit % 8); }
            else { flags[bit / 8] &= !(1 << (bit % 8)); }
        }
        field.set_toggleable_object_flags(flags);
        field.apply_hidden_object_flags();
    }
    for _ in 0..2 {
        step(&mut field, 0, false);
    }
    for _ in 0..8 {
        step(&mut field, A, false);
        if phase(&field).is_some() {
            break;
        }
    }
    assert!(
        phase(&field).is_some(),
        "ordinary talk must enter the embedded gift text"
    );
    field
}

fn to_sound(field: &mut Field, flag: &str) {
    let (mut sounds, mut gifts) = (0, 0);
    let mut intro_prompts = 0;
    for _ in 0..5000 {
        if phase(field) == Some(Phase::SoundWait) {
            assert_eq!((sounds, gifts), (1, 1));
            assert!(intro_prompts > 0);
            assert!(!field.unified_flags().get_flag(flag));
            assert!(field.pending_dialogue.as_ref().unwrap().waiting_for_input());
            assert!(field.retain_field_text);
            return;
        }
        let buttons = if matches!(phase(field), Some(Phase::Prompt(_))) {
            intro_prompts += 1;
            B
        } else {
            0
        };
        let (s, g) = step(field, buttons, false);
        sounds += s;
        gifts += g;
        assert!(
            !field.unified_flags().get_flag(flag),
            "flag cannot precede receipt completion"
        );
    }
    panic!("receipt did not begin: {:?}", field.active_script_effect);
}

fn roundtrip(field: &mut Field) {
    let before = serde_json::to_value(OverworldSnapshot::capture(field)).unwrap();
    let snapshot: OverworldSnapshot = serde_json::from_value(before.clone()).unwrap();
    snapshot.restore_into(field);
    assert_eq!(
        serde_json::to_value(OverworldSnapshot::capture(field)).unwrap(),
        before
    );
}

#[test]
fn bill_post_sound_button_wait_discards_short_taps_but_accepts_a_held_button() {
    let flag = "EVENT_GOT_SS_TICKET";
    for held in [false, true] {
        for button in [A, B] {
            let mut field = begin((MapId::BillsHouse, 4, 5, flag), false);
            to_sound(&mut field, flag);
            for frame in 0..20 {
                let keys = if frame >= 4 && (held || frame < 6) { button } else { 0 };
                step(&mut field, keys, true);
                assert_eq!(phase(&field), Some(Phase::SoundWait));
                assert!(!field.unified_flags().get_flag(flag));
            }
            roundtrip(&mut field);
            for _ in 0..16 { step(&mut field, if held { button } else { 0 }, false); }
            if !held {
                assert_eq!(phase(&field), Some(Phase::Prompt(Prompt::Return)));
                assert!(!field.unified_flags().get_flag(flag));
                assert_ne!(field.toggleable_object_flags()[7 / 8] & (1 << (7 % 8)), 0);
                roundtrip(&mut field);
                step(&mut field, button, false);
                for _ in 0..16 { step(&mut field, 0, false); }
            }
            assert!(field.unified_flags().get_flag(flag));
            assert_eq!(field.toggleable_object_flags()[7 / 8] & (1 << (7 % 8)), 0);
            assert_ne!(field.toggleable_object_flags()[9 / 8] & (1 << (9 % 8)), 0);
        }
    }
}

#[test]
fn older_receipt_snapshots_default_to_no_post_sound_prompt() {
    let mut old = serde_json::to_value(FieldRewardTextState::receipt("receipt".into(), "SFX_GET_ITEM_1".into())).unwrap();
    old.as_object_mut().unwrap().remove("post_sound_prompt");
    old.as_object_mut().unwrap().remove("continues_text");
    let restored: FieldRewardTextState = serde_json::from_value(old).unwrap();
    assert_eq!(restored.post_sound_prompt, FieldTextPrompt::Done);
    assert!(!restored.continues_text);
}

#[test]
fn copycat_leading_paragraph_keeps_receipt_and_resumes_after_a_snapshot() {
    let flag = "EVENT_GOT_TM31";
    let mut field = begin((MapId::CopycatsHouse2F, 4, 4, flag), false);
    field.script_bag_names = vec!["POKE_DOLL".into()];
    field.script_engine.seed_set("bag", &field.script_bag_names);
    to_sound(&mut field, flag);
    for _ in 0..10 {
        step(&mut field, 0, true);
    }
    for _ in 0..20 {
        if phase(&field)
            == Some(Phase::Protected {
                remaining: 3,
                next: Prompt::FirstParagraph,
            })
        {
            break;
        }
        step(&mut field, 0, false);
    }
    assert_eq!(
        phase(&field),
        Some(Phase::Protected {
            remaining: 3,
            next: Prompt::FirstParagraph
        })
    );
    roundtrip(&mut field);
    let last = field.source_foreground().unwrap().last();
    for remaining in [2, 1] {
        step(&mut field, A | DOWN, false);
        assert_eq!(
            phase(&field),
            Some(Phase::Protected {
                remaining,
                next: Prompt::FirstParagraph
            })
        );
        assert_eq!(field.source_foreground().unwrap().last(), last);
    }
    step(&mut field, 0, false);
    for _ in 0..180 {
        step(&mut field, 0, false);
    }
    assert_eq!(phase(&field), Some(Phase::Prompt(Prompt::FirstParagraph)));
    let (first, second) = field
        .displayed_field_dialogue()
        .unwrap()
        .get_display_text()
        .unwrap();
    assert!(format!("{first} {second}").contains("received TM31"));
    assert!(!field.unified_flags().get_flag(flag));
    step(&mut field, B, false);
    assert_eq!(
        phase(&field),
        Some(Phase::FirstParagraphClear { remaining: 20 })
    );
    let last = field.source_foreground().unwrap().last();
    for tick in 1..20 {
        step(&mut field, if tick < 4 { A | DOWN } else { 0 }, false);
        assert_eq!(field.source_foreground().unwrap().last(), last);
        assert_eq!(
            field
                .displayed_field_dialogue()
                .unwrap()
                .get_display_text()
                .unwrap(),
            (String::new(), String::new())
        );
        if tick == 9 {
            roundtrip(&mut field);
        }
    }
    step(&mut field, 0, false);
    assert_eq!(phase(&field), Some(Phase::Printing));
    for _ in 0..1000 {
        if phase(&field) == Some(Phase::Prompt(Prompt::Paragraph)) {
            break;
        }
        step(&mut field, 0, false);
    }
    assert_eq!(phase(&field), Some(Phase::Prompt(Prompt::Paragraph)));
    let (first, second) = field
        .displayed_field_dialogue()
        .unwrap()
        .get_display_text()
        .unwrap();
    assert!(format!("{first} {second}").contains("MIMIC"));
    assert!(!field.unified_flags().get_flag(flag));
    assert_eq!((field.state.player.x, field.state.player.y), (4, 4));
}

#[test]
fn both_embedded_gifts_set_flag_after_sound_then_require_outer_ack() {
    for case in CASES {
        let mut field = begin(case, false);
        to_sound(&mut field, case.3);
        let last = field.source_foreground().unwrap().last();
        let text = serde_json::to_value(field.pending_dialogue.as_ref().unwrap()).unwrap();
        for tick in 0..120 {
            let keys = if (5..8).contains(&tick) { B | DOWN } else { 0 };
            assert_eq!(step(&mut field, keys, true), (0, 0));
            assert!(!field.unified_flags().get_flag(case.3));
            assert_eq!(field.source_foreground().unwrap().last(), last);
            assert_eq!(
                serde_json::to_value(field.pending_dialogue.as_ref().unwrap()).unwrap(),
                text
            );
            if tick == 40 {
                roundtrip(&mut field);
            }
        }
        for _ in 0..10 {
            assert_eq!(step(&mut field, 0, false), (0, 0));
            if phase(&field) == Some(Phase::OuterAck) {
                break;
            }
        }
        assert!(field.unified_flags().get_flag(case.3));
        assert_eq!(phase(&field), Some(Phase::OuterAck));
        roundtrip(&mut field);
        for _ in 0..180 {
            assert_eq!(step(&mut field, 0, false), (0, 0));
            assert!(
                field.displayed_field_dialogue().is_some(),
                "neutral input cannot dismiss a gift receipt"
            );
        }
        assert!(field
            .displayed_field_dialogue()
            .unwrap()
            .get_display_text()
            .unwrap()
            .0
            .contains("RED"));
        for _ in 0..30 {
            step(&mut field, A, false);
        }
        assert!(field.pending_dialogue.as_ref().unwrap().holding_open());
        step(&mut field, 0, false);
        for _ in 0..4 {
            step(&mut field, 0, false);
        }
        assert!(field.displayed_field_dialogue().is_none());
        assert_eq!(
            (field.state.player.x, field.state.player.y),
            (case.1, case.2)
        );
    }
}

#[test]
fn explicit_paragraph_retains_blank_window_and_ignores_pulses_during_delay() {
    let mut field = begin(CASES[0], false);
    for _ in 0..1000 {
        if phase(&field) == Some(Phase::Prompt(Prompt::Paragraph)) {
            break;
        }
        step(&mut field, 0, false);
    }
    assert_eq!(phase(&field), Some(Phase::Prompt(Prompt::Paragraph)));
    step(&mut field, B, false);
    assert_eq!(phase(&field), Some(Phase::ParagraphClear { remaining: 20 }));
    let last = field.source_foreground().unwrap().last();
    for tick in 1..20 {
        step(&mut field, if tick < 4 { A | UP } else { 0 }, false);
        assert_eq!(field.source_foreground().unwrap().last(), last);
        let blank = field
            .displayed_field_dialogue()
            .unwrap()
            .get_display_text()
            .unwrap();
        assert_eq!(blank, (String::new(), String::new()));
        assert!(!field.unified_flags().get_flag(CASES[0].3));
    }
    step(&mut field, 0, false);
    assert_eq!(phase(&field), Some(Phase::Printing));
    assert!(field.pending_dialogue.as_ref().unwrap().char_index() > 0);
}

#[test]
fn repeat_talk_has_explicit_paragraphs_but_no_second_gift_or_sound() {
    for case in CASES {
        let mut field = begin(case, true);
        let mut paragraphs = 0;
        for _ in 0..5000 {
            if phase(&field) == Some(Phase::OuterAck) {
                break;
            }
            let keys = if matches!(phase(&field), Some(Phase::Prompt(_))) {
                paragraphs += 1;
                B
            } else {
                0
            };
            assert_eq!(step(&mut field, keys, false), (0, 0));
        }
        assert!(paragraphs > 0);
        assert_eq!(phase(&field), Some(Phase::OuterAck));
        assert!(field.unified_flags().get_flag(case.3));
        step(&mut field, B, false);
        for _ in 0..4 {
            step(&mut field, 0, false);
        }
        assert!(field.displayed_field_dialogue().is_none());
    }
}

#[test]
fn full_bag_keeps_the_reward_available_and_waits_for_outer_ack() {
    use crate::items::inventory::Inventory;
    use pokered_data::items::ItemId;
    for case in CASES {
        let mut field = begin(case, false);
        let mut bag = Inventory::<20>::new();
        for id in 1..=20 {
            bag.add_item(ItemId::from_id(id), 1).unwrap();
        }
        assert_eq!(bag.count(), 20);
        field.seed_script_bag_quantities(&bag);
        for _ in 0..5000 {
            if phase(&field) == Some(Phase::OuterAck) {
                break;
            }
            let keys = if matches!(phase(&field), Some(Phase::Prompt(_))) {
                B
            } else {
                0
            };
            let (sounds, _) = step(&mut field, keys, false);
            assert_eq!(sounds, 0, "a rejected reward must not play its fanfare");
            assert!(!field.unified_flags().get_flag(case.3));
        }
        assert_eq!(phase(&field), Some(Phase::OuterAck));
        let text = field
            .displayed_field_dialogue()
            .unwrap()
            .get_display_text()
            .unwrap();
        assert!(
            format!("{} {}", text.0, text.1).contains(
                if case.0 == MapId::CinnabarLabMetronomeRoom {
                    "full"
                } else {
                    "room"
                }
            ),
            "{text:?}"
        );
        for _ in 0..60 {
            step(&mut field, 0, false);
        }
        assert!(field.displayed_field_dialogue().is_some());
        step(&mut field, B, false);
        for _ in 0..4 {
            step(&mut field, 0, false);
        }
        assert!(field.displayed_field_dialogue().is_none());
        assert!(!field.unified_flags().get_flag(case.3));
    }
}

#[test]
fn a_hold_crossing_the_fanfare_waits_for_release_without_advancing_the_field() {
    for case in CASES {
        let mut field = begin(case, false);
        to_sound(&mut field, case.3);
        let last = field.source_foreground().unwrap().last();
        for _ in 0..60 {
            assert_eq!(step(&mut field, A | DOWN, true), (0, 0));
            assert_eq!(field.source_foreground().unwrap().last(), last);
            assert!(!field.unified_flags().get_flag(case.3));
        }
        for _ in 0..60 {
            assert_eq!(step(&mut field, A, false), (0, 0));
        }
        assert!(field.unified_flags().get_flag(case.3));
        assert_eq!(phase(&field), Some(Phase::OuterAck));
        assert!(field.pending_dialogue.as_ref().unwrap().holding_open());
        for _ in 0..5 {
            step(&mut field, 0, false);
        }
        assert!(field.displayed_field_dialogue().is_none());
        assert_eq!(
            (field.state.player.x, field.state.player.y),
            (case.1, case.2)
        );
    }
}

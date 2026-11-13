use super::*;

const AIDES: [(Case, u8); 3] = [
    (
        Case {
            map: MapId::Route2Gate,
            x: 2,
            y: 4,
            flag: EventFlag::EVENT_GOT_HM05,
            item: ItemId::Hm05,
            face: LEFT,
            flag_before_sound: false,
        },
        10,
    ),
    (
        Case {
            map: MapId::Route11Gate2F,
            x: 2,
            y: 7,
            flag: EventFlag::EVENT_GOT_ITEMFINDER,
            item: ItemId::Itemfinder,
            face: UP,
            flag_before_sound: false,
        },
        30,
    ),
    (
        Case {
            map: MapId::Route15Gate2F,
            x: 4,
            y: 3,
            flag: EventFlag::EVENT_GOT_EXP_ALL,
            item: ItemId::ExpAll,
            face: UP,
            flag_before_sound: false,
        },
        50,
    ),
];

fn boot(case: Case, owned: u8, full: bool) -> Session {
    let items: Vec<_> = if full {
        (2..=21).map(|id| (ItemId::from_id(id), 1)).collect()
    } else {
        Vec::new()
    };
    Session::boot_with_preparation(case, &items, |save| {
        for number in 1..=owned {
            save.game_data
                .pokedex
                .set_owned(Species::from_index_id(number));
        }
        assert_eq!(save.game_data.pokedex.owned_count(), u32::from(owned));
    })
}

fn answer(session: &mut Session, yes: bool) {
    for frame in 0..8000 {
        if session.game.overworld.pending_choice.is_some() {
            for attempt in 0..512 {
                if session.game.overworld.pending_choice.is_none() {
                    session.step(0);
                    return;
                }
                session.step(if attempt % 8 < 2 {
                    if yes {
                        A
                    } else {
                        B
                    }
                } else {
                    0
                });
            }
            panic!("aide choice did not accept ordinary input");
        }
        session.step(
            if frame % 8 < 2 && session.game.overworld.dialogue_needs_button() {
                B
            } else {
                0
            },
        );
    }
    panic!("aide question never reached YES/NO");
}

fn finish(session: &mut Session) {
    for _ in 0..8000 {
        assert!(
            !session.phase("SoundWait"),
            "unexpected extra aide reward fanfare"
        );
        if session.phase("OuterAck") {
            break;
        }
        session.step(if session.game.overworld.dialogue_needs_button() {
            B
        } else {
            0
        });
    }
    assert!(session.phase("OuterAck"));
    for _ in 0..32 {
        session.step(A);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_some());
    for _ in 0..32 {
        session.step(0);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
    assert!(session.game.overworld.script_engine_idle());
}

#[test]
fn aide_rewards_set_event_after_actual_receipt_audio_and_keep_description_confirmation() {
    for (case, owned) in AIDES {
        let mut session = boot(case, owned, false);
        assert_eq!(session.quantity(), 0);
        assert!(!session.received());
        answer(&mut session, true);
        session.reach("SoundWait");
        assert_eq!(session.quantity(), 1);
        assert!(!session.received(), "OaksAideScript has not returned yet");
        let mut audible = 0;
        for frame in 0..300 {
            audible += usize::from(session.step(if (8..11).contains(&frame) {
                B | DOWN
            } else {
                0
            }));
            if session.phase("SoundWait") {
                assert!(!session.received());
            }
        }
        assert!(audible > 10);
        assert!(session.received());
        assert!(session.game.overworld.displayed_field_dialogue().is_some());
        assert_eq!(
            (
                session.game.overworld.state.player.x,
                session.game.overworld.state.player.y
            ),
            (case.x.into(), case.y.into())
        );
        finish(&mut session);
        let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
        assert_eq!(saved.game_data.bag.item_quantity(case.item), 1);
        assert_ne!(
            saved.game_data.event_flags[case.flag.byte_offset()] & case.flag.bit_mask(),
            0
        );
        assert_eq!(std::fs::read(&session.path).unwrap(), session.original_sram);
        session.talk();
        finish(&mut session);
        assert_eq!(session.quantity(), 1);
    }
}

#[test]
fn aide_refusal_insufficient_dex_and_full_bag_leave_reward_available() {
    for (case, requirement) in AIDES {
        for (owned, full, yes) in [
            (requirement, false, false),
            (requirement - 1, false, true),
            (requirement, true, true),
        ] {
            let mut session = boot(case, owned, full);
            answer(&mut session, yes);
            finish(&mut session);
            assert!(!session.received());
            assert_eq!(session.quantity(), 0);
            assert_eq!(
                session.game.save_data.game_data.bag.count(),
                if full { 20 } else { 0 }
            );
            let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
            assert_eq!(
                saved.game_data.event_flags[case.flag.byte_offset()] & case.flag.bit_mask(),
                0
            );
            assert_eq!(saved.game_data.bag.item_quantity(case.item), 0);
            assert_eq!(std::fs::read(&session.path).unwrap(), session.original_sram);
        }
    }
}

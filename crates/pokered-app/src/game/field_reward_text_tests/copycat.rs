use super::*;

const COPYCAT: Case = Case {
    map: MapId::CopycatsHouse2F,
    x: 4,
    y: 4,
    flag: EventFlag::EVENT_GOT_TM31,
    item: ItemId::Tm31,
    face: UP,
    flag_before_sound: false,
};

fn dolls(session: &Session) -> u16 {
    session
        .game
        .save_data
        .game_data
        .bag
        .item_quantity(ItemId::PokeDoll)
}

fn visible(session: &Session) -> String {
    session
        .game
        .overworld
        .displayed_field_dialogue()
        .and_then(|text| text.get_display_text())
        .map(|(first, second)| format!("{first} {second}"))
        .unwrap_or_default()
}

#[test]
fn copycat_keeps_the_doll_until_the_receipt_and_explanation_are_confirmed() {
    let mut session = Session::boot_with_items(COPYCAT, &[(ItemId::PokeDoll, 2)]);
    let origin = (session.game.overworld.state.player.x, session.game.overworld.state.player.y);
    session.reach("SoundWait");
    assert_eq!(
        (session.quantity(), dolls(&session), session.received()),
        (1, 2, false)
    );
    let mut audible = 0;
    for frame in 0..300 {
        if session.step(if (8..11).contains(&frame) {
            B | DOWN
        } else {
            0
        }) {
            audible += 1;
        }
        assert_eq!((dolls(&session), session.received()), (2, false));
    }
    assert!(audible > 10);
    assert!(session.phase("Prompt(FirstParagraph)"));
    assert!(visible(&session).contains("received TM31"));
    session.reach("Prompt(Return)");
    assert!(visible(&session).contains("good POKeMON"));
    assert_eq!(
        (session.quantity(), dolls(&session), session.received()),
        (1, 2, false)
    );
    for _ in 0..32 {
        session.step(A);
    }
    assert_eq!(
        (session.quantity(), dolls(&session), session.received()),
        (1, 1, true)
    );
    assert!(session.game.overworld.displayed_field_dialogue().is_some());
    for _ in 0..32 {
        session.step(0);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
    assert_eq!(
        (
            session.game.overworld.state.player.x,
            session.game.overworld.state.player.y
        ),
        origin
    );
    let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
    assert_eq!(saved.game_data.bag.item_quantity(ItemId::PokeDoll), 1);
    assert_eq!(saved.game_data.bag.item_quantity(ItemId::Tm31), 1);
    assert_ne!(
        saved.game_data.event_flags[COPYCAT.flag.byte_offset()] & COPYCAT.flag.bit_mask(),
        0
    );
    session.talk();
    session.reach("OuterAck");
    assert_eq!((session.quantity(), dolls(&session)), (1, 1));
    session.step(B);
    for _ in 0..16 {
        session.step(0);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
}

#[test]
fn copycat_without_a_doll_returns_after_the_inner_prompt_and_a_release() {
    let mut session = Session::boot_with_items(COPYCAT, &[]);
    session.reach("Prompt(Return)");
    for _ in 0..32 {
        session.step(A);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_some());
    for _ in 0..32 {
        session.step(0);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
    assert_eq!(
        (session.quantity(), dolls(&session), session.received()),
        (0, 0, false)
    );
    assert!(session.game.overworld.script_engine_idle());
}

#[test]
fn copycat_full_bag_fails_before_removing_the_only_doll() {
    let mut items: Vec<_> = (2..=20).map(|id| (ItemId::from_id(id), 1)).collect();
    items.push((ItemId::PokeDoll, 1));
    let mut session = Session::boot_with_items(COPYCAT, &items);
    for _ in 0..2 {
        session.reach("Prompt(Return)");
        session.step(B);
        session.step(0);
    }
    session.reach("Prompt(Return)");
    assert!(visible(&session).contains("Don't you want this?"));
    assert_eq!(
        (session.quantity(), dolls(&session), session.received()),
        (0, 1, false)
    );
    assert_eq!(session.game.save_data.game_data.bag.count(), 20);
    for _ in 0..120 {
        session.step(0);
    }
    assert!(session.phase("Prompt(Return)"));
    session.step(B);
    for _ in 0..16 {
        session.step(0);
    }
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
    assert_eq!(
        (session.quantity(), dolls(&session), session.received()),
        (0, 1, false)
    );
    assert_eq!(session.game.save_data.game_data.bag.count(), 20);
}

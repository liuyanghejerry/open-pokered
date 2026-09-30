use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

fn battle(hp: u16) -> BattleScreen {
    let make = |species| {
        create_pokemon_with_moves(
            species,
            30,
            [0xff, 0xff],
            [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    let mut enemy = make(Species::Snorlax);
    enemy.hp = 100;
    enemy.max_hp = 100;
    let mut battle = BattleScreen::from_parties(
        true,
        &[make(Species::Charmander), make(Species::Squirtle)],
        &[enemy],
        None,
    );
    battle
        .battle_state
        .as_mut()
        .unwrap()
        .enemy
        .active_mon_mut()
        .hp = hp;
    battle.phase = BattlePhase::PlayerMenu;
    battle
}

#[test]
fn encouragement_uses_original_thresholds_and_zero_hp_exception() {
    for (hp, expected) in [
        (100, "Go! SQUIRTLE!"),
        (70, "Go! SQUIRTLE!"),
        (69, "Do it! SQUIRTLE!"),
        (40, "Do it! SQUIRTLE!"),
        (39, "Get'm! SQUIRTLE!"),
        (10, "Get'm! SQUIRTLE!"),
        (9, "The enemy's weak!\nGet'm! SQUIRTLE!"),
        (1, "The enemy's weak!\nGet'm! SQUIRTLE!"),
        (0, "Go! SQUIRTLE!"),
    ] {
        assert_eq!(switch_dialogue::send_out("SQUIRTLE", hp, 100), expected);
    }
    // Dividing max HP by four first can differ from hp * 100 / max_hp.
    assert_eq!(
        switch_dialogue::send_out("SQUIRTLE", 70, 103),
        "Go! SQUIRTLE!"
    );
    // Preserve both the divisor and quotient byte truncation.
    assert_eq!(
        switch_dialogue::send_out("SQUIRTLE", 600, 1100),
        "Get'm! SQUIRTLE!"
    );
}

#[test]
fn recall_uses_enemy_hp_lost_since_entry_not_player_health() {
    for (hp, praise) in [
        (100, " enough!"),
        (99, ""),
        (71, ""),
        (70, " OK!"),
        (31, " OK!"),
        (30, " good!"),
        (0, " good!"),
    ] {
        assert_eq!(
            switch_dialogue::recall("CHARMANDER", 100, hp, 100),
            format!("CHARMANDER{praise}\nCome back!")
        );
    }
    // Original subtraction wraps when an enemy heals above its entry HP.
    assert_eq!(
        switch_dialogue::recall("CHARMANDER", 80, 100, 500),
        "CHARMANDER OK!\nCome back!"
    );
}

#[test]
fn consecutive_switches_reset_the_entry_hp() {
    let mut b = battle(31);
    b.battle_state.as_mut().unwrap().player.active_mon_mut().hp = 1;
    let messages = b.apply_player_switch(1);
    assert_eq!(messages, ["CHARMANDER OK!\nCome back!", "Get'm! SQUIRTLE!"]);
    assert_eq!(b.last_switch_in_enemy_hp, 31);
    b.battle_state.as_mut().unwrap().enemy.active_mon_mut().hp = 9;
    assert_eq!(
        b.apply_player_switch(0),
        [
            "SQUIRTLE\nCome back!",
            "The enemy's weak!\nGet'm! CHARMANDER!",
        ]
    );
    assert_eq!(b.last_switch_in_enemy_hp, 9);
}

#[test]
fn initial_send_out_and_fainted_replacement_record_live_enemy_hp() {
    let mut b = battle(50);
    b.phase = BattlePhase::Intro {
        phase: IntroPhase::PlayerSendOut,
        wait_frames: 0,
    };
    b.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
    assert_eq!(b.current_message.as_deref(), Some("Do it! CHARMANDER!"));
    assert_eq!(b.last_switch_in_enemy_hp, 50);
    b.battle_state.as_mut().unwrap().enemy.active_mon_mut().hp = 5;
    b.force_switch_player(1);
    if let BattlePhase::ShowingText { messages, .. } = &b.phase {
        assert!(messages.join(" ").contains("The enemy's weak!"));
        assert!(!messages.join(" ").contains("Come back!"));
    } else {
        panic!("replacement must narrate send-out");
    }
    assert_eq!(b.last_switch_in_enemy_hp, 5);
}

#[test]
fn shift_switch_keeps_trainer_then_recall_then_encouragement_order() {
    let mut b = battle(50);
    b.apply_shift_switch(1);
    if let BattlePhase::ShowingText { messages, .. } = &b.phase {
        let text = messages.join(" ");
        assert!(text.find("sent out").unwrap() < text.find("OK!").unwrap());
        assert!(text.find("Come back!").unwrap() < text.find("Do it!").unwrap());
    } else {
        panic!("shift switch must show messages");
    }
}

#[test]
fn zero_hp_send_out_does_not_change_previous_entry_hp() {
    let mut b = battle(0);
    assert_eq!(b.apply_player_switch(1)[1], "Go! SQUIRTLE!");
    assert_eq!(b.last_switch_in_enemy_hp, 100);
}

#[test]
fn snapshot_restore_preserves_retreat_evaluation() {
    let mut b = battle(31);
    let snap = crate::snapshot::BattleSnapshot::capture(&b);
    b.last_switch_in_enemy_hp = 31;
    snap.restore_into(&mut b);
    assert_eq!(b.apply_player_switch(1)[0], "CHARMANDER OK!\nCome back!");
}

#[test]
fn new_variants_localize_before_pagination() {
    let mut b = battle(1);
    b.is_zh = true;
    b.apply_shift_switch(1);
    if let BattlePhase::ShowingText { messages, .. } = &b.phase {
        let text = messages.join(" ");
        assert!(text.contains("干得好"));
        assert!(text.contains("对手很虚弱"));
        assert!(text.contains("杰尼龟"));
        assert!(!text.contains("Get'm") && !text.contains("Come back"));
    } else {
        panic!("switch must show localized messages");
    }
}

use crate::options_menu::*;

fn press_down() -> OptionsInput {
    OptionsInput {
        down: true,
        ..OptionsInput::none()
    }
}

fn press_up() -> OptionsInput {
    OptionsInput {
        up: true,
        ..OptionsInput::none()
    }
}

fn press_left() -> OptionsInput {
    OptionsInput {
        left: true,
        ..OptionsInput::none()
    }
}

fn press_right() -> OptionsInput {
    OptionsInput {
        right: true,
        ..OptionsInput::none()
    }
}

fn press_a() -> OptionsInput {
    OptionsInput {
        a: true,
        ..OptionsInput::none()
    }
}

fn press_b() -> OptionsInput {
    OptionsInput {
        b: true,
        ..OptionsInput::none()
    }
}

fn press_start() -> OptionsInput {
    OptionsInput {
        start: true,
        ..OptionsInput::none()
    }
}

#[test]
fn initial_state() {
    let state = OptionsMenuState::new(GameOptions::default());
    assert_eq!(state.row, OptionsRow::TextSpeed);
    assert_eq!(state.options.text_speed, TextSpeed::Medium);
    assert_eq!(state.options.battle_animation, BattleAnimation::On);
    assert_eq!(state.options.battle_style, BattleStyle::Shift);
}

#[test]
fn close_with_b() {
    let mut state = OptionsMenuState::new(GameOptions::default());
    assert_eq!(state.tick(press_b()), OptionsMenuResult::Closed);
}

#[test]
fn close_with_start() {
    let mut state = OptionsMenuState::new(GameOptions::default());
    assert_eq!(state.tick(press_start()), OptionsMenuResult::Closed);
}

#[test]
fn close_with_a_on_cancel() {
    let mut state = OptionsMenuState::new(GameOptions::default());
    state.row = OptionsRow::Cancel;
    assert_eq!(state.tick(press_a()), OptionsMenuResult::Closed);
}

#[test]
fn a_on_non_cancel_stays_active() {
    let mut state = OptionsMenuState::new(GameOptions::default());
    assert_eq!(state.tick(press_a()), OptionsMenuResult::Active);
}

#[test]
fn navigate_down_through_all_rows() {
    let mut state = OptionsMenuState::new(GameOptions::default());
    assert_eq!(state.row, OptionsRow::TextSpeed);

    state.tick(press_down());
    assert_eq!(state.row, OptionsRow::BattleAnimation);

    state.tick(press_down());
    assert_eq!(state.row, OptionsRow::BattleStyle);

    state.tick(press_down());
    assert_eq!(state.row, OptionsRow::Cancel);

    state.tick(press_down());
    assert_eq!(state.row, OptionsRow::TextSpeed);
}

#[test]
fn navigate_up_wraps() {
    let mut state = OptionsMenuState::new(GameOptions::default());
    state.tick(press_up());
    assert_eq!(state.row, OptionsRow::Cancel);

    state.tick(press_up());
    assert_eq!(state.row, OptionsRow::BattleStyle);
}

#[test]
fn text_speed_cycle_right() {
    let mut state = OptionsMenuState::new(GameOptions::default());
    assert_eq!(state.options.text_speed, TextSpeed::Medium);

    state.tick(press_right());
    assert_eq!(state.options.text_speed, TextSpeed::Slow);

    state.tick(press_right());
    assert_eq!(state.options.text_speed, TextSpeed::Slow);
}

#[test]
fn text_speed_cycle_left() {
    let mut state = OptionsMenuState::new(GameOptions::default());

    state.tick(press_left());
    assert_eq!(state.options.text_speed, TextSpeed::Fast);

    state.tick(press_left());
    assert_eq!(state.options.text_speed, TextSpeed::Fast);
}

#[test]
fn text_speed_full_cycle() {
    let mut state = OptionsMenuState::new(GameOptions {
        text_speed: TextSpeed::Fast,
        ..GameOptions::default()
    });
    state.tick(press_right());
    assert_eq!(state.options.text_speed, TextSpeed::Medium);
    state.tick(press_right());
    assert_eq!(state.options.text_speed, TextSpeed::Slow);
    state.tick(press_left());
    assert_eq!(state.options.text_speed, TextSpeed::Medium);
    state.tick(press_left());
    assert_eq!(state.options.text_speed, TextSpeed::Fast);
}

#[test]
fn battle_animation_toggle() {
    let mut state = OptionsMenuState::new(GameOptions::default());
    state.row = OptionsRow::BattleAnimation;

    assert_eq!(state.options.battle_animation, BattleAnimation::On);
    state.tick(press_left());
    assert_eq!(state.options.battle_animation, BattleAnimation::Off);
    state.tick(press_right());
    assert_eq!(state.options.battle_animation, BattleAnimation::On);
}

#[test]
fn battle_style_toggle() {
    let mut state = OptionsMenuState::new(GameOptions::default());
    state.row = OptionsRow::BattleStyle;

    assert_eq!(state.options.battle_style, BattleStyle::Shift);
    state.tick(press_right());
    assert_eq!(state.options.battle_style, BattleStyle::Set);
    state.tick(press_left());
    assert_eq!(state.options.battle_style, BattleStyle::Shift);
}

#[test]
fn left_right_on_cancel_does_nothing() {
    let mut state = OptionsMenuState::new(GameOptions::default());
    state.row = OptionsRow::Cancel;
    let opts_before = state.options;
    state.tick(press_left());
    assert_eq!(state.options, opts_before);
    state.tick(press_right());
    assert_eq!(state.options, opts_before);
}

#[test]
fn options_byte_default() {
    let opts = GameOptions::default();
    let b = opts.to_byte();
    assert_eq!(b & TEXT_DELAY_MASK, TEXT_DELAY_MEDIUM);
    assert_eq!(b & (1 << BIT_BATTLE_ANIMATION), 0);
    assert_eq!(b & (1 << BIT_BATTLE_SHIFT), 0);
}

#[test]
fn options_byte_all_non_default() {
    let opts = GameOptions {
        text_speed: TextSpeed::Slow,
        battle_animation: BattleAnimation::Off,
        battle_style: BattleStyle::Set,
    };
    let b = opts.to_byte();
    assert_eq!(b & TEXT_DELAY_MASK, TEXT_DELAY_SLOW);
    assert_ne!(b & (1 << BIT_BATTLE_ANIMATION), 0);
    assert_ne!(b & (1 << BIT_BATTLE_SHIFT), 0);
}

#[test]
fn options_byte_roundtrip() {
    let test_cases = [
        GameOptions::default(),
        GameOptions {
            text_speed: TextSpeed::Fast,
            battle_animation: BattleAnimation::On,
            battle_style: BattleStyle::Shift,
        },
        GameOptions {
            text_speed: TextSpeed::Slow,
            battle_animation: BattleAnimation::Off,
            battle_style: BattleStyle::Set,
        },
        GameOptions {
            text_speed: TextSpeed::Medium,
            battle_animation: BattleAnimation::Off,
            battle_style: BattleStyle::Shift,
        },
    ];

    for opts in &test_cases {
        let b = opts.to_byte();
        let decoded = GameOptions::from_byte(b);
        assert_eq!(decoded.text_speed, opts.text_speed);
        assert_eq!(decoded.battle_animation, opts.battle_animation);
        assert_eq!(decoded.battle_style, opts.battle_style);
    }
}

#[test]
fn from_byte_specific_values() {
    let fast_on_shift = GameOptions::from_byte(TEXT_DELAY_FAST);
    assert_eq!(fast_on_shift.text_speed, TextSpeed::Fast);
    assert_eq!(fast_on_shift.battle_animation, BattleAnimation::On);
    assert_eq!(fast_on_shift.battle_style, BattleStyle::Shift);

    let slow_off_set = GameOptions::from_byte(
        TEXT_DELAY_SLOW | (1 << BIT_BATTLE_ANIMATION) | (1 << BIT_BATTLE_SHIFT),
    );
    assert_eq!(slow_off_set.text_speed, TextSpeed::Slow);
    assert_eq!(slow_off_set.battle_animation, BattleAnimation::Off);
    assert_eq!(slow_off_set.battle_style, BattleStyle::Set);
}

#[test]
fn text_speed_delay_values() {
    assert_eq!(TextSpeed::Fast.delay_frames(), 1);
    assert_eq!(TextSpeed::Medium.delay_frames(), 3);
    assert_eq!(TextSpeed::Slow.delay_frames(), 5);
}

#[test]
fn text_speed_from_delay() {
    assert_eq!(TextSpeed::from_delay(1), TextSpeed::Fast);
    assert_eq!(TextSpeed::from_delay(3), TextSpeed::Medium);
    assert_eq!(TextSpeed::from_delay(5), TextSpeed::Slow);
    assert_eq!(TextSpeed::from_delay(0), TextSpeed::Medium);
    assert_eq!(TextSpeed::from_delay(7), TextSpeed::Medium);
}

#[test]
fn options_menu_to_options_byte() {
    let mut state = OptionsMenuState::new(GameOptions::default());
    state.tick(press_right());
    assert_eq!(state.options.text_speed, TextSpeed::Slow);

    let b = state.to_options_byte();
    assert_eq!(b & TEXT_DELAY_MASK, TEXT_DELAY_SLOW);
}

#[test]
fn options_row_y_coordinates() {
    assert_eq!(OptionsRow::TextSpeed.y_coord(), 3);
    assert_eq!(OptionsRow::BattleAnimation.y_coord(), 8);
    assert_eq!(OptionsRow::BattleStyle.y_coord(), 13);
    assert_eq!(OptionsRow::Cancel.y_coord(), 16);
}

#[test]
fn no_input_stays_active() {
    let mut state = OptionsMenuState::new(GameOptions::default());
    let result = state.tick(OptionsInput::none());
    assert_eq!(result, OptionsMenuResult::Active);
    assert_eq!(state.row, OptionsRow::TextSpeed);
    assert_eq!(state.options, GameOptions::default());
}

#[test]
fn from_existing_options_byte() {
    let byte = TEXT_DELAY_FAST | (1 << BIT_BATTLE_ANIMATION);
    let opts = GameOptions::from_byte(byte);
    let state = OptionsMenuState::new(opts);
    assert_eq!(state.options.text_speed, TextSpeed::Fast);
    assert_eq!(state.options.battle_animation, BattleAnimation::Off);
    assert_eq!(state.options.battle_style, BattleStyle::Shift);
}

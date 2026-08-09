use super::naming_screen::*;

fn input_a() -> NamingInput {
    NamingInput {
        a: true,
        ..NamingInput::none()
    }
}

fn input_b() -> NamingInput {
    NamingInput {
        b: true,
        ..NamingInput::none()
    }
}

fn input_up() -> NamingInput {
    NamingInput {
        up: true,
        ..NamingInput::none()
    }
}

fn input_down() -> NamingInput {
    NamingInput {
        down: true,
        ..NamingInput::none()
    }
}

fn input_left() -> NamingInput {
    NamingInput {
        left: true,
        ..NamingInput::none()
    }
}

fn input_right() -> NamingInput {
    NamingInput {
        right: true,
        ..NamingInput::none()
    }
}

fn input_start() -> NamingInput {
    NamingInput {
        start: true,
        ..NamingInput::none()
    }
}

fn input_select() -> NamingInput {
    NamingInput {
        select: true,
        ..NamingInput::none()
    }
}

#[test]
fn initial_state_player() {
    let state = NamingScreenState::new(NamingScreenType::Player);
    assert_eq!(state.screen_type(), NamingScreenType::Player);
    assert_eq!(state.name(), "");
    assert_eq!(state.max_length(), PLAYER_NAME_MAX);
    assert!(!state.is_lowercase());
    assert_eq!(state.cursor_row(), 0);
    assert_eq!(state.cursor_col(), 0);
}

#[test]
fn initial_state_pokemon() {
    let state = NamingScreenState::new(NamingScreenType::Pokemon);
    assert_eq!(state.max_length(), MON_NAME_MAX);
}

#[test]
fn initial_state_rival() {
    let state = NamingScreenState::new(NamingScreenType::Rival);
    assert_eq!(state.max_length(), PLAYER_NAME_MAX);
}

// --- Cursor navigation ---

#[test]
fn cursor_move_right_wraps() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    for _ in 0..GRID_COLS {
        state.update_frame(input_right(), false);
    }
    assert_eq!(state.cursor_col(), 0);
}

#[test]
fn cursor_move_left_wraps() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    state.update_frame(input_left(), false);
    assert_eq!(state.cursor_col(), GRID_COLS - 1);
}

#[test]
fn cursor_move_down_wraps() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    for _ in 0..TOTAL_ROWS {
        state.update_frame(input_down(), false);
    }
    assert_eq!(state.cursor_row(), 0);
}

#[test]
fn cursor_move_up_wraps_to_case_row() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    state.update_frame(input_up(), false);
    assert_eq!(state.cursor_row(), 5);
    assert_eq!(state.cursor_col(), 0);
}

#[test]
fn case_row_blocks_left_right() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    // Navigate to case row (row 5)
    state.update_frame(input_up(), false);
    assert_eq!(state.cursor_row(), 5);
    let col_before = state.cursor_col();
    state.update_frame(input_right(), false);
    assert_eq!(state.cursor_col(), col_before);
    state.update_frame(input_left(), false);
    assert_eq!(state.cursor_col(), col_before);
}

#[test]
fn entering_case_row_via_down_forces_col_zero() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    // Move to col 4
    for _ in 0..4 {
        state.update_frame(input_right(), false);
    }
    assert_eq!(state.cursor_col(), 4);
    // Move down to row 5 (case row)
    for _ in 0..5 {
        state.update_frame(input_down(), false);
    }
    assert_eq!(state.cursor_row(), 5);
    assert_eq!(state.cursor_col(), 0);
}

// --- Character input ---

#[test]
fn pressing_a_adds_character() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    // Cursor at (0,0) = 'A' in uppercase
    let result = state.update_frame(input_a(), false);
    assert_eq!(result, NamingScreenResult::Editing);
    assert_eq!(state.name(), "A");
}

#[test]
fn pressing_a_on_lowercase() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    state.update_frame(input_select(), false); // switch to lowercase
    let result = state.update_frame(input_a(), false);
    assert_eq!(result, NamingScreenResult::Editing);
    assert_eq!(state.name(), "a");
}

#[test]
fn typing_multiple_characters() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    // Type "AB" by pressing A at (0,0) then moving right and pressing A at (0,1)
    state.update_frame(input_a(), false); // A
    state.update_frame(input_right(), false);
    state.update_frame(input_a(), false); // B
    assert_eq!(state.name(), "AB");
}

#[test]
fn max_length_player_enforced() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    // Type 7 A's
    for _ in 0..PLAYER_NAME_MAX {
        state.update_frame(input_a(), false);
    }
    assert_eq!(state.name().len(), PLAYER_NAME_MAX);
    // Cursor forced to ED tile
    assert_eq!(state.cursor_row(), 4);
    assert_eq!(state.cursor_col(), 8);
    // 8th press should not add
    state.update_frame(input_a(), false); // This hits ED tile → submit
    assert_eq!(state.name().len(), PLAYER_NAME_MAX);
}

#[test]
fn max_length_pokemon_enforced() {
    let mut state = NamingScreenState::new(NamingScreenType::Pokemon);
    for _ in 0..MON_NAME_MAX {
        state.update_frame(input_a(), false);
    }
    assert_eq!(state.name().len(), MON_NAME_MAX);
    assert_eq!(state.cursor_row(), 4);
    assert_eq!(state.cursor_col(), 8);
}

// --- Backspace ---

#[test]
fn pressing_b_deletes_last_char() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    state.update_frame(input_a(), false); // 'A'
    state.update_frame(input_right(), false);
    state.update_frame(input_a(), false); // 'B'
    assert_eq!(state.name(), "AB");
    state.update_frame(input_b(), false);
    assert_eq!(state.name(), "A");
}

#[test]
fn pressing_b_on_empty_does_nothing() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    let result = state.update_frame(input_b(), false);
    assert_eq!(result, NamingScreenResult::Editing);
    assert_eq!(state.name(), "");
}

// --- Case toggle ---

#[test]
fn select_toggles_case() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    assert!(!state.is_lowercase());
    state.update_frame(input_select(), false);
    assert!(state.is_lowercase());
    state.update_frame(input_select(), false);
    assert!(!state.is_lowercase());
}

#[test]
fn pressing_a_on_case_row_toggles_case() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    // Go to case row
    state.update_frame(input_up(), false);
    assert_eq!(state.cursor_row(), 5);
    assert!(!state.is_lowercase());
    state.update_frame(input_a(), false);
    assert!(state.is_lowercase());
}

// --- Submit ---

#[test]
fn start_submits_name() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    state.update_frame(input_a(), false); // 'A'
    let result = state.update_frame(input_start(), false);
    assert_eq!(result, NamingScreenResult::Submitted("A".to_string()));
}

#[test]
fn start_on_empty_cancels() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    let result = state.update_frame(input_start(), false);
    assert_eq!(result, NamingScreenResult::Cancelled);
}

#[test]
fn pressing_a_on_ed_tile_submits() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    state.update_frame(input_a(), false); // 'A'
                                   // Navigate to ED tile (row 4, col 8)
    for _ in 0..4 {
        state.update_frame(input_down(), false);
    }
    for _ in 0..8 {
        state.update_frame(input_right(), false);
    }
    assert_eq!(state.cursor_row(), 4);
    assert_eq!(state.cursor_col(), 8);
    let result = state.update_frame(input_a(), false);
    assert_eq!(result, NamingScreenResult::Submitted("A".to_string()));
}

#[test]
fn pressing_a_on_ed_tile_empty_cancels() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    // Navigate to ED tile
    for _ in 0..4 {
        state.update_frame(input_down(), false);
    }
    for _ in 0..8 {
        state.update_frame(input_right(), false);
    }
    let result = state.update_frame(input_a(), false);
    assert_eq!(result, NamingScreenResult::Cancelled);
}

// --- Alphabet data ---

#[test]
fn upper_alphabet_first_row() {
    use pokered_data::charmap::naming_tiles;
    assert_eq!(
        UPPER_ALPHABET[0],
        [0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88]
    );
}

#[test]
fn lower_alphabet_first_row() {
    assert_eq!(
        LOWER_ALPHABET[0],
        [0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8]
    );
}

#[test]
fn ed_tile_at_correct_position() {
    use pokered_data::charmap::naming_tiles;
    assert_eq!(UPPER_ALPHABET[4][8], naming_tiles::ED_TILE);
    assert_eq!(LOWER_ALPHABET[4][8], naming_tiles::ED_TILE);
}

#[test]
fn current_alphabet_switches() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    assert_eq!(state.current_alphabet()[0][0], 0x80);
    state.update_frame(input_select(), false);
    assert_eq!(state.current_alphabet()[0][0], 0xA0);
}

// --- Full name entry scenario ---

#[test]
fn type_red_and_submit() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    // R is at row 1, col 8
    state.update_frame(input_down(), false);
    for _ in 0..8 {
        state.update_frame(input_right(), false);
    }
    state.update_frame(input_a(), false); // 'R'
                                   // E is at row 0, col 4
    state.update_frame(input_up(), false);
    // We're now at row 0. Need to go to col 4. Currently at col 8.
    // Wrap left: col 8→7→6→5→4
    for _ in 0..4 {
        state.update_frame(input_left(), false);
    }
    state.update_frame(input_a(), false); // 'E'
                                   // D is at row 0, col 3
    state.update_frame(input_left(), false);
    state.update_frame(input_a(), false); // 'D'
    assert_eq!(state.name(), "RED");
    let result = state.update_frame(input_start(), false);
    assert_eq!(result, NamingScreenResult::Submitted("RED".to_string()));
}

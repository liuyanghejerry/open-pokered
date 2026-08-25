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

// --- Pinyin input ---
// Cursor space in pinyin mode: letter rows 0..=2 (A-Z only) followed by up
// to two candidate-strip rows. See the naming_screen module docs.

/// Letter (row, col) coordinates on the uppercase grid rows 0..=2.
const A: (usize, usize) = (0, 0);
const G: (usize, usize) = (0, 6);
const H: (usize, usize) = (0, 7);
const I: (usize, usize) = (0, 8);
const N: (usize, usize) = (1, 4);
const O: (usize, usize) = (1, 5);
const Z: (usize, usize) = (2, 7);
const SPACE: (usize, usize) = (2, 8);

fn pinyin_state(screen_type: NamingScreenType) -> NamingScreenState {
    let mut state = NamingScreenState::new(screen_type);
    state.update_frame(input_select(), true); // is_zh → Pinyin mode
    assert_eq!(state.input_mode, InputMode::Pinyin);
    state
}

/// Navigate the pinyin cursor to a letter-row (row, col). Rows and columns
/// wrap, so pressing up/right repeatedly always reaches the target.
fn nav_to_letter(state: &mut NamingScreenState, (row, col): (usize, usize)) {
    for _ in 0..TOTAL_ROWS {
        if state.cursor_row() == row {
            break;
        }
        state.update_frame(input_up(), true);
    }
    assert_eq!(state.cursor_row(), row, "row navigation failed");
    for _ in 0..GRID_COLS {
        if state.cursor_col() == col {
            break;
        }
        state.update_frame(input_right(), true);
    }
    assert_eq!(state.cursor_col(), col, "col navigation failed");
}

fn type_letters(state: &mut NamingScreenState, letters: &[(usize, usize)]) {
    for &pos in letters {
        nav_to_letter(state, pos);
        state.update_frame(input_a(), true);
    }
}

/// Move the cursor down into the candidate strip (from any letter row).
fn enter_strip(state: &mut NamingScreenState) {
    for _ in 0..TOTAL_ROWS {
        if state.cursor_row() >= PINYIN_GRID_ROWS {
            return;
        }
        state.update_frame(input_down(), true);
    }
    panic!("cursor never reached the candidate strip");
}

#[test]
fn pinyin_type_letter_into_buffer() {
    let mut state = pinyin_state(NamingScreenType::Player);
    // Cursor at (0,0) = 'A'; pressing A must append 'a' to the buffer and
    // list the candidates without touching the name.
    let result = state.update_frame(input_a(), true);
    assert_eq!(result, NamingScreenResult::Editing);
    assert_eq!(state.pinyin_buf, "a");
    assert_eq!(state.pinyin_candidates.first(), Some(&'阿'));
    assert_eq!(state.name(), "");
    assert_eq!((state.cursor_row(), state.cursor_col()), A, "typing keeps the cursor on the letter");
}

#[test]
fn pinyin_multiletter_syllable() {
    let mut state = pinyin_state(NamingScreenType::Player);
    type_letters(&mut state, &[N, I]);
    assert_eq!(state.pinyin_buf, "ni");
    assert_eq!(&state.pinyin_candidates[..4], &['你', '尼', '泥', '逆']);
    assert_eq!(state.name(), "");
}

#[test]
fn pinyin_letter_does_not_auto_submit_or_touch_name() {
    let mut state = pinyin_state(NamingScreenType::Player);
    let result = state.update_frame(input_a(), true);
    assert_eq!(result, NamingScreenResult::Editing);
    assert_eq!(state.name(), "");
}

#[test]
fn pinyin_non_letter_tile_is_ignored() {
    let mut state = pinyin_state(NamingScreenType::Player);
    type_letters(&mut state, &[Z, H, O, N, G, SPACE]);
    assert_eq!(state.pinyin_buf, "zhong", "space must not enter the buffer");
}

#[test]
fn pinyin_commit_candidate_from_strip() {
    let mut state = pinyin_state(NamingScreenType::Player);
    type_letters(&mut state, &[N, I]);
    enter_strip(&mut state);
    assert_eq!(state.cursor_row(), PINYIN_GRID_ROWS);
    assert_eq!(state.cursor_col(), 0);
    let result = state.update_frame(input_a(), true);
    assert_eq!(result, NamingScreenResult::Editing);
    assert_eq!(state.name(), "你");
    assert!(state.pinyin_buf.is_empty());
    assert!(state.pinyin_candidates.is_empty());
    // Cursor returns to the last typed letter, ready for the next syllable.
    assert_eq!((state.cursor_row(), state.cursor_col()), I);
}

#[test]
fn pinyin_commit_candidate_moved_within_strip() {
    let mut state = pinyin_state(NamingScreenType::Player);
    type_letters(&mut state, &[Z, H]); // 9 candidates → two strip lines
    assert_eq!(state.pinyin_candidates.len(), 9);
    enter_strip(&mut state);
    state.update_frame(input_right(), true); // second candidate
    state.update_frame(input_a(), true);
    assert_eq!(state.name(), "炸");

    // Second line: candidates 6..=8 (展张章) live on strip row 1.
    let mut state = pinyin_state(NamingScreenType::Player);
    type_letters(&mut state, &[Z, H]);
    enter_strip(&mut state);
    state.update_frame(input_down(), true); // strip line 1, col 0 → 展
    assert_eq!(state.cursor_row(), PINYIN_GRID_ROWS + 1);
    state.update_frame(input_a(), true);
    assert_eq!(state.name(), "展");
}

#[test]
fn pinyin_strip_columns_clamp_to_populated_slots() {
    let mut state = pinyin_state(NamingScreenType::Player);
    state.update_frame(input_a(), true); // "a" → 6 candidates, one line
    enter_strip(&mut state);
    for _ in 0..GRID_COLS {
        state.update_frame(input_right(), true);
    }
    // 9 rights wrap within the 6 populated slots → col 3.
    assert_eq!(state.cursor_col(), 3);
}

#[test]
fn pinyin_b_pops_buffer_letter_and_clears_candidates() {
    let mut state = pinyin_state(NamingScreenType::Player);
    type_letters(&mut state, &[N]);
    assert!(!state.pinyin_candidates.is_empty());
    state.update_frame(input_b(), true);
    assert!(state.pinyin_buf.is_empty());
    assert!(state.pinyin_candidates.is_empty(), "empty buffer must list no candidates");
}

#[test]
fn pinyin_b_pops_name_when_buffer_empty() {
    let mut state = pinyin_state(NamingScreenType::Player);
    type_letters(&mut state, &[A]);
    enter_strip(&mut state);
    state.update_frame(input_a(), true); // commit 阿
    assert_eq!(state.name(), "阿");
    state.update_frame(input_b(), true); // buffer empty → pop the name
    assert_eq!(state.name(), "");
}

#[test]
fn pinyin_full_cjk_name_reaches_char_limit_not_byte_limit() {
    // Regression: the limit used name.len() (bytes), capping CJK names at
    // 7/3 ≈ 2-3 characters. It must count characters.
    let mut state = pinyin_state(NamingScreenType::Player);
    for _ in 0..7 {
        state.update_frame(input_a(), true); // type 'a' (cursor back at A)
        enter_strip(&mut state);
        state.update_frame(input_a(), true); // commit 阿
    }
    assert_eq!(state.name().chars().count(), PLAYER_NAME_MAX);
    assert_eq!(state.name().len(), 21, "7 CJK chars are 21 UTF-8 bytes");
    // Like the alphabet path: name full → leave pinyin, cursor on the ED tile.
    assert_eq!(state.input_mode, InputMode::Alphabet);
    assert_eq!((state.cursor_row(), state.cursor_col()), (4, 8));
    // A now submits the completed name.
    let result = state.update_frame(input_a(), true);
    assert_eq!(result, NamingScreenResult::Submitted("阿阿阿阿阿阿阿".to_string()));
}

#[test]
fn pinyin_pokemon_limit_is_ten_chars() {
    let mut state = pinyin_state(NamingScreenType::Pokemon);
    for _ in 0..MON_NAME_MAX {
        state.update_frame(input_a(), true);
        enter_strip(&mut state);
        state.update_frame(input_a(), true);
    }
    assert_eq!(state.name().chars().count(), MON_NAME_MAX);
    assert_eq!(state.name().len(), 30, "10 CJK chars are 30 UTF-8 bytes");
    assert_eq!(state.input_mode, InputMode::Alphabet);
}

#[test]
fn pinyin_buffer_caps_at_longest_syllable() {
    let mut state = pinyin_state(NamingScreenType::Player);
    for _ in 0..8 {
        state.update_frame(input_a(), true);
    }
    assert_eq!(state.pinyin_buf.len(), 6, "buffer must cap at the longest syllable");
}

#[test]
fn pinyin_ed_tile_not_reachable() {
    // The ED tile and the specials/case rows do not exist in the pinyin
    // cursor space; the cursor never leaves rows 0..=PINYIN_GRID_ROWS+1.
    let mut state = pinyin_state(NamingScreenType::Player);
    type_letters(&mut state, &[Z, H, O, N, G]); // "zhong" → candidates exist
    for _ in 0..TOTAL_ROWS + 2 {
        state.update_frame(input_down(), true);
        assert!(state.cursor_row() <= PINYIN_GRID_ROWS + 1);
    }
    // A on whatever the cursor points at (letter or candidate) never submits.
    let result = state.update_frame(input_a(), true);
    assert_eq!(result, NamingScreenResult::Editing);
}

#[test]
fn pinyin_start_submits_from_pinyin_mode() {
    let mut state = pinyin_state(NamingScreenType::Player);
    type_letters(&mut state, &[A]);
    enter_strip(&mut state);
    state.update_frame(input_a(), true); // commit 阿
    let result = state.update_frame(input_start(), true);
    assert_eq!(result, NamingScreenResult::Submitted("阿".to_string()));
}

#[test]
fn pinyin_select_returns_cursor_to_last_typed_letter() {
    let mut state = pinyin_state(NamingScreenType::Player);
    type_letters(&mut state, &[N]);
    enter_strip(&mut state);
    assert_eq!(state.cursor_row(), PINYIN_GRID_ROWS);
    state.update_frame(input_select(), true); // back to alphabet
    assert_eq!(state.input_mode, InputMode::Alphabet);
    assert_eq!((state.cursor_row(), state.cursor_col()), N);
}

#[test]
fn pinyin_entering_from_lower_alphabet_row_resets_cursor() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    // Move to the case row (row 5), which doesn't exist in pinyin mode.
    state.update_frame(input_up(), true);
    assert_eq!(state.cursor_row(), TOTAL_ROWS - 1);
    state.update_frame(input_select(), true); // → pinyin
    assert_eq!((state.cursor_row(), state.cursor_col()), (0, 0));
}

#[test]
fn pinyin_stale_strip_cursor_returns_to_letters() {
    // Cursor sits in the strip; B empties the buffer → the strip vanishes
    // and the cursor must fall back to the last typed letter.
    let mut state = pinyin_state(NamingScreenType::Player);
    type_letters(&mut state, &[A]);
    enter_strip(&mut state);
    state.update_frame(input_b(), true);
    assert_eq!((state.cursor_row(), state.cursor_col()), A);
}

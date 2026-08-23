use pokered_core::game_state::Lang;
// Behavior-locking tests for the migrated menu draw functions in
// `pokered_ui::menus::*`. These assert the exact sequence of `Painter`
// calls each menu emits, so future refactors of the engine cannot silently
// change menu layout. They complement the framebuffer-level
// `visual_verify_party_screen` byte-equality gate in `pokered-app`.

use pokered_core::game_state::SaveFileSummary;
use pokered_core::main_menu::MainMenuState;
use pokered_core::options_menu::{
    BattleAnimation, BattleStyle, GameOptions, OptionsMenuState, OptionsRow, TextSpeed,
};
use pokered_core::save_menu::{SaveMenuState, SavePhase, SaveScreenInfo, YesNoChoice};
use pokered_core::start_menu::StartMenuState;
use pokered_data::ui_layout::schema::{MAIN_DEFAULT_LAYOUT, START_DEFAULT_LAYOUT, NAMING_DEFAULT_LAYOUT, OPTIONS_DEFAULT_LAYOUT, SAVE_DEFAULT_LAYOUT, SAVE_ASK_PROMPT_LAYOUT, PARTY_DEFAULT_LAYOUT};
use pokered_ui::{menus, Painter, Rgba, TilePos, TileRect, Ui};

#[derive(Debug, PartialEq, Eq)]
enum Op {
    Clear(Rgba),
    Box(TileRect, Rgba),
    Text(TilePos, String, Rgba),
    Glyph(TilePos, char, Rgba),
    PixelRect(u32, u32, u32, u32, Rgba),
    GbTile(TilePos, u8, String, Rgba),
}

#[derive(Default)]
struct Recorder {
    ops: Vec<Op>,
}

impl Painter for Recorder {
    fn clear(&mut self, color: Rgba) {
        self.ops.push(Op::Clear(color));
    }
    fn draw_text_box(&mut self, rect: TileRect, color: Rgba) {
        self.ops.push(Op::Box(rect, color));
    }
    fn draw_text(&mut self, pos: TilePos, text: &str, color: Rgba) {
        self.ops.push(Op::Text(pos, text.to_string(), color));
    }
    fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba) {
        self.ops.push(Op::Glyph(pos, glyph, color));
    }
    fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, color: Rgba) {
        self.ops.push(Op::PixelRect(px, py, pw, ph, color));
    }
    fn draw_gb_tile(&mut self, pos: TilePos, tile_id: u8, fallback: &str, color: Rgba) {
        self.ops.push(Op::GbTile(pos, tile_id, fallback.to_string(), color));
    }
}

#[test]
fn main_menu_no_save_uses_short_box_and_filled_triangle_cursor() {
    let state = MainMenuState::new(None);
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    menus::main::draw(&state, &MAIN_DEFAULT_LAYOUT, &mut ui, Lang::default());

    // First op: clear to white.
    assert_eq!(rec.ops[0], Op::Clear(Rgba::INK_WHITE));

    // No-save menu has 2 items → box height 4.
    let labels = state.item_labels();
    assert_eq!(labels.len(), 2);
    assert_eq!(rec.ops[1], Op::Box(TileRect::new(0, 0, 13, 7), Rgba::INK_BLACK));

    // Labels at absolute tile (2, 2 + 2i).
    let texts: Vec<_> = rec.ops.iter().filter_map(|op| match op {
        Op::Text(p, s, _) => Some((p.tx, p.ty, s.clone())),
        _ => None,
    }).collect();
    let expected: Vec<_> = labels.iter().enumerate()
        .map(|(i, l)| (2_u32, 2 + (i as u32 * 2), l.to_string()))
        .collect();
    assert_eq!(texts, expected);

    // Cursor is the filled-triangle glyph '\u{25B6}', not '>',
    // at absolute tile (1, 2 + 2 * cursor).
    let glyphs: Vec<_> = rec.ops.iter().filter_map(|op| match op {
        Op::Glyph(p, ch, _) => Some((p.tx, p.ty, *ch)),
        _ => None,
    }).collect();
    assert_eq!(glyphs, vec![(1, 2 + 2 * state.cursor as u32, '\u{25B6}')]);
}

#[test]
fn main_menu_with_save_uses_taller_box() {
    let state = MainMenuState::new(Some(SaveFileSummary {
        player_name: b"RED".to_vec(),
        badges: 0,
        pokedex_owned: 0,
        play_time_hours: 0,
        play_time_minutes: 0,
        play_time_seconds: 0,
        player_id: 0,
    }));
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    menus::main::draw(&state, &MAIN_DEFAULT_LAYOUT, &mut ui, Lang::default());

    // With save → >2 items → box height 6.
    let labels = state.item_labels();
    assert!(labels.len() > 2, "save-present menu should have > 2 items, got {}", labels.len());
    let boxes: Vec<_> = rec.ops.iter().filter_map(|op| match op {
        Op::Box(r, _) => Some(*r),
        _ => None,
    }).collect();
    assert_eq!(boxes, vec![TileRect::new(0, 0, 13, 9)]);
}

#[test]
fn start_menu_box_anchored_top_right_with_caret_cursor() {
    // 3-arg constructor: has_pokedex, has_pokemon, is_link_connected.
    let state = StartMenuState::new(true, true, false);
    let player_name = "RED";
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    menus::start::draw(&state, player_name, &START_DEFAULT_LAYOUT, &mut ui, Lang::default());

    let labels = state.item_labels(player_name);
    let expected_h = (labels.len() as u32) * 2;

    let boxes: Vec<_> = rec.ops.iter().filter_map(|op| match op {
        Op::Box(r, _) => Some(*r),
        _ => None,
    }).collect();
    // Box width is driven by the widest label, counted in *characters* (the
    // CJK-aware fix for the menu width: "POKéDEX" is 7 chars, not 8 bytes).
    assert_eq!(boxes, vec![TileRect::new(10, 0, 11, 17)]);

    // Labels at absolute tile (12, 2 + 2i).
    let texts: Vec<_> = rec.ops.iter().filter_map(|op| match op {
        Op::Text(p, s, _) => Some((p.tx, p.ty, s.clone())),
        _ => None,
    }).collect();
    let expected_texts: Vec<_> = labels.iter().enumerate()
        .map(|(i, l)| (12_u32, 2 + (i as u32 * 2), l.as_str().to_string()))
        .collect();
    assert_eq!(texts, expected_texts);

    let glyphs: Vec<_> = rec.ops.iter().filter_map(|op| match op {
        Op::Glyph(p, ch, _) => Some((p.tx, p.ty, *ch)),
        _ => None,
    }).collect();
    assert_eq!(glyphs, vec![(11, 2 + 2 * state.cursor() as u32, '\u{25B6}')]);
}

#[test]
fn options_menu_three_setting_boxes_and_cancel_outside() {
    let state = OptionsMenuState::new(GameOptions {
        text_speed: TextSpeed::Medium,
        battle_animation: BattleAnimation::On,
        battle_style: BattleStyle::Shift,
    });
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    menus::options::draw(&state, &OPTIONS_DEFAULT_LAYOUT, &mut ui, Lang::default());

    assert_eq!(rec.ops[0], Op::Clear(Rgba::INK_WHITE));

    let boxes: Vec<_> = rec.ops.iter().filter_map(|op| match op {
        Op::Box(r, _) => Some(*r),
        _ => None,
    }).collect();
    assert_eq!(boxes, vec![
        TileRect::new(0, 0, 20, 5),
        TileRect::new(0, 5, 20, 5),
        TileRect::new(0, 10, 20, 5),
    ]);

    let texts = collect_text_runs(&rec.ops);
    assert_eq!(texts, vec![
        (1, 1, "TEXT SPEED".into()),
        (1, 3, " FAST  MEDIUM SLOW".into()),
        (1, 6, "BATTLE ANIMATION".into()),
        (1, 8, " ON       OFF".into()),
        (1, 11, "BATTLE STYLE".into()),
        (1, 13, " SHIFT    SET".into()),
        (2, 16, "CANCEL".into()),
    ]);

    // Single ▶ on the active TextSpeed row only. Medium→x=7,y=3.
    let glyphs = collect_cursor_glyphs(&rec.ops);
    let pt = '\u{25B6}';
    assert_eq!(glyphs, vec![(7, 3, pt)]);
}

#[test]
fn options_menu_cursor_moves_to_battle_animation_row() {
    let mut state = OptionsMenuState::new(GameOptions {
        text_speed: TextSpeed::Medium,
        battle_animation: BattleAnimation::On,
        battle_style: BattleStyle::Shift,
    });
    state.row = OptionsRow::BattleAnimation;

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    menus::options::draw(&state, &OPTIONS_DEFAULT_LAYOUT, &mut ui, Lang::default());

    let glyphs = collect_cursor_glyphs(&rec.ops);
    let pt = '\u{25B6}';
    assert_eq!(glyphs, vec![(1, 8, pt)]);
}

#[test]
fn options_menu_cursor_x_tracks_setting_value_on_active_row() {
    let mut state = OptionsMenuState::new(GameOptions {
        text_speed: TextSpeed::Slow,
        battle_animation: BattleAnimation::Off,
        battle_style: BattleStyle::Set,
    });
    state.row = OptionsRow::TextSpeed;

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    menus::options::draw(&state, &OPTIONS_DEFAULT_LAYOUT, &mut ui, Lang::default());

    let glyphs = collect_cursor_glyphs(&rec.ops);
    let pt = '\u{25B6}';
    assert_eq!(glyphs, vec![(14, 3, pt)]);
}

#[test]
fn options_menu_no_cursor_on_inactive_rows() {
    let mut state = OptionsMenuState::new(GameOptions {
        text_speed: TextSpeed::Fast,
        battle_animation: BattleAnimation::Off,
        battle_style: BattleStyle::Set,
    });
    state.row = OptionsRow::BattleAnimation;

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    menus::options::draw(&state, &OPTIONS_DEFAULT_LAYOUT, &mut ui, Lang::default());

    let glyphs = collect_cursor_glyphs(&rec.ops);
    let pt = '\u{25B6}';
    assert_eq!(glyphs, vec![(10, 8, pt)]);
}

fn fixture_save_info() -> SaveScreenInfo {
    SaveScreenInfo {
        player_name: "RED".to_string(),
        num_badges: 3,
        pokedex_owned: 42,
        play_time_hours: 12,
        play_time_minutes: 34,
    }
}

fn collect_boxes(ops: &[Op]) -> Vec<TileRect> {
    ops.iter().filter_map(|op| match op {
        Op::Box(r, _) => Some(*r),
        _ => None,
    }).collect()
}

fn collect_texts(ops: &[Op]) -> Vec<(u32, u32, String)> {
    ops.iter().filter_map(|op| match op {
        Op::Text(p, s, _) => Some((p.tx, p.ty, s.clone())),
        _ => None,
    }).collect()
}

fn collect_glyphs(ops: &[Op]) -> Vec<(u32, u32, char)> {
    ops.iter().filter_map(|op| match op {
        Op::Glyph(p, ch, _) => Some((p.tx, p.ty, *ch)),
        _ => None,
    }).collect()
}

#[test]
fn yes_no_cursor_sits_one_tile_left_of_options() {
    use pokered_data::ui_layout::schema::YES_NO_DEFAULT_LAYOUT;
    let opts = vec!["YES".to_string(), "NO".to_string()];
    let mut rec = Recorder::default();
    menus::yes_no::draw(&opts, 0, &YES_NO_DEFAULT_LAYOUT, &mut Ui::new(&mut rec));

    // Box (11,8,9,6): interior starts at tile 12. Options at tx 13, cursor ▶
    // at tx 12 — before the fix both were emitted at tx 12 and overlapped.
    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(13, 9, "YES".into())));
    assert!(texts.contains(&(13, 11, "NO".into())));
    let cursors: Vec<_> = collect_glyphs(&rec.ops).into_iter().filter(|g| g.2 == '▶').collect();
    assert_eq!(cursors, vec![(12, 9, '▶')]);
}

/// Cursor / indicator glyphs drawn by `cursor` elements.
const CURSOR_GLYPHS: [char; 4] = ['\u{25B6}', '\u{25B7}', '\u{25BC}', '\u{25C6}'];

/// Collect rendered text the way a reader sees it. v1 menus emit whole-string
/// `draw_text` ops; the v2 layout engine's `text` element emits one
/// `draw_glyph` per character on the 8px tile grid. This helper accepts both:
/// `Op::Text` is taken as-is, and consecutive non-cursor `Op::Glyph`s on the
/// same row with adjacent columns are merged into a `(tx, ty, string)` run.
fn collect_text_runs(ops: &[Op]) -> Vec<(u32, u32, String)> {
    let mut runs: Vec<(u32, u32, String)> = Vec::new();
    // (start_tx, ty, next expected tx, accumulated text)
    let mut current: Option<(u32, u32, u32, String)> = None;
    fn flush(current: &mut Option<(u32, u32, u32, String)>, runs: &mut Vec<(u32, u32, String)>) {
        if let Some((sx, sy, _, text)) = current.take() {
            runs.push((sx, sy, text));
        }
    }
    for op in ops {
        match op {
            Op::Text(p, s, _) => {
                flush(&mut current, &mut runs);
                runs.push((p.tx, p.ty, s.clone()));
            }
            Op::Glyph(p, ch, _) if !CURSOR_GLYPHS.contains(ch) => match current.take() {
                Some((sx, sy, nx, mut text)) if sy == p.ty && nx == p.tx => {
                    text.push(*ch);
                    current = Some((sx, sy, nx + 1, text));
                }
                prev => {
                    current = prev;
                    flush(&mut current, &mut runs);
                    current = Some((p.tx, p.ty, p.tx + 1, ch.to_string()));
                }
            },
            _ => flush(&mut current, &mut runs),
        }
    }
    flush(&mut current, &mut runs);
    runs
}

/// Collect only cursor / indicator glyphs (▶ ▷ ▼ ◆), ignoring the
/// per-character glyphs that v2 `text` elements emit.
fn collect_cursor_glyphs(ops: &[Op]) -> Vec<(u32, u32, char)> {
    ops.iter().filter_map(|op| match op {
        Op::Glyph(p, ch, _) if CURSOR_GLYPHS.contains(ch) => Some((p.tx, p.ty, *ch)),
        _ => None,
    }).collect()
}

#[test]
fn save_menu_ask_phase_draws_info_box_prompt_box_and_yes_no_box() {
    let mut state = SaveMenuState::new(fixture_save_info(), false, false);
    state.phase = SavePhase::AskSave;
    state.cursor = YesNoChoice::Yes;

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    menus::save::draw(&state, &SAVE_DEFAULT_LAYOUT, &SAVE_ASK_PROMPT_LAYOUT, &mut ui, Lang::default());

    // Three boxes: info (4,0,15,10), prompt (0,11,20,6), yes/no (13,7,6,4).
    assert_eq!(collect_boxes(&rec.ops), vec![
        TileRect::new(4, 0, 15, 10),
        TileRect::new(0, 11, 20, 6),
        TileRect::new(13, 7, 6, 4),
    ]);

    // Info labels at exact original tile positions: PLAYER (5,2), name (12,2),
    // BADGES (5,4), num (17,4), #DEX (5,6), dex (16,6), TIME (5,8), time (13,8).
    // Prompt: "Would you like to" (1,12), "SAVE the game?" (1,14).
    // YES/NO: "YES" (15,8), "NO" (15,9).
    assert_eq!(collect_texts(&rec.ops), vec![
        (5, 2, "PLAYER".into()),
        (12, 2, "RED".into()),
        (5, 4, "BADGES".into()),
        (17, 4, "3".into()),
        (5, 6, "#DEX".into()),
        (16, 6, "42".into()),
        (5, 8, "TIME".into()),
        (13, 8, " 12:34".into()),
        (1, 12, "Would you like to".into()),
        (1, 14, "SAVE the game?".into()),
        (15, 8, "YES".into()),
        (15, 9, "NO".into()),
    ]);

    // Cursor on YES → absolute tile (14, 8).
    assert_eq!(collect_glyphs(&rec.ops), vec![(14, 8, '\u{25B6}')]);
}

#[test]
fn save_menu_ask_phase_no_cursor_moves_to_row_9() {
    let mut state = SaveMenuState::new(fixture_save_info(), false, false);
    state.phase = SavePhase::AskSave;
    state.cursor = YesNoChoice::No;

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    menus::save::draw(&state, &SAVE_DEFAULT_LAYOUT, &SAVE_ASK_PROMPT_LAYOUT, &mut ui, Lang::default());

    assert_eq!(collect_glyphs(&rec.ops), vec![(14, 9, '\u{25B6}')]);
}

#[test]
fn save_menu_confirm_overwrite_uses_same_layout_as_ask() {
    let mut state = SaveMenuState::new(fixture_save_info(), true, false);
    state.phase = SavePhase::ConfirmOverwrite;
    state.cursor = YesNoChoice::Yes;

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    menus::save::draw(&state, &SAVE_DEFAULT_LAYOUT, &SAVE_ASK_PROMPT_LAYOUT, &mut ui, Lang::default());

    // ConfirmOverwrite reuses AskSave's three boxes and prompt — original
    // game uses the identical layout for both phases.
    assert_eq!(collect_boxes(&rec.ops), vec![
        TileRect::new(4, 0, 15, 10),
        TileRect::new(0, 11, 20, 6),
        TileRect::new(13, 7, 6, 4),
    ]);
    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(1, 12, "Would you like to".into())));
    assert!(texts.contains(&(1, 14, "SAVE the game?".into())));
}

#[test]
fn save_menu_saving_phase_shows_only_now_saving_in_prompt_box() {
    let mut state = SaveMenuState::new(fixture_save_info(), false, false);
    state.phase = SavePhase::Saving { frames_remaining: 30 };

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    menus::save::draw(&state, &SAVE_DEFAULT_LAYOUT, &SAVE_ASK_PROMPT_LAYOUT, &mut ui, Lang::default());

    // Two boxes only — info box + prompt box; no YES/NO during saving.
    assert_eq!(collect_boxes(&rec.ops), vec![
        TileRect::new(4, 0, 15, 10),
        TileRect::new(0, 11, 18, 4),
    ]);
    assert_eq!(collect_glyphs(&rec.ops), Vec::<(u32,u32,char)>::new());

    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(1, 13, "Now saving...".into())));
    assert!(!texts.iter().any(|(_, _, s)| s == "YES" || s == "NO"));
}

#[test]
fn save_menu_complete_phase_shows_player_saved_the_game() {
    let mut state = SaveMenuState::new(fixture_save_info(), false, false);
    state.phase = SavePhase::SaveComplete;

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    menus::save::draw(&state, &SAVE_DEFAULT_LAYOUT, &SAVE_ASK_PROMPT_LAYOUT, &mut ui, Lang::default());

    let texts = collect_texts(&rec.ops);
    // Two-line completion message in the prompt box at (1,12) / (1,13).
    assert!(texts.contains(&(1, 12, "RED saved".into())));
    assert!(texts.contains(&(1, 13, "the game!".into())));

    // Same layout as Saving — info box + prompt box, no cursor.
    assert_eq!(collect_boxes(&rec.ops), vec![
        TileRect::new(4, 0, 15, 10),
        TileRect::new(0, 11, 18, 4),
    ]);
    assert_eq!(collect_glyphs(&rec.ops), Vec::<(u32,u32,char)>::new());
}

#[test]
fn save_menu_wait_after_save_phase_renders_identically_to_complete() {
    let info = fixture_save_info();

    let mut complete = SaveMenuState::new(info.clone(), false, false);
    complete.phase = SavePhase::SaveComplete;
    let mut rec_a = Recorder::default();
    menus::save::draw(&complete, &SAVE_DEFAULT_LAYOUT, &SAVE_ASK_PROMPT_LAYOUT, &mut Ui::new(&mut rec_a), Lang::default());

    let mut waiting = SaveMenuState::new(info, false, false);
    waiting.phase = SavePhase::WaitAfterSave { frames_remaining: 60 };
    let mut rec_b = Recorder::default();
    menus::save::draw(&waiting, &SAVE_DEFAULT_LAYOUT, &SAVE_ASK_PROMPT_LAYOUT, &mut Ui::new(&mut rec_b), Lang::default());

    assert_eq!(rec_a.ops, rec_b.ops);
}

use pokered_core::battle::state::{Pokemon, StatusCondition};
use pokered_core::party_screen::PartyScreenState;
use pokered_core::pokemon::stats::create_pokemon;
use pokered_data::species::Species;

fn fixture_pokemon(species: Species, level: u8, hp_ratio: f32, status: StatusCondition) -> Pokemon {
    let mut mon = create_pokemon(species, level, [0xFF, 0xFF]).unwrap();
    mon.hp = (mon.max_hp as f32 * hp_ratio) as u16;
    mon.status = status;
    mon
}

#[derive(Debug, PartialEq, Eq)]
struct PixelRect {
    px: u32,
    py: u32,
    pw: u32,
    ph: u32,
    color: Rgba,
}

fn collect_pixel_rects(ops: &[Op]) -> Vec<PixelRect> {
    ops.iter().filter_map(|op| match op {
        Op::PixelRect(px, py, pw, ph, color) => Some(PixelRect { px: *px, py: *py, pw: *pw, ph: *ph, color: *color }),
        _ => None,
    }).collect()
}

#[test]
fn party_menu_empty_party_shows_no_mon_message() {
    let state = PartyScreenState::new(vec![]);
    let mut rec = Recorder::default();
    menus::party::draw(&state, &PARTY_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), Lang::default());

    let texts = collect_texts(&rec.ops);
    assert_eq!(texts, vec![(3, 8, "No #MON!".into()), (2, 16, "B:Cancel".into())]);
    assert_eq!(collect_glyphs(&rec.ops), Vec::<(u32,u32,char)>::new());
    assert_eq!(collect_pixel_rects(&rec.ops), vec![]);
}

#[test]
fn party_menu_single_pokemon_full_hp_no_status() {
    let mon = fixture_pokemon(Species::Charizard, 36, 1.0, StatusCondition::None);
    let state = PartyScreenState::new(vec![mon]);
    let mut rec = Recorder::default();
    menus::party::draw(&state, &PARTY_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), Lang::default());

    // Cursor on row 0 of selected entry (index 0).
    assert_eq!(collect_glyphs(&rec.ops), vec![(0, 0, '▶')]);

    let texts = collect_texts(&rec.ops);
    // Name at (4,0), level marker at (14,0), no status code.
    // HP label ("HP:") moved to app layer; UI layer only renders the number.
    // HP numeric readout at column 14.
    assert!(texts.iter().any(|(tx, ty, _)| *tx == 4 && *ty == 0), "name missing at (4,0): {:?}", texts);
    assert!(texts.contains(&(14, 0, ":L36".into())));
    let hp_text = texts.iter().find(|(tx, ty, _)| *tx == 14 && *ty == 1);
    assert!(hp_text.is_some(), "HP numeric readout must be at tile column 14, got texts: {:?}", texts);

    // No pixel rects — HP bar is drawn at app layer.
    assert_eq!(collect_pixel_rects(&rec.ops).len(), 0);
}

#[test]
fn party_menu_zero_hp_skips_filled_rect() {
    let mon = fixture_pokemon(Species::Snorlax, 50, 0.0, StatusCondition::Sleep(3));
    let state = PartyScreenState::new(vec![mon]);
    let mut rec = Recorder::default();
    menus::party::draw(&state, &PARTY_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), Lang::default());

    // No pixel rects — HP bar is drawn at app layer.
    assert_eq!(collect_pixel_rects(&rec.ops).len(), 0);

    // Status code SLP is rendered at column 17.
    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(17, 0, "SLP".into())));
}

#[test]
fn party_menu_status_codes_map_correctly() {
    let cases = [
        (StatusCondition::Poison, "PSN"),
        (StatusCondition::Burn, "BRN"),
        (StatusCondition::Freeze, "FRZ"),
        (StatusCondition::Paralysis, "PAR"),
        (StatusCondition::Sleep(1), "SLP"),
    ];
    for (status, expected_code) in cases {
        let mon = fixture_pokemon(Species::Pikachu, 10, 0.5, status.clone());
        let state = PartyScreenState::new(vec![mon]);
        let mut rec = Recorder::default();
        menus::party::draw(&state, &PARTY_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), Lang::default());
        let texts = collect_texts(&rec.ops);
        assert!(
            texts.contains(&(17, 0, expected_code.into())),
            "status {:?} should render code {:?} at (17, 0), got texts: {:?}",
            status, expected_code, texts
        );
    }
}

#[test]
fn party_menu_cursor_follows_selection() {
    let party = vec![
        fixture_pokemon(Species::Charizard, 36, 1.0, StatusCondition::None),
        fixture_pokemon(Species::Blastoise, 36, 1.0, StatusCondition::None),
        fixture_pokemon(Species::Venusaur, 36, 1.0, StatusCondition::None),
    ];
    let mut state = PartyScreenState::new(party);
    state.update_frame(pokered_core::party_screen::PartyScreenInput {
        down: true,
        up: false,
        a: false,
        b: false,
    });

    let mut rec = Recorder::default();
    menus::party::draw(&state, &PARTY_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), Lang::default());
    // Cursor on row 2 (entry index 1, two tile rows per entry).
    assert_eq!(collect_glyphs(&rec.ops), vec![(0, 2, '▶')]);
}

use pokered_core::naming_screen::{NamingInput, NamingScreenState, NamingScreenType};
use pokered_data::charmap::naming_tiles;

fn collect_gb_tiles(ops: &[Op]) -> Vec<(u32, u32, u8, String)> {
    ops.iter().filter_map(|op| match op {
        Op::GbTile(pos, tile_id, fallback, _) => Some((pos.tx, pos.ty, *tile_id, fallback.clone())),
        _ => None,
    }).collect()
}

#[test]
fn naming_player_screen_renders_title_box_underscores_and_keyboard() {
    let state = NamingScreenState::new(NamingScreenType::Player);
    let mut rec = Recorder::default();
    menus::naming::draw(&state, &NAMING_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), false);

    assert_eq!(rec.ops[0], Op::Clear(Rgba::INK_WHITE));

    let boxes: Vec<_> = rec.ops.iter().filter_map(|op| match op {
        Op::Box(r, _) => Some(*r),
        _ => None,
    }).collect();
    // Box is 20×13 (rows 5..=17): tall enough to also contain the zh pinyin
    // buffer/candidate lines, matching naming.gui.
    assert_eq!(boxes, vec![TileRect::new(0, 5, 20, 13)]);

    let texts = collect_texts(&rec.ops);
    // Title and name box are centered on the 20-column screen.
    assert!(texts.contains(&(5, 1, "YOUR NAME?".into())));
    assert!(texts.contains(&(6, 3, "".into())));
    assert!(texts.contains(&(2, 16, "lower case".into())));

    let tiles = collect_gb_tiles(&rec.ops);
    let underscore_tiles: Vec<_> = tiles.iter().filter(|(_, ty, _, _)| *ty == 4).collect();
    assert_eq!(underscore_tiles.len(), 7, "Player name max_len = 7 underscores at row ty=4");
    let raised_count = underscore_tiles.iter().filter(|(_, _, id, _)| *id == naming_tiles::RAISED_UNDERSCORE).count();
    assert_eq!(raised_count, 1, "Empty name → first slot is raised underscore");

    // Alphabet rows are spaced 2 rows apart (6,8,10,12,14) in alphabet mode.
    let keyboard_tiles: Vec<_> = tiles.iter().filter(|(_, ty, _, _)| *ty >= 6 && *ty <= 14 && *ty % 2 == 0).collect();
    let cursor_tiles: Vec<_> = keyboard_tiles.iter().filter(|(_, _, id, _)| *id == naming_tiles::CURSOR_ARROW).collect();
    assert_eq!(cursor_tiles.len(), 1);
    assert_eq!((cursor_tiles[0].0, cursor_tiles[0].1), (1, 6), "Initial cursor at (1,6) = KEYBOARD_X-1, KEYBOARD_Y");
}

#[test]
fn naming_pinyin_rows_stay_inside_keyboard_box() {
    use pokered_core::naming_screen::InputMode;
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    state.input_mode = InputMode::Pinyin;
    state.pinyin_buf = "ni".to_string();
    state.pinyin_candidates = vec!['你', '尼', '泥'];
    let mut rec = Recorder::default();
    menus::naming::draw(&state, &NAMING_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), true);

    // The keyboard box (0,5,20,13) has its interior on rows 6..=16; the pinyin
    // buffer (row 12) and candidates (rows 14/16) must land inside it.
    let interior_max_ty = 5 + 13 - 2;
    for op in &rec.ops {
        let ty = match op {
            Op::Text(pos, _, _) | Op::Glyph(pos, _, _) | Op::GbTile(pos, _, _, _) => Some(pos.ty),
            _ => None,
        };
        if let Some(ty) = ty {
            assert!(ty <= interior_max_ty, "op at ty={ty} escapes the keyboard box interior");
        }
    }
    let texts = collect_texts(&rec.ops);
    assert!(texts.iter().any(|(_, ty, t)| *ty == 12 && t.starts_with("拼音")));
    assert!(texts.iter().any(|(_, ty, t)| (*ty == 14 || *ty == 16) && t.contains('你')));
}

#[test]
fn naming_rival_screen_uses_rival_title() {
    let state = NamingScreenState::new(NamingScreenType::Rival);
    let mut rec = Recorder::default();
    menus::naming::draw(&state, &NAMING_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), false);
    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(3, 1, "RIVAL's NAME?".into())));
}

#[test]
fn naming_pokemon_screen_uses_nickname_title() {
    let state = NamingScreenState::new(NamingScreenType::Pokemon);
    let mut rec = Recorder::default();
    menus::naming::draw(&state, &NAMING_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), false);
    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(5, 1, "NICKNAME?".into())));
}

#[test]
fn naming_lowercase_toggle_shows_upper_case_label_when_in_lowercase() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    for _ in 0..5 {
        state.update_frame(NamingInput { down: true, ..NamingInput::none() }, false);
    }
    state.update_frame(NamingInput { a: true, ..NamingInput::none() }, false);
    let mut rec = Recorder::default();
    menus::naming::draw(&state, &NAMING_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), false);
    let texts = collect_texts(&rec.ops);
    let case_label = texts.iter().find(|(tx, ty, _)| *tx == 2 && *ty == 16);
    assert!(case_label.is_some());
    assert!(case_label.unwrap().2 == "UPPER CASE" || case_label.unwrap().2 == "lower case",
        "case row label must toggle between cases, got {:?}", case_label);
}

#[test]
fn naming_cursor_on_case_row_renders_arrow_at_keyboard_x_minus_one() {
    let mut state = NamingScreenState::new(NamingScreenType::Player);
    for _ in 0..5 {
        state.update_frame(NamingInput { down: true, ..NamingInput::none() }, false);
    }
    let mut rec = Recorder::default();
    menus::naming::draw(&state, &NAMING_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), false);
    let tiles = collect_gb_tiles(&rec.ops);
    let case_row_arrows: Vec<_> = tiles.iter()
        .filter(|(tx, ty, id, _)| *ty == 16 && *tx == 1 && *id == naming_tiles::CURSOR_ARROW)
        .collect();
    assert_eq!(case_row_arrows.len(), 1, "case row cursor must be at (1, 16)");
}

// ── Battle menu tests ──

use pokered_core::battle::menu::{
    BagMenuState, BattleMenuInput, BattleMenuState, MoveMenuState, MoveSlot,
};
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_ui::menus::{battle_bag, battle_main, battle_move, battle_party, battle_text};
use pokered_data::ui_layout::schema::{BATTLE_TEXT_DEFAULT_LAYOUT, BATTLE_PARTY_DEFAULT_LAYOUT, BATTLE_BAG_DEFAULT_LAYOUT, BATTLE_MAIN_DEFAULT_LAYOUT, BATTLE_MOVE_DEFAULT_LAYOUT};
use pokered_data::impl_traits::PokemonRenderData;

// -- battle_main (2×2 grid) --

#[test]
fn battle_main_default_grid_labels_and_cursor() {
    let state = BattleMenuState::new();
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    battle_main::draw(&state, &BATTLE_MAIN_DEFAULT_LAYOUT, &mut ui, Lang::default());

    assert_eq!(
        collect_boxes(&rec.ops),
        vec![TileRect::new(0, 12, 20, 6), TileRect::new(8, 12, 12, 6)]
    );

    // Canonical positions per the original game's draw_battle_menu_tiles
    // (crates/pokered-app/src/render/battle.rs:1571-1578): FIGHT at (10, 14),
    // PKMN composite tiles 0xE1/0xE2 at (16, 14) / (17, 14), ITEM at (10, 16),
    // RUN at (16, 16). The v2 text element draws labels one glyph per tile;
    // collect_text_runs re-assembles them. PKMN is 2 gb_tile draws whose
    // generic "[id]" fallbacks are unused — the framebuffer painter maps
    // 0xE1/0xE2 to "PK"/"MN".
    let texts = collect_text_runs(&rec.ops);
    assert_eq!(texts, vec![
        (10, 14, "FIGHT".into()),
        (10, 16, "ITEM".into()),
        (16, 16, "RUN".into()),
    ]);

    let gb_tiles = collect_gb_tiles(&rec.ops);
    assert_eq!(gb_tiles, vec![
        (16, 14, 0xE1, "[225]".into()),
        (17, 14, 0xE2, "[226]".into()),
    ]);

    assert_eq!(collect_cursor_glyphs(&rec.ops), vec![(9, 14, '\u{25B6}')]);
}

#[test]
fn battle_main_cursor_tracks_row_and_col() {
    let mut state = BattleMenuState::new();
    state.update_frame(BattleMenuInput { down: true, ..BattleMenuInput::none() });
    assert_eq!((state.row(), state.col()), (1, 0));

    let mut rec = Recorder::default();
    battle_main::draw(&state, &BATTLE_MAIN_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), Lang::default());
    assert_eq!(collect_cursor_glyphs(&rec.ops), vec![(9, 16, '\u{25B6}')]);

    // Up wraps back to row 0
    let mut state2 = BattleMenuState::new();
    state2.update_frame(BattleMenuInput { down: true, ..BattleMenuInput::none() });
    state2.update_frame(BattleMenuInput { up: true, ..BattleMenuInput::none() });
    assert_eq!((state2.row(), state2.col()), (0, 0));

    let mut rec2 = Recorder::default();
    battle_main::draw(&state2, &BATTLE_MAIN_DEFAULT_LAYOUT, &mut Ui::new(&mut rec2), Lang::default());
    assert_eq!(collect_cursor_glyphs(&rec2.ops), vec![(9, 14, '\u{25B6}')]);

    // Right moves to col 1 → cursor at canonical (15, 14)
    let mut state3 = BattleMenuState::new();
    state3.update_frame(BattleMenuInput { right: true, ..BattleMenuInput::none() });
    assert_eq!((state3.row(), state3.col()), (0, 1));

    let mut rec3 = Recorder::default();
    battle_main::draw(&state3, &BATTLE_MAIN_DEFAULT_LAYOUT, &mut Ui::new(&mut rec3), Lang::En);
    assert_eq!(collect_cursor_glyphs(&rec3.ops), vec![(15, 14, '\u{25B6}')]);

    // Down + Right to (1, 1) → canonical (15, 16)
    let mut state4 = BattleMenuState::new();
    state4.update_frame(BattleMenuInput { down: true, ..BattleMenuInput::none() });
    state4.update_frame(BattleMenuInput { right: true, ..BattleMenuInput::none() });
    assert_eq!((state4.row(), state4.col()), (1, 1));

    let mut rec4 = Recorder::default();
    battle_main::draw(&state4, &BATTLE_MAIN_DEFAULT_LAYOUT, &mut Ui::new(&mut rec4), Lang::En);
    assert_eq!(collect_cursor_glyphs(&rec4.ops), vec![(15, 16, '\u{25B6}')]);
}

// -- battle_move --

fn fixture_move_slot(move_id: MoveId, current_pp: u8, max_pp: u8) -> MoveSlot {
    MoveSlot { move_id, current_pp, max_pp, is_disabled: false }
}

#[test]
fn battle_move_renders_move_list_with_type_pp_box() {
    let moves = vec![
        fixture_move_slot(MoveId::Pound, 35, 35),
        fixture_move_slot(MoveId::KarateChop, 25, 25),
    ];
    let state = MoveMenuState::new(moves);

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    let rd = PokemonRenderData::new(false);
    battle_move::draw(&state, &BATTLE_MOVE_DEFAULT_LAYOUT, &mut ui, &rd);

    let boxes = collect_boxes(&rec.ops);
    assert_eq!(boxes.len(), 3, "expect three boxes: base + move list + TYPE/PP");
    assert_eq!(boxes[0], TileRect::new(0, 12, 20, 6));
    assert_eq!(boxes[1], TileRect::new(4, 12, 16, 6));

    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(6, 13, "POUND".into())), "texts: {:?}", texts);
    assert!(texts.contains(&(6, 14, "KARATE CHOP".into())), "texts: {:?}", texts);

    // TYPE/PP box
    assert_eq!(boxes[2], TileRect::new(0, 8, 11, 5));
    assert!(texts.contains(&(1, 9, "TYPE/".into())));
    assert!(texts.contains(&(1, 10, "NORMAL".into())));
    assert!(texts.contains(&(2, 11, "PP".into())));
    assert!(texts.contains(&(5, 11, "35/35".into())));

    assert_eq!(collect_glyphs(&rec.ops), vec![(5, 13, '\u{25B6}')]);
}

#[test]
fn battle_move_cursor_follows_selection() {
    let moves = vec![
        fixture_move_slot(MoveId::Pound, 35, 35),
        fixture_move_slot(MoveId::KarateChop, 25, 25),
        fixture_move_slot(MoveId::Thunderbolt, 15, 15),
    ];
    let mut state = MoveMenuState::new(moves);
    state.update_frame(pokered_core::main_menu::MenuInput {
        up: false, down: true, a: false, b: false,
    });

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    let rd = PokemonRenderData::new(false);
    battle_move::draw(&state, &BATTLE_MOVE_DEFAULT_LAYOUT, &mut ui, &rd);

    assert_eq!(collect_glyphs(&rec.ops), vec![(5, 14, '\u{25B6}')]);

    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(1, 10, "NORMAL".into())));
    assert!(texts.contains(&(5, 11, "25/25".into())));
}

#[test]
fn battle_move_no_type_pp_box_when_cursor_out_of_bounds() {
    let state = MoveMenuState::new(vec![]);

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    let rd = PokemonRenderData::new(false);
    battle_move::draw(&state, &BATTLE_MOVE_DEFAULT_LAYOUT, &mut ui, &rd);

    let boxes = collect_boxes(&rec.ops);
    assert_eq!(boxes.len(), 2, "empty moves → base + move-list box only");
    assert_eq!(boxes[0], TileRect::new(0, 12, 20, 6));
    assert_eq!(boxes[1], TileRect::new(4, 12, 16, 6));
    assert!(collect_texts(&rec.ops).is_empty());
    assert_eq!(collect_glyphs(&rec.ops), vec![(5, 13, '\u{25B6}')]);
}

// -- battle_bag --

#[test]
fn battle_bag_renders_items_with_quantity_and_cancel() {
    let items = vec![(ItemId::Potion, 3u8), (ItemId::Antidote, 1u8)];
    let state = BagMenuState::new(items);

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    let rd = PokemonRenderData::new(false);
    battle_bag::draw(&state, &BATTLE_BAG_DEFAULT_LAYOUT, &mut ui, &rd);

    assert_eq!(collect_boxes(&rec.ops), vec![TileRect::new(4, 10, 16, 7)]);

    assert_eq!(collect_texts(&rec.ops), vec![
        (7, 12, "POTION \u{00D7}3".into()),
        (7, 13, "ANTIDOTE \u{00D7}1".into()),
        (7, 14, "CANCEL".into()),
    ]);

    assert_eq!(collect_glyphs(&rec.ops), vec![(6, 12, '\u{25B6}')]);
}

#[test]
fn battle_bag_cursor_tracks_selection() {
    let items = vec![(ItemId::Potion, 3u8), (ItemId::Antidote, 1u8)];
    let mut state = BagMenuState::new(items);
    state.update_frame(BattleMenuInput { down: true, ..BattleMenuInput::none() });

    let mut rec = Recorder::default();
    let rd = PokemonRenderData::new(false);
    battle_bag::draw(&state, &BATTLE_BAG_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), &rd);
    assert_eq!(collect_glyphs(&rec.ops), vec![(6, 13, '\u{25B6}')]);

    // Cursor wraps within items only, CANCEL row is not selectable
    state.update_frame(BattleMenuInput { down: true, ..BattleMenuInput::none() });
    let mut rec2 = Recorder::default();
    battle_bag::draw(&state, &BATTLE_BAG_DEFAULT_LAYOUT, &mut Ui::new(&mut rec2), &rd);
    assert_eq!(collect_glyphs(&rec2.ops), vec![(6, 12, '\u{25B6}')]);
}

#[test]
fn battle_bag_empty_items_renders_only_cancel() {
    let state = BagMenuState::new(vec![]);

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    let rd = PokemonRenderData::new(false);
    battle_bag::draw(&state, &BATTLE_BAG_DEFAULT_LAYOUT, &mut ui, &rd);

    assert_eq!(collect_texts(&rec.ops), vec![(7, 12, "CANCEL".into())]);
    assert_eq!(collect_glyphs(&rec.ops), vec![(6, 12, '\u{25B6}')]);
}

// -- battle_party --

fn fixture_battle_pokemon(species: Species, level: u8, hp: u16, max_hp: u16) -> Pokemon {
    let mut mon = create_pokemon(species, level, [0xFF, 0xFF]).unwrap();
    mon.hp = hp;
    mon.max_hp = max_hp;
    mon
}

#[test]
fn battle_party_single_pokemon_renders_name_and_hp() {
    let mon = fixture_battle_pokemon(Species::Charizard, 36, 150, 200);
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    battle_party::draw(&[mon], 0, &BATTLE_PARTY_DEFAULT_LAYOUT, &mut ui, false);

    assert_eq!(collect_boxes(&rec.ops), vec![TileRect::new(1, 13, 18, 5)]);

    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(3, 15, "CHARIZARD 150/200".into())),
        "expected CHARIZARD 150/200, got {:?}", texts);

    assert_eq!(collect_glyphs(&rec.ops), vec![(2, 15, '\u{25B6}')]);
}

#[test]
fn battle_party_fainted_shows_fnt() {
    let mon = fixture_battle_pokemon(Species::Snorlax, 50, 0, 300);
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    battle_party::draw(&[mon], 0, &BATTLE_PARTY_DEFAULT_LAYOUT, &mut ui, false);

    assert!(collect_texts(&rec.ops).contains(&(3, 15, "SNORLAX FNT".into())),
        "fainted mon should show FNT");
}

#[test]
fn battle_party_cursor_follows_selection() {
    let party = vec![
        fixture_battle_pokemon(Species::Charizard, 36, 150, 200),
        fixture_battle_pokemon(Species::Blastoise, 36, 160, 200),
        fixture_battle_pokemon(Species::Venusaur, 36, 170, 200),
    ];

    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    battle_party::draw(&party, 1, &BATTLE_PARTY_DEFAULT_LAYOUT, &mut ui, false);

    assert_eq!(collect_glyphs(&rec.ops), vec![(2, 16, '\u{25B6}')]);
}

#[test]
fn battle_party_scrolls_when_more_than_four_pokemon() {
    let species_list = [
        Species::Charizard, Species::Blastoise, Species::Venusaur,
        Species::Pikachu, Species::Snorlax, Species::Mewtwo,
    ];
    let party: Vec<Pokemon> = species_list.iter()
        .map(|&s| fixture_battle_pokemon(s, 50, 100, 200))
        .collect();

    // cursor=5 → visible_start = min(4, 2) = 2, visible: indices 2-5
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    battle_party::draw(&party, 5, &BATTLE_PARTY_DEFAULT_LAYOUT, &mut ui, false);

    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(3, 15, "VENUSAUR 100/200".into())));
    assert!(texts.contains(&(3, 16, "PIKACHU 100/200".into())));
    assert!(texts.contains(&(3, 17, "SNORLAX 100/200".into())));
    assert!(texts.contains(&(3, 18, "MEWTWO 100/200".into())));

    // Cursor at index 5 → visual row 4 → abs(2, 18)
    assert_eq!(collect_glyphs(&rec.ops), vec![(2, 18, '\u{25B6}')]);
}

#[test]
fn battle_party_empty_party_draws_nothing() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    battle_party::draw(&[], 0, &BATTLE_PARTY_DEFAULT_LAYOUT, &mut ui, false);

    assert!(rec.ops.is_empty(), "empty party should emit no ops, got {:?}", rec.ops);
}

// -- battle_text --

#[test]
fn battle_text_wraps_long_text_to_two_lines() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    battle_text::draw("What will CHARIZARD do with its last move?", false, &BATTLE_TEXT_DEFAULT_LAYOUT, &mut ui, Lang::En);

    // Box matches canonical native render: TextBoxFrame::standard_dialog() = (0,12,20,6)
    assert_eq!(collect_boxes(&rec.ops), vec![TileRect::new(0, 12, 20, 6)]);

    // Native dialog renders text at screen (1,14)/(1,16). text_box adds +1,+1 padding,
    // so frame.label(0,1)/(0,3) → absolute (1,14)/(1,16). Lines wrap at the 144px
    // interior width (28 Latin chars), not the old 18-char cap.
    assert_eq!(collect_texts(&rec.ops), vec![
        (1, 14, "What will CHARIZARD do with".into()),
        (1, 16, "its last move?".into()),
    ]);

    assert!(collect_glyphs(&rec.ops).is_empty());
}

#[test]
fn battle_text_no_wrap_for_short_text() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    battle_text::draw("Got away safely!", false, &BATTLE_TEXT_DEFAULT_LAYOUT, &mut ui, Lang::En);

    let texts = collect_texts(&rec.ops);
    assert_eq!(texts, vec![(1, 14, "Got away safely!".into())]);
    assert_eq!(texts.iter().filter(|(_, ty, _)| *ty == 16).count(), 0);
}

#[test]
fn battle_text_shows_arrow_when_show_arrow_true() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    battle_text::draw("Press A to continue", true, &BATTLE_TEXT_DEFAULT_LAYOUT, &mut ui, Lang::En);

    // JSON cursor tx=17, base_ty=3 inside box (0,12,20,6). text_box adds +1,+1 padding,
    // so absolute = (1+17, 13+3) = (18, 16) — matches native arrow position.
    assert_eq!(collect_glyphs(&rec.ops), vec![(18, 16, '\u{25BC}')]);
}

#[test]
fn battle_text_arrow_suppressed_when_show_arrow_false() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    battle_text::draw("Press A to continue", false, &BATTLE_TEXT_DEFAULT_LAYOUT, &mut ui, Lang::En);

    assert!(collect_glyphs(&rec.ops).is_empty());
}

#[test]
fn battle_text_preserves_blank_lines() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    battle_text::draw("Line one\n\nLine three", false, &BATTLE_TEXT_DEFAULT_LAYOUT, &mut ui, Lang::En);

    assert_eq!(collect_texts(&rec.ops), vec![
        (1, 14, "Line one".into()),
        (1, 16, "".into()),
        (1, 18, "Line three".into()),
    ]);
}

// ── Mart menu tests ──

use pokered_ui::menus::mart::{self, ConfirmChoice};
use pokered_data::ui_layout::schema::MART_MAIN_MENU_LAYOUT;
use pokered_data::ui_layout::schema::MART_CONFIRM_LAYOUT;
use pokered_data::ui_layout::schema::MART_QUANTITY_LAYOUT;
use pokered_data::ui_layout::schema::MART_RESULT_DIALOG_LAYOUT;

#[test]
fn draw_main_with_money_shows_buy_sell_quit_and_money_box() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    mart::draw_main_with_money(0, 3500, &MART_MAIN_MENU_LAYOUT, &mut ui, Lang::En);

    let boxes = collect_boxes(&rec.ops);
    assert_eq!(boxes, vec![
        TileRect::new(0, 0, 7, 8),
        TileRect::new(6, 0, 14, 3),
    ]);

    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(2, 2, "BUY".into())));
    assert!(texts.contains(&(2, 4, "SELL".into())));
    assert!(texts.contains(&(2, 6, "QUIT".into())));
    assert!(texts.contains(&(7, 1, "MONEY $3500".into())));

    assert_eq!(collect_glyphs(&rec.ops), vec![(1, 2, '\u{25B6}')]);
}

#[test]
fn draw_main_with_money_cursor_follows_selection() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    mart::draw_main_with_money(2, 1000, &MART_MAIN_MENU_LAYOUT, &mut ui, Lang::En);

    assert_eq!(collect_glyphs(&rec.ops), vec![(1, 6, '\u{25B6}')]);
}

#[test]
fn draw_quantity_shows_item_name_qty_cost_and_money() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    mart::draw_quantity("POTION", 5, 300, 1500, 5000, &MART_QUANTITY_LAYOUT, Lang::default(), &mut ui);

    let boxes = collect_boxes(&rec.ops);
    assert_eq!(boxes, vec![
        TileRect::new(0, 0, 10, 6),
        TileRect::new(10, 0, 8, 3),
    ]);

    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(2, 2, "POTION".into())));
    assert!(texts.contains(&(2, 4, "× 5".into())));
    assert!(texts.contains(&(6, 4, "$1500".into())));
    assert!(texts.contains(&(12, 2, "MONEY $5000".into())));
}

#[test]
fn draw_confirm_box_rect_is_exact() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    mart::draw_confirm(Lang::En, "Buy item for $300?", ConfirmChoice::Yes, &MART_CONFIRM_LAYOUT, &mut ui);

    let boxes = collect_boxes(&rec.ops);
    assert_eq!(boxes, vec![TileRect::new(14, 7, 6, 8)]);
}

#[test]
fn draw_confirm_yes_cursor_on_row_8() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    mart::draw_confirm(Lang::En, "Buy item?", ConfirmChoice::Yes, &MART_CONFIRM_LAYOUT, &mut ui);

    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(16, 9, "YES".into())));
    assert!(texts.contains(&(16, 10, "NO".into())));

    assert_eq!(collect_glyphs(&rec.ops), vec![(15, 9, '\u{25B6}')]);
}

#[test]
fn draw_confirm_no_cursor_on_row_9() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    mart::draw_confirm(Lang::En, "Sell item?", ConfirmChoice::No, &MART_CONFIRM_LAYOUT, &mut ui);

    assert_eq!(collect_glyphs(&rec.ops), vec![(15, 10, '\u{25B6}')]);
}

#[test]
fn draw_confirm_message_rendered_above_box() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    mart::draw_confirm(Lang::En, "Buy for $300?", ConfirmChoice::Yes, &MART_CONFIRM_LAYOUT, &mut ui);

    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(1, 0, "Buy for $300?".into())));
}

#[test]
fn draw_result_dialog_shows_lines_in_bottom_box() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    mart::draw_result_dialog(&["Here you are!", "Thank you!"], &MART_RESULT_DIALOG_LAYOUT, &mut ui);

    let boxes = collect_boxes(&rec.ops);
    assert_eq!(boxes, vec![TileRect::new(0, 13, 20, 5)]);

    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(2, 14, "Here you are!".into())));
    assert!(texts.contains(&(2, 16, "Thank you!".into())));
}

#[test]
fn draw_result_dialog_single_line() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    mart::draw_result_dialog(&["You don't have enough money."], &MART_RESULT_DIALOG_LAYOUT, &mut ui);

    let texts = collect_texts(&rec.ops);
    assert_eq!(texts, vec![(2, 14, "You don't have enough money.".into())]);
}

#[test]
fn draw_main_menu_uses_layout_static() {
    use pokered_core::items::shop::ShopMenuState;

    let state = ShopMenuState::new();
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    mart::draw_main_menu(&state, &MART_MAIN_MENU_LAYOUT, &mut ui, Lang::default());

    // Should produce the same ops as the layout JSON:
    // - Box at (0, 0, 8, 8) in Black
    // - Labels BUY/SELL/QUIT at relative positions (1,1), (1,3), (1,5)
    // - Cursor glyph at (1, 2) since cursor.tx=0 and cursor starts at 0
    let boxes = collect_boxes(&rec.ops);
    assert_eq!(boxes, vec![TileRect::new(0, 0, 7, 8)]);

    let texts = collect_texts(&rec.ops);
    assert!(texts.contains(&(2, 2, "BUY".into())));   // origin +1 + label.tx=1
    assert!(texts.contains(&(2, 4, "SELL".into())));
    assert!(texts.contains(&(2, 6, "QUIT".into())));

    assert_eq!(collect_glyphs(&rec.ops), vec![(1, 2, '\u{25B6}')]); // origin +1 + cursor.tx=0
}


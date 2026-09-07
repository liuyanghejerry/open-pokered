use pokered_ui::{Frame, Painter, Rgba, TilePos, TileRect, Ui};

#[derive(Debug, PartialEq, Eq)]
enum Op {
    Clear(Rgba),
    Box(TileRect, Rgba),
    Text(TilePos, String, Rgba),
    Glyph(TilePos, char, Rgba),
    PixelRect(u32, u32, u32, u32, Rgba),
    PixelText(u32, u32, String, Rgba),
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
    fn draw_text_px(&mut self, px: u32, py: u32, text: &str, color: Rgba) {
        self.ops.push(Op::PixelText(px, py, text.into(), color));
    }
    fn measure_text_px(&self, text: &str) -> u32 {
        pokered_renderer::embedded_font::measure_text(text)
    }
    fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba) {
        self.ops.push(Op::Glyph(pos, glyph, color));
    }
    fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, color: Rgba) {
        self.ops.push(Op::PixelRect(px, py, pw, ph, color));
    }
    fn draw_gb_tile(&mut self, _pos: TilePos, _tile_id: u8, _fallback: &str, _color: Rgba) {}
}

#[test]
fn text_box_draws_outer_rect_and_invokes_body_with_inset_origin() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    ui.text_box(TileRect::new(2, 3, 5, 4), Rgba::INK_BLACK, true, |f: &mut Frame<_>| {
        f.label(0, 0, "hi", Rgba::INK_BLACK);
    });

    assert_eq!(rec.ops.len(), 2);
    assert_eq!(rec.ops[0], Op::Box(TileRect::new(2, 3, 5, 4), Rgba::INK_BLACK));
    // Body's (0,0) must map to (3,4) — one tile inset inside the box at (2,3).
    assert_eq!(rec.ops[1], Op::Text(TilePos::new(3, 4), "hi".into(), Rgba::INK_BLACK));
}

#[test]
fn menu_list_draws_items_and_cursor_only_at_selected_row() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    ui.text_box(TileRect::new(0, 0, 20, 18), Rgba::INK_BLACK, false, |f: &mut Frame<_>| {
        f.menu_list(1, 2, &["A", "B", "C"], 1, 2, Rgba::INK_BLACK);
    });

    let mut texts = vec![];
    let mut cursors = vec![];
    for op in &rec.ops {
        match op {
            Op::Text(pos, s, _) => texts.push((pos.tx, pos.ty, s.clone())),
            Op::Glyph(pos, ch, _) => cursors.push((pos.tx, pos.ty, *ch)),
            _ => {}
        }
    }
    assert_eq!(texts, vec![(2, 2, "A".into()), (2, 4, "B".into()), (2, 6, "C".into())]);
    assert_eq!(cursors, vec![(1, 4, '\u{25B6}')]);
}

#[test]
fn nested_text_boxes_compose_origins_additively() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    ui.text_box(TileRect::new(1, 1, 10, 10), Rgba::INK_BLACK, true, |outer: &mut Frame<_>| {
        outer.sub_text_box(TileRect::new(2, 3, 4, 2), Rgba::INK_BLACK, |inner| {
            inner.label(0, 0, "x", Rgba::INK_BLACK);
        });
    });

    // outer box at (1,1), so outer body origin = (2,2).
    // sub_text_box rect (2,3) translates by (2,2) → absolute box at (4,5).
    // inner body origin = (5,6); label (0,0) → (5,6).
    let boxes: Vec<_> = rec.ops.iter().filter_map(|op| match op {
        Op::Box(r, _) => Some(*r),
        _ => None,
    }).collect();
    assert_eq!(boxes, vec![TileRect::new(1, 1, 10, 10), TileRect::new(4, 5, 4, 2)]);

    let texts: Vec<_> = rec.ops.iter().filter_map(|op| match op {
        Op::Text(p, s, _) => Some((p.tx, p.ty, s.clone())),
        _ => None,
    }).collect();
    assert_eq!(texts, vec![(5, 6, "x".into())]);
}

#[test]
fn pixel_rect_offsets_from_frame_origin_in_pixels() {
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    ui.text_box(TileRect::new(2, 1, 8, 3), Rgba::INK_BLACK, true, |f: &mut Frame<_>| {
        // Frame origin = (3,2) tiles = (24,16) pixels. Pixel offset (5, 3) → (29, 19).
        f.pixel_rect(5, 3, 40, 8, Rgba::INK_DARK_GRAY);
    });

    let rects: Vec<_> = rec.ops.iter().filter_map(|op| match op {
        Op::PixelRect(x, y, w, h, c) => Some((*x, *y, *w, *h, *c)),
        _ => None,
    }).collect();
    assert_eq!(rects, vec![(29, 19, 40, 8, Rgba::INK_DARK_GRAY)]);
}

/// Safari Zone START info box (PrintSafariZoneSteps, player_state.asm:225):
/// interior 7×3 at (0,0) — labels at screen (1,1) and (1,3). The earlier
/// 7×4 TOTAL box had only 2 interior rows, pushing the BALL×NN row onto the
/// bottom border (audit: safari-remaining-*.png).
#[test]
fn safari_zone_start_info_box_matches_original_layout() {
    use pokered_core::game_state::Lang;
    use pokered_core::start_menu::StartMenuState;
    use pokered_data::ui_layout::schema::START_DEFAULT_LAYOUT;

    let mut state = StartMenuState::new(true, true, false);
    state.safari_info = Some(pokered_core::start_menu::SafariZoneInfo { steps: 10, balls: 7 });
    let mut rec = Recorder::default();
    let mut ui = Ui::new(&mut rec);
    pokered_ui::menus::start::draw(&state, "RED", &START_DEFAULT_LAYOUT, &mut ui, Lang::En);

    assert!(
        rec.ops.iter().any(|op| matches!(op, Op::Box(r, _) if *r == TileRect::new(0, 0, 9, 5))),
        "info box = interior 7×3 plus its borders"
    );
    assert!(
        rec.ops.iter().any(|op| matches!(
            op,
            Op::Text(p, t, _) if *p == TilePos::new(1, 1) && t == "010/500"
        )),
        "steps label at screen (1,1)"
    );
    assert!(
        rec.ops.iter().any(|op| matches!(
            op,
            Op::Text(p, t, _) if *p == TilePos::new(1, 3) && t == "BALL×07"
        )),
        "ball label at screen (1,3)"
    );
}

#[test]
fn party_menus_keep_every_option_inside_the_border() {
    use pokered_core::{game_state::Lang, party_screen::{PartyScreenInput, PartyScreenState}, pokemon::stats::create_pokemon_with_moves};
    use pokered_data::{moves::MoveId, species::Species, ui_layout::schema::PARTY_DEFAULT_LAYOUT};
    for lang in [Lang::En, Lang::Zh] {
        for moves in [
            [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
            [MoveId::Cut, MoveId::Surf, MoveId::Strength, MoveId::Teleport],
            [MoveId::Thunderbolt, MoveId::Doubleslap, MoveId::Solarbeam, MoveId::QuickAttack],
        ] {
            let mon = create_pokemon_with_moves(Species::Bulbasaur, 7, [0x9a, 0x78], moves).unwrap();
            for forget in [false, true] {
                let mut state = if forget {
                    PartyScreenState::new_for_move_choice(vec![mon.clone()], 0)
                } else {
                    let mut state = PartyScreenState::new(vec![mon.clone()]);
                    state.update_frame(PartyScreenInput { a: true, ..PartyScreenInput::none() });
                    state
                };
                // Exercise every cursor position, including the final CANCEL.
                let count = if forget { state.selected_known_moves().len() + 1 } else { state.selected_field_moves().len() + 3 };
                for _ in 0..count {
                    let mut rec = Recorder::default();
                    pokered_ui::menus::party::draw(&state, &PARTY_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), lang);
                    let start = rec.ops.iter().rposition(|op| matches!(op, Op::Box(..))).unwrap();
                    let Op::Box(rect, _) = rec.ops[start] else { unreachable!() };
                    assert!(rect.tx + rect.tw <= 20 && rect.ty + rect.th <= 18, "{rect:?}");
                    for op in &rec.ops[start + 1..] {
                        let (pos, width) = match op {
                            Op::Text(pos, text, _) => (pos, text.chars().count() as u32),
                            Op::Glyph(pos, _, _) => (pos, 1),
                            _ => continue,
                        };
                        assert!(pos.tx > rect.tx && pos.tx + width <= rect.tx + rect.tw - 1, "{op:?} outside {rect:?}");
                        assert!(pos.ty > rect.ty && pos.ty < rect.ty + rect.th - 1, "{op:?} outside {rect:?}");
                    }
                    state.update_frame(PartyScreenInput { down: true, ..PartyScreenInput::none() });
                }
            }
        }
    }
}

#[test]
fn full_party_numbers_and_status_have_room_for_tall_glyphs() {
    use pokered_core::{battle::state::StatusCondition, game_state::Lang,
        party_screen::PartyScreenState, pokemon::stats::create_pokemon};
    use pokered_data::{species::Species, ui_layout::schema::PARTY_DEFAULT_LAYOUT};
    let mut mon = create_pokemon(Species::Chansey, 100, [0xff, 0xff]).unwrap();
    mon.status = StatusCondition::Poison;
    let state = PartyScreenState::new(vec![mon; 6]);
    let mut rec = Recorder::default();
    pokered_ui::menus::party::draw(&state, &PARTY_DEFAULT_LAYOUT, &mut Ui::new(&mut rec), Lang::En);
    let labels: Vec<_> = rec.ops.iter().filter_map(|op| match op {
        Op::PixelText(x, y, text, _) => Some((*x, *y, text)),
        _ => None,
    }).collect();
    assert_eq!(labels.len(), 24);
    for (i, (x, y, text)) in labels.iter().enumerate() {
        let right = x + pokered_renderer::embedded_font::measure_text(text);
        assert!(right <= 152 && y + 10 <= 144, "{text} outside screen");
        if text.starts_with("Lv") || text.contains('/') { assert_eq!(right, 152); }
        for (other_x, other_y, other) in &labels[i + 1..] {
            let other_right = other_x + pokered_renderer::embedded_font::measure_text(other);
            assert!(right <= *other_x || other_right <= *x || y + 10 <= *other_y || other_y + 10 <= *y,
                "{text} overlaps {other}");
        }
    }
}

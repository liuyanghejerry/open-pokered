//! PrintMenuItem shows "disabled!" instead of TYPE and PP for a disabled row.
use pokered_core::{
    battle::menu::{MoveMenuState, MoveSlot},
    game_state::Lang,
};
use pokered_data::{
    impl_traits::PokemonRenderData, moves::MoveId, ui_layout::schema::BATTLE_MOVE_DEFAULT_LAYOUT,
};
use pokered_ui::{menus::battle_move, Painter, Rgba, TilePos, TileRect, Ui};

struct Recorder {
    proportional: bool,
    texts: Vec<(u32, u32, String)>,
}
impl Recorder {
    fn new(proportional: bool) -> Self {
        Self {
            proportional,
            texts: Vec::new(),
        }
    }
    fn info(&self) -> Vec<&str> {
        self.texts
            .iter()
            .filter(|(x, y, _)| (8..80).contains(x) && (72..96).contains(y))
            .map(|(_, _, text)| text.as_str())
            .collect()
    }
}
impl Painter for Recorder {
    fn clear(&mut self, _: Rgba) {}
    fn draw_text_box(&mut self, _: TileRect, _: Rgba) {}
    fn draw_text(&mut self, p: TilePos, text: &str, _: Rgba) {
        self.texts.push((p.tx * 8, p.ty * 8, text.into()));
    }
    fn draw_glyph(&mut self, _: TilePos, _: char, _: Rgba) {}
    fn draw_pixel_rect(&mut self, _: u32, _: u32, _: u32, _: u32, _: Rgba) {}
    fn draw_gb_tile(&mut self, _: TilePos, _: u8, _: &str, _: Rgba) {}
    fn supports_proportional(&self) -> bool {
        self.proportional
    }
    fn draw_text_px(&mut self, x: u32, y: u32, text: &str, _: Rgba) {
        self.texts.push((x, y, text.into()));
    }
}
fn menu(pp: u8) -> MoveMenuState {
    MoveMenuState::new(vec![
        MoveSlot {
            move_id: MoveId::Thundershock,
            current_pp: pp,
            max_pp: 30,
            is_disabled: true,
        },
        MoveSlot {
            move_id: MoveId::Growl,
            current_pp: 9,
            max_pp: 40,
            is_disabled: false,
        },
    ])
}
#[test]
fn disabled_info_replaces_type_and_pp_for_tile_and_pixel_painters() {
    for proportional in [false, true] {
        for pp in [0, 5] {
            let state = menu(pp);
            let before = state.moves().to_vec();
            let mut rec = Recorder::new(proportional);
            battle_move::draw(
                &state,
                &BATTLE_MOVE_DEFAULT_LAYOUT,
                &mut Ui::new(&mut rec),
                Lang::En,
                &PokemonRenderData::new(false),
            );
            assert_eq!(
                rec.info(),
                vec!["disabled!"],
                "source PrintMenuItem omits all TYPE/PP labels, even at zeroPP"
            );
            assert_eq!(
                state.moves(),
                before.as_slice(),
                "drawing cannot change moves, PP or Disable"
            );
        }
    }
}
#[test]
fn cursor_redraw_enters_disabled_info_and_restores_normal_info() {
    for proportional in [false, true] {
        let mut state = menu(0);
        let mut rec = Recorder::new(proportional);
        battle_move::redraw_selection(
            1,
            &state,
            &BATTLE_MOVE_DEFAULT_LAYOUT,
            &mut Ui::new(&mut rec),
            Lang::En,
            &PokemonRenderData::new(false),
        );
        assert_eq!(rec.info(), vec!["disabled!"]);
        state.set_cursor(1);
        rec.texts.clear();
        battle_move::redraw_selection(
            0,
            &state,
            &BATTLE_MOVE_DEFAULT_LAYOUT,
            &mut Ui::new(&mut rec),
            Lang::En,
            &PokemonRenderData::new(false),
        );
        let info = rec.info().join(" ");
        let compact = info.replace(' ', "");
        assert!(
            info.contains("TYPE")
                && info.contains("NORMAL")
                && compact.contains("9/40")
                && !info.contains("disabled!"),
            "{info}"
        );
    }
}

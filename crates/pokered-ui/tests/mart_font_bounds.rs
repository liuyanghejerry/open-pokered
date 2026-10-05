//! Project-font prices/quantities and mixed Chinese item labels must fit.
use pokered_core::game_state::Lang;
use pokered_data::{
    impl_traits::PokemonRenderData,
    items::ItemId,
    ui_layout::schema::{MART_BUY_ITEMS_WITH_MONEY_LAYOUT, MART_SELL_ITEMS_WITH_MONEY_LAYOUT},
};
use pokered_ui::{menus::mart, Painter, Rgba, TilePos, TileRect, Ui};

#[derive(Default)]
struct Recorder {
    texts: Vec<(u32, u32, String)>,
}
impl Painter for Recorder {
    fn clear(&mut self, _: Rgba) {}
    fn draw_text_box(&mut self, _: TileRect, _: Rgba) {}
    fn draw_text(&mut self, p: TilePos, text: &str, _: Rgba) {
        self.texts.push((p.tx * 8, p.ty * 8, text.into()));
    }
    fn draw_text_px(&mut self, x: u32, y: u32, text: &str, _: Rgba) {
        self.texts.push((x, y, text.into()));
    }
    fn measure_text_px(&self, text: &str) -> u32 {
        pokered_data::text_layout::measure_text(text)
    }
    fn draw_glyph(&mut self, _: TilePos, _: char, _: Rgba) {}
    fn draw_pixel_rect(&mut self, _: u32, _: u32, _: u32, _: u32, _: Rgba) {}
    fn draw_gb_tile(&mut self, _: TilePos, _: u8, _: &str, _: Rgba) {}
}
fn height(_text: &str) -> u32 {
    10
}

#[test]
fn buy_prices_share_item_baselines_and_align_at_the_right_edge() {
    let mut rec = Recorder::default();
    mart::draw_buy_items_with_money(
        &[ItemId::ThunderStone, ItemId::UltraBall],
        0,
        0,
        999999,
        &MART_BUY_ITEMS_WITH_MONEY_LAYOUT,
        &mut Ui::new(&mut rec),
        Lang::En,
        &PokemonRenderData::new(false),
    );
    assert!(rec
        .texts
        .iter()
        .any(|(x, y, t)| (*x, *y) == (48, 32) && t == "THUNDERSTONE"));
    assert!(rec
        .texts
        .iter()
        .any(|(x, y, t)| (*x, *y) == (119, 32) && t.trim() == "$2100"));
    let prices: Vec<_> = rec.texts.iter().filter(|(_, y, t)| *y > 12 && t.starts_with('$')).collect();
    assert_eq!(prices.len(), 2);
    for (x, _, text) in prices { assert_eq!(x + rec.measure_text_px(text), 144); }
    assert!(rec.texts.contains(&(104, 0, "MONEY".into())));
    assert!(rec.texts.contains(&(96, 12, "$999999".into())));
    for (x, _, text) in &rec.texts {
        assert!(
            x + rec.measure_text_px(text) <= 152,
            "{text} overwrites the right border"
        );
    }
}

#[test]
fn chinese_shop_names_prices_and_quantities_do_not_share_ink_regions() {
    for sell in [false, true] {
        let mut rec = Recorder::default();
        let data = PokemonRenderData::new(true);
        let items = [ItemId::ThunderStone, ItemId::UltraBall, ItemId::SuperPotion];
        if sell {
            mart::draw_sell_items_with_money(
                &items.map(|i| (i, 99)),
                0,
                0,
                999999,
                &MART_SELL_ITEMS_WITH_MONEY_LAYOUT,
                &mut Ui::new(&mut rec),
                Lang::Zh,
                &data,
            );
        } else {
            mart::draw_buy_items_with_money(
                &items,
                0,
                0,
                999999,
                &MART_BUY_ITEMS_WITH_MONEY_LAYOUT,
                &mut Ui::new(&mut rec),
                Lang::Zh,
                &data,
            );
        }
        for (i, (x, y, text)) in rec.texts.iter().enumerate() {
            let right = x + rec.measure_text_px(text);
            assert!(
                right <= 152 && y + height(text) <= 112,
                "{text} leaves its box"
            );
            for (ox, oy, other) in &rec.texts[i + 1..] {
                let other_right = ox + rec.measure_text_px(other);
                assert!(
                    right <= *ox
                        || other_right <= *x
                        || y + height(text) <= *oy
                        || oy + height(other) <= *y,
                    "{text} overlaps {other}"
                );
            }
        }
    }
}

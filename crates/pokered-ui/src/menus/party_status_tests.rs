use super::*;
use crate::engine::{Rgba, TilePos};
use pokered_core::pokemon::stats::create_pokemon;
use pokered_data::species::Species;

#[derive(Default)]
struct Labels(Vec<(u32, u32, String)>);

impl Painter for Labels {
    fn clear(&mut self, _: Rgba) {}
    fn draw_text_box(&mut self, _: TileRect, _: Rgba) {}
    fn draw_text(&mut self, pos: TilePos, text: &str, color: Rgba) {
        self.draw_text_px(pos.tx * 8, pos.ty * 8, text, color);
    }
    fn draw_text_px(&mut self, x: u32, y: u32, text: &str, _: Rgba) {
        self.0.push((x, y, text.to_string()));
    }
    fn measure_text_px(&self, text: &str) -> u32 {
        text.chars().map(|ch| if ch == 'W' { 8 } else { 6 }).sum()
    }
    fn draw_glyph(&mut self, _: TilePos, _: char, _: Rgba) {}
    fn draw_pixel_rect(&mut self, _: u32, _: u32, _: u32, _: u32, _: Rgba) {}
    fn draw_gb_tile(&mut self, _: TilePos, _: u8, _: &str, _: Rgba) {}
}

fn ailments() -> [(StatusCondition, &'static str); 6] {
    [
        (StatusCondition::None, ""),
        (StatusCondition::Sleep(2), "SLP"),
        (StatusCondition::Poison, "PSN"),
        (StatusCondition::Burn, "BRN"),
        (StatusCondition::Freeze, "FRZ"),
        (StatusCondition::Paralysis, "PAR"),
    ]
}

fn render(mon: &Pokemon) -> Labels {
    let mut labels = Labels::default();
    draw_entry(&mut labels, mon, 0, &PARTY_ENTRY_LAYOUT);
    labels
}

fn status_text(labels: &Labels) -> &str {
    let pos = PARTY_ENTRY_LAYOUT
        .dynamic_labels
        .iter()
        .find_map(|(key, label)| (key == "status").then_some(label))
        .unwrap();
    labels
        .0
        .iter()
        .find(|(x, y, _)| *x == pos.tx * 8 && *y == pos.ty * 8)
        .map(|(_, _, text)| text.as_str())
        .unwrap_or("")
}

#[test]
fn fainted_entry_prints_fnt_before_every_ailment() {
    let mut mon = create_pokemon(Species::Bulbasaur, 12, [0x9a, 0x78]).unwrap();
    mon.hp = 0;
    for (status, _) in ailments() {
        mon.status = status;
        let labels = render(&mon);
        assert_eq!(status_text(&labels), "FNT", "{status:?}");
        assert!(labels
            .0
            .iter()
            .any(|(_, _, text)| text == &format!("0/{}", mon.max_hp)));
    }
}

#[test]
fn living_entries_keep_ailments_at_one_hp_and_nonzero_high_hp_byte() {
    for (species, level, hp) in [(Species::Bulbasaur, 12, 1), (Species::Snorlax, 100, 256)] {
        let mut mon = create_pokemon(species, level, [0xff, 0xff]).unwrap();
        assert!(hp <= mon.max_hp, "control HP must be legal");
        mon.hp = hp;
        for (status, expected) in ailments() {
            mon.status = status;
            assert_eq!(status_text(&render(&mon)), expected, "HP {hp}, {status:?}");
        }
    }
}

#[test]
fn long_name_reserves_a_tile_for_visible_fnt_or_ailment_only() {
    let mut mon = create_pokemon(Species::Bulbasaur, 12, [0x9a, 0x78]).unwrap();
    mon.set_nickname("WWWWWWWWWW");
    for (hp, status, expected) in [
        (0, StatusCondition::None, "FNT"),
        (0, StatusCondition::Poison, "FNT"),
        (1, StatusCondition::Poison, "PSN"),
        (1, StatusCondition::None, ""),
    ] {
        mon.hp = hp;
        mon.status = status;
        let labels = render(&mon);
        assert_eq!(status_text(&labels), expected);
        let (x, y, name) = &labels.0[0];
        if expected.is_empty() {
            assert_eq!(name, "WWWWWWWWWW", "healthy name keeps its full room");
            let level = labels.0.iter().find(|(_, _, text)| text == "Lv12").unwrap();
            assert!(*x + labels.measure_text_px(name) + 8 <= level.0);
        } else {
            let label = labels
                .0
                .iter()
                .find(|(_, _, text)| text == expected)
                .unwrap();
            assert_eq!(*y, label.1);
            assert!(
                *x + labels.measure_text_px(name) + 8 <= label.0,
                "name must not overlap status"
            );
            assert!(
                *x + labels.measure_text_px(&format!("{name}W")) + 8 > label.0,
                "reserve only the required tile, not extra name space"
            );
        }
    }
}

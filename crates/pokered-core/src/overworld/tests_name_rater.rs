//! Original NameRatersHouse.asm branches through the embedded scene interpreter.
//! Constructed query/party fixtures; ordinary CLI and SRAM captures are separate.
use super::{Direction, OverworldInput, OverworldScreen};
use crate::alloc_prelude::*;
use crate::naming_screen::NamingInput;
use crate::party_screen::PartyScreenInput;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId, species::Species};

type Field = OverworldScreen<PokemonRedData>;

fn tick(field: &mut Field, a: bool) {
    field.update_frame(OverworldInput::new(
        false, false, false, false, a, false, false, false,
    ));
    if field.take_party_select_request() {
        let mon = crate::pokemon::stats::create_pokemon(Species::Bulbasaur, 5, [0xff; 2]).unwrap();
        field.begin_party_select(vec![mon]);
    }
}

fn rater(owned: bool) -> Field {
    let mut field = Field::new(MapId::NameRatersHouse, None, PokemonRedData);
    field.run_on_load();
    for _ in 0..64 {
        tick(&mut field, false);
    }
    field.state.player.x = 5;
    field.state.player.y = 4;
    field.state.player.facing = Direction::Up;
    field.seed_daycare_query_state(false, "", 0, 0, &["SPROUT".into()], &[false]);
    field.seed_party_rename_query_state(&[owned]);
    field
}

fn text(field: &Field) -> String {
    field
        .pending_dialogue
        .as_ref()
        .map(|dialogue| {
            dialogue
                .pages()
                .iter()
                .map(|page| format!("{} {}", page.line1, page.line2))
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

fn advance(field: &mut Field, ready: impl Fn(&Field) -> bool) {
    for frame in 0..6000 {
        if ready(field) {
            return;
        }
        assert!(!field.is_naming_screen_active(), "unexpected naming modal");
        assert!(!field.is_party_select_active(), "unexpected party modal");
        tick(field, frame % 16 == 0);
    }
    panic!(
        "scene did not reach requested branch; effect {:?}",
        field.active_script_effect
    );
}

fn select_mon(field: &mut Field, cancel: bool) {
    advance(field, |field| field.is_party_select_active());
    field.update_party_select_input(PartyScreenInput {
        up: false,
        down: false,
        a: !cancel,
        b: cancel,
    });
}

fn naming(field: &mut Field) {
    select_mon(field, false);
    advance(field, |field| field.is_naming_screen_active());
    for _ in 0..64 {
        field.update_naming_input(NamingInput::none(), false);
    }
}

fn finish(field: &mut Field) {
    advance(field, |field| {
        field.script_engine.is_idle()
            && field.active_script_effect.is_none()
            && field.pending_dialogue.is_none()
    });
}

fn choose_no(field: &mut Field) {
    advance(field, |field| field.pending_choice.is_some());
    for _ in 0..64 {
        tick(field, false);
    }
    for _ in 0..3 {
        field.update_frame(OverworldInput::new(
            false, true, false, false, false, false, false, false,
        ));
    }
    assert_eq!(field.pending_choice.as_ref().unwrap().selected, 1);
    for _ in 0..16 {
        tick(field, false);
    }
    for _ in 0..3 {
        tick(field, true);
    }
}

#[test]
fn name_rater_owned_offer_names_the_selected_mon() {
    let mut field = rater(true);
    select_mon(&mut field, false);
    advance(&mut field, |field| text(field).contains("decent nickname!"));
    assert!(
        text(&field).starts_with("SPROUT, is it? That is a decent nickname!"),
        "{}",
        text(&field)
    );
}

#[test]
fn name_rater_party_cancel_prints_farewell_without_a_write() {
    let mut field = rater(true);
    select_mon(&mut field, true);
    advance(&mut field, |field| {
        field.pending_dialogue.is_some() || field.script_engine.is_idle()
    });
    assert_eq!(text(&field), "Fine! Come any time you like!");
    finish(&mut field);
    assert!(field.pending_set_nickname.is_none());
}

#[test]
fn name_rater_empty_name_prints_farewell_without_a_write() {
    let mut field = rater(true);
    naming(&mut field);
    field.update_naming_input(
        NamingInput {
            start: true,
            ..NamingInput::none()
        },
        false,
    );
    advance(&mut field, |field| field.pending_dialogue.is_some());
    assert_eq!(text(&field), "Fine! Come any time you like!");
    finish(&mut field);
    assert!(field.pending_set_nickname.is_none());
}

#[test]
fn name_rater_success_confirms_the_new_name_and_writes_once() {
    let mut field = rater(true);
    naming(&mut field);
    field.update_naming_input(
        NamingInput {
            a: true,
            ..NamingInput::none()
        },
        false,
    );
    field.update_naming_input(
        NamingInput {
            start: true,
            ..NamingInput::none()
        },
        false,
    );
    advance(&mut field, |field| text(field).contains("has been renamed"));
    assert!(
        text(&field).contains("has been renamed A!"),
        "{}",
        text(&field)
    );
    assert_eq!(field.pending_set_nickname.take(), Some((0, "A".into())));
    finish(&mut field);
    assert!(field.pending_set_nickname.is_none());
}

#[test]
fn name_rater_foreign_mon_keeps_its_name_without_a_naming_screen() {
    let mut field = rater(false);
    select_mon(&mut field, false);
    advance(&mut field, |field| text(field).contains("impeccable name!"));
    assert!(text(&field).starts_with("SPROUT, is it?"));
    assert!(text(&field).contains("Take good care of SPROUT!"));
    finish(&mut field);
    assert!(field.pending_set_nickname.is_none());
}

#[test]
fn name_rater_declining_rating_prints_farewell_without_a_write() {
    let mut field = rater(true);
    choose_no(&mut field);
    advance(&mut field, |field| field.pending_dialogue.is_some());
    assert_eq!(text(&field), "Fine! Come any time you like!");
    finish(&mut field);
    assert!(field.pending_set_nickname.is_none());
}

#[test]
fn name_rater_declining_rename_prints_farewell_without_a_write() {
    let mut field = rater(true);
    select_mon(&mut field, false);
    choose_no(&mut field);
    advance(&mut field, |field| field.pending_dialogue.is_some());
    assert_eq!(text(&field), "Fine! Come any time you like!");
    finish(&mut field);
    assert!(field.pending_set_nickname.is_none());
}

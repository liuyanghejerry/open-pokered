use crate::start_menu::*;

fn input_none() -> StartMenuInput {
    StartMenuInput::none()
}

fn input_a() -> StartMenuInput {
    StartMenuInput {
        a: true,
        ..input_none()
    }
}

fn input_b() -> StartMenuInput {
    StartMenuInput {
        b: true,
        ..input_none()
    }
}

fn input_start() -> StartMenuInput {
    StartMenuInput {
        start: true,
        ..input_none()
    }
}

fn input_up() -> StartMenuInput {
    StartMenuInput {
        up: true,
        ..input_none()
    }
}

fn input_down() -> StartMenuInput {
    StartMenuInput {
        down: true,
        ..input_none()
    }
}

#[test]
fn with_pokedex_has_seven_items() {
    let menu = StartMenuState::new(true, true, false);
    assert_eq!(menu.item_count(), 7);
    assert_eq!(menu.items()[0], StartMenuItem::Pokedex);
    assert_eq!(menu.items()[1], StartMenuItem::Pokemon);
    assert_eq!(menu.items()[2], StartMenuItem::Item);
    assert_eq!(menu.items()[3], StartMenuItem::TrainerInfo);
    assert_eq!(menu.items()[4], StartMenuItem::Save);
    assert_eq!(menu.items()[5], StartMenuItem::Option);
    assert_eq!(menu.items()[6], StartMenuItem::Exit);
}

#[test]
fn without_pokedex_has_six_items() {
    let menu = StartMenuState::new(false, true, false);
    assert_eq!(menu.item_count(), 6);
    assert_eq!(menu.items()[0], StartMenuItem::Pokemon);
    assert_eq!(menu.items()[1], StartMenuItem::Item);
    assert_eq!(menu.items()[2], StartMenuItem::TrainerInfo);
    assert_eq!(menu.items()[3], StartMenuItem::Save);
    assert_eq!(menu.items()[4], StartMenuItem::Option);
    assert_eq!(menu.items()[5], StartMenuItem::Exit);
}

#[test]
fn link_mode_shows_reset_instead_of_save() {
    let menu = StartMenuState::new(true, true, true);
    assert_eq!(menu.item_count(), 7);
    assert_eq!(menu.items()[4], StartMenuItem::Reset);
}

#[test]
fn link_mode_without_pokedex() {
    let menu = StartMenuState::new(false, true, true);
    assert_eq!(menu.item_count(), 6);
    assert_eq!(menu.items()[3], StartMenuItem::Reset);
}

#[test]
fn b_button_closes_menu() {
    let mut menu = StartMenuState::new(true, true, false);
    let action = menu.update_frame(input_b());
    assert_eq!(action, StartMenuAction::Close);
}

#[test]
fn start_button_closes_menu() {
    let mut menu = StartMenuState::new(true, true, false);
    let action = menu.update_frame(input_start());
    assert_eq!(action, StartMenuAction::Close);
}

#[test]
fn cursor_wraps_up_from_top() {
    let mut menu = StartMenuState::new(true, true, false);
    assert_eq!(menu.cursor(), 0);
    menu.update_frame(input_up());
    assert_eq!(menu.cursor(), 6);
}

#[test]
fn cursor_wraps_down_from_bottom() {
    let mut menu = StartMenuState::new(false, true, false);
    for _ in 0..5 {
        menu.update_frame(input_down());
    }
    assert_eq!(menu.cursor(), 5);
    menu.update_frame(input_down());
    assert_eq!(menu.cursor(), 0);
}

#[test]
fn select_pokedex() {
    let mut menu = StartMenuState::new(true, true, false);
    let action = menu.update_frame(input_a());
    assert_eq!(action, StartMenuAction::OpenPokedex);
}

#[test]
fn select_pokemon_without_pokedex() {
    let mut menu = StartMenuState::new(false, true, false);
    let action = menu.update_frame(input_a());
    assert_eq!(action, StartMenuAction::OpenPokemon);
}

#[test]
fn select_exit_closes_menu() {
    let mut menu = StartMenuState::new(true, true, false);
    for _ in 0..6 {
        menu.update_frame(input_down());
    }
    assert_eq!(menu.current_item(), StartMenuItem::Exit);
    let action = menu.update_frame(input_a());
    assert_eq!(action, StartMenuAction::Close);
}

#[test]
fn select_save_in_normal_mode() {
    let mut menu = StartMenuState::new(true, true, false);
    for _ in 0..4 {
        menu.update_frame(input_down());
    }
    assert_eq!(menu.current_item(), StartMenuItem::Save);
    let action = menu.update_frame(input_a());
    assert_eq!(action, StartMenuAction::OpenSave);
}

#[test]
fn select_reset_in_link_mode() {
    let mut menu = StartMenuState::new(true, true, true);
    for _ in 0..4 {
        menu.update_frame(input_down());
    }
    assert_eq!(menu.current_item(), StartMenuItem::Reset);
    let action = menu.update_frame(input_a());
    assert_eq!(action, StartMenuAction::TriggerReset);
}

#[test]
fn saved_cursor_persists_across_close_and_reopen() {
    let mut menu = StartMenuState::new(true, true, false);
    menu.update_frame(input_down());
    menu.update_frame(input_down());
    assert_eq!(menu.cursor(), 2);
    menu.update_frame(input_b());
    assert_eq!(menu.saved_cursor(), 2);

    menu.open(true, true, false);
    assert_eq!(menu.cursor(), 2);
}

#[test]
fn saved_cursor_clamped_when_pokedex_lost() {
    let mut menu = StartMenuState::new(true, true, false);
    for _ in 0..6 {
        menu.update_frame(input_down());
    }
    assert_eq!(menu.cursor(), 6);
    menu.update_frame(input_b());
    assert_eq!(menu.saved_cursor(), 6);

    menu.open(false, true, false);
    assert_eq!(menu.cursor(), 5);
}

#[test]
fn item_labels_with_pokedex() {
    let menu = StartMenuState::new(true, true, false);
    let labels = menu.item_labels("RED");
    assert_eq!(labels.len(), 7);
    assert_eq!(labels[0].as_str(), "POKéDEX");
    assert_eq!(labels[3].as_str(), "RED");
    assert_eq!(labels[6].as_str(), "EXIT");
}

#[test]
fn item_labels_without_pokedex() {
    let menu = StartMenuState::new(false, true, false);
    let labels = menu.item_labels("ASH");
    assert_eq!(labels.len(), 6);
    assert_eq!(labels[0].as_str(), "POKéMON");
    assert_eq!(labels[2].as_str(), "ASH");
}

#[test]
fn redisplay_restores_saved_cursor() {
    let mut menu = StartMenuState::new(true, true, false);
    menu.update_frame(input_down());
    menu.update_frame(input_down());
    menu.update_frame(input_down());
    let action = menu.update_frame(input_a());
    assert_eq!(action, StartMenuAction::OpenTrainerInfo);
    assert_eq!(menu.saved_cursor(), 3);

    menu.redisplay();
    assert_eq!(menu.cursor(), 3);
}

#[test]
fn no_input_returns_redisplay() {
    let mut menu = StartMenuState::new(true, true, false);
    let action = menu.update_frame(input_none());
    assert_eq!(action, StartMenuAction::Redisplay);
}

#[test]
fn select_all_items_with_pokedex() {
    let expected = [
        StartMenuAction::OpenPokedex,
        StartMenuAction::OpenPokemon,
        StartMenuAction::OpenItem,
        StartMenuAction::OpenTrainerInfo,
        StartMenuAction::OpenSave,
        StartMenuAction::OpenOption,
        StartMenuAction::Close,
    ];
    for (i, expected_action) in expected.iter().enumerate() {
        let mut menu = StartMenuState::new(true, true, false);
        for _ in 0..i {
            menu.update_frame(input_down());
        }
        let action = menu.update_frame(input_a());
        assert_eq!(action, *expected_action, "item index {i}");
    }
}

#[test]
fn without_pokemon_omits_pokemon_item() {
    let menu = StartMenuState::new(false, false, false);
    assert_eq!(menu.item_count(), 5);
    assert_eq!(menu.items()[0], StartMenuItem::Item);
    assert_eq!(menu.items()[1], StartMenuItem::TrainerInfo);
    assert_eq!(menu.items()[2], StartMenuItem::Save);
    assert_eq!(menu.items()[3], StartMenuItem::Option);
    assert_eq!(menu.items()[4], StartMenuItem::Exit);
}

#[test]
fn without_pokemon_with_pokedex() {
    let menu = StartMenuState::new(true, false, false);
    assert_eq!(menu.item_count(), 6);
    assert_eq!(menu.items()[0], StartMenuItem::Pokedex);
    assert_eq!(menu.items()[1], StartMenuItem::Item);
    assert_eq!(menu.items()[2], StartMenuItem::TrainerInfo);
    assert_eq!(menu.items()[3], StartMenuItem::Save);
    assert_eq!(menu.items()[4], StartMenuItem::Option);
    assert_eq!(menu.items()[5], StartMenuItem::Exit);
}

#[test]
fn without_pokemon_first_item_is_item() {
    let mut menu = StartMenuState::new(false, false, false);
    let action = menu.update_frame(input_a());
    assert_eq!(action, StartMenuAction::OpenItem);
}

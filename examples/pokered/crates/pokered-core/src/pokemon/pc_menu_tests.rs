use super::pc_menu::*;
use crate::main_menu::MenuInput;

fn menu_none() -> MenuInput {
    MenuInput::none()
}

fn menu_up() -> MenuInput {
    MenuInput {
        up: true,
        ..MenuInput::none()
    }
}

fn menu_down() -> MenuInput {
    MenuInput {
        down: true,
        ..MenuInput::none()
    }
}

fn menu_a() -> MenuInput {
    MenuInput {
        a: true,
        ..MenuInput::none()
    }
}

fn menu_b() -> MenuInput {
    MenuInput {
        b: true,
        ..MenuInput::none()
    }
}

#[test]
fn pc_main_menu_no_pokedex_no_league() {
    let menu = PcMainMenuState::new(false, false, false);
    assert_eq!(menu.item_count(), 3);
    assert_eq!(menu.items()[0], PcMainMenuTarget::BillsPc);
    assert_eq!(menu.items()[1], PcMainMenuTarget::PlayersPc);
    assert_eq!(menu.items()[2], PcMainMenuTarget::LogOff);
}

#[test]
fn pc_main_menu_with_pokedex_no_league() {
    let menu = PcMainMenuState::new(true, false, true);
    assert_eq!(menu.item_count(), 4);
    assert_eq!(menu.items()[2], PcMainMenuTarget::OaksPc);
    assert_eq!(menu.items()[3], PcMainMenuTarget::LogOff);
}

#[test]
fn pc_main_menu_with_pokedex_and_league() {
    let menu = PcMainMenuState::new(true, true, true);
    assert_eq!(menu.item_count(), 5);
    assert_eq!(menu.items()[3], PcMainMenuTarget::PkmnLeague);
    assert_eq!(menu.items()[4], PcMainMenuTarget::LogOff);
}

#[test]
fn pc_main_menu_met_bill_flag() {
    let menu = PcMainMenuState::new(false, false, true);
    assert!(menu.met_bill());
    let menu2 = PcMainMenuState::new(false, false, false);
    assert!(!menu2.met_bill());
}

#[test]
fn pc_main_menu_select_bills_pc() {
    let mut menu = PcMainMenuState::new(false, false, false);
    let result = menu.update_frame(menu_a());
    assert_eq!(result, Some(PcMainMenuTarget::BillsPc));
}

#[test]
fn pc_main_menu_b_logs_off() {
    let mut menu = PcMainMenuState::new(true, true, true);
    let result = menu.update_frame(menu_b());
    assert_eq!(result, Some(PcMainMenuTarget::LogOff));
}

#[test]
fn pc_main_menu_cursor_wraps() {
    let mut menu = PcMainMenuState::new(false, false, false);
    menu.update_frame(menu_up());
    assert_eq!(menu.cursor(), 2);
    menu.update_frame(menu_down());
    assert_eq!(menu.cursor(), 0);
}

#[test]
fn pc_main_menu_navigate_to_players_pc() {
    let mut menu = PcMainMenuState::new(false, false, false);
    menu.update_frame(menu_down());
    let result = menu.update_frame(menu_a());
    assert_eq!(result, Some(PcMainMenuTarget::PlayersPc));
}

#[test]
fn pc_main_menu_no_input_returns_none() {
    let mut menu = PcMainMenuState::new(false, false, false);
    assert!(menu.update_frame(menu_none()).is_none());
}

#[test]
fn bills_pc_menu_initial_state() {
    let menu = BillsPcMenuState::new(0);
    assert_eq!(menu.cursor(), 0);
    assert_eq!(menu.current_box(), 0);
    assert_eq!(menu.item_count(), 5);
    assert_eq!(menu.current_action(), BillsPcAction::Withdraw);
}

#[test]
fn bills_pc_menu_all_actions() {
    let expected = [
        BillsPcAction::Withdraw,
        BillsPcAction::Deposit,
        BillsPcAction::Release,
        BillsPcAction::ChangeBox,
        BillsPcAction::Exit,
    ];
    for (i, action) in expected.iter().enumerate() {
        let mut menu = BillsPcMenuState::new(0);
        for _ in 0..i {
            menu.update_frame(menu_down());
        }
        let result = menu.update_frame(menu_a());
        assert_eq!(result, Some(*action), "index={i}");
    }
}

#[test]
fn bills_pc_menu_b_exits() {
    let mut menu = BillsPcMenuState::new(3);
    let result = menu.update_frame(menu_b());
    assert_eq!(result, Some(BillsPcAction::Exit));
}

#[test]
fn bills_pc_menu_cursor_wraps() {
    let mut menu = BillsPcMenuState::new(0);
    menu.update_frame(menu_up());
    assert_eq!(menu.cursor(), 4);
    assert_eq!(menu.current_action(), BillsPcAction::Exit);
    menu.update_frame(menu_down());
    assert_eq!(menu.cursor(), 0);
}

#[test]
fn bills_pc_menu_saved_cursor() {
    let mut menu = BillsPcMenuState::new(0);
    menu.update_frame(menu_down());
    menu.update_frame(menu_down());
    menu.update_frame(menu_a());
    assert_eq!(menu.cursor(), 2);

    menu.update_frame(menu_down());
    menu.restore_saved_cursor();
    assert_eq!(menu.cursor(), 2);
    assert_eq!(menu.current_action(), BillsPcAction::Release);
}

#[test]
fn bills_pc_menu_set_current_box() {
    let mut menu = BillsPcMenuState::new(0);
    assert_eq!(menu.current_box(), 0);
    menu.set_current_box(11);
    assert_eq!(menu.current_box(), 11);
}

#[test]
fn players_pc_menu_initial_state() {
    let menu = PlayersPcMenuState::new();
    assert_eq!(menu.cursor(), 0);
    assert_eq!(menu.item_count(), 4);
    assert_eq!(menu.current_action(), PlayersPcAction::WithdrawItem);
}

#[test]
fn players_pc_menu_all_actions() {
    let expected = [
        PlayersPcAction::WithdrawItem,
        PlayersPcAction::DepositItem,
        PlayersPcAction::TossItem,
        PlayersPcAction::LogOff,
    ];
    for (i, action) in expected.iter().enumerate() {
        let mut menu = PlayersPcMenuState::new();
        for _ in 0..i {
            menu.update_frame(menu_down());
        }
        let result = menu.update_frame(menu_a());
        assert_eq!(result, Some(*action), "index={i}");
    }
}

#[test]
fn players_pc_menu_b_logs_off() {
    let mut menu = PlayersPcMenuState::new();
    let result = menu.update_frame(menu_b());
    assert_eq!(result, Some(PlayersPcAction::LogOff));
}

#[test]
fn players_pc_menu_cursor_wraps() {
    let mut menu = PlayersPcMenuState::new();
    menu.update_frame(menu_up());
    assert_eq!(menu.cursor(), 3);
    menu.update_frame(menu_down());
    assert_eq!(menu.cursor(), 0);
}

#[test]
fn players_pc_menu_saved_cursor() {
    let mut menu = PlayersPcMenuState::new();
    menu.update_frame(menu_down());
    menu.update_frame(menu_a());
    menu.restore_saved_cursor();
    assert_eq!(menu.cursor(), 1);
    assert_eq!(menu.current_action(), PlayersPcAction::DepositItem);
}

#[test]
fn players_pc_default() {
    let menu = PlayersPcMenuState::default();
    assert_eq!(menu.cursor(), 0);
}

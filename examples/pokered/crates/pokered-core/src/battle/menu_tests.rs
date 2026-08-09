use crate::battle::menu::*;
use crate::main_menu::MenuInput;
use pokered_data::moves::MoveId;

fn input_none() -> BattleMenuInput {
    BattleMenuInput::none()
}

fn input_a() -> BattleMenuInput {
    BattleMenuInput {
        a: true,
        ..BattleMenuInput::none()
    }
}

fn input_b() -> BattleMenuInput {
    BattleMenuInput {
        b: true,
        ..BattleMenuInput::none()
    }
}

fn input_up() -> BattleMenuInput {
    BattleMenuInput {
        up: true,
        ..BattleMenuInput::none()
    }
}

fn input_down() -> BattleMenuInput {
    BattleMenuInput {
        down: true,
        ..BattleMenuInput::none()
    }
}

fn input_left() -> BattleMenuInput {
    BattleMenuInput {
        left: true,
        ..BattleMenuInput::none()
    }
}

fn input_right() -> BattleMenuInput {
    BattleMenuInput {
        right: true,
        ..BattleMenuInput::none()
    }
}

fn menu_up() -> MenuInput {
    MenuInput {
        up: true,
        down: false,
        a: false,
        b: false,
    }
}

fn menu_down() -> MenuInput {
    MenuInput {
        up: false,
        down: true,
        a: false,
        b: false,
    }
}

fn menu_a() -> MenuInput {
    MenuInput {
        up: false,
        down: false,
        a: true,
        b: false,
    }
}

fn menu_b() -> MenuInput {
    MenuInput {
        up: false,
        down: false,
        a: false,
        b: true,
    }
}

fn make_move(id: MoveId, pp: u8, max_pp: u8, disabled: bool) -> MoveSlot {
    MoveSlot {
        move_id: id,
        current_pp: pp,
        max_pp,
        is_disabled: disabled,
    }
}

// =============================================================
// BattleMenuState tests
// =============================================================

#[test]
fn battle_menu_initial_position_is_top_left() {
    let menu = BattleMenuState::new();
    assert_eq!(menu.row(), 0);
    assert_eq!(menu.col(), 0);
    assert_eq!(menu.current_action(), BattleMenuAction::Fight);
}

#[test]
fn battle_menu_navigate_right_to_pokemon() {
    let mut menu = BattleMenuState::new();
    assert!(menu.update_frame(input_right()).is_none());
    assert_eq!(menu.current_action(), BattleMenuAction::Pokemon);
}

#[test]
fn battle_menu_navigate_down_to_item() {
    let mut menu = BattleMenuState::new();
    assert!(menu.update_frame(input_down()).is_none());
    assert_eq!(menu.current_action(), BattleMenuAction::Bag);
}

#[test]
fn battle_menu_navigate_to_run() {
    let mut menu = BattleMenuState::new();
    menu.update_frame(input_right());
    menu.update_frame(input_down());
    assert_eq!(menu.current_action(), BattleMenuAction::Run);
}

#[test]
fn battle_menu_clamp_at_top_left() {
    let mut menu = BattleMenuState::new();
    menu.update_frame(input_up());
    menu.update_frame(input_left());
    assert_eq!(menu.row(), 0);
    assert_eq!(menu.col(), 0);
}

#[test]
fn battle_menu_clamp_at_bottom_right() {
    let mut menu = BattleMenuState::new();
    menu.update_frame(input_down());
    menu.update_frame(input_right());
    menu.update_frame(input_down());
    menu.update_frame(input_right());
    assert_eq!(menu.row(), 1);
    assert_eq!(menu.col(), 1);
}

#[test]
fn battle_menu_a_selects_fight() {
    let mut menu = BattleMenuState::new();
    let result = menu.update_frame(input_a());
    assert_eq!(result, Some(BattleMenuAction::Fight));
}

#[test]
fn battle_menu_a_selects_run() {
    let mut menu = BattleMenuState::new();
    menu.update_frame(input_down());
    menu.update_frame(input_right());
    let result = menu.update_frame(input_a());
    assert_eq!(result, Some(BattleMenuAction::Run));
}

#[test]
fn battle_menu_no_input_returns_none() {
    let mut menu = BattleMenuState::new();
    assert!(menu.update_frame(input_none()).is_none());
}

#[test]
fn battle_menu_saved_position_persists() {
    let mut menu = BattleMenuState::new();
    menu.update_frame(input_down());
    menu.update_frame(input_right());
    menu.update_frame(input_a()); // selects Run, saves position (1,1)

    let mut menu2 = BattleMenuState::new();
    menu2.update_frame(input_down());
    menu2.update_frame(input_right());
    menu2.update_frame(input_a()); // saves position (1,1)

    let mut menu3 = BattleMenuState::new();
    menu3.restore_saved_position();
    assert_eq!(menu3.row(), 0);
    assert_eq!(menu3.col(), 0);

    // reuse menu2 which has saved_position=3
    menu2.restore_saved_position();
    assert_eq!(menu2.row(), 1);
    assert_eq!(menu2.col(), 1);
    assert_eq!(menu2.current_action(), BattleMenuAction::Run);
}

#[test]
fn battle_menu_all_four_actions() {
    // Grid layout matches original game display:
    //   FIGHT   | POKeMON
    //   ITEM    | RUN
    let positions: [(usize, usize, BattleMenuAction); 4] = [
        (0, 0, BattleMenuAction::Fight),
        (0, 1, BattleMenuAction::Pokemon),
        (1, 0, BattleMenuAction::Bag),
        (1, 1, BattleMenuAction::Run),
    ];
    for (row, col, expected) in &positions {
        let mut menu = BattleMenuState::new();
        if *row == 1 {
            menu.update_frame(input_down());
        }
        if *col == 1 {
            menu.update_frame(input_right());
        }
        let result = menu.update_frame(input_a());
        assert_eq!(result, Some(*expected), "row={row}, col={col}");
    }
}

#[test]
fn battle_menu_from_menu_input() {
    let mi = MenuInput {
        up: true,
        down: false,
        a: false,
        b: false,
    };
    let bmi = BattleMenuInput::from_menu_input(mi, false, true);
    assert!(bmi.up);
    assert!(!bmi.down);
    assert!(!bmi.left);
    assert!(bmi.right);
    assert!(!bmi.a);
    assert!(!bmi.b);
}

// =============================================================
// SafariBattleMenuState tests
// =============================================================

#[test]
fn safari_menu_initial_position() {
    let menu = SafariBattleMenuState::new(30);
    assert_eq!(menu.row(), 0);
    assert_eq!(menu.col(), 0);
    assert_eq!(menu.safari_balls_remaining, 30);
    assert_eq!(menu.current_action(), SafariMenuAction::Ball);
}

#[test]
fn safari_menu_all_four_actions() {
    let cases: [(usize, usize, SafariMenuAction); 4] = [
        (0, 0, SafariMenuAction::Ball),
        (0, 1, SafariMenuAction::Bait),
        (1, 0, SafariMenuAction::Rock),
        (1, 1, SafariMenuAction::Run),
    ];
    for (row, col, expected) in &cases {
        let mut menu = SafariBattleMenuState::new(30);
        if *row == 1 {
            menu.update_frame(input_down());
        }
        if *col == 1 {
            menu.update_frame(input_right());
        }
        let result = menu.update_frame(input_a());
        assert_eq!(result, Some(*expected), "row={row}, col={col}");
    }
}

#[test]
fn safari_menu_navigation_clamps() {
    let mut menu = SafariBattleMenuState::new(10);
    menu.update_frame(input_up());
    menu.update_frame(input_left());
    assert_eq!(menu.row(), 0);
    assert_eq!(menu.col(), 0);

    menu.update_frame(input_down());
    menu.update_frame(input_right());
    menu.update_frame(input_down());
    menu.update_frame(input_right());
    assert_eq!(menu.row(), 1);
    assert_eq!(menu.col(), 1);
}

// =============================================================
// MoveMenuState tests
// =============================================================

fn four_moves() -> Vec<MoveSlot> {
    vec![
        make_move(MoveId::Pound, 35, 35, false),
        make_move(MoveId::Tackle, 35, 35, false),
        make_move(MoveId::Thunderbolt, 15, 15, false),
        make_move(MoveId::Surf, 15, 15, false),
    ]
}

#[test]
fn move_menu_initial_cursor_at_zero() {
    let menu = MoveMenuState::new(four_moves());
    assert_eq!(menu.cursor(), 0);
    assert_eq!(menu.move_count(), 4);
}

#[test]
fn move_menu_cursor_wraps_down() {
    let mut menu = MoveMenuState::new(four_moves());
    for _ in 0..4 {
        menu.update_frame(menu_down());
    }
    assert_eq!(menu.cursor(), 0); // wrapped back
}

#[test]
fn move_menu_cursor_wraps_up() {
    let mut menu = MoveMenuState::new(four_moves());
    menu.update_frame(menu_up());
    assert_eq!(menu.cursor(), 3); // wrapped to last
}

#[test]
fn move_menu_select_valid_move() {
    let mut menu = MoveMenuState::new(four_moves());
    menu.update_frame(menu_down()); // cursor=1
    let result = menu.update_frame(menu_a());
    assert_eq!(result, Some(MoveMenuResult::Selected(1)));
}

#[test]
fn move_menu_select_no_pp_rejected() {
    let moves = vec![make_move(MoveId::Pound, 0, 35, false)];
    let mut menu = MoveMenuState::new(moves);
    let result = menu.update_frame(menu_a());
    assert_eq!(result, Some(MoveMenuResult::NoPP(0)));
}

#[test]
fn move_menu_select_disabled_rejected() {
    let moves = vec![make_move(MoveId::Tackle, 35, 35, true)];
    let mut menu = MoveMenuState::new(moves);
    let result = menu.update_frame(menu_a());
    assert_eq!(result, Some(MoveMenuResult::Disabled(0)));
}

#[test]
fn move_menu_disabled_takes_priority_over_no_pp() {
    let moves = vec![make_move(MoveId::Surf, 0, 15, true)];
    let mut menu = MoveMenuState::new(moves);
    let result = menu.update_frame(menu_a());
    assert_eq!(result, Some(MoveMenuResult::Disabled(0)));
}

#[test]
fn move_menu_b_cancels_when_allowed() {
    let mut menu = MoveMenuState::new(four_moves());
    menu.can_cancel = true;
    let result = menu.update_frame(menu_b());
    assert_eq!(result, Some(MoveMenuResult::Cancelled));
}

#[test]
fn move_menu_b_ignored_when_cannot_cancel() {
    let mut menu = MoveMenuState::new(four_moves());
    menu.can_cancel = false;
    let result = menu.update_frame(menu_b());
    assert!(result.is_none());
}

#[test]
fn move_menu_has_any_usable_move_true() {
    let menu = MoveMenuState::new(four_moves());
    assert!(menu.has_any_usable_move());
}

#[test]
fn move_menu_has_any_usable_move_false_all_no_pp() {
    let moves = vec![
        make_move(MoveId::Pound, 0, 35, false),
        make_move(MoveId::Tackle, 0, 35, false),
    ];
    let menu = MoveMenuState::new(moves);
    assert!(!menu.has_any_usable_move());
}

#[test]
fn move_menu_has_any_usable_move_false_all_disabled() {
    let moves = vec![make_move(MoveId::Pound, 35, 35, true)];
    let menu = MoveMenuState::new(moves);
    assert!(!menu.has_any_usable_move());
}

#[test]
fn move_menu_current_move_returns_cursor_slot() {
    let mut menu = MoveMenuState::new(four_moves());
    menu.update_frame(menu_down());
    menu.update_frame(menu_down()); // cursor=2
    let current = menu.current_move().unwrap();
    assert_eq!(current.move_id, MoveId::Thunderbolt);
}

#[test]
fn move_menu_single_move_wraps() {
    let moves = vec![make_move(MoveId::Pound, 35, 35, false)];
    let mut menu = MoveMenuState::new(moves);
    menu.update_frame(menu_down());
    assert_eq!(menu.cursor(), 0);
    menu.update_frame(menu_up());
    assert_eq!(menu.cursor(), 0);
}

// =============================================================
// PartySubMenuState tests
// =============================================================

use crate::battle::menu::PartySubMenuAction;
use crate::battle::menu::PartySubMenuState;

#[test]
fn party_submenu_initial_cursor_at_switch() {
    let menu = PartySubMenuState::new();
    assert_eq!(menu.cursor(), 0);
    assert_eq!(menu.current_action(), PartySubMenuAction::Switch);
}

#[test]
fn party_submenu_navigate_down_to_stats() {
    let mut menu = PartySubMenuState::new();
    assert!(menu.update_frame(input_down()).is_none());
    assert_eq!(menu.current_action(), PartySubMenuAction::Stats);
}

#[test]
fn party_submenu_navigate_down_to_cancel() {
    let mut menu = PartySubMenuState::new();
    menu.update_frame(input_down());
    assert!(menu.update_frame(input_down()).is_none());
    assert_eq!(menu.current_action(), PartySubMenuAction::Cancel);
}

#[test]
fn party_submenu_clamp_at_bottom() {
    let mut menu = PartySubMenuState::new();
    menu.update_frame(input_down());
    menu.update_frame(input_down());
    menu.update_frame(input_down()); // should stay at Cancel
    assert_eq!(menu.current_action(), PartySubMenuAction::Cancel);
}

#[test]
fn party_submenu_clamp_at_top() {
    let mut menu = PartySubMenuState::new();
    menu.update_frame(input_up()); // should stay at Switch
    assert_eq!(menu.current_action(), PartySubMenuAction::Switch);
}

#[test]
fn party_submenu_a_selects_current() {
    let mut menu = PartySubMenuState::new();
    menu.update_frame(input_down()); // cursor at Stats
    let result = menu.update_frame(input_a());
    assert_eq!(result, Some(PartySubMenuAction::Stats));
}

#[test]
fn party_submenu_b_returns_cancel() {
    let mut menu = PartySubMenuState::new();
    let result = menu.update_frame(input_b());
    assert_eq!(result, Some(PartySubMenuAction::Cancel));
}

// =============================================================
// BagMenuState tests
// =============================================================

use pokered_data::items::ItemId;

#[test]
fn bag_menu_empty_returns_cancel_on_a() {
    let mut menu = BagMenuState::new(vec![]);
    let result = menu.update_frame(input_a());
    assert_eq!(result, Some(BagMenuResult::Cancelled));
}

#[test]
fn bag_menu_empty_returns_cancel_on_b() {
    let mut menu = BagMenuState::new(vec![]);
    let result = menu.update_frame(input_b());
    assert_eq!(result, Some(BagMenuResult::Cancelled));
}

#[test]
fn bag_menu_single_item_selects() {
    let items = vec![(ItemId::Potion, 5)];
    let mut menu = BagMenuState::new(items);
    let result = menu.update_frame(input_a());
    assert_eq!(result, Some(BagMenuResult::Selected(ItemId::Potion)));
}

#[test]
fn bag_menu_b_cancels() {
    let items = vec![(ItemId::Potion, 5), (ItemId::Antidote, 3)];
    let mut menu = BagMenuState::new(items);
    let result = menu.update_frame(input_b());
    assert_eq!(result, Some(BagMenuResult::Cancelled));
}

#[test]
fn bag_menu_navigate_down() {
    let items = vec![
        (ItemId::Potion, 5),
        (ItemId::Antidote, 3),
        (ItemId::PokeBall, 10),
    ];
    let mut menu = BagMenuState::new(items);
    assert_eq!(menu.cursor(), 0);
    menu.update_frame(input_down());
    assert_eq!(menu.cursor(), 1);
    menu.update_frame(input_down());
    assert_eq!(menu.cursor(), 2);
}

#[test]
fn bag_menu_navigate_up() {
    let items = vec![(ItemId::Potion, 5), (ItemId::Antidote, 3)];
    let mut menu = BagMenuState::new(items);
    menu.update_frame(input_down()); // cursor at 1
    assert_eq!(menu.cursor(), 1);
    menu.update_frame(input_up());
    assert_eq!(menu.cursor(), 0);
}

#[test]
fn bag_menu_saved_cursor_persists() {
    let items = vec![
        (ItemId::Potion, 5),
        (ItemId::Antidote, 3),
        (ItemId::PokeBall, 10),
    ];
    let mut menu = BagMenuState::new(items.clone());
    menu.update_frame(input_down());
    menu.update_frame(input_down());
    menu.update_frame(input_a());

    let saved = menu.saved_cursor();
    assert_eq!(saved, 2);

    let menu2 = BagMenuState::with_saved_cursor(items, saved);
    assert_eq!(menu2.cursor(), 2);
}

#[test]
fn bag_menu_wrap_up() {
    let items = vec![(ItemId::Potion, 5), (ItemId::Antidote, 3)];
    let mut menu = BagMenuState::new(items);
    menu.update_frame(input_up()); // wraps to last item
    assert_eq!(menu.cursor(), 1);
}

#[test]
fn bag_menu_select_second_item() {
    let items = vec![(ItemId::Potion, 5), (ItemId::Antidote, 3)];
    let mut menu = BagMenuState::new(items);
    menu.update_frame(input_down());
    let result = menu.update_frame(input_a());
    assert_eq!(result, Some(BagMenuResult::Selected(ItemId::Antidote)));
}

#[test]
fn bag_menu_saved_cursor_clamps_to_valid() {
    let items = vec![(ItemId::Potion, 5)];
    let menu = BagMenuState::with_saved_cursor(items, 5); // saved=5 but only 1 item
    assert_eq!(menu.cursor(), 0);
}

#[test]
fn bag_menu_current_item() {
    let items = vec![(ItemId::Potion, 5), (ItemId::Antidote, 3)];
    let mut menu = BagMenuState::new(items.clone());
    assert_eq!(menu.current_item(), Some((ItemId::Potion, 5)));
    menu.update_frame(input_down());
    assert_eq!(menu.current_item(), Some((ItemId::Antidote, 3)));
}

#[test]
fn bag_menu_is_empty() {
    let empty = BagMenuState::new(vec![]);
    assert!(empty.is_empty());

    let not_empty = BagMenuState::new(vec![(ItemId::Potion, 1)]);
    assert!(!not_empty.is_empty());
}

// =============================================================
// ItemCategory tests
// =============================================================

#[test]
fn item_category_ball_classification() {
    assert_eq!(
        ItemCategory::from_item(ItemId::MasterBall),
        ItemCategory::Ball
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::UltraBall),
        ItemCategory::Ball
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::GreatBall),
        ItemCategory::Ball
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::PokeBall),
        ItemCategory::Ball
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::SafariBall),
        ItemCategory::Ball
    );
}

#[test]
fn item_category_healing_classification() {
    assert_eq!(
        ItemCategory::from_item(ItemId::Potion),
        ItemCategory::Healing
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::SuperPotion),
        ItemCategory::Healing
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::HyperPotion),
        ItemCategory::Healing
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::MaxPotion),
        ItemCategory::Healing
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::FullRestore),
        ItemCategory::Healing
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::FreshWater),
        ItemCategory::Healing
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::SodaPop),
        ItemCategory::Healing
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::Lemonade),
        ItemCategory::Healing
    );
}

#[test]
fn item_category_status_cure_classification() {
    assert_eq!(
        ItemCategory::from_item(ItemId::Antidote),
        ItemCategory::StatusCure
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::BurnHeal),
        ItemCategory::StatusCure
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::IceHeal),
        ItemCategory::StatusCure
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::Awakening),
        ItemCategory::StatusCure
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::ParlyzHeal),
        ItemCategory::StatusCure
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::FullHeal),
        ItemCategory::StatusCure
    );
}

#[test]
fn item_category_revive_classification() {
    assert_eq!(
        ItemCategory::from_item(ItemId::Revive),
        ItemCategory::Revive
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::MaxRevive),
        ItemCategory::Revive
    );
}

#[test]
fn item_category_battle_stat_classification() {
    assert_eq!(
        ItemCategory::from_item(ItemId::XAttack),
        ItemCategory::BattleStat
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::XDefend),
        ItemCategory::BattleStat
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::XSpeed),
        ItemCategory::BattleStat
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::XSpecial),
        ItemCategory::BattleStat
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::XAccuracy),
        ItemCategory::BattleStat
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::DireHit),
        ItemCategory::BattleStat
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::GuardSpec),
        ItemCategory::BattleStat
    );
}

#[test]
fn item_category_usable_in_battle_classification() {
    assert_eq!(
        ItemCategory::from_item(ItemId::PokeDoll),
        ItemCategory::UsableInBattle
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::PokeFlute),
        ItemCategory::UsableInBattle
    );
}

#[test]
fn item_category_not_usable_in_battle() {
    assert_eq!(
        ItemCategory::from_item(ItemId::Bicycle),
        ItemCategory::NotUsableInBattle
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::Repel),
        ItemCategory::NotUsableInBattle
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::MoonStone),
        ItemCategory::NotUsableInBattle
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::HpUp),
        ItemCategory::NotUsableInBattle
    );
    assert_eq!(
        ItemCategory::from_item(ItemId::TownMap),
        ItemCategory::NotUsableInBattle
    );
}

#[test]
fn item_category_is_usable_in_battle() {
    assert!(ItemCategory::Ball.is_usable_in_battle());
    assert!(ItemCategory::Healing.is_usable_in_battle());
    assert!(ItemCategory::StatusCure.is_usable_in_battle());
    assert!(ItemCategory::Revive.is_usable_in_battle());
    assert!(ItemCategory::BattleStat.is_usable_in_battle());
    assert!(ItemCategory::UsableInBattle.is_usable_in_battle());
    assert!(!ItemCategory::NotUsableInBattle.is_usable_in_battle());
}

#[test]
fn item_category_is_usable_in_trainer_battle() {
    assert!(!ItemCategory::Ball.is_usable_in_trainer_battle());
    assert!(ItemCategory::Healing.is_usable_in_trainer_battle());
    assert!(ItemCategory::StatusCure.is_usable_in_trainer_battle());
    assert!(ItemCategory::Revive.is_usable_in_trainer_battle());
    assert!(ItemCategory::BattleStat.is_usable_in_trainer_battle());
    assert!(ItemCategory::UsableInBattle.is_usable_in_trainer_battle());
    assert!(!ItemCategory::NotUsableInBattle.is_usable_in_trainer_battle());
}

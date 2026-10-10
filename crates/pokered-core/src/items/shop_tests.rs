use crate::alloc_prelude::*;
use super::inventory::{BAG_ITEM_CAPACITY, Inventory};
use super::shop::*;
use crate::main_menu::MenuInput;
use pokered_data::items::ItemId;

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
fn shop_menu_initial_state() {
    let menu = ShopMenuState::new();
    assert_eq!(menu.cursor(), 0);
    assert_eq!(menu.current_choice(), ShopMenuChoice::Buy);
}

#[test]
fn shop_menu_navigate_all_choices() {
    let expected = [
        ShopMenuChoice::Buy,
        ShopMenuChoice::Sell,
        ShopMenuChoice::Quit,
    ];
    for (i, choice) in expected.iter().enumerate() {
        let mut menu = ShopMenuState::new();
        for _ in 0..i {
            menu.update_frame(menu_down());
        }
        let result = menu.update_frame(menu_a());
        assert_eq!(result, Some(*choice), "index={i}");
    }
}

#[test]
fn shop_menu_b_quits() {
    let mut menu = ShopMenuState::new();
    let result = menu.update_frame(menu_b());
    assert_eq!(result, Some(ShopMenuChoice::Quit));
}

#[test]
fn shop_menu_cursor_wraps() {
    let mut menu = ShopMenuState::new();
    menu.update_frame(menu_up());
    assert_eq!(menu.cursor(), 2);
    menu.update_frame(menu_down());
    assert_eq!(menu.cursor(), 0);
}

#[test]
fn shop_menu_default() {
    let menu = ShopMenuState::default();
    assert_eq!(menu.cursor(), 0);
}

#[test]
fn buy_price_for_potion() {
    let price = buy_price(ItemId::Potion, 1).unwrap();
    assert_eq!(price, 300);
}

#[test]
fn buy_price_quantity_multiplied() {
    let price = buy_price(ItemId::Potion, 5).unwrap();
    assert_eq!(price, 1500);
}

#[test]
fn sell_price_is_half_buy() {
    let buy = buy_price(ItemId::Potion, 1).unwrap();
    let sell = sell_price(ItemId::Potion, 1).unwrap();
    assert_eq!(sell, buy / 2);
}

#[test]
fn can_sell_regular_item() {
    assert!(can_sell(ItemId::Potion));
    assert!(can_sell(ItemId::UltraBall));
}

#[test]
fn cannot_sell_key_item() {
    assert!(!can_sell(ItemId::OldAmber));
}

#[test]
fn try_buy_success() {
    let mut money: u32 = 1000;
    let mut bag = Inventory::new_bag();
    let result = try_buy(ItemId::Potion, 2, &mut money, &mut bag);
    assert_eq!(result, BuyResult::Success { total_cost: 600 });
    assert_eq!(money, 400);
    assert_eq!(bag.item_quantity(ItemId::Potion), 2);
}

#[test]
fn try_buy_not_enough_money() {
    let mut money: u32 = 100;
    let mut bag = Inventory::new_bag();
    let result = try_buy(ItemId::Potion, 1, &mut money, &mut bag);
    assert_eq!(result, BuyResult::NotEnoughMoney);
    assert_eq!(money, 100);
}

#[test]
fn try_buy_bag_full() {
    let mut money: u32 = 999999;
    let mut bag = Inventory::new_bag();
    let filler_items = [
        ItemId::MasterBall,
        ItemId::UltraBall,
        ItemId::GreatBall,
        ItemId::PokeBall,
        ItemId::TownMap,
        ItemId::Bicycle,
        ItemId::Antidote,
        ItemId::BurnHeal,
        ItemId::IceHeal,
        ItemId::Awakening,
        ItemId::ParlyzHeal,
        ItemId::FullRestore,
        ItemId::MaxPotion,
        ItemId::HyperPotion,
        ItemId::SuperPotion,
        ItemId::Potion,
        ItemId::EscapeRope,
        ItemId::Repel,
        ItemId::FireStone,
        ItemId::ThunderStone,
    ];
    for item in &filler_items {
        bag.add_item(*item, 1).unwrap();
    }
    let result = try_buy(ItemId::MoonStone, 1, &mut money, &mut bag);
    assert_eq!(result, BuyResult::BagFull);
    assert_eq!(money, 999999);
}

#[test]
fn try_sell_success() {
    let mut money: u32 = 0;
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 5).unwrap();
    let result = try_sell(0, 3, &mut money, &mut bag);
    assert_eq!(result, SellResult::Success { total_value: 450 });
    assert_eq!(money, 450);
    assert_eq!(bag.item_quantity(ItemId::Potion), 2);
}

#[test]
fn try_sell_unsellable_key_item() {
    let mut money: u32 = 0;
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::OldAmber, 1).unwrap();
    let result = try_sell(0, 1, &mut money, &mut bag);
    assert_eq!(result, SellResult::Unsellable);
    assert_eq!(money, 0);
    assert_eq!(bag.item_quantity(ItemId::OldAmber), 1);
}

#[test]
fn try_sell_not_in_bag() {
    let mut money: u32 = 0;
    let mut bag = Inventory::new_bag();
    let result = try_sell(0, 1, &mut money, &mut bag);
    assert_eq!(result, SellResult::NotInBag);
}

#[test]
fn try_sell_quantity_exceeds_owned() {
    let mut money: u32 = 0;
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 2).unwrap();
    let result = try_sell(0, 5, &mut money, &mut bag);
    assert_eq!(result, SellResult::NotInBag);
    assert_eq!(bag.item_quantity(ItemId::Potion), 2);
}

#[test]
fn shop_inventory_basic() {
    let shop = ShopInventory::new(vec![ItemId::PokeBall, ItemId::Potion, ItemId::Antidote]);
    assert_eq!(shop.len(), 3);
    assert!(!shop.is_empty());
    assert_eq!(shop.get(0), Some(ItemId::PokeBall));
    assert_eq!(shop.get(1), Some(ItemId::Potion));
    assert_eq!(shop.get(3), None);
}

#[test]
fn shop_inventory_empty() {
    let shop = ShopInventory::new(vec![]);
    assert!(shop.is_empty());
    assert_eq!(shop.len(), 0);
}

#[test]
fn from_strings_success() {
    let items: Vec<String> = vec![
        "PokeBall".to_string(),
        "Potion".to_string(),
        "Antidote".to_string(),
    ];
    let shop = ShopInventory::from_strings(&items).unwrap();
    assert_eq!(shop.len(), 3);
    assert_eq!(shop.get(0), Some(ItemId::PokeBall));
    assert_eq!(shop.get(1), Some(ItemId::Potion));
    assert_eq!(shop.get(2), Some(ItemId::Antidote));
}

#[test]
fn from_strings_empty() {
    let items: Vec<String> = vec![];
    let shop = ShopInventory::from_strings(&items).unwrap();
    assert!(shop.is_empty());
}

#[test]
fn from_strings_unknown_item() {
    let items: Vec<String> = vec!["PokeBall".to_string(), "NotAnItem".to_string()];
    let err = ShopInventory::from_strings(&items).unwrap_err();
    assert_eq!(err, "NotAnItem");
}

// ─── MartState tests ──────────────────────────

fn player_data(money: u32, bag: Inventory<BAG_ITEM_CAPACITY>) -> PlayerData {
    PlayerData { money, bag }
}

#[test]
fn mart_buy_happy_path() {
    let shop = ShopInventory::new(vec![ItemId::PokeBall, ItemId::Potion, ItemId::Antidote]);
    let mut mart = MartState::new(shop);
    let mut p = player_data(1000, Inventory::new_bag());

    // MainMenu: cursor starts at Buy.
    assert!(matches!(
        mart.phase,
        MartPhase::MainMenu {
            cursor: MartTopChoice::Buy
        }
    ));

    // A → enter Buy.
    assert_eq!(mart.update_frame(menu_a(), &mut p), MartUpdate::Continue);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })
    ));

    // Down → Potion (index 1). A → Quantity.
    mart.update_frame(menu_down(), &mut p);
    assert_eq!(mart.update_frame(menu_a(), &mut p), MartUpdate::Continue);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::Quantity {
            item_index: 1,
            quantity: 1,
        })
    ));

    // Up ×2 → quantity=3. A → Confirm.
    mart.update_frame(menu_up(), &mut p);
    mart.update_frame(menu_up(), &mut p);
    assert_eq!(mart.update_frame(menu_a(), &mut p), MartUpdate::Continue);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::Confirm {
            item_index: 1,
            quantity: 3,
            selected: ConfirmChoice::Yes,
        })
    ));

    // A on Yes → commits purchase.
    assert_eq!(
        mart.update_frame(menu_a(), &mut p),
        MartUpdate::PlaySound(SoundId::Purchase)
    );
    assert_eq!(p.money, 100); // 1000 - 900
    assert_eq!(p.bag.item_quantity(ItemId::Potion), 3);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::Result {
            dialogue: BuyResult::Success { total_cost: 900 },
            return_to_list: true,
        })
    ));

    // Next frame → auto-dismiss Result, back to SelectItem.
    assert_eq!(
        mart.update_frame(MenuInput::none(), &mut p),
        MartUpdate::Continue
    );
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })
    ));
}

#[test]
fn mart_buy_not_enough_money() {
    let shop = ShopInventory::new(vec![ItemId::Potion]);
    let mut mart = MartState::new(shop);
    let mut p = player_data(100, Inventory::new_bag());

    // Enter Buy → SelectItem 0 → Quantity 1 → Confirm Yes → A.
    mart.update_frame(menu_a(), &mut p); // into SelectItem
    mart.update_frame(menu_a(), &mut p); // into Quantity
    mart.update_frame(menu_a(), &mut p); // into Confirm
    assert_eq!(mart.update_frame(menu_a(), &mut p), MartUpdate::Continue);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::Result {
            dialogue: BuyResult::NotEnoughMoney,
            return_to_list: false,
        })
    ));
    assert_eq!(p.money, 100); // unchanged

    // Auto-dismiss → back to MainMenu.
    assert_eq!(
        mart.update_frame(MenuInput::none(), &mut p),
        MartUpdate::Continue
    );
    assert!(matches!(mart.phase, MartPhase::MainMenu { .. }));
}

#[test]
fn mart_buy_bag_full() {
    let shop = ShopInventory::new(vec![ItemId::Potion]);
    let mut mart = MartState::new(shop);
    let mut bag = Inventory::new_bag();
    // Fill bag with 20 unique items.
    let fill: [ItemId; 20] = [
        ItemId::MasterBall,
        ItemId::UltraBall,
        ItemId::GreatBall,
        ItemId::PokeBall,
        ItemId::TownMap,
        ItemId::Bicycle,
        ItemId::Antidote,
        ItemId::BurnHeal,
        ItemId::IceHeal,
        ItemId::Awakening,
        ItemId::ParlyzHeal,
        ItemId::FullRestore,
        ItemId::MaxPotion,
        ItemId::HyperPotion,
        ItemId::SuperPotion,
        ItemId::EscapeRope,
        ItemId::Repel,
        ItemId::FireStone,
        ItemId::ThunderStone,
        ItemId::WaterStone,
    ];
    for it in &fill {
        bag.add_item(*it, 1).unwrap();
    }
    let mut p = player_data(999999, bag);

    // Enter Buy → SelectItem → Quantity → Confirm → A.
    mart.update_frame(menu_a(), &mut p); // SelectItem
    mart.update_frame(menu_a(), &mut p); // Quantity
    mart.update_frame(menu_a(), &mut p); // Confirm
    assert_eq!(mart.update_frame(menu_a(), &mut p), MartUpdate::Continue);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::Result {
            dialogue: BuyResult::BagFull,
            return_to_list: false,
        })
    ));
    assert_eq!(p.money, 999999);

    // Auto-dismiss → MainMenu.
    assert_eq!(
        mart.update_frame(MenuInput::none(), &mut p),
        MartUpdate::Continue
    );
    assert!(matches!(mart.phase, MartPhase::MainMenu { .. }));
}

#[test]
fn mart_sell_happy_path() {
    let shop = ShopInventory::new(vec![ItemId::PokeBall]);
    let mut mart = MartState::new(shop);
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 5).unwrap();
    let mut p = player_data(0, bag);

    // MainMenu → down to Sell → A.
    mart.update_frame(menu_down(), &mut p);
    assert_eq!(mart.update_frame(menu_a(), &mut p), MartUpdate::Continue);
    assert!(matches!(
        mart.phase,
        MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 })
    ));

    // A on Potion → Quantity.
    assert_eq!(mart.update_frame(menu_a(), &mut p), MartUpdate::Continue);
    assert!(matches!(
        mart.phase,
        MartPhase::Sell(SellMenuState::Quantity {
            item_index: 0,
            quantity: 1,
            max_quantity: 5,
        })
    ));

    // Up → quantity=2. A → Confirm.
    mart.update_frame(menu_up(), &mut p);
    assert_eq!(mart.update_frame(menu_a(), &mut p), MartUpdate::Continue);
    assert!(matches!(
        mart.phase,
        MartPhase::Sell(SellMenuState::Confirm {
            item_index: 0,
            quantity: 2,
            max_quantity: 5,
            selected: ConfirmChoice::Yes,
        })
    ));

    // A → commit sell.
    assert_eq!(mart.update_frame(menu_a(), &mut p), MartUpdate::Continue);
    assert_eq!(p.money, 300); // Potion buy price 300, sell half = 150 × 2
    assert_eq!(p.bag.item_quantity(ItemId::Potion), 3);
    assert!(matches!(
        mart.phase,
        MartPhase::Sell(SellMenuState::Result {
            dialogue: SellResult::Success { total_value: 300 },
            return_to_list: true,
        })
    ));

    // Auto-dismiss → back to SelectItem.
    assert_eq!(
        mart.update_frame(MenuInput::none(), &mut p),
        MartUpdate::Continue
    );
    assert!(matches!(
        mart.phase,
        MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 })
    ));
}

#[test]
fn mart_sell_only_shows_owned_items() {
    let shop = ShopInventory::new(vec![ItemId::PokeBall]);
    let mut mart = MartState::new(shop);
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 3).unwrap();
    let mut p = player_data(0, bag);

    // Enter Sell.
    mart.update_frame(menu_down(), &mut p);
    mart.update_frame(menu_a(), &mut p);
    assert!(matches!(
        mart.phase,
        MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 })
    ));

    // Bag has exactly 1 item (Potion), so count=1.
    assert_eq!(p.bag.count(), 1);
    assert_eq!(p.bag.get(0).unwrap(), (ItemId::Potion, 3));
}

#[test]
fn mart_buy_b_backout_from_quantity() {
    let shop = ShopInventory::new(vec![ItemId::Potion]);
    let mut mart = MartState::new(shop);
    let mut p = player_data(1000, Inventory::new_bag());

    // Enter Quantity phase.
    mart.update_frame(menu_a(), &mut p); // into SelectItem
    mart.update_frame(menu_a(), &mut p); // into Quantity
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::Quantity {
            item_index: 0,
            quantity: 1,
        })
    ));

    // B → back to SelectItem.
    assert_eq!(mart.update_frame(menu_b(), &mut p), MartUpdate::Continue);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })
    ));
}

#[test]
fn mart_buy_b_backout_from_confirm() {
    let shop = ShopInventory::new(vec![ItemId::Potion]);
    let mut mart = MartState::new(shop);
    let mut p = player_data(1000, Inventory::new_bag());

    // Enter Confirm phase.
    mart.update_frame(menu_a(), &mut p); // SelectItem
    mart.update_frame(menu_a(), &mut p); // Quantity
    mart.update_frame(menu_a(), &mut p); // Confirm
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::Confirm {
            item_index: 0,
            quantity: 1,
            selected: ConfirmChoice::Yes,
        })
    ));

    // B → back to Quantity.
    assert_eq!(mart.update_frame(menu_b(), &mut p), MartUpdate::Continue);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::Quantity {
            item_index: 0,
            quantity: 1,
        })
    ));
}

#[test]
fn mart_buy_quantity_wrap() {
    let shop = ShopInventory::new(vec![ItemId::Potion]);
    let mut mart = MartState::new(shop);
    let mut p = player_data(1000, Inventory::new_bag());

    // Enter Quantity at quantity=1.
    mart.update_frame(menu_a(), &mut p); // SelectItem
    mart.update_frame(menu_a(), &mut p); // Quantity { quantity: 1 }

    // Down at 1 → wraps to 99.
    mart.update_frame(menu_down(), &mut p);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::Quantity {
            item_index: 0,
            quantity: 99,
        })
    ));

    // Up at 99 → wraps to 1.
    mart.update_frame(menu_up(), &mut p);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::Quantity {
            item_index: 0,
            quantity: 1,
        })
    ));

    // Up ×2 → 3.
    mart.update_frame(menu_up(), &mut p);
    mart.update_frame(menu_up(), &mut p);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::Quantity {
            item_index: 0,
            quantity: 3,
        })
    ));
}

#[test]
fn mart_top_menu_quit_returns_exit() {
    let shop = ShopInventory::new(vec![ItemId::Potion]);
    let mut mart = MartState::new(shop);
    let mut p = player_data(1000, Inventory::new_bag());

    // Navigate to Quit (Down×2).
    mart.update_frame(menu_down(), &mut p);
    mart.update_frame(menu_down(), &mut p);
    assert!(matches!(
        mart.phase,
        MartPhase::MainMenu {
            cursor: MartTopChoice::Quit,
        }
    ));

    // A on Quit → Exit.
    assert_eq!(mart.update_frame(menu_a(), &mut p), MartUpdate::Exit);
    assert!(matches!(mart.phase, MartPhase::Exiting));
}

#[test]
fn mart_confirm_no_returns_to_select_item() {
    let shop = ShopInventory::new(vec![ItemId::Potion]);
    let mut mart = MartState::new(shop);
    let mut p = player_data(1000, Inventory::new_bag());

    // Enter Confirm with Yes selected, then toggle to No.
    mart.update_frame(menu_a(), &mut p); // SelectItem
    mart.update_frame(menu_a(), &mut p); // Quantity
    mart.update_frame(menu_a(), &mut p); // Confirm { selected: Yes }

    // Toggle to No.
    mart.update_frame(menu_up(), &mut p);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::Confirm {
            selected: ConfirmChoice::No,
            ..
        })
    ));

    // A on No → back to SelectItem, money unchanged.
    let old_money = p.money;
    assert_eq!(mart.update_frame(menu_a(), &mut p), MartUpdate::Continue);
    assert_eq!(p.money, old_money);
    assert!(matches!(
        mart.phase,
        MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })
    ));
}

#[test]
fn mart_b_at_main_menu_exits() {
    let shop = ShopInventory::new(vec![ItemId::Potion]);
    let mut mart = MartState::new(shop);
    let mut p = player_data(1000, Inventory::new_bag());

    assert_eq!(mart.update_frame(menu_b(), &mut p), MartUpdate::Exit);
    assert!(matches!(mart.phase, MartPhase::Exiting));
}

#[test]
fn mart_main_menu_navigation_wraps() {
    let shop = ShopInventory::new(vec![ItemId::Potion]);
    let mut mart = MartState::new(shop);
    let mut p = player_data(1000, Inventory::new_bag());

    // Up from Buy → Quit.
    mart.update_frame(menu_up(), &mut p);
    assert!(matches!(
        mart.phase,
        MartPhase::MainMenu {
            cursor: MartTopChoice::Quit,
        }
    ));

    // Down from Quit → Buy.
    mart.update_frame(menu_down(), &mut p);
    assert!(matches!(
        mart.phase,
        MartPhase::MainMenu {
            cursor: MartTopChoice::Buy,
        }
    ));
}

#[test]
fn mart_sell_quantity_wrap() {
    let shop = ShopInventory::new(vec![ItemId::PokeBall]);
    let mut mart = MartState::new(shop);
    let mut bag = Inventory::new_bag();
    bag.add_item(ItemId::Potion, 3).unwrap();
    let mut p = player_data(0, bag);

    // Enter Sell → SelectItem → Quantity.
    mart.update_frame(menu_down(), &mut p); // cursor → Sell
    mart.update_frame(menu_a(), &mut p); // into SelectItem
    mart.update_frame(menu_a(), &mut p); // into Quantity { quantity: 1, max: 3 }

    // Down at 1 → wraps to 3.
    mart.update_frame(menu_down(), &mut p);
    assert!(matches!(
        mart.phase,
        MartPhase::Sell(SellMenuState::Quantity {
            quantity: 3,
            max_quantity: 3,
            ..
        })
    ));

    // Up at 3 → wraps to 1.
    mart.update_frame(menu_up(), &mut p);
    assert!(matches!(
        mart.phase,
        MartPhase::Sell(SellMenuState::Quantity {
            quantity: 1,
            max_quantity: 3,
            ..
        })
    ));
}


#[test]
fn source_quantity_mart_confirm_precedes_cancel_and_directions() {
    for sell in [false, true] {
        for b in [false, true] {
            let mut mart = MartState::new(ShopInventory::new(vec![ItemId::Potion]));
            let mut bag = Inventory::new_bag();
            bag.add_item(ItemId::Potion, 4).unwrap();
            let mut player = player_data(1000, bag);
            mart.phase = if sell {
                MartPhase::Sell(SellMenuState::Quantity { item_index: 0, quantity: 1, max_quantity: 4 })
            } else {
                MartPhase::Buy(BuyMenuState::Quantity { item_index: 0, quantity: 1 })
            };
            mart.update_frame(MenuInput { a: true, b, up: true, down: true }, &mut player);
            if sell {
                assert!(matches!(mart.phase, MartPhase::Sell(SellMenuState::Confirm { quantity: 1, .. })));
            } else {
                assert!(matches!(mart.phase, MartPhase::Buy(BuyMenuState::Confirm { quantity: 1, .. })));
            }
            assert_eq!(player.money, 1000);
            assert_eq!(player.bag.item_quantity(ItemId::Potion), 4);
        }
    }
}

fn source_mart_ready(mart: &mut MartState, player: &mut PlayerData, prompt: bool) {
    for _ in 0..2000 {
        if if prompt { mart.field_message_waiting() } else { !mart.field_message_active() } { return; }
        assert_eq!(mart.update_frame(MenuInput::none(), player), MartUpdate::Continue);
    }
    panic!("mart text did not return: {:?}", mart.field_message_lines());
}

#[test]
fn source_mart_done_greeting_prints_before_item_input() {
    for delay in [1, 3, 5] {
        let mut mart = MartState::new(ShopInventory::new(vec![ItemId::Potion]));
        mart.configure_field_text(delay);
        let mut player = player_data(1000, Inventory::new_bag());
        mart.update_frame(menu_a(), &mut player);
        assert!(mart.field_message_active());
        assert!(matches!(mart.display_phase(), MartPhase::MainMenu { .. }));
        for frame in 1..=3 + "Take your time.".len() * delay as usize {
            mart.update_frame(MenuInput::none(), &mut player);
            assert_eq!(mart.field_message_active(), frame < 3 + "Take your time.".len() * delay as usize);
        }
        assert_eq!(mart.field_message_lines(), vec!["Take your time."]);
        assert!(matches!(mart.phase, MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })));
        assert_eq!(player.money, 1000); assert_eq!(player.bag.count(), 0);
    }
}

#[test]
fn source_mart_empty_sell_prompt_waits_then_returns_through_anything_else() {
    let mut mart = MartState::new(ShopInventory::new(vec![ItemId::Potion]));
    mart.configure_field_text(5);
    let mut player = player_data(1000, Inventory::new_bag());
    mart.update_frame(menu_down(), &mut player); mart.update_frame(menu_a(), &mut player);
    source_mart_ready(&mut mart, &mut player, true);
    assert_eq!(mart.field_message_lines(), vec!["You don't have", "anything to sell."]);
    for _ in 0..60 { mart.update_frame(MenuInput::none(), &mut player); }
    assert!(mart.field_message_waiting());
    mart.update_frame(menu_b(), &mut player);
    assert!(mart.take_text_advance());
    source_mart_ready(&mut mart, &mut player, false);
    assert_eq!(mart.field_message_lines(), vec!["Is there anything", "else I can do?"]);
    assert!(matches!(mart.phase, MartPhase::MainMenu { cursor: MartTopChoice::Buy }));
    assert_eq!(player.money, 1000); assert_eq!(player.bag.count(), 0);
}

#[test]
fn source_mart_cancel_row_is_selectable_in_buy_sell_and_empty_return_list() {
    for sell in [false, true] { for empty in [false, true] {
        let mut mart = MartState::new(ShopInventory::new(if empty { vec![] } else { vec![ItemId::Potion] }));
        mart.configure_field_text(1);
        let mut bag = Inventory::new_bag(); if !empty { bag.add_item(ItemId::Potion, 4).unwrap(); }
        let mut player = player_data(1000, bag);
        mart.phase = if sell { MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 }) }
            else { MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 }) };
        mart.update_frame(menu_up(), &mut player);
        let expected = usize::from(!empty);
        assert!(match mart.phase { MartPhase::Buy(BuyMenuState::SelectItem { cursor }) | MartPhase::Sell(SellMenuState::SelectItem { cursor }) => cursor == expected, _ => false });
        mart.update_frame(menu_a(), &mut player); source_mart_ready(&mut mart, &mut player, false);
        assert!(matches!(mart.phase, MartPhase::MainMenu { cursor: MartTopChoice::Buy }));
        assert_eq!(player.money, 1000); assert_eq!(player.bag.item_quantity(ItemId::Potion), if empty { 0 } else { 4 });
    } }
}

#[test]
fn source_mart_no_and_b_confirmation_restore_list_without_transaction() {
    for sell in [false, true] { for cancel in [false, true] {
        let mut mart = MartState::new(ShopInventory::new(vec![ItemId::Antidote, ItemId::Potion]));
        mart.configure_field_text(1);
        let mut bag = Inventory::new_bag(); bag.add_item(ItemId::Antidote, 2).unwrap(); bag.add_item(ItemId::Potion, 4).unwrap();
        let mut player = player_data(1000, bag);
        mart.phase = if sell { MartPhase::Sell(SellMenuState::Confirm { item_index: 1, quantity: 2, max_quantity: 4, selected: ConfirmChoice::No }) }
            else { MartPhase::Buy(BuyMenuState::Confirm { item_index: 1, quantity: 2, selected: ConfirmChoice::No }) };
        mart.update_frame(if cancel { menu_b() } else { menu_a() }, &mut player);
        assert!(if sell { matches!(mart.phase, MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 })) }
            else { matches!(mart.phase, MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })) });
        assert_eq!(player.money, 1000); assert_eq!(player.bag.item_quantity(ItemId::Potion), 4);
    } }
}

#[test]
fn source_mart_rejects_unsellable_before_quantity() {
    for item in [ItemId::Bicycle, ItemId::Hm01] {
        let mut mart = MartState::new(ShopInventory::new(vec![ItemId::Potion])); mart.configure_field_text(1);
        let mut bag = Inventory::new_bag(); bag.add_item(item, 1).unwrap(); let mut player = player_data(1000, bag);
        mart.phase = MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 }); mart.update_frame(menu_a(), &mut player);
        assert!(mart.field_message_active()); assert!(!matches!(mart.phase, MartPhase::Sell(SellMenuState::Quantity { .. })));
        source_mart_ready(&mut mart, &mut player, true);
        assert_eq!(mart.field_message_lines(), vec!["I can't put a", "price on that."]);
        assert_eq!(player.money, 1000); assert_eq!(player.bag.item_quantity(item), 1);
    }
}

#[test]
fn source_mart_purchase_sound_then_receipt_prompt_does_not_autodismiss_or_repeat_purchase() {
    let mut mart = MartState::new(ShopInventory::new(vec![ItemId::Potion])); mart.configure_field_text(1);
    let mut player = player_data(1000, Inventory::new_bag());
    mart.phase = MartPhase::Buy(BuyMenuState::Confirm { item_index: 0, quantity: 2, selected: ConfirmChoice::Yes });
    assert_eq!(mart.update_frame(menu_a(), &mut player), MartUpdate::PlaySound(SoundId::Purchase));
    for _ in 0..30 { mart.update_frame_with_text_input(MenuInput::none(), &mut player, false, false, true); }
    assert_eq!(mart.field_message_lines(), vec!["", ""]);
    source_mart_ready(&mut mart, &mut player, true);
    assert_eq!(mart.field_message_lines(), vec!["Here you are!", "Thank you!"]);
    for _ in 0..60 { mart.update_frame(MenuInput::none(), &mut player); }
    assert_eq!(player.money, 400); assert_eq!(player.bag.item_quantity(ItemId::Potion), 2);
    assert!(mart.field_message_waiting()); mart.update_frame(menu_a(), &mut player);
    assert!(matches!(mart.phase, MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })));
    assert_eq!(mart.field_message_lines(), vec!["Take your time."]);
    assert_eq!(player.money, 400); assert_eq!(player.bag.item_quantity(ItemId::Potion), 2);
}

#[test]
fn source_mart_sell_success_returns_without_receipt_and_plays_cash_register() {
    let mut mart = MartState::new(ShopInventory::new(vec![ItemId::Potion])); mart.configure_field_text(1);
    let mut bag = Inventory::new_bag(); bag.add_item(ItemId::Potion, 4).unwrap(); let mut player = player_data(1000, bag);
    mart.phase = MartPhase::Sell(SellMenuState::Confirm { item_index: 0, quantity: 4, max_quantity: 4, selected: ConfirmChoice::Yes });
    assert_eq!(mart.update_frame(menu_a(), &mut player), MartUpdate::PlaySound(SoundId::Purchase));
    for _ in 0..20 { assert_eq!(mart.update_frame(MenuInput::none(), &mut player), MartUpdate::Continue); }
    assert!(!mart.field_message_active()); assert!(matches!(mart.phase, MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 })));
    assert_eq!(mart.field_message_lines(), vec!["What would you", "like to sell?"]);
    assert_eq!(player.money, 1600); assert_eq!(player.bag.count(), 0);
}

#[test]
fn source_mart_buy_price_cont_requires_scroll_before_confirmation() {
    let mut mart = MartState::new(ShopInventory::new(vec![ItemId::Potion])); mart.configure_field_text(1);
    let mut player = player_data(1000, Inventory::new_bag());
    mart.phase = MartPhase::Buy(BuyMenuState::Quantity { item_index: 0, quantity: 1 }); mart.update_frame(menu_a(), &mut player);
    source_mart_ready(&mut mart, &mut player, true);
    assert_eq!(mart.field_message_lines(), vec!["POTION?", "That will be"]);
    assert!(matches!(mart.display_phase(), MartPhase::Buy(BuyMenuState::Quantity { .. })));
    for _ in 0..20 { mart.update_frame(MenuInput::none(), &mut player); }
    assert_eq!(player.money, 1000); assert_eq!(player.bag.count(), 0);
    mart.update_frame(menu_a(), &mut player); assert!(mart.take_text_advance());
    source_mart_ready(&mut mart, &mut player, false);
    assert_eq!(mart.field_message_lines(), vec!["That will be", "¥300. OK?"]);
    assert!(matches!(mart.display_phase(), MartPhase::Buy(BuyMenuState::Confirm { .. })));
    assert_eq!(player.money, 1000); assert_eq!(player.bag.count(), 0);
}

#[test]
fn source_mart_exit_thank_you_and_outer_ack_wait_for_a_release() {
    let mut mart = MartState::new(ShopInventory::new(vec![ItemId::Potion])); mart.configure_field_text(1);
    let mut player = player_data(1000, Inventory::new_bag());
    assert_eq!(mart.update_frame(menu_b(), &mut player), MartUpdate::Continue);
    source_mart_ready(&mut mart, &mut player, true);
    assert_eq!(mart.field_message_lines(), vec!["Thank you!"]);
    assert_eq!(mart.update_frame_with_text_input(menu_a(), &mut player, true, false, false), MartUpdate::Continue);
    for _ in 0..20 { assert_eq!(mart.update_frame_with_text_input(MenuInput::none(), &mut player, true, false, false), MartUpdate::Continue); }
    assert!(!mart.take_text_advance(), "outer DONE acknowledgement has no ManualTextScroll sound");
    assert_eq!(mart.update_frame_with_text_input(MenuInput::none(), &mut player, false, false, false), MartUpdate::Exit);
    assert_eq!(player.money, 1000); assert_eq!(player.bag.count(), 0);
}

#[test]
fn source_mart_buy_error_prompts_do_not_dismiss_before_ack_and_anything_else() {
    for full in [false, true] {
        let mut mart = MartState::new(ShopInventory::new(vec![ItemId::Potion])); mart.configure_field_text(1);
        let mut bag = Inventory::new_bag();
        if full {
            for item in [ItemId::MasterBall,ItemId::UltraBall,ItemId::GreatBall,ItemId::PokeBall,ItemId::TownMap,
                ItemId::Bicycle,ItemId::Antidote,ItemId::BurnHeal,ItemId::IceHeal,ItemId::Awakening,ItemId::ParlyzHeal,
                ItemId::FullRestore,ItemId::MaxPotion,ItemId::HyperPotion,ItemId::SuperPotion,ItemId::EscapeRope,
                ItemId::Repel,ItemId::FireStone,ItemId::ThunderStone,ItemId::WaterStone] { bag.add_item(item,1).unwrap(); }
        }
        let mut player = player_data(if full {1000} else {0}, bag);
        let before = player.bag.items().to_vec(); let money = player.money;
        mart.phase = MartPhase::Buy(BuyMenuState::Confirm {item_index:0,quantity:1,selected:ConfirmChoice::Yes});
        assert_eq!(mart.update_frame(menu_a(), &mut player), MartUpdate::Continue);
        source_mart_ready(&mut mart,&mut player,true);
        assert_eq!(mart.field_message_lines(), if full {vec!["You can't carry","any more items."]} else {vec!["You don't have","enough money."]});
        for _ in 0..40 {mart.update_frame(MenuInput::none(),&mut player);}
        assert!(mart.field_message_waiting());
        assert_eq!(player.money,money);assert_eq!(player.bag.items(),before);
        mart.update_frame(menu_b(),&mut player);source_mart_ready(&mut mart,&mut player,false);
        assert!(matches!(mart.phase,MartPhase::MainMenu {cursor:MartTopChoice::Buy}));
        assert_eq!(player.money,money);assert_eq!(player.bag.items(),before);
    }
}

//! Temporary visual-tour harness for the PC screens: drives a `PcScreen`
//! through its phases and dumps a PNG of each. Run with:
//!   cargo run --release -p pokered-app --example pc_tour

use pokered_app::render::draw_pc;
use pokered_core::game_state::Lang;
use pokered_core::main_menu::MenuInput;
use pokered_core::pc_screen::{PcContext, PcEntry, PcOpenContext, PcPhase, PcScreen};
use pokered_core::save::SaveData;
use pokered_data::items::ItemId;
use pokered_data::species::Species;
use pokered_core::pokemon::stats::create_pokemon;
use pokered_renderer::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;

const A: MenuInput = MenuInput { up: false, down: false, a: true, b: false };
const B: MenuInput = MenuInput { up: false, down: false, a: false, b: true };
const UP: MenuInput = MenuInput { up: true, down: false, a: false, b: false };
const DOWN: MenuInput = MenuInput { up: false, down: true, a: false, b: false };

fn shot(pc: &PcScreen, save: &SaveData, name: &str) {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_pc(pc, save, &mut None, &mut fb, Lang::En);
    let path = format!("/tmp/pc_tour_{}.png", name);
    fb.save_png(std::path::Path::new(&path)).unwrap();
    println!("saved {} (phase {:?})", path, pc.phase());
}

fn step(pc: &mut PcScreen, save: &mut SaveData, input: MenuInput) {
    let mut ctx = PcContext {
        party: &mut save.party,
        pc_storage: &mut save.pc_storage,
        bag: &mut save.game_data.bag,
        pc_items: &mut save.game_data.pc_items,
        pokedex: &save.game_data.pokedex,
    };
    pc.update_frame(input, &mut ctx);
}

fn skip_msg(pc: &mut PcScreen, save: &mut SaveData) {
    while pc.phase() == PcPhase::Message {
        step(pc, save, A);
    }
}

fn press(pc: &mut PcScreen, save: &mut SaveData, input: MenuInput, n: usize) {
    for _ in 0..n {
        step(pc, save, input);
    }
}

fn open_ctx() -> PcOpenContext {
    PcOpenContext {
        has_pokedex: true,
        met_bill: true,
        beaten_league: false,
        player_name: "RED".into(),
        hof_teams: Vec::new(),
    }
}

fn fresh_at_main_menu(save: &SaveData) -> (PcScreen, SaveData) {
    let mut save = save.clone();
    let mut pc = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
    skip_msg(&mut pc, &mut save);
    (pc, save)
}

fn fresh_at_bills_menu(save: &SaveData) -> (PcScreen, SaveData) {
    let (mut pc, mut save) = fresh_at_main_menu(save);
    step(&mut pc, &mut save, A); // BILL's PC (cursor 0)
    skip_msg(&mut pc, &mut save);
    (pc, save)
}

fn main() {
    let mut save = SaveData::new();
    save.party.add(create_pokemon(Species::Pikachu, 12, [0x9A, 0x78]).unwrap()).unwrap();
    save.party.add(create_pokemon(Species::Bulbasaur, 7, [0x9A, 0x78]).unwrap()).unwrap();
    save.pc_storage.current_box_mut()
        .deposit(create_pokemon(Species::Charmander, 9, [0x9A, 0x78]).unwrap()).unwrap();
    save.pc_storage.current_box_mut()
        .deposit(create_pokemon(Species::Squirtle, 8, [0x9A, 0x78]).unwrap()).unwrap();
    save.pc_storage.get_box_mut(3).unwrap()
        .deposit(create_pokemon(Species::Abra, 6, [0x9A, 0x78]).unwrap()).unwrap();
    save.game_data.bag.add_item(ItemId::Potion, 5).unwrap();
    save.game_data.bag.add_item(ItemId::Bicycle, 1).unwrap();
    save.game_data.pc_items.add_item(ItemId::PokeBall, 3).unwrap();
    save.game_data.pc_items.add_item(ItemId::SuperPotion, 12).unwrap();
    for i in 1..=25u8 {
        let s = Species::from_index_id(i);
        save.game_data.pokedex.set_seen(s);
        save.game_data.pokedex.set_owned(s);
    }

    // Boot + main menu.
    {
        let mut save = save.clone();
        let mut pc = PcScreen::new(PcEntry::PokemonCenter, &open_ctx());
        shot(&pc, &save, "01_boot");
        skip_msg(&mut pc, &mut save);
        shot(&pc, &save, "02_main_menu");
        step(&mut pc, &mut save, A);
        shot(&pc, &save, "03_accessed_bills");
    }

    // Bill's menu + withdraw list + popup.
    {
        let (mut pc, mut save) = fresh_at_bills_menu(&save);
        shot(&pc, &save, "04_bills_menu");
        step(&mut pc, &mut save, A); // WITHDRAW
        shot(&pc, &save, "05_withdraw_list");
        step(&mut pc, &mut save, A);
        shot(&pc, &save, "06_withdraw_popup");
    }

    // Deposit flow.
    {
        let (mut pc, mut save) = fresh_at_bills_menu(&save);
        press(&mut pc, &mut save, DOWN, 1);
        step(&mut pc, &mut save, A); // DEPOSIT
        shot(&pc, &save, "07_deposit_list");
        step(&mut pc, &mut save, A);
        shot(&pc, &save, "08_deposit_popup");
        step(&mut pc, &mut save, A); // confirm
        shot(&pc, &save, "09_deposited_msg");
    }

    // Release confirm.
    {
        let (mut pc, mut save) = fresh_at_bills_menu(&save);
        press(&mut pc, &mut save, DOWN, 2);
        step(&mut pc, &mut save, A); // RELEASE
        step(&mut pc, &mut save, A); // pick mon
        shot(&pc, &save, "10_release_confirm");
    }

    // Change box.
    {
        let (mut pc, mut save) = fresh_at_bills_menu(&save);
        press(&mut pc, &mut save, DOWN, 3);
        step(&mut pc, &mut save, A); // CHANGE BOX
        shot(&pc, &save, "11_changebox_confirm");
        step(&mut pc, &mut save, UP); // YES
        step(&mut pc, &mut save, A);
        shot(&pc, &save, "12_box_list");
        press(&mut pc, &mut save, DOWN, 2);
        step(&mut pc, &mut save, A); // switch to Box 3
        assert_eq!(save.pc_storage.current_box_index(), 2);
        assert!(pc.take_save_request());
        shot(&pc, &save, "13_bills_menu_box3");
    }

    // Item PC.
    {
        let (mut pc, mut save) = fresh_at_main_menu(&save);
        press(&mut pc, &mut save, DOWN, 1);
        step(&mut pc, &mut save, A); // RED's PC
        skip_msg(&mut pc, &mut save);
        shot(&pc, &save, "14_item_menu");
        step(&mut pc, &mut save, A); // WITHDRAW ITEM
        shot(&pc, &save, "15_item_withdraw_list");
        step(&mut pc, &mut save, A); // pick POKE BALL
        press(&mut pc, &mut save, UP, 2);
        shot(&pc, &save, "16_quantity");
        step(&mut pc, &mut save, B); // back to list
        step(&mut pc, &mut save, B); // back to item menu
        press(&mut pc, &mut save, DOWN, 2);
        step(&mut pc, &mut save, A); // TOSS ITEM
        step(&mut pc, &mut save, A); // pick POKE BALL
        step(&mut pc, &mut save, A); // qty → confirm
        shot(&pc, &save, "17_toss_confirm");
    }

    // Oak's rating.
    {
        let (mut pc, mut save) = fresh_at_main_menu(&save);
        press(&mut pc, &mut save, DOWN, 2);
        step(&mut pc, &mut save, A); // PROF.OAK's PC
        skip_msg(&mut pc, &mut save);
        shot(&pc, &save, "18_oaks_confirm");
        step(&mut pc, &mut save, UP); // YES
        step(&mut pc, &mut save, A);
        shot(&pc, &save, "19_rating_page1");
        step(&mut pc, &mut save, A);
        shot(&pc, &save, "20_rating_page2");
        step(&mut pc, &mut save, A);
        shot(&pc, &save, "21_rating_text");
    }
    println!("done");
}

use super::*;
use pokered_core::party_screen::PartyScreenState;
use pokered_core::pokemon::stats::create_pokemon;
use pokered_data::species::Species;

fn battle() -> BattleScreen {
    let mut bench = create_pokemon(Species::Pikachu, 12, [0x9a, 0x78]).unwrap();
    bench.hp = 1;
    bench.status = CoreStatus::Paralysis;
    BattleScreen::from_parties(true, &[
        create_pokemon(Species::Bulbasaur, 20, [0x9a, 0x78]).unwrap(), bench,
    ], &[create_pokemon(Species::Rattata, 5, [0x9a, 0x78]).unwrap()], None)
}

#[test]
fn medicine_target_draws_the_full_live_party_instead_of_battle_hud() {
    for language in [Lang::En, Lang::Zh] {
        let mut screen = battle();
        screen.phase = BattlePhase::ItemTargetSelect { item_id: ItemId::Potion };
        for cursor in [0, 1] {
            screen.party_cursor = cursor;
            let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
            let mut actual = FrameBuffer::new(config, Rgba::BLACK);
            draw_battle(&screen, &mut None, &mut actual, &mut BattleVisualEffects::default(), language);
            let mut party = PartyScreenState::new(screen.battle_state.as_ref().unwrap().player.persistent_party());
            party.set_cursor(cursor);
            let mut expected = FrameBuffer::new(config, Rgba::BLACK);
            crate::render::menu::draw_party_screen_base(&party, None, 0, &mut expected, language);
            for y in 0..96 {
                for x in 0..160 {
                    assert_eq!(actual.get_pixel(x, y), expected.get_pixel(x, y), "party pixel {x},{y}, cursor {cursor}, {language:?}");
                }
            }
        }
    }
}

#[test]
fn medicine_result_renders_updated_bench_hp_and_original_transformed_identity() {
    for language in [Lang::En, Lang::Zh] {
        let mut screen = battle();
        screen.party_cursor = 1;
        screen.current_message = Some("PIKACHU\nrecovered by 20!".into());
        screen.phase = BattlePhase::ItemMedicineResult { wait_frames: 0 };
        let player = &mut screen.battle_state.as_mut().unwrap().player;
        player.preserve_original_identity();
        player.active_mon_mut().species = Species::Mewtwo;
        player.party[1].hp = 21;
        let canonical = serde_json::to_value(&screen.battle_state).unwrap();
        let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
        let mut actual = FrameBuffer::new(config, Rgba::BLACK);
        draw_battle(&screen, &mut None, &mut actual, &mut BattleVisualEffects::default(), language);
        let mut party = PartyScreenState::new(player_view(&screen));
        party.set_cursor(1);
        let mut expected = FrameBuffer::new(config, Rgba::BLACK);
        crate::render::menu::draw_party_screen_base(&party, None, 0, &mut expected, language);
        let mut painter = FrameBufferPainter::new(&mut expected).with_lang(language);
        menus::party::draw_notice(&mut Ui::new(&mut painter), screen.current_message.as_deref().unwrap());
        for y in 0..144 { for x in 0..160 {
            assert_eq!(actual.get_pixel(x, y), expected.get_pixel(x, y), "result {x},{y}");
        }}
        assert_eq!(serde_json::to_value(&screen.battle_state).unwrap(), canonical, "render is a read-only view");
    }
}

fn player_view(screen: &BattleScreen) -> Vec<pokered_core::battle::state::Pokemon> {
    screen.battle_state.as_ref().unwrap().player.persistent_party()
}

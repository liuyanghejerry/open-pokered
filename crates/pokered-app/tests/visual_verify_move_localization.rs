use dotzuki_engine::render_config::RenderConfig;
use pokered_app::render::{draw_battle, draw_stats_screen, BattleVisualEffects};
use pokered_core::{
    battle::{
        menu::{MoveMenuState, MoveSlot},
        BattlePhase, BattleScreen,
    },
    game_state::Lang,
    pokemon::stats::create_pokemon_with_moves,
    stats_screen::{StatsPage, StatsScreenState},
};
use pokered_data::{moves::MoveId, species::Species};
use pokered_renderer::{
    resource::{AssetRoot, ResourceManager},
    FrameBuffer, Rgba,
};

#[test]
fn capture_move_localization() {
    let out = std::env::var_os("MOVE_SHOTS").map(std::path::PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let moves = [
        MoveId::MegaPunch,
        MoveId::Thunderbolt,
        MoveId::PsychicM,
        MoveId::Bonemerang,
    ];
    let mon = create_pokemon_with_moves(Species::Bulbasaur, 30, [0xff, 0xff], moves).unwrap();
    let mut state = StatsScreenState::new(mon);
    state.page = StatsPage::Moves;
    let mut res = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
    for (lang, suffix) in [(Lang::Zh, "zh"), (Lang::En, "en")] {
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        draw_stats_screen(&state, res.as_mut(), &mut fb, lang);
        if lang == Lang::Zh {
            assert_move_ink(&fb, 16, 71, "百万吨重拳");
        }
        if let Some(out) = &out {
            fb.save_png(&out.join(format!("stats-{suffix}.png")))
                .unwrap();
        }
        let mut battle = BattleScreen::new(true);
        battle.phase = BattlePhase::MoveSelect;
        battle.move_menu = Some(MoveMenuState::new(
            moves
                .into_iter()
                .map(|move_id| MoveSlot {
                    move_id,
                    current_pp: 10,
                    max_pp: 20,
                    is_disabled: false,
                })
                .collect(),
        ));
        draw_battle(
            &battle,
            &mut res,
            &mut fb,
            &mut BattleVisualEffects::default(),
            lang,
        );
        if lang == Lang::Zh {
            assert_move_ink(&fb, 16, 95, "百万吨重拳");
        }
        if let Some(out) = &out {
            fb.save_png(&out.join(format!("battle-{suffix}.png")))
                .unwrap();
        }
    }
}

fn assert_move_ink(actual: &FrameBuffer, x: u32, y: u32, text: &str) {
    let mut expected = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    pokered_renderer::embedded_font::draw_text(text, x, y, Rgba::BLACK, &mut expected);
    for dy in 0..10 {
        for dx in 0..50 {
            assert_eq!(
                actual.get_pixel(x + dx, y + dy),
                expected.get_pixel(x + dx, y + dy),
                "localized name pixel ({dx}, {dy})"
            );
        }
    }
}

#[test]
fn stats_handles_long_chinese_species_names_on_both_pages() {
    let mon =
        create_pokemon_with_moves(Species::Bulbasaur, 40, [0xff, 0xff], [MoveId::Solarbeam; 4])
            .unwrap();
    let mut state = StatsScreenState::new(mon);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    for page in [StatsPage::Stats, StatsPage::Moves] {
        state.page = page;
        draw_stats_screen(&state, None, &mut fb, Lang::Zh);
    }
}

#[test]
fn every_chinese_move_name_has_renderable_glyphs() {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    for id in 1..=165 {
        let name = pokered_data::lang_data::move_name(MoveId::from_id(id), true);
        for ch in name.chars() {
            assert_eq!(
                pokered_renderer::embedded_font::draw_char(ch, 0, 0, Rgba::BLACK, &mut fb),
                10,
                "missing glyph {ch} in {name}"
            );
        }
    }
}

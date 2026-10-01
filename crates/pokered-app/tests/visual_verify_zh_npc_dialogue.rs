//! Deterministic real NPC-text captures and a full page dump on base/candidate.
#[path = "../../pokered-core/tests/common/zh_dialogue_corpus.rs"]
mod corpus;
use pokered_app::render::draw_overworld;
use pokered_core::{
    game_state::Lang,
    overworld::{script_bridge::text_to_dialogue, OverworldScreen},
};
use pokered_data::{dialogue_layout::contains_chinese, impl_traits::PokemonRedData, maps::MapId};
use pokered_renderer::{
    resource::{AssetRoot, ResourceManager},
    FrameBuffer, Rgba,
};

#[test]
#[ignore = "writes NPC captures and corpus dump to PR_SCREENSHOTS"]
fn capture_zh_npc_dialogue() {
    let output = std::path::PathBuf::from(std::env::var("PR_SCREENSHOTS").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    let cases = corpus::corpus();
    let mut dump = Vec::new();
    for case in &cases {
        let text = case
            .text
            .replace("<PLAYER>", "小智")
            .replace("<RIVAL>", "小茂")
            .replace("<STARTER>", "小火龙");
        if contains_chinese(&text) {
            let d = text_to_dialogue(&text);
            dump.push(serde_json::json!({"source":case.source,"text":text,"pages":d.pages()}));
        }
    }
    std::fs::write(
        output.join("pages.json"),
        serde_json::to_string_pretty(&dump).unwrap(),
    )
    .unwrap();
    let oak = cases
        .iter()
        .find(|c| c.source.starts_with("PalletTown:") && c.text.contains("你需要自"))
        .unwrap();
    let surge = cases
        .iter()
        .find(|c| c.source.starts_with("dialog_text::EXACT") && c.text.contains("告诉你，小子"))
        .unwrap();
    let daycare = cases
        .iter()
        .find(|c| c.source.starts_with("Daycare:") && c.text.contains("等级提升了"))
        .unwrap();
    let agatha = cases
        .iter()
        .find(|c| c.source.starts_with("AgathasRoom:") && c.text.contains("几十年前"))
        .unwrap();
    let mut resources = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
    let mut fb = FrameBuffer::new(
        dotzuki_engine::render_config::RenderConfig::new(160, 144),
        Rgba::WHITE,
    );
    for (name, case, map, page) in [
        ("oak-page-0", oak, MapId::PalletTown, 0),
        ("oak-page-1", oak, MapId::PalletTown, 1),
        ("surge-page-4", surge, MapId::VermilionGym, 4),
        ("daycare-page-0", daycare, MapId::Daycare, 0),
        ("agatha-page-3", agatha, MapId::AgathasRoom, 3),
    ] {
        let mut screen = OverworldScreen::new(map, None, PokemonRedData);
        screen.set_script_lang("zh");
        screen.frame_counter = 0; // fixed arrow phase and NPC/map frame
        let mut dialogue = text_to_dialogue(&case.text);
        for _ in 0..page {
            assert!(dialogue.advance());
        }
        dialogue.skip_to_full_page();
        screen.pending_dialogue = Some(dialogue);
        draw_overworld(&mut screen, &mut resources, &mut fb, Lang::Zh);
        fb.save_png(&output.join(format!("{name}.png"))).unwrap();
    }
}

#[path = "common/zh_dialogue_corpus.rs"]
mod corpus;
use pokered_core::overworld::script_bridge::text_to_dialogue_with_names;
use pokered_core::text::zh_dialogue::{no_line_end, no_line_start};
use pokered_data::dialogue_layout::{
    contains_chinese, measure_text, LINE_WIDTH_PX, SECOND_LINE_WIDTH_PX,
};

fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn every_chinese_npc_dialogue_fits_and_preserves_text() {
    let corpus = corpus::corpus();
    assert_eq!(
        corpus
            .iter()
            .filter(|c| c.source.contains("showRandomText"))
            .count(),
        10,
        "must include every Cerulean City and S.S. Anne random-dialogue option"
    );
    let mut count = 0;
    let mut report = Vec::new();
    for case in &corpus {
        for names in [
            ["RED", "BLUE", "CHARMANDER"],
            ["小智", "小茂", "小火龙"],
            ["王小明同学", "张小花同学", "妙蛙种子"],
            ["ABCDEFG", "HIJKLMN", "BULBASAUR"],
        ] {
            let text = case
                .text
                .replace("<PLAYER>", names[0])
                .replace("<RIVAL>", names[1])
                .replace("<STARTER>", names[2]);
            if !contains_chinese(&text) {
                continue;
            }
            count += 1;
            let d = text_to_dialogue_with_names(&text, &names);
            let shown = d
                .pages()
                .iter()
                .map(|p| format!("{}{}", p.line1, p.line2))
                .collect::<String>();
            assert_eq!(
                compact(&shown),
                compact(&text),
                "lost/reordered text: {}",
                case.source
            );
            let authored_leaders: Vec<String> = text
                .lines()
                .filter(|line| !line.trim().is_empty())
                .filter(|line| line.chars().next().is_some_and(no_line_start))
                .map(|line| line.chars().take_while(|&c| no_line_start(c)).collect())
                .collect();
            for (page, p) in d.pages().iter().enumerate() {
                assert!(
                    !p.line1.trim().is_empty(),
                    "blank page/first row: {}:{page}",
                    case.source
                );
                for (row, line, width) in [
                    (0, &p.line1, LINE_WIDTH_PX),
                    (1, &p.line2, SECOND_LINE_WIDTH_PX),
                ] {
                    assert!(
                        measure_text(line) as usize <= width,
                        "overflow {}:{page}/{row}: {line}",
                        case.source
                    );
                    assert!(
                        !line.chars().last().is_some_and(no_line_end),
                        "dangling opener {}:{page}/{row}: {line}",
                        case.source
                    );
                    if line.chars().next().is_some_and(no_line_start) {
                        assert!(
                            authored_leaders
                                .iter()
                                .any(|original| compact(line).starts_with(&compact(original))),
                            "new punctuation leader {}:{page}/{row}: {line}",
                            case.source
                        );
                    }
                }
            }
            // Game terms and player/rival substitutions must fit one row.
            for word in [
                "宝可梦",
                "训练家",
                "精灵球",
                "火箭队",
                "大木博士",
                "宝可梦中心",
                names[0],
                names[1],
            ] {
                if text.contains(word) {
                    let occurrences: usize = d
                        .pages()
                        .iter()
                        .flat_map(|p| [&p.line1, &p.line2])
                        .map(|line| compact(line).matches(word).count())
                        .sum();
                    assert_eq!(
                        occurrences,
                        compact(&text).matches(word).count(),
                        "split term {}: {word}",
                        case.source
                    );
                }
            }
            if names[0] == "小智" {
                report
                    .push(serde_json::json!({"source":case.source,"text":text,"pages":d.pages()}));
            }
        }
    }
    assert!(count > 8000, "corpus coverage unexpectedly shrank: {count}");
    if let Ok(path) = std::env::var("ZH_DIALOGUE_AUDIT") {
        std::fs::write(path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
    eprintln!(
        "Audited {count} Chinese dialogue/name variants from {} current scene/fallback records",
        corpus.len()
    );
}

#[test]
fn chinese_clerk_pages_finish_and_resume_the_shop_script() {
    use pokered_core::overworld::{Direction, OverworldInput, OverworldScreen};
    use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
    let mut screen = OverworldScreen::new(MapId::ViridianMart, None, PokemonRedData);
    screen.set_script_lang("zh");
    screen.set_flag_live("EVENT_GOT_OAKS_PARCEL", true);
    screen.set_flag_live("EVENT_OAK_GOT_PARCEL", true);
    screen.run_on_load();
    screen.state.player.x = 2;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Left;
    let mut saw_chinese = false;
    for frame in 0..600 {
        screen.update_frame(OverworldInput::new(
            false,
            false,
            false,
            false,
            frame % 10 == 0,
            false,
            false,
            false,
        ));
        if let Some(d) = &screen.pending_dialogue {
            saw_chinese |= d.pages().iter().any(|p| p.line1.contains("你好"));
            assert!(d.pages().iter().all(|p| !p.line1.is_empty()));
        }
        if screen.pending_shop.is_some() {
            assert!(saw_chinese, "must exercise actual Chinese dialogue");
            assert!(screen.pending_dialogue.is_none());
            return;
        }
    }
    panic!("clerk did not resume after Chinese pages");
}

#[test]
fn empty_clipboard_dialogue_never_opens_a_blank_box() {
    use pokered_core::overworld::{Direction, OverworldInput, OverworldScreen};
    use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
    let mut screen = OverworldScreen::new(MapId::MtMoonPokecenter, None, PokemonRedData);
    screen.set_script_lang("zh");
    screen.run_on_load();
    screen.state.player.x = 6;
    screen.state.player.y = 2;
    screen.state.player.facing = Direction::Right;
    let mut saw_clipboard_effect = false;
    for frame in 0..30 {
        screen.update_frame(OverworldInput::new(
            false,
            false,
            false,
            false,
            frame == 0,
            false,
            false,
            false,
        ));
        saw_clipboard_effect |=
            screen.active_script_effect_label().as_deref() == Some("ShowDialogue");
        assert!(
            screen.pending_dialogue.is_none(),
            "blank box at frame {frame}"
        );
    }
    assert!(screen.active_script_effect_label().is_none());
    assert!(screen.script_engine_idle());
    assert!(
        saw_clipboard_effect,
        "must exercise the clipboard's empty ShowDialogue"
    );
}

#[test]
fn prepared_pages_survive_a_snapshot_mid_typewriter() {
    use pokered_core::overworld::BedroomDialogue;
    let mut original = text_to_dialogue_with_names(
        "大木博士：外面很危险！野生的宝可梦生活在草丛中！\n\n你需要自己的宝可梦来保护自己。我知道了！来，跟我来！",
        &[],
    );
    original.set_text_delay_frames(3);
    for _ in 0..17 {
        original.reveal_next_char();
    }
    let mut restored: BedroomDialogue =
        serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
    while !original.is_done() {
        assert_eq!(original.get_display_text(), restored.get_display_text());
        assert_eq!(original.current_page(), restored.current_page());
        assert_eq!(original.waiting_for_input(), restored.waiting_for_input());
        if original.waiting_for_input() {
            assert_eq!(original.advance(), restored.advance());
        } else {
            original.reveal_next_char();
            restored.reveal_next_char();
        }
    }
    assert!(restored.is_done());
}

#[test]
fn daycare_runtime_nickname_stays_on_one_row() {
    use pokered_core::overworld::{Direction, OverworldInput, OverworldScreen};
    use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
    let mut screen = OverworldScreen::new(MapId::Daycare, None, PokemonRedData);
    screen.set_script_lang("zh");
    screen.run_on_load();
    let nickname = "阿尔法贝塔伽马";
    screen.seed_daycare_query_state(true, nickname, 0, 100, &[], &[]);
    screen.state.player.x = 3;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Left;
    for frame in 0..40 {
        screen.update_frame(OverworldInput::new(
            false,
            false,
            false,
            false,
            frame == 0,
            false,
            false,
            false,
        ));
        if let Some(d) = &screen.pending_dialogue {
            assert!(
                d.pages()
                    .iter()
                    .any(|p| p.line1.contains(nickname) || p.line2.contains(nickname)),
                "runtime nickname split: {:?}",
                d.pages()
            );
            return;
        }
    }
    panic!("did not reach the actual daycare dialogue");
}
